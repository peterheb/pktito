import BackgroundTasks
import Foundation

/// A transcription that goes on when the person leaves the app: iOS 26's
/// continued processing, which keeps the app running while it lasts and
/// shows it in the system's UI, the file's name over its step, as
/// Android's notification shows them (TranscribeService), where the person
/// can stop it too. iOS ends one whose progress stalls, or when it needs
/// the time, as Stop ends it. An earlier system pauses an app it can't
/// see, and the transcription goes on when the person comes back.
@MainActor
enum Continued {
    /// what its requests' identifiers begin with, each the transcription's
    /// id after it (Info.plist, BGTaskSchedulerPermittedIdentifiers, which
    /// has the pattern: iOS takes no pattern to register)
    private static let prefix = "net.pktito.app.transcribe."
    /// the transcription's task, once iOS runs it (a BGContinuedProcessingTask)
    private static var task: BGTask?

    /// Asked for as transcription id begins, which the person began: its
    /// handler registered first, under its own identifier, once, as iOS
    /// ends an app that registers an identifier again.
    static func begin(id: String, name: String) {
        guard #available(iOS 26, *),
              BGTaskScheduler.shared.register(forTaskWithIdentifier: prefix + id, using: .main, launchHandler: { t in
                  MainActor.assumeIsolated { started(t) }
              }) else { return }
        let r = BGContinuedProcessingTaskRequest(identifier: prefix + id, title: name, subtitle: L("status_reading"))
        // now or not at all: the transcription runs anyway while the app is on the screen
        r.strategy = .fail
        // the Simulator has no scheduler, and says so here
        try? BGTaskScheduler.shared.submit(r)
    }

    @available(iOS 26, *)
    private static func started(_ t: BGTask) {
        guard let t = t as? BGContinuedProcessingTask, Transcriber.shared.running else {
            // the transcription ended before iOS ran its task
            return t.setTaskCompleted(success: true)
        }
        t.progress.totalUnitCount = 1000
        // the person stopped it in the system's UI, or iOS needs its time
        t.expirationHandler = { Engine.stop() }
        task = t
        update()
    }

    /// The transcription's step and its progress, told as they change.
    static func update() {
        guard #available(iOS 26, *), let t = task as? BGContinuedProcessingTask else { return }
        let tr = Transcriber.shared
        t.progress.completedUnitCount = Int64(min(max(tr.done, 0), 1) * 1000)
        let step: String
        switch tr.step {
        case .loading: step = L("status_loading")
        case .reading: step = L("status_reading")
        default: step = L("status_transcribing", percent(tr.done))
        }
        t.updateTitle(tr.name, subtitle: step)
    }

    /// The transcription ended: done, or stopped or failed.
    static func end(done: Bool) {
        guard let t = task else { return }
        task = nil
        if #available(iOS 26, *), done, let c = t as? BGContinuedProcessingTask {
            c.progress.completedUnitCount = c.progress.totalUnitCount
        }
        t.setTaskCompleted(success: done)
    }
}
