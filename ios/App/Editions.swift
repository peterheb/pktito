import UIKit

/// This iPhone or iPad as the core asks for it (editions.rs, Device), which
/// chooses the edition of a model it runs as it chooses for every platform
/// (docs/EDITIONS.md): from iOS 18 the Neural Engine's, the slim .pkf and
/// its Core ML encoder (on an M1 iPad 254x real time on 0.6 of a core with
/// 0.9.5's encoder, 271x with 0.9.6's, where pktito's CPU encoder is about
/// 50x on all eight), unless Core ML refused that encoder here; else the
/// CPU's, the whole .pkf. Swift keeps what only it can do: the Core ML
/// load, its retry, and whether Core ML has compiled the encoder.
enum Editions {
    /// What a refusal holds for: this system's version and this build of the
    /// app, since a new system may compile what this one couldn't, and Core
    /// ML compiles the encoder again for a new build of the app.
    static var here: String {
        "ios \(UIDevice.current.systemVersion) "
            + (Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "")
    }

    /// The device for the core, as JSON: iOS, its version, and here.
    static let device: String = {
        let v = ProcessInfo.processInfo.operatingSystemVersion
        let d: [String: Any] = [
            "system": "ios", "os": [v.majorVersion, v.minorVersion], "here": here,
        ]
        let json = try? JSONSerialization.data(withJSONObject: d)
        return json.map { String(decoding: $0, as: UTF8.self) } ?? "{}"
    }()

    /// 0.9.6's refusals, kept in the defaults under
    /// neuralEngineFailed.<model> and keyed on its build, which no later one
    /// is: forgotten, the core keeping them now (models/refusals.json).
    static func forgetOld() {
        let d = UserDefaults.standard
        for k in d.dictionaryRepresentation().keys where k.hasPrefix("neuralEngineFailed.") {
            d.removeObject(forKey: k)
        }
    }

    /// PKTITO_REFUSE_NPU=1, as on every platform (editions.rs,
    /// refusing_for_test): Core ML refuses every encoder, so that a refusal's
    /// path can be walked on any device and in the Simulator, where Core ML
    /// runs it on the Mac's processor (SIMCTL_CHILD_PKTITO_REFUSE_NPU=1).
    static let refusingForTest = ProcessInfo.processInfo.environment["PKTITO_REFUSE_NPU"] == "1"

    /// Whether Core ML has compiled the encoder v runs on this system for
    /// this build: a load of it that worked here, remembered. The first load
    /// after an install, an update of the app or an update of the encoder
    /// compiles it, which the setup screen says while it lasts.
    static func compiled(_ v: ModelView) -> Bool {
        UserDefaults.standard.string(forKey: "neuralEngineReady." + v.name) == ready(v)
    }

    /// The key a compile is remembered by: this system and build, and the
    /// encoder in place by its record, not the catalog's, which an older
    /// encoder still running isn't (the M1 iPad, 2026-11-08: 0.9.5's
    /// encoders, unrecorded, ran under the catalog's newer SHA-256, and its
    /// update would have compiled with no word on the screen). One unpacked
    /// before 0.9.6 has no record.
    private static func ready(_ v: ModelView) -> String {
        let record = v.runs?.encoder.flatMap {
            try? String(contentsOf: Store.marker(URL(fileURLWithPath: $0)), encoding: .utf8)
        }
        return here + " " + (record ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// A model's engine, as it runs here: its edition, the model file it
    /// loaded, and whether Core ML refused the encoder just now and the
    /// whole .pkf's own encoder runs it instead, which a toast says.
    struct Loaded {
        let engine: Engine
        let edition: String
        let model: URL
        let refusedNow: Bool
    }

    /// The Neural Engine can't run the model, and its CPU's edition, the
    /// whole .pkf, isn't here to run it instead: the setup screen offers it.
    /// Whether the refusal holds, the encoder having been the catalog's: one
    /// that wasn't says nothing of the catalog's, which is offered instead.
    struct Refused: Error {
        let holds: Bool
    }

    /// v's engine, as the core says it runs here (v.runs): on the Neural
    /// Engine, Core ML compiles the encoder for this device the first time,
    /// 16 s on an M1 iPad, which the screen says it is doing. Where Core ML
    /// refuses it, the core keeps the refusal and deletes the encoder
    /// (Core.refuse), and the model goes on with the whole .pkf's own encoder
    /// where that is here; where only the slim file was, Refused, and
    /// nothing downloads by itself (Peter, 2026-11-05): the setup screen
    /// offers the whole file. Off the main thread.
    static func engine(for v: ModelView) throws -> Loaded {
        guard let r = v.runs else { throw CoreError(L("no_model")) }
        let model = URL(fileURLWithPath: r.model)
        guard let encoder = r.encoder else {
            return Loaded(
                engine: try Engine(file: model), edition: r.edition, model: model,
                refusedNow: false)
        }
        // Twice: one first compile after an install failed on the M1 iPad
        // because the system's compiler didn't answer ("Couldn't communicate
        // with a helper application"), and the next one succeeded. Core ML
        // reports that the same way as a program its compiler rejects, so
        // every failure is tried again; a rejection costs a second attempt,
        // 8.5 s there.
        var why = ""
        for attempt in 1...2 {
            do {
                let engine = try Engine(file: model)
                if refusingForTest { throw CoreError("PKTITO_REFUSE_NPU=1: a refusal, for a test") }
                try engine.useNeuralEngine(URL(fileURLWithPath: encoder))
                UserDefaults.standard.set(ready(v), forKey: "neuralEngineReady." + v.name)
                return Loaded(engine: engine, edition: r.edition, model: model, refusedNow: false)
            } catch {
                why = error.localizedDescription
                if attempt == 1 { Thread.sleep(forTimeInterval: 1) }
            }
        }
        trace("the Neural Engine refused \(v.name): \(why)")
        let refused = (try? Core.refuse(v.name, edition: r.edition, why: why)) ?? false
        let whole = Store.file(v.whole.file)
        guard FileManager.default.fileExists(atPath: whole.path) else {
            throw Refused(holds: refused)
        }
        return Loaded(
            engine: try Engine(file: whole), edition: "cpu", model: whole, refusedNow: refused)
    }
}
