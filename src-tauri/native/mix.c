#include "mix.h"

#include <stdlib.h>
#include <string.h>

#include "resample.h"

struct pkapp_mix {
    unsigned to;                    /* the rate handed on */
    unsigned rate, ch;              /* the frames' */
    float w[PKAPP_MAX_CHANNELS];    /* each channel's share of the mix */
    pk_resampler *rs;               /* rate to to; NULL when they are the same */
    float *mono;                    /* one push, mixed */
    size_t mono_cap;
    float *y;                       /* the signal so far */
    size_t n, cap;
};

static int room(float **p, size_t *cap, size_t need) {
    if (need <= *cap)
        return 0;
    size_t c = *cap ? *cap : (size_t)1 << 16;
    while (c < need)
        c *= 2;
    float *q = realloc(*p, c * sizeof *q);
    if (!q)
        return -1;
    *p = q;
    *cap = c;
    return 0;
}

/* The end of the signal at the current rate: what the resampler holds. */
static int finish(pkapp_mix *m) {
    if (!m->rs)
        return 0;
    if (room(&m->y, &m->cap, m->n + pk_resample_room(m->rs, 0)))
        return -1;
    m->n += pk_resample_flush(m->rs, m->y + m->n);
    pk_resampler_free(m->rs);
    m->rs = NULL;
    return 0;
}

pkapp_mix *pkapp_mix_new(unsigned rate) {
    pkapp_mix *m = calloc(1, sizeof *m);
    if (m)
        m->to = rate;
    return m;
}

void pkapp_mix_free(pkapp_mix *m) {
    if (!m)
        return;
    pk_resampler_free(m->rs);
    free(m->mono);
    free(m->y);
    free(m);
}

/* WAVE's order: left, right, centre, LFE, then the surrounds. */
static unsigned char usual(unsigned ch, unsigned c) {
    if (ch == 1)
        return PKAPP_CENTER;
    if (ch == 6 || ch == 8)
        return c < 2 ? PKAPP_FRONT : c == 2 ? PKAPP_CENTER : c == 3 ? PKAPP_LFE : PKAPP_SURROUND;
    return PKAPP_FRONT;
}

int pkapp_mix_format(pkapp_mix *m, unsigned rate, unsigned ch, const unsigned char *where) {
    /* the proportions of swresample's defaults for -ac 1 - the centre and
     * surrounds at -3 dB against the fronts, the LFE left out - scaled to sum
     * to one, so that stereo is its average and the mix never clips; 5.1
     * comes to 0.207 each front, 0.293 the centre, 0.146 each surround */
    static const float share[] = {
        [PKAPP_FRONT] = 1.0f, [PKAPP_CENTER] = 1.41421356f, [PKAPP_LFE] = 0.0f, [PKAPP_SURROUND] = 0.70710678f,
    };
    if (!rate || !ch || ch > PKAPP_MAX_CHANNELS)
        return -1;
    float sum = 0;
    for (unsigned c = 0; c < ch; c++) {
        unsigned char k = where ? where[c] : usual(ch, c);
        m->w[c] = k <= PKAPP_SURROUND ? share[k] : 1.0f;
        sum += m->w[c];
    }
    for (unsigned c = 0; c < ch; c++)
        m->w[c] = sum > 0 ? m->w[c] / sum : 1.0f / (float)ch;
    m->ch = ch;
    if (rate == m->rate)
        return 0;
    if (finish(m))
        return -1;
    m->rate = rate;
    if (rate != m->to && !(m->rs = pk_resampler_new(rate, m->to)))
        return -1;
    return 0;
}

int pkapp_mix_format_wave(pkapp_mix *m, unsigned rate, unsigned ch, unsigned mask) {
    enum {   /* WAVE's speaker bits */
        FRONT_LEFT = 0x1, FRONT_RIGHT = 0x2, FRONT_CENTER = 0x4, LOW_FREQUENCY = 0x8,
        FRONT_LEFT_OF_CENTER = 0x40, FRONT_RIGHT_OF_CENTER = 0x80,
    };
    unsigned char where[PKAPP_MAX_CHANNELS];
    unsigned c = 0;
    for (unsigned bit = 1; bit && c < ch && c < PKAPP_MAX_CHANNELS; bit <<= 1)
        if (mask & bit)
            where[c++] = bit == FRONT_CENTER ? PKAPP_CENTER
                       : bit == LOW_FREQUENCY ? PKAPP_LFE
                       : bit & (FRONT_LEFT | FRONT_RIGHT | FRONT_LEFT_OF_CENTER | FRONT_RIGHT_OF_CENTER) ? PKAPP_FRONT
                       : PKAPP_SURROUND;
    return pkapp_mix_format(m, rate, ch, c == ch ? where : NULL);
}

int pkapp_mix_push(pkapp_mix *m, const float *x, size_t n) {
    if (!m->ch || room(&m->mono, &m->mono_cap, n))
        return -1;
    for (size_t i = 0; i < n; i++) {
        float s = 0;
        for (unsigned c = 0; c < m->ch; c++)
            s += m->w[c] * x[i * m->ch + c];
        m->mono[i] = s;
    }
    if (room(&m->y, &m->cap, m->n + (m->rs ? pk_resample_room(m->rs, n) : n)))
        return -1;
    if (m->rs) {
        m->n += pk_resample(m->rs, m->mono, n, m->y + m->n);
    } else {
        memcpy(m->y + m->n, m->mono, n * sizeof *m->mono);
        m->n += n;
    }
    return 0;
}

int pkapp_mix_take(pkapp_mix *m, float **y, size_t *n) {
    if (finish(m))
        return -1;
    *y = m->y;
    *n = m->n;
    m->y = NULL;
    m->n = m->cap = 0;
    return 0;
}
