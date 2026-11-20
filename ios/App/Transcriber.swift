import Foundation
import UIKit

/// The one transcription at a time, as the screen shows it (Android's
/// Transcriber and TranscribeService), in the steps the desktop's window
/// names: loading (the model), reading (the audio), transcribing, then
/// done, stopped or error; or a transcript of the recent list, opened
/// again, done. The work runs off the main thread and tells the state on
/// it, where the screen reads it.
@MainActor @Observable
final class Transcriber {
    static let shared = Transcriber()

    enum Step { case none, loading, reading, transcribing, done, stopped, error }

    /// its place on the recent list: the time it began, in ms (recent.rs, new_id)
    private(set) var id = ""
    /// the recording it reads
    private(set) var url: URL?
    private(set) var name = ""
    private(set) var step = Step.none
    /// the fraction of the audio transcribed
    private(set) var done = 0.0
    /// the audio's length in seconds, once it is read
    private(set) var duration = 0.0
    /// its loudness for the strip (audio.rs's peaks), once it is read
    private(set) var peaks: [UInt8]?
    private(set) var segments: [Segment] = []
    /// with speakers: the names given, speaker n's at n - 1, "" for Speaker
    /// n (Speakers.nameOf); and each one's stretches of speech, for the bar
    private(set) var names: [String] = []
    /// Its speakers hidden by the Speakers switch on it, kept with it
    /// (recent.rs, hidden): this transcript's alone, never the start
    /// screen's switch (Speakers.on), which decides whether a new
    /// transcription finds them (Peter, 2026-11-19).
    private(set) var hidden = false
    private(set) var stretches: [Stretch] = []
    /// whether this run finds the speakers as it goes, the switch on and
    /// their model here: the strip keeps the bar's place for them
    private(set) var diarizing = false
    /// what went wrong, in the engine's words or the system's
    private(set) var error: String?
    /// the seconds spent transcribing so far, for the pace
    private(set) var transcribed = 0.0
    /// the pace now, seconds of audio a second over about the last 10 seconds
    /// of work, while it transcribes; the finished transcript's is the whole
    /// run's (transcribed), which while it ran was slow to show a change:
    /// speakers turned off partway, at 20x after 15x, read 17x a minute later
    /// (Peter, the S24, 2026-11-18; Android's Transcriber.pace)
    private(set) var pace = 0.0
    /// (seconds spent, seconds of audio done) at each chunk, back 10 seconds
    private var lately: [(Double, Double)] = []

    /// A chunk done: spent seconds in, audio seconds of the recording transcribed.
    private func chunk(_ spent: Double, _ audio: Double) {
        transcribed = spent
        lately.append((spent, audio))
        while lately.count > 2 && spent - lately[1].0 >= 10 { lately.removeFirst() }
        let (s0, a0) = lately[0]
        pace = lately.count > 1 && spent > s0 ? (audio - a0) / (spent - s0) : audio / spent
    }
    /// the finished transcript, for the formats
    private(set) var transcript: Transcript?
    /// a video's frame rate, for times in frames; 0 for audio
    private(set) var fps = 0.0
    /// opened from the recent list, not transcribed in this run
    private(set) var reopened = false
    /// why the recording doesn't play, in the screen's words, and whether
    /// Locate… can find it again (Recents.source)
    var why: String?
    var locate = false
    /// the edition the transcription ran on ("coreml", "cpu"), for the stats
    private(set) var edition: String?
    /// Core ML compiling the Neural Engine's encoder for this device: the
    /// first load after an install or an update, about 16 s on an M1 iPad
    private(set) var preparing = false

    var running: Bool { step == .loading || step == .reading || step == .transcribing }

    private var engine: Engine?
    /// what the engine was loaded from: a catalog model's name or the own file's path
    private(set) var loadedName: String?
    /// the model file the engine loaded, for Core.ran
    private var loadedModel: URL?
    private var loading: Task<Engine, Error>?

    private init() {}

    private func begin(_ name: String, _ url: URL?) {
        id = url == nil ? "" : String(Int64(Date().timeIntervalSince1970 * 1000))
        self.url = url
        self.name = name
        step = .reading
        done = 0
        duration = 0
        peaks = nil
        segments = []
        names = []
        stretches = []
        hidden = false
        diarizing = false
        error = nil
        transcribed = 0
        pace = 0
        lately = []
        transcript = nil
        fps = 0
        reopened = false
        why = nil
        locate = false
    }

    #if DEBUG
        /// A state as it would be, for the simulator's screenshots, which no
        /// one taps to (Root, -pretend): nothing is read or transcribed.
        func pretend(preparing p: Bool) { preparing = p }

        func pretend(
            _ step: Step, name: String, done: Double, duration: Double = 0, error: String? = nil
        ) {
            begin(name, nil)
            self.step = step
            self.done = done
            self.duration = duration
            self.error = error
        }
    #endif

    /// The recording found again where it is now (Locate…), to play.
    func relocated(_ url: URL) {
        self.url = url
        why = nil
        locate = false
    }

    /// Back to the start screen, the transcript let go.
    func clear() {
        begin("", nil)
        step = .none
    }

    /// A transcript of the recent list on the screen again, as it was when
    /// it finished, to play from its recording.
    func reopen(_ t: RecentTranscript) {
        begin(t.item.name, Recents.shared.recording(t.item))
        id = t.item.id
        duration = t.item.duration
        fps = t.item.fps
        peaks = t.peaks
        segments = t.segments
        names = t.names
        stretches = t.stretches
        hidden = t.hidden
        transcribed = t.item.transcribed
        transcript = Transcript(
            duration: t.item.duration, complete: true, model: t.item.model, segments: t.segments,
            names: t.names, stretches: t.stretches, diarizer: t.diarizer, hidden: t.hidden)
        done = 1
        reopened = true
        step = .done
    }

    /// The transcript's speakers as Speakers changed or found them: its
    /// sentences, their names and stretches, and its JSON with them, which
    /// the formats and the recent list take; with diarizer, the model that
    /// found them, where a find did.
    func respeak(
        _ segments: [Segment], names: [String], stretches: [Stretch], diarizer: String? = nil
    ) {
        self.segments = segments
        self.names = names
        self.stretches = stretches
        transcript?.segments = segments
        transcript?.names = names
        transcript?.stretches = stretches
        if let diarizer { transcript?.diarizer = diarizer }
    }

    /// Its speakers hidden, or shown again (Speakers.hide).
    func hide(_ value: Bool) {
        hidden = value
        transcript?.hidden = value
    }

    /// The model loaded from name (a catalog model's, or the own file's
    /// path) let go, its files being deleted: the engine maps them, and a
    /// deleted file keeps its blocks while it is mapped.
    func letGo(_ name: String) {
        guard loadedName == name, !running else { return }
        engine = nil
        loadedName = nil
        loadedModel = nil
    }

    /// The model in use loaded now, ahead of the first recording: Core ML
    /// compiles a Neural Engine edition's encoder the first time, which the
    /// setup screen says while it lasts.
    func prepare() {
        Task { _ = try? await model() }
    }

    /// The model in use prepared now where Core ML has yet to compile its
    /// encoder here: at launch, after an install or an update, and back on
    /// the screen after a download ended while the app was away.
    func prepareIfNew() {
        guard case .catalog(let name)? = Models.shared.usable(),
            let v = Models.shared.offer(name)?.entry, v.runs?.neuralEngine == true,
            !Editions.compiled(v)
        else { return }
        prepare()
    }

    /// The model in use, loaded once, and again once another is chosen or a
    /// download of it waits to be put in place (Core.pending), the engine
    /// that holds the files it replaces let go first: the old files ran
    /// until then (Core.settle). It runs as the core says it runs here
    /// (Editions.engine): where Core ML refuses its encoder, the whole .pkf
    /// goes on with it where that is here, a toast saying why; where only
    /// the slim file was, the transcription stops with why, and the setup
    /// screen offers the whole file, which downloads only when asked.
    private func model() async throws -> Engine {
        guard let use = Models.shared.usable() else { throw CoreError(L("no_model")) }
        let key: String
        switch use {
        case .own(let url): key = url.path
        case .catalog(let name): key = name
        }
        if case .catalog(let name) = use, loading == nil, Core.pending(name) {
            engine = nil
            loadedName = nil
            loadedModel = nil
        }
        if let engine, loadedName == key { return engine }
        if let loading { return try await loading.value }
        engine = nil
        loadedName = nil
        loadedModel = nil
        // a compile, not a load of what Core ML compiled before: a new
        // system or build, or an encoder that a download waiting to be put
        // in place brings
        if case .catalog(let name) = use, let v = Models.shared.offer(name)?.entry {
            preparing =
                v.runs?.neuralEngine == true && (!Editions.compiled(v) || Core.pending(name))
        }
        let task = Task.detached(priority: .userInitiated) { () throws -> Editions.Loaded in
            switch use {
            case .own(let url):
                return Editions.Loaded(
                    engine: try Engine(file: url), edition: "cpu", model: url, refusedNow: false)
            case .catalog(let name):
                if Core.pending(name) {
                    do {
                        try Core.settle(name)
                        trace("\(name): the update in place")
                    } catch {
                        // the old files run on, and settle is tried at the next load
                        trace("\(name): the update not put in place: \(error.localizedDescription)")
                    }
                }
                guard let v = try Core.catalog().first(where: { $0.name == name }) else {
                    throw CoreError(L("no_model"))
                }
                return try Editions.engine(for: v)
            }
        }
        let wrapped = Task { try await task.value.engine }
        loading = wrapped
        defer {
            loading = nil
            preparing = false
        }
        do {
            let l = try await task.value
            engine = l.engine
            loadedName = key
            loadedModel = l.model
            edition = l.edition
            if l.refusedNow { Models.shared.news = L("prepare_cpu", deviceKind) }
            Models.shared.refresh()
            return l.engine
        } catch let r as Editions.Refused {
            // the processor's model, which the setup screen now offers
            Models.shared.refresh()
            guard r.holds else { throw CoreError(L("model_file_damaged")) }
            let bytes = Models.shared.offer(key)?.bytes ?? 0
            throw CoreError(L("prepare_cpu_get", deviceKind, mb(bytes)))
        } catch {
            Models.shared.refresh()
            throw error
        }
    }

    /// The recording at url transcribed, as pktito transcribes one on every
    /// platform; finished, it goes on the recent list. The screen stays on
    /// while it runs, and it goes on when the person leaves the app, where
    /// the system lets it (Continued).
    func start(_ url: URL) {
        guard !running else { return }
        begin(url.lastPathComponent, url)
        let id = self.id
        let name = self.name
        // with the switch on and the speaker model here, who said what, heard
        // in the same pass (Speakers)
        let diarizer = Speakers.shared.on ? Speakers.shared.file : nil
        UIApplication.shared.isIdleTimerDisabled = true
        Continued.begin(id: id, name: name)
        trace("transcribe: \(name)")
        Task {
            defer {
                trace(
                    "transcribe: \(step), \(String(format: "%.1f", transcribed)) s for \(String(format: "%.1f", duration)) s of audio"
                )
                UIApplication.shared.isIdleTimerDisabled = false
                Continued.end(done: step == .done)
            }
            let began = Date()
            do {
                if engine == nil || loadedName == nil {
                    step = .loading
                    Continued.update()
                }
                let engine = try await model()
                step = .reading
                Continued.update()
                let progress = Engine.Progress()
                let box = TimeBox()
                progress.audio = { duration, fps, peaks in
                    box.mark()
                    Task { @MainActor in
                        self.duration = duration
                        self.fps = fps
                        self.peaks = peaks
                        self.diarizing = diarizer != nil
                        self.step = .transcribing
                        Continued.update()
                    }
                }
                progress.update = { done, segs in
                    let spent = box.since()
                    Task { @MainActor in
                        self.done = done
                        // a chunk under way when speakers were turned off comes with them still (quiet)
                        self.segments += self.diarizing ? segs : segs.map { $0.by(nil) }
                        self.chunk(spent, done * self.duration)
                        Continued.update()
                    }
                }
                let (t, moved) = try await Task.detached(priority: .userInitiated) {
                    () throws -> (Transcript, URL?) in
                    let scoped = url.startAccessingSecurityScopedResource()
                    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                    let v = try? url.resourceValues(forKeys: [
                        .fileSizeKey, .contentModificationDateKey,
                    ])
                    // a copy sent to pktito is kept itself (Recents.keep)
                    let bookmark = Recents.held(url) ? nil : try? url.bookmarkData()
                    let t =
                        try diarizer.map { file in
                            try Speakers.with(file) {
                                try engine.transcribe(url, name: name, diar: $0, progress: progress)
                            }
                        } ?? engine.transcribe(url, name: name, diar: nil, progress: progress)
                    if t.complete {
                        let item = RecentItem(
                            id: id, path: url.path, name: name, duration: t.duration,
                            finished: UInt64(Date().timeIntervalSince1970),
                            took: Date().timeIntervalSince(began),
                            transcribed: box.since(), model: t.model, fps: progress.fps,
                            size: UInt64(v?.fileSize ?? 0),
                            modified: UInt64(v?.contentModificationDate?.timeIntervalSince1970 ?? 0)
                        )
                        let moved = Recents.keep(
                            item, transcript: t, peaks: progress.peaks, recording: url,
                            bookmark: bookmark)
                        return (t, moved)
                    }
                    return (t, nil)
                }.value
                if let moved { self.url = moved }
                transcript = t
                stretches = t.stretches
                done = t.complete ? 1 : done
                step = t.complete ? .done : .stopped
                if t.complete { ran() }
            } catch {
                // the app's words where the core gave a key: not the core's
                // English, nor the file's name the header has, nor the
                // system's "(Cannot Open)" (the layout audit, 2026-11-01)
                if let e = error as? CoreError, let key = e.key {
                    self.error = words(key)
                    trace("can't transcribe: \(e.localizedDescription)")
                } else {
                    self.error = error.localizedDescription
                }
                step = .error
            }
        }
    }

    /// The transcription running ends before its next chunk.
    func stop() { Engine.stop() }

    /// The transcription running goes on without its speakers, from its next
    /// chunk (Engine.quiet): this run's alone, the start screen's switch as
    /// it was set (Peter, 2026-11-19).
    func quiet() {
        Engine.quiet()
        diarizing = false
        // what was found so far goes too, as the desktop's does (app.js,
        // spkWhileRunning): the transcript is one without speakers, which the
        // switch finds again once it is done; kept, they were saved with it
        // and the switch only hid them (Peter, Android's on the S25, 2026-11-19)
        segments = segments.map { $0.by(nil) }
    }

    /// A transcription finished with the catalog model loaded: the core
    /// hears so (Core.ran), and once the Neural Engine's edition has run from
    /// its own slim file, the whole .pkf goes, 624 MB for English, as on
    /// every platform (Peter, 2026-11-07). The engine holds the slim file,
    /// not the whole one.
    private func ran() {
        guard let name = loadedName, let model = loadedModel, let edition,
            Models.shared.offer(name) != nil
        else { return }
        Task.detached(priority: .utility) {
            guard Core.ran(name, edition: edition, model: model) else { return }
            trace("\(name): the whole file deleted, the Neural Engine's edition in its place")
            await MainActor.run { Models.shared.refresh() }
        }
    }
}

/// When a transcription's audio was read, for the seconds it spent
/// transcribing, on the work's thread.
private final class TimeBox: @unchecked Sendable {
    private let lock = NSLock()
    private var at: Date?

    func mark() {
        lock.lock()
        at = Date()
        lock.unlock()
    }

    func since() -> Double {
        lock.lock()
        defer { lock.unlock() }
        return at.map { Date().timeIntervalSince($0) } ?? 0
    }
}
