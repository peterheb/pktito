package net.pktito.app.ui

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.pktito.app.R
import net.pktito.app.Speakers
import net.pktito.app.Stretch

/*
 * Who said what on the work screen (PKT-27; the speakers design, page version 7): the switch beside
 * the views, each speaker's name as a chip that renames them, a turn's name over its first row or
 * beside its time, Who's speaking from that name, Rename, the bar over the waveform and an edit's
 * Undo, each as the desktop's window has it (ui/app.css and app.js, speakers).
 */

/** The speakers as the work screen shows them (Speakers): what Work needs of them. */
class SpeakersView(
    /** the switch beside the views: a build with a diarizer and a finished transcript */
    val offered: Boolean,
    val on: Boolean,
    /** finding them, or downloading their model: the switch waits */
    val busy: Boolean,
    /**
     * while they are found, how much of the recording the diarizer has heard, 0 to 1: the strip
     * shows it as it shows a transcription's progress (app.js, showFinding)
     */
    val heard: Double?,
    /** the names show: the switch on and a speaker or more */
    val shown: Boolean,
    /**
     * the turns show, a second voice heard: one voice's transcript, bar and formats look as with
     * the switch off, no "Speaker 1:" (Peter, 2026-11-15, with speakers on by default), while its
     * name beside the switch says one was found (app.js, turnsShown)
     */
    val turns: Boolean,
    /**
     * the strip keeps the speaker bar's place: the names shown, the speakers being found, or a
     * transcription finding them as it goes, so that the bar fills in as they are heard (app.js,
     * barShown)
     */
    val place: Boolean,
    /** a finished transcript's names can be changed */
    val editable: Boolean,
    val present: List<Int>,
    val nameOf: (Int) -> String,
    /**
     * the bar's turns, or while speakers are found the stretches heard so far, and the seconds it
     * is drawn to
     */
    val bar: List<Stretch>,
    val upto: Double,
    val toggle: () -> Unit,
    /** a turn given to a speaker, a new one where fresh */
    val move: (turn: Speakers.Turn, to: Int, fresh: Boolean) -> Unit,
    val rename: (speaker: Int) -> Unit,
    /** the times a turn is shown with in Who's speaking */
    val times: (Speakers.Turn) -> String,
) {
    companion object {
        val NONE =
            SpeakersView(
                offered = false,
                on = false,
                busy = false,
                heard = null,
                shown = false,
                turns = false,
                place = false,
                editable = false,
                present = emptyList(),
                nameOf = { "" },
                bar = emptyList(),
                upto = 0.0,
                toggle = {},
                move = { _, _, _ -> },
                rename = {},
                times = { "" },
            )
    }
}

/** Speaker n's color on the page: the tokens' speaker1 to speaker8. */
fun Palette.speaker(n: Int): Color =
    when (Speakers.slotOf(n)) {
        1 -> speaker1
        2 -> speaker2
        3 -> speaker3
        4 -> speaker4
        5 -> speaker5
        6 -> speaker6
        7 -> speaker7
        else -> speaker8
    }

/** Speaker n's color on the strip's term: the tokens' speakerBar1 to speakerBar8. */
fun Palette.speakerBar(n: Int): Color =
    when (Speakers.slotOf(n)) {
        1 -> speakerBar1
        2 -> speakerBar2
        3 -> speakerBar3
        4 -> speakerBar4
        5 -> speakerBar5
        6 -> speakerBar6
        7 -> speakerBar7
        else -> speakerBar8
    }

/**
 * A speaker's mark beside a name (app.css, .pk-mark): a 10 dp circle for speakers 1 to 4 and a 9 dp
 * square with 2 dp corners for 5 to 8, about the same area. Every pair close in color crosses that
 * line but magenta and orchid, 5 and 8 (the design's palette check).
 */
@Composable
fun Mark(n: Int, modifier: Modifier = Modifier) {
    val square = Speakers.slotOf(n) > 4
    Box(
        modifier
            .size(if (square) 9.dp else 10.dp)
            .background(pk.speaker(n), if (square) RoundedCornerShape(2.dp) else CircleShape)
    )
}

/**
 * The switch beside the views (app.css, .pk-spk), in a track of its own as the views are, its words
 * theirs: Speakers, pressed while it is on, with a check before the word then. busy, it waits for
 * the finding or the download under way. One width on or off, so that a second touch where the
 * first was turns them off again (Peter, 2026-11-15): off, the check keeps its place unseen and the
 * word moves half of it over, to the middle, in the strip's 150 ms.
 */
@Composable
fun SpeakersSwitch(on: Boolean, busy: Boolean, state: String, onClick: () -> Unit) {
    val p = pk
    Box(Modifier.background(p.sunk, Corner).padding(2.dp)) {
        Row(
            Modifier.then(if (on) Modifier.border(1.dp, p.inkMuted, CornerSm) else Modifier)
                .control(CornerSm, if (on) p.ground else Color.Transparent, role = Role.Switch) {
                    if (!busy) onClick()
                }
                .semantics { stateDescription = state }
                .padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            val shift by animateDpAsState(if (on) 0.dp else (-9).dp, tween(150), label = "word")
            Icon(
                Icons.check,
                null,
                tint = p.ink,
                modifier = Modifier.size(12.dp).alpha(if (on) 1f else 0f),
            )
            Text(
                stringResource(R.string.speakers_switch),
                Modifier.offset(x = shift),
                style = Type.ui.copy(fontSize = 13.sp, lineHeight = 16.sp),
                color = if (on) p.ink else p.inkMuted,
                maxLines = 1,
            )
        }
    }
}

/**
 * With the names shown on a screen wider than a phone's, each speaker's, in the order they first
 * speak (app.css, .pk-names): a chip renames its speaker. 32 dp tall, the desktop's 28 and a
 * little: at 48 for a finger the lines of a full eight were 50 dp apart with 16 of words, gappy
 * beside the desktop's (Peter, 2026-11-14). A finger's touch is still 48: Compose widens a smaller
 * target's reach to the system's least (ViewConfiguration.minimumTouchTargetSize), the nearer
 * target taking a touch two reach.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun Chips(
    present: List<Int>,
    nameOf: (Int) -> String,
    onRename: (Int) -> Unit,
    modifier: Modifier,
) {
    val p = pk
    val tall = 32.dp
    FlowRow(
        modifier,
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        @Composable
        fun chip(n: Int) {
            val name = nameOf(n)
            Row(
                Modifier.control(
                        CornerSm,
                        label = stringResource(R.string.speakers_chip_label, name),
                    ) {
                        onRename(n)
                    }
                    .heightIn(min = tall)
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Mark(n)
                Text(
                    name,
                    style = Type.ui.copy(fontSize = 13.sp, lineHeight = 16.sp),
                    color = p.ink,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.widthIn(max = 240.dp),
                )
            }
        }
        for ((k, n) in present.withIndex()) {
            // the (i) wraps with the last name, never on a line of its own
            if (k == present.lastIndex && present.size >= Speakers.MOST)
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    chip(n)
                    Limit(tall)
                }
            else chip(n)
        }
    }
}

/**
 * With all 8 of the diarizer's speakers in use, an (i) after their names (app.js, limitHtml): a
 * recording with more voices has some of them mixed up, which at 12 readers of 12 chapters looked
 * like a fault (Peter, 2026-11-14, Alice's chapter 11). A tap says so, in a menu's box as Who's
 * speaking is.
 */
@Composable
private fun Limit(tall: Dp) {
    val p = pk
    var open by remember { mutableStateOf(false) }
    val head = stringResource(R.string.speakers_limit, Speakers.MOST)
    Box {
        Box(
            Modifier.control(CornerSm, label = head) { open = true }.size(tall),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Icons.info, null, tint = p.inkMuted, modifier = Modifier.size(16.dp))
        }
        Menu(open, { open = false }) {
            Text(
                head,
                style = Type.ui.copy(fontWeight = FontWeight.W700, lineHeight = 20.sp),
                color = p.ink,
                modifier = Modifier.padding(start = 10.dp, end = 10.dp, top = 8.dp),
            )
            Text(
                stringResource(R.string.speakers_limit_about, Speakers.MOST),
                style = Type.ui.copy(fontSize = 13.sp, lineHeight = 20.sp),
                color = p.inkMuted,
                modifier = Modifier.padding(start = 10.dp, end = 10.dp, top = 4.dp, bottom = 8.dp),
            )
        }
    }
}

/** The inline content that is a turn's mark in its first sentence (speakerName, speakerMark). */
const val SPEAKER_MARK = "speaker-mark"

/**
 * A turn's name opening its first sentence's text (app.css, .pk-speaker): its mark, then the name
 * bold with a colon, as the saved files have it ("Speaker 1: ..."), a link that opens Who's
 * speaking. Peter's pick of three (2026-11-14): over the row the name read as a heading of its own.
 * The sentence's own text starts at the length this returns.
 */
fun AnnotatedString.Builder.speakerName(
    name: String,
    styles: TextLinkStyles,
    onClick: () -> Unit,
): Int {
    withLink(LinkAnnotation.Clickable("speaker", styles) { onClick() }) {
        appendInlineContent(SPEAKER_MARK, "\u2060")
        withStyle(SpanStyle(fontWeight = FontWeight.W700)) { append("$name:") }
    }
    append(" ")
    return length
}

/** The mark before a turn's name in its text: 10 dp, and 7 to the name, as the desktop's. */
@Composable
fun speakerMark(n: Int): InlineTextContent {
    val d = LocalDensity.current
    return InlineTextContent(
        with(d) { Placeholder(17.dp.toSp(), 10.dp.toSp(), PlaceholderVerticalAlign.TextCenter) }
    ) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.CenterStart) { Mark(n) }
    }
}

/**
 * Who's speaking for the turn whose name was tapped: on a phone a dialog in the middle of the
 * screen, where the names' chips no longer are (Peter, 2026-11-14: "maybe on mobile managing
 * speaker names is a dialog?"), as tall as eight speakers, Rename and New speaker need; on a wider
 * screen a menu under the name, as the desktop's.
 */
@Composable
fun WhoPicker(
    open: Boolean,
    dialog: Boolean,
    close: () -> Unit,
    turn: Speakers.Turn,
    spk: SpeakersView,
) {
    if (!dialog) return Menu(open, close) { Who(turn, spk, close) }
    if (open)
        Modal(close) {
            Column(
                Modifier.widthIn(max = 400.dp)
                    .fillMaxWidth()
                    .floating()
                    .padding(horizontal = 6.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Who(turn, spk, close)
            }
        }
}

/**
 * Who's speaking, for the turn whose name was tapped (app.js, openWho): its times, the speakers as
 * a choice, then Rename and New speaker. New speaker is for a voice the model merged into someone
 * else's: the lowest number not in use, so that no two share a color, and none past eight.
 */
@Composable
private fun Who(turn: Speakers.Turn, spk: SpeakersView, close: () -> Unit) {
    val p = pk
    Text(
        stringResource(R.string.speakers_who),
        style = Type.ui.copy(fontWeight = FontWeight.W700, lineHeight = 20.sp),
        color = p.ink,
        modifier = Modifier.padding(start = 10.dp, end = 10.dp, top = 8.dp),
    )
    Text(
        spk.times(turn),
        style = Type.stamp.copy(fontFeatureSettings = "tnum", lineHeight = 1.5.em),
        color = p.inkMuted,
        modifier = Modifier.padding(start = 10.dp, end = 10.dp, bottom = 6.dp),
    )
    for (n in spk.present) Radio(n, spk.nameOf(n), n == turn.speaker) {
        close()
        spk.move(turn, n, false)
    }
    MenuLine()
    Item(stringResource(R.string.speakers_rename, spk.nameOf(turn.speaker)), null) {
        close()
        spk.rename(turn.speaker)
    }
    val fresh = Speakers.fresh(spk.present)
    Item(stringResource(R.string.speakers_new), null, enabled = fresh != null) {
        close()
        if (fresh != null) spk.move(turn, fresh, true)
    }
    // with all 8 in use, why New speaker is off, and why voices may be mixed up: a phone has no
    // chips for the (i) to follow (Limit)
    if (fresh == null)
        Text(
            stringResource(R.string.speakers_limit_about, Speakers.MOST),
            style = Type.ui.copy(fontSize = 13.sp, lineHeight = 20.sp),
            color = p.inkMuted,
            modifier = Modifier.padding(start = 10.dp, end = 10.dp, top = 4.dp, bottom = 8.dp),
        )
}

/** One of Who's speaking's speakers (app.css, .pk-radio): the dot, the mark and the name. */
@Composable
private fun Radio(n: Int, name: String, checked: Boolean, onClick: () -> Unit) {
    val p = pk
    Row(
        Modifier.control(CornerSm, role = Role.RadioButton, onClick = onClick)
            .semantics { selected = checked }
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(
            Modifier.size(16.dp).border(2.dp, if (checked) p.ink else p.inkMuted, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (checked) Box(Modifier.size(7.dp).background(p.ink, CircleShape))
        }
        Box(Modifier.width(12.dp), contentAlignment = Alignment.Center) { Mark(n) }
        Text(
            name,
            style = Type.ui.copy(fontWeight = FontWeight.W500, lineHeight = 20.sp),
            color = p.ink,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * Rename, from a speaker's chip or Who's speaking (app.js, renameIn), as a Modal: a field with the
 * name, chosen, which says so where the name typed is another speaker's (joins). The desktop's
 * field takes Enter; a phone's keyboard has its Done, and the Modal its Rename and Cancel.
 */
@Composable
fun RenameSpeaker(
    name: String,
    joins: (String) -> String?,
    onRename: (String) -> Unit,
    onCancel: () -> Unit,
) {
    val p = pk
    var v by remember { mutableStateOf(TextFieldValue(name, TextRange(0, name.length))) }
    var focused by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    val label = stringResource(R.string.speakers_name)
    Modal(onCancel) {
        Column(Modifier.widthIn(max = 400.dp).fillMaxWidth().floating().padding(24.dp)) {
            Text(
                stringResource(R.string.speakers_rename_title, name),
                style = Type.ui,
                color = p.ink,
            )
            BasicTextField(
                value = v,
                onValueChange = { v = it },
                singleLine = true,
                textStyle = Type.ui.copy(fontWeight = FontWeight.W500, color = p.ink),
                cursorBrush = SolidColor(p.ink),
                keyboardOptions =
                    KeyboardOptions(
                        capitalization = KeyboardCapitalization.Words,
                        autoCorrectEnabled = false,
                        imeAction = ImeAction.Done,
                    ),
                keyboardActions = KeyboardActions(onDone = { onRename(v.text) }),
                modifier =
                    Modifier.padding(top = 16.dp)
                        .fillMaxWidth()
                        .focusRequester(focus)
                        .onFocusChanged { focused = it.isFocused }
                        .semantics { contentDescription = label },
                decorationBox = { field ->
                    Box(
                        Modifier.then(
                                if (focused) Modifier.border(2.dp, p.focus, CornerSm) else Modifier
                            )
                            .background(p.sunk, CornerSm)
                            .heightIn(min = 40.dp)
                            .padding(horizontal = 10.dp),
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        field()
                    }
                },
            )
            joins(v.text)?.let {
                Text(
                    it,
                    style = Type.small,
                    color = p.inkMuted,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            Row(
                Modifier.fillMaxWidth().padding(top = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            ) {
                PkButton(
                    stringResource(R.string.ask_cancel),
                    onCancel,
                    Modifier.widthIn(min = 80.dp),
                )
                PkButton(
                    stringResource(R.string.speakers_rename_do),
                    { onRename(v.text) },
                    Modifier.widthIn(min = 80.dp),
                    kind = Kind.Primary,
                )
            }
        }
    }
    LaunchedEffect(Unit) {
        withFrameNanos {}
        runCatching { focus.requestFocus() }
    }
}

/**
 * The speaker bar over the waveform (app.js, drawBar): the transcript's turns on a 4 dp line, each
 * in its speaker's color; while speakers are found, the stretches the diarizer has heard so far
 * (finding). Each dp of it shows whoever talks most of it (Speakers.pixels), so that an hour still
 * reads, about 14 s a dp on a 393 dp phone (its waveform's 257 dp), and a turn too short to win a
 * dp still gets the one at its middle; a run of one speaker's wider than 3 dp stands 1 dp clear of
 * the next either side. A second voice heard over the one talking has no line of its own: the
 * transcript has no words for it (Peter, 2026-11-15). 10 dp tall; the strip puts it 8 dp over the
 * waveform.
 */
@Composable
fun SpeakerBar(
    stretches: List<Stretch>,
    duration: Double,
    upto: Double,
    modifier: Modifier,
    finding: Boolean = false,
) {
    val p = pk
    val parts = remember(stretches, upto) { Speakers.parts(stretches, upto) }
    val label = stringResource(R.string.speakers_bar)
    Canvas(modifier.fillMaxWidth().height(10.dp).semantics { contentDescription = label }) {
        if (duration <= 0) return@Canvas
        val dp = density
        val px = maxOf(1, (size.width / dp).toInt())
        val per = duration / px
        val who = Speakers.pixels(parts, px, per, everyTurn = !finding)
        var x = 0
        while (x < px) {
            val n = who[x]
            var e = x + 1
            while (e < px && who[e] == n) e++
            if (n != 0) {
                val w = e - x
                val inset = if (w > 3) 1 else 0
                drawRect(
                    p.speakerBar(n),
                    Offset((x + inset) * dp, 6f * dp),
                    Size((w - 2 * inset) * dp, 4f * dp),
                )
            }
            x = e
        }
    }
}

/**
 * The toast of an edit that can be undone (app.css, .pk-toast .pk-undo): what it did, and Undo on
 * the term's tint, 40 dp tall in the toast's 48.
 */
@Composable
fun UndoToast(text: String, onUndo: () -> Unit, modifier: Modifier = Modifier) {
    val p = pk
    Row(
        modifier
            .widthIn(max = 420.dp)
            .background(p.term, Corner)
            .border(1.dp, p.toastEdge, Corner)
            .padding(start = 16.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text,
            style = Type.ui,
            color = p.termInk,
            modifier = Modifier.weight(1f, fill = false),
        )
        Box(
            Modifier.control(CornerSm, p.tintTerm, onTerm = true, onClick = onUndo)
                .heightIn(min = 40.dp)
                .padding(horizontal = 10.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                stringResource(R.string.speakers_undo),
                style = Type.ui.copy(fontSize = 13.sp, fontWeight = FontWeight.W500),
                color = p.termInk,
            )
        }
    }
}
