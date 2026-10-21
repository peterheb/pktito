/* The audio of a media file, through AVFoundation: whatever macOS can play
 * (mp3, mp4, m4a, mov, wav, aiff, caf, aac, flac, ...), its audio tracks
 * mixed to one channel and resampled by AVFoundation itself to the rate
 * asked for, as 32-bit float samples.
 *
 * 0 on success, with *out malloc'd (the caller frees it) holding *n
 * samples; -1 if the file has no audio track; -2 if AVFoundation cannot
 * read it, the reason in err. */
#import <AVFoundation/AVFoundation.h>
#include <stdlib.h>
#include <string.h>

static int fail(char *err, size_t errlen, NSError *e, const char *what) {
    snprintf(err, errlen, "%s", e ? e.localizedDescription.UTF8String : what);
    return -2;
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

        NSError *e = nil;
        AVAssetReader *reader = [AVAssetReader assetReaderWithAsset:asset error:&e];
        if (!reader)
            return fail(err, errlen, e, "the file could not be read");
        NSDictionary *settings = @{
            AVFormatIDKey : @(kAudioFormatLinearPCM),
            AVSampleRateKey : @(rate),
            AVNumberOfChannelsKey : @1,
            AVLinearPCMBitDepthKey : @32,
            AVLinearPCMIsFloatKey : @YES,
            AVLinearPCMIsBigEndianKey : @NO,
            AVLinearPCMIsNonInterleaved : @NO,
        };
        AVAssetReaderAudioMixOutput *output =
            [AVAssetReaderAudioMixOutput assetReaderAudioMixOutputWithAudioTracks:tracks
                                                                    audioSettings:settings];
        output.alwaysCopiesSampleData = NO;
        if (![reader canAddOutput:output])
            return fail(err, errlen, nil, "its audio cannot be converted");
        [reader addOutput:output];
        if (![reader startReading])
            return fail(err, errlen, reader.error, "the audio could not be read");

        size_t cap = (size_t)rate * 60, len = 0;
        float *buf = malloc(cap * sizeof *buf);
        if (!buf)
            return fail(err, errlen, nil, "out of memory");
        CMSampleBufferRef sb;
        while ((sb = [output copyNextSampleBuffer])) {
            CMBlockBufferRef bb = CMSampleBufferGetDataBuffer(sb);
            size_t k = bb ? CMBlockBufferGetDataLength(bb) / sizeof(float) : 0;
            if (len + k > cap) {
                while (len + k > cap)
                    cap *= 2;
                float *more = realloc(buf, cap * sizeof *buf);
                if (!more) {
                    CFRelease(sb);
                    free(buf);
                    return fail(err, errlen, nil, "out of memory");
                }
                buf = more;
            }
            if (k && CMBlockBufferCopyDataBytes(bb, 0, k * sizeof(float), buf + len) == kCMBlockBufferNoErr)
                len += k;
            CFRelease(sb);
        }
        if (reader.status != AVAssetReaderStatusCompleted) {
            free(buf);
            return fail(err, errlen, reader.error, "the audio could not be read to its end");
        }
        *out = buf;
        *n = len;
        return 0;
    }
}
