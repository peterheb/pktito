package net.pktito.app

import net.pktito.app.ui.rangeOf
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The waveform's range (rangeOf) on outlines of 2400 slices, as the app keeps them, of narration
 * with its pauses: a stretch of minutes quieter than the rest takes the floor 3 dB under it; one of
 * silence, or a file at one level, keeps the 10th percentile's. Each the desktop's rangeOf gives on
 * the same slices (ui/app.js, under node).
 */
@RunWith(RobolectricTestRunner::class)
class WaveTest {
    /** Narration at 150, a pause each 9 slices, and slices 1008 to 1271 (11%) at quiet. */
    private fun outline(quiet: Int) =
        ByteArray(2400) { i ->
            val base = if (i in 1008 until 1272) quiet else 150
            (base + (i * 37) % 21 - 10 + (if (i % 9 == 0) -45 else 0)).coerceIn(0, 255).toByte()
        }

    @Test
    fun aQuieterStretchTakesTheFloorUnderIt() =
        assertEquals(63.0 to 160.0, rangeOf(outline(80), 10731.0)) // the 10th percentile: 89

    @Test fun silenceKeepsTheFloor() = assertEquals(49.0 to 160.0, rangeOf(outline(40), 10731.0))

    @Test
    fun aFileAtOneLevelKeepsTheFloor() = assertEquals(113.0 to 160.0, rangeOf(outline(150), 640.0))
}
