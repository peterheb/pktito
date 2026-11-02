package net.pktito.app

import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.em
import net.pktito.app.ui.PktitoTheme
import net.pktito.app.ui.Type
import net.pktito.app.ui.cssLine
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Why cssLine is there: Compose lays a line out at least as tall as its
 * font, 64 sp of Archivo 700 at 70 dp whether the style asks 0.9 em or 1
 * (only above that does it follow), where CSS's 64px/0.9 is 57.6 tall; and
 * that cssLine makes it so. At mdpi, where a dp is a pixel.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-mdpi")
class LinesTest {
    @get:Rule
    val compose = createComposeRule()

    private fun heights(): Map<String, Int> {
        val got = mutableMapOf<String, IntSize>()
        compose.setContent {
            PktitoTheme {
                Text("41%", style = Type.figure, modifier = Modifier.onGloballyPositioned { got["0.9"] = it.size })
                Text("41%", style = Type.figure.copy(lineHeight = 1.em), modifier = Modifier.onGloballyPositioned { got["1"] = it.size })
                Text("41%", style = Type.figure.copy(lineHeight = 1.2.em), modifier = Modifier.onGloballyPositioned { got["1.2"] = it.size })
                Text("41%", style = Type.figure, modifier = Modifier.onGloballyPositioned { got["css"] = it.size }.cssLine(Type.figure))
            }
        }
        compose.waitForIdle()
        return got.mapValues { it.value.height }
    }

    @Test
    fun aLineShorterThanItsFontIsTheFontsUntilCssLine() {
        val h = heights()
        assertEquals(70, h["0.9"])
        assertEquals(70, h["1"])
        assertEquals(77, h["1.2"])
        assertEquals(58, h["css"])
    }
}
