package net.pktito.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.content.res.Resources
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PersistableBundle
import android.os.SystemClock
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.Locale
import kotlin.concurrent.thread
import kotlin.math.roundToLong
import org.json.JSONObject

/** What went wrong, from the desktop's Rust (strings.rs, Problem): the key of its words, the server they name, and the particulars as whatever reported it said them. */
class Problem(val key: String, val host: String, val detail: String)

/** How a download stands (download.rs, State). */
class Download(
    val name: String,
    /** "downloading", "checking", "paused", "failed", "done" or "cancelled" */
    val step: String,
    val got: Long,
    val total: Long,
    /** Bytes a second over the last few seconds; 0 until known. */
    val speed: Double,
    val host: String,
    val problem: Problem?,
    /** A failure it is trying again after, by itself. */
    val retry: Boolean,
    /** After a failure, trying again now: the failure stands until bytes come. */
    val trying: Boolean,
) {
    /** Whether its thread goes on: getting bytes, checking them, or waiting to try again. */
    val going: Boolean
        get() = step == "downloading" || step == "checking" || (step == "failed" && retry)

    companion object {
        fun parse(json: String): Download {
            val o = JSONObject(json)
            val p = o.optJSONObject("problem")
            return Download(
                o.getString("name"), o.getString("step"), o.getLong("got"), o.getLong("total"),
                o.getDouble("speed"), o.getString("host"),
                p?.let { Problem(it.getString("key"), it.getJSONObject("args").optString("host"), it.getString("detail")) },
                o.getBoolean("retry"), o.getBoolean("trying"),
            )
        }
    }
}

/**
 * A catalog model's download, as the desktop's window runs it
 * (download.rs), on a thread of the app's (Engine.download) that a job
 * keeps going with the screen off or another app in front: from Android 14
 * a user-initiated data transfer job (DownloadJob), Android's way for a
 * download the person asked for, with a network as its condition; before
 * it a foreground service (DownloadService). Its progress is in a
 * notification as well as on the screen. One at a time: the screen offers
 * no second while one is shown.
 */
object Downloads {
    /** The download under way, paused or failed, for the screen; null once it is done or cancelled. */
    var state by mutableStateOf<Download?>(null)

    const val NAME = "name"
    /** The notification's id; the transcription's is 1. */
    const val NOTE = 2
    const val JOB = 1
    private const val CHANNEL = "download"
    private const val TAG = "pktito"

    private val main = Handler(Looper.getMainLooper())
    private var worker: Thread? = null
    /** What ends the job or the service keeping the download going, once it ends. */
    private var over: (() -> Unit)? = null

    val running: Boolean
        @Synchronized get() = worker?.isAlive == true

    /**
     * The model of that name downloaded, or its download gone on with from
     * what it kept: the screen says so at once, from what is here, as
     * app.js's startDownload; a download that is waiting to try again
     * tries now.
     */
    fun start(context: Context, o: Offer) {
        if (running) return Engine.retryDownload()
        state = Download(o.name, "downloading", o.partial, o.bytes, 0.0, "", null, retry = false, trying = false)
        if (Build.VERSION.SDK_INT < 34)
            context.startForegroundService(Intent(context, DownloadService::class.java).putExtra(NAME, o.name))
        else if (!DownloadJob.schedule(context, o)) {
            Log.w(TAG, "${o.name}: JobScheduler refused the download's job")
            state = Download(o.name, "failed", o.partial, o.bytes, 0.0, "", Problem("", "", "JobScheduler refused the download's job"), retry = false, trying = false)
        }
    }

    /** Try again, after a failure: now, if it is waiting to by itself. */
    fun retry(context: Context) {
        val name = state?.name ?: return
        if (running) return Engine.retryDownload()
        Models.offers.find { it.name == name }?.let { start(context, it) }
    }

    fun pause(context: Context) {
        if (running) return Engine.pauseDownload()
        // a job waiting for a network has nothing to stop but itself
        cancelJob(context)
        state = state?.let { Download(it.name, "paused", it.got, it.total, 0.0, it.host, null, retry = false, trying = false) }
    }

    /** Stop it and delete what it had: its thread does, and says so, if it is running; else here. */
    fun cancel(context: Context) {
        val name = state?.name ?: return
        if (Engine.cancelDownload(Models.dir(context).path, name)) return
        cancelJob(context)
        state = null
        Models.refresh(context)
    }

    private fun cancelJob(context: Context) {
        if (Build.VERSION.SDK_INT >= 34) context.getSystemService(JobScheduler::class.java).cancel(JOB)
    }

    /**
     * The download of name on a thread of its own, or the one running told
     * to try now, for the job or the service that keeps it going: over ends
     * that once the download ends. false where there is nothing to do, the
     * model here already.
     */
    @Synchronized
    fun run(context: Context, name: String, over: () -> Unit): Boolean {
        val app = context.applicationContext
        if (Models.catalog(app).find { it.name == name }?.have != false) return false
        this.over = over
        if (worker?.isAlive == true) {
            Engine.retryDownload()
            return true
        }
        worker = thread(name = "download") { download(app, name) }
        return true
    }

    /**
     * The job keeping it going stopped by Android: for the network, the
     * download says so and tries again by itself, as the desktop's does,
     * and the job comes back with a network; for anything else (the
     * person, from Android's list of apps at work) it pauses.
     */
    @Synchronized
    fun stopped(context: Context, network: Boolean) {
        if (!network) Engine.pauseDownload()
        val app = context.applicationContext
        // its end lets go of a job that waits for the network
        over = { cancelJob(app) }
    }

    private fun download(app: Context, name: String) {
        // room for the rest made where Android can make it (Models.room); where
        // it cannot, the download stops when the storage is full, as the
        // desktop's does, keeping what it has to go on from
        Models.catalog(app).find { it.name == name }?.let { Models.room(app, it.bytes - it.partial) }
        val notes = app.getSystemService(NotificationManager::class.java)
        var shown = 0L
        val path = try {
            Engine.download(Models.dir(app).path, name, object : Engine.Download {
                override fun state(json: String) {
                    val d = Download.parse(json)
                    main.post { state = d.takeUnless { it.step == "done" || it.step == "cancelled" } }
                    // Android drops a notification's updates past a few a
                    // second; one that has ended has none, its job's or its
                    // service's notification going with it
                    val now = SystemClock.elapsedRealtime()
                    if (d.going && (now - shown >= 500 || d.step != "downloading")) {
                        shown = now
                        notes.notify(NOTE, note(app, name, d))
                    }
                }
            })
        } catch (e: Exception) {
            Log.w(TAG, "downloading $name", e)
            main.post {
                state = state?.let {
                    Download(it.name, "failed", it.got, it.total, 0.0, it.host, Problem("", "", e.toString()), retry = false, trying = false)
                }
            }
            ""
        }
        if (path.isNotEmpty()) {
            // in use once it checks, as the desktop's (main.rs, download_model)
            Models.use(app, name)
            Log.i(TAG, "downloaded $name")
        }
        main.post {
            Models.refresh(app)
            if (path.isNotEmpty())
                Models.offers.find { it.name == name }?.let {
                    Models.news = app.getString(R.string.models_ready, Models.label(app.resources, it.key))
                }
        }
        synchronized(this) {
            worker = null
            over?.invoke()
            over = null
        }
    }

    /** The download's notification: the model's, how far it has got, and the way back to the app, where its controls are. */
    fun note(context: Context, name: String, d: Download?): Notification {
        val res = context.resources
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, res.getString(R.string.channel_download), NotificationManager.IMPORTANCE_LOW),
        )
        val open = PendingIntent.getActivity(context, 0, Intent(context, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val key = Models.offers.find { it.name == name }?.key ?: Models.catalog(context).find { it.name == name }?.key ?: ""
        val b = Notification.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_note)
            .setContentTitle(res.getString(R.string.download_title, Models.label(res, key)))
            .setContentIntent(open)
            .setOngoing(true)
        when {
            d == null -> b.setProgress(0, 0, true)
            d.step == "checking" -> b.setContentText(res.getString(R.string.download_checking)).setProgress(0, 0, true)
            d.step == "failed" -> b.setContentText(words(res, d.problem?.key, d.problem?.host ?: ""))
            else -> b.setContentText(progress(res, d)).setProgress(1000, (1000 * d.got / d.total.coerceAtLeast(1)).toInt(), false)
        }
        return b.build()
    }
}

/** Megabytes as the desktop counts them, a million bytes, whole (app.js, mb). */
fun mb(bytes: Long): String = (bytes / 1e6).roundToLong().toString()

/** The fraction done, as a whole percent, down (app.js, percent). */
fun percent(x: Double): Int = (x.coerceIn(0.0, 1.0) * 100).toInt()

/** How far a download has got, in the desktop's words (app.js, updateDownload): "262 of 624 MB · 12.3 MB/s · about 30 s left". */
fun progress(res: Resources, d: Download): String {
    val got = mb(d.got)
    val total = mb(d.total)
    return when {
        d.step == "checking" -> res.getString(R.string.download_checking)
        d.step == "paused" -> res.getString(R.string.download_progress_paused, got, total)
        d.speed > 0 -> res.getString(
            R.string.download_progress_speed, got, total,
            String.format(Locale.getDefault(), "%.1f", d.speed / 1e6), timeLeft(res, (d.total - d.got) / d.speed),
        )
        else -> res.getString(R.string.download_progress, got, total)
    }
}

/** About how long is left, to 5 s under a minute, else to the minute (app.js, timeLeft). */
fun timeLeft(res: Resources, s: Double): String = when {
    !(s >= 0) || s.isInfinite() -> ""
    s < 50 -> res.getString(R.string.time_left_seconds, maxOf(5, (Math.round(s / 5) * 5).toInt()))
    else -> res.getString(R.string.time_left_minutes, Math.round(s / 60).toInt())
}

/**
 * A model's download as a user-initiated data transfer job (Android 14 and
 * later), which Android gives a transfer the person asked for: it runs with
 * the screen off or another app in front while there is a network, comes
 * back when the network does, and shows its notification (Downloads).
 */
@RequiresApi(34)
class DownloadJob : JobService() {
    override fun onStartJob(params: JobParameters): Boolean {
        val name = params.extras.getString(Downloads.NAME) ?: return false
        // the notification first, as a user-initiated job owes Android
        setNotification(params, Downloads.NOTE, Downloads.note(this, name, Downloads.state), JOB_END_NOTIFICATION_POLICY_REMOVE)
        return Downloads.run(this, name) { jobFinished(params, false) }
    }

    override fun onStopJob(params: JobParameters): Boolean {
        val network = params.stopReason == JobParameters.STOP_REASON_CONSTRAINT_CONNECTIVITY
        Downloads.stopped(this, network)
        // back with a network; stopped for anything else, it is paused
        return network
    }

    companion object {
        /** Whether JobScheduler took it: a user-initiated job is taken only while the app is in sight. */
        fun schedule(context: Context, o: Offer): Boolean {
            val job = JobInfo.Builder(Downloads.JOB, ComponentName(context, DownloadJob::class.java))
                .setUserInitiated(true)
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setEstimatedNetworkBytes(o.bytes - o.partial, 0)
                .setExtras(PersistableBundle().apply { putString(Downloads.NAME, o.name) })
                .build()
            return context.getSystemService(JobScheduler::class.java).schedule(job) == JobScheduler.RESULT_SUCCESS
        }
    }
}

/**
 * A model's download before Android 14, which has no user-initiated jobs:
 * a foreground service of the dataSync type, Android's for a transfer,
 * its notification Downloads'. The download tries again by itself while
 * the network is away.
 */
class DownloadService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val name = intent?.getStringExtra(Downloads.NAME)
        // first, as startForegroundService promised Android, within seconds
        startForeground(Downloads.NOTE, Downloads.note(this, name ?: "", Downloads.state), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        if (name == null || !Downloads.run(this, name) { done(startId) }) done(startId)
        return START_NOT_STICKY
    }

    private fun done(startId: Int) {
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf(startId)
    }
}
