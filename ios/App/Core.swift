import Foundation
import PktitoCore

/// What pktito's core says went wrong, in its own words (English).
struct CoreError: LocalizedError {
    let errorDescription: String?
    init(_ message: String) { errorDescription = message }
}

/// The core's answer to one call (src-tauri/src/ios.rs): its JSON decoded as
/// T, or the core's error. The string is the core's to have been given and
/// the caller's to free, here.
private func take<T: Decodable>(_ s: UnsafeMutablePointer<CChar>?, as: T.Type) throws -> T {
    guard let s else { throw CoreError("pktito's core gave no answer") }
    defer { pkapp_free_string(s) }
    let data = Data(bytes: s, count: strlen(s))
    if let failed = try? JSONDecoder().decode(Failed.self, from: data) {
        throw CoreError(failed.error)
    }
    return try JSONDecoder().decode(T.self, from: data)
}

private struct Failed: Decodable { let error: String }
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

    /// The models models.pktito.net serves (model.rs).
    static func catalog() throws -> [CatalogEntry] {
        try take(pkapp_catalog(), as: [CatalogEntry].self)
    }

    /// transcript as a file of the kind ext names: txt, md, srt, vtt, json.
    static func format(_ transcript: Transcript, ext: String, name: String, times: Bool) throws -> String {
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
        let l = try file.withUnsafeFileSystemRepresentation { try take(pkapp_load($0), as: Loaded.self) }
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
            _ = try encoder.withUnsafeFileSystemRepresentation { try take(pkapp_use_ane(handle, $0), as: Nothing.self) }
        } else {
            _ = try take(pkapp_use_ane(handle, nil), as: Nothing.self)
        }
    }

    /// What a transcription hears from the core as it goes, on its thread.
    final class Progress {
        /// once the recording is read: its length, a video's frame rate (or
        /// 0) and its loudness, audio::PEAKS slices of it
        var audio: (Double, Double, [UInt8]) -> Void = { _, _, _ in }
        /// after each chunk: the fraction done, and the segments just done
        var update: (Double, [Segment]) -> Void = { _, _ in }
    }

    /// The recording at file read and transcribed, as pktito transcribes
    /// one on every platform.
    func transcribe(_ file: URL, name: String, progress: Progress) throws -> Transcript {
        let ctx = Unmanaged.passRetained(progress)
        defer { ctx.release() }
        let answer = file.withUnsafeFileSystemRepresentation { path in
            pkapp_transcribe(handle, path, name, ctx.toOpaque(), { ctx, seconds, fps, peaks, n in
                let p = Unmanaged<Progress>.fromOpaque(ctx!).takeUnretainedValue()
                p.audio(seconds, fps, peaks.map { Array(UnsafeBufferPointer(start: $0, count: n)) } ?? [])
            }, { ctx, done, segments in
                let p = Unmanaged<Progress>.fromOpaque(ctx!).takeUnretainedValue()
                let segs = segments.flatMap { try? JSONDecoder().decode([Segment].self, from: Data(bytes: $0, count: strlen($0))) }
                p.update(done, segs ?? [])
            })
        }
        return try take(answer, as: Transcript.self)
    }

    /// The transcription running ends before its next chunk.
    static func stop() { pkapp_stop() }
}
