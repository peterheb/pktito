/* The audio of a media file, through Media Foundation's Source Reader:
 * whatever Windows can play (mp3, mp4, m4a, wma, wav, aac, and more with
 * Microsoft's codec extensions), its first audio stream decoded to 32-bit
 * float at the decoder's own rate and handed to the mix (mix.h).
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

#include "mix.h"

static int fail(char *err, size_t errlen, const char *what, HRESULT hr) {
    snprintf(err, errlen, "%s (0x%08lx)", what, (unsigned long)hr);
    return -2;
}

/* MF_PD_MIME_TYPE, which MinGW's mfidl.h leaves out */
static const GUID PD_MIME_TYPE = {0x6c990d37, 0xbb8e, 0x477a, {0x85, 0x98, 0x0d, 0x5d, 0x96, 0xfc, 0xd8, 0x8a}};

/* Whether the file is read by Media Foundation's MPEG-4 source: mp4, m4a,
 * mov and 3gp, by the MIME type it gives them. */
static int mpeg4_source(IMFPresentationDescriptor *pd) {
    WCHAR *mime = NULL;
    UINT32 len = 0;
    int yes = 0;
    if (SUCCEEDED(IMFPresentationDescriptor_GetAllocatedString(pd, &PD_MIME_TYPE, &mime, &len))) {
        yes = !wcscmp(mime, L"video/mp4") || !wcscmp(mime, L"audio/mp4") || !wcscmp(mime, L"video/quicktime") ||
              !wcsncmp(mime, L"video/3gpp", 10) || !wcsncmp(mime, L"audio/3gpp", 10);
        CoTaskMemFree(mime);
    }
    return yes;
}

/* The file's first audio track, as ffmpeg (0:a:0) and AVFoundation take it.
 * Media Foundation numbers a file's streams in the order it lists them,
 * and says nothing of track IDs. Most of its sources keep the file's order,
 * so the first track is the audio stream with the lowest number; its
 * MPEG-4 source lists the audio tracks before the video, last track first,
 * so there it is the highest. Its own first audio stream is neither: of an
 * MP4's two audio tracks, the second, a tone, on Windows 11 26200 and
 * 26300. (The order measured on 26200 with five files of two audio
 * tracks, in MP4, M4A and MOV, laid out four ways; Matroska keeps it.) */
static DWORD first_audio(IMFSourceReader *rd) {
    DWORD first = (DWORD)MF_SOURCE_READER_FIRST_AUDIO_STREAM, best = 0, count = 0;
    int found = 0, last = 0;
    IMFMediaSource *src = NULL;
    IMFPresentationDescriptor *pd = NULL;
    if (SUCCEEDED(IMFSourceReader_GetServiceForStream(rd, (DWORD)MF_SOURCE_READER_MEDIASOURCE, &GUID_NULL,
                                                      &IID_IMFMediaSource, (void **)&src)) &&
        SUCCEEDED(IMFMediaSource_CreatePresentationDescriptor(src, &pd)) &&
        SUCCEEDED(IMFPresentationDescriptor_GetStreamDescriptorCount(pd, &count))) {
        last = mpeg4_source(pd);
        for (DWORD i = 0; i < count; i++) {
            BOOL selected;
            IMFStreamDescriptor *sd = NULL;
            IMFMediaTypeHandler *th = NULL;
            GUID major;
            DWORD id;
            if (SUCCEEDED(IMFPresentationDescriptor_GetStreamDescriptorByIndex(pd, i, &selected, &sd)) &&
                SUCCEEDED(IMFStreamDescriptor_GetMediaTypeHandler(sd, &th)) &&
                SUCCEEDED(IMFMediaTypeHandler_GetMajorType(th, &major)) && IsEqualGUID(&major, &MFMediaType_Audio) &&
                SUCCEEDED(IMFStreamDescriptor_GetStreamIdentifier(sd, &id)) &&
                (!found || (last ? id > best : id < best))) {
                first = i;
                best = id;
                found = 1;
            }
            if (th)
                IMFMediaTypeHandler_Release(th);
            if (sd)
                IMFStreamDescriptor_Release(sd);
        }
    }
    if (pd)
        IMFPresentationDescriptor_Release(pd);
    if (src)
        IMFMediaSource_Release(src);
    return first;
}

/* The decoder's format, given to the mix. It can change mid-stream: an
 * HE-AAC decoder may learn the real rate only once it decodes. */
static int take_format(IMFSourceReader *rd, DWORD stream, pkapp_mix *mix, UINT32 *ch, char *err, size_t errlen) {
    IMFMediaType *got = NULL;
    UINT32 rate = 0, mask = 0;
    *ch = 0;
    HRESULT hr = IMFSourceReader_GetCurrentMediaType(rd, stream, &got);
    if (SUCCEEDED(hr))
        hr = IMFMediaType_GetUINT32(got, &MF_MT_AUDIO_SAMPLES_PER_SECOND, &rate);
    if (SUCCEEDED(hr))
        hr = IMFMediaType_GetUINT32(got, &MF_MT_AUDIO_NUM_CHANNELS, ch);
    if (SUCCEEDED(hr) && FAILED(IMFMediaType_GetUINT32(got, &MF_MT_AUDIO_CHANNEL_MASK, &mask)))
        mask = 0;
    if (got)
        IMFMediaType_Release(got);
    if (FAILED(hr) || !rate || !*ch)
        return fail(err, errlen, "the audio's format could not be read", hr);
    if (pkapp_mix_format_wave(mix, rate, *ch, mask))
        return fail(err, errlen, "the audio's format cannot be converted", E_FAIL);
    return 0;
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
    HRESULT mf = MFStartup(MF_VERSION, MFSTARTUP_LITE);
    HRESULT hr;
    IMFSourceReader *rd = NULL;
    IMFMediaType *want = NULL;
    pkapp_mix *mix = pkapp_mix_new((unsigned)rate);
    UINT32 ch = 0;
    int rc = 0;

    if (!mix) {
        rc = fail(err, errlen, "out of memory", E_OUTOFMEMORY);
        goto done;
    }
    if (FAILED(mf)) {
        rc = fail(err, errlen, "Media Foundation could not start", mf);
        goto done;
    }
    hr = MFCreateSourceReaderFromURL(wpath, NULL, &rd);
    if (FAILED(hr)) {
        rc = fail(err, errlen, "Windows cannot open this file", hr);
        goto done;
    }
    DWORD stream = first_audio(rd);
    IMFSourceReader_SetStreamSelection(rd, (DWORD)MF_SOURCE_READER_ALL_STREAMS, FALSE);
    hr = IMFSourceReader_SetStreamSelection(rd, stream, TRUE);
    if (FAILED(hr)) {
        rc = -1;                                  /* no audio stream */
        goto done;
    }
    MFCreateMediaType(&want);
    IMFMediaType_SetGUID(want, &MF_MT_MAJOR_TYPE, &MFMediaType_Audio);
    IMFMediaType_SetGUID(want, &MF_MT_SUBTYPE, &MFAudioFormat_Float);
    hr = IMFSourceReader_SetCurrentMediaType(rd, stream, NULL, want);
    if (FAILED(hr)) {
        rc = fail(err, errlen, "Windows has no decoder for this file's audio", hr);
        goto done;
    }
    if ((rc = take_format(rd, stream, mix, &ch, err, errlen)))
        goto done;

    for (;;) {
        DWORD flags = 0;
        IMFSample *sample = NULL;
        hr = IMFSourceReader_ReadSample(rd, stream, 0, NULL, &flags, NULL, &sample);
        if (FAILED(hr) || (flags & MF_SOURCE_READERF_ERROR))
            rc = fail(err, errlen, "the audio could not be decoded", FAILED(hr) ? hr : E_FAIL);
        else if (flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED)
            rc = take_format(rd, stream, mix, &ch, err, errlen);
        if (rc || (flags & MF_SOURCE_READERF_ENDOFSTREAM)) {
            if (sample)
                IMFSample_Release(sample);
            if (rc)
                goto done;
            break;
        }
        if (!sample)
            continue;
        IMFMediaBuffer *mb = NULL;
        BYTE *data = NULL;
        DWORD bytes = 0;
        if (SUCCEEDED(IMFSample_ConvertToContiguousBuffer(sample, &mb)) &&
            SUCCEEDED(IMFMediaBuffer_Lock(mb, &data, NULL, &bytes))) {
            if (pkapp_mix_push(mix, (const float *)data, bytes / (sizeof(float) * ch)))
                rc = fail(err, errlen, "out of memory", E_OUTOFMEMORY);
            IMFMediaBuffer_Unlock(mb);
        }
        if (mb)
            IMFMediaBuffer_Release(mb);
        IMFSample_Release(sample);
        if (rc)
            goto done;
    }
    if (pkapp_mix_take(mix, out, n))
        rc = fail(err, errlen, "out of memory", E_OUTOFMEMORY);

done:
    pkapp_mix_free(mix);
    if (want)
        IMFMediaType_Release(want);
    if (rd)
        IMFSourceReader_Release(rd);
    if (SUCCEEDED(mf))
        MFShutdown();
    if (SUCCEEDED(com))
        CoUninitialize();
    free(wpath);
    return rc;
}
