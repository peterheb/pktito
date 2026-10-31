import Foundation
import PktitoCore

/// A transcript on the recent list as the core keeps it (recent.rs, Item).
struct RecentItem: Codable, Identifiable, Hashable {
    let id: String
    /// the file transcribed, as it was found then
    let path: String
    let name: String
    let duration: Double
    /// when it finished, in seconds since 1970
    let finished: UInt64
    /// seconds it took, reading the audio included
    let took: Double
    var transcribed: Double = 0
    let model: String
    var fps: Double = 0
    /// the file as it was transcribed: its size and last change, 0 where not known
    var size: UInt64 = 0
    var modified: UInt64 = 0
}

/// A transcript of the list opened again: its item, its loudness and its
/// segments (recent.rs, Transcript).
struct RecentTranscript: Decodable {
    let item: RecentItem
    let peaks: [UInt8]
    let segments: [Segment]

    enum Keys: String, CodingKey { case peaks, segments }

    init(from d: Decoder) throws {
        item = try RecentItem(from: d)
        let c = try d.container(keyedBy: Keys.self)
        peaks = try c.decode([UInt8].self, forKey: .peaks)
        segments = try c.decode([Segment].self, forKey: .segments)
    }
}

private struct Listed: Decodable { let items: [RecentItem] }

/// The recent list, the desktop's (recent.rs) through the core (ios.rs):
/// the 50 latest finished transcripts, newest first, kept in Application
/// Support, out of backups, so that the start screen opens each again and
/// plays it from its recording. A file picked in Files is found again by a
/// bookmark of it, kept while its transcript is on the list; one shared to
/// pktito is copied in as it arrives, as Android keeps one (Recents.kt).
@MainActor @Observable
final class Recents {
    static let shared = Recents()

    private(set) var items: [RecentItem] = []

    nonisolated static let data: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let dir = base.appendingPathComponent("recent-data", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        var v = URLResourceValues()
        v.isExcludedFromBackup = true
        var d = dir
        try? d.setResourceValues(v)
        return dir
    }()

    nonisolated static var bookmarks: URL { data.appendingPathComponent("bookmarks", isDirectory: true) }
    nonisolated static var copies: URL { data.appendingPathComponent("copies", isDirectory: true) }

    private init() { refresh() }

    func refresh() {
        items = Recents.list(Recents.call { pkapp_recent($0) })
    }

    nonisolated private static func call(_ f: (UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?) -> Data {
        let s = data.path.withCString { f($0) }
        guard let s else { return Data() }
        defer { pkapp_free_string(s) }
        return Data(bytes: s, count: strlen(s))
    }

    nonisolated private static func list(_ d: Data) -> [RecentItem] {
        (try? JSONDecoder().decode(Listed.self, from: d))?.items ?? []
    }

    /// A finished transcript at the top of the list, in place of an earlier
    /// one of the same file; its file's bookmark kept with it. Off the main
    /// thread, where the transcription ends.
    nonisolated static func keep(_ item: RecentItem, transcript: Transcript, peaks: [UInt8], bookmark: Data?) {
        guard let itemJSON = try? JSONEncoder().encode(item), let tJSON = try? JSONEncoder().encode(transcript) else { return }
        if let bookmark {
            try? FileManager.default.createDirectory(at: bookmarks, withIntermediateDirectories: true)
            try? bookmark.write(to: bookmarks.appendingPathComponent(item.id))
        }
        let out = data.path.withCString { d in
            String(decoding: itemJSON, as: UTF8.self).withCString { i in
                String(decoding: tJSON, as: UTF8.self).withCString { t in
                    peaks.withUnsafeBufferPointer { p in pkapp_keep(d, i, t, p.baseAddress, p.count) }
                }
            }
        }
        if let out { pkapp_free_string(out) }
        Task { @MainActor in
            Recents.shared.refresh()
            Recents.shared.prune()
        }
    }

    /// A transcript of the list, as recent.rs keeps it.
    func open(_ id: String) -> RecentTranscript? {
        let d = Recents.data.path.withCString { data in id.withCString { pkapp_open_recent(data, $0) } }
        guard let d else { return nil }
        defer { pkapp_free_string(d) }
        return try? JSONDecoder().decode(RecentTranscript.self, from: Data(bytes: d, count: strlen(d)))
    }

    /// One transcript off the list; the file it was made from stays.
    func forget(_ id: String) {
        let d = Recents.data.path.withCString { data in id.withCString { pkapp_forget(data, $0) } }
        if let d { pkapp_free_string(d) }
        try? FileManager.default.removeItem(at: Recents.bookmarks.appendingPathComponent(id))
        try? FileManager.default.removeItem(at: Recents.copies.appendingPathComponent(id))
        refresh()
    }

    /// The list emptied, once the person says so.
    func clear() {
        Recents.data.path.withCString { pkapp_clear_recent($0) }
        try? FileManager.default.removeItem(at: Recents.bookmarks)
        try? FileManager.default.removeItem(at: Recents.copies)
        refresh()
    }

    /// The bookmarks and copies no transcript on the list has, nor the one
    /// on the screen.
    func prune() {
        let keep = Set(items.map(\.id) + [Transcriber.shared.id])
        for dir in [Recents.bookmarks, Recents.copies] {
            for f in (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
                where !keep.contains(f.lastPathComponent) {
                try? FileManager.default.removeItem(at: f)
            }
        }
    }

    /// A recording to play, or why there is none, in the screen's words, and
    /// whether Locate… can find it (Android's Recents.Source).
    struct Source {
        var url: URL?
        var why: String?
        var locate = false
    }

    /// Transcript item's recording, to play: its copy, if it was shared to
    /// pktito, or the file its bookmark finds, if it is the one transcribed
    /// (recent.rs, Item.changed: by its size, then its last change, where
    /// both sides know them).
    func source(_ item: RecentItem) -> Source {
        guard let url = recording(item) else { return Source(why: L("play_moved"), locate: true) }
        if url.path.hasPrefix(Recents.copies.path) { return Source(url: url) }
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let (size, modified) = Recents.stamp(url) else { return Source(why: L("play_moved"), locate: true) }
        let differ = { (a: UInt64, b: UInt64) in a != 0 && b != 0 && a != b }
        if differ(item.size, size) || differ(item.modified, modified) {
            return Source(why: L("play_changed"), locate: true)
        }
        return Source(url: url)
    }

    /// A file's size and last change, as the list keeps them; nil where it
    /// can't be read.
    nonisolated static func stamp(_ url: URL) -> (UInt64, UInt64)? {
        guard let v = try? url.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey]),
              FileManager.default.isReadableFile(atPath: url.path) else { return nil }
        return (UInt64(v.fileSize ?? 0), UInt64(v.contentModificationDate?.timeIntervalSince1970 ?? 0))
    }

    /// Transcript id's recording found again at url, picked by the person
    /// (Locate…): its bookmark kept, and its place on the list, if it is
    /// the one transcribed, by its size where both know it: true. False,
    /// and nothing kept, where it is another.
    func relink(_ id: String, _ url: URL) -> Bool {
        guard let item = items.first(where: { $0.id == id }) else { return false }
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let (size, modified) = Recents.stamp(url) ?? (0, 0)
        if item.size != 0 && size != 0 && item.size != size { return false }
        guard let b = try? url.bookmarkData() else { return false }
        try? FileManager.default.createDirectory(at: Recents.bookmarks, withIntermediateDirectories: true)
        try? b.write(to: Recents.bookmarks.appendingPathComponent(id))
        let out = Recents.data.path.withCString { d in
            id.withCString { i in url.path.withCString { p in pkapp_relink(d, i, p, size, modified) } }
        }
        if let out { pkapp_free_string(out) }
        refresh()
        return true
    }

    /// Where a transcript's recording is now: its copy, if it was shared to
    /// pktito, else its bookmark resolved; nil where it is gone.
    func recording(_ item: RecentItem) -> URL? {
        let copy = Recents.copies.appendingPathComponent(item.id)
        if FileManager.default.fileExists(atPath: copy.path) { return copy }
        guard let b = try? Data(contentsOf: Recents.bookmarks.appendingPathComponent(item.id)) else { return nil }
        var stale = false
        return try? URL(resolvingBookmarkData: b, bookmarkDataIsStale: &stale)
    }
}
