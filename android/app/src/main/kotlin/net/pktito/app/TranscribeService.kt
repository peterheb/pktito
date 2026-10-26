package net.pktito.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.Uri
import android.os.Build
import android.os.IBinder
import android.os.ParcelFileDescriptor
import android.os.PowerManager
import android.os.SystemClock
import android.provider.OpenableColumns
import android.util.Log
import java.io.File
import java.io.IOException
import kotlin.concurrent.thread
import org.json.JSONObject

/**
 * A transcription, in a foreground service so that Android lets it go on
 * with the screen off or another app in front, its progress in a
 * notification, and a wake lock so that the CPU stays on for it. One at a
 * time: the screen offers no second while one runs.
 */
class TranscribeService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // first, as startForegroundService promised Android, within seconds
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.channel), NotificationManager.IMPORTANCE_LOW)
        )
        // Service's own call, not ServiceCompat's: androidx.core 1.19.1 keeps
        // only the types Android 14 had (0x40000fff), and mediaProcessing
        // (0x2000) came to none, which Android 17 refused
        startForeground(NOTE, note(getString(R.string.status_reading), -1), type())
        val uri = intent?.data
        if (uri == null || Transcriber.running) {
            if (!Transcriber.running) stop(startId)
            return START_NOT_STICKY
        }
        Transcriber.begin(nameOf(uri), uri)
        thread(name = "transcribe") {
            val wake = getSystemService(PowerManager::class.java)
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "pktito:transcribe")
            wake.acquire(LIMIT_MS)
            try {
                transcribe(uri)
            } catch (e: Exception) {
                Log.w(TAG, "transcribing ${Transcriber.name}", e)
                val why = e.message ?: e.toString()
                Transcriber.post {
                    error = why
                    step = "error"
                }
            } finally {
                wake.release()
                stop(startId)
            }
        }
        return START_NOT_STICKY
    }

    /** Android 15's limit on mediaProcessing, 6 hours a day: stop at the next chunk. */
    override fun onTimeout(startId: Int, fgsType: Int) {
        Engine.stop()
    }

    private fun transcribe(uri: Uri) {
        val start = SystemClock.elapsedRealtime()
        if (!Transcriber.loaded(this)) {
            Transcriber.post { step = "loading" }
            Transcriber.model(this)
            Transcriber.post { step = "reading" }
        }
        val handle = Transcriber.model(this)
        val notes = getSystemService(NotificationManager::class.java)
        // set by begin, before this thread
        val id = Transcriber.id
        val name = Transcriber.name
        var shown = 0L
        var began = 0L
        var video = 0.0
        var loudness = ByteArray(0)
        // a recording shared or opened with pktito, which Android takes the
        // access to back once the task ends, played from a copy (Recents.copy)
        val copy = Recents.copy(this, uri, id)
        val (size, modified) = if (copy != null) copy.length() to 0L else Recents.stamp(this, uri)
        val file = copy?.let { ParcelFileDescriptor.open(it, ParcelFileDescriptor.MODE_READ_ONLY) } ?: readable(this, uri)
        file.use { pfd ->
            val t = Engine.transcribe(handle, pfd.fd, name, object : Engine.Progress {
                override fun audio(duration: Double, fps: Double, peaks: ByteArray) {
                    began = SystemClock.elapsedRealtime()
                    video = fps
                    loudness = peaks
                    Transcriber.post {
                        this.duration = duration
                        this.fps = fps
                        this.peaks = peaks
                        step = "transcribing"
                    }
                }

                override fun update(done: Double, segments: String) {
                    val segs = Transcriber.parse(segments)
                    val spent = (SystemClock.elapsedRealtime() - began) / 1000.0
                    Transcriber.post {
                        this.done = done
                        this.segments.addAll(segs)
                        transcribed = spent
                    }
                    // Android drops a notification's updates past a few a second
                    val now = SystemClock.elapsedRealtime()
                    if (now - shown >= 500) {
                        shown = now
                        val percent = (done * 100).toInt()
                        notes.notify(NOTE, note(getString(R.string.status_transcribing, percent), percent))
                    }
                }
            })
            val o = JSONObject(t)
            val complete = o.getBoolean("complete")
            // for adb's logcat: what was done, and how fast
            val secs = (SystemClock.elapsedRealtime() - began) / 1000.0
            Log.i(TAG, "transcribed $name${if (complete) "" else ", stopped"}: " +
                "${o.getJSONArray("segments").length()} segments in %.1f s".format(secs) +
                if (video > 0) ", video at %.3f fps".format(video) else "")
            // finished, it goes on the recent list, as the desktop's does;
            // a list that cannot be written leaves it on the screen alone
            if (complete)
                try {
                    val item = JSONObject()
                        .put("id", id)
                        .put("path", uri.toString())
                        .put("name", name)
                        .put("duration", o.getDouble("duration"))
                        .put("finished", System.currentTimeMillis() / 1000)
                        .put("took", (SystemClock.elapsedRealtime() - start) / 1000.0)
                        .put("transcribed", secs)
                        .put("model", o.getString("model"))
                        .put("fps", video)
                        .put("size", size)
                        .put("modified", modified)
                    Recents.keep(this, item, t, loudness)
                } catch (e: Exception) {
                    Log.w(TAG, "keeping $name on the recent list", e)
                }
            Transcriber.post {
                transcript = t
                step = if (complete) "done" else "stopped"
            }
        }
    }

    private fun stop(startId: Int) {
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf(startId)
    }

    /** The document's name, as its provider gives it. */
    private fun nameOf(uri: Uri): String =
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        } ?: uri.lastPathSegment ?: ""

    /** percent -1: a bar that moves without a number, while the audio is read. */
    private fun note(text: String, percent: Int): Notification {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_note)
            .setContentTitle(Transcriber.name.ifEmpty { getString(R.string.app_name) })
            .setContentText(text)
            .setProgress(100, percent.coerceAtLeast(0), percent < 0)
            .setContentIntent(open)
            .setOngoing(true)
            .build()
    }

    /** The manifest's two types: mediaProcessing where Android has it (15), dataSync before. */
    private fun type(): Int =
        if (Build.VERSION.SDK_INT >= 35) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROCESSING
        else ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC

    private companion object {
        const val TAG = "pktito"
        const val CHANNEL = "transcribe"
        const val NOTE = 1
        /** the wake lock's own end, should the service never release it: the 6 hours */
        const val LIMIT_MS = 6 * 60 * 60 * 1000L
    }
}

/**
 * The document at uri as a file: Android's decoder reads by offset, and a
 * provider that streams a document (a cloud drive's, a pipe with no size)
 * has none, so its bytes are copied into the cache first. The copy is
 * unlinked once open; the descriptor keeps it until it is closed.
 */
fun readable(context: Context, uri: Uri): ParcelFileDescriptor {
    val pfd = context.contentResolver.openFileDescriptor(uri, "r") ?: throw IOException("$uri: nothing to read")
    if (pfd.statSize >= 0)
        return pfd
    val copy = File.createTempFile(Recents.COPY, null, context.cacheDir)
    try {
        ParcelFileDescriptor.AutoCloseInputStream(pfd).use { input ->
            copy.outputStream().use { input.copyTo(it) }
        }
        return ParcelFileDescriptor.open(copy, ParcelFileDescriptor.MODE_READ_ONLY)
    } finally {
        copy.delete()
    }
}
