package net.pktito.app

import net.pktito.app.ui.hms
import net.pktito.app.ui.rateLabel
import net.pktito.app.ui.stamp
import net.pktito.app.ui.timecode
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The times as the desktop's window writes them: timecodes.json is ui/app.js's
 * own timecode, rateLabel, stamp and hms, run by node (android/timecodes.js)
 * over 425 times (0 to 100 hours, the minutes drop-frame skips and their
 * neighbours, 400 drawn at random), at 19 rates (the time menu's, NTSC's
 * written three ways, others) and in files of 11 lengths, and Times.kt is to
 * write each the same.
 */
@RunWith(RobolectricTestRunner::class)
class TimesTest {
    private val v = JSONObject(String(javaClass.getResourceAsStream("/timecodes.json")!!.readBytes()))

    @Test
    fun timecodes() {
        val a = v.getJSONArray("codes")
        for (i in 0 until a.length()) {
            val c = a.getJSONArray(i)
            val t = c.getDouble(0)
            val r = c.getDouble(1)
            assertEquals("timecode($t, $r)", c.getString(2), timecode(t, r))
            assertEquals("rateLabel($r)", c.getString(3), rateLabel(r))
        }
    }

    @Test
    fun secondsInAFileOfALength() {
        val a = v.getJSONArray("stamps")
        for (i in 0 until a.length()) {
            val c = a.getJSONArray(i)
            val t = c.getDouble(0)
            val len = c.getDouble(1)
            assertEquals("stamp($t, $len)", c.getString(2), stamp(t, len))
            assertEquals("hms($t, $len)", c.getString(3), hms(t, len))
        }
    }

    @Test
    fun secondsWithAllTheHours() {
        val a = v.getJSONArray("full")
        for (i in 0 until a.length()) {
            val c = a.getJSONArray(i)
            val t = c.getDouble(0)
            assertEquals("stamp($t)", c.getString(1), stamp(t))
            assertEquals("hms($t)", c.getString(2), hms(t))
        }
    }
}
