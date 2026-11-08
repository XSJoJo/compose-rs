package rs.compose.android

import android.view.View
import android.widget.Magnifier

/**
 * One platform magnifier, as Compose's `PlatformMagnifierFactoryApi29Impl.PlatformMagnifierImpl`:
 * what Rust's `Modifier.magnifier` and text selection show (D2074). The module's minimum API (29)
 * has the builder and the zoom set in place, so there's no API 28 path.
 */
class ComposeRsMagnifier private constructor(private val magnifier: Magnifier) {
    /** Its actual width, in pixels. */
    fun getWidth(): Int = magnifier.width

    /** Its actual height, in pixels. */
    fun getHeight(): Int = magnifier.height

    /** Copies the magnified pixels again, as `Magnifier.update`. */
    fun updateContent() = magnifier.update()

    /**
     * Shows it over ([sourceX], [sourceY]) in the view, at ([magnifierX], [magnifierY]) when both
     * are numbers (else where the platform puts it), with [zoom] unless it's NaN.
     */
    fun show(sourceX: Float, sourceY: Float, magnifierX: Float, magnifierY: Float, zoom: Float) {
        if (!zoom.isNaN()) magnifier.zoom = zoom
        if (!magnifierX.isNaN() && !magnifierY.isNaN()) {
            magnifier.show(sourceX, sourceY, magnifierX, magnifierY)
        } else {
            // The platform's default place for the magnifier relative to its source.
            magnifier.show(sourceX, sourceY)
        }
    }

    /** Hides it, as `Magnifier.dismiss`. */
    fun dismiss() = magnifier.dismiss()

    internal companion object {
        /**
         * A magnifier over [view], as Compose's `create`: the text magnifier's defaults (the
         * deprecated constructor, the only public way to them) with [useTextDefault], else the
         * builder's with what's given (a negative size, NaN radius, elevation or zoom: its
         * defaults).
         */
        fun create(
            view: View,
            useTextDefault: Boolean,
            width: Int,
            height: Int,
            cornerRadius: Float,
            elevation: Float,
            clippingEnabled: Boolean,
            initialZoom: Float,
        ): ComposeRsMagnifier {
            if (useTextDefault) {
                @Suppress("DEPRECATION")
                return ComposeRsMagnifier(Magnifier(view))
            }
            val builder = Magnifier.Builder(view)
            if (width >= 0 && height >= 0) builder.setSize(width, height)
            if (!cornerRadius.isNaN()) builder.setCornerRadius(cornerRadius)
            if (!elevation.isNaN()) builder.setElevation(elevation)
            if (!initialZoom.isNaN()) builder.setInitialZoom(initialZoom)
            builder.setClippingEnabled(clippingEnabled)
            return ComposeRsMagnifier(builder.build())
        }
    }
}
