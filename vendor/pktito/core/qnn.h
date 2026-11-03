/* The encoder on Qualcomm's Hexagon NPU (HTP) through QNN, Qualcomm's AI
 * Engine Direct (make QNN=1 QNN_SDK=<the SDK's folder>; Android on a
 * Snapdragon). The graph is built here from the model's own weights: its
 * int8 matrices, a scale an output row, every other tensor from the
 * file's floats; the NPU's arithmetic on the activations is fp16,
 * not ref's, and not the same bits on every chip (Qualcomm's documentation
 * says SM8550's and SM8650's differ), which is why it is a build option
 * and not one of pktito's backends. It stands in for core/encoder.c on an
 * input that fits its length. Internal. */
#ifndef PK_QNN_H
#define PK_QNN_H

#include <stddef.h>
#include <stdint.h>

#include "pktito.h"

typedef struct pk_qnn pk_qnn;

/* The encoder of model m as a graph for the NPU, taking up to frames
 * feature frames, padded to that length (3072, 30.72 s, by default: the
 * longest chunk pktito transcribes at once). QNN's libraries are found by
 * the system's loader: libQnnHtp.so beside the program (LD_LIBRARY_PATH,
 * or an app's native library folder), the NPU's own (libQnnHtpV75Skel.so,
 * for a v75) where ADSP_LIBRARY_PATH says. QNN compiles the graph on the
 * device's CPU when it is first made, which takes a minute or two (94 to
 * 114 s on a Galaxy S24 Ultra, tools/qnn/CLIMB.md); with cache a path,
 * the compiled graph (718 MB for the English model) is kept there and
 * read back the next time, for the same model, length and QNN. NULL if
 * the NPU can't run it, and why in err. */
pk_qnn *pk_qnn_open(const pk_model *m, uint32_t frames, const char *cache, char *err, size_t errlen);
void pk_qnn_free(pk_qnn *e);

/* The feature frames it takes */
uint32_t pk_qnn_frames(const pk_qnn *e);

/* The encoder's layers it runs: every one, but with PKTITO_QNN_LAYERS=k
 * (for checking) the first k, its output the stream after them */
uint32_t pk_qnn_layers(const pk_qnn *e);

/* feat [T0][n_mels], T0 at most pk_qnn_frames, padded to that: x
 * [Te][d_model], Te encoder frames, which it returns; 0 if QNN failed. One
 * call at a time. */
uint32_t pk_qnn_encode(pk_qnn *e, const float *feat, uint32_t T0, float *x);

#endif
