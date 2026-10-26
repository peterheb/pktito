package net.pktito.app.ui

import android.text.format.DateFormat
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import net.pktito.app.R
import net.pktito.app.Recents
import net.pktito.app.Seg

/** What the start screen does with what it is asked: choose a file, About, and the recent list's open, forget and clear, by a transcript's id. */
class ReadyActions(
    val choose: () -> Unit,
    val about: () -> Unit,
    val open: (id: String) -> Unit,
    val forget: (id: String) -> Unit,
    val clear: () -> Unit,
)

/**
 * The start screen (ui/index.html, #ready): the wordmark and the model in
 * use, its menu (ModelButton), on the column's top edge, and under them the
 * prompt and its button in a dashed zone, as the desktop's drop zone,
 * which says where a file goes. A phone has nothing to drop, so the zone
 * says what it is for. The wordmark opens About. With transcripts on the
 * recent list, the column is the screen's height and the list takes what
 * is left under the zone (app.css, .has-recent); without, the column is in
 * the middle.
 */
@Composable
fun Ready(recent: List<Recents.Item>, a: ReadyActions, model: @Composable () -> Unit) {
    val p = pk
    var asking by remember { mutableStateOf(false) }
    val listed = recent.isNotEmpty()
    Box(Modifier.fillMaxSize(), contentAlignment = if (listed) Alignment.TopCenter else Alignment.Center) {
        Column(
            Modifier
                .widthIn(max = 640.dp)
                .fillMaxWidth()
                .then(if (listed) Modifier.fillMaxHeight() else Modifier)
                .padding(horizontal = 20.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(22.dp),
        ) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                WordmarkButton(a.about)
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
                PkButton(stringResource(R.string.choose), a.choose, kind = Kind.Primary, large = true)
            }
            if (listed) Recent(recent, a, onClear = { asking = true }, Modifier.weight(1f))
        }
    }
    if (asking)
        AskClear(
            onClear = {
                asking = false
                a.clear()
            },
            onCancel = { asking = false },
        )
}

/**
 * The recent list (app.css, .pk-recent): its heading and Clear, which asks
 * first, then each transcript, newest first, opened by a tap; taken off the
 * list by a long press's menu, by the × over its date under a mouse, or by
 * Delete from the keyboard, which goes on to the row that takes its place
 * (app.js, renderReady and forget). Each row is a line where the list is
 * 440 dp wide, the desktop's narrowest window's (480 less its margins),
 * which leaves a name 250; on a phone, narrower, its length and date go
 * under its name, which keeps the width.
 */
@Composable
private fun Recent(items: List<Recents.Item>, a: ReadyActions, onClear: () -> Unit, modifier: Modifier) {
    val p = pk
    val rows = remember { mutableMapOf<String, FocusRequester>() }
    val now by rememberUpdatedState(items)
    var refocus by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(refocus) {
        val i = refocus ?: return@LaunchedEffect
        // a frame, for the list to lay out the row that took its place
        withFrameNanos { }
        now.getOrNull(minOf(i, now.size - 1))?.let { runCatching { rows[it.id]?.requestFocus() } }
        refocus = null
    }
    Column(modifier.padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(
            Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(stringResource(R.string.recent_title), style = Type.ui, color = p.ink, modifier = Modifier.weight(1f))
            // the list's one action, a ghost button as small as the heading,
            // its padding outside it so that its word ends over the dates,
            // in their grey (app.css, #clear-recent)
            Text(
                stringResource(R.string.recent_clear),
                style = Type.ui.copy(fontSize = 13.sp),
                color = p.inkMuted,
                modifier = Modifier
                    .bleed(8.dp, 11.dp)
                    .control(Corner, label = stringResource(R.string.recent_clear_label), onClick = onClear)
                    .padding(horizontal = 8.dp, vertical = 11.dp),
            )
        }
        BoxWithConstraints(Modifier.weight(1f)) {
            val oneLine = maxWidth >= 440.dp
            LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                itemsIndexed(items, key = { _, r -> r.id }) { i, r ->
                    val focus = remember { FocusRequester() }
                    DisposableEffect(r.id) {
                        rows[r.id] = focus
                        onDispose { rows.remove(r.id) }
                    }
                    RecentRow(
                        r, oneLine, focus,
                        onOpen = { a.open(r.id) },
                        onForget = { keys ->
                            if (keys) refocus = i
                            a.forget(r.id)
                        },
                    )
                }
            }
        }
    }
}

/** A recent row's name (app.css, .pk-row .pk-name). */
private val rowName = Type.ui.copy(lineHeight = 1.3.em)

/** onForget: true from the keyboard, which then goes on to the next row. */
@Composable
private fun RecentRow(r: Recents.Item, oneLine: Boolean, focus: FocusRequester, onOpen: () -> Unit, onForget: (keys: Boolean) -> Unit) {
    val p = pk
    var menu by remember { mutableStateOf(false) }
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    val remove = stringResource(R.string.recent_remove, r.name)
    val length = hms(r.duration, r.duration)
    val date = whenDone(r.finished)
    val stamp = Type.stamp.copy(fontFeatureSettings = "tnum")
    // the row's hover is its box's, which holds the × too: on the × the row
    // would lose it, and the × with it
    Box(Modifier.fillMaxWidth().hoverable(hover)) {
        Row(
            Modifier
                .focusRequester(focus)
                .control(Corner, onLongClick = { menu = true }, onClick = onOpen)
                .onKeyEvent { e ->
                    val del = e.type == KeyEventType.KeyDown && (e.key == Key.Delete || e.key == Key.Backspace)
                    if (del) onForget(true)
                    del
                }
                .semantics { customActions = listOf(CustomAccessibilityAction(remove) { onForget(false); true }) }
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(Icons.file, null, tint = p.ink, modifier = Modifier.size(16.dp))
            if (oneLine) {
                Text(r.name, style = rowName, color = p.ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                Text(length, style = stamp, color = p.inkMuted, maxLines = 1)
                Text(
                    date, style = Type.small, color = p.inkMuted, maxLines = 1, textAlign = TextAlign.End,
                    // the desktop's 72, and as much more as a date of another year needs
                    modifier = Modifier.widthIn(min = 72.dp).alpha(if (hovered) 0f else 1f),
                )
            } else
                Column(Modifier.weight(1f)) {
                    Text(r.name, style = rowName, color = p.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Row {
                        Text(length, style = stamp, color = p.inkMuted, maxLines = 1, modifier = Modifier.alignByBaseline())
                        Text(" · $date", style = Type.small, color = p.inkMuted, maxLines = 1, modifier = Modifier.alignByBaseline())
                    }
                }
        }
        // its × over its date under a mouse, as the desktop's (app.css,
        // .pk-x), out of the keys' way: Delete is theirs
        if (hovered)
            Box(
                Modifier
                    .align(Alignment.CenterEnd)
                    .padding(end = 6.dp)
                    .size(32.dp)
                    .focusProperties { canFocus = false }
                    .control(CornerSm, label = remove, onClick = { onForget(false) }),
                contentAlignment = Alignment.Center,
            ) { Icon(Icons.x, null, tint = p.inkMuted, modifier = Modifier.size(16.dp)) }
        Menu(menu, { menu = false }) {
            Item(stringResource(R.string.recent_remove_item), null, note = AnnotatedString(stringResource(R.string.recent_remove_note))) {
                menu = false
                onForget(false)
            }
        }
    }
}

/** When a transcript finished, as the desktop's list says it: Today, Yesterday, else the date, its year if not this one's (app.js, when). */
@Composable
private fun whenDone(secs: Long): String {
    val today = LocalDate.now()
    val day = Instant.ofEpochSecond(secs).atZone(ZoneId.systemDefault()).toLocalDate()
    return when (ChronoUnit.DAYS.between(day, today)) {
        0L -> stringResource(R.string.date_today)
        1L -> stringResource(R.string.date_yesterday)
        else -> {
            val locale = LocalConfiguration.current.locales[0]
            val skeleton = if (day.year == today.year) "MMMd" else "yMMMd"
            DateTimeFormatter.ofPattern(DateFormat.getBestDateTimePattern(locale, skeleton), locale).format(day)
        }
    }
}

/**
 * Clearing the list asks first, as the desktop's window does (app.js,
 * askClear): what it does, then Clear, in the error's red, and Cancel, the
 * one the question is for. Back, Esc and a tap beside it are Cancel; with
 * keys, Cancel has the focus, so that Enter does not clear the list.
 */
@Composable
private fun AskClear(onClear: () -> Unit, onCancel: () -> Unit) {
    val p = pk
    val cancel = remember { FocusRequester() }
    val keys = hasKeys()
    Modal(onCancel) {
        Column(
            Modifier
                .widthIn(max = 400.dp)
                .fillMaxWidth()
                .floating()
                .padding(24.dp),
        ) {
            Text(stringResource(R.string.ask_clear_title), style = Type.ui, color = p.ink)
            Text(stringResource(R.string.ask_clear_text), style = Type.ui.copy(fontWeight = FontWeight.W400), color = p.ink, modifier = Modifier.padding(top = 8.dp))
            Row(
                Modifier.fillMaxWidth().padding(top = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            ) {
                PkButton(stringResource(R.string.ask_clear), onClear, Modifier.widthIn(min = 80.dp), kind = Kind.Del)
                PkButton(stringResource(R.string.ask_cancel), onCancel, Modifier.widthIn(min = 80.dp).focusRequester(cancel), kind = Kind.Primary)
            }
        }
    }
    if (keys)
        LaunchedEffect(Unit) {
            withFrameNanos { }
            runCatching { cancel.requestFocus() }
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
    why: String?,
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
            // why the audio does not play, or not yet: a transcript of the list
            // reading its original again
            review && why != null -> why
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
