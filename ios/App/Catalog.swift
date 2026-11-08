import Foundation

/// A file models.pktito.net serves for a model (model.rs, Part), and where
/// an offer names it, its address.
struct Part: Decodable, Hashable {
    let file: String
    let bytes: Int64
    let sha256: String
    var url: URL?
}

/// One way to run a model (editions.rs, Edition): the CPU's, its whole .pkf,
/// or the Neural Engine's, the slim .pkf and the Core ML encoder.
struct EditionInfo: Decodable {
    struct Runtime: Decodable { let kind: String }
    let id: String
    let runtime: Runtime
    let model: Part
    let encoder: Part?
}

/// What runs a model here (editions.rs, Runs): its edition, the model file
/// the engine loads, and for the Neural Engine's the encoder beside it.
struct Runs: Decodable, Equatable {
    let edition: String
    let model: String
    let encoder: String?
    var neuralEngine: Bool { encoder != nil }
}

/// What Manage models and the setup screen offer for a model (editions.rs,
/// Offer): Download where nothing of it runs, Faster where the CPU's
/// edition runs and the Neural Engine's is chosen, Update where a file of
/// the edition that runs is an older build than the catalog's. Its files,
/// with their addresses, and their bytes. What a download of them that
/// stopped kept is URLSession's resume data here, which Downloads counts.
struct Get: Decodable {
    enum Kind: String, Decodable { case download, faster, update }
    let kind: Kind
    let bytes: Int64
    let parts: [Part]
}

/// A model the catalog lists, as the core sees it in the models folder for
/// this device (editions.rs, View; docs/EDITIONS.md), which decides for
/// every platform: the edition chosen, what runs, why the Neural Engine
/// refused the edition that fits best, and the offer.
struct ModelView: Decodable, Identifiable {
    let name: String
    /// the desktop's key for its words: "english", "multilingual"
    let key: String
    /// what it transcribes, as ISO 639-1 codes
    let languages: [String]
    /// where its error rate in each language is, for a model of several; "" for none
    let accuracy: String
    let editions: [EditionInfo]
    /// the edition chosen for this device: "coreml", or "cpu" where the
    /// Neural Engine refused it here
    let chosen: String
    /// what runs, nil for nothing here that does
    let runs: Runs?
    /// why the Neural Engine refused the edition that fits best, on this
    /// system and build, with this encoder; nil where it didn't
    let refused: String?
    let offer: Get?
    /// the bytes its files take here, where it runs; where nothing does,
    /// the chosen edition's, which its Download fetches
    let bytes: Int64

    var id: String { name }

    /// the CPU's edition's file, the whole .pkf
    var whole: Part {
        (editions.first { $0.runtime.kind == "cpu" } ?? editions[0]).model
    }

    /// the Neural Engine's encoder, the catalog's
    var encoder: Part? {
        editions.first { $0.runtime.kind == "coreml" }?.encoder
    }

    /// every file of its editions, each once
    var files: [Part] {
        var out: [Part] = []
        for p in editions.flatMap({ [$0.model, $0.encoder].compactMap { $0 } })
        where !out.contains(where: { $0.file == p.file }) {
            out.append(p)
        }
        return out
    }
}

/// Where the app keeps its models, which the core keeps by its rule
/// (editions.rs): Application Support/models.
enum Store {
    /// kept out of backups, since a model can be downloaded again, as
    /// Android keeps them in no_backup
    static let models: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[
            0]
        var dir = base.appendingPathComponent("models", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        var v = URLResourceValues()
        v.isExcludedFromBackup = true
        try? dir.setResourceValues(v)
        return dir
    }()

    static func file(_ name: String) -> URL { models.appendingPathComponent(name) }

    /// Where the SHA-256 a file was checked against is kept, beside it, as
    /// the core keeps it (model.rs, marker_of): its name with .sha256 after it
    static func marker(_ url: URL) -> URL {
        url.deletingLastPathComponent().appendingPathComponent(url.lastPathComponent + ".sha256")
    }
}
