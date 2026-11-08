//! What content throws reaches the test, as Compose's `UnhandledExceptionTest` (under
//! `runComposeUiTest`), `junit4/JUnitUnhandledExceptionTest` (under the rule) and
//! `junit4/UncaughtExceptionsInCoroutinesTest` (from an effect): a panic in composition,
//! recomposition, an interaction, measure or drawing unwinds into the test with its payload.
//! Compose's run each over two activities (the default and a custom host's `ComposeView`);
//! compose-rs has one headless host.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Modifier, UnitExt};

/// What the content throws, as the tests' `ExpectedException`.
struct ExpectedException;

fn throw_expected_exception() -> ! {
    std::panic::panic_any(ExpectedException)
}

/// `test` fails with an `ExpectedException`, as the tests' `ExceptionTestRule`.
#[track_caller]
fn expect_expected_exception(test: impl FnOnce()) {
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
    let panic = failed.expect_err("Expected test to fail with an exception, but none was thrown");
    let message = panic.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(
        panic.is::<ExpectedException>(),
        "Expected test to fail with an ExpectedException, got {message:?}"
    );
}

fn throwing_button(on_click: impl Fn() + 'static) {
    Box!(modifier = Modifier.clickable(on_click).size(40.dp()), {
        BasicText!("throw");
    });
}

/// The cases, each on content it sets.
fn throw_during_compose_case(ui: &mut ComposeTestRule) {
    ui.set_content(|| throw_expected_exception());
    ui.wait_for_idle();
}

fn throw_during_recompose_case(ui: &mut ComposeTestRule) {
    let state = mutable_state_of(0);
    let handle = state.handle();
    ui.set_content(move || {
        if handle.get() == 1 {
            throw_expected_exception();
        }
        throwing_button(move || handle.set(1));
    });
    ui.on_node_with_text("throw").perform_click();
    ui.wait_for_idle();
}

fn throw_during_interaction_case(ui: &mut ComposeTestRule) {
    ui.set_content(|| throwing_button(|| throw_expected_exception()));
    ui.on_node_with_text("throw").perform_click();
}

fn throw_during_measure_case(ui: &mut ComposeTestRule) {
    ui.set_content(|| {
        Box!(modifier = Modifier.layout(|_, _, _| throw_expected_exception()));
    });
    ui.wait_for_idle();
}

fn throw_during_draw_case(ui: &mut ComposeTestRule) {
    ui.set_content(|| {
        Box!(modifier = Modifier.fill_max_size().draw_with_content(|_| throw_expected_exception()));
    });
    ui.wait_for_idle();
    // Drawing is a frame's last pass: a headless rule draws when its display list is asked for.
    ui.dump_display_list();
}

/// Under `run_compose_ui_test`.
fn run(case: fn(&mut ComposeTestRule)) {
    expect_expected_exception(|| run_compose_ui_test(case));
}

/// Under the rule, as `createAndroidComposeRule`.
fn rule(case: fn(&mut ComposeTestRule)) {
    expect_expected_exception(|| {
        let mut ui = ComposeTestRule::empty(default_host());
        case(&mut ui);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/UnhandledExceptionTest.kt, throwDuringCompose
#[test]
fn throw_during_compose_in_a_run() {
    run(throw_during_compose_case);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/UnhandledExceptionTest.kt, throwDuringRecompose
#[test]
fn throw_during_recompose_in_a_run() {
    run(throw_during_recompose_case);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/UnhandledExceptionTest.kt, throwDuringInteraction
#[test]
fn throw_during_interaction_in_a_run() {
    run(throw_during_interaction_case);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/UnhandledExceptionTest.kt, throwDuringMeasure
#[test]
fn throw_during_measure_in_a_run() {
    run(throw_during_measure_case);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/UnhandledExceptionTest.kt, throwDuringDraw
#[test]
fn throw_during_draw_in_a_run() {
    run(throw_during_draw_case);
}

// Compose: compose/ui/ui-test-junit4/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/JUnitUnhandledExceptionTest.kt, throwDuringCompose
#[test]
fn throw_during_compose() {
    rule(throw_during_compose_case);
}

// Compose: compose/ui/ui-test-junit4/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/JUnitUnhandledExceptionTest.kt, throwDuringRecompose
#[test]
fn throw_during_recompose() {
    rule(throw_during_recompose_case);
}

// Compose: compose/ui/ui-test-junit4/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/JUnitUnhandledExceptionTest.kt, throwDuringInteraction
#[test]
fn throw_during_interaction() {
    rule(throw_during_interaction_case);
}

// Compose: compose/ui/ui-test-junit4/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/JUnitUnhandledExceptionTest.kt, throwDuringMeasure
#[test]
fn throw_during_measure() {
    rule(throw_during_measure_case);
}

// Compose: compose/ui/ui-test-junit4/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/JUnitUnhandledExceptionTest.kt, throwDuringDraw
#[test]
fn throw_during_draw() {
    rule(throw_during_draw_case);
}

/// An effect that throws after a frame, as the test's `throwInLaunchedEffect`.
fn throw_in_launched_effect(ui: &mut ComposeTestRule) {
    ui.set_content(|| {
        LaunchedEffect((), async {
            with_frame_nanos(|_| ()).await;
            throw_expected_exception();
        });
    });
    ui.wait_for_idle();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/UncaughtExceptionsInCoroutinesTest.kt, test1
#[test]
fn test1() {
    run(throw_in_launched_effect);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/UncaughtExceptionsInCoroutinesTest.kt, test2
/// The same again: the first one's failure didn't take the next test down.
#[test]
fn test2() {
    run(throw_in_launched_effect);
    run(throw_in_launched_effect);
}
