/* The app's native library (ios/native.sh): the engine, pktito with its
 * Neural Engine encoder, and the Rust core the desktop app shares, behind
 * src-tauri/src/ios.rs.
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

/* The models folder dir, kept by editions.rs's rule (docs/EDITIONS.md) for
 * device, editions.rs's Device in JSON: {"system": "ios", "os": [major,
 * minor], "here"}. */

/* [each model as editions::view sees it: the catalog's entry with "chosen",
 * "runs": {"edition", "model", "encoder"} or null, "refused", "bytes", and
 * "offer": {"kind", "bytes", "kept", "files", "parts": [{"file", "bytes",
 * "sha256", "url"}]} or null] */
char *pkapp_catalog(const char *dir, const char *device);
/* Once a launch, off the main thread: records for files from before 0.9.6,
 * swaps cut short finished */
void pkapp_adopt(const char *dir);
/* A file of name's that URLSession brought, at got, checked and put where it
 * lands with its record: {"path"}, or {"error", "key", "detail"} */
char *pkapp_land(const char *dir, const char *name, const char *file, const char *got);
/* Whether a download of name's checked and waits to be put in place */
bool pkapp_pending(const char *dir, const char *name);
/* What waits put in place, with no engine holding name's files: {} or {"error"} */
char *pkapp_settle(const char *dir, const char *name);
/* What replaces nothing put in place, a new Core ML encoder unpacked: {} or {"error"} */
char *pkapp_settle_new(const char *dir, const char *name);
/* The Neural Engine refused name's edition for why: {"refused": whether the
 * refusal holds, the encoder having been the catalog's} */
char *pkapp_refuse(const char *dir, const char *name, const char *edition, const char *device, const char *why);
/* A transcription finished on name's edition from model: whether the whole
 * .pkf went, the Neural Engine's edition having run from its slim file */
bool pkapp_ran(const char *dir, const char *name, const char *edition, const char *model);
/* Every file of name's editions, and its refusals */
void pkapp_delete(const char *dir, const char *name);

/* {"handle", "name", "encoder": false for a slim file, "threads"} */
char *pkapp_load(const char *path);
void pkapp_free(uint64_t handle);

/* The Core ML encoder at path (NAME.mlmodelc) loaded and run once, or
 * with NULL the model's own encoder again: {} or {"error"} */
char *pkapp_use_ane(uint64_t handle, const char *path);

/* The recording at path transcribed: its Transcript, {"duration",
 * "complete", "model", "segments"}, and with diar (a pkapp_load_diarizer
 * handle, 0 for none) each segment's "speaker" and the "stretches" and
 * "diarizer". audio(ctx, seconds, fps or 0, peaks, count) once it is read;
 * update(ctx, fraction, segments JSON) after each chunk; both on the
 * calling thread. */
char *pkapp_transcribe(uint64_t handle, uint64_t diar, const char *path, const char *name, void *ctx,
                       void (*audio)(void *ctx, double seconds, double fps, const uint8_t *peaks, size_t n),
                       void (*update)(void *ctx, double done, const char *segments));
void pkapp_stop(void);

/* {"text"}: a Transcript as txt, md, srt, vtt or json, with each turn's
 * speaker where speakers */
char *pkapp_format(const char *transcript, const char *ext, const char *name, bool times, bool speakers);

/* The speaker model this build offers: {"offer": {"name", "license",
 * "file", "bytes", "url", "kept", "have"}} or {"offer": null} */
char *pkapp_diarizer(const char *dir);
/* Its file that URLSession brought, checked and put in place: {"path"} or
 * {"error", "key", "detail"} */
char *pkapp_land_diarizer(const char *dir, const char *got);
/* Its files deleted, or with part what a download kept: {} or {"error"} */
char *pkapp_delete_diarizer(const char *dir, bool part);
/* {"handle"}: the speaker model at path, loaded, for pkapp_transcribe and
 * pkapp_find_speakers */
char *pkapp_load_diarizer(const char *path);
void pkapp_free_diarizer(uint64_t handle);
/* Who said what in a finished Transcript, from its recording at path, not
 * transcribed again: the Transcript with its speakers. done(ctx, fraction
 * heard, bar JSON {"closed": ended since the last, "open"}) a whole percent
 * at a time, on the calling thread. */
char *pkapp_find_speakers(uint64_t diar, const char *path, const char *name, const char *transcript,
                          void *ctx, void (*done)(void *ctx, double done, const char *bar));

/* The recent list, in the folder data: {"items"} */
char *pkapp_recent(const char *data);
char *pkapp_keep(const char *data, const char *item, const char *transcript, const uint8_t *peaks, size_t n);
/* A kept transcript's speakers as transcript has them now: {} or {"error"} */
char *pkapp_keep_speakers(const char *data, const char *id, const char *transcript);
char *pkapp_open_recent(const char *data, const char *id);
char *pkapp_forget(const char *data, const char *id);
void pkapp_clear_recent(const char *data);
char *pkapp_relink(const char *data, const char *id, const char *path, uint64_t size, uint64_t modified);

/* {"key"}: what a model file pkapp_load refused means, for the app's words */
char *pkapp_trouble(const char *why);

#endif
