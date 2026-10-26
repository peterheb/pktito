package net.pktito.app.ui

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
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

val Corner = RoundedCornerShape(Radius.radiusUi)
val CornerSm = RoundedCornerShape(Radius.radiusSm)

/**
 * Hover and press as every control of the desktop's has them, a tint laid
 * over its fill or what it sits on (design/README.md): darker in light,
 * lighter in dark, term's own on the strip. Keyboard focus is a 2 dp ring,
 * ink in light and plume in dark. No ripple: the desktop has none.
 */
@Composable
fun Modifier.control(
    shape: Shape,
    fill: Color = Color.Transparent,
    onTerm: Boolean = false,
    enabled: Boolean = true,
    role: Role = Role.Button,
    label: String? = null,
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
        .clickable(source, null, enabled, role = role, onClick = onClick)
        .then(if (label != null) Modifier.semantics { contentDescription = label } else Modifier)
}

enum class Kind { Default, Primary, Ghost }

/**
 * A button: sunk at rest, plume for the one a screen is for, ghost for one
 * that waits its turn (Stop). 40 dp tall, past the desktop's 32 for a
 * finger, 48 large; a dimmed one is disabled.
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
) {
    val p = pk
    val fill = when (kind) {
        Kind.Primary -> p.plume
        Kind.Default -> p.sunk
        Kind.Ghost -> Color.Transparent
    }
    val ink = if (kind == Kind.Primary) p.onPlume else p.ink
    Row(
        modifier
            .alpha(if (enabled) 1f else 0.45f)
            .control(Corner, fill, enabled = enabled, onClick = onClick)
            .heightIn(min = if (large) 48.dp else 40.dp)
            .padding(horizontal = if (large) 18.dp else 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
    ) {
        Text(text, style = Type.ui.copy(fontSize = if (large) 15.sp else 14.sp), color = ink, maxLines = 1)
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
    Row(modifier) {
        Text(
            "pktito",
            style = TextStyle(
                fontFamily = Fonts.sans, fontWeight = FontWeight.W700, fontSize = size,
                lineHeight = 1.em, lineHeightStyle = CssLine, letterSpacing = (-0.03).em,
            ),
            color = pk.ink,
            modifier = Modifier.alignByBaseline(),
        )
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
 * and the shadow of what floats; items are Item and Check below.
 */
@Composable
fun Menu(expanded: Boolean, onDismiss: () -> Unit, content: @Composable () -> Unit) {
    val p = pk
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismiss,
        offset = DpOffset(0.dp, 6.dp),
        shape = Corner,
        containerColor = p.ground,
        shadowElevation = 12.dp,
        border = BorderStroke(1.dp, p.sunk),
    ) {
        Column(
            Modifier.widthIn(min = 240.dp).padding(horizontal = 6.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) { content() }
    }
}

/** A menu's item: what it does, and under it, in mono, the file it makes. */
@Composable
fun Item(text: String, sub: String?, onClick: () -> Unit) {
    Column(
        Modifier
            .control(CornerSm, onClick = onClick)
            .fillMaxWidth()
            .padding(horizontal = 10.dp, vertical = 8.dp),
    ) {
        Text(text, style = Type.ui.copy(lineHeight = 1.35.em), color = pk.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (sub != null)
            Text(sub, style = Type.stamp.copy(lineHeight = 1.4.em), color = pk.inkMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
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
