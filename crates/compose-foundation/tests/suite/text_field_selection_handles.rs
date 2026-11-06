//! A field's selection handles under a finger (D2008, D2010), from Compose's
//! `TextFieldSelectionHandlesTest`.
//!
//! As Compose's tests: the test font at 10 sp, density 1, touch input mode.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{Density, InputMode, IntSize, Offset, TextRange, UnitExt};

use crate::text_field_touch_support::*;

const TAG: &str = "BasicTextField";
const FONT: f32 = 10.0;

fn state_with(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
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

/// A field `width` dp wide over `state`.
fn field(state: TextFieldState, width: f32) -> ComposeTestRule {
    touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(width.dp()),
            text_style = style(),
        );
    })
}

fn focus(ui: &mut ComposeTestRule) {
    ui.on_node_with_tag(TAG).request_focus();
    ui.wait_for_idle();
}

fn start_handle() -> Matcher {
    is_selection_handle(Some(Handle::SelectionStart))
}

fn end_handle() -> Matcher {
    is_selection_handle(Some(Handle::SelectionEnd))
}

fn assert_handles_shown(ui: &mut ComposeTestRule) {
    ui.on_node(start_handle()).assert_is_displayed();
    ui.on_node(end_handle()).assert_is_displayed();
}

fn assert_no_handles(ui: &mut ComposeTestRule) {
    ui.on_node(start_handle()).assert_does_not_exist();
    ui.on_node(end_handle()).assert_does_not_exist();
}

/// Drags `handle` sideways by the touch slop and `distance` more, as Compose's
/// `performHandleDrag`.
fn drag_handle(ui: &mut ComposeTestRule, handle: Handle, to_start: bool, distance: f32) {
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let slop = touch.view_configuration().touch_slop;
        let x = touch.center_x();
        if to_start {
            touch.swipe_left_with(x, x - slop - distance, 1000);
        } else {
            touch.swipe_right_with(x, x + slop + distance, 1000);
        }
    });
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_doNotShow_whenFieldNotFocused
#[test]
fn an_unfocused_field_shows_no_selection_handles() {
    let mut ui = field(state_with("hello, world", TextRange::new(2, 5)), 100.0);
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_doNotShow_inKeyboardInputMode
#[test]
fn a_field_focused_in_keyboard_mode_shows_no_selection_handles() {
    let mut ui = field(state_with("hello, world", TextRange::new(2, 5)), 100.0);
    ui.host().set_input_mode(InputMode::Keyboard);
    focus(&mut ui);
    assert_no_handles(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_haveMinimumTouchSizeArea
#[test]
fn the_selection_handles_are_at_least_40_dp_square() {
    let mut ui = field(state_with("hello, world", TextRange::new(2, 5)), 100.0);
    focus(&mut ui);
    for handle in [start_handle(), end_handle()] {
        let mut bottom_right = Offset::ZERO;
        ui.on_node(handle).perform_touch_input(|touch| bottom_right = touch.bottom_right());
        assert!((bottom_right.x - 40.0).abs() <= 1.0, "{bottom_right:?}");
        assert!((bottom_right.y - 40.0).abs() <= 1.0, "{bottom_right:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_appears_whenFieldGetsFocused
#[test]
fn a_selection_shows_its_handles_once_focused() {
    let mut ui = field(state_with("hello, world", TextRange::new(2, 5)), 100.0);
    focus(&mut ui);
    assert_handles_shown(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_disappear_whenFieldLosesFocus
#[test]
fn the_selection_handles_go_with_the_focus() {
    let state = state_with("hello, world", TextRange::new(2, 5));
    let requester = compose_ui::FocusRequester::new();
    let mut ui = touch_rule(move || {
        Column!({
            Box!(Modifier.size(100.dp()).focus_requester(requester).focusable());
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.test_tag(TAG).width(100.dp()),
                text_style = style(),
            );
        });
    });
    focus(&mut ui);
    assert_handles_shown(&mut ui);
    requester.request_focus();
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_locatedAtTheRightPosition_ltr_ltr
#[test]
fn the_selection_handles_point_at_the_ends_of_the_selection() {
    let mut ui = field(state_with("hello, world", TextRange::new(2, 5)), 100.0);
    focus(&mut ui);
    assert_handle_position(&mut ui, Handle::SelectionStart, 2.0 * FONT, FONT);
    assert_eq!(handle_info(&mut ui, Handle::SelectionStart).anchor, SelectionHandleAnchor::Left);
    assert_handle_position(&mut ui, Handle::SelectionEnd, 5.0 * FONT, FONT);
    assert_eq!(handle_info(&mut ui, Handle::SelectionEnd).anchor, SelectionHandleAnchor::Right);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_locatedAtTheRightPosition_ltr_rtl
#[test]
fn a_selection_ending_in_right_to_left_text_points_its_end_handle_left() {
    // "abc " then three Hebrew letters of two bytes each: Compose's (1, 6).
    let mut ui = field(state_with("abc \u{05D0}\u{05D1}\u{05D2}", TextRange::new(1, 8)), 100.0);
    focus(&mut ui);
    assert_handle_position(&mut ui, Handle::SelectionStart, FONT, FONT);
    assert_eq!(handle_info(&mut ui, Handle::SelectionStart).anchor, SelectionHandleAnchor::Left);
    assert_handle_position(&mut ui, Handle::SelectionEnd, 5.0 * FONT, FONT);
    assert_eq!(handle_info(&mut ui, Handle::SelectionEnd).anchor, SelectionHandleAnchor::Left);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragStartSelectionHandle_toExtendSelection
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragEndSelectionHandle_toExtendSelection
#[test]
fn dragging_a_selection_handle_moves_its_end() {
    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionStart, true, FONT * 4.0);
    assert_eq!(state.selection(), TextRange::new(0, 7));

    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionEnd, false, FONT * 4.0);
    assert_eq!(state.selection(), TextRange::new(4, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragStartSelectionHandlePastEndHandle_reversesTheSelection
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragEndSelectionHandlePastStartHandle_canReverseSelection
#[test]
fn dragging_a_selection_handle_past_the_other_reverses_the_selection() {
    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionStart, false, FONT * 7.0);
    assert_eq!(state.selection(), TextRange::new(11, 7));

    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionEnd, true, FONT * 7.0);
    assert_eq!(state.selection(), TextRange::new(4, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragStartSelectionHandle_extendsByWord
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragEndSelectionHandle_extendsByWord
#[test]
fn a_selection_handle_extends_by_words() {
    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionStart, true, FONT * 2.0);
    assert_eq!(state.selection(), TextRange::new(0, 7));

    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionEnd, false, FONT * 2.0);
    assert_eq!(state.selection(), TextRange::new(4, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragStartSelectionHandle_shrinksByCharacter
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragEndSelectionHandle_shrinksByCharacter
#[test]
fn a_selection_handle_shrinks_by_characters() {
    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionStart, false, FONT);
    assert_eq!(state.selection(), TextRange::new(5, 7));

    let state = state_with("abc def ghj", TextRange::new(4, 7));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    drag_handle(&mut ui, Handle::SelectionEnd, true, FONT);
    assert_eq!(state.selection(), TextRange::new(4, 6));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragEndSelectionHandle_outOfBounds_horizontally
#[test]
fn the_end_handle_dragged_a_character_at_a_time_past_the_field_reaches_the_end() {
    let text = "abc def ".repeat(10);
    let state = state_with(&text, TextRange::new(0, 3));
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = style(),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    });
    focus(&mut ui);
    for _ in 0..80 {
        drag_handle(&mut ui, Handle::SelectionEnd, false, FONT);
    }
    assert_eq!(state.selection(), TextRange::new(0, 80));
}
