import MetalKit
import SwiftUI
import simd

/// The mascot on About: a budgie of voxels, as the desktop draws it
/// (ui/mascot.js, which this follows function by function; its WebGL 2 is
/// Metal here, and the shaders are at the foot of this file). Its model is mascot.json,
/// the desktop's mascot-model.js without its wrapper (scripts/mascot.py
/// writes both). The first time it is shown it turns once; then it turns
/// and tilts under a finger, spins on when flung and turns back home 4.5 s
/// after it is let go, hops and talks at a tap, and glances toward a
/// pointer (an iPad's). It draws only while something moves. Under Reduce
/// Motion it neither turns by itself nor hops. Until its voxels are meshed,
/// and where there is no Metal, the picture of it at rest
/// (android/mascot.js), which a frame at rest is drawn over.
struct Mascot: View {
    @State private var scene = MascotScene.cached

    var body: some View {
        ZStack {
            Image("Mascot").resizable().scaledToFit()
            if let scene { Voxels(scene: scene) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Pk.surface)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L("about_mascot"))
        .accessibilityAddTraits(.isImage)
        .task { if scene == nil { scene = await MascotScene.load() } }
    }

    private struct Voxels: UIViewRepresentable {
        let scene: MascotScene

        func makeUIView(context: Context) -> UIView {
            if let stage = MascotStage(scene) { return stage }
            // no Metal: nothing over the picture
            let none = UIView()
            none.isUserInteractionEnabled = false
            return none
        }

        func updateUIView(_ view: UIView, context: Context) {}
    }
}

/// mascot.json: each chunk's cells as runs of palette indices.
struct MascotModel: Decodable, Sendable {
    struct Chunk: Decodable, Sendable {
        /// its cells each way, and where its first is
        let d: [Int]
        let o: [Float]
        let runs: [Int]

        /// The cells, x slowest and z fastest, from the runs:
        /// [count, colour, count, colour, ...]
        var cells: [UInt8] {
            var out = [UInt8]()
            out.reserveCapacity(d[0] * d[1] * d[2])
            for i in stride(from: 0, to: runs.count - 1, by: 2) {
                out.append(contentsOf: repeatElement(UInt8(runs[i + 1]), count: runs[i]))
            }
            return out
        }
    }
    /// The bubble's waveform: each bar's first x and their width, the
    /// middle they stand about, their depth, colours and heights at rest.
    struct Bars: Decodable, Sendable {
        let x0: [Int]
        let w: Int
        let yc: Float
        let z0: Int
        let z1: Int
        let colors: [UInt8]
        let base: [Int]
    }
    let palette: [[UInt8]]
    let chunks: [String: Chunk]
    let bars: Bars
    let pivot: [Float]
    let rxz: Float
    let ground: [Float]
}

/// A chunk's faces: per corner 20 bytes, its position (3 floats) and 8
/// bytes, the face, how open the corner is from 0 to 3 (ambient
/// occlusion), a jitter of the cell's shade, a spare, and its colour.
struct MascotMesh: Sendable {
    var vertices = Data()
    var indices = [UInt32]()
}

/// a face's normal and the two edges along it; its index is the shader's
private let faces: [(n: SIMD3<Int>, u: SIMD3<Int>, v: SIMD3<Int>)] = [
    (SIMD3(1, 0, 0), SIMD3(0, 1, 0), SIMD3(0, 0, 1)),
    (SIMD3(-1, 0, 0), SIMD3(0, 0, 1), SIMD3(0, 1, 0)),
    (SIMD3(0, 1, 0), SIMD3(0, 0, 1), SIMD3(1, 0, 0)),
    (SIMD3(0, -1, 0), SIMD3(1, 0, 0), SIMD3(0, 0, 1)),
    (SIMD3(0, 0, 1), SIMD3(1, 0, 0), SIMD3(0, 1, 0)),
    (SIMD3(0, 0, -1), SIMD3(0, 1, 0), SIMD3(1, 0, 0)),
]

/// Every face of a filled cell with an empty one beside it, as a quad
/// (mascot.js, meshGrid): hidden faces culled, corners shaded.
func mascotMesh(_ cells: [UInt8], _ d: [Int], _ o: [Float], _ palette: [[UInt8]]) -> MascotMesh {
    let (dx, dy, dz) = (d[0], d[1], d[2])
    let (sx, sy) = (dy * dz, dz)
    return cells.withUnsafeBufferPointer { g in
        func solid(_ p: SIMD3<Int>) -> Int {
            p.x < 0 || p.y < 0 || p.z < 0 || p.x >= dx || p.y >= dy || p.z >= dz
                ? 0 : g[p.x &* sx &+ p.y &* sy &+ p.z] != 0 ? 1 : 0
        }
        var count = 0
        for x in 0..<dx {
            for y in 0..<dy {
                for z in 0..<dz where g[x &* sx &+ y &* sy &+ z] != 0 {
                    for f in faces where solid(SIMD3(x, y, z) &+ f.n) == 0 { count += 1 }
                }
            }
        }
        var m = MascotMesh()
        m.vertices = Data(count: count * 4 * 20)
        m.indices.reserveCapacity(count * 6)
        m.vertices.withUnsafeMutableBytes { raw in
            var vi = 0
            for x in 0..<dx {
                for y in 0..<dy {
                    for z in 0..<dz {
                        let val = g[x &* sx &+ y &* sy &+ z]
                        if val == 0 { continue }
                        let rgb = palette[Int(val)]
                        let jit = UInt8(
                            truncatingIfNeeded: (UInt32(x) &* 73_856_093)
                                ^ (UInt32(y) &* 19_349_663) ^ (UInt32(z) &* 83_492_791))
                        let at = SIMD3(x, y, z)
                        for (f, face) in faces.enumerated() {
                            let n = at &+ face.n
                            if solid(n) != 0 { continue }
                            // the face's first corner: the cell's, a step on
                            // where the face looks the way its axis grows
                            let b = at &+ face.n.replacing(with: 0, where: face.n .< 0)
                            var ao = [0, 0, 0, 0]
                            for k in 0..<4 {
                                let du = k == 1 || k == 2 ? 1 : 0
                                let dv = k == 2 || k == 3 ? 1 : 0
                                let su = du == 1 ? 1 : -1
                                let sv = dv == 1 ? 1 : -1
                                let s1 = solid(n &+ face.u &* su)
                                let s2 = solid(n &+ face.v &* sv)
                                let c = solid(n &+ face.u &* su &+ face.v &* sv)
                                ao[k] = s1 == 1 && s2 == 1 ? 0 : 3 - (s1 + s2 + c)
                                let p = b &+ face.u &* du &+ face.v &* dv
                                let at = (vi + k) * 20
                                raw.storeBytes(
                                    of: o[0] + Float(p.x), toByteOffset: at, as: Float.self)
                                raw.storeBytes(
                                    of: o[1] + Float(p.y), toByteOffset: at + 4, as: Float.self)
                                raw.storeBytes(
                                    of: o[2] + Float(p.z), toByteOffset: at + 8, as: Float.self)
                                raw[at + 12] = UInt8(f)
                                raw[at + 13] = UInt8(ao[k])
                                raw[at + 14] = jit
                                raw[at + 16] = rgb[0]
                                raw[at + 17] = rgb[1]
                                raw[at + 18] = rgb[2]
                            }
                            // the quad cut along the diagonal between its darker
                            // corners, as voxel renderers do, so that the shading
                            // runs evenly across it
                            let q =
                                ao[0] + ao[2] > ao[1] + ao[3]
                                ? [1, 2, 3, 1, 3, 0] : [0, 1, 2, 0, 2, 3]
                            for k in q { m.indices.append(UInt32(vi + k)) }
                            vi += 4
                        }
                    }
                }
            }
        }
        return m
    }
}

/// The model and what of it never changes, meshed: the bird and its props.
/// The bubble's cells are meshed with each set of its bars' heights.
struct MascotScene: Sendable {
    let model: MascotModel
    let bird: MascotMesh
    let props: MascotMesh
    let bubble: [UInt8]

    /// meshed once a run, off the main thread
    @MainActor static var cached: MascotScene?

    @MainActor static func load() async -> MascotScene? {
        if let cached { return cached }
        let made = await Task.detached(priority: .userInitiated) { () -> MascotScene? in
            guard let url = Bundle.main.url(forResource: "mascot", withExtension: "json"),
                let data = try? Data(contentsOf: url),
                let m = try? JSONDecoder().decode(MascotModel.self, from: data),
                let bird = m.chunks["bird"], let props = m.chunks["props"],
                let bubble = m.chunks["bubble"]
            else { return nil }
            return MascotScene(
                model: m, bird: mascotMesh(bird.cells, bird.d, bird.o, m.palette),
                props: mascotMesh(props.cells, props.d, props.o, m.palette), bubble: bubble.cells)
        }
        .value
        cached = made
        return made
    }
}

/// What the voxels' shaders are given (shaders, VoxU).
private struct VoxU {
    var mvp: simd_float4x4
    var mv: simd_float4x4
    var off: SIMD4<Float>
    var l1: SIMD4<Float>
    var l2: SIMD4<Float>
}

/// And the shadow's (GndU).
private struct GndU {
    var mvp: simd_float4x4
    var blob: SIMD4<Float>
    var shadow: SIMD4<Float>
    var str: SIMD4<Float>
}

private func rad(_ d: Double) -> Double { d * .pi / 180 }
private func clamp(_ v: Double, _ a: Double, _ b: Double) -> Double { min(b, max(a, v)) }

/// The stage: a Metal view that draws while something moves, paused
/// otherwise, and the fingers on it.
final class MascotStage: MTKView, MTKViewDelegate {
    /// one turn the first time it is shown in a run, as the desktop's
    private static var introDone = false

    private let scene: MascotScene
    private let queue: MTLCommandQueue
    private let voxels: MTLRenderPipelineState
    private let ground: MTLRenderPipelineState
    private let depth: MTLDepthStencilState
    private let flat: MTLDepthStencilState
    private typealias Buffers = (vertices: MTLBuffer, indices: MTLBuffer, count: Int)
    private var bird: Buffers?
    private var props: Buffers?
    private var bubble: Buffers?
    private var bars: [Int] = []

    private let homePitch = rad(7)
    private let pitchMin = rad(-6)
    private let pitchMax = rad(48)
    private let fov = rad(24)

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
    private var lastMove: Double?
    /// the turn home's wait: a timer, not frames, since nothing moves in it
    private var home: DispatchWorkItem?

    private var calm: Bool { UIAccessibility.isReduceMotionEnabled }
    private var now: Double { CACurrentMediaTime() * 1000 }

    init?(_ scene: MascotScene) {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue(),
            let library = try? device.makeLibrary(source: shaders, options: nil)
        else { return nil }
        self.scene = scene
        self.queue = queue

        let layout = MTLVertexDescriptor()
        layout.attributes[0].format = .float3
        layout.attributes[1].format = .uchar4Normalized
        layout.attributes[1].offset = 12
        layout.attributes[2].format = .uchar4Normalized
        layout.attributes[2].offset = 16
        layout.layouts[0].stride = 20
        // the desktop's canvas: antialiased, and its bytes the palette's own,
        // not linear light's, so that the colours are the picture's
        func pipeline(_ vertex: String, _ fragment: String, blend: Bool) -> MTLRenderPipelineState?
        {
            let p = MTLRenderPipelineDescriptor()
            p.vertexFunction = library.makeFunction(name: vertex)
            p.fragmentFunction = library.makeFunction(name: fragment)
            p.rasterSampleCount = 4
            p.depthAttachmentPixelFormat = .depth32Float
            let c = p.colorAttachments[0]!
            c.pixelFormat = .bgra8Unorm
            if blend {
                c.isBlendingEnabled = true
                c.sourceRGBBlendFactor = .one
                c.sourceAlphaBlendFactor = .one
                c.destinationRGBBlendFactor = .oneMinusSourceAlpha
                c.destinationAlphaBlendFactor = .oneMinusSourceAlpha
            } else {
                p.vertexDescriptor = layout
            }
            return try? device.makeRenderPipelineState(descriptor: p)
        }
        let tested = MTLDepthStencilDescriptor()
        tested.depthCompareFunction = .less
        tested.isDepthWriteEnabled = true
        guard let voxels = pipeline("mascot_vox_v", "mascot_vox_f", blend: false),
            let ground = pipeline("mascot_gnd_v", "mascot_gnd_f", blend: true),
            let depth = device.makeDepthStencilState(descriptor: tested),
            let flat = device.makeDepthStencilState(descriptor: MTLDepthStencilDescriptor())
        else { return nil }
        self.voxels = voxels
        self.ground = ground
        self.depth = depth
        self.flat = flat
        super.init(frame: .zero, device: device)

        colorPixelFormat = .bgra8Unorm
        depthStencilPixelFormat = .depth32Float
        sampleCount = 4
        isPaused = true
        enableSetNeedsDisplay = false
        delegate = self
        bird = buffers(scene.bird)
        props = buffers(scene.props)
        setBars(scene.model.bars.base)
        surface()
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (self: Self, _) in
            self.surface()
            self.wake()
        }

        // the stage keeps a drag, as a map does in a page that scrolls
        addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(panned)))
        addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped)))
        addGestureRecognizer(UIHoverGestureRecognizer(target: self, action: #selector(hovered)))
    }

    required init(coder: NSCoder) { fatalError() }

    private func buffers(_ m: MascotMesh) -> Buffers? {
        guard let device, !m.indices.isEmpty,
            let v = m.vertices.withUnsafeBytes({
                device.makeBuffer(bytes: $0.baseAddress!, length: $0.count)
            }),
            let i = device.makeBuffer(bytes: m.indices, length: m.indices.count * 4)
        else { return nil }
        return (v, i, m.indices.count)
    }

    /// The waveform's bars, stamped into the bubble's cells and meshed again
    /// for each new set of heights.
    private func setBars(_ heights: [Int]) {
        if heights == bars { return }
        bars = heights
        let m = scene.model
        guard let c = m.chunks["bubble"] else { return }
        var g = scene.bubble
        let (sx, sy) = (c.d[1] * c.d[2], c.d[2])
        let (ox, oy, oz) = (Int(c.o[0]), Int(c.o[1]), Int(c.o[2]))
        for (i, x0) in m.bars.x0.enumerated() {
            let h = heights[i]
            // rounded as the desktop's Math.round, a half upward
            let y0 = Int((m.bars.yc - Float(h) / 2 + 0.5).rounded(.down)) - oy
            for x in x0..<x0 + m.bars.w {
                for y in y0..<y0 + h {
                    for z in m.bars.z0..<m.bars.z1 {
                        g[(x - ox) * sx + y * sy + (z - oz)] = m.bars.colors[i]
                    }
                }
            }
        }
        bubble = buffers(mascotMesh(g, c.d, c.o, m.palette))
    }

    /// Cleared to the surface under it, the theme's.
    private func surface() {
        var (r, g, b, a) = (CGFloat(0), CGFloat(0), CGFloat(0), CGFloat(0))
        UIColor(Pk.surface).resolvedColor(with: traitCollection).getRed(
            &r, green: &g, blue: &b, alpha: &a)
        clearColor = MTLClearColor(red: r, green: g, blue: b, alpha: 1)
    }

    // ------------------------- the loop, which runs while something moves --

    private func wake() {
        guard window != nil, isPaused else { return }
        last = now
        isPaused = false
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        guard window != nil else {
            // a turn under way is not begun again
            if introStart >= 0 { Self.introDone = true }
            isPaused = true
            return
        }
        wake()
        if !Self.introDone && !calm && introStart < 0 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self] in
                guard let self, self.window != nil, !Self.introDone, self.introStart < 0 else {
                    return
                }
                self.introStart = self.now
                self.wake()
            }
        }
        #if DEBUG
            // -chirp SECONDS: a tap that long after it is shown, with -show about
            let args = CommandLine.arguments
            if let i = args.firstIndex(of: "-chirp"), i + 1 < args.count,
                let s = Double(args[i + 1])
            {
                DispatchQueue.main.asyncAfter(deadline: .now() + s) { [weak self] in self?.chirp() }
            }
        #endif
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) { wake() }

    func draw(in view: MTKView) {
        let now = self.now
        let busy = step(now)
        render(now)
        if !busy { isPaused = true }
    }

    /// What moves, moved to now; whether there is more to come.
    private func step(_ now: Double) -> Bool {
        let dt = min(0.05, (now - last) / 1000)
        last = now
        var busy = false
        let turning = !Self.introDone && introStart >= 0

        // one turn, the first time it is shown
        if turning {
            let p = (now - introStart) / 3800
            if p >= 1 {
                intro = 0
                Self.introDone = true
                talkUntil = max(talkUntil, now + 250)
            } else {
                intro = .pi * 2 * (p < 0.5 ? 4 * p * p * p : 1 - pow(-2 * p + 2, 3) / 2)
                busy = true
            }
        }

        // the glance toward the pointer
        let gk = 1 - exp(-dt * 5)
        gx += (tgx - gx) * gk
        gy += (tgy - gy) * gk
        if abs(tgx - gx) > 1e-4 || abs(tgy - gy) > 1e-4 { busy = true }

        // a drag's spin running down, then an unhurried turn back home
        if !dragging {
            if abs(vel) > 0.02 {
                yawDrag += vel * dt
                vel *= exp(-dt * 3.2)
                busy = true
            } else {
                vel = 0
            }
            let turned = yawDrag != 0 || pitchDrag != 0
            if now - lastInteract > 4500 && turned {
                let home = (yawDrag / (.pi * 2)).rounded() * .pi * 2
                let rk = 1 - exp(-dt * 2.2)
                yawDrag += (home - yawDrag) * rk
                pitchDrag += (0 - pitchDrag) * rk
                if abs(home - yawDrag) < 5e-4 && abs(pitchDrag) < 5e-4 {
                    yawDrag = 0
                    pitchDrag = 0
                }
                busy = true
            } else if turned {
                // until the turn home, the same frame each frame: none
                // drawn, and the loop woken when it is due
                home?.cancel()
                let due = DispatchWorkItem { [weak self] in self?.wake() }
                home = due
                DispatchQueue.main.asyncAfter(
                    deadline: .now() + (lastInteract + 4500 - now + 1) / 1000, execute: due)
            }
        }

        // the bubble's waveform talks during the turn and after a chirp
        let rest = scene.model.bars.base
        if (!Self.introDone && introStart >= 0) || now < talkUntil {
            let t = (now / 85).rounded(.down) * 85 / 1000
            setBars(
                rest.enumerated().map { i, b in
                    let i = Double(i)
                    let k = 0.5 + 0.5 * sin(t * 9 + i * 1.7) * cos(t * 5.3 + i * 0.9)
                    return max(2, Int((Double(b) * (0.35 + 0.95 * k)).rounded()))
                })
            busy = true
        } else {
            setBars(rest)
        }

        if hopStart >= 0 { busy = true }
        return busy
    }

    // --------------------------------------------------------------- drawing --

    private func render(_ now: Double) {
        let size = drawableSize
        guard size.width > 0, size.height > 0, let pass = currentRenderPassDescriptor,
            let drawable = currentDrawable, let commands = queue.makeCommandBuffer(),
            let enc = commands.makeRenderCommandEncoder(descriptor: pass)
        else { return }
        let m = scene.model
        let aspect = Float(size.width / size.height)

        // far enough back that the whole scene fits whichever way it turns
        let halfH: Float = 70 * 1.08
        let halfW = m.rxz * 1.07
        let dist = max(halfH, halfW / aspect) / tan(Float(fov) / 2)
        let yaw = Float(intro + yawDrag + gx)
        let pitch = Float(clamp(homePitch + pitchDrag + gy, pitchMin, pitchMax))
        let mv =
            translate(0, -2, -dist) * rotateX(pitch) * rotateY(yaw)
            * translate(-m.pivot[0], -m.pivot[1], -m.pivot[2])
        let mvp = perspective(Float(fov), aspect, dist * 0.35, dist * 2.4) * mv

        var hop: Float = 0
        if hopStart >= 0 {
            let t = (now - hopStart) / 520
            if t >= 1 { hopStart = -1 } else { hop = Float(7 * sin(.pi * t) * (1 - 0.25 * t)) }
        }

        // the shadow first, without depth, then the voxels
        let dark = traitCollection.userInterfaceStyle == .dark
        var g = GndU(
            mvp: mvp, blob: SIMD4(m.ground[0], m.ground[1], m.ground[2], m.ground[3]),
            shadow: dark ? SIMD4(0.01, 0.03, 0.09, 0.55) : SIMD4(0.22, 0.36, 0.64, 0.36),
            // lighter while the bird is in the air
            str: SIMD4(1 - hop / 16, 0, 0, 0))
        enc.setRenderPipelineState(ground)
        enc.setDepthStencilState(flat)
        enc.setVertexBytes(&g, length: MemoryLayout<GndU>.stride, index: 1)
        enc.setFragmentBytes(&g, length: MemoryLayout<GndU>.stride, index: 1)
        enc.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: 4)

        enc.setRenderPipelineState(voxels)
        enc.setDepthStencilState(depth)
        enc.setFrontFacing(.counterClockwise)
        enc.setCullMode(.back)
        func draw(_ mesh: Buffers?, _ oy: Float, _ jit: Float) {
            guard let mesh else { return }
            var u = VoxU(
                mvp: mvp, mv: mv, off: SIMD4(0, oy, 0, jit),
                l1: SIMD4(simd_normalize(SIMD3<Float>(-0.5, 0.72, 0.62)), 0),
                l2: SIMD4(simd_normalize(SIMD3<Float>(0.78, 0.12, 0.3)), 0))
            enc.setVertexBuffer(mesh.vertices, offset: 0, index: 0)
            enc.setVertexBytes(&u, length: MemoryLayout<VoxU>.stride, index: 1)
            enc.setFragmentBytes(&u, length: MemoryLayout<VoxU>.stride, index: 1)
            enc.drawIndexedPrimitives(
                type: .triangle, indexCount: mesh.count, indexType: .uint32,
                indexBuffer: mesh.indices, indexBufferOffset: 0)
        }
        draw(props, 0, 0.05)
        draw(bubble, 0, 0.025)
        draw(bird, hop, 0.11)
        enc.endEncoding()
        commands.present(drawable)
        commands.commit()
    }

    private func translate(_ x: Float, _ y: Float, _ z: Float) -> simd_float4x4 {
        var m = matrix_identity_float4x4
        m.columns.3 = SIMD4(x, y, z, 1)
        return m
    }

    private func rotateX(_ a: Float) -> simd_float4x4 {
        let (c, s) = (cos(a), sin(a))
        return simd_float4x4(
            SIMD4(1, 0, 0, 0), SIMD4(0, c, s, 0), SIMD4(0, -s, c, 0), SIMD4(0, 0, 0, 1))
    }

    private func rotateY(_ a: Float) -> simd_float4x4 {
        let (c, s) = (cos(a), sin(a))
        return simd_float4x4(
            SIMD4(c, 0, -s, 0), SIMD4(0, 1, 0, 0), SIMD4(s, 0, c, 0), SIMD4(0, 0, 0, 1))
    }

    /// The desktop's, with its depth from -1 to 1 brought to Metal's 0 to 1.
    private func perspective(_ fovy: Float, _ aspect: Float, _ n: Float, _ f: Float)
        -> simd_float4x4
    {
        let t = 1 / tan(fovy / 2)
        let gl = simd_float4x4(
            SIMD4(t / aspect, 0, 0, 0), SIMD4(0, t, 0, 0), SIMD4(0, 0, (f + n) / (n - f), -1),
            SIMD4(0, 0, 2 * f * n / (n - f), 0))
        let depth = simd_float4x4(
            SIMD4(1, 0, 0, 0), SIMD4(0, 1, 0, 0), SIMD4(0, 0, 0.5, 0), SIMD4(0, 0, 0.5, 1))
        return depth * gl
    }

    // ----------------------------------------------------------------- input --

    private func dragPitch(_ v: Double) -> Double {
        clamp(v, pitchMin - homePitch, pitchMax - homePitch)
    }

    /// a finger down stops what it was doing, as the desktop's pointer does
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesBegan(touches, with: event)
        vel = 0
        Self.introDone = true
        intro = 0
        lastInteract = now
        wake()
    }

    @objc private func panned(_ pan: UIPanGestureRecognizer) {
        let now = self.now
        switch pan.state {
        case .began:
            dragging = true
            vel = 0
            lastMove = nil
        case .changed:
            let by = pan.translation(in: self)
            pan.setTranslation(.zero, in: self)
            let dyaw = by.x * 0.0105
            yawDrag += dyaw
            pitchDrag = dragPitch(pitchDrag + by.y * 0.006)
            vel = vel * 0.6 + dyaw / max(0.008, (now - (lastMove ?? now - 16)) / 1000) * 0.4
            lastMove = now
        case .ended:
            dragging = false
            vel = clamp(vel, -6, 6)
        default:
            dragging = false
        }
        lastInteract = now
        wake()
    }

    @objc private func tapped() {
        lastInteract = now
        chirp()
    }

    /// a hop, and the bubble talks
    private func chirp() {
        if calm { return }
        hopStart = now
        talkUntil = now + 1400
        wake()
    }

    /// a glance toward a pointer, by small angles
    @objc private func hovered(_ hover: UIHoverGestureRecognizer) {
        guard let window, hover.state == .began || hover.state == .changed, !dragging, !calm
        else {
            tgx = 0
            tgy = 0
            wake()
            return
        }
        let at = hover.location(in: window)
        let mid = convert(CGPoint(x: bounds.midX, y: bounds.midY), to: window)
        tgx = clamp((at.x - mid.x) / (window.bounds.width / 2), -1, 1) * rad(20)
        tgy = clamp((at.y - mid.y) / (window.bounds.height / 2), -1, 1) * rad(7)
        wake()
    }
}

/// The shaders, as the desktop's are (ui/mascot.js): the voxels under a key
/// light from the upper left, a weak fill from the right and a sky over a
/// ground for the ambient, and the shadow on the ground, a soft blob under
/// the bird on one quad. Compiled when the stage is made, from this text, so
/// that building the app asks for no Metal toolchain beside Xcode.
private let shaders = """
    #include <metal_stdlib>
    using namespace metal;

    struct VoxIn {
        float3 pos [[attribute(0)]];
        // the face, how open the corner is (0 to 3), the cell's jitter; bytes
        float4 meta [[attribute(1)]];
        float4 col [[attribute(2)]];
    };

    struct VoxU {
        float4x4 mvp;
        float4x4 mv;
        // a chunk's offset, and in w how far its cells' shades are jittered
        float4 off;
        float4 l1;
        float4 l2;
    };

    struct VoxV {
        float4 position [[position]];
        float3 col;
        float ao;
        float3 n;
    };

    vertex VoxV mascot_vox_v(VoxIn in [[stage_in]], constant VoxU &u [[buffer(1)]]) {
        const float3 N[6] = {float3(1, 0, 0),  float3(-1, 0, 0), float3(0, 1, 0),
                             float3(0, -1, 0), float3(0, 0, 1),  float3(0, 0, -1)};
        VoxV out;
        out.position = u.mvp * float4(in.pos + u.off.xyz, 1.0);
        int f = int(in.meta.x * 255.0 + 0.5);
        out.n = float3x3(u.mv[0].xyz, u.mv[1].xyz, u.mv[2].xyz) * N[f];
        out.ao = in.meta.y * 255.0 / 3.0;
        out.col = in.col.rgb * (1.0 + (in.meta.z - 0.5) * u.off.w);
        return out;
    }

    fragment float4 mascot_vox_f(VoxV in [[stage_in]], constant VoxU &u [[buffer(1)]]) {
        float3 n = normalize(in.n);
        float d1 = max(dot(n, u.l1.xyz), 0.0);
        float d2 = max(dot(n, u.l2.xyz), 0.0);
        float hemi = n.y * 0.5 + 0.5;
        float ao = mix(0.60, 1.0, smoothstep(0.0, 1.0, in.ao));
        float3 amb = mix(float3(0.62, 0.68, 0.82), float3(0.88, 0.90, 0.96), hemi);
        float3 lit = amb * 0.80 + float3(1.0, 0.97, 0.90) * d1 * 0.52 +
                     float3(0.72, 0.82, 1.0) * d2 * 0.14;
        return float4(min(in.col * lit * ao * 1.10, float3(1.0)), 1.0);
    }

    struct GndU {
        float4x4 mvp;
        // the blob's middle and its radii, on the ground
        float4 blob;
        float4 shadow;
        // in x, how strong: lighter while the bird is in the air
        float4 str;
    };

    struct GndV {
        float4 position [[position]];
        float2 xz;
    };

    vertex GndV mascot_gnd_v(uint i [[vertex_id]], constant GndU &u [[buffer(1)]]) {
        const float2 Q[4] = {float2(-90, -70), float2(110, -70), float2(-90, 70), float2(110, 70)};
        GndV out;
        out.xz = Q[i];
        out.position = u.mvp * float4(Q[i].x, 0.04, Q[i].y, 1.0);
        return out;
    }

    fragment float4 mascot_gnd_f(GndV in [[stage_in]], constant GndU &u [[buffer(1)]]) {
        float2 d = (in.xz - u.blob.xy) / u.blob.zw;
        float a = smoothstep(1.0, 0.15, length(d)) * u.str.x * u.shadow.a;
        return float4(u.shadow.rgb * a, a);
    }
    """
