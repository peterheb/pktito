import UIKit

/// Which edition of a model this device runs, chosen for whoever uses the
/// app: the Neural Engine's (the slim .pkf and its Core ML encoder: on an
/// M1 iPad 254x real time on 0.6 of a core, where pktito's CPU encoder is
/// about 50x on all eight), and where the Neural Engine can't run its
/// encoder, the CPU's (the whole .pkf), as the demo found (the ane fork's
/// tools/ane/CLIMB.md; pktito-app's ios-demo branch).
enum Editions {
    /// The edition to download for e: the Neural Engine's, unless it failed
    /// on this device before, for this system and build of the app.
    static func preferred(_ e: CatalogEntry) -> Edition {
        failure(e) == nil ? .neuralEngine : .cpu
    }

    /// Why the Neural Engine edition of e failed here, for this system and
    /// build, or nil: a new system may compile what this one couldn't, and
    /// Core ML compiles the encoder again for a new build of the app.
    static func failure(_ e: CatalogEntry) -> String? {
        guard let s = UserDefaults.standard.string(forKey: "neuralEngineFailed." + e.name),
              s.hasPrefix(here + ": ") else { return nil }
        return String(s.dropFirst(here.count + 2))
    }

    private static var here: String {
        "\(UIDevice.current.systemVersion) "
            + (Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "")
    }

    /// The Neural Engine edition of e can't run here.
    struct NeedsCPUEdition: LocalizedError {
        let why: String
        var errorDescription: String? { why }
    }

    /// e's engine, its Neural Engine edition where its files are here and
    /// the encoder runs - Core ML compiles it for this device the first
    /// time, 16 s on an M1 iPad, which the screen says it is doing - else
    /// its CPU edition. Where the Neural Engine edition fails and the CPU's
    /// isn't downloaded, NeedsCPUEdition. Off the main thread.
    static func engine(for e: CatalogEntry) throws -> (Engine, Edition) {
        if Store.have(e, .neuralEngine), failure(e) == nil {
            // Twice: one first compile after an install failed on the M1
            // iPad because the system's compiler didn't answer ("Couldn't
            // communicate with a helper application"), and the next one
            // succeeded. Core ML reports that the same way as a program its
            // compiler rejects, so every failure is tried again; a rejection
            // costs a second attempt, 8.5 s there.
            var why = ""
            for attempt in 1...2 {
                do {
                    let engine = try Engine(file: Store.file(e.coreml.slim))
                    try engine.useNeuralEngine(Store.encoder(e))
                    return (engine, .neuralEngine)
                } catch {
                    why = error.localizedDescription
                    if attempt == 1 {
                        Thread.sleep(forTimeInterval: 1)
                    }
                }
            }
            UserDefaults.standard.set(here + ": " + why, forKey: "neuralEngineFailed." + e.name)
            if !Store.have(e, .cpu) {
                throw NeedsCPUEdition(why: why)
            }
        }
        return (try Engine(file: Store.file(e.pkf)), .cpu)
    }
}
