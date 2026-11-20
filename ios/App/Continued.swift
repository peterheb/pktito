import BackgroundTasks
import Foundation
import UIKit

/// A transcription, or the finding of a transcript's speakers, that goes on
/// when the person leaves the app: iOS 26's continued processing, which
/// keeps the app running while it lasts and shows it in the system's UI,
/// the file's name over its step, as Android's notification shows them
/// (TranscribeService), where the person can stop it too. iOS ends one
/// whose progress stalls, or when it needs the time, as Stop ends it. An
/// earlier system pauses an app it can't see, and the work goes on when
/// the person comes back.
///
/// It is asked for as the app leaves the front, not as the work begins:
/// iOS shows its banner over the app too, on the header, the strip's own
/// progress said twice, and leaves it there some ten seconds after the end
/// (an iPhone 16e, iOS 26: "Transcribing 100%" over the speakers being
/// found, 2026-11-20).
@MainActor
enum Continued {
    /// what its requests' identifiers begin with, each the transcript's
    /// id and a number after it (Info.plist,
    /// BGTaskSchedulerPermittedIdentifiers, which has the pattern: iOS takes
    /// no pattern to register)
    private static let prefix = "net.pktito.app.transcribe."
    /// the work's task, once iOS runs it (a BGContinuedProcessingTask)
    private static var task: BGTask?
    /// the transcript whose work is under way, and whether iOS has been
    /// asked to continue it
    private static var id: String?
    private static var asked = false
    /// a number for each request: iOS ends an app that registers an
    /// identifier again, and a transcript's speakers are found under the
    /// id it was transcribed under
    private static var serial = 0
    private static var watching = false

    /// What is under way: the file's name, its step in words, and how far
    /// it is, 0 to 1.
    private static var now: (name: String, step: String, done: Double)? {
        let tr = Transcriber.shared
        let spk = Speakers.shared
        if tr.running {
            let step: String
            switch tr.step {
            case .loading: step = L("status_loading")
            case .reading: step = L("status_reading")
            default: step = L("status_transcribing", percent(tr.done))
            }
            return (tr.name, step, tr.done)
        }
        if spk.finding {
            return (
                tr.name, spk.reading ? L("status_reading") : L("speakers_finding"), spk.heard
            )
        }
        return nil
    }

    /// Work on transcript id begins, which the person began: iOS is asked
    /// to continue it when the app leaves the front, or now if it has
    /// (not while it is coming to the front, as with a file sent to it).
    static func begin(id: String) {
        self.id = id
        asked = false
        if !watching {
            watching = true
            NotificationCenter.default.addObserver(
                forName: UIApplication.willResignActiveNotification, object: nil, queue: .main
            ) { _ in MainActor.assumeIsolated { ask() } }
        }
        if UIApplication.shared.applicationState == .background { ask() }
    }

    /// The request, once for the work under way: its handler registered
    /// first, under an identifier of its own.
    private static func ask() {
        guard #available(iOS 26, *), let id, !asked, let now else { return }
        asked = true
        serial += 1
        let identifier = "\(prefix)\(id).\(serial)"
        guard
            BGTaskScheduler.shared.register(
                forTaskWithIdentifier: identifier, using: .main,
                launchHandler: { t in
                    MainActor.assumeIsolated { started(t) }
                })
        else { return }
        let r = BGContinuedProcessingTaskRequest(
            identifier: identifier, title: now.name, subtitle: now.step)
        // now or not at all: the work goes on anyway while the app is on the screen
        r.strategy = .fail
        // the Simulator has no scheduler, and says so here
        do {
            try BGTaskScheduler.shared.submit(r)
            trace("continued: asked for \(identifier)")
        } catch {
            trace("continued: not submitted: \(error.localizedDescription)")
        }
    }

    @available(iOS 26, *)
    private static func started(_ t: BGTask) {
        guard let t = t as? BGContinuedProcessingTask, now != nil else {
            // the work ended before iOS ran its task
            return t.setTaskCompleted(success: true)
        }
        trace("continued: \(t.identifier) running")
        t.progress.totalUnitCount = 1000
        // the person stopped it in the system's UI, or iOS needs its time:
        // a transcription stops, which ends this; a find has no stop, and
        // goes on when the person comes back
        t.expirationHandler = {
            trace("continued: expired")
            if Transcriber.shared.running { Engine.stop() } else { end(done: false) }
        }
        task = t
        update()
    }

    /// The work's step and its progress, told as they change.
    static func update() {
        guard #available(iOS 26, *), let t = task as? BGContinuedProcessingTask, let now else {
            return
        }
        t.progress.completedUnitCount = Int64(min(max(now.done, 0), 1) * 1000)
        t.updateTitle(now.name, subtitle: now.step)
    }

    /// The work ended: done, or stopped or failed. Done is the banner's
    /// last word, which iOS shows for seconds after.
    static func end(done: Bool) {
        id = nil
        guard let t = task else { return }
        trace("continued: ended, done \(done)")
        task = nil
        if #available(iOS 26, *), done, let c = t as? BGContinuedProcessingTask {
            c.progress.completedUnitCount = c.progress.totalUnitCount
            c.updateTitle(Transcriber.shared.name, subtitle: L("status_done"))
        }
        t.setTaskCompleted(success: done)
    }
}
