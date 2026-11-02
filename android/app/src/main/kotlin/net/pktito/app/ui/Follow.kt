package net.pktito.app.ui

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput

/**
 * The transcript's list and what moves it besides the reader (MainActivity):
 * the transcript's end as it comes, for a reader who is there; the row
 * playing, for a reader still with it, whose row before is in sight or who
 * has just pressed play, as the desktop's (app.js, showTime); and a seek on
 * the waveform, whose row comes wherever the reader is. None of it while a
 * finger is on the list or the fling it left still moves it: the list pulled
 * to the row playing at each new one, under a reader scrolling to a row to
 * tap, lost the playhead after a few such taps (Peter, the iPhone 16e,
 * 2026-11-01; Android's list followed as iOS's did). A reader who has
 * scrolled away is left there, and taken along again from the next row once
 * theirs is back in sight.
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
     * A row a scroll of ours set out for and has not reached, -1 for none:
     * the next row playing, which cuts that scroll short, is followed as
     * for a reader still with it.
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
     * Row at the list's top, where it isn't in sight above the list's last
     * row, which shows only in part; not under the reader's hand. Compose
     * lands a far row exactly, the rows between measured as it goes, where
     * iOS's list can stop short and tries again.
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

    /** A new row playing, -1 once playback stops: brought for a reader still with it (bring). */
    suspend fun playing(now: Int) {
        val before = was
        was = now
        if (now >= 0 && (before < 0 || chasing >= 0 || list.layoutInfo.visibleItemsInfo.any { it.index == before }))
            bring(now)
    }
}

/** Counts the fingers on the list for Follow.hands, seeing each event before the list does and taking none. */
fun Modifier.hands(f: Follow) = pointerInput(f) {
    try {
        awaitPointerEventScope {
            while (true) f.fingers = awaitPointerEvent(PointerEventPass.Initial).changes.count { it.pressed }
        }
    } finally {
        f.fingers = 0
    }
}
