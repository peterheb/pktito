import SwiftUI

/// A speed as the desktop writes it: 0.75×, 1×, 1.5× (app.js, speedMenu).
func rateWord(_ s: Double) -> String {
    (s == s.rounded(.down) ? String(Int(s)) : String(s)) + "×"
}

/// The strip's text, all of it term-ink (app.css, .pk-tstamp).
private let tstamp = Font.custom(PkType.stamp.face, fixedSize: 13)

/// The file's strip, on term in both themes (app.css, .pk-strip; Android's
/// Strip.kt): play, the speed under it on the times' line, or at work the
/// progress in play's place and size; the waveform the rest of the width;
/// under it a search's ticks, then the time, which chooses how times are
/// written, and the length. At work the waveform fills as the transcript
/// does, and the time's place says the step or the pace. A transcript whose
/// recording doesn't play has no play, and why in the times' place.
///
/// A touch screen's strip, as Android's for a finger: play 48 across, no
/// speaker or slider, as the volume buttons set the level, and a drag on the
/// waveform moves the playhead with the thumb, seeking where it lets go.
struct Strip: View {
    let review: Bool
    /// at work, the time's place: the step, or the pace
    let left: String
    var why: String?
    var locate: (() -> Void)?
    let hits: [Hit]
    let tickAt: Int
    let onTick: (Int) -> Void
    let seek: (Double) -> Void
    @State private var scrub: Double?

    var body: some View {
        let t = Transcriber.shared
        let player = Player.shared
        let silent = review && why != nil
        let duration = t.duration
        let ticks = review && duration > 0 ? hits.map { $0.time / duration } : []
        let wave: CGFloat = ticks.isEmpty ? 64 : 56
        let gap: CGFloat = ticks.isEmpty ? 3 : 7
        let at = scrub ?? (review ? (duration > 0 ? player.position / duration : 0) : t.done)
        HStack(alignment: .top, spacing: Space.space4) {
            if review && !silent {
                VStack {
                    Button { player.toggle() } label: {
                        (player.playing ? PkIcon.pause : PkIcon.play)
                            .frame(width: 24, height: 24)
                            .foregroundStyle(Pk.termInk)
                            .frame(width: 48, height: 48)
                            .background(Pk.termLine, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
                    }
                    .buttonStyle(.plain)
                    .disabled(!player.ready)
                    .opacity(player.ready ? 1 : 0.45)
                    .accessibilityLabel(L(player.playing ? "pause" : "play"))
                    Spacer(minLength: 0)
                    SpeedMenu().disabled(!player.ready).opacity(player.ready ? 1 : 0.45)
                }
                .padding(.top, (wave - 48) / 2)
            } else if !review {
                Ring(done: t.done).padding(.top, (wave - 48) / 2)
            }
            VStack(spacing: gap) {
                GeometryReader { g in
                    let x = { (v: DragGesture.Value) in min(max(v.location.x / max(g.size.width, 1), 0), 1) }
                    Waveform(peaks: t.peaks, at: at, height: wave)
                        .opacity(review && !player.ready && !silent ? 0.45 : 1)
                        .contentShape(Rectangle())
                        .gesture(review ? DragGesture(minimumDistance: 0)
                            .onChanged { scrub = x($0) }
                            .onEnded { v in
                                scrub = nil
                                seek(x(v))
                            } : nil)
                }
                .frame(height: wave)
                Ticks(ticks: ticks, at: tickAt, onTick: onTick)
                    .frame(height: 18)
                if let why, silent {
                    HStack(spacing: 0) {
                        Text(why).lineLimit(2)
                        if let locate {
                            Text(" ·")
                            Button(L("play_locate"), action: locate).buttonStyle(.plain).padding(.horizontal, 6)
                        }
                        Spacer(minLength: 0)
                    }
                    .font(tstamp)
                    .foregroundStyle(Pk.termInk)
                } else {
                    HStack {
                        if review {
                            TimeMenu(text: AppState.shared.base.with(duration: duration, video: t.fps)
                                .at(scrub.map { $0 * duration } ?? player.position))
                        } else {
                            Text(left).font(tstamp).foregroundStyle(Pk.termInk).lineLimit(1)
                        }
                        Spacer(minLength: 12)
                        Text(duration > 0 ? AppState.shared.base.with(duration: duration, video: t.fps).length() : "")
                            .font(tstamp)
                            .monospacedDigit()
                            .foregroundStyle(Pk.termInk)
                            .lineLimit(1)
                    }
                }
            }
        }
        // as tall as its lanes, play's column too, whose speed is then on
        // the times' line (Android's IntrinsicSize.Min)
        .fixedSize(horizontal: false, vertical: true)
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Pk.term, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
    }
}

extension Timebase {
    func with(duration: Double, video: Double? = nil) -> Timebase {
        var b = self
        b.duration = duration
        if let video { b.video = video }
        return b
    }
}

/// The waveform the strip's width (ui/app.js, drawWave): bars 2 wide 1
/// apart, each the mean of the outline's slices under it, between the
/// outline's own 10th percentile and its 99.5th; the bars before the
/// playhead in the played color, the playhead the beak's, a point wider each
/// side and alone reaching the top. No outline yet: the empty strip's bars.
struct Waveform: View {
    let peaks: [UInt8]?
    let at: Double
    var height: CGFloat = 64

    var body: some View {
        let range = Waveform.range(peaks)
        Canvas { ctx, size in
            let bar: CGFloat = 2, gap: CGFloat = 1
            let n = max(1, Int((size.width + gap) / (bar + gap)))
            let top = size.height
            let k = min(max(Int(at * Double(n)), 0), n - 1)
            for i in 0..<n {
                var v = 0.0
                if let peaks, !peaks.isEmpty {
                    let a = i * peaks.count / n
                    let b = max(a + 1, (i + 1) * peaks.count / n)
                    var sum = 0.0
                    for j in a..<b { sum += Double(peaks[j]) }
                    v = min(max((sum / Double(b - a) - range.0) / (range.1 - range.0), 0), 1)
                }
                let x = CGFloat(i) * (bar + gap)
                if i == k {
                    ctx.fill(Path(CGRect(x: x - gap, y: 0, width: bar + 2 * gap, height: top)), with: .color(Pk.beak))
                    continue
                }
                let h = 3 + (v * Double(top - 7)).rounded()
                let c = peaks == nil ? Pk.termLine : i < k ? Pk.wavePlayed : Pk.wave
                ctx.fill(Path(CGRect(x: x, y: (top - h) / 2, width: bar, height: h)), with: .color(c))
            }
        }
        .frame(height: height)
        .accessibilityHidden(true)
    }

    private static func range(_ peaks: [UInt8]?) -> (Double, Double) {
        guard let peaks, !peaks.isEmpty else { return (0, 255) }
        let s = peaks.sorted()
        let lo = Double(s[Int(Double(s.count) * 0.1)])
        let hi = Double(s[min(s.count - 1, Int(Double(s.count) * 0.995))])
        return (lo, max(hi, lo + 16))
    }
}

/// A search's ticks in the lane under the waveform (app.css, .pk-ticks):
/// each match at its word's time, 2 wide in plume; the current one term-ink,
/// 3 wide and longer each way. A tap goes to the nearest tick within 16.
private struct Ticks: View {
    let ticks: [Double]
    let at: Int
    let onTick: (Int) -> Void

    var body: some View {
        GeometryReader { g in
            Canvas { ctx, size in
                for (k, t) in ticks.enumerated() where k != at {
                    ctx.fill(Path(CGRect(x: t * size.width - 1, y: 3, width: 2, height: size.height - 6)), with: .color(Pk.plume))
                }
                if at >= 0 && at < ticks.count {
                    ctx.fill(Path(CGRect(x: ticks[at] * size.width - 1.5, y: 0, width: 3, height: size.height)),
                             with: .color(Pk.termInk))
                }
            }
            .contentShape(Rectangle())
            .onTapGesture { p in
                guard let k = ticks.indices.min(by: { abs(ticks[$0] * g.size.width - p.x) < abs(ticks[$1] * g.size.width - p.x) }),
                      abs(ticks[k] * g.size.width - p.x) <= 16 else { return }
                onTick(k)
            }
        }
    }
}

/// The progress, in play's place and size: a ring that fills with the
/// waveform's played color from the top, and the percent inside it
/// (app.css, .pk-prog).
private struct Ring: View {
    let done: Double

    var body: some View {
        let d = min(max(done, 0), 1)
        ZStack {
            RoundedRectangle(cornerRadius: Radius.radiusUi).fill(Pk.termLine)
            RoundedRectangle(cornerRadius: Radius.radiusUi)
                .fill(AngularGradient(stops: [.init(color: Pk.wavePlayed, location: 0), .init(color: Pk.wavePlayed, location: d),
                                              .init(color: .clear, location: d), .init(color: .clear, location: 1)],
                                      center: .center, startAngle: .degrees(-90), endAngle: .degrees(270)))
            RoundedRectangle(cornerRadius: Radius.radiusUi - 3).fill(Pk.term).padding(3)
            Text("\(percent(d))%")
                .font(.custom(PkType.figure.face, fixedSize: 11))
                .monospacedDigit()
                .foregroundStyle(Pk.termInk)
        }
        .frame(width: 48, height: 48)
        .accessibilityElement()
        .accessibilityValue("\(percent(d))%")
    }
}

/// The speed, a button on the times' line (app.css, .pk-speed), on
/// term-line when it isn't 1×; its menu the speeds, and Slower and Faster.
private struct SpeedMenu: View {
    var body: some View {
        let p = Player.shared
        Menu {
            Picker(L("speed_group"), selection: Binding(get: { p.speed }, set: { p.setSpeed($0) })) {
                ForEach(Player.speeds, id: \.self) { Text(rateWord($0)).tag($0) }
            }
            Divider()
            Button(L("speed_slower")) { step(-1) }
            Button(L("speed_faster")) { step(1) }
        } label: {
            Text(rateWord(p.speed))
                .font(tstamp)
                .foregroundStyle(Pk.termInk)
                .padding(6)
                .background(p.speed != 1 ? Pk.termLine : .clear, in: RoundedRectangle(cornerRadius: Radius.radiusSm))
        }
        .padding(-6)
        .accessibilityLabel(L("speed_label", rateWord(p.speed).dropLast()))
    }

    private func step(_ d: Int) {
        let s = Player.speeds
        let i = s.firstIndex(of: Player.shared.speed) ?? 1
        Player.shared.setSpeed(s[min(max(i + d, 0), s.count - 1)])
    }
}

/// The time, a button on the times' line, where the times' format is chosen
/// (app.js, timeMenu): seconds and hundredths, or frames, and under Frames
/// the rates, a video's own first.
private struct TimeMenu: View {
    let text: String

    var body: some View {
        let s = AppState.shared
        let t = Transcriber.shared
        let base = s.base.with(duration: t.duration, video: t.fps)
        let now = Player.shared.position
        Menu {
            Button { s.base.frames = false } label: {
                if !base.frames { Label(L("times_seconds"), systemImage: "checkmark") } else { Text(L("times_seconds")) }
                Text(stamp(now, base.duration))
            }
            Button { s.base.frames = true } label: {
                if base.frames { Label(L("times_frames"), systemImage: "checkmark") } else { Text(L("times_frames")) }
                Text(L(base.own ? "times_frames_at_video" : "times_frames_at", timecode(now, base.rate), rateLabel(base.rate)))
            }
            Menu(L("times_rates")) {
                if base.video > 0 {
                    Button(L("times_video_rate", rateLabel(base.video))) {
                        s.base.frames = true
                        s.base.videoRate = true
                    }
                }
                ForEach(rates, id: \.self) { r in
                    Button(rateLabel(r)) {
                        s.base.frames = true
                        s.base.fps = r
                        if base.video > 0 { s.base.videoRate = false }
                    }
                }
            }
        } label: {
            Text(text)
                .font(tstamp)
                .monospacedDigit()
                .foregroundStyle(Pk.termInk)
                .lineLimit(1)
                .padding(6)
        }
        .padding(-6)
        .accessibilityLabel(L("times_tip"))
    }
}
