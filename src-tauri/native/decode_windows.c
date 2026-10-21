/* The audio of a media file, through Media Foundation's Source Reader:
 * whatever Windows can play (mp3, mp4, m4a, wma, wav, aac, and more with
 * Microsoft's codec extensions), its first audio stream decoded to 32-bit
 * float at the file's own rate, its channels mixed to one, and brought to
 * the rate asked for by pktito's resampler (torchaudio's Resample).
 *
 * 0 on success, with *out malloc'd (the caller frees it) holding *n
 * samples; -1 if the file has no audio stream; -2 if Windows cannot read
 * it, the reason in err. */
#define COBJMACROS
#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mferror.h>
#include <mfreadwrite.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "resample.h"

typedef struct { float *p; size_t n, cap; } fbuf;

static int fbuf_room(fbuf *b, size_t more) {
    if (b->n + more <= b->cap)
        return 0;
    size_t cap = b->cap ? b->cap : 1 << 20;
    while (cap < b->n + more)
        cap *= 2;
    float *p = realloc(b->p, cap * sizeof *p);
    if (!p)
        return -1;
    b->p = p;
    b->cap = cap;
    return 0;
}

static int fail(char *err, size_t errlen, const char *what, HRESULT hr) {
    snprintf(err, errlen, "%s (0x%08lx)", what, (unsigned long)hr);
    return -2;
}

int pkapp_decode_os(const char *path, int rate, float **out, size_t *n, char *err, size_t errlen) {
    *out = NULL;
    *n = 0;
    int wlen = MultiByteToWideChar(CP_UTF8, 0, path, -1, NULL, 0);
    wchar_t *wpath = wlen > 0 ? malloc((size_t)wlen * sizeof *wpath) : NULL;
    if (!wpath)
        return fail(err, errlen, "the file name could not be read", E_INVALIDARG);
    MultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, wlen);

    HRESULT com = CoInitializeEx(NULL, COINIT_MULTITHREADED);
    HRESULT hr = MFStartup(MF_VERSION, MFSTARTUP_LITE);
    IMFSourceReader *rd = NULL;
    IMFMediaType *want = NULL, *got = NULL;
    pk_resampler *rs = NULL;
    fbuf res = { 0 }, mono = { 0 };
    int rc = 0;
    UINT32 srate = 0, ch = 0;

    if (FAILED(hr)) {
        rc = fail(err, errlen, "Media Foundation could not start", hr);
        goto done;
    }
    hr = MFCreateSourceReaderFromURL(wpath, NULL, &rd);
    if (FAILED(hr)) {
        rc = fail(err, errlen, "Windows cannot open this file", hr);
        goto done;
    }
    IMFSourceReader_SetStreamSelection(rd, (DWORD)MF_SOURCE_READER_ALL_STREAMS, FALSE);
    hr = IMFSourceReader_SetStreamSelection(rd, (DWORD)MF_SOURCE_READER_FIRST_AUDIO_STREAM, TRUE);
    if (FAILED(hr)) {
        rc = -1;                                  /* no audio stream */
        goto done;
    }
    MFCreateMediaType(&want);
    IMFMediaType_SetGUID(want, &MF_MT_MAJOR_TYPE, &MFMediaType_Audio);
    IMFMediaType_SetGUID(want, &MF_MT_SUBTYPE, &MFAudioFormat_Float);
    hr = IMFSourceReader_SetCurrentMediaType(rd, (DWORD)MF_SOURCE_READER_FIRST_AUDIO_STREAM, NULL, want);
    if (FAILED(hr)) {
        rc = fail(err, errlen, "Windows has no decoder for this file's audio", hr);
        goto done;
    }
    hr = IMFSourceReader_GetCurrentMediaType(rd, (DWORD)MF_SOURCE_READER_FIRST_AUDIO_STREAM, &got);
    if (SUCCEEDED(hr))
        hr = IMFMediaType_GetUINT32(got, &MF_MT_AUDIO_SAMPLES_PER_SECOND, &srate);
    if (SUCCEEDED(hr))
        hr = IMFMediaType_GetUINT32(got, &MF_MT_AUDIO_NUM_CHANNELS, &ch);
    if (FAILED(hr) || !srate || !ch) {
        rc = fail(err, errlen, "the audio's format could not be read", hr);
        goto done;
    }
    if (srate != (UINT32)rate && !(rs = pk_resampler_new(srate, (uint32_t)rate))) {
        rc = fail(err, errlen, "the audio's sample rate cannot be converted", E_FAIL);
        goto done;
    }

    for (;;) {
        DWORD flags = 0;
        IMFSample *sample = NULL;
        hr = IMFSourceReader_ReadSample(rd, (DWORD)MF_SOURCE_READER_FIRST_AUDIO_STREAM, 0, NULL, &flags,
                                        NULL, &sample);
        if (FAILED(hr)) {
            rc = fail(err, errlen, "the audio could not be decoded", hr);
            goto done;
        }
        if (flags & MF_SOURCE_READERF_ENDOFSTREAM) {
            if (sample)
                IMFSample_Release(sample);
            break;
        }
        if (!sample)
            continue;
        IMFMediaBuffer *mb = NULL;
        BYTE *data = NULL;
        DWORD bytes = 0;
        if (SUCCEEDED(IMFSample_ConvertToContiguousBuffer(sample, &mb)) &&
            SUCCEEDED(IMFMediaBuffer_Lock(mb, &data, NULL, &bytes))) {
            const float *f = (const float *)data;
            size_t frames = bytes / (sizeof(float) * ch);
            mono.n = 0;
            if (fbuf_room(&mono, frames) == 0) {
                for (size_t i = 0; i < frames; i++) {
                    float s = 0;
                    for (UINT32 c = 0; c < ch; c++)
                        s += f[i * ch + c];
                    mono.p[i] = s / (float)ch;
                }
                mono.n = frames;
                size_t room = rs ? pk_resample_room(rs, frames) : frames;
                if (fbuf_room(&res, room) == 0)
                    res.n += rs ? pk_resample(rs, mono.p, frames, res.p + res.n)
                                : (memcpy(res.p + res.n, mono.p, frames * sizeof(float)), frames);
                else
                    rc = fail(err, errlen, "out of memory", E_OUTOFMEMORY);
            } else {
                rc = fail(err, errlen, "out of memory", E_OUTOFMEMORY);
            }
            IMFMediaBuffer_Unlock(mb);
        }
        if (mb)
            IMFMediaBuffer_Release(mb);
        IMFSample_Release(sample);
        if (rc)
            goto done;
    }
    if (rs && fbuf_room(&res, pk_resample_room(rs, 0)) == 0)
        res.n += pk_resample_flush(rs, res.p + res.n);
    *out = res.p;
    *n = res.n;
    res.p = NULL;

done:
    free(res.p);
    free(mono.p);
    if (rs)
        pk_resampler_free(rs);
    if (got)
        IMFMediaType_Release(got);
    if (want)
        IMFMediaType_Release(want);
    if (rd)
        IMFSourceReader_Release(rd);
    MFShutdown();
    if (SUCCEEDED(com))
        CoUninitialize();
    free(wpath);
    return rc;
}
