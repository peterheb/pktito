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
    /// the seeks asked of AVPlayer and not yet done: until the last is, its
    /// clock can still tell the time before it (an exact seek in a FLAC with
    /// no seek table takes a while), and a row that time is in would take
    /// the rows back there (Peter's 16e, 2026-11-02: a tap at 4:52 while
    /// 1:40 played, and the list went back to 1:34)
    private var seeking = 0

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
        // precise, so that a time sought to is that time's audio: without
        // it AVFoundation guesses where a time is in a file with no index of
        // its frames, as a FLAC without a seek table: in AliceBench's 12
        // chapters as one FLAC (2:58:51) a read at 1 h was 86.5 s late and
        // one at 2.5 h 219.6 s early, and exact with it (the Mac, 2026-11-01)
        let item = AVPlayerItem(asset: AVURLAsset(url: url, options: [AVURLAssetPreferPreciseDurationAndTimingKey: true]))
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
            MainActor.assumeIsolated {
                guard let self, self.seeking == 0 else { return }
                self.position = t.seconds.isFinite ? t.seconds : 0
            }
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
        seeking = 0
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
        seeking += 1
        let key = loaded
        // done, or given up for the next seek: each is called once
        player.seek(to: CMTime(seconds: t, preferredTimescale: 600), toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
            Task { @MainActor in
                guard let self, self.loaded == key, self.seeking > 0 else { return }
                self.seeking -= 1
            }
        }
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
