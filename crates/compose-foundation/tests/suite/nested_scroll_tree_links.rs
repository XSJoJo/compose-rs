//! A dispatcher's links through the tree (D1917-D1918): moved by movable content, under a
//! disabled scrollable, after a parent was added above, after its node left. A reviewer's
//! scenarios, with the fixes' own.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::gestures::LocalBoxFuture;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    IntSize, NestedScrollConnection, NestedScrollDispatcher, NestedScrollSource, Offset, UnitExt,
};

/// Adds up the pre-scrolls and post-flings it hears along y, taking nothing.
#[derive(Default)]
struct Tally {
    pre: Cell<f32>,
    post_flings: Cell<u32>,
}

impl NestedScrollConnection for Tally {
    fn on_pre_scroll(&self, available: Offset, _: NestedScrollSource) -> Offset {
        self.pre.set(self.pre.get() + available.y);
        Offset::ZERO
    }
    fn on_post_fling(&self, _: Offset, _: Offset) -> LocalBoxFuture<'_, Offset> {
        self.post_flings.set(self.post_flings.get() + 1);
        Box::pin(async { Offset::ZERO })
    }
}

struct Nothing;
impl NestedScrollConnection for Nothing {}

fn none() -> Rc<dyn NestedScrollConnection> {
    Rc::new(Nothing)
}

fn tallying() -> (Rc<Tally>, Rc<dyn NestedScrollConnection>) {
    let tally = Rc::new(Tally::default());
    let connection: Rc<dyn NestedScrollConnection> = tally.clone();
    (tally, connection)
}

fn consuming() -> (DefaultScrollableState, Rc<Cell<f32>>) {
    let total = Rc::new(Cell::new(0.0f32));
    let adding = total.clone();
    let state = scrollable_state(move |delta| {
        adding.set(adding.get() + delta);
        delta
    });
    (state, total)
}

fn window(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), content);
    ui.wait_for_idle();
    ui
}

/// Compose's `ScrollableNestedScrollConnection` returns zero when `enabled` is false.
#[test]
fn review_a_disabled_scrollable_takes_nothing_from_a_dispatcher_inside() {
    let (state, total) = consuming();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let given = given.clone();
        Box!(
            modifier = Modifier.size(300.dp()).scrollable_with(
                state.clone(),
                Orientation::Vertical,
                false,
                false,
                None,
                None
            ),
            {
                Box!(
                    modifier =
                        Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                );
            }
        );
    });
    let taken = dispatcher.dispatch_post_scroll(
        Offset::ZERO,
        Offset::new(0.0, 50.0),
        NestedScrollSource::UserInput,
    );
    let scope = dispatcher.coroutine_scope();
    let d = dispatcher.clone();
    scope.launch(async move {
        d.dispatch_post_fling(Offset::ZERO, Offset::new(0.0, 3000.0)).await;
    });
    ui.wait_for_idle();
    assert_eq!((taken, total.get()), (Offset::ZERO, 0.0), "a disabled scrollable moved");
}

/// Kills the mutant: a scrollable's tree connection flinging loses the chain above it.
#[test]
fn review_a_tree_fling_offers_its_deltas_to_the_parents_above_the_scrollable() {
    let (tally, connection) = tallying();
    let (state, total) = consuming();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let (given, state) = (given.clone(), state.clone());
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let given = given.clone();
            Box!(
                modifier = Modifier.size(300.dp()).scrollable(state.clone(), Orientation::Vertical),
                {
                    Box!(
                        modifier =
                            Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                    );
                }
            );
        });
    });
    let scope = dispatcher.coroutine_scope();
    scope.launch(async move {
        dispatcher.dispatch_post_fling(Offset::ZERO, Offset::new(0.0, 3000.0)).await;
    });
    ui.wait_for_idle();
    assert!(total.get() != 0.0, "the scrollable flung");
    assert!(tally.pre.get() != 0.0, "its fling's deltas reached the parent above it");
    assert!(tally.post_flings.get() >= 1, "the fling's end reached the parent above");
}

/// A dispatch from an effect right after a nested-scroll node was added above, in that frame.
#[test]
fn review_an_effect_dispatch_reaches_a_parent_added_above_in_the_same_frame() {
    let (tally, connection) = tallying();
    let added = mutable_state_of(false);
    let handle = added.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let heard = Rc::new(Cell::new(Offset::ZERO));
    let into = heard.clone();
    let mut ui = window(move || {
        let on = handle.get();
        let above = if on { Modifier.nested_scroll(connection.clone()) } else { Modifier };
        let (given, into) = (given.clone(), into.clone());
        Box!(modifier = above.size(300.dp()), {
            let (given, into) = (given.clone(), into.clone());
            Box!(
                modifier = Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
            );
            LaunchedEffect(on, async move {
                if on {
                    given.dispatch_pre_scroll(Offset::new(0.0, 7.0), NestedScrollSource::UserInput);
                    into.set(Offset::new(0.0, 1.0));
                }
            });
        });
    });
    // A dispatch now resolves the link to "no parent" at this epoch.
    dispatcher.dispatch_pre_scroll(Offset::new(0.0, 1.0), NestedScrollSource::UserInput);
    added.set(true);
    ui.wait_for_idle();
    assert_eq!(heard.get(), Offset::new(0.0, 1.0), "the effect ran");
    assert_eq!(tally.pre.get(), 7.0, "the parent added above heard the effect's dispatch");
}

/// A dispatch from a click handler (the tree busy) right after a parent was added above.
#[test]
fn review_a_click_dispatch_reaches_a_parent_added_above() {
    let (tally, connection) = tallying();
    let added = mutable_state_of(false);
    let handle = added.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let above =
            if handle.get() { Modifier.nested_scroll(connection.clone()) } else { Modifier };
        let given = given.clone();
        Box!(modifier = above.size(300.dp()), {
            let clicked = given.clone();
            Box!(
                modifier = Modifier
                    .size(200.dp())
                    .nested_scroll_with(none(), Some(given.clone()))
                    .test_tag("t")
                    .clickable(move || {
                        clicked.dispatch_pre_scroll(
                            Offset::new(0.0, 5.0),
                            NestedScrollSource::UserInput,
                        );
                    })
            );
        });
    });
    dispatcher.dispatch_pre_scroll(Offset::new(0.0, 1.0), NestedScrollSource::UserInput);
    added.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag("t").perform_click();
    ui.wait_for_idle();
    assert_eq!(tally.pre.get(), 5.0, "the click's dispatch reached the parent added above");
}

/// Movable content carrying a dispatcher's node from under one parent to another.
#[test]
fn review_a_moved_dispatcher_reaches_its_new_parent_only() {
    let (first, first_connection) = tallying();
    let (second, second_connection) = tallying();
    let in_second = mutable_state_of(false);
    let there = in_second.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let content = movable_content_of(move || {
        Box!(modifier = Modifier.size(50.dp()).nested_scroll_with(none(), Some(given.clone())));
    });
    let mut ui = window(move || {
        let moved = there.get();
        let (a, b) = (content.clone(), content.clone());
        Box!(modifier = Modifier.nested_scroll(first_connection.clone()), {
            if !moved {
                a();
            }
        });
        Box!(modifier = Modifier.nested_scroll(second_connection.clone()), {
            if moved {
                b();
            }
        });
    });
    for round in 0..4 {
        let moved = round % 2 == 1;
        in_second.set(moved);
        ui.wait_for_idle();
        let (a, b) = (first.pre.get(), second.pre.get());
        dispatcher.dispatch_pre_scroll(Offset::new(0.0, 3.0), NestedScrollSource::UserInput);
        if moved {
            assert_eq!((first.pre.get(), second.pre.get()), (a, b + 3.0), "round {round}");
        } else {
            assert_eq!((first.pre.get(), second.pre.get()), (a + 3.0, b), "round {round}");
        }
    }
}

/// A dispatcher used after its node left: scrolls reach nothing, the fling's end its last parent.
#[test]
fn review_a_dispatcher_after_its_node_left() {
    let (tally, connection) = tallying();
    let shown = mutable_state_of(true);
    let handle = shown.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let show = handle.get();
        let given = given.clone();
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            if show {
                Box!(
                    modifier =
                        Modifier.size(50.dp()).nested_scroll_with(none(), Some(given.clone()))
                );
            }
        });
    });
    shown.set(false);
    ui.wait_for_idle();
    let pre = dispatcher.dispatch_pre_scroll(Offset::new(0.0, 3.0), NestedScrollSource::UserInput);
    assert_eq!((pre, tally.pre.get()), (Offset::ZERO, 0.0));
    assert!(!dispatcher.coroutine_scope().is_active());
    let scope = CoroutineScope::new();
    let d = dispatcher.clone();
    scope.launch(async move {
        d.dispatch_post_fling(Offset::ZERO, Offset::new(0.0, 100.0)).await;
    });
    ui.wait_for_idle();
    assert_eq!(tally.post_flings.get(), 1, "the fling's end reached the last parent");
}

/// As the disabled scrollable above, with `vertical_scroll(enabled = false)`: what a dispatcher
/// inside leaves goes past it to the parents above, and doesn't move it.
#[test]
fn a_disabled_vertical_scroll_takes_nothing_from_a_dispatcher_inside() {
    let (tally, connection) = tallying();
    let state = ScrollState::new(0);
    let scrolled = *state;
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let given = given.clone();
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let given = given.clone();
            Column!(
                modifier =
                    Modifier.size(300.dp()).vertical_scroll_with(scrolled, false, None, false),
                {
                    Box!(
                        modifier = Modifier
                            .size(1000.dp())
                            .nested_scroll_with(none(), Some(given.clone()))
                    );
                }
            );
        });
    });
    assert!(state.max_value() > 0, "it could scroll");
    let taken = dispatcher.dispatch_post_scroll(
        Offset::ZERO,
        Offset::new(0.0, -50.0),
        NestedScrollSource::UserInput,
    );
    let scope = dispatcher.coroutine_scope();
    let d = dispatcher.clone();
    scope.launch(async move {
        d.dispatch_post_fling(Offset::ZERO, Offset::new(0.0, -3000.0)).await;
    });
    ui.wait_for_idle();
    assert_eq!((taken, state.value()), (Offset::ZERO, 0), "a disabled vertical scroll moved");
    assert_eq!(tally.post_flings.get(), 1, "the fling's end went past it to the parent above");
}

/// What a dispatcher inside leaves while the scrollable's own scroll goes on (inside
/// `state.scroll { … }`) doesn't scroll it, as Compose's `performRawScroll`.
#[test]
fn a_dispatch_inside_while_the_scrollable_scrolls_itself_takes_nothing() {
    let (state, total) = consuming();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let scrolled = state.clone();
    let mut ui = window(move || {
        let given = given.clone();
        Box!(
            modifier = Modifier.size(300.dp()).scrollable(scrolled.clone(), Orientation::Vertical),
            {
                Box!(
                    modifier =
                        Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                );
            }
        );
    });
    let taken = Rc::new(Cell::new(None));
    let (into, d) = (taken.clone(), dispatcher.clone());
    CoroutineScope::new().launch(async move {
        let _ = state
            .scroll(MutatePriority::Default, move |_| async move {
                into.set(Some(d.dispatch_post_scroll(
                    Offset::ZERO,
                    Offset::new(0.0, 50.0),
                    NestedScrollSource::UserInput,
                )));
            })
            .await;
    });
    ui.wait_for_idle();
    assert_eq!(
        (taken.get(), total.get()),
        (Some(Offset::ZERO), 0.0),
        "it scrolled in its own scroll"
    );
    // Outside its scroll, the same dispatch scrolls it.
    let taken = dispatcher.dispatch_post_scroll(
        Offset::ZERO,
        Offset::new(0.0, 50.0),
        NestedScrollSource::UserInput,
    );
    assert_eq!((taken, total.get()), (Offset::new(0.0, 50.0), 50.0));
}

/// A dispatch while the tree is busy uses the links last resolved, past a parent that left
/// since: it reaches the node above that one, not the one gone.
#[test]
fn a_busy_dispatch_skips_a_parent_that_left() {
    let (outer, outer_connection) = tallying();
    let (inner, inner_connection) = tallying();
    let kept = mutable_state_of(true);
    let handle = kept.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let middle =
            if handle.get() { Modifier.nested_scroll(inner_connection.clone()) } else { Modifier };
        let given = given.clone();
        Box!(modifier = Modifier.nested_scroll(outer_connection.clone()), {
            let given = given.clone();
            Box!(modifier = middle.clone().size(300.dp()), {
                Box!(
                    modifier =
                        Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                );
            });
        });
    });
    // Resolved with the tree free: the middle node, then the outer one.
    dispatcher.dispatch_pre_scroll(Offset::new(0.0, 1.0), NestedScrollSource::UserInput);
    assert_eq!((outer.pre.get(), inner.pre.get()), (1.0, 1.0));
    kept.set(false);
    ui.wait_for_idle();
    let d = dispatcher.clone();
    ui.host().with_tree(move |_| {
        d.dispatch_pre_scroll(Offset::new(0.0, 5.0), NestedScrollSource::UserInput);
    });
    assert_eq!((outer.pre.get(), inner.pre.get()), (6.0, 1.0), "past the middle node, gone");
}

/// A scrollable disabled after a dispatcher inside made it a parent takes nothing from then on,
/// and takes again once enabled.
#[test]
fn a_scrollable_disabled_once_a_parent_takes_nothing_until_enabled() {
    let (state, total) = consuming();
    let enabled = mutable_state_of(true);
    let handle = enabled.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let given = given.clone();
        Box!(
            modifier = Modifier.size(300.dp()).scrollable_with(
                state.clone(),
                Orientation::Vertical,
                handle.get(),
                false,
                None,
                None
            ),
            {
                Box!(
                    modifier =
                        Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                );
            }
        );
    });
    let post = |dispatcher: &NestedScrollDispatcher| {
        dispatcher.dispatch_post_scroll(
            Offset::ZERO,
            Offset::new(0.0, 10.0),
            NestedScrollSource::UserInput,
        )
    };
    assert_eq!((post(&dispatcher), total.get()), (Offset::new(0.0, 10.0), 10.0), "enabled");
    enabled.set(false);
    ui.wait_for_idle();
    assert_eq!((post(&dispatcher), total.get()), (Offset::ZERO, 10.0), "disabled");
    enabled.set(true);
    ui.wait_for_idle();
    assert_eq!((post(&dispatcher), total.get()), (Offset::new(0.0, 10.0), 20.0), "enabled again");
}
