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

/**
 * A file of a model's that the catalog names (model.rs, Part), at its place in the models folder,
 * as it stood when the catalog was read: here once its size is the catalog's, as a download is put
 * in place only once its SHA-256 checks; else what a download of it that stopped kept. A test says
 * how it stands instead.
 */
class Part(
    val file: File,
    val bytes: Long,
    val sha256: String,
    val here: Boolean = file.length() == bytes,
    /** Its bytes here: all of them, or what a download that stopped kept. */
    val kept: Long = if (here) bytes else File(file.path + ".part").length().coerceAtMost(bytes),
    /**
     * The bytes its file and its part take, whatever is in them: an older graph an update of the
     * app left counts until the new one takes its place.
     */
    val taken: Long = file.length() + File(file.path + ".part").length(),
) {
    /** Where its download gathers until it checks (model.rs, Wanted). */
    val part: File
        get() = File(file.path + ".part")
}

/**
 * A model the catalog lists (model.rs, Entry), as it stands here (iOS's Offer): its files, the
 * edition this phone downloads (Editions), the one it can transcribe with now, if any, and what a
 * download costs. have, bytes and partial are the edition's, so that the screens say what the
 * desktop's say of a model, as the Mac's window does of its two editions.
 */
class Offer(
    val name: String,
    /** The desktop's key for its words: "english", "multilingual". */
    val key: String,
    /** What it transcribes, as ISO 639-1 codes. */
    val languages: List<String>,
    /** Where its error rate in each language is, for a model of several; "" for none. */
    val accuracy: String,
    /** The whole model file, its encoder pktito's own on the CPU. */
    val whole: Part,
    /** The model file without its encoder, the NPU's edition's with graph. */
    val slim: Part,
    /** Its graph for this phone's NPU, where the catalog has one for the chip (model.rs, Graph). */
    val graph: Part?,
    val edition: Edition,
    val usable: Edition?,
    /**
     * The graph alone, for a whole model here without it (Manage models, Faster): its bytes, 0
     * where none is offered.
     */
    val faster: Long,
) {
    val have: Boolean
        get() = usable != null

    /** The files a download of edition fetches, in order. */
    val parts: List<Part>
        get() = if (edition == Edition.NPU && graph != null) listOf(slim, graph) else listOf(whole)

    /** The bytes a download of edition fetches, all of them. */
    val bytes: Long
        get() = parts.sumOf { it.bytes }

    /**
     * The bytes a download of edition that stopped kept, its files done and its part; 0 once it is
     * here.
     */
    val partial: Long
        get() = if (usable == edition) 0 else parts.sumOf { it.kept }

    /**
     * The file the engine loads: the whole model where it is here, whose encoder runs what the
     * NPU's can't, else the slim one.
     */
    val file: File
        get() = if (whole.here || graph == null) whole.file else slim.file

    /**
     * What its files take here, either edition's, the parts of them and an older graph: what Manage
     * models' Delete frees.
     */
    val taken: Long
        get() = whole.taken + slim.taken + (graph?.taken ?: 0)

    /** Each of its files and their parts, for a delete. */
    val files: List<File>
        get() = listOfNotNull(whole, slim, graph).flatMap { listOf(it.file, it.part) }
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

    fun dir(context: Context) = File(context.noBackupFilesDir, "models")

    private fun ownDir(context: Context) = File(context.noBackupFilesDir, "own")

    private fun prefs(context: Context) =
        context.getSharedPreferences("pktito", Context.MODE_PRIVATE)

    /**
     * The catalog, as it stands in the models folder (Engine.catalog), each model in the editions
     * this phone has (Editions): the NPU's where the catalog has a graph for its chip and the NPU
     * has not refused it, else the CPU's; and the one it can run, the NPU's where the graph is here
     * with a model file, the slim one or the whole, else the CPU's where the whole file is. The
     * chip not yet known, the NPU is none. Any thread.
     */
    fun catalog(context: Context): List<Offer> {
        val dir = dir(context)
        val chip = Editions.chip(context)
        val a = JSONArray(Engine.catalog(dir.path))
        return (0 until a.length()).map { i ->
            val o = a.getJSONObject(i)
            val langs = o.getJSONArray("languages")
            val name = o.getString("name")
            val part = { p: JSONObject ->
                Part(File(dir, p.getString("file")), p.getLong("bytes"), p.getString("sha256"))
            }
            val whole = Part(File(o.getString("file")), o.getLong("bytes"), o.getString("sha256"))
            val slim = part(o.getJSONObject("coreml").getJSONObject("slim"))
            val graphs = o.getJSONArray("qnn")
            val graph =
                (0 until graphs.length())
                    .map { graphs.getJSONObject(it) }
                    .find { !chip.isNullOrEmpty() && it.getString("chip") == chip }
                    ?.let { part(it.getJSONObject("part")) }
            val npu = graph != null && Editions.failure(context, name, graph) == null
            val usable =
                when {
                    npu && graph!!.here && (slim.here || whole.here) -> Edition.NPU
                    whole.here -> Edition.CPU
                    else -> null
                }
            Offer(
                name,
                o.getString("key"),
                (0 until langs.length()).map { langs.getString(it) },
                o.getString("accuracy"),
                whole,
                slim,
                graph,
                if (npu) Edition.NPU else Edition.CPU,
                usable,
                faster = if (npu && usable == Edition.CPU) graph!!.bytes else 0,
            )
        }
    }

    /**
     * What to transcribe with: the person's own file, else the catalog model in use where it is
     * here (model.rs, usable), in the edition this phone runs: its file, and its graph for the NPU
     * where that is the edition.
     */
    class Use(val file: File, val graph: Part?, val offer: Offer?)

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
        val o = catalog(context).find { it.name == name && it.have } ?: return null
        return Use(o.file, o.graph.takeIf { o.usable == Edition.NPU }, o)
    }

    /** The screen's state anew, from the files and the preferences. */
    fun refresh(context: Context) {
        // a copy in that its process went with: unlike a download's part, it
        // cannot go on from where it stopped, and it may hold 600 MB
        if (copying == null) File(ownDir(context), ".copying").delete()
        offers = catalog(context)
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
     * model under it goes without a word. Every file of the model goes, either edition's (iOS's
     * delete), and the graph for the NPU on its way to a whole file here (Faster) with them.
     */
    fun delete(context: Context, o: Offer): String? {
        val inUse = own == null && model == o.name
        val loaded = Transcriber.loadedPath?.takeIf { path -> o.files.any { it.path == path } }
        if (Transcriber.running && (inUse || loaded != null))
            return context.getString(R.string.manage_cant_delete)
        if (Downloads.state?.name == o.name) Downloads.cancel(context)
        loaded?.let(Transcriber::letGo)
        if (!o.files.all(::gone)) return context.getString(R.string.manage_cant_delete)
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
     * (Downloads.cancel is for one that is): every file of a model not here, none of which it can
     * run without the rest, or the graph of one here on the CPU (Faster).
     */
    fun deletePart(context: Context, o: Offer): String? {
        val files =
            when {
                !o.have -> o.files
                o.usable == Edition.CPU -> o.graph?.let { listOf(it.file, it.part) }.orEmpty()
                else -> emptyList()
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
