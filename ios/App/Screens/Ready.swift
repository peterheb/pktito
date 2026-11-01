import SwiftUI

/// The start screen's top row: the wordmark, which opens About, and the
/// model in use, its menu.
private struct TopRow: View {
    var body: some View {
        // the model menu under the wordmark, at the end, where the two
        // don't fit on a line
        ViewThatFits(in: .horizontal) {
            HStack {
                WordmarkButton()
                Spacer()
                ModelMenu()
            }
            VStack(alignment: .trailing, spacing: 8) {
                WordmarkButton().frame(maxWidth: .infinity, alignment: .leading)
                ModelMenu()
            }
        }
    }
}

/// The drop zone (app.css, .pk-ready-drop): the prompt and Choose in a
/// dashed zone. A phone has nothing to drop, so the zone says what it is for.
struct Drop: View {
    var body: some View {
        VStack(spacing: 16) {
            Text(L("ready_lead"))
                .font(PkType.lead.font(22))
                .foregroundStyle(Pk.ink)
                .multilineTextAlignment(.center)
                // all its lines, the list under it giving way
                .fixedSize(horizontal: false, vertical: true)
            PkButton(text: L("choose"), kind: .primary, large: true) { AppState.shared.pick = .media }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 36)
        .frame(maxWidth: .infinity)
        .background(Pk.surface, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        .overlay {
            RoundedRectangle(cornerRadius: Radius.radiusUi)
                .strokeBorder(Pk.inkMuted, style: StrokeStyle(lineWidth: 2, dash: [6, 4]))
        }
    }
}

/// The start screen (ui/index.html, #ready; Android's Ready): the wordmark
/// and the model in use on the column's top edge, the zone, and with
/// transcripts on the recent list the list under it; without, the column
/// in the middle.
struct Ready: View {
    /// the top row and the zone, as tall as their text size and language make them
    @State private var zone: CGFloat = 330

    var body: some View {
        let listed = !Recents.shared.items.isEmpty
        GeometryReader { g in
            // where the window, less the top row and the zone, leaves the list
            // less than three rows and its heading, the column scrolls whole,
            // the list in it, rather than being squeezed into what is left
            // (Android's Ready: the zone's 330 and 150 under it)
            let short = g.size.height < zone + 40 + 150
            let column = VStack(spacing: 22) {
                // its whole height always, so that it is measured true and the
                // list is what gives way
                VStack(spacing: 22) {
                    TopRow()
                    Drop()
                }
                .fixedSize(horizontal: false, vertical: true)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { zone = $0 }
                if listed { RecentList(scrolls: !short) }
            }
            .frame(maxWidth: 640)
            .padding(20)
            if short {
                ScrollView { column.frame(maxWidth: .infinity) }
            } else if listed {
                column.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            } else {
                column.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }
}

/// A window with room for the work beside it keeps the start screen's top
/// row and recent list in a column at its left, the side, as the desktop's
/// does: beside the zone, or beside the transcript open, whose row it marks
/// (Android's Side). While a transcription runs, the rows wait for it, dimmed.
struct Side: View {
    var body: some View {
        let t = Transcriber.shared
        let open = t.step != .none
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 14) {
                TopRow()
                if open {
                    PkButton(text: L("choose"), stretch: true) { AppState.shared.pick = .media }
                        .disabled(t.running)
                }
                if !Recents.shared.items.isEmpty || (open && !Recents.shared.items.contains { $0.id == t.id }) {
                    RecentList(scrolls: true, open: open ? t.id : nil, side: true)
                }
                Spacer(minLength: 0)
            }
            .padding(.leading, 20)
            .padding(.trailing, 16)
            .padding(.top, 20)
            .padding(.bottom, 16)
            .frame(width: 300)
            .overlay(alignment: .trailing) { Rectangle().fill(Pk.sunk).frame(width: 1) }
            ZStack {
                if open {
                    Work()
                } else {
                    Drop().frame(maxWidth: 560).padding(.horizontal, 40)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

/// The recent list (app.css, .pk-recent): its heading and Clear, which asks
/// first, then each transcript, newest first, opened by a tap; taken off
/// the list by a swipe or a long press's menu (app.js, renderReady). In the
/// side, the transcript open is marked, or heads the list where the list
/// doesn't have it.
private struct RecentList: View {
    let scrolls: Bool
    var open: String? = nil
    var side = false
    @State private var asking = false

    var body: some View {
        let items = Recents.shared.items
        let t = Transcriber.shared
        let live = side && open != nil && !items.contains { $0.id == open }
        VStack(spacing: 4) {
            HStack {
                Text(L("recent_title")).pk(PkType.ui).foregroundStyle(Pk.ink)
                Spacer()
                if !items.isEmpty {
                    Button(L("recent_clear")) { asking = true }
                        .font(PkType.ui.font(13))
                        .foregroundStyle(Pk.inkMuted)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 11)
                        .padding(.horizontal, -8)
                        .padding(.vertical, -11)
                        .accessibilityLabel(L("recent_clear_label"))
                        .disabled(t.running)
                }
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 4)
            let rows = VStack(spacing: 2) {
                if live { LiveRow() }
                ForEach(items) { r in
                    RecentRow(r: r, current: r.id == open)
                        .disabled(t.running)
                        .opacity(t.running ? 0.45 : 1)
                }
            }
            if scrolls {
                ScrollView { rows }
            } else {
                rows
            }
        }
        .padding(.top, side ? 4 : 8)
        .alert(L("ask_clear_title"), isPresented: $asking) {
            Button(L("ask_clear"), role: .destructive) { Recents.shared.clear() }
            Button(L("ask_cancel"), role: .cancel) {}
        } message: {
            Text(L("ask_clear_text"))
        }
    }
}

/// When a transcript finished, as the desktop's list says it: Today,
/// Yesterday, else the date, its year if not this one's (app.js, when).
private func whenDone(_ secs: UInt64) -> String {
    let day = Date(timeIntervalSince1970: TimeInterval(secs))
    let cal = Calendar.current
    if cal.isDateInToday(day) { return L("date_today") }
    if cal.isDateInYesterday(day) { return L("date_yesterday") }
    let f = DateFormatter()
    f.setLocalizedDateFormatFromTemplate(cal.isDate(day, equalTo: Date(), toGranularity: .year) ? "MMMd" : "yMMMd")
    return f.string(from: day)
}

/// A transcript of the list: its icon, its name, its length and when it
/// finished, on a line where the list is wide, else its length and date
/// under its name. The one open beside the side on the surface.
private struct RecentRow: View {
    let r: RecentItem
    let current: Bool

    var body: some View {
        let length = hms(r.duration, r.duration)
        let date = whenDone(r.finished)
        Button {
            if !current { AppState.shared.openRecent(r.id) }
        } label: {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    PkIcon.file.frame(width: 16, height: 16)
                    Text(r.name).font(PkType.ui.font(14)).lineLimit(1).truncationMode(.middle)
                        .frame(minWidth: 180, maxWidth: .infinity, alignment: .leading)
                    Text(length).font(PkType.stamp.font(13)).monospacedDigit().foregroundStyle(Pk.inkMuted)
                    Text(date).pk(PkType.small).foregroundStyle(Pk.inkMuted).frame(minWidth: 72, alignment: .trailing)
                }
                HStack(spacing: 10) {
                    PkIcon.file.frame(width: 16, height: 16)
                    VStack(alignment: .leading, spacing: 0) {
                        Text(r.name).font(PkType.ui.font(14)).lineLimit(1).truncationMode(.middle)
                        HStack(spacing: 0) {
                            Text(length).font(PkType.stamp.font(13)).monospacedDigit()
                            Text(" · " + date).pk(PkType.small)
                        }
                        .foregroundStyle(Pk.inkMuted)
                    }
                    Spacer(minLength: 0)
                }
            }
            .foregroundStyle(Pk.ink)
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .frame(minHeight: 48)
            .background(current ? Pk.surface : Color.clear, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(current ? .isSelected : [])
        .contextMenu {
            Button(role: .destructive) { forget() } label: {
                Text(L("recent_remove_item"))
                Text(L("recent_remove_note"))
            }
        }
        .accessibilityAction(named: L("recent_remove", r.name)) { forget() }
    }

    private func forget() {
        if current { AppState.shared.leave() }
        Recents.shared.forget(r.id)
    }
}

/// The transcript open beside the side that the list doesn't have (app.js,
/// liveHtml): under way, stopped or failed; its name, and how far it has got.
private struct LiveRow: View {
    var body: some View {
        let t = Transcriber.shared
        let pct = percent(t.done)
        let (status, ink): (String, Color) = {
            switch t.step {
            case .loading: return (L("status_loading"), Pk.inkMuted)
            case .reading: return (L("status_reading"), Pk.inkMuted)
            case .transcribing: return (L("status_transcribing", pct), Pk.inkMuted)
            case .stopped: return (L("status_stopped_at", pct), Pk.inkMuted)
            case .done: return (L("status_done"), Pk.ok)
            default: return (L("status_failed"), Pk.err)
            }
        }()
        HStack(spacing: 10) {
            PkIcon.file.frame(width: 16, height: 16).foregroundStyle(Pk.ink)
            VStack(alignment: .leading, spacing: 0) {
                Text(t.name).font(PkType.ui.font(14)).foregroundStyle(Pk.ink).lineLimit(1).truncationMode(.middle)
                HStack(spacing: 4) {
                    if t.step == .done { PkIcon.check.frame(width: 14, height: 14) }
                    Text(status).pk(PkType.small).lineLimit(1)
                }
                .foregroundStyle(ink)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(minHeight: 48)
        .background(Pk.surface, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isSelected)
    }
}
