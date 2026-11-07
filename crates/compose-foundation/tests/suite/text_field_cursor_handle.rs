//! A field's cursor handle under a finger (D2008), from Compose's `TextFieldCursorHandleTest`.
//!
//! As Compose's tests: the test font (each character a font size wide, a line one font size
//! high) at 10 sp, density 1 so pixels are dp, the host in touch input mode as an Android test's.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{Density, InputMode, IntSize, Offset, Popup, PopupProperties, TextRange, UnitExt};

use crate::text_field_touch_support::*;

const TAG: &str = "BasicTextField";
/// The cursor's width (`DefaultCursorThickness`), half of it past the cursor's offset.
const HALF_CURSOR: f32 = 1.0;

fn state_of(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

/// A rule in touch input mode over `content`, in the test font, at density 1.
fn touch_rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer), content);
    ui.host().set_input_mode(InputMode::Touch);
    ui
}

/// A field over `state` in the test font at 10 sp, with `modifier` after its tag.
fn field(state: TextFieldState, modifier: Modifier) -> ComposeTestRule {
    touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).then(modifier.clone()),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
        );
    })
}

fn focus(ui: &mut ComposeTestRule) {
    ui.on_node_with_tag(TAG).request_focus();
    ui.wait_for_idle();
}

fn click(ui: &mut ComposeTestRule, at: Offset) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(at));
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_showsAtCorrectLocation_ltr
#[test]
fn a_tap_shows_the_cursor_handle_below_the_cursor() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier);
    focus(&mut ui);
    click(&mut ui, Offset::new(20.0, 5.0));
    assert_eq!(state.selection(), TextRange::at(2));
    assert_handle_position(&mut ui, Handle::Cursor, 20.0 + HALF_CURSOR, 10.0);
}

/// The cursor handle's popup hangs from the point it marks by its top center, as Compose's
/// `HandlePopup` with `Alignment.TopCenter`, in a field placed away from the window's corner and
/// scrolled.
#[test]
fn the_cursor_handle_hangs_from_its_point() {
    let state = state_with("abcd abcd abcd abcd abcd", TextRange::ZERO);
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.padding(30.dp()).test_tag(TAG).width(100.dp()),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            line_limits = TextFieldLineLimits::SingleLine,
        );
    });
    focus(&mut ui);
    click(&mut ui, Offset::new(20.0, 5.0));
    let bounds = ui.on_node(is_selection_handle(Some(Handle::Cursor))).fetch_semantics_node();
    let bounds = bounds.bounds_in_root();
    assert_eq!((bounds.left, bounds.top), (30.0 + 21.0 - 20.0, 30.0 + 10.0));
    // Scrolled: the cursor at the end, the text's start out of view to the left.
    state.edit(|buffer| buffer.place_cursor_at_end());
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.advance_event_time(1000);
        touch.click_at(Offset::new(90.0, 5.0));
    });
    ui.wait_for_idle();
    let info = handle_info(&mut ui, Handle::Cursor);
    let node = ui.on_node(is_selection_handle(Some(Handle::Cursor))).fetch_semantics_node();
    let scrolled = info.position.x - (node.bounds_in_root().left + 20.0 - 30.0);
    assert!(scrolled > 100.0, "{info:?} {:?}", node.bounds_in_root());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_hasMinimumTouchSizeArea
#[test]
fn the_cursor_handle_is_at_least_40_dp_square() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier.width(100.dp()));
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click());
    ui.wait_for_idle();
    let mut bottom_right = Offset::ZERO;
    ui.on_node(is_selection_handle(Some(Handle::Cursor)))
        .perform_touch_input(|touch| bottom_right = touch.bottom_right());
    assert!((bottom_right.x - 40.0).abs() <= 1.0, "{bottom_right:?}");
    assert!((bottom_right.y - 40.0).abs() <= 1.0, "{bottom_right:?}");
}

/// A field in a right-to-left layout.
fn rtl_field(state: TextFieldState, modifier: Modifier) -> ComposeTestRule {
    touch_rule(move || {
        let modifier = modifier.clone();
        compose_runtime::CompositionLocalProvider(
            compose_ui::LOCAL_LAYOUT_DIRECTION.provides(compose_ui::LayoutDirection::Rtl),
            move || {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.test_tag(TAG).then(modifier.clone()),
                    text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
                );
            },
        );
    })
}

const HEBREW: &str = "\u{05D0}\u{05D1}\u{05D2}\u{05D3}\u{05D4}";

/// The offset of the `index`th character of `HEBREW` (two bytes each).
fn hebrew(index: usize) -> usize {
    index * 2
}

fn assert_no_cursor_handle(ui: &mut ComposeTestRule) {
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_does_not_exist();
}

fn assert_cursor_handle_shown(ui: &mut ComposeTestRule) {
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).assert_is_displayed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_showsAtCorrectLocation_outOfTextBoundsTouch_ltr
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_showsAtCorrectLocation_outOfTextBoundsTouch_rtl
#[test]
fn a_tap_past_the_text_shows_the_handle_at_its_end() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier.width(100.dp()));
    focus(&mut ui);
    click(&mut ui, Offset::new(80.0, 5.0));
    assert_eq!(state.selection(), TextRange::at(5));
    assert_handle_position(&mut ui, Handle::Cursor, 50.0 + HALF_CURSOR, 10.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_showsAtCorrectLocation_rtl
#[test]
fn a_tap_in_right_to_left_text_shows_the_handle_below_the_cursor() {
    let state = state_of(HEBREW);
    let mut ui = rtl_field(state, Modifier.width(50.dp()));
    focus(&mut ui);
    click(&mut ui, Offset::new(20.0, 5.0));
    assert_eq!(state.selection(), TextRange::at(hebrew(3)));
    assert_handle_position(&mut ui, Handle::Cursor, 20.0 + HALF_CURSOR, 10.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, tapTextField_cursorHandleFiltered
#[test]
fn a_tap_goes_through_the_input_transformation() {
    let state = state_of("hello");
    let to_four = InputTransformation::from_fn(|buffer| buffer.set_selection(TextRange::at(4)));
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            input_transformation = Some(to_four.clone()),
        );
    });
    focus(&mut ui);
    click(&mut ui, Offset::new(20.0, 5.0));
    assert_eq!(state.selection(), TextRange::at(4));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_notVisibleOnEmptyField
#[test]
fn an_empty_field_shows_no_handle() {
    let state = state_of("");
    let mut ui = field(state, Modifier);
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    assert_no_cursor_handle(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_notVisibleInKeyboardInputMode
#[test]
fn a_field_focused_in_keyboard_mode_shows_no_handle() {
    let state = state_of("");
    let mut ui = field(state, Modifier);
    ui.host().set_input_mode(InputMode::Keyboard);
    focus(&mut ui);
    assert_no_cursor_handle(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_doesNotShow_whenTextFieldIsReadOnly
#[test]
fn a_read_only_field_shows_no_cursor_handle() {
    let state = state_of("hello");
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            read_only = true,
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
        );
    });
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    assert_no_cursor_handle(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_disappears_whenTextIsEdited
#[test]
fn typing_hides_the_cursor_handle() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier);
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    assert_cursor_handle_shown(&mut ui);
    ui.on_node_with_tag(TAG).perform_text_input("m");
    assert_no_cursor_handle(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_disappears_whenTextStateChanges
#[test]
fn a_new_text_in_the_state_hides_the_cursor_handle() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier);
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    assert_cursor_handle_shown(&mut ui);
    state.set_text_and_place_cursor_at_end("hello2");
    assert_no_cursor_handle(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_doesNotDisappear_whenSelectionChanges
#[test]
fn the_cursor_handle_follows_a_new_selection() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier);
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    assert_cursor_handle_shown(&mut ui);
    state.edit(|buffer| buffer.place_cursor_before_char_at(2));
    assert_cursor_handle_shown(&mut ui);
    assert_handle_position(&mut ui, Handle::Cursor, 20.0 + HALF_CURSOR, 10.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_coercesAtBoundaries_ltr
#[test]
fn the_cursor_handle_stays_in_the_text_at_its_edges() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier.width(100.dp()));
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    state.edit(|buffer| buffer.set_selection(TextRange::at(0)));
    assert_handle_position(&mut ui, Handle::Cursor, HALF_CURSOR, 10.0);
    state.edit(|buffer| buffer.set_selection(TextRange::at(5)));
    assert_handle_position(&mut ui, Handle::Cursor, 50.0 + HALF_CURSOR, 10.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_coercesAtBoundaries_rtl
#[test]
fn the_cursor_handle_stays_in_right_to_left_text_at_its_edges() {
    let state = state_of(HEBREW);
    let mut ui = rtl_field(state, Modifier.width(100.dp()));
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    let width = 100.0;
    state.edit(|buffer| buffer.set_selection(TextRange::at(0)));
    assert_handle_position(&mut ui, Handle::Cursor, width - HALF_CURSOR, 10.0);
    state.edit(|buffer| buffer.set_selection(TextRange::at(hebrew(5))));
    assert_handle_position(&mut ui, Handle::Cursor, width - 50.0 - HALF_CURSOR, 10.0);
}

/// Drags the cursor handle sideways by the touch slop and `distance` more, from its center, as
/// Compose's `performHandleDrag`.
fn drag_cursor_handle(ui: &mut ComposeTestRule, to_left: bool, distance: f32, millis: u64) {
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).perform_touch_input(|touch| {
        let slop = touch.view_configuration().touch_slop;
        let x = touch.center_x();
        if to_left {
            touch.swipe_left_with(x, x - slop - distance, millis);
        } else {
            touch.swipe_right_with(x, x + slop + distance, millis);
        }
    });
    ui.wait_for_idle();
}

fn field_width(ui: &mut ComposeTestRule) -> f32 {
    ui.on_node_with_tag(TAG).fetch_semantics_node().bounds_in_root().width()
}

fn state_with(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandleDrag_getsFiltered
#[test]
fn a_drag_of_the_cursor_handle_goes_through_the_input_transformation() {
    let state = state_of("abc abc");
    let to_start = InputTransformation::from_fn(|buffer| buffer.set_selection(TextRange::ZERO));
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            input_transformation = Some(to_start.clone()),
        );
    });
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    assert_cursor_handle_shown(&mut ui);
    drag_cursor_handle(&mut ui, false, 50.0, 1000);
    assert_eq!(state.selection(), TextRange::ZERO);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToRight_ltr
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToLeft_ltr
#[test]
fn dragging_the_cursor_handle_moves_the_cursor() {
    let state = state_with("abc", TextRange::ZERO);
    let mut ui = field(state, Modifier.width(100.dp()));
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    assert_cursor_handle_shown(&mut ui);
    drag_cursor_handle(&mut ui, false, 10.0, 1000);
    assert_eq!(state.selection(), TextRange::at(1));

    let state = state_with("abc", TextRange::ZERO);
    let mut ui = field(state, Modifier.width(100.dp()));
    focus(&mut ui);
    ui.on_node_with_tag(TAG)
        .perform_touch_input(|touch| touch.click_at(touch.top_right() - Offset::new(1.0, -1.0)));
    ui.wait_for_idle();
    assert_cursor_handle_shown(&mut ui);
    drag_cursor_handle(&mut ui, true, 10.0, 1000);
    assert_eq!(state.selection(), TextRange::at(2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToRight_ltr_outOfBounds
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToLeft_ltr_outOfBounds
#[test]
fn dragging_the_cursor_handle_past_the_field_stops_at_the_text_ends() {
    let state = state_with("abc", TextRange::ZERO);
    let mut ui = field(state, Modifier.width(50.dp()));
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    assert_cursor_handle_shown(&mut ui);
    let width = field_width(&mut ui);
    drag_cursor_handle(&mut ui, false, width * 2.0, 1000);
    assert_eq!(state.selection(), TextRange::at(3));

    let state = state_with("abc", TextRange::at(3));
    let mut ui = field(state, Modifier.width(50.dp()));
    focus(&mut ui);
    ui.on_node_with_tag(TAG)
        .perform_touch_input(|touch| touch.click_at(touch.top_right() - Offset::new(1.0, -1.0)));
    ui.wait_for_idle();
    assert_cursor_handle_shown(&mut ui);
    let width = field_width(&mut ui);
    drag_cursor_handle(&mut ui, true, width * 2.0, 1000);
    assert_eq!(state.selection(), TextRange::ZERO);
}

const LONG: &str = "abcd abcd abcd abcd abcd";

fn single_line(state: TextFieldState) -> ComposeTestRule {
    touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(100.dp()),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            line_limits = TextFieldLineLimits::SingleLine,
        );
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToRight_ltr_outOfBounds_scrollable_continuesDrag
#[test]
fn dragging_the_cursor_handle_past_a_scrolling_field_goes_on_to_the_end() {
    let state = state_with(LONG, TextRange::ZERO);
    let mut ui = single_line(state);
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    assert_cursor_handle_shown(&mut ui);
    let width = field_width(&mut ui);
    drag_cursor_handle(&mut ui, false, width * 3.0, 1000);
    assert_eq!(state.selection(), TextRange::at(LONG.len()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToRight_ltr_outOfBounds_scrollable
#[test]
fn a_drag_of_the_cursor_handle_counts_the_scroll_it_made() {
    let state = state_with(LONG, TextRange::ZERO);
    let mut ui = single_line(state);
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    assert_cursor_handle_shown(&mut ui);
    drag_cursor_handle(&mut ui, false, 120.0, 1);
    assert_eq!(state.selection(), TextRange::at(12));
    drag_cursor_handle(&mut ui, false, 20.0, 1);
    assert_eq!(state.selection(), TextRange::at(14));
}

/// The byte offset of the `chars`th character of `text` (Compose's offsets count UTF-16 units,
/// which these texts' characters are one of).
fn char_offset(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(index, _)| index)
}

fn hebrew_field(text: &'static str) -> (TextFieldState, ComposeTestRule) {
    let state = state_with(text, TextRange::ZERO);
    (state, rtl_field(state, Modifier.width(100.dp())))
}

fn click_top_right(ui: &mut ComposeTestRule, inset: f32) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.click_at(touch.top_right() + Offset::new(-inset, inset))
    });
    ui.wait_for_idle();
}

const SHORT_HEBREW: &str = "\u{05D0}\u{05D1}\u{05D2}";

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToRight_rtl
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToLeft_rtl
#[test]
fn dragging_the_cursor_handle_in_right_to_left_text_moves_the_cursor() {
    let (state, mut ui) = hebrew_field(SHORT_HEBREW);
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    drag_cursor_handle(&mut ui, false, 10.0, 1000);
    assert_eq!(state.selection(), TextRange::at(char_offset(SHORT_HEBREW, 2)));

    let (state, mut ui) = hebrew_field(SHORT_HEBREW);
    focus(&mut ui);
    click_top_right(&mut ui, 1.0);
    drag_cursor_handle(&mut ui, true, 10.0, 1000);
    assert_eq!(state.selection(), TextRange::at(char_offset(SHORT_HEBREW, 1)));
}

/// A finger dragging the cursor handle a little past a single line's edge: each step scrolls the
/// text after the cursor, and the next step counts that scroll, so the cursor goes further than
/// the finger's place in the text before the scroll (Compose's `handleDragPosition`, which adds
/// how far the text moved since the drag's start).
#[test]
fn a_drag_past_the_edge_counts_the_scroll_it_made_as_it_goes() {
    let state = state_with(LONG, TextRange::ZERO);
    let mut ui = single_line(state);
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    let handle = is_selection_handle(Some(Handle::Cursor));
    let mut slop = 0.0;
    ui.on_node(handle.clone()).perform_touch_input(|touch| {
        slop = touch.view_configuration().touch_slop;
        touch.down(touch.center());
        touch.move_by(Offset::new(slop, 0.0));
    });
    ui.wait_for_idle();
    for _ in 0..22 {
        ui.on_node(handle.clone())
            .perform_touch_input(|touch| touch.move_by(Offset::new(5.0, 0.0)));
        ui.wait_for_idle();
    }
    ui.on_node(handle).perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
    // The finger is 110 px from where it started: at the 11th character without the scroll.
    assert!(state.selection().start > 11, "{:?}", state.selection());
}

/// A field over "hello hello hello hello" showing one line, scrolled by `scroll`: a multi-line
/// field one line high five characters wide, or a single line ten wide.
fn scrolling(single_line: bool, scroll: ScrollState) -> (TextFieldState, ComposeTestRule) {
    let state = state_with("hello hello hello hello", TextRange::ZERO);
    let (line_limits, width) = if single_line {
        (TextFieldLineLimits::SingleLine, 100.0)
    } else {
        (TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 1 }, 50.0)
    };
    let ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(width.dp()),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            line_limits = line_limits,
            scroll_state = Some(scroll),
        );
    });
    (state, ui)
}

fn scroll_state() -> ScrollState {
    **Box::leak(Box::new(ScrollState::new(0)))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_disappearsOnVerticalScroll
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_disappearsOnHorizontalScroll
#[test]
fn the_cursor_handle_hides_when_the_cursor_scrolls_out_of_view() {
    for single_line in [false, true] {
        let scroll = scroll_state();
        let (_state, mut ui) = scrolling(single_line, scroll);
        focus(&mut ui);
        ui.on_node_with_tag(TAG).perform_click();
        assert_cursor_handle_shown(&mut ui);
        let max = scroll.max_value();
        crate::scroll_support::run(&mut ui, async move {
            let _ = scroll.scroll_to(max).await;
        });
        ui.wait_for_idle();
        assert_no_cursor_handle(&mut ui);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_reappearsOnVerticalScroll
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_reappearsOnHorizontalScroll
#[test]
fn the_cursor_handle_comes_back_when_the_cursor_scrolls_into_view() {
    for single_line in [false, true] {
        let (_state, mut ui) = scrolling(single_line, scroll_state());
        focus(&mut ui);
        ui.on_node_with_tag(TAG).perform_click();
        assert_cursor_handle_shown(&mut ui);
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
            // Not a second tap of the first: long after it.
            touch.advance_event_time(1000);
            if single_line {
                let right = touch.right();
                touch.swipe_left_with(touch.center_x(), -right, 200);
            } else {
                // Further than Compose's `-bottom`: at density 1 the swipe is slower in pixels
                // than on a phone, and flings less far.
                let bottom = touch.bottom();
                touch.swipe_up_with(touch.center_y(), -4.0 * bottom, 200);
            }
        });
        ui.wait_for_idle();
        assert_no_cursor_handle(&mut ui);
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
            touch.advance_event_time(1000);
            if single_line {
                let right = touch.right();
                touch.swipe_right_with(touch.center_x(), 2.0 * right, 200);
            } else {
                let bottom = touch.bottom();
                touch.swipe_down_with(touch.center_y(), 5.0 * bottom, 200);
            }
        });
        ui.wait_for_idle();
        assert_cursor_handle_shown(&mut ui);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandleHides_whenHardwareKeyboardIsUsed_thenComesBackWithTouch
#[test]
fn a_hardware_key_hides_the_cursor_handle_until_a_finger_comes_back() {
    let state = state_of("hello");
    let mut ui = field(state, Modifier);
    focus(&mut ui);
    click(&mut ui, Offset::new(20.0, 5.0));
    assert_cursor_handle_shown(&mut ui);
    state.edit(|buffer| buffer.place_cursor_at_end());
    assert_cursor_handle_shown(&mut ui);
    ui.on_node_with_tag(TAG)
        .perform_key_input(|keys| keys.press_key(compose_ui::Key::DirectionLeft));
    ui.wait_for_idle();
    assert_no_cursor_handle(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.advance_event_time(1000);
        touch.click_at(Offset::new(20.0, 5.0));
    });
    ui.wait_for_idle();
    assert_cursor_handle_shown(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, cursorHandle_disappears_whenWindowLosesFocus
#[test]
fn the_cursor_handle_hides_while_another_window_takes_the_keys() {
    let state = state_of("hello");
    let covered = Box::leak(Box::new(compose_runtime::mutable_state_of(false))).handle();
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
        );
        if covered.get() {
            // A window of its own over the field's, as a focusable popup is.
            Popup!(properties = PopupProperties { focusable: true, ..Default::default() }, {
                compose_foundation::layout::Spacer(Modifier.size(10.dp()));
            });
        }
    });
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_click();
    assert_cursor_handle_shown(&mut ui);
    covered.set(true);
    ui.wait_for_idle();
    assert_no_cursor_handle(&mut ui);
}

/// A decoration that moves the inner field makes the field's node anew: the old node's handle
/// goes with it, and the new node's handles follow their drags (the review of D2010).
#[test]
fn a_field_made_anew_by_its_decoration_leaves_no_handle_and_follows_drags() {
    let state = state_of("hello hello");
    let wrap = Box::leak(Box::new(compose_runtime::mutable_state_of(false))).handle();
    let mut ui = touch_rule(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG).width(200.dp()),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            decorator = move |inner: compose_runtime::Callback| {
                if wrap.get() {
                    Box!({ inner() });
                } else {
                    inner();
                }
            },
        );
    });
    focus(&mut ui);
    click(&mut ui, Offset::new(20.0, 5.0));
    assert_cursor_handle_shown(&mut ui);
    wrap.set(true);
    ui.wait_for_idle();
    assert_no_cursor_handle(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.advance_event_time(1000);
        touch.click_at(Offset::new(20.0, 5.0));
    });
    ui.wait_for_idle();
    assert_cursor_handle_shown(&mut ui);
    assert_eq!(state.selection(), TextRange::at(2));
    drag_cursor_handle(&mut ui, false, 60.0, 1000);
    assert_eq!(state.selection(), TextRange::at(8));
}

/// A field scrolled out of a scrolling container's viewport shows no handle: its visible bounds
/// are clipped by its ancestors, as Compose's `visibleBounds()` (the review of D2010).
#[test]
fn a_field_scrolled_out_of_its_container_shows_no_handle() {
    let state = state_of("hello");
    let scroll = **Box::leak(Box::new(ScrollState::new(0)));
    let mut ui = touch_rule(move || {
        Column!(Modifier.height(100.dp()).vertical_scroll(scroll), {
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.test_tag(TAG).width(100.dp()),
                text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            );
            compose_foundation::layout::Spacer(Modifier.height(1000.dp()));
        });
    });
    focus(&mut ui);
    click(&mut ui, Offset::new(20.0, 5.0));
    assert_cursor_handle_shown(&mut ui);
    crate::scroll_support::run(&mut ui, async move {
        let _ = scroll.scroll_to(300).await;
    });
    ui.wait_for_idle();
    assert_eq!(scroll.value(), 300);
    assert_no_cursor_handle(&mut ui);
    crate::scroll_support::run(&mut ui, async move {
        let _ = scroll.scroll_to(0).await;
    });
    ui.wait_for_idle();
    assert_cursor_handle_shown(&mut ui);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToRight_rtl_outOfBounds
#[test]
fn moveCursorHandleToRight_rtl_outOfBounds() {
    let state = state_of(SHORT_HEBREW);
    let mut ui = rtl_field(state, Modifier.width(50.dp()));
    focus(&mut ui);
    click(&mut ui, Offset::new(1.0, 1.0));
    assert_cursor_handle_shown(&mut ui);
    let width = field_width(&mut ui);
    drag_cursor_handle(&mut ui, false, width * 2.0, 1000);
    assert_eq!(state.selection(), TextRange::ZERO);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToLeft_rtl_outOfBounds
#[test]
fn moveCursorHandleToLeft_rtl_outOfBounds() {
    let state = state_of(SHORT_HEBREW);
    let mut ui = rtl_field(state, Modifier.width(50.dp()));
    focus(&mut ui);
    click_top_right(&mut ui, 1.0);
    assert_cursor_handle_shown(&mut ui);
    let width = field_width(&mut ui);
    drag_cursor_handle(&mut ui, true, width * 2.0, 1000);
    assert_eq!(state.selection(), TextRange::at(SHORT_HEBREW.len()));
}

const WORDS_HEBREW: &str = "\u{05D0}\u{05D1}\u{05D2}\u{05D3} \u{05D0}\u{05D1}\u{05D2}\u{05D3} \u{05D0}\u{05D1}\u{05D2}\u{05D3} \u{05D0}\u{05D1}\u{05D2}\u{05D3}";

/// A right-to-left single line 100 dp wide over `WORDS_HEBREW`, scrolled by `scroll`.
fn rtl_single_line(state: TextFieldState, scroll: Option<ScrollState>) -> ComposeTestRule {
    touch_rule(move || {
        compose_runtime::CompositionLocalProvider(
            compose_ui::LOCAL_LAYOUT_DIRECTION.provides(compose_ui::LayoutDirection::Rtl),
            move || {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.test_tag(TAG).width(100.dp()),
                    text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
                    line_limits = TextFieldLineLimits::SingleLine,
                    scroll_state = scroll,
                );
            },
        );
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToLeft_rtl_outOfBounds_scrollable_continuesDrag
#[test]
fn moveCursorHandleToLeft_rtl_outOfBounds_scrollable_continuesDrag() {
    let state = state_of(WORDS_HEBREW);
    let mut ui = rtl_single_line(state, None);
    focus(&mut ui);
    click_top_right(&mut ui, 1.0);
    assert_cursor_handle_shown(&mut ui);
    let width = field_width(&mut ui);
    drag_cursor_handle(&mut ui, true, width * 3.0, 1000);
    assert_eq!(state.selection(), TextRange::at(WORDS_HEBREW.len()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldCursorHandleTest.kt, moveCursorHandleToLeft_rtl_outOfBounds_scrollable
#[test]
fn moveCursorHandleToLeft_rtl_outOfBounds_scrollable() {
    let state = state_with(WORDS_HEBREW, TextRange::ZERO);
    let scroll = scroll_state();
    let mut ui = rtl_single_line(state, Some(scroll));
    focus(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(touch.top_right()));
    ui.wait_for_idle();
    assert_cursor_handle_shown(&mut ui);
    drag_cursor_handle(&mut ui, true, 10.0 * 12.0, 1);
    assert_eq!(state.selection(), TextRange::at(char_offset(WORDS_HEBREW, 12)));
    drag_cursor_handle(&mut ui, true, 10.0 * 2.0, 1);
    assert_eq!(state.selection(), TextRange::at(char_offset(WORDS_HEBREW, 14)));
}
