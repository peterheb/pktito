import CryptoKit
import Foundation

/// What went wrong with a download, by the desktop's key for its words
/// (strings.rs, Problem; Words.swift, words): "download.damaged", ...
struct DownloadProblem: LocalizedError {
    let key: String
    var detail = ""
    var errorDescription: String? { words(key, host: "models.pktito.net") + (detail.isEmpty ? "" : " (\(detail))") }
}

/// A model file's download from models.pktito.net: URLSession's, which
/// goes on from what it has when it is asked again (its resume data), and
/// then the file's SHA-256 checked against the catalog's before it is
/// renamed into place, as the core's download does on every other platform
/// (model.rs). A file that doesn't check is deleted.
final class Downloader: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    static let shared = Downloader()

    private lazy var session: URLSession = {
        let c = URLSessionConfiguration.default
        c.httpAdditionalHeaders = ["User-Agent": Downloader.userAgent]
        c.timeoutIntervalForRequest = 30
        return URLSession(configuration: c, delegate: self, delegateQueue: nil)
    }()

    /// As the other apps say who they are: "pktito/0.1 (iOS; arm64)"
    static let userAgent: String = {
        let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
        return "pktito/\(v) (iOS; arm64)"
    }()

    private struct Job {
        let part: Part
        let progress: @Sendable (Int64, Int64) -> Void
        let done: CheckedContinuation<URL, Error>
    }

    private let lock = NSLock()
    private var jobs: [Int: Job] = [:]
    /// what a stopped download kept, by file, to go on from
    private var resume: [String: Data] = [:]

    /// p downloaded and checked, its path once it is in place; progress
    /// hears the bytes so far and the whole, on URLSession's queue.
    func fetch(_ p: Part, progress: @escaping @Sendable (Int64, Int64) -> Void,
               checking: @escaping @Sendable () -> Void = {}) async throws -> URL {
        if Store.have(p) {
            return Store.file(p)
        }
        let got: URL = try await withCheckedThrowingContinuation { done in
            lock.lock()
            let task: URLSessionDownloadTask
            if let data = resume.removeValue(forKey: p.file) {
                task = session.downloadTask(withResumeData: data)
            } else {
                task = session.downloadTask(with: p.url)
            }
            jobs[task.taskIdentifier] = Job(part: p, progress: progress, done: done)
            lock.unlock()
            task.resume()
        }
        checking()
        return try await Task.detached(priority: .utility) { try Downloader.check(got, p) }.value
    }

    /// Whether a stopped download of p kept anything to go on from.
    func resumable(_ p: Part) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return resume[p.file] != nil
    }

    /// What a stopped download of p kept, let go.
    func forget(_ p: Part) {
        lock.lock()
        resume[p.file] = nil
        lock.unlock()
    }

    /// Every download running stops, keeping what it has for the next fetch.
    func pause() {
        session.getTasksWithCompletionHandler { _, _, downloads in
            for d in downloads {
                d.cancel(byProducingResumeData: { _ in })   // the data comes to didCompleteWithError
            }
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

    private func job(_ task: URLSessionTask) -> Job? {
        lock.lock()
        defer { lock.unlock() }
        return jobs[task.taskIdentifier]
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData _: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        guard let j = job(downloadTask) else { return }
        j.progress(totalBytesWritten, totalBytesExpectedToWrite > 0 ? totalBytesExpectedToWrite : j.part.bytes)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        // the file is URLSession's only until this returns: moved beside its
        // place now, checked by fetch
        guard let j = job(downloadTask) else { return }
        let held = Store.models.appendingPathComponent(j.part.file + ".download")
        lock.lock()
        jobs[downloadTask.taskIdentifier] = nil
        lock.unlock()
        do {
            if let http = downloadTask.response as? HTTPURLResponse, http.statusCode != 200 {
                throw DownloadProblem(key: http.statusCode == 404 ? "download.not_found" : "download.not_answering",
                                      detail: "HTTP \(http.statusCode)")
            }
            try? FileManager.default.removeItem(at: held)
            try FileManager.default.moveItem(at: location, to: held)
            j.done.resume(returning: held)
        } catch {
            j.done.resume(throwing: error)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let error, let j = job(task) else { return }
        lock.lock()
        jobs[task.taskIdentifier] = nil
        if let data = (error as NSError).userInfo[NSURLSessionDownloadTaskResumeData] as? Data {
            resume[j.part.file] = data
        }
        lock.unlock()
        j.done.resume(throwing: error)
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
/// Downloads, the desktop's download.rs): its files one after another, the
/// Neural Engine edition's encoder unpacked once it checks, a failure of
/// the network tried again by itself, 2 s after and doubling to 30, and a
/// pause that keeps what came. Once it is all here the model is in use,
/// the toast says so, and its engine is prepared. One at a time.
@MainActor @Observable
final class Downloads {
    static let shared = Downloads()

    /// the download under way, paused or failed, for the screen; nil once done or cancelled
    private(set) var state: DownloadState?
    private var work: Task<Void, Never>?
    private var wait: Task<Void, Never>?
    private var pausing = false
    /// the bytes each stopped download kept, by model
    private var keptBytes: [String: Int64] = [:]
    private var speedometer = Speed()

    var running: Bool { work != nil }

    func kept(_ name: String) -> Int64 { keptBytes[name] ?? 0 }

    /// o's edition downloaded, or its download gone on with; one waiting to
    /// try again tries now.
    func start(_ o: Offer) {
        if running {
            wait?.cancel()
            return
        }
        pausing = false
        state = DownloadState(name: o.name, step: .downloading, got: o.partial, total: o.bytes)
        work = Task { await run(o) }
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
        wait?.cancel()
        Downloader.shared.pause()
    }

    /// Stopped, and what it had deleted.
    func cancel() {
        guard let name = state?.name else { return }
        pausing = true
        work?.cancel()
        wait?.cancel()
        Downloader.shared.pause()
        forget(name)
        state = nil
        Models.shared.refresh()
    }

    /// What a stopped download of name kept, let go (Manage models' delete).
    func forget(_ name: String) {
        keptBytes[name] = nil
        if let e = Models.shared.entry(name) {
            for p in [e.pkf, e.coreml.slim, e.coreml.encoder] { Downloader.shared.forget(p) }
        }
    }

    private func run(_ o: Offer) async {
        defer { work = nil }
        let e = o.entry
        let parts = o.edition == .cpu ? [e.pkf] : [e.coreml.slim, e.coreml.encoder]
        var before: Int64 = 0
        var delay: Double = 2
        speedometer = Speed()
        for p in parts {
            let isEncoder = p.file == e.coreml.encoder.file
            if isEncoder ? Store.haveEncoder(e) : Store.have(p) {
                before += p.bytes
                continue
            }
            while true {
                do {
                    let base = before
                    let url = try await Downloader.shared.fetch(p, progress: { got, _ in
                        Task { @MainActor in
                            guard var s = self.state, !self.pausing else { return }
                            s.got = base + got
                            s.speed = self.speedometer.add(base + got)
                            s.step = .downloading
                            s.problem = nil
                            s.retry = false
                            s.trying = false
                            self.state = s
                            self.keptBytes[o.name] = base + got
                        }
                    }, checking: {
                        Task { @MainActor in self.state?.step = .checking }
                    })
                    if isEncoder {
                        state?.step = .checking
                        try await Task.detached(priority: .userInitiated) { try Unpack.encoder(e, from: url) }.value
                    }
                    before += p.bytes
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
        keptBytes[o.name] = nil
        state = nil
        Models.shared.use(o.name)
        Models.shared.news = L("models_ready", label(o.key))
        Transcriber.shared.prepare()
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
