package net.pktito.app

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException
import org.json.JSONArray
import org.json.JSONObject

/**
 * The recent list, the desktop's (src-tauri/src/recent.rs): the 50 latest
 * finished transcripts, newest first, kept in the app's no_backup folder,
 * which Android leaves out of backups and transfers, so that the start
 * screen opens each again, and plays it from its recording (Player). The
 * recording is the file it was read from, by the access the system's
 * picker gave, which is kept while the transcript is on the list; one
 * shared to pktito, or opened with it, Android takes the access to back
 * once the task ends, so it is copied as it arrives, its bytes as they
 * are, as a phone's apps keep what is shared to them, and the copy goes
 * with its transcript. On the main thread, where the screen reads it, but
 * for keep and copy, which the transcription's thread calls, and open.
 */
object Recents {
    /** A transcript on the list, as the start screen shows it: finished, in seconds since 1970; its file's size and last change as transcribed, 0 where not known. */
    class Item(
        val id: String,
        val uri: Uri,
        val name: String,
        val duration: Double,
        val finished: Long,
        val size: Long,
        val modified: Long,
    )

    var items by mutableStateOf<List<Item>>(emptyList())
        private set

    private val main = Handler(Looper.getMainLooper())
    private var pruned = false

    private fun data(context: Context) = context.noBackupFilesDir.path

    /** The copies of recordings shared to pktito, each by its transcript's id. */
    private fun copies(context: Context) = File(context.noBackupFilesDir, "copies")

    /** A transcript's copy of its recording, there if it was shared to pktito. */
    fun copyOf(context: Context, id: String) = File(copies(context), id)

    fun refresh(context: Context) {
        items = parse(Engine.recent(data(context)))
    }

    private fun parse(json: String): List<Item> {
        val a = JSONArray(json)
        return (0 until a.length()).map { i ->
            val o = a.getJSONObject(i)
            Item(
                o.getString("id"), Uri.parse(o.getString("path")), o.getString("name"), o.getDouble("duration"),
                o.getLong("finished"), o.optLong("size", 0), o.optLong("modified", 0),
            )
        }
    }

    /**
     * Once a process, before anything is transcribed, as the desktop's
     * window prunes before it opens: the copies no transcript on the list
     * has, and those a process ended before it finished; the audio cache
     * of the versions before this one, which played what pktito heard
     * (cache/audio); the copy of a streamed document that a process ended
     * before it let go (readable); and the access to originals no longer
     * on the list.
     */
    fun prune(context: Context) {
        if (pruned || Transcriber.running) return
        pruned = true
        File(context.cacheDir, "audio").deleteRecursively()
        context.cacheDir.listFiles { f -> f.name.startsWith(COPY) }?.forEach { it.delete() }
        drop(context, items)
        release(context, items)
    }

    /** The copies of transcripts not on list, nor the one on the screen. */
    private fun drop(context: Context, list: List<Item>) {
        val keep = list.mapTo(HashSet()) { it.id } + Transcriber.id
        copies(context).listFiles { f -> f.name !in keep }?.forEach { it.delete() }
    }

    /**
     * The access to uri kept past this run, for as long as its transcript is
     * on the list, where Android allows it: a document the system's picker
     * opened allows it, one shared or opened with pktito mostly not.
     */
    fun hold(context: Context, uri: Uri) {
        try {
            context.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        } catch (e: SecurityException) {
            Log.i(TAG, "no lasting access to $uri")
        }
    }

    /** Whether uri can be read again in a later run: an access kept (hold), or a file's own path. */
    private fun held(context: Context, uri: Uri): Boolean =
        uri.scheme == "file" || context.contentResolver.persistedUriPermissions.any { it.uri == uri && it.isReadPermission }

    /**
     * The recording at uri copied for transcript id to play from, where it
     * could not be otherwise: shared to pktito or opened with it, without an
     * access that lasts, or from a provider that streams it (a cloud
     * drive's, with no size), which a player cannot seek. Null where it is
     * not needed, or could not be made (no room): the recording then plays
     * from uri while it can. On the transcription's thread.
     */
    fun copy(context: Context, uri: Uri, id: String): File? {
        val streams = try {
            context.contentResolver.openFileDescriptor(uri, "r")?.use { it.statSize < 0 } ?: false
        } catch (e: Exception) {
            false
        }
        if (held(context, uri) && !streams) return null
        val dir = copies(context)
        dir.mkdirs()
        val part = File(dir, "$id.part")
        return try {
            val input = context.contentResolver.openInputStream(uri) ?: return null
            input.use { i -> part.outputStream().use { i.copyTo(it, 1 shl 20) } }
            val f = copyOf(context, id)
            if (part.renameTo(f)) f else null
        } catch (e: IOException) {
            Log.w(TAG, "copying $uri", e)
            null
        } finally {
            part.delete()
        }
    }

    /**
     * A document's size and last change, in seconds since 1970: 0 for either
     * not known. The size is the file's own where the document opens as one
     * (file): its provider's can lag what was written to it, MediaStore's
     * 800,936 bytes for a file an append had made 800,942 (on the emulator,
     * 2026-10-26). The last change is the provider's, which a cloud drive
     * keeps as the document's, where the file it opens is a cache's.
     */
    fun stamp(context: Context, uri: Uri, file: ParcelFileDescriptor? = null): Pair<Long, Long> {
        if (uri.scheme == "file") {
            val f = File(uri.path ?: return 0L to 0L)
            return f.length() to f.lastModified() / 1000
        }
        fun column(name: String): Long = try {
            context.contentResolver.query(uri, arrayOf(name), null, null, null)?.use { c ->
                if (c.moveToFirst() && !c.isNull(0)) c.getLong(0) else 0L
            } ?: 0L
        } catch (e: Exception) {
            // a provider that has no such column, as a share's often has not
            0L
        }
        // statSize is -1 for a stream
        val own = file?.statSize ?: try {
            context.contentResolver.openFileDescriptor(uri, "r")?.use { it.statSize } ?: -1L
        } catch (e: Exception) {
            -1L
        }
        val size = if (own >= 0) own else column(OpenableColumns.SIZE)
        return size to column(DocumentsContract.Document.COLUMN_LAST_MODIFIED) / 1000
    }

    /** A recording to play (file), or why there is none, in the screen's words, and whether Locate… can find it. */
    class Source(val file: ParcelFileDescriptor?, val why: String?, val locate: Boolean)

    /**
     * Transcript id's recording, open to play: its copy, if it was shared to
     * pktito, or the file at uri, if it is the one transcribed (recent.rs,
     * Item.changed: by its size, then its last change, where both sides
     * know them). Off the main thread.
     */
    fun open(context: Context, id: String, uri: Uri, size: Long, modified: Long): Source {
        val copy = copyOf(context, id)
        if (copy.isFile) return Source(ParcelFileDescriptor.open(copy, ParcelFileDescriptor.MODE_READ_ONLY), null, false)
        fun not(why: Int, locate: Boolean = true) = Source(null, context.getString(why), locate)
        return try {
            val pfd = context.contentResolver.openFileDescriptor(uri, "r") ?: return not(R.string.play_moved)
            // a provider that streams it: a player cannot seek it
            if (pfd.statSize < 0) {
                pfd.close()
                return not(R.string.play_cant, false)
            }
            val (s, m) = stamp(context, uri, pfd)
            val differ = { a: Long, b: Long -> a != 0L && b != 0L && a != b }
            if (differ(size, s) || differ(modified, m)) {
                pfd.close()
                not(R.string.play_changed)
            } else Source(pfd, null, false)
        } catch (e: FileNotFoundException) {
            not(R.string.play_moved)
        } catch (e: SecurityException) {
            // the access a share or Open with gave, which ends with the task;
            // or a document the picker gave, whose access Android takes back
            // when it is deleted (a Downloads one, on the emulator)
            not(if (DocumentsContract.isDocumentUri(context, uri)) R.string.play_moved else R.string.play_no_access)
        } catch (e: Exception) {
            // a provider that is gone: its app uninstalled
            Log.w(TAG, "opening $uri", e)
            not(R.string.play_moved)
        }
    }

    /**
     * The access held to originals that no transcript on list needs, nor the
     * one at work, given back: Android keeps 512 for an app at most (128
     * before Android 11), and a person's files are theirs.
     */
    private fun release(context: Context, list: List<Item>) {
        val keep = list.mapTo(HashSet()) { it.uri } + listOfNotNull(Transcriber.uri)
        for (p in context.contentResolver.persistedUriPermissions)
            if (p.uri !in keep)
                runCatching { context.contentResolver.releasePersistableUriPermission(p.uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
    }

    /**
     * A finished transcript at the top of the list, in place of an earlier
     * one of the same file, as the desktop keeps one (main.rs, job): item,
     * the list's fields; transcript, Engine.transcribe's; peaks, its
     * loudness. On the transcription's thread; the list is the screen's once
     * it is posted, and the copies of those it let go go then.
     */
    fun keep(context: Context, item: JSONObject, transcript: String, peaks: ByteArray) {
        val list = parse(Engine.keep(data(context), item.toString(), transcript, peaks))
        main.post {
            items = list
            drop(context, list)
            release(context, list)
        }
    }

    /** A transcript of the list as it is kept, for Transcriber.reopen. */
    fun load(context: Context, id: String): String = Engine.openRecent(data(context), id)

    /** One transcript off the list, and its copy; the file it was made from stays (app.js, forget). */
    fun forget(context: Context, id: String) {
        items = parse(Engine.forget(data(context), id))
        copyOf(context, id).delete()
        release(context, items)
    }

    /** The list emptied, once the person says so (app.js, askClear), and the copies with it. */
    fun clear(context: Context) {
        Engine.clearRecent(data(context))
        items = emptyList()
        copies(context).deleteRecursively()
        release(context, items)
    }

    /**
     * Transcript id's recording found again at uri, picked by the person
     * (Locate…): its access kept, and its place on the list, if it is the
     * one transcribed, by its size where both know it: true. False, and
     * nothing kept, where it is another.
     */
    fun relink(context: Context, id: String, uri: Uri): Boolean {
        val item = items.find { it.id == id } ?: return false
        val (size, modified) = stamp(context, uri)
        if (item.size != 0L && size != 0L && item.size != size) return false
        hold(context, uri)
        items = parse(Engine.relink(data(context), id, uri.toString(), size, modified))
        release(context, items)
        return true
    }

    private const val TAG = "pktito"

    /** What readable names its copies. */
    const val COPY = "document"
}
