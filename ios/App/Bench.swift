#if DEBUG
    import SwiftUI
    import UniformTypeIdentifiers

    /// The first build's screen, kept in debug builds for a device with no one
    /// tapping (-bench, through devicectl): each model's edition for this
    /// device downloaded, prepared and run on a file, every step said as it goes.
    @MainActor @Observable
    final class BenchState {
        var catalog: [ModelView] = []
        var line: [String: String] = [:]
        var engine: Engine?
        var edition: String?
        var model: ModelView?
        var busy = false
        var text = ""
        var stats = ""

        init() { reload() }

        /// The core's view of the models folder anew.
        func reload() {
            do {
                catalog = try Core.catalog()
            } catch {
                line["catalog"] = error.localizedDescription
            }
        }

        func view(_ name: String) -> ModelView? { catalog.first { $0.name == name } }

        private func mb(_ b: Int64) -> String { String(b / 1_000_000) }

        /// v's offer downloaded, each file checked and put in place as it
        /// comes and the encoder unpacked, then the model prepared
        func get(_ v: ModelView) async {
            busy = true
            defer { busy = false }
            do {
                try await download(v)
                await prepare(v.name)
            } catch {
                line[v.name] = error.localizedDescription
            }
        }

        private func download(_ v: ModelView) async throws {
            let parts = v.offer?.parts ?? []
            let total = parts.reduce(0) { $0 + $1.bytes }
            var before: Int64 = 0
            for p in parts {
                let base = before
                await Downloader.shared.begin([p])
                let held = try await Downloader.shared.fetch(p) { got in
                    Task { @MainActor in
                        self.line[v.name] =
                            "Downloading \(self.mb(base + got)) of \(self.mb(total)) MB"
                    }
                }
                line[v.name] = "Checking \(p.file)…"
                let name = v.name
                _ = try await Task.detached(priority: .userInitiated) {
                    try Core.land(name, file: p.file, got: held)
                }.value
                before += p.bytes
            }
            line[v.name] = "Unpacking the encoder…"
            let name = v.name
            try await Task.detached(priority: .userInitiated) { try Core.settleNew(name) }.value
            reload()
        }

        /// The model's engine as the core says it runs here: the Neural
        /// Engine's edition prepared (Core ML compiles its encoder the first
        /// time), else the CPU's; where the Neural Engine refuses it and the
        /// CPU's isn't here, that one downloaded first, as nothing in the app
        /// does by itself, for a device with no one tapping
        func prepare(_ name: String) async {
            reload()
            guard let v = view(name) else { return }
            line[name] = "Preparing pktito for this \(UIDevice.current.model)…"
            let t0 = Date()
            do {
                let l = try await Task.detached(priority: .userInitiated) {
                    try Editions.engine(for: v)
                }.value
                engine = l.engine
                edition = l.edition
                model = v
                line[name] = String(
                    format: "Ready: %@%@, prepared in %.1f s",
                    l.edition == "cpu" ? "CPU" : "Neural Engine",
                    l.refusedNow ? " (the Neural Engine refused it)" : "",
                    Date().timeIntervalSince(t0))
            } catch let refused as Editions.Refused {
                line[name] =
                    "The Neural Engine refused it here (the catalog's encoder: \(refused.holds)); getting the CPU's model"
                reload()
                guard let cpu = view(name), cpu.offer != nil else { return }
                do {
                    try await download(cpu)
                    await prepare(name)
                } catch {
                    line[name] = error.localizedDescription
                }
            } catch {
                line[name] = error.localizedDescription
            }
            reload()
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
                Task { @MainActor in self.stats = String(format: "Transcribing %.0f%%", done * 100)
                }
            }
            let t0 = Date()
            let name = url.lastPathComponent
            do {
                let t = try await Task.detached(priority: .userInitiated) {
                    try engine.transcribe(url, name: name, progress: progress)
                }.value
                let took = Date().timeIntervalSince(t0)
                text = t.segments.map(\.text).joined(separator: "\n")
                stats = String(
                    format: "%.1f s of audio in %.2f s, %.1fx real time, %@, %@", t.duration, took,
                    t.duration / max(took, 0.001),
                    edition == "cpu" ? "CPU" : "Neural Engine",
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
                        ForEach(s.catalog) { v in
                            VStack(alignment: .leading, spacing: 4) {
                                HStack {
                                    Text(label(v.key)).font(.headline)
                                    Spacer()
                                    Button(v.runs != nil ? "Use" : "Get") {
                                        Task {
                                            if v.runs != nil {
                                                await s.prepare(v.name)
                                            } else {
                                                await s.get(v)
                                            }
                                        }
                                    }
                                    .disabled(s.busy)
                                }
                                Text(s.line[v.name] ?? v.name).font(.footnote).foregroundStyle(
                                    .secondary)
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
                    // -bench KEY [FILE] [-fresh] gets and prepares the catalog model KEY
                    // ("english", "multilingual") as this device runs it, then
                    // transcribes FILE from Application Support/bench, printing each step
                    .task {
                        let args = CommandLine.arguments
                        guard let i = args.firstIndex(of: "-bench"), i + 1 < args.count,
                            let e = s.catalog.first(where: { $0.key == args[i + 1] })
                        else { return }
                        await s.bench(e, file: i + 2 < args.count ? args[i + 2] : nil)
                    }
                #endif
            }
        }
    }

    #if DEBUG
        extension BenchState {
            func bench(_ e: ModelView, file: String?) async {
                let t0 = Date()
                let say = { (s: String) in
                    print(String(format: "bench %6.1f s: %@", Date().timeIntervalSince(t0), s))
                    fflush(stdout)
                }
                // -fresh: this model's files deleted first, so that its edition
                // is downloaded, checked and unpacked as a first install's is
                if CommandLine.arguments.contains("-fresh") {
                    Core.delete(e.name)
                    for p in e.files { Downloader.shared.forget(p.file) }
                    reload()
                    say("deleted \(e.name)'s files (-fresh)")
                }
                guard let e = view(e.name) else { return }
                say(
                    "\(e.name), edition for this device: \(e.chosen)\(e.refused.map { " (the Neural Engine refused it before: \($0))" } ?? "")"
                )
                if e.runs != nil {
                    await prepare(e.name)
                } else {
                    await get(e)
                }
                say(line[e.name] ?? "")
                guard engine != nil, let file else { return }
                let base = FileManager.default.urls(
                    for: .applicationSupportDirectory, in: .userDomainMask)[0]
                await transcribe(base.appendingPathComponent("bench").appendingPathComponent(file))
                say(stats)
                // the transcript beside the file (FILE.txt), for AliceBench's scoring
                // against its references off the device
                try? text.write(
                    to: base.appendingPathComponent("bench").appendingPathComponent(file + ".txt"),
                    atomically: true, encoding: .utf8)
                say(
                    "first lines: " + text.split(separator: "\n").prefix(3).joined(separator: " / ")
                )
            }
        }
    #endif
#endif
