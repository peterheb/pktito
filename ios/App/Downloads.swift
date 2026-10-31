import CryptoKit
import Foundation

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
    func fetch(_ p: Part, progress: @escaping @Sendable (Int64, Int64) -> Void) async throws -> URL {
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
        return try await Task.detached(priority: .utility) { try Downloader.check(got, p) }.value
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
            throw CoreError("\(p.file) arrived damaged (its SHA-256 isn't the published one), so it was deleted")
        }
        let dst = Store.file(p)
        try? FileManager.default.removeItem(at: dst)
        try FileManager.default.moveItem(at: got, to: dst)
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
                throw CoreError("models.pktito.net answered \(http.statusCode) for \(j.part.file)")
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
