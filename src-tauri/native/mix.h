/* A decoder's audio on its way to the engine: interleaved float frames at
 * the decoder's own rate and in all its channels, mixed to one channel and
 * brought to the engine's rate by pktito's resampler. Every decoder - Media
 * Foundation, AVFoundation, ffmpeg - hands its samples over unconverted, so
 * the same samples make the same signal whichever decoded them, run after
 * run. (AVFoundation's resampler makes a slightly different signal on each
 * run, a different transcript one run in two; ffmpeg's -ac 1 is 3 dB louder
 * on stereo than an average, enough to move the VAD's cuts. Both measured
 * with scripts/media-check.sh.) */
#ifndef PKAPP_MIX_H
#define PKAPP_MIX_H

#include <stddef.h>

#define PKAPP_MAX_CHANNELS 32

/* Where a channel plays, as far as the mix is concerned. */
enum { PKAPP_FRONT, PKAPP_CENTER, PKAPP_LFE, PKAPP_SURROUND };

typedef struct pkapp_mix pkapp_mix;

/* NULL when memory runs out. rate is the one it hands on. */
pkapp_mix *pkapp_mix_new(unsigned rate);
void       pkapp_mix_free(pkapp_mix *m);

/* The format of the frames that follow: their rate, their channels, and
 * where each plays (NULL: the usual layout for the count, WAVE's order).
 * A change of rate mid-stream finishes what came before at the old one.
 * 0, or -1 for no rate, no channels or more than PKAPP_MAX_CHANNELS, a
 * rate the resampler cannot take, or no memory. */
int pkapp_mix_format(pkapp_mix *m, unsigned rate, unsigned ch, const unsigned char *where);

/* The same, the channels placed by a WAVE speaker mask, in the order of its
 * bits (Media Foundation's and a WAV's); 0, or a mask that does not name
 * every channel, for the usual layout. */
int pkapp_mix_format_wave(pkapp_mix *m, unsigned rate, unsigned ch, unsigned mask);

/* n frames in the current format. 0, or -1 before any format or when
 * memory runs out. */
int pkapp_mix_push(pkapp_mix *m, const float *x, size_t n);

/* After the last push: the signal, malloc'd for the caller to free (NULL
 * when it is empty), and its length. 0, or -1 when memory runs out. */
int pkapp_mix_take(pkapp_mix *m, float **y, size_t *n);

/* For a caller that hands the signal on as it is made instead of taking
 * it whole: the samples made since the last drain, in *y, good until the
 * next call on m, and their count (0 and NULL when there are none). They
 * are the mix's no longer: take gives only what came after. The samples
 * are the same ones take would give, in the same order, however the
 * pushes and drains fall. */
void pkapp_mix_drain(pkapp_mix *m, const float **y, size_t *n);

/* After the last push, for a caller that drains: what the resampler still
 * holds, made, for one drain more. 0, or -1 when memory runs out. */
int pkapp_mix_end(pkapp_mix *m);

#endif /* PKAPP_MIX_H */
