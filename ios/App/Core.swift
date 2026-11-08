import Foundation
import PktitoCore

/// What pktito's core says went wrong, in its own words (English), and
/// where it has one, its catalog key and what said so, for the app's own
/// words (Words.swift, words).
struct CoreError: LocalizedError {
    let errorDescription: String?
    var key: String?
    var detail = ""
    init(_ message: String, key: String? = nil, detail: String = "") {
        errorDescription = message
        self.key = key
        self.detail = detail
    }
}

/// The core's answer to one call (src-tauri/src/ios.rs): its JSON decoded as
/// T, or the core's error. The string is the core's to have been given and
/// the caller's to free, here.
private func take<T: Decodable>(_ s: UnsafeMutablePointer<CChar>?, as: T.Type) throws -> T {
    guard let s else { throw CoreError("pktito's core gave no answer") }
    defer { pkapp_free_string(s) }
    let data = Data(bytes: s, count: strlen(s))
    if let failed = try? JSONDecoder().decode(Failed.self, from: data) {
        throw CoreError(failed.error, key: failed.key, detail: failed.detail ?? "")
    }
    return try JSONDecoder().decode(T.self, from: data)
}

private struct Failed: Decodable {
    let error: String
    let key: String?
    let detail: String?
}
private struct Nothing: Decodable {}

/// A word of a transcript, its times in seconds.
struct Word: Codable, Hashable {
    let start: Double
    let end: Double
    let text: String
}

/// A sentence, or 20 s of one (transcribe.rs), with its words.
struct Segment: Codable, Hashable {
    let start: Double
    let end: Double
    let text: String
    let words: [Word]
}

/// What a transcription gives: the core's Transcript, which format() and
/// captions() take back as it is.
struct Transcript: Codable {
    let duration: Double
    /// false where stop() ended it before the end
    let complete: Bool
    let model: String
    let segments: [Segment]
}

enum Core {
    /// The engine's code for this processor, for About's version line.
    static var backend: String {
        guard let s = pkapp_backend() else { return "" }
        defer { pkapp_free_string(s) }
        return String(cString: s)
    }

    /// The models models.pktito.net serves, each as the core sees it in the
    /// models folder for this device (editions.rs, view): any thread.
    static func catalog() throws -> [ModelView] {
        try take(pkapp_catalog(Store.models.path, Editions.device), as: [ModelView].self)
    }

    /// Once a launch, off the main thread: files from before 0.9.6 given
    /// their records, swaps cut short finished (editions.rs, adopt).
    static func adopt() { pkapp_adopt(Store.models.path) }

    /// name's file that URLSession brought, at got: checked against the
    /// catalog's SHA-256 and put where it lands, with its record
    /// (model.rs, land): in place, or beside the file it replaces until
    /// settle. One that doesn't check is deleted, and the problem says why.
    static func land(_ name: String, file: String, got: URL) throws -> URL {
        struct Landed: Decodable { let path: String }
        do {
            let l = try take(pkapp_land(Store.models.path, name, file, got.path), as: Landed.self)
            return URL(fileURLWithPath: l.path)
        } catch let e as CoreError {
            guard let key = e.key else { throw e }
            throw DownloadProblem(key: key, detail: e.detail)
        }
    }

    /// Whether name has a download that checked and waits to be put in place.
    static func pending(_ name: String) -> Bool { pkapp_pending(Store.models.path, name) }

    /// name's downloads that checked put in place over what they replace, a
    /// Core ML encoder unpacked over the one before: only while no engine
    /// holds name's files.
    static func settle(_ name: String) throws {
        _ = try take(pkapp_settle(Store.models.path, name), as: Nothing.self)
    }

    /// What replaces nothing put in place: a Core ML encoder unpacked where
    /// none is, right after its download, whatever is loaded.
    static func settleNew(_ name: String) throws {
        _ = try take(pkapp_settle_new(Store.models.path, name), as: Nothing.self)
    }

    /// The Neural Engine refused name's edition for why: its encoder deleted,
    /// and the refusal kept for this system and build where the encoder was
    /// the catalog's. Whether it was, so that the refusal holds.
    static func refuse(_ name: String, edition: String, why: String) throws -> Bool {
        struct Refused: Decodable { let refused: Bool }
        return try take(
            pkapp_refuse(Store.models.path, name, edition, Editions.device, why),
            as: Refused.self
        ).refused
    }

    /// A transcription finished on name's edition with the model file at
    /// model: whether the whole .pkf went, the Neural Engine's edition
    /// having run from its own slim file (editions.rs, ran).
    static func ran(_ name: String, edition: String, model: URL) -> Bool {
        pkapp_ran(Store.models.path, name, edition, model.path)
    }

    /// Every file of name's editions, their records and replacements
    /// waiting, and its refusals.
    static func delete(_ name: String) { pkapp_delete(Store.models.path, name) }

    /// transcript as a file of the kind ext names: txt, md, srt, vtt, json.
    static func format(_ transcript: Transcript, ext: String, name: String, times: Bool) throws
        -> String
    {
        struct Text: Decodable { let text: String }
        let json = String(decoding: try JSONEncoder().encode(transcript), as: UTF8.self)
        return try take(pkapp_format(json, ext, name, times), as: Text.self).text
    }

    /// What a model file the core refused for why means, as the strings'
    /// key ("model_file.not_model").
    static func trouble(_ why: String) -> String {
        struct Key: Decodable { let key: String }
        return (try? take(pkapp_trouble(why), as: Key.self).key) ?? "model_file.cant_open"
    }
}

/// A model loaded, and its transcriber: one transcription at a time, on
/// the thread that calls transcribe.
final class Engine: @unchecked Sendable {
    let name: String
    /// false for a slim model file, whose encoder is only a Core ML one
    let hasEncoder: Bool
    let threads: Int
    private let handle: UInt64

    init(file: URL) throws {
        struct Loaded: Decodable {
            let handle: UInt64
            let name: String
            let encoder: Bool
            let threads: Int
        }
        let l = try file.withUnsafeFileSystemRepresentation {
            try take(pkapp_load($0), as: Loaded.self)
        }
        handle = l.handle
        name = l.name
        hasEncoder = l.encoder
        threads = l.threads
    }

    deinit { pkapp_free(handle) }

    /// The encoder on the Neural Engine: the compiled Core ML model at
    /// encoder (NAME.mlmodelc), loaded and run once, which is where a device
    /// whose Neural Engine can't run it throws, with Core ML's reason; nil
    /// for the model's own encoder again.
    func useNeuralEngine(_ encoder: URL?) throws {
        if let encoder {
            _ = try encoder.withUnsafeFileSystemRepresentation {
                try take(pkapp_use_ane(handle, $0), as: Nothing.self)
            }
        } else {
            _ = try take(pkapp_use_ane(handle, nil), as: Nothing.self)
        }
    }

    /// What a transcription hears from the core as it goes, on its thread.
    final class Progress: @unchecked Sendable {
        /// once the recording is read: its length, a video's frame rate (or
        /// 0) and its loudness, audio::PEAKS slices of it
        var audio: (Double, Double, [UInt8]) -> Void = { _, _, _ in }
        /// the frame rate and the loudness audio was given, kept
        private(set) var fps = 0.0
        private(set) var peaks: [UInt8] = []

        fileprivate func heard(_ seconds: Double, _ fps: Double, _ peaks: [UInt8]) {
            self.fps = fps
            self.peaks = peaks
            audio(seconds, fps, peaks)
        }
        /// after each chunk: the fraction done, and the segments just done
        var update: (Double, [Segment]) -> Void = { _, _ in }
    }

    /// The recording at file read and transcribed, as pktito transcribes
    /// one on every platform.
    func transcribe(_ file: URL, name: String, progress: Progress) throws -> Transcript {
        let ctx = Unmanaged.passRetained(progress)
        defer { ctx.release() }
        let answer = file.withUnsafeFileSystemRepresentation { path in
            pkapp_transcribe(
                handle, path, name, ctx.toOpaque(),
                { ctx, seconds, fps, peaks, n in
                    let p = Unmanaged<Progress>.fromOpaque(ctx!).takeUnretainedValue()
                    p.heard(
                        seconds, fps,
                        peaks.map { Array(UnsafeBufferPointer(start: $0, count: n)) } ?? [])
                },
                { ctx, done, segments in
                    let p = Unmanaged<Progress>.fromOpaque(ctx!).takeUnretainedValue()
                    let segs = segments.flatMap {
                        try? JSONDecoder().decode(
                            [Segment].self, from: Data(bytes: $0, count: strlen($0)))
                    }
                    p.update(done, segs ?? [])
                })
        }
        return try take(answer, as: Transcript.self)
    }

    /// The transcription running ends before its next chunk.
    static func stop() { pkapp_stop() }
}
