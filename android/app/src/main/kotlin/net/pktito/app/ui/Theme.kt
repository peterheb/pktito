package net.pktito.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf

val LocalPalette = staticCompositionLocalOf { Light }

/** The theme's palette, as the screens name its colours: pk.ink, pk.term. */
val pk: Palette
    @Composable @ReadOnlyComposable get() = LocalPalette.current

/**
 * pktito's look, the desktop window's (design/README.md, ui/app.css): the
 * palette of the theme Android is in, as the desktop follows the system's,
 * and Archivo at the app's working size in ink. Material is under it only
 * for what it lays out, a menu's popup, in the same colours.
 */
@Composable
fun PktitoTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val p = if (dark) Dark else Light
    val scheme = (if (dark) darkColorScheme() else lightColorScheme()).copy(
        primary = p.plume,
        onPrimary = p.onPlume,
        background = p.ground,
        onBackground = p.ink,
        surface = p.ground,
        onSurface = p.ink,
        surfaceContainer = p.ground,
        onSurfaceVariant = p.inkMuted,
        outline = p.sunk,
    )
    MaterialTheme(colorScheme = scheme) {
        CompositionLocalProvider(
            LocalPalette provides p,
            LocalContentColor provides p.ink,
            LocalTextStyle provides Type.ui,
            content = content,
        )
    }
}
