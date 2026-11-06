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
