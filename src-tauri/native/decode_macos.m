/* The audio of a media file, through AVFoundation: whatever macOS can play
 * (mp3, mp4, m4a, mov, wav, aiff, caf, aac, flac, ...), its first audio
 * track decoded to 32-bit float at the decoder's own rate and handed to the
 * mix (mix.h). AVFoundation is asked for no conversion of rate: what its
 * resampler makes differs a little from run to run.
 *
 * 0 on success, with *out malloc'd (the caller frees it) holding *n
 * samples; -1 if the file has no audio track; -2 if AVFoundation cannot
 * read it, the reason in err. */
#import <AVFoundation/AVFoundation.h>
#import <AudioToolbox/AudioToolbox.h>
#include <stdlib.h>
#include <string.h>

#include "mix.h"

static int fail(char *err, size_t errlen, NSError *e, const char *what) {
    snprintf(err, errlen, "%s", e ? e.localizedDescription.UTF8String : what);
    return -2;
}

/* Where each channel plays, from a buffer's channel layout. 0 when it has
 * none, or has a channel without a place. */
static int speakers(CMAudioFormatDescriptionRef fd, UInt32 ch, unsigned char *where) {
    size_t size = 0;
    const AudioChannelLayout *l = CMAudioFormatDescriptionGetChannelLayout(fd, &size);
    if (!l)
        return 0;
    /* a layout named by a tag or a bitmap, spelt out a channel at a time */
    AudioChannelLayout *spelt = NULL;
    if (l->mChannelLayoutTag != kAudioChannelLayoutTag_UseChannelDescriptions) {
        int bitmap = l->mChannelLayoutTag == kAudioChannelLayoutTag_UseChannelBitmap;
        AudioFormatPropertyID prop = bitmap ? kAudioFormatProperty_ChannelLayoutForBitmap
                                            : kAudioFormatProperty_ChannelLayoutForTag;
        const void *spec = bitmap ? (const void *)&l->mChannelBitmap : (const void *)&l->mChannelLayoutTag;
        UInt32 sz = 0;
        if (AudioFormatGetPropertyInfo(prop, sizeof(UInt32), spec, &sz) != noErr || !(spelt = malloc(sz)) ||
            AudioFormatGetProperty(prop, sizeof(UInt32), spec, &sz, spelt) != noErr) {
            free(spelt);
            return 0;
        }
        l = spelt;
    }
    int ok = l->mNumberChannelDescriptions == ch;
    for (UInt32 c = 0; ok && c < ch; c++) {
        AudioChannelLabel k = l->mChannelDescriptions[c].mChannelLabel;
        if (k == kAudioChannelLabel_Center || k == kAudioChannelLabel_Mono)
            where[c] = PKAPP_CENTER;
        else if (k == kAudioChannelLabel_LFEScreen || k == kAudioChannelLabel_LFE2)
            where[c] = PKAPP_LFE;
        else if (k == kAudioChannelLabel_Left || k == kAudioChannelLabel_Right || k == kAudioChannelLabel_LeftCenter ||
                 k == kAudioChannelLabel_RightCenter)
            where[c] = PKAPP_FRONT;
        else if (k >= 1 && k < kAudioChannelLabel_UseCoordinates)
            where[c] = PKAPP_SURROUND;
        else
            ok = 0;
    }
    free(spelt);
    return ok;
}

/* One buffer into the mix, telling it the format first when it is new. */
static int feed(pkapp_mix *mix, CMSampleBufferRef sb, CMFormatDescriptionRef *now, UInt32 *ch, float **tmp,
                size_t *cap, char *err, size_t errlen) {
    CMAudioFormatDescriptionRef fd = CMSampleBufferGetFormatDescription(sb);
    if (fd && (!*now || !CMFormatDescriptionEqual(fd, *now))) {
        const AudioStreamBasicDescription *d = CMAudioFormatDescriptionGetStreamBasicDescription(fd);
        unsigned char where[PKAPP_MAX_CHANNELS];
        *ch = d ? d->mChannelsPerFrame : 0;
        if (!d || !*ch || *ch > PKAPP_MAX_CHANNELS || d->mSampleRate < 1 ||
            pkapp_mix_format(mix, (unsigned)(d->mSampleRate + 0.5), *ch, speakers(fd, *ch, where) ? where : NULL))
            return fail(err, errlen, nil, "its audio's format cannot be converted");
        if (*now)
            CFRelease(*now);
        *now = (CMFormatDescriptionRef)CFRetain(fd);
    }
    CMBlockBufferRef bb = CMSampleBufferGetDataBuffer(sb);
    size_t k = bb ? CMBlockBufferGetDataLength(bb) / sizeof(float) : 0;
    if (!k || !*ch)
        return 0;
    if (k > *cap) {
        float *more = realloc(*tmp, k * sizeof *more);
        if (!more)
            return fail(err, errlen, nil, "out of memory");
        *tmp = more;
        *cap = k;
    }
    if (CMBlockBufferCopyDataBytes(bb, 0, k * sizeof(float), *tmp) != kCMBlockBufferNoErr ||
        pkapp_mix_push(mix, *tmp, k / *ch))
        return fail(err, errlen, nil, "out of memory");
    return 0;
}

int pkapp_decode_os(const char *path, int rate, float **out, size_t *n, char *err, size_t errlen) {
    @autoreleasepool {
        *out = NULL;
        *n = 0;
        NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:nil];

        /* the asset's audio tracks, loaded the asynchronous way and waited for */
        __block NSArray<AVAssetTrack *> *tracks = nil;
        __block NSError *loadError = nil;
        dispatch_semaphore_t loaded = dispatch_semaphore_create(0);
        [asset loadTracksWithMediaType:AVMediaTypeAudio
                     completionHandler:^(NSArray<AVAssetTrack *> *t, NSError *e) {
                         tracks = t;
                         loadError = e;
                         dispatch_semaphore_signal(loaded);
                     }];
        dispatch_semaphore_wait(loaded, DISPATCH_TIME_FOREVER);
        if (loadError)
            return fail(err, errlen, loadError, "the file could not be opened");
        if (tracks.count == 0)
            return -1;
        /* the first track alone, as ffmpeg (0:a:0) and Media Foundation take
         * it: a film's other tracks are other languages or commentary */
        AVAssetTrack *track = tracks.firstObject;

        /* the rate the decoder makes - for HE-AAC twice the rate the track
         * names - asked for by name, so that nothing is resampled */
        double native = 0;
        CMAudioFormatDescriptionRef fd = (__bridge CMAudioFormatDescriptionRef)track.formatDescriptions.firstObject;
        if (fd) {
            const AudioFormatListItem *richest = CMAudioFormatDescriptionGetRichestDecodableFormat(fd);
            const AudioStreamBasicDescription *d = CMAudioFormatDescriptionGetStreamBasicDescription(fd);
            native = richest ? richest->mASBD.mSampleRate : d ? d->mSampleRate : 0;
        }

        NSError *e = nil;
        AVAssetReader *reader = [AVAssetReader assetReaderWithAsset:asset error:&e];
        if (!reader)
            return fail(err, errlen, e, "the file could not be read");
        NSMutableDictionary *settings = [@{
            AVFormatIDKey : @(kAudioFormatLinearPCM),
            AVLinearPCMBitDepthKey : @32,
            AVLinearPCMIsFloatKey : @YES,
            AVLinearPCMIsBigEndianKey : @NO,
            AVLinearPCMIsNonInterleaved : @NO,
        } mutableCopy];
        if (native > 0)
            settings[AVSampleRateKey] = @(native);
        AVAssetReaderTrackOutput *output = [AVAssetReaderTrackOutput assetReaderTrackOutputWithTrack:track
                                                                                      outputSettings:settings];
        output.alwaysCopiesSampleData = NO;
        if (![reader canAddOutput:output])
            return fail(err, errlen, nil, "its audio cannot be converted");
        [reader addOutput:output];
        if (![reader startReading])
            return fail(err, errlen, reader.error, "the audio could not be read");

        pkapp_mix *mix = pkapp_mix_new((unsigned)rate);
        CMFormatDescriptionRef now = NULL;
        UInt32 ch = 0;
        float *tmp = NULL;
        size_t cap = 0;
        int rc = mix ? 0 : fail(err, errlen, nil, "out of memory");
        CMSampleBufferRef sb;
        while (!rc && (sb = [output copyNextSampleBuffer])) {
            rc = feed(mix, sb, &now, &ch, &tmp, &cap, err, errlen);
            CFRelease(sb);
        }
        if (rc)
            [reader cancelReading];
        else if (reader.status != AVAssetReaderStatusCompleted)
            rc = fail(err, errlen, reader.error, "the audio could not be read to its end");
        else if (pkapp_mix_take(mix, out, n))
            rc = fail(err, errlen, nil, "out of memory");
        if (now)
            CFRelease(now);
        free(tmp);
        pkapp_mix_free(mix);
        return rc;
    }
}

/* The frame rate of the file's first video track, as AVFoundation names
 * it; 0 for a file with none, or one it cannot read. For the window's times
 * in frames (ui/app.js). */
double pkapp_video_rate_os(const char *path) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:nil];
        __block AVAssetTrack *video = nil;
        dispatch_semaphore_t loaded = dispatch_semaphore_create(0);
        [asset loadTracksWithMediaType:AVMediaTypeVideo
                     completionHandler:^(NSArray<AVAssetTrack *> *t, NSError *e) {
                         if (!e)
                             video = t.firstObject;
                         dispatch_semaphore_signal(loaded);
                     }];
        dispatch_semaphore_wait(loaded, DISPATCH_TIME_FOREVER);
        return video ? video.nominalFrameRate : 0;
    }
}
