import SwiftUI

/// A match: its row, where it is in the row's text (in characters), and the
/// time of the word it starts in.
struct Hit: Equatable {
    let row: Int
    let start: Int
    let end: Int
    let time: Double
}

/// Speaker n's turns in rows, as the desktop finds a speaker (app.js,
/// runFind with who): a match a turn, at its first row's start, its name
/// marked (start -1: Sentence marks the name, not words).
func turnsOf(_ rows: [Segment], _ n: Int) -> [Hit] {
    rows.indices
        .filter { i in rows[i].speaker == n && (i == 0 || rows[i - 1].speaker != n) }
        .map { Hit(row: $0, start: -1, end: -1, time: rows[$0].start) }
}

/// Each match of query in rows, the sentences, as the desktop finds them
/// (app.js, runFind; Android's Find.kt): its words trimmed, without regard
/// to case, each match at a word's start (startsWord) and after the one
/// before it.
func findIn(_ rows: [Segment], _ query: String) -> [Hit] {
    let sought = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !sought.isEmpty else { return [] }
    var hits: [Hit] = []
    for (i, s) in rows.enumerated() {
        let text = Array(s.text)
        var low = Array(s.text.lowercased())
        var q = Array(sought.lowercased())
        // a lowercase that changes the length would put the marks in the wrong places
        if low.count != text.count {
            low = text
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
        switch $0.properties.generalCategory {
        case .uppercaseLetter, .lowercaseLetter, .titlecaseLetter, .modifierLetter, .otherLetter,
            .decimalNumber, .letterNumber, .otherNumber,
            .nonspacingMark, .spacingMark, .enclosingMark:
            return true
        default:
            return false
        }
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
    /// the speaker found (findWho), whose turns are the matches (turnsOf);
    /// 0 for words, as a search finds them
    var who = 0
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

    var searching: Bool { open || !query.isEmpty || who != 0 }

    /// Speaker n found, as a search finds a word (app.js, findWho; Peter,
    /// 2026-11-15, naming his speakers: "who is this Speaker 4", needing "to
    /// hear their words and read the context"): their turns the matches, the
    /// field open with their name in it, without the keyboard, which would
    /// cover the turns; the arrows step from turn to turn with the playhead.
    /// Found again, let go.
    func findWho(_ n: Int) {
        if who == n { return clear() }
        query = ""
        who = n
        open = true
    }

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
        who = 0
        open = false
        at = -1
        time = -1
        passed = -1
        spot = nil
    }
}

/// The search's field on the line under the strip (app.css, .pk-find), sunk, 40 tall:
/// the glass, what is sought, and while there is something its count and
/// the previous and next match, dimmed with fewer than two, then the way to
/// close it. Return is the next match. A speaker found shows as their mark
/// and name where words would be (app.css, .pk-find-who); words typed
/// search for words instead, and so does a hardware keyboard's delete in the
/// empty field.
struct FindField: View {
    @Bindable var finder: Finder
    let hits: [Hit]
    let seek: ((Double) -> Void)?
    /// opened across the line from the glass, which its × closes again; a
    /// field that stays at its width shows its × only with a search in
    /// it, to let the search go, as the desktop's field does
    var opened = false
    /// a speaker's name, for the one found (Finder.who)
    var nameOf: (Int) -> String = { _ in "" }
    @FocusState private var typing: Bool

    var body: some View {
        let has = !finder.query.trimmingCharacters(in: .whitespaces).isEmpty || finder.who != 0
        HStack(spacing: 8) {
            PkIcon.search.frame(width: 16, height: 16).foregroundStyle(Pk.inkMuted)
            if finder.who != 0 {
                HStack(spacing: 6) {
                    SpeakerMark(n: finder.who)
                    Text(nameOf(finder.who))
                        .font(PkType.ui.font(13))
                        .foregroundStyle(Pk.ink)
                        .lineLimit(1)
                }
                .padding(.horizontal, 8)
                .frame(maxWidth: 180, minHeight: 28)
                .background(Pk.ground, in: RoundedRectangle(cornerRadius: Radius.radiusSm))
                .fixedSize()
            }
            TextField(finder.who != 0 ? "" : L("find_placeholder"), text: $finder.query)
                .font(PkType.ui.font(14))
                .foregroundStyle(Pk.ink)
                .focused($typing)
                .submitLabel(.search)
                .onSubmit { finder.step(hits, 1, seek: seek) }
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                // words typed search for words, the speaker let go
                .onChange(of: finder.query) { _, q in if !q.isEmpty { finder.who = 0 } }
                .onKeyPress(.delete) {
                    guard finder.query.isEmpty && finder.who != 0 else { return .ignored }
                    finder.clear()
                    return .handled
                }
            if has {
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
            if opened || !finder.query.isEmpty || finder.who != 0 {
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
        // the keyboard for a field the glass opened, not for a speaker found,
        // whose turns it would cover
        .onAppear { if finder.open && finder.query.isEmpty && finder.who == 0 { typing = true } }
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
