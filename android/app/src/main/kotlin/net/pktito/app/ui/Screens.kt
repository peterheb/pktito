package net.pktito.app.ui

import android.os.SystemClock
import android.text.format.DateFormat
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.EaseOut
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.contextmenu.builder.item
import androidx.compose.foundation.text.contextmenu.modifier.appendTextContextMenuComponents
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.text.selection.SelectionState
import androidx.compose.foundation.text.selection.rememberSelectionState
import androidx.compose.material3.Icon
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.MotionDurationScale
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.LineBreak
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import net.pktito.app.R
import net.pktito.app.Recents
import net.pktito.app.Seg
import net.pktito.app.Speakers

/**
 * What the start screen does with what it is asked: choose a file, About, and the recent list's
 * open, forget and clear, by a transcript's id; and the switch beside Choose, where the build
 * offers a diarizer (TellApart).
 */
class ReadyActions(
    val choose: () -> Unit,
    val about: () -> Unit,
    val open: (id: String) -> Unit,
    val forget: (id: String) -> Unit,
    val clear: () -> Unit,
    val speakers: TellApart? = null,
)

/**
 * The start screen's switch, Tell speakers apart (Speakers.on): the one thing that decides whether
 * a new transcription finds the speakers (Peter, 2026-11-19). busy while their model downloads.
 */
class TellApart(val on: Boolean, val busy: Boolean, val set: (Boolean) -> Unit)

/**
 * TellApart's row under Choose: its words, and the platform's own switch at its end, the whole row
 * one target that turns it, as Android's settings rows are (android-platform-over-pixels). On in
 * plume, as Choose is; off a muted thumb on the sunk track.
 */
@Composable
private fun TellApartRow(s: TellApart, modifier: Modifier = Modifier) {
    val p = pk
    Row(
        modifier
            .toggleable(s.on, enabled = !s.busy, role = Role.Switch) { s.set(it) }
            .heightIn(min = 48.dp)
            .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.speakers_tell_apart), style = Type.ui, color = p.ink)
            Text(
                stringResource(R.string.speakers_tell_apart_note),
                style = Type.small,
                color = p.inkMuted,
            )
        }
        Switch(
            s.on,
            null,
            enabled = !s.busy,
            colors =
                SwitchDefaults.colors(
                    checkedThumbColor = p.onPlume,
                    checkedTrackColor = p.plume,
                    checkedBorderColor = p.plume,
                    uncheckedThumbColor = p.inkMuted,
                    uncheckedTrackColor = p.sunk,
                    uncheckedBorderColor = p.inkMuted,
                ),
        )
    }
}

/**
 * The start screen (ui/index.html, #ready): the wordmark and the model in use, its menu
 * (ModelButton), on the column's top edge, and under them the prompt and its button in a dashed
 * zone, as the desktop's drop zone, which says where a file goes. A phone has nothing to drop, so
 * the zone says what it is for. The wordmark opens About. With transcripts on the recent list, the
 * column is the screen's height and the list takes what is left under the zone (app.css,
 * .has-recent); without, the top row and the zone have their middle in the window's, as iOS's and
 * the desktop's (.pk-ready). Where the top row and the zone, as tall as their words make them,
 * leave the list under 160 dp, the screen scrolls as a whole, the list with it: the desktop's
 * window is never that short, but a split screen is, and a phone on its side at any text size. The
 * emulator's Pixel 5 on its side has 345 dp, which leave the list 66 at the default text size and
 * 38 at twice it; upright at twice it they leave 427 (2026-11-01).
 */
@Composable
fun Ready(recent: List<Recents.Item>, a: ReadyActions, model: @Composable () -> Unit) {
    var asking by rememberSaveable { mutableStateOf(false) }
    val listed = recent.isNotEmpty()
    // the top row's height and the zone's, as they were last laid out
    var top by remember { mutableIntStateOf(0) }
    var zone by remember { mutableIntStateOf(0) }
    BoxWithConstraints(
        Modifier.fillMaxSize(),
        contentAlignment = if (listed) Alignment.TopCenter else Alignment.Center,
    ) {
        // what they leave the list, less the column's 11 dp above and 16
        // below and its two gaps of 24
        val left = maxHeight - 75.dp - with(LocalDensity.current) { (top + zone).toDp() }
        val short = left < 160.dp
        val edge = if (maxWidth < 600.dp) 16.dp else 40.dp
        Column(
            Modifier.widthIn(max = 640.dp)
                .fillMaxWidth()
                .then(
                    when {
                        short -> Modifier.scrolls()
                        listed -> Modifier.fillMaxHeight()
                        else -> Modifier
                    }
                )
                // 16 from a phone's sides, as every screen there, and 40 in
                // the 640 dp column of a wider one (design/README.md, Edges);
                // its steps the scale's 24, and 32 over the list with the
                // list's own 8; its top row on the work header's line, 11
                // down round its 40 as the side's, and 16 under the list
                // (app.css, .pk-ready): they were 20 in, steps of 22, and
                // 20 above and below (the layout audit, 2026-11-01). With
                // nothing on the list, the column in the window's middle, 16
                // above and below, so that the top row and the zone have
                // their middle there: at 11 above they stood 2.6 dp over it
                // on a phone
                .padding(
                    start = edge,
                    end = edge,
                    top = if (listed) 11.dp else 16.dp,
                    bottom = 16.dp,
                ),
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            TopRow(a, model, Modifier.onSizeChanged { top = it.height })
            Drop(a, Modifier.fillMaxWidth().onSizeChanged { zone = it.height })
            if (listed)
                Recent(
                    recent,
                    a,
                    onClear = { asking = true },
                    scrolls = !short,
                    if (short) Modifier else Modifier.weight(1f),
                )
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
 * The start screen's top row: the wordmark, which opens About, and the model in use, its menu
 * (ModelButton), under the wordmark where the two do not fit (Beside).
 */
@Composable
private fun TopRow(a: ReadyActions, model: @Composable () -> Unit, modifier: Modifier = Modifier) {
    Beside(modifier.fillMaxWidth(), words = { WordmarkButton(a.about) }) { model() }
}

/**
 * The drop zone (app.css, .pk-ready-drop): the prompt and Choose in a dashed zone, its dashes 6.2:1
 * on the surface inside, 6.8 on the ground outside, past the 3:1 a boundary needs.
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
                    p.inkMuted,
                    Offset(w / 2, w / 2),
                    size.copy(size.width - w, size.height - w),
                    CornerRadius(Radius.radiusUi.toPx()),
                    style =
                        Stroke(
                            w,
                            pathEffect =
                                PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx())),
                        ),
                )
            }
            .padding(horizontal = 24.dp, vertical = 36.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            stringResource(R.string.ready_lead),
            // balanced, as the desktop's (app.css, .pk-lead): at twice the
            // text size a phone left "video" alone on the second line
            style = Type.lead.copy(fontSize = 22.sp, lineBreak = LineBreak.Heading),
            color = p.ink,
            textAlign = TextAlign.Center,
        )
        PkButton(stringResource(R.string.choose), a.choose, kind = Kind.Primary, large = true)
        // beside where the file is chosen, as wide as a phone's zone allows
        a.speakers?.let { TellApartRow(it, Modifier.widthIn(max = 360.dp).fillMaxWidth()) }
    }
}

/**
 * A transcript open beside the side, as its row there says it (LiveRow): how far it has got, by the
 * desktop's steps.
 */
class Opened(val id: String, val name: String, val step: String, val done: Double) {
    val running: Boolean
        get() = step == "loading" || step == "reading" || step == "transcribing"
}

/**
 * A window with room for the work beside it (MainActivity, sided) keeps the start screen's top row
 * and recent list in a column at its left, the side, as the desktop's does from 1160 px (app.css,
 * the side): beside the drop zone while the list has anything in it (open, null), and beside the
 * transcript open, whose row it marks, or heads the list with where the list does not have it
 * (LiveRow); Choose a file… stands in there for the zone's. 300 dp, the desktop's 300 px. Another
 * row there leaves the one open as Back would, and opens (ReadyActions.open). While a transcription
 * runs, the rows and Choose wait for it, dimmed: Android transcribes one file at a time, and a
 * phone has no way back from one under way either.
 */
@Composable
fun Side(
    recent: List<Recents.Item>,
    a: ReadyActions,
    open: Opened?,
    model: @Composable () -> Unit,
    work: @Composable () -> Unit,
) {
    val p = pk
    var asking by rememberSaveable { mutableStateOf(false) }
    val running = open?.running == true
    val live = open?.takeIf { o -> recent.none { it.id == o.id } }
    Row(Modifier.fillMaxSize()) {
        Column(
            Modifier.width(300.dp)
                .fillMaxHeight()
                // its edge, the desktop's border-right
                .drawBehind {
                    drawRect(
                        p.sunk,
                        Offset(size.width - 1.dp.toPx(), 0f),
                        Size(1.dp.toPx(), size.height),
                    )
                }
                // its top row on the work's header's line, the model menu's
                // 40 in the middle of the header's 62, and Choose with the
                // strip's top, 16 under the row as the strip is under the
                // header's buttons (their 11 and its 5): at 20 the row sat 9
                // lower than the header's and Choose 7 lower than the strip
                // (Peter, on the iPad's, 2026-11-01; app.css, the side). Its
                // steps the scale's 16, as the desktop's: they were 14, and
                // 18 over the list (the layout audit, 2026-11-01)
                .padding(start = 20.dp, top = 11.dp, end = 16.dp, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            TopRow(a, model)
            if (open != null)
                PkButton(
                    stringResource(R.string.choose),
                    a.choose,
                    Modifier.fillMaxWidth(),
                    enabled = !running,
                )
            if (open != null) a.speakers?.let { TellApartRow(it, Modifier.fillMaxWidth()) }
            if (recent.isNotEmpty() || live != null)
                Recent(
                    recent,
                    a,
                    onClear = { asking = true },
                    scrolls = true,
                    Modifier.weight(1f),
                    top = 0.dp,
                    open = open?.id,
                    live = live,
                    enabled = !running,
                )
        }
        Box(Modifier.weight(1f).fillMaxHeight()) {
            // the zone in the middle, 560 wide, 40 from either edge at the least
            if (open == null)
                Drop(
                    a,
                    Modifier.align(Alignment.Center)
                        .padding(horizontal = 40.dp)
                        .widthIn(max = 560.dp)
                        .fillMaxWidth(),
                )
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
 * The recent list (app.css, .pk-recent): its heading and Clear, which asks first, then each
 * transcript, newest first, opened by a tap; taken off the list by a long press's menu, by the ×
 * over its date under a mouse, or by Delete from the keyboard, which goes on to the row that takes
 * its place (app.js, renderReady and forget). Each row is a line where that leaves its name 250 dp,
 * as the desktop's narrowest window's list, 440 wide (480 less its margins), leaves it, grown with
 * the text (withText), beside the list's widest length and date; on a phone, narrower, its length
 * and date go under its name, which keeps the width. On an open fold, 673 dp, whose start screen
 * has 560 inside its column and the list 12 past either side of that, a row keeps its line through
 * 1.3 times the text size, the name 393 dp, and at 1.5 would have 385 to the 390 asked there, so
 * the two go under (2026-11-01). scrolls: the list scrolls under its heading, else it is all there,
 * for a screen that scrolls (Ready). In the side, the row of the transcript open is marked (open),
 * one the list does not have heads it (live), and while one is under way the rows and Clear wait
 * (enabled); Clear goes with the last of the list's own.
 */
@Composable
private fun Recent(
    items: List<Recents.Item>,
    a: ReadyActions,
    onClear: () -> Unit,
    scrolls: Boolean,
    modifier: Modifier,
    top: Dp = 8.dp,
    open: String? = null,
    live: Opened? = null,
    enabled: Boolean = true,
) {
    val p = pk
    val rows = remember { mutableMapOf<String, FocusRequester>() }
    // the widest length and date of a row: the length's figures, of one
    // width, and the date in its 72 dp, or what a longer one needs (RecentRow)
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val dates = items.map { whenDone(it.finished) }
    val beside =
        remember(items, dates, measurer, density) {
            with(density) {
                items.indices
                    .maxOfOrNull { i ->
                        val length =
                            measurer
                                .measure(hms(items[i].duration, items[i].duration), figures)
                                .size
                                .width
                        length +
                            maxOf(
                                72.dp.roundToPx(),
                                measurer.measure(dates[i], Type.small).size.width,
                            )
                    }
                    ?.toDp() ?: 0.dp
            }
        }
    val least = 250.dp.withText()
    val now by rememberUpdatedState(items)
    var refocus by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(refocus) {
        val i = refocus ?: return@LaunchedEffect
        // a frame, for the list to lay out the row that took its place
        withFrameNanos {}
        now.getOrNull(minOf(i, now.size - 1))?.let { runCatching { rows[it.id]?.requestFocus() } }
        refocus = null
    }
    // 12 dp past the edges the screen keeps on either side, so that the
    // heading's words, Clear's and the rows' icons stand on those edges and
    // the rows' boxes go round them, as the desktop's rows' hover does, 4 px
    // from a narrow window's edge (design/README.md, On an edge): they were
    // 12 inside them (the layout audit, 2026-11-01)
    Column(
        modifier.bleed(12.dp).padding(top = top),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                stringResource(R.string.recent_title),
                style = Type.ui,
                color = p.ink,
                modifier = Modifier.weight(1f),
            )
            // the list's one action, a ghost button as small as the heading,
            // its padding outside it so that its word ends over the dates,
            // in their grey (app.css, #clear-recent)
            if (items.isNotEmpty())
                Text(
                    stringResource(R.string.recent_clear),
                    style = Type.ui.copy(fontSize = 13.sp),
                    color = p.inkMuted,
                    modifier =
                        Modifier.bleed(8.dp, 11.dp)
                            .then(if (enabled) Modifier else Modifier.alpha(0.45f))
                            .control(
                                Corner,
                                enabled = enabled,
                                label = stringResource(R.string.recent_clear_label),
                                onClick = onClear,
                            )
                            .padding(horizontal = 8.dp, vertical = 11.dp),
                )
        }
        BoxWithConstraints(if (scrolls) Modifier.weight(1f) else Modifier) {
            // the row's 10 and 12 dp, its icon's 16 and three gaps of 10
            val oneLine = maxWidth - 68.dp - beside >= least
            val row =
                @Composable { i: Int, r: Recents.Item ->
                    val focus = remember { FocusRequester() }
                    DisposableEffect(r.id) {
                        rows[r.id] = focus
                        onDispose { rows.remove(r.id) }
                    }
                    RecentRow(
                        r,
                        oneLine,
                        focus,
                        current = r.id == open,
                        enabled = enabled,
                        onOpen = { a.open(r.id) },
                        onForget = { keys ->
                            if (keys) refocus = i
                            a.forget(r.id)
                        },
                    )
                }
            if (scrolls)
                LazyColumn(
                    Modifier.scrollbar(rememberBar(rememberLazyListState())),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
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

/**
 * A recent row's room before its icon: 12, the heading's, less the 2 dp that the file icon's ink
 * stands in its 16 dp box (Feather's file, from 3 of 24 with its stroke), so that the ink is on the
 * screen's edge, under the heading's first letter (design/README.md, On an edge).
 */
private val rowStart = 10.dp

/** A time or a length, in the stamp's type with figures of one width. */
private val figures = Type.stamp.copy(fontFeatureSettings = "tnum")

/**
 * Between a row's times and its text, 46 dp, the desktop's (app.css, --gutter): the same beside
 * every file's times, so that a turn's line in its middle stands as far from them in each, and the
 * text takes what the times leave of the row (Peter, 2026-11-15).
 */
private val GUTTER = 46.dp

/** Change speaker's key among the selection menu's items (Rows). */
private val CHANGE_SPEAKER = Any()

/**
 * onForget: true from the keyboard, which then goes on to the next row. The transcript open beside
 * the side (current) is on the surface, as the desktop's row with aria-current, and a tap there is
 * the one already open.
 */
@Composable
private fun RecentRow(
    r: Recents.Item,
    oneLine: Boolean,
    focus: FocusRequester,
    current: Boolean,
    enabled: Boolean,
    onOpen: () -> Unit,
    onForget: (keys: Boolean) -> Unit,
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
    Box(
        Modifier.fillMaxWidth()
            .then(if (enabled) Modifier else Modifier.alpha(0.45f))
            .hoverable(hover)
    ) {
        Row(
            Modifier.focusRequester(focus)
                .control(
                    Corner,
                    if (current) p.surface else Color.Transparent,
                    enabled = enabled,
                    held = menu,
                    onLongClick = { menu = true },
                    onClick = { if (!current) onOpen() },
                )
                .then(if (current) Modifier.semantics { selected = true } else Modifier)
                .onKeyEvent { e ->
                    val del =
                        e.type == KeyEventType.KeyDown &&
                            (e.key == Key.Delete || e.key == Key.Backspace)
                    if (del) onForget(true)
                    del
                }
                .semantics {
                    customActions =
                        listOf(
                            CustomAccessibilityAction(remove) {
                                onForget(false)
                                true
                            }
                        )
                }
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .padding(start = rowStart, end = 12.dp, top = 10.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(Icons.file, null, tint = p.ink, modifier = Modifier.size(16.dp))
            if (oneLine) {
                Text(
                    r.name,
                    style = rowName,
                    color = p.ink,
                    maxLines = 1,
                    overflow = TextOverflow.MiddleEllipsis,
                    modifier = Modifier.weight(1f),
                )
                Text(length, style = figures, color = p.inkMuted, maxLines = 1)
                Text(
                    date,
                    style = Type.small,
                    color = p.inkMuted,
                    maxLines = 1,
                    textAlign = TextAlign.End,
                    // the desktop's 72, and as much more as a date of another year needs
                    modifier = Modifier.widthIn(min = 72.dp).alpha(if (hovered) 0f else 1f),
                )
            } else
                Column(Modifier.weight(1f)) {
                    Text(
                        r.name,
                        style = rowName,
                        color = p.ink,
                        maxLines = 1,
                        overflow = TextOverflow.MiddleEllipsis,
                    )
                    Row {
                        Text(
                            length,
                            style = figures,
                            color = p.inkMuted,
                            maxLines = 1,
                            modifier = Modifier.alignByBaseline(),
                        )
                        Text(
                            "$DOT$date",
                            style = Type.small,
                            color = p.inkMuted,
                            maxLines = 1,
                            modifier = Modifier.alignByBaseline(),
                        )
                    }
                }
        }
        // its × over its date under a mouse, as the desktop's (app.css,
        // .pk-x), out of the keys' way: Delete is theirs
        if (hovered && enabled)
            Box(
                Modifier.align(Alignment.CenterEnd)
                    .padding(end = 6.dp)
                    .size(32.dp)
                    .focusProperties { canFocus = false }
                    .control(CornerSm, label = remove, onClick = { onForget(false) }),
                contentAlignment = Alignment.Center,
            ) {
                Icon(Icons.x, null, tint = p.inkMuted, modifier = Modifier.size(16.dp))
            }
        Menu(menu, { menu = false }) {
            Item(
                stringResource(R.string.recent_remove_item),
                null,
                note = AnnotatedString(stringResource(R.string.recent_remove_note)),
            ) {
                menu = false
                onForget(false)
            }
        }
    }
}

/**
 * The transcript open beside the side that the list does not have (app.js, liveHtml): under way,
 * stopped or failed, or taken off the list; its name, and under it how far it has got (rowBody), on
 * the open row's surface.
 */
@Composable
private fun LiveRow(o: Opened) {
    val p = pk
    val pct = (o.done * 100).toInt()
    val (status, ink) =
        when (o.step) {
            "loading" -> stringResource(R.string.status_loading) to p.inkMuted
            "reading" ->
                (if (o.done > 0) stringResource(R.string.status_reading_at, pct)
                else stringResource(R.string.status_reading)) to p.inkMuted
            "transcribing" -> stringResource(R.string.status_transcribing, pct) to p.inkMuted
            "stopped" -> stringResource(R.string.status_stopped_at, pct) to p.inkMuted
            "done" -> stringResource(R.string.status_done) to p.ok
            else -> stringResource(R.string.status_failed) to p.err
        }
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 48.dp)
            .background(p.surface, Corner)
            .semantics(mergeDescendants = true) { selected = true }
            .padding(start = rowStart, end = 12.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Icon(Icons.file, null, tint = p.ink, modifier = Modifier.size(16.dp))
        Column(Modifier.weight(1f)) {
            Text(
                o.name,
                style = rowName,
                color = p.ink,
                maxLines = 1,
                overflow = TextOverflow.MiddleEllipsis,
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                if (o.step == "done")
                    Icon(Icons.check, null, tint = ink, modifier = Modifier.size(14.dp))
                Text(status, style = Type.small, color = ink)
            }
        }
    }
}

/**
 * When a transcript finished, as the desktop's list says it: Today, Yesterday, else the date, its
 * year if not this one's (app.js, when).
 */
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
            DateTimeFormatter.ofPattern(DateFormat.getBestDateTimePattern(locale, skeleton), locale)
                .format(day)
        }
    }
}

/**
 * Clearing the list asks first, as the desktop's window does (app.js, askClear): what it does, then
 * Cancel, the one the question is for, and Clear after it in the error's red, where Android's
 * dialogs put the confirming action (Confirm). Back, Esc and a tap beside it are Cancel; with keys,
 * Cancel has the focus, so that Enter does not clear the list.
 */
@Composable
private fun AskClear(onClear: () -> Unit, onCancel: () -> Unit) =
    Confirm(
        stringResource(R.string.ask_clear_title),
        stringResource(R.string.ask_clear_text),
        stringResource(R.string.ask_clear),
        onClear,
        onCancel,
    )

/**
 * The wordmark as each screen's way to About, its padding outside it so that the word stays where
 * it is (app.css, button.pk-wm).
 */
@Composable
fun WordmarkButton(onAbout: () -> Unit) {
    Box(
        Modifier.bleed(6.dp, 4.dp)
            .control(Corner, label = stringResource(R.string.about_open), onClick = onAbout)
            .padding(horizontal = 6.dp, vertical = 4.dp)
    ) {
        Wordmark()
    }
}

/** What the work screen does with what it is asked. */
class WorkActions(
    val back: () -> Unit,
    val stop: () -> Unit,
    val copy: () -> Unit,
    val share: (ext: String) -> Unit,
    val setTimes: (Boolean) -> Unit,
    val play: () -> Unit,
    val seek: (seconds: Double, play: Boolean) -> Unit,
    /** a seek on the waveform, whose row the list brings wherever the reader is (Follow) */
    val jump: (seconds: Double) -> Unit,
    val setBase: (Timebase) -> Unit,
    /** Only matches turned, the reader kept on their sentence (MainActivity) */
    val only: () -> Unit = {},
)

/** The formats Share offers, the desktop's Save as's, in its order. */
class Format(val ext: String, val label: Int, val withTimes: Int = label)

val FORMATS =
    listOf(
        Format("txt", R.string.format_txt, R.string.format_txt_times),
        Format("srt", R.string.format_srt),
        Format("vtt", R.string.format_vtt),
        Format("md", R.string.format_md, R.string.format_md_times),
        Format("json", R.string.format_json),
    )

/**
 * One file at work and then its transcript (ui/index.html, #work): the header with the way back,
 * the name, and what can be done with it (at work only Stop; Copy and Share once it is done), the
 * strip, the speakers' switch and Find, a line for what went wrong, and the rows. While the
 * keyboard is up for Find and leaves the rows under the floor (160 dp withText) below the header,
 * the strip and the line under it as they were last shown, those make room for it: the field takes
 * the top line and the rows the rest, as a phone's find in page takes its top; the keyboard put
 * away, or the search closed, they come back (the three apps, Peter, 2026-11-01). On its side the
 * emulator's Pixel 5 has 120 dp above the keyboard, where the three took 235 and left the field out
 * of sight and the transcript not a row; upright, 499 leave the rows 250 at the default text size,
 * and at twice it 164, under the floor's 297. The emulator's 1280 by 800 dp tablet makes room too
 * on its side, at the default text size: 408 dp above the keyboard, where the three as shown leave
 * the rows 152; upright, at twice the text size, it keeps 598 dp of rows.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun Work(
    name: String,
    step: String,
    done: Double,
    /** While Android's decoder reads the recording, how far it has got; -1 where it says none. */
    read: Double = -1.0,
    duration: Double,
    peaks: ByteArray?,
    rows: List<Seg>,
    error: String?,
    why: String?,
    /** Locate…, where the recording that does not play can be found again */
    locate: (() -> Unit)?,
    stats: String?,
    pace: String?,
    times: Boolean,
    playable: Boolean,
    playing: Boolean,
    position: Double,
    follow: Follow,
    sound: Sound,
    chosen: Timebase,
    find: Finder,
    hits: List<Hit>,
    a: WorkActions,
    /** who said what, where the build offers it (Who.kt) */
    spk: SpeakersView = SpeakersView.NONE,
    /** with Only matches, the rows shown (Find.kt, shownRows); null for all */
    view: IntArray? = null,
) {
    val p = pk
    // the file's times written with its length's hours
    val base = chosen.copy(duration = duration)
    val running = step == "loading" || step == "reading" || step == "transcribing"
    val review = step == "done" || step == "stopped"
    // the heights of the header, the strip and the line under it as last
    // shown, which making room is measured by, so that their going cannot undo it
    var head by remember { mutableIntStateOf(0) }
    var strip by remember { mutableIntStateOf(0) }
    var line by remember { mutableIntStateOf(0) }
    // the screen's height, the keyboard's taken from it
    var tall by remember { mutableIntStateOf(0) }
    val floor = 160.dp.withText()
    // what the three leave the rows, less the rows' 16 dp above them
    val makeRoom =
        WindowInsets.isImeVisible &&
            find.typing &&
            review &&
            rows.isNotEmpty() &&
            tall > 0 &&
            with(LocalDensity.current) { (tall - head - strip - line).toDp() } - 16.dp < floor
    // 16 dp from the sides of a work under 600 dp wide, a phone's, and 24
    // from a wider one's, by its own width, beside the side or not
    // (design/README.md, Edges; Material's margin at medium and expanded
    // widths): it was 16 at every width (the layout audit, 2026-11-01)
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val edge = if (maxWidth < 600.dp) 16.dp else 24.dp
        // a phone's width, Material's compact: the names' chips give way to
        // Who's speaking as a dialog from a turn's name (WhoPicker)
        val compact = maxWidth < 600.dp
        Column(Modifier.fillMaxSize().onSizeChanged { tall = it.height }) {
            // the header's buttons in the middle of its 62 dp (app.css, .pk-head),
            // the way back's arrow on the cards' line, the screen's edge, its
            // button hung past it: the arrow's ink is 14 dp into its 40 (11 to
            // the 18 dp icon, and 3 in that), more in a larger button at a
            // larger text size (inkInset)
            if (!makeRoom)
                Head(
                    back =
                        if (running) null
                        else
                            ({
                                IconBtn(
                                    Icons.back,
                                    stringResource(R.string.back),
                                    a.back,
                                    Modifier.hang(start = inkInset(4f)),
                                    kind = Kind.Ghost,
                                )
                            }),
                    name = {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Text(
                                name,
                                style = Type.ui,
                                color = p.ink,
                                maxLines = 1,
                                overflow = TextOverflow.MiddleEllipsis,
                                modifier = Modifier.weight(1f, fill = false),
                            )
                            // until the ring has progress: reading, the model loading, the first
                            // chunk (Spinner)
                            if (running && !(step == "transcribing" && done > 0)) Spinner()
                        }
                    },
                    modifier =
                        Modifier.onSizeChanged { head = it.height }
                            .fillMaxWidth()
                            .heightIn(min = 62.dp)
                            .padding(start = edge, end = edge, top = 11.dp, bottom = 11.dp),
                ) {
                    if (running) PkButton(stringResource(R.string.stop), a.stop, kind = Kind.Ghost)
                    if (review && rows.isNotEmpty()) {
                        IconBtn(Icons.copy, stringResource(R.string.copy_text), a.copy)
                        ShareAs(name, times, a)
                    }
                }
            if (step != "error" && !makeRoom)
                // while the speakers are found, the ring in play's place and the waveform
                // filling, as while it was transcribed (app.js, showFinding; Peter,
                // 2026-11-14: a 3-hour file showed no progress)
                Strip(
                    peaks = peaks,
                    review = review && spk.heard == null,
                    done = spk.heard ?: if (step == "transcribing") done else 0.0,
                    playing = playing,
                    playable = playable,
                    position = position,
                    duration = duration,
                    left =
                        when {
                            spk.heard != null -> stringResource(R.string.speakers_finding)
                            review -> base.at(position)
                            step == "loading" -> stringResource(R.string.status_loading)
                            step == "reading" && read > 0 ->
                                stringResource(R.string.status_reading_at, (read * 100).toInt())
                            step == "reading" -> stringResource(R.string.status_reading)
                            else -> pace ?: ""
                        },
                    right = if (duration > 0) base.length() else "",
                    playLabel = stringResource(if (playing) R.string.pause else R.string.play),
                    onPlay = a.play,
                    onSeek = { a.jump(it * duration) },
                    // the transcript at the sentence under a scrubbing thumb
                    onScrub = { f ->
                        val r = rows.indexOfLast { it.start <= f * duration + 1e-3 }
                        follow.show(itemIn(view, maxOf(r, 0)))
                    },
                    sound = sound,
                    base = base,
                    onBase = a.setBase,
                    ticks =
                        if (review && duration > 0) hits.map { it.time / duration }
                        else emptyList(),
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
                    modifier =
                        Modifier.onSizeChanged { strip = it.height }
                            .padding(start = edge, end = edge, top = 5.dp),
                    why = why,
                    locate = locate,
                    bar =
                        if (spk.place && duration > 0)
                            ({ m ->
                                SpeakerBar(
                                    spk.bar,
                                    duration,
                                    spk.upto,
                                    m,
                                    finding = spk.heard != null,
                                    exact = find.who != 0 && spk.heard == null,
                                )
                            })
                        else null,
                )
            // while it transcribes with its speakers, the way to go on without them
            if (spk.running && !makeRoom)
                RunningSpeakers(spk, Modifier.padding(start = edge, end = edge, top = 16.dp))
            if (review && rows.isNotEmpty())
                // a step takes the playhead to its match (Finder.step)
                SpeakersLine(
                    find,
                    hits,
                    { t: Double -> a.seek(t, false) },
                    makeRoom,
                    Modifier.onSizeChanged { if (!makeRoom) line = it.height }
                        .padding(start = edge, end = edge, top = 16.dp),
                    spk,
                    a.only,
                    compact,
                )
            // with the names shown, each speaker's chip, which renames them (app.css, .pk-names),
            // where the screen is wider than a phone's
            if (review && rows.isNotEmpty() && spk.offered && spk.shown && !makeRoom && !compact)
                Chips(
                    spk.present,
                    spk.nameOf,
                    find::findWho,
                    find.who,
                    Modifier.padding(start = edge, end = edge, top = 8.dp),
                )
            val msg =
                when {
                    error != null -> error
                    step == "stopped" ->
                        stringResource(R.string.status_stopped_at, (done * 100).toInt())
                    step == "done" && rows.isEmpty() -> stringResource(R.string.no_speech)
                    else -> null
                }
            if (msg != null && !makeRoom)
                Row(
                    Modifier.padding(start = edge, end = edge, top = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    val bad = error != null
                    if (bad) Icon(Icons.alert, null, tint = p.err, modifier = Modifier.size(16.dp))
                    Text(
                        msg,
                        style = Type.body.copy(fontSize = 15.sp),
                        color = if (bad) p.err else p.inkMuted,
                    )
                }
            Rows(
                rows,
                review,
                playing,
                position,
                stats,
                base,
                hits,
                find.at,
                find.shown,
                follow,
                a,
                edge,
                Modifier.weight(1f),
                spk,
                compact,
                view,
            ) {
                if (find.typing) find.keys = Keys.Drop
            }
        }
    }
}

/**
 * The header's line: the way back at its start, what can be done with the file at its end, and the
 * name between them while it keeps the words' least there (wordsFloor), or its whole width where
 * that is less; else the name on a line of its own 16 dp under them, under the arrow's ink, and the
 * buttons stay where the thumb knows them. On a 393 dp phone the name has 187 dp beside Copy and
 * Share at the default text size, over the floor's 160, and 175 at 1.3 times it, under the floor's
 * 208, so it goes under them there (2026-10-31). Where even that line is too short for the name it
 * gives up its middle, which keeps its extension.
 */
@Composable
private fun Head(
    back: (@Composable () -> Unit)?,
    name: @Composable () -> Unit,
    modifier: Modifier,
    actions: @Composable () -> Unit,
) {
    val least = wordsFloor()
    Layout(
        {
            Box { back?.invoke() }
            Box { name() }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                actions()
            }
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
            // at the line's start, under the arrow's ink, which its button
            // hangs there (Work)
            val np = n.measure(loose.copy(maxWidth = c.maxWidth))
            val bar = maxOf(bp.height, kp.height, c.minHeight)
            // the screen's one step between its blocks (Work): flush under
            // the buttons, the name's line sat on Share's edge at 1.3 times
            // the text size on a fold's 337 dp cover (Peter, 2026-10-31)
            val step = 16.dp.roundToPx()
            layout(c.maxWidth, bar + step + np.height) {
                bp.place(0, (bar - bp.height) / 2)
                kp.place(c.maxWidth - kp.width, (bar - kp.height) / 2)
                np.place(0, bar + step)
            }
        }
    }
}

/**
 * The line under the strip: the speakers' switch, where the build offers a diarizer, and the search
 * right after it, 12 dp apart, one group of the transcript's controls (app.css, .pk-spk-line and
 * .pk-find): alone at the line's far end, a reader might not see that the transcript can be
 * searched (Peter, 2026-10-31). Its field, which says Find, where that fits beside the switch, the
 * desktop's 200 dp and 280 with a search in it, grown with the text (withText): at twice the text
 * size a tablet's 280 left "rabbit" 53 dp and scrolled its first letter out (2026-10-31). Else the
 * glass, which opens the field across the line until the search is let go, as a phone's find in
 * page does. Alone, while the header and the strip make room for the keyboard (Work): the field by
 * itself across the line, the same field as beside the switch, so that it keeps the keys and the
 * keyboard stays up as it moves.
 *
 * While something is found, Only matches after the field, as the desktop has it after the names and
 * Find (app.js, applyOnly): on the field's line where it fits, else on a line of its own 16 dp
 * under it, where a phone's field runs across the line. Not while the keyboard has the room
 * (alone), whose rows it would take; a search kept to its matches stays so.
 */
@Composable
private fun SpeakersLine(
    find: Finder,
    hits: List<Hit>,
    seek: (Double) -> Unit,
    alone: Boolean,
    modifier: Modifier,
    spk: SpeakersView,
    onOnly: () -> Unit = {},
    /** a phone's width: the switch, while names are shown, opens SpeakersMenu */
    compact: Boolean = false,
) {
    // the switch pressed while names are shown or being found: on with none
    // found yet, it finds them (MainActivity.toggleSpeakers)
    val pressed = spk.on && (spk.present.isNotEmpty() || spk.busy)
    val menu = compact && pressed && spk.present.isNotEmpty() && !spk.busy
    var menuOpen by remember { mutableStateOf(false) }
    val state =
        stringResource(
            when {
                !pressed -> R.string.speakers_show
                spk.present.isNotEmpty() -> R.string.speakers_hide
                else -> R.string.speakers_show
            }
        )
    val label = stringResource(R.string.find_label)
    val opened = find.open || find.query.isNotBlank()
    val sought = find.query.isNotBlank() || find.who != 0
    val only = stringResource(R.string.find_only)
    val onlyState =
        stringResource(if (find.only) R.string.find_only_all else R.string.find_only_show)
    // wider with a search, for its count and arrows, and with a speaker
    // found for their name as well (FindField)
    val width =
        (when {
                find.who != 0 -> 380.dp
                find.query.isNotBlank() -> 280.dp
                else -> 200.dp
            })
            .withText()
    val gapDp = 12.dp
    SubcomposeLayout(modifier.fillMaxWidth()) { c ->
        val gap = gapDp.roundToPx()
        val loose = c.copy(minWidth = 0, minHeight = 0)
        // the switch, where the build offers a diarizer
        val v =
            if (spk.offered)
                subcompose("switch") {
                        Box {
                            SpeakersSwitch(
                                pressed,
                                spk.busy,
                                state,
                                if (menu) ({ menuOpen = true }) else spk.toggle,
                                menu = menu,
                            )
                            SpeakersMenu(menu && menuOpen, { menuOpen = false }, spk, find.who)
                        }
                    }
                    .first()
                    .measure(loose.copy(maxWidth = c.maxWidth - gap - 40.dp.roundToPx()))
            else null
        val field = width.roundToPx()
        val beside = !alone && (v?.let { it.width + gap } ?: 0) + field <= c.maxWidth
        // one field at either width, which keeps the keys when a search
        // makes it too wide to stay beside the switch, or the keyboard needs the room
        val f =
            if (beside || opened || alone) {
                val w = if (beside) field else c.maxWidth
                subcompose("field") {
                        FindField(
                            find,
                            hits,
                            seek,
                            find::clear,
                            nameOf = spk.nameOf,
                            whoInside = !compact,
                        )
                    }
                    .first()
                    .measure(loose.copy(minWidth = w, maxWidth = w))
            } else
                subcompose("glass") { IconBtn(Icons.search, label, find::focus, kind = Kind.Ghost) }
                    .first()
                    .measure(loose)
        val sw = v?.takeIf { beside || !(opened || alone) }
        // Only matches, after the field or under it
        val o =
            if (sought && !alone)
                subcompose("only") {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(gapDp),
                        ) {
                            // a phone's speaker found, out of the field (FindField, whoInside), in
                            // a
                            // sunk track as tall as the switch beside it
                            if (compact && find.who != 0)
                                Box(
                                    Modifier.background(pk.sunk, Corner)
                                        .height(40.dp)
                                        .padding(horizontal = 6.dp),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    var picking by remember { mutableStateOf(false) }
                                    WhoBadge(find, spk.nameOf(find.who), true) { picking = true }
                                    SpeakersMenu(picking, { picking = false }, spk, find.who)
                                }
                            SpeakersSwitch(find.only, false, onlyState, onOnly, only)
                        }
                    }
                    .first()
                    .measure(loose)
            else null
        val end = (if (sw != null) sw.width + gap else 0) + f.width
        val inline = o != null && end + gap + o.width <= c.maxWidth
        val top = maxOf(sw?.height ?: 0, f.height, if (inline) o!!.height else 0)
        val under = 16.dp.roundToPx()
        val h = top + if (o != null && !inline) under + o.height else 0
        layout(c.maxWidth, h) {
            sw?.place(0, (top - sw.height) / 2)
            f.place(if (sw != null) sw.width + gap else 0, (top - f.height) / 2)
            if (o != null)
                if (inline) o.place(end + gap, (top - o.height) / 2) else o.place(0, top + under)
        }
    }
}

/**
 * Share, the plume button a finished transcript is for, and its menu of formats, the desktop's Save
 * as's: each sends the transcript as a file of its kind to the app picked in the system's share
 * sheet, which has Save file too (MainActivity.share). Where the desktop's button saves, a phone's
 * shares: on one a transcript goes to another app, Drive or a chat or mail, far more often than to
 * a folder. Ctrl+S still saves, where there are keys.
 */
@Composable
private fun ShareAs(name: String, times: Boolean, a: WorkActions) {
    var open by remember { mutableStateOf(false) }
    val stem = name.substringBeforeLast('.').ifEmpty { name }
    Box {
        PkButton(
            stringResource(R.string.share),
            { open = true },
            kind = Kind.Primary,
            trailing = Icons.down,
            held = open,
        )
        Menu(open, { open = false }) {
            for (f in FORMATS) Item(
                stringResource(if (times) f.withTimes else f.label),
                "$stem.${f.ext}",
            ) {
                open = false
                a.share(f.ext)
            }
            MenuLine()
            Check(stringResource(R.string.save_times), times, a.setTimes)
        }
    }
}

/**
 * The transcript's rows (app.css, .pk-seg): each sentence's time, then its text, at most 560 dp
 * wide; the time above the text under 620 dp, as the desktop's window has it (app.css, max-width:
 * 619px), else beside it in a column as wide as the file's times. The row playing on the surface
 * and the word playing underlined in it; a row, tapped anywhere, plays from its time. The fingers
 * on the list are counted for what moves it besides the reader (Follow).
 */
@Composable
private fun Rows(
    rows: List<Seg>,
    review: Boolean,
    playing: Boolean,
    position: Double,
    stats: String?,
    base: Timebase,
    hits: List<Hit>,
    at: Int,
    shown: Int,
    follow: Follow,
    a: WorkActions,
    /** the work's edge (Work): the rows' boxes stand on it */
    edge: Dp,
    modifier: Modifier,
    spk: SpeakersView,
    /** a phone's width: Who's speaking as a dialog (WhoPicker) */
    compact: Boolean,
    /**
     * With Only matches, the rows shown (Find.kt, shownRows); null for all. The list's items are
     * then those rows alone, the n-th item the n-th of them (MainActivity, itemOf).
     */
    view: IntArray? = null,
    /** a finger on the list: the Find field lets go of the keys (Finder.typing) */
    letGo: () -> Unit = {},
) {
    val p = pk
    val current =
        if (review && (playing || position > 0)) rows.indexOfLast { it.start <= position + 1e-3 }
        else -1
    // the row playing out of sight while it plays, the reader having scrolled away from it: which
    // way it is, up (-1) or down (1), for the Now button; 0 in sight or not playing
    val nowItem = itemIn(view, current)
    val away by
        remember(nowItem, playing) {
            derivedStateOf {
                val seen = follow.list.layoutInfo.visibleItemsInfo
                when {
                    !playing || nowItem < 0 || seen.isEmpty() -> 0
                    seen.any { it.index == nowItem } -> 0
                    nowItem < seen.first().index -> -1
                    else -> 1
                }
            }
        }
    val scope = rememberCoroutineScope()
    // each row's matches, by their place among all of them
    val marks = remember(hits) { hits.withIndex().groupBy { it.value.row } }
    // the text selected, if any (SegRow)
    val selection = rememberSelectionState()
    // the list from one gap under the line under the strip, its rows scrolling under
    // that edge, not up to the line, where the playing row's card had met the
    // search's field (Peter, the S25, 2026-10-27)
    // the times' column as wide as the file's times, the desktop's (app.css,
    // .pk-seg): all of a file's as long as its length's, which stamp gives
    // the hours of the file's length, measured at the reader's text size
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val times =
        remember(base, measurer, density) {
            with(density) { measurer.measure(base.at(base.duration), figures).size.width.toDp() }
        }
    // the list's bar, its thumb to drag on a long transcript (Scrollbar.kt)
    val bar = rememberBar(follow.list)
    // each row's key its time (the desktop's data-t), so that the list keeps
    // the rows in sight where they are as sentences above them are made one
    // again (Speakers.rejoin), and those after one slide up into its place
    // (animateItem); a second row at one time, which pktito-c's sentences
    // never have, keyed apart, for a key used twice stops the list
    val keys by
        remember(rows) {
            derivedStateOf {
                val seen = HashMap<Double, Int>()
                rows.map { r ->
                    val n = (seen[r.start] ?: 0) + 1
                    seen[r.start] = n
                    if (n == 1) r.start else "${r.start}/$n"
                }
            }
        }
    // a sentence made one again above the first row in sight, the row moved
    // having been that first, its key gone with it: the sentence in sight,
    // where the row was, not the one after it
    val joined = Speakers.joined
    // Only matches turned: the reader's place, now that the rows have changed (Follow.keep)
    SideEffect { follow.take() }
    LaunchedEffect(joined) {
        val row = joined?.let { j -> rows.indexOfFirst { it === j.seg } } ?: -1
        // its place in the list, with Only matches among the rows shown
        val at = itemIn(view, row)
        if (at >= 0 && at < follow.list.firstVisibleItemIndex) follow.list.requestScrollToItem(at)
    }
    // the rows' slide (animateItem, below) for the join's own change of the
    // rows alone, half a second from it: left on, it ran on the list's own
    // scroll to the row playing too, which a moment after landing at the top
    // dropped rows down and slid them back up over the next (Peter's S25
    // recording, 2026-11-18; the S24, gone with the slide off)
    var slid by remember { mutableStateOf<Speakers.Joined?>(null) }
    LaunchedEffect(joined) {
        delay(500)
        slid = joined
    }
    val slide = joined != null && slid !== joined
    // Change speaker in the menu a press-and-hold's selection brings, beside
    // Copy: Android's and iOS's way in to a sentence, where the desktop has
    // its button on the line, as the transcript's text is there to be
    // selected and copied (the write-up's pick, which Peter took,
    // 2026-11-15). Of the sentence the finger last came down on, where the
    // hold began the selection; Who's speaking for it alone, as a dialog.
    val pressed = remember { intArrayOf(-1) }
    var alone by remember { mutableStateOf<Speakers.Turn?>(null) }
    val change = stringResource(R.string.speakers_change)
    val menu =
        if (review && spk.shown && spk.editable)
            Modifier.appendTextContextMenuComponents {
                val r = pressed[0]
                val s = rows.getOrNull(r)
                val n = s?.speaker
                if (s != null && n != null)
                    item(CHANGE_SPEAKER, change) {
                        close()
                        selection.clear()
                        alone = Speakers.Turn(r, r, n, s.start, s.end)
                    }
            }
        else Modifier
    alone?.let { u -> WhoPicker(true, true, { alone = null }, u, spk) }
    // the first row's text 16 dp under the line above, past the row's own 6
    // (app.css, .pk-transcript; the spacing audit, 2026-11-15: it was 24)
    BoxWithConstraints(
        modifier.fillMaxWidth().padding(top = 10.dp).scrollbar(bar).then(menu).pointerInput(Unit) {
            awaitEachGesture {
                awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                letGo()
            }
        }
    ) {
        // a column from 620 dp with an hour's times at the default text size,
        // the desktop's (app.css, max-width: 619px), where the text keeps 416
        // beside them: the list's 24 dp and the row's 16 either side, and the
        // gutter; further out as the times grow with the text
        val column = times.takeIf { maxWidth >= 496.dp + GUTTER + it }
        SelectionContainer(selection) {
            LazyColumn(
                Modifier.hands(follow),
                state = follow.list,
                contentPadding = PaddingValues(start = edge, end = edge, bottom = 16.dp),
            ) {
                // with Only matches and nothing found, where the sentences would be
                if (view != null && view.isEmpty())
                    item(key = "none") {
                        Text(
                            stringResource(R.string.find_only_none),
                            style = Type.ui.copy(fontWeight = FontWeight.W400),
                            color = p.inkMuted,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp),
                        )
                    }
                items(view?.size ?: rows.size, key = { k -> keys[view?.get(k) ?: k] }) { k ->
                    val i = view?.get(k) ?: k
                    val s = rows[i]
                    val now = i == current
                    val word = if (now) s.words.indexOfLast { it.start <= position + 1e-3 } else -1
                    // with Only matches, a run of the rows shown stands apart as a
                    // turn does, its line ending inside its first and last, and a
                    // run from inside a turn has the turn's name (app.js, applyOnly)
                    val from = view != null && (k == 0 || view[k - 1] != i - 1)
                    val to = view != null && (k == view.lastIndex || view[k + 1] != i + 1)
                    // with the turns shown, a turn's name opens its first sentence
                    // (a sentence no speaker was heard in has none)
                    val turn =
                        if (
                            spk.turns && s.speaker != null && (Speakers.turnStarts(rows, i) || from)
                        ) {
                            var a = i
                            while (a > 0 && rows[a - 1].speaker == s.speaker) a--
                            Speakers.turnAt(rows, a)
                        } else null
                    // and each row has its turn's line, ending inside its first and last
                    val line =
                        s.speaker
                            ?.takeIf { spk.turns }
                            ?.let {
                                TurnLine(
                                    it,
                                    Speakers.turnStarts(rows, i) || from,
                                    i == rows.lastIndex || rows[i + 1].speaker != it || to,
                                )
                            }
                    // the rows after a sentence made one again sliding up into its
                    // place over 150 ms, the desktop's (app.js, closeUp)
                    Box(
                        Modifier.animateItem(
                                null,
                                if (slide) tween(150, easing = EaseOut) else null,
                                null,
                            )
                            .pointerInput(i) {
                                // the row a finger comes down on, for Change speaker
                                awaitEachGesture {
                                    awaitFirstDown(
                                        requireUnconsumed = false,
                                        pass = PointerEventPass.Initial,
                                    )
                                    pressed[0] = i
                                }
                            }
                    ) {
                        SegRow(
                            s,
                            now,
                            word,
                            marks[i],
                            at,
                            shown,
                            column,
                            review,
                            base,
                            selection,
                            turn,
                            line,
                            spk,
                            compact,
                            k == 0,
                            from,
                        ) {
                            a.seek(s.start, true)
                        }
                    }
                }
                // what the whole transcript cost says nothing of the matches shown
                if (stats != null && view == null)
                    item {
                        Text(
                            stats,
                            style = Type.small.copy(fontSize = 12.sp, fontFeatureSettings = "tnum"),
                            color = p.inkMuted,
                            modifier =
                                Modifier.padding(
                                    start = if (column != null) 16.dp + column + GUTTER else 16.dp,
                                    top = 16.dp,
                                ),
                        )
                    }
            }
        }
        if (away != 0)
            NowButton(
                away < 0,
                base.at(position),
                // over a toast at the screen's foot (its 24 and 48, and 12 between), not under it
                Modifier.align(Alignment.BottomCenter)
                    .padding(bottom = if (spk.lifted) 84.dp else 16.dp),
            ) {
                scope.launch { follow.back(nowItem) }
            }
    }
}

/**
 * Back to the row playing, for a reader who has scrolled away from it while it plays (Peter,
 * 2026-11-18: "I find myself getting lost"): over the foot of the transcript, an arrow the way the
 * row is and the playhead's time, in the toast's floating box. The time says what the arrow points
 * at where no icon says "now". A touch brings the row to the top and follows it again
 * (Follow.back).
 */
@Composable
private fun NowButton(up: Boolean, time: String, modifier: Modifier, onClick: () -> Unit) {
    val p = pk
    val label = stringResource(R.string.follow_back, time)
    Row(
        modifier
            .shadow(12.dp, CircleShape)
            .border(1.dp, p.menuEdge, CircleShape)
            .control(CircleShape, p.ground, label = label, onClick = onClick)
            .heightIn(min = 40.dp)
            .padding(start = 12.dp, end = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(
            if (up) Icons.arrowUp else Icons.arrowDown,
            null,
            tint = p.ink,
            modifier = Modifier.size(16.dp),
        )
        Text(
            time,
            style = Type.stamp.copy(fontSize = 13.sp, fontFeatureSettings = "tnum"),
            color = p.ink,
            maxLines = 1,
        )
    }
}

@Composable
private fun SegRow(
    s: Seg,
    now: Boolean,
    word: Int,
    marks: List<IndexedValue<Hit>>?,
    at: Int,
    shown: Int,
    column: Dp?,
    review: Boolean,
    base: Timebase,
    selection: SelectionState,
    /** the turn this row starts, if any: its name opens the text (Sentence) */
    turn: Speakers.Turn?,
    /** its turn's line, with the turns shown */
    turnLine: TurnLine?,
    spk: SpeakersView,
    /** Who's speaking as a dialog (WhoPicker) */
    dialog: Boolean,
    first: Boolean,
    /**
     * with Only matches, the first of a run of the rows shown, which stands apart as a turn does
     */
    apart: Boolean,
    onTime: () -> Unit,
) {
    val p = pk
    // its time. The row playing is its ground alone: the beak's square after
    // its time went, as on the desktop, where beside the speakers' marks it
    // read as one of them (Peter, 2026-11-14), and with the turn's line
    // beside it here.
    val time =
        @Composable { mod: Modifier ->
            Text(
                base.at(s.start),
                style = figures.copy(lineHeight = 20.sp),
                color = p.inkMuted,
                modifier = mod,
            )
        }
    // the whole row plays from its time, its words as well as the time, which
    // a thumb is too broad to find (Peter, the S25, 2026-10-26); but not a
    // tap that lets go of a selection of the text. The selection is gone by
    // the time the tap is, so whether there was one is taken as the finger
    // comes down, before the text's own gestures see it.
    val selecting = remember { booleanArrayOf(false) }
    val box =
        Modifier.widthIn(max = 716.dp)
            .fillMaxWidth()
            .background(if (now) p.surface else Color.Transparent, Corner)
            .then(
                if (review)
                    Modifier.pointerInput(selection) {
                            awaitEachGesture {
                                awaitFirstDown(
                                    requireUnconsumed = false,
                                    pass = PointerEventPass.Initial,
                                )
                                selecting[0] = selection.selectedTexts.isNotEmpty()
                            }
                        }
                        .clickable(
                            remember { MutableInteractionSource() },
                            null,
                            onClickLabel = stringResource(R.string.play_from, base.at(s.start)),
                        ) {
                            if (!selecting[0]) onTime()
                        }
                else Modifier
            )
    // a turn 16 dp under the one before (app.css, .pk-seg.pk-turn): with the
    // rows' 6 dp, sentences 12 apart and turns 28 (Peter, 2026-11-15: at 8, a
    // touch loose)
    val gap = if ((turn != null || apart) && !first) Modifier.padding(top = 16.dp) else Modifier
    // its turn's line in the middle of the gutter, past the row's 16 dp and
    // the times; with the time over the text, 5 dp in, as the desktop's
    // narrow window has it
    val ruled =
        turnLine?.let {
            Modifier.turnLine(
                it,
                p.speaker(it.speaker),
                if (column != null) 16.dp + column + GUTTER / 2 else 6.5.dp,
            )
        } ?: Modifier
    if (column != null)
        // a row of one line 48 dp tall at the least, Android's least for a
        // finger, its line in the middle: it was 44, iOS's 44 pt (the layout
        // audit, 2026-11-01). With its time over it, a row has two lines.
        Row(
            gap.then(box)
                .then(ruled)
                .heightIn(min = 48.dp)
                .wrapContentHeight()
                .padding(horizontal = 16.dp, vertical = 6.dp)
        ) {
            time(Modifier.width(column).alignByBaseline())
            Spacer(Modifier.width(GUTTER))
            Sentence(
                s,
                word,
                marks,
                at,
                shown,
                Modifier.weight(1f).alignByBaseline(),
                turn,
                spk,
                dialog,
            )
        }
    else
    // the time over the text
    Column(
            gap.then(box).then(ruled).padding(horizontal = 16.dp, vertical = 6.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            time(Modifier)
            Sentence(s, word, marks, at, shown, Modifier, turn, spk, dialog)
        }
}

/**
 * A row's text in the transcript's type, the word playing underlined in the beak's colour, 2 dp, 4
 * below the line; and a search's matches marked (app.css, .pk-hit): each on plume mixed 45% into
 * the ground, as tall as its letters and 2 dp round them, its corners 3 dp; the one the reader is
 * on in plume, its letters on-plume, an ink bar 2 dp under it, for a state shown by more than its
 * colour (the tokens' contrast, 2026-10-22). Once the list has brought a move's row into sight
 * (Finder.show), the match's own line too, and the row's 6 dp round it: a row taller than the room,
 * above the keyboard (Work) or a long one, had its top in sight and the match below.
 */
@Composable
private fun Sentence(
    s: Seg,
    word: Int,
    marks: List<IndexedValue<Hit>>?,
    at: Int,
    shown: Int,
    modifier: Modifier,
    /** the turn it starts, whose name opens it (speakerName), and Who's speaking from that */
    turn: Speakers.Turn? = null,
    spk: SpeakersView = SpeakersView.NONE,
    dialog: Boolean = false,
) {
    val p = pk
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    var who by remember { mutableStateOf(false) }
    val span = remember(s, word) { if (word < 0) null else wordSpan(s, word) }
    val on = marks?.firstOrNull { it.index == at }?.value
    val name = turn?.let { spk.nameOf(it.speaker) }
    // a touched or hovered name's box, the desktop's (app.css, .pk-speaker:hover)
    val links =
        TextLinkStyles(
            focusedStyle = SpanStyle(background = p.tint),
            hoveredStyle = SpanStyle(background = p.tint),
            pressedStyle = SpanStyle(background = p.tintPress),
        )
    val editable = spk.editable
    // the sentence's own text after the name, where one opens it: lead
    // characters on, for the search's marks and the word playing
    val (text, lead) =
        remember(s, on, p, name, editable) {
            var lead = 0
            val t = buildAnnotatedString {
                if (name != null) lead = speakerName(name, links) { if (editable) who = true }
                append(s.text)
                // a turn of a speaker found: its name, the colon with it
                if (on != null && on.start < 0 && lead > 1)
                    addStyle(SpanStyle(color = p.onPlume), 0, lead - 1)
                else if (on != null && on.start >= 0 && on.end <= s.text.length)
                    addStyle(SpanStyle(color = p.onPlume), lead + on.start, lead + on.end)
            }
            t to lead
        }
    val hl = mix(p.ground, p.plume, 0.45f)
    // a sentence a move made one again: the words joined to it lit in their
    // speaker's colour, 30% of it fading out over 1.6 s (app.css,
    // .pk-joined), from where it has got to if the row comes back into sight
    // meanwhile; not at all where animations are off
    val joined = Speakers.joined?.takeIf { it.seg === s }
    val lit = remember(joined) { Animatable(0f) }
    LaunchedEffect(joined) {
        val j = joined ?: return@LaunchedEffect
        if (coroutineContext[MotionDurationScale]?.scaleFactor == 0f) return@LaunchedEffect
        val left = 1f - (SystemClock.uptimeMillis() - j.at) / 1600f
        if (left <= 0f) return@LaunchedEffect
        lit.snapTo(left)
        lit.animateTo(0f, tween((1600 * left).toInt(), easing = EaseOut))
    }
    val sight = remember { BringIntoViewRequester() }
    val pad = with(LocalDensity.current) { 6.dp.toPx() }
    // what was shown before this row came onto the list is not this row's to show
    val first = remember { shown }
    LaunchedEffect(shown) {
        if (shown == first || on == null || on.end > s.text.length) return@LaunchedEffect
        val l = snapshotFlow { layout }.filterNotNull().first()
        // a turn's name opens the text
        val a = if (on.start < 0) 0 else lead + on.start
        val b = if (on.start < 0) maxOf(0, lead - 2) else lead + maxOf(on.start, on.end - 1)
        val top = l.getLineTop(l.getLineForOffset(a))
        val bottom = l.getLineBottom(l.getLineForOffset(b))
        sight.bringIntoView(Rect(0f, top - pad, l.size.width.toFloat(), bottom + pad))
    }
    Box(modifier) {
        Text(
            text,
            style = Type.transcript,
            color = p.ink,
            onTextLayout = { layout = it },
            modifier =
                Modifier.bringIntoViewRequester(sight).drawBehind {
                    val l = layout ?: return@drawBehind
                    val v = lit.value
                    if (joined != null && v > 0f)
                        wordSpan(s, joined.words)?.let { (a, _) ->
                            val fill = p.speaker(joined.speaker).copy(alpha = 0.3f * v)
                            mark(l, lead + a, lead + s.text.length, fill, null)
                        }
                    for (m in marks.orEmpty()) {
                        val current = m.index == at
                        // a turn of a speaker found (turnsOf): its name and colon
                        val name = m.value.start < 0
                        mark(
                            l,
                            if (name) 0 else lead + m.value.start,
                            if (name) lead - 1 else lead + m.value.end,
                            if (current) p.plume else hl,
                            if (current) p.ink else null,
                        )
                    }
                    val (a, b) = span?.let { (a, b) -> lead + a to lead + b } ?: return@drawBehind
                    if (b <= a || b > l.layoutInput.text.length) return@drawBehind
                    val line = l.getLineForOffset(a)
                    val y = l.getLineBaseline(line) + 4.dp.toPx()
                    val x0 = l.getHorizontalPosition(a, true)
                    val x1 =
                        if (l.getLineForOffset(b - 1) == line) l.getHorizontalPosition(b, true)
                        else l.getLineRight(line)
                    drawLine(p.beak, Offset(x0, y), Offset(x1, y), 2.dp.toPx())
                },
        )
        // Who's speaking under the name's line, where the name opens the text
        if (turn != null) {
            val y = layout?.getLineBottom(0)?.roundToInt() ?: 0
            Box(Modifier.offset { IntOffset(0, y) }) {
                WhoPicker(who, dialog, { who = false }, turn, spk)
            }
        }
    }
}

/**
 * A match's mark behind the letters from a to b, on each line they are on: their height, from
 * Archivo's ascent to its descent (0.878 and 0.21 em, its hhea), and 2 dp round them, as an inline
 * mark's box is drawn on the desktop; under, an ink bar 2 dp under it, the width of the letters.
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
        val x1 =
            if (l.getLineForOffset(s1) == line) l.getHorizontalPosition(s1, true)
            else l.getLineRight(line)
        val base = l.getLineBaseline(line)
        val top = base - 0.878f * em - round
        val bottom = base + 0.21f * em + round
        drawRoundRect(
            fill,
            Offset(x0 - round, top),
            Size(x1 - x0 + 2 * round, bottom - top),
            CornerRadius(3.dp.toPx()),
        )
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
