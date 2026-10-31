import SwiftUI

/// About (ui/index.html, #about; Android's About): the mascot, and under or
/// beside it pktito's name, what it is, its version and the engine's code
/// for this processor, its copyright and links, and the credits: the
/// models, Silero's VAD, and everything else, whose licenses open over
/// About as Privacy does. As the desktop's, the mascot is over the words in
/// a portrait window or one under 600 wide, the words as tall as they are
/// and the mascot the rest, 160 at the least; and beside them otherwise
/// (app.css, .pk-about). Here the mascot is a picture of it at rest
/// (android/mascot.js), where the desktop's turns and chirps.
struct About: View {
    enum Sheet: String, Identifiable {
        case privacy, notices
        var id: String { rawValue }
    }

    @State private var sheet: Sheet?
    @State private var tall: CGFloat = 0

    var body: some View {
        GeometryReader { g in
            let narrow = g.size.width < 600
            let stacked = narrow || g.size.height >= g.size.width
            let wide = g.size.width >= 880
            // the credits beside their names while there is room for both
            let words = Credits(twoColumns: stacked || wide) { sheet = $0 }
            ZStack(alignment: .topLeading) {
                if stacked {
                    // a phone scrolls the two together where the words fill it
                    ScrollView {
                        VStack(spacing: 0) {
                            Stage().frame(height: max(g.size.height - tall, 160))
                            words
                                .padding(narrow ? EdgeInsets(top: 24, leading: 20, bottom: 28, trailing: 20)
                                         : EdgeInsets(top: 28, leading: 40, bottom: 32, trailing: 40))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { tall = $0 }
                        }
                    }
                    .scrollBounceBehavior(.basedOnSize)
                } else {
                    HStack(spacing: 0) {
                        Stage().frame(width: g.size.width * 1.08 / 2.08)
                        // in the middle while they fit, else scrolled from their top
                        ScrollView {
                            words
                                .padding(wide ? EdgeInsets(top: 32, leading: 40, bottom: 32, trailing: 40)
                                         : EdgeInsets(top: 28, leading: 24, bottom: 28, trailing: 24))
                                .frame(maxWidth: .infinity, minHeight: g.size.height, alignment: .leading)
                        }
                        .scrollBounceBehavior(.basedOnSize)
                    }
                }
                // where the work screen's way back is
                IconButton(icon: .back, label: L("back"), kind: .ghost) { AppState.shared.about = false }
                    .padding(.leading, 8)
                    .padding(.top, 11)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L("about_open"))
        .sheet(item: $sheet) { s in
            switch s {
            case .privacy: SheetFrame(title: L("privacy_title")) { Privacy() }
            case .notices: SheetFrame(title: L("notices_title")) { Licenses() }
            }
        }
    }
}

/// The mascot on the surface, as large as its place lets it be.
private struct Stage: View {
    var body: some View {
        Image("Mascot")
            .resizable()
            .scaledToFit()
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Pk.surface)
            .accessibilityLabel(L("about_mascot"))
    }
}

/// About's words, at most the desktop's measure wide (.pk-about-in): 34 em
/// of its 14.
private struct Credits: View {
    let twoColumns: Bool
    let open: (About.Sheet) -> Void

    var body: some View {
        let small = PkType.small
        VStack(alignment: .leading, spacing: 12) {
            Wordmark(size: 56).padding(.bottom, 8)
            let tagline = PkStyle(face: PkType.body.face, size: 15, lineHeight: 1.5, tracking: 0)
            linked(markdown: L("about_tagline"), style: tagline)
                .pk(tagline)
                .foregroundStyle(Pk.ink)
            VStack(alignment: .leading, spacing: 0) {
                // the engine's code for this processor, without its words (Android's)
                Text(L("about_version", version, Core.backend.components(separatedBy: " (")[0]))
                    .pk(small)
                    .foregroundStyle(Pk.inkMuted)
                linked(markdown: L("about_copyright")).pk(small).foregroundStyle(Pk.inkMuted)
                HStack(spacing: 20) {
                    linked(L("about_docs"), "https://pktito.net/docs").pk(small)
                    Button { open(.privacy) } label: {
                        Text(L("about_privacy")).pk(small).underline().foregroundStyle(Pk.link)
                    }
                    .buttonStyle(.plain)
                    linked("GitHub", "https://github.com/peterheb/pktito").pk(small)
                }
                .padding(.top, 6)
            }
            VStack(alignment: .leading, spacing: twoColumns ? 10 : 12) {
                Credit(name: "Parakeet-TDT", twoColumns: twoColumns) {
                    linked(markdown: L("about_models")).pk(small).foregroundStyle(Pk.inkMuted)
                }
                Credit(name: "Silero VAD", twoColumns: twoColumns) {
                    linked(markdown: L("about_silero")).pk(small).foregroundStyle(Pk.inkMuted)
                }
                Credit(name: L("about_else"), twoColumns: twoColumns) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(L("about_else_list_ios")).pk(small).foregroundStyle(Pk.inkMuted)
                        Button { open(.notices) } label: {
                            Text(L("about_licenses")).pk(small).underline().foregroundStyle(Pk.link)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(.top, 20)
        }
        .frame(maxWidth: 476, alignment: .leading)
    }

    private var version: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? ""
    }
}

/// A credit, its name before it 105 wide, or above it where the words are
/// narrow (.pk-credits).
private struct Credit<What: View>: View {
    let name: String
    let twoColumns: Bool
    @ViewBuilder let what: () -> What

    var body: some View {
        let style = PkStyle(face: PkType.ui.face, size: 14, lineHeight: 1.45, tracking: 0)
        if twoColumns {
            HStack(alignment: .top, spacing: 16) {
                Text(name).pk(style).foregroundStyle(Pk.ink).frame(width: 105, alignment: .leading)
                what().frame(maxWidth: .infinity, alignment: .leading)
            }
        } else {
            VStack(alignment: .leading, spacing: 2) {
                Text(name).pk(style).foregroundStyle(Pk.ink)
                what()
            }
        }
    }
}

/// The icon of a link that leaves the app, as an image a line of text can
/// hold: 12 tall, drawn once.
@MainActor private let externalIcon: Image = {
    let r = ImageRenderer(content: PkIcon.external.frame(width: 12, height: 12).foregroundStyle(.black))
    r.scale = 3
    return r.uiImage.map { Image(uiImage: $0).renderingMode(.template) } ?? Image(systemName: "arrow.up.right.square")
}()

/// One of the desktop's *_html strings (Localizable's Markdown, from
/// strings.xml by scripts/strings-ios.py), its links underlined in the
/// link's colour, each, as all of About's links leave the app for another
/// site or the mail app, with the icon of one that does after it, held to
/// its last word (app.js). Its italics in style's face slanted, as the
/// desktop's browser and Android slant a face with no italic of its own:
/// the apps carry none (Android's skew, 0.25).
@MainActor func linked(markdown: String, style: PkStyle? = nil) -> Text {
    let options = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
    guard var a = try? AttributedString(markdown: markdown, options: options) else { return Text(markdown) }
    if let style {
        let slant = CGAffineTransform(a: 1, b: 0, c: 0.25, d: 1, tx: 0, ty: 0)
        let slanted = Font(UIFont(descriptor: style.uiFont.fontDescriptor.withMatrix(slant), size: style.size))
        let runs = a.runs[\.inlinePresentationIntent].filter { $0.0?.contains(.emphasized) == true }.map(\.1)
        for r in runs { a[r].font = slanted }
    }
    return linked(a)
}

/// Words that are a link, as the Markdown's are.
@MainActor func linked(_ text: String, _ url: String) -> Text {
    var a = AttributedString(text)
    a.link = URL(string: url)
    return linked(a)
}

@MainActor private func linked(_ source: AttributedString) -> Text {
    var a = source
    let links = a.runs[\.link].compactMap { $0.0 == nil ? nil : $0.1 }
    for r in links {
        a[r].underlineStyle = .single
        a[r].foregroundColor = Pk.link
    }
    var out = Text(verbatim: "")
    var at = a.startIndex
    for r in links {
        // the word joiner keeps the icon on its word's line
        out = out + Text(AttributedString(a[at..<r.upperBound])) + Text(verbatim: "\u{2060}")
            + Text(externalIcon).foregroundStyle(Pk.link).baselineOffset(-1)
        at = r.upperBound
    }
    return out + Text(AttributedString(a[at..<a.endIndex]))
}

/// A sheet over About (app.css, .pk-notices): its title and the way to
/// close it on the surface, what it holds scrolling under them.
private struct SheetFrame<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(title).pk(PkType.ui).foregroundStyle(Pk.ink).accessibilityAddTraits(.isHeader)
                Spacer()
                IconButton(icon: .x, label: L("sheet_close"), kind: .ghost) { dismiss() }
            }
            .padding(.leading, 24)
            .padding(.trailing, 6)
            .padding(.vertical, 6)
            .background(Pk.surface)
            content()
        }
        .background(Pk.ground)
        .presentationDragIndicator(.hidden)
    }
}

private let sheetText = PkStyle(face: PkType.body.face, size: 14, lineHeight: 1.4, tracking: 0)

/// Privacy's words (ui/index.html, #privacy), as the apps have them.
private struct Privacy: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                para(Text(L("privacy_intro")))
                h3(L("privacy_internet"))
                para(linked(markdown: L("privacy_download")))
                para(linked(markdown: L("privacy_links")))
                h3(L("privacy_files"))
                para(Text(L("privacy_files_text")))
                h3(L("privacy_choices"))
                VStack(alignment: .leading, spacing: 0) {
                    item(L("privacy_choice_file"))
                    item(L("privacy_choice_uninstall_ios"))
                }
                .padding(.bottom, 6)
                h3(L("privacy_questions"))
                para(linked(markdown: L("privacy_ask")))
                Text(L("privacy_updated")).pk(PkType.small).foregroundStyle(Pk.inkMuted)
            }
            .textSelection(.enabled)
            .padding(.horizontal, 24)
            .padding(.top, 16)
            .padding(.bottom, 28)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func para(_ t: Text) -> some View {
        t.pk(sheetText).foregroundStyle(Pk.ink).frame(maxWidth: 560, alignment: .leading).padding(.bottom, 12)
    }

    private func h3(_ s: String) -> some View {
        Text(s)
            .pk(PkStyle(face: PkType.figure.face, size: 17, lineHeight: 1.3, tracking: -0.005))
            .foregroundStyle(Pk.ink)
            .accessibilityAddTraits(.isHeader)
            .padding(.top, 16)
            .padding(.bottom, 6)
    }

    private func item(_ s: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 0) {
            Text(verbatim: "•").pk(sheetText).frame(width: 22, alignment: .trailing).padding(.trailing, 8)
            Text(s).pk(sheetText)
        }
        .foregroundStyle(Pk.ink)
        .frame(maxWidth: 560, alignment: .leading)
        .padding(.bottom, 6)
    }
}

/// A row of the licenses: what it is, its license, its copyright, its
/// texts' keys, and whether its name is code.
private struct Notice: Identifiable {
    let name: String
    let license: String
    let notice: String
    let keys: [String]
    let code: Bool
    var id: String { name }
}

/// notices.json, which ios/notices.py makes: the typefaces and icons, the
/// libraries, and the texts by key.
private struct Notices {
    let type: [Notice]
    let libraries: [Notice]
    let texts: [String: (title: String, text: String)]

    static func read() -> Notices {
        guard let url = Bundle.main.url(forResource: "notices", withExtension: "json"),
              let d = try? Data(contentsOf: url),
              let j = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else {
            return Notices(type: [], libraries: [], texts: [:])
        }
        func rows(_ a: Any?, code: Bool?) -> [Notice] {
            (a as? [[Any]] ?? []).compactMap { r in
                guard r.count >= 4, let n = r[0] as? String, let l = r[1] as? String, let x = r[2] as? String,
                      let k = r[3] as? [String] else { return nil }
                return Notice(name: n, license: l, notice: x, keys: k,
                              code: code ?? (r.count > 4 ? r[4] as? Bool ?? false : false))
            }
        }
        let texts = (j["texts"] as? [String: [String: String]] ?? [:]).mapValues { ($0["title"] ?? "", $0["text"] ?? "") }
        return Notices(type: rows(j["type"], code: false), libraries: rows(j["libraries"], code: nil), texts: texts)
    }
}

/// The licenses (app.js, noticesHtml): the typefaces and icons, the
/// libraries, and the texts they are under, each once, in the order the
/// list first names them.
private struct Licenses: View {
    @State private var n = Notices.read()

    var body: some View {
        var seen = Set<String>()
        let keys = (n.type + n.libraries).flatMap(\.keys).filter { seen.insert($0).inserted }
        let h3 = PkStyle(face: PkType.ui.face, size: 14, lineHeight: 1.4, tracking: 0)
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                Text(L("notices_intro")).pk(PkType.small).foregroundStyle(Pk.inkMuted)
                Text(L("notices_type")).pk(h3).foregroundStyle(Pk.ink).padding(.top, 24).padding(.bottom, 6)
                ForEach(n.type) { NoticeRow(x: $0) }
                Text(L("notices_libraries_ios")).pk(h3).foregroundStyle(Pk.ink).padding(.top, 24).padding(.bottom, 6)
                ForEach(n.libraries) { NoticeRow(x: $0) }
                Text(L("notices_texts")).pk(h3).foregroundStyle(Pk.ink).padding(.top, 24)
                ForEach(keys, id: \.self) { k in
                    let t = n.texts[k] ?? ("", "")
                    VStack(alignment: .leading, spacing: 0) {
                        Text(t.title)
                            .pk(PkStyle(face: PkType.ui.face, size: 13, lineHeight: 1.4, tracking: 0))
                            .foregroundStyle(Pk.ink)
                            .padding(.top, 20)
                            .padding(.bottom, 6)
                        Text(t.text.trimmingCharacters(in: .whitespacesAndNewlines))
                            .pk(PkStyle(face: PkType.code.face, size: 12, lineHeight: 1.5, tracking: 0))
                            .foregroundStyle(Pk.ink)
                            .textSelection(.enabled)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(Pk.surface, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
                    }
                }
            }
            .padding(.horizontal, 24)
            .padding(.top, 16)
            .padding(.bottom, 28)
        }
    }
}

/// A row of the licenses (.pk-nlist li): its name, a crate's in mono, and
/// its license, and under them its copyright.
private struct NoticeRow: View {
    let x: Notice

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                Text(x.name)
                    .pk(x.code ? PkStyle(face: PkType.code.face, size: 12.5, lineHeight: 1.55, tracking: 0) : PkType.small)
                    .foregroundStyle(Pk.ink)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text(x.license).pk(PkType.small).foregroundStyle(Pk.inkMuted)
            }
            Text(x.notice).pk(PkType.small).foregroundStyle(Pk.inkMuted)
        }
        .padding(.vertical, 3)
    }
}
