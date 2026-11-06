//! The layout-rect callbacks' bookkeeping, as Compose's `RectManager` (design:
//! docs/parity/design/layout-rect-callbacks.md).
//!
//! No index of every node: a tree computes the rects of the layout nodes something registered on,
//! when a layout, a graphics layer, a registration or the window may have changed them, and
//! nothing at all until a first registration (`UiTree::rects` stays `None`).

mod relative_layout_bounds;
mod throttled_callbacks;

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use compose_runtime::{Callback, IntoCallback, Job};

use crate::graphics_layer::GraphicsLayerNode;
use crate::input::Action;
use crate::layout_callbacks::layer_origin;
use crate::node_context::NodeContext;
use crate::tree::{NodeId, UiTree};
use crate::unit::{IntOffset, IntRect, IntSize, Offset};

pub use relative_layout_bounds::RelativeLayoutBounds;
use throttled_callbacks::{Fired, ThrottledCallbacks};

/// The shortest wait for a debounced dispatch, as Compose's "next frame-ish" (16 ms).
const NEXT_FRAME_ISH: i64 = 16;

/// A registered layout node and its rect when last looked at.
struct Tracked {
    node: NodeId,
    /// In the window; `None` while it isn't placed (or wasn't looked at yet).
    rect: Option<IntRect>,
    /// Where its inner layers are in it: a move of one is told, as Compose's
    /// `invalidateCallbacksFor` when an inner coordinator moves.
    inner: u64,
    /// Its callbacks hear its rect at the next dispatch even if it didn't change (a new
    /// registration, as `updateHasCallbacksAt` marks the rect updated).
    updated: bool,
}

/// A tree's layout-rect callbacks, as Compose's `RectManager`, made at the first registration.
#[derive(Default)]
pub(crate) struct RectManager {
    callbacks: ThrottledCallbacks,
    tracked: Vec<Tracked>,
    /// A rect callback was registered: the next dispatch looks at the rects, and calls the
    /// global listeners, as Compose's `invalidate()` there (a global listener's registration
    /// invalidates nothing).
    dirty: bool,
    /// The layout of the whole tree when the global listeners last heard of it: for them, a
    /// change anywhere is a change, as Compose's rect list marked dirty by any node's move.
    tree_signature: Option<u64>,
    /// The window's origin and size the entries last heard of.
    window: Option<(IntOffset, IntSize)>,
    /// The dispatch scheduled for a debounce: its deadline and its task.
    scheduled: Option<(i64, Job)>,
}

/// What unregisters a callback, as Compose's `DelegatableNode.RegistrationHandle`.
pub struct RegistrationHandle {
    rects: Weak<RefCell<RectManager>>,
    id: u64,
}

impl RegistrationHandle {
    /// Unregisters the callback: it's not called any more. Once is enough; later calls do
    /// nothing.
    pub fn unregister(&self) {
        if let Some(rects) = self.rects.upgrade() {
            rects.borrow_mut().callbacks.unregister(self.id);
        }
    }
}

impl std::fmt::Debug for RegistrationHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("RegistrationHandle").field("id", &self.id).finish()
    }
}

/// The executor's clock in milliseconds: a test's main clock, or the host's.
fn now_millis() -> i64 {
    (compose_runtime::platform::now_nanos() / 1_000_000) as i64
}

impl NodeContext<'_> {
    /// Calls `callback` with the bounds of where this node is in its layout node (in the root,
    /// the window and the screen) after a layout or a graphics layer moved it, at most once a
    /// `throttle_millis` and once it rested `debounce_millis`, as Compose's
    /// `DelegatableNode.registerOnLayoutRectChanged`. Both 0: at each change. A throttle with no
    /// debounce debounces by the throttle, so the last place is heard. Returns what unregisters
    /// it, which `on_detach` should do.
    pub fn register_on_layout_rect_changed(
        &mut self,
        throttle_millis: i64,
        debounce_millis: i64,
        callback: impl IntoCallback<dyn Fn(RelativeLayoutBounds)>,
    ) -> RegistrationHandle {
        self.register_rect_callback(
            throttle_millis,
            debounce_millis,
            callback.into_callback(),
            false,
        )
    }

    /// Calls `callback` with this node's bounds after any change of the layout, a graphics layer
    /// or the window, throttled and debounced as `register_on_layout_rect_changed`, as Compose's
    /// `DelegatableNode.registerOnGlobalLayoutListener`: for what depends on other nodes (its
    /// occlusions).
    pub fn register_on_global_layout_listener(
        &mut self,
        throttle_millis: i64,
        debounce_millis: i64,
        callback: impl IntoCallback<dyn Fn(RelativeLayoutBounds)>,
    ) -> RegistrationHandle {
        self.register_rect_callback(
            throttle_millis,
            debounce_millis,
            callback.into_callback(),
            true,
        )
    }

    fn register_rect_callback(
        &mut self,
        throttle: i64,
        debounce: i64,
        callback: Callback<dyn Fn(RelativeLayoutBounds)>,
        global: bool,
    ) -> RegistrationHandle {
        let node = self.node;
        // A delegate reports where its owner is.
        let owner = match self.index {
            Some(index) => self.tree.node(node).chain[index].id,
            None => self.entry,
        };
        let mut tree = self.tree.enter();
        let rects = tree.rects.get_or_insert_with(Rc::default).clone();
        let id = {
            let mut manager = rects.borrow_mut();
            let manager = &mut *manager;
            if global {
                manager
                    .callbacks
                    .register_on_global_change(node, owner, throttle, debounce, callback)
            } else {
                manager.dirty = true;
                match manager.tracked.iter_mut().find(|tracked| tracked.node == node) {
                    Some(tracked) => tracked.updated = true,
                    None => {
                        manager.tracked.push(Tracked { node, rect: None, inner: 0, updated: true })
                    }
                }
                manager
                    .callbacks
                    .register_on_rect_changed(node, owner, throttle, debounce, callback)
            }
        };
        // Outside a frame (a task's), one dispatches it.
        tree.focus_requests.wake();
        RegistrationHandle { rects: Rc::downgrade(&rects), id }
    }
}

impl UiTree {
    /// Hands what changed since the last dispatch to the callbacks, as `dispatchCallbacks`:
    /// the registered nodes' rects if `changed` (a layout, a graphics layer moved), the window's
    /// move, and the debounced calls now due; schedules the next debounced one. The callbacks to
    /// run go to `layout_callbacks`.
    pub(crate) fn dispatch_rects(&mut self, changed: bool) {
        let Some(rects) = self.rects.clone() else { return };
        let now = now_millis();
        let fired = {
            let mut manager = rects.borrow_mut();
            let manager = &mut *manager;
            let origin = self.window_origin;
            let window =
                (IntOffset::new(origin.x.round() as i32, origin.y.round() as i32), self.window);
            let window_moved = manager.window.is_some_and(|last| last != window);
            manager.window = Some(window);
            manager.callbacks.begin_dispatch();
            let registered = std::mem::take(&mut manager.dirty);
            // Compose's `isDirty`: a rect changed or a callback came.
            let mut dirty = registered;
            if changed || registered {
                manager
                    .tracked
                    .retain(|tracked| manager.callbacks.has_rect_callbacks(tracked.node));
                // Parents first, as Compose's rect list (a child's callback may read a parent's
                // `LayoutBoundsHolder`, which the parent's sets).
                manager.tracked.sort_by_cached_key(|tracked| self.depth_of(tracked.node));
                for index in 0..manager.tracked.len() {
                    let Tracked { node, rect: last, inner: last_inner, updated } =
                        manager.tracked[index];
                    let rect = self.rect_of_layout_node(node);
                    let inner = self.inner_layers_signature(node);
                    // Its own graphics layer, or a layer in it, moved what's in it.
                    let updated = updated
                        || inner != last_inner
                        || self.get(node).is_some_and(|value| value.layer_moved);
                    if rect != last || (updated && rect.is_some()) {
                        dirty = true;
                        manager.tracked[index].rect = rect;
                        manager.tracked[index].inner = inner;
                        manager.tracked[index].updated = false;
                        if let Some(rect) = rect {
                            manager.callbacks.fire_on_updated_rect(node, rect, now);
                        }
                    }
                }
            }
            if window_moved {
                manager.callbacks.fire_on_rect_changed_entries(now);
            }
            // Any node's move, which only a look at the whole tree tells (with global listeners
            // only): its layers' places and sizes, and a graphics layer's move.
            if changed && manager.callbacks.has_global() {
                let signature = self.tree_signature();
                if manager.tree_signature != Some(signature) || !self.layers_moved.is_empty() {
                    manager.tree_signature = Some(signature);
                    dirty = true;
                }
            }
            if dirty || window_moved {
                manager.callbacks.fire_global_change_entries(now, |node| {
                    self.rect_of_layout_node(node).map(|rect| {
                        // A global listener hears its node's size as placed (`LayoutNode.width`).
                        let size = self.node(node).layers[0].size;
                        IntRect::from_offset_size(rect.top_left(), size)
                    })
                });
            }
            manager.callbacks.trigger_debounced(now);
            self.schedule_debounced(manager, now);
            manager.callbacks.take_fired()
        };
        // All gone: back to no cost.
        if fired.is_empty() && rects.borrow().callbacks.is_empty() {
            self.rects = None;
        }
        for fired in fired {
            if let Some(bounds) = self.bounds_of(&fired) {
                let callback = fired.callback;
                self.layout_callbacks.push(Action::Run(Box::new(move || callback(bounds))));
            }
        }
    }

    /// Schedules a dispatch at the next debounce's deadline (16 ms from now at the earliest), as
    /// `scheduleDebounceCallback`: an executor task, so a test's clock runs it.
    fn schedule_debounced(&self, manager: &mut RectManager, now: i64) {
        let Some(deadline) = manager.callbacks.min_debounce_deadline else {
            if let Some((_, job)) = manager.scheduled.take() {
                job.cancel();
            }
            return;
        };
        let deadline = deadline.max(now + NEXT_FRAME_ISH);
        if manager.scheduled.as_ref().is_some_and(|(at, job)| *at == deadline && job.is_active()) {
            return;
        }
        if let Some((_, job)) = manager.scheduled.take() {
            job.cancel();
        }
        let key = self.key;
        let rects = Rc::downgrade(self.rects.as_ref().expect("a manager"));
        let job = compose_runtime::platform::launch(async move {
            compose_runtime::delay(Duration::from_millis((deadline - now) as u64)).await;
            if rects.upgrade().is_none() {
                return;
            }
            let mut actions = Vec::new();
            let dispatched = crate::node_context::with_free_tree(key, &mut |tree| {
                tree.dispatch_rects(false);
                actions = std::mem::take(&mut tree.layout_callbacks);
            });
            if dispatched {
                crate::input::run_actions(actions);
            } else if let Some(requests) = crate::node_context::requests_of(key) {
                // Busy: the next frame's layout dispatches.
                requests.wake();
            }
        });
        manager.scheduled = Some((deadline, job));
    }

    /// The rect the layout-rect callbacks compute for `node` (`None` if it's not placed): what
    /// Compose's tests read in its rect list. For tests.
    #[doc(hidden)]
    pub fn layout_rect_for_test(&self, node: NodeId) -> Option<IntRect> {
        self.rect_of_layout_node(node)
    }

    /// How many of `node` and the layout nodes under it have a rect (are placed). For tests.
    #[doc(hidden)]
    pub fn placed_rects_under_for_test(&self, node: NodeId) -> usize {
        let mut count = 0;
        let mut stack = vec![node];
        while let Some(node) = stack.pop() {
            if self.rect_of_layout_node(node).is_some() {
                count += 1;
            }
            stack.extend(
                self.get(node)
                    .into_iter()
                    .flat_map(|value| value.children.iter().map(|it| it.node)),
            );
        }
        count
    }

    /// Where every layout node of the tree is in its parent and at what size, whether it's
    /// placed, hashed: what the global listeners hear a change of.
    fn tree_signature(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |part: u64| hash = (hash ^ part).wrapping_mul(0x100_0000_01b3);
        let mut stack = vec![self.root_id()];
        stack.extend(self.overlays.iter().copied());
        while let Some(node) = stack.pop() {
            let Some(value) = self.get(node) else { continue };
            mix(node.to_bits());
            mix(value.placed as u64);
            for layer in &value.layers {
                mix((layer.offset.x as u32 as u64) << 32 | layer.offset.y as u32 as u64);
                mix((layer.measured.width as u32 as u64) << 32
                    | layer.measured.height as u32 as u64);
            }
            stack.extend(value.children.iter().map(|child| child.node));
        }
        hash
    }

    /// How many ancestors `node` has (0 if it's gone).
    fn depth_of(&self, node: NodeId) -> usize {
        let mut depth = 0;
        let mut current = self.get(node).and_then(|value| value.parent);
        while let Some(parent) = current {
            depth += 1;
            current = self.get(parent).and_then(|value| value.parent);
        }
        depth
    }

    /// The rect of `node`'s outer layer in the window, as Compose's rect list keeps it: its
    /// measured size at its origin, mapped through the offsets and graphics layers around it (not
    /// its own), its bounding box truncated. `None` if it's gone or not placed.
    pub(crate) fn rect_of_layout_node(&self, node: NodeId) -> Option<IntRect> {
        let value = self.get(node)?;
        if value.deactivated || value.layers.is_empty() || !self.is_placed_in_root(node) {
            return None;
        }
        let size = value.layers[0].measured;
        let rect = self.rect_in_root(node, 0, Some(0), size);
        Some(IntRect::new(rect.left as i32, rect.top as i32, rect.right as i32, rect.bottom as i32))
    }

    /// Where `node`'s inner layers are and their sizes, hashed: what moves its callbacks inside
    /// its layout modifiers without moving it.
    fn inner_layers_signature(&self, node: NodeId) -> u64 {
        let Some(value) = self.get(node) else { return 0 };
        value.layers.iter().skip(1).fold(0xcbf2_9ce4_8422_2325, |hash: u64, layer| {
            let parts =
                [layer.offset.x, layer.offset.y, layer.measured.width, layer.measured.height];
            parts
                .iter()
                .fold(hash, |hash, part| (hash ^ *part as u32 as u64).wrapping_mul(0x100_0000_01b3))
        })
    }

    /// The bounds a fired entry tells, as `rectInfoFor`: its node's rect, moved to the modifier
    /// node's place in its layout node (through the layers and graphics layers before it there),
    /// at its layer's size; in its root. `None` if its node isn't placed.
    fn bounds_of(&self, fired: &Fired) -> Option<RelativeLayoutBounds> {
        let value = self.get(fired.node)?;
        if value.deactivated || !self.is_placed_in_root(fired.node) {
            return None;
        }
        let mut rect = fired.rect;
        if let Some(index) = value.chain.iter().position(|entry| entry.id == fired.owner) {
            let layer = value.layer_of(index);
            let transformed = layer != 0
                || value.chain[..index]
                    .iter()
                    .any(|entry| entry.node.downcast_ref::<GraphicsLayerNode>().is_some());
            if transformed {
                let top_left = Offset::new(rect.left as f32, rect.top as f32);
                let moved = self.map_within_node(fired.node, layer, index, top_left);
                let at = IntOffset::new(moved.x.round() as i32, moved.y.round() as i32);
                rect = IntRect::from_offset_size(at, value.layers[layer].measured);
            }
        }
        let popup = self.popup_origin(fired.node);
        let popup = IntOffset::new(popup.x.round() as i32, popup.y.round() as i32);
        let origin = self.window_origin;
        let window_offset = IntOffset::new(origin.x.round() as i32, origin.y.round() as i32);
        Some(RelativeLayoutBounds {
            rect: rect.translate(IntOffset::new(-popup.x, -popup.y)),
            window_offset,
            screen_offset: window_offset + popup,
            window_size: self.window,
            tree: self.key,
            node: fired.node,
        })
    }

    /// `point`, in layer `layer` of `node` seen by its chain entry `before_entry`, in the node's
    /// outer layer: through the layers' offsets and the graphics layers before the entry, as
    /// `localPositionOf` from that place to the outer coordinator.
    fn map_within_node(
        &self,
        node: NodeId,
        layer: usize,
        before_entry: usize,
        point: Offset,
    ) -> Offset {
        let value = self.node(node);
        let mut point = point + layer_origin(value, layer);
        // A layer placed with one after the chain's in it (D1909); not layer 0's, which the
        // outer layer is in.
        let placed = |index: usize, point: Offset| match value.placed_layer_at(index, layer) {
            Some((graphics, at)) if at > 0 => {
                let map =
                    graphics.layer_map(layer_origin(value, at), value.layers[at].measured.into());
                graphics.note_transform(&map, self.position_listeners);
                map.map(point)
            }
            _ => point,
        };
        point = placed(before_entry, point);
        for index in (0..before_entry).rev() {
            if let Some(graphics) = value.chain[index].node.downcast_ref::<GraphicsLayerNode>() {
                let layer = value.layer_of(index);
                let map = graphics
                    .layer_map(layer_origin(value, layer), value.layers[layer].measured.into());
                graphics.note_transform(&map, self.position_listeners);
                point = map.map(point);
            }
            point = placed(index, point);
        }
        point
    }

    /// The registered nodes map through the graphics layers around them again, so those layers
    /// tell their moves (after `position_listeners` moved).
    pub(crate) fn note_rect_layers(&self) {
        let Some(rects) = &self.rects else { return };
        for tracked in &rects.borrow().tracked {
            if self.rect_of_layout_node(tracked.node).is_some() {
                let value = self.node(tracked.node);
                let last = value.chain.len();
                let layer = value.layers.len() - 1;
                self.map_within_node(tracked.node, layer, last, Offset::ZERO);
            }
        }
    }

    /// The outer rects (in its root) of the placed layout nodes that may draw over `target`, as
    /// `calculateOcclusions` (see `RelativeLayoutBounds::calculate_occlusions`).
    pub(crate) fn occlusions_of(&self, target: NodeId) -> Vec<IntRect> {
        let Some(bounds) = self.rect_of_layout_node(target) else { return Vec::new() };
        let mut top = target;
        while let Some(parent) =
            self.get(top).and_then(|value| if value.overlay { None } else { value.parent })
        {
            top = parent;
        }
        let popup = self.popup_origin(target);
        let popup = IntOffset::new(popup.x.round() as i32, popup.y.round() as i32);
        let mut occlusions = Vec::new();
        let mut stack = vec![top];
        while let Some(node) = stack.pop() {
            let Some(rect) = self.rect_of_layout_node(node) else { continue };
            let intersects = rect.left <= bounds.right
                && bounds.left <= rect.right
                && rect.top <= bounds.bottom
                && bounds.top <= rect.bottom;
            if node != target && intersects && self.is_drawn_first(target, node) {
                occlusions.push(rect.translate(IntOffset::new(-popup.x, -popup.y)));
            }
            let children = &self.node(node).children;
            stack.extend(children.iter().rev().map(|child| child.node));
        }
        occlusions
    }

    /// Whether `target` draws before `other`, neither being the other's ancestor, as
    /// `RectManager.isTargetDrawnFirst`: their branching ancestors' z-index, then place order.
    fn is_drawn_first(&self, target: NodeId, other: NodeId) -> bool {
        let path = |node: NodeId| {
            let mut path = vec![node];
            let mut current = node;
            while let Some(parent) = self.get(current).and_then(|value| value.parent) {
                path.push(parent);
                current = parent;
            }
            path.reverse();
            path
        };
        let (a, b) = (path(target), path(other));
        // The root never is.
        if a.len() == 1 || b.len() == 1 {
            return false;
        }
        let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        if common == 0 || common == a.len() || common == b.len() {
            return false;
        }
        let (a, b) = (self.node(a[common]), self.node(b[common]));
        if a.z_index == b.z_index { a.place_order < b.place_order } else { a.z_index < b.z_index }
    }
}
