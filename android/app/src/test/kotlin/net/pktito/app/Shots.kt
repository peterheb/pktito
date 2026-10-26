package net.pktito.app

import android.graphics.Bitmap
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.isPopup
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import java.io.File
import net.pktito.app.ui.PktitoTheme
import net.pktito.app.ui.Ready
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
    private val cues = Transcriber.parseCues(String(resource("alice-cues.json")))
    private val peaks = resource("alice-peaks.bin")
    private val actions = WorkActions({}, {}, {}, {}, {}, {}, {}, {}, { _, _ -> })

    /** The screen, or what act leads to on it (a menu, its own window). */
    private fun shot(
        name: String, dark: Boolean, wide: Boolean = false,
        act: (() -> SemanticsNodeInteraction)? = null, content: @Composable () -> Unit,
    ) {
        RuntimeEnvironment.setQualifiers(if (wide) "w840dp-h900dp-xhdpi" else "w411dp-h891dp-xxhdpi")
        RuntimeEnvironment.setQualifiers(if (dark) "+night" else "+notnight")
        compose.setContent {
            PktitoTheme { Box(Modifier.fillMaxSize().background(pk.ground)) { content() } }
        }
        val dir = File("build/shots").apply { mkdirs() }
        val file = File(dir, "$name-${if (dark) "dark" else "light"}${if (wide) "-wide" else ""}.png")
        val node = act?.invoke() ?: compose.onRoot()
        file.outputStream().use { node.captureToImage().asAndroidBitmap().compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    @Composable
    private fun work(step: String, done: Double, rows: List<Seg> = segments, captions: Boolean = false,
                     position: Double = 0.0, playing: Boolean = false, error: String? = null) =
        Work(
            name = "wonderland_ch_01.flac", step = step, done = done,
            duration = if (step == "reading" || step == "loading") 0.0 else 640.505,
            peaks = if (step == "reading" || step == "loading") null else peaks,
            rows = rows, error = error,
            stats = if (step == "done") "Transcribed in 0:11, 58.2x real-time" else null,
            pace = if (step == "transcribing") "57.9x real-time" else null,
            captions = captions, times = false, playable = step == "done", playing = playing,
            position = position, list = rememberLazyListState(), a = actions,
        )

    @Test fun ready() = shot("ready", false) { Ready("English", true) {} }
    @Test fun readyDark() = shot("ready", true) { Ready("English", true) {} }
    @Test fun readyNoModel() = shot("ready-no-model", false) { Ready("", false) {} }
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
    @Test fun saveAsMenu() = shot("save-as", false, act = saveAs) { work("done", 1.0) }
    @Test fun saveAsMenuDark() = shot("save-as", true, act = saveAs) { work("done", 1.0) }
    @Test fun captions() = shot("captions", false) { work("done", 1.0, cues, captions = true, position = 12.0) }
    @Test fun error() = shot("error", false) {
        work("error", 0.0, emptyList(), error = "Can't read this file (Android has no decoder for audio/alac)")
    }
}
