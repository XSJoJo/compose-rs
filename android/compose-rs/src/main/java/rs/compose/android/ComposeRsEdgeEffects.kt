package rs.compose.android

import android.content.Context
import android.graphics.Canvas
import android.graphics.RenderNode
import android.os.Build
import android.util.SparseArray
import android.widget.EdgeEffect
import kotlin.math.abs

/**
 * One edge's `EdgeEffect`, as Rust's `compose_ui::EdgeEffect` calls it (`services.rs`), made at its
 * side's first pull. Where the platform stretches (API 31 on), it keeps the effect that takes back
 * what the render node was given by the last frame ([negation]): see
 * [ComposeRsEdgeEffects.drawStretch]. Where it glows, `distance` stays 0 and a pull is a whole
 * pull, as Compose's `EdgeEffectCompat`.
 */
class ComposeRsEdgeEffect internal constructor(
    private val context: Context,
    private val color: Int,
    width: Int,
    height: Int,
) {
    private var width = width
    private var height = height
    internal val effect: EdgeEffect = make()
    /** Pulled, after each frame, by what the frame stretched its node; its side's opposite draws it. */
    internal var negation: EdgeEffect? = null
    private var displacement = 0.5f

    private fun make(): EdgeEffect {
        val made =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) EdgeEffect(context, null)
            else GlowEdgeEffect(context)
        made.color = color
        if (width != 0 || height != 0) made.setSize(width, height)
        return made
    }

    fun onPullDistance(delta: Float, displacement: Float): Float {
        this.displacement = displacement
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            return effect.onPullDistance(delta, displacement)
        }
        effect.onPull(delta, displacement)
        return delta
    }

    fun onRelease() = effect.onRelease()

    fun onReleaseWithOppositeDelta(delta: Float) {
        val glow = effect as? GlowEdgeEffect
        if (glow != null) glow.releaseWithOppositeDelta(delta) else effect.onRelease()
    }

    fun onAbsorb(velocity: Int) {
        // A glow from a drag doesn't add a fling's: it just decays.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S || effect.isFinished) effect.onAbsorb(velocity)
    }

    fun getDistance(): Float =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) effect.distance else 0f

    /** Nothing to draw: over, and no stretch left on the node to take back. */
    fun isFinished(): Boolean = effect.isFinished && !isNegationStretched()

    fun finish() = effect.finish()

    fun setSize(width: Int, height: Int) {
        this.width = width
        this.height = height
        effect.setSize(width, height)
        negation?.setSize(width, height)
    }

    internal fun finishAll() {
        effect.finish()
        negation?.finish()
    }

    internal fun isNegationStretched(): Boolean {
        val negation = negation ?: return false
        return Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && negation.distance != 0f
    }

    /** What this frame stretched goes in [negation], for the next frame to take back first. */
    internal fun negate() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return
        if (effect.distance == 0f) return
        val negation = negation ?: make().also { negation = it }
        negation.onPullDistance(effect.distance, displacement)
    }
}

/**
 * Before API 31, as Compose's `GlowEdgeEffectCompat`: a release by a delta the other way only past
 * a dp in all, as a finger holding a pull shakes a little, and the framework's glow would vanish
 * at the first small delta back (b/242864658).
 */
private class GlowEdgeEffect(context: Context) : EdgeEffect(context) {
    private val threshold = context.resources.displayMetrics.density
    private var oppositeDelta = 0f

    override fun onPull(deltaDistance: Float, displacement: Float) {
        oppositeDelta = 0f
        super.onPull(deltaDistance, displacement)
    }

    override fun onPull(deltaDistance: Float) {
        oppositeDelta = 0f
        super.onPull(deltaDistance)
    }

    override fun onRelease() {
        oppositeDelta = 0f
        super.onRelease()
    }

    override fun onAbsorb(velocity: Int) {
        oppositeDelta = 0f
        super.onAbsorb(velocity)
    }

    fun releaseWithOppositeDelta(delta: Float) {
        oppositeDelta += delta
        if (abs(oppositeDelta) > threshold) onRelease()
    }
}

/**
 * A view's edge effects, by their scroller's number, and how a frame draws them, as Compose's
 * `StretchOverscrollNode` and `GlowOverscrollNode`. Nothing here exists for a scroller until a side
 * of it is pulled.
 */
internal class ComposeRsEdgeEffects(private val context: Context) {
    class Owner {
        val sides = arrayOfNulls<ComposeRsEdgeEffect>(SIDES)
        var node: RenderNode? = null
    }

    private val owners = SparseArray<Owner>()

    fun create(owner: Int, side: Int, color: Int): ComposeRsEdgeEffect {
        var kept = owners.get(owner)
        if (kept == null) {
            kept = Owner()
            owners.put(owner, kept)
        }
        val made = ComposeRsEdgeEffect(context, color, 0, 0)
        kept.sides[side] = made
        return made
    }

    fun release(owner: Int) {
        owners.get(owner)?.node?.discardDisplayList()
        owners.remove(owner)
    }

    fun owner(owner: Int): Owner? = owners.get(owner)

    /** The render nodes' recordings go: the window's gone, or the view's forgetting its groups. */
    fun discardNodes() {
        for (index in 0 until owners.size()) owners.valueAt(index).node?.discardDisplayList()
    }

    fun stretches(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

    fun finishAll(owner: Owner) {
        for (side in owner.sides) side?.finishAll()
    }

    /** Whether something of it stretches vertically or horizontally this frame. */
    private fun shows(effect: ComposeRsEdgeEffect?): Boolean =
        effect != null && (!effect.effect.isFinished || effect.isNegationStretched())

    fun showsVertical(owner: Owner): Boolean = shows(owner.sides[TOP]) || shows(owner.sides[BOTTOM])

    fun showsHorizontal(owner: Owner): Boolean = shows(owner.sides[LEFT]) || shows(owner.sides[RIGHT])

    /**
     * The stretch for a frame, on the recording canvas of [Owner.node], before its content.
     *
     * Views call `RenderNode.clearStretch()` as they draw, which is hidden: a node's stretch adds
     * up across frames. So what the last frame gave the node is taken back first, by [negation]'s
     * effect drawn at the opposite side, as Compose does; then the frame's distance goes on. Its
     * `draw` doesn't draw: it sets the stretch on the node, so it comes before the content.
     */
    fun drawStretch(owner: Owner, canvas: Canvas) {
        for (side in STRETCH_ORDER) {
            val effect = owner.sides[side] ?: continue
            if (effect.isNegationStretched()) {
                val negation = effect.negation!!
                drawWithRotation(negation, ROTATIONS[OPPOSITE[side]], canvas)
                negation.finish()
            }
            if (!effect.effect.isFinished) {
                drawWithRotation(effect.effect, ROTATIONS[side], canvas)
                effect.negate()
            }
        }
    }

    private fun drawWithRotation(effect: EdgeEffect, degrees: Float, canvas: Canvas) {
        if (degrees == 0f) {
            effect.draw(canvas)
            return
        }
        val restore = canvas.save()
        canvas.rotate(degrees)
        effect.draw(canvas)
        canvas.restoreToCount(restore)
    }

    /** The glows, over the content at the canvas's origin, of a scroller of [width] × [height]. */
    fun drawGlow(
        owner: Owner,
        canvas: Canvas,
        width: Float,
        height: Float,
        left: Float,
        top: Float,
        right: Float,
        bottom: Float,
    ) {
        for (side in STRETCH_ORDER) {
            val effect = owner.sides[side] ?: continue
            if (effect.effect.isFinished) continue
            val restore = canvas.save()
            canvas.rotate(ROTATIONS[side])
            when (side) {
                LEFT -> canvas.translate(-height, left)
                TOP -> canvas.translate(0f, top)
                RIGHT -> canvas.translate(0f, -width + right)
                else -> canvas.translate(-width, -height + bottom)
            }
            effect.effect.draw(canvas)
            canvas.restoreToCount(restore)
        }
    }

    companion object {
        /** As `compose_ui::EdgeEffectSide`. */
        const val TOP = 0
        const val BOTTOM = 1
        const val LEFT = 2
        const val RIGHT = 3
        const val SIDES = 4
        /** Compose's order of the sides. */
        private val STRETCH_ORDER = intArrayOf(LEFT, TOP, RIGHT, BOTTOM)
        private val OPPOSITE = intArrayOf(BOTTOM, TOP, RIGHT, LEFT)
        private val ROTATIONS = floatArrayOf(0f, 180f, 270f, 90f)
    }
}
