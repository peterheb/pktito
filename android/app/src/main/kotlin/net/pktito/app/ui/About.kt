package net.pktito.app.ui

import android.content.Context
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.fromHtml
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import net.pktito.app.R
import org.json.JSONArray
import org.json.JSONObject

/**
 * About (ui/index.html, #about): the mascot, and under or beside it pktito's
 * name, what it is, its version and the engine's code for this processor,
 * its copyright and links, and the credits: the models, Silero's VAD, and
 * everything else, whose licenses open over About as Privacy does. As the
 * desktop's, the mascot is over the words in a portrait window or one under
 * 600 dp wide, the words as tall as they are and the mascot the rest, and
 * beside them otherwise (app.css, .pk-about). Here the mascot is a picture
 * of it at rest (android/mascot.js), where the desktop's turns and chirps.
 */
@Composable
fun About(version: String, onBack: () -> Unit, onLink: (String) -> Unit) {
    val p = pk
    var sheet by rememberSaveable { mutableStateOf("") }
    val title = stringResource(R.string.about_open)
    BoxWithConstraints(Modifier.fillMaxSize().semantics { paneTitle = title }) {
        val narrow = maxWidth < 600.dp
        val stacked = narrow || maxHeight >= maxWidth
        val wide = maxWidth >= 880.dp
        val words = @Composable { mod: Modifier ->
            // the credits beside their names while there is room for both
            Credits(version, twoColumns = stacked || wide, onLink, { sheet = it }, mod)
        }
        val stage = @Composable { mod: Modifier ->
            Box(mod.background(p.surface)) {
                Image(
                    painterResource(R.drawable.mascot), stringResource(R.string.about_mascot),
                    Modifier.fillMaxSize(), contentScale = ContentScale.Fit,
                )
            }
        }
        if (stacked) {
            // the mascot what the words leave, and 160 dp at the least: a
            // phone scrolls the two together where the words fill it
            val tall = constraints.maxHeight
            Layout(
                {
                    stage(Modifier)
                    words(
                        if (narrow) Modifier.padding(start = 20.dp, end = 20.dp, top = 24.dp, bottom = 28.dp)
                        else Modifier.padding(start = 40.dp, end = 40.dp, top = 28.dp, bottom = 32.dp),
                    )
                },
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()),
            ) { (s, w), c ->
                val text = w.measure(c.copy(minWidth = 0, minHeight = 0, maxHeight = Constraints.Infinity))
                val art = s.measure(Constraints.fixed(c.maxWidth, maxOf(tall - text.height, 160.dp.roundToPx())))
                layout(c.maxWidth, art.height + text.height) {
                    art.place(0, 0)
                    text.place(0, art.height)
                }
            }
        } else
            Row(Modifier.fillMaxSize()) {
                stage(Modifier.weight(1.08f).fillMaxHeight())
                // in the middle while they fit, else scrolled from their top
                Column(
                    Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .verticalScroll(rememberScrollState())
                        .padding(if (wide) PaddingValues(40.dp, 32.dp) else PaddingValues(24.dp, 28.dp)),
                    verticalArrangement = Arrangement.Center,
                ) { words(Modifier) }
            }
        // where the work screen's way back is
        IconBtn(Icons.back, stringResource(R.string.back), onBack, Modifier.padding(start = 8.dp, top = 11.dp), kind = Kind.Ghost)
        when (sheet) {
            "privacy" -> Sheet(stringResource(R.string.privacy_title), narrow = true, onClose = { sheet = "" }) { Privacy(onLink, it) }
            "notices" -> Sheet(stringResource(R.string.notices_title), narrow = false, onClose = { sheet = "" }) { Licenses(it) }
        }
    }
}

/** About's words, at most the desktop's measure wide (.pk-about-in): 34 em of its 14 px. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Credits(version: String, twoColumns: Boolean, onLink: (String) -> Unit, open: (String) -> Unit, modifier: Modifier) {
    val p = pk
    val text = TextStyle(fontFamily = Fonts.sans, fontWeight = FontWeight.W400, fontSize = 15.sp, lineHeight = 1.5.em, lineHeightStyle = CssLine)
    Column(modifier.widthIn(max = 476.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Wordmark(Modifier.padding(bottom = 8.dp), size = 56.sp)
        Words(html(stringResource(R.string.about_tagline), onLink), text, p.ink)
        Column {
            Text(version, style = Type.small, color = p.inkMuted)
            Words(html(stringResource(R.string.about_copyright), onLink), Type.small, p.inkMuted)
            FlowRow(
                Modifier.padding(top = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(20.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Words(link(stringResource(R.string.about_docs), "https://pktito.net/docs", onLink), Type.small, p.inkMuted)
                Words(button(stringResource(R.string.about_privacy)) { open("privacy") }, Type.small.copy(lineHeight = 20.sp), p.inkMuted)
                Words(link("GitHub", "https://github.com/peterheb/pktito", onLink), Type.small, p.inkMuted)
            }
        }
        Column(
            Modifier.padding(top = 20.dp),
            verticalArrangement = Arrangement.spacedBy(if (twoColumns) 10.dp else 12.dp),
        ) {
            Credit("Parakeet-TDT", twoColumns) {
                Words(html(stringResource(R.string.about_models), onLink), Type.small, p.inkMuted)
            }
            Credit("Silero VAD", twoColumns) {
                Words(html(stringResource(R.string.about_silero), onLink), Type.small, p.inkMuted)
            }
            Credit(stringResource(R.string.about_else), twoColumns) {
                Column {
                    Text(stringResource(R.string.about_else_list), style = Type.small, color = p.inkMuted)
                    Words(button(stringResource(R.string.about_licenses)) { open("notices") }, Type.small.copy(lineHeight = 20.sp), p.inkMuted)
                }
            }
        }
    }
}

/** A credit, its name before it 7.5 em of 14 sp wide, or above it where the words are narrow (.pk-credits). */
@Composable
private fun Credit(name: String, twoColumns: Boolean, what: @Composable () -> Unit) {
    val p = pk
    val style = TextStyle(fontFamily = Fonts.sans, fontWeight = FontWeight.W500, fontSize = 14.sp, lineHeight = 1.45.em, lineHeightStyle = CssLine)
    if (twoColumns)
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(name, style = style, color = p.ink, modifier = Modifier.width(105.dp))
            Box(Modifier.weight(1f)) { what() }
        }
    else
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(name, style = style, color = p.ink)
            what()
        }
}

/** The inline content of a link that leaves the app: its icon. */
private const val EXTERNAL = "external"

/** Text with links in it, each in the link's colour, underlined; one that leaves the app with its icon after it. */
@Composable
private fun Words(s: AnnotatedString, style: TextStyle, color: Color, modifier: Modifier = Modifier) {
    val p = pk
    // 12 sp after 3, its foot 1 under the line (.pk-about a .pk-ico)
    val icon = InlineTextContent(Placeholder(15.sp, 12.sp, PlaceholderVerticalAlign.AboveBaseline)) {
        Icon(Icons.external, null, tint = p.link, modifier = Modifier.padding(start = 3.dp).offset(y = 1.dp).fillMaxSize())
    }
    Text(s, modifier, style = style, color = color, inlineContent = mapOf(EXTERNAL to icon))
}

@Composable
private fun linkStyles() = TextLinkStyles(SpanStyle(color = pk.link, textDecoration = TextDecoration.Underline))

/**
 * One of the desktop's *_html strings (strings.xml, in CDATA), its links
 * opened by onLink; each, as all of About's links go to another site, with
 * the icon of one that leaves the app, held to its last word (app.js).
 */
@Composable
private fun html(s: String, onLink: (String) -> Unit): AnnotatedString {
    val styles = linkStyles()
    return remember(s, styles) {
        external(AnnotatedString.fromHtml(s, styles) { onLink((it as LinkAnnotation.Url).url) })
    }
}

@Composable
private fun link(text: String, url: String, onLink: (String) -> Unit): AnnotatedString {
    val styles = linkStyles()
    return remember(text, url, styles) {
        external(buildAnnotatedString { withLink(LinkAnnotation.Url(url, styles) { onLink(url) }) { append(text) } })
    }
}

/** Privacy or the licenses: words that open a sheet, written as a link is, without the icon (.pk-linkbtn). */
@Composable
private fun button(text: String, onClick: () -> Unit): AnnotatedString {
    val styles = linkStyles()
    return buildAnnotatedString { withLink(LinkAnnotation.Clickable(text, styles) { onClick() }) { append(text) } }
}

private fun external(s: AnnotatedString) = buildAnnotatedString {
    var at = 0
    for (r in s.getLinkAnnotations(0, s.length).sortedBy { it.start }) {
        append(s.subSequence(at, r.end))
        // the word joiner keeps the icon on its word's line; it stands for
        // the icon too, which a reader skips, as the desktop's is hidden
        // from one (aria-hidden): an arrow there was read out
        withLink(r.item) { append("\u2060"); appendInlineContent(EXTERNAL, "\u2060") }
        at = r.end
    }
    append(s.subSequence(at, s.length))
}

/**
 * A sheet over About (app.css, .pk-notices): its title and the way to close
 * it on surface, what it holds scrolling under them, in the middle of the
 * window on the desktop's scrim, 600 dp wide at most for Privacy, as tall
 * as it is, and 760 by 760 for the licenses. A dialog, so that a reader and
 * the keys stay in it while it is up; Back, Esc and a tap beside it close it.
 */
@Composable
private fun Sheet(title: String, narrow: Boolean, onClose: () -> Unit, body: @Composable (Modifier) -> Unit) {
    Dialog(onClose, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        // the desktop's scrim, not the system's darker dim
        val window = (LocalView.current.parent as? DialogWindowProvider)?.window
        SideEffect { window?.setDimAmount(0f) }
        val p = pk
        Box(
            Modifier
                .fillMaxSize()
                .background(p.scrim)
                .pointerInput(onClose) { detectTapGestures { onClose() } }
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .padding(24.dp),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                Modifier
                    .widthIn(max = if (narrow) 600.dp else 760.dp)
                    .heightIn(max = 760.dp)
                    .then(if (narrow) Modifier else Modifier.fillMaxHeight())
                    .fillMaxWidth()
                    .testTag("sheet")
                    .shadow(12.dp, Corner)
                    .clip(Corner)
                    .background(p.ground)
                    // a tap on it is not one beside it
                    .pointerInput(Unit) { detectTapGestures { } },
            ) {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .background(p.surface)
                        .padding(start = 24.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(title, style = Type.ui, color = p.ink, modifier = Modifier.weight(1f))
                    IconBtn(Icons.x, stringResource(R.string.sheet_close), onClose, kind = Kind.Ghost)
                }
                body(Modifier.weight(1f, fill = !narrow))
            }
        }
    }
}

private val sheetText = TextStyle(fontFamily = Fonts.sans, fontWeight = FontWeight.W400, fontSize = 14.sp, lineHeight = 1.4.em, lineHeightStyle = CssLine)

/** Privacy's words (ui/index.html, #privacy), as Android has them, and a file's own section for its pickers. */
@Composable
private fun Privacy(onLink: (String) -> Unit, modifier: Modifier) {
    val p = pk
    val heading = TextStyle(
        fontFamily = Fonts.sans, fontWeight = FontWeight.W700, fontSize = 17.sp, lineHeight = 1.3.em,
        lineHeightStyle = CssLine, letterSpacing = (-0.005).em,
    )
    val para = @Composable { s: AnnotatedString -> Words(s, sheetText, p.ink, Modifier.widthIn(max = 560.dp).padding(bottom = 12.dp)) }
    val h3 = @Composable { s: String -> Text(s, style = heading, color = p.ink, modifier = Modifier.padding(top = 16.dp, bottom = 6.dp)) }
    val item = @Composable { s: String ->
        Row(Modifier.widthIn(max = 560.dp).padding(bottom = 6.dp)) {
            Box(Modifier.width(22.dp)) { Text("•", style = sheetText, color = p.ink, modifier = Modifier.align(Alignment.TopEnd).padding(end = 8.dp)) }
            Text(s, style = sheetText, color = p.ink, modifier = Modifier.weight(1f))
        }
    }
    SelectionContainer(modifier) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 28.dp)) {
            para(AnnotatedString(stringResource(R.string.privacy_intro)))
            h3(stringResource(R.string.privacy_internet))
            para(html(stringResource(R.string.privacy_download), onLink))
            para(html(stringResource(R.string.privacy_links), onLink))
            h3(stringResource(R.string.privacy_files))
            para(AnnotatedString(stringResource(R.string.privacy_files_text)))
            h3(stringResource(R.string.privacy_choices))
            Column(Modifier.padding(bottom = 6.dp)) {
                item(stringResource(R.string.privacy_choice_file))
                item(stringResource(R.string.privacy_choice_uninstall))
            }
            h3(stringResource(R.string.privacy_questions))
            para(html(stringResource(R.string.privacy_ask), onLink))
            Text(stringResource(R.string.privacy_updated), style = Type.small, color = p.inkMuted)
        }
    }
}

/** A row of the licenses: what it is, its license, its copyright, its texts' keys, and whether its name is code. */
private class Notice(val name: String, val license: String, val notice: String, val keys: List<String>, val code: Boolean)

/** assets/notices.json, which android/notices.py makes: the typefaces and icons, the libraries, and the texts by key. */
private class Notices(val type: List<Notice>, val libraries: List<Notice>, val texts: Map<String, Pair<String, String>>) {
    companion object {
        fun read(context: Context): Notices {
            val j = JSONObject(context.assets.open("notices.json").use { String(it.readBytes()) })
            fun strings(a: JSONArray) = List(a.length()) { a.getString(it) }
            fun rows(a: JSONArray, code: Boolean?) = List(a.length()) {
                val r = a.getJSONArray(it)
                Notice(r.getString(0), r.getString(1), r.getString(2), strings(r.getJSONArray(3)), code ?: r.getBoolean(4))
            }
            val t = j.getJSONObject("texts")
            return Notices(
                rows(j.getJSONArray("type"), false),
                rows(j.getJSONArray("libraries"), null),
                t.keys().asSequence().associateWith { t.getJSONObject(it).let { x -> x.getString("title") to x.getString("text") } },
            )
        }
    }
}

/**
 * The licenses (app.js, noticesHtml): the typefaces and icons, the
 * libraries, and the texts they are under, each once, in the order the list
 * first names them.
 */
@Composable
private fun Licenses(modifier: Modifier) {
    val p = pk
    val context = LocalContext.current
    val n = remember { Notices.read(context) }
    val keys = remember(n) { (n.type + n.libraries).flatMap { it.keys }.distinct() }
    val h3 = TextStyle(fontFamily = Fonts.sans, fontWeight = FontWeight.W500, fontSize = 14.sp, lineHeight = 1.4.em, lineHeightStyle = CssLine)
    val h4 = h3.copy(fontSize = 13.sp)
    LazyColumn(modifier, contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 28.dp)) {
        item { Text(stringResource(R.string.notices_intro), style = Type.small, color = p.inkMuted) }
        item { Text(stringResource(R.string.notices_type), style = h3, color = p.ink, modifier = Modifier.padding(top = 24.dp, bottom = 6.dp)) }
        items(n.type.size) { NoticeRow(n.type[it]) }
        item { Text(stringResource(R.string.notices_libraries), style = h3, color = p.ink, modifier = Modifier.padding(top = 24.dp, bottom = 6.dp)) }
        items(n.libraries.size) { NoticeRow(n.libraries[it]) }
        item { Text(stringResource(R.string.notices_texts), style = h3, color = p.ink, modifier = Modifier.padding(top = 24.dp)) }
        items(keys.size) {
            val (title, text) = n.texts.getValue(keys[it])
            Column {
                Text(title, style = h4, color = p.ink, modifier = Modifier.padding(top = 20.dp, bottom = 6.dp))
                SelectionContainer {
                    Text(
                        text.trimEnd(),
                        style = TextStyle(fontFamily = Fonts.mono, fontSize = 12.sp, lineHeight = 1.5.em, lineHeightStyle = CssLine),
                        color = p.ink,
                        modifier = Modifier.fillMaxWidth().background(p.surface, Corner).padding(horizontal = 14.dp, vertical = 12.dp),
                    )
                }
            }
        }
    }
}

/**
 * A row of the licenses (.pk-nlist li): its name, a crate's or a module's
 * in mono, and its license, and under them its copyright, as the desktop's
 * window has them under 880 px. Its three columns over that are for
 * crates' names; a module's is twice as long (androidx.compose.foundation:
 * foundation-layout-android), and in 15 em it broke in every row. A
 * module's may break after its group.
 */
@Composable
private fun NoticeRow(x: Notice) {
    val p = pk
    val name = if (x.code)
        TextStyle(fontFamily = Fonts.mono, fontWeight = FontWeight.W400, fontSize = 12.5.sp, lineHeight = 1.55.em, lineHeightStyle = CssLine)
    else Type.small
    Column(Modifier.padding(vertical = 3.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(x.name.replace(":", ":\u200B"), style = name, color = p.ink, modifier = Modifier.weight(1f))
            Text(x.license, style = Type.small, color = p.inkMuted)
        }
        Text(x.notice, style = Type.small, color = p.inkMuted)
    }
}
