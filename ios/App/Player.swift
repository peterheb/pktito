import AVFoundation
import Foundation

/// The transcript's recording, played as the desktop's player plays it
/// (Android's Player.kt): at 0.75 to 2x, its pitch kept, a time sought to
/// exactly, where the rows and the strip follow it; paused when the app
/// leaves the screen. AVPlayer seeks an MP3 to the frame with no tolerance,
/// which Android's player could not, so the recording plays as it is.
@MainActor @Observable
final class Player {
    static let shared = Player()
    static let speeds: [Double] = [0.75, 1, 1.25, 1.5, 1.75, 2]

    private(set) var ready = false
    private(set) var playing = false
    private(set) var position = 0.0
    private(set) var speed = 1.0
    private(set) var volume = 1.0
    private(set) var muted = false
    /// what is loaded: a transcript's id and its recording
    private(set) var loaded: String?

    private var player: AVPlayer?
    private var url: URL?
    private var scoped = false
    private var observer: Any?
    private var ended: NSObjectProtocol?
    private var status: NSKeyValueObservation?

    private init() {
        let d = UserDefaults.standard
        speed = d.object(forKey: "speed") as? Double ?? 1
        volume = d.object(forKey: "volume") as? Double ?? 1
    }

    /// url played for the transcript key, the one before let go; ready
    /// once AVFoundation has read it, or failed where it can't play it.
    func load(_ key: String, _ url: URL, failed: @escaping @MainActor () -> Void) {
        release()
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
        scoped = url.startAccessingSecurityScopedResource()
        self.url = url
        let item = AVPlayerItem(url: url)
        item.audioTimePitchAlgorithm = .timeDomain
        let p = AVPlayer(playerItem: item)
        p.volume = Float(muted ? 0 : volume)
        player = p
        loaded = key
        status = item.observe(\.status) { [weak self] item, _ in
            let s = item.status
            Task { @MainActor in
                guard let self, self.loaded == key else { return }
                if s == .readyToPlay {
                    self.ready = true
                } else if s == .failed {
                    self.ready = false
                    failed()
                }
            }
        }
        observer = p.addPeriodicTimeObserver(forInterval: CMTime(value: 1, timescale: 30), queue: .main) { [weak self] t in
            MainActor.assumeIsolated { self?.position = t.seconds.isFinite ? t.seconds : 0 }
        }
        ended = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.playing = false }
        }
    }

    func release() {
        if let observer { player?.removeTimeObserver(observer) }
        if let ended { NotificationCenter.default.removeObserver(ended) }
        observer = nil
        ended = nil
        status?.invalidate()
        status = nil
        player?.pause()
        player = nil
        if scoped { url?.stopAccessingSecurityScopedResource() }
        scoped = false
        url = nil
        loaded = nil
        ready = false
        playing = false
        position = 0
    }

    func toggle() {
        guard let player else { return }
        if playing {
            player.pause()
            playing = false
        } else {
            try? AVAudioSession.sharedInstance().setActive(true)
            player.playImmediately(atRate: Float(speed))
            playing = true
        }
    }

    func pause() {
        player?.pause()
        playing = false
    }

    /// t sought to exactly, and played from there where play is.
    func seek(_ t: Double, play: Bool) {
        guard let player else { return }
        position = t
        player.seek(to: CMTime(seconds: t, preferredTimescale: 600), toleranceBefore: .zero, toleranceAfter: .zero)
        if play && !playing { toggle() }
    }

    func setSpeed(_ s: Double) {
        speed = s
        UserDefaults.standard.set(s, forKey: "speed")
        if playing { player?.rate = Float(s) }
    }

    func setVolume(_ v: Double, keep: Bool) {
        volume = v
        muted = false
        player?.volume = Float(v)
        if keep { UserDefaults.standard.set(v, forKey: "volume") }
    }

    /// Muted, or not: unmuting at none sets it to full, which is kept.
    func toggleMute() {
        muted.toggle()
        if !muted && volume == 0 { volume = 1 }
        player?.volume = Float(muted ? 0 : volume)
        UserDefaults.standard.set(volume, forKey: "volume")
    }
}
