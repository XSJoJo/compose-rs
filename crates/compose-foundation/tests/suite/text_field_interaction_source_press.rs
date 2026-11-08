//! The press interactions a state-based field emits into its interaction source, from Compose's
//! `TextFieldInteractionSourcePressTest`: a click presses and releases, a press dragged out of
//! the field is cancelled, a decoration without the inner field is pressed too, a new source
//! gets the next presses, and a double tap's second press (taken to select a word) is cancelled.
//!
//! As Compose's tests: the test font at 10 sp, density 1. Only the presses are kept, as its
//! `filterIsInstance<PressInteraction>()` (a tap's focus is emitted too).

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::mutable_state_of;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{Density, IntSize, MutableInteractionSource, Offset, UnitExt};

use crate::interaction_helpers::{Recorded, kinds, record};

const TAG: &str = "BasicTextField";

fn style() -> TextStyle {
    TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT }
}

fn state() -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new("Hello World")))
}

/// The test font at density 1, as `setTextFieldTestContent`.
fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer), content);
    ui.wait_for_idle();
    ui
}

/// A field over "Hello World" telling `source` of its interactions.
fn field(source: &MutableInteractionSource) -> ComposeTestRule {
    let (state, source) = (state(), source.clone());
    rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            text_style = style(),
            interaction_source = Some(source.clone()),
        );
    })
}

/// The presses recorded, in order: `"Press"`, `"Release"`, `"Cancel"`.
fn presses(recorded: &Recorded) -> Vec<&'static str> {
    kinds(recorded)
        .into_iter()
        .filter(|kind| matches!(*kind, "Press" | "Release" | "Cancel"))
        .collect()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldInteractionSourcePressTest.kt, normalClick_sendsPressAndReleaseEvents
#[test]
fn normalClick_sendsPressAndReleaseEvents() {
    let source = MutableInteractionSource::new();
    let mut ui = field(&source);
    let recorded = record(&source);
    ui.on_node_with_tag(TAG).perform_click();
    ui.run_on_idle(|_| assert_eq!(presses(&recorded), ["Press", "Release"]));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldInteractionSourcePressTest.kt, touchDown_thenSwipeOutOfBounds_sendsPressAndCancelEvents
#[test]
fn touchDown_thenSwipeOutOfBounds_sendsPressAndCancelEvents() {
    let source = MutableInteractionSource::new();
    let mut ui = field(&source);
    let recorded = record(&source);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
        touch.move_by(Offset::new(500.0, 500.0));
        touch.up();
    });
    ui.run_on_idle(|_| assert_eq!(presses(&recorded), ["Press", "Cancel"]));
}

/// The decoration alone (the inner field not called) is enough to emit them.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldInteractionSourcePressTest.kt, decorationBox_sendsEvents
#[test]
fn decorationBox_sendsEvents() {
    let source = MutableInteractionSource::new();
    let (state, given) = (state(), source.clone());
    let mut ui = rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            text_style = style(),
            interaction_source = Some(given.clone()),
            decorator = |_inner: compose_runtime::Callback| {
                Box!(Modifier.size(100.dp()));
            },
        );
    });
    let recorded = record(&source);
    ui.on_node_with_tag(TAG).perform_click();
    ui.run_on_idle(|_| assert_eq!(presses(&recorded), ["Press", "Release"]));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldInteractionSourcePressTest.kt, interactionSourceChanges_eventsAreSentToNewSource
#[test]
fn interactionSourceChanges_eventsAreSentToNewSource() {
    let first = MutableInteractionSource::new();
    let current = mutable_state_of(first.clone());
    let (state, given) = (state(), current.handle());
    let mut ui = rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            text_style = style(),
            interaction_source = Some(given.get()),
        );
    });
    let recorded = record(&first);
    ui.on_node_with_tag(TAG).perform_click();
    ui.run_on_idle(|_| assert_eq!(presses(&recorded).len(), 2));

    let second = MutableInteractionSource::new();
    current.set(second.clone());
    ui.wait_for_idle();
    let recorded_second = record(&second);
    ui.on_node_with_tag(TAG).perform_click();
    ui.run_on_idle(|_| {
        assert_eq!(presses(&recorded_second).len(), 2);
        // The first source heard nothing more.
        assert_eq!(presses(&recorded).len(), 2);
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldInteractionSourcePressTest.kt, doubleTap_sendsTwoPressEvents
#[test]
fn doubleTap_sendsTwoPressEvents() {
    let source = MutableInteractionSource::new();
    let mut ui = field(&source);
    let recorded = record(&source);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.double_click());
    // The second tap is consumed to select the tapped word: it's cancelled.
    ui.run_on_idle(|_| assert_eq!(presses(&recorded), ["Press", "Release", "Press", "Cancel"]));
}
