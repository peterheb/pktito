package net.pktito.app

import android.Manifest
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContract
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.IntentCompat
import java.util.Locale
import kotlinx.coroutines.delay
import net.pktito.app.ui.PktitoTheme
import net.pktito.app.ui.Ready
import net.pktito.app.ui.Toast
import net.pktito.app.ui.Work
import net.pktito.app.ui.WorkActions
import net.pktito.app.ui.clock
import net.pktito.app.ui.pk

/**
 * The screen, the desktop window's: open a recording, watch its transcript
 * come, play it, and copy, share or save it. The system's pickers grant the
 * one document picked and the one saved, as Open with and Share grant
 * theirs, so the app asks for no access to files; the work is
 * TranscribeService's.
 */
class MainActivity : ComponentActivity() {
    private val open = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) start(uri)
    }
    private val notify = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}
    private val create = registerForActivityResult(CreateText) { uri -> if (uri != null) write(uri) }

    private val prefs by lazy { getSharedPreferences("pktito", MODE_PRIVATE) }
    /** Save as's choice: each sentence's time before it in .txt and .md. */
    private var times by mutableStateOf(false)
    /** The transcript as its subtitle track's cues, not its sentences. */
    private var captions by mutableStateOf(false)
    private var toast by mutableStateOf<String?>(null)
    /** The format being saved while the system's dialog is up. */
    private var saving: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        times = prefs.getBoolean("times", false)
        captions = prefs.getBoolean("captions", false)
        saving = savedInstanceState?.getString("saving")
        setContent { PktitoTheme { App() } }
        if (savedInstanceState == null) opened(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        opened(intent)
    }

    /** A recording opened with pktito, or shared to it: transcribed, unless one is under way. */
    private fun opened(i: Intent) {
        val uri = when (i.action) {
            Intent.ACTION_VIEW -> i.data
            Intent.ACTION_SEND -> IntentCompat.getParcelableExtra(i, Intent.EXTRA_STREAM, Uri::class.java)
            else -> null
        }
        if (uri != null && !Transcriber.running) start(uri)
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putString("saving", saving)
    }

    // the transcript's audio stops with the screen; the transcription does not
    override fun onStop() {
        super.onStop()
        Player.pause(this)
    }

    @Composable
    private fun App() {
        val t = Transcriber
        val list = rememberLazyListState()
        val model = remember { Transcriber.modelFile(this) }
        val label = remember(model) { model?.let { Engine.label(it.nameWithoutExtension) } ?: "" }
        val review = t.step == "done" || t.step == "stopped"
        val rows = if (captions && review) t.cues ?: emptyList() else t.segments

        // the finished transcript's audio, ready to play; another file's let go
        LaunchedEffect(t.step) {
            if (review) Player.load(this@MainActivity, Transcriber.wav(this@MainActivity))
            else Player.release(this@MainActivity)
        }
        // where playback has got to, each frame while it plays
        LaunchedEffect(Player.playing) {
            while (Player.playing) {
                withFrameNanos { }
                Player.tick()
            }
        }
        // the cues, the first time the transcript is shown as captions
        LaunchedEffect(captions, t.transcript) {
            val tr = t.transcript
            if (captions && tr != null && t.cues == null)
                t.cues = runCatching { Transcriber.parseCues(Engine.captions(Transcriber.model(this@MainActivity), tr)) }
                    .onFailure { Log.w(TAG, "captions", it) }
                    .getOrDefault(emptyList())
        }
        // the transcript's end followed as it comes, for a reader who is there;
        // finished, it starts at its top (ui/app.js, buildMain)
        LaunchedEffect(t.segments.size) {
            val last = list.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
            if (t.running && t.segments.isNotEmpty() && last >= t.segments.size - 3)
                list.animateScrollToItem(t.segments.size - 1)
        }
        LaunchedEffect(review) {
            if (review) list.scrollToItem(0)
        }
        // the row playing kept in sight
        val current = if (Player.playing) rows.indexOfLast { it.start <= Player.position + 1e-3 } else -1
        LaunchedEffect(current) {
            val seen = list.layoutInfo.visibleItemsInfo.map { it.index }
            if (current >= 0 && current !in seen.dropLast(1)) list.animateScrollToItem(current)
        }
        LaunchedEffect(toast) {
            if (toast != null) {
                delay(2000)
                toast = null
            }
        }
        BackHandler(enabled = t.step.isNotEmpty() && !t.running) { back() }

        Box(
            Modifier
                .fillMaxSize()
                .background(pk.ground)
                .windowInsetsPadding(WindowInsets.safeDrawing),
        ) {
            if (t.step.isEmpty())
                Ready(label, model != null) { open.launch(arrayOf("audio/*", "video/*")) }
            else
                Work(
                    name = t.name,
                    step = t.step,
                    done = t.done,
                    duration = t.duration,
                    peaks = t.peaks,
                    rows = rows,
                    error = t.error,
                    stats = if (review && t.segments.isNotEmpty() && t.transcribed > 0)
                        getString(R.string.stats_transcribed, clock(t.transcribed), pace(t.done * t.duration, t.transcribed))
                    else null,
                    pace = if (t.step == "transcribing" && t.done > 0 && t.transcribed > 0) pace(t.done * t.duration, t.transcribed) else null,
                    captions = captions,
                    times = times,
                    playable = Player.ready,
                    playing = Player.playing,
                    position = Player.position,
                    list = list,
                    a = WorkActions(
                        back = ::back,
                        stop = { Engine.stop() },
                        copy = ::copy,
                        share = ::share,
                        save = ::save,
                        setTimes = {
                            times = it
                            prefs.edit().putBoolean("times", it).apply()
                        },
                        setCaptions = {
                            captions = it
                            prefs.edit().putBoolean("captions", it).apply()
                        },
                        play = { Player.toggle(this@MainActivity) },
                        seek = { s, play -> Player.seek(this@MainActivity, s, play) },
                    ),
                )
            toast?.let { Toast(it, Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp)) }
        }
    }

    /** how many seconds of audio a second's work transcribed (ui/app.js, realTime) */
    private fun pace(audio: Double, secs: Double) =
        getString(R.string.real_time, String.format(Locale.getDefault(), "%.1f", audio / secs))

    private fun start(uri: Uri) {
        startForegroundService(
            Intent(this, TranscribeService::class.java)
                .setData(uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION),
        )
        // the progress notification, from Android 13, is the person's to allow
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        )
            notify.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    private fun back() {
        Player.release(this)
        Transcriber.clear()
    }

    /** The transcript in a format, the desktop's bytes for it. */
    private fun text(ext: String, times: Boolean): String? {
        val tr = Transcriber.transcript ?: return null
        return Engine.format(Transcriber.model(this), tr, ext, Transcriber.name, times)
    }

    /** Its text, or as captions its subtitles, on the clipboard, as the desktop's Copy. */
    private fun copy() {
        val text = text(if (captions) "srt" else "txt", false) ?: return
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(Transcriber.name, text))
        // Android 13 and later say so themselves
        if (Build.VERSION.SDK_INT < 33)
            toast = getString(if (captions) R.string.copied_captions else R.string.copied)
    }

    private fun share() {
        val text = text("txt", times) ?: return
        val send = Intent(Intent.ACTION_SEND)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, Transcriber.name)
            .putExtra(Intent.EXTRA_TEXT, text)
        startActivity(Intent.createChooser(send, null))
    }

    private fun save(ext: String) {
        saving = ext
        val stem = Transcriber.name.substringBeforeLast('.').ifEmpty { Transcriber.name }
        create.launch(MIME.getValue(ext) to "$stem.$ext")
    }

    private fun write(uri: Uri) {
        val ext = saving ?: return
        saving = null
        val name = nameOf(uri)
        toast = try {
            val text = text(ext, times) ?: return
            contentResolver.openOutputStream(uri, "wt")!!.use { it.write(text.toByteArray()) }
            getString(R.string.saved, name)
        } catch (e: Exception) {
            Log.w(TAG, "saving $name", e)
            getString(R.string.save_failed, name)
        }
    }

    private fun nameOf(uri: Uri): String =
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        } ?: uri.lastPathSegment ?: ""

    private companion object {
        const val TAG = "pktito"
        /** what each format is, for the system's Save dialog */
        val MIME = mapOf(
            "txt" to "text/plain",
            "srt" to "application/x-subrip",
            "vtt" to "text/vtt",
            "md" to "text/markdown",
            "json" to "application/json",
        )
    }
}

/** The system's Save dialog, for a file of a kind and a name, both given each time: Android's CreateDocument. */
private object CreateText : ActivityResultContract<Pair<String, String>, Uri?>() {
    override fun createIntent(context: Context, input: Pair<String, String>) =
        Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType(input.first)
            .putExtra(Intent.EXTRA_TITLE, input.second)

    override fun parseResult(resultCode: Int, intent: Intent?): Uri? =
        intent?.data?.takeIf { resultCode == Activity.RESULT_OK }
}
