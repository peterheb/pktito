package net.pktito.app.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import kotlin.math.floor
import kotlin.math.roundToInt

/** The strip's text, all of it term-ink: which label leads is said by where it is (app.css, .pk-tstamp). */
private val Tstamp = Type.stamp.copy(lineHeight = 1.em, fontFeatureSettings = "tnum")

/**
 * The file's strip, on term in both themes (app.css, .pk-strip): play, or
 * at work the progress in play's place and size, so that the waveform
 * keeps its width when the transcript finishes; the waveform the rest of
 * the width; under its two ends, the time and the length. At work the
 * waveform fills to the playhead as the transcript does, and the time's
 * place says the step or the pace.
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
    modifier: Modifier = Modifier,
) {
    val p = pk
    Row(
        modifier
            .fillMaxWidth()
            .background(p.term, Corner)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Box(Modifier.padding(top = 12.dp)) {
            if (review)
                Box(
                    Modifier
                        .alpha(if (playable) 1f else 0.45f)
                        .control(Corner, p.termLine, onTerm = true, enabled = playable, label = playLabel, onClick = onPlay)
                        .size(40.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(if (playing) Icons.pause else Icons.play, null, tint = p.termInk, modifier = Modifier.size(20.dp))
                }
            else
                Ring(done)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            val at = if (review) (if (duration > 0) position / duration else 0.0) else done
            Waveform(
                peaks, at,
                Modifier
                    .alpha(if (review && !playable) 0.45f else 1f)
                    .then(
                        if (review && playable)
                            Modifier.pointerInput(Unit) {
                                detectTapGestures { o -> onSeek((o.x / size.width).toDouble().coerceIn(0.0, 1.0)) }
                            }
                        else Modifier,
                    ),
            )
            Spacer(Modifier.height(6.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(left, style = Tstamp, color = p.termInk, maxLines = 1)
                Spacer(Modifier.weight(1f))
                Text(right, style = Tstamp, color = p.termInk, maxLines = 1)
            }
        }
    }
}

/**
 * The progress, in play's place and size: a ring that fills with the
 * waveform's played colour from the top, and the percent inside it (app.css,
 * .pk-prog).
 */
@Composable
private fun Ring(done: Double) {
    val p = pk
    val d = done.toFloat().coerceIn(0f, 1f)
    Box(
        Modifier
            .size(40.dp)
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
                letterSpacing = (-0.02).em, fontFeatureSettings = "tnum",
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
