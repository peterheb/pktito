package net.pktito.app

import android.widget.Magnifier
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements

/**
 * Android's magnifier, which a held word shows over the text, as nothing under Robolectric: it has
 * no surface there, and dismissing one threw (Surface.destroy on null, Shots.speakersChange).
 */
@Implements(Magnifier::class)
class NoMagnifier {
    @Implementation fun show(sourceCenterX: Float, sourceCenterY: Float) {}

    @Implementation
    fun show(sourceCenterX: Float, sourceCenterY: Float, magnifierX: Float, magnifierY: Float) {}

    @Implementation fun update() {}

    @Implementation fun dismiss() {}
}
