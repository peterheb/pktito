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
    /// what went wrong, in the engine's words or the system's
    private(set) var error: String?
    /// the seconds spent transcribing so far, for the pace
    private(set) var transcribed = 0.0
    /// the finished transcript, for the formats and the captions
    private(set) var transcript: Transcript?
    /// a video's frame rate, for times in frames; 0 for audio
    private(set) var fps = 0.0
    /// opened from the recent list, not transcribed in this run
    private(set) var reopened = false
    /// why the recording doesn't play, in the screen's words, and whether
    /// Locate… can find it again (Recents.source)
    var why: String?
    var locate = false
    /// the edition the transcription ran on, for the stats
    private(set) var edition: Edition?
    /// Core ML compiling the Neural Engine's encoder for this device: the
    /// first load after an install or an update, about 16 s on an M1 iPad
    private(set) var preparing = false

    var running: Bool { step == .loading || step == .reading || step == .transcribing }

    private var engine: Engine?
    /// what the engine was loaded from: a catalog model's name or the own file's path
    private(set) var loadedName: String?
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
        error = nil
        transcribed = 0
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

    func pretend(_ step: Step, name: String, done: Double, duration: Double = 0, error: String? = nil) {
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
        transcribed = t.item.transcribed
        transcript = Transcript(duration: t.item.duration, complete: true, model: t.item.model, segments: t.segments)
        done = 1
        reopened = true
        step = .done
    }

    /// The model loaded from name (a catalog model's, or the own file's
    /// path) let go, its files being deleted: the engine maps them, and a
    /// deleted file keeps its blocks while it is mapped.
    func letGo(_ name: String) {
        guard loadedName == name, !running else { return }
        engine = nil
        loadedName = nil
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
        guard case .catalog(let name)? = Models.shared.usable(), let e = Models.shared.entry(name),
              Models.usable(e) == .neuralEngine, !Editions.compiled(e) else { return }
        prepare()
    }

    /// The model in use, loaded once, and again once another is chosen: its
    /// Neural Engine edition where this device runs it, else its CPU's
    /// (Editions). Where the Neural Engine edition fails and the CPU's isn't
    /// here, that one is downloaded, and a toast says why.
    private func model() async throws -> Engine {
        guard let use = Models.shared.usable() else { throw CoreError(L("no_model")) }
        let key: String
        switch use {
        case .own(let url): key = url.path
        case .catalog(let name): key = name
        }
        if let engine, loadedName == key { return engine }
        if let loading { return try await loading.value }
        engine = nil
        loadedName = nil
        let task = Task.detached(priority: .userInitiated) { () throws -> (Engine, Edition) in
            switch use {
            case .own(let url):
                return (try Engine(file: url), .cpu)
            case .catalog(let name):
                guard let e = await Models.shared.entry(name) else { throw CoreError(L("no_model")) }
                return try Editions.engine(for: e)
            }
        }
        // a compile, not a load of what Core ML compiled before
        if case .catalog(let name) = use, let e = Models.shared.entry(name) {
            preparing = Models.usable(e) == .neuralEngine && !Editions.compiled(e)
        }
        let wrapped = Task { try await task.value.0 }
        loading = wrapped
        defer {
            loading = nil
            preparing = false
        }
        do {
            let (e, ed) = try await task.value
            engine = e
            loadedName = key
            edition = ed
            return e
        } catch let failed as Editions.NeedsCPUEdition {
            Models.shared.refresh()
            Models.shared.news = L("prepare_cpu", deviceKind)
            if let o = Models.shared.offers.first(where: { $0.name == key }) {
                Downloads.shared.start(o)
            }
            throw CoreError(failed.why)
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
        UIApplication.shared.isIdleTimerDisabled = true
        Continued.begin(id: id, name: name)
        trace("transcribe: \(name)")
        Task {
            defer {
                trace("transcribe: \(step), \(String(format: "%.1f", transcribed)) s for \(String(format: "%.1f", duration)) s of audio")
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
                        self.step = .transcribing
                        Continued.update()
                    }
                }
                progress.update = { done, segs in
                    let spent = box.since()
                    Task { @MainActor in
                        self.done = done
                        self.segments += segs
                        self.transcribed = spent
                        Continued.update()
                    }
                }
                let (t, moved) = try await Task.detached(priority: .userInitiated) { () throws -> (Transcript, URL?) in
                    let scoped = url.startAccessingSecurityScopedResource()
                    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                    let v = try? url.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey])
                    // a copy sent to pktito is kept itself (Recents.keep)
                    let bookmark = Recents.held(url) ? nil : try? url.bookmarkData()
                    let t = try engine.transcribe(url, name: name, progress: progress)
                    if t.complete {
                        let item = RecentItem(
                            id: id, path: url.path, name: name, duration: t.duration,
                            finished: UInt64(Date().timeIntervalSince1970), took: Date().timeIntervalSince(began),
                            transcribed: box.since(), model: t.model, fps: progress.fps,
                            size: UInt64(v?.fileSize ?? 0),
                            modified: UInt64(v?.contentModificationDate?.timeIntervalSince1970 ?? 0))
                        let moved = Recents.keep(item, transcript: t, peaks: progress.peaks, recording: url, bookmark: bookmark)
                        return (t, moved)
                    }
                    return (t, nil)
                }.value
                if let moved { self.url = moved }
                transcript = t
                done = t.complete ? 1 : done
                step = t.complete ? .done : .stopped
            } catch {
                self.error = error.localizedDescription
                step = .error
            }
        }
    }

    /// The transcription running ends before its next chunk.
    func stop() { Engine.stop() }
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
