import SwiftUI

/// The system's language, which a model of several may speak.
private var ownLanguage: String { Locale.current.language.languageCode?.identifier ?? "en" }

/// A language's name in English, the screen's language (app.js, langName).
private func languageName(_ code: String) -> String {
    Locale(identifier: "en").localizedString(forLanguageCode: code) ?? code
}

/// A model of several's languages, about, and the system's language where
/// it is one of them, as the one sentence models_languages_among makes of
/// them, about where the sentence puts it (ui/app.js, withOwn).
private func withOwn(_ e: CatalogEntry, _ about: AttributedString) -> AttributedString {
    guard e.languages.count > 1, e.languages.contains(ownLanguage) else { return about }
    let mark = "\u{1}"
    let parts = L("models_languages_among", mark, languageName(ownLanguage)).components(
        separatedBy: mark)
    guard parts.count == 2 else { return about }
    return AttributedString(parts[0]) + about + AttributedString(parts[1])
}

/// The setup screen (ui/app.js, renderSetup; Android's Setup.kt): choose a
/// speech model and download it, or use a model file of one's own; a
/// download under way or paused, one that failed; a model file being copied
/// in; and iOS's own, the Neural Engine's encoder being compiled for this
/// device, the first time after an install or an update.
struct Setup: View {
    let waiting: Int

    var body: some View {
        let m = Models.shared
        if let c = m.copying {
            Copying(c: c)
        } else if let d = Downloads.shared.state {
            if d.step == .failed { Failed(d: d) } else { Downloading(d: d, waiting: waiting) }
        } else if Transcriber.shared.preparing {
            Preparing()
        } else {
            Choose(waiting: waiting)
        }
    }
}

/// The screen's frame (app.css, .pk-center > .pk-screen): a column 640 wide
/// at most, 20 in from a narrow window's sides and 40 from a wider one's,
/// what it says at its top and its foot at its bottom; taller than the
/// window, it scrolls.
private struct Screen<Top: View, Foot: View>: View {
    @ViewBuilder let top: Top
    @ViewBuilder let foot: Foot
    /// under the window's controls, where the column's top is under them
    @State private var clear = Clearance()

    var body: some View {
        GeometryReader { g in
            let narrow = g.size.width < 600
            ScrollView {
                // its foot at the bottom of a box at least 520 tall, the box in
                // the window's middle while it fits and from its top where it
                // doesn't (app.css, .pk-screen): on the iPad Pro 13 the foot sat
                // at the window's bottom, 450 under the words. A phone's box is
                // the window's height, its foot where a thumb is.
                AtLeast(height: narrow ? g.size.height : min(g.size.height, 520)) {
                    VStack(alignment: .leading, spacing: 0) {
                        // 24, as the start screen's (20 before; the desktop's .pk-screen)
                        VStack(alignment: .leading, spacing: 24) { top }
                        Spacer(minLength: 20)
                        VStack(alignment: .leading, spacing: 0) { foot }
                    }
                    .frame(maxWidth: 640, alignment: .leading)
                    .padding(.top, clear.top)
                    .windowControls($clear)
                    // a phone's 16, as every screen's (the layout audit, 2026-11-01)
                    .padding(.horizontal, narrow ? 16 : 40)
                    .padding(.top, 28)
                    .padding(.bottom, 24)
                }
                .frame(maxWidth: .infinity, minHeight: g.size.height)
            }
        }
    }
}

/// Its one view at its own height, or at `height` where that is more, which
/// a Spacer in it fills: a box that grows with its words but never shrinks
/// under its least, whatever is offered round it.
private struct AtLeast: SwiftUI.Layout {
    let height: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: LayoutSubviews, cache: inout ())
        -> CGSize
    {
        guard let v = subviews.first else { return .zero }
        let own = v.sizeThatFits(ProposedViewSize(width: proposal.width, height: nil))
        // its own width, which the frame round it centres
        return CGSize(width: own.width, height: max(own.height, height))
    }

    func placeSubviews(
        in bounds: CGRect, proposal: ProposedViewSize, subviews: LayoutSubviews, cache: inout ()
    ) {
        subviews.first?.place(at: bounds.origin, proposal: ProposedViewSize(bounds.size))
    }
}

/// The screen's heading, and the words under it (.pk-intro).
private struct Intro: View {
    let title: String
    var text: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).pk(PkType.lead).foregroundStyle(Pk.ink).accessibilityAddTraits(.isHeader)
            if let text {
                Text(text).font(PkType.body.font(15)).foregroundStyle(Pk.ink)
            }
        }
    }
}

/// The wordmark as each screen's way to About (app.css, button.pk-wm).
struct WordmarkButton: View {
    var body: some View {
        Button {
            AppState.shared.about = true
        } label: {
            Wordmark().padding(.horizontal, 6).padding(.vertical, 4)
        }
        .buttonStyle(.plain)
        .padding(.horizontal, -6)
        .padding(.vertical, -4)
        .accessibilityLabel(L("about_open"))
    }
}

/// The choice (app.js, chooseHtml): a card for each model, the one that
/// speaks the system's language chosen to begin with, else the one in use;
/// its button says what choosing it does, and under it the way to a model
/// file of one's own.
private struct Choose: View {
    let waiting: Int
    @State private var choice = ""

    var body: some View {
        let m = Models.shared
        let chosen =
            m.offers.first { $0.name == choice } ?? m.offers.first { $0.name == m.model }
            ?? m.offers.first { $0.entry.languages.contains(ownLanguage) } ?? m.offers.first
        Screen {
            WordmarkButton()
            Intro(title: L("setup_title"))
            VStack(spacing: 12) {
                ForEach(m.offers) { o in
                    Card(o: o, on: o.name == chosen?.name) { choice = o.name }
                }
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel(L("setup_legend"))
        } foot: {
            if let chosen {
                let name = label(chosen.key)
                let go =
                    chosen.have
                    ? L("setup_use", name)
                    : chosen.partial > 0
                        ? L("setup_download_rest", name, mb(chosen.bytes - chosen.partial))
                        : L("setup_download", name, mb(chosen.bytes))
                VStack(alignment: .leading, spacing: 16) {
                    PkButton(
                        text: go, kind: .primary, large: true, stretch: true,
                        leading: chosen.have ? .check : .download
                    ) {
                        if chosen.have {
                            Models.shared.use(chosen.name)
                            Transcriber.shared.prepare()
                        } else {
                            Downloads.shared.start(chosen)
                        }
                    }
                    ViewThatFits(in: .horizontal) {
                        HStack {
                            LinkButton(text: L("setup_have_file")) { AppState.shared.pick = .model }
                            Spacer(minLength: 16)
                            waitingLine
                        }
                        VStack(alignment: .leading, spacing: 16) {
                            LinkButton(text: L("setup_have_file")) { AppState.shared.pick = .model }
                            waitingLine
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder private var waitingLine: some View {
        if waiting > 0 {
            Text(L("setup_waiting", count: waiting)).pk(PkType.small).foregroundStyle(Pk.inkMuted)
        }
    }
}

/// A model's card (.pk-choice): its radio, its name, its size or that it is
/// here; under them its file's name and what it transcribes, a model of
/// several's count opening their list. The one chosen on the ground in an
/// ink ring, the others on surface.
private struct Card: View {
    let o: Offer
    let on: Bool
    let choose: () -> Void
    @State private var open = false

    /// the languages' words as a link, which opens their list, not a page
    private static let languages = URL(string: "pktito-ui:languages")!

    var body: some View {
        let many = o.entry.languages.count > 1
        let size = Text(o.have ? L("setup_on_device") : L("setup_size", mb(o.bytes)))
            .font(PkType.stamp.font(13))
            .foregroundStyle(Pk.inkMuted)
            .lineLimit(1)
        VStack(alignment: .leading, spacing: 2) {
            // the size under the name where there is no room beside it
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    radio
                    title.fixedSize()
                    Spacer()
                    size
                }
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        radio
                        title
                    }
                    size.padding(.leading, 30)
                }
            }
            VStack(alignment: .leading, spacing: 0) {
                // one paragraph, which wraps as a sentence does
                Text(line(many))
                    .pk(PkType.small)
                    .foregroundStyle(Pk.inkMuted)
                    .tint(Pk.inkMuted)
                    .environment(
                        \.openURL,
                        OpenURLAction { url in
                            guard url == Card.languages else { return .systemAction }
                            open.toggle()
                            return .handled
                        })
                if many && open { Languages(o: o) }
            }
            .padding(.leading, 30)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
        .background(
            on ? Pk.ground : Pk.surface, in: RoundedRectangle(cornerRadius: Radius.radiusUi)
        )
        .overlay {
            if on { RoundedRectangle(cornerRadius: Radius.radiusUi).stroke(Pk.ink, lineWidth: 2) }
        }
        .contentShape(Rectangle())
        // a tap anywhere but the languages' link chooses it
        .onTapGesture(perform: choose)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(on ? [.isButton, .isSelected] : [.isButton])
        .accessibilityAction { choose() }
    }

    private var radio: some View {
        Radio(on: on).frame(width: 18).alignmentGuide(.firstTextBaseline) {
            $0[VerticalAlignment.center] + 5
        }
    }

    private var title: some View {
        Text(label(o.key))
            .font(PkType.figure.font(16))
            .tracking(-0.16)
            .foregroundStyle(Pk.ink)
    }

    /// Its file's name, what it is for, the languages a link where it has
    /// several, and the system's own among them.
    private func line(_ many: Bool) -> AttributedString {
        var what = AttributedString(about(o.key))
        if many {
            what.link = Card.languages
            what.underlineStyle = .single
        }
        return AttributedString(o.name + dot) + withOwn(o.entry, what)
    }
}

/// A radio button in ink, as the desktop's takes the accent color, 16 across.
private struct Radio: View {
    let on: Bool

    var body: some View {
        ZStack {
            Circle().stroke(on ? Pk.ink : Pk.inkMuted, lineWidth: 1.5)
            if on { Circle().fill(Pk.ink).frame(width: 8, height: 8) }
        }
        .frame(width: 16, height: 16)
    }
}

/// A model of several's languages (.pk-langs), by their English names in
/// three columns, down each, the system's own in ink; and under them that
/// accuracy varies, and the way to its table.
private struct Languages: View {
    let o: Offer

    var body: some View {
        let names = o.entry.languages.map { ($0, languageName($0)) }.sorted { $0.1 < $1.1 }
        VStack(alignment: .leading, spacing: 8) {
            // two columns, then one, before a name breaks
            ViewThatFits(in: .horizontal) {
                columns(3, names)
                columns(2, names)
                columns(1, names)
            }
            if let url = URL(string: o.entry.accuracy), !o.entry.accuracy.isEmpty {
                Text(accuracy(url))
                    .pk(PkType.small)
                    .foregroundStyle(Pk.inkMuted)
                    .tint(Pk.inkMuted)
            }
        }
        .padding(.top, 8)
    }

    /// The names in n columns, down each, each whole.
    private func columns(_ n: Int, _ names: [(String, String)]) -> some View {
        let rows = (names.count + n - 1) / n
        return HStack(alignment: .top, spacing: 12) {
            ForEach(0..<n, id: \.self) { c in
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(names.dropFirst(c * rows).prefix(rows)), id: \.0) { code, name in
                        Text(name)
                            .font((code == ownLanguage ? PkType.ui : PkType.small).font(13))
                            .foregroundStyle(code == ownLanguage ? Pk.ink : Pk.inkMuted)
                            .lineSpacing(13 * 0.6 - 2)
                            .fixedSize()
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    /// That accuracy varies, and the way to its table, as one paragraph.
    private func accuracy(_ url: URL) -> AttributedString {
        var details = AttributedString(L("setup_details"))
        details.link = url
        details.underlineStyle = .single
        return AttributedString(L("setup_accuracy") + " ") + details
    }
}

/// How far, as the desktop's figure is set (.pk-figure, 700 64/0.9), at
/// the person's text size.
private struct Figure: View {
    let percent: Int
    @Environment(\.dynamicTypeSize) private var type

    var body: some View {
        Text(verbatim: "\(percent)%")
            .font(PkType.figure.font(64))
            .monospacedDigit()
            .foregroundStyle(Pk.ink)
            .frame(height: PkType.figure.scaled(type, 64) * 0.9)
    }
}

/// A download's bar (.pk-bar): 6 tall on sunk, its fill the waveform's blue
/// in the light theme and plume in the dark, ink-muted while stopped.
private struct Bar: View {
    let done: Double
    let stopped: Bool
    let label: String
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Capsule().fill(Pk.sunk)
                Capsule()
                    .fill(stopped ? Pk.inkMuted : scheme == .dark ? Pk.plume : Pk.wave)
                    .frame(width: g.size.width * min(max(done, 0), 1))
                    .animation(.easeOut(duration: 0.3), value: done)
            }
        }
        .frame(height: 6)
        .accessibilityElement()
        .accessibilityLabel(label)
        .accessibilityValue("\(percent(done))%")
    }
}

/// A figure and the words beside it, their feet level, or the words under
/// the figure where the line hasn't room (.pk-spread).
private struct Spread<A: View, B: View>: View {
    @ViewBuilder let a: A
    @ViewBuilder let b: B

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .bottom) {
                a
                Spacer(minLength: 16)
                b
            }
            VStack(alignment: .leading, spacing: 16) {
                a
                b
            }
            // the column's width, the figure on its edge: narrower, the stack
            // was centred, 15 in from the words above (the layout audit)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// A download under way (app.js, downloadHtml): the model's, from where, how
/// far in a figure and in megabytes, at what speed and how long is left, its
/// bar, and the buttons to pause it or let it go.
private struct Downloading: View {
    let d: DownloadState
    let waiting: Int

    var body: some View {
        let name = Models.shared.offers.first { $0.name == d.name }.map { label($0.key) } ?? d.name
        let done = d.total > 0 ? Double(d.got) / Double(d.total) : 0
        let paused = d.step == .paused
        Screen {
            WordmarkButton()
            Intro(
                title: L("download_title", name),
                text: L("download_from", d.name, "models.pktito.net"))
            VStack(spacing: 12) {
                Spread {
                    Figure(percent: percent(d.step == .checking ? 1 : done))
                } b: {
                    Text(progressLine(d))
                        .font(PkType.stamp.font(13))
                        .foregroundStyle(Pk.inkMuted)
                        .padding(.bottom, 4)
                }
                Bar(done: done, stopped: d.step != .downloading, label: L("download_bar"))
            }
            .padding(.top, 8)
        } foot: {
            HStack(spacing: 8) {
                PkButton(
                    text: L(paused ? "download_resume" : "download_pause"),
                    kind: paused ? .primary : .plain, large: true
                ) {
                    if paused { Downloads.shared.retry() } else { Downloads.shared.pause() }
                }
                .disabled(d.step == .checking)
                PkButton(text: L("download_cancel"), kind: .ghost, large: true) {
                    Downloads.shared.cancel()
                }
                Spacer()
                if waiting > 0 {
                    Text(L("setup_waiting", count: waiting)).pk(PkType.small).foregroundStyle(
                        Pk.inkMuted)
                }
            }
        }
    }

    /// How far a download has got, in the desktop's words (app.js,
    /// updateDownload): "262 of 589 MB · 12.3 MB/s · about 30 s left".
    private func progressLine(_ d: DownloadState) -> String {
        let got = mb(d.got)
        let total = mb(d.total)
        switch d.step {
        case .checking: return L("download_checking")
        case .paused: return L("download_progress_paused", got, total)
        default:
            if d.speed > 0 {
                return L(
                    "download_progress_speed", got, total,
                    String(format: "%.1f", locale: Locale.current, d.speed / 1e6),
                    timeLeft(Double(d.total - d.got) / d.speed))
            }
            return L("download_progress", got, total)
        }
    }
}

/// A download that stopped (app.js, failedHtml): what stopped it, what it
/// has kept, whether it is trying again by itself, the particulars folded
/// away, and the ways on: try again, a model file of one's own, or let it go.
private struct Failed: View {
    let d: DownloadState
    @State private var open = false

    var body: some View {
        let done = d.total > 0 ? Double(d.got) / Double(d.total) : 0
        Screen {
            WordmarkButton()
            Intro(title: words(d.problem?.key, host: "models.pktito.net"))
            VStack(spacing: 10) {
                Bar(done: done, stopped: true, label: L("download_bar_stopped"))
                HStack {
                    HStack(spacing: 6) {
                        PkIcon.alert.frame(width: 16, height: 16)
                        Text(
                            d.got > 0
                                ? L("download_stopped_at", mb(d.got), mb(d.total))
                                : L("download_nothing_yet")
                        )
                        .pk(PkType.small)
                    }
                    .foregroundStyle(Pk.err)
                    Spacer()
                    if d.retry && !d.trying {
                        Text(L("download_retrying")).pk(PkType.small).foregroundStyle(Pk.inkMuted)
                    }
                }
            }
            if let detail = d.problem?.detail, !detail.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Button {
                        open.toggle()
                    } label: {
                        HStack(spacing: 6) {
                            PkIcon.down.frame(width: 16, height: 16).rotationEffect(
                                .degrees(open ? 0 : -90))
                            Text(L("download_details")).pk(PkType.ui)
                        }
                        .foregroundStyle(Pk.ink)
                    }
                    .buttonStyle(.plain)
                    if open {
                        Text(detail)
                            .font(PkType.stamp.font(13))
                            .foregroundStyle(Pk.ink)
                            .textSelection(.enabled)
                    }
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Pk.sunk, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
            }
        } foot: {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 8) { buttons }
                VStack(alignment: .leading, spacing: 8) { buttons }
            }
        }
    }

    @ViewBuilder private var buttons: some View {
        PkButton(
            text: L(d.trying ? "download_trying" : "download_try_again"), kind: .primary,
            large: true
        ) {
            Downloads.shared.retry()
        }
        .disabled(d.trying)
        PkButton(text: L("download_use_file"), large: true) { AppState.shared.pick = .model }
        PkButton(text: L("download_cancel"), kind: .ghost, large: true) {
            Downloads.shared.cancel()
        }
    }
}

/// A model file of one's own on its way in (Models.open), as a download is shown.
private struct Copying: View {
    let c: Copy

    var body: some View {
        let done = c.checking ? 1 : c.total > 0 ? Double(c.got) / Double(c.total) : 0
        Screen {
            WordmarkButton()
            Intro(title: L("copy_title"), text: c.name)
            VStack(spacing: 12) {
                Spread {
                    Figure(percent: percent(done))
                } b: {
                    Text(
                        c.checking
                            ? L("download_checking")
                            : c.total > 0
                                ? L("download_progress", mb(c.got), mb(c.total))
                                : L("setup_size", mb(c.got))
                    )
                    .font(PkType.stamp.font(13))
                    .foregroundStyle(Pk.inkMuted)
                    .padding(.bottom, 4)
                }
                Bar(done: done, stopped: false, label: L("copy_title"))
            }
            .padding(.top, 8)
        } foot: {
            PkButton(text: L("download_cancel"), kind: .ghost, large: true) {
                Models.shared.stopCopying()
            }
            .disabled(c.checking)
        }
    }
}

/// iOS's own: the Neural Engine's encoder compiled for this device, the
/// first time after an install or an update, about half a minute; the
/// spinner turns, there being no progress to show.
private struct Preparing: View {
    var body: some View {
        Screen {
            WordmarkButton()
            Intro(title: L("prepare_title", deviceKind), text: L("prepare_why"))
            Spinner().scaleEffect(2).padding(.top, 16).padding(.leading, 8)
        } foot: {
            EmptyView()
        }
    }
}

/// The model in use on the start screen's top edge, a ghost button whose
/// menu is the desktop's (app.js, modelMenu): each catalog model for what
/// it is for and whether it is here, the one in use checked; a model file
/// of one's own while it is in use; Open a model file; and Manage models.
/// A model that is not here is downloaded.
struct ModelMenu: View {
    var body: some View {
        let m = Models.shared
        let title =
            m.own?.lastPathComponent ?? m.offers.first { $0.name == m.model }.map {
                L("models_named", label($0.key))
            } ?? ""
        Menu {
            ForEach(m.offers) { o in
                let on = m.own == nil && o.name == m.model
                Button {
                    guard !on else { return }
                    if o.have {
                        Models.shared.use(o.name)
                        Transcriber.shared.prepare()
                        AppState.shared.say(L("models_using", label(o.key)))
                    } else {
                        Downloads.shared.start(o)
                    }
                } label: {
                    if on {
                        Label(label(o.key), systemImage: "checkmark")
                    } else {
                        Text(label(o.key))
                    }
                    Text(
                        String(withOwn(o.entry, AttributedString(about(o.key))).characters) + dot
                            + (o.have
                                ? L("models_downloaded") : L("models_download_size", mb(o.bytes))))
                }
            }
            if let own = m.own {
                Label(own.lastPathComponent, systemImage: "checkmark")
            }
            Divider()
            Button(L("models_open_file")) { AppState.shared.pick = .model }
            Button(L("models_manage")) { AppState.shared.managing = true }
        } label: {
            HStack(spacing: 6) {
                Text(title).font(PkType.ui.font(14)).lineLimit(2)
                PkIcon.down.frame(width: 16, height: 16)
            }
            .foregroundStyle(Pk.ink)
            .padding(.horizontal, 12)
            .frame(minHeight: 40)
        }
        // its padding outside it, and the chevron's own 3.5 of its 16 (its stroke's
        // cap in): the chevron's ink on the column's edge, as the desktop's (the
        // layout audit: at -12 it stood 3.5 in)
        .padding(.trailing, -15.5)
    }
}
