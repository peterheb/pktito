import Foundation

/// Who said what (PKT-27; the speakers design, page version 7, every call
/// Peter's of 2026-11-09), as the desktop's window and Android's app have it
/// (ui/app.js, speakers; Speakers.kt). A segment says its speaker, numbered
/// from 1 by first word as pktito-c numbers them, eight at most, the
/// diarizer's; a change of speaker ends a sentence, so a turn is a speaker's
/// segments in a row. The names are the file's own, speaker n's at n - 1, ""
/// for Speaker n: no voice is remembered between recordings. The start
/// screen's switch (on) alone decides whether a new transcription finds
/// them; the Speakers switch on a transcript finds, hides or shows that
/// transcript's alone (Transcriber.hidden), and every format shared or
/// copied has them while they show (Peter, 2026-11-19). The transcript
/// itself is Transcriber's; this changes its speakers and keeps them on the
/// recent list (Core.keepSpeakers).
@MainActor @Observable
final class Speakers {
    static let shared = Speakers()

    /// The diarizer's most, and the colors' (the tokens' speaker1 to speaker8).
    nonisolated static let most = 8

    /// The speaker model a build offers (ios.rs, pkapp_diarizer): its file's
    /// name in the models folder.
    struct Offer: Decodable, Equatable {
        let name: String
        let license: String
        let file: String
        let bytes: Int64
        let url: String
        let kept: Int64
        let have: Bool

        /// its file, for URLSession (Downloader)
        var part: Part { Part(file: file, bytes: bytes, sha256: "", url: URL(string: url)) }
    }

    /// The turn starting at segment from: its segments, from through to,
    /// its speaker, its times.
    struct Turn: Equatable {
        let from: Int
        let to: Int
        let speaker: Int
        let start: Double
        let end: Double
    }

    /// What an edit changed, for its toast's Undo to put back.
    struct Before {
        let segments: [Segment]
        let names: [String]
        let stretches: [Stretch]
    }

    /// A sentence a move made one again (rejoin): the sentence, its words
    /// from words on the ones joined to it, their speaker, and when, for its
    /// row to light them a moment (Sentence).
    struct Joined: Equatable {
        let seg: Segment
        let words: Int
        let speaker: Int
        let at: Date
    }

    /// The start screen's switch, Tell speakers apart: whether a new
    /// transcription finds the speakers, a recording shared in from another
    /// app too. Off at first on a phone, where the speaker model takes as
    /// long again as the words or more (the S25: 54 to 57 s with, 20 to 28 s
    /// without, 2026-11-19); the desktop's is on. Nothing else turns it on or
    /// off, not the switch on a transcript nor the menu while one runs.
    private(set) var on = false
    /// The last move's sentence made one again, its row lit a moment; nil
    /// after an Undo.
    private(set) var joined: Joined?
    /// The speaker model this build offers, nil where none.
    private(set) var offer: Offer?
    /// Finding the speakers of a transcript from before (find).
    private(set) var finding = false
    /// Of its recording, how much the diarizer has heard while finding, 0
    /// to 1: the strip's ring.
    private(set) var heard = 0.0
    /// While finding, the speaker bar so far: the stretches the diarizer has
    /// told apart, those ended and those going where it has got to, for the
    /// bar to fill in as they are heard (Peter, 2026-11-15).
    private(set) var growing: [Stretch] = []

    /// One diarizer at work at a time: a transcription's or a find's (with).
    nonisolated private static let working = NSLock()
    /// The recent list's writes of speakers, in the order they were made.
    nonisolated private static let keeper = DispatchQueue(label: "net.pktito.app.speakers")

    private init() {}

    /// The start screen's switch as it was kept, and the offer as the models folder has it.
    func load() {
        on = UserDefaults.standard.bool(forKey: "speakers")
        refresh()
    }

    func set(_ value: Bool) {
        on = value
        UserDefaults.standard.set(value, forKey: "speakers")
    }

    /// The transcript's speakers hidden or shown again, kept with it.
    func hide(_ value: Bool) {
        Transcriber.shared.hide(value)
        keep()
    }

    /// The offer again, after a download or a delete.
    func refresh() {
        do {
            offer = try Core.diarizer()
        } catch {
            trace("the speaker model's offer: \(error.localizedDescription)")
            offer = nil
        }
    }

    /// The speaker model's file, where it is downloaded.
    var file: URL? {
        guard let o = offer, o.have else { return nil }
        return Store.models.appendingPathComponent(o.file)
    }

    /// The speaker model deleted for Manage models, or with part what a
    /// download of it that stopped kept, and what it said, if anything: that
    /// a transcription or a find has it (with), whose Delete is dimmed then,
    /// or that it failed. The names found already stay with their
    /// transcripts; either switch asks for the model again.
    func delete(part: Bool) -> String? {
        guard Speakers.working.try() else { return L("manage_speakers_busy") }
        var failed = false
        do {
            try Core.deleteDiarizer(part: part)
        } catch {
            trace("deleting the speaker model: \(error.localizedDescription)")
            failed = true
        }
        Speakers.working.unlock()
        if let o = offer { Downloader.shared.forget(o.file) }
        refresh()
        return failed ? L("manage_cant_delete") : nil
    }

    /// block with a diarizer for the speaker model at file, loaded for it and
    /// let go after, one at a time, on the thread that calls it.
    nonisolated static func with<T>(_ file: URL, _ block: (Diarizer) throws -> T) throws -> T {
        working.lock()
        defer { working.unlock() }
        // the diarizer let go inside the lock, as its scope ends
        return try { () throws -> T in try block(try Diarizer(file: file)) }()
    }

    /// The speakers of segs, in the order they first speak.
    static func present(_ segs: [Segment]) -> [Int] {
        var seen: [Int] = []
        for s in segs { if let n = s.speaker, !seen.contains(n) { seen.append(n) } }
        return seen
    }

    /// Speaker n's name: the one given, else Speaker n.
    static func nameOf(_ names: [String], _ n: Int) -> String {
        let given =
            n >= 1 && n <= names.count ? names[n - 1].trimmingCharacters(in: .whitespaces) : ""
        return given.isEmpty ? L("speakers_default", n) : given
    }

    /// Whether segment i starts a turn.
    static func turnStarts(_ segs: [Segment], _ i: Int) -> Bool {
        segs[i].speaker != nil && (i == 0 || segs[i - 1].speaker != segs[i].speaker)
    }

    static func turnAt(_ segs: [Segment], _ i: Int) -> Turn {
        let s = segs[i].speaker ?? 1
        var j = i
        while j + 1 < segs.count && segs[j + 1].speaker == s { j += 1 }
        return Turn(from: i, to: j, speaker: s, start: segs[i].start, end: segs[j].end)
    }

    /// New speaker's number: the lowest not in use, so that no two share a
    /// color; none past eight.
    static func fresh(_ present: [Int]) -> Int? { (1...most).first { !present.contains($0) } }

    /// The speaker already called v, other than s: renaming s to it joins
    /// the two.
    static func joinee(_ segs: [Segment], _ names: [String], _ s: Int, _ v: String) -> Int? {
        let v = v.trimmingCharacters(in: .whitespaces)
        return present(segs).first {
            $0 != s && nameOf(names, $0).caseInsensitiveCompare(v) == .orderedSame
        }
    }

    /// The transcript's speakers as an edit changes them (edit).
    private struct Draft {
        var segments: [Segment]
        var names: [String]
        var stretches: [Stretch]
    }

    /// Turn u given to speaker to, a new one where fresh: what it was
    /// before, for Undo; nil where it is that speaker's already.
    func move(_ u: Turn, to: Int, fresh: Bool) -> Before? {
        guard to != u.speaker else { return nil }
        var lit: Joined?
        let before = edit { d in
            if fresh { d.names = Speakers.named(d.names, to, "") }
            for k in u.from...u.to { d.segments[k] = d.segments[k].by(to) }
            d.stretches = Speakers.moved(d.stretches, u.speaker, to, u.start, u.end)
            if let (at, words) = Speakers.rejoin(&d.segments, u.from, u.to) {
                lit = Joined(seg: d.segments[at], words: words, speaker: to, at: Date())
            }
        }
        joined = lit
        return before
    }

    /// Speaker s renamed v: a name of its own, or where v is another
    /// speaker's, the two joined, the other's number with what it was before
    /// for Undo. Nil where nothing changes.
    func rename(_ s: Int, _ v: String) -> (other: Int?, before: Before)? {
        let t = Transcriber.shared
        let name = v.trimmingCharacters(in: .whitespaces)
        if name.isEmpty || name == Speakers.nameOf(t.names, s) { return nil }
        guard let other = Speakers.joinee(t.segments, t.names, s, name) else {
            return (nil, edit { d in d.names = Speakers.named(d.names, s, name) })
        }
        return (
            other,
            edit { d in
                let changed = d.segments.indices.filter { d.segments[$0].speaker == s }
                for k in changed { d.segments[k] = d.segments[k].by(other) }
                d.stretches = d.stretches.map {
                    $0.speaker == s ? Stretch(start: $0.start, end: $0.end, speaker: other) : $0
                }
                // no row of the reader's to light: the whole transcript's turns
                if let a = changed.first, let b = changed.last {
                    _ = Speakers.rejoin(&d.segments, a, b)
                }
            }
        )
    }

    func undo(_ b: Before) {
        joined = nil
        Transcriber.shared.respeak(b.segments, names: b.names, stretches: b.stretches)
        keep()
    }

    /// Who said what in the transcript on the screen, one from before the
    /// switch: the diarizer over its recording, which takes seconds, not a
    /// transcription again (Diarizer.find), and kept on the recent list.
    /// failed hears why it could not, in the app's words.
    func find(failed: @escaping (String) -> Void) {
        let t = Transcriber.shared
        guard let tr = t.transcript, let url = t.url, let file, !finding else { return }
        let id = t.id
        let name = t.name
        finding = true
        // found to be shown, whatever its switch hid before
        t.hide(false)
        heard = 0
        growing = []
        // the strip's ring has play's place until they are found (Work)
        Player.shared.pause()
        let heard = Diarizer.Heard()
        // the bar's stretches ended so far, on the find's thread
        var ended: [Stretch] = []
        heard.done = { d, closed, open in
            ended += closed
            let now = ended + open
            Task { @MainActor in
                self.heard = d
                self.growing = now
            }
        }
        Task {
            let got = await Task.detached(priority: .userInitiated) {
                Result { () throws -> Transcript in
                    let scoped = url.startAccessingSecurityScopedResource()
                    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                    return try Speakers.with(file) {
                        try $0.find(url, name: name, transcript: tr, heard: heard)
                    }
                }
            }.value
            finding = false
            growing = []
            switch got {
            case .success(let found):
                // the screen's transcript still, not one opened since
                guard t.id == id, t.segments == tr.segments else { return }
                t.respeak(
                    found.segments, names: [], stretches: found.stretches,
                    diarizer: found.diarizer)
                keep()
            case .failure(let e):
                trace("finding the speakers of \(name): \(e.localizedDescription)")
                if let e = e as? CoreError, let key = e.key {
                    failed(words(key))
                } else {
                    failed(e.localizedDescription)
                }
            }
        }
    }

    /// A sentence's end: its stop, and the quotes and brackets closing after
    /// it (app.js, ENDS).
    private static let ends = #/[.?!…]["”’')\]]*$/#

    /// The longest a sentence made one again runs: pk_sentences' 30 s, a
    /// caption's most.
    private static let longest = 30.0

    /// The sentences from from - 1 through to + 1 that a change left one
    /// speaker's in a row made one again, where the first has no end of its
    /// own, up to 30 s (app.js, rejoin): a word given to another speaker cut
    /// its sentence in two (a change of speaker ends one), and put back with
    /// the speaker around it, the sentence is whole again. The last sentence
    /// that grew and how many words it had before, for its row to light
    /// those after them; nil where none did.
    static func rejoin(_ segs: inout [Segment], _ from: Int, _ to: Int) -> (Int, Int)? {
        var joined: (Int, Int)?
        var k = max(0, from - 1)
        var last = min(segs.count - 1, to + 1)
        while k < last {
            let a = segs[k]
            let b = segs[k + 1]
            let open = a.text.trimmingCharacters(in: .whitespaces).firstMatch(of: ends) == nil
            if a.speaker != nil && a.speaker == b.speaker && open && b.end - a.start <= longest {
                if joined?.0 != k { joined = (k, a.words.count) }
                segs[k] = Segment(
                    start: a.start, end: b.end, text: "\(a.text) \(b.text)",
                    words: a.words + b.words, speaker: a.speaker)
                segs.remove(at: k + 1)
                last -= 1
            } else {
                k += 1
            }
        }
        return joined
    }

    /// A change made to the transcript's speakers, then drawn and kept:
    /// what it was before.
    private func edit(_ change: (inout Draft) -> Void) -> Before {
        let t = Transcriber.shared
        let before = Before(segments: t.segments, names: t.names, stretches: t.stretches)
        var d = Draft(segments: t.segments, names: t.names, stretches: t.stretches)
        change(&d)
        t.respeak(d.segments, names: d.names, stretches: d.stretches)
        keep()
        return before
    }

    /// The transcript on the screen kept on the recent list with its
    /// speakers as they are now, the writes in the order they were made.
    private func keep() {
        let t = Transcriber.shared
        guard !t.id.isEmpty, let tr = t.transcript else { return }
        let id = t.id
        Speakers.keeper.async {
            do {
                try Core.keepSpeakers(id, tr)
            } catch {
                trace("keeping the speakers of \(id): \(error.localizedDescription)")
            }
        }
    }

    /// names with speaker n's set, those before it kept as they are.
    private static func named(_ names: [String], _ n: Int, _ name: String) -> [String] {
        var out = names
        while out.count < n { out.append("") }
        out[n - 1] = name
        return out
    }

    /// The stretches of speaker from between a and b given to speaker to.
    private static func moved(_ list: [Stretch], _ from: Int, _ to: Int, _ a: Double, _ b: Double)
        -> [Stretch]
    {
        var out: [Stretch] = []
        for g in list {
            if g.speaker != from || g.end <= a || g.start >= b {
                out.append(g)
                continue
            }
            if g.start < a { out.append(Stretch(start: g.start, end: a, speaker: g.speaker)) }
            out.append(Stretch(start: max(g.start, a), end: min(g.end, b), speaker: to))
            if g.end > b { out.append(Stretch(start: b, end: g.end, speaker: g.speaker)) }
        }
        return out
    }

    /// A line of the speaker bar: each speaker's part of it, from a to b seconds.
    struct BarPart {
        let speaker: Int
        let a: Double
        let b: Double
    }

    /// The bar's parts (the desktop's barParts): each speaker's stretches up
    /// to upto seconds, those that meet made one, and where two overlap (the
    /// diarizer's can), the one who was talking keeps the line until they
    /// stop.
    nonisolated static func parts(_ stretches: [Stretch], upto: Double) -> [BarPart] {
        var by: [Int: [(Double, Double)]] = [:]
        for g in stretches {
            let b = min(g.end, upto)
            if b <= g.start { continue }
            by[g.speaker, default: []].append((g.start, b))
        }
        var all: [(s: Int, a: Double, b: Double)] = []
        for s in by.keys.sorted() {
            var cur: Int?
            for (a, b) in by[s]!.sorted(by: { $0.0 < $1.0 }) {
                if let c = cur, a <= all[c].b {
                    all[c].b = max(all[c].b, b)
                } else {
                    all.append((s, a, b))
                    cur = all.count - 1
                }
            }
        }
        all.sort { $0.a != $1.a ? $0.a < $1.a : $0.b < $1.b }
        var main: [BarPart] = []
        var holder: (s: Int, a: Double, b: Double)?
        for u in all {
            if let h = holder, u.a < h.b {
                if u.b > h.b {
                    main.append(BarPart(speaker: u.s, a: h.b, b: u.b))
                    holder = u
                }
            } else {
                main.append(BarPart(speaker: u.s, a: u.a, b: u.b))
                holder = u
            }
        }
        return main
    }

    /// Each of px points' speaker, per seconds a point: whoever talks most
    /// of it; 0 none. With everyTurn, a part that wins no point takes the one
    /// at its middle, so that a word given to another speaker shows on the
    /// bar, to be found and put right (app.js, pixels).
    nonisolated static func pixels(
        _ parts: [BarPart], _ px: Int, _ per: Double, everyTurn: Bool = false
    )
        -> [Int]
    {
        var time = Array(repeating: Array(repeating: 0.0, count: px), count: most + 1)
        // the points a part is over, none where it is past the end
        func span(_ p: BarPart) -> StrideThrough<Int> {
            stride(from: max(0, Int(p.a / per)), through: min(px - 1, Int(p.b / per)), by: 1)
        }
        for p in parts {
            let n = slotOf(p.speaker)
            for x in span(p) {
                let t = min(p.b, Double(x + 1) * per) - max(p.a, Double(x) * per)
                if t > 0 { time[n][x] += t }
            }
        }
        var who = Array(repeating: 0, count: px)
        for x in 0..<px {
            var top = 0.0
            for n in 1...most where time[n][x] > top {
                top = time[n][x]
                who[x] = n
            }
        }
        if everyTurn {
            for p in parts {
                let n = slotOf(p.speaker)
                if !span(p).contains(where: { who[$0] == n }) {
                    who[min(max(Int((p.a + p.b) / 2 / per), 0), px - 1)] = n
                }
            }
        }
        return who
    }

    /// Speaker n's color, 1 to 8.
    nonisolated static func slotOf(_ n: Int) -> Int { ((n - 1) % most + most) % most + 1 }
}
