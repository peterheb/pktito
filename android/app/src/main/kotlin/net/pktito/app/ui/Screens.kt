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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
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
import androidx.compose.ui.geometry.Rect
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
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.SubcomposeLayout
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
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
 * the middle. Where the top row and the zone, as tall as their words make
 * them, leave the list under 160 dp, the screen scrolls as a whole, the
 * list with it: the desktop's window is never that short, but a split
 * screen is, and a phone on its side at any text size. The emulator's
 * Pixel 5 on its side has 345 dp, which leave the list 57 at the default
 * text size and 29 at twice it; upright at twice it they leave 418
 * (2026-10-31).
 */
@Composable
fun Ready(recent: List<Recents.Item>, a: ReadyActions, model: @Composable () -> Unit) {
    var asking by rememberSaveable { mutableStateOf(false) }
    val listed = recent.isNotEmpty()
    // the top row's height and the zone's, as they were last laid out
    var top by remember { mutableIntStateOf(0) }
    var zone by remember { mutableIntStateOf(0) }
    BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = if (listed) Alignment.TopCenter else Alignment.Center) {
        // what they leave the list, less the column's 20 dp above and below
        // and its two gaps of 22
        val left = maxHeight - 84.dp - with(LocalDensity.current) { (top + zone).toDp() }
        val short = left < 160.dp
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
            TopRow(a, model, Modifier.onSizeChanged { top = it.height })
            Drop(a, Modifier.fillMaxWidth().onSizeChanged { zone = it.height })
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
 * The start screen's top row: the wordmark, which opens About, and the
 * model in use, its menu (ModelButton), under the wordmark where the two
 * do not fit (Beside).
 */
@Composable
private fun TopRow(a: ReadyActions, model: @Composable () -> Unit, modifier: Modifier = Modifier) {
    Beside(modifier.fillMaxWidth(), words = { WordmarkButton(a.about) }) { model() }
}

/**
 * The drop zone (app.css, .pk-ready-drop): the prompt and Choose in a dashed
 * zone, its dashes 6.2:1 on the surface inside, 6.8 on the ground outside,
 * past the 3:1 a boundary needs.
 */
@Composable
private fun Drop(a: ReadyActions, modifier: Modifier) {
    val p = pk
    Column(
        modifier
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
}

/** A transcript open beside the side, as its row there says it (LiveRow): how far it has got, by the desktop's steps. */
class Opened(val id: String, val name: String, val step: String, val done: Double) {
    val running: Boolean
        get() = step == "loading" || step == "reading" || step == "transcribing"
}

/**
 * A window with room for the work beside it (MainActivity, sided) keeps the
 * start screen's top row and recent list in a column at its left, the side,
 * as the desktop's does from 1160 px (app.css, the side): beside the drop
 * zone while the list has anything in it (open, null), and beside the
 * transcript open, whose row it marks, or heads the list with where the
 * list does not have it (LiveRow); Choose a file… stands in there for the
 * zone's. 300 dp, the desktop's 300 px. Another row there leaves the one
 * open as Back would, and opens (ReadyActions.open). While a transcription
 * runs, the rows and Choose wait for it, dimmed: Android transcribes one
 * file at a time, and a phone has no way back from one under way either.
 */
@Composable
fun Side(recent: List<Recents.Item>, a: ReadyActions, open: Opened?, model: @Composable () -> Unit, work: @Composable () -> Unit) {
    val p = pk
    var asking by rememberSaveable { mutableStateOf(false) }
    val running = open?.running == true
    val live = open?.takeIf { o -> recent.none { it.id == o.id } }
    Row(Modifier.fillMaxSize()) {
        Column(
            Modifier
                .width(300.dp)
                .fillMaxHeight()
                // its edge, the desktop's border-right
                .drawBehind { drawRect(p.sunk, Offset(size.width - 1.dp.toPx(), 0f), Size(1.dp.toPx(), size.height)) }
                .padding(start = 20.dp, top = 20.dp, end = 16.dp, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            TopRow(a, model)
            if (open != null) PkButton(stringResource(R.string.choose), a.choose, Modifier.fillMaxWidth(), enabled = !running)
            if (recent.isNotEmpty() || live != null)
                Recent(recent, a, onClear = { asking = true }, scrolls = true, Modifier.weight(1f), top = 4.dp, open = open?.id, live = live, enabled = !running)
        }
        Box(Modifier.weight(1f).fillMaxHeight()) {
            // the zone in the middle, 560 wide, 40 from either edge at the least
            if (open == null) Drop(a, Modifier.align(Alignment.Center).padding(horizontal = 40.dp).widthIn(max = 560.dp).fillMaxWidth())
            else work()
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
 * (app.js, renderReady and forget). Each row is a line where that leaves
 * its name 250 dp, as the desktop's narrowest window's list, 440 wide (480
 * less its margins), leaves it, grown with the text (withText),
 * beside the list's widest length and date; on a phone, narrower, its
 * length and date go under its name, which keeps the width. On an open
 * fold, 673 dp, a row keeps its line through 1.5 times the text size, the
 * name 399 dp to the 390 asked there, and at twice it would have 383 to
 * 464, so the two go under (2026-10-31). scrolls: the list scrolls under
 * its heading, else it is all there, for a screen that scrolls (Ready). In
 * the side, the row of the transcript open is marked (open), one the list
 * does not have heads it (live), and while one is under way the rows and
 * Clear wait (enabled); Clear goes with the last of the list's own.
 */
@Composable
private fun Recent(
    items: List<Recents.Item>, a: ReadyActions, onClear: () -> Unit, scrolls: Boolean, modifier: Modifier,
    top: Dp = 8.dp, open: String? = null, live: Opened? = null, enabled: Boolean = true,
) {
    val p = pk
    val rows = remember { mutableMapOf<String, FocusRequester>() }
    // the widest length and date of a row: the length's figures, of one
    // width, and the date in its 72 dp, or what a longer one needs (RecentRow)
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val dates = items.map { whenDone(it.finished) }
    val beside = remember(items, dates, measurer, density) {
        with(density) {
            items.indices.maxOfOrNull { i ->
                val length = measurer.measure(hms(items[i].duration, items[i].duration), figures).size.width
                length + maxOf(72.dp.roundToPx(), measurer.measure(dates[i], Type.small).size.width)
            }?.toDp() ?: 0.dp
        }
    }
    val least = 250.dp.withText()
    val now by rememberUpdatedState(items)
    var refocus by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(refocus) {
        val i = refocus ?: return@LaunchedEffect
        // a frame, for the list to lay out the row that took its place
        withFrameNanos { }
        now.getOrNull(minOf(i, now.size - 1))?.let { runCatching { rows[it.id]?.requestFocus() } }
        refocus = null
    }
    Column(modifier.padding(top = top), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(
            Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(stringResource(R.string.recent_title), style = Type.ui, color = p.ink, modifier = Modifier.weight(1f))
            // the list's one action, a ghost button as small as the heading,
            // its padding outside it so that its word ends over the dates,
            // in their grey (app.css, #clear-recent)
            if (items.isNotEmpty())
                Text(
                    stringResource(R.string.recent_clear),
                    style = Type.ui.copy(fontSize = 13.sp),
                    color = p.inkMuted,
                    modifier = Modifier
                        .bleed(8.dp, 11.dp)
                        .then(if (enabled) Modifier else Modifier.alpha(0.45f))
                        .control(Corner, enabled = enabled, label = stringResource(R.string.recent_clear_label), onClick = onClear)
                        .padding(horizontal = 8.dp, vertical = 11.dp),
                )
        }
        BoxWithConstraints(if (scrolls) Modifier.weight(1f) else Modifier) {
            // the row's 12 dp either side, its icon's 16 and three gaps of 10
            val oneLine = maxWidth - 70.dp - beside >= least
            val row = @Composable { i: Int, r: Recents.Item ->
                val focus = remember { FocusRequester() }
                DisposableEffect(r.id) {
                    rows[r.id] = focus
                    onDispose { rows.remove(r.id) }
                }
                RecentRow(
                    r, oneLine, focus, current = r.id == open, enabled = enabled,
                    onOpen = { a.open(r.id) },
                    onForget = { keys ->
                        if (keys) refocus = i
                        a.forget(r.id)
                    },
                )
            }
            if (scrolls)
                LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    if (live != null) item(key = "live") { LiveRow(live) }
                    itemsIndexed(items, key = { _, r -> r.id }) { i, r -> row(i, r) }
                }
            else
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    if (live != null) LiveRow(live)
                    items.forEachIndexed { i, r -> key(r.id) { row(i, r) } }
                }
        }
    }
}

/** A recent row's name (app.css, .pk-row .pk-name). */
private val rowName = Type.ui.copy(lineHeight = 1.3.em)

/** A time or a length, in the stamp's type with figures of one width. */
private val figures = Type.stamp.copy(fontFeatureSettings = "tnum")

/**
 * onForget: true from the keyboard, which then goes on to the next row. The
 * transcript open beside the side (current) is on the surface, as the
 * desktop's row with aria-current, and a tap there is the one already open.
 */
@Composable
private fun RecentRow(
    r: Recents.Item, oneLine: Boolean, focus: FocusRequester, current: Boolean, enabled: Boolean,
    onOpen: () -> Unit, onForget: (keys: Boolean) -> Unit,
) {
    val p = pk
    var menu by remember { mutableStateOf(false) }
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    val remove = stringResource(R.string.recent_remove, r.name)
    val length = hms(r.duration, r.duration)
    val date = whenDone(r.finished)
    // the row's hover is its box's, which holds the × too: on the × the row
    // would lose it, and the × with it
    Box(Modifier.fillMaxWidth().then(if (enabled) Modifier else Modifier.alpha(0.45f)).hoverable(hover)) {
        Row(
            Modifier
                .focusRequester(focus)
                .control(
                    Corner, if (current) p.surface else Color.Transparent, enabled = enabled, held = menu,
                    onLongClick = { menu = true }, onClick = { if (!current) onOpen() },
                )
                .then(if (current) Modifier.semantics { selected = true } else Modifier)
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
                Text(r.name, style = rowName, color = p.ink, maxLines = 1, overflow = TextOverflow.MiddleEllipsis, modifier = Modifier.weight(1f))
                Text(length, style = figures, color = p.inkMuted, maxLines = 1)
                Text(
                    date, style = Type.small, color = p.inkMuted, maxLines = 1, textAlign = TextAlign.End,
                    // the desktop's 72, and as much more as a date of another year needs
                    modifier = Modifier.widthIn(min = 72.dp).alpha(if (hovered) 0f else 1f),
                )
            } else
                Column(Modifier.weight(1f)) {
                    Text(r.name, style = rowName, color = p.ink, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
                    Row {
                        Text(length, style = figures, color = p.inkMuted, maxLines = 1, modifier = Modifier.alignByBaseline())
                        Text("$DOT$date", style = Type.small, color = p.inkMuted, maxLines = 1, modifier = Modifier.alignByBaseline())
                    }
                }
        }
        // its × over its date under a mouse, as the desktop's (app.css,
        // .pk-x), out of the keys' way: Delete is theirs
        if (hovered && enabled)
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

/**
 * The transcript open beside the side that the list does not have (app.js,
 * liveHtml): under way, stopped or failed, or taken off the list; its name,
 * and under it how far it has got (rowBody), on the open row's surface.
 */
@Composable
private fun LiveRow(o: Opened) {
    val p = pk
    val pct = (o.done * 100).toInt()
    val (status, ink) = when (o.step) {
        "loading" -> stringResource(R.string.status_loading) to p.inkMuted
        "reading" -> stringResource(R.string.status_reading) to p.inkMuted
        "transcribing" -> stringResource(R.string.status_transcribing, pct) to p.inkMuted
        "stopped" -> stringResource(R.string.status_stopped_at, pct) to p.inkMuted
        "done" -> stringResource(R.string.status_done) to p.ok
        else -> stringResource(R.string.status_failed) to p.err
    }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .background(p.surface, Corner)
            .semantics(mergeDescendants = true) { selected = true }
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Icon(Icons.file, null, tint = p.ink, modifier = Modifier.size(16.dp))
        Column(Modifier.weight(1f)) {
            Text(o.name, style = rowName, color = p.ink, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                if (o.step == "done") Icon(Icons.check, null, tint = ink, modifier = Modifier.size(14.dp))
                Text(status, style = Type.small, color = ink)
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
private fun AskClear(onClear: () -> Unit, onCancel: () -> Unit) =
    Confirm(stringResource(R.string.ask_clear_title), stringResource(R.string.ask_clear_text), stringResource(R.string.ask_clear), onClear, onCancel)

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
    val share: (ext: String) -> Unit,
    val setTimes: (Boolean) -> Unit,
    val setCaptions: (Boolean) -> Unit,
    val play: () -> Unit,
    val seek: (seconds: Double, play: Boolean) -> Unit,
    val setBase: (Timebase) -> Unit,
)

/** The formats Share offers, the desktop's Save as's, in its order. */
class Format(val ext: String, val label: Int, val withTimes: Int = label)

val FORMATS = listOf(
    Format("txt", R.string.format_txt, R.string.format_txt_times),
    Format("srt", R.string.format_srt),
    Format("vtt", R.string.format_vtt),
    Format("md", R.string.format_md, R.string.format_md_times),
    Format("json", R.string.format_json),
)

/**
 * One file at work and then its transcript (ui/index.html, #work): the
 * header with the way back, the name, and what can be done with it (at
 * work only Stop; Copy and Share once it is done), the strip, the
 * choice of sentences or captions, a line for what went wrong, and the rows.
 * While the keyboard is up for Find and leaves the rows under the floor
 * (160 dp withText) below the header, the strip and the views line as they
 * were last shown, those make room for it: the field takes the top line and
 * the rows the rest, as a phone's find in page takes its top; the keyboard
 * put away, or the search closed, they come back (the three apps, Peter,
 * 2026-11-01). On its side the emulator's Pixel 5 has 120 dp above the
 * keyboard, where the three took 235 and left the field out of sight and
 * the transcript not a row; upright, 499 leave the rows 250 at the default
 * text size, and at twice it 164, under the floor's 297. The emulator's
 * 1280 by 800 dp tablet makes room too on its side, at the default text
 * size: 408 dp above the keyboard, where the three as shown leave the rows
 * 152; upright, at twice the text size, it keeps 598 dp of rows.
 */
@OptIn(ExperimentalLayoutApi::class)
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
    // the header's, the strip's and the views line's heights as last shown,
    // which making room is measured by, so that their going cannot undo it
    var head by remember { mutableIntStateOf(0) }
    var strip by remember { mutableIntStateOf(0) }
    var line by remember { mutableIntStateOf(0) }
    // the screen's height, the keyboard's taken from it
    var tall by remember { mutableIntStateOf(0) }
    val floor = 160.dp.withText()
    // what the three leave the rows, less the rows' 16 dp above them
    val makeRoom = WindowInsets.isImeVisible && find.typing && review && rows.isNotEmpty() && tall > 0 &&
        with(LocalDensity.current) { (tall - head - strip - line).toDp() } - 16.dp < floor
    Column(Modifier.fillMaxSize().onSizeChanged { tall = it.height }) {
        // the header's buttons in the middle of its 62 dp (app.css, .pk-head),
        // the way back's arrow on the cards' line, 16 dp from the screen's
        // edge, its button's padding outside it: the arrow's ink is 14 dp into
        // its 40 (11 to the 18 dp icon, and 3 in that), so the button is at 2
        if (!makeRoom) Head(
            back = if (running) null else ({ IconBtn(Icons.back, stringResource(R.string.back), a.back, kind = Kind.Ghost) }),
            name = {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(name, style = Type.ui, color = p.ink, maxLines = 1, overflow = TextOverflow.MiddleEllipsis, modifier = Modifier.weight(1f, fill = false))
                    // until the ring has progress: reading, the model loading, the first chunk (Spinner)
                    if (running && !(step == "transcribing" && done > 0)) Spinner()
                }
            },
            modifier = Modifier
                .onSizeChanged { head = it.height }
                .fillMaxWidth()
                .heightIn(min = 62.dp)
                .padding(start = if (running) 16.dp else 2.dp, end = 16.dp, top = 11.dp, bottom = 11.dp),
        ) {
            if (running)
                PkButton(stringResource(R.string.stop), a.stop, kind = Kind.Ghost)
            if (review && rows.isNotEmpty()) {
                IconBtn(Icons.copy, stringResource(if (captions) R.string.copy_captions else R.string.copy_text), a.copy)
                ShareAs(name, times, a)
            }
        }
        if (step != "error" && !makeRoom)
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
                // 16 under the header's buttons, the one gap between the
                // screen's blocks (Peter, 2026-10-27): their 11 and 5
                modifier = Modifier.onSizeChanged { strip = it.height }.padding(start = 16.dp, end = 16.dp, top = 5.dp),
                why = why,
                locate = locate,
            )
        if (review && rows.isNotEmpty())
            // a step while it plays takes the playhead to its match (Finder.step)
            Views(
                captions, a.setCaptions, find, hits, if (playing) ({ t: Double -> a.seek(t, false) }) else null, makeRoom,
                Modifier.onSizeChanged { if (!makeRoom) line = it.height }.padding(start = 16.dp, end = 16.dp, top = 16.dp),
            )
        val msg = when {
            error != null -> error
            step == "stopped" -> stringResource(R.string.status_stopped_at, (done * 100).toInt())
            step == "done" && rows.isEmpty() -> stringResource(R.string.no_speech)
            else -> null
        }
        if (msg != null && !makeRoom)
            Row(
                Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                val bad = error != null
                if (bad) Icon(Icons.alert, null, tint = p.err, modifier = Modifier.size(16.dp))
                Text(msg, style = Type.body.copy(fontSize = 15.sp), color = if (bad) p.err else p.inkMuted)
            }
        Rows(rows, review, captions, playing, position, stats, base, hits, find.at, find.shown, list, a, Modifier.weight(1f))
    }
}

/**
 * The header's line: the way back at its start, what can be done with the
 * file at its end, and the name between them while it keeps the words'
 * least there (wordsFloor), or its whole width where that is less; else the
 * name on a line of its own 16 dp under them, under the arrow's ink, and the
 * buttons stay where the thumb knows them. On a 393 dp phone the name has
 * 187 dp beside Copy and Share at the default text size, over the floor's
 * 160, and 175 at 1.3 times it, under the floor's 208, so it goes under
 * them there (2026-10-31). Where even that line is too short for the name
 * it gives up its middle, which keeps its extension.
 */
@Composable
private fun Head(back: (@Composable () -> Unit)?, name: @Composable () -> Unit, modifier: Modifier, actions: @Composable () -> Unit) {
    val least = wordsFloor()
    Layout(
        {
            Box { back?.invoke() }
            Box { name() }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { actions() }
        },
        modifier,
    ) { (b, n, k), c ->
        val gap = 8.dp.roundToPx()
        val loose = c.copy(minWidth = 0, minHeight = 0)
        val bp = b.measure(loose)
        val kp = k.measure(loose)
        val from = if (bp.width > 0) bp.width + gap else 0
        val end = if (kp.width > 0) kp.width + gap else 0
        val room = c.maxWidth - from - end
        if (room >= minOf(least.roundToPx(), n.maxIntrinsicWidth(Constraints.Infinity))) {
            val np = n.measure(loose.copy(maxWidth = room))
            val h = maxOf(bp.height, np.height, kp.height, c.minHeight)
            layout(c.maxWidth, h) {
                bp.place(0, (h - bp.height) / 2)
                np.place(from, (h - np.height) / 2)
                kp.place(c.maxWidth - kp.width, (h - kp.height) / 2)
            }
        } else {
            // the arrow's ink is 14 dp into its button
            val indent = if (bp.width > 0) 14.dp.roundToPx() else 0
            val np = n.measure(loose.copy(maxWidth = c.maxWidth - indent))
            val bar = maxOf(bp.height, kp.height, c.minHeight)
            // the screen's one step between its blocks (Work): flush under
            // the buttons, the name's line sat on Share's edge at 1.3 times
            // the text size on a fold's 337 dp cover (Peter, 2026-10-31)
            val step = 16.dp.roundToPx()
            layout(c.maxWidth, bar + step + np.height) {
                bp.place(0, (bar - bp.height) / 2)
                kp.place(c.maxWidth - kp.width, (bar - kp.height) / 2)
                np.place(indent, bar + step)
            }
        }
    }
}

/**
 * The views line: Sentences or Captions, and the search right after them,
 * 12 dp apart, one group of the transcript's controls (app.css, .pk-views
 * and .pk-find): alone at the line's far end, a reader might not see that
 * the transcript can be searched (Peter, 2026-10-31). Its field, which
 * says Find, where that fits beside them, the desktop's 200 dp and 280 with
 * a search in it, grown with the text (withText): at twice the text
 * size a tablet's 280 left "rabbit" 53 dp and scrolled its first letter
 * out (2026-10-31). Else the glass, which opens the field across the line
 * until the search is let go, as a phone's find in page does. The views
 * have what the glass leaves them, stacked where that is too little for
 * the two side by side (Switch2). Alone, while the header and the strip
 * make room for the keyboard (Work): the field by itself across the line,
 * the same field as beside the views, so that it keeps the keys and the
 * keyboard stays up as it moves.
 */
@Composable
private fun Views(captions: Boolean, setCaptions: (Boolean) -> Unit, find: Finder, hits: List<Hit>, seek: ((Double) -> Unit)?, alone: Boolean, modifier: Modifier) {
    val first = stringResource(R.string.sentences)
    val second = stringResource(R.string.captions)
    val label = stringResource(R.string.find_label)
    val opened = find.open || find.query.isNotBlank()
    val width = (if (find.query.isNotBlank()) 280.dp else 200.dp).withText()
    SubcomposeLayout(modifier.fillMaxWidth()) { c ->
        val gap = 12.dp.roundToPx()
        val loose = c.copy(minWidth = 0, minHeight = 0)
        val v = subcompose("views") { Switch2(first, second, captions, setCaptions) }
            .first().measure(loose.copy(maxWidth = c.maxWidth - gap - 40.dp.roundToPx()))
        val field = width.roundToPx()
        val beside = !alone && v.width + gap + field <= c.maxWidth
        // one field at either width, which keeps the keys when a search
        // makes it too wide to stay beside the views, or the keyboard needs the room
        val f = if (beside || opened || alone) {
            val w = if (beside) field else c.maxWidth
            subcompose("field") { FindField(find, hits, seek, find::clear) }.first().measure(loose.copy(minWidth = w, maxWidth = w))
        } else
            subcompose("glass") { IconBtn(Icons.search, label, find::focus, kind = Kind.Ghost) }.first().measure(loose)
        val views = beside || !(opened || alone)
        val h = maxOf(if (views) v.height else 0, f.height)
        layout(c.maxWidth, h) {
            if (views) v.place(0, (h - v.height) / 2)
            f.place(if (views) v.width + gap else 0, (h - f.height) / 2)
        }
    }
}

/**
 * Share, the plume button a finished transcript is for, and its menu of
 * formats, the desktop's Save as's: each sends the transcript as a file of
 * its kind to the app picked in the system's share sheet, which has Save
 * file too (MainActivity.share). Where the desktop's button saves, a
 * phone's shares: on one a transcript goes to another app, Drive or a chat
 * or mail, far more often than to a folder. Ctrl+S still saves, where there
 * are keys.
 */
@Composable
private fun ShareAs(name: String, times: Boolean, a: WorkActions) {
    var open by remember { mutableStateOf(false) }
    val stem = name.substringBeforeLast('.').ifEmpty { name }
    Box {
        PkButton(stringResource(R.string.share), { open = true }, kind = Kind.Primary, trailing = Icons.down)
        Menu(open, { open = false }) {
            for (f in FORMATS)
                Item(stringResource(if (times) f.withTimes else f.label), "$stem.${f.ext}") {
                    open = false
                    a.share(f.ext)
                }
            MenuLine()
            Check(stringResource(R.string.save_times), times, a.setTimes)
        }
    }
}

/**
 * The transcript's rows (app.css, .pk-seg): each sentence's time, or a cue's
 * start and end, then its text, at most 560 dp wide; the time above the
 * text where the text would keep under 432 dp beside the time's column, 112
 * dp or what the file's longest time needs at a larger text size: under 620
 * dp at the default, as the desktop's window has it (app.css, max-width:
 * 619px), and a cue's two times a range. The row playing on the surface, the beak's square after
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
    shown: Int,
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
    // the list from one gap under the views line, its rows scrolling under
    // that edge, not up to the line, where the playing row's card had met the
    // search's field (Peter, the S25, 2026-10-27)
    // the time's column 112 dp, the desktop's, or as wide as the file's
    // longest time and the beak after it (SegRow) need, at a larger text size
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val times = remember(base, measurer, density) {
        with(density) { measurer.measure(base.at(base.duration), figures).size.width.toDp() } + 10.dp
    }
    BoxWithConstraints(modifier.fillMaxWidth().padding(top = 16.dp)) {
        // a column while the text keeps 432 dp beside it, as at 620 beside
        // 112: the list's 16 dp and the row's 16 either side, and the gap's 12
        val column = maxOf(112.dp, times).takeIf { maxWidth >= 508.dp + it }
        SelectionContainer(selection) {
            LazyColumn(state = list, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 16.dp)) {
                itemsIndexed(rows) { i, s ->
                    val now = i == current
                    val word = if (now) s.words.indexOfLast { it.start <= position + 1e-3 } else -1
                    SegRow(s, now, word, marks[i], at, shown, column, captions, review, base, selection) { a.seek(s.start, true) }
                }
                if (stats != null)
                    item {
                        Text(
                            stats,
                            style = Type.small.copy(fontSize = 12.sp, fontFeatureSettings = "tnum"),
                            color = p.inkMuted,
                            modifier = Modifier.padding(start = if (column != null) column + 28.dp else 16.dp, top = 16.dp),
                        )
                    }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SegRow(
    s: Seg, now: Boolean, word: Int, marks: List<IndexedValue<Hit>>?, at: Int, shown: Int, column: Dp?, cue: Boolean, review: Boolean,
    base: Timebase, selection: SelectionState, onTime: () -> Unit,
) {
    val p = pk
    val stamp = figures
    val line = stamp.copy(lineHeight = 20.sp)
    val start = @Composable { mod: Modifier -> Text(base.at(s.start), style = line, color = p.inkMuted, modifier = mod) }
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
    // times of their own (Peter, the S25, 2026-10-26). A range too wide for
    // the line takes its end to the next whole: at 2.0 times the text size
    // a phone's line broke the end's time in two.
    val time = @Composable { mod: Modifier ->
        if (cue && column == null)
            FlowRow(mod) {
                Text("${base.at(s.start)} – ", style = line, color = p.inkMuted)
                Row {
                    Text(base.at(s.end), style = line, color = p.inkMuted, modifier = Modifier.alignByBaseline())
                    beak(this)
                }
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
    if (column != null)
        Row(box.padding(horizontal = 16.dp, vertical = 8.dp)) {
            time(Modifier.width(column).alignByBaseline())
            Spacer(Modifier.width(12.dp))
            Sentence(s, word, marks, at, shown, Modifier.weight(1f).alignByBaseline())
        }
    else
        Column(box.padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            time(Modifier)
            Sentence(s, word, marks, at, shown, Modifier)
        }
}

/**
 * A row's text in the transcript's type, the word playing underlined in the
 * beak's colour, 2 dp, 4 below the line; and a search's matches marked
 * (app.css, .pk-hit): each on plume mixed 45% into the ground, as tall as
 * its letters and 2 dp round them, its corners 3 dp; the one the reader is
 * on in plume, its letters on-plume, an ink bar 2 dp under it, for a state
 * shown by more than its colour (the tokens' contrast, 2026-10-22). Once
 * the list has brought a move's row into sight (Finder.show), the match's
 * own line too, and the row's 8 dp round it: a row taller than the room,
 * above the keyboard (Work) or a long one, had its top in sight and the
 * match below.
 */
@Composable
private fun Sentence(s: Seg, word: Int, marks: List<IndexedValue<Hit>>?, at: Int, shown: Int, modifier: Modifier) {
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
    val sight = remember { BringIntoViewRequester() }
    val pad = with(LocalDensity.current) { 8.dp.toPx() }
    // what was shown before this row came onto the list is not this row's to show
    val first = remember { shown }
    LaunchedEffect(shown) {
        if (shown == first || on == null || on.end > s.text.length) return@LaunchedEffect
        val l = snapshotFlow { layout }.filterNotNull().first()
        val top = l.getLineTop(l.getLineForOffset(on.start))
        val bottom = l.getLineBottom(l.getLineForOffset(maxOf(on.start, on.end - 1)))
        sight.bringIntoView(Rect(0f, top - pad, l.size.width.toFloat(), bottom + pad))
    }
    Text(
        text,
        style = Type.transcript,
        color = p.ink,
        onTextLayout = { layout = it },
        modifier = modifier.bringIntoViewRequester(sight).drawBehind {
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
