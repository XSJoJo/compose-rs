# Shared elements and `animateBounds`

Design note, written before building, from Compose's `SharedTransitionScope.kt`, `SharedElement.kt`, `SharedElementEntry.kt`, `SharedContentNode.kt`, `SharedTransitionStateMachine.kt`, `BoundsAnimation.kt`, `AnimateBoundsModifier.kt`, `RenderInTransitionOverlayNodeElement.kt` and `SkipToLookaheadSizeNode.kt` (compose/animation/animation). The features are `shared-transition` and `animate-bounds` in [animation-navigation.md](../animation-navigation.md). It builds on lookahead ([placement-and-lookahead](placement-and-lookahead.md) §7, D1221 to D1226, D1319, D1320, D1327), placing with a layer ([place-with-layer](place-with-layer.md), D1909) and the `GraphicsLayer` object (`graphics_layer_object.rs`).

## What Compose does

**`animateBounds`** is two approach nodes around the user's modifier, sharing nothing but their parameters. Ahead each is what's inside it. In the approach, `isMeasurementApproachInProgress(lookaheadSize)` notes the target size; `isPlacementApproachInProgress(lookaheadCoordinates)` notes the target offset (the node's lookahead position in the scope's coordinates, `localLookaheadPositionOf`) and, when the target moved or resized, launches an `Animatable<Rect>` from the current bounds to it along the `BoundsTransform`'s spec. `approachMeasure` measures at the animated size (the outer node passes the constraints through, the inner one fixes them to it), and places the content at the animated top-left minus where it is now in the scope (`lookaheadScopeCoordinates.localPositionOf(coordinates)`). Offsets a motion frame of reference (a scroll) introduces are left out unless `animateMotionFrameOfReference`, and the node keeps the motion frames it went through so a change of them doesn't animate.

**`SharedTransitionLayout`** is a `LookaheadScope` with a `Box` whose root modifier records the scope's coordinates (`root` now, `lookaheadRoot` ahead) and, after drawing its content, draws the **overlay**: every `LayerRenderer` (a shared entry, or a `renderInSharedTransitionScopeOverlay` node) sorted by z-index, each drawing a `GraphicsLayer` its node recorded its content into.

**`sharedElement` and `sharedBounds`** are composed modifiers. They remember, per key, a `SharedElement` (the key's state machine and its entries), a child transition of the `AnimatedVisibilityScope`'s (`visible`), a deferred `Rect` animation on it (`BoundsAnimation`), and a `SharedElementEntry` (the call site's parameters; a `RememberObserver` joining and leaving the key and the overlay). The node, `SharedBoundsNode`, is an approach node:

- **Ahead**, its placement block tells the key it's placed (`onLookaheadPlaced`): the visible (target) entry of a key with an active match configures the match's **target data**, its lookahead size and its lookahead position in the scope (with and without the motion frames: `structuralOffset`).
- **In the approach**, it measures its content at the animated bounds' size (or the match's current bounds' before the animation starts), and is the `PlaceholderSize` of that and its lookahead size to its parent. Its placement animates the `BoundsAnimation` from the match's current bounds to the target's, writes the animated bounds back as the current bounds (the target entry only), and places its content at them, relative to where it is in the scope.
- **The state machine** (`NoMatchFound`, `ActiveMatchFoundConfigPending`, `ActiveMatchConfigured`, `ActiveMatchRemovedDuringTransition`) is told of entries joining, leaving, enabling and disabling (`updateMatch`, from the node's observation of the entry's target and enablement) and applies what it's told at the next lookahead placement or approach measurement, so a frame's passes see one state.
- **Drawing**: while the scope's transition is active and the entry renders in the overlay, the node records its content into the entry's layer instead of drawing it; the overlay draws the layer at the current bounds' top left, clipped by the entry's `OverlayClip` path. An entry not rendering at all (an invisible `sharedElement`) draws nothing.
- `sharedBounds` adds the visibility scope's enter and exit (fade by default, active only once a match is found) and, for `scaleToBounds`, a `skipToLookaheadSize` with the content scale (the content measured at its lookahead constraints, placed scaled with a layer).
- `isTransitionActive` is a state: true while any enabled, matched entry's bounds animation runs (`BoundsAnimation.isRunning`: the root transition isn't at its target).

## How it maps onto compose-rs

### Approach nodes

D1222 has `Modifier::approach_layout` take closures, which a new modifier call replaces. Both nodes here keep state across modifier calls (an animation, a match), so compose-ui gets the public **`ApproachLayoutModifierNode`** trait: `measure` ahead (a pass-through by default), `is_measurement_approach_in_progress`, `approach_measure`, and the placement check as a block the node gives (`placement_approach_in_progress`, an `Rc` closure over the node's shared state, since compose-rs's placement blocks outlive `measure` and can't borrow the node). A node implementing it delegates its `LayoutModifierNode::measure` to `measure_approach` and calls `NodeContext::approach_attached` on attach; `approach_layout`'s element becomes one such node, so there's one path. Nothing changes for nodes that aren't approach nodes.

### State

The nodes' state that their placement blocks need lives in an `Rc` they share with those blocks (`Rc<RefCell<…>>`, `Cell`s for flags). Compose's `mutableStateOf` fields stay states where something reads them to be told (the match's current bounds, read by the overlay's drawing and the approach measurement; `isTransitionActive`; an entry's enablement), plain cells elsewhere.

### Composed modifiers

compose-rs has no `composed` (D18): `shared_element`, `shared_bounds` and `shared_element_with_caller_managed_visibility` are methods of `SharedTransitionScope` returning a `Modifier`, to call on every pass of the content as `AnimatedVisibilityScope::animate_enter_exit` is; they remember in a group keyed by the content state's key. `SharedContentState`'s `RememberObserver` is a `DisposableEffect` on the entry.

### The overlay on compose-ui's layers

The overlay is drawn by the root modifier's `draw_with_content`: the content, then each renderer's `GraphicsLayer` with `DrawScope::draw_layer`, translated to its bounds in the root and clipped by its path. The layers are the `GraphicsLayer` objects of `graphics_layer_object.rs`: a node records its content into one with `ContentDrawScope::record_layer` (and draws nothing in place); a layer's change draws again, in the same frame, each drawing that drew it other than the pass leaves it, in what the drawing didn't set on it itself (a version per drawing, read in `draw_layer`; D1331, D1337): not what drew it after the change, current already, nor a drawing setting a property and drawing the layer, which would draw the same again, so the root's overlay follows a content's change with no invalidation of its own, drawing once a frame; a change of the bounds is a state read in that drawing. On Android the recorded ops are replayed in the root's group, so the overlay is in the root's render node, above all of the layout's content, as Compose's (which draws it into the root's canvas too).

Placement with a layer (D1909) is what `skipToLookaheadSize`'s scale uses (`place_with_layer`, a block setting the scale and the origin). Nothing else here needs a placement layer: the shared content's motion is a placement offset each frame, as Compose's.

### Lookahead's bounds drive the animation

- The target is read ahead: the shared node's lookahead block (`ApproachLayoutModifierNode::measure`'s placement, `is_looking_ahead()`) reads its lookahead position in the layout's lookahead coordinates, `LookaheadScope::local_lookahead_position_of_with`, and its size ahead.
- The current bounds are read now: the approach placement reads where the node is now in the layout's coordinates (`LayoutCoordinates::local_position_of`), and writes the animated bounds.
- A reversal shows the frame's step toward the old target, then turns back, as Compose's: a frame's animations run before the recomposition reversing the target, and the deferred match update applies in that frame's lookahead placement (D1332).
- A node measured ahead is measured now after (D1319): a change ahead (the other content showing) reaches the approach in the same frame. An animation frame is a state change read in the approach measurement and placement only, so it measures and places the approach alone (D1221's per-pass observation).

### Deferred transitions' manual transforms (D1340)

Compose's deferred transitions (D828) move a content by hand while a target is pending (a predictive back gesture): a scale and its origin in the content's layer, an offset in its placement. Its shared elements follow, as Compose's `SharedElementEntry.activeMutableTransformState` and `SharedBoundsNode`:

- **The state:** the content's `EnterExitNode` (a deferred transition's own content only, Compose's `provideTransformState`: `Transition::is_deferred_root`) provides its `SharedMutableTransformState` as a modifier local, and notes its coordinates, outside the layer, when placed (`parentLayoutCoordinates`). A shared node reads the local when attached. An entry's active state is its content's, if its config permits it (`SharedContentConfig(permitTransformDuringDeferredTransition)`, the scope's `shared_content_config_with`) and its transition is a deferred one's; while mutating, the incoming entry takes the outgoing one's (the transition is held at its old state then, so the targets are inverted), and keeps it through the hand-off.
- **While mutating:** the entry renders in the overlay even if no transition is active (permitted or not: not permitted, it stays put, unscaled), and a permitted one makes the scope's transition active. The overlay draws its layer scaled around the content's pivot (the origin in the content's bounds in the layout) and moved by the offset; the approach placement puts its content at the transformed position, which the content's layer maps back (so, drawn in place, it shows where the overlay would draw it, and its coordinates say so).
- **The hand-off:** once the phase ends, the first approach measurement takes the bounds where the deferred phase left them (the last manual scale and offset applied to the current bounds; the scale left out when drawn in place in its own content, whose layer still scales it) and their velocity from the gesture's, and the bounds animation starts from them (`forcedInitialValue`, `forcedInitialVelocity` of the deferred animation, D828).

### Cost

Nothing is added to compose-ui's frame path: the trait is a node kind's measurement, and the overlay is a draw node of `SharedTransitionLayout`'s. An app without shared transitions runs none of it; the display lists stay the same.

## Not done here

- Visual debugging (`LookaheadAnimationVisualDebugging`, `feature:lookahead-visual-debug`).
