# `NestedScrollDispatcher`

Design note for D1917 and D1918 (with D1920 to D1922, and D1478 for the lazy layouts), from Compose's `NestedScrollModifier.kt` and `NestedScrollNode.kt` (ui) and `Scrollable.kt` (foundation), and kept to what was built.

## What Compose does

`Modifier.nestedScroll(connection, dispatcher)` makes a `NestedScrollNode`, a traversable node that is itself a connection. Its `onPreScroll` asks its parent first (the nearest `NestedScrollNode` above, found with `findNearestAncestor` at each call), then its own connection with what's left; `onPostScroll` asks its own connection first, then the parent. A dispatcher is the node's way out: `dispatchPreScroll` and the others call the node's *parent*, never the node's own connection. A node given no dispatcher makes one of its own.

Attached, the node sets its dispatcher's node and scope. Detached, it clears them, but keeps the nearest attached ancestor it had (`lastKnownParentNode`), and `dispatchPostFling` goes there: a fling that started before the node left still ends its cycle. `dispatcher.coroutineScope` is the outermost active scope of the nested-scroll nodes above (each node's own `coroutineScope`), or a cancelled scope once detached.

Foundation's scrollables delegate to `nestedScrollModifierNode(connection, dispatcher)`: they are nested-scroll parents of what's inside, and their drags, flings, keys, semantics actions and `ContentInViewNode` animations all dispatch through the dispatcher.

## How it fits compose-rs

**What's there.** compose-rs found connections on the pointer's hit path (D188): each nested-scroll node, a pointer observer, adds its connection in the initial pass, and a scrollable takes the ones before it as an event starts. D188 rejected walking the tree from a handler, because the tree is borrowed while a node handles an event. That path stays for drags and the wheel: it costs one `Vec` push per node per press.

**The tree's chain.** Each `NestedScrollNode` now holds a shared link (`Rc`) with its connection and its parent's link, and is traversable under one key. Its `on_attach` takes a handle and finds its parent with `find_nearest_ancestor`. The tree is free for the node's own context then, and children attach before parents with every node already marked attached, so a fresh subtree links up as it attaches.

What can change a parent after that is a nested-scroll node attaching above (a chain change or an insertion) or leaving, or the node itself moving. A thread-wide epoch counts these attaches and detaches, and the subtrees the tree's `Applier::attach` puts under an attached parent (D1920). That last one is how movable content moves a node: removed and attached again in one pass, its modifier nodes stay attached and are never told (docs/parity/design/modifier-nodes.md; Compose detaches and attaches them, a parity gap listed in missing.md), so no `on_detach` or `on_attach` would bump the epoch. A link resolved under an older epoch is resolved again at its next use, through `NodeHandle::with_node` (O(depth), as Compose's `findNearestAncestor` at each call).

When the tree is busy (a dispatch from inside a node callback, or anything holding the tree), the last link is used, and links to nodes that left are skipped, each giving way to the parent it had when it left. The cases a busy dispatch gets wrong are a nested-scroll node attached *above*, or the node moved, since the last free resolution and before the dispatch: the first misses the new parent, the second reaches the old one, still attached. No built-in dispatches from there (a click's handler and a task run with the tree free); a free dispatch, or the next one after it, puts it right.

**The dispatcher.** `NestedScrollDispatcher` is a cheap `Rc` handle, compared by identity in the element. Attached, the node sets the dispatcher's link to its own. An update that moves a dispatcher to another node clears it from the old one only if it still points there, as Compose's `resetDispatcherFields`. A dispatch collects the links above the node into a small vector and calls the connections outside any borrow, the outermost first before the scroll and the innermost first after. This is `NestedScrollChain`'s order, and it equals Compose's recursion.

**Detaching.** A detached link keeps the parent it had then (`last_parent`). `dispatch_post_fling` from a detached dispatcher walks from there. The other three return zero, as Compose's.

**The scope.** `coroutine_scope()` is the outermost active node scope above, through the links: each node's scope comes from its handle (`NodeHandle::coroutine_scope`, made on first use). Detached, it's a freed scope, which is never active.

**Velocities stay `Offset`s.** The connection's fling callbacks take `Offset` (D891). The dispatcher does the same, so `Velocity(x, y)` in Compose's tests is `Offset::new(x, y)` here. Moving both to `Velocity` is one change, still listed under geometry-unit-api.

**Cost.** Nothing for an app without `Modifier.nested_scroll` but an epoch bump (a thread-local add) per subtree the tree attaches under an attached parent (a paused composition's node, or movable content moved, D1920). A nested-scroll node costs a handle, an ancestor walk on attach, and an epoch bump on attach and detach. A dispatch costs a walk of the links, plus one ancestor walk per stale link while the tree is free.

## Scrollables in the tree's chain (D1918)

`Modifier.scrollable`, `vertical_scroll` and `horizontal_scroll` take part in the tree's chain at no cost until it's used. A first version delegated to a nested-scroll node in each scrollable, as Compose does. It cost 5.8 % more instructions on `first_frame_scrollables`: a delegate, a registered handle, an ancestor walk and a connection per scrollable.

**As a parent.** A nested-scroll node looking for its parent visits the traversable nodes above it. A node that isn't a `NestedScrollNode` is asked `ModifierNode::as_nested_scroll_participant`. A scrollable makes its `NestedScrollParticipant` then (a link like a nested-scroll node's, with a registered handle), so only scrollables with a nested-scroll node inside pay for one. Its connection holds the scrollable's `ScrollTarget` (state, axis, direction, fling behavior, density) and whether it's enabled, set again on each update, density change and layout-direction change. What a dispatcher inside leaves scrolls the state, unless a scroll of the state's own is going on (Compose's `performRawScroll`). A post-fling velocity flings it, its fling's deltas offered to the chain above it. Disabled, it takes nothing of either, as Compose's `ScrollableNestedScrollConnection` (D1921), and stays in the chain: what's left goes on to the parents above it.

**Its own scrolls.** At attach a scrollable keeps an unregistered handle (`NodeContext::unregistered_handle`), a copy and nothing more. Drags, the wheel and flings keep the pointer's path. The scrolls a pointer doesn't start find the chain above the scrollable through that handle when they run (`NestedScrollChain::above`, one ancestor walk with the tree free):

- Page Up and Page Down, in the task the key launches;
- the `ScrollBy` semantics action, at once as before (D491), in a scope over `dispatch_raw_delta`;
- the bring-into-view animation's deltas, in its scroll.

These keep no link: the tree busy, `NestedScrollChain::above` answers `None` and the scroll reaches no parent (D1922). None of them runs busy: the key's task, the bring-into-view animation and the fling a dispatcher inside hands a scrollable (`TreeConnection::on_post_fling`, in the dispatcher's task) run with the tree free, and the host calls a semantics action outside the tree. Keeping a last link for them would cost each scrollable a shared cell at attach, which D1918 kept off the first frame.

## The lazy layouts and `scrollable_2d` (D1478)

Lists, grids, staggered grids and pagers share one node, `LazyScrollNode`, which takes part as a scrollable does: an unregistered handle at attach, and a participant made when a nested-scroll node in an item looks for its parent. What a dispatcher or a scrollable inside an item leaves scrolls the list first, as Compose's lazy layouts (each a `scrollable`), then what's above it.

**Its estimated ends.** A list's end not laid out is an estimate: its scroll takes the whole delta and the measurement finds how far it went, offering the rest to the parents its scope names (D479). The tree connection scrolls through that scope too (`ScrollTarget::set_outer`), naming the parents above the list, found when it scrolls; `LazyListState::dispatch_raw_delta` reads them as the consumer of its scrolls did. So a dispatcher's leftover past an estimated end reaches the parents above in the frame after, as a drag's does. A dispatch from a busy tree finds no parents there, and that rest is lost.

**Its own scrolls.** Page keys, and the `ScrollBy` semantics action (at once, D491), go through the nodes above, as a scrollable's. A pager's page connection is a nested-scroll node inside the pager, so it comes first: outside a gesture it takes what's left along the pager, as Compose's (`isAbsorbNonGestureNestedScrollInPagerFixEnabled`), and a dispatcher in a page moves its pager only during a drag.

**Bringing into view.** A lazy layout answers a bring-into-view request at once (`ModifierNode::bring_into_view`, its rect where the item will be), with the tree busy: `NestedScrollChain::above` finds nothing then, and its scroll reaches no parent. Compose's animates it through the chain (`ContentInViewNode`). This stays with the lazy layouts' bring-into-view gap in missing.md: it closes when they animate.

**`scrollable_2d`.** Traversable under the drag gestures' key (Compose's is a `DragGestureNode`, its orientation none, so no drag is aligned with it), it makes a connection over its logic for the nodes inside, and its `ScrollBy` goes through the nodes above. It has no Page keys or bring-into-view in compose-rs.

**Not yet.** `LocalScrollCaptureInProgress` doesn't exist, so a semantics scroll always dispatches.
