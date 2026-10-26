package net.pktito.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.abs
import net.pktito.app.R
import net.pktito.app.Seg

/** a and b mixed as CSS's color-mix(in srgb) mixes them, t of b: a match's mark is plume 45% into the ground (app.css, .pk-hit). */
fun mix(a: Color, b: Color, t: Float) =
    Color(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t, a.alpha + (b.alpha - a.alpha) * t)

/** A match: its row, where it is in the row's text, and the time of the word it starts in. */
class Hit(val row: Int, val start: Int, val end: Int, val time: Double)

/**
 * Each match of query in rows, sentences or cues, as the desktop finds them
 * (app.js, bodyHtml and runFind): its words trimmed, without regard to
 * case, a cue's line break read as the space it stands for, each match
 * after the one before it.
 */
fun findIn(rows: List<Seg>, query: String): List<Hit> {
    val sought = query.trim()
    if (sought.isEmpty()) return emptyList()
    val hits = ArrayList<Hit>()
    for ((i, s) in rows.withIndex()) {
        val flat = s.text.replace('\n', ' ')
        var low = flat.lowercase()
        var q = sought.lowercase()
        // a lowercase that changes the length would put the marks in the wrong places
        if (low.length != flat.length) {
            low = flat
            q = sought
        }
        var from = 0
        while (true) {
            val j = low.indexOf(q, from)
            if (j < 0) break
            hits += Hit(i, j, j + q.length, timeAt(s, j))
            from = j + q.length
        }
    }
    return hits
}

/** The start of the word of s at offset in its text, its words found in it in order; s's own before its first (app.js, wordAt). */
private fun timeAt(s: Seg, offset: Int): Double {
    var at = 0
    var time = s.start
    for (w in s.words) {
        val t = w.text.trim()
        val k = s.text.indexOf(t, at)
        if (k < 0) continue
        if (k > offset) break
        time = w.start
        at = k + t.length
    }
    return time
}

/** What a Finder asks of its field: to take the keys, or to let them go. */
enum class Keys { Take, Drop }

/**
 * The search of the transcript on the screen: what is sought, and the match
 * the reader is on, by its place among the matches and by its time, which
 * keeps it where the matches change under it (the other view's, or a
 * longer word that still has it). On a phone the views line has room for
 * the views or the field, so the field opens over it (open). moved counts
 * the reader's moves, for the screen to bring each one into sight.
 */
class Finder {
    var query by mutableStateOf("")
    var open by mutableStateOf(false)
    var at by mutableIntStateOf(-1)
        private set
    var moved by mutableIntStateOf(0)
        private set
    /** the field has the keys: a key typed there is the field's, not playback's */
    var typing by mutableStateOf(false)
    /**
     * What the field is asked to do with the keys, which it does once it is
     * on the screen, and then sets back to null: on a phone the field comes
     * onto the screen with the search, a frame after the asking.
     */
    var keys by mutableStateOf<Keys?>(null)
    private var time = -1.0

    val searching: Boolean
        get() = open || query.isNotEmpty()

    /**
     * Once the matches have changed: on the same match, where it still is
     * one; else on the one the playhead last passed, so that a search typed
     * as it plays or rests starts where the listener is, the first where it
     * has passed none (app.js, runFind).
     */
    fun settle(hits: List<Hit>, position: Double) {
        if (hits.isEmpty()) {
            at = -1
            return
        }
        val same = if (time >= 0) hits.indexOfFirst { abs(it.time - time) < 1e-9 } else -1
        go(hits, if (same >= 0) same else maxOf(0, hits.indexOfLast { it.time <= position }))
    }

    fun go(hits: List<Hit>, k: Int) {
        at = k
        time = hits[k].time
        moved++
    }

    /** The next match, or the one before, round the end (app.js, stepHit). */
    fun step(hits: List<Hit>, d: Int) {
        if (hits.isNotEmpty()) go(hits, (at + d + hits.size) % hits.size)
    }

    /** The field, with its keys. */
    fun focus() {
        open = true
        keys = Keys.Take
    }

    /** The search let go, and the keys with it: Esc, Back, the field's ×, another transcript. */
    fun clear() {
        query = ""
        open = false
        at = -1
        time = -1.0
        keys = Keys.Drop
    }
}

/**
 * The search's field on the views line (app.css, .pk-find), sunk, 40 dp
 * tall for a finger to the desktop's 32: the glass, what is sought, and
 * while there is something, its count and the previous and next match,
 * dimmed with fewer than two, then the way to close it. Enter is the next
 * match and Shift+Enter the one before, as is the keyboard's search key;
 * Esc closes it from the screen's keys (MainActivity), as Back does.
 */
@Composable
fun FindField(f: Finder, hits: List<Hit>, onClose: () -> Unit, modifier: Modifier = Modifier) {
    val p = pk
    val focus = remember { FocusRequester() }
    val manager = LocalFocusManager.current
    val has = f.query.isNotBlank()
    // let go, the field lets go of the keyboard too; on a phone it is gone with the search
    LaunchedEffect(f.keys) {
        when (f.keys) {
            Keys.Take -> runCatching { focus.requestFocus() }
            Keys.Drop -> if (f.typing) manager.clearFocus()
            null -> return@LaunchedEffect
        }
        f.keys = null
    }
    // taken off the screen with the keys, it has let them go
    DisposableEffect(Unit) {
        onDispose { f.typing = false }
    }
    Row(
        modifier
            .height(40.dp)
            .then(if (f.typing) Modifier.border(2.dp, p.focus, Corner) else Modifier)
            .background(p.sunk, Corner)
            .padding(start = 10.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(Icons.search, null, tint = p.inkMuted, modifier = Modifier.size(16.dp))
        val label = stringResource(R.string.find_label)
        BasicTextField(
            value = f.query,
            onValueChange = { f.query = it },
            singleLine = true,
            textStyle = Type.ui.copy(fontWeight = FontWeight.W400, color = p.ink),
            cursorBrush = SolidColor(p.ink),
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { f.step(hits, 1) }),
            modifier = Modifier
                .weight(1f)
                .focusRequester(focus)
                .onFocusChanged { f.typing = it.isFocused }
                .onPreviewKeyEvent { e ->
                    val enter = e.key == Key.Enter || e.key == Key.NumPadEnter
                    if (enter && e.type == KeyEventType.KeyDown) f.step(hits, if (e.isShiftPressed) -1 else 1)
                    enter
                }
                .semantics { contentDescription = label },
            decorationBox = { field ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (f.query.isEmpty())
                        Text(stringResource(R.string.find_placeholder), style = Type.ui.copy(fontWeight = FontWeight.W400), color = p.inkMuted, maxLines = 1)
                    field()
                }
            },
        )
        if (has) {
            Text(
                "${if (hits.isEmpty()) 0 else f.at + 1}/${hits.size}",
                style = Type.stamp.copy(fontSize = 13.sp, fontFeatureSettings = "tnum"),
                color = p.inkMuted,
                maxLines = 1,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
            )
            Step(Icons.up, stringResource(R.string.find_previous), hits.size >= 2) { f.step(hits, -1) }
            Step(Icons.down, stringResource(R.string.find_next), hits.size >= 2) { f.step(hits, 1) }
        }
        if (has || f.open) Step(Icons.x, stringResource(R.string.find_close), true, onClose)
    }
}

/** A small button of the field's, an icon in ink-muted, 32 dp: previous, next, close (app.css, .pk-find-step). */
@Composable
private fun Step(icon: ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier
            .alpha(if (enabled) 1f else 0.45f)
            .control(CornerSm, Color.Transparent, enabled = enabled, label = label, onClick = onClick)
            .size(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, null, tint = pk.inkMuted, modifier = Modifier.size(16.dp))
    }
}
