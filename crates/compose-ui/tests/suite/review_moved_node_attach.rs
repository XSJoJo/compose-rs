//! Review tests for D1923: nodes movable content moves are detached and attached, as Compose's
//! `LayoutNode.removeAt` and `insertAt`. Edge cases of the branch `moved-node-attach`.

#![allow(non_snake_case)]

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use compose_runtime::*;
use compose_ui::platform::{NodeId, UiHost};
use compose_ui::*;
use support::*;

type Log = Rc<RefCell<Vec<String>>>;

fn take(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

fn sorted(mut heard: Vec<String>) -> Vec<String> {
    heard.sort();
    heard
}

/// As `ModifierNodeAttachOrderTest`'s `LoggerElement`.
#[derive(Clone)]
struct Logger {
    log: Log,
    name: &'static str,
}

struct LoggerNode {
    log: Log,
    name: &'static str,
}

impl ModifierNodeElement for Logger {
    type Node = LoggerNode;
    fn create(&self) -> LoggerNode {
        LoggerNode { log: self.log.clone(), name: self.name }
    }
    fn update(&self, node: &mut LoggerNode) {
        node.name = self.name;
    }
    fn same(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl ModifierNode for LoggerNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        assert!(context.is_attached());
        self.log.borrow_mut().push(format!("attach({})", self.name));
    }
    fn on_detach(&mut self, context: &mut NodeContext<'_>) {
        assert!(context.is_attached());
        self.log.borrow_mut().push(format!("detach({})", self.name));
    }
    fn on_reset(&mut self, _context: &mut NodeContext<'_>) {
        self.log.borrow_mut().push(format!("reset({})", self.name));
    }
}

fn logger(log: &Log, name: &'static str) -> Modifier {
    Modifier.then_custom(Logger { log: log.clone(), name })
}

// ---- Traversals from a moved node's callbacks ---------------------------------------------------

static MARK: TraverseKey = TraverseKey::new("review mark");

/// A traversable node naming the layout it's on.
#[derive(Clone, Copy, PartialEq)]
struct Mark(&'static str);

struct MarkNode(&'static str);

impl ModifierNodeElement for Mark {
    type Node = MarkNode;
    fn create(&self) -> MarkNode {
        MarkNode(self.0)
    }
    fn update(&self, node: &mut MarkNode) {
        node.0 = self.0;
    }
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl ModifierNode for MarkNode {
    fn as_traversable(&mut self) -> Option<&mut dyn TraversableNode> {
        Some(self)
    }
}

impl TraversableNode for MarkNode {
    fn traverse_key(&self) -> &'static TraverseKey {
        &MARK
    }
}

fn mark(name: &'static str) -> Modifier {
    Modifier.then_custom(Mark(name))
}

/// Logs the nearest `Mark` above it in its `on_attach` and `on_detach`.
#[derive(Clone)]
struct Probe(Log);

struct ProbeNode(Log);

impl ModifierNodeElement for Probe {
    type Node = ProbeNode;
    fn create(&self) -> ProbeNode {
        ProbeNode(self.0.clone())
    }
    fn update(&self, node: &mut ProbeNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, _: &Self) -> bool {
        true
    }
}

fn nearest_mark(context: &mut NodeContext<'_>) -> &'static str {
    context
        .find_nearest_ancestor_with_key(&MARK)
        .map(|node| node.downcast_ref::<MarkNode>().map_or("?", |mark| mark.0))
        .unwrap_or("none")
}

impl ModifierNode for ProbeNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        let found = nearest_mark(context);
        self.0.borrow_mut().push(format!("attach under {found}"));
    }
    fn on_detach(&mut self, context: &mut NodeContext<'_>) {
        let found = nearest_mark(context);
        self.0.borrow_mut().push(format!("detach under {found}"));
    }
}

fn probe(log: &Log) -> Modifier {
    Modifier.then_custom(Probe(log.clone()))
}

/// `content` in the first of two layouts that stay (marked "one" and "two"), or the second when
/// `moved`.
#[composable]
fn Between(moved: bool, content: impl Fn() + 'static) {
    let here = content.clone();
    Stack!(modifier = mark("one"), {
        if !moved {
            here();
        }
    });
    let there = content.clone();
    Stack!(modifier = mark("two"), {
        if moved {
            there();
        }
    });
}

/// A moved node's `on_detach` sees the layout it left above it, its `on_attach` the one it went
/// to; also for a node deeper in the moved subtree (its own parent is in the subtree, which is
/// in the new layout: the walk must switch to the old parent at the subtree's root).
#[test]
fn a_moved_subtree_s_callbacks_see_the_parent_it_left_then_the_one_it_went_to() {
    for deep in [false, true] {
        let log = Log::default();
        let moved = mutable_state_of(false);
        let (there, content_log) = (moved.handle(), log.clone());
        let content = movable_content_of(move || {
            if deep {
                let probe = probe(&content_log);
                Stack!({
                    Leaf(probe.clone());
                });
            } else {
                Leaf(probe(&content_log));
            }
        });
        let mut host = host(move || {
            let (content, moved) = (content.clone(), there.get());
            Stack(Modifier, callback(move || Between(moved, content.clone())));
        });
        assert_eq!(take(&log), ["attach under one"]);
        moved.set(true);
        host.frame();
        assert_eq!(take(&log), ["detach under one", "attach under two"], "deep: {deep}");
        moved.set(false);
        host.frame();
        assert_eq!(take(&log), ["detach under two", "attach under one"], "deep: {deep}");
    }
}

/// `content` in a layout marked "old", or when `moved` in one marked "new" that the pass inserts
/// before it (attached before the content is taken out of the old one).
#[composable]
fn NewBefore(moved: bool, content: impl Fn() + 'static) {
    if moved {
        Stack(mark("new"), content.clone());
    }
    let old = content.clone();
    Stack!(modifier = mark("old"), {
        if !moved {
            old();
        }
    });
}

/// The same when the layout it goes to is inserted by the pass (detached in its `Attach` step).
#[test]
fn a_node_moved_into_a_new_layout_sees_the_old_parent_in_on_detach() {
    let log = Log::default();
    let moved = mutable_state_of(false);
    let (there, content_log) = (moved.handle(), log.clone());
    let content = movable_content_of(move || Leaf(probe(&content_log)));
    let mut host = host(move || {
        let (content, moved) = (content.clone(), there.get());
        Stack(Modifier, callback(move || NewBefore(moved, content.clone())));
    });
    assert_eq!(take(&log), ["attach under old"]);
    moved.set(true);
    host.frame();
    assert_eq!(take(&log), ["detach under old", "attach under new"]);
}

/// Logs where it is in the root in its `on_detach`.
#[derive(Clone)]
struct WhereOnDetach(Log);

struct WhereOnDetachNode(Log);

impl ModifierNodeElement for WhereOnDetach {
    type Node = WhereOnDetachNode;
    fn create(&self) -> WhereOnDetachNode {
        WhereOnDetachNode(self.0.clone())
    }
    fn update(&self, node: &mut WhereOnDetachNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, _: &Self) -> bool {
        true
    }
}

impl ModifierNode for WhereOnDetachNode {
    fn on_detach(&mut self, context: &mut NodeContext<'_>) {
        let at = context.require_layout_coordinates().position_in_root();
        self.0.borrow_mut().push(format!("detach at {} {}", at.x, at.y));
    }
}

/// `content` in the first of two layouts that stay, at (30, 20), or the second, at the origin,
/// when `moved`.
#[composable]
fn BetweenPlaces(moved: bool, content: impl Fn() + 'static) {
    let here = content.clone();
    Stack!(modifier = Modifier.offset(30.dp(), 20.dp()), {
        if !moved {
            here();
        }
    });
    let there = content.clone();
    Stack!({
        if moved {
            there();
        }
    });
}

/// A moved node's `on_detach` reads where it was: its coordinates go up through the parent it
/// left, as Compose's `removeAt` detaches it while it's still there (D1924). Through the parent
/// it went to, it would read where it lands, and a shared element would start from there.
#[test]
fn a_moved_node_s_coordinates_in_on_detach_are_where_it_was() {
    for deep in [false, true] {
        let log = Log::default();
        let moved = mutable_state_of(false);
        let (there, content_log) = (moved.handle(), log.clone());
        let content = movable_content_of(move || {
            let reads = Modifier.then_custom(WhereOnDetach(content_log.clone()));
            if deep {
                Stack!({
                    Leaf(reads.clone());
                });
            } else {
                Leaf(reads);
            }
        });
        let mut host = host(move || {
            let (content, moved) = (content.clone(), there.get());
            Stack(Modifier, callback(move || BetweenPlaces(moved, content.clone())));
        });
        moved.set(true);
        host.frame();
        assert_eq!(take(&log), ["detach at 30 20"], "deep: {deep}");
        moved.set(false);
        host.frame();
        assert_eq!(take(&log), ["detach at 0 0"], "deep: {deep}");
    }
}

// ---- Nested movable content ---------------------------------------------------------------------

/// `outer` in a row, or when `split` in a stack, `inner` in another stack after it.
#[composable]
fn Split(split: bool, outer: Callback<dyn Fn(bool)>, inner: Callback<dyn Fn()>) {
    if split {
        let outer = outer.clone();
        Stack!({
            outer(true);
        });
        let inner = inner.clone();
        Stack!({
            inner();
        });
    } else {
        let outer = outer.clone();
        Row!({
            outer(false);
        });
    }
}

/// Content moved out of other movable content that moves in the same pass: Compose detaches the
/// outer content with everything in it when it leaves (the inner content with it), then takes the
/// inner out of the detached outer (no lifecycle), then attaches each where it goes. Each is
/// detached once and attached once.
#[test]
fn content_moved_out_of_moving_content_is_detached_and_attached_once() {
    let log = Log::default();
    let split = mutable_state_of(false);
    let (there, inner_log, outer_log) = (split.handle(), log.clone(), log.clone());
    let inner = movable_content_of(move || Leaf(logger(&inner_log, "i")));
    let in_outer = inner.clone();
    let outer = movable_content_of_with(move |split: bool| {
        let inner = in_outer.clone();
        Stack!(modifier = logger(&outer_log, "o"), {
            if !split {
                inner();
            }
        });
    });
    let mut host = host(move || {
        let (inner, outer, split) = (inner.clone(), outer.clone(), there.get());
        Stack(Modifier, callback(move || Split(split, outer.clone(), inner.clone())));
    });
    assert_eq!(take(&log), ["attach(i)", "attach(o)"]);
    split.set(true);
    host.frame();
    assert_eq!(
        sorted(take(&log)),
        ["attach(i)", "attach(o)", "detach(i)", "detach(o)"],
        "each detached and attached once"
    );
}

/// `content` two layouts deep under one of `layout`'s modifier, or when `moved` in a new layout.
#[composable]
fn OutOfReleased(moved: bool, layout: Modifier, content: impl Fn() + 'static) {
    if !moved {
        let content = content.clone();
        Stack!(modifier = layout, {
            let content = content.clone();
            Stack!({
                content();
            });
        });
    } else {
        Stack(Modifier, content.clone());
    }
}

/// Content moved out of a layout the pass removes for good (the layout is released, the content
/// lives on elsewhere): the content is detached once, attached once; the layout detached once.
#[test]
fn content_moved_out_of_a_released_layout() {
    let log = Log::default();
    let moved = mutable_state_of(false);
    let (there, content_log, layout_log) = (moved.handle(), log.clone(), log.clone());
    let content = movable_content_of(move || Leaf(logger(&content_log, "c")));
    let mut host = host(move || {
        let (content, moved) = (content.clone(), there.get());
        let layout = logger(&layout_log, "l");
        Stack(Modifier, callback(move || OutOfReleased(moved, layout.clone(), content.clone())));
    });
    assert_eq!(take(&log), ["attach(c)", "attach(l)"]);
    moved.set(true);
    host.frame();
    let heard = take(&log);
    assert_eq!(sorted(heard.clone()), ["attach(c)", "detach(c)", "detach(l)"], "{heard:?}");
    // Compose takes the content out of the layout before it removes the layout (`recordDelete`
    // reports movable content first): the content is detached first.
    assert_eq!(heard, ["detach(c)", "detach(l)", "attach(c)"]);
}

/// Two contents swapped between two layouts that stay: each detached, then attached, once.
#[test]
fn two_contents_swapped() {
    let log = Log::default();
    let swapped = mutable_state_of(false);
    let (there, a_log, b_log) = (swapped.handle(), log.clone(), log.clone());
    let a = movable_content_of(move || Leaf(logger(&a_log, "a")));
    let b = movable_content_of(move || Leaf(logger(&b_log, "b")));
    let mut host = host(move || {
        let (a, b, swapped) = (a.clone(), b.clone(), there.get());
        Stack(
            Modifier,
            callback(move || {
                let (first, second) =
                    if swapped { (b.clone(), a.clone()) } else { (a.clone(), b.clone()) };
                Stack!({
                    first();
                });
                Stack!({
                    second();
                });
            }),
        );
    });
    assert_eq!(sorted(take(&log)), ["attach(a)", "attach(b)"]);
    swapped.set(true);
    host.frame();
    let heard = take(&log);
    assert_eq!(sorted(heard.clone()), ["attach(a)", "attach(b)", "detach(a)", "detach(b)"]);
    for name in ["a", "b"] {
        let detach = heard.iter().position(|it| *it == format!("detach({name})"));
        let attach = heard.iter().position(|it| *it == format!("attach({name})"));
        assert!(detach < attach, "{heard:?}");
    }
}

/// Keyed children reordered in their parent (`move_nodes`, Compose's `LayoutNode.move`): no
/// lifecycle, as Compose's.
#[test]
fn reordered_children_are_not_detached() {
    let log = Log::default();
    let reversed = mutable_state_of(false);
    let (there, content_log) = (reversed.handle(), log.clone());
    let mut host = host(move || {
        let reversed = there.get();
        let log = content_log.clone();
        Stack(
            Modifier,
            callback(move || {
                let names: [&'static str; 3] =
                    if reversed { ["z", "y", "x"] } else { ["x", "y", "z"] };
                for name in names {
                    let log = log.clone();
                    key(name, move || Leaf(logger(&log, name)));
                }
            }),
        );
    });
    // The leaves' names, in the order of the tree.
    let order = |host: &UiHost| {
        host.with_tree(|tree| {
            let root = tree.children_of(tree.root_id()).next().expect("the stack");
            let leaves: Vec<NodeId> = tree.children_of(root).collect();
            leaves
                .into_iter()
                .flat_map(|leaf| {
                    tree.modifier_nodes(leaf)
                        .filter_map(|node| node.downcast_ref::<LoggerNode>().map(|it| it.name))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(take(&log), ["attach(x)", "attach(y)", "attach(z)"]);
    assert_eq!(order(&host), ["x", "y", "z"]);
    reversed.set(true);
    host.frame();
    assert_eq!(order(&host), ["z", "y", "x"], "reordered");
    assert_eq!(take(&log), Vec::<String>::new());
}

/// Content moved, then removed in the next pass: detached once more, not twice.
#[test]
fn moved_then_removed_in_the_next_pass_is_detached_once() {
    let log = Log::default();
    let place = mutable_state_of(0);
    let (there, content_log) = (place.handle(), log.clone());
    let content = movable_content_of(move || Leaf(logger(&content_log, "c")));
    let mut host = host(move || {
        let (content, place) = (content.clone(), there.get());
        Stack(
            Modifier,
            callback(move || {
                let (first, second) = (content.clone(), content.clone());
                Stack!({
                    if place == 0 {
                        first();
                    }
                });
                Stack!({
                    if place == 1 {
                        second();
                    }
                });
            }),
        );
    });
    assert_eq!(take(&log), ["attach(c)"]);
    place.set(1);
    host.frame();
    assert_eq!(take(&log), ["detach(c)", "attach(c)"]);
    place.set(2);
    host.frame();
    assert_eq!(take(&log), ["detach(c)"]);
    place.set(0);
    host.frame();
    // Gone for good last pass (not used anywhere): new nodes now.
    assert_eq!(take(&log), ["attach(c)"]);
}

// ---- The cursor through the pass's removals ------------------------------------------------------

/// `content` leaving the first of two layouts that stay for the second when `moved`, and a leaf
/// of `z`'s modifier, after them, leaving then.
#[composable]
fn LeavingWithZ(moved: bool, z: Modifier, content: impl Fn() + 'static) {
    let first = content.clone();
    Stack!({
        if !moved {
            first();
        }
    });
    let second = content.clone();
    Stack!({
        if moved {
            second();
        }
    });
    if !moved {
        Leaf(z.clone());
    }
}

/// The removal order holds at every pass of a tree, not only its first: the cursor through the
/// pass's removals starts again at each pass.
#[test]
fn a_moved_node_is_detached_in_the_order_of_the_removals_at_every_pass() {
    let log = Log::default();
    let moved = mutable_state_of(false);
    let (there, content_log, z) = (moved.handle(), log.clone(), logger(&log, "z"));
    let content = movable_content_of(move || Leaf(logger(&content_log, "a")));
    let mut host = host(move || {
        let (content, z, moved) = (content.clone(), z.clone(), there.get());
        Stack(Modifier, callback(move || LeavingWithZ(moved, z.clone(), content.clone())));
    });
    assert_eq!(take(&log), ["attach(a)", "attach(z)"]);
    for round in 0..3 {
        moved.set(true);
        host.frame();
        assert_eq!(take(&log), ["detach(a)", "detach(z)", "attach(a)"], "round {round}");
        moved.set(false);
        host.frame();
        assert_eq!(sorted(take(&log)), ["attach(a)", "attach(z)", "detach(a)"], "round {round}");
    }
}

// ---- A moved node's tasks ------------------------------------------------------------------------

/// Runs its function when dropped.
struct OnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// Launches, when attached, a task that lives until cancelled, counted in `live`.
struct Launching(Rc<std::cell::Cell<i32>>);

struct LaunchingNode(Rc<std::cell::Cell<i32>>);

impl ModifierNodeElement for Launching {
    type Node = LaunchingNode;
    fn create(&self) -> LaunchingNode {
        LaunchingNode(self.0.clone())
    }
    fn update(&self, _: &mut LaunchingNode) {}
    fn same(&self, _: &Self) -> bool {
        true
    }
}

impl ModifierNode for LaunchingNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        let live = self.0.clone();
        context.coroutine_scope().launch(async move {
            live.set(live.get() + 1);
            let counted = live.clone();
            let _gone = OnDrop(move || counted.set(counted.get() - 1));
            loop {
                compose_runtime::with_frame_nanos(|_| ()).await;
            }
        });
    }
}

/// A moved node's scope is cancelled with its detach, and its new `on_attach` launches in a new
/// one: one task lives after the move, as Compose's (`markAsDetached` cancels the scope).
#[test]
fn a_moved_node_s_task_is_cancelled_and_launched_again() {
    let live = Rc::new(std::cell::Cell::new(0));
    let moved = mutable_state_of(false);
    let (there, counted) = (moved.handle(), live.clone());
    let content =
        movable_content_of(move || Leaf(Modifier.then_custom(Launching(counted.clone()))));
    let mut host = host(move || {
        let (content, moved) = (content.clone(), there.get());
        Stack(Modifier, callback(move || Between(moved, content.clone())));
    });
    host.frame();
    assert_eq!(live.get(), 1);
    for to in [true, false, true] {
        moved.set(to);
        host.frame();
        host.frame();
        assert_eq!(live.get(), 1, "moved: {to}");
    }
}

// ---- A panic in a moved node's on_detach ---------------------------------------------------------

/// Panics in its `on_detach` while `armed`, and logs the nearest mark in its callbacks.
#[derive(Clone)]
struct Fragile(Log, Rc<std::cell::Cell<bool>>);

struct FragileNode(Log, Rc<std::cell::Cell<bool>>);

impl ModifierNodeElement for Fragile {
    type Node = FragileNode;
    fn create(&self) -> FragileNode {
        FragileNode(self.0.clone(), self.1.clone())
    }
    fn update(&self, _: &mut FragileNode) {}
    fn same(&self, _: &Self) -> bool {
        true
    }
}

impl ModifierNode for FragileNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        let found = nearest_mark(context);
        self.0.borrow_mut().push(format!("attach under {found}"));
    }
    fn on_detach(&mut self, context: &mut NodeContext<'_>) {
        if self.1.replace(false) {
            panic!("review: on_detach panics");
        }
        let found = nearest_mark(context);
        self.0.borrow_mut().push(format!("detach under {found}"));
    }
}

/// After a moved node's `on_detach` panics (caught by the app), the tree's per-pass state of the
/// move (`detaching_moved`, `removed_parents`, the cursor) doesn't leak into later passes: the
/// node's callbacks at its next moves see its real parents. `detach_removed` sets
/// `detaching_moved` and restores it after `detach_subtree` with no guard: after the panic it
/// stays `Some(node)`, and at the next pass moving the node, its `on_attach` (not detaching)
/// walks up through the parent it left (`parent_for_callbacks` finds the pass's removal first).
#[test]
fn a_panic_in_a_moved_node_s_on_detach_leaves_no_stale_parent() {
    let log = Log::default();
    let armed = Rc::new(std::cell::Cell::new(false));
    let moved = mutable_state_of(false);
    let (there, content_log, fuse) = (moved.handle(), log.clone(), armed.clone());
    let content = movable_content_of(move || {
        Leaf(Modifier.then_custom(Fragile(content_log.clone(), fuse.clone())))
    });
    let mut host = host(move || {
        let (content, moved) = (content.clone(), there.get());
        Stack(Modifier, callback(move || Between(moved, content.clone())));
    });
    assert_eq!(take(&log), ["attach under one"]);
    armed.set(true);
    moved.set(true);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.frame()));
    assert!(panicked.is_err(), "the panic reaches the frame's caller");
    // The panic cut the pass's queue short: the node's `on_attach` where it went never ran
    // (as before D1923), which this test leaves be.
    assert_eq!(take(&log), Vec::<String>::new());
    host.frame();
    take(&log);
    moved.set(false);
    host.frame();
    assert_eq!(take(&log), ["detach under two", "attach under one"]);
    moved.set(true);
    host.frame();
    assert_eq!(take(&log), ["detach under one", "attach under two"]);
}

// ---- Several nodes removed at once ----------------------------------------------------------------

/// Compose's `LayoutNode.removeAt(index, count)` detaches the children it removes from the last
/// to the first (`for (i in index + count - 1 downTo index)`): two leaves leaving together are
/// detached second first. Here `Applier::remove` queues their `Detach` steps first to last
/// (pre-existing for plain removals).
#[test]
fn siblings_removed_together_are_detached_last_first() {
    let log = Log::default();
    let shown = mutable_state_of(true);
    let (there, content_log) = (shown.handle(), log.clone());
    let mut host = host(move || {
        let (shown, log) = (there.get(), content_log.clone());
        Stack(Modifier, callback(move || Pair(shown, logger(&log, "a"), logger(&log, "b"))));
    });
    assert_eq!(take(&log), ["attach(a)", "attach(b)"]);
    shown.set(false);
    host.frame();
    assert_eq!(take(&log), ["detach(b)", "detach(a)"]);
}

/// Two leaves of `a`'s and `b`'s modifiers, when `shown`.
#[composable]
fn Pair(shown: bool, a: Modifier, b: Modifier) {
    if shown {
        Leaf(a.clone());
        Leaf(b.clone());
    }
}

/// The same for content of two top nodes that moves: Compose detaches them second first (one
/// `removeAt` of both), then attaches them first first (an `insertAt` each). D1923 detaches moved
/// nodes "in the order of the removals", which inherits `Applier::remove`'s first-to-last order.
#[test]
fn moved_content_of_two_nodes_is_detached_last_first() {
    let log = Log::default();
    let moved = mutable_state_of(false);
    let (there, content_log) = (moved.handle(), log.clone());
    let content = movable_content_of(move || {
        Leaf(logger(&content_log, "a"));
        Leaf(logger(&content_log, "b"));
    });
    let mut host = host(move || {
        let (content, moved) = (content.clone(), there.get());
        Stack(Modifier, callback(move || Between(moved, content.clone())));
    });
    assert_eq!(take(&log), ["attach(a)", "attach(b)"]);
    moved.set(true);
    host.frame();
    assert_eq!(take(&log), ["detach(b)", "detach(a)", "attach(a)", "attach(b)"]);
}

// ---- Content taken out of removed content ---------------------------------------------------------

/// `outer` (which holds `inner`) in a row, or when `split` only `inner`, in a stack: the outer
/// content leaves the composition, the inner one is taken out of it.
#[composable]
fn TakeOut(split: bool, outer: Callback<dyn Fn()>, inner: Callback<dyn Fn()>) {
    if split {
        let inner = inner.clone();
        Stack!({
            inner();
        });
    } else {
        let outer = outer.clone();
        Row!({
            outer();
        });
    }
}

/// Content taken out of movable content that leaves the composition in the same pass (no longer
/// called anywhere): Compose's `removeAt` detaches the outer content with the inner one in it,
/// then the inner is taken out of the detached outer (`extractMovableContentAtCurrent`'s
/// `applier.remove`, no lifecycle: its parent isn't attached) and inserted where it goes
/// (`insertAt`: attached). The inner content is detached once and attached once.
///
/// Here the outer content is taken out of the tree first (its nodes parked, `detach_pending`),
/// then the inner one out of the parked outer, whose entries are still attached: D1923 left it
/// unmarked, its entries attached, so `attach_subtree` where it went ran no `on_attach` and the
/// outer's detach no longer reached it. D1924 marks it and detaches it with the outer.
#[test]
fn content_taken_out_of_removed_content_is_detached_and_attached_once() {
    let log = Log::default();
    let split = mutable_state_of(false);
    let (there, inner_log, outer_log) = (split.handle(), log.clone(), log.clone());
    let inner = movable_content_of(move || Leaf(logger(&inner_log, "i")));
    let in_outer = inner.clone();
    let outer = movable_content_of(move || {
        let inner = in_outer.clone();
        Stack!(modifier = logger(&outer_log, "o"), {
            inner();
        });
    });
    let mut host = host(move || {
        let (inner, outer, split) = (inner.clone(), outer.clone(), there.get());
        Stack(Modifier, callback(move || TakeOut(split, outer.clone(), inner.clone())));
    });
    assert_eq!(take(&log), ["attach(i)", "attach(o)"]);
    split.set(true);
    host.frame();
    assert_eq!(take(&log), ["detach(o)", "detach(i)", "attach(i)"]);
}

/// `outer` (holding `inner` a layout deep) in a row marked "row", or when `split` only `inner`,
/// in a stack marked "new".
#[composable]
fn TakeOutMarked(split: bool, outer: Callback<dyn Fn()>, inner: Callback<dyn Fn()>) {
    if split {
        Stack(mark("new"), inner.clone());
    } else {
        let outer = outer.clone();
        Row!(modifier = mark("row"), {
            outer();
        });
    }
}

/// The same, the inner content a layout below the outer one's top: it's detached with the outer
/// content, while what that was in is still there (the row the pass releases), as Compose's
/// `removeAt` detached it, under the row; then attached where it goes.
#[test]
fn content_taken_out_of_removed_content_is_detached_under_what_it_was_in() {
    let log = Log::default();
    let split = mutable_state_of(false);
    let (there, inner_log) = (split.handle(), log.clone());
    let inner = movable_content_of(move || Leaf(probe(&inner_log)));
    let in_outer = inner.clone();
    let outer = movable_content_of(move || {
        let inner = in_outer.clone();
        Stack!({
            let inner = inner.clone();
            Stack!({
                inner();
            });
        });
    });
    let mut host = host(move || {
        let (inner, outer, split) = (inner.clone(), outer.clone(), there.get());
        Stack(Modifier, callback(move || TakeOutMarked(split, outer.clone(), inner.clone())));
    });
    assert_eq!(take(&log), ["attach under row"]);
    split.set(true);
    host.frame();
    assert_eq!(take(&log), ["detach under row", "attach under new"]);
}

/// `outer(false)` (holding `inner`) in a row, or when `split` `inner` in a stack then
/// `outer(true)` (no longer holding it) in another: the inner content is wanted before the outer
/// content is placed again.
#[composable]
fn SplitInnerFirst(split: bool, outer: Callback<dyn Fn(bool)>, inner: Callback<dyn Fn()>) {
    if split {
        let inner = inner.clone();
        Stack!({
            inner();
        });
        let outer = outer.clone();
        Stack!({
            outer(true);
        });
    } else {
        let outer = outer.clone();
        Row!({
            outer(false);
        });
    }
}

/// As `content_moved_out_of_moving_content_is_detached_and_attached_once`, the inner content
/// placed before the outer: each is detached once and attached once.
#[test]
fn content_moved_out_of_moving_content_placed_first_is_detached_and_attached_once() {
    let log = Log::default();
    let split = mutable_state_of(false);
    let (there, inner_log, outer_log) = (split.handle(), log.clone(), log.clone());
    let inner = movable_content_of(move || Leaf(logger(&inner_log, "i")));
    let in_outer = inner.clone();
    let outer = movable_content_of_with(move |split: bool| {
        let inner = in_outer.clone();
        Stack!(modifier = logger(&outer_log, "o"), {
            if !split {
                inner();
            }
        });
    });
    let mut host = host(move || {
        let (inner, outer, split) = (inner.clone(), outer.clone(), there.get());
        Stack(Modifier, callback(move || SplitInnerFirst(split, outer.clone(), inner.clone())));
    });
    assert_eq!(take(&log), ["attach(i)", "attach(o)"]);
    split.set(true);
    host.frame();
    assert_eq!(sorted(take(&log)), ["attach(i)", "attach(o)", "detach(i)", "detach(o)"]);
}

/// As `SplitInnerFirst`, the layouts marked: "row" at first, then "inner" and "outer".
#[composable]
fn SplitInnerFirstMarked(split: bool, outer: Callback<dyn Fn(bool)>, inner: Callback<dyn Fn()>) {
    if split {
        Stack(mark("inner"), inner.clone());
        let outer = outer.clone();
        Stack!(modifier = mark("outer"), {
            outer(true);
        });
    } else {
        let outer = outer.clone();
        Row!(modifier = mark("row"), {
            outer(false);
        });
    }
}

/// The same, the outer content in the tree again (under "outer") when it's detached: the inner
/// content, detached with it, finds above it what the outer content left (the row), not where
/// the outer content went, as under Compose's `removeAt`.
#[test]
fn content_moved_out_of_moving_content_is_detached_under_what_it_was_in() {
    let log = Log::default();
    let split = mutable_state_of(false);
    let (there, inner_log) = (split.handle(), log.clone());
    let inner = movable_content_of(move || Leaf(probe(&inner_log)));
    let in_outer = inner.clone();
    let outer = movable_content_of_with(move |split: bool| {
        let inner = in_outer.clone();
        Stack!({
            if !split {
                inner();
            }
        });
    });
    let mut host = host(move || {
        let (inner, outer, split) = (inner.clone(), outer.clone(), there.get());
        Stack(
            Modifier,
            callback(move || SplitInnerFirstMarked(split, outer.clone(), inner.clone())),
        );
    });
    assert_eq!(take(&log), ["attach under row"]);
    split.set(true);
    host.frame();
    assert_eq!(take(&log), ["detach under row", "attach under inner"]);
}

/// `a` then `b` in a layout that stays (each in its own when `apart`), or when `moved` in one the
/// pass inserts before it.
#[composable]
fn TwoIntoNew(moved: bool, apart: bool, a: Callback<dyn Fn()>, b: Callback<dyn Fn()>) {
    if moved {
        let (a, b) = (a.clone(), b.clone());
        Stack!({
            a();
            b();
        });
    }
    if apart {
        let old_a = a.clone();
        Stack!({
            if !moved {
                old_a();
            }
        });
        let old_b = b.clone();
        Stack!({
            if !moved {
                old_b();
            }
        });
    } else {
        let (old_a, old_b) = (a.clone(), b.clone());
        Stack!({
            if !moved {
                old_a();
                old_b();
            }
        });
    }
}

/// Logs of moving `a` and `b` into a layout the pass inserts before them, out of one layout or
/// (`apart`) out of one each.
fn moved_into_a_new_layout(apart: bool) -> Vec<String> {
    let log = Log::default();
    let moved = mutable_state_of(false);
    let (there, a_log, b_log) = (moved.handle(), log.clone(), log.clone());
    let a = movable_content_of(move || Leaf(logger(&a_log, "a")));
    let b = movable_content_of(move || Leaf(logger(&b_log, "b")));
    let mut host = host(move || {
        let (a, b, moved) = (a.clone(), b.clone(), there.get());
        Stack(Modifier, callback(move || TwoIntoNew(moved, apart, a.clone(), b.clone())));
    });
    assert_eq!(take(&log), ["attach(a)", "attach(b)"]);
    moved.set(true);
    host.frame();
    take(&log)
}

/// Two contents moved out of two layouts into one the pass inserts before them: Compose detaches
/// each where it's taken out, `a` first (each its own `removeAt`, in the order the composer
/// removes their groups). D1923 detached both in the new layout's `Attach` step, which went
/// through its subtree in reverse post-order ("outer first"): the later sibling first.
#[test]
fn two_contents_moved_into_a_new_layout_are_detached_in_the_order_they_leave() {
    assert_eq!(moved_into_a_new_layout(true), ["detach(a)", "detach(b)", "attach(a)", "attach(b)"]);
}

/// The same out of one layout, where both leave with one group: Compose removes the group's two
/// nodes with one `removeNode(index, 2)` (`GapComposer`'s group removal; the movable content in
/// it is released, not deleted on its own), and `LayoutNode.removeAt` detaches them last to first.
#[test]
fn two_contents_leaving_one_layout_together_are_detached_last_first() {
    assert_eq!(
        moved_into_a_new_layout(false),
        ["detach(b)", "detach(a)", "attach(a)", "attach(b)"]
    );
}
