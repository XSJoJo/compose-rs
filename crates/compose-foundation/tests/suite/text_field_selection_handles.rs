//! A field's selection handles under a finger (D2008, D2010), from Compose's
//! `TextFieldSelectionHandlesTest`.
//!
//! As Compose's tests: the test font at 10 sp, density 1, touch input mode.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{Density, InputMode, IntSize, Offset, Popup, TextRange, UnitExt};

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

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandles_appear_whenTextAlignedToEnd
#[test]
fn a_selection_in_text_aligned_to_the_end_shows_its_handles() {
    let state = state_with("hello", TextRange::new(0, 5));
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).fill_max_width(),
            text_style = TextStyle {
                text_align: Some(compose_ui::TextAlign::End),
                letter_spacing: 1.2.sp(),
                ..style()
            },
        );
    });
    focus(&mut ui);
    assert_handles_shown(&mut ui);
}

/// A field over `state`, 100 dp wide, under a focusable popup while `covered` (another window
/// taking the keys, as Compose's `isWindowFocused` going false).
fn coverable_field(
    state: TextFieldState,
    covered: compose_runtime::MutableState<bool>,
) -> ComposeTestRule {
    touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = style(),
        );
        if covered.get() {
            let properties = compose_ui::PopupProperties { focusable: true, ..Default::default() };
            Popup!(properties = properties, {
                Spacer(Modifier.size(10.dp()));
            });
        }
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, textField_noSelectionHandles_whenWindowLosesFocus
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, textField_redisplaysSelectionHandles_whenWindowRegainsFocus
#[test]
fn the_selection_handles_hide_while_another_window_takes_the_keys() {
    let state = state_with("hello, world", TextRange::new(2, 5));
    let covered = Box::leak(Box::new(compose_runtime::mutable_state_of(false))).handle();
    let mut ui = coverable_field(state, covered);
    focus(&mut ui);
    assert_handles_shown(&mut ui);
    covered.set(true);
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
    covered.set(false);
    ui.wait_for_idle();
    assert_handles_shown(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, textField_redisplaysSelectionHandles_whenTextFieldStateChanges
#[test]
fn a_new_state_hides_the_handles_until_a_long_press_selects() {
    let first = state_with("hello, world", TextRange::new(2, 5));
    let current = Box::leak(Box::new(compose_runtime::mutable_state_of(first))).handle();
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            current.get(),
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = style(),
        );
    });
    focus(&mut ui);
    assert_handles_shown(&mut ui);
    current.set(state_with("hello", TextRange::at(0)));
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(Offset::new(FONT * 2.0, FONT / 2.0), duration);
    });
    ui.wait_for_idle();
    assert_handles_shown(&mut ui);
}

/// "hello " ten times from 1 to 2, on one line 100 dp wide or on two lines at most.
fn scrollable_field(single_line: bool) -> (ComposeTestRule, TextFieldState) {
    let state = state_with(&"hello ".repeat(10), TextRange::new(1, 2));
    let ui = touch_rule(move || {
        let line_limits = if single_line {
            TextFieldLineLimits::SingleLine
        } else {
            TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 2 }
        };
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = style(),
            line_limits = line_limits,
        );
    });
    (ui, state)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandlesDisappear_whenScrolledOutOfView_horizontally
#[test]
fn the_selection_handles_hide_while_the_line_scrolls_them_out_of_view() {
    let (mut ui, state) = scrollable_field(true);
    focus(&mut ui);
    assert_handles_shown(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.swipe_left());
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
    assert_eq!(state.selection(), TextRange::new(1, 2));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.swipe_right());
    ui.wait_for_idle();
    assert_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(1, 2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandlesDisappear_whenScrolledOutOfView_vertically
#[test]
fn the_selection_handles_hide_while_the_lines_scroll_them_out_of_view() {
    let (mut ui, state) = scrollable_field(false);
    focus(&mut ui);
    assert_handles_shown(&mut ui);
    // Compose's test injects its events into the field's window, past the handles' popups (windows
    // of their own) under the field's middle; here a popup over the field takes them, so the
    // swipes go along the field's right side, beside the handles.
    let side = |touch: &TouchInjectionScope<'_>| touch.right() - 1.0;
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (x, bottom) = (side(touch), touch.bottom());
        touch.swipe(Offset::new(x, bottom), Offset::new(x, 0.0), 200);
    });
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
    assert_eq!(state.selection(), TextRange::new(1, 2));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (x, bottom) = (side(touch), touch.bottom());
        touch.swipe(Offset::new(x, 0.0), Offset::new(x, bottom), 200);
    });
    ui.wait_for_idle();
    assert_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(1, 2));
}

const CONTAINER: &str = "container";

/// "hello" from 1 to 2 in a field at the start of a container scrolling `horizontally` or not.
fn field_in_scrolling_container(horizontally: bool) -> (ComposeTestRule, TextFieldState) {
    let state = state_with("hello", TextRange::new(1, 2));
    let ui = touch_rule(move || {
        let scroll = remember_scroll_state(0);
        if horizontally {
            let modifier = Modifier.width(200.dp()).horizontal_scroll(scroll).test_tag(CONTAINER);
            Row!(modifier = modifier, {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.test_tag(TAG).width(100.dp()),
                    text_style = style(),
                );
                Box!(Modifier.height(12.dp()).width(400.dp()));
            });
        } else {
            let modifier = Modifier.height(200.dp()).vertical_scroll(scroll).test_tag(CONTAINER);
            Column!(modifier = modifier, {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.test_tag(TAG).height(100.dp()),
                    text_style = style(),
                );
                Box!(Modifier.width(12.dp()).height(400.dp()));
            });
        }
    });
    (ui, state)
}

fn scrolled_out_of_its_container(horizontally: bool) {
    let (mut ui, state) = field_in_scrolling_container(horizontally);
    focus(&mut ui);
    assert_handles_shown(&mut ui);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| {
        if horizontally { touch.swipe_left() } else { touch.swipe_up() }
    });
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
    assert_eq!(state.selection(), TextRange::new(1, 2));
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| {
        if horizontally { touch.swipe_right() } else { touch.swipe_down() }
    });
    ui.wait_for_idle();
    assert_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(1, 2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandlesDisappear_whenScrolledOutOfView_horizontally_inContainer
#[test]
fn the_selection_handles_hide_while_a_row_scrolls_the_field_out_of_view() {
    scrolled_out_of_its_container(true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandlesDisappear_whenScrolledOutOfView_vertically_inContainer
#[test]
fn the_selection_handles_hide_while_a_column_scrolls_the_field_out_of_view() {
    scrolled_out_of_its_container(false);
}

/// A finger's double tap at `at`, as Compose's `doubleClick(position)`.
fn double_tap(ui: &mut ComposeTestRule, at: Offset) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let configuration = touch.view_configuration();
        let delay = (configuration.double_tap_min_time_millis
            + configuration.double_tap_timeout_millis)
            / 2;
        touch.double_click_at(at, delay);
    });
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, doubleClickOnWord_toSelectWord
#[test]
fn a_double_tap_on_a_word_selects_it() {
    let state = state_with("abc def ghj", TextRange::at(11));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    double_tap(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0));
    assert_eq!(state.selection(), TextRange::new(4, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, doubleClickOnWhitespace_doesNotSelectWhitespace
#[test]
fn a_double_tap_on_a_space_selects_a_word_not_the_space() {
    let state = state_with("abc def ghj", TextRange::at(11));
    let mut ui = field(state, 200.0);
    focus(&mut ui);
    double_tap(&mut ui, Offset::new(FONT * 3.5, FONT / 2.0));
    assert_ne!(state.selection(), TextRange::new(3, 4));
    assert!(!state.selection().collapsed());
}

/// Drags `handle` up or down by the touch slop and `distance` more, as Compose's
/// `performHandleDrag` along the vertical.
fn drag_handle_vertically(ui: &mut ComposeTestRule, handle: Handle, up: bool, distance: f32) {
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let slop = touch.view_configuration().touch_slop;
        let y = touch.center_y();
        if up {
            touch.swipe_up_with(y, y - slop - distance, 1000);
        } else {
            touch.swipe_down_with(y, y + slop + distance, 1000);
        }
    });
    ui.wait_for_idle();
}

/// "abc def " ten times from 77 to 80, scrolled to its end by its scroll state, focused.
fn field_scrolled_to_the_end(
    width: f32,
    line_limits: TextFieldLineLimits,
) -> (ComposeTestRule, TextFieldState, ScrollState) {
    let state = state_with(&"abc def ".repeat(10), TextRange::new(77, 80));
    let scroll = **Box::leak(Box::new(ScrollState::new(0)));
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(width.dp()),
            text_style = style(),
            line_limits = line_limits,
            scroll_state = Some(scroll),
        );
    });
    ui.wait_for_idle();
    crate::scroll_support::run(&mut ui, async move {
        let _ = scroll.scroll_to(scroll.max_value()).await;
    });
    focus(&mut ui);
    (ui, state, scroll)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragStartSelectionHandle_outOfBounds_horizontally
#[test]
fn the_start_handle_dragged_a_character_at_a_time_past_the_field_reaches_the_start() {
    let (mut ui, state, _) = field_scrolled_to_the_end(100.0, TextFieldLineLimits::SingleLine);
    for _ in 0..80 {
        drag_handle(&mut ui, Handle::SelectionStart, true, FONT);
    }
    assert_eq!(state.selection(), TextRange::new(0, 80));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragStartSelectionHandle_outOfBounds_vertically
#[test]
fn the_start_handle_dragged_up_past_the_field_reaches_the_start() {
    let lines = TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 3 };
    let (mut ui, state, scroll) = field_scrolled_to_the_end(50.0, lines);
    let up = scroll.max_value() as f32 * 2.0;
    drag_handle_vertically(&mut ui, Handle::SelectionStart, true, up);
    // And to the first line's start.
    drag_handle(&mut ui, Handle::SelectionStart, true, FONT * 10.0);
    assert_eq!(state.selection(), TextRange::new(0, 80));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, dragEndSelectionHandle_outOfBounds_vertically
#[test]
fn the_end_handle_dragged_down_past_the_field_reaches_the_end() {
    let state = state_with(&"abc def ".repeat(10), TextRange::new(0, 3));
    let size: &'static std::cell::Cell<compose_ui::IntSize> =
        Box::leak(Box::new(std::cell::Cell::new(compose_ui::IntSize::ZERO)));
    let mut ui = touch_rule(move || {
        let lines =
            TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 3 };
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = style(),
            line_limits = lines,
            on_text_layout =
                Some(move |layout: compose_ui::TextLayoutResult| size.set(layout.size)),
        );
    });
    focus(&mut ui);
    let layout = size.get();
    drag_handle_vertically(&mut ui, Handle::SelectionEnd, false, layout.height as f32);
    drag_handle(&mut ui, Handle::SelectionEnd, false, layout.width as f32);
    assert_eq!(state.selection(), TextRange::new(0, 80));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldSelectionHandlesTest.kt, selectionHandlesHide_whenHardwareKeyboardIsUsed_thenComesBackWithTouch
#[test]
fn a_hardware_key_hides_the_selection_handles_until_a_finger_comes_back() {
    let state = state_with("hello", TextRange::at(5));
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG), text_style = style());
    });
    focus(&mut ui);
    let long_click = |ui: &mut ComposeTestRule| {
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
            touch.advance_event_time(1000);
            let duration = touch.view_configuration().long_press_timeout_millis + 100;
            touch.long_click_at(Offset::new(FONT * 2.0, FONT / 2.0), duration);
        });
        ui.wait_for_idle();
    };
    long_click(&mut ui);
    assert_handles_shown(&mut ui);
    // Everything selected from the keyboard: Ctrl+A (Cmd+A on macOS).
    let modifier = if cfg!(target_os = "macos") {
        compose_ui::Key::MetaLeft
    } else {
        compose_ui::Key::CtrlLeft
    };
    ui.on_node_with_tag(TAG).perform_key_input(|keys| {
        keys.with_key_down(modifier, |keys| keys.press_key(compose_ui::Key::Character('a')));
    });
    ui.wait_for_idle();
    assert_no_handles(&mut ui);
    assert_eq!(state.selection(), TextRange::new(0, 5));
    long_click(&mut ui);
    assert_handles_shown(&mut ui);
}
