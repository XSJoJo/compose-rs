//! `Modifier.scrollable_2d` and its state: Compose's `Scrollable2DTest`, `Scrollable2DInputTest`
//! (by touch, as `TouchScrollable2DTest` runs it), `TouchScrollable2DTest` and
//! `MouseWheelScrollable2DTest`, rewritten. Density 1.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use compose_animation::spring;
use compose_foundation::gestures::LocalBoxFuture;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    IntSize, MutableInteractionSource, NestedScrollConnection, NestedScrollSource, Offset, UnitExt,
    ViewConfiguration,
};

use crate::interaction_helpers::{kinds, record};
use crate::scroll_support::{launch, run};

const TAG: &str = "scrollableBox";

/// A state consuming `fraction` of each delta, adding up what it consumed.
fn consuming(fraction: f32) -> (DefaultScrollable2DState, Rc<Cell<Offset>>) {
    let total = Rc::new(Cell::new(Offset::ZERO));
    let adding = total.clone();
    let state = scrollable_2d_state(move |delta| {
        let used = Offset::new(delta.x * fraction, delta.y * fraction);
        adding.set(adding.get() + used);
        used
    });
    (state, total)
}

fn window(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), content);
    ui.wait_for_idle();
    ui
}

fn scrolling(state: &DefaultScrollable2DState) -> Modifier {
    Modifier.scrollable_2d(state.clone(), true, None, None, None)
}

/// A 100 px box tagged `TAG` with what `modifier` gives, as `setScrollable2DContent`.
fn show(modifier: impl Fn() -> Modifier + 'static) -> ComposeTestRule {
    window(move || Box!(modifier = Modifier.test_tag(TAG).size(100.dp()).then(modifier())))
}

/// A touch drag by `delta` from the tagged node's center over `millis`, as `performScrollGesture`.
fn drag(ui: &mut ComposeTestRule, delta: Offset, millis: u64) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.swipe(center, center + delta, millis);
    });
}

/// The same, held still for 3 s before the release, so it doesn't fling.
fn drag_still(ui: &mut ComposeTestRule, from: Offset, delta: Offset) {
    ui.on_root().perform_touch_input(|touch| {
        touch.down(from);
        touch.move_by(delta);
        touch.advance_event_time(3000);
        touch.up();
    });
}

fn fling(ui: &mut ComposeTestRule, delta: Offset, velocity: f32, millis: u64) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.swipe_with_velocity(center, center + delta, velocity, Some(millis));
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_horizontalScroll
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_verticalScroll
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_diagonalScroll
#[test]
fn a_drag_any_way_scrolls_that_way_and_back() {
    for way in [Offset::new(100.0, 0.0), Offset::new(0.0, 100.0), Offset::new(100.0, 100.0)] {
        let (state, total) = consuming(1.0);
        let mut ui = show(move || scrolling(&state));
        drag(&mut ui, way, 100);
        let there = total.get();
        assert!((way.x == 0.0 || there.x > 1.0) && (way.y == 0.0 || there.y > 1.0), "{there:?}");
        drag(&mut ui, Offset::ZERO - way, 100);
        assert!(total.get().x < 0.01 && total.get().y < 0.01, "{:?}", total.get());
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_disabledWontCallLambda
#[test]
fn disabled_it_scrolls_nothing() {
    let (state, total) = consuming(1.0);
    let enabled = mutable_state_of(true);
    let read = enabled.handle();
    let mut ui = show(move || Modifier.scrollable_2d(state.clone(), read.get(), None, None, None));
    drag(&mut ui, Offset::new(100.0, 100.0), 100);
    let before = total.get();
    assert!(before.x > 0.0 && before.y > 0.0);
    enabled.set(false);
    ui.wait_for_idle();
    drag(&mut ui, Offset::new(100.0, 100.0), 100);
    assert_eq!(total.get(), before);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_startWithoutSlop_ifFlinging
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_blocksDownEvents_ifFlingingCaught
#[test]
fn a_press_catching_a_fling_drags_at_once_and_clicks_nothing() {
    let (state, total) = consuming(1.0);
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let mut ui = window(move || {
        let counted = counted.clone();
        Box!(modifier = Modifier.size(300.dp()).then(scrolling(&state)), {
            let counted = counted.clone();
            Box!(
                modifier = Modifier
                    .size(300.dp())
                    .test_tag(TAG)
                    .clickable(move || counted.set(counted.get() + 1))
            );
        });
    });
    ui.set_auto_advance(false);
    fling(&mut ui, Offset::new(200.0, 200.0), 4000.0, 100);
    let released = total.get();
    assert!(released.x > 0.0 && released.y > 0.0);
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    let flung = total.get();
    assert!(flung.x > released.x && flung.y > released.y);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
        touch.move_by(Offset::new(115.0, 115.0));
        touch.up();
    });
    assert_eq!(total.get(), flung + Offset::new(115.0, 115.0), "no slop");
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.down(touch.center());
        touch.up();
    });
    ui.advance_time_by(Duration::from_millis(100));
    assert_eq!(clicks.get(), 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_explicitDisposal
#[test]
fn a_fling_ends_with_its_modifier() {
    let (state, total) = consuming(1.0);
    let emit = mutable_state_of(true);
    let read = emit.handle();
    let mut ui = show(move || if read.get() { scrolling(&state) } else { Modifier });
    ui.set_auto_advance(false);
    fling(&mut ui, Offset::new(200.0, 200.0), 4000.0, 100);
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    emit.set(false);
    ui.advance_time_by_frame();
    let before = total.get();
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(total.get(), before);
}

/// An outer 300 px scrollable taking all, an inner one taking half, tagged.
fn nested(outer: &DefaultScrollable2DState, inner: &DefaultScrollable2DState) -> ComposeTestRule {
    let (outer, inner) = (outer.clone(), inner.clone());
    window(move || {
        let inner = inner.clone();
        Box!(modifier = Modifier.size(300.dp()).then(scrolling(&outer)), {
            Box!(modifier = Modifier.test_tag(TAG).size(300.dp()).then(scrolling(&inner)));
        });
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_nestedDrag
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_nestedFling
#[test]
fn what_the_inner_leaves_scrolls_the_outer_dragging_and_flinging() {
    let (outer, outer_total) = consuming(1.0);
    let (inner, inner_total) = consuming(0.5);
    let mut ui = nested(&outer, &inner);
    fling(&mut ui, Offset::new(200.0, 200.0), 0.0, 300);
    let equal = inner_total.get();
    assert!(equal.x > 0.0 && equal.y > 0.0);
    assert_eq!(outer_total.get(), equal);
    ui.wait_for_idle();
    assert_eq!((inner_total.get(), outer_total.get()), (equal, equal));

    let (outer, outer_total) = consuming(1.0);
    let (inner, inner_total) = consuming(0.5);
    let mut ui = nested(&outer, &inner);
    ui.set_auto_advance(false);
    drag(&mut ui, Offset::new(200.0, 200.0), 300);
    let equal = inner_total.get();
    assert!(equal.x > 0.0 && equal.y > 0.0);
    assert_eq!(outer_total.get(), equal);
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    let (inner_now, outer_now) = (inner_total.get(), outer_total.get());
    assert!(
        inner_now.x > equal.x
            && inner_now.y > equal.y
            && outer_now.x > equal.x
            && outer_now.y > equal.y
    );
}

/// Takes `fraction` of what's offered before the scroll, and all the velocity.
struct PreConsuming {
    fraction: f32,
    last_available: Rc<Cell<Offset>>,
}

impl NestedScrollConnection for PreConsuming {
    fn on_pre_scroll(&self, available: Offset, _: NestedScrollSource) -> Offset {
        self.last_available.set(available);
        Offset::new(available.x * self.fraction, available.y * self.fraction)
    }

    fn on_pre_fling(&self, available: Offset) -> LocalBoxFuture<'_, Offset> {
        Box::pin(async move { available })
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_nestedScrollAbove_respectsPreConsumption
#[test]
fn what_a_parent_takes_first_and_all_the_velocity_leave_no_fling() {
    let last = Rc::new(Cell::new(Offset::ZERO));
    let (seen, value) = (last.clone(), Rc::new(Cell::new(Offset::ZERO)));
    let adding = value.clone();
    let fraction = 0.7;
    let state = scrollable_2d_state(move |delta| {
        let expected = seen.get();
        assert!(
            (delta.x - expected.x * (1.0 - fraction)).abs() < 0.01,
            "{delta:?} of {expected:?}"
        );
        assert!(
            (delta.y - expected.y * (1.0 - fraction)).abs() < 0.01,
            "{delta:?} of {expected:?}"
        );
        adding.set(adding.get() + delta);
        delta
    });
    let connection: Rc<dyn NestedScrollConnection> =
        Rc::new(PreConsuming { fraction, last_available: last });
    let mut ui = window(move || {
        let state = state.clone();
        Box!(modifier = Modifier.size(300.dp()).nested_scroll(connection.clone()), {
            Box!(modifier = Modifier.size(300.dp()).test_tag(TAG).then(scrolling(&state)));
        });
    });
    ui.set_auto_advance(false);
    drag(&mut ui, Offset::new(200.0, 200.0), 300);
    let released = value.get();
    assert!(released.x > 0.0);
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(value.get(), released);
}

/// Told what's left of each scroll (taking it from a drag) and of the fling.
struct PostCycles {
    expected_left: Rc<Cell<Offset>>,
    post_flings: Rc<Cell<u32>>,
}

impl NestedScrollConnection for PostCycles {
    fn on_post_scroll(&self, _: Offset, available: Offset, source: NestedScrollSource) -> Offset {
        assert_eq!(available, self.expected_left.get());
        if source == NestedScrollSource::SideEffect { Offset::ZERO } else { available }
    }

    fn on_post_fling(&self, consumed: Offset, available: Offset) -> LocalBoxFuture<'_, Offset> {
        Box::pin(async move {
            assert!(consumed.x < 5000.0 * 0.7 && consumed.y < 5000.0 * 0.7, "{consumed:?}");
            self.post_flings.set(self.post_flings.get() + 1);
            available
        })
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_nestedScrollAbove_proxiesPostCycles
#[test]
fn a_parent_is_told_what_is_left_of_scrolls_and_of_the_fling() {
    let expected_left = Rc::new(Cell::new(Offset::ZERO));
    let post_flings = Rc::new(Cell::new(0));
    let left = expected_left.clone();
    let state = scrollable_2d_state(move |delta| {
        let used = Offset::new(delta.x * 0.345, delta.y * 0.345);
        left.set(delta - used);
        used
    });
    let connection: Rc<dyn NestedScrollConnection> =
        Rc::new(PostCycles { expected_left, post_flings: post_flings.clone() });
    let mut ui = window(move || {
        let state = state.clone();
        Box!(modifier = Modifier.size(300.dp()).nested_scroll(connection.clone()), {
            Box!(modifier = Modifier.size(300.dp()).test_tag(TAG).then(scrolling(&state)));
        });
    });
    fling(&mut ui, Offset::new(500.0, 500.0), 5000.0, 300);
    ui.wait_for_idle();
    assert_eq!(post_flings.get(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_nestedScroll_allowParentWhenDisabled
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_nestedScroll_disabledConnectionNoOp
#[test]
fn a_disabled_scrollable_scrolls_nothing_and_passes_the_rest_on() {
    let (parent, parent_total) = consuming(1.0);
    let (child, child_total) = consuming(1.0);
    let mut ui = window(move || {
        let child = child.clone();
        Box!(modifier = Modifier.size(300.dp()).then(scrolling(&parent)), {
            Box!(
                modifier = Modifier.size(200.dp()).test_tag(TAG).scrollable_2d(
                    child.clone(),
                    false,
                    None,
                    None,
                    None
                )
            );
        });
    });
    drag(&mut ui, Offset::new(100.0, 100.0), 200);
    assert_eq!(child_total.get(), Offset::ZERO);
    assert!(parent_total.get().x > 0.0 && parent_total.get().y > 0.0);

    let (parent, parent_total) = consuming(0.5);
    let (middle, middle_total) = consuming(0.5);
    let (child, child_total) = consuming(0.5);
    let mut ui = window(move || {
        let (middle, child) = (middle.clone(), child.clone());
        Box!(modifier = Modifier.size(300.dp()).then(scrolling(&parent)), {
            let child = child.clone();
            Box!(
                modifier =
                    Modifier.size(200.dp()).scrollable_2d(middle.clone(), false, None, None, None),
                {
                    Box!(modifier = Modifier.size(200.dp()).test_tag(TAG).then(scrolling(&child)));
                }
            );
        });
    });
    drag(&mut ui, Offset::new(100.0, 100.0), 200);
    assert!(child_total.get().x > 0.0 && child_total.get().y > 0.0);
    assert_eq!(middle_total.get(), Offset::ZERO);
    assert!(parent_total.get().x > 0.0 && parent_total.get().y > 0.0);
}

/// Records each scroll's pre-scroll offer and post-scroll consumption and offer.
#[derive(Default)]
struct Recorded {
    pre_available: Cell<Offset>,
    post_consumed: Cell<Offset>,
    post_available: Cell<Offset>,
}

struct Recording(Rc<Recorded>);

impl NestedScrollConnection for Recording {
    fn on_pre_scroll(&self, available: Offset, _: NestedScrollSource) -> Offset {
        self.0.pre_available.set(self.0.pre_available.get() + available);
        Offset::ZERO
    }

    fn on_post_scroll(&self, consumed: Offset, available: Offset, _: NestedScrollSource) -> Offset {
        self.0.post_consumed.set(self.0.post_consumed.get() + consumed);
        self.0.post_available.set(self.0.post_available.get() + available);
        Offset::ZERO
    }
}

/// No touch slop: a drag's deltas are all its move.
fn without_slop(ui: &mut ComposeTestRule) {
    ui.host().set_view_configuration(Some(ViewConfiguration {
        touch_slop: 0.0,
        ..ViewConfiguration::default()
    }));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, dispatchScroll_shouldReturnConsumedDeltaInNestedScrollChain
#[test]
fn the_chain_is_told_what_both_scrollables_used() {
    let (outer, outer_total) = consuming(1.0);
    let (inner, inner_total) = consuming(0.5);
    let recording = Rc::new(Recorded::default());
    let connection: Rc<dyn NestedScrollConnection> = Rc::new(Recording(recording.clone()));
    let mut ui = window(move || {
        let (outer, inner) = (outer.clone(), inner.clone());
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let inner = inner.clone();
            Box!(modifier = Modifier.size(300.dp()).then(scrolling(&outer)), {
                Box!(modifier = Modifier.size(300.dp()).then(scrolling(&inner)));
            });
        });
    });
    without_slop(&mut ui);
    drag_still(&mut ui, Offset::new(150.0, 150.0), Offset::new(200.0, 200.0));
    let (inner, outer) = (inner_total.get(), outer_total.get());
    assert!(inner.x > 0.0 && inner.y > 0.0 && outer.x > 0.0 && outer.y > 0.0);
    assert_eq!(recording.post_available.get().x, 0.0);
    assert_eq!(recording.post_consumed.get().x, 200.0);
    assert_eq!(recording.pre_available.get().x, 200.0);
    assert_eq!((inner.x + outer.x, inner.y + outer.y), (200.0, 200.0));
}

/// A fling behavior counting its calls and keeping its velocity, scrolling `step` then using
/// all the velocity.
struct Counting {
    calls: Rc<Cell<u32>>,
    velocity: Rc<Cell<f32>>,
    step: f32,
    scrolled: Rc<Cell<f32>>,
}

impl FlingBehavior for Counting {
    fn perform_fling<'a>(
        &'a self,
        scope: ScrollScope,
        initial_velocity: f32,
    ) -> LocalBoxFuture<'a, f32> {
        Box::pin(async move {
            self.calls.set(self.calls.get() + 1);
            self.velocity.set(initial_velocity);
            if self.step != 0.0 {
                self.scrolled.set(scope.scroll_by(self.step));
            }
            0.0
        })
    }
}

/// The behavior, its calls, the velocity it was given and what its step scrolled.
type Counted = (Rc<dyn FlingBehavior>, Rc<Cell<u32>>, Rc<Cell<f32>>, Rc<Cell<f32>>);

fn counting(step: f32) -> Counted {
    let (calls, velocity, scrolled) =
        (Rc::new(Cell::new(0)), Rc::new(Cell::new(f32::MAX)), Rc::new(Cell::new(0.0)));
    let behavior = Rc::new(Counting {
        calls: calls.clone(),
        velocity: velocity.clone(),
        step,
        scrolled: scrolled.clone(),
    });
    (behavior, calls, velocity, scrolled)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_flingBehaviourCalled_whenVelocity0
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DInputTest.kt, scrollable_flingBehaviourCalled
#[test]
fn a_release_flings_with_its_velocity_once_still_or_not() {
    let (state, _) = consuming(1.0);
    let (behavior, calls, velocity, _) = counting(0.0);
    let given = behavior.clone();
    let mut ui =
        show(move || Modifier.scrollable_2d(state.clone(), true, None, Some(given.clone()), None));
    drag_still(&mut ui, Offset::new(50.0, 50.0), Offset::new(115.0, 0.0));
    assert_eq!(calls.get(), 1);
    assert!(velocity.get().abs() < 0.01, "{}", velocity.get());
    let (state, _) = consuming(1.0);
    let (behavior, calls, velocity, _) = counting(0.0);
    let given = behavior.clone();
    let mut ui =
        show(move || Modifier.scrollable_2d(state.clone(), true, None, Some(given.clone()), None));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.swipe_with_velocity(center, center + Offset::new(115.0, 0.0), 1000.0, None);
    });
    assert_eq!(calls.get(), 1);
    assert!((velocity.get() - 1000.0).abs() <= 1000.0 * 0.005, "{}", velocity.get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TouchScrollable2DTest.kt, scrollable_flingBehaviourCalled_correctScope
#[test]
fn a_fling_scrolls_along_its_angle() {
    let (state, total) = consuming(1.0);
    let (behavior, _, _, scrolled) = counting(123.0);
    let given = behavior.clone();
    let mut ui =
        show(move || Modifier.scrollable_2d(state.clone(), true, None, Some(given.clone()), None));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
        touch.move_by(Offset::new(100.0, 100.0));
    });
    let before = total.get();
    assert!(before.x > 0.0 && before.y > 0.0);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.move_by(Offset::new(100.0, 100.0));
        touch.up();
    });
    let expected = before.x + 123.0 * 0.7 + 100.0;
    assert!(
        (total.get().x - expected).abs() < 1.0 && (total.get().y - expected).abs() < 1.0,
        "{:?}",
        total.get()
    );
    assert_eq!(scrolled.get().round(), 123.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TouchScrollable2DTest.kt, scrollable_interactionSource
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TouchScrollable2DTest.kt, scrollable_interactionSource_resetWhenDisposed
#[test]
fn its_drags_are_told_to_the_interaction_source() {
    let source = MutableInteractionSource::new();
    let recorded = record(&source);
    let (state, _) = consuming(1.0);
    let emit = mutable_state_of(true);
    let (read, given) = (emit.handle(), source.clone());
    let mut ui = window(move || {
        if read.get() {
            Box!(
                modifier = Modifier.test_tag(TAG).size(100.dp()).scrollable_2d(
                    state.clone(),
                    true,
                    None,
                    None,
                    Some(given.clone())
                )
            );
        }
    });
    let press_and_drag = |ui: &mut ComposeTestRule| {
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
            touch.down(Offset::new(25.0, 50.0));
            touch.move_by(Offset::new(50.0, 0.0));
        });
    };
    press_and_drag(&mut ui);
    assert_eq!(kinds(&recorded), ["Start"]);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.up());
    assert_eq!(kinds(&recorded), ["Start", "Stop"]);
    press_and_drag(&mut ui);
    emit.set(false);
    ui.wait_for_idle();
    assert_eq!(kinds(&recorded), ["Start", "Stop", "Start", "DragCancel"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TouchScrollable2DTest.kt, scrollable_noMomentum_shouldChangeScrollStateAfterRelease
#[test]
fn small_moves_held_still_scroll_by_their_sum() {
    let (state, total) = consuming(1.0);
    let mut ui = show(move || scrolling(&state));
    without_slop(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.down(touch.center());
        for _ in 0..30 {
            touch.move_pointer_by(0, Offset::new(10.0, 10.0), 8);
        }
        touch.advance_event_time(3000);
        touch.up();
    });
    assert_eq!(total.get(), Offset::new(300.0, 300.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DTest.kt, scrollable_snappingScrolling
#[test]
fn an_animated_scroll_reaches_its_target_either_way() {
    let (state, total) = consuming(1.0);
    let given = state.clone();
    let mut ui = show(move || scrolling(&given));
    let animating = state.clone();
    run(&mut ui, async move {
        let _ = animating.animate_scroll_by(Offset::new(1000.0, 1000.0), spring()).await;
    });
    assert!((total.get().x - 1000.0).abs() < 0.001 && (total.get().y - 1000.0).abs() < 0.001);
    let animating = state.clone();
    run(&mut ui, async move {
        let _ = animating.animate_scroll_by(Offset::new(-200.0, -200.0), spring()).await;
    });
    assert!((total.get().x - 800.0).abs() < 0.001 && (total.get().y - 800.0).abs() < 0.001);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DTest.kt, scrollable_cancellingAnimateScrollUpdatesIsScrollInProgress
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DTest.kt, scrollable_preemptingAnimateScrollUpdatesIsScrollInProgress
#[test]
fn a_cancelled_or_preempted_scroll_isnt_in_progress() {
    let (state, _) = consuming(1.0);
    let given = state.clone();
    let mut ui = show(move || scrolling(&given));
    ui.set_auto_advance(false);
    let animating = state.clone();
    let job = launch(async move {
        let _ = animating
            .animate_scroll_by(Offset::new(100.0, 100.0), compose_animation::tween(1000))
            .await;
    });
    ui.advance_time_by(Duration::from_millis(500));
    assert!(state.is_scroll_in_progress());
    job.cancel();
    ui.advance_time_by_frame();
    assert!(!state.is_scroll_in_progress());
    let animating = state.clone();
    launch(async move {
        let _ = animating
            .animate_scroll_by(Offset::new(100.0, 100.0), compose_animation::tween(1000))
            .await;
    });
    ui.advance_time_by(Duration::from_millis(500));
    assert!(state.is_scroll_in_progress());
    let preempting = state.clone();
    launch(async move {
        let _ = preempting
            .animate_scroll_by(Offset::new(100.0, 100.0), compose_animation::tween(1000))
            .await;
    });
    ui.advance_time_by(Duration::from_millis(500));
    assert!(state.is_scroll_in_progress());
    ui.advance_time_by(Duration::from_millis(600));
    assert!(!state.is_scroll_in_progress());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DTest.kt, testInspectorValue
#[test]
fn its_inspector_name_is_scrollable2D() {
    let (state, _) = consuming(1.0);
    assert_eq!(format!("{:?}", scrolling(&state)), "Modifier.scrollable2D");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MouseWheelScrollable2DTest.kt, scrollable_noMomentum_shouldChangeScrollStateAfterRelease
#[test]
fn a_wheel_scrolls_by_its_turns() {
    let (state, total) = consuming(1.0);
    let mut ui = show(move || scrolling(&state));
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        mouse.move_to(mouse.center());
        for _ in 0..30 {
            mouse.scroll(0.1, ScrollWheel::Vertical);
        }
    });
    // A turn down scrolls as a drag up does.
    assert!((total.get().y + 30.0 * 6.4).abs() < 0.01, "{:?}", total.get());
}
