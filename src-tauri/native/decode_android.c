/* The audio of a media file, through Android's media framework, the NDK's
 * AMediaExtractor and AMediaCodec: whatever the device plays (mp3; aac in
 * mp4, m4a, 3gp and adts; opus, vorbis, flac, amr, and what its maker
 * adds), its first audio track decoded at the decoder's own rate, in its
 * own channels and in its own sample format (16-bit, or float where the
 * decoder gives float of itself), and handed to the mix (mix.h). The
 * decoder is asked for no conversion of any of them (the configure below
 * says why). Every decoder of the Googlebook's (Android 17, c2.android.*)
 * gave 16-bit samples, asked for float or not, as did its FLAC extractor
 * of a 24-bit file.
 *
 * The file is a descriptor, as Android's document picker hands it over,
 * and the extractor reads it by offset, so it must be a file, not a pipe;
 * fd stays the caller's.
 *
 * 0 on success, with *out malloc'd (the caller frees it) holding *n
 * samples (decode.rs says where they start and end against ffmpeg's); -1
 * if the file has no audio track Android shows, which a track it has no
 * decoder for is not; -2 if Android cannot read it, the reason in err. */
#include <dlfcn.h>
#include <media/NdkMediaCodec.h>
#include <media/NdkMediaExtractor.h>
#include <media/NdkMediaFormat.h>
#include <stdarg.h>
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

static int fail(char *err, size_t errlen, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(err, errlen, fmt, ap);
    va_end(ap);
    return -2;
}

/* The output format: rate, channels and encoding, into the mix. Android's
 * channel masks are WAVE's speaker bits two places up (AudioFormat's
 * CHANNEL_OUT_FRONT_LEFT is 0x4, WAVE's SPEAKER_FRONT_LEFT 0x1), as the
 * AAC decoder's are (stereo 12); the Vorbis and Opus decoders give WAVE's
 * own (stereo 3, mono 1), which two places down name fewer channels than
 * there are, and the mix takes the usual layout for the count, the same
 * one (the Googlebook, Android 17). */
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

/* For a program of its own, as pktito-cli is when adb's shell runs it:
 * starts the process's binder thread pool, which an app's runtime starts
 * before any of the app runs. The decoder answers through it: without one,
 * a decode waited for its first output for good, and with one the same file
 * decoded (the Googlebook, Android 17, c2.android.aac.decoder). The NDK
 * leaves the call to the platform's own programs, so it is looked up. 0, or
 * -1 if there is none to call. */
int pkapp_binder_pool(void) {
    void *h = dlopen("libbinder_ndk.so", RTLD_NOW);
    void (*start)(void) = h ? (void (*)(void))dlsym(h, "ABinderProcess_startThreadPool") : NULL;
    if (!start)
        return -1;
    start();
    return 0;
}

int pkapp_decode_fd(int fd, unsigned rate, float **out, size_t *n, char *err, size_t errlen) {
    *out = NULL;
    *n = 0;
    struct stat st;
    if (fstat(fd, &st) || !S_ISREG(st.st_mode))
        return fail(err, errlen, "Android's extractor reads a file, and this is not one");
    AMediaExtractor *ex = AMediaExtractor_new();
    AMediaCodec *c = NULL;
    AMediaFormat *f = NULL;
    pkapp_mix *mix = pkapp_mix_new(rate);
    float *x = NULL;
    size_t xcap = 0;
    int rc = -2;
    media_status_t s;
    if (!ex || !mix) {
        rc = fail(err, errlen, "out of memory");
        goto done;
    }
    if ((s = AMediaExtractor_setDataSourceFd(ex, fd, 0, st.st_size)) != AMEDIA_OK) {
        rc = fail(err, errlen, "Android doesn't read files of this kind (%d)", s);
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
        rc = fail(err, errlen, "Android has no decoder for %s", mime);
        goto done;
    }
    /* The decoder's own samples, 16-bit unless it gives float of itself: it
     * is not asked for float, because a decoder asked can say float and give
     * 16-bit. Samsung's FLAC decoder (c2.sec.flac.decoder, a Galaxy S25
     * Ultra, Android 17) did, and a 10:40 chapter came out 5:20 long, with
     * no speech found in it; a track that is PCM already (audio/raw: the
     * Googlebook's FLAC extractor decodes in the extractor) passed 16-bit
     * samples on as float, and a 60 s FLAC came out 30 s of noise. Every
     * decoder of the Googlebook's gave 16-bit, asked for float or not, so
     * its transcripts are the same either way. */
    if ((s = AMediaCodec_configure(c, f, NULL, NULL, 0)) != AMEDIA_OK || (s = AMediaCodec_start(c)) != AMEDIA_OK) {
        rc = fail(err, errlen, "Android's decoder for %s would not start (%d)", mime, s);
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
                rc = fail(err, errlen, "Android's decoder gives samples of a kind pktito does not take (%d)", enc);
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
                        rc = fail(err, errlen, "Android's decoder gives samples of a kind pktito does not take (%d)", enc);
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
                            rc = fail(err, errlen, "out of memory");
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
                    rc = fail(err, errlen, "out of memory");
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
            rc = fail(err, errlen, "Android's decoder stopped part way");
            goto done;
        }
    }
    rc = pkapp_mix_take(mix, out, n) ? fail(err, errlen, "out of memory") : 0;
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

static int earlier(const void *a, const void *b) {
    int64_t x = *(const int64_t *)a, y = *(const int64_t *)b;
    return (x > y) - (x < y);
}

/* ex's first video track's rate: its frames over its length where it says
 * both (MP4, QuickTime, 3GP), else its first frames' times, their span over
 * their count (Matroska and WebM, which say neither). Android's own
 * frame-rate key is a whole number, 30 for a 29.97 fps MP4 and 24 for a
 * 23.976, where frames over length were 29.969 and 23.975 (the Googlebook,
 * Android 17). The times are a decoder's order, sorted; where the reading
 * stops short of the end, the last read may be shown after frames not read
 * yet (B-frames), and the latest 40 are left out: with them, an H.264 MKV
 * at 23.976 read 23.779. 259 intervals of Matroska's millisecond times put
 * 29.97 within 0.003. */
static double rate_of(AMediaExtractor *ex) {
    size_t tracks = AMediaExtractor_getTrackCount(ex), t;
    for (t = 0; t < tracks; t++) {
        AMediaFormat *f = AMediaExtractor_getTrackFormat(ex, t);
        const char *mime = NULL;
        int video = AMediaFormat_getString(f, AMEDIAFORMAT_KEY_MIME, &mime) && !strncmp(mime, "video/", 6);
        int32_t frames = 0;
        int64_t us = 0;
        double r = 0;
        if (video && AMediaFormat_getInt32(f, AMEDIAFORMAT_KEY_FRAME_COUNT, &frames) &&
            AMediaFormat_getInt64(f, AMEDIAFORMAT_KEY_DURATION, &us) && frames > 1 && us > 0)
            r = frames * 1e6 / us;
        AMediaFormat_delete(f);
        if (r > 0)
            return r;
        if (video)
            break;
    }
    if (t == tracks)
        return 0;
    int64_t at[300];
    size_t n = 0;
    int end = 0;
    AMediaExtractor_selectTrack(ex, t);
    while (n < sizeof at / sizeof at[0]) {
        int64_t us = AMediaExtractor_getSampleTime(ex);
        if (us < 0) {
            end = 1;
            break;
        }
        at[n++] = us;
        if (!AMediaExtractor_advance(ex)) {
            end = 1;
            break;
        }
    }
    qsort(at, n, sizeof at[0], earlier);
    if (!end)
        n -= 40;
    if (n < 3)
        return 0;
    return at[n - 1] > at[0] ? (n - 1) * 1e6 / (at[n - 1] - at[0]) : 0;
}

/* A video's frame rate, for times in frames (decode.rs, frame_rate_fd), as
 * the desktop's comes from the OS or ffmpeg: 0 for a file with no video
 * track, or one Android cannot read. fd stays the caller's. */
double pkapp_video_rate_fd(int fd) {
    struct stat st;
    if (fstat(fd, &st) || !S_ISREG(st.st_mode))
        return 0;
    AMediaExtractor *ex = AMediaExtractor_new();
    if (!ex)
        return 0;
    double r = AMediaExtractor_setDataSourceFd(ex, fd, 0, st.st_size) == AMEDIA_OK ? rate_of(ex) : 0;
    AMediaExtractor_delete(ex);
    return r;
}
