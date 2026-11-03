import PktitoCore
import SwiftUI
import UniformTypeIdentifiers

/// The formats Share offers, the desktop's Save as's, in its order.
struct Format: Identifiable {
    let ext: String
    let label: String
    let withTimes: String
    var id: String { ext }

    static let all = [
        Format(ext: "txt", label: "format_txt", withTimes: "format_txt_times"),
        Format(ext: "srt", label: "format_srt", withTimes: "format_srt"),
        Format(ext: "vtt", label: "format_vtt", withTimes: "format_vtt"),
        Format(ext: "md", label: "format_md", withTimes: "format_md_times"),
        Format(ext: "json", label: "format_json", withTimes: "format_json"),
    ]
}

/// What the screen keeps beside the transcription (Android's MainActivity):
/// the choices kept between runs, the toast, About and Manage models over
/// the screen, a recording opened before there was a model to transcribe
/// it with, and the search.
@MainActor @Observable
final class AppState {
    static let shared = AppState()

    /// Share's choice: each sentence's time before it in .txt and .md
    var times: Bool { didSet { UserDefaults.standard.set(times, forKey: "times") } }
    /// the transcript as its subtitle track's cues, not its sentences
    var captions: Bool { didSet { UserDefaults.standard.set(captions, forKey: "captions") } }
    /// the strip's choice of how times are written
    var base: Timebase {
        didSet {
            let d = UserDefaults.standard
            d.set(base.frames, forKey: "frames")
            d.set(base.fps, forKey: "fps")
            d.set(base.videoRate, forKey: "video_rate")
        }
    }
    var toast: String?
    /// the toast says something went wrong: its alert in the check's place
    var toastErr = false
    var about = false
    var managing = false
    /// a file to pick: a recording, a model file, or the recording of the
    /// transcript open found again (Locate…)
    var pick: Pick?
    /// a recording opened before there was a model: transcribed once there is
    var waiting: URL?
    let finder = Finder()
    /// the transcript's cues, made the first time the Captions view shows
    /// them, by the transcript they are of
    @ObservationIgnored private var cues: (key: String, rows: [Segment])?

    enum Pick { case media, model, locate }

    private init() {
        let d = UserDefaults.standard
        times = d.bool(forKey: "times")
        captions = d.bool(forKey: "captions")
        base = Timebase(frames: d.bool(forKey: "frames"), fps: d.object(forKey: "fps") as? Double ?? 24,
                        videoRate: d.object(forKey: "video_rate") as? Bool ?? true)
    }

    /// Whether the screen is the setup screen's: no model yet, or one
    /// coming, or Core ML compiling the one in use for this device.
    var setup: Bool {
        #if DEBUG
        if pretendSetup { return true }
        #endif
        return !Models.shared.ready || Downloads.shared.state != nil || Models.shared.copying != nil
            || Transcriber.shared.preparing
    }

    #if DEBUG
    /// the setup screen's choice with a model at hand, for the simulator's
    /// screenshots (Root, -pretend setup)
    var pretendSetup = false
    #endif

    func say(_ s: String, err: Bool = false) {
        toast = s
        toastErr = err
        Task {
            try? await Task.sleep(for: .seconds(2))
            if toast == s { toast = nil }
        }
    }

    /// A recording to transcribe: now, or once there is a model; shown at
    /// work, About put away.
    func start(_ url: URL) {
        let t = Transcriber.shared
        guard !t.running else { return }
        about = false
        if setup {
            waiting = url
            return
        }
        leave(keeping: url)
        t.start(url)
    }

    /// A recording sent to pktito from another app: Open in, or the share
    /// sheet's pktito, which iOS gives as a copy. Held out of the backups
    /// (Recents.hold) and transcribed; not while a transcription runs, when
    /// the copy is let go, as Android lets a share go then (onNewIntent).
    func received(_ url: URL) {
        guard url.isFileURL else { return }
        if Transcriber.shared.running {
            try? FileManager.default.removeItem(at: url)
            return
        }
        guard let held = Recents.hold(url) else {
            say(L("problem_unexpected"), err: true)
            return
        }
        start(held)
    }

    /// A transcript of the recent list on the screen again.
    func openRecent(_ id: String) {
        if Transcriber.shared.step != .none { leave() }
        guard let r = Recents.shared.open(id) else {
            say(L("recent_unreadable"), err: true)
            return
        }
        Transcriber.shared.reopen(r)
    }

    /// The transcript let go, its search and its player with it.
    func leave(keeping url: URL? = nil) {
        Player.shared.release()
        finder.clear()
        Transcriber.shared.clear()
        // a recording sent to pktito that no transcript kept
        Recents.shared.letGoHeld(except: [url, waiting])
    }

    /// A delete of Manage models' done, and what it said, if anything: the
    /// model now in use, or that it failed. With no model left, Manage
    /// models goes, and the screen is the setup's, its choice of a model to
    /// download, a transcript open let go first: it needs no model to be
    /// read, but would to be transcribed again (manage.ask_text_last).
    func deleted(_ said: String?) {
        if let said { say(said, err: said == L("manage_cant_delete")) }
        if Models.shared.ready { return }
        managing = false
        if Transcriber.shared.step != .none && !Transcriber.shared.running { leave() }
    }

    func back() {
        if finder.searching {
            finder.clear()
        } else if !Transcriber.shared.running {
            leave()
        }
    }

    /// The rows shown: the sentences, or as captions their cues.
    var rows: [Segment] {
        let t = Transcriber.shared
        let review = t.step == .done || t.step == .stopped
        guard captions && review, let tr = t.transcript else { return t.segments }
        let key = "\(t.id) \(tr.segments.count)"
        if cues?.key != key { cues = (key, AppState.cuesOf(tr)) }
        return cues?.rows ?? []
    }

    /// A transcript's cues (format::cues), each with its words, taken in
    /// order from the transcript's (app.js, cuesOf).
    private static func cuesOf(_ t: Transcript) -> [Segment] {
        struct Cue: Decodable {
            let start: Int64
            let end: Int64
            let text: String
            var first: Int? = 0
            var n: Int? = 0
        }
        struct Cues: Decodable { let cues: [Cue] }
        guard let json = try? JSONEncoder().encode(t) else { return [] }
        let out = String(decoding: json, as: UTF8.self).withCString { pkapp_captions($0) }
        guard let out else { return [] }
        defer { pkapp_free_string(out) }
        guard let c = try? JSONDecoder().decode(Cues.self, from: Data(bytes: out, count: strlen(out))) else { return [] }
        let words = t.segments.flatMap(\.words)
        return c.cues.map { q in
            let first = q.first ?? 0, n = q.n ?? 0
            return Segment(start: Double(q.start) / 1000, end: Double(q.end) / 1000, text: q.text,
                           words: first + n <= words.count ? Array(words[first..<(first + n)]) : [])
        }
    }

    /// The transcript as text, or as captions their SRT, on the clipboard.
    func copy() {
        guard let t = Transcriber.shared.transcript else { return }
        let ext = captions ? "srt" : "txt"
        if let text = try? Core.format(t, ext: ext, name: Transcriber.shared.name, times: false) {
            UIPasteboard.general.string = text
            say(L(captions ? "copied_captions" : "copied"))
        }
    }

    /// The transcript as a file of ext's kind, for the share sheet: in the
    /// app's temporary folder, named as the recording is, its extension the
    /// format's.
    func file(_ ext: String) -> URL? {
        let t = Transcriber.shared
        guard let tr = t.transcript,
              let text = try? Core.format(tr, ext: ext, name: t.name, times: times) else { return nil }
        let stem = (t.name as NSString).deletingPathExtension
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("shared", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent((stem.isEmpty ? "transcript" : stem) + "." + ext)
        do {
            try Data(text.utf8).write(to: url)
            return url
        } catch {
            say(L("share_failed", url.lastPathComponent), err: true)
            return nil
        }
    }
}

/// Work's pace, as the desktop's says it (app.js, pace): "12.3x real-time",
/// its decimal the locale's.
func pace(_ audio: Double, _ took: Double) -> String {
    guard took > 0 else { return "" }
    return L("real_time", String(format: "%.1f", locale: Locale.current, audio / took))
}
