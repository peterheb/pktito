import SwiftUI
import UniformTypeIdentifiers

/// The app's window (Android's MainActivity, App): About; the setup screen
/// while there is no model, one is coming, or Core ML compiles the one in
/// use; the start screen; or a file at work and then its transcript; where
/// a list and what it opens fit side by side, the start screen's list
/// beside them (Side). Over it the toast, Manage models, and the picker of
/// a recording, a model file, or a recording found again (Locate…).
struct Root: View {
    @Environment(\.scenePhase) private var phase
    /// what the picker was opened for, kept for when it answers
    @State private var picked = AppState.Pick.media
    /// the window's height as if the keyboard weren't up
    @State private var tall: CGFloat = 0

    /// What the player loads anew for.
    private struct Playing: Equatable {
        let step: Transcriber.Step
        let id: String
        let url: URL?
    }

    var body: some View {
        let s = AppState.shared
        let t = Transcriber.shared
        GeometryReader { g in
            // the side where a list and what it opens fit side by side:
            // 840 wide, which leaves the work 540, where its rows stack
            // (Rows, under 620) and Find keeps the views line (Views, from
            // 480); and 480 tall, which keeps a phone on its side one column.
            // The window's height, not what the keyboard leaves of it, which
            // on an 11-inch iPad on its side (834 tall) can be under 480 and
            // would change the layout under the field being typed in (as in
            // Android's app, b5cfeda)
            let sided = g.size.width >= 840 && max(g.size.height, tall) >= 480
            ZStack(alignment: .bottom) {
                Group {
                    if s.about {
                        About()
                    } else if t.step == .none && s.setup {
                        Setup(waiting: s.waiting != nil ? 1 : 0)
                    } else if sided && (t.step != .none || !Recents.shared.items.isEmpty) {
                        Side()
                    } else if t.step == .none {
                        Ready()
                    } else {
                        Work()
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                // over Manage models while it is up, which has it too
                if !s.managing, let toast = s.toast {
                    Toast(text: toast, err: s.toastErr)
                        .padding(.horizontal, 16)
                        .padding(.bottom, 24)
                        .allowsHitTesting(false)
                }
            }
        }
        .background {
            GeometryReader { w in
                Color.clear.onChange(of: w.size.height, initial: true) { _, h in tall = h }
                    #if DEBUG
                        .onChange(of: "\(w.size) \(w.safeAreaInsets)", initial: true) { _, s in
                            trace("window: \(s)")
                        }
                    #endif
            }
            .ignoresSafeArea(.keyboard)
        }
        // the ground to the window's every edge, its safe areas too: a window
        // on the iPad showed the system's grey under the rows (2026-11-01)
        .background { Pk.ground.ignoresSafeArea() }
        .tint(Pk.ink)
        .fileImporter(
            isPresented: Binding(get: { s.pick != nil }, set: { if !$0 { s.pick = nil } }),
            // a .pkf has no type of its own to ask for
            allowedContentTypes: s.pick == .model ? [.item] : [.audio, .movie]
        ) { r in
            if case .success(let url) = r { opened(url) }
        }
        .onChange(of: s.pick) { if let p = s.pick { picked = p } }
        // a recording sent to pktito from another app
        .onOpenURL { s.received($0) }
        .sheet(isPresented: Binding(get: { s.managing }, set: { s.managing = $0 })) {
            Manage().presentationBackground(Pk.ground)
        }
        // the finished transcript's recording, ready to play; another's let
        // go. Where it can't be played, the strip says why in its times'
        // place, and Locate… finds it again
        .onChange(of: Playing(step: t.step, id: t.id, url: t.url), initial: true) { load() }
        // a model downloaded, said so
        .onChange(of: Models.shared.news, initial: true) {
            if let n = Models.shared.news {
                s.say(n)
                Models.shared.news = nil
            }
        }
        // the recording that waited for a model, once there is one
        .onChange(of: s.setup, initial: true) { startWaiting() }
        .onChange(of: s.waiting) { startWaiting() }
        // the transcript's audio stops with the screen; the transcription
        // goes on. Back on the screen, a model a download brought while the
        // app was away is prepared
        .onChange(of: phase) {
            if phase == .background { Player.shared.pause() }
            if phase == .active { Transcriber.shared.prepareIfNew() }
        }
        // the models folder put right (Models.start); a download a run before
        // began goes on; Core ML compiles a Neural Engine edition's encoder
        // the first time after an install or an update, which the setup
        // screen says
        .task {
            await Models.shared.start()
            Downloads.shared.resumeAfterLaunch()
            Transcriber.shared.prepareIfNew()
            #if DEBUG
                debugArguments()
            #endif
        }
    }

    #if DEBUG
        /// For a device or a Simulator with no one tapping, through devicectl
        /// or simctl: -open NAME transcribes Application Support/bench/NAME,
        /// and -receive NAME a copy of it as iOS sends one (Open in);
        /// -recent opens the newest transcript of the list; -play SECONDS plays
        /// it from there; -find WORDS searches it, and with -typing as typed,
        /// the keyboard up; -landscape turns a phone on its side; -show about
        /// or manage opens that, and with about, -sheet privacy or notices one
        /// of its sheets;
        /// -download english or multilingual downloads that model (its server
        /// PKTITO_MODEL_BASE, where the environment gives one), and with it
        /// -pauseAfter S pauses it S seconds on, and -resumeAfter S goes on S
        /// seconds after that; -pretend setup, download, checking, paused,
        /// failed, preparing, loading, transcribing, error or toast shows that
        /// state, with nothing downloaded or transcribed. (A default's own -key
        /// value, as -captions YES, sets it for the run.)
        private func debugArguments() {
            let args = CommandLine.arguments
            let s = AppState.shared
            func value(_ flag: String) -> String? {
                guard let i = args.firstIndex(of: flag), i + 1 < args.count else { return nil }
                return args[i + 1]
            }
            let bench = FileManager.default.urls(
                for: .applicationSupportDirectory, in: .userDomainMask)[0]
                .appendingPathComponent("bench")
            if let name = value("-open") { s.start(bench.appendingPathComponent(name)) }
            // as iOS sends a file: a copy in Documents/Inbox
            if let name = value("-receive") {
                let inbox = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[
                    0
                ]
                .appendingPathComponent("Inbox", isDirectory: true)
                try? FileManager.default.createDirectory(
                    at: inbox, withIntermediateDirectories: true)
                let copy = inbox.appendingPathComponent(name)
                try? FileManager.default.removeItem(at: copy)
                try? FileManager.default.copyItem(at: bench.appendingPathComponent(name), to: copy)
                s.received(copy)
            }
            if args.contains("-recent"), let first = Recents.shared.items.first {
                s.openRecent(first.id)
            }
            if let t = value("-play").flatMap(Double.init) {
                Task {
                    while !Player.shared.ready { try? await Task.sleep(for: .milliseconds(100)) }
                    Player.shared.seek(t, play: true)
                }
            }
            if args.contains("-landscape"),
                let scene = UIApplication.shared.connectedScenes.first(where: {
                    $0 is UIWindowScene
                }) as? UIWindowScene
            {
                scene.requestGeometryUpdate(.iOS(interfaceOrientations: .landscapeRight)) {
                    trace("landscape: \($0.localizedDescription)")
                }
            }
            if let q = value("-find") {
                s.finder.open = true
                if args.contains("-typing") {
                    // the field opened empty takes the keyboard, as the glass's does; then the words
                    Task {
                        try? await Task.sleep(for: .seconds(1))
                        s.finder.query = q
                    }
                } else {
                    s.finder.query = q
                }
            }
            if let key = value("-download"),
                let o = Models.shared.offers.first(where: { $0.key == key })
            {
                Downloads.shared.start(o)
                let pause = value("-pauseAfter").flatMap(Double.init)
                let resume = value("-resumeAfter").flatMap(Double.init)
                Task {
                    if let pause {
                        try? await Task.sleep(for: .seconds(pause))
                        Downloads.shared.pause()
                    }
                    if let resume {
                        try? await Task.sleep(for: .seconds(resume))
                        Downloads.shared.start(o)
                    }
                }
            }
            switch value("-show") {
            case "about": s.about = true
            case "manage": s.managing = true
            default: break
            }
            // a state as it would be, which no one taps to, for the screenshots
            let english = "parakeet-tdt-0.6b-v2"
            let size: Int64 = 624_480_196
            switch value("-pretend") {
            case "setup": s.pretendSetup = true
            case "download":
                Downloads.shared.pretend(
                    DownloadState(
                        name: english, step: .downloading, got: 262_000_000, total: size,
                        speed: 14_200_000))
            case "checking":
                Downloads.shared.pretend(
                    DownloadState(name: english, step: .checking, got: size, total: size))
            case "paused":
                Downloads.shared.pretend(
                    DownloadState(name: english, step: .paused, got: 262_000_000, total: size))
            case "failed":
                Downloads.shared.pretend(
                    DownloadState(
                        name: english, step: .failed, got: 262_000_000, total: size,
                        problem: DownloadProblem(key: "download.not_answering")))
            case "preparing": Transcriber.shared.pretend(preparing: true)
            case "loading":
                Transcriber.shared.pretend(.loading, name: "Interview with the author.m4a", done: 0)
            case "transcribing":
                Transcriber.shared.pretend(
                    .transcribing, name: "Interview with the author.m4a", done: 0.37, duration: 2712
                )
            case "error":
                Transcriber.shared.pretend(
                    .error, name: "Interview with the author.m4a", done: 0,
                    error: L("problem_unexpected"))
            case "toast": s.toast = L("models_using", L("catalog_english"))
            default: break
            }
        }
    #endif

    /// A file the picker gave, for what it was opened for.
    private func opened(_ url: URL) {
        let s = AppState.shared
        let t = Transcriber.shared
        switch picked {
        case .media:
            s.start(url)
        case .model:
            Models.shared.open(url) { said, ok in s.say(said, err: !ok) }
        case .locate:
            // the transcript's recording, picked again where it is now, if it
            // is the one transcribed (Recents.relink); the player then loads it
            if Recents.shared.relink(t.id, url) {
                t.relocated(url)
            } else {
                s.say(L("play_not_it"), err: true)
            }
        }
    }

    private func load() {
        let t = Transcriber.shared
        guard t.step == .done || t.step == .stopped else {
            Player.shared.release()
            return
        }
        var url = t.url
        if t.reopened, let item = Recents.shared.items.first(where: { $0.id == t.id }) {
            let src = Recents.shared.source(item)
            t.why = src.why
            t.locate = src.locate
            url = src.url
        }
        guard let url else { return }
        let key = "\(t.id) \(url.path)"
        if Player.shared.loaded == key { return }
        Player.shared.load(key, url) {
            t.why = L("play_cant_ios")
            t.locate = false
        }
    }

    private func startWaiting() {
        let s = AppState.shared
        if !s.setup, let w = s.waiting, Transcriber.shared.step == .none {
            s.waiting = nil
            s.start(w)
        }
    }
}
