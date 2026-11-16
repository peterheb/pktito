package net.pktito.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.dp
import net.pktito.app.ui.Follow
import net.pktito.app.ui.itemIn
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Only matches turned keeps the reader on their sentence, where it was on the screen (Follow.keep),
 * as the desktop's and iOS's do: in a list of a hundred rows of two heights, keyed as the
 * transcript's are, every third row shown with the switch on.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class KeepTest {
    @get:Rule val compose = createComposeRule()

    private val everyThird = IntArray(34) { it * 3 }
    private lateinit var follow: Follow
    /** the rows shown, null for all (shownRows) */
    private var view by mutableStateOf<IntArray?>(null)

    /** The list, its first row first, offset pixels of it above the top. */
    private fun list(first: Int, offset: Int) {
        compose.setContent {
            val state = remember { LazyListState(first, offset) }
            follow = remember { Follow(state) }
            val v = view
            LazyColumn(Modifier.fillMaxSize(), state = state) {
                items(v?.size ?: 100, key = { v?.get(it) ?: it }) { k ->
                    val row = v?.get(k) ?: k
                    Box(Modifier.fillMaxWidth().height(if (row % 2 == 0) 60.dp else 100.dp))
                }
            }
            // as the transcript's rows ask for it (Screens.kt, Rows)
            SideEffect { follow.take() }
        }
        compose.waitForIdle()
    }

    /** Where row is drawn, in pixels from the list's top; null where it isn't. */
    private fun at(row: Int): Int? {
        val k = itemIn(view, row)
        return follow.list.layoutInfo.visibleItemsInfo.firstOrNull { it.index == k }?.offset
    }

    /** The switch turned, the reader on the match in row hit: the rows shown become to. */
    private fun turn(to: IntArray?, hit: Int?) {
        compose.runOnIdle {
            // as the screen does: the rows to show change, then the list is asked
            val was = view
            view = to
            follow.keep(was, to, hit)
        }
        compose.waitForIdle()
    }

    @Test
    fun theMatchStaysWhereItWas() {
        list(40, 30)
        val y = at(45)
        assertNotNull(y)
        turn(everyThird, 45)
        assertEquals("on", y, at(45))
        turn(null, 45)
        assertEquals("off again", y, at(45))
    }

    /**
     * The match out of sight: the first row in sight that stays shown, 42 (40 and 41 go); turned
     * off, the first in sight then, 36, partly over the top, the rows after it back under it.
     */
    @Test
    fun awayFromTheMatchTheFirstRowThatStays() {
        list(40, 30)
        val y = at(42)
        assertNotNull(y)
        turn(everyThird, 3)
        assertEquals("on", y, at(42))
        val top = at(36)
        assertEquals(-30, top)
        turn(null, 3)
        assertEquals("off", top, at(36))
    }

    /** No row in sight stays shown: the match, brought into sight. */
    @Test
    fun noneInSightTheMatch() {
        list(40, 0)
        turn(intArrayOf(3, 90), 90)
        assertNotNull(at(90))
    }
}
