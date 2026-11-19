package net.pktito.app.ui

import androidx.compose.foundation.MutatePriority
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput

/**
 * The transcript's list and what moves it besides the reader (MainActivity): the transcript's end
 * as it comes, for a reader who is there; the row playing, for a reader still with it, whose row
 * before is in sight or who has just pressed play, as the desktop's (app.js, showTime); and a seek
 * on the waveform, whose row comes wherever the reader is. None of it while a finger is on the list
 * or the fling it left still moves it: the list pulled to the row playing at each new one, under a
 * reader scrolling to a row to tap, lost the playhead after a few such taps (Peter, the iPhone 16e,
 * 2026-11-01; Android's list followed as iOS's did). A reader who has scrolled away is left there,
 * and taken along again from the next row once theirs is back in sight.
 */
class Follow(val list: LazyListState) {
    /** Fingers on the list (Modifier.hands); one coming down ends a chase. */
    var fingers = 0
        set(n) {
            field = n
            if (n > 0) chasing = -1
        }

    /** Scrolls of ours under way (scroll), which hands does not take for a fling. */
    private var ours = 0
    /** The row playing at the last change, -1 for none (playing). */
    private var was = -1
    /**
     * A row a scroll of ours set out for and has not reached, -1 for none: the next row playing,
     * which cuts that scroll short, is followed as for a reader still with it.
     */
    private var chasing = -1

    /** A finger on the list, or the fling it left: the reader moving it. */
    val hands: Boolean
        get() = fingers > 0 || (list.isScrollInProgress && ours == 0)

    /** A scroll of the list's own, which hands does not take for the reader's. */
    suspend fun scroll(block: suspend LazyListState.() -> Unit) {
        ours++
        try {
            list.block()
        } finally {
            ours--
        }
    }

    /**
     * Row at the list's top, where it isn't in sight above the list's last row, which shows only in
     * part; not under the reader's hand. Compose lands a far row exactly, the rows between measured
     * as it goes, where iOS's list can stop short and tries again.
     */
    suspend fun bring(row: Int) {
        if (row < 0 || hands) return
        if (list.layoutInfo.visibleItemsInfo.dropLast(1).any { it.index == row }) {
            chasing = -1
            return
        }
        chasing = row
        scroll { animateScrollToItem(row) }
        chasing = -1
    }

    /**
     * Only matches turned (Finder.only), the list's rows going from was to now, each the rows shown
     * in order, or null for all (shownRows): the reader kept on their sentence, where it is on the
     * screen, as the rows around it come and go (app.js, the switch's click; iOS's Rows.keep). The
     * match they are on (hit, a row), where it is in sight; else the first row in sight that stays
     * shown; with none of those, the match a third of the way down, as a move to it brings it. Read
     * from the list as it is drawn before the change; asked of it once the rows have changed
     * (take).
     */
    fun keep(was: IntArray?, now: IntArray?, hit: Int?) {
        val info = list.layoutInfo
        val seen =
            info.visibleItemsInfo.filter {
                it.offset + it.size > info.viewportStartOffset && it.offset < info.viewportEndOffset
            }
        // an item's row; -1 for No matches, and past the rows for the stats line
        val rowOf = { k: Int -> if (was == null) k else was.getOrElse(k) { -1 } }
        val place =
            seen.firstOrNull { hit != null && it.index == itemIn(was, hit) }
                ?: seen.firstOrNull { rowOf(it.index).let { r -> r >= 0 && itemIn(now, r) >= 0 } }
        val k = place?.let { itemIn(now, rowOf(it.index)) } ?: -1
        val match = hit?.let { itemIn(now, it) } ?: -1
        asked =
            when {
                place != null && k >= 0 -> k to -place.offset
                match >= 0 -> match to -info.viewportSize.height / 3
                else -> 0 to 0
            }
    }

    /** The place keep found, until the list is asked for it (take). */
    private var asked: Pair<Int, Int>? = null

    /**
     * The place keep found, asked of the list from the composition that has the new rows (Rows), so
     * that the list's next measure has both. Asked at once, the list was measured with the rows
     * before the change, the place taken among those, then the first row in sight kept by its key
     * once the new rows came (KeepTest, under Robolectric: rows 45 and 42 lost).
     */
    fun take() {
        val (k, offset) = asked ?: return
        asked = null
        list.requestScrollToItem(k, offset)
    }

    /**
     * The reader back with the row playing, at the list's top (the transcript's Now button): the
     * fling under way stopped, and the row followed from there on, as for a reader who never left.
     */
    suspend fun back(row: Int) {
        if (row < 0) return
        list.scroll(MutatePriority.PreventUserInput) {}
        chasing = row
        scroll { animateScrollToItem(row) }
        chasing = -1
    }

    /**
     * Row at the list's top at once, for a thumb scrubbing the waveform: the sentence under the
     * playhead it drags, to read while looking for a place (Peter, 2026-11-18). Not under the
     * reader's own hand on the list.
     */
    fun show(row: Int) {
        if (row >= 0 && fingers == 0) list.requestScrollToItem(row)
    }

    /** A new row playing, -1 once playback stops: brought for a reader still with it (bring). */
    suspend fun playing(now: Int) {
        val before = was
        was = now
        if (
            now >= 0 &&
                (before < 0 ||
                    chasing >= 0 ||
                    list.layoutInfo.visibleItemsInfo.any { it.index == before })
        )
            bring(now)
    }
}

/**
 * Counts the fingers on the list for Follow.hands, seeing each event before the list does and
 * taking none.
 */
fun Modifier.hands(f: Follow) =
    pointerInput(f) {
        try {
            awaitPointerEventScope {
                while (true) f.fingers =
                    awaitPointerEvent(PointerEventPass.Initial).changes.count { it.pressed }
            }
        } finally {
            f.fingers = 0
        }
    }
