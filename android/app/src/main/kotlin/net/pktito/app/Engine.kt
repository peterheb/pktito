package net.pktito.app

/**
 * The engine and the desktop app's Rust core, in libpktito_app.so
 * (native.sh builds it; src-tauri/src/android.rs is this object's other
 * half). A transcript crosses as JSON: its duration, whether it is complete,
 * and its segments, each a sentence with its words and their times.
 * What fails comes back as an IOException that says why.
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

    /** The model at path and a transcriber for it: a handle for the calls below. */
    @JvmStatic external fun load(path: String): Long

    @JvmStatic external fun free(handle: Long)

    /** The model, the backend and the threads it runs on. */
    @JvmStatic external fun about(handle: Long): String

    /** The engine's code for this processor, by its name and what it uses: "neon-dp (ARMv8.2 NEON: SDOT int8, fp32)". */
    @JvmStatic external fun backend(): String

    /** What the catalog calls the model of that name ("English"), or "" for one it does not list. */
    @JvmStatic external fun label(name: String): String

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
    @JvmStatic external fun format(handle: Long, transcript: String, ext: String, name: String, times: Boolean): String

    /** A transcript's subtitle track, as its .srt has it: JSON, each cue's start and end in ms, and its text. */
    @JvmStatic external fun captions(handle: Long, transcript: String): String
}
