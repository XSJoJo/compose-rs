# Layout-rect callbacks

Design note, written before building, from Compose's `spatial/` (`RectManager.kt`, `ThrottledCallbacks.kt`, `RelativeLayoutBounds.kt`, `RectList.kt`), `layout/OnLayoutRectChangedModifier.kt`, `OnGlobalLayoutListener.kt`, `OnVisibilityChangedModifier.kt` and `LayoutBoundsHolder.kt` at the ledger's commit. The feature is `layout-rect-callbacks` in [ui-core.md](../ui-core.md) (§3.22).

## What Compose does

Every `Owner` has a `RectManager`. It keeps a `RectList`, a flat array holding the bounding box in the root of **every placed layout node**, updated as each node is placed (`recalculateRectIfDirty` from `MeasurePassDelegate.placeAt`) and as a coordinator's layer changes (`onCoordinatorRectChanged`). Rects are stored relative to the parent's so a move shifts a subtree in one pass. After each `measureAndLayout`, `dispatchCallbacks` walks the rects marked updated and hands them to `ThrottledCallbacks`, which calls the registered callbacks now, or later for a throttle or a debounce. Debounced calls are run by a `postDelayed` of the dispatch at the earliest deadline, at least 16 ms away. The callbacks get a `RelativeLayoutBounds`: the rect in the root, the window's and screen's offsets and the window's size, and the node (for `calculateOcclusions`, which intersects the `RectList`).

## What compose-rs does

### Where rects are tracked

No index of every node. compose-rs computes a rect only for the layout nodes something registered on, when it may have changed.

- `UiTree` gets one field, `rects: Option<Box<RectManager>>`, made by the first registration (`register_on_layout_rect_changed` or `register_on_global_layout_listener`, from a node's `NodeContext`).
- The manager keeps, per registered layout node, the last rect it computed (`None` while unplaced), and the throttled entries (Compose's `ThrottledCallbacks.Entry`: the node's chain entry, the throttle and debounce, the callback, the last invoke and the last fire not invoked).
- A node's rect is Compose's: its outer layer's measured size at its origin, mapped to the root through every offset and graphics layer on the way (`UiTree::rect_in_root`, the walk `bounds_in_root` makes, without the clips), as `boundingRectInRoot`, truncated to integers. A callback registered by a modifier inside the node's layout modifiers gets `rectInfoFor`'s rect: that top left moved by its layer's offset in the node, at its layer's size.
- Placement is untouched: nothing is computed while measuring or placing. After a layout, the tree recomputes the registered nodes' rects (`take_layout_callbacks`, after the position-aware nodes are told); after a graphics layer's change with no layout, those under a moved layer (the `layers_moved` path `on_globally_positioned` uses). A rect that changed goes to the entries as Compose's `fireOnUpdatedRect`.
- Occlusions (`calculate_occlusions`) are computed when asked: one walk of the placed nodes collects their rects, and those intersecting the target and drawn after it (Compose's `isTargetDrawnFirst`: the branching ancestors' z-index, then place order; ancestors and descendants never occlude) are the answer. The `RectList` data structure is ported on its own, with its tests, as the index such a walk fills; it isn't kept between queries.

### Zero cost when unused

With no registration, `rects` is `None`. The hooks are a `None` test each, in code that already runs only after a layout or a layer change: `take_layout_callbacks`, `note_layer_change`'s early return (which also returns when no node listens to positions), `set_window_origin` and `forget_layers_no_one_listens_to`. Nothing is added to measuring, placing, drawing or to `LayoutNode`. A removed node's registrations go with its modifier nodes' `on_detach`, which unregisters them; the manager then forgets the layout node at its next dispatch.

### Throttle, debounce and the clocks

- Time is the executor's clock in milliseconds (`platform::now_nanos() / 1_000_000`): the host's monotonic clock, or a test's manual main clock. Compose reads the wall clock; the executor's is the one tests control.
- Throttling is decided when a changed rect is dispatched, after the layout, as Compose's. `ThrottledCallbacks` is ported as it is (the three cases of `fireWithUpdatedRect`, `triggerDebounced`, `minDebounceDeadline`, debounce 0 taking the throttle's value), with its own tests run on explicit times.
- Debouncing: after a dispatch that leaves a deadline, the manager launches one executor task, `delay(max(deadline, now + 16 ms) - now)`, which takes the free tree and dispatches again (Compose's `postDelayed(dispatchLambda)`); a new deadline cancels and replaces it. The frame clock isn't involved: a debounced call needs no frame, as Compose's needs no vsync. In compose-test the task fires as the main clock passes the deadline (`advance_time_by`, `wait_until`); `wait_for_idle` doesn't wait for it, as Compose's idling doesn't wait for a delayed post (D900).
- Callbacks run outside the tree, after the dispatch, as the other layout callbacks (`Action`s run by the host after `take_layout_callbacks`, or by the dispatching task): they may write state, register and unregister.
- The window: compose-rs's root fills the window, so `position_in_window` is the rect in the window (a popup's content: from the popup's root, as `LayoutCoordinates`), and `position_in_screen` adds `UiHost::set_window_origin`'s origin; a new origin fires every entry, as `isScreenOrWindowDirty`. There's no view-to-window matrix (compose-rs has no transformed host view).

### The modifiers

`on_layout_rect_changed(throttle, debounce, callback)`, `layout_bounds(holder)` and `on_visibility_changed(min_duration, min_fraction, viewport, callback)` (with `on_visibility_changed_node` for delegation) are modifier nodes registering in `on_attach` and unregistering in `on_detach`, as Compose's. `LayoutBoundsHolder` holds its bounds in a state. `on_visibility_changed`'s minimum duration is a task in the node's coroutine scope; it is an `UnplacedAwareModifierNode` and an observer of its viewport's state, as Compose's.
