import SwiftUI

/// A match: its row, where it is in the row's text (in characters), and the
/// time of the word it starts in.
struct Hit: Equatable {
    let row: Int
    let start: Int
    let end: Int
    let time: Double
}

/// Each match of query in rows, sentences or cues, as the desktop finds them
/// (app.js, runFind; Android's Find.kt): its words trimmed, without regard
/// to case, a cue's line break read as the space it stands for, each match
/// at a word's start (startsWord) and after the one before it.
func findIn(_ rows: [Segment], _ query: String) -> [Hit] {
    let sought = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !sought.isEmpty else { return [] }
    var hits: [Hit] = []
    for (i, s) in rows.enumerated() {
        let flat = Array(s.text.replacingOccurrences(of: "\n", with: " "))
        var low = Array(String(flat).lowercased())
        var q = Array(sought.lowercased())
        // a lowercase that changes the length would put the marks in the wrong places
        if low.count != flat.count {
            low = flat
            q = Array(sought)
        }
        var from = 0
        while from + q.count <= low.count {
            guard let j = index(of: q, in: low, from: from) else { break }
            if startsWord(low, j, q) {
                hits.append(Hit(row: i, start: j, end: j + q.count, time: timeAt(s, j)))
                from = j + q.count
            } else {
                from = j + 1
            }
        }
    }
    return hits
}

/// Whether a match of q at j in text starts a word, as every match must:
/// "cat" finds "Cat" and "cats", not "concatenate" or "scat" (Peter,
/// 2026-11-04; app.js, startsWord). At the text's start or after a character
/// that is no letter, digit or mark, an apostrophe too ("neill" finds
/// "O'Neill", "homme" "l'homme"); a search that itself begins with no letter
/// or digit ("'s", "-19") matches anywhere.
func startsWord(_ text: [Character], _ j: Int, _ q: [Character]) -> Bool {
    j == 0 || !wordChar(q[0]) || !wordChar(text[j - 1])
}

private func wordChar(_ c: Character) -> Bool {
    c.unicodeScalars.contains {
        $0.properties.isAlphabetic || $0.properties.numericType != nil
            || $0.properties.generalCategory == .nonspacingMark
    }
}

private func index(of q: [Character], in s: [Character], from: Int) -> Int? {
    guard !q.isEmpty, s.count >= q.count else { return nil }
    var i = from
    while i + q.count <= s.count {
        if s[i] == q[0] && Array(s[i..<(i + q.count)]) == q { return i }
        i += 1
    }
    return nil
}

/// The start of the word of s at offset in its text, its words found in it
/// in order; s's own before its first (app.js, wordAt).
private func timeAt(_ s: Segment, _ offset: Int) -> Double {
    let text = Array(s.text)
    var at = 0
    var time = s.start
    for w in s.words {
        let t = Array(w.text.trimmingCharacters(in: .whitespaces))
        guard let k = index(of: t, in: text, from: at) else { continue }
        if k > offset { break }
        time = w.start
        at = k + t.count
    }
    return time
}

/// The search of the transcript on the screen (Android's Finder): what is
/// sought, and the match the reader is on, by its place among the matches
/// and by its time, which keeps it where the matches change under it.
/// moved counts the reader's moves, for the screen to bring each into sight.
@MainActor @Observable
final class Finder {
    var query = ""
    var open = false
    private(set) var at = -1
    private(set) var moved = 0
    private var time = -1.0
    private var passed = -1
    /// where the current match is drawn in its row, which its renderer
    /// tells as it draws it (Marks), for the rows to bring that line into
    /// sight: a long row's match can be in a line a short list doesn't show
    private(set) var spot: Spot?

    struct Spot: Equatable {
        let k: Int
        let rect: CGRect
    }

    var searching: Bool { open || !query.isEmpty }

    func spotted(_ k: Int, _ rect: CGRect) {
        let s = Spot(k: k, rect: rect)
        if spot != s { spot = s }
    }

    /// Once the matches have changed: on the same match, where it still is
    /// one; else on the one the playhead last passed (app.js, runFind); the
    /// playhead taken to it first where a search was typed at rest (seek),
    /// before go, since the seek's follow would take the reader to the last
    /// of the matches in one word.
    func settle(_ hits: [Hit], position: Double, seek: ((Double) -> Void)? = nil) {
        passed = passedAt(hits, position)
        if hits.isEmpty {
            at = -1
            return
        }
        let same = time >= 0 ? hits.firstIndex { abs($0.time - time) < 1e-9 } : nil
        let k = same ?? max(0, passed)
        seek?(hits[k].time)
        go(hits, k)
    }

    private func passedAt(_ hits: [Hit], _ position: Double) -> Int {
        hits.lastIndex { $0.time <= position + 1e-3 } ?? -1
    }

    /// The reader on the match the playhead last passed, as playback passes
    /// a match, and wherever a seek lands (jump).
    func follow(_ hits: [Hit], position: Double, jump: Bool) {
        let k = passedAt(hits, position)
        if hits.isEmpty || (!jump && k == passed) { return }
        passed = k
        at = max(0, k)
        time = hits[at].time
    }

    func go(_ hits: [Hit], _ k: Int) {
        at = k
        time = hits[k].time
        moved += 1
    }

    /// The next match, or the one before, round the end (app.js, stepHit),
    /// the playhead taken to it (seek), playing or at rest.
    func step(_ hits: [Hit], _ d: Int, seek: ((Double) -> Void)? = nil) {
        guard !hits.isEmpty else { return }
        let k = (at + d + hits.count) % hits.count
        seek?(hits[k].time)
        go(hits, k)
    }

    func clear() {
        query = ""
        open = false
        at = -1
        time = -1
        passed = -1
        spot = nil
    }
}

/// The search's field on the views line (app.css, .pk-find), sunk, 40 tall:
/// the glass, what is sought, and while there is something its count and
/// the previous and next match, dimmed with fewer than two, then the way to
/// close it. Return is the next match.
struct FindField: View {
    @Bindable var finder: Finder
    let hits: [Hit]
    let seek: ((Double) -> Void)?
    /// opened across the line from the glass, which its × closes again; a
    /// field that stays beside the views shows its × only with a search in
    /// it, to let the search go, as the desktop's field does
    var opened = false
    @FocusState private var typing: Bool

    var body: some View {
        HStack(spacing: 8) {
            PkIcon.search.frame(width: 16, height: 16).foregroundStyle(Pk.inkMuted)
            TextField(L("find_placeholder"), text: $finder.query)
                .font(PkType.ui.font(14))
                .foregroundStyle(Pk.ink)
                .focused($typing)
                .submitLabel(.search)
                .onSubmit { finder.step(hits, 1, seek: seek) }
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
            if !finder.query.trimmingCharacters(in: .whitespaces).isEmpty {
                Text(hits.isEmpty ? "0/0" : "\(finder.at + 1)/\(hits.count)")
                    .font(PkType.stamp.font(12))
                    .monospacedDigit()
                    .foregroundStyle(Pk.inkMuted)
                Group {
                    step(PkIcon.up, L("find_previous"), -1)
                    step(PkIcon.down, L("find_next"), 1)
                }
                .disabled(hits.count < 2)
            }
            if opened || !finder.query.isEmpty {
                Button {
                    finder.clear()
                    typing = false
                } label: {
                    PkIcon.x.frame(width: 16, height: 16).foregroundStyle(Pk.inkMuted).frame(
                        width: 32, height: 32
                    )
                    // 44 to a finger, past the field's edges (HIG's least)
                    .contentShape(Rectangle().inset(by: -6))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L("find_close"))
            }
        }
        .padding(.leading, 14)
        .padding(.trailing, 4)
        .frame(minHeight: 40)
        .background(Pk.sunk, in: RoundedRectangle(cornerRadius: Radius.radiusUi))
        .overlay {
            if typing {
                RoundedRectangle(cornerRadius: Radius.radiusUi).stroke(Pk.focus, lineWidth: 2)
            }
        }
        .onAppear { if finder.open && finder.query.isEmpty { typing = true } }
    }

    private func step(_ icon: PkIcon, _ label: String, _ d: Int) -> some View {
        Button {
            finder.step(hits, d, seek: seek)
        } label: {
            icon.frame(width: 16, height: 16).foregroundStyle(Pk.ink).frame(width: 28, height: 32)
                // 40 by 44 to a finger, its drawing as narrow as the field
                // needs it (the layout audit: 28 by 32 was all there was)
                .contentShape(Rectangle().inset(by: -6))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }
}
