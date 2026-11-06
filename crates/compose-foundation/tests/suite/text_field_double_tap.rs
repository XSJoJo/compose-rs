//! A finger's double tap on a field, from Compose's `TextFieldDoubleTapTest`: the first tap puts
//! the cursor (and its handle), the second selects the word with its handles; a drag after it
//! extends by words, and a handle scrolled out of view while dragging doesn't show.
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

/// Two taps at `at`, the double-tap minimum apart, the second held unless `lift`, as Compose's
/// `doubleTap(offset, liftPointer)`.
fn double_tap(touch: &mut TouchInjectionScope<'_>, at: Offset, lift: bool) {
    touch.down(at);
    touch.move_to(at);
    touch.up();
    let wait = touch.view_configuration().double_tap_min_time_millis + 1;
    touch.advance_event_time(wait);
    touch.down(at);
    touch.move_to(at);
    if lift {
        touch.up();
    }
}

fn assert_selection_handles_shown(ui: &mut ComposeTestRule) {
    ui.on_node(is_selection_handle(Some(Handle::SelectionStart))).assert_is_displayed();
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).assert_is_displayed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, emptyTextField_doubleTapDoesNotShowCursor
#[test]
fn a_double_tap_on_an_empty_field_shows_no_cursor_handle() {
    let mut ui = field(state_of(""));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let center = touch.center();
        double_tap(touch, center, true);
    });
    ui.wait_for_idle();
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTap_firstPlacesTheCursor_thenSelectsWord
#[test]
fn a_double_tap_puts_the_cursor_then_selects_the_word() {
    let state = state_of("abc def ghi");
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
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTap_emptySpace_hidesTheCursorHandle
#[test]
fn a_double_tap_between_words_hides_the_cursor_handle() {
    let state = state_of("abc  def");
    let mut ui = field(state);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        double_tap(touch, Offset::new(FONT * 4.0, FONT / 2.0), true);
    });
    ui.wait_for_idle();
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_is_not_displayed();
    assert_eq!(state.selection(), TextRange::at(4));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTap_scrolledTextField_selectsWord
#[test]
fn a_double_tap_on_a_scrolled_field_selects_the_word_under_it() {
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
        double_tap(touch, at, true);
    });
    ui.wait_for_idle();
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(20, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTap_decoratedTextField_selectsWord
#[test]
fn a_double_tap_in_a_decorated_field_selects_the_word_under_it() {
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
        double_tap(touch, Offset::new(32.0 + FONT * 5.0, 32.0 + FONT / 2.0), true);
    });
    ui.wait_for_idle();
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(4, 7));
}

/// A field of "abc def ghi " ten times, 100 dp wide, on one line or on one to three, tapped.
fn scrolling_field(single_line: bool) -> ComposeTestRule {
    let state = state_of(&"abc def ghi ".repeat(10));
    let mut ui = touch_rule(move || {
        let line_limits = if single_line {
            TextFieldLineLimits::SingleLine
        } else {
            TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 3 }
        };
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = style(),
            line_limits = line_limits,
        );
    });
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click());
    ui.wait_for_idle();
    ui
}

/// `scrolling_field` swiped to its end: on one line to its right, else to its bottom. Compose's
/// test injects its events into the field's window, past the cursor handle the tap shows (a
/// popup); here a popup over the field takes them, so the swipe up goes along the field's left
/// side, beside the handle under the middle (D2017).
fn field_scrolled_to_its_end(single_line: bool) -> ComposeTestRule {
    let mut ui = scrolling_field(single_line);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let wait = touch.view_configuration().double_tap_timeout_millis * 2;
        touch.advance_event_time(wait);
        if single_line {
            let right = touch.right();
            touch.swipe_left_with(right, -10000.0, 200);
        } else {
            let bottom = touch.bottom();
            touch.swipe(Offset::new(1.0, bottom), Offset::new(1.0, -10000.0), 200);
        }
    });
    ui.wait_for_idle();
    ui
}

/// A double tap at `at` held, then a move by `by`.
fn double_tap_and_hold(
    ui: &mut ComposeTestRule,
    at: impl Fn(&TouchInjectionScope<'_>) -> Offset,
    by: Offset,
) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let wait = touch.view_configuration().double_tap_timeout_millis * 2;
        touch.advance_event_time(wait);
        let at = at(touch);
        double_tap(touch, at, false);
        touch.move_by(by);
    });
    ui.wait_for_idle();
}

fn lift_after(ui: &mut ComposeTestRule, by: Offset) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.move_by(by);
        touch.up();
    });
    ui.wait_for_idle();
}

/// Which handles show: the start's and the end's.
#[track_caller]
fn assert_handles(ui: &mut ComposeTestRule, start: bool, end: bool) {
    for (handle, shown) in [(Handle::SelectionStart, start), (Handle::SelectionEnd, end)] {
        let mut node = ui.on_node(is_selection_handle(Some(handle)));
        if shown {
            node.assert_is_displayed();
        } else {
            node.assert_does_not_exist();
        }
    }
}

fn near_start(_: &TouchInjectionScope<'_>) -> Offset {
    Offset::new(FONT, FONT / 2.0)
}

fn near_right(touch: &TouchInjectionScope<'_>) -> Offset {
    Offset::new(touch.right() - 1.0, touch.center_y())
}

fn near_bottom(touch: &TouchInjectionScope<'_>) -> Offset {
    Offset::new(touch.center_x(), touch.bottom() - 1.0)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollRight_startHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_right_until_the_field_scrolls_hides_the_start_handle() {
    let mut ui = scrolling_field(true);
    double_tap_and_hold(&mut ui, near_start, Offset::new(FONT * 30.0, 0.0));
    lift_after(&mut ui, Offset::ZERO);
    assert_handles(&mut ui, false, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollRightThenSlightlyBack_startHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_right_then_slightly_back_hides_the_start_handle() {
    let mut ui = scrolling_field(true);
    double_tap_and_hold(&mut ui, near_start, Offset::new(FONT * 30.0, 0.0));
    assert_handles(&mut ui, false, false);
    lift_after(&mut ui, Offset::new(-FONT, 0.0));
    assert_handles(&mut ui, false, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollDown_startHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_down_until_the_field_scrolls_hides_the_start_handle() {
    let mut ui = scrolling_field(false);
    double_tap_and_hold(&mut ui, near_start, Offset::new(0.0, FONT * 30.0));
    lift_after(&mut ui, Offset::ZERO);
    assert_handles(&mut ui, false, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollDownThenSlightlyBack_startHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_down_then_slightly_back_hides_the_start_handle() {
    let mut ui = scrolling_field(false);
    double_tap_and_hold(&mut ui, near_start, Offset::new(0.0, FONT * 30.0));
    assert_handles(&mut ui, false, false);
    lift_after(&mut ui, Offset::new(0.0, -FONT));
    assert_handles(&mut ui, false, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollLeft_endHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_left_until_the_field_scrolls_hides_the_end_handle() {
    let mut ui = field_scrolled_to_its_end(true);
    double_tap_and_hold(&mut ui, near_right, Offset::new(-FONT * 30.0, 0.0));
    lift_after(&mut ui, Offset::ZERO);
    assert_handles(&mut ui, true, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollLeftThenSlightlyForward_endHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_left_then_slightly_forward_hides_the_end_handle() {
    let mut ui = field_scrolled_to_its_end(true);
    double_tap_and_hold(&mut ui, near_right, Offset::new(-FONT * 30.0, 0.0));
    assert_handles(&mut ui, false, false);
    lift_after(&mut ui, Offset::new(FONT, 0.0));
    assert_handles(&mut ui, true, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollUp_endHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_up_until_the_field_scrolls_hides_the_end_handle() {
    let mut ui = field_scrolled_to_its_end(false);
    double_tap_and_hold(&mut ui, near_bottom, Offset::new(0.0, -FONT * 30.0));
    lift_after(&mut ui, Offset::ZERO);
    assert_handles(&mut ui, true, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startDraggingToScrollUpThenSlightlyForward_endHandleDoesNotShow_ltr
#[test]
fn a_double_tap_dragged_up_then_slightly_forward_hides_the_end_handle() {
    let mut ui = field_scrolled_to_its_end(false);
    double_tap_and_hold(&mut ui, near_bottom, Offset::new(0.0, -FONT * 30.0));
    assert_handles(&mut ui, false, false);
    lift_after(&mut ui, Offset::new(0.0, FONT));
    assert_handles(&mut ui, true, false);
}

/// A field over `state` with `modifier`, its text in `style`.
fn field_with(state: TextFieldState, modifier: Modifier, style: TextStyle) -> ComposeTestRule {
    touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).then(modifier.clone()),
            text_style = style.clone(),
        );
    })
}

/// A double tap at `at` held, then a move by `by`, then lifted.
fn double_tap_and_drag(ui: &mut ComposeTestRule, at: Offset, by: Offset) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        double_tap(touch, at, false);
        touch.move_by(by);
        touch.up();
    });
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragToRight_selectsCurrentAndNextWord_ltr
#[test]
fn a_drag_right_after_a_double_tap_selects_the_next_word_too() {
    let state = state_of("abc def ghi");
    let mut ui = field(state);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), Offset::new(FONT * 5.0, 0.0));
    assert_eq!(state.selection(), TextRange::new(4, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragToLeft_selectsCurrentAndPreviousWord_ltr
#[test]
fn a_drag_left_after_a_double_tap_selects_the_previous_word_too() {
    let state = state_of("abc def ghi");
    let mut ui = field(state);
    let by = Offset::new(-FONT * 4.0, 0.0);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), by);
    assert_eq!(state.selection(), TextRange::new(0, 7));
}

const THREE_LINES: &str = "abc def\nabc def\nabc def";

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragDown_selectsFromCurrentToTargetWord_ltr
#[test]
fn a_drag_down_after_a_double_tap_selects_to_the_word_below() {
    let state = state_of(THREE_LINES);
    let mut ui = field(state);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), Offset::new(0.0, FONT * 2.0));
    assert_eq!(state.selection(), TextRange::new(4, 15));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragUp_selectsFromCurrentToTargetWord_ltr
#[test]
fn a_drag_up_after_a_double_tap_selects_from_the_word_above() {
    let state = state_of(THREE_LINES);
    let mut ui = field(state);
    // The second line's "def".
    let by = Offset::new(0.0, -FONT * 2.0);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT * 1.5), by);
    assert_eq!(state.selection(), TextRange::new(4, 15));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startingFromEndPadding_dragToLeft_selectsLastWord_ltr
#[test]
fn a_double_tap_past_the_text_dragged_onto_it_selects_its_last_word() {
    let state = state_of("abc def");
    let mut ui = field_with(state, Modifier.width(100.dp()), style());
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let at = touch.center_right();
        double_tap(touch, at, false);
        touch.move_to(Offset::new(FONT * 5.0, FONT / 2.0));
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(4, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startingFromEndPadding_draggingUp_selectsFromLastWord_ltr
#[test]
fn a_double_tap_past_the_last_line_dragged_up_selects_from_its_last_word() {
    let state = state_of("abc def\nghi jkl\nmno pqr");
    // Compose's `TextStyle()`: the default size.
    let mut ui = field_with(state, Modifier.width(200.dp()), TextStyle::DEFAULT);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (bottom_right, top_right) = (touch.bottom_right(), touch.top_right());
        double_tap(touch, bottom_right, false);
        for _ in 0..(bottom_right - top_right).y.round() as i32 {
            touch.move_by(Offset::new(0.0, -1.0));
        }
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(4, 23));
}

/// Compose's `rtlText2` and `rtlText3`.
const RTL2: &str = "\u{05D0}\u{05D1}\u{05D2} \u{05D3}\u{05D4}\u{05D5}";
const RTL3: &str = "\u{05D0}\u{05D1}\u{05D2} \u{05D3}\u{05D4}\u{05D5} \u{05D6}\u{05D7}\u{05D8}";

/// Compose's range of characters `start` to `end` of `text`, in its byte offsets.
fn chars(text: &str, start: usize, end: usize) -> TextRange {
    let byte = |n: usize| text.char_indices().nth(n).map_or(text.len(), |(at, _)| at);
    TextRange::new(byte(start), byte(end))
}

fn rtl_lines() -> String {
    format!("{RTL2}\n{RTL2}\n{RTL2}")
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragToRight_selectsCurrentAndPreviousWord_rtl
#[test]
fn a_drag_right_after_a_double_tap_on_right_to_left_text_selects_the_previous_word_too() {
    let state = state_of(RTL3);
    let mut ui = field(state);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), Offset::new(FONT * 4.0, 0.0));
    assert_eq!(state.selection(), chars(RTL3, 0, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragToLeft_selectsCurrentAndNextWord_rtl
#[test]
fn a_drag_left_after_a_double_tap_on_right_to_left_text_selects_the_next_word_too() {
    let state = state_of(RTL3);
    let mut ui = field(state);
    let by = Offset::new(-FONT * 3.0, 0.0);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), by);
    assert_eq!(state.selection(), chars(RTL3, 4, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragDown_selectsFromCurrentToTargetWord_rtl
#[test]
fn a_drag_down_after_a_double_tap_on_right_to_left_text_selects_to_the_word_below() {
    let text = rtl_lines();
    let state = state_of(&text);
    let mut ui = field(state);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), Offset::new(0.0, FONT * 2.0));
    assert_eq!(state.selection(), chars(&text, 0, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_dragUp_selectsFromCurrentToTargetWord_rtl
#[test]
fn a_drag_up_after_a_double_tap_on_right_to_left_text_selects_from_the_word_above() {
    let text = rtl_lines();
    let state = state_of(&text);
    let mut ui = field(state);
    let by = Offset::new(0.0, -FONT * 2.0);
    double_tap_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT * 1.5), by);
    assert_eq!(state.selection(), chars(&text, 0, 11));
}

/// A field in a right-to-left layout, `width` dp wide, its text in `style`.
fn rtl_field(state: TextFieldState, width: f32, style: TextStyle) -> ComposeTestRule {
    touch_rule(move || {
        let style = style.clone();
        compose_runtime::CompositionLocalProvider(
            compose_ui::LOCAL_LAYOUT_DIRECTION.provides(compose_ui::LayoutDirection::Rtl),
            move || {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.test_tag(TAG).width(width.dp()),
                    text_style = style.clone(),
                );
            },
        );
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startingFromEndPadding_dragToRight_selectsLastWord_rtl
#[test]
fn a_double_tap_past_right_to_left_text_dragged_onto_it_selects_its_last_word() {
    let state = state_of(RTL2);
    let mut ui = rtl_field(state, 100.0, style());
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let at = touch.center_left();
        double_tap(touch, at, false);
        touch.move_to(Offset::new(FONT * 5.0, FONT / 2.0));
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), chars(RTL2, 4, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldDoubleTapTest.kt, doubleTapThen_startingFromEndPadding_draggingUp_selectsFromLastWord_rtl
#[test]
fn a_double_tap_past_the_last_right_to_left_line_dragged_up_selects_from_its_last_word() {
    let text = rtl_lines();
    let state = state_of(&text);
    // Compose's `TextStyle()`: the default size.
    let mut ui = rtl_field(state, 200.0, TextStyle::DEFAULT);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (bottom_left, top_left) = (touch.bottom_left(), touch.top_left());
        double_tap(touch, bottom_left, false);
        for _ in 0..(bottom_left - top_left).y.round() as i32 {
            touch.move_by(Offset::new(0.0, -1.0));
        }
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), chars(&text, 4, 23));
}
