package net.pktito.app

import android.content.res.Configuration
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.requestFocus
import java.io.File
import net.pktito.app.ui.Manage
import net.pktito.app.ui.ManageActions
import net.pktito.app.ui.PktitoTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Where the keys start in Manage models, as the desktop's (manage.*): on
 * the first Delete that can be pressed, past the model in use while a
 * transcription runs, or on Close where nothing can be deleted; and that a
 * question put away gives them back to the Delete that asked, rows changed
 * under it or not, and goes itself where a transcription greys its Delete.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w840dp-h900dp-xhdpi")
class ManageTest {
    @get:Rule
    val compose = createComposeRule()

    private fun offer(name: String, key: String, have: Boolean) =
        Offer(name, key, listOf("en"), "", 624_480_196, "", have, 0, File("$name.pkf"), File("$name.pkf.part"))
    private val both = listOf(offer("v2", "english", true), offer("v3", "multilingual", true))

    /** Whether a transcription runs, which a test may change while Manage models is up. */
    private var transcribing by mutableStateOf(false)

    private fun show(offers: List<Offer>, running: Boolean = false, download: Download? = null, own: File? = null) {
        transcribing = running
        compose.setContent {
            val c = Configuration(LocalConfiguration.current).apply {
                keyboard = Configuration.KEYBOARD_QWERTY
                hardKeyboardHidden = Configuration.HARDKEYBOARDHIDDEN_NO
            }
            CompositionLocalProvider(LocalConfiguration provides c) {
                PktitoTheme { Manage(offers, "v2", own, download, transcribing, null, 0, null, compact = false, ManageActions({}, {}, {}, {}, {})) }
            }
        }
        compose.waitForIdle()
    }

    @Test fun theKeysStartOnTheFirstDelete() {
        show(both)
        compose.onAllNodesWithText("Delete")[0].assertIsFocused()
    }

    @Test fun whileATranscriptionRunsTheyPassTheModelInUse() {
        show(both, running = true)
        compose.onAllNodesWithText("Delete")[0].assertIsNotEnabled()
        compose.onAllNodesWithText("Delete")[1].assertIsFocused()
    }

    @Test fun withNothingToDeleteTheyStartOnClose() {
        show(
            listOf(offer("v2", "english", false), offer("v3", "multilingual", false)),
            download = Download("v3", "downloading", 262_144_000, 647_552_644, 11.8e6, "models.pktito.net", null, retry = false, trying = false),
        )
        compose.onNodeWithText("Close").assertIsFocused()
    }

    @Test fun aQuestionPutAwayGivesThemBackToItsDelete() {
        show(both)
        compose.onAllNodesWithText("Delete")[1].requestFocus().performClick()
        compose.onNodeWithText("Cancel").performClick()
        compose.waitForIdle()
        compose.onAllNodesWithText("Delete")[1].assertIsFocused()
    }

    @Test fun rowsThatChangeUnderAQuestionLeaveThemOnItsDelete() {
        // the own file in use, so a transcription greys its Delete alone
        show(both, own = File("mine.pkf"))
        compose.onAllNodesWithText("Delete")[1].requestFocus().performClick()
        transcribing = true
        compose.waitForIdle()
        compose.onNodeWithText("Cancel").performClick()
        compose.waitForIdle()
        compose.onAllNodesWithText("Delete")[1].assertIsFocused()
    }

    @Test fun aTranscriptionThatGreysItsDeletePutsItsQuestionAway() {
        show(both)
        compose.onAllNodesWithText("Delete")[0].performClick()
        compose.onNodeWithText("Cancel").assertExists()
        transcribing = true
        compose.waitForIdle()
        compose.onAllNodesWithText("Cancel").assertCountEquals(0)
        compose.onAllNodesWithText("Delete")[0].assertIsNotEnabled()
    }
}
