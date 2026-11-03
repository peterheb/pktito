import SwiftUI
import UIKit

@main
struct PktitoApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    /// The largest text size the app follows: the second of the
    /// accessibility sizes, body text 33 against the default's 17, about
    /// Android's largest (200%), so that both apps go as far and no further
    /// (Peter, 2026-10-31); a larger one in Settings shows as this.
    static let largest = DynamicTypeSize.accessibility2

    var body: some Scene {
        WindowGroup {
            #if DEBUG
                if CommandLine.arguments.contains("-bench") {
                    Bench()
                } else {
                    Root().dynamicTypeSize(...PktitoApp.largest)
                }
            #else
                Root().dynamicTypeSize(...PktitoApp.largest)
            #endif
        }
    }
}

/// What iOS tells the app outside its window: a model's transfer ended
/// while the app was away (Downloader's background session), for which iOS
/// woke it, or launched it again.
final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication, handleEventsForBackgroundURLSession identifier: String,
        completionHandler: @escaping () -> Void
    ) {
        guard identifier == Downloader.sessionID else { return completionHandler() }
        Downloader.shared.wakeDone = completionHandler
        // the session made again, to hear what happened
        _ = Downloader.shared.session
    }
}
