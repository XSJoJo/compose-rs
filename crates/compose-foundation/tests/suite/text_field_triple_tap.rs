//! A finger's triple tap on a state-based field, from Compose's `TextFieldTripleTapTest`: the
//! first tap puts the cursor, the second selects the word, the third the paragraph (on a word,
//! between words, scrolled, decorated); a drag after it extends by paragraphs, down or up.
//!
//! As Compose's tests (and `text_field_double_tap.rs`): the test font at 10 sp, density 1, touch
//! input mode.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{Density, InputMode, IntSize, Offset, TextRange, UnitExt};

use crate::text_field_touch_support::*;

const TAG: &str = "BasicTextField";
const FONT: f32 = 10.0;

fn state_of(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

fn touch_rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer), content);
    ui.host().set_input_mode(InputMode::Touch);
    ui
}

fn style() -> TextStyle {
    TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT }
}

fn field(state: TextFieldState) -> ComposeTestRule {
    touch_rule(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG), text_style = style());
    })
}

/// Three taps at `at`, the double-tap minimum apart, the third held unless `lift`, as the
/// selection tests' `tripleTap(offset, liftPointer)`.
fn triple_tap(touch: &mut TouchInjectionScope<'_>, at: Offset, lift: bool) {
    for tap in 0..3 {
        if tap > 0 {
            let wait = touch.view_configuration().double_tap_min_time_millis + 1;
            touch.advance_event_time(wait);
        }
        touch.down(at);
        touch.move_to(at);
        if tap < 2 || lift {
            touch.up();
        }
    }
}

fn assert_selection_handles_shown(ui: &mut ComposeTestRule) {
    ui.on_node(is_selection_handle(Some(Handle::SelectionStart))).assert_is_displayed();
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).assert_is_displayed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTripleTapTest.kt, emptyTextField_tripleTapDoesNotShowCursor
#[test]
fn emptyTextField_tripleTapDoesNotShowCursor() {
    let mut ui = field(state_of(""));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        triple_tap(touch, center, true);
    });
    ui.wait_for_idle();
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTripleTapTest.kt, tripleTap_firstPlacesTheCursor_thenSelectsWord_thenSelectsParagraph
#[test]
fn tripleTap_firstPlacesTheCursor_thenSelectsWord_thenSelectsParagraph() {
    let state = state_of("abc def ghi\nabc def ghi");
    let mut ui = field(state);
    let at = Offset::new(FONT * 5.0, FONT / 2.0);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(at));
    ui.wait_for_idle();
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_is_displayed();
    assert_eq!(state.selection(), TextRange::at(5));

    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(at));
    ui.wait_for_idle();
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(4, 7));

    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(at));
    ui.wait_for_idle();
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(0, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTripleTapTest.kt, tripleTap_emptySpace_selectsParagraph
#[test]
fn tripleTap_emptySpace_selectsParagraph() {
    let state = state_of("abc  def");
    let mut ui = field(state);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        triple_tap(touch, Offset::new(FONT * 4.0, FONT / 2.0), true);
    });
    ui.wait_for_idle();
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(0, 8));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTripleTapTest.kt, tripleTap_scrolledTextField_selectsParagraph
#[test]
fn tripleTap_scrolledTextField_selectsParagraph() {
    let state = state_of("abc def ghi abc def ghi");
    let scroll = **Box::leak(Box::new(ScrollState::new(0)));
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(30.dp()),
            text_style = style(),
            line_limits = TextFieldLineLimits::SingleLine,
            scroll_state = Some(scroll),
        );
    });
    ui.wait_for_idle();
    assert!(scroll.max_value() > 0);
    crate::scroll_support::run(&mut ui, async move {
        let _ = scroll.scroll_to(scroll.max_value()).await;
    });
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let at = Offset::new(touch.right(), touch.center_y());
        triple_tap(touch, at, true);
    });
    ui.wait_for_idle();
    ui.on_node(is_selection_handle(Some(Handle::SelectionStart))).assert_is_displayed();
    // The whole selection can't fit.
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).assert_is_not_displayed();
    assert_eq!(state.selection(), TextRange::new(0, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTripleTapTest.kt, tripleTap_decoratedTextField_selectsParagraph
#[test]
fn tripleTap_decoratedTextField_selectsParagraph() {
    let state = state_of("abc def ghi");
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            text_style = style(),
            decorator = |inner: compose_runtime::Callback| {
                Box!(Modifier.padding(32.dp()), { inner() });
            },
        );
    });
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        triple_tap(touch, Offset::new(32.0 + FONT * 5.0, 32.0 + FONT / 2.0), true);
    });
    ui.wait_for_idle();
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(0, 11));
}

/// A triple tap at `at` held, then a move by `by` and a lift: the selection then (while the
/// field is there: it collapses as the field goes, losing the focus).
fn triple_tap_and_drag(state: TextFieldState, at: Offset, by: Offset) -> TextRange {
    let mut ui = field(state);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        triple_tap(touch, at, false);
        touch.move_by(by);
        touch.up();
    });
    ui.wait_for_idle();
    state.selection()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTripleTapTest.kt, tripleTapThen_dragDown_selectsFromCurrentToTargetParagraph_ltr
#[test]
fn tripleTapThen_dragDown_selectsFromCurrentToTargetParagraph_ltr() {
    let state = state_of("abc def\nabc def\nabc def");
    let selection = triple_tap_and_drag(
        state,
        Offset::new(FONT * 5.0, FONT / 2.0),
        Offset::new(0.0, FONT * 3.0 / 2.0),
    );
    assert_eq!(selection, TextRange::new(0, 15));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTripleTapTest.kt, tripleTapThen_dragUp_selectsFromCurrentToTargetWord_ltr
#[test]
fn tripleTapThen_dragUp_selectsFromCurrentToTargetWord_ltr() {
    let state = state_of("abc def\nabc def\nabc def");
    // The second line's "def", then up a line.
    let selection = triple_tap_and_drag(
        state,
        Offset::new(FONT * 5.0, FONT * 3.0 / 2.0),
        Offset::new(0.0, -FONT * 2.0),
    );
    assert_eq!(selection, TextRange::new(0, 15));
}
