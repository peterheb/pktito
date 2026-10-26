package net.pktito.app

import android.content.res.Configuration
import android.graphics.Bitmap
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isPopup
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.onChildren
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import java.io.File
import net.pktito.app.ui.About
import net.pktito.app.ui.Finder
import net.pktito.app.ui.findIn
import net.pktito.app.ui.ModelButton
import net.pktito.app.ui.PktitoTheme
import net.pktito.app.ui.Ready
import net.pktito.app.ui.ReadyActions
import net.pktito.app.ui.Setup
import net.pktito.app.ui.SetupActions
import net.pktito.app.ui.Sound
import net.pktito.app.ui.Timebase
import net.pktito.app.ui.Work
import net.pktito.app.ui.WorkActions
import net.pktito.app.ui.pk
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The screens drawn on the JVM, Robolectric's native graphics with the
 * app's own fonts, into build/shots: each state of each screen, light and
 * dark, a phone's width and a laptop window's, to look at beside the
 * desktop's (site/screenshots). The transcript is AliceBench's chapter 1 as
 * pktito-cli transcribes it, and its outline audio.rs's of the same audio
 * (src/test/resources). Nothing is asserted: the pictures are for a person.
 *
 *   ./gradlew testDebugUnitTest --tests net.pktito.app.Shots
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class Shots {
    @get:Rule
    val compose = createComposeRule()

    private fun resource(name: String) = javaClass.getResourceAsStream("/$name")!!.readBytes()
    private val segments = Transcriber.parse(String(resource("alice-segments.json")))
    private val cues = Transcriber.parseCues(String(resource("alice-cues.json")), segments.flatMap { it.words })
    private val peaks = resource("alice-peaks.bin")
    private val actions = WorkActions({}, {}, {}, {}, {}, {}, {}, {}, { _, _ -> }, {})

    /**
     * The screen, or what act leads to on it (a menu, its own window); with
     * keys, as with the Googlebook's keyboard, which a qualifier cannot say
     * (its hardKeyboardHidden is the device's, not a resource's). land is a
     * landscape window's, the Googlebook's own window the default one's
     * there, 680 by 800 inside its header.
     */
    private fun shot(
        name: String, dark: Boolean, wide: Boolean = false, keys: Boolean = false, narrow: Boolean = false,
        land: Boolean = false, window: Boolean = false,
        act: (() -> SemanticsNodeInteraction)? = null, content: @Composable () -> Unit,
    ) {
        RuntimeEnvironment.setQualifiers(
            when {
                land -> "w1100dp-h760dp-xhdpi"
                window -> "w680dp-h800dp-hdpi"
                wide -> "w840dp-h900dp-xhdpi"
                narrow -> "w360dp-h780dp-xxhdpi"
                else -> "w411dp-h891dp-xxhdpi"
            },
        )
        RuntimeEnvironment.setQualifiers(if (dark) "+night" else "+notnight")
        compose.setContent {
            val c = Configuration(LocalConfiguration.current)
            if (keys) {
                c.keyboard = Configuration.KEYBOARD_QWERTY
                c.hardKeyboardHidden = Configuration.HARDKEYBOARDHIDDEN_NO
            }
            CompositionLocalProvider(LocalConfiguration provides c) {
                PktitoTheme { Box(Modifier.fillMaxSize().background(pk.ground)) { content() } }
            }
        }
        val dir = File("build/shots").apply { mkdirs() }
        val size = if (land) "-land" else if (window) "-window" else if (wide) "-wide" else ""
        val file = File(dir, "$name${if (keys) "-keys" else ""}-${if (dark) "dark" else "light"}$size.png")
        val node = act?.invoke() ?: compose.onRoot()
        file.outputStream().use { node.captureToImage().asAndroidBitmap().compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    /** A search for query, its field open on a phone if open, on the match k (-1: the one the playhead last passed). */
    private fun finder(rows: List<Seg>, query: String, open: Boolean = query.isNotEmpty(), k: Int = -1, position: Double = 0.0) =
        Finder().apply {
            this.query = query
            this.open = open
            val hits = findIn(rows, query)
            settle(hits, position)
            if (k >= 0 && k < hits.size) go(hits, k)
        }

    @Composable
    private fun work(step: String, done: Double, rows: List<Seg> = segments, captions: Boolean = false,
                     position: Double = 0.0, playing: Boolean = false, error: String? = null,
                     speed: Double = 1.0, volume: Double = 1.0, base: Timebase = Timebase(),
                     find: Finder = Finder()) =
        Work(
            name = "wonderland_ch_01.flac", step = step, done = done,
            duration = if (step == "reading" || step == "loading") 0.0 else 640.505,
            peaks = if (step == "reading" || step == "loading") null else peaks,
            rows = rows, error = error, why = null,
            stats = if (step == "done") "Transcribed in 0:11, 58.2x real-time" else null,
            pace = if (step == "transcribing") "57.9x real-time" else null,
            captions = captions, times = false, playable = step == "done", playing = playing,
            position = position, list = rememberLazyListState(),
            sound = Sound(speed, Player.SPEEDS, volume, false, {}, { _, _ -> }, {}), chosen = base,
            find = find, hits = if (step == "done") findIn(rows, find.query) else emptyList(), a = actions,
        )

    // the catalog as model.rs has it, nothing downloaded, or the English model here
    private val languages = listOf(
        "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt",
        "lv", "mt", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
    )
    private fun offers(have: Boolean = false) = listOf(
        Offer("parakeet-tdt-0.6b-v2", "english", listOf("en"), "", 624_480_196, "", have, 0, File("")),
        Offer(
            "parakeet-tdt-0.6b-v3", "multilingual", languages, "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3#multilingual-asr",
            647_552_644, "", false, 0, File(""),
        ),
    )
    private val models = @Composable { ModelButton(offers(have = true), "parakeet-tdt-0.6b-v2", null, {}, {}) }
    private val readyActions = ReadyActions({}, {}, {}, {}, {})
    // the recent list: today's, yesterday's, this year's and last year's,
    // an hour long and minutes, a name that fits and two that do not
    private val day = 86_400L
    private val now = System.currentTimeMillis() / 1000
    private val recent = listOf(
        Recents.Item("5", Uri.parse("content://x/5"), "wonderland_ch_01.flac", 640.505, now),
        Recents.Item("4", Uri.parse("content://x/4"), "Interview with Dr. Ramirez, part 2.m4a", 3725.0, now - day),
        Recents.Item("3", Uri.parse("content://x/3"), "Recording 2026-10-24 14-32-11.m4a", 312.4, now - 2 * day),
        Recents.Item("2", Uri.parse("content://x/2"), "Team standup.mp4", 905.0, now - 6 * day),
        Recents.Item("1", Uri.parse("content://x/1"), "Lecture 7, Thermodynamics.mp3", 4810.0, now - 400 * day),
    )
    @Test fun ready() = shot("ready", false) { Ready(emptyList(), readyActions, models) }
    @Test fun readyDark() = shot("ready", true) { Ready(emptyList(), readyActions, models) }
    @Test fun readyRecent() = shot("ready-recent", false) { Ready(recent, readyActions, models) }
    @Test fun readyRecentDark() = shot("ready-recent", true) { Ready(recent, readyActions, models) }
    @Test fun readyRecentNarrow() = shot("ready-recent-360", false, narrow = true) { Ready(recent, readyActions, models) }
    @Test fun readyRecentWindow() = shot("ready-recent", false, window = true) { Ready(recent, readyActions, models) }
    @Test fun readyRecentLand() = shot("ready-recent", false, land = true) { Ready(recent, readyActions, models) }
    private val removeMenu = {
        compose.onNodeWithText("Team standup.mp4").performTouchInput { longClick() }
        compose.onRoot()
    }
    @Test fun recentMenu() = shot("recent-menu", false, act = removeMenu) { Ready(recent, readyActions, models) }
    private val askClear = {
        compose.onNodeWithText("Clear").performClick()
        compose.onNode(isDialog())
    }
    @Test fun recentClear() = shot("recent-clear", false, act = askClear) { Ready(recent, readyActions, models) }
    @Test fun recentClearDark() = shot("recent-clear", true, act = askClear) { Ready(recent, readyActions, models) }
    private val modelMenu = {
        compose.onNodeWithText("English model").performClick()
        compose.onNode(isPopup())
    }
    @Test fun modelMenu() = shot("models", false, act = modelMenu) { Ready(emptyList(), readyActions, models) }
    @Test fun modelMenuDark() = shot("models", true, act = modelMenu) { Ready(emptyList(), readyActions, models) }

    // the setup screen in each of its modes, a download a third of the way, as the desktop's stand-in has it
    private val setupActions = SetupActions({}, {}, {}, {}, {}, {}, {}, {})
    private fun download(step: String, got: Long, speed: Double = 0.0, problem: Problem? = null, retry: Boolean = false) =
        Download("parakeet-tdt-0.6b-v2", step, got, 624_480_196, speed, "models.pktito.net", problem, retry, trying = false)
    private val lost = Problem(
        "download.cant_reach", "models.pktito.net",
        "Dns Failed: resolve dns name 'models.pktito.net:443': failed to lookup address information: " +
            "No address associated with hostname\nGET https://models.pktito.net/8bfa6750b83c/parakeet-tdt-0.6b-v2.pkf\nRange: bytes=262144000-",
    )
    @Composable
    private fun setupScreen(d: Download? = null, copying: Copy? = null, waiting: Int = 0) =
        Setup(offers(), null, d, copying, waiting, setupActions)
    @Test fun setup() = shot("setup", false) { setupScreen() }
    @Test fun setupDark() = shot("setup", true) { setupScreen() }
    @Test fun setupNarrow() = shot("setup-360", false, narrow = true) { setupScreen() }
    @Test fun setupWindow() = shot("setup", false, window = true) { setupScreen(waiting = 1) }
    // the count is a link in its card's words, a node of its own under them
    private val languagesOpen = {
        compose.onNodeWithText("25 European languages", substring = true, useUnmergedTree = true).onChildren().onFirst().performClick()
        compose.onRoot()
    }
    @Test fun setupLanguages() = shot("setup-languages", false, act = languagesOpen) { setupScreen() }
    @Test fun setupLanguagesDark() = shot("setup-languages", true, act = languagesOpen) { setupScreen() }
    @Test fun downloading() = shot("downloading", false) { setupScreen(download("downloading", 262_144_000, 11.8e6)) }
    @Test fun downloadingDark() = shot("downloading", true) { setupScreen(download("downloading", 262_144_000, 11.8e6)) }
    @Test fun downloadingNarrow() = shot("downloading-360", false, narrow = true) { setupScreen(download("downloading", 262_144_000, 11.8e6), waiting = 1) }
    @Test fun downloadPaused() = shot("download-paused", false) { setupScreen(download("paused", 262_144_000)) }
    @Test fun downloadChecking() = shot("download-checking", false) { setupScreen(download("checking", 624_480_196)) }
    private val details = {
        compose.onNodeWithText("Details").performClick()
        compose.onRoot()
    }
    @Test fun downloadFailed() = shot("download-failed", false, act = details) { setupScreen(download("failed", 262_144_000, problem = lost, retry = true)) }
    @Test fun downloadFailedDark() = shot("download-failed", true) { setupScreen(download("failed", 262_144_000, problem = lost, retry = true)) }
    @Test fun downloadFailedNarrow() = shot("download-failed-360", false, narrow = true) { setupScreen(download("failed", 262_144_000, problem = lost, retry = true)) }
    @Test fun copying() = shot("copying", false) { setupScreen(copying = Copy("parakeet-tdt-0.6b-v3.pkf", 302_000_000, 647_552_644)) }
    private val about = @Composable { About("Version 0.9.2 · neon-dp", {}, {}) }
    @Test fun about() = shot("about", false, content = about)
    @Test fun aboutDark() = shot("about", true, content = about)
    @Test fun aboutNarrow() = shot("about-360", false, narrow = true, content = about)
    @Test fun aboutWindow() = shot("about", false, window = true, content = about)
    @Test fun aboutLand() = shot("about", false, land = true, content = about)
    @Test fun aboutLandDark() = shot("about", true, land = true, content = about)
    // the sheets, each alone, as About opens it
    private fun sheet(link: String) = {
        compose.onNodeWithText(link).performClick()
        compose.onNodeWithTag("sheet")
    }
    @Test fun privacy() = shot("privacy", false, act = sheet("Privacy"), content = about)
    @Test fun privacyDark() = shot("privacy", true, act = sheet("Privacy"), content = about)
    @Test fun privacyWindow() = shot("privacy", false, window = true, act = sheet("Privacy"), content = about)
    @Test fun licenses() = shot("licenses", false, act = sheet("Open source licenses"), content = about)
    @Test fun licensesDark() = shot("licenses", true, act = sheet("Open source licenses"), content = about)
    @Test fun licensesLand() = shot("licenses", false, land = true, act = sheet("Open source licenses"), content = about)
    @Test fun reading() = shot("reading", false) { work("reading", 0.0, emptyList()) }
    @Test fun transcribing() = shot("transcribing", false) { work("transcribing", 0.42, segments.take(6)) }
    @Test fun transcribingDark() = shot("transcribing", true) { work("transcribing", 0.42, segments.take(6)) }
    @Test fun done() = shot("done", false) { work("done", 1.0, position = 30.0, playing = true) }
    @Test fun doneDark() = shot("done", true) { work("done", 1.0, position = 30.0, playing = true) }
    @Test fun doneWide() = shot("done", false, wide = true) { work("done", 1.0, position = 30.0, playing = true) }
    @Test fun doneWideDark() = shot("done", true, wide = true) { work("done", 1.0, position = 30.0, playing = true) }
    private val saveAs = {
        compose.onNodeWithText("Save as").performClick()
        compose.onNode(isPopup())
    }
    private val speedMenu = {
        compose.onNodeWithText("1×").performClick()
        compose.onNode(isPopup())
    }
    @Test fun speedMenu() = shot("speed", false, act = speedMenu) { work("done", 1.0) }
    @Test fun speedMenuDark() = shot("speed", true, act = speedMenu) { work("done", 1.0) }
    // a speed other than 1× on play's ground, and the speaker's one wave under half the volume
    @Test fun doneSlowQuiet() = shot("done-1.5x-quiet", false) { work("done", 1.0, position = 30.0, speed = 1.5, volume = 0.3) }
    @Test fun saveAsMenu() = shot("save-as", false, act = saveAs) { work("done", 1.0) }
    @Test fun saveAsMenuDark() = shot("save-as", true, act = saveAs) { work("done", 1.0) }
    @Test fun speedMenuKeys() = shot("speed", false, keys = true, act = speedMenu) { work("done", 1.0) }
    @Test fun saveAsMenuKeys() = shot("save-as", false, keys = true, act = saveAs) { work("done", 1.0) }
    @Test fun saveAsMenuKeysDark() = shot("save-as", true, keys = true, act = saveAs) { work("done", 1.0) }
    @Test fun captions() = shot("captions", false) { work("done", 1.0, cues, captions = true, position = 12.0) }
    @Test fun captionsDark() = shot("captions", true) { work("done", 1.0, cues, captions = true, position = 12.0) }
    // a cue's range on a phone at 360 in frames, its longest; a laptop's window keeps start over end
    @Test fun captionsNarrowFrames() = shot("captions-360-frames", false, narrow = true) {
        work("done", 1.0, cues, captions = true, position = 12.0, base = Timebase(frames = true, video = 29.97))
    }
    @Test fun captionsWide() = shot("captions", false, wide = true) { work("done", 1.0, cues, captions = true, position = 12.0) }
    // a search: on a phone, the glass on the views line, then its field open over it with "alice" in it, on its
    // third match; in a laptop's window beside the views; a word that is not there; and in the captions
    @Test fun findClosed() = shot("find-closed", false) { work("done", 1.0, position = 30.0) }
    @Test fun find() = shot("find", false) { work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2)) }
    @Test fun findDark() = shot("find", true) { work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2)) }
    @Test fun findNarrow() = shot("find-360", false, narrow = true) { work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2)) }
    @Test fun findWindow() = shot("find", false, window = true) { work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2)) }
    @Test fun findWindowEmpty() = shot("find-empty", false, window = true) { work("done", 1.0, find = finder(segments, "")) }
    @Test fun findNone() = shot("find-none", false) { work("done", 1.0, find = finder(segments, "jabberwock")) }
    @Test fun findCaptions() = shot("find-captions", false) { work("done", 1.0, cues, captions = true, find = finder(cues, "the", k = 3)) }
    // the time menu as the desktop's stand-in has it (an audio file, 25 fps chosen, at the start),
    // and a 29.97 video's in frames at its own rate, drop-frame, half a minute in
    private fun timeMenu(time: String) = {
        compose.onNodeWithText(time).performClick()
        compose.onNode(isPopup())
    }
    @Test fun timeMenu() = shot("time", false, act = timeMenu("00:00.00")) { work("done", 1.0, base = Timebase(fps = 25.0)) }
    @Test fun timeMenuDark() = shot("time", true, act = timeMenu("00:00.00")) { work("done", 1.0, base = Timebase(fps = 25.0)) }
    @Test fun timeMenuVideo() = shot("time-video", false, act = timeMenu("00:00:29;29")) {
        work("done", 1.0, position = 30.0, base = Timebase(frames = true, video = 29.97))
    }
    @Test fun doneFrames() = shot("done-frames", false) { work("done", 1.0, position = 30.0, base = Timebase(frames = true, video = 29.97)) }
    // a phone at 360 dp: the slider gives way, in seconds and in frames
    @Test fun doneNarrow() = shot("done-360", false, narrow = true) { work("done", 1.0, position = 30.0) }
    @Test fun doneNarrowFrames() = shot("done-360-frames", false, narrow = true) { work("done", 1.0, position = 30.0, base = Timebase(frames = true, video = 29.97)) }
    @Test fun error() = shot("error", false) {
        work("error", 0.0, emptyList(), error = "Can't read this file (Android has no decoder for audio/alac)")
    }
}
