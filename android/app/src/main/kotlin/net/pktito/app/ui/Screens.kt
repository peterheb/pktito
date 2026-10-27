package net.pktito.app.ui

import android.text.format.DateFormat
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.text.selection.SelectionState
import androidx.compose.foundation.text.selection.rememberSelectionState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
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
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.buildAnnotatedString
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
 * the middle. A screen too short for the list under the zone, which with
 * the wordmark and the list's heading takes 330 dp of it, scrolls as a
 * whole, the list with it: the desktop's window is never that short, but
 * a phone on its side is (393 dp on the emulator's Pixel 5), and a split
 * screen.
 */
@Composable
fun Ready(recent: List<Recents.Item>, a: ReadyActions, model: @Composable () -> Unit) {
    val p = pk
    var asking by remember { mutableStateOf(false) }
    val listed = recent.isNotEmpty()
    BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = if (listed) Alignment.TopCenter else Alignment.Center) {
        // room for three rows under the zone's 330 dp, or the whole scrolls
        val short = maxHeight < 480.dp
        Column(
            Modifier
                .widthIn(max = 640.dp)
                .fillMaxWidth()
                .then(
                    when {
                        short -> Modifier.verticalScroll(rememberScrollState())
                        listed -> Modifier.fillMaxHeight()
                        else -> Modifier
                    },
                )
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
            if (listed) Recent(recent, a, onClear = { asking = true }, scrolls = !short, if (short) Modifier else Modifier.weight(1f))
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
 * under its name, which keeps the width. scrolls: the list scrolls under
 * its heading, else it is all there, for a screen that scrolls (Ready).
 */
@Composable
private fun Recent(items: List<Recents.Item>, a: ReadyActions, onClear: () -> Unit, scrolls: Boolean, modifier: Modifier) {
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
        BoxWithConstraints(if (scrolls) Modifier.weight(1f) else Modifier) {
            val oneLine = maxWidth >= 440.dp
            val row = @Composable { i: Int, r: Recents.Item ->
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
            if (scrolls)
                LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    itemsIndexed(items, key = { _, r -> r.id }) { i, r -> row(i, r) }
                }
            else
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    items.forEachIndexed { i, r -> key(r.id) { row(i, r) } }
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
    /** Locate…, where the recording that does not play can be found again */
    locate: (() -> Unit)?,
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
    find: Finder,
    hits: List<Hit>,
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
                ticks = if (review && duration > 0) hits.map { it.time / duration } else emptyList(),
                tickAt = find.at,
                // a tick: its match, and the playhead there (app.js, #ticks)
                onTick = { k ->
                    a.seek(hits[k].time, false)
                    // after the seek, which puts the reader on the match it
                    // last passed: a tick's own, where two share its time
                    find.go(hits, k)
                },
                modifier = Modifier.padding(horizontal = 16.dp),
                why = why,
                locate = locate,
            )
        if (review && rows.isNotEmpty())
            // a step while it plays takes the playhead to its match (Finder.step)
            Views(
                captions, a.setCaptions, find, hits, if (playing) ({ t: Double -> a.seek(t, false) }) else null,
                Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp),
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
        Rows(rows, review, captions, playing, position, stats, base, hits, find.at, list, a, Modifier.weight(1f))
    }
}

/**
 * The views line: Sentences or Captions, and the search's field at its end
 * (app.css, .pk-views and .pk-find), the desktop's 200 dp and 280 with a
 * search in it, where the line has room for both, 480 dp; on a phone the
 * glass in the field's place, which opens the field over the line until
 * the search is let go, as a phone's find in page does.
 */
@Composable
private fun Views(captions: Boolean, setCaptions: (Boolean) -> Unit, find: Finder, hits: List<Hit>, seek: ((Double) -> Unit)?, modifier: Modifier) {
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val room = maxWidth >= 480.dp
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            if (room || !find.open) {
                Switch2(stringResource(R.string.sentences), stringResource(R.string.captions), captions, setCaptions)
                Spacer(Modifier.weight(1f))
            }
            when {
                room -> FindField(find, hits, seek, find::clear, Modifier.width(if (find.query.isNotBlank()) 280.dp else 200.dp))
                find.open -> FindField(find, hits, seek, find::clear, Modifier.weight(1f))
                else -> IconBtn(Icons.search, stringResource(R.string.find_label), find::focus, kind = Kind.Ghost)
            }
        }
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
 * 619px), where its 112 dp column left too little of the line, a cue's two
 * times a range. The row playing on the surface, the beak's square after
 * its time and the word playing underlined in it; a row, tapped anywhere,
 * plays from its time.
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
    hits: List<Hit>,
    at: Int,
    list: LazyListState,
    a: WorkActions,
    modifier: Modifier,
) {
    val p = pk
    val current = if (review && (playing || position > 0)) rows.indexOfLast { it.start <= position + 1e-3 } else -1
    // each row's matches, by their place among all of them
    val marks = remember(hits) { hits.withIndex().groupBy { it.value.row } }
    // the text selected, if any (SegRow)
    val selection = rememberSelectionState()
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val wide = maxWidth >= 620.dp
        SelectionContainer(selection) {
            LazyColumn(state = list, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 16.dp)) {
                itemsIndexed(rows) { i, s ->
                    val now = i == current
                    val word = if (now) s.words.indexOfLast { it.start <= position + 1e-3 } else -1
                    SegRow(s, now, word, marks[i], at, wide, captions, review, base, selection) { a.seek(s.start, true) }
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
private fun SegRow(
    s: Seg, now: Boolean, word: Int, marks: List<IndexedValue<Hit>>?, at: Int, wide: Boolean, cue: Boolean, review: Boolean,
    base: Timebase, selection: SelectionState, onTime: () -> Unit,
) {
    val p = pk
    val stamp = Type.stamp.copy(fontFeatureSettings = "tnum")
    val start = @Composable { mod: Modifier ->
        Text(base.at(s.start), style = stamp.copy(lineHeight = 20.sp), color = p.inkMuted, modifier = mod)
    }
    val end = @Composable { mod: Modifier -> Text(base.at(s.end), style = stamp, color = p.inkMuted, modifier = mod) }
    // the row playing: the beak's square after its time, on its baseline
    val beak = @Composable { scope: RowScope ->
        if (now)
            with(scope) {
                Spacer(Modifier.width(3.dp))
                Box(Modifier.size(7.dp).alignBy { it.measuredHeight }.background(p.beak))
            }
    }
    // a cue's end under its start, or on a narrow screen after it as a
    // range, 00:02.88 – 00:05.60, the beak after the range: side by side, a
    // start with the beak after it and the end beside that read as two
    // times of their own (Peter, the S25, 2026-10-26)
    val time = @Composable { mod: Modifier ->
        if (cue && !wide)
            Row(mod) {
                start(Modifier.alignByBaseline())
                Text(" – ", style = stamp, color = p.inkMuted, modifier = Modifier.alignByBaseline())
                end(Modifier.alignByBaseline())
                beak(this)
            }
        else
            Column(mod, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Row {
                    start(Modifier.alignByBaseline())
                    beak(this)
                }
                if (cue) end(Modifier)
            }
    }
    // the whole row plays from its time, its words as well as the time, which
    // a thumb is too broad to find (Peter, the S25, 2026-10-26); but not a
    // tap that lets go of a selection of the text. The selection is gone by
    // the time the tap is, so whether there was one is taken as the finger
    // comes down, before the text's own gestures see it.
    val selecting = remember { booleanArrayOf(false) }
    val box = Modifier
        .widthIn(max = 716.dp)
        .fillMaxWidth()
        .background(if (now) p.surface else Color.Transparent, Corner)
        .then(
            if (review)
                Modifier
                    .pointerInput(selection) {
                        awaitEachGesture {
                            awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                            selecting[0] = selection.selectedTexts.isNotEmpty()
                        }
                    }
                    .clickable(remember { MutableInteractionSource() }, null, onClickLabel = stringResource(R.string.play_from, base.at(s.start))) {
                        if (!selecting[0]) onTime()
                    }
            else Modifier
        )
    if (wide)
        Row(box.padding(horizontal = 16.dp, vertical = 8.dp)) {
            time(Modifier.width(112.dp).alignByBaseline())
            Spacer(Modifier.width(12.dp))
            Sentence(s, word, marks, at, Modifier.weight(1f).alignByBaseline())
        }
    else
        Column(box.padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            time(Modifier)
            Sentence(s, word, marks, at, Modifier)
        }
}

/**
 * A row's text in the transcript's type, the word playing underlined in the
 * beak's colour, 2 dp, 4 below the line; and a search's matches marked
 * (app.css, .pk-hit): each on plume mixed 45% into the ground, as tall as
 * its letters and 2 dp round them, its corners 3 dp; the one the reader is
 * on in plume, its letters on-plume, an ink bar 2 dp under it, for a state
 * shown by more than its colour (the tokens' contrast, 2026-10-22).
 */
@Composable
private fun Sentence(s: Seg, word: Int, marks: List<IndexedValue<Hit>>?, at: Int, modifier: Modifier) {
    val p = pk
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val span = remember(s, word) { if (word < 0) null else wordSpan(s, word) }
    val on = marks?.firstOrNull { it.index == at }?.value
    val text = remember(s, on, p) {
        buildAnnotatedString {
            append(s.text)
            if (on != null && on.end <= s.text.length) addStyle(SpanStyle(color = p.onPlume), on.start, on.end)
        }
    }
    val hl = mix(p.ground, p.plume, 0.45f)
    Text(
        text,
        style = Type.transcript,
        color = p.ink,
        onTextLayout = { layout = it },
        modifier = modifier.drawBehind {
            val l = layout ?: return@drawBehind
            for (m in marks.orEmpty()) {
                val current = m.index == at
                mark(l, m.value.start, m.value.end, if (current) p.plume else hl, if (current) p.ink else null)
            }
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

/**
 * A match's mark behind the letters from a to b, on each line they are on:
 * their height, from Archivo's ascent to its descent (0.878 and 0.21 em,
 * its hhea), and 2 dp round them, as an inline mark's box is drawn on the
 * desktop; under, an ink bar 2 dp under it, the width of the letters.
 */
private fun DrawScope.mark(l: TextLayoutResult, a: Int, b: Int, fill: Color, under: Color?) {
    if (b <= a || b > l.layoutInput.text.length) return
    val em = l.layoutInput.style.fontSize.toPx()
    val round = 2.dp.toPx()
    for (line in l.getLineForOffset(a)..l.getLineForOffset(b - 1)) {
        val s0 = maxOf(a, l.getLineStart(line))
        val s1 = minOf(b, l.getLineEnd(line, visibleEnd = true))
        if (s1 <= s0) continue
        val x0 = l.getHorizontalPosition(s0, true)
        val x1 = if (l.getLineForOffset(s1) == line) l.getHorizontalPosition(s1, true) else l.getLineRight(line)
        val base = l.getLineBaseline(line)
        val top = base - 0.878f * em - round
        val bottom = base + 0.21f * em + round
        drawRoundRect(fill, Offset(x0 - round, top), Size(x1 - x0 + 2 * round, bottom - top), CornerRadius(3.dp.toPx()))
        if (under != null) drawRect(under, Offset(x0, bottom), Size(x1 - x0, 2.dp.toPx()))
    }
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
