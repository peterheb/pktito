package net.pktito.app.ui

import android.content.res.Configuration
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.offset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import kotlin.math.roundToInt

val Corner = RoundedCornerShape(Radius.radiusUi)
val CornerSm = RoundedCornerShape(Radius.radiusSm)

/**
 * Hover and press as every control of the desktop's has them, a tint laid
 * over its fill or what it sits on (design/README.md): darker in light,
 * lighter in dark, term's own on the strip. Keyboard focus is a 2 dp ring,
 * ink in light and plume in dark. No ripple: the desktop has none. A long
 * press, where a control has one, is Android's way to a row's other
 * actions, which the desktop keeps under the pointer.
 */
@Composable
fun Modifier.control(
    shape: Shape,
    fill: Color = Color.Transparent,
    onTerm: Boolean = false,
    enabled: Boolean = true,
    role: Role = Role.Button,
    label: String? = null,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit,
): Modifier {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val hovered by source.collectIsHoveredAsState()
    val focused by source.collectIsFocusedAsState()
    val p = pk
    val tint = when {
        !enabled -> Color.Transparent
        pressed -> if (onTerm) p.tintTermPress else p.tintPress
        hovered -> if (onTerm) p.tintTerm else p.tint
        else -> Color.Transparent
    }
    return this
        .then(if (focused) Modifier.border(2.dp, if (onTerm) p.plume else p.focus, shape) else Modifier)
        .clip(shape)
        .background(fill)
        .background(tint)
        .hoverable(source, enabled)
        .then(
            if (onLongClick == null) Modifier.clickable(source, null, enabled, role = role, onClick = onClick)
            else Modifier.combinedClickable(source, null, enabled, role = role, onLongClick = onLongClick, onClick = onClick)
        )
        .then(if (label != null) Modifier.semantics { contentDescription = label } else Modifier)
}

/**
 * The room a control takes back around itself, as CSS's negative margin
 * does: laid out the size of what it holds less horizontal and vertical on
 * each side, drawn and touched whole, so that the strip's small buttons
 * leave its lines as tall as their words (app.css, .pk-speed, .pk-vol).
 */
fun Modifier.bleed(horizontal: Dp = 0.dp, vertical: Dp = 0.dp) = layout { measurable, constraints ->
    val h = horizontal.roundToPx()
    val v = vertical.roundToPx()
    val p = measurable.measure(constraints.offset(2 * h, 2 * v))
    layout(p.width - 2 * h, p.height - 2 * v) { p.place(-h, -v) }
}

/**
 * A line of text as tall as CSS makes it where that is shorter than its
 * font, as the desktop's figure (64 px at 0.9) and wordmark (20 at 1) are:
 * Compose keeps a line at least as tall as its font's ascent and descent
 * (ui-text 1.12: 64 sp at 0.9 em and at 1 em both 70 dp, at 1.2 em 77;
 * LinesTest), where CSS lets the letters overflow a shorter line evenly
 * above and below. A single line, in style.
 */
fun Modifier.cssLine(style: TextStyle) = layout { measurable, constraints ->
    val line = (if (style.lineHeight.isEm) style.fontSize.toPx() * style.lineHeight.value else style.lineHeight.toPx()).roundToInt()
    val p = measurable.measure(constraints.copy(minHeight = 0, maxHeight = Constraints.Infinity))
    layout(p.width, line) { p.place(0, (line - p.height) / 2) }
}

enum class Kind { Default, Primary, Ghost, Del }

/**
 * A button: sunk at rest, plume for the one a screen is for, ghost for one
 * that waits its turn (Stop), and sunk with the error's red for one that
 * deletes (app.css, .pk-btn-del). 40 dp tall, past the desktop's 32 for a
 * finger, 48 large; a dimmed one is disabled. An icon before its words or
 * after them, 6 dp apart.
 */
@Composable
fun PkButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    kind: Kind = Kind.Default,
    large: Boolean = false,
    enabled: Boolean = true,
    trailing: ImageVector? = null,
    leading: ImageVector? = null,
) {
    val p = pk
    val fill = when (kind) {
        Kind.Primary -> p.plume
        Kind.Default, Kind.Del -> p.sunk
        Kind.Ghost -> Color.Transparent
    }
    val ink = when (kind) {
        Kind.Primary -> p.onPlume
        Kind.Del -> p.err
        else -> p.ink
    }
    Row(
        modifier
            .alpha(if (enabled) 1f else 0.45f)
            .control(Corner, fill, enabled = enabled, onClick = onClick)
            .heightIn(min = if (large) 48.dp else 40.dp)
            .padding(horizontal = if (large) 18.dp else 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
    ) {
        if (leading != null)
            Icon(leading, null, tint = ink, modifier = Modifier.size(16.dp))
        Text(text, style = Type.ui.copy(fontSize = if (large) 15.sp else 14.sp), color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (trailing != null)
            Icon(trailing, null, tint = ink, modifier = Modifier.size(16.dp))
    }
}

/** A button of an icon alone, its words for a reader and a long press. */
@Composable
fun IconBtn(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    kind: Kind = Kind.Default,
    enabled: Boolean = true,
) {
    val p = pk
    Box(
        modifier
            .alpha(if (enabled) 1f else 0.45f)
            .control(Corner, if (kind == Kind.Ghost) Color.Transparent else p.sunk, enabled = enabled, label = label, onClick = onClick)
            .size(40.dp),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, null, tint = p.ink, modifier = Modifier.size(18.dp))
    }
}

/** pktito in Archivo 700, and the beak's square after it on the line. */
@Composable
fun Wordmark(modifier: Modifier = Modifier, size: TextUnit = 20.sp) {
    val square = with(LocalDensity.current) { (size * 0.5f).toDp() }
    val gap = with(LocalDensity.current) { (size * 0.08f).toDp() }
    val style = TextStyle(
        fontFamily = Fonts.sans, fontWeight = FontWeight.W700, fontSize = size,
        lineHeight = 1.em, lineHeightStyle = CssLine, letterSpacing = (-0.03).em,
    )
    Row(modifier) {
        Text("pktito", style = style, color = pk.ink, modifier = Modifier.alignByBaseline().cssLine(style))
        Spacer(Modifier.width(gap))
        Box(Modifier.size(square).alignBy { it.measuredHeight }.background(pk.beak))
    }
}

/** Beside the file's name while it is at work: a ring in sunk, a quarter of it ink-muted, turning. */
@Composable
fun Spinner(modifier: Modifier = Modifier) {
    val p = pk
    val turn by rememberInfiniteTransition(label = "spin").animateFloat(
        0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "turn",
    )
    Canvas(modifier.size(14.dp)) {
        val w = 2.dp.toPx()
        val r = Size(size.width - w, size.height - w)
        val at = Offset(w / 2, w / 2)
        drawArc(p.sunk, 0f, 360f, false, at, r, style = Stroke(w))
        drawArc(p.inkMuted, turn - 135f, 90f, false, at, r, style = Stroke(w))
    }
}

/**
 * Two choices in a sunk track, the one chosen raised to the ground with an
 * ink-muted edge (app.css, .pk-views): 8 dp above and below the words to
 * the desktop's 5, for a finger, as PkButton is 40 dp to its 32.
 */
@Composable
fun Switch2(first: String, second: String, onSecond: Boolean, onChange: (Boolean) -> Unit, modifier: Modifier = Modifier) {
    val p = pk
    Row(modifier.background(p.sunk, Corner).padding(2.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        for ((text, second2) in listOf(first to false, second to true)) {
            val on = onSecond == second2
            Text(
                text,
                style = Type.ui.copy(fontSize = 13.sp, lineHeight = 16.sp),
                color = if (on) p.ink else p.inkMuted,
                modifier = Modifier
                    .then(if (on) Modifier.border(1.dp, p.inkMuted, CornerSm) else Modifier)
                    .control(CornerSm, if (on) p.ground else Color.Transparent, role = Role.RadioButton) { onChange(second2) }
                    .padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }
    }
}

/**
 * A button that reads as a link (app.css, .pk-linkbtn): its words in ink,
 * underlined, 500 14/20. Its padding outside it holds its tint and its
 * focus ring, 8 dp above and below where the desktop's has 2, for a finger.
 */
@Composable
fun LinkButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Text(
        text,
        style = Type.ui.copy(lineHeight = 20.sp, textDecoration = TextDecoration.Underline),
        color = pk.ink,
        modifier = modifier
            .bleed(4.dp, 8.dp)
            .control(RoundedCornerShape(4.dp), onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 8.dp),
    )
}

/**
 * What floats over a screen until it is done with, About's sheets and the
 * recent list's question: a dialog, so that a reader and the keys stay in
 * it while it is up, in the middle of the window on the desktop's scrim,
 * not the system's darker dim, 24 dp in from its edges. Back, Esc and a
 * tap beside what it holds dismiss it; what it holds is floating.
 */
@Composable
fun Modal(onDismiss: () -> Unit, content: @Composable BoxScope.() -> Unit) {
    Dialog(onDismiss, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        val window = (LocalView.current.parent as? DialogWindowProvider)?.window
        SideEffect { window?.setDimAmount(0f) }
        Box(
            Modifier
                .fillMaxSize()
                .background(pk.scrim)
                .pointerInput(onDismiss) { detectTapGestures { onDismiss() } }
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .padding(24.dp),
            contentAlignment = Alignment.Center,
            content = content,
        )
    }
}

/** What a Modal holds: on the ground, with the shadow of what floats, the app's corners; a tap on it is not one beside it. */
@Composable
fun Modifier.floating(): Modifier {
    val p = pk
    return this
        .shadow(12.dp, Corner)
        .clip(Corner)
        .background(p.ground)
        .pointerInput(Unit) { detectTapGestures { } }
}

/** A word that comes and goes at the screen's foot, on term, as the desktop's toast. */
@Composable
fun Toast(text: String, modifier: Modifier = Modifier) {
    val p = pk
    Text(
        text,
        style = Type.ui,
        color = p.termInk,
        modifier = modifier
            .widthIn(max = 420.dp)
            .background(p.term, Corner)
            .padding(horizontal = 16.dp, vertical = 10.dp),
    )
}

/**
 * The desktop's menu: on the ground, its corners the app's, a ring of sunk
 * and the shadow of what floats, its items 6 dp in from its sides and 2
 * apart (app.css, .pk-menu), and Material's 8 above and below them; at
 * most 420 dp wide, or 480 for the time menu's rates (.pk-menu.is-wide).
 * Items are Item and Check below.
 */
@Composable
fun Menu(expanded: Boolean, onDismiss: () -> Unit, gap: Dp = 6.dp, wide: Boolean = false, content: @Composable () -> Unit) {
    val p = pk
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismiss,
        offset = DpOffset(0.dp, gap),
        shape = Corner,
        containerColor = p.ground,
        shadowElevation = 12.dp,
        border = BorderStroke(1.dp, p.sunk),
    ) {
        Column(
            Modifier.widthIn(min = 240.dp, max = if (wide) 480.dp else 420.dp).padding(horizontal = 6.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) { content() }
    }
}

/**
 * A menu's item (app.css, .pk-mi): what it does, under it in mono the file
 * it makes, or in its words what it is (note, .pk-sub, as the model menu
 * has it), and 16 dp after its words, on their line, the key that does it
 * too where there are keys (hint: Kbd or KeyCap), or the check of the one
 * chosen of a menu's choices (selected, not null for a choice).
 */
@Composable
fun Item(
    text: String, sub: String?, hint: (@Composable () -> Unit)? = null, selected: Boolean? = null,
    note: AnnotatedString? = null, onClick: () -> Unit,
) {
    val words = Type.ui.copy(lineHeight = 1.35.em)
    Row(
        Modifier
            .control(CornerSm, role = if (selected != null) Role.RadioButton else Role.Button, onClick = onClick)
            .then(if (selected != null) Modifier.semantics { this.selected = selected } else Modifier)
            .fillMaxWidth()
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(text, style = words, color = pk.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (sub != null)
                Text(sub, style = Type.stamp.copy(lineHeight = 1.4.em), color = pk.inkMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (note != null)
                Text(note, style = Type.small.copy(lineHeight = 1.4.em), color = pk.inkMuted)
        }
        if (hint != null) {
            val line = with(LocalDensity.current) { (words.fontSize * 1.35f).toDp() }
            Box(
                Modifier.align(if (sub != null) Alignment.Top else Alignment.CenterVertically).heightIn(min = line),
                contentAlignment = Alignment.Center,
            ) { hint() }
        }
    }
}

/**
 * Whether there are keys to press: a keyboard as the Googlebook's, or one
 * given to a tablet or a phone. A phone alone has none, and its menus say no
 * keys.
 */
@Composable
fun hasKeys(): Boolean {
    val c = LocalConfiguration.current
    return c.keyboard == Configuration.KEYBOARD_QWERTY && c.hardKeyboardHidden == Configuration.HARDKEYBOARDHIDDEN_NO
}

/**
 * Whether the screen is a finger's: no keyboard, and so no trackpad either,
 * as on a phone or a tablet, where the Googlebook has both, and a tablet's
 * keyboard cover a trackpad with its keys. The input devices cannot say it:
 * a Tab S7 FE with no cover on lists sec_touchpad, a mouse among its
 * sources (2026-10-27). A finger's strip is a thumb's (Strip).
 */
@Composable
fun byFinger(): Boolean = !hasKeys()

/** A shortcut beside its item, Ctrl+S, in the type of the file under it (app.css, .pk-kbd). */
@Composable
fun Kbd(keys: String) {
    Text(keys, style = Type.stamp.copy(lineHeight = 1.em), color = pk.inkMuted, maxLines = 1, softWrap = false)
}

/**
 * A key to press beside its item, on a key's tone step: a lone < or > in the
 * type of a shortcut read as no key at all (app.css, .pk-key).
 */
@Composable
fun KeyCap(key: String) {
    Box(
        Modifier
            .height(22.dp)
            .widthIn(min = 22.dp)
            .background(pk.sunk, RoundedCornerShape(5.dp))
            .padding(horizontal = 6.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(key, style = Type.stamp.copy(fontWeight = FontWeight.W500, lineHeight = 1.em), color = pk.ink, maxLines = 1)
    }
}

/** A menu's choice that stays: a box ticked or not, on its words' first line, and the words (app.css, .pk-check). */
@Composable
fun Check(text: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    val p = pk
    Row(
        Modifier
            .control(CornerSm, role = Role.Checkbox) { onChange(!checked) }
            .fillMaxWidth()
            .padding(horizontal = 10.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(
            Modifier
                .padding(top = 1.dp)
                .size(16.dp)
                .background(if (checked) p.ink else Color.Transparent, RoundedCornerShape(3.dp))
                .border(1.5.dp, if (checked) p.ink else p.inkMuted, RoundedCornerShape(3.dp)),
            contentAlignment = Alignment.Center,
        ) {
            if (checked) Icon(Icons.check, null, tint = p.ground, modifier = Modifier.size(12.dp))
        }
        Text(text, style = Type.ui.copy(fontWeight = FontWeight.W400, lineHeight = 1.35.em), color = p.ink)
    }
}

/** Between a menu's groups, the line the platforms' own menus draw. */
@Composable
fun MenuLine() {
    Box(Modifier.padding(horizontal = 10.dp, vertical = 3.dp).fillMaxWidth().height(1.dp).background(pk.sunk))
}

/**
 * A menu's row of chips, the one chosen in ink (app.css, .pk-rates): the
 * speeds, and the frame rates. top is 8 dp where the chips open the menu,
 * 2 under a choice they serve.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun <T> Rates(chips: List<Pair<String, T>>, chosen: (T) -> Boolean, label: String, top: Dp, onPick: (T) -> Unit) {
    val p = pk
    FlowRow(
        Modifier
            .padding(start = 10.dp, end = 10.dp, top = top, bottom = 8.dp)
            .semantics { contentDescription = label },
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        for ((text, value) in chips) {
            val on = chosen(value)
            Text(
                text,
                style = TextStyle(
                    fontFamily = Fonts.mono, fontWeight = FontWeight.W400, fontSize = 12.sp, lineHeight = 16.sp,
                    lineHeightStyle = CssLine, fontFeatureSettings = "tnum",
                ),
                color = if (on) p.ground else p.ink,
                modifier = Modifier
                    .control(CornerSm, if (on) p.ink else p.surface) { onPick(value) }
                    .padding(horizontal = 7.dp, vertical = 4.dp),
            )
        }
    }
}
