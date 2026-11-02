package net.pktito.app

import android.media.MediaDataSource

/**
 * A view of a recording (Engine.view) as MediaPlayer reads it: the
 * player's thread asks for its bytes, and the view, cut and kept in Rust,
 * gives them, the file's own where it has them. The view goes when the
 * player is done with it.
 */
class ViewSource(val view: Long) : MediaDataSource() {
    private var closed = false

    @Synchronized
    override fun readAt(position: Long, buffer: ByteArray, offset: Int, size: Int): Int {
        if (closed) return -1
        if (size == 0) return 0
        return Engine.viewRead(view, position, buffer, offset, size)
    }

    @Synchronized
    override fun getSize(): Long = if (closed) -1 else Engine.viewSize(view)

    @Synchronized
    override fun close() {
        if (closed) return
        closed = true
        Engine.viewFree(view)
    }
}
