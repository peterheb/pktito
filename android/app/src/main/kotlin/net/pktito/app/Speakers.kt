package net.pktito.app

import android.content.Context
import android.content.res.Resources
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.thread
import kotlin.concurrent.withLock
import org.json.JSONArray
import org.json.JSONObject

/**
 * Who said what (PKT-27; the speakers design, page version 7, every call Peter's of 2026-11-09), as
 * the desktop's window has it (ui/app.js, speakers). A segment says its speaker, numbered from 1 by
 * first word as pktito-c numbers them, eight at most, the diarizer's; a change of speaker ends a
 * sentence and a cue, so a turn is a speaker's segments in a row. The names are the file's own,
 * speaker n's at n - 1, "" for Speaker n: no voice is remembered between recordings. The switch
 * shows them, kept for the next file, and every format shared or copied has them while it does. The
 * transcript itself is Transcriber's; this changes its speakers and keeps them on the recent list
 * (Engine.keepSpeakers).
 */
object Speakers {
    /** The diarizer's most, and the colors' (the tokens' speaker1 to speaker8). */
    const val MOST = 8
    private const val TAG = "pktito"
    private const val PREF = "speakers"

    /** The diarizer a build offers (Engine.diarizer): its file's name in the models folder. */
    class Offer(
        val name: String,
        val license: String,
        val file: String,
        val bytes: Long,
        val kept: Long,
        val have: Boolean,
    )

    /** The turn starting at segment from: its segments, from through to, its speaker, its times. */
    class Turn(val from: Int, val to: Int, val speaker: Int, val start: Double, val end: Double)

    /** What an edit changed, for its toast's Undo to put back. */
    class Before(val segments: List<Seg>, val names: List<String>, val stretches: List<Stretch>)

    /** The switch, kept for the next file. */
    var on by mutableStateOf(false)
        private set

    /** The diarizer this build offers, null where none. */
    var offer by mutableStateOf<Offer?>(null)
        private set

    /** Finding the speakers of a transcript from before (find). */
    var finding by mutableStateOf(false)
        private set

    /**
     * Of its recording, how much the diarizer has heard while finding, 0 to 1: the strip's ring.
     */
    var heard by mutableDoubleStateOf(0.0)
        private set

    private val main = Handler(Looper.getMainLooper())
    /** One diarizer at work at a time: a transcription's or find's (with). */
    private val working = ReentrantLock()
    /** The recent list's writes of speakers, in the order they were made. */
    private val keeper = Executors.newSingleThreadExecutor()

    private fun prefs(context: Context) =
        context.getSharedPreferences("pktito", Context.MODE_PRIVATE)

    /** The switch as it was kept, and the offer as the models folder has it. */
    fun load(context: Context) {
        on = prefs(context).getBoolean(PREF, false)
        refresh(context)
    }

    fun set(context: Context, value: Boolean) {
        on = value
        prefs(context).edit().putBoolean(PREF, value).apply()
    }

    /** The offer again, after a download or a delete. */
    fun refresh(context: Context) {
        offer =
            runCatching {
                val json = Engine.diarizer(Models.dir(context).path)
                if (json.isEmpty()) null
                else
                    JSONObject(json).let { o ->
                        Offer(
                            o.getString("name"),
                            o.getString("license"),
                            o.getString("file"),
                            o.getLong("bytes"),
                            o.getLong("kept"),
                            o.getBoolean("have"),
                        )
                    }
            }
                .onFailure { Log.w(TAG, "the diarizer's offer", it) }
                .getOrNull()
    }

    /** The diarizer's file, where it is downloaded. */
    fun file(context: Context): File? =
        offer?.takeIf { it.have }?.let { File(Models.dir(context), it.file) }

    /**
     * The speaker model deleted for Manage models, or with part what a download of it that stopped
     * kept, and what it said, if anything: that a transcription or a find has it (working), whose
     * Delete is dimmed then, or that it failed. The names found already stay with their
     * transcripts; turning the switch on asks for the model again.
     */
    fun delete(context: Context, part: Boolean): String? {
        if (!working.tryLock()) return context.getString(R.string.manage_speakers_busy)
        val failed =
            try {
                runCatching { Engine.deleteDiarizer(Models.dir(context).path, part) }
                    .onFailure { Log.w(TAG, "deleting the speaker model", it) }
                    .isFailure
            } finally {
                working.unlock()
            }
        refresh(context)
        return if (failed) context.getString(R.string.manage_cant_delete) else null
    }

    /**
     * block with a diarizer for the model at path, loaded for it and let go after, one at a time:
     * loading its 100 MB takes a moment beside a transcription's seconds, and the phone keeps none
     * of it between them.
     */
    fun <T> with(path: File, block: (Long) -> T): T = working.withLock {
        val h = Engine.loadDiarizer(path.path)
        try {
            block(h)
        } finally {
            Engine.freeDiarizer(h)
        }
    }

    /** The speakers of segs, in the order they first speak. */
    fun present(segs: List<Seg>): List<Int> {
        val seen = ArrayList<Int>()
        for (s in segs) s.speaker?.let { if (it !in seen) seen.add(it) }
        return seen
    }

    /** Speaker n's name: the one given, else Speaker n. */
    fun nameOf(res: Resources, names: List<String>, n: Int): String =
        names.getOrNull(n - 1)?.trim()?.takeIf { it.isNotEmpty() }
            ?: res.getString(R.string.speakers_default, n)

    /** Whether segment i starts a turn. */
    fun turnStarts(segs: List<Seg>, i: Int): Boolean =
        segs[i].speaker != null && (i == 0 || segs[i - 1].speaker != segs[i].speaker)

    fun turnAt(segs: List<Seg>, i: Int): Turn {
        val s = segs[i].speaker ?: 1
        var j = i
        while (j + 1 < segs.size && segs[j + 1].speaker == s) j++
        return Turn(i, j, s, segs[i].start, segs[j].end)
    }

    /**
     * New speaker's number: the lowest not in use, so that no two share a color; none past eight.
     */
    fun fresh(present: List<Int>): Int? = (1..MOST).firstOrNull { it !in present }

    /** The speaker already called v, other than s: renaming s to it joins the two. */
    fun joinee(res: Resources, segs: List<Seg>, names: List<String>, s: Int, v: String): Int? =
        present(segs).firstOrNull { it != s && nameOf(res, names, it).equals(v.trim(), true) }

    /**
     * Turn u given to speaker to, a new one where fresh: what it was before, for Undo; null where
     * it is that speaker's already.
     */
    fun move(context: Context, u: Turn, to: Int, fresh: Boolean): Before? {
        if (to == u.speaker) return null
        return edit(context) {
            if (fresh) names = named(names, to, "")
            for (k in u.from..u.to) segments[k] = segments[k].by(to)
            stretches = moved(stretches, u.speaker, to, u.start, u.end)
        }
    }

    /**
     * Speaker s renamed v: a name of its own, or where v is another speaker's, the two joined, the
     * other's number with what it was before for Undo. Null where nothing changes.
     */
    fun rename(context: Context, s: Int, v: String): Pair<Int?, Before>? {
        val t = Transcriber
        val name = v.trim()
        val res = context.resources
        if (name.isEmpty() || name == nameOf(res, t.names, s)) return null
        val other =
            joinee(res, t.segments, t.names, s, name)
                ?: return null to edit(context) { names = named(names, s, name) }
        return other to
            edit(context) {
                for (k in segments.indices) if (segments[k].speaker == s)
                    segments[k] = segments[k].by(other)
                stretches = stretches.map {
                    if (it.speaker == s) Stretch(it.start, it.end, other) else it
                }
            }
    }

    fun undo(context: Context, b: Before) {
        val t = Transcriber
        t.segments.clear()
        t.segments.addAll(b.segments)
        t.names = b.names
        t.stretches = b.stretches
        changed(context)
    }

    /**
     * Who said what in the transcript on the screen, one from before the switch: the diarizer over
     * its recording, which takes seconds, not a transcription again (Engine.findSpeakers), and kept
     * on the recent list. failed hears why it could not, in the app's words.
     */
    fun find(context: Context, failed: (String) -> Unit) {
        val t = Transcriber
        val tr = t.transcript ?: return
        val uri = t.uri ?: return
        val path = file(context) ?: return
        if (finding) return
        val id = t.id
        val name = t.name
        val app = context.applicationContext
        finding = true
        heard = 0.0
        // the strip's ring has play's place until they are found (Work)
        Player.pause(context)
        thread(name = "speakers") {
            val got = runCatching {
                // the recording's copy where it was shared, as it plays (Recents.open)
                val copy = Recents.copyOf(app, id)
                val file =
                    if (copy.isFile)
                        ParcelFileDescriptor.open(copy, ParcelFileDescriptor.MODE_READ_ONLY)
                    else readable(app, uri)
                file.use { pfd ->
                    with(path) { h ->
                        Engine.findSpeakers(h, pfd.fd, name, tr) { d -> main.post { heard = d } }
                    }
                }
            }
            main.post {
                finding = false
                got.onSuccess { json ->
                        // the screen's transcript still, not one opened since
                        if (t.id != id || t.transcript != tr) return@onSuccess
                        val o = JSONObject(json)
                        t.segments.clear()
                        t.segments.addAll(Transcriber.parse(o.getJSONArray("segments")))
                        t.names = emptyList()
                        t.stretches = Stretch.parse(o.optJSONArray("stretches"))
                        t.transcript = json
                        t.cues = null
                        keep(app, id, json)
                    }
                    .onFailure { e ->
                        Log.w(TAG, "finding the speakers of $name", e)
                        val key = Engine.problem(e)
                        failed(
                            if (key == null) e.message ?: e.toString()
                            else words(app.resources, key)
                        )
                    }
            }
        }
    }

    /** A change made to the transcript's speakers, then drawn and kept: what it was before. */
    private fun edit(context: Context, change: Transcriber.() -> Unit): Before {
        val t = Transcriber
        val before = Before(t.segments.toList(), t.names, t.stretches)
        t.change()
        changed(context)
        return before
    }

    /**
     * The transcript's JSON as its speakers are now, for the formats, the captions and the list.
     */
    private fun changed(context: Context) {
        val t = Transcriber
        val tr = t.transcript ?: return
        val json =
            JSONObject(tr)
                .put("segments", JSONArray(t.segments.map { it.json() }))
                .put("names", JSONArray(t.names))
                .put("stretches", JSONArray(t.stretches.map { it.json() }))
                .toString()
        t.transcript = json
        t.cues = null
        keep(context.applicationContext, t.id, json)
    }

    private fun keep(app: Context, id: String, json: String) {
        if (id.isEmpty()) return
        keeper.execute {
            runCatching { Recents.keepSpeakers(app, id, json) }
                .onFailure { Log.w(TAG, "keeping the speakers of $id", it) }
        }
    }

    /** names with speaker n's set, those before it kept as they are. */
    private fun named(names: List<String>, n: Int, name: String): List<String> {
        val out = names.toMutableList()
        while (out.size < n) out.add("")
        out[n - 1] = name
        return out
    }

    /** The stretches of speaker from between a and b given to speaker to. */
    private fun moved(
        list: List<Stretch>,
        from: Int,
        to: Int,
        a: Double,
        b: Double,
    ): List<Stretch> {
        val out = ArrayList<Stretch>()
        for (g in list) {
            if (g.speaker != from || g.end <= a || g.start >= b) {
                out.add(g)
                continue
            }
            if (g.start < a) out.add(Stretch(g.start, a, g.speaker))
            out.add(Stretch(maxOf(g.start, a), minOf(g.end, b), to))
            if (g.end > b) out.add(Stretch(b, g.end, g.speaker))
        }
        return out
    }

    /** A line of the speaker bar: each speaker's part of it, from a to b seconds. */
    class Part(val speaker: Int, val a: Double, val b: Double)

    /**
     * The bar's two lines (the desktop's barLines): each speaker's stretches up to upto seconds,
     * those that meet made one, on the main line, and one who starts while another talks on the
     * second until the first stops, then on the main one.
     */
    fun lines(stretches: List<Stretch>, upto: Double): Pair<List<Part>, List<Part>> {
        val by = sortedMapOf<Int, MutableList<Pair<Double, Double>>>()
        for (g in stretches) {
            val a = g.start
            val b = minOf(g.end, upto)
            if (b <= a) continue
            by.getOrPut(g.speaker) { ArrayList() }.add(a to b)
        }
        class Run(val s: Int, val a: Double, var b: Double)
        val all = ArrayList<Run>()
        for ((s, list) in by) {
            list.sortBy { it.first }
            var cur: Run? = null
            for ((a, b) in list) {
                val c = cur
                if (c != null && a <= c.b) c.b = maxOf(c.b, b)
                else
                    Run(s, a, b).also {
                        all.add(it)
                        cur = it
                    }
            }
        }
        all.sortWith(compareBy<Run>({ it.a }, { it.b }))
        val main = ArrayList<Part>()
        val second = ArrayList<Part>()
        var holder: Run? = null
        for (u in all) {
            val h = holder
            if (h != null && u.a < h.b) {
                second.add(Part(u.s, u.a, minOf(u.b, h.b)))
                if (u.b > h.b) {
                    main.add(Part(u.s, h.b, u.b))
                    holder = u
                }
            } else {
                main.add(Part(u.s, u.a, u.b))
                holder = u
            }
        }
        return main to second
    }

    /**
     * Each of px pixels' speaker on a line, per seconds a pixel: whoever talks most of it; 0 none.
     */
    fun pixels(parts: List<Part>, px: Int, per: Double): IntArray {
        val time = Array(MOST + 1) { DoubleArray(px) }
        for (p in parts) {
            val n = slotOf(p.speaker)
            val x0 = maxOf(0, (p.a / per).toInt())
            val x1 = minOf(px - 1, (p.b / per).toInt())
            for (x in x0..x1) {
                val t = minOf(p.b, (x + 1) * per) - maxOf(p.a, x * per)
                if (t > 0) time[n][x] += t
            }
        }
        val who = IntArray(px)
        for (x in 0 until px) {
            var top = 0.0
            for (n in 1..MOST) if (time[n][x] > top) {
                top = time[n][x]
                who[x] = n
            }
        }
        return who
    }

    /** Speaker n's color, 1 to 8. */
    fun slotOf(n: Int): Int = (n - 1).mod(MOST) + 1
}
