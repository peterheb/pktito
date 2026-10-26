package net.pktito.app

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaPlayer
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File

/**
 * A transcript played back: the WAV Engine.transcribe wrote of the samples
 * pktito heard, so that it plays in step with the transcript's times
 * whatever the original's format, as the desktop's window plays it. On the
 * main thread; position is in seconds, and moves as the screen asks it to
 * (tick) while it plays.
 */
object Player {
    var playing by mutableStateOf(false)
        private set
    var position by mutableStateOf(0.0)
        private set
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

    private var mp: MediaPlayer? = null
    private var path = ""
    private var focus: AudioFocusRequest? = null

    /**
     * The audio at file, if it is not the one loaded; nothing if there is
     * none, or none Android can play, and then the transcript is shown
     * without play.
     */
    fun load(context: Context, file: File) {
        if (file.path == path && mp != null) return
        release(context)
        if (!file.isFile) return
        val app = context.applicationContext
        val p = MediaPlayer()
        try {
            p.setAudioAttributes(attributes)
            p.setDataSource(file.path)
            p.prepare()
        } catch (e: Exception) {
            Log.w("pktito", "playing ${file.path}", e)
            p.release()
            return
        }
        heard(p)
        p.setOnCompletionListener {
            playing = false
            position = p.duration / 1000.0
            abandon(app)
        }
        mp = p
        path = file.path
        ready = true
    }

    fun toggle(context: Context) {
        val p = mp ?: return
        if (p.isPlaying) pause(context) else play(context)
    }

    /** From s seconds in; playing on if it was, or if play. */
    fun seek(context: Context, s: Double, play: Boolean = false) {
        val p = mp ?: return
        p.seekTo((s * 1000).toLong(), MediaPlayer.SEEK_CLOSEST)
        position = s
        if (play && !p.isPlaying) play(context)
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
        playing = false
        abandon(context)
    }

    /** Where it has got to, read while it plays. */
    fun tick() {
        mp?.takeIf { it.isPlaying }?.let { position = it.currentPosition / 1000.0 }
    }

    fun release(context: Context) {
        mp?.release()
        mp = null
        path = ""
        ready = false
        playing = false
        position = 0.0
        abandon(context)
    }

    private val attributes = AudioAttributes.Builder()
        .setUsage(AudioAttributes.USAGE_MEDIA)
        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
        .build()

    // another app's sound, or a call, pauses it, as Android asks of a player
    private fun play(context: Context) {
        val p = mp ?: return
        val app = context.applicationContext
        val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(attributes)
            .setOnAudioFocusChangeListener { change ->
                if (change < 0) pause(app)
            }
            .build()
        val audio = app.getSystemService(AudioManager::class.java)
        if (audio.requestAudioFocus(request) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) return
        focus = request
        if (position >= p.duration / 1000.0 - 0.05) p.seekTo(0)
        // the speed with the start: a speed set on a player that waits
        // starts it, which is why changeSpeed leaves it to here
        p.playbackParams = p.playbackParams.setSpeed(speed.toFloat())
        if (!p.isPlaying) p.start()
        playing = true
    }

    private fun abandon(context: Context) {
        focus?.let { context.getSystemService(AudioManager::class.java).abandonAudioFocusRequest(it) }
        focus = null
    }
}
