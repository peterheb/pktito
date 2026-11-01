import SwiftUI
import UIKit

/// One file at work and then its transcript (ui/index.html, #work; Android's
/// Work): the header with the way back, the name, and what can be done with
/// it (at work only Stop; Copy and Share once it is done), the strip, the
/// choice of sentences or captions, a line for what went wrong, and the rows.
struct Work: View {
    /// a search's matches in the rows shown, a finished transcript's only,
    /// and each row's of them
    @State private var hits: [Hit] = []
    @State private var marks: [Int: [Marked]] = [:]

    /// What the matches are found anew for.
    private struct Sought: Equatable {
        let query: String
        let captions: Bool
        let review: Bool
        let id: String
        let count: Int
    }

    var body: some View {
        let t = Transcriber.shared
        let s = AppState.shared
        let review = t.step == .done || t.step == .stopped
        let rows = s.rows
        VStack(spacing: 0) {
            Header(review: review, shown: !rows.isEmpty)
            if t.step != .error {
                Strip(
                    review: review, left: left, why: t.why,
                    locate: t.locate ? { s.pick = .locate } : nil,
                    hits: hits, tickAt: s.finder.at,
                    // a tick: its match, and the playhead there (app.js,
                    // #ticks); after the seek, which puts the reader on the
                    // match it last passed, a tick's own, where two share its time
                    onTick: { k in
                        seek(hits[k].time)
                        s.finder.go(hits, k)
                    },
                    seek: { seek($0 * t.duration) })
                    // 16 under the header's buttons: their 11 and 5
                    .padding(.horizontal, 16)
                    .padding(.top, 5)
            }
            if review && !rows.isEmpty {
                // a step while it plays takes the playhead to its match (Finder.step)
                Views(hits: hits, seek: Player.shared.playing ? { seek($0) } : nil)
                    .padding(.horizontal, 16)
                    .padding(.top, 16)
            }
            if let msg = message(rows) {
                HStack(spacing: 6) {
                    if t.error != nil { PkIcon.alert.frame(width: 16, height: 16).foregroundStyle(Pk.err) }
                    Text(msg).font(PkType.body.font(15)).foregroundStyle(t.error != nil ? Pk.err : Pk.inkMuted)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 16)
                .padding(.top, 16)
            }
            Rows(rows: rows, review: review, marks: marks, hits: hits, stats: stats(review)) { time, play in
                seek(time, play: play)
            }
        }
        .onChange(of: Sought(query: s.finder.query, captions: s.captions, review: review, id: t.id, count: rows.count),
                  initial: true) {
            hits = review ? findIn(rows, s.finder.query) : []
            marks = Dictionary(grouping: hits.indices.map { Marked(k: $0, hit: hits[$0]) }, by: \.hit.row)
            s.finder.settle(hits, position: Player.shared.position)
        }
    }

    /// The playhead at time, and the reader on the match it last passed.
    private func seek(_ time: Double, play: Bool = false) {
        Player.shared.seek(time, play: play)
        AppState.shared.finder.follow(hits, position: time, jump: true)
    }

    /// At work, the strip's time's place: the step, then the pace.
    private var left: String {
        let t = Transcriber.shared
        switch t.step {
        case .loading: return L("status_loading")
        case .reading: return L("status_reading")
        case .transcribing where t.done > 0 && t.transcribed > 0: return pace(t.done * t.duration, t.transcribed)
        default: return ""
        }
    }

    private func message(_ rows: [Segment]) -> String? {
        let t = Transcriber.shared
        if let e = t.error { return e }
        if t.step == .stopped { return L("status_stopped_at", percent(t.done)) }
        if t.step == .done && rows.isEmpty { return L("no_speech") }
        return nil
    }

    private func stats(_ review: Bool) -> String? {
        let t = Transcriber.shared
        guard review && !t.segments.isEmpty && t.transcribed > 0 else { return nil }
        return L("stats_transcribed", clock(t.transcribed), pace(t.done * t.duration, t.transcribed))
    }
}

/// The header's buttons in the middle of its 62 (app.css, .pk-head), the way
/// back's arrow on the cards' line, 16 from the screen's edge, its button's
/// padding outside it: the arrow's ink is 14 into its 40, so the button is
/// at 2.
private struct Header: View {
    let review: Bool
    /// whether there are rows to copy and share
    let shown: Bool
    /// what the name keeps beside the buttons before it goes under them:
    /// 160 at the default text size, about twenty characters, and as many
    /// at any other
    @ScaledMetric(relativeTo: .subheadline) private var floor: CGFloat = 160

    var body: some View {
        let t = Transcriber.shared
        // where the name would keep under its floor beside the buttons, the way
        // back and the buttons keep the bar's line and the name goes under
        // them, as a large title does on both platforms (agreed with
        // Android's app, 2026-10-31)
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                back
                name.frame(minWidth: floor, idealWidth: floor, maxWidth: .infinity, alignment: .leading)
                buttons
            }
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    back
                    Spacer(minLength: 0)
                    buttons
                }
                // on the cards' edge, 16 in
                name.padding(.leading, Transcriber.shared.running ? 0 : 14)
            }
        }
        .padding(.leading, t.running ? 16 : 2)
        .padding(.trailing, 16)
        .padding(.vertical, 11)
        .frame(minHeight: 62)
    }

    @ViewBuilder private var back: some View {
        if !Transcriber.shared.running {
            IconButton(icon: .back, label: L("back"), kind: .ghost) { AppState.shared.back() }
        }
    }

    private var name: some View {
        let t = Transcriber.shared
        return HStack(spacing: 10) {
            // a file's name: one line, cut in the middle, which keeps its extension
            Text(t.name).pk(PkType.ui).foregroundStyle(Pk.ink).lineLimit(1).truncationMode(.middle)
            // until the ring has progress: reading, the model loading, the first chunk
            if t.running && !(t.step == .transcribing && t.done > 0) { Spinner() }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Stop, or Copy and Share, each its words' whole width
    @ViewBuilder private var buttons: some View {
        let t = Transcriber.shared
        let s = AppState.shared
        if t.running {
            PkButton(text: L("stop"), kind: .ghost) { t.stop() }.fixedSize()
        }
        if review && shown {
            IconButton(icon: .copy, label: L(s.captions ? "copy_captions" : "copy_text")) { s.copy() }
            ShareAs().fixedSize()
        }
    }
}

/// Share, the plume button a finished transcript is for, and its menu of
/// formats, the desktop's Save as's: each sends the transcript as a file of
/// its kind to the app picked in the share sheet, which has Save to Files
/// too. Where the desktop's button saves, a phone's shares: on one a
/// transcript goes to another app, a chat or mail, far more often than to a
/// folder (Android's ShareAs).
private struct ShareAs: View {
    @State private var anchor = Anchor()

    var body: some View {
        let s = AppState.shared
        let name = Transcriber.shared.name
        let bare = (name as NSString).deletingPathExtension
        let stem = bare.isEmpty ? name : bare
        Menu {
            ForEach(Format.all) { f in
                Button {
                    if let url = s.file(f.ext) { share(url, from: anchor.view) }
                } label: {
                    Text(L(s.times ? f.withTimes : f.label))
                    Text(verbatim: "\(stem).\(f.ext)")
                }
            }
            Divider()
            Toggle(L("save_times"), isOn: Binding(get: { s.times }, set: { s.times = $0 }))
                .menuActionDismissBehavior(.disabled)
        } label: {
            HStack(spacing: 6) {
                Text(L("share")).font(PkType.ui.font(14)).lineLimit(1)
                PkIcon.down.frame(width: 16, height: 16)
            }
            .foregroundStyle(Pk.onPlume)
            .padding(.horizontal, 12)
            .frame(minHeight: 40)
            .background(Pk.plume, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        }
        .background(AnchorView(anchor: anchor))
    }
}

/// A view's UIKit view, kept for a popover to point at: the share sheet's,
/// on an iPad, at the button it came from.
final class Anchor {
    weak var view: UIView?
}

struct AnchorView: UIViewRepresentable {
    let anchor: Anchor

    func makeUIView(context: Context) -> UIView {
        let v = UIView()
        v.isUserInteractionEnabled = false
        anchor.view = v
        return v
    }

    func updateUIView(_ v: UIView, context: Context) { anchor.view = v }
}

/// The share sheet for url, from the view it came from.
@MainActor
func share(_ url: URL, from view: UIView?) {
    let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
    guard var top = (view?.window ?? scene?.keyWindow)?.rootViewController else { return }
    while let p = top.presentedViewController { top = p }
    let vc = UIActivityViewController(activityItems: [url], applicationActivities: nil)
    if let pop = vc.popoverPresentationController {
        pop.sourceView = view ?? top.view
        pop.sourceRect = view?.bounds ?? CGRect(x: top.view.bounds.midX, y: top.view.bounds.midY, width: 0, height: 0)
    }
    top.present(vc, animated: true)
}

/// The views line: Sentences or Captions, and the search's field at its end
/// (app.css, .pk-views and .pk-find), the desktop's 200 wide and 280 with a
/// search in it, where the line has room for both, 480; on a phone the
/// glass in the field's place, which opens the field over the line until
/// the search is let go, as a phone's find in page does.
private struct Views: View {
    let hits: [Hit]
    let seek: ((Double) -> Void)?
    @State private var width: CGFloat = 0

    var body: some View {
        let s = AppState.shared
        let f = s.finder
        let room = width >= 480
        HStack(spacing: 0) {
            if room || !f.open {
                Switch2(first: L("sentences"), second: L("captions"),
                        onSecond: Binding(get: { s.captions }, set: { s.captions = $0 }))
                Spacer(minLength: 12)
            }
            if room {
                FindField(finder: f, hits: hits, seek: seek)
                    .frame(width: f.query.trimmingCharacters(in: .whitespaces).isEmpty ? 200 : 280)
            } else if f.open {
                FindField(finder: f, hits: hits, seek: seek).frame(maxWidth: .infinity)
            } else {
                IconButton(icon: .search, label: L("find_label"), kind: .ghost) { f.open = true }
            }
        }
        .frame(maxWidth: .infinity)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
    }
}

/// A match in a row: its place among all of them, and where it is.
struct Marked: Equatable {
    let k: Int
    let hit: Hit
}

/// The transcript's rows (app.css, .pk-seg): each sentence's time, or a
/// cue's start and end, then its text, at most 560 wide; under 620 the time
/// above the text, as the desktop's window has it, where its 112 column
/// left too little of the line, a cue's two times a range. The row playing
/// on the surface, the beak's square after its time and the word playing
/// underlined in it; a row, tapped anywhere, plays from its time.
///
/// The list follows: the transcript's end as it comes, for a reader who is
/// there; finished, it starts at its top; the row playing is kept in sight,
/// and each move to a match is brought into sight, its row a third of the
/// way down (app.js, goHit).
private struct Rows: View {
    let rows: [Segment]
    let review: Bool
    let marks: [Int: [Marked]]
    let hits: [Hit]
    let stats: String?
    let seek: (Double, Bool) -> Void
    @State private var seen: [Int] = []
    @State private var width: CGFloat = 0
    /// the time's column, which grows with the stamps in it at a larger
    /// text size, and the row's text with it, 508 at the least
    @ScaledMetric(relativeTo: .footnote) private var column: CGFloat = 112

    var body: some View {
        let t = Transcriber.shared
        let s = AppState.shared
        let p = Player.shared
        let f = s.finder
        let position = p.position
        let base = s.base.with(duration: t.duration, video: t.fps)
        let current = review && (p.playing || position > 0) ? rows.lastIndex { $0.start <= position + 1e-3 } ?? -1 : -1
        let wide = width >= 508 + column
        ScrollViewReader { list in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(rows.indices, id: \.self) { i in
                        let now = i == current
                        let mine = marks[i] ?? []
                        SegRow(
                            s: rows[i], now: now,
                            word: now ? rows[i].words.lastIndex { $0.start <= position + 1e-3 } ?? -1 : -1,
                            marks: mine, at: mine.contains { $0.k == f.at } ? f.at : -1,
                            wide: wide, column: column, cue: s.captions && review, review: review, base: base
                        ) { seek(rows[i].start, true) }
                            .equatable()
                    }
                    if let stats {
                        Text(stats)
                            .font(PkType.small.font(12))
                            .monospacedDigit()
                            .foregroundStyle(Pk.inkMuted)
                            .padding(.leading, wide ? 28 + column : 16)
                            .padding(.top, 16)
                    }
                }
                .scrollTargetLayout()
                .padding(.horizontal, 16)
                .padding(.bottom, 16)
            }
            .onScrollTargetVisibilityChange(idType: Int.self) { seen = $0.sorted() }
            // the row playing kept in sight
            .onChange(of: p.playing ? current : -1) { _, now in
                if now >= 0 && !seen.dropLast().contains(now) {
                    withAnimation { list.scrollTo(now, anchor: .top) }
                }
            }
            // the transcript's end followed as it comes, for a reader who is there
            .onChange(of: t.segments.count) { _, n in
                if t.running && n > 0 && (seen.last ?? 0) >= n - 4 {
                    withAnimation { list.scrollTo(n - 1, anchor: .bottom) }
                }
            }
            // finished, it starts at its top
            .onChange(of: review ? t.id : "") { _, id in
                if !id.isEmpty { list.scrollTo(0, anchor: .top) }
            }
            .onChange(of: f.moved) {
                guard f.at >= 0 && f.at < hits.count else { return }
                let row = hits[f.at].row
                if !seen.dropFirst().dropLast().contains(row) {
                    withAnimation { list.scrollTo(row, anchor: UnitPoint(x: 0, y: 1.0 / 3)) }
                }
            }
            // the match playback passes, the reader with it (Finder.follow)
            .onChange(of: position) { f.follow(hits, position: position, jump: false) }
        }
        .padding(.top, 16)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
    }
}

/// A row: its time, or a cue's start and end, and its text.
private struct SegRow: View, Equatable {
    let s: Segment
    let now: Bool
    let word: Int
    let marks: [Marked]
    let at: Int
    let wide: Bool
    let column: CGFloat
    let cue: Bool
    let review: Bool
    let base: Timebase
    let onTime: () -> Void

    // the action aside, which is a new closure each frame and does the same
    static func == (a: SegRow, b: SegRow) -> Bool {
        a.s == b.s && a.now == b.now && a.word == b.word && a.marks == b.marks && a.at == b.at
            && a.wide == b.wide && a.column == b.column && a.cue == b.cue && a.review == b.review && a.base == b.base
    }

    var body: some View {
        Group {
            if wide {
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    time.frame(width: column, alignment: .leading)
                    Sentence(s: s, word: word, marks: marks, at: at).frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                VStack(alignment: .leading, spacing: 2) {
                    time
                    Sentence(s: s, word: word, marks: marks, at: at).frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .frame(maxWidth: 716, alignment: .leading)
        .background(now ? Pk.surface : Color.clear, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        .contentShape(Rectangle())
        // the whole row plays from its time, its words as well as the time,
        // which a thumb is too broad to find (Peter, the S25, 2026-10-26)
        .onTapGesture { if review { onTime() } }
        .accessibilityElement(children: .combine)
        .accessibilityAction(named: L("play_from", base.at(s.start))) { if review { onTime() } }
    }

    private var stamp: Font { PkType.stamp.font(13) }

    /// The row playing: the beak's square after its time, its foot on the
    /// time's baseline.
    @ViewBuilder private var beak: some View {
        if now {
            Rectangle()
                .fill(Pk.beak)
                .frame(width: 7, height: 7)
                .padding(.leading, 3)
                .alignmentGuide(.firstTextBaseline) { $0[.bottom] }
                .accessibilityHidden(true)
        }
    }

    /// A cue's end under its start, or on a narrow screen after it as a
    /// range, 00:02.88 – 00:05.60, the beak after the range: side by side, a
    /// start with the beak after it and the end beside that read as two
    /// times of their own (Peter, the S25, 2026-10-26).
    @ViewBuilder private var time: some View {
        if cue && !wide {
            HStack(alignment: .firstTextBaseline, spacing: 0) {
                Text(verbatim: base.at(s.start) + " – " + base.at(s.end))
                beak
            }
            .font(stamp)
            .monospacedDigit()
            .foregroundStyle(Pk.inkMuted)
        } else {
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 0) {
                    Text(verbatim: base.at(s.start))
                    beak
                }
                if cue { Text(verbatim: base.at(s.end)) }
            }
            .font(stamp)
            .monospacedDigit()
            .foregroundStyle(Pk.inkMuted)
        }
    }
}

/// What a stretch of a row's text is: in the k-th match (-1: none), that
/// the reader is on, and in the word playing.
private struct Mark: TextAttribute {
    var k = -1
    var current = false
    var word = false
}

/// A row's text in the transcript's type, the word playing underlined in
/// the beak's colour, 2 thick, 4 below the line; and a search's matches
/// marked (app.css, .pk-hit): each on plume mixed 45% into the ground, as
/// tall as its letters and 2 round them, its corners 3; the one the reader
/// is on in plume, its letters on-plume, an ink bar 2 under it, for a state
/// shown by more than its colour (the tokens' contrast, 2026-10-22).
private struct Sentence: View {
    let s: Segment
    let word: Int
    let marks: [Marked]
    let at: Int
    @Environment(\.dynamicTypeSize) private var type

    var body: some View {
        marked
            .pk(PkType.transcript)
            .foregroundStyle(Pk.ink)
            .textRenderer(Marks(em: PkType.transcript.scaled(type)))
            .fixedSize(horizontal: false, vertical: true)
    }

    /// The text cut where a match or the word playing begins or ends, each
    /// stretch with what it is.
    private var marked: Text {
        let chars = Array(s.text)
        let span = word >= 0 ? wordSpan(s, word) : nil
        let ms = marks.filter { $0.hit.end <= chars.count && $0.hit.start < $0.hit.end }
        if span == nil && ms.isEmpty { return Text(verbatim: s.text) }
        var cuts: Set<Int> = [0, chars.count]
        if let span { cuts.formUnion([span.0, span.1]) }
        for m in ms { cuts.formUnion([m.hit.start, m.hit.end]) }
        let at = cuts.sorted()
        var out = Text(verbatim: "")
        for (a, b) in zip(at, at.dropFirst()) where b > a {
            let m = ms.first { $0.hit.start <= a && b <= $0.hit.end }
            let w = span.map { $0.0 <= a && b <= $0.1 } ?? false
            var piece = Text(verbatim: String(chars[a..<b]))
            if m != nil || w {
                let current = m?.k == self.at && m != nil
                piece = piece.customAttribute(Mark(k: m?.k ?? -1, current: current, word: w))
                if current { piece = piece.foregroundStyle(Pk.onPlume) }
            }
            out = out + piece
        }
        return out
    }
}

/// Where the word-th word of s is in its text, in characters: the words,
/// in order, each found after the last.
private func wordSpan(_ s: Segment, _ word: Int) -> (Int, Int)? {
    let text = Array(s.text)
    var at = 0
    for (i, w) in s.words.enumerated() {
        let t = Array(w.text.trimmingCharacters(in: .whitespaces))
        guard !t.isEmpty, let k = firstIndex(of: t, in: text, from: at) else { return nil }
        if i == word { return (k, k + t.count) }
        at = k + t.count
    }
    return nil
}

private func firstIndex(of q: [Character], in s: [Character], from: Int) -> Int? {
    var i = from
    while i + q.count <= s.count {
        if s[i] == q[0] && Array(s[i..<(i + q.count)]) == q { return i }
        i += 1
    }
    return nil
}

/// The marks behind a row's letters, then the letters, then the word
/// playing's line: a match's mark is its letters' height, from Archivo's
/// ascent to its descent (0.878 and 0.21 em, its hhea), and 2 round them,
/// as an inline mark's box is drawn on the desktop; a match's stretches on
/// a line drawn as one.
private struct Marks: TextRenderer {
    let em: CGFloat

    func draw(layout: Text.Layout, in ctx: inout GraphicsContext) {
        let hl = Pk.matchFill
        for line in layout {
            var spans: [(k: Int, current: Bool, x0: CGFloat, x1: CGFloat, y: CGFloat)] = []
            for run in line {
                guard let m = run[Mark.self], m.k >= 0 else { continue }
                let b = run.typographicBounds
                if let last = spans.last, last.k == m.k {
                    spans[spans.count - 1].x1 = b.origin.x + b.width
                } else {
                    spans.append((m.k, m.current, b.origin.x, b.origin.x + b.width, b.origin.y))
                }
            }
            for sp in spans {
                let top = sp.y - 0.878 * em - 2
                let bottom = sp.y + 0.21 * em + 2
                let r = CGRect(x: sp.x0 - 2, y: top, width: sp.x1 - sp.x0 + 4, height: bottom - top)
                ctx.fill(Path(roundedRect: r, cornerRadius: 3), with: .color(sp.current ? Pk.plume : hl))
                if sp.current {
                    ctx.fill(Path(CGRect(x: sp.x0, y: bottom, width: sp.x1 - sp.x0, height: 2)), with: .color(Pk.ink))
                }
            }
        }
        for line in layout {
            for run in line { ctx.draw(run) }
        }
        for line in layout {
            for run in line {
                guard let m = run[Mark.self], m.word else { continue }
                let b = run.typographicBounds
                ctx.fill(Path(CGRect(x: b.origin.x, y: b.origin.y + 3, width: b.width, height: 2)), with: .color(Pk.beak))
            }
        }
    }
}

extension Pk {
    /// a match's mark: plume mixed 45% into the ground (app.css, .pk-hit)
    static let matchFill = Color(uiColor: UIColor { t in
        func rgb(_ c: Color) -> (CGFloat, CGFloat, CGFloat) {
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            UIColor(c).resolvedColor(with: t).getRed(&r, green: &g, blue: &b, alpha: &a)
            return (r, g, b)
        }
        let (g, p) = (rgb(Pk.ground), rgb(Pk.plume))
        return UIColor(red: g.0 * 0.55 + p.0 * 0.45, green: g.1 * 0.55 + p.1 * 0.45, blue: g.2 * 0.55 + p.2 * 0.45, alpha: 1)
    })
}
