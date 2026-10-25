package net.pktito.app

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.io.IOException
import org.json.JSONArray

/**
 * The one transcription at a time, as the screen shows it: written by
 * TranscribeService's thread, read by MainActivity's composition, which
 * Compose's state lets either do from any thread.
 */
object Transcriber {
    var name by mutableStateOf("")
    /** The fraction of the audio transcribed. */
    var done by mutableStateOf(0.0)
    var text by mutableStateOf("")
    var running by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    /** The model, backend and threads, once the model is loaded. */
    var about by mutableStateOf("")
    /** The last transcript's JSON, for Engine.format to share it from. */
    var transcript by mutableStateOf<String?>(null)

    private var handle = 0L

    fun begin(name: String) {
        this.name = name
        done = 0.0
        text = ""
        error = null
        transcript = null
        running = true
    }

    /** The segments of a chunk, each a line of the text, as they come. */
    fun add(done: Double, segments: String) {
        val segs = JSONArray(segments)
        val lines = (0 until segs.length()).map { segs.getJSONObject(it).getString("text") }
        if (lines.isNotEmpty())
            text = if (text.isEmpty()) lines.joinToString("\n") else text + "\n" + lines.joinToString("\n")
        this.done = done
    }

    /**
     * The model, loaded once for the process: the first .pkf, by name, in
     * the app's no_backup/models, which a backup leaves out; the English one
     * (parakeet-tdt-0.6b-v2) sorts first.
     */
    @Synchronized
    fun model(context: Context): Long {
        if (handle == 0L) {
            val dir = File(context.noBackupFilesDir, "models")
            val pkf = dir.listFiles { f -> f.name.endsWith(".pkf") }?.minByOrNull { it.name }
                ?: throw IOException(context.getString(R.string.no_model, dir.path))
            handle = Engine.load(pkf.path)
            about = Engine.about(handle)
        }
        return handle
    }
}
