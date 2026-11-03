import SwiftUI
import UIKit

/// How far a view must move to clear a window's controls, iPadOS 26's
/// three dots in a window's top leading corner: past them, or below them.
/// Zero where they aren't (full screen, a phone) or the view is clear of
/// them. The desktop's window does the same round the Mac's traffic lights
/// (app.css, --lights): the work's header starts past them, a one-column
/// screen's top row and the side's go under them (Peter, the M1 iPad in a
/// window, 2026-11-01).
struct Clearance: Equatable {
    var leading: CGFloat = 0
    var top: CGFloat = 0
}

extension View {
    /// Measures this view's clearance from the window's controls into
    /// clearance, for the view to pad what is in it by: UIKit's safe area
    /// adapted to the window's corner (UIView.LayoutRegion, iOS 26), less
    /// the plain one, in the view's own place, so a view away from the
    /// corner gets none.
    func windowControls(_ clearance: Binding<Clearance>) -> some View {
        background(
            CornerProbe { clearance.wrappedValue = $0 }.allowsHitTesting(false).accessibilityHidden(
                true))
    }
}

private struct CornerProbe: UIViewRepresentable {
    let report: (Clearance) -> Void

    func makeUIView(context: Context) -> Probe { Probe(report: report) }
    func updateUIView(_ v: Probe, context: Context) { v.report = report }

    final class Probe: UIView {
        var report: (Clearance) -> Void
        private var last: Clearance?

        init(report: @escaping (Clearance) -> Void) {
            self.report = report
            super.init(frame: .zero)
            isUserInteractionEnabled = false
        }

        required init?(coder: NSCoder) { fatalError("not from a nib") }

        override func layoutSubviews() {
            super.layoutSubviews()
            measure()
        }

        override func safeAreaInsetsDidChange() {
            super.safeAreaInsetsDidChange()
            measure()
        }

        private func measure() {
            guard #available(iOS 26, *), window != nil else { return }
            let plain = directionalEdgeInsets(for: .safeArea())
            let across = directionalEdgeInsets(for: .safeArea(cornerAdaptation: .horizontal))
            let down = directionalEdgeInsets(for: .safeArea(cornerAdaptation: .vertical))
            let c = Clearance(
                leading: max(0, across.leading - plain.leading), top: max(0, down.top - plain.top))
            guard c != last else { return }
            last = c
            #if DEBUG
                trace(
                    "window controls: \(c) for \(frame.integral) in \(window?.bounds.size ?? .zero), safe \(plain)"
                )
            #endif
            // after this layout, which the padding it asks for would change
            DispatchQueue.main.async { [report] in report(c) }
        }
    }
}
