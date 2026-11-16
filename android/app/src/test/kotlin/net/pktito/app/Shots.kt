package net.pktito.app

import android.content.res.Configuration
import android.graphics.Bitmap
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.contextmenu.data.TextContextMenuComponent
import androidx.compose.foundation.text.contextmenu.data.TextContextMenuItem
import androidx.compose.foundation.text.contextmenu.data.TextContextMenuSession
import androidx.compose.foundation.text.contextmenu.provider.LocalTextContextMenuToolbarProvider
import androidx.compose.foundation.text.contextmenu.provider.TextContextMenuDataProvider
import androidx.compose.foundation.text.contextmenu.provider.TextContextMenuProvider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isPopup
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onChildren
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performFirstLinkClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.io.File
import kotlinx.coroutines.awaitCancellation
import net.pktito.app.ui.About
import net.pktito.app.ui.Confirm
import net.pktito.app.ui.Finder
import net.pktito.app.ui.Follow
import net.pktito.app.ui.Hit
import net.pktito.app.ui.Manage
import net.pktito.app.ui.ManageActions
import net.pktito.app.ui.ModelButton
import net.pktito.app.ui.Opened
import net.pktito.app.ui.PktitoTheme
import net.pktito.app.ui.Ready
import net.pktito.app.ui.ReadyActions
import net.pktito.app.ui.RenameSpeaker
import net.pktito.app.ui.Setup
import net.pktito.app.ui.SetupActions
import net.pktito.app.ui.Side
import net.pktito.app.ui.Sound
import net.pktito.app.ui.SpeakersView
import net.pktito.app.ui.Timebase
import net.pktito.app.ui.UndoToast
import net.pktito.app.ui.Work
import net.pktito.app.ui.WorkActions
import net.pktito.app.ui.findIn
import net.pktito.app.ui.pk
import net.pktito.app.ui.turnsOf
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The screens drawn on the JVM, Robolectric's native graphics with the app's own fonts, into
 * build/shots: each state of each screen, light and dark, a phone's width and a laptop window's, to
 * look at beside the desktop's (site/screenshots). The transcript is AliceBench's chapter 1 as
 * pktito-cli transcribes it, and its outline audio.rs's of the same audio (src/test/resources).
 * Nothing is asserted: the pictures are for a person.
 *
 * ./gradlew testDebugUnitTest --tests net.pktito.app.Shots
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class Shots {
    @get:Rule val compose = createComposeRule()

    private fun resource(name: String) = javaClass.getResourceAsStream("/$name")!!.readBytes()

    private val segments = Transcriber.parse(String(resource("alice-segments.json")))
    private val peaks = resource("alice-peaks.bin")
    private val actions = WorkActions({}, {}, {}, {}, {}, {}, { _, _ -> }, {}, {})

    /**
     * The screen, or what act leads to on it (a menu, its own window); with keys, as with the
     * Googlebook's keyboard, which a qualifier cannot say (its hardKeyboardHidden is the device's,
     * not a resource's). land is a landscape window's, the Googlebook's own window the default
     * one's there, 680 by 800 inside its header.
     */
    private fun shot(
        name: String,
        dark: Boolean,
        wide: Boolean = false,
        keys: Boolean = false,
        narrow: Boolean = false,
        land: Boolean = false,
        window: Boolean = false,
        short: Boolean = false,
        tab: Int = 0,
        targets: Boolean = false,
        xa: Boolean = false,
        scale: Float = 1f,
        act: (() -> SemanticsNodeInteraction)? = null,
        content: @Composable () -> Unit,
    ) {
        RuntimeEnvironment.setQualifiers(
            when {
                tab == 1 -> "w1205dp-h753dp-xhdpi"
                tab == 2 -> "w753dp-h1205dp-xhdpi"
                // the least window with the side (MainActivity, sided)
                tab == 3 -> "w840dp-h480dp-xhdpi"
                land -> "w1100dp-h760dp-xhdpi"
                // a phone on its side, the emulator's Pixel 5
                short -> "w851dp-h393dp-xxhdpi"
                window -> "w680dp-h800dp-hdpi"
                wide -> "w840dp-h900dp-xhdpi"
                narrow -> "w360dp-h780dp-xxhdpi"
                else -> "w411dp-h891dp-xxhdpi"
            }
        )
        RuntimeEnvironment.setQualifiers(if (dark) "+night" else "+notnight")
        if (xa) RuntimeEnvironment.setQualifiers("+en-rXA")
        compose.setContent {
            val c = Configuration(LocalConfiguration.current)
            c.fontScale = scale
            if (keys) {
                c.keyboard = Configuration.KEYBOARD_QWERTY
                c.hardKeyboardHidden = Configuration.HARDKEYBOARDHIDDEN_NO
            }
            CompositionLocalProvider(LocalConfiguration provides c) {
                PktitoTheme { Box(Modifier.fillMaxSize().background(pk.ground)) { content() } }
            }
        }
        val dir = File("build/shots").apply { mkdirs() }
        val size =
            if (tab == 1) "-tab-land"
            else if (tab == 2) "-tab-port"
            else if (tab == 3) "-840x480"
            else if (land) "-land"
            else if (short) "-short" else if (window) "-window" else if (wide) "-wide" else ""
        val file =
            File(
                dir,
                "$name${if (keys) "-keys" else ""}${if (xa) "-xa" else ""}-" +
                    "${if (dark) "dark" else "light"}$size.png",
            )
        val node = act?.invoke() ?: compose.onRoot()
        file.outputStream().use {
            node.captureToImage().asAndroidBitmap().compress(Bitmap.CompressFormat.PNG, 100, it)
        }
        // what can be touched, and where, for a picture of the targets: px in the root
        if (targets) {
            val touched =
                hasClickAction() or
                    SemanticsMatcher.keyIsDefined(SemanticsProperties.ProgressBarRangeInfo) or
                    hasTestTag("wave") or
                    hasTestTag("lane") or
                    hasTestTag("strip")
            val nodes = compose.onAllNodes(touched, useUnmergedTree = true).fetchSemanticsNodes()
            File(dir, file.name.removeSuffix(".png") + ".json")
                .writeText(
                    nodes.joinToString(",\n", "[", "]") { n ->
                        val c = n.config
                        val label =
                            c.getOrNull(SemanticsProperties.TestTag)
                                ?: c.getOrNull(SemanticsProperties.ContentDescription)
                                    ?.firstOrNull()
                                ?: c.getOrNull(SemanticsProperties.Text)?.firstOrNull()?.text
                                ?: ""
                        val b = n.boundsInRoot
                        "{\"label\":\"${label.replace("\"", "")}\",\"l\":${b.left},\"t\":${b.top},\"r\":${b.right},\"b\":${b.bottom}}"
                    }
                )
        }
    }

    /**
     * A search for query, its field open on a phone if open, on the match k (-1: the one the
     * playhead last passed).
     */
    private fun finder(
        rows: List<Seg>,
        query: String,
        open: Boolean = query.isNotEmpty(),
        k: Int = -1,
        position: Double = 0.0,
    ) =
        Finder().apply {
            this.query = query
            this.open = open
            val hits = findIn(rows, query)
            settle(hits, position)
            if (k >= 0 && k < hits.size) go(hits, k)
        }

    @Composable
    private fun work(
        step: String,
        done: Double,
        rows: List<Seg> = segments,
        name: String = "wonderland_ch_01.flac",
        position: Double = 0.0,
        playing: Boolean = false,
        error: String? = null,
        speed: Double = 1.0,
        volume: Double = 1.0,
        muted: Boolean = false,
        base: Timebase = Timebase(),
        find: Finder = Finder(),
        why: String? = null,
        locate: Boolean = false,
    ) =
        Work(
            name = name,
            step = step,
            done = done,
            duration = if (step == "reading" || step == "loading") 0.0 else 640.505,
            peaks = if (step == "reading" || step == "loading") null else peaks,
            rows = rows,
            error = error,
            why = why,
            locate = if (locate) ({}) else null,
            stats = if (step == "done") "Transcribed in 0:11, 58.2× real time" else null,
            pace = if (step == "transcribing") "57.9× real time" else null,
            times = false,
            playable = step == "done" && why == null,
            playing = playing,
            position = position,
            follow = Follow(rememberLazyListState()),
            sound = Sound(speed, Player.SPEEDS, volume, muted, {}, { _, _ -> }, {}),
            chosen = base,
            find = find,
            hits = if (step == "done") findIn(rows, find.query) else emptyList(),
            a = actions,
        )

    // the catalog as model.rs has it, nothing downloaded, or the English model here
    private val languages =
        listOf(
            "bg",
            "cs",
            "da",
            "de",
            "el",
            "en",
            "es",
            "et",
            "fi",
            "fr",
            "hr",
            "hu",
            "it",
            "lt",
            "lv",
            "mt",
            "nl",
            "pl",
            "pt",
            "ro",
            "ru",
            "sk",
            "sl",
            "sv",
            "uk",
        )

    // npu: in the edition of a phone whose NPU the catalog has a graph for
    // (the Galaxy S24's), nothing downloaded; faster: the English model here
    // on the CPU, its NPU's edition offered; refused: the NPU refused English,
    // its CPU's edition to download
    private fun offers(
        have: Boolean = false,
        both: Boolean = false,
        partial: Long = 0,
        npu: Boolean = false,
        faster: Boolean = false,
        refused: Boolean = false,
    ) =
        listOf(
            offer(
                "parakeet-tdt-0.6b-v2",
                "english",
                listOf("en"),
                "",
                624_480_196,
                12_115_524,
                714_506_856,
                have,
                0,
                npu && !refused,
                faster,
                refused,
            ),
            offer(
                "parakeet-tdt-0.6b-v3",
                "multilingual",
                languages,
                "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3#multilingual-asr",
                647_552_644,
                35_187_972,
                714_506_856,
                both,
                if (both) 0 else partial,
                npu,
                false,
                false,
            ),
        )

    /**
     * A catalog model: its whole file here (have) or partial bytes of it kept; with npu, nothing
     * here and the NPU's edition its; with faster, the NPU's edition offered, the slim file and the
     * graph.
     */
    private fun offer(
        name: String,
        key: String,
        langs: List<String>,
        accuracy: String,
        whole: Long,
        slim: Long,
        graph: Long,
        have: Boolean,
        partial: Long,
        npu: Boolean,
        faster: Boolean,
        refused: Boolean,
    ): Offer {
        val npuBytes = slim + graph
        val get =
            when {
                have && faster -> Get(Get.FASTER, npuBytes, 0, listOf("slim", "graph"))
                have -> null
                npu -> Get(Get.DOWNLOAD, npuBytes, partial, listOf("slim", "graph"))
                else -> Get(Get.DOWNLOAD, whole, partial, listOf("whole"))
            }
        return Offer(
            name,
            key,
            langs,
            accuracy,
            if (npu || faster) "qnn-sm8650" else "cpu",
            if (have) Runs("cpu", File("$name.pkf"), null) else null,
            if (refused) "PKTITO_REFUSE_NPU=1" else null,
            get,
            if (have) whole else get?.bytes ?: whole,
            emptyList(),
            Part(File("$name.pkf"), whole, ""),
            taken = if (have) whole else partial,
        )
    }

    private val models =
        @Composable { ModelButton(offers(have = true), "parakeet-tdt-0.6b-v2", null, {}, {}, {}) }
    private val readyActions = ReadyActions({}, {}, {}, {}, {})
    // the recent list: today's, yesterday's, this year's and last year's,
    // an hour long and minutes, a name that fits and two that do not
    private val day = 86_400L
    private val now = System.currentTimeMillis() / 1000
    private val recent =
        listOf(
            Recents.Item(
                "5",
                Uri.parse("content://x/5"),
                "wonderland_ch_01.flac",
                640.505,
                now,
                0,
                0,
            ),
            Recents.Item(
                "4",
                Uri.parse("content://x/4"),
                "Interview with Dr. Ramirez, part 2.m4a",
                3725.0,
                now - day,
                0,
                0,
            ),
            Recents.Item(
                "3",
                Uri.parse("content://x/3"),
                "Recording 2026-10-24 14-32-11.m4a",
                312.4,
                now - 2 * day,
                0,
                0,
            ),
            Recents.Item(
                "2",
                Uri.parse("content://x/2"),
                "Team standup.mp4",
                905.0,
                now - 6 * day,
                0,
                0,
            ),
            Recents.Item(
                "1",
                Uri.parse("content://x/1"),
                "Lecture 7, Thermodynamics.mp3",
                4810.0,
                now - 400 * day,
                0,
                0,
            ),
        )

    @Test fun ready() = shot("ready", false) { Ready(emptyList(), readyActions, models) }

    @Test fun readyDark() = shot("ready", true) { Ready(emptyList(), readyActions, models) }

    @Test fun readyRecent() = shot("ready-recent", false) { Ready(recent, readyActions, models) }

    @Test fun readyRecentDark() = shot("ready-recent", true) { Ready(recent, readyActions, models) }

    @Test
    fun readyRecentNarrow() =
        shot("ready-recent-360", false, narrow = true) { Ready(recent, readyActions, models) }

    @Test
    fun readyRecentWindow() =
        shot("ready-recent", false, window = true) { Ready(recent, readyActions, models) }

    @Test
    fun readyRecentLand() =
        shot("ready-recent", false, land = true) { Ready(recent, readyActions, models) }

    @Test
    fun readyRecentShort() =
        shot("ready-recent", false, short = true) { Ready(recent, readyActions, models) }

    private val removeMenu = {
        compose.onNodeWithText("Team standup.mp4").performTouchInput { longClick() }
        compose.onRoot()
    }

    @Test
    fun recentMenu() =
        shot("recent-menu", false, act = removeMenu) { Ready(recent, readyActions, models) }

    private val askClear = {
        compose.onNodeWithText("Clear").performClick()
        compose.onNode(isDialog())
    }

    @Test
    fun recentClear() =
        shot("recent-clear", false, act = askClear) { Ready(recent, readyActions, models) }

    @Test
    fun recentClearDark() =
        shot("recent-clear", true, act = askClear) { Ready(recent, readyActions, models) }

    private val modelMenu = {
        compose.onNodeWithText("English model").performClick()
        compose.onNode(isPopup())
    }

    @Test
    fun modelMenu() =
        shot("models", false, act = modelMenu) { Ready(emptyList(), readyActions, models) }

    @Test
    fun modelMenuDark() =
        shot("models", true, act = modelMenu) { Ready(emptyList(), readyActions, models) }

    // where a size and its word could part, as the S24's menu did (models_download_size)
    @Test
    fun modelMenuNarrow() =
        shot("models-360", false, narrow = true, act = modelMenu) {
            Ready(emptyList(), readyActions, models)
        }

    // Manage models: the English model in use and the Multilingual's download
    // stopped at 312 MB, on a phone; both downloaded, on the tablet on its side;
    // and there with a transcription running, and a delete's question; the
    // English model's NPU edition offered, and coming
    @Composable
    private fun manageModels(
        both: Boolean = false,
        running: Boolean = false,
        d: Download? = null,
        toast: String? = null,
        compact: Boolean = true,
        faster: Boolean = false,
        spk: Speakers.Offer? = null,
    ) =
        Manage(
            offers(
                have = true,
                both = both,
                partial = if (both) 0 else 312_000_000,
                faster = faster,
            ),
            "parakeet-tdt-0.6b-v2",
            null,
            d,
            running,
            null,
            spk,
            false,
            18_400_000_000,
            toast,
            compact,
            ManageActions({}, {}, {}, {}, {}, {}, {}),
        )

    private val manageUp = { compose.onAllNodes(isDialog()).onFirst() }
    private val askDelete = {
        compose.onAllNodesWithText("Delete").onFirst().performClick()
        compose.onAllNodes(isDialog()).onLast()
    }

    @Test fun manage() = shot("manage", false, act = manageUp) { manageModels() }

    // the speaker model with them, downloaded, on a phone and on the tablet on
    // its side; what a download of it kept; and its Delete's question
    private fun speakerModel(have: Boolean, kept: Long = 0) =
        Speakers.Offer(
            "nemotron-3-diarization",
            "OpenMDW-1.1",
            "nemotron-3-diarization.pkf",
            100_498_048,
            kept,
            have,
        )

    private val askLast = {
        compose.onAllNodesWithText("Delete").onLast().performClick()
        compose.onAllNodes(isDialog()).onLast()
    }

    @Test
    fun manageSpeakers() =
        shot("manage-speakers", false, act = manageUp) { manageModels(spk = speakerModel(true)) }

    @Test
    fun manageSpeakersTab() =
        shot("manage-speakers", false, tab = 1, act = manageUp) {
            manageModels(both = true, compact = false, spk = speakerModel(true))
        }

    @Test
    fun manageSpeakersPartDark() =
        shot("manage-speakers-part", true, act = manageUp) {
            manageModels(both = true, spk = speakerModel(false, kept = 31_457_280))
        }

    @Test
    fun manageSpeakersAsk() =
        shot("manage-speakers-ask", false, tab = 1, act = askLast) {
            manageModels(both = true, compact = false, spk = speakerModel(true))
        }

    @Test fun manageDark() = shot("manage", true, act = manageUp) { manageModels() }

    @Test
    fun manageNarrow() = shot("manage-360", false, narrow = true, act = manageUp) { manageModels() }

    @Test
    fun manageDownloading() =
        shot("manage-downloading", false, act = manageUp) {
            manageModels(
                d =
                    Download(
                        "parakeet-tdt-0.6b-v3",
                        "downloading",
                        262_144_000,
                        647_552_644,
                        11.8e6,
                        "models.pktito.net",
                        null,
                        retry = false,
                        trying = false,
                    )
            )
        }

    @Test
    fun manageToast() =
        shot("manage-toast", false, act = manageUp) {
            manageModels(both = true, toast = "Using the Multilingual model")
        }

    @Test
    fun manageTab() =
        shot("manage", false, tab = 1, act = manageUp) {
            manageModels(both = true, compact = false)
        }

    @Test
    fun manageTabDark() =
        shot("manage", true, tab = 1, act = manageUp) { manageModels(both = true, compact = false) }

    @Test
    fun manageTabToast() =
        shot("manage-toast", false, tab = 1, act = manageUp) {
            manageModels(both = true, toast = "Using the Multilingual model", compact = false)
        }

    @Test
    fun manageRunning() =
        shot("manage-running", false, tab = 1, act = manageUp) {
            manageModels(both = true, running = true, compact = false)
        }

    @Test
    fun manageAsk() =
        shot("manage-ask", false, tab = 1, act = askDelete) {
            manageModels(both = true, compact = false)
        }

    @Test
    fun manageAskPhone() = shot("manage-ask", false, act = askDelete) { manageModels(both = true) }

    @Test
    fun manageFaster() =
        shot("manage-faster", false, act = manageUp) { manageModels(both = true, faster = true) }

    @Test
    fun manageFasterDark() =
        shot("manage-faster", true, act = manageUp) { manageModels(both = true, faster = true) }

    @Test
    fun manageFasterTab() =
        shot("manage-faster", false, tab = 1, act = manageUp) {
            manageModels(both = true, faster = true, compact = false)
        }

    @Test
    fun manageFasterDownloading() =
        shot("manage-faster-downloading", false, act = manageUp) {
            manageModels(
                both = true,
                faster = true,
                d =
                    Download(
                        "parakeet-tdt-0.6b-v2",
                        "downloading",
                        262_144_000,
                        730_054_828,
                        11.8e6,
                        "models.pktito.net",
                        null,
                        retry = false,
                        trying = false,
                    ),
            )
        }

    // the setup screen in each of its modes, a download a third of the way, as the desktop's
    // stand-in has it
    private val setupActions = SetupActions({}, {}, {}, {}, {}, {}, {}, {})

    private fun download(
        step: String,
        got: Long,
        speed: Double = 0.0,
        problem: Problem? = null,
        retry: Boolean = false,
    ) =
        Download(
            "parakeet-tdt-0.6b-v2",
            step,
            got,
            624_480_196,
            speed,
            "models.pktito.net",
            problem,
            retry,
            trying = false,
        )

    private val lost =
        Problem(
            "download.cant_reach",
            "models.pktito.net",
            "Dns Failed: resolve dns name 'models.pktito.net:443': failed to lookup address information: " +
                "No address associated with hostname\nGET https://models.pktito.net/8bfa6750b83c/parakeet-tdt-0.6b-v2.pkf\nRange: bytes=262144000-",
        )

    @Composable
    private fun setupScreen(
        d: Download? = null,
        copying: Copy? = null,
        waiting: Int = 0,
        npu: Boolean = false,
    ) = Setup(offers(npu = npu), null, d, copying, waiting, setupActions)

    @Test fun setup() = shot("setup", false) { setupScreen() }

    @Test fun setupNpu() = shot("setup-npu", false) { setupScreen(npu = true) }

    @Test fun setupDark() = shot("setup", true) { setupScreen() }

    @Test fun setupNarrow() = shot("setup-360", false, narrow = true) { setupScreen() }

    // the NPU refused English: its card says so, in en-XA's longest words at
    // one text size up (1.15) on a 360 dp phone
    @Test
    fun setupRefused() =
        shot("setup-refused-360", false, narrow = true, xa = true, scale = 1.15f) {
            Setup(offers(npu = true, refused = true), null, null, null, 0, setupActions)
        }

    @Test fun setupWindow() = shot("setup", false, window = true) { setupScreen(waiting = 1) }

    // the count is a link in its card's words, a node of its own under them
    private val languagesOpen = {
        compose
            .onNodeWithText("25 European languages", substring = true, useUnmergedTree = true)
            .onChildren()
            .onFirst()
            .performClick()
        compose.onRoot()
    }

    @Test
    fun setupLanguages() = shot("setup-languages", false, act = languagesOpen) { setupScreen() }

    @Test
    fun setupLanguagesDark() = shot("setup-languages", true, act = languagesOpen) { setupScreen() }

    @Test
    fun downloading() =
        shot("downloading", false) { setupScreen(download("downloading", 262_144_000, 11.8e6)) }

    @Test
    fun downloadingDark() =
        shot("downloading", true) { setupScreen(download("downloading", 262_144_000, 11.8e6)) }

    @Test
    fun downloadingNarrow() =
        shot("downloading-360", false, narrow = true) {
            setupScreen(download("downloading", 262_144_000, 11.8e6), waiting = 1)
        }

    @Test
    fun downloadPaused() =
        shot("download-paused", false) { setupScreen(download("paused", 262_144_000)) }

    @Test
    fun downloadChecking() =
        shot("download-checking", false) { setupScreen(download("checking", 624_480_196)) }

    private val details = {
        compose.onNodeWithText("Details").performClick()
        compose.onRoot()
    }

    @Test
    fun downloadFailed() =
        shot("download-failed", false, act = details) {
            setupScreen(download("failed", 262_144_000, problem = lost, retry = true))
        }

    @Test
    fun downloadFailedDark() =
        shot("download-failed", true) {
            setupScreen(download("failed", 262_144_000, problem = lost, retry = true))
        }

    @Test
    fun downloadFailedNarrow() =
        shot("download-failed-360", false, narrow = true) {
            setupScreen(download("failed", 262_144_000, problem = lost, retry = true))
        }

    @Test
    fun copying() =
        shot("copying", false) {
            setupScreen(copying = Copy("parakeet-tdt-0.6b-v3.pkf", 302_000_000, 647_552_644))
        }

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

    @Test
    fun privacyWindow() =
        shot("privacy", false, window = true, act = sheet("Privacy"), content = about)

    @Test fun licenses() = shot("licenses", false, act = sheet("Licenses"), content = about)

    @Test fun licensesDark() = shot("licenses", true, act = sheet("Licenses"), content = about)

    @Test
    fun licensesLand() =
        shot("licenses", false, land = true, act = sheet("Licenses"), content = about)

    @Test fun reading() = shot("reading", false) { work("reading", 0.0, emptyList()) }

    @Test
    fun transcribing() =
        shot("transcribing", false) { work("transcribing", 0.42, segments.take(6)) }

    @Test
    fun transcribingDark() =
        shot("transcribing", true) { work("transcribing", 0.42, segments.take(6)) }

    @Test fun done() = shot("done", false) { work("done", 1.0, position = 30.0, playing = true) }

    @Test fun doneDark() = shot("done", true) { work("done", 1.0, position = 30.0, playing = true) }

    @Test
    fun doneWide() =
        shot("done", false, wide = true) { work("done", 1.0, position = 30.0, playing = true) }

    @Test
    fun doneWideDark() =
        shot("done", true, wide = true) { work("done", 1.0, position = 30.0, playing = true) }

    private val shareMenu = {
        compose.onNodeWithText("Share").performClick()
        compose.onNode(isPopup())
    }
    private val speedMenu = {
        compose.onNodeWithText("1×").performClick()
        compose.onNode(isPopup())
    }

    @Test fun speedMenu() = shot("speed", false, act = speedMenu) { work("done", 1.0) }

    @Test fun speedMenuDark() = shot("speed", true, act = speedMenu) { work("done", 1.0) }

    // a speed other than 1× on play's ground, and the speaker's one wave under half the volume
    @Test
    fun doneSlowQuiet() =
        shot("done-1.5x-quiet", false) {
            work("done", 1.0, position = 30.0, speed = 1.5, volume = 0.3)
        }

    // silent, muted or at none, the speaker lit, where there are keys and so a speaker
    @Test
    fun doneMutedKeys() =
        shot("done-muted", false, keys = true) { work("done", 1.0, position = 30.0, muted = true) }

    @Test
    fun doneMutedKeysDark() =
        shot("done-muted", true, keys = true) { work("done", 1.0, position = 30.0, muted = true) }

    @Test
    fun doneNoneKeys() =
        shot("done-none", false, keys = true) { work("done", 1.0, position = 30.0, volume = 0.0) }

    @Test fun shareMenu() = shot("share", false, act = shareMenu) { work("done", 1.0) }

    @Test fun shareMenuDark() = shot("share", true, act = shareMenu) { work("done", 1.0) }

    @Test
    fun speedMenuKeys() = shot("speed", false, keys = true, act = speedMenu) { work("done", 1.0) }

    @Test
    fun shareMenuKeys() = shot("share", false, keys = true, act = shareMenu) { work("done", 1.0) }

    @Test
    fun shareMenuKeysDark() =
        shot("share", true, keys = true, act = shareMenu) { work("done", 1.0) }

    // a search: closed, then its field open with "alice" in it, on its third match; in a laptop's
    // window; and a word that is not there
    @Test fun findClosed() = shot("find-closed", false) { work("done", 1.0, position = 30.0) }

    @Test
    fun find() =
        shot("find", false) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun findDark() =
        shot("find", true) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun findNarrow() =
        shot("find-360", false, narrow = true) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun findWindow() =
        shot("find", false, window = true) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun findWindowEmpty() =
        shot("find-empty", false, window = true) { work("done", 1.0, find = finder(segments, "")) }

    @Test
    fun findNone() =
        shot("find-none", false) { work("done", 1.0, find = finder(segments, "jabberwock")) }

    // the time menu as the desktop's stand-in has it (an audio file, 25 fps chosen, at the start),
    // and a 29.97 video's in frames at its own rate, drop-frame, half a minute in
    private fun timeMenu(time: String) = {
        compose.onNodeWithText(time).performClick()
        compose.onNode(isPopup())
    }

    @Test
    fun timeMenu() =
        shot("time", false, act = timeMenu("00:00.00")) {
            work("done", 1.0, base = Timebase(fps = 25.0))
        }

    @Test
    fun timeMenuDark() =
        shot("time", true, act = timeMenu("00:00.00")) {
            work("done", 1.0, base = Timebase(fps = 25.0))
        }

    @Test
    fun timeMenuVideo() =
        shot("time-video", false, act = timeMenu("00:00:29;29")) {
            work("done", 1.0, position = 30.0, base = Timebase(frames = true, video = 29.97))
        }

    @Test
    fun doneFrames() =
        shot("done-frames", false) {
            work("done", 1.0, position = 30.0, base = Timebase(frames = true, video = 29.97))
        }

    // a phone at 360 dp: the slider gives way, in seconds and in frames
    @Test
    fun doneNarrow() = shot("done-360", false, narrow = true) { work("done", 1.0, position = 30.0) }

    // a recording that does not play: no play, why in the times' place, Locate… where it can be
    // found again
    private val moved = "Original not found"
    private val changed = "Original changed since transcribing"

    @Test
    fun doneMoved() =
        shot("done-moved", false) { work("done", 1.0, position = 30.0, why = moved, locate = true) }

    @Test
    fun doneMovedDark() =
        shot("done-moved", true) { work("done", 1.0, position = 30.0, why = moved, locate = true) }

    @Test
    fun doneChangedNarrow() =
        shot("done-changed-360", false, narrow = true) {
            work("done", 1.0, position = 30.0, why = changed, locate = true)
        }

    @Test
    fun doneCant() =
        shot("done-cant", false) { work("done", 1.0, why = "Android can't play this file") }

    @Test
    fun doneNarrowFrames() =
        shot("done-360-frames", false, narrow = true) {
            work("done", 1.0, position = 30.0, base = Timebase(frames = true, video = 29.97))
        }

    // the strip's targets with a search's ticks: a finger's (no keys) and the desktop's (keys), a
    // phone's and a tablet's
    @Test
    fun targetsFinger() =
        shot("targets", false, targets = true) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun targetsKeys() =
        shot("targets", false, keys = true, targets = true) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun targetsFingerTab() =
        shot("targets", false, tab = 1, targets = true) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun targetsKeysTab() =
        shot("targets", false, keys = true, tab = 1, targets = true) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun tabDoneLand() =
        shot("done", false, tab = 1) { work("done", 1.0, position = 30.0, playing = true) }

    @Test
    fun tabDonePort() =
        shot("done", false, tab = 2) { work("done", 1.0, position = 30.0, playing = true) }

    @Test
    fun tabFindLand() =
        shot("find", false, tab = 1) {
            work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
        }

    @Test
    fun tabReadyLand() =
        shot("ready-recent", false, tab = 1) { Ready(recent, readyActions, models) }

    @Test
    fun tabReadyPort() =
        shot("ready-recent", false, tab = 2) { Ready(recent, readyActions, models) }

    // the side, where the window has room for it: beside the drop zone, beside
    // a transcript of the list, marked, and heading the list with one under
    // way, its rows waiting, or stopped
    private val opened = Opened("5", "wonderland_ch_01.flac", "done", 1.0)

    @Test
    fun sideReady() =
        shot("side-ready", false, tab = 1) { Side(recent, readyActions, null, models) {} }

    @Test
    fun sideReadyDark() =
        shot("side-ready", true, tab = 1) { Side(recent, readyActions, null, models) {} }

    @Test
    fun sideDone() =
        shot("side-done", false, tab = 1) {
            Side(recent, readyActions, opened, models) {
                work("done", 1.0, position = 30.0, playing = true)
            }
        }

    @Test
    fun sideDoneDark() =
        shot("side-done", true, tab = 1) {
            Side(recent, readyActions, opened, models) {
                work("done", 1.0, position = 30.0, playing = true)
            }
        }

    @Test
    fun sideDoneKeys() =
        shot("side-done", false, keys = true, tab = 1) {
            Side(recent, readyActions, opened, models) {
                work("done", 1.0, position = 30.0, playing = true)
            }
        }

    @Test
    fun sideFind() =
        shot("side-find", false, tab = 1) {
            Side(recent, readyActions, opened, models) {
                work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
            }
        }

    @Test
    fun sideTranscribing() =
        shot("side-transcribing", false, tab = 1) {
            Side(
                recent,
                readyActions,
                Opened("6", "wonderland_ch_02.flac", "transcribing", 0.42),
                models,
            ) {
                work("transcribing", 0.42, segments.take(6), name = "wonderland_ch_02.flac")
            }
        }

    @Test
    fun sideTranscribingDark() =
        shot("side-transcribing", true, tab = 1) {
            Side(
                recent,
                readyActions,
                Opened("6", "wonderland_ch_02.flac", "transcribing", 0.42),
                models,
            ) {
                work("transcribing", 0.42, segments.take(6), name = "wonderland_ch_02.flac")
            }
        }

    @Test
    fun sideStopped() =
        shot("side-stopped", false, tab = 1) {
            Side(
                recent.take(2),
                readyActions,
                Opened("6", "wonderland_ch_02.flac", "stopped", 0.42),
                models,
            ) {
                work("stopped", 0.42, segments.take(6), name = "wonderland_ch_02.flac")
            }
        }

    @Test
    fun sideFirst() =
        shot("side-first", false, tab = 1) {
            Side(
                emptyList(),
                readyActions,
                Opened("6", "wonderland_ch_01.flac", "transcribing", 0.42),
                models,
            ) {
                work("transcribing", 0.42, segments.take(6))
            }
        }

    @Test
    fun sideLeast() =
        shot("side-done", false, tab = 3) {
            Side(recent, readyActions, opened, models) {
                work("done", 1.0, position = 30.0, playing = true)
            }
        }

    @Test
    fun sideLeastFind() =
        shot("side-find", false, tab = 3) {
            Side(recent, readyActions, opened, models) {
                work("done", 1.0, position = 30.0, find = finder(segments, "alice", k = 2))
            }
        }

    @Test
    fun error() =
        shot("error", false) {
            work(
                "error",
                0.0,
                emptyList(),
                error = "Can't read this file (Android has no decoder for audio/alac)",
            )
        }

    // a talk of two, conv2.wav (libpktito's diar tests) as the app keeps it with its speakers,
    // from the desktop's UI harness: who said what (Who.kt)
    private val talk = JSONObject(String(resource("conv2.json")))
    private val talkRows = Transcriber.parse(talk.getJSONArray("segments"))
    private val talkBar = Stretch.parse(talk.getJSONArray("stretches"))
    private val talkPeaks =
        talk.getJSONArray("peaks").let { a -> ByteArray(a.length()) { a.getInt(it).toByte() } }

    @Composable
    private fun talk(
        names: List<String> = emptyList(),
        on: Boolean = true,
        busy: Boolean = false,
        rows: List<Seg> = talkRows,
        bar: List<Stretch> = talkBar,
        heard: Double? = null,
        /** the row at the list's top */
        first: Int = 0,
        move: (Speakers.Turn, Int, Boolean) -> Unit = { _, _, _ -> },
        find: Finder = Finder(),
        hits: List<Hit> = emptyList(),
        onFind: (Int) -> Unit = {},
    ) {
        val res = LocalContext.current.resources
        val present = Speakers.present(rows)
        val tb = Timebase(duration = 60.0)
        Work(
            name = "Kitchen conversation.wav",
            step = "done",
            done = 1.0,
            duration = 60.0,
            peaks = talkPeaks,
            rows = rows,
            error = null,
            why = null,
            locate = null,
            stats = "Transcribed in 0:06, 10.9× real time",
            pace = null,
            times = false,
            playable = true,
            playing = false,
            position = 0.0,
            follow = Follow(rememberLazyListState(first)),
            sound = Sound(1.0, Player.SPEEDS, 1.0, false, {}, { _, _ -> }, {}),
            chosen = Timebase(),
            find = find,
            hits = hits,
            a = actions,
            spk =
                SpeakersView(
                    offered = true,
                    on = on,
                    busy = busy,
                    heard = heard,
                    shown = on && present.isNotEmpty(),
                    turns = on && present.size > 1,
                    place = (on && present.size > 1) || heard != null,
                    editable = true,
                    present = present,
                    nameOf = { Speakers.nameOf(res, names, it) },
                    bar = bar,
                    upto = Double.MAX_VALUE,
                    toggle = {},
                    move = move,
                    rename = {},
                    times = { u -> "${tb.at(u.start)}–${tb.at(u.end)}" },
                    find = onFind,
                ),
        )
    }

    // a speaker found (Finder.findWho): Speaker 2's turns are the matches, the field with their
    // name, the first turn's name marked; wider, their name over the transcript pressed
    private fun found(n: Int, k: Int = 0) =
        turnsOf(talkRows, n).let { h ->
            Finder().apply {
                findWho(n)
                settle(h, 0.0)
                if (k > 0) go(h, k)
            } to h
        }

    @Test
    fun speakersFind() {
        val (f, h) = found(2)
        shot("speakers-find", false) { talk(find = f, hits = h) }
    }

    @Test
    fun speakersFindDark() {
        val (f, h) = found(2, 1)
        shot("speakers-find", true) { talk(find = f, hits = h) }
    }

    @Test
    fun speakersFindWide() {
        val (f, h) = found(2)
        shot("speakers-find", false, wide = true) { talk(find = f, hits = h) }
    }

    // a phone has no names over the transcript: Who's speaking finds them
    @Test
    fun speakersFindWho() {
        val asked = ArrayList<Int>()
        shot("speakers-find-who", false, act = whoDialog) { talk(onFind = { asked += it }) }
        compose.onNodeWithText("Find where Speaker 2 speaks").performClick()
        assertEquals(listOf(2), asked)
    }

    // Change speaker in the menu a held word's selection brings (Rows): Robolectric draws no
    // toolbar of the platform's, so its items are taken by a provider of the test's own
    private val held = ArrayList<TextContextMenuComponent>()
    private val toolbar =
        object : TextContextMenuProvider {
            override suspend fun showTextContextMenu(dataProvider: TextContextMenuDataProvider) {
                held.clear()
                held.addAll(dataProvider.data().components)
                awaitCancellation()
            }
        }

    /** word held, Change speaker chosen in its menu: Who's speaking for its sentence alone */
    private fun changeSpeaker(word: String) = {
        // the text's own node, not its row's, which merges it: the row's middle is past the word
        compose.onNodeWithText(word, substring = true, useUnmergedTree = true).performTouchInput {
            longClick()
        }
        compose.waitForIdle()
        held
            .filterIsInstance<TextContextMenuItem>()
            .first { it.label == "Change speaker" }
            .onClick(
                object : TextContextMenuSession {
                    override fun close() {}
                }
            )
        compose.waitForIdle()
        compose.onNode(isDialog())
    }

    @Test
    @Config(shadows = [NoMagnifier::class])
    fun speakersChange() {
        val moved = ArrayList<Pair<Speakers.Turn, Int>>()
        shot("speakers-change", false, act = changeSpeaker("Hoorang")) {
            CompositionLocalProvider(LocalTextContextMenuToolbarProvider provides toolbar) {
                talk(move = { u, to, _ -> moved.add(u to to) })
            }
        }
        // the third sentence alone, of Speaker 2's turn of two, given to Speaker 1
        compose.onNodeWithText("Speaker 1").performClick()
        val (u, to) = moved.single()
        assertEquals(listOf(2, 2, 2, 1), listOf(u.from, u.to, u.speaker, to))
    }

    // the menu has it only where the speakers show and can be changed
    @Test
    @Config(shadows = [NoMagnifier::class])
    fun speakersChangeOff() {
        shot(
            "speakers-change-off",
            false,
            act = {
                compose
                    .onNodeWithText("Hoorang", substring = true, useUnmergedTree = true)
                    .performTouchInput { longClick() }
                compose.waitForIdle()
                compose.onRoot()
            },
        ) {
            CompositionLocalProvider(LocalTextContextMenuToolbarProvider provides toolbar) {
                talk(on = false)
            }
        }
        assertTrue(held.isNotEmpty())
        assertTrue(held.none { it is TextContextMenuItem && it.label == "Change speaker" })
    }

    @Test fun speakers() = shot("speakers", false) { talk() }

    @Test fun speakersDark() = shot("speakers", true) { talk() }

    @Test fun speakersNamed() = shot("speakers-named", false) { talk(listOf("Ada", "Ben Okafor")) }

    @Test
    fun speakersNamedDark() = shot("speakers-named", true) { talk(listOf("Ada", "Ben Okafor")) }

    @Test fun speakersWide() = shot("speakers", false, wide = true) { talk(listOf("Ada")) }

    @Test fun speakersWideDark() = shot("speakers", true, wide = true) { talk(listOf("Ada")) }

    @Test fun speakersOff() = shot("speakers-off", false) { talk(on = false) }

    // one voice: no names on the turns and no bar, its name beside the switch
    @Test
    fun speakersOne() =
        shot("speakers-one", false, wide = true) {
            talk(
                rows = talkRows.map { it.by(1) },
                bar = talkBar.map { Stretch(it.start, it.end, 1) },
            )
        }

    // twice the text size, as the system's setting gives it: Compose's sp
    // follow LocalDensity's fontScale (the configuration's alone moves none)
    @Test
    fun speakersLarge() =
        shot("speakers-large", false) {
            val d = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(d.density, 2f)) { talk() }
        }

    @Test fun speakersXa() = shot("speakers-360", false, narrow = true, xa = true) { talk() }

    // a transcript from before the switch, its speakers being found: the ring in play's place,
    // and the bar filling in as the diarizer tells them apart, its rows some way behind what it
    // has heard (Speakers.growing)
    private val talkHeard =
        talkBar
            .filter { it.start < 18.0 }
            .map { Stretch(it.start, minOf(it.end, 18.0), it.speaker) }

    @Test
    fun speakersFinding() =
        shot("speakers-finding", false) {
            talk(busy = true, rows = talkRows.map { it.by(null) }, bar = talkHeard, heard = 0.42)
        }

    @Test
    fun speakersFindingDark() =
        shot("speakers-finding", true) {
            talk(busy = true, rows = talkRows.map { it.by(null) }, bar = talkHeard, heard = 0.42)
        }

    // the find just begun: the bar's place, empty, the strip its height without speakers
    @Test
    fun speakersFindingStart() =
        shot("speakers-finding-start", false) {
            talk(busy = true, rows = talkRows.map { it.by(null) }, bar = emptyList(), heard = 0.0)
        }

    // conv2's tenth sentence cut in three by a stray word, "falling" heard as Speaker 1's (a
    // change of speaker ends a sentence), the bar drawn from the rows' turns as MainActivity draws
    // it
    private val strayRows: List<Seg> =
        talkRows.toMutableList().also { rows ->
            val w = rows[10].words
            fun part(a: Int, b: Int, speaker: Int) =
                w.subList(a, b).let { ws ->
                    Seg(
                        ws.first().start,
                        ws.last().end,
                        ws.joinToString(" ") { it.text.trim() },
                        ws,
                        speaker,
                    )
                }
            rows.removeAt(10)
            rows.addAll(10, listOf(part(0, 5, 2), part(5, 6, 1), part(6, w.size, 2)))
        }

    private fun turns(rows: List<Seg>) = rows.mapNotNull { s ->
        s.speaker?.let { Stretch(s.start, s.end, it) }
    }

    @Test
    fun speakersStray() =
        shot("speakers-stray", false) { talk(rows = strayRows, bar = turns(strayRows), first = 8) }

    // the stray moved back to Speaker 2 (Speakers.move): the sentence one again, the words joined
    // to it lit in Speaker 2's colour, 100 ms into the light's 1.6 s
    @Test
    fun speakersRejoined() {
        val t = Transcriber
        val app = RuntimeEnvironment.getApplication()
        t.segments.clear()
        t.segments.addAll(strayRows)
        val before = Speakers.move(app, Speakers.turnAt(t.segments, 11), 2, false)!!
        assertEquals(talkRows.size, t.segments.size)
        assertEquals(talkRows[10].text, t.segments[10].text)
        compose.mainClock.autoAdvance = false
        shot(
            "speakers-rejoined",
            false,
            act = {
                compose.mainClock.advanceTimeBy(100)
                compose.onRoot()
            },
        ) {
            talk(rows = t.segments, bar = turns(t.segments), first = 8)
        }
        Speakers.undo(app, before)
        t.segments.clear()
    }

    // all 8 of the diarizer's speakers in use, conv2's rows dealt among them: the (i) after the
    // names, and what it says
    private val eight = talkRows.mapIndexed { k, r -> r.by(k % Speakers.MOST + 1) }
    private val eightBar = eight.map { Stretch(it.start, it.end, it.speaker!!) }
    private val limitNote = {
        compose.onNodeWithContentDescription("Up to 8 speakers").performClick()
        compose.onNode(isPopup())
    }

    @Test fun speakersEight() = shot("speakers-eight", false) { talk(rows = eight, bar = eightBar) }

    @Test
    fun speakersEightWide() =
        shot("speakers-eight", false, wide = true) { talk(rows = eight, bar = eightBar) }

    // a phone's: the note at Who's speaking's foot, where New speaker is off
    @Test
    fun speakersEightWho() =
        shot("speakers-eight-who", false, act = whoDialog) { talk(rows = eight, bar = eightBar) }

    @Test
    fun speakersLimit() =
        shot("speakers-limit", false, wide = true, act = limitNote) {
            talk(rows = eight, bar = eightBar)
        }

    @Test
    fun speakersLimitDark() =
        shot("speakers-limit", true, wide = true, act = limitNote) {
            talk(rows = eight, bar = eightBar)
        }

    // a turn's name in its text, a link (Sentence): on a phone Who's speaking is a dialog, on a
    // tablet a menu under the name
    private fun tapName(name: String) =
        compose.onAllNodesWithText("$name:", substring = true).onFirst().performFirstLinkClick()

    private val whoDialog = {
        tapName("Speaker 2")
        compose.onNode(isDialog())
    }
    private val whoMenu = {
        tapName("Speaker 2")
        compose.onNode(isPopup())
    }

    @Test fun speakersWho() = shot("speakers-who", false, act = whoDialog) { talk() }

    @Test fun speakersWhoDark() = shot("speakers-who", true, act = whoDialog) { talk() }

    @Test fun speakersWhoWide() = shot("speakers-who", false, wide = true, act = whoMenu) { talk() }

    private val renameAsk = { compose.onNode(isDialog()) }

    @Test
    fun speakersRename() =
        shot("speakers-rename", false, act = renameAsk) {
            var who by remember { mutableStateOf<Int?>(null) }
            Box {
                talk()
                who?.let { n ->
                    RenameSpeaker(
                        "Speaker $n",
                        joins = { v ->
                            if (v.trim().equals("Speaker 2", true)) "Joins Speaker 2" else null
                        },
                        onRename = { who = null },
                        onCancel = { who = null },
                    )
                }
            }
            LaunchedEffect(Unit) { who = 1 }
        }

    @Test
    fun speakersUndo() =
        shot("speakers-undo", false) {
            Box(Modifier.fillMaxSize()) {
                talk()
                UndoToast(
                    "Moved to Speaker 2",
                    {},
                    Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp),
                )
            }
        }

    @Test
    fun speakersUndoDark() =
        shot("speakers-undo", true) {
            Box(Modifier.fillMaxSize()) {
                talk()
                UndoToast(
                    "Moved to Speaker 2",
                    {},
                    Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp),
                )
            }
        }

    @Test
    fun speakersAsk() =
        shot("speakers-ask", false, act = { compose.onNode(isDialog()) }) {
            Box {
                talk(on = false)
                Confirm(
                    "Download the speaker model",
                    "To tell the speakers apart, pktito needs a second open-source AI model " +
                        "from NVIDIA (100\u00a0MB). It handles up to 8 speakers. Download it now?",
                    "Download",
                    {},
                    {},
                    safe = true,
                )
            }
        }
}
