package net.pktito.app.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import kotlin.math.abs
import kotlin.math.floor
import kotlin.math.roundToInt
import net.pktito.app.R

/** The strip's text, all of it term-ink: which label leads is said by where it is (app.css, .pk-tstamp). */
private val Tstamp = Type.stamp.copy(lineHeight = 1.em, fontFeatureSettings = "tnum")

/** What the strip's speed and speaker say and do: playback's (Player's), kept between runs by the screen. */
class Sound(
    val speed: Double,
    val speeds: List<Double>,
    val volume: Double,
    val muted: Boolean,
    val setSpeed: (Double) -> Unit,
    /** a level moved to, and whether it is let go there, to be kept */
    val setVolume: (Double, Boolean) -> Unit,
    val mute: () -> Unit,
)

/** A speed as the desktop writes it: 0.75×, 1×, 1.5× (app.js, speedMenu). */
fun rate(s: Double): String = (if (s == floor(s)) s.toInt().toString() else s.toString()) + "×"

/** The speed d steps from now among speeds, held at the ends (app.js, stepSpeed): Slower, Faster, < and >. */
fun stepped(speeds: List<Double>, now: Double, d: Int): Double =
    speeds[(speeds.indexOf(now) + d).coerceIn(0, speeds.size - 1)]

/**
 * The file's strip, on term in both themes (app.css, .pk-strip): play, the
 * speed under it on the times' line, or at work the progress in play's
 * place and size, so that the waveform keeps its width when the transcript
 * finishes; the waveform the rest of the width; under its two ends, the
 * time, which chooses how times are written, and the length, and after
 * the time the speaker and its volume. At work the waveform fills to the
 * playhead as the transcript does, and the time's place says the step or
 * the pace. A finished transcript whose recording does not play (why) has
 * no play, its waveform the whole width, and why in the times' place,
 * with Locate… where the file can be found again; a tap on the waveform
 * still moves the playhead (Peter, 2026-10-26: "Just no play button").
 *
 * A finger's strip (byFinger: no keyboard, so no trackpad, as on a phone
 * or a tablet) is a thumb's: play 48 dp, Android's least target, to the
 * desktop's 40; no speaker or slider, as the device's volume keys set the
 * level (Player.keysOnly); the ticks' lane 24 dp, where the times' line
 * had taken a thumb's taps under its 15 (the S25's mute, 2026-10-26); and
 * a drag on the waveform or the lane moves the playhead and the time with
 * the thumb, seeking where it is let go. 125 dp tall to the desktop's 116.
 */
@Composable
fun Strip(
    peaks: ByteArray?,
    review: Boolean,
    done: Double,
    playing: Boolean,
    playable: Boolean,
    position: Double,
    duration: Double,
    left: String,
    right: String,
    playLabel: String,
    onPlay: () -> Unit,
    onSeek: (Double) -> Unit,
    sound: Sound,
    base: Timebase,
    onBase: (Timebase) -> Unit,
    modifier: Modifier = Modifier,
    ticks: List<Double> = emptyList(),
    tickAt: Int = -1,
    onTick: (Int) -> Unit = {},
    why: String? = null,
    locate: (() -> Unit)? = null,
) {
    val p = pk
    val silent = review && why != null
    val finger = byFinger()
    val button = if (finger) 48.dp else 40.dp
    // where a thumb dragging on the waveform or the lane has the playhead, 0 to 1, until it lets go
    var scrub by remember { mutableStateOf<Double?>(null) }
    val seek by rememberUpdatedState(onSeek)
    val dragged = if (!finger || !review) Modifier
    else Modifier.pointerInput(Unit) {
        fun at(x: Float) = (x / size.width).toDouble().coerceIn(0.0, 1.0)
        detectHorizontalDragGestures(
            onDragStart = { scrub = at(it.x) },
            onDragEnd = {
                scrub?.let { seek(it) }
                scrub = null
            },
            onDragCancel = { scrub = null },
        ) { c, _ -> scrub = at(c.position.x) }
    }
    Row(
        modifier
            .fillMaxWidth()
            .height(IntrinsicSize.Min)
            .testTag("strip")
            .background(p.term, Corner)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        // 24 between play and the waveform, the desktop's (app.css, #strip):
        // the speed under play is wider than it from 1.25×, and at 14 its box
        // came within 3 px of the time's
        horizontalArrangement = Arrangement.spacedBy(Space.space4),
    ) {
        if (review && !silent)
            // play, and the speed at the foot, on the times' line (app.css, .pk-transport)
            Column(
                Modifier.fillMaxHeight().padding(top = (64.dp - button) / 2),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.SpaceBetween,
            ) {
                Box(
                    Modifier
                        .alpha(if (playable) 1f else 0.45f)
                        .control(Corner, p.termLine, onTerm = true, enabled = playable, label = playLabel, onClick = onPlay)
                        .size(button),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(if (playing) Icons.pause else Icons.play, null, tint = p.termInk, modifier = Modifier.size(if (finger) 24.dp else 20.dp))
                }
                Speed(sound, playable)
            }
        else if (!review)
            Box(Modifier.padding(top = (64.dp - button) / 2)) { Ring(done, button) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            val at = scrub ?: if (review) (if (duration > 0) position / duration else 0.0) else done
            Waveform(
                peaks, at,
                Modifier
                    .testTag("wave")
                    .alpha(if (review && !playable && !silent) 0.45f else 1f)
                    .then(
                        if (review)
                            Modifier.pointerInput(Unit) {
                                detectTapGestures { o -> seek((o.x / size.width).toDouble().coerceIn(0.0, 1.0)) }
                            }
                        else Modifier,
                    )
                    .then(dragged),
            )
            // the desktop's lane for a search's ticks, 9 dp, which keeps the strip its 116; a finger's, 18
            Ticks(ticks, tickAt, onTick, finger, if (finger && review) seek else null, dragged)
            if (silent) Why(why!!, locate)
            else TimesLine(
                time = {
                    // a dragged playhead's time, where the thumb would let it go
                    if (review) Time(scrub?.let { base.at(it * duration) } ?: left, base, position, onBase)
                    else Text(left, style = Tstamp, color = p.termInk, maxLines = 1)
                },
                speaker = { if (review && !finger) Speaker(sound, playable) },
                slider = {
                    if (review && !finger)
                        Level(
                            if (sound.muted) 0.0 else sound.volume, playable, stringResource(R.string.volume_label), sound.setVolume,
                            Modifier.bleed(vertical = 6.dp).alpha(if (playable) 1f else 0.45f),
                        )
                },
                length = { Text(right, style = Tstamp, color = p.termInk, maxLines = 1) },
            )
        }
    }
}

/**
 * A search's ticks in the lane under the waveform (app.css, .pk-ticks), each
 * match at its word's time, 2 dp of plume; the current one term-ink, 3 dp,
 * and taller by the lane's gaps, so that its shape tells it too. Drawn and
 * touched across the gaps, 15 dp, and a tap goes to the nearest tick within
 * 16 dp of it, the earlier where two are as near, as a click on the
 * desktop's does (f441c18): a tick is 9 wide there, and a finger is wider.
 * Over the times' line in touch: its controls reach 6 dp up into the lane
 * (bleed), and the line, after the lane, took the lower half of each tick
 * there; a tick tapped there muted the speaker (Peter's S25, 2026-10-26;
 * on the emulator, y 545 px of the lane's 516 to 558). A finger's lane
 * is 18 dp, 24 touched, at work too, so that the strip keeps its height;
 * in review a tap there with no tick near seeks there (miss), as one on
 * the waveform does, where the desktop's does nothing, and a drag along it
 * is the thumb's along the waveform (dragged): after the lane's own
 * modifiers, so that it has the lane's 24 dp, the bled ones too, which
 * lint's rule for a Modifier parameter, named modifier and first in the
 * chain, would take from it.
 */
@Suppress("ModifierParameter")
@Composable
private fun Ticks(ticks: List<Double>, at: Int, onTick: (Int) -> Unit, finger: Boolean, miss: ((Double) -> Unit)?, dragged: Modifier) {
    val p = pk
    val gap = 3.dp
    val tick by rememberUpdatedState(onTick)
    val missed by rememberUpdatedState(miss)
    Canvas(
        Modifier
            .zIndex(1f)
            .fillMaxWidth()
            .testTag("lane")
            .bleed(vertical = gap)
            .height((if (finger) 18.dp else 9.dp) + gap * 2)
            .then(
                if (ticks.isEmpty() && miss == null) Modifier
                else Modifier.pointerInput(ticks) {
                    detectTapGestures { o ->
                        val k = ticks.indices.minByOrNull { abs(ticks[it] * size.width - o.x) }
                        if (k != null && abs(ticks[k] * size.width - o.x) <= 16.dp.toPx()) tick(k)
                        else missed?.invoke((o.x / size.width).toDouble().coerceIn(0.0, 1.0))
                    }
                },
            )
            .then(dragged),
    ) {
        val g = gap.toPx()
        for ((k, t) in ticks.withIndex())
            if (k != at) drawLine(p.plume, Offset(t.toFloat() * size.width, g), Offset(t.toFloat() * size.width, size.height - g), 2.dp.toPx())
        ticks.getOrNull(at)?.let { t ->
            drawLine(p.termInk, Offset(t.toFloat() * size.width, 0f), Offset(t.toFloat() * size.width, size.height), 3.dp.toPx())
        }
    }
}

/**
 * The speed, a button on the times' line, its box 6 dp round its words and
 * the line kept its height (app.css, .pk-speed): on term-line when it is
 * not 1×, as play is. Its menu: the speeds as chips, then Slower and
 * Faster, with their keys where there are keys, a step of which keeps the
 * menu open for the next.
 */
@Composable
private fun Speed(sound: Sound, enabled: Boolean) {
    val p = pk
    var open by remember { mutableStateOf(false) }
    val speeds = sound.speeds
    val step = { d: Int -> sound.setSpeed(stepped(speeds, sound.speed, d)) }
    val keys = hasKeys()
    Box {
        Text(
            rate(sound.speed),
            style = Tstamp,
            color = p.termInk,
            maxLines = 1,
            modifier = Modifier
                .bleed(6.dp, 6.dp)
                .alpha(if (enabled) 1f else 0.45f)
                .control(
                    CornerSm, if (sound.speed != 1.0) p.termLine else Color.Transparent, onTerm = true, enabled = enabled,
                    label = stringResource(R.string.speed_label, rate(sound.speed).dropLast(1)),
                ) { open = true }
                .padding(6.dp),
        )
        // 6 dp under its box, which reaches 6 under the line (app.js, openMenu)
        Menu(open, { open = false }, gap = 12.dp) {
            Rates(speeds.map { rate(it) to it }, { it == sound.speed }, stringResource(R.string.speed_group), top = 8.dp) {
                sound.setSpeed(it)
                open = false
            }
            MenuLine()
            Item(stringResource(R.string.speed_slower), null, hint = if (keys) ({ KeyCap("<") }) else null) { step(-1) }
            Item(stringResource(R.string.speed_faster), null, hint = if (keys) ({ KeyCap(">") }) else null) { step(1) }
        }
    }
}

/**
 * The time, a button on the times' line as the speed is (app.css,
 * .pk-timebtn), where the times' format is chosen (app.js, timeMenu):
 * seconds and hundredths, or frames, each with the time now in it, and
 * under Frames the rates, a video's own first. A rate picked is a rate to
 * count in; picked over a video's own, it is the rate for every video.
 */
@Composable
private fun Time(text: String, base: Timebase, now: Double, onBase: (Timebase) -> Unit) {
    val p = pk
    var open by remember { mutableStateOf(false) }
    val pick = { b: Timebase ->
        onBase(b)
        open = false
    }
    Box {
        Text(
            text,
            style = Tstamp,
            color = p.termInk,
            maxLines = 1,
            modifier = Modifier
                .bleed(6.dp, 6.dp)
                .control(CornerSm, onTerm = true, label = stringResource(R.string.times_tip)) { open = true }
                .padding(6.dp),
        )
        Menu(open, { open = false }, gap = 12.dp, wide = true) {
            val check = @Composable { Icon(Icons.check, null, tint = p.ink, modifier = Modifier.size(16.dp)) }
            Item(stringResource(R.string.times_seconds), stamp(now, base.duration), hint = if (!base.frames) check else null, selected = !base.frames) {
                pick(base.copy(frames = false))
            }
            Item(
                stringResource(R.string.times_frames),
                stringResource(
                    if (base.own) R.string.times_frames_at_video else R.string.times_frames_at,
                    timecode(now, base.rate), rateLabel(base.rate),
                ),
                hint = if (base.frames) check else null,
                selected = base.frames,
            ) { pick(base.copy(frames = true)) }
            // the video's own rate, a null among the rates
            val chips: List<Pair<String, Double?>> =
                (if (base.video > 0) listOf(stringResource(R.string.times_video_rate, rateLabel(base.video)) to null) else emptyList()) +
                    RATES.map { rateLabel(it) to it }
            Rates(
                chips,
                { r -> if (r == null) base.own else !base.own && abs(r - base.rate) < 0.001 },
                stringResource(R.string.times_rates),
                top = 2.dp,
            ) { r ->
                pick(
                    if (r == null) base.copy(frames = true, videoRate = true)
                    else base.copy(frames = true, fps = r, videoRate = if (base.video > 0) false else base.videoRate),
                )
            }
        }
    }
}

/**
 * The times' line (app.css, .pk-times): the time, the speaker 20 dp after
 * it and the slider 8 after that, and the length at the end. It keeps what
 * fits, the times first and 12 dp before the length: the slider gives way
 * first, as a phone's volume keys stand in for it, then the speaker. At 411
 * dp a phone had 283 for 292 in frames, and 360 dp or a large font leaves
 * less; the desktop's window is never under 480.
 */
@Composable
private fun TimesLine(
    time: @Composable () -> Unit,
    speaker: @Composable () -> Unit,
    slider: @Composable () -> Unit,
    length: @Composable () -> Unit,
) {
    // a box each, so that one with nothing in it is there at no size
    Layout({ Box { time() }; Box { speaker() }; Box { slider() }; Box { length() } }) { m, c ->
        val (t, s, v, l) = m.map { it.measure(c.copy(minWidth = 0)) }
        val fits = { w: Int -> w + 12.dp.roundToPx() <= c.maxWidth }
        val speakerAt = t.width + 20.dp.roundToPx()
        val sliderAt = speakerAt + s.width + 8.dp.roundToPx()
        val withSpeaker = s.width > 0 && fits(speakerAt + s.width + l.width)
        val withSlider = withSpeaker && v.width > 0 && fits(sliderAt + v.width + l.width)
        val h = maxOf(t.height, s.height, v.height, l.height)
        layout(c.maxWidth, h) {
            t.place(0, (h - t.height) / 2)
            if (withSpeaker) s.place(speakerAt, (h - s.height) / 2)
            if (withSlider) v.place(sliderAt, (h - v.height) / 2)
            l.place(c.maxWidth - l.width, (h - l.height) / 2)
        }
    }
}

/**
 * Why the recording does not play, in the times' place, whose time, speed
 * and volume mean nothing without it: one quiet line in the strip's ink,
 * two where a phone is too narrow, and Locate… after it, a control as the
 * speaker is, where the file can be found again.
 */
@Composable
private fun Why(why: String, locate: (() -> Unit)?) {
    val p = pk
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(why, style = Tstamp.copy(lineHeight = 1.3.em), color = p.termInk, maxLines = 2, modifier = Modifier.weight(1f, fill = false))
        if (locate != null) {
            val label = stringResource(R.string.play_locate)
            Text(" ·", style = Tstamp, color = p.termInk)
            Box(
                Modifier
                    .bleed(vertical = 6.dp)
                    .control(CornerSm, onTerm = true, label = label, onClick = locate)
                    .padding(start = 6.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            ) {
                Text(label, style = Tstamp, color = p.termInk, maxLines = 1)
            }
        }
    }
}

/**
 * The speaker, after the time (app.css, .pk-volbtn): it mutes, or muted or
 * at none brings the sound back. 25 dp tall in the times' 13, as the
 * slider is, so that their line stays as tall as at work, where they are
 * hidden. Silent, muted or at none, it is lit, its × in the strip's ground
 * on the beak's orange: at 16 dp its three icons are too alike to tell
 * silence by, and a mute that went unseen kept the S25 silent (Peter,
 * 2026-10-27). The orange on the strip, and the × on it, 4.02:1 in light
 * and 7.59 in dark, past the 3:1 a control's state needs. Filled, where the
 * × alone in orange passed as well: a mute that is on deserves it (Peter,
 * 2026-10-27), the one area the beak fills (design/tokens.json).
 */
@Composable
private fun Speaker(sound: Sound, enabled: Boolean) {
    val p = pk
    val silent = sound.muted || sound.volume == 0.0
    val v = if (sound.muted) 0.0 else sound.volume
    Box(
        Modifier
            .bleed(vertical = 6.dp)
            .alpha(if (enabled) 1f else 0.45f)
            .control(
                CornerSm, if (silent) p.beak else Color.Transparent, onTerm = true, enabled = enabled,
                label = stringResource(if (silent) R.string.volume_unmute else R.string.volume_mute),
                onClick = sound.mute,
            )
            .padding(start = 6.dp, end = 6.dp, top = 4.dp, bottom = 5.dp),
    ) {
        Icon(
            when {
                silent -> Icons.volX
                v < 0.5 -> Icons.vol1
                else -> Icons.vol2
            },
            null, tint = if (silent) p.term else p.termInk, modifier = Modifier.size(16.dp),
        )
    }
}

/**
 * The volume's slider (app.css, .pk-vol-range): 64 by 25 dp, a 4 dp track
 * in the strip's line, filled to the level in the played waveform's
 * colour, its knob 12 dp in term-ink; in steps of 0.05, as the desktop's.
 */
@Composable
private fun Level(v: Double, enabled: Boolean, label: String, onChange: (Double, Boolean) -> Unit, modifier: Modifier) {
    val p = pk
    val change by rememberUpdatedState(onChange)
    val knob = with(LocalDensity.current) { 12.dp.toPx() }
    fun level(x: Float, width: Float) =
        (((x - knob / 2) / (width - knob)).coerceIn(0f, 1f) / 0.05f).roundToInt() * 0.05
    Canvas(
        modifier
            .size(64.dp, 25.dp)
            .semantics {
                contentDescription = label
                progressBarRangeInfo = ProgressBarRangeInfo(v.toFloat(), 0f..1f, 19)
                if (enabled) setProgress { change(it.toDouble(), true); true }
            }
            .then(
                if (enabled)
                    Modifier
                        .pointerInput(Unit) {
                            detectTapGestures { o -> change(level(o.x, size.width.toFloat()), true) }
                        }
                        .pointerInput(Unit) {
                            var last = 0.0
                            detectHorizontalDragGestures(onDragEnd = { change(last, true) }) { c, _ ->
                                last = level(c.position.x, size.width.toFloat())
                                change(last, false)
                            }
                        }
                else Modifier,
            ),
    ) {
        val track = 4.dp.toPx()
        val y = size.height / 2
        val x = knob / 2 + (size.width - knob) * v.toFloat()
        drawRoundRect(p.termLine, Offset(0f, y - track / 2), Size(size.width, track), CornerRadius(track / 2))
        drawRoundRect(p.wavePlayed, Offset(0f, y - track / 2), Size(x, track), CornerRadius(track / 2))
        drawCircle(p.termInk, knob / 2, Offset(x, y))
    }
}

/**
 * The progress, in play's place and size: a ring that fills with the
 * waveform's played colour from the top, and the percent inside it (app.css,
 * .pk-prog).
 */
@Composable
private fun Ring(done: Double, side: Dp) {
    val p = pk
    val d = done.toFloat().coerceIn(0f, 1f)
    Box(
        Modifier
            .size(side)
            .drawBehind {
                val r = Radius.radiusUi.toPx()
                val inset = 3.dp.toPx()
                val sweep = Brush.sweepGradient(
                    0f to p.wavePlayed, d to p.wavePlayed, d to p.termLine, 1f to p.termLine,
                    center = center,
                )
                // a sweep starts at three o'clock, CSS's conic at twelve
                rotate(-90f) { drawRoundRect(sweep, cornerRadius = CornerRadius(r)) }
                drawRoundRect(
                    p.term, Offset(inset, inset), Size(size.width - 2 * inset, size.height - 2 * inset),
                    CornerRadius(r - inset),
                )
            },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            "${(d * 100).toInt()}%",
            style = TextStyle(
                fontFamily = Fonts.sans, fontWeight = FontWeight.W700, fontSize = 11.sp, lineHeight = 1.em,
                lineHeightStyle = CssLine, letterSpacing = (-0.02).em, fontFeatureSettings = "tnum",
            ),
            color = p.termInk,
        )
    }
}

/**
 * The waveform the strip's width (ui/app.js, drawWave): bars 2 dp wide a dp
 * apart, each the mean of the outline's slices under it, between the
 * outline's own 10th percentile and its 99.5th, which even narration
 * leaves room between; the bars before the playhead in the played colour,
 * the playhead the beak's, a dp wider each side and alone reaching the top.
 * No outline yet: the empty strip's bars, 3 dp each.
 */
@Composable
fun Waveform(peaks: ByteArray?, at: Double, modifier: Modifier = Modifier) {
    val p = pk
    val range = remember(peaks) {
        if (peaks == null || peaks.isEmpty()) 0.0 to 255.0
        else {
            val s = peaks.map { it.toInt() and 0xff }.sorted()
            val lo = s[(s.size * 0.1).toInt()].toDouble()
            val hi = s[minOf(s.size - 1, (s.size * 0.995).toInt())].toDouble()
            lo to maxOf(hi, lo + 16)
        }
    }
    Canvas(modifier.fillMaxWidth().height(64.dp)) {
        val bar = 2.dp.toPx()
        val gap = 1.dp.toPx()
        val n = maxOf(1, floor((size.width + gap) / (bar + gap)).toInt())
        val top = size.height
        val k = (at * n).toInt().coerceIn(0, n - 1)
        val (lo, hi) = range
        for (i in 0 until n) {
            var v = 0.0
            if (peaks != null && peaks.isNotEmpty()) {
                val a = i * peaks.size / n
                val b = maxOf(a + 1, (i + 1) * peaks.size / n)
                var sum = 0.0
                for (j in a until b) sum += peaks[j].toInt() and 0xff
                v = ((sum / (b - a) - lo) / (hi - lo)).coerceIn(0.0, 1.0)
            }
            val x = i * (bar + gap)
            if (i == k) {
                drawRect(p.beak, Offset(x - gap, 0f), Size(bar + 2 * gap, top))
                continue
            }
            val h = 3.dp.toPx() + (v * (top - 7.dp.toPx())).roundToInt()
            val c = when {
                peaks == null -> p.termLine
                i < k -> p.wavePlayed
                else -> p.wave
            }
            drawRect(c, Offset(x, (top - h) / 2), Size(bar, h))
        }
    }
}
