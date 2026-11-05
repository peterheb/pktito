package net.pktito.app

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.io.File
import java.io.IOException
import kotlin.concurrent.thread

/** Which of a model's two editions a transcriber runs (iOS's Edition). */
enum class Edition {
    /** the slim model file and the graph compiled for this phone's NPU */
    NPU,

    /** the whole model file, its encoder pktito's own on the CPU */
    CPU,
}

/**
 * Which edition of a model this phone runs, chosen for whoever uses the app, as iOS's
 * Editions.swift chooses: the NPU's - the slim model file and the graph a PC compiled for the
 * phone's chip (model.rs, Graph), 727 MB for English where the whole file is 624; with the NPU's
 * encoder the app on a Galaxy S24 Ultra transcribed chapter 1 of Alice in 3.2 s (2026-11-03), where
 * its CPU took 22.6 (2026-11-02) - where the catalog has a graph for the chip and the NPU has not
 * refused that graph here, on this system with this build of the app; else the CPU's, the whole
 * model file. The chip is QNN's word for it (Engine.npuChip), asked once a system and build, off
 * the main thread: QNN loads the NPU's own library to say, 137 to 168 ms on the Googlebook
 * (2026-11-03).
 */
object Editions {
    /** The chip, "" for none; null until it is known for this system and build. */
    @Volatile private var chip: String? = null
    private val main = Handler(Looper.getMainLooper())

    private fun prefs(context: Context) =
        context.getSharedPreferences("pktito", Context.MODE_PRIVATE)

    /**
     * This system and this install of the app: a new system may run what this one could not, and a
     * new build brings a new QNN, with the libraries for more chips and the catalog's graphs for
     * them.
     */
    private fun here(context: Context): String {
        val installed = context.packageManager.getPackageInfo(context.packageName, 0).lastUpdateTime
        return "${Build.FINGERPRINT} $installed"
    }

    /** The NPU's chip as QNN names it, "" for none; null until it is known (ask). Any thread. */
    fun chip(context: Context): String? {
        chip?.let {
            return it
        }
        val kept = prefs(context).getString("npu_chip", null) ?: return null
        val h = here(context)
        if (!kept.startsWith("$h\n")) return null
        return kept.substring(h.length + 1).also { chip = it }
    }

    /**
     * The chip asked of QNN where it is not known for this system and build, on a thread of its
     * own, and the models' state made anew with it. The main thread waits for it up to wait ms, so
     * that a launch that asks shows the sizes of the edition it downloads from its first frame.
     */
    fun ask(context: Context, wait: Long) {
        if (chip(context) != null) return
        val app = context.applicationContext
        val t =
            thread(name = "npu") {
                known(app)
                main.post { Models.refresh(app) }
            }
        t.join(wait)
    }

    /**
     * The chip, asked of QNN now where it is not known: for a thread that may wait, the
     * transcription's.
     */
    fun chipNow(context: Context): String = chip(context) ?: known(context)

    /**
     * The chip QNN describes, kept for this system and build. One at a time: QNN's device is made
     * to ask, and the transcription's waits for this one before it makes its own.
     */
    @Synchronized
    private fun known(context: Context): String {
        chip(context)?.let {
            return it
        }
        val c =
            try {
                Engine.npuChip(libs(context))
            } catch (e: IOException) {
                Log.i(TAG, "no NPU QNN runs on: ${e.message}")
                ""
            }
        prefs(context).edit().putString("npu_chip", "${here(context)}\n$c").apply()
        chip = c
        if (c.isNotEmpty()) Log.i(TAG, "the NPU: $c")
        return c
    }

    /**
     * The folders QNN's libraries are read from (Engine.npuChip's and Engine.npu's libs): the NPU's
     * own, its skels, copied out of the app's assets on a Snapdragon the first time, then the app's
     * native library folder for the rest. The skels come in the assets, not that folder, where Play
     * refuses their 4 KB pages (android/native.sh). Kept in the code cache, which Android empties
     * when the app is updated, and copied again for each system and install (here): the three, 39
     * MB, in 0.12 to 0.16 s on the Googlebook and the Galaxy S24 and S25 Ultras (2026-11-04).
     */
    @Synchronized
    fun libs(context: Context): String {
        val native = context.applicationInfo.nativeLibraryDir
        if (Build.VERSION.SDK_INT < 31 || Build.SOC_MANUFACTURER != "QTI") return native
        val dir = File(context.codeCacheDir, "qnn")
        val stamp = File(dir, "here")
        val h = here(context)
        if (!stamp.isFile || stamp.readText() != h) {
            val t = System.nanoTime()
            dir.deleteRecursively()
            dir.mkdirs()
            val skels = context.assets.list("qnn").orEmpty().filter { it.endsWith("Skel.so") }
            for (name in skels) context.assets.open("qnn/$name").use { i ->
                File(dir, name).outputStream().use { i.copyTo(it) }
            }
            // last, so that a copy cut short is made again
            stamp.writeText(h)
            Log.i(TAG, "the NPU's skels %s in %.2f s".format(skels, (System.nanoTime() - t) / 1e9))
        }
        return "${dir.path};$native"
    }

    /**
     * Why the NPU refused name's graph here, on this system with this build of the app, or null: a
     * new system or build may run what this one could not, as iOS's failure has it, and so may a
     * new graph, so the graph's SHA-256 is kept with them.
     */
    fun failure(context: Context, name: String, graph: Part?): String? {
        if (graph == null) return null
        val s = prefs(context).getString("npu_failed.$name", null) ?: return null
        val key = "${here(context)} ${graph.sha256}: "
        return if (s.startsWith(key)) s.substring(key.length) else null
    }

    /**
     * The NPU's refusal of name's graph, remembered for this system, build and graph: until one of
     * them changes, the model goes to its CPU's edition at once.
     */
    fun refused(context: Context, name: String, graph: Part, why: String) {
        prefs(context)
            .edit()
            .putString("npu_failed.$name", "${here(context)} ${graph.sha256}: $why")
            .apply()
    }

    /**
     * Which of the words that name the device fits it, as iOS's say iPhone or iPad (Peter,
     * 2026-11-03): a computer where Android says it is one (the Googlebook), a tablet from
     * Material's 600 dp, else a phone.
     */
    fun kind(context: Context, phone: Int, tablet: Int, computer: Int): Int =
        when {
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_PC) -> computer
            context.resources.configuration.smallestScreenWidthDp >= 600 -> tablet
            else -> phone
        }

    /**
     * That the NPU can't run the model, so pktito uses the processor instead (iOS's prepare_cpu).
     */
    fun cpuWords(context: Context): String =
        context.getString(
            kind(
                context,
                R.string.npu_cpu_phone,
                R.string.npu_cpu_tablet,
                R.string.npu_cpu_computer,
            )
        )

    private const val TAG = "pktito"
}
