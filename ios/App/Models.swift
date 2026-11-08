import CryptoKit
import Foundation

/// A yes or no that threads share: a stop asked for, read by the work's thread.
final class Flag: @unchecked Sendable {
    private let lock = NSLock()
    private var on = false

    var isSet: Bool {
        lock.lock()
        defer { lock.unlock() }
        return on
    }

    func set(_ v: Bool) {
        lock.lock()
        on = v
        lock.unlock()
    }
}

/// A catalog model as it stands here (Android's Offer): the core's view of
/// it (ModelView), and the bytes a download of its offer that stopped kept,
/// URLSession's resume data and files that came but aren't checked yet.
struct Offer: Identifiable {
    let entry: ModelView
    let partial: Int64

    var id: String { entry.name }
    var name: String { entry.name }
    var key: String { entry.key }
    /// whether something of it runs here
    var have: Bool { entry.runs != nil }

    private func of(_ kind: Get.Kind) -> Get? { entry.offer.flatMap { $0.kind == kind ? $0 : nil } }

    /// the bytes a first download fetches: the chosen edition's
    var bytes: Int64 { of(.download)?.bytes ?? entry.bytes }

    /// the Neural Engine's edition for a model here on the CPU (Manage
    /// models, Faster): the bytes of its files not here yet, the slim one and
    /// the encoder, which take the whole file's place once the Neural Engine
    /// has run a transcription (Core.ran); 0 where none is offered
    var faster: Int64 { of(.faster)?.bytes ?? 0 }

    /// the catalog's build of a file of the edition that runs, an older one
    /// here: its bytes; 0 where none is offered
    var update: Int64 { of(.update)?.bytes ?? 0 }
}

/// A model file of the person's own on its way in: its name, the bytes
/// copied of its size (0 where it isn't known), then its check.
struct Copy {
    let name: String
    let got: Int64
    let total: Int64
    var checking = false
}

/// The models (the desktop's model.rs; Android's Models.kt): the
/// catalog's, downloaded into Application Support/models in the edition the
/// core chooses for this device (Editions); a model file of the person's
/// own, copied into Application Support/own; and which one is in use, a
/// catalog model by name, or the own file instead, in the app's defaults.
/// The state is the screen's, changed on the main thread.
@MainActor @Observable
final class Models {
    static let shared = Models()

    private(set) var offers: [Offer] = []
    /// the catalog model in use, by name
    private(set) var model: String?
    /// the person's own model file, in use instead when there is one
    private(set) var own: URL?
    /// whether there is a model to transcribe with (usable)
    private(set) var ready = false
    /// a file of the person's own on its way in
    var copying: Copy?
    /// a word for the screen's toast once a model is in use: "English model ready"
    var news: String?

    private let stopCopy = Flag()
    /// what refresh last told the log of the catalog
    private var told = ""

    private init() {
        refresh()
    }

    nonisolated static let ownDir: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[
            0]
        let dir = base.appendingPathComponent("own", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        var v = URLResourceValues()
        v.isExcludedFromBackup = true
        var d = dir
        try? d.setResourceValues(v)
        return dir
    }()

    func offer(_ name: String) -> Offer? { offers.first { $0.name == name } }

    /// Once a launch, off the main thread: the core's records given to files
    /// from before 0.9.6 and swaps cut short finished (Core.adopt), 0.9.6's
    /// refusals forgotten (Editions.forgetOld); then the screen's state anew.
    func start() async {
        await Task.detached(priority: .userInitiated) {
            Editions.forgetOld()
            Core.adopt()
            // a first download cut short between its encoder's archive
            // coming and its unpacking: nothing of the model runs to settle
            // it at its load (Transcriber.model), so it is unpacked now
            for v in (try? Core.catalog()) ?? [] { try? Core.settleNew(v.name) }
        }.value
        refresh()
    }

    /// The screen's state anew, from the core's view of the models folder
    /// and the defaults.
    func refresh() {
        let views = (try? Core.catalog()) ?? []
        offers = views.map { Offer(entry: $0, partial: Downloads.shared.kept($0)) }
        // the core's view in the log, each time it changes: what runs, what
        // is offered, a refusal
        let said = views.map {
            "\($0.name): chosen \($0.chosen), runs \($0.runs?.edition ?? "nothing"), offer "
                + "\($0.offer.map { "\($0.kind.rawValue) \($0.bytes)" } ?? "none")"
                + ($0.refused.map { ", refused: \($0)" } ?? "")
        }.joined(separator: "; ")
        if said != told { trace(said) }
        told = said
        let d = UserDefaults.standard
        model = d.string(forKey: "model")
        own = d.string(forKey: "model_file").map { URL(fileURLWithPath: $0) }
            .flatMap { FileManager.default.fileExists(atPath: $0.path) ? $0 : nil }
        ready = usable() != nil
    }

    /// What to transcribe with: the person's own file, else the catalog
    /// model in use, where it is here (model.rs, usable).
    enum Usable: Equatable {
        case own(URL)
        case catalog(String)
    }

    func usable() -> Usable? {
        if let own { return .own(own) }
        if let model, offer(model)?.have == true { return .catalog(model) }
        return nil
    }

    /// The catalog model of that name in use, which is here. The own file is
    /// let go and deleted, as the desktop's menu lets it go (choose_model):
    /// it was a copy, and it took as much room as a model.
    func use(_ name: String) {
        UserDefaults.standard.set(name, forKey: "model")
        UserDefaults.standard.removeObject(forKey: "model_file")
        for f
            in (try? FileManager.default.contentsOfDirectory(
                at: Models.ownDir, includingPropertiesForKeys: nil)) ?? []
        {
            try? FileManager.default.removeItem(at: f)
        }
        refresh()
    }

    /// The catalog model to use once the one in use goes: the one the
    /// person's own file stood in for where that is here, else any here.
    func next(model: String?, deleting: String?) -> Offer? {
        let here = offers.filter { $0.have && $0.name != deleting }
        return here.first { $0.name == model } ?? here.first
    }

    /// Manage models' delete of a catalog model, every edition's files with
    /// their records and replacements waiting, and its refusals (Core.delete),
    /// none while a transcription has it in its hands; its engine let go
    /// first, since the engine maps the file and a deleted file keeps its
    /// blocks while it is mapped (Android's Models.delete). What the toast
    /// says, if anything.
    func delete(_ o: Offer) -> String? {
        let inUse = own == nil && model == o.name
        if Transcriber.shared.running && (inUse || Transcriber.shared.loadedName == o.name) {
            return L("manage_cant_delete")
        }
        if Downloads.shared.state?.name == o.name { Downloads.shared.cancel() }
        Transcriber.shared.letGo(o.name)
        Core.delete(o.name)
        Downloads.shared.forget(o.name)
        return taken(inUse ? next(model: model, deleting: o.name) : nil)
    }

    /// The person's own file deleted, the copy of it the app kept.
    func deleteOwn() -> String? {
        guard let own else { return nil }
        if Transcriber.shared.running { return L("manage_cant_delete") }
        Transcriber.shared.letGo(own.path)
        try? FileManager.default.removeItem(at: own)
        UserDefaults.standard.removeObject(forKey: "model_file")
        refresh()
        return taken(next(model: model, deleting: nil))
    }

    private func taken(_ then: Offer?) -> String? {
        if let then { use(then.name) } else { refresh() }
        return then.map { L("models_using", label($0.key)) }
    }

    /// The room left where the models are, as Settings counts it.
    func free() -> Int64 {
        let v = try? Store.models.resourceValues(forKeys: [
            .volumeAvailableCapacityForImportantUsageKey
        ])
        return v?.volumeAvailableCapacityForImportantUsage ?? 0
    }

    /// A model file the person chose, copied in and checked by the engine,
    /// off the main thread: then in use, and told says so (true), or says
    /// what is wrong with it (false), and it is gone again. A copy that is
    /// a catalog model's .pkf, byte for byte, is that model's CPU edition.
    func open(_ url: URL, told: @escaping (String, Bool) -> Void) {
        let name =
            url.lastPathComponent.hasPrefix(".") || url.lastPathComponent.isEmpty
            ? "model.pkf" : url.lastPathComponent
        let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init) ?? 0
        stopCopy.set(false)
        copying = Copy(name: name, got: 0, total: size)
        let catalog = offers.map { (name: $0.name, key: $0.key, whole: $0.entry.whole) }
        let stop = stopCopy
        Task.detached(priority: .userInitiated) {
            let said = Models.copyIn(url, name: name, size: size, catalog: catalog, stop: stop)
            await MainActor.run {
                self.copying = nil
                self.refresh()
                if let said { told(said.0, said.1) }
            }
        }
    }

    func stopCopying() { stopCopy.set(true) }

    nonisolated private static func copyIn(
        _ url: URL, name: String, size: Int64,
        catalog: [(name: String, key: String, whole: Part)], stop: Flag
    ) -> (String, Bool)? {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let part = Models.ownDir.appendingPathComponent(".copying")
        let fm = FileManager.default
        try? fm.removeItem(at: part)
        do {
            let free =
                (try? Models.ownDir.resourceValues(forKeys: [
                    .volumeAvailableCapacityForImportantUsageKey
                ]))?
                .volumeAvailableCapacityForImportantUsage ?? .max
            if size > 0 && size > free { return (L("download_disk_full"), false) }
            let sha = try copyHashing(url, to: part, name: name, size: size, stop: stop)
            guard let sha else {
                try? fm.removeItem(at: part)
                return nil
            }
            Task { @MainActor in
                Models.shared.copying = Copy(name: name, got: size, total: size, checking: true)
            }
            do {
                _ = try Engine(file: part)
            } catch {
                try? fm.removeItem(at: part)
                return (words(Core.trouble(error.localizedDescription)), false)
            }
            let bytes =
                (try? part.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init) ?? 0
            // the catalog's whole .pkf, byte for byte: that model's CPU
            // edition, with the record the core reads it by
            if let same = catalog.first(where: { $0.whole.sha256 == sha && $0.whole.bytes == bytes }
            ) {
                let dst = Store.file(same.whole.file)
                try? fm.removeItem(at: dst)
                try fm.moveItem(at: part, to: dst)
                try? (sha + "\n").write(to: Store.marker(dst), atomically: true, encoding: .utf8)
                Task { @MainActor in Models.shared.use(same.name) }
                return (L("models_using", label(same.key)), true)
            }
            for f
                in (try? fm.contentsOfDirectory(at: Models.ownDir, includingPropertiesForKeys: nil))
                ?? [] where f != part
            {
                try? fm.removeItem(at: f)
            }
            let file = Models.ownDir.appendingPathComponent(name)
            try fm.moveItem(at: part, to: file)
            UserDefaults.standard.set(file.path, forKey: "model_file")
            return (L("models_using_file", name), true)
        } catch {
            try? fm.removeItem(at: part)
            return (L("model_file_cant_open"), false)
        }
    }

    /// url's bytes into part, saying how far at most ten times a second:
    /// their SHA-256, or nil if the copy was stopped.
    nonisolated private static func copyHashing(
        _ url: URL, to part: URL, name: String, size: Int64,
        stop: Flag
    ) throws -> String? {
        FileManager.default.createFile(atPath: part.path, contents: nil)
        let input = try FileHandle(forReadingFrom: url)
        let output = try FileHandle(forWritingTo: part)
        defer {
            try? input.close()
            try? output.close()
        }
        var hash = SHA256()
        var got: Int64 = 0
        var shown = Date.distantPast
        while let chunk = try input.read(upToCount: 1 << 20), !chunk.isEmpty {
            if stop.isSet { return nil }
            try output.write(contentsOf: chunk)
            hash.update(data: chunk)
            got += Int64(chunk.count)
            if Date().timeIntervalSince(shown) >= 0.1 {
                shown = Date()
                let at = got
                Task { @MainActor in
                    if Models.shared.copying != nil {
                        Models.shared.copying = Copy(name: name, got: at, total: size)
                    }
                }
            }
        }
        try output.synchronize()
        return hash.finalize().map { String(format: "%02x", $0) }.joined()
    }
}
