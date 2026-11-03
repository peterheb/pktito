import SwiftUI

/// One thing the app keeps: what it is (its id: its kind and its name,
/// "have:", "part:" or "own:"), its words, what it takes, its button's, and
/// the question before the button does it (none for Stop).
private struct Kept: Identifiable {
    let id: String
    let title: String
    let note: String
    var inUse = false
    let bytes: Int64
    let act: String
    var enabled = true
    let ask: (title: String, text: String)?
    let onAct: () -> Void
}

/// Manage models, from the model menu, in the desktop's words (manage.*;
/// Android's Manage): what the app keeps on the device, which iOS shows
/// only as one number in Settings, and which only deleting the app would
/// free, the recent list with it. In the catalog's order, each model
/// downloaded, its name and size, the one in use said so in green; what a
/// download that stopped kept, which it would go on from; a download under
/// way, with Stop; and the copy of a model file of the person's own; each
/// deleted after a question that says what follows. Under them what they
/// take in all, and the room left. A sheet from the screen's foot on a
/// phone, in its middle on an iPad. While a transcription runs, the model
/// in use and the one it loaded, if another was chosen since, keep their
/// Delete dimmed: the second is mapped under the transcription
/// (Models.delete), the first will be the next one's. The toast is shown
/// here while this is up, over the screen's.
struct Manage: View {
    /// the row whose question is up, by its id
    @State private var asking: String?
    /// what the rows take, for the sheet to be as tall as they are
    @State private var tall: CGFloat = 320
    @Environment(\.horizontalSizeClass) private var size
    /// what a row's words keep beside its button before it goes under them
    /// (the header's floor): 160 at the default text size, about twenty
    /// characters, and as many at any other
    @ScaledMetric(relativeTo: .subheadline) private var floor: CGFloat = 160

    var body: some View {
        let s = AppState.shared
        let rows = kept()
        let total = rows.reduce(0) { $0 + $1.bytes }
        // a row gone while its question was up (a download that ended) takes
        // it with it, and so does one whose Delete a transcription begun
        // since has dimmed: that model is in the transcription's hands
        let k = rows.first { $0.id == asking && $0.enabled && $0.ask != nil }
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(L("manage_title"))
                    .pk(PkType.ui)
                    .foregroundStyle(Pk.ink)
                    .accessibilityAddTraits(.isHeader)
                VStack(spacing: 0) {
                    if rows.isEmpty {
                        Text(L("manage_none"))
                            .pk(PkType.small)
                            .foregroundStyle(Pk.inkMuted)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 12)
                    }
                    ForEach(rows) { r in
                        Line()
                        fitting {
                            VStack(alignment: .leading, spacing: 0) {
                                Text(r.title).pk(PkType.ui).foregroundStyle(Pk.ink).lineLimit(1).truncationMode(.middle)
                                note(r).pk(PkType.small).foregroundStyle(Pk.inkMuted)
                            }
                        } button: {
                            PkButton(text: r.act, kind: r.ask != nil ? .delete : .plain, minWidth: 80) {
                                if r.ask != nil { asking = r.id } else { r.onAct() }
                            }
                            .disabled(!r.enabled)
                        }
                        .padding(.vertical, 12)
                    }
                    Line()
                }
                .padding(.top, 12)
                fitting {
                    Text(used(total, Models.shared.free()))
                        .pk(PkType.small)
                        .foregroundStyle(Pk.inkMuted)
                } button: {
                    PkButton(text: L("sheet_close"), minWidth: 80) { s.managing = false }
                }
                .padding(.top, 16)
            }
            .padding(.horizontal, sheetEdge(size))
            .padding(.top, 24)
            .padding(.bottom, 20)
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { tall = $0 }
        }
        .scrollBounceBehavior(.basedOnSize)
        .presentationDetents([.height(tall)])
        .background(Pk.ground)
        .overlay(alignment: .bottom) {
            if let toast = s.toast { Toast(text: toast, err: s.toastErr).padding(.bottom, 24) }
        }
        .alert(k?.ask?.title ?? "", isPresented: Binding(get: { k != nil }, set: { if !$0 { asking = nil } }),
               presenting: k) { k in
            Button(L("manage_delete"), role: .destructive) {
                asking = nil
                k.onAct()
            }
            Button(L("ask_cancel"), role: .cancel) { asking = nil }
        } message: { k in
            Text(k.ask?.text ?? "")
        }
    }

    /// A row's words and its button side by side, the words wrapping beside
    /// it, or the button under them, at the end, where the words would keep
    /// less than their floor beside it.
    private func fitting<Words: View, Button: View>(@ViewBuilder _ words: () -> Words,
                                                     @ViewBuilder button: () -> Button) -> some View {
        let w = words(), b = button()
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 16) {
                w.frame(minWidth: floor, idealWidth: floor, maxWidth: .infinity, alignment: .leading)
                b.fixedSize()
            }
            VStack(alignment: .trailing, spacing: 8) {
                w.frame(maxWidth: .infinity, alignment: .leading)
                b
            }
        }
    }

    /// A row's second line, and In use after it in green where it is the
    /// one in use, its two words on one line.
    private func note(_ r: Kept) -> Text {
        guard r.inUse else { return Text(r.note) }
        return Text(r.note + dot) + Text(L("manage_in_use").replacingOccurrences(of: " ", with: "\u{00A0}")).foregroundStyle(Pk.ok)
    }

    private func kept() -> [Kept] {
        let s = AppState.shared
        let m = Models.shared
        let t = Transcriber.shared
        let d = Downloads.shared.state
        var out: [Kept] = []
        for o in m.offers {
            let name = label(o.key)
            let dl = d?.name == o.name ? d : nil
            if o.have {
                let inUse = m.own == nil && o.name == m.model
                let bytes = Store.bytes(o.entry)
                let text: String
                if !inUse {
                    text = L("manage_ask_text", mb(bytes))
                } else if let then = m.next(model: m.model, deleting: o.name) {
                    text = L("manage_ask_text_switch", mb(bytes), label(then.key))
                } else {
                    text = L("manage_ask_text_last", mb(bytes))
                }
                out.append(Kept(id: "have:" + o.name, title: name, note: o.name + dot + L("manage_size", mb(bytes)),
                                inUse: inUse, bytes: bytes, act: L("manage_delete"),
                                enabled: !(t.running && (inUse || t.loadedName == o.name)),
                                ask: (L("manage_ask_title", name), text)) { s.deleted(m.delete(o)) })
            } else if let dl, dl.going {
                out.append(Kept(id: "part:" + o.name, title: name, note: L("manage_downloading", mb(dl.got), mb(dl.total)),
                                bytes: dl.got, act: L("stop"), ask: nil) {
                    Downloads.shared.cancel()
                    s.deleted(nil)
                })
            } else if dl != nil || o.partial > 0 {
                let got = dl?.got ?? o.partial
                out.append(Kept(id: "part:" + o.name, title: name, note: L("manage_paused", mb(got), mb(o.bytes)),
                                bytes: got, act: L("manage_delete"),
                                ask: (L("manage_ask_part_title"), L("manage_ask_part_text", mb(got), name))) {
                    if dl != nil {
                        Downloads.shared.cancel()
                    } else {
                        Downloads.shared.forget(o.name)
                        m.refresh()
                    }
                    s.deleted(nil)
                })
            }
        }
        // the own file is the one in use whenever it is here (Models.use deletes it)
        if let own = m.own {
            let bytes = Int64((try? own.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0)
            let text = m.next(model: m.model, deleting: nil).map { L("manage_ask_own_switch", mb(bytes), label($0.key)) }
                ?? L("manage_ask_own_last", mb(bytes))
            out.append(Kept(id: "own:" + own.lastPathComponent, title: own.lastPathComponent,
                            note: L("manage_size", mb(bytes)), inUse: true, bytes: bytes, act: L("manage_delete"),
                            enabled: !t.running, ask: (L("manage_ask_own_title", own.lastPathComponent), text)) {
                s.deleted(m.deleteOwn())
            })
        }
        return out
    }
}

/// What the models take, in megabytes under a gigabyte and in gigabytes to
/// a hundredth from one (manage.used_mb, used_gb), and the room left.
private func used(_ total: Int64, _ free: Int64) -> String {
    let used = total < 1_000_000_000 ? L("manage_used_mb", mb(total))
        : L("manage_used_gb", String(format: "%.2f", locale: .current, Double(total) / 1e9))
    let left = free < 1_000_000_000 ? L("manage_free_mb", mb(free))
        : L("manage_free_gb", String(format: "%.1f", locale: .current, Double(free) / 1e9))
    return used + dot + left
}

/// Between the rows, the line a menu draws between its groups, across.
private struct Line: View {
    var body: some View {
        Rectangle().fill(Pk.sunk).frame(height: 1)
    }
}
