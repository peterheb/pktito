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

/// A sentence, or 20 s of one (transcribe.rs), with its words; with
/// speakers, whose it is, numbered from 1 by first word (Speakers), left
/// out of the JSON without them.
struct Segment: Codable, Hashable {
    let start: Double
    let end: Double
    let text: String
    let words: [Word]
    var speaker: Int? = nil

    /// The same sentence, said by speaker.
    func by(_ speaker: Int?) -> Segment {
        Segment(start: start, end: end, text: text, words: words, speaker: speaker)
    }
}

/// A stretch of one speaker's speech, in seconds, for the speaker bar:
/// they overlap where two talk (transcribe.rs, Stretch).
struct Stretch: Codable, Hashable {
    let start: Double
    let end: Double
    let speaker: Int
}

/// What a transcription gives: the core's Transcript, which format() takes
/// back as it is. With speakers, the names given in place of Speaker n,
/// speaker n's at n - 1 ("" keeps Speaker n), each speaker's stretches for
/// the bar, and the diarizer that found them, each left out of the JSON
/// without them, as ios.rs's are, and hidden where its own switch hid them.
struct Transcript: Codable {
    let duration: Double
    /// false where stop() ended it before the end
    let complete: Bool
    let model: String
    var segments: [Segment]
    var names: [String] = []
    var stretches: [Stretch] = []
    var diarizer = ""
    /// its speakers hidden by the Speakers switch on it (recent.rs, hidden)
    var hidden = false
}

extension Transcript {
    private enum Keys: String, CodingKey {
        case duration, complete, model, segments, names, stretches, diarizer, hidden
    }

    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: Keys.self)
        duration = try c.decode(Double.self, forKey: .duration)
        complete = try c.decode(Bool.self, forKey: .complete)
        model = try c.decode(String.self, forKey: .model)
        segments = try c.decode([Segment].self, forKey: .segments)
        names = try c.decodeIfPresent([String].self, forKey: .names) ?? []
        stretches = try c.decodeIfPresent([Stretch].self, forKey: .stretches) ?? []
        diarizer = try c.decodeIfPresent(String.self, forKey: .diarizer) ?? ""
        hidden = try c.decodeIfPresent(Bool.self, forKey: .hidden) ?? false
    }

    func encode(to e: Encoder) throws {
        var c = e.container(keyedBy: Keys.self)
        try c.encode(duration, forKey: .duration)
        try c.encode(complete, forKey: .complete)
        try c.encode(model, forKey: .model)
        try c.encode(segments, forKey: .segments)
        if !names.isEmpty { try c.encode(names, forKey: .names) }
        if !stretches.isEmpty { try c.encode(stretches, forKey: .stretches) }
        if !diarizer.isEmpty { try c.encode(diarizer, forKey: .diarizer) }
        if hidden { try c.encode(hidden, forKey: .hidden) }
    }
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

    /// transcript as a file of the kind ext names: txt, md, srt, vtt, json;
    /// with speakers, each turn's name, where the switch shows them.
    static func format(
        _ transcript: Transcript, ext: String, name: String, times: Bool, speakers: Bool
    ) throws -> String {
        struct Text: Decodable { let text: String }
        let json = String(decoding: try JSONEncoder().encode(transcript), as: UTF8.self)
        return try take(pkapp_format(json, ext, name, times, speakers), as: Text.self).text
    }

    /// The speaker model this build offers, as the models folder has it;
    /// nil where it offers none (ios.rs, pkapp_diarizer).
    static func diarizer() throws -> Speakers.Offer? {
        struct Answer: Decodable { let offer: Speakers.Offer? }
        return try take(pkapp_diarizer(Store.models.path), as: Answer.self).offer
    }

    /// The speaker model's file that URLSession brought, at got, checked
    /// and put in place, as land puts a model's.
    static func landDiarizer(got: URL) throws -> URL {
        struct Landed: Decodable { let path: String }
        do {
            let l = try take(pkapp_land_diarizer(Store.models.path, got.path), as: Landed.self)
            return URL(fileURLWithPath: l.path)
        } catch let e as CoreError {
            guard let key = e.key else { throw e }
            throw DownloadProblem(key: key, detail: e.detail)
        }
    }

    /// The speaker model's files deleted, or with part what a download of
    /// it kept. Nothing may have it loaded (Speakers.with).
    static func deleteDiarizer(part: Bool) throws {
        _ = try take(pkapp_delete_diarizer(Store.models.path, part), as: Nothing.self)
    }

    /// A kept transcript's speakers as transcript has them now, found or
    /// changed on the screen (recent.rs, replace). Off the main thread.
    static func keepSpeakers(_ id: String, _ transcript: Transcript) throws {
        let json = String(decoding: try JSONEncoder().encode(transcript), as: UTF8.self)
        _ = try take(pkapp_keep_speakers(Recents.data.path, id, json), as: Nothing.self)
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
    /// one on every platform; with diar, who said what too, heard in the
    /// same pass.
    func transcribe(_ file: URL, name: String, diar: Diarizer?, progress: Progress) throws
        -> Transcript
    {
        let ctx = Unmanaged.passRetained(progress)
        defer { ctx.release() }
        let answer = file.withUnsafeFileSystemRepresentation { path in
            pkapp_transcribe(
                handle, diar?.handle ?? 0, path, name, ctx.toOpaque(),
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
    /// the transcription running goes on without its speakers from its next chunk
    static func quiet() { pkapp_quiet() }
}

/// The speaker model loaded, and a diarizer for it, for one transcription
/// or one find at a time (Speakers.with): loading its 100 MB takes a moment
/// beside a transcription's seconds, and the phone keeps none of it between
/// them.
final class Diarizer: @unchecked Sendable {
    fileprivate let handle: UInt64

    init(file: URL) throws {
        struct Loaded: Decodable { let handle: UInt64 }
        handle = try file.withUnsafeFileSystemRepresentation {
            try take(pkapp_load_diarizer($0), as: Loaded.self)
        }.handle
    }

    deinit { pkapp_free_diarizer(handle) }

    /// What a find hears as it goes, on its thread: the fraction of the
    /// recording heard, and the bar's stretches ended since the last and
    /// those going.
    final class Heard: @unchecked Sendable {
        var done: (Double, [Stretch], [Stretch]) -> Void = { _, _, _ in }
    }

    /// Who said what in transcript, a finished one, from its recording at
    /// file, without transcribing it again (transcribe.rs, find): the
    /// transcript with its speakers and no names yet. Seconds where a
    /// transcription takes a minute.
    func find(_ file: URL, name: String, transcript: Transcript, heard: Heard) throws
        -> Transcript
    {
        struct Bar: Decodable {
            let closed: [Stretch]
            let open: [Stretch]
        }
        let json = String(decoding: try JSONEncoder().encode(transcript), as: UTF8.self)
        let ctx = Unmanaged.passRetained(heard)
        defer { ctx.release() }
        let answer = file.withUnsafeFileSystemRepresentation { path in
            pkapp_find_speakers(handle, path, name, json, ctx.toOpaque()) { ctx, done, bar in
                let h = Unmanaged<Heard>.fromOpaque(ctx!).takeUnretainedValue()
                let b = bar.flatMap {
                    try? JSONDecoder().decode(Bar.self, from: Data(bytes: $0, count: strlen($0)))
                }
                h.done(done, b?.closed ?? [], b?.open ?? [])
            }
        }
        return try take(answer, as: Transcript.self)
    }
}
