//! A finger's long press on a field (D2010), from Compose's `TextFieldLongPressTest`: it selects
//! the word under it, or puts the cursor at the end of an empty region, and a drag after it
//! extends the selection by words; the field is focused once the finger lifts.
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

fn field(state: TextFieldState, modifier: Modifier) -> ComposeTestRule {
    touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).then(modifier.clone()),
            text_style = style(),
        );
    })
}

/// A finger held at `at` past the long-press timeout, not lifted, as Compose's `longPress`.
fn long_press(touch: &mut TouchInjectionScope<'_>, at: Offset) {
    let timeout = touch.view_configuration().long_press_timeout_millis;
    touch.down(at);
    touch.advance_event_time(timeout + 100);
    touch.move_to(at);
}

fn long_click(ui: &mut ComposeTestRule, at: Offset) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(at, duration);
    });
    ui.wait_for_idle();
}

fn assert_selection_handles_shown(ui: &mut ComposeTestRule) {
    ui.on_node(is_selection_handle(Some(Handle::SelectionStart))).assert_is_displayed();
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).assert_is_displayed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, emptyTextField_longPressDoesNotShowCursor
#[test]
fn a_long_press_on_an_empty_field_shows_no_cursor_handle() {
    let state = state_of("");
    let mut ui = field(state, Modifier);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.long_click());
    ui.wait_for_idle();
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_doesNotRequestsFocus_beforePointerIsReleased
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_requestsFocus_afterPointerIsReleased
#[test]
fn a_long_press_selects_at_once_and_focuses_when_lifted() {
    let state = state_of("abc def ghi");
    let mut ui = field(state, Modifier);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| long_press(touch, touch.center()));
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_is_not_focused();
    ui.on_node(is_selection_handle(Some(Handle::SelectionStart))).assert_does_not_exist();
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).assert_does_not_exist();
    assert_eq!(state.selection(), TextRange::new(4, 7));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_is_focused();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPressOnEmptyRegion_showsCursorAtTheEnd
#[test]
fn a_long_press_past_the_text_puts_the_cursor_at_its_end() {
    let state = state_of("abc");
    let mut ui = field(state, Modifier.width(100.dp()));
    long_click(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0));
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_is_displayed();
    assert_eq!(state.selection(), TextRange::at(3));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPressOnWord_selectsWord
#[test]
fn a_long_press_on_a_word_selects_it() {
    let state = state_of("abc def ghi");
    let mut ui = field(state, Modifier);
    long_click(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0));
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(4, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPressOnWhitespace_doesNotSelectWhitespace
#[test]
fn a_long_press_on_a_space_selects_a_word_not_the_space() {
    let state = state_of("abc def ghi");
    let mut ui = field(state, Modifier);
    long_click(&mut ui, Offset::new(FONT * 7.5, FONT / 2.0));
    assert_selection_handles_shown(&mut ui);
    assert_ne!(state.selection(), TextRange::new(7, 8));
    assert!(!state.selection().collapsed());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPressOnDecoratedTextField_selectsWord
#[test]
fn a_long_press_in_a_decorated_field_selects_the_word_under_it() {
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
    long_click(&mut ui, Offset::new(32.0 + FONT * 5.0, 32.0 + FONT / 2.0));
    assert_selection_handles_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::new(4, 7));
}

/// A long press at `at`, then a move by `by`, then lifted.
fn long_press_and_drag(ui: &mut ComposeTestRule, at: Offset, by: Offset) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        long_press(touch, at);
        touch.move_by(by);
        touch.up();
    });
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragToRight_selectsCurrentAndNextWord_ltr
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragToLeft_selectsCurrentAndPreviousWord_ltr
#[test]
fn a_drag_after_a_long_press_extends_by_words() {
    let state = state_of("abc def ghi");
    let mut ui = field(state, Modifier);
    long_press_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), Offset::new(FONT * 3.0, 0.0));
    assert_eq!(state.selection(), TextRange::new(4, 11));

    let state = state_of("abc def ghi");
    let mut ui = field(state, Modifier);
    long_press_and_drag(
        &mut ui,
        Offset::new(FONT * 5.0, FONT / 2.0),
        Offset::new(-FONT * 3.0, 0.0),
    );
    assert_eq!(state.selection(), TextRange::new(0, 7));
}

/// A finger that moved past the touch slop before the long-press timeout isn't a long press,
/// however long it's held after, as Compose's `awaitLongPressOrCancellation`.
#[test]
fn a_finger_moved_past_the_slop_is_no_long_press() {
    let state = state_of("abc def ghi");
    let mut ui = field(state, Modifier);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        let slop = touch.view_configuration().touch_slop;
        touch.down(Offset::new(FONT * 2.0, FONT / 2.0));
        touch.move_by(Offset::new(slop + 5.0, 0.0));
        touch.advance_event_time(timeout + 100);
        touch.move_by(Offset::new(0.0, 1.0));
    });
    ui.wait_for_idle();
    assert!(state.selection().collapsed(), "{:?}", state.selection());
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.up());
}

const THREE_LINES: &str = "abc def\nabc def\nabc def";

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragDown_selectsFromCurrentToTargetWord_ltr
#[test]
fn a_drag_down_after_a_long_press_selects_to_the_word_below() {
    let state = state_of(THREE_LINES);
    let mut ui = field(state, Modifier);
    long_press_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), Offset::new(0.0, FONT));
    assert_eq!(state.selection(), TextRange::new(4, 15));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragUp_selectsFromCurrentToTargetWord_ltr
#[test]
fn a_drag_up_after_a_long_press_selects_from_the_word_above() {
    let state = state_of(THREE_LINES);
    let mut ui = field(state, Modifier);
    // The second line's "def".
    long_press_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT * 1.5), Offset::new(0.0, -FONT));
    assert_eq!(state.selection(), TextRange::new(4, 15));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_startingFromEndPadding_dragToLeft_selectsLastWord_ltr
#[test]
fn a_long_press_past_the_text_dragged_onto_it_selects_its_last_word() {
    let state = state_of("abc def");
    let mut ui = field(state, Modifier.width(100.dp()));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let at = touch.center_right();
        long_press(touch, at);
        touch.move_to(Offset::new(FONT * 5.0, FONT / 2.0));
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(4, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_startingFromEndPadding_draggingUp_selectsFromLastWord_ltr
#[test]
fn a_long_press_past_the_last_line_dragged_up_selects_from_its_last_word() {
    let state = state_of("abc def\nghi jkl\nmno pqr");
    let mut ui = touch_rule(move || {
        // Compose's `TextStyle()`: the default size.
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG).width(200.dp()));
    });
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (bottom_right, top_right) = (touch.bottom_right(), touch.top_right());
        long_press(touch, bottom_right);
        for _ in 0..(bottom_right - top_right).y.round() as i32 {
            touch.move_by(Offset::new(0.0, -1.0));
        }
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(4, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, deleteWhileDraggingSelection
#[test]
fn a_deletion_while_a_mouse_drags_a_selection_keeps_the_drag_going() {
    let state = state_of("Hello");
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG));
    });
    let mut results = Vec::new();
    ui.on_node_with_tag(TAG).perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    let layout = results.pop().expect("a layout");
    let height = layout.size.height as f32;
    let at =
        |offset: usize| Offset::new(layout.get_horizontal_position(offset, true), height / 2.0);
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        mouse.move_to(at(5));
        mouse.press(MouseButton::Primary);
        mouse.move_to(at(4));
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(5, 4));
    ui.on_node_with_tag(TAG).perform_key_input(|keys| keys.press_key(compose_ui::Key::Backspace));
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(4, 4));
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.move_to(at(3)));
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(4, 3));
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.release(MouseButton::Primary));
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

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragToRight_selectsCurrentAndPreviousWord_rtl
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragToLeft_selectsCurrentAndNextWord_rtl
#[test]
fn a_drag_after_a_long_press_on_right_to_left_text_extends_by_words() {
    let at = Offset::new(FONT * 5.0, FONT / 2.0);
    let state = state_of(RTL3);
    let mut ui = field(state, Modifier);
    long_press_and_drag(&mut ui, at, Offset::new(FONT * 3.0, 0.0));
    assert_eq!(state.selection(), chars(RTL3, 0, 7));

    let state = state_of(RTL3);
    let mut ui = field(state, Modifier);
    long_press_and_drag(&mut ui, at, Offset::new(-FONT * 3.0, 0.0));
    assert_eq!(state.selection(), chars(RTL3, 4, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragDown_selectsFromCurrentToTargetWord_rtl
#[test]
fn a_drag_down_after_a_long_press_on_right_to_left_text_selects_to_the_word_below() {
    let text = rtl_lines();
    let state = state_of(&text);
    let mut ui = field(state, Modifier);
    long_press_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT / 2.0), Offset::new(0.0, FONT));
    assert_eq!(state.selection(), chars(&text, 0, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_dragUp_selectsFromCurrentToTargetWord_rtl
#[test]
fn a_drag_up_after_a_long_press_on_right_to_left_text_selects_from_the_word_above() {
    let text = rtl_lines();
    let state = state_of(&text);
    let mut ui = field(state, Modifier);
    long_press_and_drag(&mut ui, Offset::new(FONT * 5.0, FONT * 1.5), Offset::new(0.0, -FONT));
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

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_startingFromEndPadding_dragToRight_selectsLastWord_rtl
#[test]
fn a_long_press_past_right_to_left_text_dragged_onto_it_selects_its_last_word() {
    let state = state_of(RTL2);
    let mut ui = rtl_field(state, 100.0, style());
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let at = touch.center_left();
        long_press(touch, at);
        touch.move_to(Offset::new(FONT * 5.0, FONT / 2.0));
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), chars(RTL2, 4, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_startingFromEndPadding_draggingUp_selectsFromLastWord_rtl
#[test]
fn a_long_press_past_the_last_right_to_left_line_dragged_up_selects_from_its_last_word() {
    let text = rtl_lines();
    let state = state_of(&text);
    // Compose's `TextStyle()`: the default size.
    let mut ui = rtl_field(state, 200.0, TextStyle::DEFAULT);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (bottom_left, top_left) = (touch.bottom_left(), touch.top_left());
        long_press(touch, bottom_left);
        for _ in 0..(bottom_left - top_left).y.round() as i32 {
            touch.move_by(Offset::new(0.0, -1.0));
        }
        touch.up();
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), chars(&text, 4, 23));
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

/// A long press at `at`, then a move by `by`, the finger held.
fn long_press_and_hold(
    ui: &mut ComposeTestRule,
    at: impl Fn(&TouchInjectionScope<'_>) -> Offset,
    by: Offset,
) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let wait = touch.view_configuration().double_tap_timeout_millis * 2;
        touch.advance_event_time(wait);
        let at = at(touch);
        long_press(touch, at);
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

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_startDraggingToScrollRight_startHandleDoesNotShow_ltr
#[test]
fn a_long_press_dragged_right_until_the_field_scrolls_hides_the_start_handle() {
    let mut ui = scrolling_field(true);
    long_press_and_hold(&mut ui, near_start, Offset::new(FONT * 30.0, 0.0));
    lift_after(&mut ui, Offset::ZERO);
    assert_handles(&mut ui, false, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldLongPressTest.kt, longPress_startDraggingToScrollRightThenSlightlyBack_startHandleDoesNotShow_ltr
#[test]
fn a_long_press_dragged_right_then_slightly_back_hides_the_start_handle() {
    let mut ui = scrolling_field(true);
    long_press_and_hold(&mut ui, near_start, Offset::new(FONT * 30.0, 0.0));
    assert_handles(&mut ui, false, false);
    lift_after(&mut ui, Offset::new(-FONT, 0.0));
    assert_handles(&mut ui, false, true);
}
