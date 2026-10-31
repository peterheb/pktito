import SwiftUI
import UIKit

@main
struct PktitoApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        WindowGroup {
            #if DEBUG
            if CommandLine.arguments.contains("-bench") {
                Bench()
            } else {
                Root()
            }
            #else
            Root()
            #endif
        }
    }
}

/// What iOS tells the app outside its window: a model's transfer ended
/// while the app was away (Downloader's background session), for which iOS
/// woke it, or launched it again.
final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, handleEventsForBackgroundURLSession identifier: String,
                     completionHandler: @escaping () -> Void) {
        guard identifier == Downloader.sessionID else { return completionHandler() }
        Downloader.shared.wakeDone = completionHandler
        // the session made again, to hear what happened
        _ = Downloader.shared.session
    }
}
