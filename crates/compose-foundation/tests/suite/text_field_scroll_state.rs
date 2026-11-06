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

// ---- The rules of following, after step 5's review (D1140) ---------------------------------

/// A focused multi-line field `height` dp high over `state`, scrolled by `scroll`.
fn sized(
    state: TextFieldState,
    scroll: State<ScrollState>,
    height: State<f32>,
    direction: compose_ui::LayoutDirection,
    single_line: bool,
) -> ComposeTestRule {
    let line_limits =
        if single_line { TextFieldLineLimits::SingleLine } else { TextFieldLineLimits::DEFAULT };
    compose_test(move || {
        CompositionLocalProvider(
            compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
            move || {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.size((300.dp(), height.get().dp())).test_tag("field"),
                    line_limits = line_limits,
                    scroll_state = Some(scroll.get()),
                );
            },
        );
    })
}

fn fixed_height(height: f32) -> MutableState<f32> {
    Box::leak(Box::new(mutable_state_of(height))).handle()
}

/// A focused field that gets smaller follows its cursor, as Compose's `calculateOffsetToFollow`
/// when the container's size changes.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, cursorScrolledIntoView_whenHeightChanges_inVerticallyScrollableField
#[test]
fn a_shrinking_field_follows_its_cursor() {
    let state = text_state(LONG_TEXT, TextRange::at(LONG_TEXT.len()));
    let scroll_state = scroll(0);
    let height = fixed_height(300.0);
    let mut ui =
        sized(state, fixed(scroll_state), height.into(), compose_ui::LayoutDirection::Ltr, false);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!((scroll_state.value(), scroll_state.max_value()), (0, 0), "all of it shows");
    height.set(30.0);
    ui.wait_for_idle();
    assert!(scroll_state.max_value() > 0);
    assert_eq!(scroll_state.value(), scroll_state.max_value());
}

/// A cursor moved while the field isn't focused is followed once it is, as Compose's: nothing is
/// followed while the cursor doesn't show.
#[test]
fn a_cursor_moved_before_the_focus_is_followed_at_the_focus() {
    let state = text_state(LONG_TEXT, TextRange::ZERO);
    let scroll_state = scroll(0);
    let mut ui = scrollable(state, fixed(scroll_state), true, 50.0);
    state.edit(|buffer| buffer.place_cursor_at_end());
    ui.wait_for_idle();
    assert_eq!(scroll_state.value(), 0);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert!(scroll_state.value() > 0);
    assert_eq!(scroll_state.value(), scroll_state.max_value());
}

/// A new scroll state given to a focused field keeps its value: the cursor didn't move.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldScrollStateChange_shouldResetTheScroll
#[test]
fn a_new_scroll_state_of_a_focused_field_starts_where_it_is() {
    let (first, second) = (scroll(0), scroll(0));
    let which = Box::leak(Box::new(mutable_state_of(first))).handle();
    let state = text_state(LONG_TEXT, TextRange::at(LONG_TEXT.len()));
    let mut ui = scrollable(state, which.into(), true, 50.0);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!(first.value(), first.max_value(), "the cursor at the end, followed");
    which.set(second);
    ui.wait_for_idle();
    assert!(second.max_value() > 0 && second.max_value() < i32::MAX);
    assert_eq!(second.value(), 0);
}

const RIGHT_TO_LEFT: &str = "שלום עולם שלום עולם שלום עולם שלום עולם שלום עולם שלום עולם";

/// A right-to-left single line scrolls from its right, its start: at 0 the start shows, and a
/// swipe right, the content following the finger, shows more of it.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldScrollTest.kt, textFieldRtl_horizontalScroll_isReversed
#[test]
fn a_right_to_left_line_scrolls_from_its_start() {
    let state = text_state(RIGHT_TO_LEFT, TextRange::ZERO);
    let scroll_state = scroll(0);
    let rtl = compose_ui::LayoutDirection::Rtl;
    let mut ui = sized(state, fixed(scroll_state), fixed_height(50.0).into(), rtl, true);
    ui.wait_for_idle();
    assert!(scroll_state.max_value() > 0);
    // A tap near the field's right edge lands near the text's start.
    ui.on_node_with_tag("field").perform_touch_input(|touch| {
        let at = compose_ui::Offset::new(touch.right() - 3.0, touch.center_y());
        touch.click_at(at);
    });
    ui.wait_for_idle();
    assert!(state.selection().start < 4, "near the start: {:?}", state.selection());
    assert_eq!(scroll_state.value(), 0);
    ui.on_node_with_tag("field").perform_touch_input(|touch| touch.swipe_right());
    ui.wait_for_idle();
    assert!(scroll_state.value() > 0);
}

/// A tap in the frame an application scrolls the field, an edit making it wait for the next
/// layout, lands where it was on the screen: under the scroll last drawn, as Compose's tap reads
/// the coordinates the last placement left.
#[test]
fn a_tap_in_the_frame_of_a_scroll_lands_where_it_was_shown() {
    let state = text_state(LONG_TEXT, TextRange::ZERO);
    let scroll_state = scroll(0);
    let mut ui = scrollable(state, fixed(scroll_state), true, 50.0);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!(scroll_state.value(), 0);
    state.edit(|buffer| buffer.append("!"));
    scroll_state.dispatch_raw_delta(400.0);
    // No frame between: the field's at the root's corner.
    let touch = |event: compose_ui::PointerEvent| {
        event.with_pointer(compose_ui::PointerId(1), compose_ui::PointerType::Touch)
    };
    let at = compose_ui::Offset::new(1.0, 10.0);
    ui.host().send_pointer(touch(compose_ui::PointerEvent::press(at)));
    ui.host().send_pointer(touch(compose_ui::PointerEvent::release(at)));
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::ZERO, "under the finger as shown");
}

/// A finger's second press dragging selects by words, as Compose's
/// `touchSelectionSubsequentPress`, and the field doesn't scroll under it, as Compose's
/// `scrollable` disabled while a selection drags. A long press doesn't select yet
/// (`touch-text-selection`).
#[test]
fn a_selecting_touch_drag_does_not_scroll() {
    let state = text_state(LONG_TEXT, TextRange::ZERO);
    let scroll_state = scroll(0);
    let mut ui = scrollable(state, fixed(scroll_state), true, 50.0);
    ui.on_node_with_tag("field").perform_touch_input(|touch| {
        let at = compose_ui::Offset::new(touch.left() + 10.0, touch.center_y());
        touch.click_at(at);
        touch.advance_event_time(100);
        touch.down(at);
        touch.advance_event_time(50);
        for step in 1..=10 {
            touch.move_to(at + compose_ui::Offset::new(step as f32 * 20.0, 0.0));
        }
        touch.up();
    });
    ui.wait_for_idle();
    assert!(!state.selection().collapsed(), "selected: {:?}", state.selection());
    assert_eq!(scroll_state.value(), 0);
}
