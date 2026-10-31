import Foundation

/// A file models.pktito.net serves beside a model's .pkf (model.rs, Part).
struct Part: Decodable {
    let file: String
    let bytes: Int64
    let sha256: String
    let url: URL
}

/// A model of the catalog (model.rs, Entry), with the addresses of its two
/// editions: the .pkf, whose encoder is pktito's own on the CPU, and the
/// Core ML one, a slim .pkf and the encoder for the Neural Engine.
struct CatalogEntry: Decodable, Identifiable {
    let name: String
    let key: String
    let label: String
    let about: String
    let languages: [String]
    let accuracy: String
    let bytes: Int64
    let sha256: String
    let url: URL
    let coreml: Coreml

    struct Coreml: Decodable {
        let slim: Part
        let encoder: Part
    }

    var id: String { name }

    /// The CPU edition's file, as the core names it everywhere
    var pkf: Part { Part(file: name + ".pkf", bytes: bytes, sha256: sha256, url: url) }
}

/// Which of a model's two editions a transcriber runs.
enum Edition: String {
    /// the slim .pkf and the encoder on the Neural Engine
    case neuralEngine
    /// the whole .pkf, its encoder pktito's own on the CPU
    case cpu
}

/// Where the app keeps its models, and which of them are here.
enum Store {
    /// Application Support/models: kept out of backups, since a model can
    /// be downloaded again, as Android keeps them in no_backup
    static let models: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        var dir = base.appendingPathComponent("models", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        var v = URLResourceValues()
        v.isExcludedFromBackup = true
        try? dir.setResourceValues(v)
        return dir
    }()

    static func file(_ p: Part) -> URL { models.appendingPathComponent(p.file) }

    /// Where a model's Core ML encoder is unpacked to (Unpack.swift)
    static func encoder(_ e: CatalogEntry) -> URL {
        models.appendingPathComponent(e.name + "-encoder.mlmodelc", isDirectory: true)
    }

    /// A file is renamed into place only after its SHA-256 checks, so the
    /// size is enough to tell, as the core tells (model.rs, have).
    static func have(_ p: Part) -> Bool {
        let size = (try? FileManager.default.attributesOfItem(atPath: file(p).path)[.size] as? Int64) ?? nil
        return size == p.bytes
    }

    /// The encoder is renamed into place only once its archive unpacked whole.
    static func haveEncoder(_ e: CatalogEntry) -> Bool {
        FileManager.default.fileExists(atPath: encoder(e).appendingPathComponent("model.mil").path)
    }

    static func have(_ e: CatalogEntry, _ edition: Edition) -> Bool {
        switch edition {
        case .cpu: return have(e.pkf)
        case .neuralEngine: return have(e.coreml.slim) && haveEncoder(e)
        }
    }
}
