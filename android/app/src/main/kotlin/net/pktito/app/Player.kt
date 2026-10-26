package net.pktito.app

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaPlayer
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

    private var mp: MediaPlayer? = null
    private var path = ""
    private var focus: AudioFocusRequest? = null

    /** The audio at file, if it is not the one loaded; nothing if there is none. */
    fun load(context: Context, file: File) {
        if (file.path == path && mp != null) return
        release(context)
        if (!file.isFile) return
        val app = context.applicationContext
        mp = MediaPlayer().apply {
            setAudioAttributes(attributes)
            setDataSource(file.path)
            prepare()
            setOnCompletionListener {
                playing = false
                position = duration / 1000.0
                abandon(app)
            }
        }
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
        p.start()
        playing = true
    }

    private fun abandon(context: Context) {
        focus?.let { context.getSystemService(AudioManager::class.java).abandonAudioFocusRequest(it) }
        focus = null
    }
}
