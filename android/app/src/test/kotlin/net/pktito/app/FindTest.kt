package net.pktito.app

import net.pktito.app.ui.Finder
import net.pktito.app.ui.Hit
import net.pktito.app.ui.findIn
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The match the reader is on follows the playhead (Finder.follow), as the desktop's does (app.js,
 * followHit): a seek puts them on the match it last passed, though it lands short of the next;
 * playback, on each it passes. A step, and a search typed at rest, take the playhead to their match
 * (Finder.step, Finder.settle). Alice chapter 1's matches of "rabbit", in seconds.
 */
class FindTest {
    private val hits =
        listOf(14.72, 44.96, 53.60, 64.93, 75.65, 85.98, 94.54).mapIndexed { i, t ->
            Hit(i, 0, 6, t)
        }

    /**
     * The screen's seek (MainActivity), each kept in seeks: the playhead to s, the reader with it.
     */
    private fun Finder.seek(hits: List<Hit>, seeks: MutableList<Double>) = { s: Double ->
        seeks += s
        follow(hits, s, jump = true)
    }

    @Test
    fun followsThePlayhead() {
        val f = Finder()
        f.settle(hits, 0.0)
        assertEquals("before any, the first", 0, f.at)
        f.follow(hits, 60.0, jump = true)
        assertEquals("a seek: the last passed", 2, f.at)
        f.follow(hits, 61.0, jump = false)
        assertEquals("playback short of the next", 2, f.at)
        f.follow(hits, 65.0, jump = false)
        assertEquals("playback past one", 3, f.at)
        f.follow(hits, 1.0, jump = true)
        assertEquals("a seek before any: the first", 0, f.at)
    }

    @Test
    fun aStepSeeks() {
        val f = Finder()
        f.settle(hits, 0.0)
        val seeks = ArrayList<Double>()
        f.step(hits, 1, f.seek(hits, seeks))
        assertEquals(1, f.at)
        f.step(hits, -1, f.seek(hits, seeks))
        f.step(hits, -1, f.seek(hits, seeks))
        assertEquals("round the end", 6, f.at)
        assertEquals("the playhead to each", listOf(44.96, 14.72, 94.54), seeks)
    }

    @Test
    fun aSearchTypedAtRestSeeks() {
        val f = Finder()
        val seeks = ArrayList<Double>()
        f.query = "rabbit"
        f.settle(hits, 5.0, f.seek(hits, seeks))
        assertEquals("none passed: the first", 0, f.at)
        f.follow(hits, 60.0, jump = true)
        f.query = "rabbit "
        f.settle(hits, 60.0, f.seek(hits, seeks))
        assertEquals("typed on: the match kept", 2, f.at)
        val fewer = hits.filter { it.time != 53.60 }
        f.query = "rabbit h"
        f.settle(fewer, 53.60, f.seek(fewer, seeks))
        assertEquals("its match gone: the one passed", 1, f.at)
        assertEquals("the playhead to each", listOf(14.72, 53.60, 44.96), seeks)
    }

    @Test
    fun theSameSearchMarkedAnewKeepsThePlayhead() {
        val f = Finder()
        val seeks = ArrayList<Double>()
        f.query = "rabbit"
        f.settle(hits, 5.0, f.seek(hits, seeks))
        f.follow(hits, 30.0, jump = true)
        // the other view's, its rows the cues
        val cues = hits.map { Hit(it.row * 2, it.start, it.end, it.time) }
        f.settle(cues, 30.0, f.seek(cues, seeks))
        assertEquals(0, f.at)
        assertEquals("the other view's: the playhead left at 30", listOf(14.72), seeks)
        val turned = Finder()
        turned.restore("rabbit")
        turned.settle(hits, 30.0, turned.seek(hits, seeks))
        assertEquals("nor through a turn of the screen", listOf(14.72), seeks)
    }

    // two matches in one word share its time; the one moved to is the one
    private val twins = listOf(Hit(0, 0, 2, 10.0), Hit(0, 3, 5, 10.0), Hit(1, 0, 2, 20.0))

    @Test
    fun aTickKeepsItsOwn() {
        val f = Finder()
        f.settle(twins, 0.0)
        // the screen's order: the seek, then the tick's match (Screens.kt, onTick)
        f.follow(twins, 10.0, jump = true)
        f.go(twins, 0)
        assertEquals(0, f.at)
        f.follow(twins, 10.5, jump = false)
        assertEquals("playback from it keeps it", 0, f.at)
    }

    @Test
    fun aSearchTypedKeepsItsOwn() {
        val f = Finder()
        f.query = "ra"
        f.settle(twins, 0.0, f.seek(twins, ArrayList()))
        assertEquals("the seek first, which puts the reader on the second, then the first", 0, f.at)
    }

    /** Each search's matches start a word (startsWord), as the desktop's and iOS's do. */
    @Test
    fun aMatchStartsAWord() {
        fun at(text: String, q: String) = findIn(listOf(Seg(0.0, 1.0, text)), q).map { it.start }
        assertEquals(listOf(4, 21), at("The cat concatenates cats", "cat"))
        assertEquals(listOf(0), at("Cat scat", "cat"))
        assertEquals("an apostrophe ends a word", listOf(14), at("o'clock and O’Neill", "neill"))
        assertEquals(
            "a search not starting with a letter",
            listOf(5, 13),
            at("covid-19 and -19", "-19"),
        )
        assertEquals(listOf(5), at("x123 123", "123"))
        assertEquals(listOf(0, 5), at("Café caféine", "caf"))
    }
}
