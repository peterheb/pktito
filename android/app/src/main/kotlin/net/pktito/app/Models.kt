package net.pktito.app

import android.content.Context
import android.content.res.Resources
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.os.storage.StorageManager
import android.provider.OpenableColumns
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.io.IOException
import java.nio.file.Files
import java.security.MessageDigest
import kotlin.concurrent.thread
import org.json.JSONArray
import org.json.JSONObject

/** A file of the catalog's (model.rs, Part), at its place in the models folder. */
class Part(val file: File, val bytes: Long, val sha256: String)

/**
 * What runs a model here (editions.rs, Runs): its edition, the model file the engine loads, and the
 * NPU's graph beside it, for an NPU's edition.
 */
class Runs(val edition: String, val model: File, val graph: File?) {
    val npu: Boolean
        get() = graph != null
}

/**
 * What Manage models offers for a model (editions.rs, Offer): Download where nothing of it runs,
 * Faster where the CPU's edition runs and an NPU's is chosen, Update where a file of the edition
 * that runs is an older build than the catalog's. Its files as Engine.download takes them, their
 * bytes, and those a download that stopped kept.
 */
class Get(val kind: String, val bytes: Long, val kept: Long, val files: List<String>) {
    companion object {
        const val DOWNLOAD = "download"
        const val FASTER = "faster"
        const val UPDATE = "update"
    }
}

/**
 * A model the catalog lists, as the core sees it here (editions.rs, View; docs/EDITIONS.md), which
 * decides for every platform: the edition chosen for this phone, what runs, why the NPU refused the
 * edition that fits best, and the offer. The core reads the folder; this keeps its answer and the
 * paths of the model's files, for what they take and for a delete.
 */
class Offer(
    val name: String,
    /** The desktop's key for its words: "english", "multilingual". */
    val key: String,
    /** What it transcribes, as ISO 639-1 codes. */
    val languages: List<String>,
    /** Where its error rate in each language is, for a model of several; "" for none. */
    val accuracy: String,
    /** The edition chosen for this phone: "cpu", "qnn-sm8650". */
    val chosen: String,
    /** What runs, null for nothing here that does. */
    val runs: Runs?,
    /**
     * Why the NPU refused the edition that fits best, on this system with this build of the app and
     * this graph; null where it didn't. With nothing running, it is why: the NPU stopped and the
     * CPU's edition is to download, not a model never downloaded.
     */
    val refused: String?,
    val get: Get?,
    /** The bytes its files take here, or the chosen edition's where none is (View.bytes). */
    val size: Long,
    /** Each of its editions' files in the models folder, the whole .pkf, the slim one, graphs. */
    val files: List<File>,
    /** The CPU's edition's file, the whole .pkf, for a person's own copy of it (Models.take). */
    val whole: Part,
    /**
     * What its files take here, every edition's, their parts and replacements waiting, whatever is
     * in them, when the catalog was read: what Manage models' Delete frees.
     */
    val taken: Long = files.sumOf {
        it.length() + File(it.path + ".part").length() + File(it.path + ".next").length()
    },
) {
    val have: Boolean
        get() = runs != null

    private fun of(kind: String) = get?.takeIf { it.kind == kind }

    /** The bytes a first download fetches: the chosen edition's. */
    val bytes: Long
        get() = of(Get.DOWNLOAD)?.bytes ?: size

    /** The bytes a first download that stopped kept; 0 once it is here. */
    val partial: Long
        get() = of(Get.DOWNLOAD)?.kept ?: 0

    /**
     * The NPU's edition for a model here on the CPU (Manage models, Faster): the bytes of its files
     * not here yet, the slim one and the graph, which take the whole file's place once the NPU has
     * run a transcription (Engine.ran): 727 MB of English, where the whole file and the graph keep
     * 1339. 0 where none is offered.
     */
    val faster: Long
        get() = of(Get.FASTER)?.bytes ?: 0

    /** The catalog's build of a file of the edition that runs, an older one here: its bytes. */
    val update: Long
        get() = of(Get.UPDATE)?.bytes ?: 0
}

/**
 * A model file of the person's own on its way in: its name, the bytes copied of its size (0 where
 * its app does not say), then its check.
 */
class Copy(val name: String, val got: Long, val total: Long, val checking: Boolean = false)

/**
 * The models (the desktop's model.rs): the catalog's, downloaded into no_backup/models, which a
 * backup leaves out (Downloads); a model file of the person's own, copied into no_backup/own; and
 * which one is in use, as the desktop's settings keep it, a catalog model by name, or the own file
 * instead, here in the app's preferences. The state is the screen's, changed on the main thread.
 */
object Models {
    var offers by mutableStateOf<List<Offer>>(emptyList())
    /** The catalog model in use, by name. */
    var model by mutableStateOf<String?>(null)
    /** The person's own model file, in use instead when there is one. */
    var own by mutableStateOf<File?>(null)
    /** The model file to transcribe with (usable); null until there is one. */
    var ready by mutableStateOf<File?>(null)
    /** A file of the person's own on its way in. */
    var copying by mutableStateOf<Copy?>(null)
    /** A word for the screen's toast once a model is in use: "English model ready". */
    var news by mutableStateOf<String?>(null)

    private val main = Handler(Looper.getMainLooper())
    @Volatile private var stopCopy = false
    /** What refresh last told the log of the catalog. */
    private var told = ""

    /** Whether this process has run start's once-a-process work. On the main thread. */
    private var started = false

    fun dir(context: Context) = File(context.noBackupFilesDir, "models")

    private fun ownDir(context: Context) = File(context.noBackupFilesDir, "own")

    private fun prefs(context: Context) =
        context.getSharedPreferences("pktito", Context.MODE_PRIVATE)

    /**
     * The catalog, each model as the core sees it in the models folder for this phone
     * (Engine.catalog, Editions.device): the chip not yet known, the NPU is none. Any thread.
     */
    fun catalog(context: Context): List<Offer> {
        val dir = dir(context)
        val a = JSONArray(Engine.catalog(dir.path, Editions.device(context)))
        return (0 until a.length()).map { i -> offer(dir, a.getJSONObject(i)) }
    }

    /** A model of Engine.catalog's JSON, its files in dir. */
    internal fun offer(dir: File, o: JSONObject): Offer {
        val langs = o.getJSONArray("languages")
        val runs =
            o.optJSONObject("runs")?.let { r ->
                Runs(
                    r.getString("edition"),
                    File(r.getString("model")),
                    r.optString("encoder")
                        .takeUnless { r.isNull("encoder") || it.isEmpty() }
                        ?.let(::File),
                )
            }
        val get =
            o.optJSONObject("offer")?.let { g ->
                val f = g.getJSONArray("files")
                Get(
                    g.getString("kind"),
                    g.getLong("bytes"),
                    g.getLong("kept"),
                    (0 until f.length()).map { f.getString(it) },
                )
            }
        val eds = o.getJSONArray("editions")
        val whole =
            (0 until eds.length())
                .map { eds.getJSONObject(it) }
                .first { it.getJSONObject("runtime").getString("kind") == "cpu" }
                .getJSONObject("model")
                .let {
                    Part(
                        File(dir, it.getString("file")),
                        it.getLong("bytes"),
                        it.getString("sha256"),
                    )
                }
        val files =
            (0 until eds.length())
                .flatMap { i ->
                    val ed = eds.getJSONObject(i)
                    listOfNotNull(ed.getJSONObject("model"), ed.optJSONObject("encoder"))
                }
                .map { it.getString("file") }
                .distinct()
                .map { File(dir, it) }
        return Offer(
            o.getString("name"),
            o.getString("key"),
            (0 until langs.length()).map { langs.getString(it) },
            o.getString("accuracy"),
            o.getString("chosen"),
            runs,
            o.optString("refused").takeUnless { o.isNull("refused") || it.isEmpty() },
            get,
            o.getLong("bytes"),
            files,
            whole,
        )
    }

    /**
     * What to transcribe with: the person's own file, else the catalog model in use where something
     * of it runs here (Offer.runs): its file, its graph for the NPU where that is the edition.
     */
    class Use(val file: File, val graph: File?, val offer: Offer?) {
        val edition: String?
            get() = offer?.runs?.edition
    }

    /** The model to transcribe with (Use), or null where there is none here. Any thread. */
    fun usable(context: Context): Use? {
        val p = prefs(context)
        p.getString("model_file", null)
            ?.let(::File)
            ?.takeIf { it.isFile }
            ?.let {
                return Use(it, null, null)
            }
        val name = p.getString("model", null) ?: return null
        val o = catalog(context).find { it.name == name } ?: return null
        val r = o.runs ?: return null
        return Use(r.model, r.graph, o)
    }

    /**
     * Once a process, off the main thread: the core's records given to files from before 0.9.6 and
     * swaps cut short finished (Engine.adopt), 0.9.6's refusals forgotten (Editions.forgetOld),
     * then the screen's state anew.
     */
    fun start(context: Context) {
        if (started) return
        started = true
        val app = context.applicationContext
        thread(name = "models") {
            Editions.forgetOld(app)
            Engine.adopt(dir(app).path)
            main.post { refresh(app) }
        }
    }

    /** The screen's state anew, from the files and the preferences. */
    fun refresh(context: Context) {
        // a copy in that its process went with: unlike a download's part, it
        // cannot go on from where it stopped, and it may hold 600 MB
        if (copying == null) File(ownDir(context), ".copying").delete()
        offers = catalog(context)
        // the core's view in the log, each time it changes: what runs, what is
        // offered, a refusal
        val said =
            offers.joinToString("; ") {
                "${it.name}: chosen ${it.chosen}, runs ${it.runs?.edition}, " +
                    "offer ${it.get?.kind} ${it.get?.bytes ?: 0}" +
                    (it.refused?.let { why -> ", refused: $why" } ?: "")
            }
        if (said != told) Log.i(TAG, said)
        told = said
        val p = prefs(context)
        model = p.getString("model", null)
        own = p.getString("model_file", null)?.let(::File)?.takeIf { it.isFile }
        ready = usable(context)?.file
    }

    /**
     * The catalog model of that name in use, which is here. The own file is let go and deleted, as
     * the desktop's menu lets it go (choose_model): it was a copy, and it took as much room as a
     * model.
     */
    fun use(context: Context, name: String) {
        prefs(context).edit().putString("model", name).remove("model_file").apply()
        ownDir(context).listFiles()?.forEach { it.delete() }
    }

    /**
     * The catalog model to use once the file in use goes (Manage models): for the person's own file
     * (deleting null), the catalog model it stood in for where that is here, else any downloaded;
     * for the catalog model in use, another downloaded. Null for none, and the screen is setup's.
     */
    fun next(offers: List<Offer>, model: String?, deleting: String?): Offer? {
        val here = offers.filter { it.have && it.name != deleting }
        return here.find { it.name == model } ?: here.firstOrNull()
    }

    /**
     * Manage models' deletes, each on the main thread and none of a model a transcription has in
     * its hands, the one other user of a model file (Transcriber.running: the model in use, or the
     * one loaded), whose engine freed under Engine.transcribe frees what it is reading. Manage
     * greys those Deletes and puts away a question asked before the transcription began; these
     * refuse one all the same, for a file opened with pktito in the frame before Manage is drawn
     * again. Each says what its toast says, if anything: the model now in use, where the one in use
     * gave way to it, or that the delete failed, the system's words for why in the log. A model
     * goes with the transcriber's copy where that is the one loaded (Transcriber.letGo): the engine
     * maps the file (core/os.c), and a file deleted while mapped keeps its 600 MB on the disk until
     * the process ends (the Tab S7 FE, 2026-10-30: the English model deleted after a transcription
     * with it, 606 MB free at once and no .pkf left in the process's maps). The model in use gives
     * way to the next (next); with none, none is in use, the setup screen's choice still the model
     * deleted, to download again. The own file, while it is in use, stays in use, and the catalog
     * model under it goes without a word. Every file of the model goes, every edition's, with their
     * records, a download's parts and an update waiting, and its refusals (Engine.delete).
     */
    fun delete(context: Context, o: Offer): String? {
        val inUse = own == null && model == o.name
        val loaded = Transcriber.loadedPath?.takeIf { path -> o.files.any { it.path == path } }
        if (Transcriber.running && (inUse || loaded != null))
            return context.getString(R.string.manage_cant_delete)
        if (Downloads.state?.name == o.name) Downloads.cancel(context)
        loaded?.let(Transcriber::letGo)
        Engine.delete(dir(context).path, o.name)
        if (o.files.any { it.exists() }) return context.getString(R.string.manage_cant_delete)
        return taken(context, if (inUse) next(offers, model, o.name) else null)
    }

    /**
     * The person's own file deleted, the copy of it the app kept: the file they opened is theirs,
     * and stays where it is.
     */
    fun deleteOwn(context: Context): String? {
        val f = own ?: return null
        if (Transcriber.running) return context.getString(R.string.manage_cant_delete)
        Transcriber.letGo(f.path)
        if (!gone(f)) return context.getString(R.string.manage_cant_delete)
        prefs(context).edit().remove("model_file").apply()
        return taken(context, next(offers, model, null))
    }

    /**
     * What a download that stopped kept, deleted where no download is under way for it
     * (Downloads.cancel is for one that is): its offer's files (Offer.get), their parts and any
     * that checked and wait to replace one here, but never a file of what runs, which an Update
     * would have replaced.
     */
    fun deletePart(context: Context, o: Offer): String? {
        val dir = dir(context)
        val runs = listOfNotNull(o.runs?.model, o.runs?.graph)
        val files =
            o.get
                ?.files
                .orEmpty()
                .map { File(dir, it) }
                .flatMap { f ->
                    listOf(
                        File(f.path + ".part"),
                        File(f.path + ".next"),
                        File(f.path + ".next.sha256"),
                    ) + if (f in runs) emptyList() else listOf(f, File(f.path + ".sha256"))
                }
        if (!files.all(::gone)) return context.getString(R.string.manage_cant_delete)
        refresh(context)
        return null
    }

    /** f deleted, or already not there; why not, in the log. */
    private fun gone(f: File): Boolean =
        try {
            Files.deleteIfExists(f.toPath())
            true
        } catch (e: IOException) {
            Log.w(TAG, "deleting $f", e)
            false
        }

    /**
     * After a delete, then in use if there is one to give way to, said so (models.using), and the
     * screen's state anew.
     */
    private fun taken(context: Context, then: Offer?): String? {
        if (then != null) use(context, then.name)
        refresh(context)
        return then?.let {
            context.getString(R.string.models_using, label(context.resources, it.key))
        }
    }

    /** The room left where the models are, as the system's storage settings count it. */
    fun free(context: Context): Long = context.noBackupFilesDir.usableSpace

    /** A catalog model's words, as the desktop's window has them (catalog.KEY). */
    fun label(res: Resources, key: String): String =
        res.getString(
            when (key) {
                "english" -> R.string.catalog_english
                "multilingual" -> R.string.catalog_multilingual
                else -> return key
            }
        )

    fun about(res: Resources, key: String): String =
        res.getString(
            when (key) {
                "english" -> R.string.catalog_english_about
                "multilingual" -> R.string.catalog_multilingual_about
                else -> return ""
            }
        )

    /**
     * A model file the person chose, copied in from uri and checked by the engine, on a thread of
     * its own: then in use, as the desktop's Open a model file uses one (main.rs, use_file), and
     * told says so (true), or says what is wrong with it (false), and it is gone again. A document
     * comes as a stream from whichever app holds it, a cloud drive's with no file to map, so it is
     * copied rather than used where it is. A copy that is a catalog model, byte for byte (a
     * download from models.pktito.net in a browser), is that model, as the desktop's choose_file
     * has a catalog model's own file.
     */
    fun open(context: Context, uri: Uri, told: (String, Boolean) -> Unit) {
        val app = context.applicationContext
        val (shown, size) = nameAndSize(app, uri)
        // its own name, where nothing else of the app's is
        val name =
            shown.substringAfterLast('/').takeUnless { it.isEmpty() || it.startsWith('.') }
                ?: "model.pkf"
        stopCopy = false
        copying = Copy(name, 0, size)
        thread(name = "model file") {
            val dir = ownDir(app).apply { mkdirs() }
            val part = File(dir, ".copying")
            val said =
                try {
                    // a model's 600 MB that will not fit is said before it is copied,
                    // since a copy, unlike a download, cannot go on from where it stopped
                    if (!room(app, size)) throw IOException("ENOSPC: no room for $size bytes")
                    val sha = copy(app, uri, part, name, size)
                    if (sha == null) {
                        part.delete()
                        null
                    } else {
                        main.post {
                            copying = Copy(name, part.length(), part.length(), checking = true)
                        }
                        take(app, name, part, sha)
                    }
                } catch (e: IOException) {
                    Log.w(TAG, "copying $name", e)
                    part.delete()
                    app.getString(
                        if ("ENOSPC" in (e.message ?: "")) R.string.download_disk_full
                        else R.string.model_file_cant_open
                    ) to false
                }
            main.post {
                copying = null
                refresh(app)
                if (said != null) told(said.first, said.second)
            }
        }
    }

    /** The copy in, let go, deleted. */
    fun stopCopying() {
        stopCopy = true
    }

    /**
     * Whether the app's storage has room for bytes more, made if Android can make it: for an app
     * that asks, it deletes other apps' cached files (StorageManager.allocateBytes), which
     * File.usableSpace does not count. Not on the main thread, as freeing them can take a while.
     */
    fun room(context: Context, bytes: Long): Boolean {
        if (bytes <= 0) return true
        val storage = context.getSystemService(StorageManager::class.java)
        return try {
            val uuid = storage.getUuidForPath(context.noBackupFilesDir)
            if (bytes > storage.getAllocatableBytes(uuid)) return false
            storage.allocateBytes(uuid, bytes)
            true
        } catch (e: IOException) {
            Log.w(TAG, "room for $bytes bytes", e)
            false
        }
    }

    /**
     * uri's bytes into part, at most ten times a second saying how far: their SHA-256, or null if
     * the copy was stopped.
     */
    private fun copy(app: Context, uri: Uri, part: File, name: String, size: Long): String? {
        val hash = MessageDigest.getInstance("SHA-256")
        val input =
            app.contentResolver.openInputStream(uri) ?: throw IOException("$uri: nothing to read")
        input.use {
            part.outputStream().use { out ->
                val buf = ByteArray(1 shl 20)
                var got = 0L
                var shown = 0L
                while (true) {
                    if (stopCopy) return null
                    val k = input.read(buf)
                    if (k < 0) break
                    out.write(buf, 0, k)
                    hash.update(buf, 0, k)
                    got += k
                    val now = SystemClock.elapsedRealtime()
                    if (now - shown >= 100) {
                        shown = now
                        val at = got
                        main.post { if (copying != null) copying = Copy(name, at, size) }
                    }
                }
                out.fd.sync()
            }
        }
        return hash.digest().joinToString("") { "%02x".format(it) }
    }

    /**
     * The copy at part, its SHA-256 sha, in use if the engine loads it: a catalog model's bytes as
     * that model, else as the own file, the one before it deleted. What is now in use, and true;
     * else what was wrong, in the person's words.
     */
    private fun take(app: Context, name: String, part: File, sha: String): Pair<String, Boolean> {
        try {
            Engine.free(Engine.load(part.path))
        } catch (e: IOException) {
            val why = e.message ?: ""
            Log.w(TAG, "$name: $why")
            part.delete()
            return words(app.resources, Engine.trouble(why)) to false
        }
        val res = app.resources
        val same = catalog(app).find { it.whole.sha256 == sha && it.whole.bytes == part.length() }
        if (same != null) {
            dir(app).mkdirs()
            if (!part.renameTo(same.whole.file))
                throw IOException("${same.whole.file}: cannot move the model there")
            // its record, as a download's (editions.rs, have), the copy checked here
            File(same.whole.file.path + ".sha256").writeText("$sha\n")
            use(app, same.name)
            return res.getString(R.string.models_using, label(res, same.key)) to true
        }
        ownDir(app).listFiles()?.filter { it != part }?.forEach { it.delete() }
        val file = File(ownDir(app), name)
        if (!part.renameTo(file)) throw IOException("$file: cannot move the model there")
        prefs(app).edit().putString("model_file", file.path).apply()
        return res.getString(R.string.models_using_file, name) to true
    }

    /** The document's name and size, as its app gives them; 0 for a size it does not. */
    private fun nameAndSize(app: Context, uri: Uri): Pair<String, Long> =
        app.contentResolver
            .query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE),
                null,
                null,
                null,
            )
            ?.use { c ->
                if (!c.moveToFirst()) null
                else (c.getString(0) ?: "") to (if (c.isNull(1)) 0L else c.getLong(1))
            } ?: ((uri.lastPathSegment ?: "") to 0L)

    private const val TAG = "pktito"
}

/**
 * A problem's words, the desktop's for its key (strings.rs, Problem; ui/ locales/en.json): a
 * download's, a model file's, a recording's that can't be read (Engine.problem), and anything else
 * as something gone wrong. host is the server's name, for the words that say it.
 */
fun words(res: Resources, key: String?, host: String = ""): String =
    when (key) {
        "download.damaged" -> res.getString(R.string.download_damaged)
        "download.not_answering" -> res.getString(R.string.download_not_answering, host)
        "download.not_found" -> res.getString(R.string.download_not_found)
        "download.cant_reach" -> res.getString(R.string.download_cant_reach, host)
        "download.connection_lost" -> res.getString(R.string.download_connection_lost)
        "download.too_much" -> res.getString(R.string.download_too_much, host)
        "download.cant_check" -> res.getString(R.string.download_cant_check)
        "download.disk_full" -> res.getString(R.string.download_disk_full)
        "download.cant_save" -> res.getString(R.string.download_cant_save)
        "model_file.newer" -> res.getString(R.string.model_file_newer)
        "model_file.not_model" -> res.getString(R.string.model_file_not_model)
        "model_file.cant_open" -> res.getString(R.string.model_file_cant_open)
        "model_file.damaged" -> res.getString(R.string.model_file_damaged)
        "decode.unreadable" -> res.getString(R.string.decode_unreadable)
        "decode.not_media" -> res.getString(R.string.decode_not_media)
        "decode.no_audio" -> res.getString(R.string.decode_no_audio)
        "decode.not_found" -> res.getString(R.string.decode_not_found)
        "decode.cant_convert" -> res.getString(R.string.decode_cant_convert)
        "problem.memory" -> res.getString(R.string.problem_memory)
        else -> res.getString(R.string.problem_unexpected)
    }
