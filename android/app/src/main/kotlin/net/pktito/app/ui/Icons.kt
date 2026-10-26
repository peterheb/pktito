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
    val check = icon("check", "M20 6L9 17L4 12")
    val alert = icon("alert", "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z", "M12 8v4", "M12 16h0.01")
    // feather's share, in the set the desktop's icons come from
    val share = icon("share", "M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8", "M16 6L12 2L8 6", "M12 2v13")
    val play = icon("play", "M6 3L20 12L6 21Z", filled = true)
    val pause = icon("pause", "M6 4h4v16H6z", "M14 4h4v16h-4z", filled = true)
}
