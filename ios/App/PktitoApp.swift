import SwiftUI

@main
struct PktitoApp: App {
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
