package net.pktito.app.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.collapse
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.expand
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import java.util.Locale
import net.pktito.app.Copy
import net.pktito.app.Download
import net.pktito.app.Models
import net.pktito.app.Offer
import net.pktito.app.R
import net.pktito.app.mb
import net.pktito.app.percent
import net.pktito.app.progress
import net.pktito.app.words

/** What the setup screen does with what it is asked. */
class SetupActions(
    /** Download a model, or go on with its download. */
    val download: (Offer) -> Unit,
    /** Use a model that is here. */
    val use: (Offer) -> Unit,
    val pause: () -> Unit,
    val cancel: () -> Unit,
    val retry: () -> Unit,
    /** A model file of one's own, from the system's picker. */
    val ownFile: () -> Unit,
    val link: (String) -> Unit,
    val about: () -> Unit,
)

/**
 * The setup screen (ui/app.js, renderSetup): choose a speech model and
 * download it, or use a model file of one's own, in the desktop's modes,
 * the choice, a download under way or paused, one that failed; and one of
 * Android's own, a model file of one's own being copied in (Models.open).
 * waiting counts the recordings opened or shared before there was a model,
 * which are transcribed once there is.
 */
@Composable
fun Setup(offers: List<Offer>, model: String?, download: Download?, copying: Copy?, waiting: Int, a: SetupActions) {
    when {
        copying != null -> Copying(copying, a)
        download == null -> Choose(offers, model, waiting, a)
        download.step == "failed" -> Failed(download, a)
        else -> Downloading(offers, download, waiting, a)
    }
}

/** The system's language, which a model of several may speak. */
private fun ownLanguage() = Locale.getDefault().language

/** A language's name in English, the screen's language (app.js, langName). */
private fun languageName(code: String) = Locale.forLanguageTag(code).getDisplayLanguage(Locale.ENGLISH).ifEmpty { code }

/** .pk-text: 400 15/1.5 */
private val Text15 = Type.body.copy(fontSize = 15.sp)

/**
 * The screen's frame (app.css, .pk-center > .pk-screen): a column 640 dp
 * wide at most, 20 dp in from a narrow window's sides and 40 from a wider
 * one's, at least 520 tall where the window has that, in the middle of the
 * window; what it says at its top, 20 apart, and its foot at its bottom.
 * Taller than the window, it scrolls.
 */
@Composable
private fun Screen(top: @Composable ColumnScope.() -> Unit, foot: @Composable ColumnScope.() -> Unit) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val side = if (maxWidth < 600.dp) 20.dp else 40.dp
        val least = minOf(maxHeight, 520.dp)
        Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), contentAlignment = Alignment.Center) {
            Column(
                Modifier
                    .widthIn(max = 640.dp)
                    .fillMaxWidth()
                    .heightIn(min = least)
                    .padding(start = side, end = side, top = 28.dp, bottom = 24.dp),
                verticalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(20.dp), content = top)
                Column(Modifier.padding(top = 20.dp), content = foot)
            }
        }
    }
}

/** The screen's heading, and the words under it (.pk-intro). */
@Composable
private fun Intro(title: String, text: String? = null) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = Type.lead, color = pk.ink, modifier = Modifier.semantics { heading() })
        if (text != null) Text(text, style = Text15, color = pk.ink)
    }
}

/**
 * Two things on a line, one at its start and one at its end, 16 dp apart
 * at the least, else the second under the first (.pk-spread, which wraps);
 * on a line, their middles level, or their feet where bottom is.
 */
@Composable
private fun Spread(bottom: Boolean = false, content: @Composable () -> Unit) {
    Layout(content, Modifier.fillMaxWidth()) { ms, c ->
        val gap = 16.dp.roundToPx()
        val ps = ms.map { it.measure(c.copy(minWidth = 0, minHeight = 0)) }
        val w = c.maxWidth
        if (ps.size < 2 || ps.sumOf { it.width } + gap <= w) {
            val h = ps.maxOfOrNull { it.height } ?: 0
            layout(w, h) {
                ps.forEachIndexed { i, p ->
                    val y = if (bottom) h - p.height else (h - p.height) / 2
                    p.place(if (i == 0) 0 else w - p.width, y)
                }
            }
        } else {
            layout(w, ps[0].height + gap + ps[1].height) {
                ps[0].place(0, 0)
                ps[1].place(0, ps[0].height + gap)
            }
        }
    }
}

/**
 * The choice (app.js, chooseHtml): a card for each model, the one that
 * speaks the system's language chosen to begin with, else the one in use;
 * its button says what choosing it does, and under it the way to a model
 * file of one's own.
 */
@Composable
private fun Choose(offers: List<Offer>, model: String?, waiting: Int, a: SetupActions) {
    val res = LocalResources.current
    val own = ownLanguage()
    var choice by rememberSaveable { mutableStateOf("") }
    val chosen = offers.find { it.name == choice } ?: offers.find { it.name == model }
        ?: offers.find { own in it.languages } ?: offers.firstOrNull() ?: return
    val legend = stringResource(R.string.setup_legend)
    Screen(
        top = {
            WordmarkButton(a.about)
            Intro(stringResource(R.string.setup_title))
            Column(
                Modifier.selectableGroup().semantics { contentDescription = legend },
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                for (o in offers) Card(o, o.name == chosen.name, own, { choice = o.name }, a.link)
            }
        },
        foot = {
            val label = Models.label(res, chosen.key)
            val go = when {
                chosen.have -> stringResource(R.string.setup_use, label)
                chosen.partial > 0 -> stringResource(R.string.setup_download_rest, label, mb(chosen.bytes - chosen.partial))
                else -> stringResource(R.string.setup_download, label, mb(chosen.bytes))
            }
            Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                PkButton(
                    go, { if (chosen.have) a.use(chosen) else a.download(chosen) }, Modifier.fillMaxWidth(),
                    kind = Kind.Primary, large = true, leading = if (chosen.have) Icons.check else Icons.download,
                )
                Spread {
                    LinkButton(stringResource(R.string.setup_have_file), a.ownFile)
                    if (waiting > 0)
                        Text(pluralStringResource(R.plurals.setup_waiting, waiting, waiting), style = Type.small, color = pk.inkMuted)
                }
            }
        },
    )
}

/**
 * A model's card (.pk-choice): its radio, its name, its size or that it is
 * here; under them its file's name and what it transcribes, a model of
 * several's count opening their list, the system's language in ink, and
 * under it that accuracy varies, with the way to how much. The one chosen
 * on the ground in an ink ring, the others on surface.
 */
@Composable
private fun Card(o: Offer, on: Boolean, own: String, onChoose: () -> Unit, onLink: (String) -> Unit) {
    val p = pk
    val res = LocalResources.current
    var open by rememberSaveable(o.name) { mutableStateOf(false) }
    val many = o.languages.size > 1
    val about = Models.about(res, o.key)
    val among = if (many && own in o.languages) stringResource(R.string.models_among, languageName(own)) else ""
    // the count as a button that reads as a link, in the card's colour (.pk-linkbtn-sm)
    val count = TextLinkStyles(SpanStyle(textDecoration = TextDecoration.Underline))
    val desc = remember(o.name, about, among, count) {
        buildAnnotatedString {
            append(o.name)
            append(" · ")
            if (many) withLink(LinkAnnotation.Clickable("languages", count) { open = !open }) { append(about) }
            else append(about)
            append(among)
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .then(if (on) Modifier.border(2.dp, p.ink, Corner) else Modifier)
            .control(Corner, if (on) p.ground else p.surface, role = Role.RadioButton, onClick = onChoose)
            .semantics { selected = on }
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Row {
            Box(Modifier.width(18.dp).align(Alignment.CenterVertically)) { Radio(on) }
            Spacer(Modifier.width(12.dp))
            Text(
                Models.label(res, o.key),
                style = TextStyle(fontFamily = Fonts.sans, fontWeight = FontWeight.W700, fontSize = 16.sp, lineHeight = 1.3.em, lineHeightStyle = CssLine, letterSpacing = (-0.01).em),
                color = p.ink,
                modifier = Modifier.weight(1f).alignByBaseline(),
            )
            Text(
                if (o.have) stringResource(R.string.setup_on_device) else stringResource(R.string.setup_size, mb(o.bytes)),
                style = Type.stamp, color = p.inkMuted, maxLines = 1, modifier = Modifier.alignByBaseline(),
            )
        }
        Column(Modifier.padding(start = 30.dp)) {
            Text(desc, style = Type.small, color = p.inkMuted)
            if (many && open) Languages(o, own, onLink)
        }
    }
}

/** A radio button in ink, as the desktop's takes the accent colour (.pk-choice input), 16 dp. */
@Composable
private fun Radio(on: Boolean) {
    val p = pk
    Canvas(Modifier.size(16.dp)) {
        val w = 1.5.dp.toPx()
        drawCircle(if (on) p.ink else p.inkMuted, radius = size.minDimension / 2 - w / 2, style = Stroke(w))
        if (on) drawCircle(p.ink, radius = size.minDimension / 4)
    }
}

/**
 * A model of several's languages (.pk-langs), by their English names in
 * three columns, down each, the system's own in ink; and under them, where
 * its card links to it, that accuracy varies, and the way to its table.
 */
@Composable
private fun Languages(o: Offer, own: String, onLink: (String) -> Unit) {
    val p = pk
    val names = o.languages.map { it to languageName(it) }.sortedBy { it.second }
    val rows = (names.size + 2) / 3
    val style = Type.small.copy(lineHeight = 1.6.em)
    Column(Modifier.padding(top = 8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            for (c in 0 until 3)
                Column(Modifier.weight(1f)) {
                    for ((code, name) in names.drop(c * rows).take(rows))
                        Text(
                            name,
                            style = if (code == own) style.copy(fontWeight = FontWeight.W500) else style,
                            color = if (code == own) p.ink else p.inkMuted,
                        )
                }
        }
        if (o.accuracy.isNotEmpty()) {
            val details = link(stringResource(R.string.setup_details), o.accuracy, onLink, color = p.inkMuted)
            val note = stringResource(R.string.setup_accuracy)
            Words(
                remember(note, details) { buildAnnotatedString { append("$note "); append(details) } },
                Type.small, p.inkMuted, Modifier.padding(top = 8.dp), linkColor = p.inkMuted,
            )
        }
    }
}

/**
 * A download under way (app.js, downloadHtml): the model's, from where, how
 * far in a figure and in megabytes, at what speed and how long is left, its
 * bar, and the buttons to pause it or let it go.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Downloading(offers: List<Offer>, d: Download, waiting: Int, a: SetupActions) {
    val p = pk
    val res = LocalResources.current
    val label = offers.find { it.name == d.name }?.let { Models.label(res, it.key) } ?: d.name
    val done = if (d.total > 0) d.got.toDouble() / d.total else 0.0
    val paused = d.step == "paused"
    Screen(
        top = {
            WordmarkButton(a.about)
            Intro(stringResource(R.string.download_title, label), stringResource(R.string.download_from, d.name, d.host.ifEmpty { "models.pktito.net" }))
            Column(Modifier.padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Spread(bottom = true) {
                    Figure(percent(if (d.step == "checking") 1.0 else done))
                    Text(progress(res, d), style = Type.stamp, color = p.inkMuted, modifier = Modifier.padding(bottom = 4.dp))
                }
                Bar(done, stopped = d.step != "downloading", stringResource(R.string.download_bar))
            }
        },
        foot = {
            FlowRow(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                PkButton(
                    stringResource(if (paused) R.string.download_resume else R.string.download_pause),
                    if (paused) a.retry else a.pause,
                    kind = if (paused) Kind.Primary else Kind.Default, large = true, enabled = d.step != "checking",
                )
                PkButton(stringResource(R.string.download_cancel), a.cancel, kind = Kind.Ghost, large = true)
                if (waiting > 0)
                    Text(
                        pluralStringResource(R.plurals.setup_waiting, waiting, waiting), style = Type.small, color = p.inkMuted,
                        textAlign = TextAlign.End, modifier = Modifier.weight(1f),
                    )
            }
        },
    )
}

/**
 * A download that stopped (app.js, failedHtml): what stopped it in a few
 * words, what it has kept, whether it is trying again by itself, the
 * particulars folded away, and the ways on: try again, a model file of
 * one's own, or let it go.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Failed(d: Download, a: SetupActions) {
    val p = pk
    val res = LocalResources.current
    val done = if (d.total > 0) d.got.toDouble() / d.total else 0.0
    Screen(
        top = {
            WordmarkButton(a.about)
            Intro(words(res, d.problem?.key, d.problem?.host ?: ""))
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Bar(done, stopped = true, stringResource(R.string.download_bar_stopped))
                Spread {
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.alert, null, tint = p.err, modifier = Modifier.size(16.dp))
                        Text(
                            if (d.got > 0) stringResource(R.string.download_stopped_at, mb(d.got), mb(d.total))
                            else stringResource(R.string.download_nothing_yet),
                            style = Type.small, color = p.err,
                        )
                    }
                    if (d.retry && !d.trying) Text(stringResource(R.string.download_retrying), style = Type.small, color = p.inkMuted)
                }
            }
            val detail = d.problem?.detail ?: ""
            if (detail.isNotEmpty()) Details(detail)
        },
        foot = {
            FlowRow(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                PkButton(
                    stringResource(if (d.trying) R.string.download_trying else R.string.download_try_again), a.retry,
                    kind = Kind.Primary, large = true, enabled = !d.trying,
                )
                PkButton(stringResource(R.string.download_use_file), a.ownFile, large = true)
                PkButton(stringResource(R.string.download_cancel), a.cancel, kind = Kind.Ghost, large = true)
            }
        },
    )
}

/** A failure's particulars, folded away under Details (.pk-details), in mono, to select and copy. */
@Composable
private fun Details(detail: String) {
    val p = pk
    var open by rememberSaveable { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().background(p.sunk, Corner).padding(horizontal = 14.dp, vertical = 12.dp)) {
        Row(
            Modifier
                .bleed(6.dp, 6.dp)
                .control(CornerSm) { open = !open }
                .semantics { if (open) collapse { open = false; true } else expand { open = true; true } }
                .padding(6.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.down, null, tint = p.ink, modifier = Modifier.size(16.dp).rotate(if (open) 0f else -90f))
            Text(stringResource(R.string.download_details), style = Type.ui, color = p.ink)
        }
        if (open)
            SelectionContainer {
                Text(detail, style = Type.stamp, color = p.ink, modifier = Modifier.padding(top = 8.dp))
            }
    }
}

/**
 * A model file of one's own on its way in (Models.open), as a download is
 * shown: Android's own step, where the desktop uses a file where it is.
 */
@Composable
private fun Copying(c: Copy, a: SetupActions) {
    val p = pk
    val res = LocalResources.current
    val done = if (c.checking) 1.0 else if (c.total > 0) c.got.toDouble() / c.total else 0.0
    Screen(
        top = {
            WordmarkButton(a.about)
            Intro(stringResource(R.string.copy_title), c.name)
            Column(Modifier.padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Spread(bottom = true) {
                    Figure(percent(done))
                    Text(
                        when {
                            c.checking -> res.getString(R.string.download_checking)
                            c.total > 0 -> res.getString(R.string.download_progress, mb(c.got), mb(c.total))
                            else -> res.getString(R.string.setup_size, mb(c.got))
                        },
                        style = Type.stamp, color = p.inkMuted, modifier = Modifier.padding(bottom = 4.dp),
                    )
                }
                Bar(done, stopped = false, stringResource(R.string.copy_title))
            }
        },
        foot = {
            Row { PkButton(stringResource(R.string.download_cancel), a.cancel, kind = Kind.Ghost, large = true, enabled = !c.checking) }
        },
    )
}

/** How far, as the desktop's figure is set (.pk-figure, 700 64/0.9, its digits tabular). */
@Composable
private fun Figure(percent: Int) {
    val style = Type.figure.copy(fontFeatureSettings = "tnum")
    Text("$percent%", style = style, color = pk.ink, modifier = Modifier.cssLine(style))
}

/**
 * A download's bar (.pk-bar): 6 dp on sunk, its fill the waveform's blue on
 * the light theme and plume on the dark (their contrast with the track,
 * app.css), ink-muted while it is stopped, moving as the desktop's does.
 */
@Composable
private fun Bar(done: Double, stopped: Boolean, label: String) {
    val p = pk
    val shown by animateFloatAsState(done.toFloat().coerceIn(0f, 1f), tween(300), label = "bar")
    Box(
        Modifier
            .fillMaxWidth()
            .height(6.dp)
            .clip(RoundedCornerShape(Radius.radiusDot))
            .background(p.sunk)
            .semantics {
                contentDescription = label
                progressBarRangeInfo = ProgressBarRangeInfo(done.toFloat().coerceIn(0f, 1f), 0f..1f)
            },
    ) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(shown).background(if (stopped) p.inkMuted else if (p === Dark) p.plume else p.wave))
    }
}

/**
 * The model in use on the start screen's top edge, a ghost button whose
 * menu is the desktop's (app.js, modelMenu): each catalog model for what it
 * is for and whether it is here, Downloaded in green or its download's
 * size, the one in use checked; a model file of one's own while it is in
 * use; and Open a model file. A model that is not here is downloaded.
 */
@Composable
fun ModelButton(offers: List<Offer>, model: String?, own: String?, onPick: (Offer) -> Unit, onOwnFile: () -> Unit) {
    val p = pk
    val res = LocalResources.current
    var open by remember { mutableStateOf(false) }
    val lang = ownLanguage()
    val label = own ?: offers.find { it.name == model }?.let { stringResource(R.string.models_named, Models.label(res, it.key)) } ?: ""
    val check = @Composable { Icon(Icons.check, null, tint = p.ink, modifier = Modifier.size(16.dp)) }
    Box {
        // its words to the column's edge, as the desktop's (#ready-model)
        PkButton(label, { open = true }, Modifier.offset(x = 12.dp), kind = Kind.Ghost, trailing = Icons.down)
        Menu(open, { open = false }) {
            for (o in offers) {
                val on = own == null && o.name == model
                val note = buildAnnotatedString {
                    append(Models.about(res, o.key))
                    if (o.languages.size > 1 && lang in o.languages) append(res.getString(R.string.models_among, languageName(lang)))
                    append(" · ")
                    if (o.have) withStyle(SpanStyle(color = p.ok)) { append(res.getString(R.string.models_downloaded)) }
                    else append(res.getString(R.string.models_download_size, mb(o.bytes)))
                }
                Item(Models.label(res, o.key), null, hint = if (on) check else null, selected = on, note = note) {
                    open = false
                    if (!on) onPick(o)
                }
            }
            if (own != null) Item(own, null, hint = check, selected = true) { open = false }
            MenuLine()
            Item(stringResource(R.string.models_open_file), null) {
                open = false
                onOwnFile()
            }
        }
    }
}
