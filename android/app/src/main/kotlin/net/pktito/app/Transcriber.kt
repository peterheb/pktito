package net.pktito.app

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
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
 * The NPU's failure with a slim model, which has no encoder of its own to go on with
 * (Transcriber.model): its words, as Transcriber.failed says it.
 */
class NpuFailed(message: String) : IOException(message)

/**
 * The one transcription at a time, as the screen shows it, in the steps the desktop's window names:
 * loading (the model), reading (the audio), transcribing, then done, stopped or error; or a
 * transcript of the recent list, opened again, done. TranscribeService's thread tells it through
 * post, so that the state only changes on the main thread, where the screen reads it.
 */
object Transcriber {
    /**
     * Its place on the recent list, and its copy's name if it was shared: the time it began, in ms
     * (recent.rs, new_id).
     */
    var id by mutableStateOf("")
    /** The recording it reads, as its picker gave it, or Locate… found it again. */
    var uri by mutableStateOf<Uri?>(null)
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
    /**
     * Why its recording does not play (app.js, audioWhy), and whether Locate… can find it again.
     */
    var why by mutableStateOf<String?>(null)
    var locate by mutableStateOf(false)
    /**
     * A recording whose transcription the NPU failed (again), for the screen to keep as it keeps
     * one opened before there was a model, and to transcribe once its model is here.
     */
    var resume by mutableStateOf<Uri?>(null)

    val running: Boolean
        get() = step == "loading" || step == "reading" || step == "transcribing"

    private val main = Handler(Looper.getMainLooper())
    private var handle = 0L
    /** The file the model in handle was loaded from. */
    private var loadedFrom: String? = null
    /** That file and its graph for the NPU, if it was given one: what loaded compares. */
    private var loadedAs: String? = null
    /**
     * The model loaded in an NPU's edition, for the core to hear that a transcription ran on it
     * (ran), which deletes its whole file once the slim one has run.
     */
    private var onNpu: Models.Use? = null

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
        locate = false
    }

    /** Back to the start screen, the transcript let go. */
    fun clear() {
        begin("", null)
        step = ""
    }

    /**
     * The NPU's failure of uri's transcription with a slim model, which has no encoder of its own
     * to go on with, why its words (TranscribeService): its model is in its CPU's edition now, its
     * whole file downloading where it isn't here (npuFailed), and the recording is transcribed
     * again once it is, as the words promise (Peter, 2026-11-04). Its words in a toast, and the
     * start screen, the setup's while the file downloads, with the recording waiting there
     * (resume). On the main thread.
     */
    fun again(uri: Uri, why: String) {
        clear()
        Models.news = why
        resume = uri
    }

    /**
     * A transcript of the recent list (Recents.load's JSON) on the screen again, as it was when it
     * finished (app.js, openRecent), to play from its recording.
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
        transcript =
            JSONObject()
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
                s.getDouble("start"),
                s.getDouble("end"),
                s.getString("text"),
                (0 until w.length()).map { j ->
                    val o = w.getJSONObject(j)
                    Word(o.getString("text"), o.getDouble("start"), o.getDouble("end"))
                },
            )
        }
    }

    /**
     * The cues of Engine.captions' JSON, in seconds, each with its words, which it takes from
     * words, the transcript's in order: the Captions view follows the word playing in them, and a
     * search's ticks stand at them, as the desktop's do (app.js, cuesOf).
     */
    fun parseCues(json: String, words: List<Word>): List<Seg> {
        val a = JSONArray(json)
        return (0 until a.length()).map { i ->
            val c = a.getJSONObject(i)
            val first = c.optInt("first", 0)
            val n = c.optInt("n", 0)
            Seg(
                c.getLong("start") / 1000.0,
                c.getLong("end") / 1000.0,
                c.getString("text"),
                if (first + n <= words.size) words.subList(first, first + n) else emptyList(),
            )
        }
    }

    /** Whether the model in use is the one loaded, in the edition it is in now. */
    @Synchronized
    fun loaded(context: Context): Boolean =
        handle != 0L && loadedAs == Models.usable(context)?.let(::key)

    private fun key(u: Models.Use) = u.file.path + (u.graph?.let { " " + it.path } ?: "")

    /**
     * The file of the model loaded, which a transcription running uses, whichever is in use now.
     */
    val loadedPath: String?
        @Synchronized get() = loadedFrom.takeIf { handle != 0L }

    /**
     * The model loaded from path let go, its file being deleted (Models.delete): the engine maps
     * it, and a deleted file keeps its blocks until it is unmapped. On the main thread with no
     * transcription running, where the handle is in no transcription's hands: begin sets the step
     * there before the transcription's thread starts, and the step leaves running there only once
     * Engine.transcribe has returned (TranscribeService).
     */
    @Synchronized
    fun letGo(path: String) {
        if (handle == 0L || loadedFrom != path) return
        free()
    }

    private fun free() {
        Engine.free(handle)
        handle = 0L
        loadedFrom = null
        loadedAs = null
        onNpu = null
    }

    /**
     * After a transcription (TranscribeService): the model let go once no transcription has used it
     * for IDLE_MS. In the NPU's edition its graph holds DMA buffers that the app's own memory
     * figures don't show, for as long as the process lives: on the Galaxy S24 Ultra, 39 MiB of them
     * before a transcription, 898 MiB after it and 858 MiB a minute after Home (2026-11-04). Read
     * in again, it costs the next transcription 0.8 s (the Galaxy S25 Ultra, chapter 1: 3.71 s
     * against 2.93 s, 2026-11-04). On the main thread, where running is read.
     */
    fun rest() {
        main.removeCallbacks(idle)
        main.postDelayed(idle, IDLE_MS)
    }

    private val idle = Runnable { idleOut() }

    /** rest's model let go, where no transcription has it (letGo). */
    @Synchronized
    private fun idleOut() {
        if (running || handle == 0L) return
        Log.i(TAG, "the model let go, unused for ${IDLE_MS / 1000} s")
        free()
    }

    /**
     * The model in use (Models), loaded once for the process, and again once another is chosen or
     * it is in another edition, the one before let go: on the transcription's thread, before it
     * starts, where nothing else uses it. An update that checked is put in place first
     * (Engine.settle), the model loaded let go for it: the old file ran until then. In the NPU's
     * edition its encoder is the graph a PC compiled for this chip, which QNN reads in under a
     * second (0.7 s on the Galaxy S24 Ultra, 2026-11-02); one QNN refuses is the NPU's refusal
     * (refuse), and a model whose whole file is here goes on with that file's encoder.
     */
    @Synchronized
    fun model(context: Context): Long {
        // the chip, for the edition, where the launch's question of it has not come back
        Editions.chipNow(context)
        val dir = Models.dir(context).path
        var use = Models.usable(context) ?: throw IOException(context.getString(R.string.no_model))
        val name = use.offer?.name
        if (name != null && Engine.pending(dir, name)) {
            if (handle != 0L) free()
            try {
                Engine.settle(dir, name)
                Log.i(TAG, "$name: the update in place")
            } catch (e: IOException) {
                // the old file runs on, and settle is tried at the next load
                Log.w(TAG, "$name: the update not put in place: ${e.message}")
            }
            val app = context.applicationContext
            main.post { Models.refresh(app) }
            use = Models.usable(context) ?: throw IOException(context.getString(R.string.no_model))
        }
        if (handle != 0L && loadedAs != key(use)) free()
        if (handle == 0L) {
            handle = Engine.load(use.file.path)
            loadedFrom = use.file.path
            loadedAs = key(use)
            val graph = use.graph
            val o = use.offer
            if (graph != null && o != null)
                try {
                    val s = Engine.npu(handle, graph.path, Editions.libs(context))
                    Log.i(TAG, "the NPU's graph for ${o.name} in %.2f s".format(s))
                    onNpu = use
                } catch (e: IOException) {
                    val refused = refuse(context, use, e.message ?: "")
                    val whole = o.whole.file
                    // the whole model's encoder runs it, as its CPU's edition,
                    // said so in a toast; a slim model has none, and its
                    // transcription waits for the whole file (again)
                    if (!whole.isFile) {
                        free()
                        throw NpuFailed(failed(context, refused))
                    }
                    if (refused) {
                        val app = context.applicationContext
                        main.post { Models.news = Editions.cpuWords(app) }
                    }
                    if (use.file != whole) {
                        free()
                        handle = Engine.load(whole.path)
                        loadedFrom = whole.path
                    }
                    loadedAs = whole.path
                }
        }
        return handle
    }

    /**
     * A transcription finished with words, on its thread: with the NPU's edition loaded, the core
     * hears so (Engine.ran), and the whole file goes where the slim one ran it. The NPU failing a
     * chunk before then, the whole file is still here for the CPU's edition to go on with. Not the
     * whole file loaded with a graph: its encoder runs a chunk the NPU fails without saying so
     * (core/encoder.c), so a transcription with it proves nothing, and the core keeps the file.
     */
    @Synchronized
    fun ran(context: Context) {
        val u = onNpu ?: return
        val o = u.offer ?: return
        val edition = u.edition ?: return
        if (handle == 0L || loadedFrom != u.file.path) return
        onNpu = null
        if (Engine.ran(Models.dir(context).path, o.name, edition, u.file.path)) {
            Log.i(TAG, "${o.name}: the whole file deleted, the NPU's edition in its place")
            val app = context.applicationContext
            main.post { Models.refresh(app) }
        }
    }

    /**
     * A transcription's failure on the NPU with the slim model, which has no encoder of its own to
     * go on with (Engine.transcribe's npu.failed), for why: the model let go, its failure the NPU's
     * (refuse). Its words, for again. On the transcription's thread.
     */
    @Synchronized
    fun npuFailed(context: Context, why: String): String {
        val u = Models.usable(context)
        if (handle != 0L) free()
        return failed(context, u?.graph != null && refuse(context, u, why))
    }

    /**
     * What a slim model's failure on the NPU says: that the NPU can't run the model, or for a graph
     * that wasn't the catalog's, that it is damaged, as Manage models then offers the catalog's.
     */
    private fun failed(context: Context, refused: Boolean) =
        if (refused) Editions.cpuWords(context) else context.getString(R.string.model_file_damaged)

    /**
     * The NPU's refusal of u's graph (Engine.npu), or its failure in a transcription
     * (Engine.transcribe's npu.failed), for why, told to the core (Engine.refuse): the graph
     * deleted, and where it was the catalog's the refusal recorded for this system and build, the
     * model then the CPU's edition. Nothing downloads by itself (Peter, 2026-11-05): without the
     * whole file, Manage models and the setup screen offer it, and the recording waits for it
     * (again). Not tried twice: no failure of QNN's has been seen to pass. Whether it was the
     * catalog's graph refused. Any thread.
     */
    private fun refuse(context: Context, u: Models.Use, why: String): Boolean {
        val o = u.offer ?: return false
        val edition = u.edition ?: return false
        val app = context.applicationContext
        val refused =
            try {
                Engine.refuse(Models.dir(app).path, o.name, edition, Editions.device(app), why)
            } catch (e: IOException) {
                Log.w(TAG, "the NPU's refusal of ${o.name} not recorded: ${e.message}")
                false
            }
        Log.w(
            TAG,
            "the NPU's graph for ${o.name}${if (refused) "" else ", not the catalog's"}: $why",
        )
        main.post { Models.refresh(app) }
        return refused
    }

    private const val TAG = "pktito"
    private const val IDLE_MS = 30_000L
}
