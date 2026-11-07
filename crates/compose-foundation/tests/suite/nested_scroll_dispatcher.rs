//! `NestedScrollDispatcher` through `Modifier.nested_scroll_with`: Compose's
//! `NestedScrollModifierTest`, its dispatcher cases, rewritten. Compose's velocities are
//! `Offset`s here, as the connection's (D1917).

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    LocalBoxFuture, ModifierNode, ModifierNodeElement, NestedScrollConnection,
    NestedScrollDispatcher, NestedScrollSource, NodeContext, Offset, UnitExt,
};

const PRE_SCROLL: Offset = Offset::new(120.0, 120.0);
const SCROLL: Offset = Offset::new(125.0, 125.0);
const SCROLL_LEFT: Offset = Offset::new(32.0, 32.0);
const PRE_FLING: Offset = Offset::new(120.0, 120.0);
const POST_FLING_CONSUMED: Offset = Offset::new(151.0, 63.0);
const POST_FLING_LEFT: Offset = Offset::new(11.0, 13.0);

/// A call a connection heard, with what it was given.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Call {
    PreScroll(&'static str, Offset, NestedScrollSource),
    PostScroll(&'static str, Offset, Offset, NestedScrollSource),
    PreFling(&'static str, Offset),
    PostFling(&'static str, Offset, Offset),
}

type Log = Rc<RefCell<Vec<Call>>>;

/// What a probe returns from each callback.
#[derive(Clone, Copy, Default)]
struct Returns {
    pre_scroll: Offset,
    post_scroll: Offset,
    pre_fling: Offset,
    post_fling: Offset,
}

/// A connection writing each call it hears to a log, returning what it's told to.
struct Probe {
    name: &'static str,
    log: Log,
    returns: Cell<Returns>,
}

impl NestedScrollConnection for Probe {
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        self.log.borrow_mut().push(Call::PreScroll(self.name, available, source));
        self.returns.get().pre_scroll
    }

    fn on_post_scroll(
        &self,
        consumed: Offset,
        available: Offset,
        source: NestedScrollSource,
    ) -> Offset {
        self.log.borrow_mut().push(Call::PostScroll(self.name, consumed, available, source));
        self.returns.get().post_scroll
    }

    fn on_pre_fling(&self, available: Offset) -> LocalBoxFuture<'_, Offset> {
        self.log.borrow_mut().push(Call::PreFling(self.name, available));
        let returned = self.returns.get().pre_fling;
        Box::pin(async move { returned })
    }

    fn on_post_fling(&self, consumed: Offset, available: Offset) -> LocalBoxFuture<'_, Offset> {
        self.log.borrow_mut().push(Call::PostFling(self.name, consumed, available));
        let returned = self.returns.get().post_fling;
        Box::pin(async move { returned })
    }
}

fn probe(name: &'static str, log: &Log, returns: Returns) -> Rc<Probe> {
    Rc::new(Probe { name, log: log.clone(), returns: Cell::new(returns) })
}

/// A connection that hears nothing, as `object : NestedScrollConnection {}`.
struct Silent;

impl NestedScrollConnection for Silent {}

fn silent() -> Rc<dyn NestedScrollConnection> {
    Rc::new(Silent)
}

/// The result of a future its connections answer at once.
fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let polled =
        future.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
    match polled {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("the dispatch didn't end at once"),
    }
}

fn nested(connection: &Rc<impl NestedScrollConnection + 'static>) -> Modifier {
    let connection: Rc<dyn NestedScrollConnection> = connection.clone();
    Modifier.nested_scroll(connection)
}

fn dispatching(dispatcher: &NestedScrollDispatcher) -> Modifier {
    Modifier.nested_scroll_with(silent(), Some(dispatcher.clone()))
}

/// Runs all four dispatches with the test's values.
fn dispatch_all(dispatcher: &NestedScrollDispatcher, source: NestedScrollSource) {
    dispatcher.dispatch_pre_scroll(PRE_SCROLL, source);
    dispatcher.dispatch_post_scroll(SCROLL, SCROLL_LEFT, source);
    ready(dispatcher.dispatch_pre_fling(PRE_FLING));
    ready(dispatcher.dispatch_post_fling(POST_FLING_CONSUMED, POST_FLING_LEFT));
}

/// Three boxes inside each other: `grand_parent`, `parent`, and the child dispatching.
fn three_levels(
    grand_parent: Rc<Probe>,
    parent: Rc<Probe>,
    dispatcher: NestedScrollDispatcher,
) -> ComposeTestRule {
    let mut ui = compose_test(move || {
        let (parent, dispatcher) = (parent.clone(), dispatcher.clone());
        Box!(modifier = Modifier.required_size(100.dp()).then(nested(&grand_parent)), {
            let dispatcher = dispatcher.clone();
            Box!(modifier = Modifier.required_size(100.dp()).then(nested(&parent)), {
                Box!(modifier = Modifier.required_size(100.dp()).then(dispatching(&dispatcher)));
            });
        });
    });
    ui.wait_for_idle();
    ui
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_twoNodes_orderTest
#[test]
fn a_child_dispatches_each_step_to_its_parent_in_order() {
    let log = Log::default();
    let parent =
        probe("parent", &log, Returns { post_fling: POST_FLING_LEFT, ..Returns::default() });
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        let given = given.clone();
        Box!(modifier = Modifier.required_size(100.dp()).then(nested(&parent)), {
            Box!(modifier = Modifier.required_size(100.dp()).then(dispatching(&given)));
        });
    });
    ui.wait_for_idle();
    assert!(log.borrow().is_empty());
    dispatch_all(&dispatcher, NestedScrollSource::UserInput);
    assert_eq!(
        *log.borrow(),
        [
            Call::PreScroll("parent", PRE_SCROLL, NestedScrollSource::UserInput),
            Call::PostScroll("parent", SCROLL, SCROLL_LEFT, NestedScrollSource::UserInput),
            Call::PreFling("parent", PRE_FLING),
            Call::PostFling("parent", POST_FLING_CONSUMED, POST_FLING_LEFT),
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_NNodes_orderTest_preScroll
#[test]
fn a_pre_scroll_goes_to_the_outermost_first() {
    let log = Log::default();
    let grand_parent = probe("grand parent", &log, Returns::default());
    let parent = probe("parent", &log, Returns::default());
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    dispatcher.dispatch_pre_scroll(PRE_SCROLL, NestedScrollSource::UserInput);
    let names: Vec<_> = log
        .borrow()
        .iter()
        .map(|call| match call {
            Call::PreScroll(name, ..) => *name,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(names, ["grand parent", "parent"]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_NNodes_orderTest_scroll
#[test]
fn a_post_scroll_goes_to_the_innermost_first() {
    let log = Log::default();
    let grand_parent = probe("grand parent", &log, Returns::default());
    let parent = probe("parent", &log, Returns::default());
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    dispatcher.dispatch_post_scroll(SCROLL, SCROLL_LEFT, NestedScrollSource::UserInput);
    let source = NestedScrollSource::UserInput;
    assert_eq!(
        *log.borrow(),
        [
            Call::PostScroll("parent", SCROLL, SCROLL_LEFT, source),
            Call::PostScroll("grand parent", SCROLL, SCROLL_LEFT, source),
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_NNodes_orderTest_preFling
#[test]
fn a_pre_fling_goes_to_the_outermost_first() {
    let log = Log::default();
    let grand_parent = probe("grand parent", &log, Returns::default());
    let parent = probe("parent", &log, Returns::default());
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    ready(dispatcher.dispatch_pre_fling(PRE_FLING));
    assert_eq!(
        *log.borrow(),
        [Call::PreFling("grand parent", PRE_FLING), Call::PreFling("parent", PRE_FLING)]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_NNodes_orderTest_fling
#[test]
fn a_post_fling_goes_to_the_innermost_first() {
    let log = Log::default();
    let grand_parent = probe("grand parent", &log, Returns::default());
    let parent = probe("parent", &log, Returns::default());
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    ready(dispatcher.dispatch_post_fling(POST_FLING_CONSUMED, POST_FLING_LEFT));
    assert_eq!(
        *log.borrow(),
        [
            Call::PostFling("parent", POST_FLING_CONSUMED, POST_FLING_LEFT),
            Call::PostFling("grand parent", POST_FLING_CONSUMED, POST_FLING_LEFT),
        ]
    );
}

/// What `twoNodes_hierarchyDispatch` and `twoNodes_flatDispatch` check, the parent and child
/// laid out by `content`.
fn check_two_nodes(content: fn(Modifier, Modifier)) {
    let log = Log::default();
    let pre_scroll_return = Offset::new(60.0, 30.0);
    let pre_fling_return = Offset::new(154.0, 56.0);
    let parent = probe(
        "parent",
        &log,
        Returns {
            pre_scroll: pre_scroll_return,
            pre_fling: pre_fling_return,
            ..Returns::default()
        },
    );
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || content(nested(&parent), dispatching(&given)));
    ui.wait_for_idle();
    for source in [NestedScrollSource::UserInput, NestedScrollSource::SideEffect] {
        assert_eq!(dispatcher.dispatch_pre_scroll(PRE_SCROLL, source), pre_scroll_return);
        dispatcher.dispatch_post_scroll(SCROLL, SCROLL_LEFT, source);
    }
    assert_eq!(ready(dispatcher.dispatch_pre_fling(PRE_FLING)), pre_fling_return);
    ready(dispatcher.dispatch_post_fling(POST_FLING_CONSUMED, POST_FLING_LEFT));
    let (user, effect) = (NestedScrollSource::UserInput, NestedScrollSource::SideEffect);
    assert_eq!(
        *log.borrow(),
        [
            Call::PreScroll("parent", PRE_SCROLL, user),
            Call::PostScroll("parent", SCROLL, SCROLL_LEFT, user),
            Call::PreScroll("parent", PRE_SCROLL, effect),
            Call::PostScroll("parent", SCROLL, SCROLL_LEFT, effect),
            Call::PreFling("parent", PRE_FLING),
            Call::PostFling("parent", POST_FLING_CONSUMED, POST_FLING_LEFT),
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_twoNodes_hierarchyDispatch
#[test]
fn a_parent_box_hears_what_its_child_dispatches() {
    check_two_nodes(|parent, child| {
        Box!(modifier = Modifier.required_size(100.dp()).then(parent), {
            Box!(modifier = Modifier.required_size(100.dp()).then(child.clone()));
        });
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_twoNodes_flatDispatch
#[test]
fn a_node_before_in_the_chain_hears_what_the_next_dispatches() {
    check_two_nodes(|parent, child| {
        Box!(modifier = Modifier.required_size(100.dp()).then(parent).then(child));
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_deltaCalculation_preScroll
#[test]
fn each_pre_scroll_is_offered_what_the_ones_above_left() {
    let log = Log::default();
    let dispatched = Offset::new(10.0, 10.0);
    let (by_grand_parent, by_parent) = (Offset::new(2.0, 2.0), Offset::new(1.0, 1.0));
    let grand_parent =
        probe("grand parent", &log, Returns { pre_scroll: by_grand_parent, ..Returns::default() });
    let parent = probe("parent", &log, Returns { pre_scroll: by_parent, ..Returns::default() });
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    let consumed = dispatcher.dispatch_pre_scroll(dispatched, NestedScrollSource::UserInput);
    assert_eq!(consumed, by_grand_parent + by_parent);
    let source = NestedScrollSource::UserInput;
    assert_eq!(
        *log.borrow(),
        [
            Call::PreScroll("grand parent", dispatched, source),
            Call::PreScroll("parent", dispatched - by_grand_parent, source),
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_deltaCalculation_scroll
#[test]
fn each_post_scroll_is_told_what_the_ones_inside_consumed() {
    let log = Log::default();
    let (consumed, dispatched) = (Offset::new(4.0, 4.0), Offset::new(10.0, 10.0));
    let (by_grand_parent, by_parent) = (Offset::new(2.0, 2.0), Offset::new(1.0, 1.0));
    let grand_parent =
        probe("grand parent", &log, Returns { post_scroll: by_grand_parent, ..Returns::default() });
    let parent = probe("parent", &log, Returns { post_scroll: by_parent, ..Returns::default() });
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    let source = NestedScrollSource::UserInput;
    let taken = dispatcher.dispatch_post_scroll(consumed, dispatched, source);
    assert_eq!(taken, by_grand_parent + by_parent);
    assert_eq!(
        *log.borrow(),
        [
            Call::PostScroll("parent", consumed, dispatched, source),
            Call::PostScroll("grand parent", by_parent + consumed, dispatched - by_parent, source),
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_deltaCalculation_preFling
#[test]
fn each_pre_fling_is_offered_what_the_ones_above_left() {
    let log = Log::default();
    let dispatched = Offset::new(10.0, 10.0);
    let (by_grand_parent, by_parent) = (Offset::new(2.0, 2.0), Offset::new(1.0, 1.0));
    let grand_parent =
        probe("grand parent", &log, Returns { pre_fling: by_grand_parent, ..Returns::default() });
    let parent = probe("parent", &log, Returns { pre_fling: by_parent, ..Returns::default() });
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    assert_eq!(ready(dispatcher.dispatch_pre_fling(dispatched)), by_grand_parent + by_parent);
    assert_eq!(
        *log.borrow(),
        [
            Call::PreFling("grand parent", dispatched),
            Call::PreFling("parent", dispatched - by_grand_parent),
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_deltaCalculation_fling
#[test]
fn each_post_fling_is_told_what_the_ones_inside_consumed() {
    let log = Log::default();
    let (consumed, left) = (Offset::new(4.0, 4.0), Offset::new(10.0, 10.0));
    let (by_grand_parent, by_parent) = (Offset::new(2.0, 2.0), Offset::new(1.0, 1.0));
    let grand_parent =
        probe("grand parent", &log, Returns { post_fling: by_grand_parent, ..Returns::default() });
    let parent = probe("parent", &log, Returns { post_fling: by_parent, ..Returns::default() });
    let dispatcher = NestedScrollDispatcher::new();
    let _ui = three_levels(grand_parent, parent, dispatcher.clone());
    let taken = ready(dispatcher.dispatch_post_fling(consumed, left));
    assert_eq!(taken, by_grand_parent + by_parent);
    assert_eq!(
        *log.borrow(),
        [
            Call::PostFling("parent", consumed, left),
            Call::PostFling("grand parent", by_parent + consumed, left - by_parent),
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_shouldNotCalledSelfConnection
#[test]
fn a_dispatcher_never_calls_its_own_node_s_connection() {
    let log = Log::default();
    let own = probe("own", &log, Returns::default());
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        let (own, given) = (own.clone(), given.clone());
        Box!(modifier = Modifier.nested_scroll(silent()), {
            let own: Rc<dyn NestedScrollConnection> = own.clone();
            Box!(modifier = Modifier.nested_scroll_with(own, Some(given.clone())));
        });
    });
    ui.wait_for_idle();
    dispatcher.dispatch_pre_scroll(PRE_SCROLL, NestedScrollSource::UserInput);
    dispatcher.dispatch_post_scroll(SCROLL, SCROLL_LEFT, NestedScrollSource::SideEffect);
    ready(dispatcher.dispatch_pre_fling(PRE_FLING));
    ready(dispatcher.dispatch_post_fling(POST_FLING_CONSUMED, POST_FLING_LEFT));
    assert!(log.borrow().is_empty(), "{:?}", log.borrow());
}

/// `testRootParentAdditionRemoval`: a parent taken away and put back, `content` laying out the
/// parent's modifier (`Modifier` without it) and the child's.
fn check_root_parent_removal(content: fn(Modifier, Modifier)) {
    let pre_scroll_return = Offset::new(60.0, 30.0);
    let log = Log::default();
    let parent =
        probe("parent", &log, Returns { pre_scroll: pre_scroll_return, ..Returns::default() });
    let shown = mutable_state_of(true);
    let read = shown.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        let root = if read.get() { nested(&parent) } else { Modifier };
        content(root, dispatching(&given));
    });
    ui.wait_for_idle();
    let source = NestedScrollSource::UserInput;
    assert_eq!(dispatcher.dispatch_pre_scroll(PRE_SCROLL, source), pre_scroll_return);
    shown.set(false);
    ui.wait_for_idle();
    assert_eq!(dispatcher.dispatch_pre_scroll(PRE_SCROLL, source), Offset::ZERO);
    shown.set(true);
    ui.wait_for_idle();
    assert_eq!(dispatcher.dispatch_pre_scroll(PRE_SCROLL, source), pre_scroll_return);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_hierarchyDispatch_rootParentRemoval
#[test]
fn a_parent_box_s_connection_comes_and_goes() {
    check_root_parent_removal(|root, child| {
        Box!(modifier = Modifier.required_size(100.dp()).then(root), {
            Box!(modifier = child.clone());
        });
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_flatDispatch_rootParentRemoval
#[test]
fn a_connection_before_in_the_chain_comes_and_goes() {
    check_root_parent_removal(|root, child| {
        Box!(modifier = root.then(child));
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_flatDispatch_longChain_rootParentRemoval
#[test]
fn a_connection_before_in_a_long_chain_comes_and_goes() {
    check_root_parent_removal(|root, child| {
        Box!(
            modifier = Modifier
                .required_size(100.dp())
                .then(root)
                .padding(5.dp())
                .required_size(50.dp())
                .then(child)
        );
    });
}

/// `testMiddleParentAdditionRemoval`: a parent between the root's and the child's taken away
/// and put back; `content` lays out the root's, the middle one's (`Modifier` without it) and the
/// child's modifiers.
fn check_middle_parent_removal(content: fn(Modifier, Modifier, Modifier)) {
    let root_consumed = Offset::new(60.0, 30.0);
    let middle_consumed = Offset::new(21.0, 44.0);
    let log = Log::default();
    let root = probe("root", &log, Returns { pre_scroll: root_consumed, ..Returns::default() });
    let middle =
        probe("middle", &log, Returns { pre_scroll: middle_consumed, ..Returns::default() });
    let shown = mutable_state_of(true);
    let read = shown.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        let maybe = if read.get() { nested(&middle) } else { Modifier };
        content(nested(&root), maybe, dispatching(&given));
    });
    ui.wait_for_idle();
    let source = NestedScrollSource::UserInput;
    assert_eq!(dispatcher.dispatch_pre_scroll(PRE_SCROLL, source), root_consumed + middle_consumed);
    shown.set(false);
    ui.wait_for_idle();
    log.borrow_mut().clear();
    assert_eq!(dispatcher.dispatch_pre_scroll(PRE_SCROLL, source), root_consumed);
    assert_eq!(*log.borrow(), [Call::PreScroll("root", PRE_SCROLL, source)], "not the middle one");
    shown.set(true);
    ui.wait_for_idle();
    assert_eq!(dispatcher.dispatch_pre_scroll(PRE_SCROLL, source), root_consumed + middle_consumed);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_hierarchyDispatch_middleParentRemoval
#[test]
fn a_middle_box_s_connection_comes_and_goes() {
    check_middle_parent_removal(|root, middle, child| {
        Box!(modifier = Modifier.required_size(100.dp()).then(root), {
            let (middle, child) = (middle.clone(), child.clone());
            Box!({
                let child = child.clone();
                Box!(modifier = Modifier.required_size(100.dp()).then(middle.clone()), {
                    let child = child.clone();
                    Box!({
                        Box!(modifier = child.clone());
                    });
                });
            });
        });
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_flatDispatch_middleParentRemoval
#[test]
fn a_middle_connection_in_the_chain_comes_and_goes() {
    check_middle_parent_removal(|root, middle, child| {
        Box!(modifier = root.then(middle).then(child));
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_flatDispatch_longChain_middleParentRemoval
#[test]
fn a_middle_connection_in_a_long_chain_comes_and_goes() {
    check_middle_parent_removal(|root, middle, child| {
        Box!(
            modifier = Modifier
                .required_size(100.dp())
                .then(root)
                .required_size(90.dp())
                .clip_to_bounds()
                .then(middle)
                .padding(5.dp())
                .then(child)
        );
    });
}

/// What the two `runtimeSwapChange_orderTest`s check: two connections swapped between the outer
/// and the inner place, by `content` (given which is outer, the two and the child's modifier).
fn check_swap(content: fn(bool, Modifier, Modifier, Modifier)) {
    let log = Log::default();
    let returns = Returns {
        pre_scroll: Offset::new(60.0, 30.0),
        pre_fling: Offset::new(154.0, 56.0),
        ..Returns::default()
    };
    let (first, second) = (probe("1", &log, returns), probe("2", &log, returns));
    let first_outer = mutable_state_of(true);
    let read = first_outer.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        content(read.get(), nested(&first), nested(&second), dispatching(&given));
    });
    ui.wait_for_idle();
    for outer_first in [true, false] {
        log.borrow_mut().clear();
        dispatch_all(&dispatcher, NestedScrollSource::UserInput);
        let (outer, inner) = if outer_first { ("1", "2") } else { ("2", "1") };
        let names: Vec<_> = log
            .borrow()
            .iter()
            .map(|call| match call {
                Call::PreScroll(name, ..)
                | Call::PostScroll(name, ..)
                | Call::PreFling(name, ..)
                | Call::PostFling(name, ..) => *name,
            })
            .collect();
        assert_eq!(names, [outer, inner, inner, outer, outer, inner, inner, outer]);
        first_outer.set(!outer_first);
        ui.wait_for_idle();
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_flatDispatch_runtimeSwapChange_orderTest
#[test]
fn connections_swapped_in_the_chain_swap_their_order() {
    check_swap(|first_outer, first, second, child| {
        let parents = if first_outer { first.then(second) } else { second.then(first) };
        Box!(modifier = Modifier.required_size(100.dp()).then(parents).then(child));
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_hierarchyDispatch_runtimeSwapChange_orderTest
#[test]
fn connections_swapped_between_boxes_swap_their_order() {
    check_swap(|first_outer, first, second, child| {
        let (outer, inner) = if first_outer { (first, second) } else { (second, first) };
        Box!(modifier = Modifier.required_size(100.dp()).then(outer), {
            let child = child.clone();
            Box!(modifier = Modifier.required_size(100.dp()).then(inner.clone()), {
                Box!(modifier = child.clone());
            });
        });
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_rootParentRemoval_childHasProperDispatchScope
#[test]
fn a_child_keeps_an_active_scope_when_its_parent_leaves() {
    let shown = mutable_state_of(true);
    let read = shown.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        let parent = if read.get() { Modifier.nested_scroll(silent()) } else { Modifier };
        let given = given.clone();
        Box!(modifier = parent, {
            Box!(modifier = dispatching(&given));
        });
    });
    ui.wait_for_idle();
    assert!(dispatcher.coroutine_scope().is_active());
    shown.set(false);
    ui.wait_for_idle();
    assert!(dispatcher.coroutine_scope().is_active());
}

/// An outer column's dispatcher and an inner box's, the inner one there while `kept` says so.
fn outer_and_inner(
    kept: MutableState<bool>,
    outer: NestedScrollDispatcher,
    inner: NestedScrollDispatcher,
    outer_connection: Rc<dyn NestedScrollConnection>,
) -> ComposeTestRule {
    let mut ui = compose_test(move || {
        let inner = inner.clone();
        Column!(
            modifier = Modifier.nested_scroll_with(outer_connection.clone(), Some(outer.clone())),
            {
                let maybe = if kept.get() {
                    Modifier.nested_scroll_with(silent(), Some(inner.clone()))
                } else {
                    Modifier
                };
                Box!(modifier = Modifier.size(400.dp()).then(maybe));
            }
        );
    });
    ui.wait_for_idle();
    ui
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, modifierIsRemoved_shouldKeepInfoAboutPreviousParent
#[test]
fn a_removed_node_s_dispatcher_ends_a_fling_with_the_node_that_was_above() {
    let log = Log::default();
    let outer_probe = probe("outer", &log, Returns::default());
    let kept = mutable_state_of(true);
    let (outer, inner) = (NestedScrollDispatcher::new(), NestedScrollDispatcher::new());
    let mut ui = outer_and_inner(kept.handle(), outer, inner.clone(), outer_probe);
    let source = NestedScrollSource::UserInput;
    inner.dispatch_pre_scroll(PRE_SCROLL, source);
    assert_eq!(*log.borrow(), [Call::PreScroll("outer", PRE_SCROLL, source)]);
    kept.set(false);
    ui.wait_for_idle();
    log.borrow_mut().clear();
    // Its node gone, it has no parent...
    assert_eq!(inner.dispatch_pre_scroll(PRE_SCROLL, source), Offset::ZERO);
    assert!(log.borrow().is_empty());
    // ...but a fling's end still reaches the node that was above.
    ready(inner.dispatch_post_fling(POST_FLING_CONSUMED, POST_FLING_LEFT));
    assert_eq!(*log.borrow(), [Call::PostFling("outer", POST_FLING_CONSUMED, POST_FLING_LEFT)]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, modifierIsRemoved_thenReadded_scopeIsCorrect
#[test]
fn a_node_put_back_under_a_cancelled_parent_has_its_own_active_scope() {
    let kept = mutable_state_of(true);
    let (outer, inner) = (NestedScrollDispatcher::new(), NestedScrollDispatcher::new());
    let mut ui = outer_and_inner(kept.handle(), outer.clone(), inner.clone(), silent());
    let before = inner.coroutine_scope();
    assert!(before.is_active());
    assert_eq!(before, outer.coroutine_scope(), "the outermost one's");
    kept.set(false);
    ui.wait_for_idle();
    kept.set(true);
    outer.coroutine_scope().cancel();
    ui.wait_for_idle();
    let after = inner.coroutine_scope();
    assert_ne!(after, before);
    assert!(after.is_active());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, modifierIsRemoved_scopeIsCleared
#[test]
fn a_removed_node_s_dispatcher_has_no_active_scope() {
    let kept = mutable_state_of(true);
    let (outer, inner) = (NestedScrollDispatcher::new(), NestedScrollDispatcher::new());
    let mut ui = outer_and_inner(kept.handle(), outer, inner.clone(), silent());
    let before = inner.coroutine_scope();
    assert!(before.is_active());
    kept.set(false);
    ui.wait_for_idle();
    let after = inner.coroutine_scope();
    assert_ne!(after, before);
    assert!(!after.is_active());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, unattachedDispatcher_coroutineScopeShouldNotBeActive
#[test]
fn a_dispatcher_s_scope_is_active_only_while_its_node_is_attached() {
    let dispatcher = NestedScrollDispatcher::new();
    assert!(!dispatcher.coroutine_scope().is_active());
    let attached = mutable_state_of(true);
    let read = attached.handle();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        Box!(modifier = if read.get() { dispatching(&given) } else { Modifier });
    });
    ui.wait_for_idle();
    assert!(dispatcher.coroutine_scope().is_active());
    attached.set(false);
    ui.wait_for_idle();
    assert!(!dispatcher.coroutine_scope().is_active());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, modifierIsRemoved_shouldKeepInfoAboutPreviousParent (its parent removed with it: `NestedScrollNode.onPostFling` goes on through the detached parents)
#[test]
fn a_fling_s_end_goes_on_through_parents_removed_with_the_node() {
    let log = Log::default();
    let outer = probe("outer", &log, Returns::default());
    let middle = probe("middle", &log, Returns::default());
    let kept = mutable_state_of(true);
    let read = kept.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = compose_test(move || {
        let (middle, given) = (middle.clone(), given.clone());
        Box!(modifier = nested(&outer), {
            if read.get() {
                let given = given.clone();
                Box!(modifier = nested(&middle), {
                    Box!(modifier = dispatching(&given));
                });
            }
        });
    });
    ui.wait_for_idle();
    kept.set(false);
    ui.wait_for_idle();
    ready(dispatcher.dispatch_post_fling(POST_FLING_CONSUMED, POST_FLING_LEFT));
    assert_eq!(
        *log.borrow(),
        [
            Call::PostFling("middle", POST_FLING_CONSUMED, POST_FLING_LEFT),
            Call::PostFling("outer", POST_FLING_CONSUMED, POST_FLING_LEFT),
        ]
    );
}

/// A node reading a dispatcher's scope when it's detached.
#[derive(Clone)]
struct ReadsScopeOnDetach {
    dispatcher: NestedScrollDispatcher,
    read: Rc<Cell<Option<CoroutineScope>>>,
}

struct ReadsScopeOnDetachNode(ReadsScopeOnDetach);

impl ModifierNode for ReadsScopeOnDetachNode {
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.0.read.set(Some(self.0.dispatcher.coroutine_scope()));
    }
}

impl ModifierNodeElement for ReadsScopeOnDetach {
    type Node = ReadsScopeOnDetachNode;
    fn create(&self) -> ReadsScopeOnDetachNode {
        ReadsScopeOnDetachNode(self.clone())
    }
    fn update(&self, _node: &mut ReadsScopeOnDetachNode) {}
    fn same(&self, _other: &Self) -> bool {
        true
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/nestedscroll/NestedScrollModifierTest.kt, nestedScroll_coroutineScope_duringParentDetach_doesNotThrow
#[test]
fn a_dispatcher_s_scope_read_while_its_parent_is_detached_is_there() {
    let dispatcher = NestedScrollDispatcher::new();
    let read = Rc::new(Cell::new(None));
    let element = ReadsScopeOnDetach { dispatcher: dispatcher.clone(), read: read.clone() };
    let attached = mutable_state_of(true);
    let shown = attached.handle();
    let mut ui = compose_test(move || {
        if shown.get() {
            let (dispatcher, element) = (dispatcher.clone(), element.clone());
            Box!(modifier = Modifier.nested_scroll(silent()), {
                Box!(
                    modifier = dispatching(&dispatcher).then_custom(element.clone()).size(100.dp())
                );
            });
        }
    });
    ui.wait_for_idle();
    attached.set(false);
    ui.wait_for_idle();
    assert!(read.get().is_some());
}
