import SwiftUI

/// The desktop's icons (ui/app.js, ICON), as Android draws them
/// (android/.../ui/Icons.kt): the same SVG paths on the same 24-unit grid,
/// lines 2 wide with round ends in their button's color, play and pause
/// filled.
struct PkIcon: View {
    let paths: [String]
    var filled = false

    var body: some View {
        GeometryReader { g in
            let scale = min(g.size.width, g.size.height) / 24
            let shape = SVGPath(d: paths, scale: scale)
            if filled {
                shape.fill()
            } else {
                shape.stroke(
                    style: StrokeStyle(lineWidth: 2 * scale, lineCap: .round, lineJoin: .round))
            }
        }
        .accessibilityHidden(true)
    }

    static let back = PkIcon(paths: ["M19 12H5", "M12 19L5 12L12 5"])
    static let copy = PkIcon(paths: [
        "M11 9h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2v-9a2 2 0 0 1 2-2z",
        "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1",
    ])
    static let down = PkIcon(paths: ["M6 9L12 15L18 9"])
    static let up = PkIcon(paths: ["M18 15L12 9L6 15"])
    static let search = PkIcon(paths: [
        "M11 3a8 8 0 1 0 0 16a8 8 0 1 0 0-16z", "M21 21L16.65 16.65",
    ])
    static let check = PkIcon(paths: ["M20 6L9 17L4 12"])
    static let download = PkIcon(paths: [
        "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M7 10L12 15L17 10", "M12 15V3",
    ])
    static let alert = PkIcon(paths: [
        "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z", "M12 8v4", "M12 16h0.01",
    ])
    static let x = PkIcon(paths: ["M18 6L6 18", "M6 6L18 18"])
    static let file = PkIcon(paths: [
        "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z", "M14 2L14 8L20 8",
        "M16 13H8", "M16 17H8",
    ])
    static let external = PkIcon(paths: [
        "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6", "M15 3h6v6", "M10 14L21 3",
    ])
    static let play = PkIcon(paths: ["M6 3L20 12L6 21Z"], filled: true)
    static let pause = PkIcon(paths: ["M6 4h4v16H6z", "M14 4h4v16h-4z"], filled: true)
    private static let speaker = "M11 5L6 9H2v6h4l5 4z"
    static let vol2 = PkIcon(paths: [
        speaker, "M19.07 4.93a10 10 0 0 1 0 14.14", "M15.54 8.46a5 5 0 0 1 0 7.07",
    ])
    static let vol1 = PkIcon(paths: [speaker, "M15.54 8.46a5 5 0 0 1 0 7.07"])
    static let volX = PkIcon(paths: [speaker, "M23 9L17 15", "M17 9L23 15"])
}

/// SVG path data, the commands the icons use (M L H V A Z, absolute and
/// relative), as a Shape scaled from the 24-unit grid. An arc, all of them
/// circles here, goes as cubic curves of at most a quarter turn each.
struct SVGPath: Shape {
    let d: [String]
    let scale: CGFloat

    func path(in rect: CGRect) -> Path {
        var p = Path()
        for s in d {
            add(s, to: &p)
        }
        return p.applying(CGAffineTransform(scaleX: scale, y: scale))
    }

    private func add(_ s: String, to p: inout Path) {
        // tokens: command letters and numbers ("0.01", "-2", "1-2" is 1 and -2)
        var tokens: [String] = []
        var cur = ""
        for ch in s {
            if ch.isLetter {
                if !cur.isEmpty {
                    tokens.append(cur)
                    cur = ""
                }
                tokens.append(String(ch))
            } else if ch == " " || ch == "," {
                if !cur.isEmpty {
                    tokens.append(cur)
                    cur = ""
                }
            } else if ch == "-" && !cur.isEmpty && cur.last != "e" {
                tokens.append(cur)
                cur = "-"
            } else if ch == "." && cur.contains(".") {
                tokens.append(cur)
                cur = "."
            } else {
                cur.append(ch)
            }
        }
        if !cur.isEmpty { tokens.append(cur) }

        var i = 0
        var cmd: Character = "M"
        var at = CGPoint.zero
        var start = CGPoint.zero
        func num() -> CGFloat {
            defer { i += 1 }
            return CGFloat(Double(tokens[i]) ?? 0)
        }
        while i < tokens.count {
            if let c = tokens[i].first, c.isLetter {
                cmd = c
                i += 1
                if c == "Z" || c == "z" {
                    p.closeSubpath()
                    at = start
                    continue
                }
            }
            let rel = cmd.isLowercase
            switch cmd.uppercased() {
            case "M":
                let x = num()
                let y = num()
                at = rel ? CGPoint(x: at.x + x, y: at.y + y) : CGPoint(x: x, y: y)
                p.move(to: at)
                start = at
                cmd = rel ? "l" : "L"  // further pairs are lines
            case "L":
                let x = num()
                let y = num()
                at = rel ? CGPoint(x: at.x + x, y: at.y + y) : CGPoint(x: x, y: y)
                p.addLine(to: at)
            case "H":
                let x = num()
                at.x = rel ? at.x + x : x
                p.addLine(to: at)
            case "V":
                let y = num()
                at.y = rel ? at.y + y : y
                p.addLine(to: at)
            case "A":
                let r = num()
                _ = num()  // ry: these arcs are circles
                _ = num()  // their rotation, none
                let large = num() != 0
                let sweep = num() != 0
                let x = num()
                let y = num()
                let end = rel ? CGPoint(x: at.x + x, y: at.y + y) : CGPoint(x: x, y: y)
                arc(&p, from: at, to: end, r: r, large: large, sweep: sweep)
                at = end
            default:
                i += 1
            }
        }
    }

    /// A circle's arc of radius r from a to b (SVG's endpoint form, its
    /// implementation notes F.6.5), as cubics
    private func arc(
        _ p: inout Path, from a: CGPoint, to b: CGPoint, r rIn: CGFloat, large: Bool, sweep: Bool
    ) {
        let dx = (a.x - b.x) / 2
        let dy = (a.y - b.y) / 2
        let d2 = dx * dx + dy * dy
        guard d2 > 0 else { return }
        let r = max(rIn, sqrt(d2))
        var k = sqrt(max(0, r * r - d2) / d2)
        if large == sweep { k = -k }
        let c = CGPoint(x: (a.x + b.x) / 2 + k * dy, y: (a.y + b.y) / 2 - k * dx)
        let t1 = atan2(a.y - c.y, a.x - c.x)
        var dt = atan2(b.y - c.y, b.x - c.x) - t1
        if sweep && dt < 0 { dt += 2 * .pi }
        if !sweep && dt > 0 { dt -= 2 * .pi }
        let n = max(1, Int(ceil(abs(dt) / (.pi / 2))))
        let step = dt / CGFloat(n)
        let h = 4.0 / 3.0 * tan(step / 4)
        var t = t1
        for _ in 0..<n {
            let p0 = CGPoint(x: c.x + r * cos(t), y: c.y + r * sin(t))
            let p3 = CGPoint(x: c.x + r * cos(t + step), y: c.y + r * sin(t + step))
            let c1 = CGPoint(x: p0.x - h * r * sin(t), y: p0.y + h * r * cos(t))
            let c2 = CGPoint(x: p3.x + h * r * sin(t + step), y: p3.y - h * r * cos(t + step))
            p.addCurve(to: p3, control1: c1, control2: c2)
            t += step
        }
    }
}
