package net.pktito.app.ui

import android.view.ViewConfiguration
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ScrollIndicatorState
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest

/**
 * A scroll's bar, as Android's views draw theirs and Compose draws none (foundation 1.12 gives a
 * scroll's place, ScrollIndicatorState, and nothing that shows it; iOS's lists show the system's,
 * and the desktop's the browser's): a thin bar while the list moves, gone a moment after, at the
 * end edge. On a scroll of four screens or more, where Android's lists take a fast scroller
 * (AbsListView's FastScroller, MIN_PAGES), a thumb a finger can take instead, and drag to anywhere
 * in it: a three-hour book is 1709 sentences (a Galaxy S26 Ultra, 2026-11-04), a long way to flick.
 */
class Bar
internal constructor(
    private val state: ScrollIndicatorState,
    private val moving: () -> Boolean,
    private val to: suspend (Float) -> Unit,
) {
    /** The thumb's place under a finger, 0 to 1 of its travel; null while none holds it. */
    internal var held by mutableStateOf<Float?>(null)
    internal val shown = Animatable(0f)

    private val travel: Int
        get() = state.contentSize - state.viewportSize

    /** Four screens or more: a thumb to take, not only a bar. */
    internal val long: Boolean
        get() = state.viewportSize > 0 && state.contentSize >= 4 * state.viewportSize

    /** Where the scroll is, 0 to 1 of its travel; null where it all fits. */
    internal val place: Float?
        get() =
            if (state.viewportSize <= 0 || travel <= 0) null
            else held ?: (state.scrollOffset.toFloat() / travel).coerceIn(0f, 1f)

    /** The bar's length on a track of height px. */
    internal fun length(height: Float, d: Density): Float =
        with(d) {
            if (long) THUMB_LONG.toPx()
            else
                (height * state.viewportSize / state.contentSize)
                    .coerceAtLeast(SHORTEST.toPx())
                    .coerceAtMost(height)
        }

    /** The scroll moved, and the bar shown, for as long as it moves or is held. */
    internal suspend fun run(hold: Long, longHold: Long, fade: Int) {
        snapshotFlow { moving() || held != null }
            .collectLatest { busy ->
                if (busy) shown.snapTo(1f)
                else {
                    delay(if (long) longHold else hold)
                    shown.animateTo(0f, tween(fade))
                }
            }
    }

    /** The scroll taken to where the thumb is held. */
    internal suspend fun follow() {
        snapshotFlow { held }.collectLatest { f -> if (f != null) to(f * travel) }
    }
}

/**
 * A lazy list's bar. Compose measures only the rows near the screen, and its offset and length
 * are estimates from those (calculateContentSize); a thumb dragged to a place goes to the row the
 * same estimate puts there.
 */
@Composable
fun rememberBar(list: LazyListState): Bar =
    remember(list) {
        // a list's and a column's always have one; a ScrollableState may not
        Bar(requireNotNull(list.scrollIndicatorState), { list.isScrollInProgress }) { px ->
            val rows = list.layoutInfo.visibleItemsInfo
            if (rows.isNotEmpty()) {
                val first = rows.first()
                val last = rows.last()
                val each = (last.offset + last.size - first.offset).toFloat() / rows.size
                val i = (px / each).toInt().coerceIn(0, list.layoutInfo.totalItemsCount - 1)
                list.scrollToItem(i, (px - i * each).toInt())
            }
        }
    }

/** A column's bar, its length known. */
@Composable
fun rememberBar(scroll: ScrollState): Bar =
    remember(scroll) {
        Bar(requireNotNull(scroll.scrollIndicatorState), { scroll.isScrollInProgress }) { px ->
            scroll.scrollTo(px.toInt())
        }
    }

/**
 * The bar drawn over the content at its end edge, and its thumb taken by a finger before the
 * content sees it, as Follow's hands counts it (Follow.kt): a list dragged by its thumb is the
 * reader's, and nothing pulls it back to the row playing. Android's own timings: the bar gone
 * ViewConfiguration's default delay after a scroll, in its fade; the thumb 1.5 s after, as
 * FastScroller's.
 */
@Composable
fun Modifier.scrollbar(bar: Bar): Modifier {
    val p = pk
    val context = LocalContext.current
    val thin = remember(context) { ViewConfiguration.get(context).scaledScrollBarSize.toFloat() }
    val dir = LocalLayoutDirection.current
    LaunchedEffect(bar) {
        bar.run(
            ViewConfiguration.getScrollDefaultDelay().toLong(),
            1500,
            ViewConfiguration.getScrollBarFadeDuration(),
        )
    }
    LaunchedEffect(bar) { bar.follow() }
    return this.pointerInput(bar, dir) {
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                val w = size.width.toFloat()
                val h = size.height.toFloat()
                val at = thumb(bar, w, h, thin, this, dir)
                val start = bar.place
                if (!bar.long || bar.shown.value == 0f || start == null) return@awaitEachGesture
                // the thumb's whole width of reach, a finger's, and a little above and below
                val reach = GRAB.toPx()
                val x = down.position.x
                val nearX = if (dir == LayoutDirection.Rtl) x <= reach else x >= w - reach
                val nearY = down.position.y in (at.top - reach / 4)..(at.bottom + reach / 4)
                if (!nearX || !nearY) return@awaitEachGesture
                down.consume()
                val run = h - at.height
                val grip = down.position.y - at.top
                bar.held = start
                try {
                    while (true) {
                        val e = awaitPointerEvent(PointerEventPass.Initial)
                        val c = e.changes.firstOrNull { it.id == down.id } ?: break
                        c.consume()
                        if (!c.pressed) break
                        if (run > 0) bar.held = ((c.position.y - grip) / run).coerceIn(0f, 1f)
                    }
                } finally {
                    bar.held = null
                }
            }
        }
        .drawWithContentAndBar(bar, thin, p.inkMuted, p.ink)
}

private fun Modifier.drawWithContentAndBar(bar: Bar, thin: Float, muted: Color, held: Color) =
    drawWithContent {
            drawContent()
            val a = bar.shown.value
            if (a > 0f && bar.place != null) {
                val r = thumb(bar, size.width, size.height, thin, this, layoutDirection)
                val color =
                    when {
                        bar.held != null -> held
                        bar.long -> muted
                        else -> muted.copy(alpha = 0.5f)
                    }
                drawRoundRect(
                    color,
                    r.topLeft,
                    r.size,
                    CornerRadius(r.width / 2),
                    alpha = a * color.alpha,
                )
            }
        }

/** The bar's or thumb's rectangle on content of width w and height h, at its end edge. */
private fun thumb(bar: Bar, w: Float, h: Float, thin: Float, d: Density, dir: LayoutDirection): Rect =
    with(d) {
        val f = bar.place ?: 0f
        val width = if (bar.long) THUMB_WIDE.toPx() else thin
        val inset = INSET.toPx()
        val length = bar.length(h, d)
        val top = f * (h - length)
        val left = if (dir == LayoutDirection.Rtl) inset else w - inset - width
        Rect(Offset(left, top), Size(width, length))
    }

/** the shortest a bar gets, on a long scroll that takes no thumb */
private val SHORTEST = 24.dp
/** the thumb: a finger's length, and wider than the bar */
private val THUMB_LONG = 48.dp
private val THUMB_WIDE = 8.dp
/** from the end edge */
private val INSET = 2.dp
/** a finger's reach across the edge, where the thumb is taken */
private val GRAB = 48.dp

/** verticalScroll with its bar: the bar on the viewport, before the scroll in the chain. */
@Composable
fun Modifier.scrolls(): Modifier {
    val s = rememberScrollState()
    return scrollbar(rememberBar(s)).verticalScroll(s)
}
