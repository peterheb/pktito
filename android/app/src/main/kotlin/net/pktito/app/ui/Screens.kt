package net.pktito.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import net.pktito.app.R
import net.pktito.app.Seg

/**
 * The start screen (ui/index.html, #ready): the wordmark and the model in
 * use, its menu (ModelButton), on the column's top edge, and under them the
 * prompt and its button in a dashed zone, as the desktop's drop zone,
 * which says where a file goes. A phone has nothing to drop, so the zone
 * says what it is for. The wordmark opens About.
 */
@Composable
fun Ready(onChoose: () -> Unit, onAbout: () -> Unit, model: @Composable () -> Unit) {
    val p = pk
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(
            Modifier
                .widthIn(max = 640.dp)
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(22.dp),
        ) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                WordmarkButton(onAbout)
                Spacer(Modifier.weight(1f))
                model()
            }
            // its dashes 6.2:1 on the surface inside, 6.8 on the ground
            // outside, past the 3:1 a boundary needs (app.css, .pk-ready-drop)
            Column(
                Modifier
                    .fillMaxWidth()
                    .background(p.surface, Corner)
                    .drawBehind {
                        val w = 2.dp.toPx()
                        drawRoundRect(
                            p.inkMuted, Offset(w / 2, w / 2), size.copy(size.width - w, size.height - w),
                            CornerRadius(Radius.radiusUi.toPx()),
                            style = Stroke(w, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))),
                        )
                    }
                    .padding(horizontal = 24.dp, vertical = 36.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Text(
                    stringResource(R.string.ready_lead),
                    style = Type.lead.copy(fontSize = 22.sp),
                    color = p.ink,
                    textAlign = TextAlign.Center,
                )
                PkButton(stringResource(R.string.choose), onChoose, kind = Kind.Primary, large = true)
            }
        }
    }
}

/** The wordmark as each screen's way to About, its padding outside it so that the word stays where it is (app.css, button.pk-wm). */
@Composable
fun WordmarkButton(onAbout: () -> Unit) {
    Box(
        Modifier
            .bleed(6.dp, 4.dp)
            .control(Corner, label = stringResource(R.string.about_open), onClick = onAbout)
            .padding(horizontal = 6.dp, vertical = 4.dp),
    ) { Wordmark() }
}

/** What the work screen does with what it is asked. */
class WorkActions(
    val back: () -> Unit,
    val stop: () -> Unit,
    val copy: () -> Unit,
    val share: () -> Unit,
    val save: (ext: String) -> Unit,
    val setTimes: (Boolean) -> Unit,
    val setCaptions: (Boolean) -> Unit,
    val play: () -> Unit,
    val seek: (seconds: Double, play: Boolean) -> Unit,
    val setBase: (Timebase) -> Unit,
)

/** The formats Save as offers, the desktop's, in its order, and the letter that saves one with Ctrl (and Shift). */
class Format(val ext: String, val label: Int, val withTimes: Int = label, val key: String? = null, val shift: Boolean = false)

val FORMATS = listOf(
    Format("txt", R.string.format_txt, R.string.format_txt_times, key = "S"),
    Format("srt", R.string.format_srt, key = "S", shift = true),
    Format("vtt", R.string.format_vtt),
    Format("md", R.string.format_md, R.string.format_md_times),
    Format("json", R.string.format_json),
)

/**
 * One file at work and then its transcript (ui/index.html, #work): the
 * header with the way back, the name, and what can be done with it (at
 * work only Stop; Copy, Share and Save as once it is done), the strip, the
 * choice of sentences or captions, a line for what went wrong, and the rows.
 */
@Composable
fun Work(
    name: String,
    step: String,
    done: Double,
    duration: Double,
    peaks: ByteArray?,
    rows: List<Seg>,
    error: String?,
    stats: String?,
    pace: String?,
    captions: Boolean,
    times: Boolean,
    playable: Boolean,
    playing: Boolean,
    position: Double,
    list: LazyListState,
    sound: Sound,
    chosen: Timebase,
    a: WorkActions,
) {
    val p = pk
    // the file's times written with its length's hours
    val base = chosen.copy(duration = duration)
    val running = step == "loading" || step == "reading" || step == "transcribing"
    val review = step == "done" || step == "stopped"
    Column(Modifier.fillMaxSize()) {
        // the header's buttons in the middle of its 62 dp (app.css, .pk-head),
        // the way back's 8 dp into its padding, so that its arrow is drawn
        // near the strip's edge and its button's padding is outside it
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 62.dp)
                .padding(start = if (running) 16.dp else 8.dp, end = 16.dp, top = 11.dp, bottom = 11.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (!running)
                IconBtn(Icons.back, stringResource(R.string.back), a.back, kind = Kind.Ghost)
            Row(
                Modifier.weight(1f),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Text(name, style = Type.ui, color = p.ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                if (running) Spinner()
            }
            if (running)
                PkButton(stringResource(R.string.stop), a.stop, kind = Kind.Ghost)
            if (review && rows.isNotEmpty()) {
                IconBtn(Icons.copy, stringResource(if (captions) R.string.copy_captions else R.string.copy_text), a.copy)
                IconBtn(Icons.share, stringResource(R.string.share), a.share)
                SaveAs(name, times, a)
            }
        }
        if (step != "error")
            Strip(
                peaks = peaks,
                review = review,
                done = if (step == "transcribing") done else 0.0,
                playing = playing,
                playable = playable,
                position = position,
                duration = duration,
                left = when {
                    review -> base.at(position)
                    step == "loading" -> stringResource(R.string.status_loading)
                    step == "reading" -> stringResource(R.string.status_reading)
                    else -> pace ?: ""
                },
                right = if (duration > 0) base.length() else "",
                playLabel = stringResource(if (playing) R.string.pause else R.string.play),
                onPlay = a.play,
                onSeek = { a.seek(it * duration, false) },
                sound = sound,
                base = base,
                onBase = a.setBase,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        if (review && rows.isNotEmpty())
            Switch2(
                stringResource(R.string.sentences), stringResource(R.string.captions), captions, a.setCaptions,
                Modifier.padding(start = 16.dp, top = 16.dp),
            )
        val msg = when {
            error != null -> error
            step == "stopped" -> stringResource(R.string.status_stopped_at, (done * 100).toInt())
            step == "done" && rows.isEmpty() -> stringResource(R.string.no_speech)
            else -> null
        }
        if (msg != null)
            Row(
                Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                val bad = error != null
                if (bad) Icon(Icons.alert, null, tint = p.err, modifier = Modifier.size(16.dp))
                Text(msg, style = Type.body.copy(fontSize = 15.sp), color = if (bad) p.err else p.inkMuted)
            }
        Rows(rows, review, captions, playing, position, stats, base, list, a, Modifier.weight(1f))
    }
}

/** Save as, the plume button a finished transcript is for, and its menu of formats. */
@Composable
private fun SaveAs(name: String, times: Boolean, a: WorkActions) {
    var open by remember { mutableStateOf(false) }
    val stem = name.substringBeforeLast('.').ifEmpty { name }
    val keys = hasKeys()
    Box {
        PkButton(stringResource(R.string.save_as), { open = true }, kind = Kind.Primary, trailing = Icons.down)
        Menu(open, { open = false }) {
            for (f in FORMATS) {
                val shortcut = f.key?.takeIf { keys }?.let { kbd(it, f.shift) }
                Item(
                    stringResource(if (times) f.withTimes else f.label), "$stem.${f.ext}",
                    hint = shortcut?.let { { Kbd(it) } },
                ) {
                    open = false
                    a.save(f.ext)
                }
            }
            MenuLine()
            Check(stringResource(R.string.save_times), times, a.setTimes)
        }
    }
}

/** A shortcut's keys as the desktop writes them off macOS: Ctrl+S, Ctrl+Shift+S (app.js, kbd). */
@Composable
fun kbd(key: String, shift: Boolean = false): String =
    stringResource(R.string.key_ctrl) + "+" + (if (shift) stringResource(R.string.key_shift) + "+" else "") + key

/**
 * The transcript's rows (app.css, .pk-seg): each sentence's time, or a cue's
 * start and end, then its text, at most 560 dp wide; under 620 dp the time
 * above the text, as the desktop's window has it (app.css, max-width:
 * 619px), where its 112 dp column left too little of the line. The row
 * playing on the surface, the beak's square after its
 * time and the word playing underlined in it; a time plays from there.
 */
@Composable
private fun Rows(
    rows: List<Seg>,
    review: Boolean,
    captions: Boolean,
    playing: Boolean,
    position: Double,
    stats: String?,
    base: Timebase,
    list: LazyListState,
    a: WorkActions,
    modifier: Modifier,
) {
    val p = pk
    val current = if (review && (playing || position > 0)) rows.indexOfLast { it.start <= position + 1e-3 } else -1
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val wide = maxWidth >= 620.dp
        SelectionContainer {
            LazyColumn(state = list, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 16.dp)) {
                itemsIndexed(rows) { i, s ->
                    val now = i == current
                    val word = if (now && !captions) s.words.indexOfLast { it.start <= position + 1e-3 } else -1
                    SegRow(s, now, word, wide, captions, review, base) { a.seek(s.start, true) }
                }
                if (stats != null)
                    item {
                        Text(
                            stats,
                            style = Type.small.copy(fontSize = 12.sp, fontFeatureSettings = "tnum"),
                            color = p.inkMuted,
                            modifier = Modifier.padding(start = if (wide) 140.dp else 16.dp, top = 16.dp),
                        )
                    }
            }
        }
    }
}

@Composable
private fun SegRow(s: Seg, now: Boolean, word: Int, wide: Boolean, cue: Boolean, review: Boolean, base: Timebase, onTime: () -> Unit) {
    val p = pk
    val start = @Composable { mod: Modifier ->
        Row(mod) {
            Text(
                base.at(s.start),
                style = Type.stamp.copy(lineHeight = 20.sp, fontFeatureSettings = "tnum"),
                color = p.inkMuted,
                modifier = Modifier
                    .alignByBaseline()
                    .then(if (review) Modifier.clickable(remember { MutableInteractionSource() }, null, onClick = onTime) else Modifier),
            )
            if (now) {
                Spacer(Modifier.width(3.dp))
                Box(Modifier.size(7.dp).alignBy { it.measuredHeight }.background(p.beak))
            }
        }
    }
    val end = @Composable { mod: Modifier ->
        Text(base.at(s.end), style = Type.stamp.copy(fontFeatureSettings = "tnum"), color = p.inkMuted, modifier = mod)
    }
    // a cue's end under its start, or on a narrow screen beside it
    val time = @Composable { mod: Modifier ->
        if (cue && !wide)
            Row(mod, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                start(Modifier.alignByBaseline())
                end(Modifier.alignByBaseline())
            }
        else
            Column(mod, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                start(Modifier)
                if (cue) end(Modifier)
            }
    }
    val box = Modifier
        .widthIn(max = 716.dp)
        .fillMaxWidth()
        .background(if (now) p.surface else Color.Transparent, Corner)
    if (wide)
        Row(box.padding(horizontal = 16.dp, vertical = 8.dp)) {
            time(Modifier.width(112.dp).alignByBaseline())
            Spacer(Modifier.width(12.dp))
            Sentence(s, word, Modifier.weight(1f).alignByBaseline())
        }
    else
        Column(box.padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            time(Modifier)
            Sentence(s, word, Modifier)
        }
}

/** A row's text in the transcript's type, the word playing underlined in the beak's colour, 2 dp, 4 below the line. */
@Composable
private fun Sentence(s: Seg, word: Int, modifier: Modifier) {
    val p = pk
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val span = remember(s, word) { if (word < 0) null else wordSpan(s, word) }
    Text(
        s.text,
        style = Type.transcript,
        color = p.ink,
        onTextLayout = { layout = it },
        modifier = modifier.drawBehind {
            val l = layout ?: return@drawBehind
            val (a, b) = span ?: return@drawBehind
            if (b <= a || b > l.layoutInput.text.length) return@drawBehind
            val line = l.getLineForOffset(a)
            val y = l.getLineBaseline(line) + 4.dp.toPx()
            val x0 = l.getHorizontalPosition(a, true)
            val x1 = if (l.getLineForOffset(b - 1) == line) l.getHorizontalPosition(b, true) else l.getLineRight(line)
            drawLine(p.beak, Offset(x0, y), Offset(x1, y), 2.dp.toPx())
        },
    )
}

/** Where the word-th word of s is in its text: the words, in order, each found after the last. */
private fun wordSpan(s: Seg, word: Int): Pair<Int, Int>? {
    var at = 0
    for ((i, w) in s.words.withIndex()) {
        val k = s.text.indexOf(w.text.trim(), at)
        if (k < 0) return null
        if (i == word) return k to k + w.text.trim().length
        at = k + w.text.trim().length
    }
    return null
}
