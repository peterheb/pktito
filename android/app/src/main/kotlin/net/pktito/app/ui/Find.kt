package net.pktito.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.abs
import net.pktito.app.R
import net.pktito.app.Seg

/**
 * a and b mixed as CSS's color-mix(in srgb) mixes them, t of b: a match's mark is plume 45% into
 * the ground (app.css, .pk-hit).
 */
fun mix(a: Color, b: Color, t: Float) =
    Color(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )

/** A match: its row, where it is in the row's text, and the time of the word it starts in. */
class Hit(val row: Int, val start: Int, val end: Int, val time: Double)

/**
 * Speaker n's turns in rows, as the desktop finds a speaker (app.js, runFind with who): a match a
 * turn, at its first row's start, its name marked (start -1: Sentence marks the name, not words).
 */
fun turnsOf(rows: List<Seg>, n: Int): List<Hit> =
    rows.indices
        .filter { i -> rows[i].speaker == n && (i == 0 || rows[i - 1].speaker != n) }
        .map { Hit(it, -1, -1, rows[it].start) }

/**
 * Each match of query in rows, the sentences, as the desktop finds them (app.js, bodyHtml and
 * runFind): its words trimmed, without regard to case, each match at a word's start (startsWord)
 * and after the one before it; with a speaker found (who), in their sentences alone (Peter,
 * 2026-11-15: a speaker and a phrase of theirs).
 */
fun findIn(rows: List<Seg>, query: String, who: Int = 0): List<Hit> {
    val sought = query.trim()
    if (sought.isEmpty()) return emptyList()
    val hits = ArrayList<Hit>()
    for ((i, s) in rows.withIndex()) {
        if (who != 0 && s.speaker != who) continue
        var low = s.text.lowercase()
        var q = sought.lowercase()
        // a lowercase that changes the length would put the marks in the wrong places
        if (low.length != s.text.length) {
            low = s.text
            q = sought
        }
        var from = 0
        while (true) {
            val j = low.indexOf(q, from)
            if (j < 0) break
            if (startsWord(low, j, q)) {
                hits += Hit(i, j, j + q.length, timeAt(s, j))
                from = j + q.length
            } else {
                from = j + 1
            }
        }
    }
    return hits
}

/**
 * The rows Only matches shows (app.js, applyOnly), in order: those with a match of the words, or
 * with only a speaker found, every sentence of theirs.
 */
fun shownRows(rows: List<Seg>, hits: List<Hit>, query: String, who: Int): IntArray {
    val shown = BooleanArray(rows.size) { query.isBlank() && who != 0 && rows[it].speaker == who }
    for (h in hits) if (h.row in shown.indices) shown[h.row] = true
    return shown.indices.filter { shown[it] }.toIntArray()
}

/**
 * Row's place in a list of the rows shown (shownRows), -1 where it isn't one of them; with all
 * shown (null), the row itself.
 */
fun itemIn(shown: IntArray?, row: Int): Int =
    if (shown == null || row < 0) row
    else java.util.Arrays.binarySearch(shown, row).let { if (it >= 0) it else -1 }

/**
 * Whether a match of q at j in text starts a word, as every match must: "cat" finds "Cat" and
 * "cats", not "concatenate" or "scat" (Peter, 2026-11-04; app.js, startsWord). At the text's start
 * or after a character that is no letter, digit or mark, an apostrophe too ("neill" finds
 * "O'Neill", "homme" "l'homme"); a search that itself begins with no letter or digit ("'s", "-19")
 * matches anywhere.
 */
fun startsWord(text: String, j: Int, q: String) =
    j == 0 || !wordChar(q[0]) || !wordChar(text[j - 1])

private fun wordChar(c: Char) =
    c.isLetterOrDigit() ||
        Character.getType(c).let {
            it == Character.LETTER_NUMBER.toInt() ||
                it == Character.OTHER_NUMBER.toInt() ||
                it == Character.NON_SPACING_MARK.toInt() ||
                it == Character.COMBINING_SPACING_MARK.toInt() ||
                it == Character.ENCLOSING_MARK.toInt()
        }

/**
 * The start of the word of s at offset in its text, its words found in it in order; s's own before
 * its first (app.js, wordAt).
 */
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
enum class Keys {
    Take,
    Drop,
}

/**
 * The search of the transcript on the screen: what is sought, and the match the reader is on, by
 * its place among the matches and by its time, which keeps it where the matches change under it
 * (after a change of speaker, or a longer word that still has it). On a phone the line under the
 * strip may have room for the speakers' switch or the field, so the field opens over it (open).
 * moved counts the reader's moves, for the screen to bring each one into sight.
 */
class Finder {
    var query by mutableStateOf("")
    /**
     * The speaker found (findWho): their turns are the matches (turnsOf), or with words typed the
     * words' matches in their sentences alone (findIn); 0 for none.
     */
    var who by mutableIntStateOf(0)
    var open by mutableStateOf(false)
    /**
     * Only the sentences found shown (shownRows; app.js, applyOnly): off by default, and off again
     * once the search is let go (clear, and the screen where nothing is sought).
     */
    var only by mutableStateOf(false)
    var at by mutableIntStateOf(-1)
        private set

    var moved by mutableIntStateOf(0)
        private set

    /**
     * Counts the moves whose rows the list has brought into sight, for each row then to bring its
     * match's own line (Sentence): a row taller than the room had its top in sight and the match
     * below it.
     */
    var shown by mutableIntStateOf(0)
        private set

    /** the field has the keys: a key typed there is the field's, not playback's */
    var typing by mutableStateOf(false)
    /**
     * What the field is asked to do with the keys, which it does once it is on the screen, and then
     * sets back to null: on a phone the field comes onto the screen with the search, a frame after
     * the asking.
     */
    var keys by mutableStateOf<Keys?>(null)
    private var time = -1.0
    /**
     * The match the playhead last passed when the reader was last put by it (follow); -1 for none.
     */
    private var passed = -1
    /**
     * The search the matches were last settled for (settle), to tell a search typed from the same
     * one's matches marked anew.
     */
    private var settled = ""

    val searching: Boolean
        get() = open || query.isNotEmpty() || who != 0

    /**
     * The search kept through a turn of the screen, the field open with it: not one typed, so the
     * playhead stays (settle).
     */
    fun restore(q: String) {
        query = q
        open = q.isNotEmpty()
        settled = sought()
    }

    /**
     * Once the matches or the search have changed: on the same match, where it still is one; else
     * on the one the playhead last passed, so that a search typed as it plays or rests starts where
     * the listener is, the first where it has passed none (app.js, runFind). A search typed at rest
     * (seek, not null) takes the playhead to that match, as a step does: typing it says the
     * listener means to go where it is (Peter, 2026-11-01). The same search's matches marked anew,
     * after a change of speaker, leave the playhead where it is. The seek first, as a step's.
     */
    fun settle(hits: List<Hit>, position: Double, seek: ((Double) -> Unit)? = null) {
        val sought = sought()
        val typed = sought != settled
        settled = sought
        passed = passedAt(hits, position)
        if (hits.isEmpty()) {
            at = -1
            return
        }
        val same = if (time >= 0) hits.indexOfFirst { abs(it.time - time) < 1e-9 } else -1
        val k = if (same >= 0) same else maxOf(0, passed)
        if (typed) seek?.invoke(hits[k].time)
        go(hits, k)
    }

    // what is sought: a speaker found as words typed are, and with them
    private fun sought() = "$query\u0000$who"

    // as the rows find the one playing (Screens.kt, Rows)
    private fun passedAt(hits: List<Hit>, position: Double) = hits.indexOfLast {
        it.time <= position + 1e-3
    }

    /**
     * The reader on the match the playhead last passed, the first where it has passed none, as a
     * tick tapped puts them (Peter, 2026-10-26): as playback passes a match, and wherever a seek
     * lands (jump), though it lands short of the next. A match stepped to stays until then. The
     * rows stay where the playhead has them: no move to bring into sight.
     */
    fun follow(hits: List<Hit>, position: Double, jump: Boolean) {
        val k = passedAt(hits, position)
        if (hits.isEmpty() || (!jump && k == passed)) return
        passed = k
        at = maxOf(0, k)
        time = hits[at].time
    }

    /** The list has brought the move's row into sight (MainActivity): now its match's line. */
    fun show() {
        shown++
    }

    fun go(hits: List<Hit>, k: Int) {
        at = k
        time = hits[k].time
        moved++
    }

    /**
     * The next match, or the one before, round the end (app.js, stepHit), with the playhead taken
     * to it, as a tick takes it, so that the reader follows the playhead from there: at rest too,
     * where a transcript brought to a match looks ready to play from it, and Play from the old
     * place read as a jump (Peter, 2026-11-01). The seek first, which puts the reader on the match
     * it last passed, then this one, where two share its time.
     */
    fun step(hits: List<Hit>, d: Int, seek: (Double) -> Unit) {
        if (hits.isEmpty()) return
        val k = (at + d + hits.size) % hits.size
        seek(hits[k].time)
        go(hits, k)
    }

    /**
     * Speaker n found, as a search finds a word (app.js, findWho; Peter, 2026-11-15, naming his
     * speakers: "who is this Speaker 4", needing "to hear their words and read the context"): their
     * turns the matches, the field open with their name in it, without the keyboard, which would
     * cover the turns; the arrows step from turn to turn with the playhead. Words typed then, or
     * before, are sought in their sentences alone. Found again, let go, and the words kept.
     */
    fun findWho(n: Int) {
        if (who == n) return dropWho()
        who = n
        open = true
    }

    /**
     * The speaker found let go and the words kept (FindField: their x, which a phone's keyboard
     * needs, having no Backspace in an empty field; Peter, 2026-11-16); with no words, the search
     * ends.
     */
    fun dropWho() {
        who = 0
        if (query.isEmpty()) clear()
    }

    /** The field, with its keys. */
    fun focus() {
        open = true
        keys = Keys.Take
    }

    /** The search let go, and the keys with it: Esc, Back, the field's ×, another transcript. */
    fun clear() {
        query = ""
        who = 0
        only = false
        open = false
        at = -1
        time = -1.0
        passed = -1
        keys = Keys.Drop
    }
}

/**
 * The search's field on the line under the strip (app.css, .pk-find), sunk, 40 dp tall for a finger
 * to the desktop's 32: the glass, what is sought, and while there is something, its count and the
 * previous and next match, dimmed with fewer than two, then the way to close it, where it closes
 * something: a search, or the field the glass opened (SpeakersLine); one resting beside the switch
 * has none, as the desktop's hasn't. Enter is the next match and Shift+Enter the one before, as is
 * the keyboard's search key; Esc closes it from the screen's keys (MainActivity), as Back does. A
 * step takes the playhead with it (seek: Finder.step).
 */
@Composable
fun FindField(
    f: Finder,
    hits: List<Hit>,
    seek: (Double) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    /** a speaker's name, for the one found (Finder.who) */
    nameOf: (Int) -> String = { "" },
) {
    val p = pk
    val focus = remember { FocusRequester() }
    val manager = LocalFocusManager.current
    val has = f.query.isNotBlank() || f.who != 0
    // a field with room for a speaker's name and words of theirs: in a
    // phone's, with words typed, the speaker is their mark alone
    var roomy by remember { mutableStateOf(true) }
    val density = LocalDensity.current
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
            .onSizeChanged { roomy = with(density) { it.width.toDp() } >= 360.dp }
            .then(if (f.typing) Modifier.border(2.dp, p.focus, Corner) else Modifier)
            .background(p.sunk, Corner)
            // the glass's ink and the x's on the content line, 16 dp in, as
            // the strip's play and the rows' words are (Peter, 2026-10-27):
            // the glass's is 4/3 dp into its 16, the x's 34/3 into its 32
            .padding(start = (16f - 4f / 3).dp, end = (16f - 34f / 3).dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(Icons.search, null, tint = p.inkMuted, modifier = Modifier.size(16.dp))
        // the speaker found, their mark and name before the words (app.css,
        // .pk-find-who), which are sought in their sentences alone; in a
        // phone's field, with words typed, their mark alone, its name for
        // the screen reader
        if (f.who != 0) {
            val name = nameOf(f.who)
            val full = roomy || f.query.isEmpty()
            // a button, whose x lets them go and keeps the words (Finder.dropWho);
            // the field's height to a finger, its 28 drawn
            Box(
                Modifier.fillMaxHeight().pointerInput(Unit) { detectTapGestures { f.dropWho() } },
                contentAlignment = Alignment.Center,
            ) {
                Row(
                    Modifier.widthIn(max = 180.dp)
                        .height(28.dp)
                        .control(
                            CornerSm,
                            p.ground,
                            label = stringResource(R.string.find_who_remove, name),
                            onClick = f::dropWho,
                        )
                        .padding(start = 8.dp, end = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Mark(f.who)
                    if (full)
                        Text(
                            name,
                            Modifier.weight(1f, fill = false),
                            style = Type.ui.copy(fontSize = 13.sp, lineHeight = 16.sp),
                            color = p.ink,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    Icon(Icons.x, null, tint = p.inkMuted, modifier = Modifier.size(12.dp))
                }
            }
        }
        val label = stringResource(R.string.find_label)
        BasicTextField(
            value = f.query,
            onValueChange = { f.query = it },
            singleLine = true,
            textStyle = Type.ui.copy(fontWeight = FontWeight.W400, color = p.ink),
            cursorBrush = SolidColor(p.ink),
            keyboardOptions =
                KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { f.step(hits, 1, seek) }),
            modifier =
                Modifier.weight(1f)
                    .focusRequester(focus)
                    .onFocusChanged { f.typing = it.isFocused }
                    .onPreviewKeyEvent { e ->
                        val enter = e.key == Key.Enter || e.key == Key.NumPadEnter
                        if (enter && e.type == KeyEventType.KeyDown)
                            f.step(hits, if (e.isShiftPressed) -1 else 1, seek)
                        // the speaker before an empty field taken back, as a word's last letter is
                        val back = e.key == Key.Backspace && f.who != 0 && f.query.isEmpty()
                        if (back && e.type == KeyEventType.KeyDown) f.clear()
                        enter || back
                    }
                    .semantics { contentDescription = label },
            decorationBox = { field ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (f.query.isEmpty())
                        Text(
                            stringResource(R.string.find_placeholder),
                            style = Type.ui.copy(fontWeight = FontWeight.W400),
                            color = p.inkMuted,
                            maxLines = 1,
                        )
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
                // as far from the first arrow as the arrows are from one
                // another: the 34/3 dp a button keeps round its glyph, after it
                modifier =
                    Modifier.padding(end = (34f / 3).dp).semantics {
                        liveRegion = LiveRegionMode.Polite
                    },
            )
            Step(Icons.up, stringResource(R.string.find_previous), hits.size >= 2) {
                f.step(hits, -1, seek)
            }
            Step(Icons.down, stringResource(R.string.find_next), hits.size >= 2) {
                f.step(hits, 1, seek)
            }
        }
        if (has || f.open) Step(Icons.x, stringResource(R.string.find_close), true, onClose)
    }
}

/**
 * A small button of the field's, an icon in ink-muted, 32 dp: previous, next, close (app.css,
 * .pk-find-step).
 */
@Composable
private fun Step(icon: ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.alpha(if (enabled) 1f else 0.45f)
            .control(
                CornerSm,
                Color.Transparent,
                enabled = enabled,
                label = label,
                onClick = onClick,
            )
            .size(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, null, tint = pk.inkMuted, modifier = Modifier.size(16.dp))
    }
}
