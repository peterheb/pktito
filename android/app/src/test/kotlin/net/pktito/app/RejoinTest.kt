package net.pktito.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Speakers.rejoin, the desktop's rule (app.js, rejoin): a sentence a stray word cut in two is one
 * again once the word is moved back; a sentence that ends, another speaker's, or one past 30 s is
 * left as it is. Under Robolectric for Speakers' main thread.
 */
@RunWith(RobolectricTestRunner::class)
class RejoinTest {
    private fun seg(a: Double, b: Double, text: String, speaker: Int?) =
        Seg(
            a,
            b,
            text,
            text.split(" ").let { ws ->
                val per = (b - a) / ws.size
                ws.mapIndexed { k, w -> Word(w, a + k * per, a + (k + 1) * per) }
            },
            speaker,
        )

    private fun texts(segs: List<Seg>) = segs.map { "${it.speaker}: ${it.text}" }

    /** The panel's 28:25: "you" was Speaker 3's, cutting Speaker 5's sentence; moved back. */
    @Test
    fun strayBackJoinsAllThree() {
        val segs =
            mutableListOf(
                seg(1700.0, 1706.5, "I was reading it and", 5),
                seg(1706.5, 1706.7, "you", 5),
                seg(1706.7, 1709.0, "know, it looks okay.", 5),
                seg(1709.5, 1712.0, "Right.", 3),
            )
        // the row that grew, 0, and its 5 words before "you"
        assertEquals(0 to 5, Speakers.rejoin(segs, 1, 1))
        assertEquals(
            listOf("5: I was reading it and you know, it looks okay.", "3: Right."),
            texts(segs),
        )
        assertEquals(1709.0, segs[0].end, 0.0)
        assertEquals(10, segs[0].words.size)
    }

    @Test
    fun joinedRowAndItsWords() {
        val segs =
            mutableListOf(
                seg(0.0, 2.0, "So we went", 1),
                seg(2.0, 3.0, "home.", 1),
            )
        // the row that grew, 0, and its 3 words before
        assertEquals(0 to 3, Speakers.rejoin(segs, 1, 1))
    }

    @Test
    fun aSentenceThatEndsStaysAsItIs() {
        val segs =
            mutableListOf(
                seg(0.0, 2.0, "Yes.", 2),
                seg(2.0, 3.0, "Exactly.", 2),
                seg(3.0, 5.0, "I said “no.”", 2),
                seg(5.0, 6.0, "Fine", 2),
            )
        assertNull(Speakers.rejoin(segs, 1, 2))
        assertEquals(4, segs.size)
    }

    @Test
    fun anotherSpeakerOrNoneStaysApart() {
        val segs =
            mutableListOf(
                seg(0.0, 2.0, "and then", 1),
                seg(2.0, 3.0, "we left", 2),
                seg(3.0, 4.0, "the house", null),
                seg(4.0, 5.0, "early.", null),
            )
        assertNull(Speakers.rejoin(segs, 1, 2))
        assertEquals(4, segs.size)
    }

    @Test
    fun notPastThirtySeconds() {
        val segs =
            mutableListOf(
                seg(0.0, 20.0, "a long run of words with no end", 4),
                seg(20.0, 30.5, "and more", 4),
            )
        assertNull(Speakers.rejoin(segs, 1, 1))
        // exactly 30 s is a sentence's most, and joins
        segs[1] = seg(20.0, 30.0, "and more", 4)
        assertEquals(0 to 8, Speakers.rejoin(segs, 1, 1))
    }

    @Test
    fun onlyAroundTheRowsChanged() {
        // rows 0-1 would join, but the change was at 3 (from 2 through 4)
        val segs =
            mutableListOf(
                seg(0.0, 1.0, "far away", 1),
                seg(1.0, 2.0, "too", 1),
                seg(2.0, 3.0, "Hello.", 2),
                seg(3.0, 4.0, "and so", 2),
                seg(4.0, 5.0, "on.", 2),
            )
        assertEquals(3 to 2, Speakers.rejoin(segs, 3, 3))
        assertEquals(
            listOf("1: far away", "1: too", "2: Hello.", "2: and so on."),
            texts(segs),
        )
    }
}
