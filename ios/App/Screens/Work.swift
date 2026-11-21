import SwiftUI
import UIKit

/// One file at work and then its transcript (ui/index.html, #work; Android's
/// Work): the header with the way back, the name, and what can be done with
/// it (at work only Stop; Copy and Share once it is done), the strip, Find,
/// a line for what went wrong, and the rows.
///
/// Where the keyboard leaves the rows under `least` below the header, the
/// strip and Find's line, as on a phone on its side, those make room
/// while it is up: the search takes the top line, its words, count and
/// steps, and the rows the rest, the match in sight. The keyboard put away
/// brings them back with the search kept; the field's × closes the search
/// as it always does. Peter asked for what is most usable (2026-11-01);
/// the same in Android's app.
struct Work: View {
    /// a search's matches in the rows shown, a finished transcript's only,
    /// and each row's of them
    @State private var hits: [Hit] = []
    @State private var marks: [Int: [Marked]] = [:]
    /// the screen's height above the keyboard, and as if there were none
    @State private var height: CGFloat = 0
    @State private var whole: CGFloat = 0
    /// the header's, strip's, Find line's and message's height as last
    /// shown, which the rows' room is reckoned from while they make room
    @State private var above: CGFloat = 0
    /// the least of the rows worth the header and the strip with the
    /// keyboard up: 160 at the default text size, scaled as the text is
    /// (Android's 160 dp, Dp.withText)
    @ScaledMetric(relativeTo: .body) private var least: CGFloat = 160
    /// its own width 600 or more: its edges 24, as the desktop's wide work
    /// and Material's; 16 under that, a phone's (the layout audit, 2026-11-01)
    @State private var roomy = false
    /// the waveform's seeks, each a jump the rows bring into sight (Rows)
    @State private var jumped = 0
    /// where a finger scrubbing the waveform has the playhead, in seconds, and
    /// a count of its moves, for the transcript to follow it (Rows)
    @State private var scrubbed = Scrub()

    /// What the matches are found anew for: a search, or a speaker found
    /// and their turns, which a change of speaker moves.
    private struct Sought: Equatable {
        let query: String
        let who: Int
        let review: Bool
        let id: String
        let count: Int
        let speakers: [Int?]
    }

    var body: some View {
        let t = Transcriber.shared
        let s = AppState.shared
        let review = t.step == .done || t.step == .stopped
        let rows = t.segments
        let room = makesRoom(review: review, rows: rows)
        let edge: CGFloat = roomy ? 24 : 16
        // who said what (Speakers): those of the transcript in the order they
        // first speak, their names shown unless the transcript's switch hid
        // them, whatever the start screen's (Peter, 2026-11-19), one as much as
        // eight: the model may have heard two voices as one, which New
        // speaker puts right (Peter, 2026-11-14); the formats name no one
        // for one voice (app.js, turnsShown)
        let spk = Speakers.shared
        let present = Speakers.present(rows)
        let spoken = !t.hidden && !present.isEmpty
        // with one voice too, which the bar shows where it was heard (Peter,
        // 2026-11-18; app.js, turnsShown)
        let turns = spoken
        // a speaker found, their turns (Finder.findWho), or with words typed
        // the words' in their sentences alone, while their name shows:
        // switched off, or joined to another by a rename, the search is let go
        let who = spoken && present.contains(s.finder.who) ? s.finder.who : 0
        // Only matches (Finder.only): the rows the list shows, while
        // something is sought; off again once nothing is
        let sought =
            review && (!s.finder.query.trimmingCharacters(in: .whitespaces).isEmpty || who != 0)
        let view = s.finder.only && sought ? shownRows(rows, hits, s.finder.query, who: who) : nil
        // the strip keeps the bar's place: the turns shown, the speakers being
        // found, or a transcription finding them as it goes (app.js, barShown)
        // and a speaker found, one voice or more, whose own turns it shows
        // (Peter, 2026-11-18)
        let place =
            (review && (turns || spk.finding)) || (review && spoken && who != 0)
            || (t.running && t.diarizing)
        VStack(spacing: 0) {
            VStack(spacing: 0) {
                if !room { Header(review: review, edge: edge, shown: !rows.isEmpty) }
                if t.step != .error && !room {
                    Strip(
                        review: review && !spk.finding,
                        left: spk.finding
                            ? L(spk.reading ? "status_reading" : "speakers_finding") : left,
                        why: t.why,
                        locate: t.locate ? { s.pick = .locate } : nil,
                        hits: hits, tickAt: s.finder.at,
                        // a tick: its match, and the playhead there (app.js,
                        // #ticks); after the seek, which puts the reader on the
                        // match it last passed, a tick's own, where two share its time
                        onTick: { k in
                            seek(hits[k].time)
                            s.finder.go(hits, k)
                        },
                        seek: {
                            seek($0 * t.duration)
                            jumped += 1
                        },
                        heard: spk.finding ? spk.heard : nil,
                        bar: place
                            ? Strip.Bar(
                                stretches: spk.finding
                                    ? spk.growing
                                    : who != 0
                                        ? barOf(rows).filter { $0.speaker == who } : barOf(rows),
                                upto: t.running ? t.done * t.duration : .greatestFiniteMagnitude,
                                finding: spk.finding, exact: who != 0 && !spk.finding) : nil,
                        // the transcript at the sentence under a scrubbing finger
                        onScrub: { scrubbed = Scrub(at: $0 * t.duration, n: scrubbed.n + 1) }
                    )
                    // 16 under the header's buttons: their 11 and 5
                    .padding(.horizontal, edge)
                    .padding(.top, 5)
                }
                // while it transcribes with its speakers, the way to go on without them
                if t.running && t.diarizing && !room {
                    RunningSpeakers()
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, edge)
                        .padding(.top, 16)
                }
                if review && !rows.isEmpty {
                    // a step takes the playhead to its match, at rest too (Finder.step)
                    FindLine(
                        hits: hits, seek: { seek($0) }, alone: room,
                        speakers: switchOf(present: present),
                        only: sought
                            ? FindLine.Switch(
                                on: s.finder.only, busy: false,
                                hint: L(s.finder.only ? "find_only_all" : "find_only_show"),
                                toggle: { s.finder.only.toggle() }) : nil,
                        nameOf: { Speakers.nameOf(t.names, $0) },
                        // a phone's: the names over the transcript give way to the text
                        people: spoken && !roomy && spk.offer != nil
                            ? People(
                                present: present, nameOf: { Speakers.nameOf(t.names, $0) },
                                found: who, find: { s.finder.findWho($0) },
                                rename: { s.renaming = $0 },
                                hide: { s.toggleSpeakers() },
                                undo: s.last?.text, onUndo: { s.undoLast() }) : nil
                    )
                    .padding(.horizontal, edge)
                    // alone, the search where the header's buttons were
                    .padding(.top, room ? 11 : 16)
                }
                // with the names shown on a screen wider than a phone's, each
                // speaker's name, which finds where they speak (app.css,
                // .pk-names); a phone finds them from Who's speaking
                if review && !rows.isEmpty && spk.offer != nil && spoken && !room && roomy {
                    Chips(
                        present: present, nameOf: { Speakers.nameOf(t.names, $0) }, found: who,
                        onFind: { s.finder.findWho($0) }
                    )
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, edge)
                    .padding(.top, 8)
                }
                if let msg = message(rows), !room {
                    HStack(spacing: 6) {
                        if t.error != nil {
                            PkIcon.alert.frame(width: 16, height: 16).foregroundStyle(Pk.err)
                        }
                        Text(msg).font(PkType.body.font(15)).foregroundStyle(
                            t.error != nil ? Pk.err : Pk.inkMuted)
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, edge)
                    .padding(.top, 16)
                }
            }
            // its whole height always, the rows giving way: a short window
            // squeezed the header from its 98 to 62, its buttons pushed up
            // to the window's top edge (Peter's iPad in a window at its least,
            // 375 x 466, 2026-11-01)
            .fixedSize(horizontal: false, vertical: true)
            // what the rows' room is reckoned from while the rest makes room
            .onGeometryChange(for: CGFloat.self) {
                $0.size.height
            } action: {
                if !room { above = $0 }
            }
            Rows(
                rows: rows, review: review, marks: marks, hits: hits, view: view,
                stats: stats(review), edge: edge, jump: jumped, scrub: scrubbed,
                speakers: Rows.Voices(
                    turns: turns, editable: review && spoken, present: present, names: t.names,
                    joined: spk.joined,
                    gutter: turns || spk.finding || (t.running && t.diarizing),
                    heard: spk.finding && !turns
                        ? Rows.Heard(stretches: spk.growing, upto: spk.heard * t.duration) : nil)
            ) { time, play in
                seek(time, play: play)
            }
        }
        .onGeometryChange(for: CGFloat.self) {
            $0.size.height
        } action: {
            height = $0
        }
        .onGeometryChange(for: Bool.self) {
            $0.size.width >= 600
        } action: {
            roomy = $0
        }
        // the height as if the keyboard weren't up, which it makes room in
        .background {
            GeometryReader { g in
                Color.clear.onChange(of: g.size.height, initial: true) { _, h in whole = h }
            }
            .ignoresSafeArea(.keyboard)
        }
        .onChange(of: room) {
            trace("room for the keyboard: \(room); \(height) of \(whole), \(above) above the rows")
        }
        .onChange(of: s.finder.who != 0 && who == 0) { _, gone in
            if gone { s.finder.clear() }
        }
        .onChange(of: sought) { _, now in if !now { s.finder.only = false } }
        .onChange(
            of: Sought(
                query: s.finder.query, who: who, review: review, id: t.id, count: rows.count,
                speakers: rows.map(\.speaker)),
            initial: true
        ) { was, now in
            let words = !s.finder.query.trimmingCharacters(in: .whitespaces).isEmpty
            hits =
                !review
                ? []
                : who != 0 && !words ? turnsOf(rows, who) : findIn(rows, s.finder.query, who: who)
            marks = Dictionary(
                grouping: hits.indices.map { Marked(k: $0, hit: hits[$0]) }, by: \.hit.row)
            // a search typed at rest takes the playhead to its match, so that
            // Play starts where the rows went (app.js, runFind), and so does
            // a speaker found; the rows marked anew for the list, or after a
            // change of speaker, leave it where it is
            let typed = (was.query != now.query || was.who != now.who) && !Player.shared.playing
            s.finder.settle(
                hits, position: Player.shared.position, seek: typed ? { seek($0) } : nil)
        }
    }

    /// Whether the header and the strip make room for the keyboard: it is
    /// up, a transcript is searched, and under them and Find's line, 16
    /// over the rows, the rows would keep less than `least`. Reckoned from
    /// their height as last shown, so that making room doesn't undo itself;
    /// a hardware keyboard's bar leaves the room it finds.
    private func makesRoom(review: Bool, rows: [Segment]) -> Bool {
        review && !rows.isEmpty && whole > height && height - above - 16 < least
    }

    /// The speakers' switch on the line under the strip, where the build
    /// offers a speaker model and the transcript is finished; pressed while
    /// names are shown or being found: on with none found yet, it finds them
    /// (AppState.toggleSpeakers).
    private func switchOf(present: [Int]) -> FindLine.Switch? {
        let spk = Speakers.shared
        let d = Downloads.shared
        guard spk.offer != nil else { return nil }
        let busy = spk.finding || (d.speakers && d.state?.step != .failed)
        let pressed = !Transcriber.shared.hidden && (!present.isEmpty || busy)
        return FindLine.Switch(
            on: pressed, busy: busy,
            hint: L(pressed && !present.isEmpty ? "speakers_hide" : "speakers_show"),
            toggle: { AppState.shared.toggleSpeakers() })
    }

    /// The bar's turns: the transcript's sentences, each its speaker's, a
    /// change included (app.js, drawBar).
    private func barOf(_ rows: [Segment]) -> [Stretch] {
        rows.compactMap { s in s.speaker.map { Stretch(start: s.start, end: s.end, speaker: $0) } }
    }

    /// The playhead at time, and the reader on the match it last passed.
    private func seek(_ time: Double, play: Bool = false) {
        Player.shared.seek(time, play: play)
        AppState.shared.finder.follow(hits, position: time, jump: true)
    }

    /// At work, the strip's time's place: the step, then the pace.
    private var left: String {
        let t = Transcriber.shared
        switch t.step {
        case .loading: return L("status_loading")
        case .reading: return L("status_reading")
        case .transcribing where t.done > 0 && t.pace > 0:
            return L("real_time", String(format: "%.1f", locale: .current, t.pace))
        default: return ""
        }
    }

    private func message(_ rows: [Segment]) -> String? {
        let t = Transcriber.shared
        if let e = t.error { return e }
        if t.step == .stopped { return L("status_stopped_at", percent(t.done)) }
        if t.step == .done && rows.isEmpty { return L("no_speech") }
        return nil
    }

    private func stats(_ review: Bool) -> String? {
        let t = Transcriber.shared
        guard review && !t.segments.isEmpty && t.transcribed > 0 else { return nil }
        return L(
            "stats_transcribed", clock(t.transcribed), pace(t.done * t.duration, t.transcribed))
    }
}

/// The header's buttons in the middle of its 62 (app.css, .pk-head), the way
/// back's arrow on the cards' line, 16 from the screen's edge, its button's
/// padding outside it: the arrow's ink is 14 into its 40, so the button is
/// at 2.
private struct Header: View {
    let review: Bool
    /// the screen's edge, 16 or 24 (Work)
    let edge: CGFloat
    /// whether there are rows to copy and share
    let shown: Bool
    /// what the name keeps beside the buttons before it goes under them:
    /// 160 at the default text size, about twenty characters, and as many
    /// at any other
    @ScaledMetric(relativeTo: .subheadline) private var floor: CGFloat = 160
    /// past the window's controls, where they are in its corner (Clearance)
    @State private var clear = Clearance()

    var body: some View {
        let t = Transcriber.shared
        // where the name would keep under its floor beside the buttons, the way
        // back and the buttons keep the bar's line and the name goes under
        // them, as a large title does on both platforms (agreed with
        // Android's app, 2026-10-31)
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                back
                name.frame(
                    minWidth: floor, idealWidth: floor, maxWidth: .infinity, alignment: .leading)
                buttons
            }
            .padding(.leading, clear.leading)
            // the name 16 under the buttons, the screen's one step, as in
            // Android's app (2026-10-31)
            VStack(alignment: .leading, spacing: 16) {
                HStack(spacing: 8) {
                    back
                    Spacer(minLength: 0)
                    buttons
                }
                .padding(.leading, clear.leading)
                // on the cards' edge
                name.padding(.leading, Transcriber.shared.running ? 0 : 14)
            }
        }
        .padding(.leading, t.running ? edge : edge - 14)
        .padding(.trailing, edge)
        .padding(.vertical, 11)
        .frame(minHeight: 62)
        .windowControls($clear)
    }

    @ViewBuilder private var back: some View {
        if !Transcriber.shared.running {
            IconButton(icon: .back, label: L("back"), kind: .ghost) { AppState.shared.back() }
        }
    }

    private var name: some View {
        let t = Transcriber.shared
        return HStack(spacing: 10) {
            // a file's name: one line, cut in the middle, which keeps its extension
            Text(t.name).pk(PkType.ui).foregroundStyle(Pk.ink).lineLimit(1).truncationMode(.middle)
            // until the ring has progress: reading, the model loading, the first chunk
            if t.running && !(t.step == .transcribing && t.done > 0) { Spinner() }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Stop, or Copy and Share, each its words' whole width
    @ViewBuilder private var buttons: some View {
        let t = Transcriber.shared
        let s = AppState.shared
        if t.running {
            PkButton(text: L("stop"), kind: .ghost) { t.stop() }.fixedSize()
        }
        if review && shown {
            IconButton(icon: .copy, label: L("copy_text")) { s.copy() }
            ShareAs().fixedSize()
        }
    }
}

/// Share, the plume button a finished transcript is for, and its menu of
/// formats, the desktop's Save as's: each sends the transcript as a file of
/// its kind to the app picked in the share sheet, which has Save to Files
/// too. Where the desktop's button saves, a phone's shares: on one a
/// transcript goes to another app, a chat or mail, far more often than to a
/// folder (Android's ShareAs).
private struct ShareAs: View {
    @State private var anchor = Anchor()

    var body: some View {
        let s = AppState.shared
        let name = Transcriber.shared.name
        let bare = (name as NSString).deletingPathExtension
        let stem = bare.isEmpty ? name : bare
        Menu {
            ForEach(Format.all) { f in
                Button {
                    if let url = s.file(f.ext) { share(url, from: anchor.view) }
                } label: {
                    Text(L(s.times ? f.withTimes : f.label))
                    Text(verbatim: "\(stem).\(f.ext)")
                }
            }
            Divider()
            Toggle(L("save_times"), isOn: Binding(get: { s.times }, set: { s.times = $0 }))
                .menuActionDismissBehavior(.disabled)
        } label: {
            HStack(spacing: 6) {
                Text(L("share")).font(PkType.ui.font(14)).lineLimit(1)
                PkIcon.down.frame(width: 16, height: 16)
            }
            .foregroundStyle(Pk.onPlume)
            .padding(.horizontal, 12)
            .frame(minHeight: 40)
            .background(Pk.plume, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        }
        .background(AnchorView(anchor: anchor))
    }
}

/// A view's UIKit view, kept for a popover to point at: the share sheet's,
/// on an iPad, at the button it came from.
final class Anchor {
    weak var view: UIView?
}

struct AnchorView: UIViewRepresentable {
    let anchor: Anchor

    func makeUIView(context: Context) -> UIView {
        let v = UIView()
        v.isUserInteractionEnabled = false
        anchor.view = v
        return v
    }

    func updateUIView(_ v: UIView, context: Context) { anchor.view = v }
}

/// The share sheet for url, from the view it came from.
@MainActor
func share(_ url: URL, from view: UIView?) {
    let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
    guard var top = (view?.window ?? scene?.keyWindow)?.rootViewController else { return }
    while let p = top.presentedViewController { top = p }
    let vc = UIActivityViewController(activityItems: [url], applicationActivities: nil)
    if let pop = vc.popoverPresentationController {
        pop.sourceView = view ?? top.view
        pop.sourceRect =
            view?.bounds
            ?? CGRect(x: top.view.bounds.midX, y: top.view.bounds.midY, width: 0, height: 0)
    }
    top.present(vc, animated: true)
}

/// The line under the strip (app.css, .pk-spk-line): the speakers'
/// switch, where the build offers a speaker model, and Find right after it,
/// 12 apart, one group of the transcript's controls: a field that says
/// "Find", 200 wide, 280 with a search in it and 380 with a speaker found,
/// for their name, where that fits beside the switch. Else the field across
/// the line, the switch giving it its place, while there is a search or a
/// speaker found, and the glass, which opens it so, while there is neither,
/// as a phone's find in page does (Android's SpeakersLine). At the line's
/// start, not its far end, where a reader might not see that the transcript
/// can be searched (Peter, 2026-10-31).
struct FindLine: View {
    /// The speakers' switch as the line shows it (Work.switchOf).
    struct Switch {
        let on: Bool
        let busy: Bool
        let hint: String
        let toggle: () -> Void
    }

    let hits: [Hit]
    let seek: ((Double) -> Void)?
    /// the search alone on the line, while the rest makes room for the
    /// keyboard (Work)
    let alone: Bool
    var speakers: Switch?
    /// Only matches, while something is found (Finder.only): after the
    /// field where the line has room, as the desktop has it after the names
    /// and Find (app.js, applyOnly), else on a line of its own 16 under it,
    /// where a phone's field runs across the line. Not while the search is
    /// alone on the line, whose rows it would take.
    var only: Switch?
    var nameOf: (Int) -> String = { _ in "" }
    /// on a phone, with the speakers shown: the switch opens their menu
    /// (SpeakersMenu), and a speaker found sits under the field (WhoBadge)
    var people: People?
    /// the line's width, and the switches'
    @State private var width: CGFloat = 0
    @State private var switchWidth: CGFloat = 0
    @State private var onlyWidth: CGFloat = 0
    /// the field's 200, 280 and 380, scaled as its text is: Android's app at
    /// twice the text size left a search 53 of a fixed 280 (2026-10-31)
    @ScaledMetric(relativeTo: .subheadline) private var narrow: CGFloat = 200
    @ScaledMetric(relativeTo: .subheadline) private var searching: CGFloat = 280
    @ScaledMetric(relativeTo: .subheadline) private var finding: CGFloat = 380

    var body: some View {
        let f = AppState.shared.finder
        let typed = !f.query.trimmingCharacters(in: .whitespaces).isEmpty
        let wide = f.who != 0 ? finding : typed ? searching : narrow
        let before = speakers == nil ? 0 : switchWidth + 12
        let field: CGFloat? = !alone && width > 0 && before + wide <= width ? wide : nil
        let across = alone || (field == nil && (f.open || typed || f.who != 0))
        let o = alone ? nil : only
        let inline = o != nil && field.map { before + $0 + 12 + onlyWidth <= width } == true
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 12) {
                if let s = speakers, !across {
                    if let people, !s.busy {
                        SpeakersMenu(people: people) {
                            SwitchFace(on: s.on, label: L("speakers_switch"), menu: true)
                        }
                        .fixedSize()
                        .accessibilityHint(s.hint)
                    } else {
                        SpeakersSwitch(on: s.on, busy: s.busy, hint: s.hint, action: s.toggle)
                            .fixedSize()
                    }
                }
                // one field, at its width or across the line, so that it keeps
                // the keyboard from one to the other
                if field != nil || across {
                    FindField(
                        finder: f, hits: hits, seek: seek, opened: across, nameOf: nameOf,
                        whoInside: people == nil
                    )
                    .frame(width: across ? nil : field)
                    .frame(maxWidth: across ? .infinity : nil)
                } else {
                    IconButton(icon: .search, label: L("find_label"), kind: .ghost) {
                        f.open = true
                    }
                }
                if let o, inline { onlySwitch(o) }
            }
            if let o, !inline {
                HStack(spacing: 12) {
                    if let people, f.who != 0 { WhoBadge(people: people, finder: f) }
                    onlySwitch(o)
                }
            }
        }
        // the switches' widths, while they are given their places too
        .background(alignment: .leading) {
            ZStack(alignment: .leading) {
                if let s = speakers {
                    SwitchFace(on: s.on, label: L("speakers_switch"), menu: people != nil)
                        .fixedSize()
                        .hidden()
                        .onGeometryChange(for: CGFloat.self) {
                            $0.size.width
                        } action: {
                            switchWidth = $0
                        }
                }
                if let o {
                    onlySwitch(o)
                        .hidden()
                        .onGeometryChange(for: CGFloat.self) {
                            $0.size.width
                        } action: {
                            onlyWidth = $0
                        }
                }
            }
        }
        // to the start by its frame, not a spacer after it: the line's 12
        // between the field across it and a spacer left the field 12 short
        // of the strip's edge above it (Peter, the iPhone 16e, 2026-11-01)
        .frame(maxWidth: .infinity, alignment: .leading)
        .onGeometryChange(for: CGFloat.self) {
            $0.size.width
        } action: {
            width = $0
        }
    }

    private func onlySwitch(_ o: Switch) -> some View {
        SpeakersSwitch(
            on: o.on, busy: o.busy, hint: o.hint, action: o.toggle, label: L("find_only")
        )
        .fixedSize()
    }
}

/// A match in a row: its place among all of them, and where it is.
struct Marked: Equatable {
    let k: Int
    let hit: Hit
}

/// The transcript's rows (app.css, .pk-seg): each sentence's time, then
/// its text, in a column as wide as the file's times, 46 between them, the
/// desktop's and Android's, where the text keeps 496 beside them; else the
/// time above the text. The row playing on the surface and the word playing
/// underlined in it; a row, tapped anywhere, plays from its time. With the
/// turns shown, a turn's name opens its first sentence, Who's speaking from
/// it, and its line runs down its rows, in the middle of the gutter, or 5
/// in where the time is over the text; a sentence held, Change speaker.
///
/// The list follows: the transcript's end as it comes, for a reader who is
/// there; finished, it starts at its top; the row playing is kept in sight
/// for a reader still with it, and each move to a match brings the match's
/// own line into sight, by the least scroll that shows it: a long row's
/// match can be in a line the list under the keyboard doesn't show
/// (Android's app as well, 2026-11-01). A row out of sight comes a third of
/// the way down first (app.js, goHit), which draws its match.
///
/// None of it while a finger or a fling moves the list: the list pulled to
/// the row playing at each new one, under a reader scrolling to a row to
/// tap, lost the playhead after a few such taps (Peter, the iPhone 16e,
/// 2026-11-01). Playback takes along only a reader whose row before is in
/// sight, as the desktop's does (app.js, showTime); a reader who has
/// scrolled away is left there, and the waveform's seeks bring their row.
///
/// With Only matches (Finder.only), the rows found alone (shownRows), each
/// still playing from its time: a run of them stands apart as a turn does,
/// its line ending inside its first and last, and a run from inside a turn
/// has the turn's name (app.js, applyOnly). Turned on, the reader stays on
/// their sentence or is given the match nearest it; turned off, they are
/// back where they were, unless they have moved since (keep).
/// Where a finger scrubbing the waveform has the playhead (Work, Rows).
struct Scrub: Equatable {
    var at: Double = 0
    var n = 0
}

private struct Rows: View {
    let rows: [Segment]
    let review: Bool
    let marks: [Int: [Marked]]
    let hits: [Hit]
    /// with Only matches, the rows shown, in order; nil for all
    let view: [Int]?
    let stats: String?
    /// the screen's edge, 16 or 24 (Work)
    let edge: CGFloat
    /// the waveform's seeks so far, each a jump to bring into sight (Work)
    let jump: Int
    let scrub: Scrub
    let speakers: Voices
    let seek: (Double, Bool) -> Void

    /// The speakers as the rows show them (Work): the turns, a second voice
    /// heard; whether a sentence's speaker can be changed, the names shown
    /// on a finished transcript; who is in it and their names; and the last
    /// move's sentence made one again, lit a moment. The rows keep the
    /// lines' gutter only where there are lines or will be: a transcript
    /// without speakers has its text at the screen's edge, and wraps anew
    /// when they are turned on (Peter, 2026-11-20). While they are found
    /// for a transcript already made, heard: what the diarizer has so far.
    struct Voices {
        let turns: Bool
        let editable: Bool
        let present: [Int]
        let names: [String]
        let joined: Speakers.Joined?
        let gutter: Bool
        let heard: Heard?
    }

    /// The speakers found so far (Speakers.growing), up to a time: a
    /// sentence wholly heard has the line and name of whoever talks most of
    /// it, so the rows on the screen fill in as the find reaches them, as a
    /// transcription with speakers draws them (Peter, 2026-11-20). The
    /// find's own answer replaces them at its end, a sentence two people
    /// share then split between them.
    struct Heard {
        let stretches: [Stretch]
        let upto: Double

        func speaker(_ s: Segment) -> Int? {
            guard s.end <= upto else { return nil }
            var most: [Int: Double] = [:]
            for x in stretches where x.end > s.start && x.start < s.end {
                most[x.speaker, default: 0] += min(x.end, s.end) - max(x.start, s.start)
            }
            // of two who talk as much, the lower number, the same each time
            return most.max { ($0.value, -$0.key) < ($1.value, -$1.key) }?.key
        }
    }
    /// the id of the current match's view in its row (Sentence)
    static let match = "match"
    @State private var seen: [Int] = []
    /// what moves the list: a hand (tracking, interacting, a fling's
    /// decelerating), an animated scroll of ours, or nothing
    @State private var phase: ScrollPhase = .idle
    /// a row a follow scrolled to, until it shows: a far scroll in a lazy
    /// list lands by the heights of the rows not yet drawn, and can stop a
    /// row or more short of it; tried again once the list is still
    @State private var chasing: Int?
    @State private var tries = 0
    /// a move to a match not yet brought into sight
    @State private var revealing = false
    @State private var width: CGFloat = 0
    @Environment(\.dynamicTypeSize) private var type
    /// where each row the list holds was last drawn, in the list's room,
    /// and the room's height, for a turn of Only matches to keep the
    /// reader's sentence where it was (keep); held aside, as nothing drawn
    /// depends on it. What is in sight is reckoned from these, not from
    /// seen, which kept rows Only matches had just taken away (the iPhone 17
    /// Simulator, 2026-11-15). Each with which list drew it, all the rows'
    /// or the matches' alone: the turn makes the list anew, and the new
    /// one's rows are drawn as the old one's go.
    @State private var drawn = Drawn()
    /// where the reader was when Only matches was turned on, until they
    /// move the list, seek or go to a match: where turning it off takes
    /// them back to (keep)
    @State private var back: Back?

    private final class Drawn {
        var frames: [Int: (only: Bool, at: CGRect)] = [:]
        var height: CGFloat = 0
    }

    /// A row and where it was in the list's room (the anchor that puts it
    /// there again), and whether the reader was with the row playing.
    private struct Back {
        let row: Int
        let y: CGFloat
        let following: Bool
    }

    var body: some View {
        let t = Transcriber.shared
        let s = AppState.shared
        let p = Player.shared
        let f = s.finder
        let position = p.position
        let base = s.base.with(duration: t.duration, video: t.fps)
        let current =
            review && (p.playing || position > 0)
            ? rows.lastIndex { $0.start <= position + 1e-3 } ?? -1 : -1
        // the times' column as wide as the file's times: all of a file's as
        // long as its length's, at the reader's text size
        let column = ceil(
            (base.at(base.duration) as NSString).size(withAttributes: [
                .font: PkType.stamp.uiFont(PkType.stamp.scaled(type))
            ]).width)
        let wide = width >= 496 + 46 + column
        let v = speakers
        let shown = view ?? Array(rows.indices)
        // with Only matches, which rows show, for where their runs start and end
        let shows: [Bool]? = view.map { list in
            var b = [Bool](repeating: false, count: rows.count)
            for i in list { b[i] = true }
            return b
        }
        ScrollViewReader { list in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(shown, id: \.self) { i in
                        let now = i == current
                        let mine = marks[i] ?? []
                        let s = rows[i]
                        // a run of the rows Only matches shows, from its first to its last
                        let from = shows.map { i == 0 || !$0[i - 1] } ?? false
                        let to = shows.map { i == rows.count - 1 || !$0[i + 1] } ?? false
                        // with the turns shown, a turn's name opens its first
                        // sentence, and each row has its turn's line, ending
                        // inside its first and last; a run from inside a turn
                        // has the turn's name (a sentence no speaker was heard
                        // in has none)
                        let starts = v.turns && Speakers.turnStarts(rows, i)
                        let lent = v.turns && from && s.speaker != nil && !starts
                        let turn =
                            starts
                            ? Speakers.turnAt(rows, i)
                            : lent ? Speakers.turnAt(rows, turnStart(i)) : nil
                        // while they are found, the line and name of
                        // whoever was heard in it (Heard), no menu on them yet
                        let said = v.heard?.speaker(s)
                        let opens = said != nil && (i == 0 || v.heard?.speaker(rows[i - 1]) != said)
                        let line =
                            v.turns
                            ? s.speaker.map { n in
                                TurnLine(
                                    speaker: n, first: starts || from,
                                    last: i == rows.count - 1 || rows[i + 1].speaker != n || to)
                            }
                            : said.map { n in
                                TurnLine(
                                    speaker: n, first: opens || from,
                                    last: i == rows.count - 1
                                        || v.heard?.speaker(rows[i + 1]) != n || to)
                            }
                        SegRow(
                            s: s, index: i, now: now,
                            word: now
                                ? s.words.lastIndex { $0.start <= position + 1e-3 } ?? -1
                                : -1,
                            marks: mine, at: mine.contains { $0.k == f.at } ? f.at : -1,
                            column: wide ? column : nil, review: review, base: base,
                            name: turn.map { Speakers.nameOf(v.names, $0.speaker) }
                                ?? (opens ? said.map { Speakers.nameOf(v.names, $0) } : nil),
                            turn: turn, line: line, gutter: v.gutter,
                            who: v.editable ? Who(present: v.present, names: v.names) : nil,
                            lit: v.joined?.seg == s ? v.joined : nil, first: i == shown.first,
                            apart: from || opens
                        ) { seek(s.start, true) }
                        .equatable()
                        .onGeometryChange(for: CGRect.self) {
                            $0.frame(in: .scrollView)
                        } action: {
                            drawn.frames[i] = (view != nil, $0)
                        }
                        .onDisappear {
                            if drawn.frames[i]?.only == (view != nil) { drawn.frames[i] = nil }
                        }
                    }
                    // with Only matches and nothing found, where the sentences would be
                    if view?.isEmpty == true {
                        Text(L("find_only_none"))
                            .font(PkType.ui.font(14))
                            .foregroundStyle(Pk.inkMuted)
                            .padding(.horizontal, wide ? 16 : SegRow.inset)
                            .padding(.vertical, 6)
                    }
                    // what the whole transcript cost says nothing of the matches shown
                    if let stats, view == nil {
                        Text(stats)
                            .font(PkType.small.font(12))
                            .monospacedDigit()
                            .foregroundStyle(Pk.inkMuted)
                            .padding(
                                .leading,
                                wide
                                    ? 16 + column + 46
                                    : SegRow.inset + (v.gutter ? SegRow.gutter : 0)
                            )
                            .padding(.top, 16)
                    }
                }
                .scrollTargetLayout()
                // the time over the text: the text at the screen's edge, the
                // row playing's ground its inset wider (SegRow)
                .padding(.horizontal, wide ? edge : edge - SegRow.inset)
                .padding(.bottom, 16)
            }
            // a list anew for each set of rows Only matches shows: the rows
            // shown fewer, from far down, the lazy list drew none at all,
            // blank where they were (the iPhone 17 Simulator, 2026-11-15);
            // a new one lays them out from its top, and keep or the match's
            // reveal takes the reader to their sentence
            .id(view)
            .onScrollTargetVisibilityChange(idType: Int.self) { seen = $0.sorted() }
            .onScrollGeometryChange(for: CGFloat.self) {
                $0.containerSize.height
            } action: { _, h in
                drawn.height = h
            }
            .onScrollPhaseChange { _, now in
                phase = now
                if hands {
                    chasing = nil
                    back = nil
                }
                // a follow that stopped short, once the list is still: twice more at most
                if now == .idle, let r = chasing {
                    if seen.dropLast().contains(r) || tries >= 2 {
                        chasing = nil
                    } else {
                        tries += 1
                        withAnimation { list.scrollTo(r, anchor: .top) }
                    }
                }
            }
            // the row playing kept in sight, for a reader still with it: the
            // row before in sight, or playback just begun or followed to now
            // (with only the matches shown, through those alone)
            .onChange(of: p.playing ? current : -1) { was, now in
                guard now >= 0, !hands, view?.contains(now) ?? true else { return }
                if was < 0 || chasing != nil || seen.contains(was) { bring(now, list) }
            }
            // a finger scrubbing the waveform: the sentence under the playhead,
            // at once, to read while looking for a place (Peter, 2026-11-18)
            .onChange(of: scrub) {
                guard !hands, let r = rows.lastIndex(where: { $0.start <= scrub.at + 1e-3 }),
                    view?.contains(r) ?? true
                else { return }
                list.scrollTo(r, anchor: .top)
            }
            // the keys let go of by a finger on the transcript, or its scroll
            .scrollDismissesKeyboard(.immediately)
            .simultaneousGesture(
                TapGesture().onEnded {
                    back = nil
                    UIApplication.shared.sendAction(
                        #selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                }
            )
            // scrolled away from the row playing while it plays: Now, an arrow
            // the way the row is and the playhead's time, which brings the row
            // to the top and follows it again (Android's NowButton)
            .overlay(alignment: .bottom) {
                let a = AppState.shared
                let lifted = a.undo != nil || a.toast != nil || a.speakersNote != nil
                let away: Int =
                    !p.playing || current < 0 || !(view?.contains(current) ?? true)
                        || seen.isEmpty || seen.contains(current)
                    ? 0 : current < seen.first! ? -1 : 1
                if away != 0 {
                    // whole seconds: the strip's hundredths running here as well, at
                    // the screen's top and its foot, drew the eye from the text
                    // (Peter, 2026-11-20)
                    NowButton(up: away < 0, time: hms(position, base.duration)) {
                        chasing = current
                        tries = 0
                        withAnimation { list.scrollTo(current, anchor: .top) }
                    }
                    // over a toast at the screen's foot (Root: its 24 and its
                    // height, and 12 between), not under it
                    .padding(.bottom, lifted ? 84 : 16)
                }
            }
            // a seek on the waveform: its row, wherever the reader is
            .onChange(of: jump) {
                back = nil
                if current >= 0 && !hands && view?.contains(current) ?? true {
                    bring(current, list)
                }
            }
            .onChange(of: f.only) { keep(list, playing: current) }
            // the transcript's end followed as it comes, for a reader who is there
            .onChange(of: t.segments.count) { _, n in
                if t.running && n > 0 && (seen.last ?? 0) >= n - 4 && !hands {
                    withAnimation { list.scrollTo(n - 1, anchor: .bottom) }
                }
            }
            // finished, it starts at its top
            .onChange(of: review ? t.id : "") { _, id in
                if !id.isEmpty { list.scrollTo(0, anchor: .top) }
            }
            .onChange(of: f.moved) {
                back = nil
                revealing = true
                reveal(list)
            }
            // the match drawn where it is
            .onChange(of: f.spot) { if revealing { reveal(list) } }
            // the match playback passes, the reader with it (Finder.follow)
            .onChange(of: position) { f.follow(hits, position: position, jump: false) }
        }
        // the first row's text 16 under the line above, past the row's own 6
        // (app.css, .pk-transcript; the spacing audit, 2026-11-15: it was 24)
        .padding(.top, 10)
        .onGeometryChange(for: CGFloat.self) {
            $0.size.width
        } action: {
            width = $0
        }
    }

    /// A finger on the list, or its fling: the reader moving it.
    private var hands: Bool {
        phase == .tracking || phase == .interacting || phase == .decelerating
    }

    /// The first row of row i's turn.
    private func turnStart(_ i: Int) -> Int {
        var a = i
        while a > 0 && rows[a - 1].speaker == rows[i].speaker { a -= 1 }
        return a
    }

    /// Only matches turned (Finder.only), the same on the desktop (app.js,
    /// the switch's click) and Android (Follow.keep). The reader's row is
    /// the match they are on or the row playing, where one is in sight,
    /// else the first row wholly in sight.
    ///
    /// Turned on: their row stays where it was on the screen if it is a
    /// match, else the first match in sight does; with no match in sight,
    /// the next match after their row (the last, past them all) comes to
    /// where their row was. Never the match they are on from out of sight:
    /// that took a reader 27 minutes from the playhead (Peter, the iPhone
    /// 16e, 2026-11-20).
    ///
    /// Turned off: back where they were when it was turned on (back), with
    /// the row playing there instead for a reader who was with it; once
    /// they have moved the list, tapped a row, sought or gone to a match,
    /// their row now stays where it is.
    ///
    /// Called as the turn is drawn, from the places the list before the
    /// turn drew its rows at; a row at fraction y of the list's room, less
    /// its own height, is put there again by the anchor y.
    private func keep(_ list: ScrollViewProxy, playing: Int) {
        let f = AppState.shared.finder
        let on = view != nil
        let h = drawn.height
        let frames = drawn.frames.filter { $0.value.only != on }.mapValues { $0.at }
        drawn.frames = drawn.frames.filter { $0.value.only == on }
        let inSight = { (i: Int) in frames[i].map { $0.maxY > 0 && $0.minY < h } ?? false }
        let whole = { (i: Int) in frames[i].map { $0.minY >= 0 && $0.minY < h } ?? false }
        let y = { (i: Int) -> CGFloat in
            guard let r = frames[i], h > r.height else { return 0 }
            return min(max(r.minY / (h - r.height), 0), 1)
        }
        let hit = f.at >= 0 && f.at < hits.count ? hits[f.at].row : -1
        let mine =
            [hit, playing].first { $0 >= 0 && inSight($0) }
            ?? frames.keys.filter(whole).min() ?? frames.keys.filter(inSight).min()
        var to: (Int, CGFloat)?
        if let shown = view {
            back = mine.map {
                Back(
                    row: $0, y: y($0),
                    following: Player.shared.playing && playing >= 0 && inSight(playing))
            }
            if let mine, shown.contains(mine) {
                to = (mine, y(mine))
            } else if let first = shown.first(where: whole) {
                to = (first, y(first))
            } else if let mine, let near = shown.first(where: { $0 > mine }) ?? shown.last {
                to = (near, y(mine))
            } else {
                to = shown.first.map { ($0, 0) }
            }
        } else if let b = back {
            back = nil
            to = (b.following && playing >= 0 ? playing : b.row, b.y)
        } else {
            to = mine.map { ($0, y($0)) }
        }
        guard let (row, at) = to else { return }
        let anchor = UnitPoint(x: 0, y: at)
        // now, which the list made anew (id) applies as it first lays out its
        // rows, by their estimated heights; then once they are laid out, to
        // their measured places (the iPhone 17 Simulator, 2026-11-15: the
        // row first 13 points short of where it was, then on it)
        list.scrollTo(row, anchor: anchor)
        Task { @MainActor in list.scrollTo(row, anchor: anchor) }
    }

    /// Row at the list's top, where it isn't in sight above the list's last
    /// row (which shows only in part), and followed until it shows.
    private func bring(_ row: Int, _ list: ScrollViewProxy) {
        // in sight: nothing to chase, not even a row asked for before
        guard !seen.dropLast().contains(row) else {
            chasing = nil
            return
        }
        chasing = row
        tries = 0
        withAnimation { list.scrollTo(row, anchor: .top) }
    }

    /// The current match into sight: once its row has drawn it, its line,
    /// by the least scroll (none where it shows); until then, its row out
    /// of sight a third of the way down.
    private func reveal(_ list: ScrollViewProxy) {
        let f = AppState.shared.finder
        guard f.at >= 0 && f.at < hits.count else {
            revealing = false
            return
        }
        if f.spot?.k == f.at {
            revealing = false
            withAnimation { list.scrollTo(Rows.match) }
        } else if !seen.contains(hits[f.at].row) {
            withAnimation { list.scrollTo(hits[f.at].row, anchor: UnitPoint(x: 0, y: 1.0 / 3)) }
        }
    }
}

/// Who a sentence or a turn can be given to, where its speaker can be
/// changed: the speakers in the transcript, and their names.
private struct Who: Equatable {
    let present: [Int]
    let names: [String]
}

/// Who's speaking for a turn, or for one sentence (WhoItems): the turn,
/// the speakers and their names, and its times.
private struct WhoMenu: Equatable {
    let turn: Speakers.Turn
    let who: Who
    let times: String
}

/// Change speaker, a held row's menu, where its speaker can be changed: no
/// menu at all elsewhere, so that a row held without one doesn't lift.
private struct ChangeSpeaker: ViewModifier {
    let menu: WhoMenu?

    func body(content: Content) -> some View {
        if let menu {
            content.contextMenu {
                WhoItems(
                    turn: menu.turn, present: menu.who.present, names: menu.who.names,
                    times: menu.times, alone: true)
            }
        } else {
            content
        }
    }
}

/// A row: its time and its text.
private struct SegRow: View, Equatable {
    let s: Segment
    let index: Int
    let now: Bool
    let word: Int
    let marks: [Marked]
    let at: Int
    /// the times' column's width, where the time stands beside the text;
    /// nil where it stands over it
    let column: CGFloat?
    let review: Bool
    let base: Timebase
    /// the turn this row starts, if any, and its speaker's name, which opens
    /// the text (Sentence)
    let name: String?
    let turn: Speakers.Turn?
    /// its turn's line, with the turns shown
    let line: TurnLine?
    /// with the time over the text, room for the lines before it (Voices)
    let gutter: Bool
    /// with the time over the text: the text this far inside the row's
    /// ground, at the screen's edge with the header's and the strip's; and
    /// the lines' room before it, where there are any, a line 6.5 into it
    /// as the desktop's narrow window has it
    static let inset: CGFloat = 8
    static let gutter: CGFloat = 16
    /// who it can be given to, where its speaker can be changed
    let who: Who?
    /// the sentence a move made one again, lit a moment
    let lit: Speakers.Joined?
    let first: Bool
    /// with Only matches, the first of a run of the rows shown, which stands
    /// apart as a turn does
    let apart: Bool
    let onTime: () -> Void

    // the action aside, which is a new closure each frame and does the same
    static func == (a: SegRow, b: SegRow) -> Bool {
        a.s == b.s && a.index == b.index && a.now == b.now && a.word == b.word
            && a.marks == b.marks && a.at == b.at && a.column == b.column
            && a.review == b.review && a.base == b.base && a.name == b.name && a.turn == b.turn
            && a.line == b.line && a.who == b.who && a.lit == b.lit && a.first == b.first
            && a.apart == b.apart && a.gutter == b.gutter
    }

    var body: some View {
        let menu = turn.flatMap { t in who.map { WhoMenu(turn: t, who: $0, times: times(t)) } }
        // Change speaker, for this sentence alone: iOS's menu of a held row
        let alone = who.flatMap { w in
            s.speaker.map { n in
                let t = Speakers.Turn(
                    from: index, to: index, speaker: n, start: s.start, end: s.end)
                return WhoMenu(turn: t, who: w, times: times(t))
            }
        }
        let sentence = Sentence(
            s: s, word: word, marks: marks, at: at, name: name, menu: menu, lit: lit)
        Group {
            if let column {
                HStack(alignment: .firstTextBaseline, spacing: 46) {
                    time.frame(width: column, alignment: .leading)
                    sentence.frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                VStack(alignment: .leading, spacing: 2) {
                    time
                    sentence.frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        // sentences 12 apart (app.css, .pk-seg; Peter, 2026-11-15: at 16, a
        // touch loose)
        .padding(.leading, column != nil ? 16 : Self.inset + (gutter ? Self.gutter : 0))
        .padding(.trailing, column != nil ? 16 : Self.inset)
        .padding(.vertical, 6)
        .frame(maxWidth: 716, alignment: .leading)
        // its turn's line in the middle of the gutter, past the row's 16 and
        // the times; with the time over the text, in the room before it
        .background(alignment: .leading) {
            if let line {
                TurnLineShape(
                    line: line, mid: column.map { 16 + $0 + 23 } ?? Self.inset + 6.5
                )
                .fill(Pk.speaker(line.speaker))
                .flipsForRightToLeftLayoutDirection(true)
            }
        }
        .background(
            now ? Pk.surface : Color.clear, in: RoundedRectangle(cornerRadius: Radius.radiusUi)
        )
        .contentShape(Rectangle())
        // the whole row plays from its time, its words as well as the time,
        // which a thumb is too broad to find (Peter, the S25, 2026-10-26)
        .onTapGesture { if review { onTime() } }
        .modifier(ChangeSpeaker(menu: alone))
        .accessibilityElement(children: .combine)
        .accessibilityAction(named: L("play_from", base.at(s.start))) { if review { onTime() } }
        // a turn 16 under the one before (app.css, .pk-seg.pk-turn): with the
        // rows' 6, sentences 12 apart and turns 28; with Only matches, a run
        // of the rows shown the same
        .padding(.top, (turn != nil || apart) && !first ? 16 : 0)
    }

    private func times(_ t: Speakers.Turn) -> String { "\(base.at(t.start))–\(base.at(t.end))" }

    /// Its time, its line the stamp's 1.5 as the desktop's and Android's
    /// are: in the font's own, 11 over the text where they have 14 (the
    /// spacing audit, 2026-11-15). The row playing is its ground alone: the
    /// beak's square after its time went, as on the desktop and Android,
    /// where beside the speakers' marks it read as one of them (Peter,
    /// 2026-11-14).
    private var time: some View {
        Text(verbatim: base.at(s.start))
            .pk(PkType.stamp)
            .monospacedDigit()
            .foregroundStyle(Pk.inkMuted)
    }
}

/// What a stretch of a row's text is: in the k-th match (-1: none), that
/// the reader is on, in the word playing, and in the words a move joined to
/// it (Speakers.rejoin).
private struct Mark: TextAttribute {
    var k = -1
    var current = false
    var word = false
    var lit = false
}

/// A row's text in the transcript's type, the word playing underlined in
/// the beak's colour, 2 thick, 4 below the line; and a search's matches
/// marked (app.css, .pk-hit): each on plume mixed 45% into the ground, as
/// tall as its letters and 2 round them, its corners 3; the one the reader
/// is on in plume, its letters on-plume, an ink bar 2 under it, for a state
/// shown by more than its colour (the tokens' contrast, 2026-10-22).
///
/// A turn's name opens its first sentence's text, bold with a colon, as the
/// saved files have it ("Speaker 1: ..."), Peter's pick of three
/// (2026-11-14), iOS's menu of Who's speaking over it; marked as a match is
/// where a speaker found has the turn for one (turnsOf). A sentence a move
/// made one again has the words joined to it lit in their speaker's colour,
/// 30% of it fading out over 1.6 s (app.css, .pk-joined), from where it has
/// got to if the row comes back into sight meanwhile; not at all with Reduce
/// Motion.
private struct Sentence: View {
    let s: Segment
    let word: Int
    let marks: [Marked]
    let at: Int
    var name: String? = nil
    var menu: WhoMenu? = nil
    var lit: Speakers.Joined? = nil
    @State private var glow = 0.0
    @Environment(\.dynamicTypeSize) private var type
    @Environment(\.accessibilityReduceMotion) private var still

    private static let bold = Font.custom(
        "ArchivoRoman-Bold", size: PkType.transcript.size, relativeTo: .body)

    var body: some View {
        marked
            // the current match where its renderer drew it, a view of its
            // own for the rows to scroll to (Rows.reveal)
            .overlay(alignment: .topLeading) {
                if at >= 0, let spot = AppState.shared.finder.spot, spot.k == at {
                    Color.clear
                        .frame(width: max(spot.rect.width, 1), height: spot.rect.height)
                        .id(Rows.match)
                        .padding(.leading, max(spot.rect.minX, 0))
                        .padding(.top, max(spot.rect.minY, 0))
                        .accessibilityHidden(true)
                }
            }
            .pk(PkType.transcript)
            .foregroundStyle(Pk.ink)
            .textRenderer(
                Marks(
                    em: PkType.transcript.scaled(type),
                    spotted: at < 0
                        ? nil
                        : { k, r in
                            Task { @MainActor in AppState.shared.finder.spotted(k, r) }
                        },
                    glow: glow, glowColor: lit.map { Pk.speaker($0.speaker) } ?? .clear)
            )
            .fixedSize(horizontal: false, vertical: true)
            .overlay(alignment: .topLeading) {
                if let menu, let name { who(menu, name) }
            }
            .onChange(of: lit?.at, initial: true) { _, at in light(at) }
    }

    /// Who's speaking from the turn's name: iOS's menu, from the name's own
    /// place on the text's first line.
    private func who(_ m: WhoMenu, _ name: String) -> some View {
        let size = PkType.transcript.scaled(type)
        let font = UIFont(name: "ArchivoRoman-Bold", size: size) ?? .boldSystemFont(ofSize: size)
        let w = ceil(((name + ":") as NSString).size(withAttributes: [.font: font]).width)
        return Menu {
            WhoItems(turn: m.turn, present: m.who.present, names: m.who.names, times: m.times)
        } label: {
            Color.clear
                .frame(width: w + 4, height: PkType.transcript.lineHeight * size)
                .contentShape(Rectangle())
        }
        .padding(.leading, -2)
        .accessibilityLabel(L("speakers_name_label", name))
    }

    /// The words a move joined lit from where their 1.6 s has got to,
    /// fading out.
    private func light(_ at: Date?) {
        let left = at.map { 1 - Date().timeIntervalSince($0) / 1.6 } ?? 0
        guard left > 0, !still else {
            glow = 0
            return
        }
        var now = Transaction()
        now.disablesAnimations = true
        withTransaction(now) { glow = left }
        Task { @MainActor in withAnimation(.easeOut(duration: 1.6 * left)) { glow = 0 } }
    }

    /// The text cut where a match, the word playing or the words a move
    /// joined begin or end, each stretch with what it is, after the turn's
    /// name, where one opens it.
    private var marked: Text {
        let chars = Array(s.text)
        let span = word >= 0 ? wordSpan(s, word) : nil
        let ms = marks.filter {
            $0.hit.start >= 0 && $0.hit.end <= chars.count && $0.hit.start < $0.hit.end
        }
        let from = lit.flatMap { wordSpan(s, $0.words)?.0 }
        if span == nil && ms.isEmpty && from == nil { return lead + Text(verbatim: s.text) }
        var cuts: Set<Int> = [0, chars.count]
        if let span { cuts.formUnion([span.0, span.1]) }
        if let from { cuts.insert(from) }
        for m in ms { cuts.formUnion([m.hit.start, m.hit.end]) }
        let at = cuts.sorted()
        var out = lead
        for (a, b) in zip(at, at.dropFirst()) where b > a {
            let m = ms.first { $0.hit.start <= a && b <= $0.hit.end }
            let w = span.map { $0.0 <= a && b <= $0.1 } ?? false
            let l = from.map { $0 <= a } ?? false
            var piece = Text(verbatim: String(chars[a..<b]))
            if m != nil || w || l {
                let current = m?.k == self.at && m != nil
                piece = piece.customAttribute(
                    Mark(k: m?.k ?? -1, current: current, word: w, lit: l))
                if current { piece = piece.foregroundStyle(Pk.onPlume) }
            }
            out = out + piece
        }
        return out
    }

    /// The turn's name and its colon, then a space; nothing where the row
    /// starts no turn.
    private var lead: Text {
        guard let name else { return Text(verbatim: "") }
        var piece = Text(verbatim: name + ":").font(Sentence.bold)
        if let m = marks.first(where: { $0.hit.start < 0 }) {
            let current = m.k == at
            piece = piece.customAttribute(Mark(k: m.k, current: current))
            if current { piece = piece.foregroundStyle(Pk.onPlume) }
        }
        return piece + Text(verbatim: " ")
    }
}

/// Where the word-th word of s is in its text, in characters: the words,
/// in order, each found after the last.
private func wordSpan(_ s: Segment, _ word: Int) -> (Int, Int)? {
    let text = Array(s.text)
    var at = 0
    for (i, w) in s.words.enumerated() {
        let t = Array(w.text.trimmingCharacters(in: .whitespaces))
        guard !t.isEmpty, let k = firstIndex(of: t, in: text, from: at) else { return nil }
        if i == word { return (k, k + t.count) }
        at = k + t.count
    }
    return nil
}

private func firstIndex(of q: [Character], in s: [Character], from: Int) -> Int? {
    var i = from
    while i + q.count <= s.count {
        if s[i] == q[0] && Array(s[i..<(i + q.count)]) == q { return i }
        i += 1
    }
    return nil
}

/// The marks behind a row's letters, then the letters, then the word
/// playing's line: a match's mark is its letters' height, from Archivo's
/// ascent to its descent (0.878 and 0.21 em, its hhea), and 2 round them,
/// as an inline mark's box is drawn on the desktop; a match's stretches on
/// a line drawn as one. The row with the current match is told where it
/// drew it, both lines' where it breaks across two.
private struct Marks: TextRenderer {
    let em: CGFloat
    var spotted: ((Int, CGRect) -> Void)? = nil
    /// the words a move joined, lit: 1 is 30% of their speaker's colour
    var glow: Double = 0
    var glowColor: Color = .clear

    var animatableData: Double {
        get { glow }
        set { glow = newValue }
    }

    func draw(layout: Text.Layout, in ctx: inout GraphicsContext) {
        let hl = Pk.matchFill
        var spot: (k: Int, rect: CGRect)?
        defer { if let spot, let spotted { spotted(spot.k, spot.rect) } }
        if glow > 0 {
            for line in layout {
                var x0: CGFloat?
                var x1: CGFloat = 0
                var y: CGFloat = 0
                for run in line {
                    guard let m = run[Mark.self], m.lit else { continue }
                    let b = run.typographicBounds
                    if x0 == nil {
                        x0 = b.origin.x
                        y = b.origin.y
                    }
                    x1 = b.origin.x + b.width
                }
                guard let x0 else { continue }
                let top = y - 0.878 * em - 2
                let bottom = y + 0.21 * em + 2
                ctx.fill(
                    Path(
                        roundedRect: CGRect(
                            x: x0 - 2, y: top, width: x1 - x0 + 4, height: bottom - top),
                        cornerRadius: 3),
                    with: .color(glowColor.opacity(0.3 * glow)))
            }
        }
        for line in layout {
            var spans: [(k: Int, current: Bool, x0: CGFloat, x1: CGFloat, y: CGFloat)] = []
            for run in line {
                guard let m = run[Mark.self], m.k >= 0 else { continue }
                let b = run.typographicBounds
                if let last = spans.last, last.k == m.k {
                    spans[spans.count - 1].x1 = b.origin.x + b.width
                } else {
                    spans.append((m.k, m.current, b.origin.x, b.origin.x + b.width, b.origin.y))
                }
            }
            for sp in spans {
                let top = sp.y - 0.878 * em - 2
                let bottom = sp.y + 0.21 * em + 2
                let r = CGRect(x: sp.x0 - 2, y: top, width: sp.x1 - sp.x0 + 4, height: bottom - top)
                ctx.fill(
                    Path(roundedRect: r, cornerRadius: 3), with: .color(sp.current ? Pk.plume : hl))
                if sp.current {
                    ctx.fill(
                        Path(CGRect(x: sp.x0, y: bottom, width: sp.x1 - sp.x0, height: 2)),
                        with: .color(Pk.ink))
                    // with its underline, and 4 clear of it either side
                    let seen = CGRect(
                        x: r.minX, y: r.minY - 4, width: r.width, height: r.height + 10)
                    spot = (sp.k, spot.map { $0.rect.union(seen) } ?? seen)
                }
            }
        }
        for line in layout {
            for run in line { ctx.draw(run) }
        }
        for line in layout {
            for run in line {
                guard let m = run[Mark.self], m.word else { continue }
                let b = run.typographicBounds
                ctx.fill(
                    Path(CGRect(x: b.origin.x, y: b.origin.y + 3, width: b.width, height: 2)),
                    with: .color(Pk.beak))
            }
        }
    }
}

extension Pk {
    /// a match's mark: plume mixed 45% into the ground (app.css, .pk-hit)
    static let matchFill = Color(
        uiColor: UIColor { t in
            func rgb(_ c: Color) -> (CGFloat, CGFloat, CGFloat) {
                var r: CGFloat = 0
                var g: CGFloat = 0
                var b: CGFloat = 0
                var a: CGFloat = 0
                UIColor(c).resolvedColor(with: t).getRed(&r, green: &g, blue: &b, alpha: &a)
                return (r, g, b)
            }
            let (g, p) = (rgb(Pk.ground), rgb(Pk.plume))
            return UIColor(
                red: g.0 * 0.55 + p.0 * 0.45, green: g.1 * 0.55 + p.1 * 0.45,
                blue: g.2 * 0.55 + p.2 * 0.45, alpha: 1)
        })
}

/// Back to the row playing, for a reader who has scrolled away from it while
/// it plays (Peter, 2026-11-18: "I find myself getting lost"): over the foot
/// of the transcript, an arrow the way the row is and the playhead's time,
/// in the toast's floating box. The time says what the arrow points at,
/// where no icon says "now".
private struct NowButton: View {
    let up: Bool
    let time: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                (up ? PkIcon.arrowUp : PkIcon.arrowDown).frame(width: 16, height: 16)
                Text(time).font(PkType.stamp.font(13)).monospacedDigit().lineLimit(1)
            }
            .foregroundStyle(Pk.ink)
            .padding(.leading, 12)
            .padding(.trailing, 14)
            .frame(minHeight: 40)
            .background(Pk.ground, in: Capsule())
            .overlay { Capsule().strokeBorder(Pk.menuEdge, lineWidth: 1) }
            .shadow(color: .black.opacity(0.25), radius: 12, y: 4)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L("follow_back", time))
    }
}
