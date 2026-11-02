package net.pktito.app.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/**
 * The desktop's icons (ui/app.js, ICON), on the same 24-unit grid: lines
 * 2 wide with round ends, drawn in their button's colour; play and pause
 * filled. Each SVG shape is written here as a path.
 */
object Icons {
    private fun icon(name: String, vararg paths: String, filled: Boolean = false): ImageVector {
        val b = ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f)
        for (d in paths) {
            if (filled)
                b.addPath(addPathNodes(d), fill = SolidColor(Color.Black))
            else
                b.addPath(
                    addPathNodes(d), stroke = SolidColor(Color.Black), strokeLineWidth = 2f,
                    strokeLineCap = StrokeCap.Round, strokeLineJoin = StrokeJoin.Round,
                )
        }
        return b.build()
    }

    val back = icon("back", "M19 12H5", "M12 19L5 12L12 5")
    val copy = icon(
        "copy",
        "M11 9h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2v-9a2 2 0 0 1 2-2z",
        "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1",
    )
    val down = icon("down", "M6 9L12 15L18 9")
    val up = icon("up", "M18 15L12 9L6 15")
    // the finder's glass
    val search = icon("search", "M11 3a8 8 0 1 0 0 16a8 8 0 1 0 0-16z", "M21 21L16.65 16.65")
    val check = icon("check", "M20 6L9 17L4 12")
    val download = icon("download", "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M7 10L12 15L17 10", "M12 15V3")
    val alert = icon("alert", "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z", "M12 8v4", "M12 16h0.01")
    val x = icon("x", "M18 6L6 18", "M6 6L18 18")
    // a recent transcript's, before its name
    val file = icon("file", "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z", "M14 2L14 8L20 8", "M16 13H8", "M16 17H8")
    // after a link that leaves the app, as the desktop's About has it
    val external = icon("external", "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6", "M15 3h6v6", "M10 14L21 3")
    val play = icon("play", "M6 3L20 12L6 21Z", filled = true)
    val pause = icon("pause", "M6 4h4v16H6z", "M14 4h4v16h-4z", filled = true)
    // the speaker, and its sound: two waves from half the volume up, one
    // under it, a cross at none
    private const val SPEAKER = "M11 5L6 9H2v6h4l5 4z"
    val vol2 = icon("vol2", SPEAKER, "M19.07 4.93a10 10 0 0 1 0 14.14", "M15.54 8.46a5 5 0 0 1 0 7.07")
    val vol1 = icon("vol1", SPEAKER, "M15.54 8.46a5 5 0 0 1 0 7.07")
    val volX = icon("volX", SPEAKER, "M23 9L17 15", "M17 9L23 15")
}
