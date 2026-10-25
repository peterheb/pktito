/* The audio of a media file, through Android's media framework, the NDK's
 * AMediaExtractor and AMediaCodec: whatever the device plays (mp3; aac in
 * mp4, m4a, 3gp and adts; opus, vorbis, flac, amr, and what its maker
 * adds), its first audio track decoded at the decoder's own rate and in
 * its own channels, as float where the decoder gives float, and handed to
 * the mix (mix.h). The decoder is asked for no conversion of rate or
 * channels.
 *
 * The file is a descriptor, as Android's document picker hands it over,
 * and the extractor reads it by offset, so it must be a file, not a pipe;
 * fd stays the caller's.
 *
 * 0 on success, with *out malloc'd (the caller frees it) holding *n
 * samples and codec the decoder's name (its own start and end are the
 * caller's to cut, decode.rs); -1 if the file has no audio track; -2 if
 * Android cannot read it, the reason in err. */
#include <media/NdkMediaCodec.h>
#include <media/NdkMediaExtractor.h>
#include <media/NdkMediaFormat.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include "mix.h"

/* AudioFormat's encodings, as a decoder's output format names them */
enum { PCM_16 = 2, PCM_FLOAT = 4 };

/* How long the decoder may go without taking input or giving output
 * before it is given up on, in waits of 10 ms: 10 s. */
#define STALL 1000

static int fail(char *err, size_t errlen, const char *what, int code) {
    snprintf(err, errlen, "%s (%d)", what, code);
    return -2;
}

/* The output format: rate, channels and encoding, into the mix. Android's
 * channel masks are WAVE's speaker bits two places up (AudioFormat's
 * CHANNEL_OUT_FRONT_LEFT is 0x4, WAVE's SPEAKER_FRONT_LEFT 0x1). */
static int format(AMediaCodec *c, pkapp_mix *mix, int32_t *ch, int32_t *enc) {
    AMediaFormat *f = AMediaCodec_getOutputFormat(c);
    int32_t rate = 0, mask = 0;
    *enc = PCM_16;
    AMediaFormat_getInt32(f, AMEDIAFORMAT_KEY_SAMPLE_RATE, &rate);
    AMediaFormat_getInt32(f, AMEDIAFORMAT_KEY_CHANNEL_COUNT, ch);
    AMediaFormat_getInt32(f, AMEDIAFORMAT_KEY_CHANNEL_MASK, &mask);
    AMediaFormat_getInt32(f, AMEDIAFORMAT_KEY_PCM_ENCODING, enc);
    AMediaFormat_delete(f);
    if (rate <= 0 || *ch <= 0 || (*enc != PCM_16 && *enc != PCM_FLOAT))
        return -1;
    return pkapp_mix_format_wave(mix, (unsigned)rate, (unsigned)*ch, (unsigned)mask >> 2);
}

int pkapp_decode_fd(int fd, unsigned rate, float **out, size_t *n, char *codec, size_t codeclen, char *err,
                    size_t errlen) {
    *out = NULL;
    *n = 0;
    if (codeclen)
        codec[0] = '\0';
    struct stat st;
    if (fstat(fd, &st) || !S_ISREG(st.st_mode))
        return fail(err, errlen, "not a file Android's extractor can read", 0);
    AMediaExtractor *ex = AMediaExtractor_new();
    AMediaCodec *c = NULL;
    AMediaFormat *f = NULL;
    pkapp_mix *mix = pkapp_mix_new(rate);
    float *x = NULL;
    size_t xcap = 0;
    int rc = -2;
    media_status_t s;
    if (!ex || !mix) {
        rc = fail(err, errlen, "out of memory", 0);
        goto done;
    }
    if ((s = AMediaExtractor_setDataSourceFd(ex, fd, 0, st.st_size)) != AMEDIA_OK) {
        rc = fail(err, errlen, "Android cannot read this file", s);
        goto done;
    }
    /* the first audio track */
    const char *mime = NULL;
    size_t tracks = AMediaExtractor_getTrackCount(ex), t;
    for (t = 0; t < tracks; t++) {
        f = AMediaExtractor_getTrackFormat(ex, t);
        if (AMediaFormat_getString(f, AMEDIAFORMAT_KEY_MIME, &mime) && !strncmp(mime, "audio/", 6))
            break;
        AMediaFormat_delete(f);
        f = NULL;
    }
    if (!f) {
        rc = -1;
        goto done;
    }
    AMediaExtractor_selectTrack(ex, t);
    if (!(c = AMediaCodec_createDecoderByType(mime))) {
        rc = fail(err, errlen, "Android has no decoder for this audio", 0);
        snprintf(err + strlen(err), errlen - strlen(err), ": %s", mime);
        goto done;
    }
    char *name = NULL;
    if (AMediaCodec_getName(c, &name) == AMEDIA_OK) {
        snprintf(codec, codeclen, "%s", name);
        AMediaCodec_releaseName(c, name);
    }
    /* float, where the decoder gives it; 16-bit otherwise */
    AMediaFormat_setInt32(f, AMEDIAFORMAT_KEY_PCM_ENCODING, PCM_FLOAT);
    if ((s = AMediaCodec_configure(c, f, NULL, NULL, 0)) != AMEDIA_OK || (s = AMediaCodec_start(c)) != AMEDIA_OK) {
        rc = fail(err, errlen, "Android's decoder would not start", s);
        goto done;
    }
    int in_done = 0, have_format = 0, idle = 0;
    int32_t ch = 0, enc = PCM_16;
    for (;;) {
        int moved = 0;
        if (!in_done) {
            ssize_t ib = AMediaCodec_dequeueInputBuffer(c, 10000);
            if (ib >= 0) {
                size_t cap = 0;
                uint8_t *buf = AMediaCodec_getInputBuffer(c, (size_t)ib, &cap);
                ssize_t size = buf ? AMediaExtractor_readSampleData(ex, buf, cap) : -1;
                if (size < 0) {
                    AMediaCodec_queueInputBuffer(c, (size_t)ib, 0, 0, 0, AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM);
                    in_done = 1;
                } else {
                    AMediaCodec_queueInputBuffer(c, (size_t)ib, 0, (size_t)size,
                                                 (uint64_t)AMediaExtractor_getSampleTime(ex), 0);
                    AMediaExtractor_advance(ex);
                }
                moved = 1;
            }
        }
        AMediaCodecBufferInfo info;
        ssize_t ob = AMediaCodec_dequeueOutputBuffer(c, &info, 10000);
        if (ob == AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED) {
            if (format(c, mix, &ch, &enc)) {
                rc = fail(err, errlen, "Android's decoder gives audio in a form it cannot use", enc);
                goto done;
            }
            have_format = 1;
            moved = 1;
        } else if (ob >= 0) {
            size_t size = 0;
            uint8_t *buf = AMediaCodec_getOutputBuffer(c, (size_t)ob, &size);
            if (buf && info.size > 0) {
                if (!have_format) {
                    if (format(c, mix, &ch, &enc)) {
                        AMediaCodec_releaseOutputBuffer(c, (size_t)ob, false);
                        rc = fail(err, errlen, "Android's decoder gives audio in a form it cannot use", enc);
                        goto done;
                    }
                    have_format = 1;
                }
                const uint8_t *p = buf + info.offset;
                size_t frames = (size_t)info.size / (enc == PCM_FLOAT ? 4 : 2) / (size_t)ch;
                const float *y = (const float *)p;
                if (enc == PCM_16) {
                    /* torchaudio's int16 / 32768, as pktito's own reader */
                    size_t k = frames * (size_t)ch;
                    if (k > xcap) {
                        float *bigger = realloc(x, k * sizeof *x);
                        if (!bigger) {
                            AMediaCodec_releaseOutputBuffer(c, (size_t)ob, false);
                            rc = fail(err, errlen, "out of memory", 0);
                            goto done;
                        }
                        x = bigger;
                        xcap = k;
                    }
                    const int16_t *v = (const int16_t *)p;
                    for (size_t i = 0; i < k; i++)
                        x[i] = (float)v[i] / 32768.0f;
                    y = x;
                }
                if (pkapp_mix_push(mix, y, frames)) {
                    AMediaCodec_releaseOutputBuffer(c, (size_t)ob, false);
                    rc = fail(err, errlen, "out of memory", 0);
                    goto done;
                }
            }
            AMediaCodec_releaseOutputBuffer(c, (size_t)ob, false);
            moved = 1;
            if (info.flags & AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM)
                break;
        }
        idle = moved ? 0 : idle + 1;
        if (idle > STALL) {
            rc = fail(err, errlen, "Android's decoder stopped part way", 0);
            goto done;
        }
    }
    rc = pkapp_mix_take(mix, out, n) ? fail(err, errlen, "out of memory", 0) : 0;
done:
    if (c) {
        AMediaCodec_stop(c);
        AMediaCodec_delete(c);
    }
    if (f)
        AMediaFormat_delete(f);
    if (ex)
        AMediaExtractor_delete(ex);
    pkapp_mix_free(mix);
    free(x);
    return rc;
}
