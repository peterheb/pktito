/* The app's native library (ios/native.sh): the engine, pktito's ane fork,
 * and the Rust core the desktop app shares, behind src-tauri/src/ios.rs.
 * Every char * returned is JSON, the caller's to free with
 * pkapp_free_string; one that failed is {"error": its reason, in English}.
 * Paths are UTF-8, of files the app can read. */
#ifndef PKTITO_APP_H
#define PKTITO_APP_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

void pkapp_free_string(char *s);

/* "neon-dp (ARMv8.2 NEON: SDOT int8, fp32)": a plain string, not JSON */
char *pkapp_backend(void);

/* [model.rs's entries, each with "url", and "coreml": {"slim", "encoder"},
 * each {"file", "bytes", "sha256", "url"}] */
char *pkapp_catalog(void);

/* {"handle", "name", "encoder": false for a slim file, "threads"} */
char *pkapp_load(const char *path);
void pkapp_free(uint64_t handle);

/* The Core ML encoder at path (NAME.mlmodelc) loaded and run once, or
 * with NULL the model's own encoder again: {} or {"error"} */
char *pkapp_use_ane(uint64_t handle, const char *path);

/* The recording at path transcribed: its Transcript, {"duration",
 * "complete", "model", "segments"}. audio(ctx, seconds, fps or 0, peaks,
 * count) once it is read; update(ctx, fraction, segments JSON) after each
 * chunk; both on the calling thread. */
char *pkapp_transcribe(uint64_t handle, const char *path, const char *name, void *ctx,
                       void (*audio)(void *ctx, double seconds, double fps, const uint8_t *peaks, size_t n),
                       void (*update)(void *ctx, double done, const char *segments));
void pkapp_stop(void);

/* {"text"}: a Transcript as txt, md, srt, vtt or json */
char *pkapp_format(const char *transcript, const char *ext, const char *name, bool times);
/* {"cues"} */
char *pkapp_captions(const char *transcript);

/* The recent list, in the folder data: {"items"} */
char *pkapp_recent(const char *data);
char *pkapp_keep(const char *data, const char *item, const char *transcript, const uint8_t *peaks, size_t n);
char *pkapp_open_recent(const char *data, const char *id);
char *pkapp_forget(const char *data, const char *id);
void pkapp_clear_recent(const char *data);
char *pkapp_relink(const char *data, const char *id, const char *path, uint64_t size, uint64_t modified);

/* {"key"}: what a model file pkapp_load refused means, for the app's words */
char *pkapp_trouble(const char *why);

#endif
