#if DEBUG
import SwiftUI
import UniformTypeIdentifiers

/// The first build's screen, kept in debug builds for a device with no one
/// tapping (-bench, through devicectl): each model's edition for this
/// device downloaded, prepared and run on a file, every step said as it goes.
@MainActor @Observable
final class BenchState {
    var catalog: [CatalogEntry] = []
    var line: [String: String] = [:]
    var engine: Engine?
    var edition: Edition?
    var model: CatalogEntry?
    var busy = false
    var text = ""
    var stats = ""

    init() {
        do {
            catalog = try Core.catalog()
        } catch {
            line["catalog"] = error.localizedDescription
        }
    }

    private func mb(_ b: Int64) -> String { String(b / 1_000_000) }

    /// e's edition for this device, downloaded (and its encoder unpacked),
    /// then prepared
    func get(_ e: CatalogEntry) async {
        busy = true
        defer { busy = false }
        do {
            try await download(e, Editions.preferred(e))
            await prepare(e)
        } catch {
            line[e.name] = error.localizedDescription
        }
    }

    private func download(_ e: CatalogEntry, _ edition: Edition) async throws {
        let parts = edition == .cpu ? [e.pkf] : [e.coreml.slim, e.coreml.encoder]
        let total = parts.reduce(0) { $0 + $1.bytes }
        var before: Int64 = 0
        for p in parts where !(p.file.hasSuffix(".aar") && Store.haveEncoder(e)) {
            let base = before
            let url = try await Downloader.shared.fetch(p) { got, _ in
                Task { @MainActor in
                    self.line[e.name] = "Downloading \(self.mb(base + got)) of \(self.mb(total)) MB"
                }
            }
            if p.file.hasSuffix(".aar") {
                line[e.name] = "Unpacking the encoder…"
                try await Task.detached(priority: .userInitiated) { try Unpack.encoder(e, from: url) }.value
            }
            before += p.bytes
        }
    }

    /// e's engine for this device: the Neural Engine edition prepared (Core
    /// ML compiles its encoder the first time), else the CPU's, downloaded
    /// first where it isn't here
    func prepare(_ e: CatalogEntry) async {
        line[e.name] = "Preparing pktito for this \(UIDevice.current.model)…"
        let t0 = Date()
        do {
            let (engine, edition) = try await Task.detached(priority: .userInitiated) { try Editions.engine(for: e) }.value
            self.engine = engine
            self.edition = edition
            model = e
            line[e.name] = String(format: "Ready: %@, prepared in %.1f s", edition == .neuralEngine ? "Neural Engine" : "CPU",
                                  Date().timeIntervalSince(t0))
        } catch let failed as Editions.NeedsCPUEdition {
            line[e.name] = "The Neural Engine can't run it here (\(failed.why)); getting the CPU's model"
            do {
                try await download(e, .cpu)
                await prepare(e)
            } catch {
                line[e.name] = error.localizedDescription
            }
        } catch {
            line[e.name] = error.localizedDescription
        }
    }

    func transcribe(_ url: URL) async {
        guard let engine else { return }
        busy = true
        defer { busy = false }
        text = ""
        stats = "Reading…"
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let progress = Engine.Progress()
        progress.update = { done, _ in
            Task { @MainActor in self.stats = String(format: "Transcribing %.0f%%", done * 100) }
        }
        let t0 = Date()
        let name = url.lastPathComponent
        do {
            let t = try await Task.detached(priority: .userInitiated) {
                try engine.transcribe(url, name: name, progress: progress)
            }.value
            let took = Date().timeIntervalSince(t0)
            text = t.segments.map(\.text).joined(separator: "\n")
            stats = String(format: "%.1f s of audio in %.2f s, %.1fx real time, %@, %@", t.duration, took,
                           t.duration / max(took, 0.001), edition == .neuralEngine ? "Neural Engine" : "CPU",
                           Core.backend)
        } catch {
            stats = error.localizedDescription
        }
    }
}

struct Bench: View {
    @State private var s = BenchState()
    @State private var picking = false

    var body: some View {
        NavigationStack {
            List {
                Section("Models") {
                    ForEach(s.catalog) { e in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(e.label).font(.headline)
                                Spacer()
                                Button(Store.have(e, Editions.preferred(e)) ? "Use" : "Get") {
                                    Task {
                                        if Store.have(e, Editions.preferred(e)) {
                                            await s.prepare(e)
                                        } else {
                                            await s.get(e)
                                        }
                                    }
                                }
                                .disabled(s.busy)
                            }
                            Text(s.line[e.name] ?? e.about).font(.footnote).foregroundStyle(.secondary)
                        }
                    }
                    if let why = s.line["catalog"] {
                        Text(why).foregroundStyle(.red)
                    }
                }
                Section("Transcribe") {
                    Button("Choose a file…") { picking = true }
                        .disabled(s.engine == nil || s.busy)
                    if !s.stats.isEmpty {
                        Text(s.stats).font(.footnote.monospacedDigit())
                    }
                    if !s.text.isEmpty {
                        Text(s.text).textSelection(.enabled)
                    }
                }
            }
            .navigationTitle("pktito")
            .fileImporter(isPresented: $picking, allowedContentTypes: [.audio, .movie]) { r in
                if case .success(let url) = r {
                    Task { await s.transcribe(url) }
                }
            }
            #if DEBUG
            // for a device with no one tapping, through devicectl:
            // -bench KEY [FILE] gets and prepares the catalog model KEY
            // ("english", "multilingual") as this device runs it, then
            // transcribes FILE from Application Support/bench, printing each step
            .task {
                let args = CommandLine.arguments
                guard let i = args.firstIndex(of: "-bench"), i + 1 < args.count,
                      let e = s.catalog.first(where: { $0.key == args[i + 1] }) else { return }
                await s.bench(e, file: i + 2 < args.count ? args[i + 2] : nil)
            }
            #endif
        }
    }
}

#if DEBUG
extension BenchState {
    func bench(_ e: CatalogEntry, file: String?) async {
        let t0 = Date()
        let say = { (s: String) in print(String(format: "bench %6.1f s: %@", Date().timeIntervalSince(t0), s)); fflush(stdout) }
        say("\(e.name), edition for this device: \(Editions.preferred(e).rawValue)\(Editions.failure(e).map { " (Neural Engine failed before: \($0))" } ?? "")")
        if Store.have(e, Editions.preferred(e)) {
            await prepare(e)
        } else {
            await get(e)
        }
        say(line[e.name] ?? "")
        guard engine != nil, let file else { return }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        await transcribe(base.appendingPathComponent("bench").appendingPathComponent(file))
        say(stats)
        say("first lines: " + text.split(separator: "\n").prefix(3).joined(separator: " / "))
    }
}
#endif
#endif
