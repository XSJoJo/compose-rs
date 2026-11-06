//! Throttling and debouncing of the layout-rect callbacks, as Compose's `ThrottledCallbacks`.
//!
//! Times are milliseconds of the executor's clock (`RectManager::now_millis`). The rules are
//! Compose's: a callback with neither a throttle nor a debounce is called at each change; with a
//! debounce alone, once its node rested that long; with both, at once if it wasn't called for the
//! throttle's time, else once its node rested for the debounce's. A debounce of 0 with a throttle
//! takes the throttle's value, so the callback hears where the node settled.

use compose_runtime::Callback;

use crate::node_chain::EntryId;
use crate::spatial::RelativeLayoutBounds;
use crate::tree::NodeId;
use crate::unit::IntRect;

/// A registered callback, as Compose's `ThrottledCallbacks.Entry`.
pub(crate) struct Entry {
    pub(crate) id: u64,
    /// Its layout node.
    pub(crate) node: NodeId,
    /// The chain entry of the modifier node that registered it: where in the node it reports.
    pub(crate) owner: EntryId,
    throttle: i64,
    debounce: i64,
    pub(crate) callback: Callback<dyn Fn(RelativeLayoutBounds)>,
    /// The node's rect when last told, in the window.
    pub(crate) rect: IntRect,
    last_invoke: Option<i64>,
    last_uninvoked_fire: Option<i64>,
    /// The dispatch it was last called in.
    invoked_in: Option<u64>,
}

/// An entry to call: what `ThrottledCallbacks` decided, for its owner to resolve.
pub(crate) struct Fired {
    pub(crate) node: NodeId,
    pub(crate) owner: EntryId,
    pub(crate) rect: IntRect,
    pub(crate) callback: Callback<dyn Fn(RelativeLayoutBounds)>,
}

#[derive(Default)]
pub(crate) struct ThrottledCallbacks {
    /// The `onLayoutRectChanged` entries, in registration order.
    rect_changed: Vec<Entry>,
    /// The global layout listeners, the newest first (Compose's linked list).
    global: Vec<Entry>,
    /// When the next debounced call is due, if any.
    pub(crate) min_debounce_deadline: Option<i64>,
    next_id: u64,
    /// The dispatch under way (`begin_dispatch`).
    dispatch: u64,
    fired: Vec<Fired>,
}

impl ThrottledCallbacks {
    fn entry(
        &mut self,
        node: NodeId,
        owner: EntryId,
        throttle: i64,
        debounce: i64,
        callback: Callback<dyn Fn(RelativeLayoutBounds)>,
    ) -> Entry {
        self.next_id += 1;
        Entry {
            id: self.next_id,
            node,
            owner,
            throttle,
            // A debounce of 0 takes the throttle's value: the callback hears where it settled.
            debounce: if debounce == 0 { throttle } else { debounce },
            callback,
            rect: IntRect::ZERO,
            last_invoke: None,
            last_uninvoked_fire: None,
            invoked_in: None,
        }
    }

    /// Registers a callback for `node`'s rect, as `registerOnRectChanged`. Returns its id.
    pub(crate) fn register_on_rect_changed(
        &mut self,
        node: NodeId,
        owner: EntryId,
        throttle: i64,
        debounce: i64,
        callback: Callback<dyn Fn(RelativeLayoutBounds)>,
    ) -> u64 {
        let entry = self.entry(node, owner, throttle, debounce, callback);
        let id = entry.id;
        self.rect_changed.push(entry);
        id
    }

    /// Registers a callback for any change, as `registerOnGlobalChange`. Returns its id.
    pub(crate) fn register_on_global_change(
        &mut self,
        node: NodeId,
        owner: EntryId,
        throttle: i64,
        debounce: i64,
        callback: Callback<dyn Fn(RelativeLayoutBounds)>,
    ) -> u64 {
        let entry = self.entry(node, owner, throttle, debounce, callback);
        let id = entry.id;
        self.global.insert(0, entry);
        id
    }

    /// Forgets entry `id`. Returns the layout node it was a rect callback of.
    pub(crate) fn unregister(&mut self, id: u64) -> Option<NodeId> {
        if let Some(at) = self.rect_changed.iter().position(|entry| entry.id == id) {
            return Some(self.rect_changed.remove(at).node);
        }
        if let Some(at) = self.global.iter().position(|entry| entry.id == id) {
            self.global.remove(at);
        }
        None
    }

    /// Whether `node` has rect callbacks.
    pub(crate) fn has_rect_callbacks(&self, node: NodeId) -> bool {
        self.rect_changed.iter().any(|entry| entry.node == node)
    }

    /// Whether there are global entries.
    pub(crate) fn has_global(&self) -> bool {
        !self.global.is_empty()
    }

    /// Whether there's no entry at all.
    pub(crate) fn is_empty(&self) -> bool {
        self.rect_changed.is_empty() && self.global.is_empty()
    }

    /// `node`'s rect changed (or its callbacks are new), as `fireOnUpdatedRect`: throttled
    /// entries may be called, debounced ones get a deadline.
    pub(crate) fn fire_on_updated_rect(&mut self, node: NodeId, rect: IntRect, now: i64) {
        for index in 0..self.rect_changed.len() {
            if self.rect_changed[index].node == node {
                let entry = &mut self.rect_changed[index];
                entry.rect = rect;
                let past_throttle =
                    entry.last_invoke.is_none_or(|last| now - last >= entry.throttle);
                // Neither (called at once), or both (called at once past the throttle); a
                // debounce alone waits for it.
                let can_invoke = entry.debounce == 0 || entry.throttle != 0;
                if past_throttle && can_invoke {
                    entry.last_uninvoked_fire = None;
                    entry.last_invoke = Some(now);
                    Self::invoke(&mut self.fired, entry, self.dispatch);
                } else if entry.debounce != 0 {
                    entry.last_uninvoked_fire = Some(now);
                }
            }
        }
    }

    /// The window moved or resized: every rect entry hears it, as `fireOnRectChangedEntries`.
    pub(crate) fn fire_on_rect_changed_entries(&mut self, now: i64) {
        for entry in &mut self.rect_changed {
            Self::fire(&mut self.fired, entry, now, self.dispatch);
        }
    }

    /// Something changed: every global entry hears it with its node's rect now (`rect_of`, `None`
    /// if it's not placed), as `fireGlobalChangeEntries`.
    pub(crate) fn fire_global_change_entries(
        &mut self,
        now: i64,
        mut rect_of: impl FnMut(NodeId) -> Option<IntRect>,
    ) {
        for entry in &mut self.global {
            if let Some(rect) = rect_of(entry.node) {
                entry.rect = rect;
            }
            Self::fire(&mut self.fired, entry, now, self.dispatch);
        }
    }

    /// Calls the debounced entries whose node rested long enough, as `triggerDebounced`, and
    /// keeps when the next is due.
    pub(crate) fn trigger_debounced(&mut self, now: i64) {
        if self.min_debounce_deadline.is_some_and(|deadline| deadline > now) {
            return;
        }
        let mut min_deadline = None;
        for entries in [&mut self.rect_changed, &mut self.global] {
            for entry in entries.iter_mut() {
                let Some(fired_at) = entry.last_uninvoked_fire else { continue };
                if entry.debounce <= 0 {
                    continue;
                }
                if now - fired_at >= entry.debounce {
                    entry.last_invoke = Some(now);
                    entry.last_uninvoked_fire = None;
                    Self::invoke(&mut self.fired, entry, self.dispatch);
                } else {
                    let deadline = fired_at + entry.debounce;
                    min_deadline =
                        Some(min_deadline.map_or(deadline, |min: i64| min.min(deadline)));
                }
            }
        }
        self.min_debounce_deadline = min_deadline;
    }

    /// A dispatch starts: an entry called in it isn't called again in it.
    pub(crate) fn begin_dispatch(&mut self) {
        self.dispatch += 1;
    }

    /// The entries to call since the last take, in the order they were decided.
    pub(crate) fn take_fired(&mut self) -> Vec<Fired> {
        std::mem::take(&mut self.fired)
    }

    /// As Compose's private `fire`: called at once if past its throttle and not debounced, else
    /// left to its debounce. Never twice in one dispatch: a rect change and a window move in one
    /// are one call, as Compose's strict `>` makes them. Unlike Compose's, a later dispatch in
    /// the same millisecond calls it (D1373).
    fn fire(fired: &mut Vec<Fired>, entry: &mut Entry, now: i64, dispatch: u64) {
        let throttle_okay = entry.invoked_in != Some(dispatch)
            && entry.last_invoke.is_none_or(|last| now - last >= entry.throttle);
        let debounce_okay = entry.debounce == 0;
        entry.last_uninvoked_fire = Some(now);
        if throttle_okay && debounce_okay {
            entry.last_invoke = Some(now);
            Self::invoke(fired, entry, dispatch);
        }
    }

    fn invoke(fired: &mut Vec<Fired>, entry: &mut Entry, dispatch: u64) {
        entry.invoked_in = Some(dispatch);
        fired.push(Fired {
            node: entry.node,
            owner: entry.owner,
            rect: entry.rect,
            callback: entry.callback.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use compose_runtime::IntoCallback;

    use super::*;

    /// Compose's test fixture: callbacks registered on numbered nodes, fired on explicit times
    /// (starting at 1), the fired entries called as the fixture's `fire` does.
    struct Fixture {
        callbacks: ThrottledCallbacks,
        now: i64,
    }

    fn test(block: impl FnOnce(&mut Fixture)) {
        block(&mut Fixture { callbacks: ThrottledCallbacks::default(), now: 1 });
    }

    fn node(id: u64) -> NodeId {
        NodeId::from_bits(id)
    }

    fn counter() -> Rc<Cell<u32>> {
        Rc::default()
    }

    impl Fixture {
        fn register(
            &mut self,
            id: u64,
            throttle: i64,
            debounce: i64,
            count: &Rc<Cell<u32>>,
        ) -> u64 {
            let count = count.clone();
            let callback =
                (move |_: RelativeLayoutBounds| count.set(count.get() + 1)).into_callback();
            self.callbacks.register_on_rect_changed(
                node(id),
                EntryId(id),
                throttle,
                debounce,
                callback,
            )
        }

        fn run_fired(&mut self) {
            for fired in self.callbacks.take_fired() {
                (fired.callback)(RelativeLayoutBounds::for_test(fired.rect));
            }
        }

        fn fire(&mut self, id: u64) {
            self.callbacks.fire_on_updated_rect(node(id), IntRect::ZERO, self.now);
            self.run_fired();
        }

        fn advance_by(&mut self, ms: i64) {
            self.now += ms;
            self.callbacks.trigger_debounced(self.now);
            self.run_fired();
        }
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testImmediateFire
    #[test]
    fn neither_throttled_nor_debounced_is_called_at_once() {
        test(|f| {
            let called = counter();
            f.register(1, 0, 0, &called);
            f.fire(1);
            assert_eq!(called.get(), 1);
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testImmediateFireMultipleCallbacks
    #[test]
    fn each_callback_of_a_node_is_called() {
        test(|f| {
            let (a, b) = (counter(), counter());
            f.register(1, 0, 0, &a);
            f.register(1, 0, 0, &b);
            f.fire(1);
            assert_eq!((a.get(), b.get()), (1, 1));
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testImmediateFireMultipleCallbacksRemoval
    #[test]
    fn an_unregistered_callback_is_no_longer_called_the_others_are() {
        test(|f| {
            let (a, b, c) = (counter(), counter(), counter());
            let ha = f.register(1, 0, 0, &a);
            let hb = f.register(1, 0, 0, &b);
            let hc = f.register(1, 0, 0, &c);
            f.fire(1);
            assert_eq!((a.get(), b.get(), c.get()), (1, 1, 1));
            assert_eq!(f.callbacks.unregister(hb), Some(node(1)));
            f.fire(1);
            assert_eq!((a.get(), b.get(), c.get()), (2, 1, 2));
            f.callbacks.unregister(ha);
            f.fire(1);
            assert_eq!((a.get(), b.get(), c.get()), (2, 1, 3));
            f.callbacks.unregister(hc);
            f.fire(1);
            assert_eq!((a.get(), b.get(), c.get()), (2, 1, 3));
            assert!(f.callbacks.is_empty());
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testImmediateFireOnlyTargetsId
    #[test]
    fn a_change_calls_only_its_node_s_callbacks() {
        test(|f| {
            let (a, b) = (counter(), counter());
            f.register(1, 0, 0, &a);
            f.register(2, 0, 0, &b);
            f.fire(1);
            assert_eq!((a.get(), b.get()), (1, 0));
            f.advance_by(1);
            f.fire(2);
            assert_eq!((a.get(), b.get()), (1, 1));
            f.advance_by(1);
            f.fire(2);
            assert_eq!((a.get(), b.get()), (1, 2));
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testDebounceSingleFire
    #[test]
    fn a_debounced_callback_is_called_once_the_node_rested() {
        test(|f| {
            let called = counter();
            f.register(1, 0, 10, &called);
            f.fire(1);
            assert_eq!(called.get(), 0);
            f.advance_by(8);
            assert_eq!(called.get(), 0);
            f.advance_by(8);
            assert_eq!(called.get(), 1);
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testDebounceManyFire
    #[test]
    fn each_change_puts_the_debounced_call_off() {
        test(|f| {
            let called = counter();
            f.register(1, 0, 10, &called);
            f.fire(1);
            assert_eq!(called.get(), 0);
            f.advance_by(8);
            assert_eq!(called.get(), 0);
            f.fire(1);
            assert_eq!(called.get(), 0);
            f.advance_by(8);
            assert_eq!(called.get(), 0);
            f.fire(1);
            assert_eq!(called.get(), 0);
            f.advance_by(8);
            assert_eq!(called.get(), 0);
            f.advance_by(8);
            assert_eq!(called.get(), 1);
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testThrottleSingleFire
    #[test]
    fn a_throttled_callback_is_called_at_most_once_a_throttle() {
        test(|f| {
            let called = counter();
            f.register(1, 10, 0, &called);
            f.fire(1);
            assert_eq!(called.get(), 1);
            f.advance_by(8);
            assert_eq!(called.get(), 1);
            f.fire(1);
            assert_eq!(called.get(), 1);
            f.advance_by(8);
            assert_eq!(called.get(), 1);
            f.fire(1);
            assert_eq!(called.get(), 2);
            f.advance_by(8);
            assert_eq!(called.get(), 2);
            f.fire(1);
            assert_eq!(called.get(), 2);
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testThrottleRapidFireCallsOncePerDuration
    #[test]
    fn rapid_changes_call_a_throttled_callback_once_a_throttle_then_where_it_settled() {
        test(|f| {
            let called = counter();
            f.register(1, 10, 0, &called);
            for _ in 0..10 {
                f.fire(1);
                f.advance_by(1);
                assert_eq!(called.get(), 1);
            }
            // Past the throttle: the second call.
            f.fire(1);
            f.advance_by(1);
            assert_eq!(called.get(), 2);
            // Not now...
            f.fire(1);
            f.advance_by(1);
            assert_eq!(called.get(), 2);
            // ...but where it settled, once rested for the throttle's time.
            f.advance_by(12);
            assert_eq!(called.get(), 3);
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testThrottleAndDebounceSlowFiring
    #[test]
    fn slow_changes_call_a_throttled_and_debounced_callback_at_once() {
        test(|f| {
            let called = counter();
            f.register(1, 20, 10, &called);
            f.fire(1);
            assert_eq!(called.get(), 1);
            f.advance_by(30);
            assert_eq!(called.get(), 1);
            f.fire(1);
            assert_eq!(called.get(), 2);
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testThrottleAndDebounceRapidFiring
    #[test]
    fn a_change_within_the_throttle_is_called_once_debounced() {
        test(|f| {
            let called = counter();
            f.register(1, 20, 10, &called);
            f.fire(1);
            assert_eq!(called.get(), 1);
            f.advance_by(16);
            assert_eq!(called.get(), 1);
            f.fire(1);
            assert_eq!(called.get(), 1);
            f.advance_by(12);
            // The debounce's deadline passed: the last position isn't lost.
            assert_eq!(called.get(), 2);
            f.advance_by(30);
            assert_eq!(called.get(), 2);
            f.fire(1);
            assert_eq!(called.get(), 3);
        });
    }

    // Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/spatial/ThrottledCallbacksTest.kt, testThrottleAndDebounceWithLargerDebounce
    #[test]
    fn a_larger_debounce_calls_once_the_node_rested_that_long() {
        test(|f| {
            let called = counter();
            f.register(1, 20, 400, &called);
            f.fire(1);
            assert_eq!(called.get(), 1);
            f.advance_by(16);
            assert_eq!(called.get(), 1);
            f.fire(1);
            assert_eq!(called.get(), 1);
            f.advance_by(16);
            assert_eq!(called.get(), 1);
            f.fire(1);
            assert_eq!(called.get(), 2);
            f.advance_by(10);
            f.fire(1);
            assert_eq!(called.get(), 2);
            f.advance_by(100);
            assert_eq!(called.get(), 2);
            f.advance_by(500);
            assert_eq!(called.get(), 3);
        });
    }

    /// A rect change and a window move in one dispatch call a callback once, as Compose's
    /// `dispatchCallbacks` (its `fire` strictly past the throttle); in the next, the move calls it.
    // Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/spatial/RectManager.kt, dispatchCallbacks
    #[test]
    fn a_rect_change_and_a_window_move_in_one_dispatch_call_once() {
        test(|f| {
            let called = counter();
            f.register(1, 0, 0, &called);
            f.callbacks.begin_dispatch();
            f.fire(1);
            f.callbacks.fire_on_rect_changed_entries(f.now);
            f.run_fired();
            assert_eq!(called.get(), 1);
            f.callbacks.begin_dispatch();
            f.callbacks.fire_on_rect_changed_entries(f.now);
            f.run_fired();
            assert_eq!(called.get(), 2);
        });
    }

    /// The deadline the owner schedules its next dispatch at.
    #[test]
    fn the_next_debounce_deadline_is_kept() {
        test(|f| {
            let called = counter();
            f.register(1, 0, 10, &called);
            f.register(2, 0, 30, &called);
            f.fire(1);
            f.fire(2);
            f.advance_by(0);
            assert_eq!(f.callbacks.min_debounce_deadline, Some(11));
            f.advance_by(10);
            assert_eq!(f.callbacks.min_debounce_deadline, Some(31));
            f.advance_by(20);
            assert_eq!(f.callbacks.min_debounce_deadline, None);
            assert_eq!(called.get(), 2);
        });
    }
}
