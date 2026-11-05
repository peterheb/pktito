import AppleArchive
import Foundation
import System

/// A model's Core ML encoder out of its Apple Archive (the .aar
/// models.pktito.net serves, `aa archive` of the compiled model), into
/// Store.encoder: unpacked beside it first and renamed into place whole, so
/// that an encoder in place is a whole one, and the archive deleted after,
/// 547 MB for v2's. Only an archive whose SHA-256 checked comes here
/// (Downloader), so its contents are the ones published.
enum Unpack {
    static func encoder(_ e: CatalogEntry, from archive: URL) throws {
        let dst = Store.encoder(e)
        let work = dst.deletingLastPathComponent().appendingPathComponent(
            dst.lastPathComponent + ".unpacking")
        let fm = FileManager.default
        try? fm.removeItem(at: work)
        try fm.createDirectory(at: work, withIntermediateDirectories: true)
        do {
            try unpack(archive, into: work)
            guard fm.fileExists(atPath: work.appendingPathComponent("model.mil").path) else {
                throw CoreError("\(archive.lastPathComponent) holds no Core ML model")
            }
            try? fm.removeItem(at: dst)
            try fm.moveItem(at: work, to: dst)
        } catch {
            try? fm.removeItem(at: work)
            throw error
        }
        try? fm.removeItem(at: archive)
    }

    private static func unpack(_ archive: URL, into dir: URL) throws {
        let failed = CoreError("\(archive.lastPathComponent) couldn't be unpacked")
        guard
            let file = ArchiveByteStream.fileStream(
                path: FilePath(archive.path), mode: .readOnly,
                options: [], permissions: FilePermissions(rawValue: 0o644)),
            let inflate = ArchiveByteStream.decompressionStream(readingFrom: file),
            let decode = ArchiveStream.decodeStream(readingFrom: inflate),
            let extract = ArchiveStream.extractStream(
                extractingTo: FilePath(dir.path),
                flags: [.ignoreOperationNotPermitted])
        else { throw failed }
        defer {
            try? extract.close()
            try? decode.close()
            try? inflate.close()
            try? file.close()
        }
        _ = try ArchiveStream.process(readingFrom: decode, writingTo: extract)
    }
}
