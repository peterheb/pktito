import Foundation

// How pktito writes a time, as the desktop's window does (ui/app.js,
// formatting) and Android's (Times.kt): the rows' and the strip's in
// seconds and hundredths, or in frames, which the strip's time chooses; a
// length; a duration in words' place. In ASCII digits whatever the
// language, as the desktop's are.

private func two(_ n: Int64) -> String { String(format: "%02lld", n) }

/// hh:mm:ss, with the hours' digits that say something of the longest time
/// shown with it (len): none under an hour (10:40), one under ten
/// (1:05:00), so that a file's rows keep one width; a length on its own is
/// its own len. Without len, all of them (ui/app.js, hms).
func hms(_ s: Double, _ len: Double = 36000) -> String {
    let t = max(0, Int64(s))
    let h = t / 3600
    let ms = two(t / 60 % 60) + ":" + two(t % 60)
    let top = max(len, Double(t))
    return top >= 36000 ? two(h) + ":" + ms : top >= 3600 ? "\(h):" + ms : ms
}

/// hh:mm:ss.cc, a row's time and the strip's, the hours as hms has them
/// (ui/app.js, stamp).
func stamp(_ s: Double, _ len: Double = 36000) -> String {
    let cs = Int64((max(0, s) * 100).rounded())
    return hms(Double(cs / 100), len) + "." + two(cs % 100)
}

/// m:ss, or h:mm:ss from an hour (ui/app.js, clock).
func clock(_ s: Double) -> String {
    let t = max(0, Int64(s.rounded()))
    let h = t / 3600, m = t / 60 % 60
    return h > 0 ? "\(h):\(two(m)):\(two(t % 60))" : "\(m):\(two(t % 60))"
}

/// The rates the time menu offers, the desktop's (app.js, RATES).
let rates: [Double] = [23.976, 24, 25, 29.97, 30, 50, 59.94, 60]

/// A frame rate counted exactly: NTSC's 23.976, 29.97 and 59.94 are 24, 30
/// and 60 frames in 1.001 seconds, whatever their rounding says (app.js,
/// exactRate).
func exactRate(_ r: Double) -> Double {
    for n in [24.0, 30, 48, 60, 120] where abs(r - n * 1000 / 1001) < 0.01 {
        return n * 1000 / 1001
    }
    return abs(r - r.rounded()) < 0.01 ? r.rounded() : r
}

/// t as SMPTE timecode at a rate (app.js, timecode): hh:mm:ss:ff, counting
/// the frames of its nominal rate (24 for 23.976, 30 for 29.97), the frame
/// t falls in; at 29.97 and 59.94 drop-frame, hh:mm:ss;ff.
func timecode(_ t: Double, _ rate: Double) -> String {
    let r = exactRate(rate)
    let nominal = Int64(r.rounded())
    let drop = abs(r - 30000.0 / 1001) < 1e-9 || abs(r - 60000.0 / 1001) < 1e-9
    var frames = Int64((max(0, t) * r + 1e-6).rounded(.down))
    if drop {
        let d: Int64 = nominal == 60 ? 4 : 2
        let minute = nominal * 60 - d
        let tens = minute * 10 + d
        let rem = frames % tens
        frames += 9 * d * (frames / tens) + (rem >= d ? d * ((rem - d) / minute) : 0)
    }
    let s = frames / nominal
    return two(s / 3600) + ":" + two(s / 60 % 60) + ":" + two(s % 60) + (drop ? ";" : ":") + two(frames % nominal)
}

/// A rate as the time menu writes it: 23.976, 24, 29.97 (app.js, rateLabel).
func rateLabel(_ r: Double) -> String {
    let x = (exactRate(r) * 1000).rounded() / 1000
    return x == x.rounded(.down) ? String(Int64(x)) : String(x)
}

/// How the strip and the rows write a time (app.js, tc and lengthOf):
/// seconds and hundredths, the hours as the file's duration has them, or
/// frames, whose timecode is always hh:mm:ss:ff; a video's frames at its
/// own rate unless the rate chosen is to count a video's too, an audio
/// file's at the rate chosen.
struct Timebase: Equatable {
    var frames = false
    var fps: Double = 24
    var videoRate = true
    var video: Double = 0
    var duration: Double = 0

    var own: Bool { video > 0 && videoRate }
    var rate: Double { own ? video : fps > 0 ? fps : 24 }
    func at(_ t: Double) -> String { frames ? timecode(t, rate) : stamp(t, duration) }
    func length() -> String { frames ? timecode(duration, rate) : hms(duration, duration) }
}
