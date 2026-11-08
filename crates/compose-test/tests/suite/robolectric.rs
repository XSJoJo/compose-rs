//! Compose's `RobolectricComposeTest`: `run_compose_ui_test`'s basic scenarios (state, input,
//! animation, a layout that never settles, a paused clock and a fling, a text field, work
//! posted to the UI thread, a popup, taps by injection and by raw events, a clipped node
//! growing). Robolectric's main looper is compose-rs's UI-thread executor; Android's
//! `dispatchTouchEvent` is the host's `send_pointer`.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use compose_animation::*;
use compose_foundation::gestures::{FlingBehavior, LocalBoxFuture, ScrollScope};
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Dp, Modifier, Offset, PointerEvent, PointerId, PointerType, Popup, SemanticsActions,
    TapGestures, UnitExt, ViewConfiguration,
};

#[composable]
fn ClickCounter(clicks: MutableState<i32>) {
    Column!({
        Box!(
            modifier = Modifier.clickable(move || clicks.update(|count| count + 1)).size(40.dp()),
            {
                BasicText!("Click me");
            }
        );
        BasicText!("Click count: {}", clicks.get());
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testStateChange
#[test]
fn test_state_change() {
    run_compose_ui_test(|ui| {
        let clicks = mutable_state_of(0);
        let handle = clicks.handle();
        ui.set_content(move || ClickCounter(handle));
        ui.on_node_with_text("Click me").assert_exists();
        clicks.update(|count| count + 1);
        ui.on_node(has_text_containing("Click count")).assert_text_equals(&["Click count: 1"]);
        clicks.update(|count| count + 1);
        ui.on_node(has_text_containing("Click count")).assert_text_equals(&["Click count: 2"]);
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testInputInjection
#[test]
fn test_input_injection() {
    run_compose_ui_test(|ui| {
        let clicks = mutable_state_of(0);
        let handle = clicks.handle();
        ui.set_content(move || ClickCounter(handle));
        ui.on_node_with_text("Click me").assert_exists();
        ui.on_node_with_text("Click me").perform_click();
        ui.on_node(has_text_containing("Click count")).assert_text_equals(&["Click count: 1"]);
        ui.on_node_with_text("Click me").perform_click();
        ui.on_node(has_text_containing("Click count")).assert_text_equals(&["Click count: 2"]);
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testAnimation
#[test]
fn test_animation() {
    run_compose_ui_test(|ui| {
        let target = mutable_state_of(0.0f32);
        let handle = target.handle();
        ui.set_content(move || {
            let offset = animate_float_as_state!(handle.get());
            Box!(modifier = Modifier.fill_max_size(), {
                Box!(
                    modifier =
                        Modifier.size(10.dp()).offset(Dp(offset.get()), 0.dp()).test_tag("box")
                );
            });
        });
        ui.on_node_with_tag("box").assert_left_position_in_root_is_equal_to(0.dp());
        target.set(100.0);
        ui.on_node_with_tag("box").assert_left_position_in_root_is_equal_to(100.dp());
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testTimeout
/// A measure policy that writes the state it reads never settles: waiting fails, where
/// Compose's says it didn't get idle within its idling policy's 2 s. compose-rs's passes run at
/// the current time without a frame between them, so the rule gives up after its count of
/// passes ("never settled"), before any idle timeout.
#[test]
fn test_timeout() {
    let message = super::input::expect_error(|| {
        run_compose_ui_test(|ui| {
            ui.set_idle_timeout(Duration::from_secs(2));
            ui.set_content(|| {
                let x = remember(|| mutable_state_of(0));
                Box!(modifier = Modifier.required_size(100.dp()), {
                    let policy =
                        compose_ui::measure_policy(move |scope, measurables, constraints| {
                            let placeables: Vec<_> = measurables
                                .iter()
                                .map(|it| scope.measure(*it, constraints))
                                .collect();
                            // Read x, so a change of it lays out again.
                            let offset = if x.get() >= 0 { 0 } else { -1 };
                            let width = offset + placeables.iter().map(|p| p.width).max().unwrap();
                            let height =
                                offset + placeables.iter().map(|p| p.height).max().unwrap();
                            // Whoops, x always changes during layout!
                            x.set(if x.get() == 0 { 1 } else { 0 });
                            scope.layout_with(width, height, move |placement| {
                                for placeable in &placeables {
                                    placement.place(placeable, 0, 0);
                                }
                            })
                        });
                    compose_ui::Layout(
                        Modifier,
                        policy,
                        callback(|| {
                            Box!(modifier = Modifier.size(10.dp()));
                        }),
                    );
                });
            });
        });
    });
    assert!(message.contains("the UI never settled"), "{message}");
}

/// Scrolls one of `deltas` each frame, whatever the velocity, as Compose's test's
/// `SimpleFlingBehavior`.
struct SimpleFlingBehavior {
    deltas: Vec<i32>,
}

impl SimpleFlingBehavior {
    fn total_distance(&self) -> i32 {
        self.deltas.iter().sum()
    }
}

impl FlingBehavior for SimpleFlingBehavior {
    fn perform_fling<'a>(
        &'a self,
        scope: ScrollScope,
        _initial_velocity: f32,
    ) -> LocalBoxFuture<'a, f32> {
        Box::pin(async move {
            for delta in &self.deltas {
                compose_runtime::with_frame_nanos(|_| ()).await;
                scope.scroll_by(*delta as f32);
            }
            0.0
        })
    }
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testControlledScrolling
/// A swipe with the clock paused leaves the list where the swipe ends; resumed, the fling
/// runs on.
#[test]
fn test_controlled_scrolling() {
    run_compose_ui_test(|ui| {
        let touch_slop = 16.0;
        let scroll_state = ScrollState::new(0);
        let given = *scroll_state;
        let fling = Rc::new(SimpleFlingBehavior { deltas: (1..=20).rev().collect() });
        let behavior: Rc<dyn FlingBehavior> = fling.clone();
        ui.host()
            .set_view_configuration(Some(ViewConfiguration { touch_slop, ..Default::default() }));
        ui.set_content(move || {
            let behavior = behavior.clone();
            // No visual overscroll, for the distances.
            CompositionLocalProvider(LOCAL_OVERSCROLL_FACTORY.provides(None), move || {
                let behavior = behavior.clone();
                Box!(modifier = Modifier.fill_max_size(), {
                    Column!(
                        modifier = Modifier
                            .required_size(200.dp())
                            .vertical_scroll_with(given, true, Some(behavior.clone()), false)
                            .test_tag("list"),
                        {
                            for _ in 0..100 {
                                Spacer!(modifier = Modifier.fill_max_width().height(30.dp()));
                            }
                        }
                    );
                });
            });
        });

        // The clock paused, a swipe: the list stays where the swipe ends.
        ui.set_auto_advance(false);
        ui.on_node_with_tag("list").perform_touch_input(|touch| {
            let bottom_center = touch.bottom_center();
            touch.down(bottom_center);
            for step in 0..10 {
                let up = touch.percent_offset(0.0, (step + 1) as f32 / 10.0);
                touch.move_to(bottom_center - up);
            }
            touch.up();
        });
        ui.wait_for_idle();
        let viewport = ui.density().to_px(200.dp());
        let swiped = (viewport - touch_slop).round() as i32;
        assert_eq!(scroll_state.value(), swiped);

        // Resumed, the fling runs: the list stops its total distance further on.
        ui.set_auto_advance(true);
        ui.wait_for_idle();
        assert_eq!(scroll_state.value(), swiped + fling.total_distance());
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testTextFieldInteraction
/// A field's selection set at the end, typing appends (b/227120770).
#[test]
fn test_text_field_interaction() {
    run_compose_ui_test(|ui| {
        let state: Rc<RefCell<Option<TextFieldState>>> = Rc::default();
        let kept = state.clone();
        ui.set_content(move || {
            let field = remember_text_field_state("a", None);
            *kept.borrow_mut() = Some(field);
            BasicTextFieldOfState!(field);
        });
        ui.on_node_with_text("a").assert_is_displayed();
        // As `performTextInputSelection(TextRange(1))`.
        ui.on_node_with_text("a").perform_semantics_action_with(
            &SemanticsActions::SET_SELECTION,
            |set| {
                set(1, 1, false);
            },
        );
        ui.on_node_with_text("a").perform_text_input("b");
        ui.run_on_idle(|_| {
            let text = state.borrow().as_ref().unwrap().text().to_string();
            assert_eq!(text, "ab");
        });
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testComposeMainQueueTrampolining
/// Work posted to the UI thread and recompositions alternating: waiting for idle waits for
/// both to be done.
#[test]
fn test_compose_main_queue_trampolining() {
    run_compose_ui_test(|ui| {
        const ITERATIONS: i32 = 10;
        let phase = mutable_state_of(0);
        let handle = phase.handle();
        let done = Rc::new(Cell::new(false));
        let finished = done.clone();
        ui.set_content(move || {
            let current = handle.get();
            if (1..ITERATIONS).contains(&current) {
                DisposableEffect(current, move |scope| {
                    // As `handler.post { phase++ }`.
                    compose_ui::platform::run_soon(move || handle.update(|phase| phase + 1));
                    scope.on_dispose(|| {})
                });
            } else if current == ITERATIONS {
                let finished = finished.clone();
                DisposableEffect((), move |scope| {
                    finished.set(true);
                    scope.on_dispose(|| {})
                });
            }
        });
        ui.run_on_idle(|_| {
            assert_eq!(phase.get(), 0);
            assert!(!done.get());
        });
        // The trampolining starts.
        phase.set(1);
        ui.run_on_idle(|_| {
            assert_eq!(phase.get(), ITERATIONS);
            assert!(done.get());
        });
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testWaitForPopupWindow
/// A popup's content is found once it shows (a box stands for Compose's `DropdownMenuItem`).
#[test]
fn test_wait_for_popup_window() {
    run_compose_ui_test(|ui| {
        let expanded = mutable_state_of(false);
        let handle = expanded.handle();
        ui.set_content(move || {
            Box!(modifier = Modifier.required_size(20.dp()), {
                if handle.get() {
                    Popup!({
                        Box!(modifier = Modifier.test_tag("MenuContent").clickable(|| {}), {
                            BasicText!("Option 1");
                        });
                    });
                }
            });
        });
        ui.on_node_with_tag("MenuContent").assert_does_not_exist();
        expanded.set(true);
        ui.on_node_with_tag("MenuContent").assert_is_displayed();
    });
}

/// What a tap detector filling the window heard: presses, taps, double taps, long presses.
#[derive(Default)]
struct Taps {
    touches: Cell<u32>,
    taps: Cell<u32>,
    double_taps: Cell<u32>,
    long_taps: Cell<u32>,
}

impl Taps {
    fn counts(&self) -> (u32, u32, u32, u32) {
        (self.touches.get(), self.taps.get(), self.double_taps.get(), self.long_taps.get())
    }
}

fn bump(cell: &Cell<u32>) {
    cell.set(cell.get() + 1);
}

/// A "mainBox" filling the window, its taps detected into `taps`.
fn tap_content(ui: &mut ComposeUiTest, taps: &Rc<Taps>) {
    let taps = taps.clone();
    ui.set_content(move || {
        let taps = taps.clone();
        Box!(
            modifier =
                Modifier.test_tag("mainBox").fill_max_size().pointer_input((), move |scope| {
                    let (press, tap, double, long) =
                        (taps.clone(), taps.clone(), taps.clone(), taps.clone());
                    async move {
                        scope
                            .detect_tap_gestures(
                                TapGestures::default()
                                    .on_press(move |_| bump(&press.touches))
                                    .on_tap(move |_| bump(&tap.taps))
                                    .on_double_tap(move |_| bump(&double.double_taps))
                                    .on_long_press(move |_| bump(&long.long_taps)),
                            )
                            .await
                    }
                }),
            {
                Box!(modifier = Modifier.fill_max_size());
            }
        );
    });
    ui.on_node_with_tag("mainBox").assert_is_displayed();
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, areTwoTapsFired_dispatchTwoDelayedTapsWithPerformTouchInput_assertTrue
/// Two taps 400 ms apart are two taps (past the double tap's timeout).
#[test]
fn are_two_taps_fired_dispatch_two_delayed_taps_with_perform_touch_input_assert_true() {
    run_compose_ui_test(|ui| {
        let taps = Rc::new(Taps::default());
        tap_content(ui, &taps);
        for _ in 0..2 {
            ui.on_node_with_tag("mainBox").perform_touch_input(|touch| {
                touch.down(touch.center());
                touch.up();
            });
            // The tap detector's timeouts go by the clock.
            ui.advance_time_by(Duration::from_millis(400));
        }
        assert_eq!(taps.counts(), (2, 2, 0, 0));
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, isDoubleTapFired_dispatchTwoTapsWithPerformTouchInput_assertTrue
/// Two taps 100 ms apart are a double tap; the second's down and up in separate inputs.
#[test]
fn is_double_tap_fired_dispatch_two_taps_with_perform_touch_input_assert_true() {
    run_compose_ui_test(|ui| {
        let taps = Rc::new(Taps::default());
        tap_content(ui, &taps);
        ui.on_node_with_tag("mainBox").perform_touch_input(|touch| {
            touch.down(touch.center());
            touch.up();
        });
        ui.advance_time_by(Duration::from_millis(100));
        ui.on_node_with_tag("mainBox").perform_touch_input(|touch| touch.down(touch.center()));
        ui.on_node_with_tag("mainBox").perform_touch_input(|touch| touch.up());
        // Inside the double tap's time.
        ui.advance_time_by(Duration::from_millis(100));
        assert_eq!(taps.counts(), (2, 0, 1, 0));
    });
}

/// A finger's raw down and up at the window's top left, as the `MotionEvent`s Compose's test
/// dispatches to its view.
fn raw_tap(ui: &mut ComposeUiTest) {
    let finger = |event: PointerEvent| event.with_pointer(PointerId(1), PointerType::Touch);
    ui.host().send_pointer(finger(PointerEvent::press(Offset::ZERO)));
    ui.host().send_pointer(finger(PointerEvent::release(Offset::ZERO)));
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, isTapFired_dispatchTapWithMotionEvents_assertTrue
#[test]
fn is_tap_fired_dispatch_tap_with_motion_events_assert_true() {
    run_compose_ui_test(|ui| {
        let taps = Rc::new(Taps::default());
        tap_content(ui, &taps);
        raw_tap(ui);
        // Past the double tap's timeout: the tap is a tap.
        ui.advance_time_by(Duration::from_millis(400));
        assert_eq!(taps.counts(), (1, 1, 0, 0));
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, isDoubleTapFired_dispatchTwoTapsWithMotionEvents_assertTrue
#[test]
fn is_double_tap_fired_dispatch_two_taps_with_motion_events_assert_true() {
    run_compose_ui_test(|ui| {
        let taps = Rc::new(Taps::default());
        tap_content(ui, &taps);
        raw_tap(ui);
        ui.advance_time_by(Duration::from_millis(100));
        raw_tap(ui);
        ui.advance_time_by(Duration::from_millis(100));
        assert_eq!(taps.counts(), (2, 0, 1, 0));
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricComposeTest.kt, testInputInjectionIntoClippedItemChangingItsSize
/// A clipped node grown from nothing is clicked where it now is.
#[test]
fn test_input_injection_into_clipped_item_changing_its_size() {
    run_compose_ui_test(|ui| {
        let size = mutable_state_of(0.0f32);
        let handle = size.handle();
        let clicks = Rc::new(Cell::new(0));
        let counted = clicks.clone();
        ui.set_content(move || {
            let counted = counted.clone();
            Column!({
                let counted = counted.clone();
                Box!(
                    modifier = Modifier
                        .clip_to_bounds()
                        .size(Dp(handle.get()))
                        .test_tag("tag")
                        .clickable(move || counted.set(counted.get() + 1))
                );
            });
        });
        ui.run_on_idle(|_| size.set(200.0));
        ui.on_node_with_tag("tag").perform_click();
        ui.run_on_idle(|_| assert_eq!(clicks.get(), 1));
    });
}
