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

    /** Told after each chunk: the fraction of the audio done, and the segments just done. */
    fun interface Progress {
        fun update(done: Double, segments: String)
    }

    /** The model at path and a transcriber for it: a handle for the calls below. */
    @JvmStatic external fun load(path: String): Long

    @JvmStatic external fun free(handle: Long)

    /** The model, the backend and the threads it runs on. */
    @JvmStatic external fun about(handle: Long): String

    /**
     * The recording open at fd, read and transcribed on this thread: the
     * transcript. fd stays the caller's; name is what an error calls it.
     */
    @JvmStatic external fun transcribe(handle: Long, fd: Int, name: String, progress: Progress): String

    /** The transcription running stops before its next chunk. */
    @JvmStatic external fun stop()

    /** A transcript as a file of the kind ext names: txt, md, srt, vtt or json. */
    @JvmStatic external fun format(handle: Long, transcript: String, ext: String, name: String): String
}
