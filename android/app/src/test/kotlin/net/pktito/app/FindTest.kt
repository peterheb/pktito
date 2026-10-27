package net.pktito.app

import net.pktito.app.ui.Finder
import net.pktito.app.ui.Hit
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The match the reader is on follows the playhead (Finder.follow), as the
 * desktop's does (app.js, followHit): a seek puts them on the match it last
 * passed, though it lands short of the next; playback, on each it passes; a
 * match stepped to stays until one or the other. Alice chapter 1's matches
 * of "rabbit", in seconds.
 */
class FindTest {
    private val hits = listOf(14.72, 44.96, 53.60, 64.93, 75.65, 85.98, 94.54).mapIndexed { i, t -> Hit(i, 0, 6, t) }

    @Test
    fun followsThePlayhead() {
        val f = Finder()
        f.settle(hits, 0.0)
        assertEquals("before any, the first", 0, f.at)
        f.follow(hits, 60.0, jump = true)
        assertEquals("a seek: the last passed", 2, f.at)
        f.step(hits, 1)
        f.step(hits, 1)
        assertEquals(4, f.at)
        f.follow(hits, 61.0, jump = false)
        assertEquals("playback short of the next keeps a step", 4, f.at)
        f.follow(hits, 62.0, jump = true)
        assertEquals("a seek short of the next does not", 2, f.at)
        f.follow(hits, 65.0, jump = false)
        assertEquals("playback past one", 3, f.at)
        f.follow(hits, 1.0, jump = true)
        assertEquals("a seek before any: the first", 0, f.at)
    }

    @Test
    fun aStepWhilePlayingSeeks() {
        val f = Finder()
        f.settle(hits, 0.0)
        val seeks = ArrayList<Double>()
        f.step(hits, 1) { seeks += it }
        assertEquals("playing: the playhead to the match", listOf(44.96), seeks)
        assertEquals(1, f.at)
        f.step(hits, -1, null)
        f.step(hits, -1, null)
        assertEquals("at rest: only the reader, round the end", 6, f.at)
        assertEquals(listOf(44.96), seeks)
    }

    @Test
    fun aTickKeepsItsOwn() {
        // two matches in one word share its time; the tick tapped is the one
        val twins = listOf(Hit(0, 0, 2, 10.0), Hit(0, 3, 5, 10.0), Hit(1, 0, 2, 20.0))
        val f = Finder()
        f.settle(twins, 0.0)
        // the screen's order: the seek, then the tick's match (Screens.kt, onTick)
        f.follow(twins, 10.0, jump = true)
        f.go(twins, 0)
        assertEquals(0, f.at)
        f.follow(twins, 10.5, jump = false)
        assertEquals("playback from it keeps it", 0, f.at)
    }
}
