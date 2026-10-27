package net.pktito.app

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.media.AudioManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContract
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.IntentCompat
import androidx.core.os.BundleCompat
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import net.pktito.app.ui.About
import net.pktito.app.ui.Finder
import net.pktito.app.ui.ModelButton
import net.pktito.app.ui.Opened
import net.pktito.app.ui.PktitoTheme
import net.pktito.app.ui.Ready
import net.pktito.app.ui.ReadyActions
import net.pktito.app.ui.Setup
import net.pktito.app.ui.SetupActions
import net.pktito.app.ui.Side
import net.pktito.app.ui.Sound
import net.pktito.app.ui.Timebase
import net.pktito.app.ui.Toast
import net.pktito.app.ui.Work
import net.pktito.app.ui.WorkActions
import net.pktito.app.ui.byFinger
import net.pktito.app.ui.clock
import net.pktito.app.ui.findIn
import net.pktito.app.ui.pk
import net.pktito.app.ui.stepped

/**
 * The screen, the desktop window's: choose a speech model and download it,
 * open a recording, watch its transcript come, play it, and copy, share or
 * save it, and open it again from the recent list. The system's pickers
 * grant the one document picked and the one saved, as Open with and Share
 * grant theirs, so the app asks for no access to files; the work is
 * TranscribeService's, and a download's Downloads'.
 */
class MainActivity : ComponentActivity() {
    private val open = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            Recents.hold(this, uri)
            start(uri)
        }
    }
    private val notify = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}
    // a .pkf has no type of its own to ask for
    private val pickModel = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) ownFile(uri)
    }
    private val create = registerForActivityResult(CreateText) { uri -> if (uri != null) write(uri) }
    // Locate…: the transcript's recording, picked again where it is now,
    // if it is the one transcribed (Recents.relink); the player then loads it
    private val locate = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        val id = Transcriber.id
        if (uri == null || id.isEmpty()) return@registerForActivityResult
        if (Recents.relink(this, id, uri)) {
            Transcriber.why = null
            Transcriber.locate = false
            Transcriber.uri = uri
        } else toast = getString(R.string.play_not_it)
    }

    private val prefs by lazy { getSharedPreferences("pktito", MODE_PRIVATE) }
    /** Save as's choice: each sentence's time before it in .txt and .md. */
    private var times by mutableStateOf(false)
    /** The transcript as its subtitle track's cues, not its sentences. */
    private var captions by mutableStateOf(false)
    /** The strip's choice of how times are written, the video's own rate aside. */
    private var base by mutableStateOf(Timebase())
    private var toast by mutableStateOf<String?>(null)
    /** About is up, over the start screen. */
    private var about by mutableStateOf(false)
    /** About's version line: the app's, and the engine's code for this processor by its name. */
    private val version by lazy {
        val name = packageManager.getPackageInfo(packageName, 0).versionName ?: ""
        getString(R.string.about_version, name, Engine.backend().substringBefore(" ("))
    }
    /** The format being saved while the system's dialog is up. */
    private var saving: String? = null
    /** A recording opened or shared before there was a model to transcribe it with: transcribed once there is. */
    private var waiting by mutableStateOf<Uri?>(null)
    /** The search of the transcript on the screen. */
    private val finder = Finder()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // the volume keys set playback's level while pktito is in front, playing
        // or not, as a player's do: on a finger's screen they are its only one
        volumeControlStream = AudioManager.STREAM_MUSIC
        times = prefs.getBoolean("times", false)
        captions = prefs.getBoolean("captions", false)
        base = Timebase(
            frames = prefs.getBoolean("frames", false),
            fps = prefs.getFloat("fps", 24f).toDouble(),
            videoRate = prefs.getBoolean("video_rate", true),
        )
        Player.restore(prefs.getFloat("speed", 1f).toDouble(), prefs.getFloat("volume", 1f).toDouble())
        saving = savedInstanceState?.getString("saving")
        about = savedInstanceState?.getBoolean("about") ?: false
        savedInstanceState?.getString("find")?.let {
            finder.query = it
            finder.open = it.isNotEmpty()
        }
        waiting = savedInstanceState?.let { BundleCompat.getParcelable(it, "waiting", Uri::class.java) }
        Models.refresh(this)
        Recents.refresh(this)
        Recents.prune(this)
        // a transcript of the list that was on the screen when Android let the
        // process go, in the background, is there again
        val shown = savedInstanceState?.getString("recent")
        if (shown != null && Transcriber.step.isEmpty() && Recents.items.any { it.id == shown }) openRecent(shown)
        setContent { PktitoTheme { App() } }
        if (savedInstanceState == null) opened(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        opened(intent)
    }

    /**
     * A recording opened with pktito, or shared to it: transcribed, unless
     * one is under way; kept until there is a model, as the desktop keeps
     * files dropped on its setup screen.
     */
    private fun opened(i: Intent) {
        val uri = when (i.action) {
            Intent.ACTION_VIEW -> i.data
            Intent.ACTION_SEND -> IntentCompat.getParcelableExtra(i, Intent.EXTRA_STREAM, Uri::class.java)
            else -> null
        }
        if (uri == null || Transcriber.running) return
        Recents.hold(this, uri)
        if (setup()) {
            waiting = uri
            about = false
        } else start(uri)
    }

    /** Whether the screen is the setup screen's: no model yet, or one coming, by download or from a file. */
    private fun setup() = Models.ready == null || Downloads.state != null || Models.copying != null

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putString("saving", saving)
        outState.putBoolean("about", about)
        outState.putParcelable("waiting", waiting)
        outState.putString("find", finder.query)
        val t = Transcriber
        if (t.step == "done" && Recents.items.any { it.id == t.id }) outState.putString("recent", t.id)
    }

    // the transcript's audio stops with the screen, but not for a turn or a
    // window's resize, which make the screen anew; the transcription goes on
    override fun onStop() {
        super.onStop()
        if (!isChangingConfigurations) Player.pause(this)
    }

    @Composable
    private fun App() {
        val t = Transcriber
        val list = rememberLazyListState()
        // a finger's strip has no speaker or slider: the device's volume keys alone (Player.keysOnly)
        val finger = byFinger()
        SideEffect { Player.keysOnly = finger }
        val review = t.step == "done" || t.step == "stopped"
        val rows = if (captions && review) t.cues ?: emptyList() else t.segments
        // a search's matches in the rows shown, a finished transcript's only
        val hits = remember(rows, finder.query, review, t.segments.size) { if (review) findIn(rows, finder.query) else emptyList() }

        // the finished transcript's recording, ready to play, its views'
        // index read first where it plays as views; another's let go. Where
        // it cannot be played, the strip says why in its times' place, and
        // Locate… finds it again (the one already loaded, kept through a
        // turn of the screen)
        LaunchedEffect(t.step, t.id, t.uri) {
            if (!review) return@LaunchedEffect Player.release(this@MainActivity)
            val uri = t.uri ?: return@LaunchedEffect
            val key = "${t.id} $uri"
            if (Player.loaded == key) return@LaunchedEffect
            Player.release(this@MainActivity)
            val (id, duration) = t.id to t.duration
            val item = Recents.items.find { it.id == id }
            // each step whole, and its file and index let go here if the
            // transcript was let go while it ran
            val off = NonCancellable + Dispatchers.IO
            val src = withContext(off) { Recents.open(this@MainActivity, id, uri, item?.size ?: 0, item?.modified ?: 0) }
            val file = src.file
            if (!isActive) return@LaunchedEffect file?.close() ?: Unit
            t.why = src.why
            t.locate = src.locate
            if (file == null) return@LaunchedEffect
            val index = withContext(off) { Engine.viewIndex(file.fd) }
            if (!isActive) {
                file.close()
                if (index != 0L) Engine.viewIndexFree(index)
                return@LaunchedEffect
            }
            Player.load(this@MainActivity, key, file, index, duration) {
                t.why = getString(R.string.play_cant)
                t.locate = false
            }
        }
        // where playback has got to, each frame while it plays
        LaunchedEffect(Player.playing) {
            while (Player.playing) {
                withFrameNanos { }
                Player.tick()
            }
        }
        // the match to be on once the matches change, and each move to one
        // brought into sight, its row a third of the way down (app.js, goHit)
        LaunchedEffect(hits) { finder.settle(hits, Player.position) }
        // the match playback passes, the reader with it (Finder.follow)
        LaunchedEffect(hits) { snapshotFlow { Player.position }.collect { finder.follow(hits, it, false) } }
        LaunchedEffect(finder.moved) {
            val h = hits.getOrNull(finder.at) ?: return@LaunchedEffect
            val info = list.layoutInfo
            val row = info.visibleItemsInfo.firstOrNull { it.index == h.row }
            if (row == null || row.offset < info.viewportStartOffset || row.offset + row.size > info.viewportEndOffset)
                list.animateScrollToItem(h.row, -info.viewportSize.height / 3)
        }
        // the cues, the first time the transcript is shown as captions
        LaunchedEffect(captions, t.transcript) {
            val tr = t.transcript
            if (captions && tr != null && t.cues == null)
                t.cues = runCatching { Transcriber.parseCues(Engine.captions(tr), t.segments.flatMap { it.words }) }
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
        LaunchedEffect(review, t.id) {
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
        // a model downloaded, said so
        LaunchedEffect(Models.news) {
            Models.news?.let {
                toast = it
                Models.news = null
            }
        }
        // the recording that waited for a model, once there is one
        val setup = setup()
        LaunchedEffect(setup, waiting) {
            val w = waiting
            if (!setup && w != null && t.step.isEmpty()) {
                waiting = null
                start(w)
            }
        }
        BackHandler(enabled = t.step.isNotEmpty() && !t.running) { back() }
        // a search is let go before the transcript is
        BackHandler(enabled = t.step.isNotEmpty() && finder.searching) { finder.clear() }
        BackHandler(enabled = about) { about = false }

        val readyActions = ReadyActions(
            choose = { open.launch(MEDIA) },
            about = { about = true },
            // another of the side's rows: the transcript open let go first
            open = { id ->
                if (t.step.isNotEmpty()) leave()
                openRecent(id)
            },
            forget = { Recents.forget(this@MainActivity, it) },
            clear = { Recents.clear(this@MainActivity) },
        )
        val model = @Composable {
            ModelButton(
                Models.offers, Models.model, Models.own?.name,
                onPick = { o ->
                    if (!o.have) download(o)
                    else {
                        Models.use(this@MainActivity, o.name)
                        Models.refresh(this@MainActivity)
                        toast = getString(R.string.models_using, Models.label(resources, o.key))
                    }
                },
                onOwnFile = { pickModel.launch(arrayOf("*/*")) },
            )
        }
        val work = @Composable {
            Work(
                name = t.name,
                step = t.step,
                done = t.done,
                duration = t.duration,
                peaks = t.peaks,
                rows = rows,
                error = t.error,
                why = t.why,
                locate = if (t.locate) ({ locate.launch(MEDIA) }) else null,
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
                chosen = base.copy(video = t.fps),
                find = finder,
                hits = hits,
                sound = Sound(
                    speed = Player.speed,
                    speeds = Player.SPEEDS,
                    volume = Player.volume,
                    muted = Player.muted,
                    setSpeed = ::setSpeed,
                    setVolume = { v, kept ->
                        Player.changeVolume(v)
                        if (kept) prefs.edit().putFloat("volume", v.toFloat()).apply()
                    },
                    // unmuting at none sets it to full, which is kept
                    mute = {
                        Player.toggleMute()
                        prefs.edit().putFloat("volume", Player.volume.toFloat()).apply()
                    },
                ),
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
                    seek = { s, play ->
                        Player.seek(this@MainActivity, s, play)
                        finder.follow(hits, Player.position, true)
                    },
                    setBase = {
                        base = it
                        prefs.edit()
                            .putBoolean("frames", it.frames)
                            .putFloat("fps", it.fps.toFloat())
                            .putBoolean("video_rate", it.videoRate)
                            .apply()
                    },
                ),
            )
        }
        BoxWithConstraints(
            Modifier
                .fillMaxSize()
                .background(pk.ground)
                .windowInsetsPadding(WindowInsets.safeDrawing),
        ) {
            // the side (Side) where a list and what it opens fit side by side:
            // Material's expanded width, 840 dp, which leaves the work 540,
            // where its rows stack (Rows, under 620) and Find keeps the views
            // line (Views, from 480); and 480 tall, which keeps a phone on its
            // side one column (the emulator's Pixel 5, 851 by 393). The Tab S7
            // FE has it on its side, 1205 by 753 less its bars, and not
            // upright, 753 wide
            val sided = maxWidth >= 840.dp && maxHeight >= 480.dp
            if (about)
                About(version, onBack = { about = false }, onLink = ::link)
            else if (t.step.isEmpty() && setup)
                Setup(
                    Models.offers, Models.model, Downloads.state, Models.copying, if (waiting != null) 1 else 0,
                    SetupActions(
                        download = ::download,
                        use = {
                            Models.use(this@MainActivity, it.name)
                            Models.refresh(this@MainActivity)
                        },
                        pause = { Downloads.pause(this@MainActivity) },
                        cancel = {
                            if (Models.copying != null) Models.stopCopying() else Downloads.cancel(this@MainActivity)
                        },
                        retry = { Downloads.retry(this@MainActivity) },
                        ownFile = { pickModel.launch(arrayOf("*/*")) },
                        link = ::link,
                        about = { about = true },
                    ),
                )
            else if (sided && (t.step.isNotEmpty() || Recents.items.isNotEmpty()))
                Side(Recents.items, readyActions, if (t.step.isEmpty()) null else Opened(t.id, t.name, t.step, t.done), model, work)
            else if (t.step.isEmpty())
                Ready(Recents.items, readyActions, model)
            else
                work()
            toast?.let { Toast(it, Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp)) }
        }
    }

    /**
     * The desktop's keys, where there is a keyboard (app.js, keydown): Space
     * plays and pauses, < and > step the speed, Ctrl+S saves the text and
     * Ctrl+Shift+S the subtitles, Ctrl+F finds, Ctrl+O chooses a recording
     * on the start screen, and Esc lets go of a search and then goes back as
     * the back gesture does, which Android leaves to the app (the
     * Googlebook, 2026-10-25), and closes About, where it is the only key
     * the app has. Space plays as it comes up, where the desktop's plays as
     * it goes down: after a touch, Android takes a Space's press as the way
     * out of touch mode (ViewRootImpl), and the app sees only its release
     * (the emulator, 2026-10-26: the first Space after opening a transcript
     * played nothing). In the search's field, Space and < and > are its
     * letters, and Enter its next match (FindField). Before the controls, as
     * the desktop's are before its buttons: a control with the focus would
     * take Space as a press, and Back's threw the transcript away (the
     * Googlebook). An open menu is a window of its own, and has the keys
     * first; it closes on Esc. The method is Activity's, which lint takes for
     * a restricted one (RestrictedApi): it sees it through androidx.core's
     * ComponentActivity, a class between the two that androidx keeps to itself.
     */
    @SuppressLint("RestrictedApi")
    override fun dispatchKeyEvent(event: KeyEvent): Boolean =
        key(event) || super.dispatchKeyEvent(event)

    private fun key(event: KeyEvent): Boolean {
        val t = Transcriber
        val review = t.step == "done" || t.step == "stopped"
        val typing = finder.typing
        if (event.keyCode == KeyEvent.KEYCODE_SPACE && event.hasNoModifiers() && review && Player.ready && !typing && !about) {
            if (event.action == KeyEvent.ACTION_UP && !event.isCanceled) Player.toggle(this)
            return true
        }
        if (event.action != KeyEvent.ACTION_DOWN) return false
        val ctrl = event.isCtrlPressed && !event.isAltPressed && !event.isMetaPressed
        val char = event.unicodeChar.toChar()
        if (about) {
            if (event.keyCode != KeyEvent.KEYCODE_ESCAPE) return false
            about = false
            return true
        }
        when {
            ctrl && event.keyCode == KeyEvent.KEYCODE_S ->
                if (review && t.segments.isNotEmpty()) save(if (event.isShiftPressed) "srt" else "txt")
            ctrl && event.keyCode == KeyEvent.KEYCODE_O && !event.isShiftPressed ->
                if (t.step.isEmpty() && !setup()) open.launch(MEDIA)
            ctrl && event.keyCode == KeyEvent.KEYCODE_F && !event.isShiftPressed ->
                if (review && t.segments.isNotEmpty()) finder.focus()
            event.keyCode == KeyEvent.KEYCODE_ESCAPE && (finder.searching || typing) -> finder.clear()
            event.keyCode == KeyEvent.KEYCODE_ESCAPE && t.step.isNotEmpty() && !t.running -> back()
            (char == '<' || char == '>') && !event.isCtrlPressed && !event.isMetaPressed && review && Player.ready && !typing ->
                setSpeed(stepped(Player.SPEEDS, Player.speed, if (char == '>') 1 else -1))
            else -> return false
        }
        return true
    }

    private fun setSpeed(s: Double) {
        Player.changeSpeed(s)
        prefs.edit().putFloat("speed", s.toFloat()).apply()
    }

    /** how many seconds of audio a second's work transcribed (ui/app.js, realTime) */
    private fun pace(audio: Double, secs: Double) =
        getString(R.string.real_time, String.format(Locale.getDefault(), "%.1f", audio / secs))

    private fun start(uri: Uri) {
        // a recording opened or shared to pktito while About is up is shown at work
        about = false
        // one chosen in the side, or shared, while a transcript is open lets it go first
        if (Transcriber.step.isNotEmpty() && !Transcriber.running) leave()
        startForegroundService(
            Intent(this, TranscribeService::class.java)
                .setData(uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION),
        )
        askToNotify()
    }

    /** A catalog model's download, its progress in a notification as a transcription's is. */
    private fun download(o: Offer) {
        Downloads.start(this, o)
        askToNotify()
    }

    /** The progress notifications, from Android 13, are the person's to allow. */
    private fun askToNotify() {
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        )
            notify.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    /**
     * A model file of the person's own, copied in and checked, then in use
     * (Models.open): a download under way is let go once it is, as the
     * desktop's useOwnFile does, and what is wrong with one it is not, said.
     */
    private fun ownFile(uri: Uri) {
        Models.open(this, uri) { said, used ->
            toast = said
            if (used && Downloads.state != null) Downloads.cancel(this)
        }
    }

    /** A link of About's, in the browser, as the desktop's open in the system's (main.rs, open_link). */
    private fun link(url: String) {
        try {
            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
        } catch (e: ActivityNotFoundException) {
            Log.w(TAG, "no app for $url", e)
            toast = getString(R.string.link_failed, url)
        }
    }

    private fun back() {
        leave()
        Transcriber.clear()
    }

    /**
     * The transcript on the screen let go, as Back lets it go, its screen kept
     * until the next one takes its place: another of the side's rows, or a
     * recording chosen there or shared (app.js, leave).
     */
    private fun leave() {
        finder.clear()
        Player.release(this)
        // the copy of a shared recording whose transcript is not on the list,
        // stopped or failed, goes with it
        val id = Transcriber.id
        if (id.isNotEmpty() && Recents.items.none { it.id == id }) Recents.copyOf(this, id).delete()
    }

    /** A transcript of the recent list on the screen again (app.js, openRecent); one that cannot be read, said so. */
    private fun openRecent(id: String) {
        try {
            Transcriber.reopen(id, Recents.load(this, id))
        } catch (e: Exception) {
            Log.w(TAG, "opening $id", e)
            Transcriber.clear()
            toast = getString(R.string.recent_unreadable)
            Recents.refresh(this)
        }
    }

    /** The transcript in a format, the desktop's bytes for it. */
    private fun text(ext: String, times: Boolean): String? {
        val tr = Transcriber.transcript ?: return null
        return Engine.format(tr, ext, Transcriber.name, times)
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
        /** what the system's picker offers */
        val MEDIA = arrayOf("audio/*", "video/*")
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
