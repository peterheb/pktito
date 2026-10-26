package net.pktito.app

/**
 * The engine and the desktop app's Rust core, in libpktito_app.so
 * (native.sh builds it; src-tauri/src/android.rs is this object's other
 * half). A transcript crosses as JSON: its duration, whether it is complete,
 * the model that made it, and its segments, each a sentence with its words
 * and their times; the catalog, a download's state and the recent list
 * cross as JSON too. What fails comes back as an IOException that says why.
 */
object Engine {
    init {
        System.loadLibrary("pktito_app")
    }

    /** Told what the transcription has got to, on its thread. */
    interface Progress {
        /** The audio is read: its length in seconds, a video's frame rate (0 for audio), and its loudness in slices, 0 to 255 each. */
        fun audio(duration: Double, fps: Double, peaks: ByteArray)

        /** After each chunk: the fraction of the audio done, and the segments just done. */
        fun update(done: Double, segments: String)
    }

    /** Told how a model's download stands, on its thread. */
    interface Download {
        /** download.rs's State, as JSON: the model, its step, the bytes got of its total, the speed, the server, what went wrong. */
        fun state(json: String)
    }

    /** The model at path and a transcriber for it: a handle for the calls below. */
    @JvmStatic external fun load(path: String): Long

    @JvmStatic external fun free(handle: Long)

    /** The model, the backend and the threads it runs on. */
    @JvmStatic external fun about(handle: Long): String

    /** The engine's code for this processor, by its name and what it uses: "neon-dp (ARMv8.2 NEON: SDOT int8, fp32)". */
    @JvmStatic external fun backend(): String

    /**
     * The recording open at fd, read and transcribed on this thread: the
     * transcript. fd stays the caller's; name is what an error calls it. The
     * samples as pktito hears them go to wav, a 16-bit WAV to play the
     * transcript back by, in step with its times.
     */
    @JvmStatic external fun transcribe(handle: Long, fd: Int, name: String, wav: String, progress: Progress): String

    /** The transcription running stops before its next chunk. */
    @JvmStatic external fun stop()

    /**
     * A transcript as a file of the kind ext names: txt, md, srt, vtt or
     * json, the desktop's bytes; times puts each sentence's time before it
     * in txt and md.
     */
    @JvmStatic external fun format(transcript: String, ext: String, name: String, times: Boolean): String

    /** A transcript's subtitle track, as its .srt has it: JSON, each cue's start and end in ms, and its text. */
    @JvmStatic external fun captions(transcript: String): String

    /**
     * The recent list in data, the app's no_backup folder, newest first, as
     * JSON: each transcript's id, the URI it was read from (path), its name,
     * length (duration), when it finished in seconds since 1970, what it
     * took, the model and a video's frame rate (the desktop's recent.rs).
     */
    @JvmStatic external fun recent(data: String): String

    /**
     * A finished transcript at the top of the list, in place of an earlier
     * one of the same file: item, the list's fields, as JSON; transcript,
     * what transcribe gave; peaks, its loudness. The audio of those let go
     * goes from audio, the cache's folder, and the rest is held to what the
     * cache keeps. The list after.
     */
    @JvmStatic external fun keep(data: String, audio: String, item: String, transcript: String, peaks: ByteArray): String

    /** A transcript of the list as it is kept: the list's fields, its peaks and its segments, as JSON. */
    @JvmStatic external fun openRecent(data: String, id: String): String

    /** One transcript off the list, its audio in the cache with it; the file it was made from stays. The list after. */
    @JvmStatic external fun forget(data: String, audio: String, id: String): String

    /** The list emptied, and its audio. */
    @JvmStatic external fun clearRecent(data: String, audio: String)

    /** Before anything is transcribed: the audio no transcript on the list needs let go, and the rest held to what the cache keeps. */
    @JvmStatic external fun prune(data: String, audio: String)

    /**
     * A transcript's audio read again from its original, open at fd, and
     * written to wav as transcribe writes it, if its length is still the
     * transcript's (true); false, and nothing written, where the file has
     * changed since.
     */
    @JvmStatic external fun reread(fd: Int, name: String, wav: String, duration: Double): Boolean

    /** The models models.pktito.net serves, as JSON, each as it stands in dir, the app's models folder: downloaded, or the bytes a download kept. */
    @JvmStatic external fun catalog(dir: String): String

    /**
     * The catalog model name downloaded into dir, or its download gone on
     * with from what it kept, on this thread, checked against its size and
     * SHA-256, and tried again by itself while the network is away (the
     * desktop's download.rs): progress hears each change. The checked
     * file's path, or "" where it stopped short, paused, cancelled or
     * failed, as the last state says.
     */
    @JvmStatic external fun download(dir: String, name: String, progress: Download): String

    /** The download running stops before its next read, keeping what it has. */
    @JvmStatic external fun pauseDownload()

    /** The download running, waiting to try again after the network failed it, tries now. */
    @JvmStatic external fun retryDownload()

    /** The download of name stops and what it had is deleted: by its own thread, which says so, if it runs (true), else here (false). */
    @JvmStatic external fun cancelDownload(dir: String, name: String): Boolean

    /** What a model file that load refused for why means, as the desktop's key for its words ("model_file.not_model"). */
    @JvmStatic external fun trouble(why: String): String
}
