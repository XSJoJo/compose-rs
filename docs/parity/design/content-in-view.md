# The animated `ContentInViewNode`

Design note for D1372 and D1373, from Compose's `ContentInViewNode.kt`, `BringIntoViewRequestPriorityQueue.kt`, `UpdatableAnimationState.kt`, `BringIntoViewSpec.kt` and `BringIntoViewResponder.kt`, and kept to what was built.

## What Compose does

A request (`BringIntoViewRequester.bringIntoView`, a focused node) calls `bringIntoView` on the nearest `BringIntoViewModifierNode` with a bounds provider. Each responder node (`BringIntoViewResponderNode`) runs two coroutines side by side: its responder's suspending `bringChildIntoView(localRect)`, and the same request passed to its parent, whose bounds are `calculateRectForParent(localRect())`. The request returns when all of them have.

A scrollable's responder is its `ContentInViewNode`. A request that already shows returns at once. Otherwise it goes into a queue sorted by containment, where a request beside the others cancels them. If no animation runs, one starts: in `scroll(MutatePriority.Default)` it animates an `UpdatableAnimationState` (the spec's `scrollAnimationSpec`, a spring) to zero. Each frame scrolls by the fall. After the frame's layout, the requests that now show are resumed, smallest first, and the target is worked out again from the largest request that fits. The animation ends when nothing is left to scroll, and the rest are resumed. If the scroll is interrupted (a drag) or the node is detached, the rest are cancelled. If a frame's scroll isn't all taken (an edge), the animation is cancelled.

## How it fits compose-rs

**The walk.** compose-rs's tree passes a request on itself (D1371): `UiTree::bring_into_view_from` walks the entries before the requester in its chain, then each ancestor's, innermost first. That stays. A node that animates answers `ModifierNode::as_bring_into_view_responder` with a `BringIntoViewResponderNode` (compose-ui):

- `calculate_rect_for_parent(rect, scope)`: where the rect will be once shown.
- `bring_child_into_view(bounds, scope) -> Option<future>`: the future ends once it's shown or interrupted. Dropped, the request is withdrawn.

The walk returns the futures, and the request waits for all of them (`join_all`), which stands for Compose's sibling coroutines. `NodeHandle::bring_into_view` returns that future. `UiTree::bring_into_view` (focus) launches it as a task of its own, since Compose's focusable launches its request too. Nodes that answer at once (`ModifierNode::bring_into_view`, the lazy layouts until C8) are still asked during the walk, with where the rect is then.

**The bounds.** `BringIntoViewBounds` is Compose's `() -> Rect?`. It holds a path: the base (the requester's entry and rect, a focused node, or where a node answering at once left it), the responders inside, and the responder it's for. Reading it peeks at the tree: the base's rect in the root now, through each responder's `calculate_rect_for_parent`, into the responder's layer. Entries are named by `EntryId` and nodes by generational `NodeId`, so a detached requester reads `None`. The walk also asks each responder for its rect for the parent once, right away, to carry the rect on to nodes answering at once. Compose asks only when the parent reads it, so a spec is asked one more time here.

**The frame.** Compose's `afterFrame` runs after the frame's composition and layout. compose-rs runs frame awaiters before both, so the animation waits for the frame's end with a frame-end callback of the thread's recomposer. The host runs these after drawing, then polls the woken tasks in the same frame. Each frame is: `with_frame_nanos`, scroll by the fall (before layout), wait for the frame's end, resume the requests that show, and set the new target.

**Scroll priority.** The animation is `state.scroll(MutatePriority::Default, …)`. A drag takes `UserInput` and interrupts it (D850): the task's `scroll` returns `MutationInterrupted`, and a guard dropped with the task cancels the requests left. A frame whose scroll isn't all taken calls `cancel_current_task`, which is compose-rs's form of Compose's `animationJob.cancel()`.

**The task.** It runs in the container entry's coroutine scope, through a handle the walk registers. Detaching the container cancels it, and the guard cancels the requests.

**Signs.** As Compose, the spec is given offsets in the container's layout coordinates, left to right and top to bottom. The state is scrolled by `±delta`, with the sign set by the scrollable's `reverseDirection`. For `vertical_scroll` and `horizontal_scroll` that is `ScrollableDefaults.reverseDirection(layoutDirection, orientation, reverseScrolling)`, and for `scrollable` it's the parameter. D1371 gave a reversed or right-to-left scroll's spec mirrored offsets, so this is a change.

**The pending scroll.** A `vertical_scroll` lays out its own content. A request read between a scroll and its layout is moved by what the scroll still owes (`value - laid_out`), as in D1371. `scrollable` has no such knowledge, as in Compose.

**Cost.** A container keeps `Option<Rc<ContentInView>>`, `None` until its first request. The queue, the animation state and the config are made then. Asking a container and building its bounds costs only while a request is made.

## What's left out

- `onRemeasured`'s keeping of the focused child in view when the viewport shrinks (feature keep-focused-in-view). It needs `getFocusedRect` and runs on every remeasure, so it's its own step.
- Nested scrolling and overscroll during the animation. Compose scrolls through `ScrollingLogic`, so the connections around see the deltas. compose-rs finds connections only on a pointer's path (D1459), so `focusScroll_nestedScroll_childPartialConsumptionForSemantics` waits on nested-scroll-dispatcher.
- The lazy layouts still jump (C8).
