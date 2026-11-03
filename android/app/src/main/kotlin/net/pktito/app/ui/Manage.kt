package net.pktito.app.ui

import android.content.Context
import android.content.res.Resources
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import java.io.File
import java.util.Locale
import net.pktito.app.Download
import net.pktito.app.Editions
import net.pktito.app.Models
import net.pktito.app.Offer
import net.pktito.app.R
import net.pktito.app.mb
import net.pktito.app.words

/** What Manage models does with what it is asked. */
class ManageActions(
    val delete: (Offer) -> Unit,
    val deletePart: (Offer) -> Unit,
    val deleteOwn: () -> Unit,
    /**
     * The download under way stopped and what it had deleted (Downloads.cancel), as the setup
     * screen's Cancel does.
     */
    val stop: () -> Unit,
    /** A model here on the CPU made faster, its NPU's edition downloaded (Downloads.parts). */
    val faster: (Offer) -> Unit,
    val close: () -> Unit,
)

/**
 * One thing the app keeps: what it is (key: its kind and its name, "have:", "part:", "faster:" or
 * "own:"), its words, what it takes, its button's, and the question before the button does it (none
 * for Stop or Download).
 */
private class Kept(
    val key: String,
    val title: String,
    val note: AnnotatedString,
    val bytes: Long,
    val act: String,
    val enabled: Boolean,
    val ask: Pair<String, String>?,
    val onAct: () -> Unit,
)

/**
 * Manage models, from the model menu (ModelButton), in the desktop's words (manage.*): what the app
 * keeps on the device, which Android shows no one but as one number in its settings, and which only
 * clearing all the app's storage would free, the recent list with it. In the catalog's order, each
 * model downloaded, its file's name and size, the one in use said so in the green of the menu's
 * Downloaded; what a download that stopped kept, which it would go on from; a download under way,
 * with Stop; and the copy of a model file of the person's own; each deleted after a question that
 * says what follows. A model's row counts all its files, either edition's and what a download of
 * them kept, and its Delete deletes them all, as iOS's does; under one here on the CPU where the
 * NPU can run it, the offer of its graph, Faster, which iOS makes of its Neural Engine's. Under
 * them what they take in all, and the room left. In a window of Material's compact width (compact:
 * under 600 dp, a phone's as the screen's root measures it, MainActivity), Android's sheet from the
 * screen's foot, where the thumb is; in a wider one, a dialog in its middle, as the desktop's, 440
 * dp wide grown with the text (withText) to the window's margins: at twice the text size on a
 * tablet, 440 put Delete and Close under their words (2026-10-31). While a transcription runs, the
 * model in use and the one it loaded, if another was chosen since, keep their Delete dimmed: the
 * second is mapped under the transcription (Models.delete), the first will be the next one's. The
 * toast is shown here while this is up, over the screen's.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Manage(
    offers: List<Offer>,
    model: String?,
    own: File?,
    download: Download?,
    running: Boolean,
    loaded: String?,
    free: Long,
    toast: String?,
    compact: Boolean,
    a: ManageActions,
) {
    val p = pk
    val res = LocalResources.current
    val context = LocalContext.current
    // the row whose question is up, by its key, kept through a turn of the
    // screen or a fold, which make the screen anew (the pkfold emulator,
    // 2026-10-30: the question went, Manage models stayed)
    var asking by rememberSaveable { mutableStateOf<String?>(null) }
    val delete = stringResource(R.string.manage_delete)
    val ownBytes = remember(own) { own?.length() ?: 0L }
    val rows = buildList {
        for (o in offers) {
            val label = Models.label(res, o.key)
            val d = download?.takeIf { it.name == o.name }
            when {
                o.have -> {
                    val inUse = own == null && o.name == model
                    val then = Models.next(offers, model, o.name)
                    val text =
                        when {
                            !inUse -> res.getString(R.string.manage_ask_text, mb(o.taken))
                            then != null ->
                                res.getString(
                                    R.string.manage_ask_text_switch,
                                    mb(o.taken),
                                    Models.label(res, then.key),
                                )
                            else -> res.getString(R.string.manage_ask_text_last, mb(o.taken))
                        }
                    add(
                        Kept(
                            "have:${o.name}",
                            label,
                            note(
                                res,
                                "${o.name}$DOT${res.getString(R.string.manage_size, mb(o.taken))}",
                                inUse,
                                p.ok,
                            ),
                            o.taken,
                            delete,
                            !(running && (inUse || o.files.any { it.path == loaded })),
                            res.getString(R.string.manage_ask_title, label) to text,
                        ) {
                            a.delete(o)
                        }
                    )
                    if (o.faster > 0) add(faster(context, o, label, d, a))
                }
                d != null && d.going ->
                    add(
                        Kept(
                            "part:${o.name}",
                            label,
                            AnnotatedString(
                                res.getString(R.string.manage_downloading, mb(d.got), mb(d.total))
                            ),
                            d.got,
                            res.getString(R.string.stop),
                            true,
                            null,
                            a.stop,
                        )
                    )
                d != null || o.partial > 0 -> {
                    val got = d?.got ?: o.partial
                    val ask =
                        res.getString(R.string.manage_ask_part_title) to
                            res.getString(R.string.manage_ask_part_text, mb(o.taken), label)
                    add(
                        Kept(
                            "part:${o.name}",
                            label,
                            AnnotatedString(
                                res.getString(R.string.manage_paused, mb(got), mb(o.bytes))
                            ),
                            o.taken,
                            delete,
                            true,
                            ask,
                        ) {
                            if (d != null) a.stop() else a.deletePart(o)
                        }
                    )
                }
            }
        }
        // the own file is the one in use whenever it is here (Models.use deletes it)
        if (own != null) {
            val then = Models.next(offers, model, null)
            val text =
                if (then != null)
                    res.getString(
                        R.string.manage_ask_own_switch,
                        mb(ownBytes),
                        Models.label(res, then.key),
                    )
                else res.getString(R.string.manage_ask_own_last, mb(ownBytes))
            add(
                Kept(
                    "own:${own.name}",
                    own.name,
                    note(res, res.getString(R.string.manage_size, mb(ownBytes)), true, p.ok),
                    ownBytes,
                    delete,
                    !running,
                    res.getString(R.string.manage_ask_own_title, own.name) to text,
                    a.deleteOwn,
                )
            )
        }
    }
    // with keys, the first Delete that can be pressed has them, or Close,
    // when it opens and after each delete
    val first = rows.indexOfFirst { it.ask != null && it.enabled }
    val start = remember { FocusRequester() }
    val keys = hasKeys()
    val panel =
        @Composable { top: Dp, edge: Dp ->
            Panel(
                rows,
                rows.sumOf { it.bytes },
                free,
                top,
                edge,
                start,
                first,
                onAct = { if (it.ask != null) asking = it.key else it.onAct() },
                a.close,
            )
        }
    if (compact)
        ModalBottomSheet(
            onDismissRequest = a.close,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = p.ground,
            contentColor = p.ink,
            scrimColor = p.scrim,
            dragHandle = { BottomSheetDefaults.DragHandle(color = p.inkMuted) },
        ) {
            Box(Modifier.fillMaxWidth()) {
                panel(0.dp, 16.dp)
                toast?.let {
                    Toast(it, Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp))
                }
            }
        }
    else
        Modal(a.close) {
            Box(Modifier.widthIn(max = 440.dp.withText()).fillMaxWidth().floating()) {
                panel(24.dp, 24.dp)
            }
            toast?.let { Toast(it, Modifier.align(Alignment.BottomCenter)) }
        }
    // keyed on the rows alone, so a question put away (Cancel, Esc) leaves
    // the keys on the Delete that asked, as the desktop's ask() gives them
    // back to its from (on the Googlebook, 2026-10-30, while a question also
    // held this off, Esc took them to the first Delete); rows that change
    // under a question leave them there too
    if (keys)
        LaunchedEffect(rows.map { it.title to it.enabled }) {
            withFrameNanos {}
            if (asking == null) runCatching { start.requestFocus() }
        }
    val k = rows.find { it.key == asking }
    // a row gone while its question was up (a download that ended) takes it
    // with it, and so does one whose Delete a transcription begun since has
    // greyed: that model is in the transcription's hands (Models.delete)
    if (asking != null && (k?.ask == null || !k.enabled)) SideEffect { asking = null }
    k?.takeIf { it.enabled }
        ?.ask
        ?.let { (title, text) ->
            Confirm(
                title,
                text,
                delete,
                onYes = {
                    asking = null
                    k.onAct()
                },
                onCancel = { asking = null },
            )
        }
}

/**
 * The offer of a model here on the CPU in the NPU's edition, the slim file and the graph, which
 * take the whole file's place once the NPU runs the graph (Offer.faster): Faster on this phone (or
 * tablet, or computer: Editions.kind), and its download's megabytes, then how the download goes,
 * with Stop, as the downloads under way above; what a download of it that stopped kept is the model
 * row's, counted and deleted with the rest of the model's files.
 */
private fun faster(
    context: Context,
    o: Offer,
    label: String,
    d: Download?,
    a: ManageActions,
): Kept {
    val res = context.resources
    val title =
        res.getString(
            Editions.kind(
                context,
                R.string.manage_faster_phone,
                R.string.manage_faster_tablet,
                R.string.manage_faster_computer,
            )
        )
    val kept = o.fasterParts.sumOf { it.kept }
    val key = "faster:${o.name}"
    val get = { a.faster(o) }
    return when {
        d != null && d.going ->
            Kept(
                key,
                title,
                AnnotatedString(res.getString(R.string.manage_downloading, mb(d.got), mb(d.total))),
                0,
                res.getString(R.string.stop),
                true,
                null,
                a.stop,
            )
        d != null && d.step == "failed" ->
            Kept(
                key,
                title,
                AnnotatedString(words(res, d.problem?.key, d.problem?.host ?: "")),
                0,
                res.getString(R.string.download_try_again),
                true,
                null,
                get,
            )
        d != null || kept > 0 ->
            Kept(
                key,
                title,
                AnnotatedString(
                    res.getString(R.string.manage_paused, mb(d?.got ?: kept), mb(o.faster))
                ),
                0,
                res.getString(R.string.download_resume),
                true,
                null,
                get,
            )
        else -> {
            val more =
                Editions.kind(
                    context,
                    R.string.manage_faster_text_phone,
                    R.string.manage_faster_text_tablet,
                    R.string.manage_faster_text_computer,
                )
            Kept(
                key,
                title,
                AnnotatedString(res.getString(more, label, mb(o.faster))),
                0,
                res.getString(R.string.manage_get),
                true,
                null,
                get,
            )
        }
    }
}

/**
 * A row's second line, and In use after it in green where it is the one in use, its two words on
 * one line: at 337 dp (a folded phone's cover, the pkfold emulator) the line broke between them.
 */
private fun note(res: Resources, text: String, inUse: Boolean, ok: Color) = buildAnnotatedString {
    append(text)
    if (inUse) {
        append(DOT)
        withStyle(SpanStyle(color = ok)) {
            append(res.getString(R.string.manage_in_use).replace(' ', '\u00A0'))
        }
    }
}

/**
 * What the models take, in megabytes under a gigabyte and in gigabytes to a hundredth from one
 * (manage.used_mb, used_gb), and the room left.
 */
private fun used(res: Resources, total: Long, free: Long): String {
    val used =
        if (total < 1_000_000_000) res.getString(R.string.manage_used_mb, mb(total))
        else
            res.getString(
                R.string.manage_used_gb,
                String.format(Locale.getDefault(), "%.2f", total / 1e9),
            )
    val left =
        if (free < 1_000_000_000) res.getString(R.string.manage_free_mb, mb(free))
        else
            res.getString(
                R.string.manage_free_gb,
                String.format(Locale.getDefault(), "%.1f", free / 1e9),
            )
    return "$used$DOT$left"
}

/**
 * The rows between lines of sunk, what they take and the room left under them, and Close, each
 * button beside its words or under them (Beside); start is the focus of the row first, or of Close
 * where first is -1. In from its sides by edge: the dialog's 24, and the sheet's 16, which spans a
 * phone's width, as every screen's edge there (it was 24; the layout audit, 2026-11-01).
 */
@Composable
private fun Panel(
    rows: List<Kept>,
    total: Long,
    free: Long,
    top: Dp,
    edge: Dp,
    start: FocusRequester,
    first: Int,
    onAct: (Kept) -> Unit,
    onClose: () -> Unit,
) {
    val p = pk
    val res = LocalResources.current
    Column(
        Modifier.fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(start = edge, end = edge, top = top, bottom = 20.dp)
    ) {
        Text(
            stringResource(R.string.manage_title),
            style = Type.ui,
            color = p.ink,
            modifier = Modifier.semantics { heading() },
        )
        Column(Modifier.padding(top = 12.dp)) {
            if (rows.isEmpty())
                Text(
                    stringResource(R.string.manage_none),
                    style = Type.small,
                    color = p.inkMuted,
                    modifier = Modifier.padding(vertical = 12.dp),
                )
            for ((i, r) in rows.withIndex()) {
                Line()
                Beside(
                    Modifier.fillMaxWidth().padding(vertical = 12.dp),
                    words = {
                        Column {
                            Text(
                                r.title,
                                style = Type.ui,
                                color = p.ink,
                                maxLines = 1,
                                overflow = TextOverflow.MiddleEllipsis,
                            )
                            Text(r.note, style = Type.small, color = p.inkMuted)
                        }
                    },
                ) {
                    PkButton(
                        r.act,
                        { onAct(r) },
                        Modifier.widthIn(min = 80.dp)
                            .then(if (i == first) Modifier.focusRequester(start) else Modifier),
                        kind = if (r.ask != null) Kind.Del else Kind.Default,
                        enabled = r.enabled,
                    )
                }
            }
            Line()
        }
        Beside(
            Modifier.fillMaxWidth().padding(top = 16.dp),
            words = { Text(used(res, total, free), style = Type.small, color = p.inkMuted) },
        ) {
            PkButton(
                stringResource(R.string.sheet_close),
                onClose,
                Modifier.widthIn(min = 80.dp)
                    .then(if (first < 0) Modifier.focusRequester(start) else Modifier),
            )
        }
    }
}

/** Between the rows, the line a menu draws between its groups (MenuLine), across. */
@Composable
private fun Line() {
    Box(Modifier.fillMaxWidth().height(1.dp).background(pk.sunk))
}
