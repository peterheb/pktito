package net.pktito.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.io.IOException
import org.json.JSONArray

/** A word of a transcript, its times in seconds. */
class Word(val text: String, val start: Double, val end: Double)

/** A sentence of a transcript, or a cue of its subtitle track (no words): times in seconds. */
class Seg(val start: Double, val end: Double, val text: String, val words: List<Word> = emptyList())

/**
 * The one transcription at a time, as the screen shows it, in the steps the
 * desktop's window names: loading (the model), reading (the audio),
 * transcribing, then done, stopped or error. TranscribeService's thread
 * tells it through post, so that the state only changes on the main
 * thread, where the screen reads it.
 */
object Transcriber {
    var name by mutableStateOf("")
    var step by mutableStateOf("")
    /** The fraction of the audio transcribed. */
    var done by mutableStateOf(0.0)
    /** The audio's length in seconds, once it is read. */
    var duration by mutableStateOf(0.0)
    /** Its loudness for the strip (audio.rs's peaks), once it is read. */
    var peaks by mutableStateOf<ByteArray?>(null)
    val segments = mutableStateListOf<Seg>()
    /** What went wrong, in the engine's words or Android's. */
    var error by mutableStateOf<String?>(null)
    /** The seconds spent transcribing so far, for the pace. */
    var transcribed by mutableStateOf(0.0)
    /** The finished transcript's JSON, for Engine.format and Engine.captions. */
    var transcript by mutableStateOf<String?>(null)
    /** Its cues, made the first time the Captions view is shown. */
    var cues by mutableStateOf<List<Seg>?>(null)
    /** A video's frame rate, for times in frames; 0 for audio. */
    var fps by mutableStateOf(0.0)

    val running: Boolean
        get() = step == "loading" || step == "reading" || step == "transcribing"

    private val main = Handler(Looper.getMainLooper())
    private var handle = 0L

    /** block on the main thread, where the screen reads this state. */
    fun post(block: Transcriber.() -> Unit) {
        main.post { block() }
    }

    fun begin(name: String) {
        this.name = name
        step = "reading"
        done = 0.0
        duration = 0.0
        peaks = null
        segments.clear()
        error = null
        transcribed = 0.0
        transcript = null
        cues = null
        fps = 0.0
    }

    /** Back to the start screen, the transcript let go. */
    fun clear() {
        begin("")
        step = ""
    }

    /** The segments of Engine.transcribe's JSON. */
    fun parse(json: String): List<Seg> {
        val a = JSONArray(json)
        return (0 until a.length()).map { i ->
            val s = a.getJSONObject(i)
            val w = s.getJSONArray("words")
            Seg(
                s.getDouble("start"), s.getDouble("end"), s.getString("text"),
                (0 until w.length()).map { j ->
                    val o = w.getJSONObject(j)
                    Word(o.getString("text"), o.getDouble("start"), o.getDouble("end"))
                },
            )
        }
    }

    /** The cues of Engine.captions' JSON, in seconds. */
    fun parseCues(json: String): List<Seg> {
        val a = JSONArray(json)
        return (0 until a.length()).map { i ->
            val c = a.getJSONObject(i)
            Seg(c.getLong("start") / 1000.0, c.getLong("end") / 1000.0, c.getString("text"))
        }
    }

    /** Where Engine.transcribe writes the audio it heard, to play the transcript back by. */
    fun wav(context: Context) = File(context.cacheDir, "audio/playing.wav")

    /** The model's file: the first .pkf, by name, in the app's no_backup/models, which a backup leaves out. */
    fun modelFile(context: Context): File? =
        File(context.noBackupFilesDir, "models").listFiles { f -> f.name.endsWith(".pkf") }?.minByOrNull { it.name }

    val loaded: Boolean
        @Synchronized get() = handle != 0L

    /** The model, loaded once for the process; the English one (parakeet-tdt-0.6b-v2) sorts first. */
    @Synchronized
    fun model(context: Context): Long {
        if (handle == 0L) {
            val pkf = modelFile(context) ?: throw IOException(context.getString(R.string.no_model))
            handle = Engine.load(pkf.path)
        }
        return handle
    }
}
