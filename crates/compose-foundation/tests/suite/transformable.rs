//! `Modifier.transformable` and its state: Compose's `TransformableTest`, rewritten (density 1). Its
//! trackpad tests aren't here: a trackpad's pans and scales aren't events in compose-ui yet.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use compose_animation::tween;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{IntSize, Key, Offset, UnitExt, ViewConfiguration};

use crate::scroll_support::{launch, run};

const TAG: &str = "transformable";
/// The share of the box's width a pinch keeps from its edges, as `EDGE_FUZZ_FACTOR`.
const EDGE: f32 = 0.2;

/// What the state was told.
struct Seen {
    scale: Cell<f32>,
    pan: Cell<Offset>,
    rotation: Cell<f32>,
    centroids: RefCell<Vec<Offset>>,
    calls: Cell<u32>,
}

/// A state recording what it's told.
fn recording() -> (Rc<Seen>, DefaultTransformableState) {
    let seen = Rc::new(Seen {
        scale: Cell::new(1.0),
        pan: Cell::new(Offset::ZERO),
        rotation: Cell::new(0.0),
        centroids: RefCell::default(),
        calls: Cell::new(0),
    });
    let into = seen.clone();
    let state = transformable_state(move |centroid, zoom, pan, rotation| {
        into.centroids.borrow_mut().push(centroid);
        into.scale.set(into.scale.get() * zoom);
        into.pan.set(into.pan.get() + pan);
        into.rotation.set(into.rotation.get() + rotation);
        into.calls.set(into.calls.get() + 1);
    });
    (seen, state)
}

fn slop() -> f32 {
    ViewConfiguration::default().touch_slop
}

/// A 600 px box tagged `TAG` with what `modifier` gives, as `setTransformableContent`.
fn show(modifier: impl Fn() -> Modifier + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), move || {
        Box!(modifier = Modifier.size(600.dp()).test_tag(TAG).then(modifier()));
    });
    ui.wait_for_idle();
    ui
}

fn transformable(state: &DefaultTransformableState) -> impl Fn() -> Modifier + 'static {
    let state = state.clone();
    move || Modifier.transformable(state.clone(), false, true)
}

/// Two fingers from 10 px either side of the center out to the edges (`out`), or back in.
fn pinch(ui: &mut ComposeTestRule, out: bool) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (center, width) = (touch.center(), touch.width() as f32);
        let near = (Offset::new(center.x - 10.0, center.y), Offset::new(center.x + 10.0, center.y));
        let far =
            (Offset::new(width * EDGE, center.y), Offset::new(width * (1.0 - EDGE), center.y));
        let (from, to) = if out { (near, far) } else { (far, near) };
        touch.pinch(from.0, to.0, from.1, to.1, 400);
    });
}

fn assert_centroids(seen: &Seen, expected: Offset) {
    for centroid in seen.centroids.borrow().iter() {
        assert!(
            (centroid.x - expected.x).abs() < 0.001 && (centroid.y - expected.y).abs() < 0.001,
            "{centroid:?}"
        );
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_zoomIn
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_zoomOut
#[test]
fn a_pinch_zooms_around_its_center() {
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    pinch(&mut ui, true);
    assert!(seen.scale.get() >= 4.0, "{}", seen.scale.get());
    assert_centroids(&seen, Offset::new(300.0, 300.0));
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    pinch(&mut ui, false);
    assert!(seen.scale.get() <= 0.25, "{}", seen.scale.get());
    assert_centroids(&seen, Offset::new(300.0, 300.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_pan
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_panWithOneFinger
#[test]
fn one_or_two_fingers_pan_by_their_move() {
    let expected = Offset::new(50.0 + slop(), 0.0);
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down_pointer(1, center);
        touch.down_pointer(2, center + Offset::new(10.0, 10.0));
        touch.move_pointer_by(1, expected, 16);
        touch.move_pointer_by(2, expected, 16);
        touch.up_pointer(1);
        touch.up_pointer(2);
    });
    assert_eq!(seen.pan.get(), expected);
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.down_pointer(1, touch.center());
        touch.move_pointer_by(1, expected, 16);
        touch.up_pointer(1);
    });
    assert_eq!(seen.pan.get(), expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_pan_disallowed
#[test]
fn a_pan_can_pan_refuses_starts_nothing() {
    let expected = Offset::new(50.0 + slop(), 0.0);
    let (seen, state) = recording();
    let allowed = Rc::new(Cell::new(false));
    let (given, asked) = (state.clone(), allowed.clone());
    let mut ui = show(move || {
        let asked = asked.clone();
        Modifier.transformable_with_can_pan(given.clone(), move |_| asked.get(), false, true)
    });
    let drag = |ui: &mut ComposeTestRule| {
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
            touch.down_pointer(1, touch.center());
            touch.move_pointer_by(1, expected, 16);
            touch.up_pointer(1);
        });
    };
    drag(&mut ui);
    assert!(!state.is_transform_in_progress());
    assert_eq!(seen.pan.get(), Offset::ZERO);
    allowed.set(true);
    drag(&mut ui);
    assert_eq!(seen.pan.get(), expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformableInsideScroll_pan_disallowed_parentScrolls
#[test]
fn a_pan_it_refuses_scrolls_its_parent() {
    let scroll = ScrollState::new(0);
    let given = *scroll;
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), move || {
        Column!(modifier = Modifier.size(100.dp()).test_tag(TAG).vertical_scroll(given), {
            for _ in 0..3 {
                let state = remember_transformable_state!(|_, _, _, _| {});
                Box!(
                    modifier = Modifier.size(100.dp()).transformable_with_can_pan(
                        state,
                        |_| false,
                        false,
                        true
                    )
                );
            }
        });
    });
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.down_pointer(1, touch.center());
        touch.move_pointer_by(1, Offset::new(0.0, -50.0 - slop()), 16);
        touch.advance_event_time(3000);
        touch.up_pointer(1);
    });
    assert_eq!(scroll.value(), 50);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformableInsideScroll_canPan_offsetProvided
#[test]
fn can_pan_is_asked_with_each_moves_pan() {
    let last = Rc::new(Cell::new(Offset::ZERO));
    let into = last.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), move || {
        let into = into.clone();
        Column!(modifier = Modifier.size(100.dp()).test_tag(TAG), {
            for _ in 0..3 {
                let into = into.clone();
                let state = remember_transformable_state!(|_, _, _, _| {});
                let can_pan = move |offset: Offset| {
                    into.set(offset);
                    false
                };
                Box!(
                    modifier = Modifier
                        .size(100.dp())
                        .transformable_with_can_pan(state, can_pan, false, true)
                );
            }
        });
    });
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.down_pointer(1, touch.center());
        touch.move_pointer_by(1, Offset::new(0.0, -50.0), 16);
    });
    assert_eq!(last.get(), Offset::new(0.0, -50.0));
    ui.on_node_with_tag(TAG)
        .perform_touch_input(|touch| touch.move_pointer_by(1, Offset::new(30.0, 0.0), 16));
    assert_eq!(last.get(), Offset::new(30.0, 0.0));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.up_pointer(1));
}

/// Two fingers 50 px apart down the box, the second turning a quarter around the first, after
/// `pans` steps of a slop's pan together.
fn turn(ui: &mut ComposeTestRule, pans: usize) {
    let shift = Offset::new(slop(), 0.0);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down_pointer(1, center);
        touch.down_pointer(2, center + Offset::new(0.0, 50.0));
        for _ in 0..pans {
            touch.move_pointer_by(1, shift, 16);
            touch.move_pointer_by(2, shift, 16);
        }
        touch.move_pointer_by(2, Offset::new(50.0, -50.0), 16);
        touch.up_pointer(1);
        touch.up_pointer(2);
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_rotate
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_rotationLock
#[test]
fn two_fingers_rotate_unless_a_pan_started_it_locked() {
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    turn(&mut ui, 0);
    assert_eq!(seen.rotation.get(), -90.0);
    let (seen, state) = recording();
    let lock = mutable_state_of(false);
    let read = lock.handle();
    let given = state.clone();
    let mut ui = show(move || Modifier.transformable(given.clone(), read.get(), true));
    turn(&mut ui, 2);
    assert_eq!(seen.rotation.get(), -90.0);
    seen.rotation.set(0.0);
    lock.set(true);
    ui.wait_for_idle();
    turn(&mut ui, 2);
    assert_eq!(seen.rotation.get(), 0.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_startStop_notify
#[test]
fn a_gesture_is_in_progress_until_its_fingers_lift() {
    let (_, state) = recording();
    let mut ui = show(transformable(&state));
    assert!(!state.is_transform_in_progress());
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down_pointer(1, center);
        touch.down_pointer(2, center + Offset::new(10.0, 10.0));
        touch.move_pointer_by(2, Offset::new(slop() * 2.0, slop() * 2.0), 16);
    });
    assert!(state.is_transform_in_progress());
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.up_pointer(1);
        touch.up_pointer(2);
    });
    assert!(!state.is_transform_in_progress());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_disabledWontCallLambda
#[test]
fn disabled_it_tells_its_state_nothing() {
    let (seen, state) = recording();
    let enabled = mutable_state_of(true);
    let read = enabled.handle();
    let given = state.clone();
    let mut ui = show(move || Modifier.transformable(given.clone(), false, read.get()));
    pinch(&mut ui, true);
    let scale = seen.scale.get();
    assert!(scale >= 4.0);
    enabled.set(false);
    ui.wait_for_idle();
    pinch(&mut ui, false);
    assert_eq!(seen.scale.get(), scale);
}

/// `animation` launched on `state`, frames passing one at a time: the calls after the first two
/// frames, then after a long while.
fn animated(
    seen: &Seen,
    state: &DefaultTransformableState,
    animation: impl FnOnce(DefaultTransformableState) -> LocalBoxFuture<'static, ()> + 'static,
) -> ComposeTestRule {
    let mut ui = show(transformable(state));
    ui.set_auto_advance(false);
    launch(animation(state.clone()));
    ui.advance_time_by_frame();
    assert!(seen.calls.get() >= 1);
    ui.advance_time_by_frame();
    assert!(seen.calls.get() >= 2);
    ui.advance_time_by(Duration::from_secs(100));
    assert!(seen.calls.get() >= 3);
    ui
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_animateTo_zoom
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_animateTo_rotate
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_animateTo_pan
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_animateTo_all
#[test]
fn animations_reach_their_targets_over_frames() {
    let (seen, state) = recording();
    animated(&seen, &state, |state| {
        Box::pin(async move {
            let _ = state.animate_zoom_by(4.0).await;
        })
    });
    assert!(seen.scale.get() >= 3.9);
    let (seen, state) = recording();
    animated(&seen, &state, |state| {
        Box::pin(async move {
            let _ = state.animate_rotate_by(180.0).await;
        })
    });
    assert!(seen.rotation.get() >= 179.0);
    let target = Offset::new(100.0, 80.0);
    let (seen, state) = recording();
    animated(&seen, &state, move |state| {
        Box::pin(async move {
            let _ = state.animate_pan_by(target).await;
        })
    });
    assert_eq!(seen.pan.get(), target);
    let (seen, state) = recording();
    animated(&seen, &state, move |state| {
        Box::pin(async move {
            let _ = state.animate_by(4.0, target, 180.0).await;
        })
    });
    assert!(seen.scale.get() >= 3.9 && seen.rotation.get() >= 179.0);
    assert_eq!(seen.pan.get(), target);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_snapTo_zoom
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_snapTo_rotate
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_snapTo_pan
#[test]
fn a_step_is_one_call() {
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    let target = Offset::new(100.0, 80.0);
    let stepping = state.clone();
    run(&mut ui, async move {
        let _ = stepping.zoom_by(4.0, Offset::UNSPECIFIED).await;
        let _ = stepping.rotate_by(180.0, Offset::UNSPECIFIED).await;
        let _ = stepping.pan_by(target, Offset::UNSPECIFIED).await;
    });
    assert_eq!(seen.calls.get(), 3);
    assert_eq!((seen.scale.get(), seen.rotation.get(), seen.pan.get()), (4.0, 180.0, target));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_stopTransformations
#[test]
fn stopping_ends_an_animation() {
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    ui.set_auto_advance(false);
    let animating = state.clone();
    launch(async move {
        let _ = animating.animate_rotate_by(180.0).await;
    });
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    let calls = seen.calls.get();
    assert!(calls >= 2);
    let stopping = state.clone();
    launch(async move {
        let _ = stopping.stop_transformation(MutatePriority::Default).await;
    });
    ui.advance_time_by(Duration::from_secs(100));
    assert_eq!(seen.calls.get(), calls);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_animateCancelledUpdatesIsTransformInProgress
#[test]
fn a_cancelled_animation_isnt_in_progress() {
    let (_, state) = recording();
    let mut ui = show(transformable(&state));
    ui.set_auto_advance(false);
    let animating = state.clone();
    let job = launch(async move {
        let _ = animating.animate_zoom_by_with(4.0, tween(1000), Offset::UNSPECIFIED).await;
    });
    ui.advance_time_by(Duration::from_millis(500));
    assert!(state.is_transform_in_progress());
    job.cancel();
    ui.advance_time_by_frame();
    assert!(!state.is_transform_in_progress());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, testInspectorValue
#[test]
fn its_inspector_name_is_transformable() {
    let (_, state) = recording();
    let modifier = Modifier.transformable(state, false, true);
    assert_eq!(format!("{modifier:?}"), "Modifier.transformable");
}

/// The wheel delta that zooms twice, as `scrollDeltaFor2xZoom`: 545 px against a unit's 64.
const TWICE: f32 = 545.0 / -64.0;

/// A wheel turn of `delta` at (20, 30) on the tagged node, Ctrl held if `ctrl`.
fn wheel(ui: &mut ComposeTestRule, delta: f32, ctrl: bool) {
    if ctrl {
        ui.on_node_with_tag(TAG).perform_key_input(|keys| keys.key_down(Key::CtrlLeft));
    }
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        mouse.move_to(Offset::new(20.0, 30.0));
        mouse.scroll(delta, ScrollWheel::Vertical);
    });
    if ctrl {
        ui.on_node_with_tag(TAG).perform_key_input(|keys| keys.key_up(Key::CtrlLeft));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_ctrlAndMouseScrollUp_doesZoomIn
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_ctrlAndMouseScrollDown_doesZoomOut
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformable_mouseScrollOnly_noZoom
#[test]
fn ctrl_and_the_wheel_zoom_at_the_mouse() {
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    wheel(&mut ui, TWICE, true);
    assert!(seen.scale.get() >= 2.0, "{}", seen.scale.get());
    assert_centroids(&seen, Offset::new(20.0, 30.0));
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    wheel(&mut ui, -TWICE, true);
    assert!(seen.scale.get() <= 0.5, "{}", seen.scale.get());
    assert_centroids(&seen, Offset::new(20.0, 30.0));
    let (seen, state) = recording();
    let mut ui = show(transformable(&state));
    wheel(&mut ui, TWICE, false);
    assert_eq!(seen.scale.get(), 1.0);
}

/// A transformable box over another in a 100 px vertical scroll: what it's told and the scroll.
fn in_a_scroll() -> (Rc<Seen>, Owned<ScrollState>, ComposeTestRule) {
    let (seen, state) = recording();
    let scroll = ScrollState::new(0);
    let given = *scroll;
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), move || {
        let state = state.clone();
        Column!(modifier = Modifier.size(100.dp()).vertical_scroll(given), {
            Box!(
                modifier =
                    Modifier.size(100.dp()).test_tag(TAG).transformable(state.clone(), false, true)
            );
            Box!(modifier = Modifier.size(100.dp()));
        });
    });
    ui.wait_for_idle();
    (seen, scroll, ui)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformableInsideScroll_ctrlAndMouseScroll_doesZoomNoScroll
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/TransformableTest.kt, transformableInsideScroll_mouseScrollOnly_doesScrollNoZoom
#[test]
fn in_a_scroll_ctrl_and_the_wheel_zoom_and_the_wheel_alone_scrolls() {
    let (seen, scroll, mut ui) = in_a_scroll();
    wheel(&mut ui, -TWICE, true);
    assert!(seen.scale.get() <= 0.5, "{}", seen.scale.get());
    assert_eq!(scroll.value(), 0);
    assert_centroids(&seen, Offset::new(20.0, 30.0));
    let (seen, scroll, mut ui) = in_a_scroll();
    wheel(&mut ui, 1.0, false);
    assert_eq!(seen.scale.get(), 1.0);
    assert!(scroll.value() > 0);
}
