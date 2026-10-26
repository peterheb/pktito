package net.pktito.app

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaPlayer
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * A transcript played back: the recording itself, from the file it was
 * read from or the copy of one shared to pktito (Recents), none of its
 * samples kept, so that it plays as it sounds. Android's player seeks
 * FLAC, MP4's AAC and WAV to where it is asked, and is handed those as
 * they are; an MP3 or an Ogg file it seeks seconds off, and those play as
 * views (Engine.view, src-tauri/src/view.rs): the recording from a time
 * on, a player of its own for each seek, prepared while the one before
 * waits, its time counted from the view's start. On the main thread;
 * position is in seconds, set by a seek whether or not there is anything
 * to play, and moves as the screen asks it to (tick) while it plays.
 */
object Player {
    var playing by mutableStateOf(false)
        private set
    var position by mutableStateOf(0.0)
        private set
    /** A recording is ready to play: without one the screen offers no play. */
    var ready by mutableStateOf(false)
        private set

    /**
     * Playback's speed, one of SPEEDS, and its volume, 0 to 1, as the desktop
     * keeps them (app.js, showSpeed and showVolume; the screen keeps them
     * between runs). The voice keeps its pitch at any speed (PlaybackParams'
     * own), and the playhead and the transcript follow the recording's own
     * time. Muted is for now: the volume is kept, and heard again unmuted.
     */
    var speed by mutableStateOf(1.0)
        private set
    var volume by mutableStateOf(1.0)
        private set
    var muted by mutableStateOf(false)
        private set
    val SPEEDS = listOf(0.75, 1.0, 1.25, 1.5, 1.75, 2.0)
    private var restored = false

    /** The speed and volume the last run left, once a run: a screen made anew keeps what this one has. */
    fun restore(speed: Double, volume: Double) {
        if (restored) return
        restored = true
        this.speed = if (speed in SPEEDS) speed else 1.0
        this.volume = volume.coerceIn(0.0, 1.0)
    }

    /** What is loaded, as load's caller names it; "" for nothing. */
    var loaded = ""
        private set
    private var mp: MediaPlayer? = null
    private var source: ViewSource? = null
    /** Where mp's time 0 is in the recording: its view's start, 0 for the file as it is. */
    private var start = 0.0
    /** The recording's views (Engine.viewIndex), 0 where it plays as it is. */
    private var index = 0L
    /** The recording's length, the transcript's. */
    private var length = 0.0
    /** A view's player being prepared, to take mp's place, playing if resume. */
    private var next: MediaPlayer? = null
    private var nextSource: ViewSource? = null
    private var resume = false
    /** A seek asked while next prepares: the latest is prepared after it. */
    private var then: Double? = null
    /** The playhead moved before the player was ready, which then follows it. */
    private var stale = false
    private var focus: AudioFocusRequest? = null
    /** The application's, for the focus given back from a player's own listeners, which have no screen at hand. */
    private var audio: AudioManager? = null

    /**
     * The recording open at file, as key names it, length seconds long, to
     * play: as views by index (Engine.viewIndex(file.fd), read off the main
     * thread by the caller), or as it is where index is 0. file is closed
     * once the player has it. why() hears it if Android cannot play it.
     */
    fun load(context: Context, key: String, file: ParcelFileDescriptor, index: Long, length: Double, why: () -> Unit) {
        release(context)
        audio = context.applicationContext.getSystemService(AudioManager::class.java)
        loaded = key
        this.index = index
        this.length = length
        val p = MediaPlayer()
        try {
            p.setAudioAttributes(attributes)
            if (index != 0L) {
                val s = view(0.0) ?: throw IllegalStateException("no view of $key")
                p.setDataSource(s)
                source = s
                start = Engine.viewStart(s.view)
            } else p.setDataSource(file.fileDescriptor)
        } catch (e: Exception) {
            Log.w(TAG, "playing $key", e)
            p.release()
            release(context)
            why()
            return
        } finally {
            // the player keeps a descriptor of its own, and the index its file
            runCatching { file.close() }
        }
        p.setOnPreparedListener {
            if (mp !== p) return@setOnPreparedListener
            heard(p)
            ready = true
            if (stale) {
                stale = false
                seek(context, position)
            }
        }
        p.setOnErrorListener { _, what, extra ->
            Log.w(TAG, "playing $key: error $what, $extra")
            if (mp === p) {
                release(context)
                why()
            }
            true
        }
        ended(p)
        mp = p
        p.prepareAsync()
    }

    private fun view(t: Double): ViewSource? = Engine.view(index, t).takeIf { it != 0L }?.let(::ViewSource)

    /** At its end it stops, at the recording's end. */
    private fun ended(p: MediaPlayer) {
        p.setOnCompletionListener {
            if (mp !== p) return@setOnCompletionListener
            playing = false
            position = length
            abandon()
        }
    }

    fun toggle(context: Context) {
        if (!ready) return
        if (playing) pause(context) else play(context)
    }

    /** From s seconds in; playing on if it was, or if play. With nothing to play, the playhead alone moves. */
    fun seek(context: Context, s: Double, play: Boolean = false) {
        position = s
        val p = mp ?: return
        if (!ready) {
            stale = true
            return
        }
        val on = playing || play
        if (index == 0L) {
            p.seekTo((s * 1000).toLong(), MediaPlayer.SEEK_CLOSEST)
            if (on && !playing) play(context)
            return
        }
        // a view from s, its player prepared while this one waits
        if (p.isPlaying) p.pause()
        if (on && !gain(context)) return
        playing = on
        cue(s, on)
    }

    /** A view's player from s, to take mp's place once it is prepared, or the one preparing's after it. */
    private fun cue(s: Double, play: Boolean) {
        resume = play
        if (next != null) {
            then = s
            return
        }
        then = null
        val src = view(s) ?: return
        val from = Engine.viewStart(src.view)
        val p = MediaPlayer()
        try {
            p.setAudioAttributes(attributes)
            p.setDataSource(src)
        } catch (e: Exception) {
            Log.w(TAG, "a view of $loaded from $s", e)
            p.release()
            src.close()
            return
        }
        p.setOnPreparedListener {
            if (next !== p) return@setOnPreparedListener
            next = null
            nextSource = null
            val later = then
            if (later != null) {
                p.release()
                src.close()
                cue(later, resume)
                return@setOnPreparedListener
            }
            mp?.release()
            source?.close()
            mp = p
            source = src
            start = from
            heard(p)
            ended(p)
            if (resume && playing) begin(p)
        }
        p.setOnErrorListener { _, what, extra ->
            Log.w(TAG, "a view of $loaded from $s: error $what, $extra")
            if (next === p) {
                next = null
                nextSource = null
                p.release()
                src.close()
                val later = then
                if (later != null) cue(later, resume)
                else if (playing) {
                    playing = false
                    abandon()
                }
            }
            true
        }
        next = p
        nextSource = src
        p.prepareAsync()
    }

    /** Set while it plays, the speed is heard at once; set while it waits, at play. */
    fun changeSpeed(s: Double) {
        speed = s
        mp?.takeIf { it.isPlaying }?.let { it.playbackParams = it.playbackParams.setSpeed(s.toFloat()) }
    }

    fun changeVolume(v: Double) {
        volume = v.coerceIn(0.0, 1.0)
        muted = false
        mp?.let(::heard)
    }

    /** Muted, or at none, the speaker brings the sound back: at none, at full (app.js, #mute). */
    fun toggleMute() {
        if (muted || volume == 0.0) {
            muted = false
            if (volume == 0.0) volume = 1.0
        } else muted = true
        mp?.let(::heard)
    }

    private fun heard(p: MediaPlayer) {
        val v = if (muted) 0f else volume.toFloat()
        p.setVolume(v, v)
    }

    fun pause(context: Context) {
        mp?.takeIf { it.isPlaying }?.pause()
        resume = false
        playing = false
        abandon()
    }

    /** Where it has got to, read while it plays; not while a seek's view is prepared, whose place the playhead has. */
    fun tick() {
        if (next != null) return
        mp?.takeIf { it.isPlaying }?.let { position = (start + it.currentPosition / 1000.0).coerceAtLeast(0.0) }
    }

    fun release(context: Context) {
        mp?.release()
        mp = null
        source?.close()
        source = null
        next?.release()
        next = null
        nextSource?.close()
        nextSource = null
        then = null
        stale = false
        start = 0.0
        if (index != 0L) Engine.viewIndexFree(index)
        index = 0L
        loaded = ""
        ready = false
        playing = false
        position = 0.0
        abandon()
    }

    private val attributes = AudioAttributes.Builder()
        .setUsage(AudioAttributes.USAGE_MEDIA)
        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
        .build()

    // another app's sound, or a call, pauses it, as Android asks of a player
    private fun gain(context: Context): Boolean {
        if (focus != null) return true
        val app = context.applicationContext
        val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(attributes)
            .setOnAudioFocusChangeListener { change ->
                if (change < 0) pause(app)
            }
            .build()
        val audio = app.getSystemService(AudioManager::class.java)
        this.audio = audio
        if (audio.requestAudioFocus(request) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) return false
        focus = request
        return true
    }

    private fun play(context: Context) {
        val p = mp ?: return
        if (!gain(context)) return
        if (position >= length - 0.05 && length > 0.1) {
            seek(context, 0.0, true)
            return
        }
        playing = true
        if (next != null) resume = true else begin(p)
    }

    /** p from where it waits, at the speed set: a speed set on a player that waits starts it, which is why changeSpeed leaves it to here. */
    private fun begin(p: MediaPlayer) {
        p.playbackParams = p.playbackParams.setSpeed(speed.toFloat())
        if (!p.isPlaying) p.start()
    }

    private fun abandon() {
        focus?.let { audio?.abandonAudioFocusRequest(it) }
        focus = null
    }

    private const val TAG = "pktito"
}
