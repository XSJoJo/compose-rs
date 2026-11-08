//! What waiting for idle waits for, as Compose's `junit4/WaitingForOnCommitCallbackTest` (effects
//! and the cascades they start), `junit4/WaitingForPopupTest` (popups), `StandardTestDispatcher`
//! `PhaseOrderingTest` (the order of composition, layout, placement, effects and frame
//! callbacks), and `PrefetchNotHangingMainThreadTest`.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::lazy::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Modifier, Popup, UnitExt};

// ---- WaitingForOnCommitCallbackTest ------------------------------------------------------------

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitingForOnCommitCallbackTest.kt, setContentAndWaitForIdleReleasesAfterOnCommitCallback
#[test]
fn set_content_and_wait_for_idle_releases_after_on_commit_callback() {
    run_compose_ui_test(|ui| {
        let committed = Rc::new(Cell::new(false));
        let switch = mutable_state_of(true);
        let (handle, seen) = (switch.handle(), committed.clone());
        ui.set_content(move || {
            let seen = seen.clone();
            let on = handle.get();
            DisposableEffect(on, move |scope| {
                seen.set(on);
                scope.on_dispose(|| {})
            });
        });
        assert!(committed.get());
        ui.run_on_idle(|_| switch.set(false));
        ui.wait_for_idle();
        assert!(!committed.get());
    });
}

/// Four effects, each keyed on a switch the one before writes: switching the first cascades
/// through them all, in order, before waiting returns.
fn cascade(wait: impl FnOnce(&mut ComposeUiTest)) {
    run_compose_ui_test(|ui| {
        let values: Rc<RefCell<Vec<i32>>> = Rc::default();
        let switches = std::array::from_fn::<_, 4, _>(|_| mutable_state_of(true));
        let handles: Vec<MutableState<bool>> =
            switches.iter().map(|switch| switch.handle()).collect();
        let recorded = values.clone();
        ui.set_content(move || {
            for (index, switch) in handles.iter().enumerate() {
                let (values, next) = (recorded.clone(), handles.get(index + 1).copied());
                let on = switch.get();
                DisposableEffect(on, move |scope| {
                    values.borrow_mut().push(index as i32 + 2);
                    if let Some(next) = next {
                        next.set(on);
                    }
                    scope.on_dispose(|| {})
                });
            }
        });
        ui.run_on_idle(|_| {
            values.borrow_mut().clear();
            // Kick off the cascade.
            values.borrow_mut().push(1);
            switches[0].set(false);
        });
        wait(ui);
        // Mark the end.
        values.borrow_mut().push(6);
        assert_eq!(*values.borrow(), [1, 2, 3, 4, 5, 6]);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitingForOnCommitCallbackTest.kt, cascadingOnCommits
#[test]
fn cascading_on_commits() {
    cascade(|ui| ui.wait_for_idle());
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitingForOnCommitCallbackTest.kt, cascadingOnCommits_suspendedWait_standardTestDispatcher
/// As the one before, waiting with `await_idle`.
#[test]
fn cascading_on_commits_suspended_wait_standard_test_dispatcher() {
    cascade(|ui| {
        // `await_idle` waits as it's first polled.
        let mut waited = std::pin::pin!(ui.await_idle());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(waited.as_mut().poll(&mut context).is_ready());
    });
}

// ---- WaitingForPopupTest -----------------------------------------------------------------------

#[composable]
fn ShowPopup() {
    Popup!({
        Box!(modifier = Modifier.size((10.dp(), 10.dp())));
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitingForPopupTest.kt, popupInFirstComposition
#[test]
fn popup_in_first_composition() {
    run_compose_ui_test(|ui| {
        ui.set_content(ShowPopup);
        ui.on_node(is_popup()).assert_exists();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitingForPopupTest.kt, popupInLaterComposition
#[test]
fn popup_in_later_composition() {
    run_compose_ui_test(|ui| {
        let show = mutable_state_of(false);
        let handle = show.handle();
        ui.set_content(move || {
            if handle.get() {
                ShowPopup();
            }
        });
        ui.on_node(is_popup()).assert_does_not_exist();
        show.set(true);
        ui.on_node(is_popup()).assert_exists();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitingForPopupTest.kt, popupTogglingRepeatedly
#[test]
fn popup_toggling_repeatedly() {
    run_compose_ui_test(|ui| {
        let show = mutable_state_of(false);
        let handle = show.handle();
        ui.set_content(move || {
            if handle.get() {
                ShowPopup();
            }
        });
        ui.on_node(is_popup()).assert_does_not_exist();
        for _ in 0..4 {
            show.update(|shown| !shown);
            if show.get() {
                ui.on_node(is_popup()).assert_exists();
            } else {
                ui.on_node(is_popup()).assert_does_not_exist();
            }
        }
    });
}

// ---- StandardTestDispatcherPhaseOrderingTest --------------------------------------------------

/// Counts steps, each expected in turn, as Compose's `TestCounter`.
#[derive(Clone, Default)]
struct TestCounter(Rc<Cell<i32>>);

impl TestCounter {
    #[track_caller]
    fn expect(&self, step: i32) {
        assert_eq!(self.0.get() + 1, step, "step {step} after {}", self.0.get());
        self.0.set(step);
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/StandardTestDispatcherPhaseOrderingTest.kt, singlePass
/// Composition, measure, placement, then the effect (after layout), then its frame callback;
/// what the callback launches runs after the frame.
#[test]
#[ignore = "a coroutine awaiting with_frame_nanos resumes as its frame callback returns, before a task launched in the callback; Compose's resumption is dispatched after the frame, the launched task first"]
fn single_pass() {
    let counter = TestCounter::default();
    let steps = counter.clone();
    let mut ui = compose_test(move || {
        // This never recomposes.
        steps.expect(1);
        let effect = steps.clone();
        LaunchedEffect((), async move {
            // Started after layout.
            effect.expect(4);
            let in_frame = effect.clone();
            with_frame_nanos(move |_| {
                in_frame.expect(5);
                let launched = in_frame.clone();
                compose_ui::platform::run_soon(move || {
                    // Not before the frame's callbacks are done.
                    launched.expect(7);
                });
                in_frame.expect(6);
            })
            .await;
            effect.expect(8);
        });
        let (measured, placed) = (steps.clone(), steps.clone());
        compose_ui::Layout(
            Modifier,
            compose_ui::measure_policy(move |scope, _, _| {
                measured.expect(2);
                let placed = placed.clone();
                scope.layout_with(1, 1, move |_| placed.expect(3))
            }),
            callback(|| {}),
        );
    });
    ui.wait_for_idle();
    assert_eq!(counter.0.get(), 8);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/StandardTestDispatcherPhaseOrderingTest.kt, frameCallbackRestartsLayout
/// The effect's frame callback, on the second frame, changes what the layout reads: it lays out
/// again in that frame.
#[test]
#[ignore = "a coroutine awaiting with_frame_nanos resumes before the frame's layout, which Compose's resumes after (the layout its callback asked for comes first)"]
fn frame_callback_restarts_layout() {
    let counter = TestCounter::default();
    let first_pass = mutable_state_of(true);
    let layouts = Rc::new(Cell::new(0));
    let (steps, first, counted) = (counter.clone(), first_pass.handle(), layouts.clone());
    let mut ui = compose_test(move || {
        let effect = steps.clone();
        LaunchedEffect((), async move {
            // Started after layout.
            effect.expect(3);
            let in_frame = effect.clone();
            with_frame_nanos(move |_| {
                // Not before the second frame: the first layout comes first.
                in_frame.expect(4);
                first.set(false);
            })
            .await;
            effect.expect(7);
        });
        let (measured, placed, counted) = (steps.clone(), steps.clone(), counted.clone());
        compose_ui::Layout(
            Modifier,
            compose_ui::measure_policy(move |scope, _, _| {
                let first_pass = first.get();
                measured.expect(if first_pass { 1 } else { 5 });
                counted.set(counted.get() + 1);
                let placed = placed.clone();
                scope.layout_with(1, 1, move |_| placed.expect(if first_pass { 2 } else { 6 }))
            }),
            callback(|| {}),
        );
    });
    ui.run_on_idle(|_| assert_eq!(layouts.get(), 2));
    assert_eq!(counter.0.get(), 7);
}

// ---- PrefetchNotHangingMainThreadTest ----------------------------------------------------------

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/PrefetchNotHangingMainThreadTest.kt, prefetchNotHangingMainThread
/// A swipe up a list, then waiting: the prefetch it starts doesn't keep the UI thread busy.
#[test]
fn prefetch_not_hanging_main_thread() {
    let mut ui = compose_test(|| {
        LazyColumn!(|scope: LazyListScope| {
            scope.items(100, |_| {
                Box!(modifier = Modifier.size(30.dp()));
            });
        });
    });
    ui.on_node(has_scroll_to_index_action()).perform_touch_input(|touch| touch.swipe_up());
    ui.wait_for_idle();
    assert!(!ui.has_pending_work());
}
