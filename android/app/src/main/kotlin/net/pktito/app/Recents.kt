package net.pktito.app

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
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
 * screen opens each again; and each one's audio in the cache, so that it
 * plays again without its original. The cache keeps 512 MB of that audio,
 * the oldest let go first (audio.rs, CACHE_BYTES), and Android may empty
 * it when storage runs short: a transcript whose audio has gone reads its
 * original again, by the access the system's picker gave, which is kept
 * while the transcript is on the list. On the main thread, where the
 * screen reads it, but for keep, which the transcription's thread calls.
 */
object Recents {
    /** A transcript on the list, as the start screen shows it: finished, in seconds since 1970. */
    class Item(val id: String, val uri: Uri, val name: String, val duration: Double, val finished: Long)

    var items by mutableStateOf<List<Item>>(emptyList())
        private set

    private val main = Handler(Looper.getMainLooper())
    private var pruned = false

    private fun data(context: Context) = context.noBackupFilesDir.path

    /** The cache's folder for the list's audio. */
    private fun audio(context: Context) = File(context.cacheDir, "audio").path

    /** A transcript's audio, as Engine.transcribe wrote it: its id's WAV. */
    fun wav(context: Context, id: String) = File(audio(context), "$id.wav")

    fun refresh(context: Context) {
        items = parse(Engine.recent(data(context)))
    }

    private fun parse(json: String): List<Item> {
        val a = JSONArray(json)
        return (0 until a.length()).map { i ->
            val o = a.getJSONObject(i)
            Item(o.getString("id"), Uri.parse(o.getString("path")), o.getString("name"), o.getDouble("duration"), o.getLong("finished"))
        }
    }

    /**
     * Once a process, before anything is transcribed, as the desktop's
     * window does before it opens: the audio no transcript on the list
     * needs, the copy of a streamed document that a process ended before it
     * let go (readable), and the access to originals no longer on the list.
     */
    fun prune(context: Context) {
        if (pruned || Transcriber.running) return
        pruned = true
        Engine.prune(data(context), audio(context))
        context.cacheDir.listFiles { f -> f.name.startsWith(COPY) }?.forEach { it.delete() }
        release(context, items)
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
     * it is posted.
     */
    fun keep(context: Context, item: JSONObject, transcript: String, peaks: ByteArray) {
        val list = parse(Engine.keep(data(context), audio(context), item.toString(), transcript, peaks))
        release(context, list)
        main.post { items = list }
    }

    /** A transcript of the list as it is kept, for Transcriber.reopen. */
    fun load(context: Context, id: String): String = Engine.openRecent(data(context), id)

    /** One transcript off the list, its audio with it; the file it was made from stays (app.js, forget). */
    fun forget(context: Context, id: String) {
        items = parse(Engine.forget(data(context), audio(context), id))
        release(context, items)
    }

    /** The list emptied, once the person says so (app.js, askClear). */
    fun clear(context: Context) {
        Engine.clearRecent(data(context), audio(context))
        items = emptyList()
        release(context, items)
    }

    /**
     * A transcript's audio read again from its original, once the cache has
     * let it go (app.js, reloadAudio), off the main thread: null once it can
     * play, else why not, in the desktop's words, or the reader's where it
     * could not read the file.
     */
    fun reread(context: Context, id: String, uri: Uri, name: String, duration: Double): String? =
        try {
            readable(context, uri).use { pfd ->
                if (Engine.reread(pfd.fd, name, wav(context, id).path, duration)) null
                else context.getString(R.string.play_changed)
            }
        } catch (e: FileNotFoundException) {
            context.getString(R.string.play_moved)
        } catch (e: SecurityException) {
            // the access a share or Open with gave, which ends with the task
            context.getString(R.string.play_no_access)
        } catch (e: IOException) {
            Log.w(TAG, "reading $name again", e)
            e.message ?: context.getString(R.string.play_cant)
        } catch (e: Exception) {
            // a provider that is gone: its app uninstalled
            Log.w(TAG, "reading $name again", e)
            context.getString(R.string.play_moved)
        }

    private const val TAG = "pktito"

    /** What readable names its copies. */
    const val COPY = "document"
}
