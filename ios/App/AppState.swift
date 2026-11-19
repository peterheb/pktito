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
    /// an edit of the speakers that Undo takes back, and what its toast says
    var undo: (text: String, before: Speakers.Before)?
    /// the last edit undo was, kept past its toast for the Speakers menu
    /// (People.undo) until the next edit or another transcript (Peter,
    /// 2026-11-18: a move made by mistake, and no Undo seen; Android's last)
    var last: (text: String, before: Speakers.Before)?
    /// the speaker Rename is up for
    var renaming: Int?
    /// the speaker model's download, asked about
    var askSpeakers = false
    var about = false
    var managing = false
    /// a file to pick: a recording, a model file, or the recording of the
    /// transcript open found again (Locate…)
    var pick: Pick?
    /// a recording opened before there was a model: transcribed once there is
    var waiting: URL?
    let finder = Finder()

    enum Pick { case media, model, locate }

    private init() {
        let d = UserDefaults.standard
        times = d.bool(forKey: "times")
        base = Timebase(
            frames: d.bool(forKey: "frames"), fps: d.object(forKey: "fps") as? Double ?? 24,
            videoRate: d.object(forKey: "video_rate") as? Bool ?? true)
    }

    /// Whether the screen is the setup screen's: no model yet, or one
    /// coming, or Core ML compiling the one in use for this device.
    var setup: Bool {
        #if DEBUG
            if pretendSetup { return true }
        #endif
        // the speaker model's download is the work screen's (Downloads.speakers)
        return !Models.shared.ready
            || (Downloads.shared.state != nil && !Downloads.shared.speakers)
            || Models.shared.copying != nil || Transcriber.shared.preparing
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
        // an edit's Undo is the transcript's that goes
        undo = nil
        last = nil
        renaming = nil
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

    /// The last edit taken back, from its toast or the Speakers menu.
    func undoLast() {
        guard let u = last ?? undo else { return }
        undo = nil
        last = nil
        Speakers.shared.undo(u.before)
    }

    func back() {
        if finder.searching {
            finder.clear()
        } else if !Transcriber.shared.running {
            leave()
        }
    }

    /// An edit of the speakers done, said in a toast whose Undo takes it
    /// back, for 6 s, as Android's.
    func offerUndo(_ text: String, _ before: Speakers.Before) {
        undo = (text, before)
        last = undo
        Task {
            try? await Task.sleep(for: .seconds(6))
            if undo?.text == text { undo = nil }
        }
    }

    /// Whether the formats name the speakers: the switch on and a voice or
    /// more found. format.rs leaves a lone one out of the text, as the screen
    /// does, and the JSON keeps it.
    private var spoken: Bool {
        Speakers.shared.on && !Speakers.present(Transcriber.shared.segments).isEmpty
    }

    /// The switch (app.js, the switch's click): on, the first time it asks
    /// to download the speaker model, and for a transcript from before finds
    /// its speakers, which takes seconds, not a transcription again; tapped
    /// again, the names go from the screen and the formats, kept for when it
    /// is on again. On with none found yet, it finds them; while their
    /// model's download has failed, it tries again.
    func toggleSpeakers() {
        let t = Transcriber.shared
        let spk = Speakers.shared
        guard t.step == .done || t.step == .stopped, !spk.finding else { return }
        if Downloads.shared.speakers {
            if Downloads.shared.state?.step == .failed { Downloads.shared.retry() }
            return
        }
        let none = Speakers.present(t.segments).isEmpty
        let on = !spk.on || none
        if on && spk.offer?.have == false {
            askSpeakers = true
            return
        }
        spk.set(on)
        if on && none { spk.find { self.say($0, err: true) } }
    }

    /// A turn given to a speaker in Who's speaking, a new one where fresh;
    /// its toast's Undo.
    func moveSpeaker(_ u: Speakers.Turn, to: Int, fresh: Bool) {
        guard let before = Speakers.shared.move(u, to: to, fresh: fresh) else { return }
        let name = Speakers.nameOf(Transcriber.shared.names, to)
        offerUndo(L("speakers_moved", name), before)
    }

    /// Rename's name for speaker n: where it is another's, the two join, and
    /// Undo parts them.
    func renameSpeaker(_ n: Int, _ v: String) {
        guard let (other, before) = Speakers.shared.rename(n, v), let other else { return }
        offerUndo(L("speakers_joined", Speakers.nameOf(Transcriber.shared.names, other)), before)
    }

    /// What the speakers' work says at the screen's foot while it goes on
    /// (app.js's toasts): their model's download and how far it has got, or
    /// why it failed. Finding them says so in the strip (Work, the ring).
    var speakersNote: String? {
        guard Downloads.shared.speakers, let d = Downloads.shared.state else { return nil }
        if d.step == .failed, let p = d.problem { return p.localizedDescription }
        return L("speakers_downloading", percent(Double(d.got) / Double(max(d.total, 1))))
    }

    /// The transcript as text on the clipboard, as the desktop's Copy.
    func copy() {
        guard let t = Transcriber.shared.transcript else { return }
        if let text = try? Core.format(
            t, ext: "txt", name: Transcriber.shared.name, times: false, speakers: spoken)
        {
            UIPasteboard.general.string = text
            say(L("copied"))
        }
    }

    /// The transcript as a file of ext's kind, for the share sheet: in the
    /// app's temporary folder, named as the recording is, its extension the
    /// format's.
    func file(_ ext: String) -> URL? {
        let t = Transcriber.shared
        guard let tr = t.transcript,
            let text = try? Core.format(tr, ext: ext, name: t.name, times: times, speakers: spoken)
        else { return nil }
        let stem = (t.name as NSString).deletingPathExtension
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(
            "shared", isDirectory: true)
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

/// Work's pace, as the desktop's says it (app.js, pace): "12.3× real time",
/// its decimal the locale's.
func pace(_ audio: Double, _ took: Double) -> String {
    guard took > 0 else { return "" }
    return L("real_time", String(format: "%.1f", locale: Locale.current, audio / took))
}
