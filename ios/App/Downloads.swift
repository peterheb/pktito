import CryptoKit
import Foundation
import UIKit

/// What went wrong with a download, by the desktop's key for its words
/// (strings.rs, Problem; Words.swift, words): "download.damaged", ...
struct DownloadProblem: LocalizedError {
    let key: String
    var detail = ""
    var errorDescription: String? { words(key, host: "models.pktito.net") + (detail.isEmpty ? "" : " (\(detail))") }
}

/// A model file's download from models.pktito.net, in URLSession's
/// background session: a transfer goes on while the app is away, after iOS
/// ends the app too, and iOS wakes the app when it is done (AppDelegate).
/// Each carries its file's name (taskDescription), by which a later run of
/// the app finds it again. A stopped one keeps what it has, its resume
/// data, beside the models (FILE.resume) to go on from. A file that came is
/// held beside its place (FILE.download) until its SHA-256 checks against
/// the catalog's, as the core's download checks it on every other platform
/// (model.rs), then renamed into place; one that doesn't check is deleted.
final class Downloader: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    static let shared = Downloader()
    static let sessionID = "net.pktito.app.models"

    private(set) lazy var session: URLSession = {
        let c = URLSessionConfiguration.background(withIdentifier: Downloader.sessionID)
        c.httpAdditionalHeaders = ["User-Agent": Downloader.userAgent]
        // begun by the person, so not left for when the system likes
        c.isDiscretionary = false
        c.sessionSendsLaunchEvents = true
        return URLSession(configuration: c, delegate: self, delegateQueue: nil)
    }()

    /// iOS's to call once the session's events have come, after it woke the
    /// app for them (AppDelegate)
    var wakeDone: (() -> Void)?

    /// As the other apps say who they are: "pktito/0.1 (iOS; arm64)"
    static let userAgent: String = {
        let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
        return "pktito/\(v) (iOS; arm64)"
    }()

    private struct Job {
        let progress: @Sendable (Int64) -> Void
        let done: CheckedContinuation<URL, Error>
    }

    private let lock = NSLock()
    /// the transfer each file is coming by, by the file's name: a pause's
    /// or a stop's ends with what it has, and a later one's events are not
    /// mistaken for a newer one's
    private var current: [String: Int] = [:]
    /// who waits for each file's transfer
    private var jobs: [String: Job] = [:]
    /// a file's transfer that failed before anyone waited for it, for fetch
    private var failed: [String: Error] = [:]

    private static func held(_ file: String) -> URL { Store.models.appendingPathComponent(file + ".download") }
    private static func kept(_ file: String) -> URL { Store.models.appendingPathComponent(file + ".resume") }

    /// Each of parts on its way: its transfer a run before began, where the
    /// session still has it, else one begun now, from what a stopped one
    /// kept. All at once, while the app is on the screen: one begun from the
    /// background is left for when the system likes.
    func begin(_ parts: [Part]) async {
        let tasks = await session.allTasks
        for p in parts where !Store.have(p) && !FileManager.default.fileExists(atPath: Downloader.held(p.file).path) {
            let task: URLSessionTask
            if let t = tasks.first(where: { $0.taskDescription == p.file && ($0.state == .running || $0.state == .suspended) }) {
                task = t
            } else {
                let new: URLSessionDownloadTask
                if let data = try? Data(contentsOf: Downloader.kept(p.file)) {
                    new = session.downloadTask(withResumeData: data)
                } else {
                    new = session.downloadTask(with: p.url)
                }
                try? FileManager.default.removeItem(at: Downloader.kept(p.file))
                new.taskDescription = p.file
                task = new
            }
            lock.withLock {
                if current[p.file] != task.taskIdentifier { failed[p.file] = nil }
                current[p.file] = task.taskIdentifier
            }
            task.resume()
        }
    }

    /// p's transfer waited for (begin), then checked: its path once it is in
    /// place. progress hears its bytes so far, on URLSession's queue.
    func fetch(_ p: Part, progress: @escaping @Sendable (Int64) -> Void,
               checking: @escaping @Sendable () -> Void = {}) async throws -> URL {
        if Store.have(p) {
            return Store.file(p)
        }
        let got: URL = try await withCheckedThrowingContinuation { done in
            lock.lock()
            defer { lock.unlock() }
            // one that came, or failed, before anyone waited for it
            let held = Downloader.held(p.file)
            if FileManager.default.fileExists(atPath: held.path) {
                done.resume(returning: held)
            } else if let error = failed.removeValue(forKey: p.file) {
                done.resume(throwing: error)
            } else {
                jobs[p.file] = Job(progress: progress, done: done)
            }
        }
        checking()
        return try await Task.detached(priority: .utility) { try Downloader.check(got, p) }.value
    }

    /// The bytes a stopped download of p kept, by its resume data.
    func kept(_ p: Part) -> Int64 {
        guard let data = try? Data(contentsOf: Downloader.kept(p.file)),
              let plist = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any]
        else { return 0 }
        return (plist["NSURLSessionResumeBytesReceived"] as? NSNumber)?.int64Value ?? 0
    }

    /// What a download of p kept or got, let go.
    func forget(_ p: Part) {
        for f in [Downloader.kept(p.file), Downloader.held(p.file)] { try? FileManager.default.removeItem(at: f) }
        lock.lock()
        failed[p.file] = nil
        lock.unlock()
    }

    /// Every transfer stops, keeping what it has for the next begin; each
    /// fetch waiting hears it stopped (didCompleteWithError).
    func pause() {
        session.getTasksWithCompletionHandler { _, _, downloads in
            for d in downloads {
                let file = d.taskDescription
                d.cancel(byProducingResumeData: { data in
                    if let data, let file { try? data.write(to: Downloader.kept(file)) }
                })
            }
        }
    }

    /// Every transfer stops, keeping nothing; each fetch waiting hears it
    /// was cancelled, now.
    func stop() {
        lock.lock()
        let waiting = jobs.values
        jobs = [:]
        current = [:]
        failed = [:]
        lock.unlock()
        for j in waiting { j.done.resume(throwing: CancellationError()) }
        session.getTasksWithCompletionHandler { _, _, downloads in
            for d in downloads { d.cancel() }
        }
    }

    /// The file a finished transfer gave, checked: its SHA-256 the catalog's,
    /// then renamed into place; else deleted.
    private static func check(_ got: URL, _ p: Part) throws -> URL {
        defer { try? FileManager.default.removeItem(at: got) }
        let h = try sha256(of: got)
        guard h == p.sha256 else {
            throw DownloadProblem(key: "download.damaged", detail: p.file)
        }
        let dst = Store.file(p)
        try? FileManager.default.removeItem(at: dst)
        do {
            try FileManager.default.moveItem(at: got, to: dst)
        } catch {
            throw DownloadProblem(key: "download.cant_save", detail: error.localizedDescription)
        }
        return dst
    }

    static func sha256(of url: URL) throws -> String {
        let f = try FileHandle(forReadingFrom: url)
        defer { try? f.close() }
        var h = SHA256()
        while let chunk = try f.read(upToCount: 4 << 20), !chunk.isEmpty {
            h.update(data: chunk)
        }
        return h.finalize().map { String(format: "%02x", $0) }.joined()
    }

    /// Whether task is the one its file is coming by; none is, for a file
    /// no run of the app has asked for since it began (unclaimed), as one
    /// that ended while the app was gone.
    private func standing(_ task: URLSessionTask) -> (file: String, current: Bool, unclaimed: Bool)? {
        guard let file = task.taskDescription else { return nil }
        lock.lock()
        defer { lock.unlock() }
        return (file, current[file] == task.taskIdentifier, current[file] == nil)
    }

    /// The current transfer of file ended: its waiter told, or, with none
    /// yet, a failure kept for the fetch to come.
    private func ended(_ file: String, _ result: Result<URL, Error>) {
        lock.lock()
        current[file] = nil
        let j = jobs.removeValue(forKey: file)
        if j == nil, case .failure(let error) = result { failed[file] = error }
        lock.unlock()
        j?.done.resume(with: result)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData _: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite _: Int64) {
        guard let t = standing(downloadTask), t.current else { return }
        lock.lock()
        let j = jobs[t.file]
        lock.unlock()
        j?.progress(totalBytesWritten)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        // the file is URLSession's only until this returns: moved beside its
        // place now, checked by fetch; one that ended while the app was gone
        // too, for the fetch to come
        guard let t = standing(downloadTask), t.current || t.unclaimed else { return }
        // a transfer gone on from resume data ends with 206, its rest
        if let http = downloadTask.response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            if t.current {
                ended(t.file, .failure(DownloadProblem(key: http.statusCode == 404 ? "download.not_found" : "download.not_answering",
                                                       detail: "HTTP \(http.statusCode)")))
            }
            return
        }
        let held = Downloader.held(t.file)
        do {
            try? FileManager.default.removeItem(at: held)
            try FileManager.default.moveItem(at: location, to: held)
            if t.current { ended(t.file, .success(held)) }
        } catch {
            if t.current { ended(t.file, .failure(error)) }
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let error, let t = standing(task), t.current || t.unclaimed else { return }
        if let data = (error as NSError).userInfo[NSURLSessionDownloadTaskResumeData] as? Data {
            try? data.write(to: Downloader.kept(t.file))
        }
        if t.current { ended(t.file, .failure(error)) }
    }

    func urlSessionDidFinishEvents(forBackgroundURLSession session: URLSession) {
        DispatchQueue.main.async {
            let done = self.wakeDone
            self.wakeDone = nil
            done?()
        }
    }
}

/// How a model's download stands (download.rs, State; Android's Download).
struct DownloadState {
    enum Step { case downloading, checking, paused, failed, done, cancelled }

    let name: String
    var step: Step
    var got: Int64
    var total: Int64
    /// bytes a second over the last few seconds; 0 until known
    var speed: Double = 0
    var problem: DownloadProblem?
    /// a failure it is trying again after, by itself
    var retry = false
    /// after a failure, trying again now: the failure stands until bytes come
    var trying = false

    /// whether it goes on: getting bytes, checking them, or waiting to try again
    var going: Bool { step == .downloading || step == .checking || (step == .failed && retry) }
}

/// A catalog model's download in this device's edition (Android's
/// Downloads, the desktop's download.rs): its files on their way at once,
/// each checked as it comes, the Neural Engine edition's encoder unpacked
/// once it checks; a failure of the network tried again by itself, 2 s
/// after and doubling to 30, and a pause that keeps what came. Once it is
/// all here the model is in use, the toast says so, and its engine is
/// prepared, once the app is on the screen. One at a time. The model being
/// downloaded is remembered, and a later run of the app goes on with it
/// (resumeAfterLaunch), its transfers having gone on without it.
@MainActor @Observable
final class Downloads {
    static let shared = Downloads()

    /// the download under way, paused or failed, for the screen; nil once done or cancelled
    private(set) var state: DownloadState?
    private var work: Task<Void, Never>?
    private var wait: Task<Void, Never>?
    private var pausing = false
    /// the bytes each of the download's files has so far
    private var got: [String: Int64] = [:]
    private var speedometer = Speed()

    var running: Bool { work != nil }

    /// The bytes a stopped download of e kept: its files here, and what the
    /// others' transfers kept.
    func kept(_ e: CatalogEntry) -> Int64 {
        if state?.name == e.name, let s = state { return s.got }
        let parts = Editions.preferred(e) == .cpu ? [e.pkf] : [e.coreml.slim, e.coreml.encoder]
        let n = parts.reduce(Int64(0)) { $0 + (Store.have($1) ? $1.bytes : Downloader.shared.kept($1)) }
        return Store.have(e, Editions.preferred(e)) ? 0 : n
    }

    /// o's edition downloaded, or its download gone on with; one waiting to
    /// try again tries now.
    func start(_ o: Offer) {
        if running {
            wait?.cancel()
            return
        }
        pausing = false
        UserDefaults.standard.set(o.name, forKey: "downloading")
        UserDefaults.standard.set(false, forKey: "downloadPaused")
        state = DownloadState(name: o.name, step: .downloading, got: o.partial, total: o.bytes)
        work = Task { await run(o) }
    }

    /// The download a run before began and the person didn't pause, gone on
    /// with: its transfers went on while the app was away, or ended.
    func resumeAfterLaunch() {
        let d = UserDefaults.standard
        guard state == nil, !d.bool(forKey: "downloadPaused"), let name = d.string(forKey: "downloading"),
              let o = Models.shared.offers.first(where: { $0.name == name }) else { return }
        if Store.have(o.entry, o.edition) {
            d.removeObject(forKey: "downloading")
            return
        }
        start(o)
    }

    func retry() {
        if running {
            wait?.cancel()
            state?.trying = true
            return
        }
        if let name = state?.name, let o = Models.shared.offers.first(where: { $0.name == name }) {
            start(o)
        }
    }

    func pause() {
        pausing = true
        UserDefaults.standard.set(true, forKey: "downloadPaused")
        wait?.cancel()
        Downloader.shared.pause()
    }

    /// Stopped, and what it had deleted.
    func cancel() {
        guard let name = state?.name else { return }
        pausing = true
        work?.cancel()
        wait?.cancel()
        Downloader.shared.stop()
        forget(name)
        state = nil
        Models.shared.refresh()
    }

    /// What a stopped download of name kept, let go (Manage models' delete).
    func forget(_ name: String) {
        if UserDefaults.standard.string(forKey: "downloading") == name {
            UserDefaults.standard.removeObject(forKey: "downloading")
            UserDefaults.standard.removeObject(forKey: "downloadPaused")
        }
        guard let e = Models.shared.entry(name) else { return }
        for p in [e.pkf, e.coreml.slim, e.coreml.encoder] { Downloader.shared.forget(p) }
    }

    private func run(_ o: Offer) async {
        defer { work = nil }
        let e = o.entry
        let parts = o.edition == .cpu ? [e.pkf] : [e.coreml.slim, e.coreml.encoder]
        let here = { (p: Part) in p.file == e.coreml.encoder.file ? Store.haveEncoder(e) : Store.have(p) }
        got = [:]
        for p in parts {
            got[p.file] = here(p) ? p.bytes : Downloader.shared.kept(p)
        }
        var delay: Double = 2
        speedometer = Speed()
        for p in parts where !here(p) {
            while true {
                do {
                    await Downloader.shared.begin(parts.filter { !here($0) })
                    let url = try await Downloader.shared.fetch(p, progress: { n in
                        Task { @MainActor in self.heard(p, n) }
                    }, checking: {
                        Task { @MainActor in self.state?.step = .checking }
                    })
                    // iOS's time to finish, where the transfer ended while the app was away
                    let time = BackgroundTime("model " + p.file)
                    defer { time.end() }
                    if p.file == e.coreml.encoder.file {
                        state?.step = .checking
                        try await Task.detached(priority: .userInitiated) { try Unpack.encoder(e, from: url) }.value
                    }
                    got[p.file] = p.bytes
                    delay = 2
                    break
                } catch {
                    if pausing || Task.isCancelled {
                        if state != nil { state?.step = .paused; state?.speed = 0 }
                        return
                    }
                    let (problem, retry) = Downloads.problem(error)
                    state?.step = .failed
                    state?.problem = problem
                    state?.retry = retry
                    state?.trying = false
                    state?.speed = 0
                    guard retry else { return }
                    let seconds = delay
                    let sleep = Task { _ = try? await Task.sleep(for: .seconds(seconds)) }
                    wait = sleep
                    await sleep.value
                    wait = nil
                    if pausing {
                        state?.step = .paused
                        return
                    }
                    delay = min(delay * 2, 30)
                }
            }
        }
        UserDefaults.standard.removeObject(forKey: "downloading")
        state = nil
        Models.shared.use(o.name)
        Models.shared.news = L("models_ready", label(o.key))
        // Core ML's compile, on the screen, where it says what it is doing
        // (Root, back from the background)
        if UIApplication.shared.applicationState != .background { Transcriber.shared.prepare() }
    }

    /// A file's bytes so far, and the download's with them.
    private func heard(_ p: Part, _ n: Int64) {
        guard var s = state, !pausing else { return }
        got[p.file] = n
        let all = got.values.reduce(0, +)
        s.got = all
        s.speed = speedometer.add(all)
        s.step = .downloading
        s.problem = nil
        s.retry = false
        s.trying = false
        state = s
    }

    /// An error's problem, and whether to try again by itself: the network's
    /// failures are tried again, as the desktop's download tries them.
    nonisolated private static func problem(_ error: Error) -> (DownloadProblem, Bool) {
        if let p = error as? DownloadProblem { return (p, false) }
        if let u = error as? URLError {
            switch u.code {
            case .notConnectedToInternet, .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed,
                 .internationalRoamingOff, .dataNotAllowed:
                return (DownloadProblem(key: "download.cant_reach", detail: u.localizedDescription), true)
            case .networkConnectionLost:
                return (DownloadProblem(key: "download.connection_lost", detail: u.localizedDescription), true)
            case .timedOut:
                return (DownloadProblem(key: "download.not_answering", detail: u.localizedDescription), true)
            default:
                return (DownloadProblem(key: "download.cant_save", detail: u.localizedDescription), false)
            }
        }
        let ns = error as NSError
        if ns.domain == NSCocoaErrorDomain && ns.code == NSFileWriteOutOfSpaceError {
            return (DownloadProblem(key: "download.disk_full"), false)
        }
        return (DownloadProblem(key: "download.cant_save", detail: error.localizedDescription), false)
    }
}

/// Time iOS gives the app to finish what it is doing once it is in the
/// background: a model checked and unpacked after its transfer ended while
/// the app was away. iOS ends it when its time is up, and the work goes on
/// when the app is back.
@MainActor final class BackgroundTime {
    private var id = UIBackgroundTaskIdentifier.invalid

    init(_ name: String) {
        id = UIApplication.shared.beginBackgroundTask(withName: name) { [weak self] in
            MainActor.assumeIsolated { self?.end() }
        }
    }

    func end() {
        guard id != .invalid else { return }
        UIApplication.shared.endBackgroundTask(id)
        id = .invalid
    }
}

/// Bytes a second over the last few seconds, as the desktop's download
/// smooths it.
private struct Speed {
    private var points: [(Date, Int64)] = []

    mutating func add(_ got: Int64) -> Double {
        let now = Date()
        points.append((now, got))
        points.removeAll { now.timeIntervalSince($0.0) > 3 }
        guard let first = points.first, now.timeIntervalSince(first.0) > 0.5 else { return 0 }
        return Double(got - first.1) / now.timeIntervalSince(first.0)
    }
}
