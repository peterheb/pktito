import SwiftUI

/// What a button is for (Android's Controls.kt, Kind; the desktop's
/// app.css): sunk at rest, plume for the one a screen is for, ghost for one
/// that waits its turn (Stop), and sunk with the error's red for one that
/// deletes.
enum Kind { case plain, primary, ghost, delete }

/// A press's tint over a control's fill, darker in light and lighter in
/// dark, term's own on the strip (design/README.md); no ripple, as the
/// desktop has none. A dimmed control is a disabled one.
struct PkButtonStyle: ButtonStyle {
    var kind: Kind = .plain
    var large = false
    var onTerm = false
    /// its width at the least, its padding in it
    var minWidth: CGFloat = 0
    /// across the width it is given
    var stretch = false
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        let pad: CGFloat = large ? 18 : 12
        configuration.label
            .foregroundStyle(ink)
            .frame(minWidth: max(minWidth - 2 * pad, 0), maxWidth: stretch ? .infinity : nil, minHeight: large ? 48 : 40)
            .padding(.horizontal, pad)
            .background(fill)
            .overlay(configuration.isPressed ? (onTerm ? Pk.tintTermPress : Pk.tintPress) : Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: Radius.radiusUi))
            .opacity(enabled ? 1 : 0.45)
            // 44 tall to a finger where it is drawn 40 (HIG's least)
            .contentShape(Rectangle().inset(by: -2))
    }

    private var fill: Color {
        switch kind {
        case .primary: return Pk.plume
        case .plain, .delete: return Pk.sunk
        case .ghost: return .clear
        }
    }

    private var ink: Color {
        switch kind {
        case .primary: return Pk.onPlume
        case .delete: return Pk.err
        default: return onTerm ? Pk.termInk : Pk.ink
        }
    }
}

/// A button of words, an icon before them or after, 6 apart; 40 tall, 48
/// large, for a finger.
struct PkButton: View {
    let text: String
    var kind: Kind = .plain
    var large = false
    var minWidth: CGFloat = 0
    var stretch = false
    var leading: PkIcon? = nil
    var trailing: PkIcon? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if let leading { leading.frame(width: 16, height: 16) }
                // two lines and more at a large text size, not cut short
                Text(text)
                    .font(PkType.ui.font(large ? 15 : 14))
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                if let trailing { trailing.frame(width: 16, height: 16) }
            }
        }
        .buttonStyle(PkButtonStyle(kind: kind, large: large, minWidth: minWidth, stretch: stretch))
    }
}

/// A button of an icon alone, its words for a reader.
struct IconButton: View {
    let icon: PkIcon
    let label: String
    var kind: Kind = .plain
    var onTerm = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            icon.frame(width: 18, height: 18)
                .frame(width: 40, height: 40)
        }
        .buttonStyle(IconStyle(ghost: kind == .ghost, onTerm: onTerm))
        .accessibilityLabel(label)
    }

    private struct IconStyle: ButtonStyle {
        let ghost: Bool
        let onTerm: Bool
        @Environment(\.isEnabled) private var enabled

        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                .foregroundStyle(onTerm ? Pk.termInk : Pk.ink)
                .background(ghost ? Color.clear : Pk.sunk)
                .overlay(configuration.isPressed ? (onTerm ? Pk.tintTermPress : Pk.tintPress) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: Radius.radiusUi))
                .opacity(enabled ? 1 : 0.45)
                // drawn 40, 44 to a finger (HIG's least), into the gap round it
                .contentShape(Rectangle().inset(by: -2))
        }
    }
}

/// pktito in Archivo 700, and the beak's square after it on the line: a
/// mark, its size its own, not the person's text size.
struct Wordmark: View {
    var size: CGFloat = 20

    var body: some View {
        HStack(alignment: .lastTextBaseline, spacing: size * 0.08) {
            Text(verbatim: "pktito")
                .font(.custom(PkType.figure.face, fixedSize: size))
                .tracking(-0.03 * size)
                .foregroundStyle(Pk.ink)
            Rectangle()
                .fill(Pk.beak)
                .frame(width: size * 0.5, height: size * 0.5)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: "pktito"))
    }
}

/// Beside the file's name while it is at work until its progress shows: a
/// ring in sunk, a quarter of it ink-muted, turning. Not through the whole
/// job: a frame each 16 ms is the CPU's and the battery's, taken from the
/// engine (Android's Spinner; design/README.md, Motion).
struct Spinner: View {
    @State private var turn = false

    var body: some View {
        ZStack {
            Circle().stroke(Pk.sunk, lineWidth: 2)
            Circle()
                .trim(from: 0, to: 0.25)
                .stroke(Pk.inkMuted, style: StrokeStyle(lineWidth: 2, lineCap: .butt))
                .rotationEffect(.degrees(turn ? 360 : 0))
        }
        .frame(width: 14, height: 14)
        .onAppear {
            withAnimation(.linear(duration: 0.9).repeatForever(autoreverses: false)) { turn = true }
        }
        .accessibilityHidden(true)
    }
}

/// Two choices in a sunk track, the one chosen raised to the ground with an
/// ink-muted edge (app.css, .pk-views); the second under the first where a
/// large text size leaves no room for both on a line, each word whole.
struct Switch2: View {
    let first: String
    let second: String
    @Binding var onSecond: Bool

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 2) {
                choice(first, second: false)
                choice(second, second: true)
            }
            VStack(alignment: .leading, spacing: 2) {
                choice(first, second: false)
                choice(second, second: true)
            }
        }
        .padding(2)
        .background(Pk.sunk, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
    }

    private func choice(_ text: String, second: Bool) -> some View {
        let on = onSecond == second
        return Button { onSecond = second } label: {
            Text(text)
                .font(PkType.ui.font(13))
                .fixedSize()
                .foregroundStyle(on ? Pk.ink : Pk.inkMuted)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                // 36 and the switch's 2 round it: 40, as Find's field and the
                // buttons on its line, which it was 3 short of (the layout audit)
                .frame(minHeight: 36)
                .background(on ? Pk.ground : Color.clear, in: RoundedRectangle(cornerRadius: Radius.radiusSm))
                .overlay {
                    if on { RoundedRectangle(cornerRadius: Radius.radiusSm).stroke(Pk.inkMuted, lineWidth: 1) }
                }
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// A button that reads as a link (app.css, .pk-linkbtn): its words in ink,
/// underlined, 500 14/20.
struct LinkButton: View {
    let text: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(text)
                .font(PkType.ui.font(14))
                .underline()
                .foregroundStyle(Pk.ink)
                .padding(.horizontal, 4)
                .padding(.vertical, 8)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// A word that comes and goes at the screen's foot, on term, as the
/// desktop's toast.
struct Toast: View {
    let text: String
    var err = false

    var body: some View {
        // as wide as its words, 420 at most: it filled a phone's width, the
        // desktop's hugs them; the check before them, as the desktop's
        // (app.js, toast), its ink 12 from the end as the words' are from the
        // other; 40 tall at least (the layout audit, 2026-11-01)
        HStack(spacing: 8) {
            (err ? PkIcon.alert : PkIcon.check).frame(width: 16, height: 16).accessibilityHidden(true)
            Text(text).pk(PkType.ui)
        }
        .foregroundStyle(Pk.termInk)
        .padding(.leading, 12)
        .padding(.trailing, 14)
        .padding(.vertical, 8)
        .frame(minHeight: 40)
        .background(Pk.term, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        .shadow(color: .black.opacity(0.18), radius: 12, y: 4)
        .frame(maxWidth: 420)
    }
}

/// A string of the catalog (Localizable.xcstrings, from Android's
/// strings.xml by scripts/strings-ios.py) by its key, with its arguments;
/// in a debug build launched with -pseudo, the pseudo-language's (Pseudo).
func L(_ key: String, _ args: CVarArg...) -> String {
    let format = Pseudo.word(key) ?? Bundle.main.localizedString(forKey: key, value: nil, table: nil)
    return args.isEmpty ? format : String(format: format, locale: Locale.current, arguments: args)
}

/// The same, for a plural: the catalog's form for n.
func L(_ key: String, count n: Int) -> String {
    if let forms = Pseudo.forms(key) {
        return String(format: forms[n == 1 ? "one" : "other"] ?? forms["other"] ?? key, locale: Locale.current, n)
    }
    return String.localizedStringWithFormat(Bundle.main.localizedString(forKey: key, value: nil, table: nil), n)
}

/// The words in scripts/pseudo.py's pseudo-language, longer, accented and
/// bracketed, for a debug build launched with -pseudo: whether the screens
/// hold a language less compact than English, and which text is cut short,
/// hard-coded or put together from pieces (Pseudo/en-XA.json, which a
/// release build hasn't: EXCLUDED_SOURCE_FILE_NAMES).
enum Pseudo {
    #if DEBUG
    private static let table: [String: Any]? = {
        guard CommandLine.arguments.contains("-pseudo"),
              let url = Bundle.main.url(forResource: "en-XA", withExtension: "json"),
              let d = try? Data(contentsOf: url) else { return nil }
        return try? JSONSerialization.jsonObject(with: d) as? [String: Any]
    }()
    #else
    private static let table: [String: Any]? = nil
    #endif

    static func word(_ key: String) -> String? { table?[key] as? String }

    static func forms(_ key: String) -> [String: String]? { table?[key] as? [String: String] }
}
