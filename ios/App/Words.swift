import Foundation
import UIKit

/// A line on the console in a debug build, its time first: what a device
/// run is followed by (devicectl --console).
func trace(_ s: @autoclosure () -> String) {
    #if DEBUG
    print(String(format: "%.3f ", Date().timeIntervalSince1970.truncatingRemainder(dividingBy: 1000)) + s())
    fflush(stdout)
    #endif
}

/// Megabytes as the desktop counts them, a million bytes, whole (app.js, mb).
func mb(_ bytes: Int64) -> String { String(Int64((Double(bytes) / 1e6).rounded())) }

/// The fraction done, as a whole percent, down (app.js, percent).
func percent(_ x: Double) -> Int { Int(min(max(x, 0), 1) * 100) }

/// About how long is left, to 5 s under a minute, else to the minute
/// (app.js, timeLeft).
func timeLeft(_ s: Double) -> String {
    guard s >= 0, s.isFinite else { return "" }
    if s < 50 {
        return L("time_left_seconds", max(5, Int((s / 5).rounded() * 5)))
    }
    return L("time_left_minutes", Int((s / 60).rounded()))
}

/// "iPhone" or "iPad", for the words that name this device's kind.
var deviceKind: String { UIDevice.current.model }

/// A problem's words, the desktop's for its key (strings.rs, Problem; ui/
/// locales/en.json; Android's Models.kt, words): a download's, a model
/// file's, and anything else as something gone wrong. host is the server's
/// name, for the words that say it.
func words(_ key: String?, host: String = "") -> String {
    switch key {
    case "download.damaged": return L("download_damaged")
    case "download.not_answering": return L("download_not_answering", host)
    case "download.not_found": return L("download_not_found")
    case "download.cant_reach": return L("download_cant_reach", host)
    case "download.connection_lost": return L("download_connection_lost")
    case "download.too_much": return L("download_too_much", host)
    case "download.cant_check": return L("download_cant_check")
    case "download.disk_full": return L("download_disk_full")
    case "download.cant_save": return L("download_cant_save")
    case "model_file.newer": return L("model_file_newer")
    case "model_file.not_model": return L("model_file_not_model")
    case "model_file.cant_open": return L("model_file_cant_open")
    case "model_file.damaged": return L("model_file_damaged")
    case "problem.memory": return L("problem_memory")
    default: return L("problem_unexpected")
    }
}

/// A catalog model's words, as the desktop's window has them (catalog.KEY).
func label(_ key: String) -> String {
    switch key {
    case "english": return L("catalog_english")
    case "multilingual": return L("catalog_multilingual")
    default: return key
    }
}

func about(_ key: String) -> String {
    switch key {
    case "english": return L("catalog_english_about")
    case "multilingual": return L("catalog_multilingual_about")
    default: return ""
    }
}
