//! Movable content on a real tree (D1305): its nodes move with their layout, input and effects, and
//! what's read under a `BoxWithConstraints` follows the locals where it's moved to.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::subcompose::*;
use compose_ui::{
    ClickableOptions, FocusRequester, FocusState, Indication, LOCAL_DENSITY, LayoutDirection,
    MutableInteractionSource, Offset, Placeable, TapGestures, UnitExt,
};

use crate::interaction_helpers::*;

const TAG: &str = "moved";

/// Repeated moves between two places: bounds follow, clicks land, what it remembered stays and its
/// effect isn't run again.
#[test]
fn content_moves_with_its_bounds_clicks_and_effects() {
    let (clicks, effects, disposals, made) = (
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
    );
    let in_second = mutable_state_of(false);
    let (c, e, d, m, second) =
        (clicks.clone(), effects.clone(), disposals.clone(), made.clone(), in_second.handle());
    let content = movable_content_of(move || {
        let c = c.clone();
        let (e, d, m) = (e.clone(), d.clone(), m.clone());
        remember(move || m.set(m.get() + 1));
        DisposableEffect((), move |scope| {
            e.set(e.get() + 1);
            scope.on_dispose(move || d.set(d.get() + 1))
        });
        Box!(
            modifier = Modifier
                .size((20.dp(), 20.dp()))
                .clickable(move || c.set(c.get() + 1))
                .test_tag(TAG)
        );
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved_there = second.get();
        Box!({
            let first = content.clone();
            Box!(modifier = Modifier.offset(0.dp(), 0.dp()), {
                if !moved_there {
                    first();
                }
            });
            let then = content.clone();
            Box!(modifier = Modifier.offset(50.dp(), 30.dp()), {
                if moved_there {
                    then();
                }
            });
        });
    });
    for round in 0..4 {
        let there = round % 2 == 1;
        let (left, top) = if there { (50.dp(), 30.dp()) } else { (0.dp(), 0.dp()) };
        in_second.set(there);
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_position_in_root_is_equal_to(left, top);
        ui.on_node_with_tag(TAG).perform_click();
        ui.wait_for_idle();
        assert_eq!(clicks.get(), round + 1, "the click lands where it is, round {round}");
    }
    assert_eq!(made.get(), 1, "what it remembered moved with it");
    assert_eq!((effects.get(), disposals.get()), (1, 0), "its effect stays");
}

static MOVED_LOCAL: CompositionLocal<i32> = CompositionLocal::new_static("moved.local", || 0);

/// Under a `BoxWithConstraints` in moved content: what its content reads follows where it is, also
/// with the value changing in the frame of the move.
#[test]
fn box_with_constraints_reads_the_locals_where_its_moved() {
    let seen = Rc::new(Cell::new(0));
    #[composable]
    fn Reads(seen: Rc<Cell<i32>>) {
        seen.set(MOVED_LOCAL.current());
    }
    let (in_second, first_value) = (mutable_state_of(false), mutable_state_of(1));
    let s = seen.clone();
    let content = movable_content_of(move || {
        let s = s.clone();
        BoxWithConstraints!(move |_| Reads!(s.clone()));
    });
    let (second, value) = (in_second.handle(), first_value.handle());
    let mut ui = compose_test(move || {
        let moved_there = second.get();
        let (first, then) = (content.clone(), content.clone());
        CompositionLocalProvider(MOVED_LOCAL.provides(value.get()), move || {
            let first = first.clone();
            Box!({
                if !moved_there {
                    first();
                }
            });
        });
        CompositionLocalProvider(MOVED_LOCAL.provides(100), move || {
            let then = then.clone();
            Box!({
                if moved_there {
                    then();
                }
            });
        });
    });
    ui.wait_for_idle();
    assert_eq!(seen.get(), 1);
    in_second.set(true);
    ui.wait_for_idle();
    assert_eq!(seen.get(), 100);
    first_value.set(2);
    in_second.set(false);
    ui.wait_for_idle();
    assert_eq!(seen.get(), 2, "the value and the move, in one frame");
}

// ---- Moved nodes are detached and attached (D1923) -------------------------------------------------

/// `content` in a box of each branch of `moved`, of `old`'s modifier or `new`'s, as Compose's
/// `if (moveContent) Box(new) { content() } else Box(old) { content() }`: the move takes it out of
/// one layout and into another.
#[composable]
fn Moving(moved: bool, old: Modifier, new: Modifier, content: impl Fn() + 'static) {
    if moved {
        let content = content.clone();
        Box!(modifier = new, { content() });
    } else {
        let content = content.clone();
        Box!(modifier = old, { content() });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/FocusableTest.kt, movableContent_movedContentBecomesUnfocused
#[test]
fn a_focused_focusable_moved_is_unfocused_and_can_be_focused_again() {
    let source = MutableInteractionSource::new();
    let requester = FocusRequester::new();
    let state = Rc::new(Cell::new(FocusState::Inactive));
    let moved = mutable_state_of(false);
    let (given, seen, there) = (source.clone(), state.clone(), moved.handle());
    let content = movable_content_of(move || {
        let seen = seen.clone();
        Box!(
            modifier = Modifier
                .test_tag(TAG)
                .size(5.dp())
                .focus_requester(requester)
                .on_focus_event(move |focus: FocusState| seen.set(focus))
                .focusable_with(true, Some(given.clone()))
        );
    });
    let mut ui = compose_test(move || {
        Moving(there.get(), Modifier.size(10.dp()), Modifier.size(5.dp()), content.clone());
    });
    let recorded = record(&source);
    ui.wait_for_idle();
    assert!(recorded.borrow().is_empty());
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert!(state.get().is_focused());
    assert_eq!(kinds(&recorded), ["Focus"]);
    ui.on_node_with_tag(TAG).assert_is_focused();
    moved.set(true);
    ui.wait_for_idle();
    assert!(!state.get().is_focused(), "the focus is reset");
    ui.on_node_with_tag(TAG).assert_is_not_focused();
    assert_eq!(kinds(&recorded), ["Focus", "Unfocus"]);
    assert!(ends(&recorded, 0, 1));
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert!(state.get().is_focused());
    assert_eq!(kinds(&recorded), ["Focus", "Unfocus", "Focus"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/FocusableTest.kt, movableContent_canRequestFocusAfterBeingMovedBetweenLayouts
#[test]
fn a_focusable_moved_between_layouts_can_take_the_focus() {
    let (focused, group_has_focus) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let requester = FocusRequester::new();
    let moved = mutable_state_of(false);
    let (seen, there) = (focused.clone(), moved.handle());
    let content = movable_content_of(move || {
        let seen = seen.clone();
        Box!(
            modifier = Modifier
                .size(50.dp())
                .focus_requester(requester)
                .on_focus_changed(move |state: FocusState| seen.set(state.is_focused()))
                .focusable()
                .test_tag(TAG)
        );
    });
    let group = group_has_focus.clone();
    let mut ui = compose_test(move || {
        let group = group.clone();
        let content = content.clone();
        let moved = there.get();
        Box!(
            modifier = Modifier
                .on_focus_changed(move |state: FocusState| group.set(state.has_focus()))
                .focus_group(),
            { Moving(moved, Modifier, Modifier, content.clone()) }
        );
    });
    ui.wait_for_idle();
    assert_eq!((focused.get(), group_has_focus.get()), (false, false));
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert_eq!((focused.get(), group_has_focus.get()), (true, true));
    focused.set(false);
    group_has_focus.set(false);
    moved.set(true);
    ui.wait_for_idle();
    assert_eq!((focused.get(), group_has_focus.get()), (false, false));
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert_eq!((focused.get(), group_has_focus.get()), (true, true));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/HoverableTest.kt, hoverableTest_interactionSource_resetWhenMoved
#[test]
fn a_hoverable_moved_ends_its_hover_and_is_entered_again() {
    let source = MutableInteractionSource::new();
    let moved = mutable_state_of(false);
    let (given, there) = (source.clone(), moved.handle());
    let content = movable_content_of(move || {
        Box!(modifier = Modifier.size(128.dp()).test_tag(TAG).hoverable(given.clone()));
    });
    let mut ui = compose_test(move || {
        Moving(there.get(), Modifier, Modifier, content.clone());
    });
    let recorded = record(&source);
    ui.wait_for_idle();
    assert!(recorded.borrow().is_empty());
    mouse_move(&mut ui, Offset::new(64.0, 64.0));
    assert_eq!(kinds(&recorded), ["Enter"]);
    moved.set(true);
    ui.wait_for_idle();
    // Moved during the hover: it ends, and the mouse resting over the content enters it again.
    assert_eq!(kinds(&recorded), ["Enter", "Exit", "Enter"]);
    assert!(ends(&recorded, 0, 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ClickableTest.kt, interactionSource_resetWhenMoved
#[test]
fn a_clickable_moved_mid_press_cancels_it() {
    let source = MutableInteractionSource::new();
    let moved = mutable_state_of(false);
    let (given, there) = (source.clone(), moved.handle());
    let content = movable_content_of(move || {
        let options = ClickableOptions {
            interaction_source: Some(given.clone()),
            indication: Indication::NONE,
            ..ClickableOptions::DEFAULT
        };
        BasicText!(
            "ClickableText",
            modifier = Modifier.test_tag(TAG).clickable_with(options, || {})
        );
    });
    let mut ui = compose_test(move || {
        Moving(there.get(), Modifier, Modifier, content.clone());
    });
    let recorded = record(&source);
    ui.wait_for_idle();
    assert!(recorded.borrow().is_empty());
    let at = center(&mut ui, TAG);
    touch_down(&mut ui, at);
    assert_eq!(kinds(&recorded), ["Press"]);
    moved.set(true);
    ui.wait_for_idle();
    assert_eq!(kinds(&recorded), ["Press", "Cancel"]);
    assert!(ends(&recorded, 0, 1));
}

/// Runs its function when dropped: a `finally` block.
struct OnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/pointer/SuspendingPointerInputFilterTest.kt, pointerInput_whenMovedBetweenLayouts_restartsBlock
#[test]
fn a_pointer_input_block_moved_between_layouts_starts_again() {
    let (taps, runs, cancellations) =
        (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let moved = mutable_state_of(false);
    let there = moved.handle();
    let (t, r, c) = (taps.clone(), runs.clone(), cancellations.clone());
    let content = movable_content_of(move || {
        let (t, r, c) = (t.clone(), r.clone(), c.clone());
        Box!(
            modifier = Modifier
                .size(60.dp())
                .pointer_input((), move |scope| {
                    let (t, r, c) = (t.clone(), r.clone(), c.clone());
                    async move {
                        r.set(r.get() + 1);
                        // Counted when the block ends, finished or cancelled, as `finally`.
                        let _counted = OnDrop(move || c.set(c.get() + 1));
                        scope
                            .detect_tap_gestures(
                                TapGestures::default().on_tap(move |_| t.set(t.get() + 1)),
                            )
                            .await;
                    }
                })
                .test_tag("pointer_box")
        );
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        Box!(modifier = Modifier.size(200.dp()), {
            let (old, new) = (Modifier.offset(0.dp(), 0.dp()), Modifier.offset(100.dp(), 100.dp()));
            Moving(moved, old, new, content.clone());
        });
    });
    ui.on_node_with_tag("pointer_box").perform_click();
    ui.wait_for_idle();
    assert_eq!((taps.get(), runs.get(), cancellations.get()), (1, 1, 0));
    moved.set(true);
    ui.wait_for_idle();
    assert_eq!((taps.get(), runs.get(), cancellations.get()), (1, 1, 1));
    ui.on_node_with_tag("pointer_box").perform_click();
    ui.wait_for_idle();
    assert_eq!((taps.get(), runs.get(), cancellations.get()), (2, 2, 1));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/WindowInsetsPaddingTest.kt, consumeWindowInsets_movableContent_updateWhileUnattached
#[test]
fn insets_consumed_in_moved_content_updated_in_the_move_reach_what_is_inside() {
    let insets = mutable_state_of(WindowInsets::new(0, 0, 0, 0));
    let moved = mutable_state_of(false);
    let consumed = Rc::new(Cell::new(-1));
    let (value, seen, there) = (insets.handle(), consumed.clone(), moved.handle());
    let content = movable_content_of(move || {
        let seen = seen.clone();
        SubcomposeLayout!(
            measure_policy = subcompose_policy(move |scope, constraints| {
                let seen = seen.clone();
                let measurables = scope.subcompose(
                    "content",
                    Callback::new(move || {
                        let seen = seen.clone();
                        let density = LOCAL_DENSITY.current();
                        Box!(modifier = Modifier.consume_window_insets(value.get()), {
                            let seen = seen.clone();
                            Box!(
                                modifier = Modifier.on_consumed_window_insets_changed(
                                    move |consumed: WindowInsets| {
                                        seen.set(consumed.get_left(&density, LayoutDirection::Ltr))
                                    }
                                )
                            );
                        });
                    }),
                );
                let placeables: Vec<Placeable> = measurables
                    .into_iter()
                    .map(|measurable| scope.measure(measurable, constraints))
                    .collect();
                for placeable in &placeables {
                    scope.place(placeable, 0, 0);
                }
                scope.layout(constraints.max_width, constraints.max_height)
            })
        );
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        Column!({ Moving(moved, Modifier, Modifier, content.clone()) });
    });
    ui.wait_for_idle();
    assert_eq!(consumed.get(), 0);
    moved.set(true);
    insets.set(WindowInsets::new(10, 0, 0, 0));
    ui.wait_for_idle();
    assert_eq!(consumed.get(), 10);
}

/// `content` in the first of two layouts that stay, of `first`'s modifier and `second`'s, or in
/// the second when `moved`.
#[composable]
fn Between(moved: bool, first: Modifier, second: Modifier, content: impl Fn() + 'static) {
    let here = content.clone();
    Box!(modifier = first, {
        if !moved {
            here();
        }
    });
    let there = content.clone();
    Box!(modifier = second, {
        if moved {
            there();
        }
    });
}

/// What a moved `on_consumed_window_insets_changed` (a consuming node) found above it when
/// attached is found again where it goes: what its new ancestors consume, which nothing new
/// above tells it.
#[test]
fn moved_content_hears_what_its_new_ancestors_consume() {
    let moved = mutable_state_of(false);
    let consumed = Rc::new(Cell::new(-1));
    let (seen, there) = (consumed.clone(), moved.handle());
    let content = movable_content_of(move || {
        let seen = seen.clone();
        let density = LOCAL_DENSITY.current();
        Box!(
            modifier = Modifier.on_consumed_window_insets_changed(move |consumed: WindowInsets| {
                seen.set(consumed.get_left(&density, LayoutDirection::Ltr))
            })
        );
    });
    let mut ui = compose_test(move || {
        let consuming = |left| Modifier.consume_window_insets(WindowInsets::new(left, 0, 0, 0));
        Between(there.get(), consuming(10), consuming(20), content.clone());
    });
    ui.wait_for_idle();
    assert_eq!(consumed.get(), 10);
    for (to, left) in [(true, 20), (false, 10), (true, 20)] {
        moved.set(to);
        ui.wait_for_idle();
        assert_eq!(consumed.get(), left, "moved: {to}");
    }
}

/// Regression b/289461011: content kept for reuse, moved, then active again. Compose's host is a
/// `SubcompositionReusableContentHost`; here a `ReusableContentHost` in a `BoxWithConstraints`'s
/// subcomposition.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/node/ModifierNodeAttachOrderTest.kt, reusable_nodes_in_movable_content
#[test]
fn reusable_nodes_in_moved_content_are_reused_after_the_move() {
    let (active, in_box) = (mutable_state_of(true), mutable_state_of(true));
    let (on, boxed) = (active.handle(), in_box.handle());
    let content = movable_content_of(move || {
        BoxWithConstraints!(move |_| {
            ReusableContentHost(on.get(), || {
                BasicText!("Hello World", modifier = Modifier.test_tag(TAG));
            });
        });
    });
    let mut ui = compose_test(move || {
        Moving(!boxed.get(), Modifier, Modifier, content.clone());
    });
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_exists();
    active.set(false);
    ui.wait_for_idle();
    in_box.set(false);
    ui.wait_for_idle();
    active.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_exists();
}
