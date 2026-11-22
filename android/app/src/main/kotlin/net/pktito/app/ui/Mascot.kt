package net.pktito.app.ui

import android.animation.ValueAnimator
import android.annotation.SuppressLint
import android.content.Context
import android.graphics.SurfaceTexture
import android.opengl.EGL14
import android.opengl.EGLConfig
import android.opengl.EGLContext
import android.opengl.EGLDisplay
import android.opengl.EGLSurface
import android.opengl.GLES30
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.Choreographer
import android.view.MotionEvent
import android.view.TextureView
import android.view.View
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.viewinterop.AndroidView
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.IntBuffer
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicReference
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.round
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.math.tan
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import net.pktito.app.R
import org.json.JSONArray
import org.json.JSONObject

/**
 * The mascot on About: a budgie of voxels, as the desktop draws it (ui/mascot.js, which this
 * follows function by function, its WebGL 2 here OpenGL ES 3.0 and its shaders the same text) and
 * as iOS does (ios/App/UI/Mascot.swift). Its model is assets/mascot.json, the desktop's
 * mascot-model.js without its wrapper (scripts/mascot.py writes all three). The first time it is
 * shown in a run it turns once; then it turns and tilts under a finger, spins on when flung and
 * turns back home 4.5 s after it is let go, hops and talks at a tap, and glances toward a mouse's
 * pointer (a Googlebook's, or a phone's on a desk). It draws only while something moves. With
 * Remove animations on it neither turns by itself nor hops. Until its voxels are meshed, and where
 * there is no OpenGL ES 3.0, the picture of it at rest (android/mascot.js), which a frame at rest
 * is drawn over.
 */
@Composable
fun Mascot(modifier: Modifier = Modifier) {
    val p = pk
    val context = LocalContext.current
    var scene by remember { mutableStateOf(MascotScene.cached) }
    LaunchedEffect(Unit) { if (scene == null) scene = MascotScene.load(context) }
    val label = stringResource(R.string.about_mascot)
    val ground = p.surface.toArgb()
    val dark = p === Dark
    Box(
        modifier.background(p.surface).clearAndSetSemantics {
            contentDescription = label
            role = Role.Image
        }
    ) {
        Image(
            painterResource(R.drawable.mascot),
            null,
            Modifier.fillMaxSize(),
            contentScale = ContentScale.Fit,
        )
        scene?.let { s ->
            AndroidView(
                { MascotStage(it, s) },
                Modifier.fillMaxSize(),
                update = { it.ground(ground, dark) },
            )
        }
    }
}

/** mascot.json: each chunk's cells, its size each way and where its first is. */
class MascotChunk(val d: IntArray, val o: FloatArray, val cells: ByteArray)

/**
 * The bubble's waveform: each bar's first x and their width, the middle they stand about, their
 * depth, colours and heights at rest.
 */
class MascotBars(
    val x0: IntArray,
    val w: Int,
    val yc: Float,
    val z0: Int,
    val z1: Int,
    val colors: ByteArray,
    val base: IntArray,
)

/**
 * A chunk's faces: per corner 20 bytes, its position (3 floats) and 8 bytes, the face, how open the
 * corner is from 0 to 3 (ambient occlusion), a jitter of the cell's shade, a spare, and its colour.
 */
class MascotMesh(val vertices: ByteBuffer, val indices: IntBuffer, val count: Int)

/**
 * a face's normal and the two edges along it, nx ny nz ux uy uz vx vy vz; its index is the shader's
 */
private val FACES =
    arrayOf(
        intArrayOf(1, 0, 0, 0, 1, 0, 0, 0, 1),
        intArrayOf(-1, 0, 0, 0, 0, 1, 0, 1, 0),
        intArrayOf(0, 1, 0, 0, 0, 1, 1, 0, 0),
        intArrayOf(0, -1, 0, 1, 0, 0, 0, 0, 1),
        intArrayOf(0, 0, 1, 1, 0, 0, 0, 1, 0),
        intArrayOf(0, 0, -1, 0, 1, 0, 1, 0, 0),
    )

/**
 * Every face of a filled cell with an empty one beside it, as a quad (mascot.js, meshGrid): hidden
 * faces culled, corners shaded.
 */
fun mascotMesh(g: ByteArray, d: IntArray, o: FloatArray, palette: Array<IntArray>): MascotMesh {
    val (dx, dy, dz) = d
    val sx = dy * dz
    val sy = dz
    fun solid(x: Int, y: Int, z: Int): Int =
        if (x < 0 || y < 0 || z < 0 || x >= dx || y >= dy || z >= dz) 0
        else if (g[x * sx + y * sy + z].toInt() != 0) 1 else 0
    var count = 0
    for (x in 0 until dx) for (y in 0 until dy) for (z in 0 until dz) {
        if (g[x * sx + y * sy + z].toInt() == 0) continue
        for (f in FACES) if (solid(x + f[0], y + f[1], z + f[2]) == 0) count++
    }
    val vb = ByteBuffer.allocateDirect(count * 4 * 20).order(ByteOrder.nativeOrder())
    val ib = ByteBuffer.allocateDirect(count * 6 * 4).order(ByteOrder.nativeOrder()).asIntBuffer()
    val ao = IntArray(4)
    var vi = 0
    for (x in 0 until dx) for (y in 0 until dy) for (z in 0 until dz) {
        val v = g[x * sx + y * sy + z].toInt() and 0xff
        if (v == 0) continue
        val rgb = palette[v]
        // the low byte of the cell's hash, as UInt32's wrap leaves it
        val jit = ((x * 73_856_093) xor (y * 19_349_663) xor (z * 83_492_791)).toByte()
        for ((fi, f) in FACES.withIndex()) {
            val nx = x + f[0]
            val ny = y + f[1]
            val nz = z + f[2]
            if (solid(nx, ny, nz) != 0) continue
            // the face's first corner: the cell's, a step on where the face
            // looks the way its axis grows
            val bx = x + max(f[0], 0)
            val by = y + max(f[1], 0)
            val bz = z + max(f[2], 0)
            for (k in 0 until 4) {
                val du = if (k == 1 || k == 2) 1 else 0
                val dv = if (k == 2 || k == 3) 1 else 0
                val su = if (du == 1) 1 else -1
                val sv = if (dv == 1) 1 else -1
                val s1 = solid(nx + f[3] * su, ny + f[4] * su, nz + f[5] * su)
                val s2 = solid(nx + f[6] * sv, ny + f[7] * sv, nz + f[8] * sv)
                val c =
                    solid(
                        nx + f[3] * su + f[6] * sv,
                        ny + f[4] * su + f[7] * sv,
                        nz + f[5] * su + f[8] * sv,
                    )
                ao[k] = if (s1 == 1 && s2 == 1) 0 else 3 - (s1 + s2 + c)
                vb.putFloat(o[0] + bx + f[3] * du + f[6] * dv)
                vb.putFloat(o[1] + by + f[4] * du + f[7] * dv)
                vb.putFloat(o[2] + bz + f[5] * du + f[8] * dv)
                vb.put(fi.toByte()).put(ao[k].toByte()).put(jit).put(0.toByte())
                vb.put(rgb[0].toByte()).put(rgb[1].toByte()).put(rgb[2].toByte()).put(0.toByte())
            }
            // the quad cut along the diagonal between its darker corners, as
            // voxel renderers do, so that the shading runs evenly across it
            val q =
                if (ao[0] + ao[2] > ao[1] + ao[3]) intArrayOf(1, 2, 3, 1, 3, 0)
                else intArrayOf(0, 1, 2, 0, 2, 3)
            for (k in q) ib.put(vi + k)
            vi += 4
        }
    }
    vb.flip()
    ib.flip()
    return MascotMesh(vb, ib, count * 6)
}

/**
 * The model and what of it never changes, meshed: the bird and its props. The bubble's cells are
 * meshed with each set of its bars' heights.
 */
class MascotScene(
    val palette: Array<IntArray>,
    val bubble: MascotChunk,
    val bars: MascotBars,
    val pivot: FloatArray,
    val rxz: Float,
    val ground: FloatArray,
    val bird: MascotMesh,
    val props: MascotMesh,
) {
    companion object {
        /** meshed once a run, off the main thread */
        var cached: MascotScene? = null

        suspend fun load(context: Context): MascotScene? {
            cached?.let {
                return it
            }
            val made =
                withContext(Dispatchers.Default) {
                    runCatching {
                        val t0 = SystemClock.elapsedRealtime()
                        val text =
                            context.assets.open("mascot.json").bufferedReader().use {
                                it.readText()
                            }
                        val json = JSONObject(text)
                        val t1 = SystemClock.elapsedRealtime()
                        parse(json).also {
                            val t2 = SystemClock.elapsedRealtime()
                            Log.i("pktito", "the mascot: read ${t1 - t0} ms, meshed ${t2 - t1} ms")
                        }
                    }
                        .getOrNull()
                }
            cached = made
            return made
        }

        private fun ints(a: JSONArray) = IntArray(a.length()) { a.getInt(it) }

        private fun floats(a: JSONArray) = FloatArray(a.length()) { a.getDouble(it).toFloat() }

        /**
         * The cells, x slowest and z fastest, from the runs: [count, colour, count, colour, ...]
         */
        private fun chunk(c: JSONObject): MascotChunk {
            val d = ints(c.getJSONArray("d"))
            val runs = ints(c.getJSONArray("runs"))
            val cells = ByteArray(d[0] * d[1] * d[2])
            var at = 0
            for (i in 0 until runs.size - 1 step 2) {
                cells.fill(runs[i + 1].toByte(), at, at + runs[i])
                at += runs[i]
            }
            return MascotChunk(d, floats(c.getJSONArray("o")), cells)
        }

        private fun parse(m: JSONObject): MascotScene {
            val pal = m.getJSONArray("palette")
            val palette = Array(pal.length()) { ints(pal.getJSONArray(it)) }
            val chunks = m.getJSONObject("chunks")
            val bird = chunk(chunks.getJSONObject("bird"))
            val props = chunk(chunks.getJSONObject("props"))
            val b = m.getJSONObject("bars")
            val colors = ints(b.getJSONArray("colors"))
            return MascotScene(
                palette,
                chunk(chunks.getJSONObject("bubble")),
                MascotBars(
                    ints(b.getJSONArray("x0")),
                    b.getInt("w"),
                    b.getDouble("yc").toFloat(),
                    b.getInt("z0"),
                    b.getInt("z1"),
                    ByteArray(colors.size) { colors[it].toByte() },
                    ints(b.getJSONArray("base")),
                ),
                floats(m.getJSONArray("pivot")),
                m.getDouble("rxz").toFloat(),
                floats(m.getJSONArray("ground")),
                mascotMesh(bird.cells, bird.d, bird.o, palette),
                mascotMesh(props.cells, props.d, props.o, palette),
            )
        }
    }
}

private fun rad(d: Double) = d * PI / 180

private fun clamp(v: Double, a: Double, b: Double) = min(b, max(a, v))

/**
 * What one frame shows: where it is turned, how high the bird is, the bars' heights, and the ground
 * under it.
 */
private class MascotFrame(
    val yaw: Float,
    val pitch: Float,
    val hop: Float,
    val bars: IntArray,
    val ground: Int,
    val dark: Boolean,
)

/**
 * The stage: a TextureView, which draws over the picture under it and scrolls with About as a view
 * does, where a SurfaceView's hole would show black until its first frame. What moves is reckoned
 * here on the main thread at each of Choreographer's frames, from its clock, so that a 120 Hz
 * screen turns it no faster than a 60 Hz one; the frame is drawn on a thread of its own.
 */
@SuppressLint("ViewConstructor")
class MascotStage(context: Context, private val scene: MascotScene) :
    TextureView(context), TextureView.SurfaceTextureListener, Choreographer.FrameCallback {

    companion object {
        /** one turn the first time it is shown in a run, as the desktop's */
        private var introDone = false
    }

    private val homePitch = rad(7.0)
    private val pitchMin = rad(-6.0)
    private val pitchMax = rad(48.0)

    // the state, as mascot.js's S: angles in radians, times in milliseconds
    private var intro = 0.0
    private var introStart = -1.0
    private var yawDrag = 0.0
    private var pitchDrag = 0.0
    private var vel = 0.0
    private var gx = 0.0
    private var gy = 0.0
    private var tgx = 0.0
    private var tgy = 0.0
    private var dragging = false
    private var lastInteract = -1e9
    private var hopStart = -1.0
    private var talkUntil = 0.0
    private var last = 0.0
    private var running = false
    private var groundColor = 0
    private var dark = false
    private var bars = scene.bars.base

    // a finger's: where it was, how far it has gone, and when it last moved
    private var downX = 0f
    private var downY = 0f
    private var moved = 0f
    private var lastMove = -1.0

    private val main = Handler(Looper.getMainLooper())
    private val home = Runnable { wake() }
    private val start = Runnable {
        if (isAttachedToWindow && !introDone && introStart < 0) {
            introStart = now()
            wake()
        }
    }
    private var render: Render? = null

    /** under Remove animations, as the desktop's prefers-reduced-motion */
    private val calm
        get() = !ValueAnimator.areAnimatorsEnabled()

    /** Choreographer's clock, System.nanoTime's, in milliseconds */
    private fun now() = System.nanoTime() / 1e6

    init {
        isOpaque = false
        surfaceTextureListener = this
        // the Box around it says what it is
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
    }

    /** Cleared to the surface under it, the theme's. */
    fun ground(argb: Int, dark: Boolean) {
        if (argb == groundColor && dark == this.dark) return
        groundColor = argb
        this.dark = dark
        wake()
    }

    // ------------------------------------------ the loop, which runs while something moves --

    private fun wake() {
        if (!isAttachedToWindow || running) return
        running = true
        last = now()
        Choreographer.getInstance().postFrameCallback(this)
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        wake()
        if (!introDone && !calm && introStart < 0) main.postDelayed(start, 350)
    }

    /** home, for the next time, and a turn under way is not begun again */
    override fun onDetachedFromWindow() {
        if (introStart >= 0) introDone = true
        main.removeCallbacks(start)
        main.removeCallbacks(home)
        Choreographer.getInstance().removeFrameCallback(this)
        running = false
        super.onDetachedFromWindow()
    }

    override fun doFrame(frameTimeNanos: Long) {
        val now = frameTimeNanos / 1e6
        val busy = step(now)
        var hop = 0.0
        if (hopStart >= 0) {
            val t = (now - hopStart) / 520
            if (t >= 1) hopStart = -1.0 else hop = 7 * sin(PI * t) * (1 - 0.25 * t)
        }
        render?.show(
            MascotFrame(
                (intro + yawDrag + gx).toFloat(),
                clamp(homePitch + pitchDrag + gy, pitchMin, pitchMax).toFloat(),
                hop.toFloat(),
                bars,
                groundColor,
                dark,
            )
        )
        if (busy || hopStart >= 0) Choreographer.getInstance().postFrameCallback(this)
        else running = false
    }

    /** What moves, moved to now; whether there is more to come. */
    private fun step(now: Double): Boolean {
        val dt = min(0.05, max(0.0, now - last) / 1000)
        last = now
        var busy = false

        // one turn, the first time it is shown
        if (!introDone && introStart >= 0) {
            val p = (now - introStart) / 3800
            if (p >= 1) {
                intro = 0.0
                introDone = true
                talkUntil = max(talkUntil, now + 250)
            } else {
                intro = PI * 2 * (if (p < 0.5) 4 * p * p * p else 1 - (-2 * p + 2).pow(3) / 2)
                busy = true
            }
        }

        // the glance toward the pointer
        val gk = 1 - exp(-dt * 5)
        gx += (tgx - gx) * gk
        gy += (tgy - gy) * gk
        if (abs(tgx - gx) > 1e-4 || abs(tgy - gy) > 1e-4) busy = true

        // a drag's spin running down, then an unhurried turn back home
        if (!dragging) {
            if (abs(vel) > 0.02) {
                yawDrag += vel * dt
                vel *= exp(-dt * 3.2)
                busy = true
            } else vel = 0.0
            val turned = yawDrag != 0.0 || pitchDrag != 0.0
            if (now - lastInteract > 4500 && turned) {
                val home = round(yawDrag / (PI * 2)) * PI * 2
                val rk = 1 - exp(-dt * 2.2)
                yawDrag += (home - yawDrag) * rk
                pitchDrag += (0 - pitchDrag) * rk
                if (abs(home - yawDrag) < 5e-4 && abs(pitchDrag) < 5e-4) {
                    yawDrag = 0.0
                    pitchDrag = 0.0
                }
                busy = true
            } else if (turned) {
                // until the turn home, the same frame each frame: none drawn,
                // and the loop woken when it is due
                main.removeCallbacks(this.home)
                main.postDelayed(this.home, (lastInteract + 4500 - now + 1).toLong())
            }
        }

        // the bubble's waveform talks during the turn and after a chirp
        val rest = scene.bars.base
        if ((!introDone && introStart >= 0) || now < talkUntil) {
            val t = floor(now / 85) * 85 / 1000
            bars =
                IntArray(rest.size) { i ->
                    val k = 0.5 + 0.5 * sin(t * 9 + i * 1.7) * cos(t * 5.3 + i * 0.9)
                    max(2, (rest[i] * (0.35 + 0.95 * k)).roundToInt())
                }
            busy = true
        } else bars = rest
        return busy
    }

    // ---------------------------------------------------------------------------- input --

    private fun dragPitch(v: Double) = clamp(v, pitchMin - homePitch, pitchMax - homePitch)

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(e: MotionEvent): Boolean {
        val now = now()
        val dp = resources.displayMetrics.density
        when (e.actionMasked) {
            // a finger down stops what it was doing, as the desktop's pointer
            // does, and the stage keeps the drag, as a map does in a page
            // that scrolls
            MotionEvent.ACTION_DOWN -> {
                parent?.requestDisallowInterceptTouchEvent(true)
                downX = e.x
                downY = e.y
                moved = 0f
                lastMove = -1.0
                dragging = true
                vel = 0.0
                introDone = true
                intro = 0.0
                tgx = 0.0
                tgy = 0.0
            }
            MotionEvent.ACTION_MOVE -> {
                // in dp, as the desktop's are CSS pixels and iOS's points
                val dx = (e.x - downX) / dp
                val dy = (e.y - downY) / dp
                downX = e.x
                downY = e.y
                moved += abs(dx) + abs(dy)
                val dyaw = dx * 0.0105
                yawDrag += dyaw
                pitchDrag = dragPitch(pitchDrag + dy * 0.006)
                val since = (now - (if (lastMove < 0) now - 16 else lastMove)) / 1000
                vel = vel * 0.6 + dyaw / max(0.008, since) * 0.4
                lastMove = now
            }
            MotionEvent.ACTION_UP -> {
                dragging = false
                // under 6 dp is a tap, not a drag
                if (moved < 6) {
                    vel = 0.0
                    chirp(now)
                    performClick()
                } else vel = clamp(vel, -6.0, 6.0)
            }
            MotionEvent.ACTION_CANCEL -> dragging = false
            else -> return true
        }
        lastInteract = now
        wake()
        return true
    }

    override fun performClick(): Boolean = super.performClick()

    /** a hop, and the bubble talks */
    private fun chirp(now: Double) {
        if (calm) return
        hopStart = now
        talkUntil = now + 1400
        wake()
    }

    /** a glance toward a mouse's pointer, by small angles */
    override fun onHoverEvent(e: MotionEvent): Boolean {
        val mouse = e.isFromSource(android.view.InputDevice.SOURCE_MOUSE)
        if (
            !mouse ||
                dragging ||
                calm ||
                e.actionMasked == MotionEvent.ACTION_HOVER_EXIT ||
                width == 0
        ) {
            tgx = 0.0
            tgy = 0.0
        } else {
            val root = rootView
            val at = IntArray(2).also { getLocationInWindow(it) }
            val cx = at[0] + width / 2.0
            val cy = at[1] + height / 2.0
            val x = at[0] + e.x
            val y = at[1] + e.y
            tgx = clamp((x - cx) / (root.width / 2.0), -1.0, 1.0) * rad(20.0)
            tgy = clamp((y - cy) / (root.height / 2.0), -1.0, 1.0) * rad(7.0)
        }
        wake()
        return mouse
    }

    // ------------------------------------------------------------------------- surface --

    override fun onSurfaceTextureAvailable(st: SurfaceTexture, w: Int, h: Int) {
        render = Render(scene, st, w, h)
        wake()
    }

    override fun onSurfaceTextureSizeChanged(st: SurfaceTexture, w: Int, h: Int) {
        render?.size(w, h)
        wake()
    }

    override fun onSurfaceTextureDestroyed(st: SurfaceTexture): Boolean {
        render?.end()
        render = null
        return true
    }

    override fun onSurfaceTextureUpdated(st: SurfaceTexture) {}
}

/**
 * The drawing, on a thread of its own with its own EGL context: an ES 3.0 one, antialiased 4x where
 * the GPU offers it, as the desktop's canvas is. Its window surface is the default colour space,
 * not sRGB: the shaders' colours are already the display's (the palette's bytes), and an sRGB
 * target would lighten them twice. Only the newest frame asked for is drawn.
 */
private class Render(
    private val scene: MascotScene,
    texture: SurfaceTexture,
    w: Int,
    h: Int,
) {
    private val thread = HandlerThread("mascot").apply { start() }
    private val gl = Handler(thread.looper)
    private val next = AtomicReference<MascotFrame?>(null)
    @Volatile private var width = w
    @Volatile private var height = h

    private var display: EGLDisplay = EGL14.EGL_NO_DISPLAY
    private var context: EGLContext = EGL14.EGL_NO_CONTEXT
    private var surface: EGLSurface = EGL14.EGL_NO_SURFACE
    private var ok = false

    private var vox = 0
    private var gnd = 0

    private class Mesh(val vao: Int, val vb: Int, val ib: Int, var count: Int)

    private var bird: Mesh? = null
    private var props: Mesh? = null
    private var bubble: Mesh? = null
    private var gndVao = 0
    private var shown: IntArray? = null

    init {
        gl.post { ok = runCatching { begin(texture) }.getOrDefault(false) }
    }

    fun size(w: Int, h: Int) {
        width = w
        height = h
    }

    fun show(f: MascotFrame) {
        // a frame already waiting is replaced, not drawn as well
        if (next.getAndSet(f) == null) gl.post { next.getAndSet(null)?.let { if (ok) draw(it) } }
    }

    /** the context and surface let go before the texture is, and the thread with them */
    fun end() {
        val done = CountDownLatch(1)
        gl.post {
            if (display != EGL14.EGL_NO_DISPLAY) {
                EGL14.eglMakeCurrent(
                    display,
                    EGL14.EGL_NO_SURFACE,
                    EGL14.EGL_NO_SURFACE,
                    EGL14.EGL_NO_CONTEXT,
                )
                if (surface != EGL14.EGL_NO_SURFACE) EGL14.eglDestroySurface(display, surface)
                if (context != EGL14.EGL_NO_CONTEXT) EGL14.eglDestroyContext(display, context)
                EGL14.eglReleaseThread()
            }
            ok = false
            done.countDown()
        }
        done.await()
        thread.quitSafely()
    }

    private fun config(samples: Int): EGLConfig? {
        val attrs =
            intArrayOf(
                EGL14.EGL_RED_SIZE,
                8,
                EGL14.EGL_GREEN_SIZE,
                8,
                EGL14.EGL_BLUE_SIZE,
                8,
                EGL14.EGL_ALPHA_SIZE,
                8,
                EGL14.EGL_DEPTH_SIZE,
                24,
                EGL14.EGL_RENDERABLE_TYPE,
                0x40, // EGL_OPENGL_ES3_BIT_KHR
                EGL14.EGL_SURFACE_TYPE,
                EGL14.EGL_WINDOW_BIT,
                EGL14.EGL_SAMPLE_BUFFERS,
                if (samples > 0) 1 else 0,
                EGL14.EGL_SAMPLES,
                samples,
                EGL14.EGL_NONE,
            )
        val out = arrayOfNulls<EGLConfig>(1)
        val n = IntArray(1)
        return if (EGL14.eglChooseConfig(display, attrs, 0, out, 0, 1, n, 0) && n[0] > 0) out[0]
        else null
    }

    private fun begin(texture: SurfaceTexture): Boolean {
        display = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY)
        if (!EGL14.eglInitialize(display, IntArray(2), 0, IntArray(2), 1)) return false
        val cfg = config(4) ?: config(0) ?: return false
        context =
            EGL14.eglCreateContext(
                display,
                cfg,
                EGL14.EGL_NO_CONTEXT,
                intArrayOf(EGL14.EGL_CONTEXT_CLIENT_VERSION, 3, EGL14.EGL_NONE),
                0,
            )
        if (context == EGL14.EGL_NO_CONTEXT) return false
        surface = EGL14.eglCreateWindowSurface(display, cfg, texture, intArrayOf(EGL14.EGL_NONE), 0)
        if (surface == EGL14.EGL_NO_SURFACE) return false
        if (!EGL14.eglMakeCurrent(display, surface, surface, context)) return false

        vox = program(VOX_V, VOX_F)
        gnd = program(GND_V, GND_F)
        if (vox == 0 || gnd == 0) return false
        bird = mesh(scene.bird)
        props = mesh(scene.props)
        bubble = mesh(null)
        val ids = IntArray(2)
        GLES30.glGenVertexArrays(1, ids, 0)
        gndVao = ids[0]
        GLES30.glBindVertexArray(gndVao)
        GLES30.glGenBuffers(1, ids, 1)
        GLES30.glBindBuffer(GLES30.GL_ARRAY_BUFFER, ids[1])
        val quad = floatArrayOf(-90f, -70f, 110f, -70f, -90f, 70f, 110f, 70f)
        val qb =
            ByteBuffer.allocateDirect(quad.size * 4).order(ByteOrder.nativeOrder()).apply {
                asFloatBuffer().put(quad)
            }
        GLES30.glBufferData(GLES30.GL_ARRAY_BUFFER, quad.size * 4, qb, GLES30.GL_STATIC_DRAW)
        GLES30.glEnableVertexAttribArray(0)
        GLES30.glVertexAttribPointer(0, 2, GLES30.GL_FLOAT, false, 0, 0)
        GLES30.glBindVertexArray(0)
        return true
    }

    private fun program(vs: String, fs: String): Int {
        val p = GLES30.glCreateProgram()
        for ((type, src) in
            listOf(GLES30.GL_VERTEX_SHADER to vs, GLES30.GL_FRAGMENT_SHADER to fs)) {
            val s = GLES30.glCreateShader(type)
            GLES30.glShaderSource(s, src)
            GLES30.glCompileShader(s)
            val status = IntArray(1)
            GLES30.glGetShaderiv(s, GLES30.GL_COMPILE_STATUS, status, 0)
            if (status[0] == 0) return 0
            GLES30.glAttachShader(p, s)
        }
        GLES30.glLinkProgram(p)
        val status = IntArray(1)
        GLES30.glGetProgramiv(p, GLES30.GL_LINK_STATUS, status, 0)
        return if (status[0] == 0) 0 else p
    }

    private fun mesh(m: MascotMesh?): Mesh {
        val ids = IntArray(3)
        GLES30.glGenVertexArrays(1, ids, 0)
        GLES30.glGenBuffers(2, ids, 1)
        val mesh = Mesh(ids[0], ids[1], ids[2], 0)
        GLES30.glBindVertexArray(mesh.vao)
        GLES30.glBindBuffer(GLES30.GL_ARRAY_BUFFER, mesh.vb)
        GLES30.glBindBuffer(GLES30.GL_ELEMENT_ARRAY_BUFFER, mesh.ib)
        GLES30.glEnableVertexAttribArray(0)
        GLES30.glVertexAttribPointer(0, 3, GLES30.GL_FLOAT, false, 20, 0)
        GLES30.glEnableVertexAttribArray(1)
        GLES30.glVertexAttribPointer(1, 4, GLES30.GL_UNSIGNED_BYTE, true, 20, 12)
        GLES30.glEnableVertexAttribArray(2)
        GLES30.glVertexAttribPointer(2, 4, GLES30.GL_UNSIGNED_BYTE, true, 20, 16)
        GLES30.glBindVertexArray(0)
        if (m != null) upload(mesh, m, GLES30.GL_STATIC_DRAW)
        return mesh
    }

    private fun upload(mesh: Mesh, m: MascotMesh, use: Int) {
        GLES30.glBindVertexArray(mesh.vao)
        GLES30.glBindBuffer(GLES30.GL_ARRAY_BUFFER, mesh.vb)
        GLES30.glBufferData(GLES30.GL_ARRAY_BUFFER, m.vertices.limit(), m.vertices, use)
        GLES30.glBindBuffer(GLES30.GL_ELEMENT_ARRAY_BUFFER, mesh.ib)
        GLES30.glBufferData(GLES30.GL_ELEMENT_ARRAY_BUFFER, m.indices.limit() * 4, m.indices, use)
        GLES30.glBindVertexArray(0)
        mesh.count = m.count
    }

    /**
     * The waveform's bars, stamped into the bubble's cells and meshed again for each new set of
     * heights.
     */
    private fun setBars(heights: IntArray) {
        if (shown?.contentEquals(heights) == true) return
        shown = heights
        val c = scene.bubble
        val b = scene.bars
        val g = c.cells.copyOf()
        val sx = c.d[1] * c.d[2]
        val sy = c.d[2]
        val ox = c.o[0].toInt()
        val oy = c.o[1].toInt()
        val oz = c.o[2].toInt()
        for ((i, x0) in b.x0.withIndex()) {
            val h = heights[i]
            // rounded as the desktop's Math.round, a half upward
            val y0 = floor(b.yc - h / 2f + 0.5f).toInt() - oy
            for (x in x0 until x0 + b.w) for (y in y0 until y0 + h) for (z in b.z0 until b.z1) {
                g[(x - ox) * sx + y * sy + (z - oz)] = b.colors[i]
            }
        }
        upload(bubble!!, mascotMesh(g, c.d, c.o, scene.palette), GLES30.GL_DYNAMIC_DRAW)
    }

    private fun draw(f: MascotFrame) {
        val w = width
        val h = height
        if (w <= 0 || h <= 0) return
        setBars(f.bars)
        GLES30.glViewport(0, 0, w, h)
        val gc = f.ground
        GLES30.glClearColor(
            (gc shr 16 and 0xff) / 255f,
            (gc shr 8 and 0xff) / 255f,
            (gc and 0xff) / 255f,
            1f,
        )
        GLES30.glClear(GLES30.GL_COLOR_BUFFER_BIT or GLES30.GL_DEPTH_BUFFER_BIT)

        // far enough back that the whole scene fits whichever way it turns
        val aspect = w.toFloat() / h
        val fov = rad(24.0).toFloat()
        val halfH = 70 * 1.08f
        val halfW = scene.rxz * 1.07f
        val dist = max(halfH, halfW / aspect) / tan(fov / 2)
        val pv = scene.pivot
        val mv =
            mul(
                trans(0f, -2f, -dist),
                mul(rotX(f.pitch), mul(rotY(f.yaw), trans(-pv[0], -pv[1], -pv[2]))),
            )
        val mvp = mul(persp(fov, aspect, dist * 0.35f, dist * 2.4f), mv)

        // the shadow first, without depth, then the voxels
        GLES30.glDisable(GLES30.GL_DEPTH_TEST)
        GLES30.glEnable(GLES30.GL_BLEND)
        GLES30.glBlendFunc(GLES30.GL_ONE, GLES30.GL_ONE_MINUS_SRC_ALPHA)
        GLES30.glUseProgram(gnd)
        GLES30.glUniformMatrix4fv(GLES30.glGetUniformLocation(gnd, "uMVP"), 1, false, mvp, 0)
        GLES30.glUniform4fv(GLES30.glGetUniformLocation(gnd, "uBlob"), 1, scene.ground, 0)
        // lighter while the bird is in the air
        GLES30.glUniform1f(GLES30.glGetUniformLocation(gnd, "uStr"), 1 - f.hop / 16)
        val shadow =
            if (f.dark) floatArrayOf(0.01f, 0.03f, 0.09f, 0.55f)
            else floatArrayOf(0.22f, 0.36f, 0.64f, 0.36f)
        GLES30.glUniform4fv(GLES30.glGetUniformLocation(gnd, "uShadow"), 1, shadow, 0)
        GLES30.glBindVertexArray(gndVao)
        GLES30.glDrawArrays(GLES30.GL_TRIANGLE_STRIP, 0, 4)

        GLES30.glDisable(GLES30.GL_BLEND)
        GLES30.glEnable(GLES30.GL_DEPTH_TEST)
        GLES30.glEnable(GLES30.GL_CULL_FACE)
        GLES30.glUseProgram(vox)
        GLES30.glUniformMatrix4fv(GLES30.glGetUniformLocation(vox, "uMVP"), 1, false, mvp, 0)
        GLES30.glUniformMatrix4fv(GLES30.glGetUniformLocation(vox, "uMV"), 1, false, mv, 0)
        GLES30.glUniform3fv(
            GLES30.glGetUniformLocation(vox, "uL1"),
            1,
            norm3(-0.5f, 0.72f, 0.62f),
            0,
        )
        GLES30.glUniform3fv(
            GLES30.glGetUniformLocation(vox, "uL2"),
            1,
            norm3(0.78f, 0.12f, 0.3f),
            0,
        )
        val off = GLES30.glGetUniformLocation(vox, "uOffset")
        val jit = GLES30.glGetUniformLocation(vox, "uJit")
        fun drawMesh(m: Mesh?, oy: Float, j: Float) {
            if (m == null || m.count == 0) return
            GLES30.glUniform3f(off, 0f, oy, 0f)
            GLES30.glUniform1f(jit, j)
            GLES30.glBindVertexArray(m.vao)
            GLES30.glDrawElements(GLES30.GL_TRIANGLES, m.count, GLES30.GL_UNSIGNED_INT, 0)
        }
        drawMesh(props, 0f, 0.05f)
        drawMesh(bubble, 0f, 0.025f)
        drawMesh(bird, f.hop, 0.11f)
        GLES30.glBindVertexArray(0)
        EGL14.eglSwapBuffers(display, surface)
    }

    // ------------------------------------------------------------ matrices, column-major --

    private fun mul(a: FloatArray, b: FloatArray): FloatArray {
        val o = FloatArray(16)
        for (c in 0 until 4) for (r in 0 until 4) {
            var s = 0f
            for (k in 0 until 4) s += a[k * 4 + r] * b[c * 4 + k]
            o[c * 4 + r] = s
        }
        return o
    }

    private fun trans(x: Float, y: Float, z: Float) =
        floatArrayOf(1f, 0f, 0f, 0f, 0f, 1f, 0f, 0f, 0f, 0f, 1f, 0f, x, y, z, 1f)

    private fun rotX(a: Float): FloatArray {
        val c = cos(a)
        val s = sin(a)
        return floatArrayOf(1f, 0f, 0f, 0f, 0f, c, s, 0f, 0f, -s, c, 0f, 0f, 0f, 0f, 1f)
    }

    private fun rotY(a: Float): FloatArray {
        val c = cos(a)
        val s = sin(a)
        return floatArrayOf(c, 0f, -s, 0f, 0f, 1f, 0f, 0f, s, 0f, c, 0f, 0f, 0f, 0f, 1f)
    }

    private fun persp(fovy: Float, asp: Float, n: Float, f: Float): FloatArray {
        val t = 1 / tan(fovy / 2)
        return floatArrayOf(
            t / asp,
            0f,
            0f,
            0f,
            0f,
            t,
            0f,
            0f,
            0f,
            0f,
            (f + n) / (n - f),
            -1f,
            0f,
            0f,
            2 * f * n / (n - f),
            0f,
        )
    }

    private fun norm3(x: Float, y: Float, z: Float): FloatArray {
        val l = sqrt(x * x + y * y + z * z)
        return floatArrayOf(x / l, y / l, z / l)
    }
}

// The shaders, the desktop's text (ui/mascot.js): the voxels under a key light
// from the upper left, a weak fill from the right and a sky over a ground for
// the ambient, and the shadow on the ground, a soft blob under the bird on one
// quad.

private const val VOX_V =
    """#version 300 es
precision highp float;
layout(location=0) in vec3 aPos;
layout(location=1) in vec4 aMeta;
layout(location=2) in vec4 aCol;
uniform mat4 uMVP;
uniform mat4 uMV;
uniform vec3 uOffset;
uniform float uJit;
out vec3 vCol; out float vAO; out vec3 vN;
const vec3 N[6] = vec3[6](vec3(1,0,0), vec3(-1,0,0), vec3(0,1,0), vec3(0,-1,0), vec3(0,0,1), vec3(0,0,-1));
void main() {
  gl_Position = uMVP * vec4(aPos + uOffset, 1.0);
  int f = int(aMeta.x * 255.0 + 0.5);
  vN = mat3(uMV) * N[f];
  vAO = aMeta.y * 255.0 / 3.0;
  vCol = aCol.rgb * (1.0 + (aMeta.z - 0.5) * uJit);
}"""

private const val VOX_F =
    """#version 300 es
precision highp float;
in vec3 vCol; in float vAO; in vec3 vN;
uniform vec3 uL1; uniform vec3 uL2;
out vec4 outColor;
void main() {
  vec3 n = normalize(vN);
  float d1 = max(dot(n, uL1), 0.0);
  float d2 = max(dot(n, uL2), 0.0);
  float hemi = n.y * 0.5 + 0.5;
  float ao = mix(0.60, 1.0, smoothstep(0.0, 1.0, vAO));
  vec3 amb = mix(vec3(0.62, 0.68, 0.82), vec3(0.88, 0.90, 0.96), hemi);
  vec3 lit = amb * 0.80 + vec3(1.0, 0.97, 0.90) * d1 * 0.52 + vec3(0.72, 0.82, 1.0) * d2 * 0.14;
  outColor = vec4(min(vCol * lit * ao * 1.10, vec3(1.0)), 1.0);
}"""

private const val GND_V =
    """#version 300 es
precision highp float;
layout(location=0) in vec2 aXZ;
uniform mat4 uMVP;
out vec2 vXZ;
void main() { vXZ = aXZ; gl_Position = uMVP * vec4(aXZ.x, 0.04, aXZ.y, 1.0); }"""

private const val GND_F =
    """#version 300 es
precision highp float;
in vec2 vXZ;
uniform vec4 uBlob;
uniform float uStr;
uniform vec4 uShadow;
out vec4 outColor;
void main() {
  vec2 d = (vXZ - uBlob.xy) / uBlob.zw;
  float a = smoothstep(1.0, 0.15, length(d)) * uStr * uShadow.a;
  outColor = vec4(uShadow.rgb * a, a);
}"""
