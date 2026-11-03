/* core/qnn.h through QNN's C API. The graph is core/encoder.c's arithmetic
 * written as QNN ops, with the HTP's layout: the subsampling's images
 * [1][time][frequency][channels], as encoder.c's rows already are, and
 * then the stream [T][d_model]. Each matrix is a FullyConnected op on the
 * model's own int8 weights, a scale an output row as the .pkf has them,
 * the rows unpacked from the file's panels (the NPU takes its activations
 * in fp16 against them); everything else a float tensor, which the HTP
 * holds and computes in fp16.
 *
 * The HTP runs fixed shapes, so the graph takes one length, padded with
 * zeros, and four masks for the input's true length (made in
 * pk_qnn_encode): as tools/ane/encoder.py does for the Neural Engine, the
 * subsampling zeroes the frames past each stage's end before the next
 * stage's convolution sees them, the attention gives the keys past the end
 * no weight, and the convolution module zeroes them before its depthwise
 * convolution, so every real frame is what the unpadded input gives (up
 * to fp16). And as there:
 *
 *  - the residual stream is carried divided by PK_QNN_SCALE, which fp16
 *    holds where the stream's own size (about 300 inside a layer, squared
 *    in a norm's variance) it may not: each block's last matrix and each
 *    layer's output norm scaled to match, and each norm's epsilon by the
 *    scale's square, which leaves every norm's output what it was;
 *  - the attention's position term is the query turned rather than the
 *    scores shifted: in each (sin, cos) pair of frequency w, the encoding
 *    of position i - j is that of j turned through i w, so
 *        (q_i + v) . W_pos pe(i - j) = qr_i . pe(j)
 *    with qr_i = R(i)^T W_pos^T (q_i + v) (tools/ane/encoder.py's
 *    docstring has the algebra), and pe(j) less its mean over j, which the
 *    softmax doesn't see and fp16 holds nearer. Here a head at a time: one
 *    product of the head's q_i + v with W_pos^T and with it again with each
 *    (sin, cos) pair of columns swapped, [T][2 d_model]; the turn's tables
 *    multiplied in; one product with pe twice over, which adds the halves
 *    as it goes. For the eight heads together, as the Neural Engine has
 *    it, the same arithmetic took half the NPU's time, and so a head at a
 *    time saves a third of it (tools/qnn/CLIMB.md, 2026-11-02);
 *  - the attention's 1/sqrt(dk) is folded into K's scales and into pe.
 */
#include <assert.h>              /* the HTP headers' static_assert */
#include <dlfcn.h>
#include <math.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "HTP/QnnHtpDevice.h"
#include "HTP/QnnHtpGraph.h"
#include "HTP/QnnHtpPerfInfrastructure.h"
#include "QnnInterface.h"
#include "QnnOpDef.h"

#include "model.h"
#include "pkf.h"
#include "qnn.h"

#define PK_QNN_SCALE 8.0f        /* tools/ane/encoder.py's --scale: a power of two, exact */
#define PK_QNN_FRAMES 3072       /* 30.72 s */

typedef Qnn_ErrorHandle_t (*get_providers_fn)(const QnnInterface_t ***, uint32_t *);

struct pk_qnn {
    void *lib;
    QNN_INTERFACE_VER_TYPE q;
    Qnn_LogHandle_t log;
    Qnn_BackendHandle_t backend;
    Qnn_DeviceHandle_t device;
    Qnn_ContextHandle_t context;
    Qnn_GraphHandle_t graph;
    uint32_t n_mels, d, T0, T1, T2, T3;
    uint32_t layers;             /* the encoder's layers it runs: all, but for checking */
    Qnn_Tensor_t in[5], out;     /* mel, the three stages' masks, the keys' bias; x */
    float *buf[5], *x;           /* their data */
    char names[6][16];           /* in's and out's names and shapes, their own */
    uint32_t dims[6][4];
};

/* in[0..4] then out: the graph's inputs and output */
static Qnn_Tensor_t *io(pk_qnn *e, int k) { return k < 5 ? &e->in[k] : &e->out; }

/* the inputs' and output's names and shapes copied into e, from the
 * builder's memory, which goes when the graph is compiled */
static void own_io(pk_qnn *e) {
    for (int k = 0; k < 6; k++) {
        Qnn_Tensor_t *t = io(e, k);
        snprintf(e->names[k], sizeof e->names[k], "%s", t->v1.name ? t->v1.name : "");
        memcpy(e->dims[k], t->v1.dimensions, t->v1.rank * sizeof(uint32_t));
        t->v1.name = e->names[k];
        t->v1.dimensions = e->dims[k];
    }
}

/* ------------------------------------------------------------ logging --- */

/* QNN's messages: the last error kept for err, and every message at
 * PKTITO_QNN_LOG's level or below on stderr (1 errors, 2 warnings, 3
 * info, 4 verbose) */
static char last_error[256];

static void log_cb(const char *fmt, QnnLog_Level_t level, uint64_t ts, va_list ap) {
    (void)ts;
    char line[512];
    vsnprintf(line, sizeof line, fmt, ap);
    if (level == QNN_LOG_LEVEL_ERROR)
        snprintf(last_error, sizeof last_error, "%s", line);
    const char *v = getenv("PKTITO_QNN_LOG");
    if (v && (int)level <= atoi(v))
        fprintf(stderr, "qnn: %s%s", line, line[0] && line[strlen(line) - 1] == '\n' ? "" : "\n");
}

static int fail(char *err, size_t errlen, const char *what, Qnn_ErrorHandle_t e) {
    if (err && errlen)
        snprintf(err, errlen, "%s failed (QNN error %lu)%s%s", what, (unsigned long)(e & 0xffff),
                 last_error[0] ? ": " : "", last_error);
    return -1;
}

/* ---------------------------------------------------------- profiling --- */

/* PKTITO_QNN_PROFILE=1: QNN's detailed profile of compiling the graph and
 * of each run, an event a line on stderr - "qnn-prof", its depth among the
 * events, its type, unit (1 us, 3 cycles), value and name - the ops named
 * for their block and type (op_name). (The HTP's own linting level crashed
 * the NPU's process on this graph: tools/qnn/CLIMB.md, 2026-11-02.) */
static Qnn_ProfileHandle_t profile(const pk_qnn *e) {
    Qnn_ProfileHandle_t p = NULL;
    if (getenv("PKTITO_QNN_PROFILE") && e->q.profileCreate(e->backend, QNN_PROFILE_LEVEL_DETAILED, &p))
        p = NULL;
    return p;
}

static void print_events(const pk_qnn *e, const QnnProfile_EventId_t *ev, uint32_t n, int depth) {
    for (uint32_t i = 0; i < n; i++) {
        QnnProfile_EventData_t d = QNN_PROFILE_EVENT_DATA_INIT;
        const QnnProfile_EventId_t *sub = NULL;
        uint32_t nsub = 0;
        if (e->q.profileGetEventData(ev[i], &d))
            continue;
        fprintf(stderr, "qnn-prof %d %u %u %llu %s\n", depth, (unsigned)d.type, (unsigned)d.unit,
                (unsigned long long)d.value, d.identifier ? d.identifier : "");
        if (!e->q.profileGetSubEvents(ev[i], &sub, &nsub))
            print_events(e, sub, nsub, depth + 1);
    }
}

static void profile_done(const pk_qnn *e, Qnn_ProfileHandle_t p) {
    const QnnProfile_EventId_t *ev = NULL;
    uint32_t n = 0;
    if (!p)
        return;
    if (!e->q.profileGetEvents(p, &ev, &n))
        print_events(e, ev, n, 0);
    e->q.profileFree(p);
}

/* -------------------------------------------------------- the builder --- */

/* A tensor of the graph being made, and its shape */
typedef struct {
    Qnn_Tensor_t t;
    uint32_t rank, d[4];
} val;

/* What making the graph holds until QNN has compiled it: every name,
 * shape and array made here, freed together after */
typedef struct {
    pk_qnn *e;
    void **mem;
    size_t nmem, cap;
    uint32_t count;              /* tensors and ops made: their names */
    const char *block;           /* the block whose ops are being made, in their names */
    Qnn_ErrorHandle_t status;    /* the first failure, and its call */
    const char *failed;
} build;

static void *keep(build *b, size_t bytes) {
    if (b->nmem == b->cap) {
        size_t cap = b->cap ? 2 * b->cap : 256;
        void **mem = realloc(b->mem, cap * sizeof *mem);
        if (!mem)
            return NULL;
        b->mem = mem;
        b->cap = cap;
    }
    void *p = calloc(1, bytes ? bytes : 1);
    if (p)
        b->mem[b->nmem++] = p;
    else if (!b->status)
        b->status = QNN_COMMON_ERROR_MEM_ALLOC, b->failed = "memory";
    return p;
}

static void build_free(build *b) {
    for (size_t i = 0; i < b->nmem; i++)
        free(b->mem[i]);
    free(b->mem);
}

static char *name(build *b) {
    char *s = keep(b, 16);
    if (s)
        snprintf(s, 16, "t%u", b->count++);
    return s;
}

/* an op's name: its block, its type and a number, which QNN's profile
 * reports it by (PKTITO_QNN_PROFILE) */
static char *op_name(build *b, const char *type) {
    char *s = keep(b, 48);
    if (s)
        snprintf(s, 48, "%s_%s_%u", b->block, type, b->count++);
    return s;
}

/* A tensor of type and data type dt, of rank and shape d; data its
 * contents for a static one, which must last until the graph is compiled */
static val tensor(build *b, Qnn_TensorType_t type, Qnn_DataType_t dt, uint32_t rank, const uint32_t *d,
                  const void *data, size_t bytes) {
    val v = { QNN_TENSOR_INIT, rank, { 0 } };
    memcpy(v.d, d, rank * sizeof *d);
    uint32_t *dims = keep(b, rank * sizeof *dims);
    char *nm = name(b);
    if (b->status)
        return v;
    memcpy(dims, d, rank * sizeof *dims);
    v.t.v1.name = nm;
    v.t.v1.type = type;
    v.t.v1.dataType = dt;
    v.t.v1.rank = rank;
    v.t.v1.dimensions = dims;
    v.t.v1.memType = QNN_TENSORMEMTYPE_RAW;
    if (data) {
        v.t.v1.clientBuf.data = (void *)data;     /* QNN only reads it */
        v.t.v1.clientBuf.dataSize = (uint32_t)bytes;
    }
    Qnn_ErrorHandle_t e = b->e->q.tensorCreateGraphTensor(b->e->graph, &v.t);
    if (e && !b->status)
        b->status = e, b->failed = "tensorCreateGraphTensor";
    return v;
}

static size_t count(uint32_t rank, const uint32_t *d) {
    size_t n = 1;
    for (uint32_t i = 0; i < rank; i++)
        n *= d[i];
    return n;
}

/* an intermediate float tensor */
static val native(build *b, uint32_t rank, const uint32_t *d) {
    return tensor(b, QNN_TENSOR_TYPE_NATIVE, QNN_DATATYPE_FLOAT_32, rank, d, NULL, 0);
}

/* a float constant: data lasts until the graph is compiled (the model's
 * own, or kept) */
static val constant(build *b, uint32_t rank, const uint32_t *d, const float *data) {
    return tensor(b, QNN_TENSOR_TYPE_STATIC, QNN_DATATYPE_FLOAT_32, rank, d, data,
                  count(rank, d) * sizeof(float));
}

/* n floats of the model's, times k, as a constant of rank 1 */
static val scaled(build *b, const float *x, uint32_t n, float k) {
    float *c = keep(b, n * sizeof *c);
    if (c)
        for (uint32_t i = 0; i < n; i++)
            c[i] = x[i] * k;
    return constant(b, 1, &n, c ? c : x);
}

/* Element (r, k) of a model matrix: the .pkf keeps them in panels of 16
 * rows, 4 inputs of each row together (core/pkf.h, PK_Q8), the shape
 * SDOT multiplies; QNN takes them a row at a time */
static int8_t q8_at(const pk_q8 *w, uint32_t r, uint32_t k) {
    return w->q[((size_t)(r / PK_QR) * (w->cols / PK_QG) + k / PK_QG) * (PK_QR * PK_QG) +
                (r % PK_QR) * PK_QG + k % PK_QG];
}

/* rows [r0, r0 + n) of a model matrix as int8 weights [n][cols], row-major,
 * row r's scale its own times k */
static val weights(build *b, const pk_q8 *w, uint32_t r0, uint32_t n, float k) {
    val v = { QNN_TENSOR_INIT, 2, { n, w->cols } };
    uint32_t *dims = keep(b, 2 * sizeof *dims);
    Qnn_ScaleOffset_t *so = keep(b, n * sizeof *so);
    int8_t *q = keep(b, (size_t)n * w->cols);
    char *nm = name(b);
    if (b->status)
        return v;
    dims[0] = n;
    dims[1] = w->cols;
    for (uint32_t i = 0; i < n; i++) {
        so[i] = (Qnn_ScaleOffset_t){ w->s[r0 + i] * k, 0 };
        for (uint32_t c = 0; c < w->cols; c++)
            q[(size_t)i * w->cols + c] = q8_at(w, r0 + i, c);
    }
    v.t.v1.name = nm;
    v.t.v1.type = QNN_TENSOR_TYPE_STATIC;
    v.t.v1.dataType = QNN_DATATYPE_SFIXED_POINT_8;
    v.t.v1.rank = 2;
    v.t.v1.dimensions = dims;
    v.t.v1.memType = QNN_TENSORMEMTYPE_RAW;
    v.t.v1.clientBuf.data = q;
    v.t.v1.clientBuf.dataSize = n * w->cols;
    v.t.v1.quantizeParams.encodingDefinition = QNN_DEFINITION_DEFINED;
    v.t.v1.quantizeParams.quantizationEncoding = QNN_QUANTIZATION_ENCODING_AXIS_SCALE_OFFSET;
    v.t.v1.quantizeParams.axisScaleOffsetEncoding.axis = 0;
    v.t.v1.quantizeParams.axisScaleOffsetEncoding.numScaleOffsets = n;
    v.t.v1.quantizeParams.axisScaleOffsetEncoding.scaleOffset = so;
    Qnn_ErrorHandle_t e = b->e->q.tensorCreateGraphTensor(b->e->graph, &v.t);
    if (e && !b->status)
        b->status = e, b->failed = "tensorCreateGraphTensor (int8 weights)";
    return v;
}

static Qnn_Param_t scalar_bool(const char *nm, int v) {
    Qnn_Param_t p = { .paramType = QNN_PARAMTYPE_SCALAR, .name = nm };
    p.scalarParam.dataType = QNN_DATATYPE_BOOL_8;
    p.scalarParam.bool8Value = (uint8_t)(v != 0);
    return p;
}

static Qnn_Param_t scalar_f32(const char *nm, float v) {
    Qnn_Param_t p = { .paramType = QNN_PARAMTYPE_SCALAR, .name = nm };
    p.scalarParam.dataType = QNN_DATATYPE_FLOAT_32;
    p.scalarParam.floatValue = v;
    return p;
}

/* a parameter of 32-bit integers of type dt in the shape d of the given
 * rank */
static Qnn_Param_t int_param(build *b, const char *nm, Qnn_DataType_t dt, uint32_t rank, const uint32_t *d,
                             const void *v) {
    const size_t n = count(rank, d);
    uint32_t *c = keep(b, n * sizeof *c);
    if (c)
        memcpy(c, v, n * sizeof *c);
    val t = tensor(b, QNN_TENSOR_TYPE_STATIC, dt, rank, d, c, n * sizeof *c);
    Qnn_Param_t p = { .paramType = QNN_PARAMTYPE_TENSOR, .name = nm };
    p.tensorParam = t.t;
    return p;
}

static Qnn_Param_t ints(build *b, const char *nm, uint32_t rank, const uint32_t *d, const uint32_t *v) {
    return int_param(b, nm, QNN_DATATYPE_UINT_32, rank, d, v);
}

static Qnn_Param_t scalar_u32(const char *nm, uint32_t v) {
    Qnn_Param_t p = { .paramType = QNN_PARAMTYPE_SCALAR, .name = nm };
    p.scalarParam.dataType = QNN_DATATYPE_UINT_32;
    p.scalarParam.uint32Value = v;
    return p;
}

/* an op of QNN's own package on the inputs, into out */
static void node(build *b, const char *type, const val *in, uint32_t nin, const val *out,
                 Qnn_Param_t *par, uint32_t npar) {
    Qnn_Tensor_t ins[16], outs[1] = { out->t };
    for (uint32_t i = 0; i < nin; i++)
        ins[i] = in[i].t;
    char *nm = op_name(b, type);
    if (b->status)
        return;
    Qnn_OpConfig_t op = QNN_OPCONFIG_INIT;
    op.v1.name = nm;
    op.v1.packageName = QNN_OP_PACKAGE_NAME_QTI_AISW;
    op.v1.typeName = type;
    op.v1.numOfParams = npar;
    op.v1.params = par;
    op.v1.numOfInputs = nin;
    op.v1.inputTensors = ins;
    op.v1.numOfOutputs = 1;
    op.v1.outputTensors = outs;
    Qnn_ErrorHandle_t e = b->e->q.graphAddNode(b->e->graph, op);
    if (e && !b->status)
        b->status = e, b->failed = type;
}

/* ------------------------------------------------------------- the ops --- */

/* x [..][in] W^T, plus bias where there is one: [..][rows] */
static val fc(build *b, val x, const pk_q8 *w, uint32_t r0, uint32_t rows, float k, const val *bias) {
    val in[3] = { x, weights(b, w, r0, rows, k) };
    if (bias)
        in[2] = *bias;
    uint32_t d[2] = { (uint32_t)(count(x.rank, x.d) / w->cols), rows };
    val y = native(b, 2, d);
    node(b, QNN_OP_FULLY_CONNECTED, in, bias ? 3 : 2, &y, NULL, 0);
    return y;
}

/* x [..][in] W^T for a float constant W [out][in]: [..][out] */
static val fc_const(build *b, val x, val w) {
    const uint32_t d[2] = { (uint32_t)(count(x.rank, x.d) / w.d[1]), w.d[0] };
    val y = native(b, 2, d);
    val in[2] = { x, w };
    node(b, QNN_OP_FULLY_CONNECTED, in, 2, &y, NULL, 0);
    return y;
}

/* an element-wise op of two, broadcast to the first's shape */
static val binary(build *b, const char *type, val x, val y) {
    val z = native(b, x.rank, x.d);
    val in[2] = { x, y };
    node(b, type, in, 2, &z, NULL, 0);
    return z;
}

static val add(build *b, val x, val y) { return binary(b, QNN_OP_ELEMENT_WISE_ADD, x, y); }
static val mul(build *b, val x, val y) { return binary(b, QNN_OP_ELEMENT_WISE_MULTIPLY, x, y); }

static val sigmoid(build *b, val x) {
    val y = native(b, x.rank, x.d);
    node(b, QNN_OP_SIGMOID, &x, 1, &y, NULL, 0);
    return y;
}

static val silu(build *b, val x) { return mul(b, x, sigmoid(b, x)); }

static val relu(build *b, val x) {
    val y = native(b, x.rank, x.d);
    node(b, QNN_OP_RELU, &x, 1, &y, NULL, 0);
    return y;
}

static val reshape(build *b, val x, uint32_t rank, const uint32_t *d) {
    val y = native(b, rank, d);
    node(b, QNN_OP_RESHAPE, &x, 1, &y, NULL, 0);
    return y;
}

/* rank 3, axes 0 and 1 swapped */
static val swap01(build *b, val x) {
    const uint32_t d[3] = { x.d[1], x.d[0], x.d[2] }, perm[3] = { 1, 0, 2 }, n = 3;
    val y = native(b, 3, d);
    Qnn_Param_t p = ints(b, QNN_OP_TRANSPOSE_PARAM_PERM, 1, &n, perm);
    node(b, QNN_OP_TRANSPOSE, &x, 1, &y, &p, 1);
    return y;
}

/* x [T][D] as [H][T][dk] */
static val heads(build *b, val x, uint32_t H) {
    const uint32_t d[3] = { x.d[0], H, x.d[1] / H };
    return swap01(b, reshape(b, x, 3, d));
}

/* and back */
static val unheads(build *b, val x) {
    val t = swap01(b, x);
    const uint32_t d[2] = { t.d[0], t.d[1] * t.d[2] };
    return reshape(b, t, 2, d);
}

/* x [lo[0], hi[0]) [lo[1], hi[1]) .. on each of its axes */
static val slice(build *b, val x, const uint32_t *lo, const uint32_t *hi) {
    uint32_t d[4], dr[2] = { x.rank, 3 };
    int32_t r[12];
    for (uint32_t i = 0; i < x.rank; i++) {
        d[i] = hi[i] - lo[i];
        r[3 * i] = (int32_t)lo[i], r[3 * i + 1] = (int32_t)hi[i], r[3 * i + 2] = 1;
    }
    val y = native(b, x.rank, d);
    Qnn_Param_t p = int_param(b, QNN_OP_STRIDED_SLICE_PARAM_RANGES, QNN_DATATYPE_INT_32, 2, dr, r);
    node(b, QNN_OP_STRIDED_SLICE, &x, 1, &y, &p, 1);
    return y;
}

/* n tensors of one shape but on axis, end to end along it */
static val concat(build *b, const val *x, uint32_t n, uint32_t axis) {
    uint32_t d[4];
    memcpy(d, x[0].d, sizeof d);
    d[axis] = 0;
    for (uint32_t i = 0; i < n; i++)
        d[axis] += x[i].d[axis];
    val y = native(b, x[0].rank, d);
    Qnn_Param_t p = scalar_u32(QNN_OP_CONCAT_PARAM_AXIS, axis);
    node(b, QNN_OP_CONCAT, x, n, &y, &p, 1);
    return y;
}

/* x [..][m][k] times y [..][k][n], or y [..][n][k] with ty: [..][m][n] */
static val matmul(build *b, val x, val y, int ty) {
    val z = { QNN_TENSOR_INIT, x.rank, { 0 } };
    memcpy(z.d, x.d, sizeof z.d);
    z.d[x.rank - 1] = ty ? y.d[y.rank - 2] : y.d[y.rank - 1];
    z = native(b, z.rank, z.d);
    Qnn_Param_t p[2] = { scalar_bool(QNN_OP_MAT_MUL_PARAM_TRANSPOSE_IN0, 0),
                         scalar_bool(QNN_OP_MAT_MUL_PARAM_TRANSPOSE_IN1, ty) };
    val in[2] = { x, y };
    node(b, QNN_OP_MAT_MUL, in, 2, &z, p, 2);
    return z;
}

static val softmax(build *b, val x) {
    val y = native(b, x.rank, x.d);
    node(b, QNN_OP_SOFTMAX, &x, 1, &y, NULL, 0);
    return y;
}

/* over the last axis of x [T][D]: gamma and beta the model's times k, into
 * a tensor of type (the graph's output, for the last) */
static val norm(build *b, val x, const float *w, const float *bb, float k, float eps, Qnn_TensorType_t type) {
    const uint32_t D = x.d[x.rank - 1], axis = x.rank - 1, one = 1;
    val in[3] = { x, scaled(b, w, D, k), scaled(b, bb, D, k) };
    val y = tensor(b, type, QNN_DATATYPE_FLOAT_32, x.rank, x.d, NULL, 0);
    Qnn_Param_t p[2] = { scalar_f32(QNN_OP_LAYER_NORM_PARAM_EPSILON, eps),
                         ints(b, QNN_OP_LAYER_NORM_PARAM_AXES, 1, &one, &axis) };
    node(b, QNN_OP_LAYER_NORM, in, 3, &y, p, 2);
    return y;
}

/* A convolution over [1][H][W][C]: filter [kh][kw][1 or C][Co] and bias,
 * stride s both ways, padding ph before and after along H and pw along W,
 * depthwise or not */
static val conv(build *b, val x, val filt, val bias, uint32_t s, uint32_t ph, uint32_t pw, int depthwise) {
    const uint32_t kh = filt.d[0], kw = filt.d[1];
    const uint32_t d[4] = { 1, (x.d[1] + 2 * ph - kh) / s + 1, (x.d[2] + 2 * pw - kw) / s + 1, filt.d[3] };
    val y = native(b, 4, d);
    const uint32_t two = 2, two2[2] = { 2, 2 }, stride[2] = { s, s }, pad[4] = { ph, ph, pw, pw };
    Qnn_Param_t p[2] = { ints(b, depthwise ? QNN_OP_DEPTH_WISE_CONV_2D_PARAM_STRIDE : QNN_OP_CONV_2D_PARAM_STRIDE,
                              1, &two, stride),
                         ints(b, depthwise ? QNN_OP_DEPTH_WISE_CONV_2D_PARAM_PAD_AMOUNT
                                           : QNN_OP_CONV_2D_PARAM_PAD_AMOUNT, 2, two2, pad) };
    val in[3] = { x, filt, bias };
    node(b, depthwise ? QNN_OP_DEPTH_WISE_CONV_2D : QNN_OP_CONV_2D, in, 3, &y, p, 2);
    return y;
}

/* reference's 3x3 taps, [C][9] at 3 * (time offset) + (frequency
 * offset), as a filter [3 time][3 frequency][1][C] */
static val taps3(build *b, const float *w, uint32_t C) {
    float *f = keep(b, 9 * (size_t)C * sizeof *f);
    if (f)
        for (uint32_t c = 0; c < C; c++)
            for (uint32_t k = 0; k < 9; k++)
                f[(size_t)k * C + c] = w[(size_t)c * 9 + k];
    const uint32_t d[4] = { 3, 3, 1, C };
    return constant(b, 4, d, f ? f : w);
}

/* ------------------------------------------------------------- the graph --- */

typedef struct {
    val cs, pe;                  /* the turn's tables, [T3][2 D] each */
    val m3, kbias;               /* [T3][1], [1][1][T3] */
    float s, eps;                /* the stream's scale, and the norms' epsilon for it */
} common;

/* x + ff(LN x): up, SiLU, down (its 0.5 in down's weights) */
static val ff(build *b, const common *c, val x, const float *nw, const float *nb, const pk_q8 *up,
              const pk_q8 *down) {
    val h = fc(b, norm(b, x, nw, nb, 1, c->eps, QNN_TENSOR_TYPE_NATIVE), up, 0, up->rows, 1, NULL);
    return add(b, x, fc(b, silu(b, h), down, 0, down->rows, c->s, NULL));
}

/* The turn's first product for head h, as the weights of a matrix [2 D][dk]:
 * row e of W_pos^T's head h, the model's matrix as floats, and below them
 * the same rows with each (sin, cos) pair of columns swapped */
static val pos_turn(build *b, const pk_q8 *w, uint32_t h, uint32_t dk) {
    const uint32_t D = w->cols;
    float *f = keep(b, (size_t)2 * D * dk * sizeof *f);
    if (f)
        for (uint32_t e = 0; e < 2 * D; e++)
            for (uint32_t k = 0; k < dk; k++) {
                const uint32_t r = h * dk + k;
                f[(size_t)e * dk + k] = q8_at(w, r, e < D ? e : (e - D) ^ 1) * w->s[r];
            }
    const uint32_t d[2] = { 2 * D, dk };
    return constant(b, 2, d, f);
}

static val layer(build *b, const pk_model *m, const common *c, const pk_layer *L, val x, int last) {
    const uint32_t D = m->d_model, H = m->n_heads, dk = D / H, T = x.d[0], K = m->conv_kernel;
    const float sk = 1.0f / sqrtf((float)dk);

    b->block = "ff1";
    x = ff(b, c, x, L->ff1_nw, L->ff1_nb, &L->ff1_up, &L->ff1_down);

    /* attention with relative positions */
    b->block = "att";
    val n = norm(b, x, L->att_nw, L->att_nb, 1, c->eps, QNN_TENSOR_TYPE_NATIVE);
    val Q = fc(b, n, &L->q, 0, D, 1, NULL);
    val Kh = heads(b, fc(b, n, &L->k, 0, D, sk, NULL), H);       /* 1/sqrt(dk) in K */
    val Vh = heads(b, fc(b, n, &L->v, 0, D, 1, NULL), H);
    val qu = heads(b, add(b, Q, constant(b, 1, &D, L->bias_u)), H);
    val qv = add(b, Q, constant(b, 1, &D, L->bias_v));
    val ac = matmul(b, qu, Kh, 1);                                 /* [H][T][T] */
    b->block = "pos";
    val bdh[16];
    for (uint32_t h = 0; h < H; h++) {
        /* head h's columns of q + v, turned (two products with W_pos as
         * one, then the tables) and multiplied by pe */
        const uint32_t lo[2] = { 0, h * dk }, hi[2] = { T, (h + 1) * dk }, d3[3] = { 1, T, T };
        val u = fc_const(b, slice(b, qv, lo, hi), pos_turn(b, &L->pos, h, dk));     /* [T][2D] */
        bdh[h] = reshape(b, fc_const(b, mul(b, u, c->cs), c->pe), 3, d3);           /* [1][T][T] */
    }
    val bd = concat(b, bdh, H, 0);                                 /* 1/sqrt(dk) in pe */
    b->block = "att";
    val p = softmax(b, add(b, add(b, ac, bd), c->kbias));
    val o = unheads(b, matmul(b, p, Vh, 0));                       /* [T][D] */
    x = add(b, x, fc(b, o, &L->o, 0, D, c->s, NULL));

    /* convolution module: GLU, the padding zeroed, depthwise K, SiLU */
    b->block = "conv";
    n = norm(b, x, L->conv_nw, L->conv_nb, 1, c->eps, QNN_TENSOR_TYPE_NATIVE);
    val g = mul(b, mul(b, fc(b, n, &L->pw1, 0, D, 1, NULL), sigmoid(b, fc(b, n, &L->pw1, D, D, 1, NULL))), c->m3);
    const uint32_t d4[4] = { 1, T, 1, D }, d2[2] = { T, D }, fd[4] = { K, 1, 1, D };
    float *f = keep(b, (size_t)K * D * sizeof *f);
    if (f)
        for (uint32_t ch = 0; ch < D; ch++)
            for (uint32_t j = 0; j < K; j++)
                f[(size_t)j * D + ch] = L->dw[(size_t)ch * K + j];
    g = conv(b, reshape(b, g, 4, d4), constant(b, 4, fd, f), constant(b, 1, &D, L->dw_b), 1, K / 2, 0, 1);
    g = reshape(b, silu(b, g), 2, d2);
    x = add(b, x, fc(b, g, &L->pw2, 0, D, c->s, NULL));

    b->block = "ff2";
    x = ff(b, c, x, L->ff2_nw, L->ff2_nb, &L->ff2_up, &L->ff2_down);
    /* the last layer's norm unscaled: the encoder's own output */
    b->block = "out";
    return norm(b, x, L->out_nw, L->out_nb, last ? 1 : c->s, c->eps,
                last ? QNN_TENSOR_TYPE_APP_READ : QNN_TENSOR_TYPE_NATIVE);
}

/* The turn's tables for positions 0..T-1, [T][2 D] each, in double,
 * rounded once: (-cos, cos) of j w for each pair of frequency w, then
 * (sin, sin); pe(j) less its mean over j, times 1/sqrt(dk), twice */
static void tables(const pk_model *m, uint32_t T, float *cs, float *pe) {
    const uint32_t D = m->d_model;
    const double sk = 1.0 / sqrt((double)(D / m->n_heads));
    for (uint32_t e = 0; e < D; e++) {
        const double w = m->pos_freq[e / 2];
        double mean = 0;
        for (uint32_t j = 0; j < T; j++)
            mean += (e % 2 ? cos(j * w) : sin(j * w)) / T;
        for (uint32_t j = 0; j < T; j++) {
            const size_t r = (size_t)j * 2 * D;
            pe[r + e] = pe[r + D + e] = (float)(((e % 2 ? cos(j * w) : sin(j * w)) - mean) * sk);
            cs[r + e] = (float)(e % 2 ? cos(j * w) : -cos(j * w));
            cs[r + D + e] = (float)sin(j * w);
        }
    }
}

static int make_graph(pk_qnn *e, const pk_model *m, char *err, size_t errlen) {
    build b = { e, NULL, 0, 0, 0, "sub", 0, NULL };
    const uint32_t F0 = m->n_mels, C = m->sub_channels, D = m->d_model;
    const uint32_t F1 = (F0 + 1) / 2, F2 = (F1 + 1) / 2, F3 = (F2 + 1) / 2;
    const uint32_t T0 = e->T0, T1 = e->T1, T2 = e->T2, T3 = e->T3;
    common c;
    c.s = 1.0f / PK_QNN_SCALE;
    c.eps = m->ln_eps * c.s * c.s;

    /* the inputs: the features, padded with zeros; the masks */
    const uint32_t dmel[4] = { 1, T0, F0, 1 }, dm1[4] = { 1, T1, 1, 1 }, dm2[4] = { 1, T2, 1, 1 };
    const uint32_t dm3[2] = { T3, 1 }, dkb[3] = { 1, 1, T3 };
    val mel = tensor(&b, QNN_TENSOR_TYPE_APP_WRITE, QNN_DATATYPE_FLOAT_32, 4, dmel, NULL, 0);
    val m1 = tensor(&b, QNN_TENSOR_TYPE_APP_WRITE, QNN_DATATYPE_FLOAT_32, 4, dm1, NULL, 0);
    val m2 = tensor(&b, QNN_TENSOR_TYPE_APP_WRITE, QNN_DATATYPE_FLOAT_32, 4, dm2, NULL, 0);
    c.m3 = tensor(&b, QNN_TENSOR_TYPE_APP_WRITE, QNN_DATATYPE_FLOAT_32, 2, dm3, NULL, 0);
    c.kbias = tensor(&b, QNN_TENSOR_TYPE_APP_WRITE, QNN_DATATYPE_FLOAT_32, 3, dkb, NULL, 0);
    e->in[0] = mel.t, e->in[1] = m1.t, e->in[2] = m2.t, e->in[3] = c.m3.t, e->in[4] = c.kbias.t;

    /* subsampling: [1][time][frequency][C] at each stage */
    val x = relu(&b, conv(&b, mel, taps3(&b, m->conv0_w, C), constant(&b, 1, &C, m->conv0_b), 2, 1, 1, 0));
    x = mul(&b, x, m1);
    x = conv(&b, x, taps3(&b, m->dw1_w, C), constant(&b, 1, &C, m->dw1_b), 2, 1, 1, 1);
    val pw1_b = constant(&b, 1, &C, m->pw1_b), pw2_b = constant(&b, 1, &C, m->pw2_b);
    const uint32_t d2[4] = { 1, T2, F2, C }, d3[4] = { 1, T3, F3, C }, flat[2] = { T3, F3 * C };
    x = mul(&b, reshape(&b, relu(&b, fc(&b, x, &m->sub_pw1, 0, C, 1, &pw1_b)), 4, d2), m2);
    x = conv(&b, x, taps3(&b, m->dw2_w, C), constant(&b, 1, &C, m->dw2_b), 2, 1, 1, 1);
    x = reshape(&b, relu(&b, fc(&b, x, &m->sub_pw2, 0, C, 1, &pw2_b)), 4, d3);
    /* row t is frame t's [F3][C], sub.out's input order (encoder.c's s5).
     * The frames past the end are left as they come, not zeroed: a frame
     * of zeros has no variance, and its layer norm, with the scaled
     * epsilon (1.6e-7) below what fp16 holds, is 0 / 0, which every
     * query's attention then takes in. */
    val out_b = scaled(&b, m->out_b, D, c.s);
    x = fc(&b, reshape(&b, x, 2, flat), &m->sub_out, 0, D, c.s, &out_b);

    const uint32_t dt[2] = { T3, 2 * D };
    float *cs = keep(&b, (size_t)T3 * 2 * D * sizeof *cs), *pe = keep(&b, (size_t)T3 * 2 * D * sizeof *pe);
    if (cs && pe)
        tables(m, T3, cs, pe);
    c.cs = constant(&b, 2, dt, cs);
    c.pe = constant(&b, 2, dt, pe);

    for (uint32_t i = 0; i < e->layers && !b.status; i++)
        x = layer(&b, m, &c, &m->layers[i], x, i + 1 == m->n_layers);
    if (e->layers < m->n_layers) {
        /* the stream after fewer layers, for checking (PKTITO_QNN_LAYERS),
         * unscaled into the output */
        float *k = keep(&b, sizeof *k);
        if (k)
            *k = PK_QNN_SCALE;
        const uint32_t one = 1;
        b.block = "out";
        val y = tensor(&b, QNN_TENSOR_TYPE_APP_READ, QNN_DATATYPE_FLOAT_32, x.rank, x.d, NULL, 0);
        val in[2] = { x, constant(&b, 1, &one, k) };
        node(&b, QNN_OP_ELEMENT_WISE_MULTIPLY, in, 2, &y, NULL, 0);
        x = y;
    }
    e->out = x.t;

    int rc = 0;
    if (b.status)
        rc = fail(err, errlen, b.failed, b.status);
    else {
        Qnn_ProfileHandle_t prof = profile(e);
        Qnn_ErrorHandle_t s = e->q.graphFinalize(e->graph, prof, NULL);
        profile_done(e, prof);
        if (s)
            rc = fail(err, errlen, "graphFinalize", s);
        else
            own_io(e);
    }
    build_free(&b);
    return rc;
}

/* ---------------------------------------------------------- the cache --- */

/* The compiled graph kept (pk_qnn_open's cache): this header, then QNN's
 * binary of the context. The key is what the binary was made of and for:
 * the model's data (its SHA-256, the .pkf header's), the length and layers
 * built, and QNN's build; PK_QNN_CACHE changes with what make_graph
 * writes. */
#define PK_QNN_CACHE "pktito-qnn 5"

typedef struct {
    char magic[16], key[176];
    char names[6][16];
    uint32_t id[6], rank[6], dims[6][4];
    uint64_t bytes;
} cache_head;

static void cache_key(const pk_qnn *e, const pk_model *m, char *key, size_t n) {
    const pk_header *h = (const pk_header *)m->map;
    char hex[65];
    for (int i = 0; i < 32; i++)
        snprintf(hex + 2 * i, 3, "%02x", h->digest[i]);
    const char *build = NULL;
    if (e->q.backendGetBuildId(&build) || !build)
        build = "?";
    snprintf(key, n, "%s %u %u %s", hex, e->T0, e->layers, build);
}

/* 0 if the cache held this graph and the NPU took it */
static int read_cache(pk_qnn *e, const pk_model *m, const char *path) {
    FILE *f = fopen(path, "rb");
    if (!f)
        return -1;
    cache_head h;
    char key[sizeof h.key];
    cache_key(e, m, key, sizeof key);
    void *bin = NULL;
    int rc = -1;
    if (fread(&h, sizeof h, 1, f) == 1 && !strncmp(h.magic, PK_QNN_CACHE, sizeof h.magic) &&
        !strncmp(h.key, key, sizeof h.key) && h.bytes > 0 && h.bytes < ((uint64_t)1 << 32) &&
        (bin = malloc(h.bytes)) && fread(bin, 1, h.bytes, f) == h.bytes &&
        !e->q.contextCreateFromBinary(e->backend, e->device, NULL, bin, h.bytes, &e->context, NULL) &&
        !e->q.graphRetrieve(e->context, "encoder", &e->graph)) {
        for (int k = 0; k < 6; k++) {
            Qnn_Tensor_t *t = io(e, k);
            *t = (Qnn_Tensor_t)QNN_TENSOR_INIT;
            memcpy(e->names[k], h.names[k], sizeof e->names[k]);
            e->names[k][sizeof e->names[k] - 1] = 0;
            memcpy(e->dims[k], h.dims[k], sizeof e->dims[k]);
            t->v1.id = h.id[k];
            t->v1.name = e->names[k];
            t->v1.type = k < 5 ? QNN_TENSOR_TYPE_APP_WRITE : QNN_TENSOR_TYPE_APP_READ;
            t->v1.dataType = QNN_DATATYPE_FLOAT_32;
            t->v1.rank = h.rank[k] <= 4 ? h.rank[k] : 4;
            t->v1.dimensions = e->dims[k];
            t->v1.memType = QNN_TENSORMEMTYPE_RAW;
        }
        rc = 0;
    } else if (e->context) {
        e->q.contextFree(e->context, NULL);
        e->context = NULL;
    }
    free(bin);
    fclose(f);
    return rc;
}

/* the compiled graph written to path, for the next time; a failure only
 * costs that */
static void write_cache(pk_qnn *e, const pk_model *m, const char *path) {
    Qnn_ContextBinarySize_t n = 0, got = 0;
    if (e->q.contextGetBinarySize(e->context, &n) || n == 0)
        return;
    void *bin = malloc(n);
    cache_head h;
    memset(&h, 0, sizeof h);
    snprintf(h.magic, sizeof h.magic, "%s", PK_QNN_CACHE);
    cache_key(e, m, h.key, sizeof h.key);
    for (int k = 0; k < 6; k++) {
        const Qnn_Tensor_t *t = io(e, k);
        memcpy(h.names[k], e->names[k], sizeof h.names[k]);
        h.id[k] = t->v1.id;
        h.rank[k] = t->v1.rank;
        memcpy(h.dims[k], e->dims[k], sizeof h.dims[k]);
    }
    FILE *f = bin ? fopen(path, "wb") : NULL;
    if (f && !e->q.contextGetBinary(e->context, bin, n, &got) && (h.bytes = got) > 0 &&
        fwrite(&h, sizeof h, 1, f) == 1 && fwrite(bin, 1, got, f) == got) {
        fclose(f);
        f = NULL;
    } else if (f) {
        fclose(f);
        remove(path);
        f = NULL;
    }
    free(bin);
}

/* ------------------------------------------------------------ the NPU --- */

/* The HTP's clocks held up while it runs: QNN's default votes for its
 * clocks as each call comes, and a call's time then depends on what ran
 * before it */
static void burst(pk_qnn *e) {
    QnnDevice_Infrastructure_t infra = NULL;
    if (e->q.deviceGetInfrastructure(&infra) || !infra)
        return;
    QnnHtpDevice_PerfInfrastructure_t *perf = &((QnnHtpDevice_Infrastructure_t *)infra)->perfInfra;
    uint32_t id;
    if (!perf->createPowerConfigId || perf->createPowerConfigId(0, 0, &id))
        return;
    QnnHtpPerfInfrastructure_PowerConfig_t dcvs = QNN_HTP_PERF_INFRASTRUCTURE_POWER_CONFIG_INIT;
    dcvs.option = QNN_HTP_PERF_INFRASTRUCTURE_POWER_CONFIGOPTION_DCVS_V3;
    QnnHtpPerfInfrastructure_DcvsV3_t *v = &dcvs.dcvsV3Config;
    v->contextId = id;
    v->setDcvsEnable = 1, v->dcvsEnable = 0;
    v->powerMode = QNN_HTP_PERF_INFRASTRUCTURE_POWERMODE_PERFORMANCE_MODE;
    v->setSleepLatency = 1, v->sleepLatency = 40;
    v->setSleepDisable = 1, v->sleepDisable = 1;
    v->setBusParams = 1;
    v->busVoltageCornerMin = v->busVoltageCornerTarget = v->busVoltageCornerMax =
        DCVS_VOLTAGE_VCORNER_MAX_VOLTAGE_CORNER;
    v->setCoreParams = 1;
    v->coreVoltageCornerMin = v->coreVoltageCornerTarget = v->coreVoltageCornerMax =
        DCVS_VOLTAGE_VCORNER_MAX_VOLTAGE_CORNER;
    QnnHtpPerfInfrastructure_PowerConfig_t lat = QNN_HTP_PERF_INFRASTRUCTURE_POWER_CONFIG_INIT;
    lat.option = QNN_HTP_PERF_INFRASTRUCTURE_POWER_CONFIGOPTION_RPC_CONTROL_LATENCY;
    lat.rpcControlLatencyConfig = 100;
    QnnHtpPerfInfrastructure_PowerConfig_t poll = QNN_HTP_PERF_INFRASTRUCTURE_POWER_CONFIG_INIT;
    poll.option = QNN_HTP_PERF_INFRASTRUCTURE_POWER_CONFIGOPTION_RPC_POLLING_TIME;
    poll.rpcPollingTimeConfig = 9999;
    const QnnHtpPerfInfrastructure_PowerConfig_t *cfg[] = { &dcvs, &lat, &poll, NULL };
    perf->setPowerConfig(id, cfg);
}

/* The graph made and compiled, into a context of its own, and kept in
 * cache where there is one: 0, or -1 with why in err */
static int compile(pk_qnn *e, const pk_model *m, const char *cache, char *err, size_t errlen) {
    Qnn_ErrorHandle_t s;
    if ((s = e->q.contextCreate(e->backend, e->device, NULL, &e->context)))
        return fail(err, errlen, "contextCreate", s);
    /* the compiler's highest optimization, and the HTP's fp16 sigmoid of
     * high precision (SiLU's, GLU's) */
    QnnHtpGraph_CustomConfig_t opt = QNN_HTP_GRAPH_CUSTOM_CONFIG_INIT, sig = QNN_HTP_GRAPH_CUSTOM_CONFIG_INIT;
    opt.option = QNN_HTP_GRAPH_CONFIG_OPTION_OPTIMIZATION;
    opt.optimizationOption.type = QNN_HTP_GRAPH_OPTIMIZATION_TYPE_FINALIZE_OPTIMIZATION_FLAG;
    opt.optimizationOption.floatValue = 3;
    sig.option = QNN_HTP_GRAPH_CONFIG_OPTION_HIGH_PRECISION_SIGMOID;
    sig.highPrecisionSigmoid = true;
    QnnGraph_Config_t g_opt = { .option = QNN_GRAPH_CONFIG_OPTION_CUSTOM, .customConfig = &opt };
    QnnGraph_Config_t g_sig = { .option = QNN_GRAPH_CONFIG_OPTION_CUSTOM, .customConfig = &sig };
    const QnnGraph_Config_t *gcfg[] = { &g_opt, &g_sig, NULL };
    if ((s = e->q.graphCreate(e->context, "encoder", gcfg, &e->graph)))
        return fail(err, errlen, "graphCreate", s);
    if (make_graph(e, m, err, errlen))
        return -1;
    if (cache)
        write_cache(e, m, cache);
    return 0;
}

pk_qnn *pk_qnn_open(const pk_model *m, uint32_t frames, const char *cache, char *err, size_t errlen) {
    if (!m->layers) {
        snprintf(err, errlen, "the model has no encoder of its own");
        return NULL;
    }
    if (m->n_heads > 16) {
        snprintf(err, errlen, "the graph is made for 16 heads at most, not %u", m->n_heads);
        return NULL;
    }
    pk_qnn *e = calloc(1, sizeof *e);
    if (!e) {
        snprintf(err, errlen, "out of memory");
        return NULL;
    }
    e->n_mels = m->n_mels;
    e->d = m->d_model;
    e->T0 = frames ? frames : PK_QNN_FRAMES;
    e->T1 = (e->T0 + 1) / 2, e->T2 = (e->T1 + 1) / 2, e->T3 = (e->T2 + 1) / 2;
    /* PKTITO_QNN_LAYERS=k: the graph through layer k only (0, the
     * subsampling), its stream the output, to find where the NPU's
     * numbers part from the CPU's (encoder.c's PKTITO_QNN_CHECK) */
    const char *nl = getenv("PKTITO_QNN_LAYERS");
    e->layers = nl && (uint32_t)atoi(nl) < m->n_layers ? (uint32_t)atoi(nl) : m->n_layers;
    last_error[0] = 0;

    e->lib = dlopen("libQnnHtp.so", RTLD_NOW | RTLD_LOCAL);
    get_providers_fn gp = e->lib ? (get_providers_fn)dlsym(e->lib, "QnnInterface_getProviders") : NULL;
    const QnnInterface_t **prov = NULL;
    uint32_t np = 0;
    if (!gp || gp(&prov, &np) || np == 0 || prov[0]->apiVersion.coreApiVersion.major != QNN_API_VERSION_MAJOR) {
        snprintf(err, errlen, "no QNN HTP library: %s", e->lib ? "no provider of QNN 2" : dlerror());
        pk_qnn_free(e);
        return NULL;
    }
    e->q = prov[0]->QNN_INTERFACE_VER_NAME;

    Qnn_ErrorHandle_t s;
    if (e->q.logCreate(log_cb, QNN_LOG_LEVEL_WARN, &e->log))
        e->log = NULL;
    if ((s = e->q.backendCreate(e->log, NULL, &e->backend)) ||
        (s = e->q.deviceCreate(e->log, NULL, &e->device))) {
        fail(err, errlen, e->backend ? "deviceCreate" : "backendCreate", s);
        pk_qnn_free(e);
        return NULL;
    }
    burst(e);
    if (!cache || read_cache(e, m, cache))
        if (compile(e, m, cache, err, errlen)) {
            pk_qnn_free(e);
            return NULL;
        }
    const size_t n[5] = { (size_t)e->T0 * e->n_mels, e->T1, e->T2, e->T3, e->T3 };
    for (int k = 0; k < 5; k++) {
        e->buf[k] = calloc(n[k], sizeof(float));
        e->in[k].v1.clientBuf.data = e->buf[k];
        e->in[k].v1.clientBuf.dataSize = (uint32_t)(n[k] * sizeof(float));
    }
    e->x = calloc((size_t)e->T3 * e->d, sizeof(float));
    e->out.v1.clientBuf.data = e->x;
    e->out.v1.clientBuf.dataSize = (uint32_t)((size_t)e->T3 * e->d * sizeof(float));
    for (int k = 0; k < 5; k++)
        if (!e->buf[k] || !e->x) {
            snprintf(err, errlen, "out of memory");
            pk_qnn_free(e);
            return NULL;
        }
    return e;
}

void pk_qnn_free(pk_qnn *e) {
    if (!e)
        return;
    if (e->context)
        e->q.contextFree(e->context, NULL);
    if (e->device)
        e->q.deviceFree(e->device);
    if (e->backend)
        e->q.backendFree(e->backend);
    if (e->log)
        e->q.logFree(e->log);
    for (int k = 0; k < 5; k++)
        free(e->buf[k]);
    free(e->x);
    /* libQnnHtp.so stays loaded: QNN's documentation has it loaded once a
     * process */
    free(e);
}

uint32_t pk_qnn_frames(const pk_qnn *e) { return e->T0; }

uint32_t pk_qnn_layers(const pk_qnn *e) { return e->layers; }

uint32_t pk_qnn_encode(pk_qnn *e, const float *feat, uint32_t T0, float *x) {
    if (T0 == 0 || T0 > e->T0)
        return 0;
    const uint32_t T1 = (T0 + 1) / 2, T2 = (T1 + 1) / 2, T3 = (T2 + 1) / 2;
    memcpy(e->buf[0], feat, (size_t)T0 * e->n_mels * sizeof(float));
    memset(e->buf[0] + (size_t)T0 * e->n_mels, 0, (size_t)(e->T0 - T0) * e->n_mels * sizeof(float));
    for (uint32_t t = 0; t < e->T1; t++)
        e->buf[1][t] = t < T1;
    for (uint32_t t = 0; t < e->T2; t++)
        e->buf[2][t] = t < T2;
    for (uint32_t t = 0; t < e->T3; t++) {
        e->buf[3][t] = t < T3;
        e->buf[4][t] = t < T3 ? 0.0f : -10000.0f;
    }
    Qnn_ProfileHandle_t prof = profile(e);
    Qnn_ErrorHandle_t s = e->q.graphExecute(e->graph, e->in, 5, &e->out, 1, prof, NULL);
    profile_done(e, prof);
    if (s)
        return 0;
    memcpy(x, e->x, (size_t)T3 * e->d * sizeof(float));
    return T3;
}
