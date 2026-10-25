package net.pktito.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.Uri
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.os.SystemClock
import android.provider.OpenableColumns
import android.util.Log
import androidx.core.app.ServiceCompat
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
        ServiceCompat.startForeground(this, NOTE, note(getString(R.string.reading), -1), type())
        val uri = intent?.data
        if (uri == null || Transcriber.running) {
            if (!Transcriber.running) stop(startId)
            return START_NOT_STICKY
        }
        Transcriber.begin(nameOf(uri))
        thread(name = "transcribe") {
            val wake = getSystemService(PowerManager::class.java)
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "pktito:transcribe")
            wake.acquire(LIMIT_MS)
            try {
                transcribe(uri)
            } catch (e: Exception) {
                Log.w(TAG, "transcribing ${Transcriber.name}", e)
                Transcriber.error = e.message ?: e.toString()
            } finally {
                wake.release()
                Transcriber.running = false
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
        val handle = Transcriber.model(this)
        val notes = getSystemService(NotificationManager::class.java)
        var shown = 0L
        val pfd = contentResolver.openFileDescriptor(uri, "r") ?: throw IOException("$uri: nothing to read")
        pfd.use {
            // PROBE, to remove: can the app open the document again by its fd's name?
            val again = runCatching { java.io.File("/proc/self/fd/${it.fd}").inputStream().use { s -> s.read() } }
            Log.i(TAG, "probe: /proc/self/fd reopen ${again.exceptionOrNull() ?: "works"} for ${uri.authority}")
            val t = Engine.transcribe(handle, it.fd, Transcriber.name) { done, segments ->
                Transcriber.add(done, segments)
                // Android drops a notification's updates past a few a second
                val now = SystemClock.elapsedRealtime()
                if (now - shown >= 500) {
                    shown = now
                    val percent = (done * 100).toInt()
                    notes.notify(NOTE, note(getString(R.string.transcribing, percent), percent))
                }
            }
            Transcriber.transcript = t
            if (!JSONObject(t).getBoolean("complete"))
                Transcriber.error = getString(R.string.stopped)
        }
    }

    private fun stop(startId: Int) {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
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
