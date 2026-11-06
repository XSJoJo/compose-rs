//! A state-based field's `scroll_state` (D1138), from Compose's `TextFieldScrollTest`.

#![allow(non_snake_case)]

use compose_foundation::gestures::ScrollableState;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{TextRange, UnitExt};

use crate::scroll_support::run;

const LONG_TEXT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod \
    tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
    exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

fn scroll(initial: i32) -> ScrollState {
    **Box::leak(Box::new(ScrollState::new(initial)))
}

fn text_state(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
}

/// A field 300 by `height` dp over `state`, scrolled by `scroll`, a single line or many.
fn scrollable(
    state: TextFieldState,
    scroll: State<ScrollState>,
    single_line: bool,
    height: f32,
) -> ComposeTestRule {
    let line_limits =
        if single_line { TextFieldLineLimits::SingleLine } else { TextFieldLineLimits::DEFAULT };
    compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.size((300.dp(), height.dp())).test_tag("field"),
            line_limits = line_limits,
            scroll_state = Some(scroll.get()),
        );
    })
}

fn fixed(scroll: ScrollState) -> State<ScrollState> {
    Box::leak(Box::new(mutable_state_of(scroll))).handle().into()
}

fn content_size(scroll: ScrollState) -> i32 {
    scroll.scroll_indicator_state().expect("an indicator").content_size()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_horizontal_scrollable_withLongInput
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_vertical_scrollable_withLongInput
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_horizontal_notScrollable_withShortInput
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_vertical_notScrollable_withShortInput
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_horizontal_setsViewportSize
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_horizontal_setsContentSize
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_vertical_setsViewportSize
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_vertical_setsContentSize
#[test]
fn the_layout_says_how_far_the_field_scrolls() {
    for single_line in [true, false] {
        let long = scroll(0);
        let mut ui =
            scrollable(text_state(LONG_TEXT, TextRange::ZERO), fixed(long), single_line, 50.0);
        ui.wait_for_idle();
        assert!(long.max_value() > 0 && long.max_value() < i32::MAX, "single {single_line}");
        let short = scroll(0);
        let mut ui =
            scrollable(text_state("text", TextRange::ZERO), fixed(short), single_line, 100.0);
        ui.wait_for_idle();
        assert_eq!(short.max_value(), 0, "single {single_line}");
        assert!(short.viewport_size() > 0, "single {single_line}");
        assert!(content_size(short) > 0, "single {single_line}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_horizontal_swipe_whenLongInput
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScroll_vertical_swipe_whenLongInput
#[test]
fn a_swipe_scrolls_a_long_text() {
    for single_line in [true, false] {
        let state = scroll(0);
        let mut ui =
            scrollable(text_state(LONG_TEXT, TextRange::ZERO), fixed(state), single_line, 50.0);
        ui.wait_for_idle();
        assert_eq!(state.value(), 0);
        ui.on_node_with_tag("field").perform_touch_input(|touch| {
            if single_line { touch.swipe_left() } else { touch.swipe_up() }
        });
        ui.wait_for_idle();
        let first = state.value();
        assert!(first > 0, "single {single_line}");
        ui.on_node_with_tag("field").perform_touch_input(|touch| {
            if single_line { touch.swipe_right() } else { touch.swipe_down() }
        });
        ui.wait_for_idle();
        assert!(state.value() < first, "single {single_line}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScrollStateChange_shouldResetTheScroll
#[test]
fn a_new_scroll_state_starts_where_it_is() {
    let (first, second) = (scroll(0), scroll(0));
    let which = Box::leak(Box::new(mutable_state_of(first))).handle();
    let mut ui = scrollable(text_state(LONG_TEXT, TextRange::ZERO), which.into(), true, 50.0);
    ui.wait_for_idle();
    assert!(first.max_value() > 0 && first.max_value() < i32::MAX);
    assert_eq!((second.max_value(), second.value()), (i32::MAX, 0), "not laid out");
    ui.on_node_with_tag("field").perform_touch_input(|touch| touch.swipe_left());
    ui.wait_for_idle();
    assert!(first.value() > 0);
    which.set(second);
    ui.wait_for_idle();
    assert!(second.max_value() > 0 && second.max_value() < i32::MAX);
    assert_eq!(second.value(), 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldDoesNotFollowCursor_whenNotFocused
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldFollowsCursor_whenFocused
#[test]
fn the_scroll_follows_the_cursor_only_when_focused() {
    let state = text_state(LONG_TEXT, TextRange::ZERO);
    let unfocused = scroll(0);
    let mut ui = scrollable(state, fixed(unfocused), true, 50.0);
    state.edit(|buffer| buffer.place_cursor_at_end());
    ui.wait_for_idle();
    assert_eq!(unfocused.value(), 0);

    let state = text_state(LONG_TEXT, TextRange::ZERO);
    let focused = scroll(0);
    let mut ui = scrollable(state, fixed(focused), true, 50.0);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    state.edit(|buffer| buffer.place_cursor_at_end());
    ui.wait_for_idle();
    assert!(focused.value() > 0);
    assert_eq!(focused.value(), focused.max_value());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldDoesNotFollowCursor_whenScrollStateChanges_butCursorRemainsTheSame
#[test]
fn a_scroll_the_application_makes_stays() {
    let state = text_state(LONG_TEXT, TextRange::at(5));
    let scroll_state = scroll(0);
    let mut ui = scrollable(state, fixed(scroll_state), true, 50.0);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    let max = scroll_state.max_value();
    run(&mut ui, async move {
        let _ = scroll_state.scroll_to(max).await;
    });
    ui.wait_for_idle();
    assert_eq!(scroll_state.value(), max);
    assert_eq!(state.selection(), TextRange::at(5));
}

/// Typing at the end of a long single line scrolls it to keep the cursor in view.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, cursorScrolledIntoViewWhenTyping_inHorizontallyScrollableField_whenAtEnd
#[test]
fn typing_at_the_end_keeps_the_cursor_in_view() {
    let state = text_state(LONG_TEXT, TextRange::at(LONG_TEXT.len()));
    let scroll_state = scroll(0);
    let mut ui = scrollable(state, fixed(scroll_state), true, 50.0);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    let max = scroll_state.max_value();
    ui.type_text("more");
    assert!(scroll_state.max_value() > max);
    assert_eq!(scroll_state.value(), scroll_state.max_value());
}
