package net.pktito.app.ui

import java.util.Locale
import kotlin.math.abs
import kotlin.math.floor
import kotlin.math.roundToInt
import kotlin.math.roundToLong

/*
 * How pktito writes a time, as the desktop's window does (ui/app.js,
 * formatting): the rows' and the strip's in seconds and hundredths, or in
 * frames, which the strip's time chooses; a length; a duration in words'
 * place.
 */

// times in ASCII digits whatever the language, as the desktop's are
internal fun two(n: Long) = String.format(Locale.ROOT, "%02d", n)

/**
 * hh:mm:ss, with the hours' digits that say something of the longest time
 * shown with it (len): none under an hour (10:40), one under ten (1:05:00),
 * so that a file's rows keep one width and the hours are not a wall of
 * zeros (Peter, 2026-10-25); a length on its own is its own len. Without
 * len, all of them (ui/app.js, hms).
 */
fun hms(s: Double, len: Double = 36000.0): String {
    val t = maxOf(0L, s.toLong())
    val h = t / 3600
    val ms = two(t / 60 % 60) + ":" + two(t % 60)
    val top = maxOf(len, t.toDouble())
    return if (top >= 36000) two(h) + ":" + ms else if (top >= 3600) "$h:$ms" else ms
}

/** hh:mm:ss.cc, a row's time and the strip's, the hours as hms has them (ui/app.js, stamp). */
fun stamp(s: Double, len: Double = 36000.0): String {
    val cs = Math.round(maxOf(0.0, s) * 100)
    return hms(cs / 100.0, len) + "." + two(cs % 100)
}

/** m:ss, or h:mm:ss from an hour (ui/app.js, clock). */
fun clock(s: Double): String {
    val t = maxOf(0L, Math.round(s))
    val h = t / 3600
    val m = t / 60 % 60
    return if (h > 0) "$h:${two(m)}:${two(t % 60)}" else "$m:${two(t % 60)}"
}

/** The rates the time menu offers, the desktop's (app.js, RATES). */
val RATES = listOf(23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 59.94, 60.0)

/**
 * A frame rate counted exactly: NTSC's 23.976, 29.97 and 59.94 are 24, 30
 * and 60 frames in 1.001 seconds, whatever their rounding says (app.js,
 * exactRate).
 */
fun exactRate(r: Double): Double {
    for (n in intArrayOf(24, 30, 48, 60, 120))
        if (abs(r - n * 1000.0 / 1001) < 0.01) return n * 1000.0 / 1001
    return if (abs(r - r.roundToInt()) < 0.01) r.roundToInt().toDouble() else r
}

/**
 * t as SMPTE timecode at a rate (app.js, timecode): hh:mm:ss:ff, counting
 * the frames of its nominal rate (24 for 23.976, 30 for 29.97), the frame
 * t falls in. At 29.97 and 59.94 it is drop-frame, hh:mm:ss;ff: the first
 * 2 (or 4) frame numbers of every minute but each tenth are skipped, so
 * that the clock keeps to real time.
 */
fun timecode(t: Double, rate: Double): String {
    val r = exactRate(rate)
    val nominal = r.roundToInt()
    val drop = abs(r - 30000.0 / 1001) < 1e-9 || abs(r - 60000.0 / 1001) < 1e-9
    var frames = floor(maxOf(0.0, t) * r + 1e-6).toLong()
    if (drop) {
        val d = if (nominal == 60) 4 else 2
        val minute = nominal * 60 - d
        val tens = minute * 10 + d
        val rem = frames % tens
        frames += 9L * d * (frames / tens) + if (rem >= d) d * ((rem - d) / minute) else 0
    }
    val s = frames / nominal
    return two(s / 3600) + ":" + two(s / 60 % 60) + ":" + two(s % 60) + (if (drop) ";" else ":") + two(frames % nominal)
}

/** A rate as the time menu writes it: 23.976, 24, 29.97 (app.js, rateLabel). */
fun rateLabel(r: Double): String {
    val x = (exactRate(r) * 1000).roundToLong() / 1000.0
    return if (x == floor(x)) x.toLong().toString() else x.toString()
}

/**
 * How the strip and the rows write a time (app.js, tc and lengthOf):
 * seconds and hundredths, the hours as the file's duration has them, or
 * frames, whose timecode is always hh:mm:ss:ff, as an editor's is; a
 * video's frames at its own rate (video, 0 for audio) unless the rate
 * chosen is to count a video's too (videoRate false, an editor's timeline
 * having one rate), an audio file's at the rate chosen.
 */
data class Timebase(
    val frames: Boolean = false,
    val fps: Double = 24.0,
    val videoRate: Boolean = true,
    val video: Double = 0.0,
    val duration: Double = 0.0,
) {
    val own get() = video > 0 && videoRate
    val rate get() = if (own) video else if (fps > 0) fps else 24.0
    fun at(t: Double) = if (frames) timecode(t, rate) else stamp(t, duration)
    fun length() = if (frames) timecode(duration, rate) else hms(duration, duration)
}
