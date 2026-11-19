import SwiftUI
import UIKit

// Who said what on the work screen (PKT-27; the speakers design, page
// version 7), as Android's app has it (ui/Who.kt) and the desktop's window
// (ui/app.css and app.js, speakers): the switch under the strip, on a
// screen wider than a phone's each speaker's name above the transcript,
// which finds where they speak; a turn's name opening its first sentence,
// which opens Who's speaking, iOS's menu; a sentence held, Change speaker;
// the turn's line beside its rows, the bar over the waveform and an edit's
// Undo.

extension Pk {
    /// Speaker n's color on the page: the tokens' speaker1 to speaker8.
    static func speaker(_ n: Int) -> Color {
        [speaker1, speaker2, speaker3, speaker4, speaker5, speaker6, speaker7, speaker8][
            Speakers.slotOf(n) - 1]
    }

    /// Speaker n's color on the strip's term: the tokens' speakerBar1 to speakerBar8.
    static func speakerBar(_ n: Int) -> Color {
        [
            speakerBar1, speakerBar2, speakerBar3, speakerBar4, speakerBar5, speakerBar6,
            speakerBar7, speakerBar8,
        ][Speakers.slotOf(n) - 1]
    }
}

/// A speaker's mark beside a name (app.css, .pk-mark): a 10 pt circle for
/// speakers 1 to 4 and a 9 pt square with 2 pt corners for 5 to 8, about
/// the same area. Every pair close in color crosses that line but magenta
/// and orchid, 5 and 8 (the design's palette check).
struct SpeakerMark: View {
    let n: Int

    var body: some View {
        let square = Speakers.slotOf(n) > 4
        RoundedRectangle(cornerRadius: square ? 2 : 5)
            .fill(Pk.speaker(n))
            .frame(width: square ? 9 : 10, height: square ? 9 : 10)
            .accessibilityHidden(true)
    }
}

/// Speaker n's mark as an image for a menu's item, in its own color for
/// the theme the screen is in: a menu draws its items' images in its ink
/// unless they keep their own.
@MainActor func markImage(_ n: Int, _ scheme: ColorScheme) -> UIImage {
    let square = Speakers.slotOf(n) > 4
    let side: CGFloat = square ? 9 : 10
    let style: UIUserInterfaceStyle = scheme == .dark ? .dark : .light
    let color = UIColor(Pk.speaker(n)).resolvedColor(
        with: UITraitCollection(userInterfaceStyle: style))
    let image = UIGraphicsImageRenderer(size: CGSize(width: 12, height: 12)).image { _ in
        color.setFill()
        let r = CGRect(x: (12 - side) / 2, y: (12 - side) / 2, width: side, height: side)
        (square ? UIBezierPath(roundedRect: r, cornerRadius: 2) : UIBezierPath(ovalIn: r)).fill()
    }
    return image.withRenderingMode(.alwaysOriginal)
}

/// The switch under the strip (app.css, .pk-spk; Android's SpeakersSwitch),
/// in a sunk track of its own: Speakers, raised to the ground with an
/// ink-muted edge while it is on, a check before the word then; 40 tall, as
/// the field and the buttons it lines up with. One width on or off, so that
/// a second tap where the first was turns them off again (Peter,
/// 2026-11-15): off, the check keeps its place unseen and the word moves half
/// of it over, to the middle, in the strip's 0.15 s. busy, it waits for the
/// finding or the download under way.
struct SpeakersSwitch: View {
    let on: Bool
    let busy: Bool
    /// what a tap does, for VoiceOver: show who said what, or hide them
    let hint: String
    let action: () -> Void
    /// what it says: Speakers, or Only matches (Finder.only)
    var label: String? = nil

    var body: some View {
        Button {
            if !busy { action() }
        } label: {
            SwitchFace(on: on, label: label ?? L("speakers_switch"))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
        .accessibilityHint(hint)
    }
}

/// What the switch looks like (SpeakersSwitch), and a menu's that opens from
/// it (SpeakersMenu): a chevron after the word then.
struct SwitchFace: View {
    let on: Bool
    let label: String
    var menu = false

    var body: some View {
        HStack(spacing: 6) {
            PkIcon.check.frame(width: 12, height: 12).opacity(on ? 1 : 0)
            Text(label)
                .font(PkType.ui.font(13))
                .lineLimit(1)
                .frame(minHeight: 16)
                .offset(x: on ? 0 : -9)
            if menu {
                PkIcon.down.frame(width: 12, height: 12).foregroundStyle(Pk.inkMuted)
            }
        }
        .foregroundStyle(on ? Pk.ink : Pk.inkMuted)
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(
            on ? Pk.ground : Color.clear, in: RoundedRectangle(cornerRadius: Radius.radiusSm)
        )
        .overlay {
            if on {
                RoundedRectangle(cornerRadius: Radius.radiusSm).strokeBorder(
                    Pk.inkMuted, lineWidth: 1)
            }
        }
        .padding(2)
        .background(Pk.sunk, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        .animation(.easeInOut(duration: 0.15), value: on)
        .contentShape(Rectangle())
    }
}

/// While a transcription finds its speakers as it goes, the switch, which
/// opens a menu of one: Turn off speakers, which transcribes faster (Peter,
/// 2026-11-18: for a long recording of one voice). The speaker model takes as
/// long again as the words or more on a phone: on a Galaxy S24 Ultra a
/// 39-minute podcast took 1.6 to 2.6 times as long with it, on an iPhone 16e
/// 4.7 times (Android's RunningSpeakers).
struct RunningSpeakers: View {
    var body: some View {
        Menu {
            Button {
                Transcriber.shared.quiet()
            } label: {
                Text(L("speakers_turn_off"))
                Text(L("speakers_turn_off_note"))
            }
        } label: {
            SwitchFace(on: true, label: L("speakers_switch"), menu: true)
        }
        .buttonStyle(.plain)
        .fixedSize()
        .accessibilityHint(L("speakers_turn_off"))
    }
}

/// The speakers of a transcript, for a phone, where the names over the
/// transcript give way to the text (Chips): Hide speakers, each speaker,
/// which finds where they speak as a name does (the one found checked), and
/// Rename, a speaker each (Android's SpeakersMenu; Peter, 2026-11-18: "my
/// intuition is to click Speakers like I'll get a drop-down menu").
struct People {
    let present: [Int]
    let nameOf: (Int) -> String
    /// the speaker found, 0 for none
    let found: Int
    let find: (Int) -> Void
    let rename: (Int) -> Void
    let hide: () -> Void
}

/// The speakers' menu (People), opened by label.
struct SpeakersMenu<Face: View>: View {
    let people: People
    @ViewBuilder let label: () -> Face
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        Menu {
            Button(L("speakers_hide")) { people.hide() }
            Section {
                ForEach(people.present, id: \.self) { n in
                    Button {
                        if n != people.found { people.find(n) }
                    } label: {
                        Label {
                            Text(people.nameOf(n))
                        } icon: {
                            Image(uiImage: markImage(n, scheme))
                        }
                        if n == people.found { Image(systemName: "checkmark") }
                    }
                    .accessibilityLabel(L("speakers_chip_label", people.nameOf(n)))
                }
            }
            Menu(L("speakers_rename_menu")) {
                ForEach(people.present, id: \.self) { n in
                    Button(people.nameOf(n)) { people.rename(n) }
                }
            }
        } label: {
            label()
        }
        .buttonStyle(.plain)
    }
}

/// Lines of views, each as wide as it is, wrapping where the next won't fit
/// (Android's FlowRow).
struct Flow: SwiftUI.Layout {
    var spacing: CGFloat = 2

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = lines(proposal.width ?? .infinity, subviews)
        let w = rows.map { $0.width }.max() ?? 0
        let h = rows.reduce(0) { $0 + $1.height } + spacing * CGFloat(max(rows.count - 1, 0))
        return CGSize(width: w, height: h)
    }

    func placeSubviews(
        in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()
    ) {
        var y = bounds.minY
        for row in lines(bounds.width, subviews) {
            var x = bounds.minX
            for i in row.items {
                let s = subviews[i].sizeThatFits(.unspecified)
                subviews[i].place(
                    at: CGPoint(x: x, y: y + (row.height - s.height) / 2), proposal: .unspecified)
                x += s.width + spacing
            }
            y += row.height + spacing
        }
    }

    private func lines(_ width: CGFloat, _ subviews: Subviews) -> [(
        items: [Int], width: CGFloat, height: CGFloat
    )] {
        var rows: [(items: [Int], width: CGFloat, height: CGFloat)] = []
        for (i, v) in subviews.enumerated() {
            let s = v.sizeThatFits(.unspecified)
            if let last = rows.last, last.width + spacing + s.width <= width {
                rows[rows.count - 1].items.append(i)
                rows[rows.count - 1].width += spacing + s.width
                rows[rows.count - 1].height = max(last.height, s.height)
            } else {
                rows.append(([i], s.width, s.height))
            }
        }
        return rows
    }
}

/// With the names shown on a screen wider than a phone's, each speaker's,
/// in the order they first speak (app.css, .pk-names): a name finds where
/// its speaker speaks (Finder.findWho), the one found ringed, as the switch
/// is while on. 32 tall, 44 to a finger past its edges. With all 8 of the
/// diarizer's speakers in use, an (i) after them, wrapping with the last.
struct Chips: View {
    let present: [Int]
    let nameOf: (Int) -> String
    /// the speaker found, whose name is pressed; 0 for none
    let found: Int
    let onFind: (Int) -> Void

    var body: some View {
        Flow(spacing: 2) {
            ForEach(present, id: \.self) { n in
                if n == present.last && present.count >= Speakers.most {
                    HStack(spacing: 2) {
                        chip(n)
                        Limit()
                    }
                } else {
                    chip(n)
                }
            }
        }
    }

    private func chip(_ n: Int) -> some View {
        let on = n == found
        let name = nameOf(n)
        return Button {
            onFind(n)
        } label: {
            HStack(spacing: 8) {
                SpeakerMark(n: n)
                Text(name)
                    .font(PkType.ui.font(13))
                    .foregroundStyle(Pk.ink)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .frame(maxWidth: 240, alignment: .leading)
                    .fixedSize()
            }
            .padding(.horizontal, 8)
            .frame(minHeight: 32)
            .background(
                on ? Pk.tint : Color.clear, in: RoundedRectangle(cornerRadius: Radius.radiusSm)
            )
            .overlay {
                if on {
                    RoundedRectangle(cornerRadius: Radius.radiusSm).strokeBorder(
                        Pk.inkMuted, lineWidth: 1)
                }
            }
            .contentShape(Rectangle().inset(by: -6))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L("speakers_chip_label", name))
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The (i) after the names with all 8 of the diarizer's speakers in use
/// (app.js, limitHtml): a recording with more voices has some of them mixed
/// up, which at 12 readers of 12 chapters looked like a fault (Peter,
/// 2026-11-14, Alice's chapter 11). A tap says so, in a popover.
private struct Limit: View {
    @State private var open = false

    var body: some View {
        let head = L("speakers_limit", Speakers.most)
        Button {
            open = true
        } label: {
            PkIcon.info.frame(width: 16, height: 16).foregroundStyle(Pk.inkMuted)
                .frame(width: 32, height: 32)
                .contentShape(Rectangle().inset(by: -6))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(head)
        .popover(isPresented: $open) {
            VStack(alignment: .leading, spacing: 4) {
                Text(head).font(PkType.ui.font(14)).bold().foregroundStyle(Pk.ink)
                Text(L("speakers_limit_about", Speakers.most))
                    .pk(PkType.small)
                    .foregroundStyle(Pk.inkMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(16)
            .frame(maxWidth: 320, alignment: .leading)
            .presentationCompactAdaptation(.popover)
        }
    }
}

/// A row's place in its speaker's turn, for the line beside its text
/// (app.css, .pk-seg[data-spk]; Peter, 2026-11-15): a 3 pt line in the
/// speaker's colour down each row of a turn, so that a turn reads as a
/// paragraph of its own and its name, at its top, says whose. It starts and
/// ends 8 pt inside the turn's first and last rows, 2 inside their text,
/// round there.
struct TurnLine: Equatable {
    let speaker: Int
    let first: Bool
    let last: Bool
}

/// The turn's line in a row's box, its middle mid from the row's leading
/// edge (Work, SegRow).
struct TurnLineShape: Shape {
    let line: TurnLine
    let mid: CGFloat

    func path(in r: CGRect) -> Path {
        let w: CGFloat = 3
        let top: CGFloat = line.first ? 8 : 0
        let bottom = r.height - (line.last ? 8 : 0)
        guard bottom > top else { return Path() }
        let head: CGFloat = line.first ? w / 2 : 0
        let foot: CGFloat = line.last ? w / 2 : 0
        return Path(
            roundedRect: CGRect(x: mid - w / 2, y: top, width: w, height: bottom - top),
            cornerRadii: RectangleCornerRadii(
                topLeading: head, bottomLeading: foot, bottomTrailing: foot, topTrailing: head))
    }
}

/// Who's speaking, for a turn, or with alone for one sentence of it
/// (Change speaker; app.js, openWho): the speakers as a choice, the turn's
/// own checked, its times over them; then Rename, Find where they speak, to
/// hear who they are (a phone has no names over the transcript to find them
/// by), and New speaker, for a voice the model merged into someone else's:
/// the lowest number not in use, so that no two share a color, none past
/// eight, which the note under it says. iOS's menu, the way an iOS app
/// offers a choice and the actions beside it.
struct WhoItems: View {
    let turn: Speakers.Turn
    let present: [Int]
    let names: [String]
    let times: String
    var alone = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let s = AppState.shared
        let name = Speakers.nameOf(names, turn.speaker)
        let fresh = Speakers.fresh(present)
        // each speaker a toggle, the turn's own on, as an inline picker's
        // items are: in a section of their own, whose header a picker's
        // nested menu would drop. The times on a line of their own, where
        // the desktop has them after a dot: a phone's menu is too narrow for
        // both, and broke the times in two
        Section((alone ? L("speakers_change") : L("speakers_who")) + "\n" + times) {
            ForEach(present, id: \.self) { n in
                Toggle(
                    isOn: Binding(
                        get: { turn.speaker == n },
                        set: { if $0 { s.moveSpeaker(turn, to: n, fresh: false) } })
                ) {
                    Label {
                        Text(Speakers.nameOf(names, n))
                    } icon: {
                        Image(uiImage: markImage(n, scheme)).renderingMode(.original)
                    }
                }
            }
        }
        Section {
            Button(L("speakers_rename", name)) { s.renaming = turn.speaker }
            Button(L("speakers_find", name)) { s.finder.findWho(turn.speaker) }
            Button(L("speakers_new")) {
                if let fresh { s.moveSpeaker(turn, to: fresh, fresh: true) }
            }
            .disabled(fresh == nil)
        }
        if fresh == nil {
            Section { Text(L("speakers_limit_about", Speakers.most)) }
        }
    }
}

/// The speaker bar over the waveform (app.js, drawBar; Android's
/// SpeakerBar): the transcript's turns on a 4 pt line, each in its
/// speaker's color; while speakers are found, the stretches the diarizer has
/// heard so far (finding). Each point of it shows whoever talks most of it
/// (Speakers.pixels), so that an hour still reads, and a turn too short to
/// win a point still gets the one at its middle; a run of one speaker's
/// wider than 3 stands 1 clear of the next either side. 10 tall; the strip
/// puts it 8 over the waveform.
struct SpeakerBar: View {
    let stretches: [Stretch]
    let duration: Double
    let upto: Double
    var finding = false
    /// one speaker's turns alone (a speaker found): each at its own times,
    /// not by the point, from half a tick before its start, so that a turn's
    /// mark hangs from the same line as its match's tick and the playhead
    /// there, a short one centred on it (Peter, Android's on the S25, 2026-11-18)
    var exact = false

    var body: some View {
        let parts = Speakers.parts(stretches, upto: upto)
        Canvas { ctx, size in
            guard duration > 0 else { return }
            if exact {
                let half: CGFloat = 1.5
                for s in stretches where s.start < upto {
                    let a = CGFloat(s.start / duration) * size.width - half
                    let b = max(CGFloat(min(s.end, upto) / duration) * size.width, a + 2 * half)
                    ctx.fill(
                        Path(CGRect(x: a, y: 6, width: b - a, height: 4)),
                        with: .color(Pk.speakerBar(s.speaker)))
                }
                return
            }
            let px = max(1, Int(size.width))
            let per = duration / Double(px)
            let who = Speakers.pixels(parts, px, per, everyTurn: !finding)
            var x = 0
            while x < px {
                let n = who[x]
                var e = x + 1
                while e < px && who[e] == n { e += 1 }
                if n != 0 {
                    let w = e - x
                    let inset = w > 3 ? 1 : 0
                    ctx.fill(
                        Path(
                            CGRect(
                                x: CGFloat(x + inset), y: 6, width: CGFloat(w - 2 * inset),
                                height: 4)),
                        with: .color(Pk.speakerBar(n)))
                }
                x = e
            }
        }
        .frame(height: 10)
        .accessibilityLabel(L("speakers_bar"))
    }
}

/// The toast of an edit that can be undone (app.css, .pk-toast .pk-undo):
/// what it did, and Undo on the term's tint, 40 tall in the toast's 48.
struct UndoToast: View {
    let text: String
    let onUndo: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Text(text).pk(PkType.ui).foregroundStyle(Pk.termInk)
            Button(action: onUndo) {
                Text(L("speakers_undo"))
                    .font(PkType.ui.font(13))
                    .foregroundStyle(Pk.termInk)
                    .padding(.horizontal, 10)
                    .frame(minHeight: 40)
                    .background(Pk.tintTerm, in: RoundedRectangle(cornerRadius: Radius.radiusSm))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
        .padding(.leading, 16)
        .padding(.trailing, 4)
        .padding(.vertical, 4)
        .background(Pk.term, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        .overlay(
            RoundedRectangle(cornerRadius: Radius.radiusUi).strokeBorder(Pk.toastEdge, lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.18), radius: 12, y: 4)
        .frame(maxWidth: 420)
    }
}
