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
 * Only matches turned on keeps the reader on their sentence or gives them the match nearest it, and
 * turned off has them back where they were (Follow.keep), as the desktop's and iOS's do: in a list
 * of a hundred rows of two heights, keyed as the transcript's are, every third row shown with the
 * switch on.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class KeepTest {
    @get:Rule val compose = createComposeRule()

    private val everyThird = IntArray(34) { it * 3 }
    /** a match far above, and none again until row 60: none near row 40 */
    private val farOn = intArrayOf(3) + IntArray(40) { 60 + it }
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

    /**
     * The switch turned, the reader on the match in row hit, the playhead in row playing: the rows
     * shown become to.
     */
    private fun turn(to: IntArray?, hit: Int?, playing: Int? = null, plays: Boolean = false) {
        compose.runOnIdle {
            // as the screen does: the rows to show change, then the list is asked
            val was = view
            view = to
            follow.keep(was, to, hit, playing, plays)
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
     * The match they are on out of sight: the first match in sight stays, 42 (40 and 41 go); turned
     * off, the reader's row before, 41, the first wholly in sight then, is back where it was.
     */
    @Test
    fun awayFromTheMatchTheFirstMatchInSightThenBack() {
        list(40, 30)
        val y = at(42)
        val mine = at(41)
        assertNotNull(y)
        turn(everyThird, 3)
        assertEquals("on", y, at(42))
        turn(null, 3)
        assertEquals("off", mine, at(41))
    }

    /**
     * No match in sight: the next after the reader's row comes to where theirs was, not the match
     * they are on, far above; turned off, their row is back, and so each time.
     */
    @Test
    fun noneInSightTheNextMatchThenBack() {
        list(40, 0)
        for (n in 1..3) {
            turn(farOn, 3)
            assertEquals("on, time $n", 0, at(60))
            turn(null, 3)
            assertEquals("off, time $n", 0, at(40))
        }
    }

    /** Past the last match: the last one. */
    @Test
    fun pastThemAllTheLast() {
        list(40, 0)
        turn(intArrayOf(3, 20), 3)
        assertNotNull(at(20))
    }

    /** The reader moved while it was on: turned off, they stay on the row they are on. */
    @Test
    fun movedTheyStay() {
        list(40, 0)
        turn(farOn, 3)
        compose.runOnIdle { follow.moved() }
        turn(null, 3)
        assertEquals(0, at(60))
    }

    /**
     * With the row playing as it was turned on: turned off, the row playing by then is where theirs
     * was.
     */
    @Test
    fun withThePlayheadBackToIt() {
        list(40, 30)
        val y = at(41)
        turn(farOn, 3, playing = 41, plays = true)
        turn(null, 3, playing = 47, plays = true)
        assertEquals(y, at(47))
    }
}
