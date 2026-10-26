package net.pktito.app

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.IOException
import org.json.JSONArray
import org.json.JSONObject

/** A word of a transcript, its times in seconds. */
class Word(val text: String, val start: Double, val end: Double)

/** A sentence of a transcript, or a cue of its subtitle track (no words): times in seconds. */
class Seg(val start: Double, val end: Double, val text: String, val words: List<Word> = emptyList())

/**
 * The one transcription at a time, as the screen shows it, in the steps the
 * desktop's window names: loading (the model), reading (the audio),
 * transcribing, then done, stopped or error; or a transcript of the recent
 * list, opened again, done. TranscribeService's thread tells it through
 * post, so that the state only changes on the main thread, where the
 * screen reads it.
 */
object Transcriber {
    /** Its place on the recent list, and its audio's name in the cache: the time it began, in ms (recent.rs, new_id). */
    var id by mutableStateOf("")
    /** The recording it reads, as its picker gave it. */
    var uri: Uri? = null
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
    /** Opened from the recent list, not transcribed in this run. */
    var reopened by mutableStateOf(false)
    /** Why its audio does not play, or not yet: read again from its original (app.js, audioWhy). */
    var why by mutableStateOf<String?>(null)

    val running: Boolean
        get() = step == "loading" || step == "reading" || step == "transcribing"

    private val main = Handler(Looper.getMainLooper())
    private var handle = 0L
    /** The file the model in handle was loaded from. */
    private var loadedFrom: String? = null

    /** block on the main thread, where the screen reads this state. */
    fun post(block: Transcriber.() -> Unit) {
        main.post { block() }
    }

    fun begin(name: String, uri: Uri?) {
        id = if (uri == null) "" else System.currentTimeMillis().toString()
        this.uri = uri
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
        reopened = false
        why = null
    }

    /** Back to the start screen, the transcript let go. */
    fun clear() {
        begin("", null)
        step = ""
    }

    /**
     * A transcript of the recent list (Recents.load's JSON) on the screen
     * again, as it was when it finished (app.js, openRecent); its audio is
     * the cache's, or its original's, read again.
     */
    fun reopen(id: String, json: String) {
        val o = JSONObject(json)
        val segs = o.getJSONArray("segments")
        val loud = o.getJSONArray("peaks")
        begin(o.getString("name"), Uri.parse(o.getString("path")))
        this.id = id
        duration = o.getDouble("duration")
        fps = o.optDouble("fps", 0.0)
        peaks = ByteArray(loud.length()) { loud.getInt(it).toByte() }
        segments.addAll(parse(segs))
        transcribed = o.optDouble("transcribed", 0.0)
        // what Engine.format and Engine.captions take, as Engine.transcribe gave it
        transcript = JSONObject()
            .put("duration", duration)
            .put("complete", true)
            .put("model", o.getString("model"))
            .put("segments", segs)
            .toString()
        done = 1.0
        reopened = true
        step = "done"
    }

    /** The segments of Engine.transcribe's JSON. */
    fun parse(json: String): List<Seg> = parse(JSONArray(json))

    private fun parse(a: JSONArray): List<Seg> {
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

    /** Where Engine.transcribe writes the audio it heard, to play the transcript back by: the cache's, at its id. */
    fun wav(context: Context) = Recents.wav(context, id)

    /** Whether the model in use is the one loaded. */
    @Synchronized
    fun loaded(context: Context): Boolean = handle != 0L && loadedFrom == Models.usable(context)?.path

    /**
     * The model in use (Models), loaded once for the process, and again
     * once another is chosen, the one before let go: on the
     * transcription's thread, before it starts, where nothing else uses it.
     */
    @Synchronized
    fun model(context: Context): Long {
        val pkf = Models.usable(context) ?: throw IOException(context.getString(R.string.no_model))
        if (handle != 0L && loadedFrom != pkf.path) {
            Engine.free(handle)
            handle = 0L
        }
        if (handle == 0L) {
            handle = Engine.load(pkf.path)
            loadedFrom = pkf.path
        }
        return handle
    }
}
