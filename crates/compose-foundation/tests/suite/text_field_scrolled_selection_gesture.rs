//! A finger's long press and a selection handle's drag in a field scrolled to its end, from
//! Compose's `TextFieldScrolledSelectionGestureTest` (the state-based field's, b/314385218).
//!
//! As Compose's tests: the test font at 15 sp, density 1, a touch slop of nearly nothing, the
//! field 300 dp wide in a padded box, touch input mode. Compose's magnifier checks wait on the
//! magnifier (none is shown here, as on a platform without one).

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    Density, InputMode, IntSize, Offset, Rect, TextLayoutResult, TextRange, UnitExt,
    ViewConfiguration,
};

use crate::text_field_touch_support::*;

const TAG: &str = "testTag";
const FONT: f32 = 15.0;

/// A field over `text`, single line or four lines at most, scrolled to its end by four swipes.
fn scrolled_field(text: String, single_line: bool) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::new(&text)));
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(800, 800),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let line_limits = if single_line {
                TextFieldLineLimits::SingleLine
            } else {
                TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 4 }
            };
            Box!(modifier = Modifier.fill_max_size().padding(32.dp()).wrap_content_size(), {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.width(300.dp()).test_tag(TAG),
                    text_style = TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT },
                    line_limits = line_limits,
                );
            });
        },
    );
    ui.host().set_input_mode(InputMode::Touch);
    ui.host().set_view_configuration(Some(ViewConfiguration {
        touch_slop: f32::MIN_POSITIVE,
        minimum_touch_target_size: compose_ui::DpSize::new(0.dp(), 0.dp()),
        ..ViewConfiguration::default()
    }));
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).request_focus();
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        for _ in 0..4 {
            let (start, end) = if single_line {
                (touch.center_right(), touch.center_left())
            } else {
                (touch.bottom_center(), touch.top_center())
            };
            touch.swipe(start, end, 200);
            // Not a multi-tap.
            touch.advance_event_time(1000);
        }
    });
    ui.wait_for_idle();
    (ui, state)
}

fn text_layout(ui: &mut ComposeTestRule) -> TextLayoutResult {
    let mut results = Vec::new();
    ui.on_node_with_tag(TAG).perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    results.pop().expect("a text layout")
}

/// Where the text is in the field scrolled to its end, as Compose's `translateScroll`.
fn scroll_translation(ui: &mut ComposeTestRule, single_line: bool) -> Offset {
    let field = ui.on_node_with_tag(TAG).fetch_semantics_node().size();
    let text = text_layout(ui).size;
    if single_line {
        assert_eq!(field.height, text.height);
        Offset::new((field.width - text.width) as f32, 0.0)
    } else {
        assert_eq!(field.width, text.width);
        Offset::new(0.0, (field.height - text.height) as f32)
    }
}

/// Character `offset`'s box's left middle in the scrolled field, as Compose's
/// `positionForCharacterScrolled`.
fn position_for_character_scrolled(
    ui: &mut ComposeTestRule,
    single_line: bool,
    offset: usize,
) -> Offset {
    let scroll = scroll_translation(ui, single_line);
    let box_: Rect = text_layout(ui).get_bounding_box(offset).translate(scroll);
    Offset::new(box_.left, box_.center().y)
}

/// A finger held at `at` past the long-press timeout, as Compose's `longPress`.
fn long_press(touch: &mut TouchInjectionScope<'_>, at: Offset) {
    let timeout = touch.view_configuration().long_press_timeout_millis;
    touch.down(at);
    touch.advance_event_time(timeout + 10);
    touch.move_to(at);
}

/// The text of "text0" to "text9" apart by `separator`.
fn texts(separator: &str) -> String {
    (0..10).map(|index| format!("text{index}")).collect::<Vec<_>>().join(separator)
}

fn long_press_gesture(single_line: bool) {
    let separator = if single_line { " " } else { "\n" };
    let (mut ui, state) = scrolled_field(texts(separator), single_line);
    // "text8".
    let at = position_for_character_scrolled(&mut ui, single_line, 50);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| long_press(touch, at));
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(48, 53));
    // Back through "text7".
    let to = position_for_character_scrolled(&mut ui, single_line, 46);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.move_to(to));
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(42, 53));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(42, 53));
}

fn handle_gesture(single_line: bool) {
    let separator = if single_line { " " } else { "\n" };
    let (mut ui, state) = scrolled_field(texts(separator), single_line);
    // "text8".
    let at = position_for_character_scrolled(&mut ui, single_line, 50);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(at, duration);
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(48, 53));
    // The start handle pressed, then moved to the bottom left of character 45, as Compose's
    // `withHandlePressed` and `moveHandleToCharacter`: the finger goes by where the handle points
    // to there, in the text's coordinates (the handle's semantics').
    let handle = is_selection_handle(Some(Handle::SelectionStart));
    ui.on_node(handle.clone()).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(48, 53));
    let from = handle_info(&mut ui, Handle::SelectionStart).position;
    let to = text_layout(&mut ui).get_bounding_box(45).bottom_left();
    ui.on_node(handle.clone()).perform_touch_input(|touch| touch.move_by(to - from));
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(42, 53));
    ui.on_node(handle).perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(42, 53));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/gesture/TextFieldScrolledSelectionGestureTest.kt, whenHorizontalScroll_longPressGesture_selectAndDrag
#[test]
fn a_long_press_in_a_line_scrolled_to_its_end_selects_and_drags_by_words() {
    long_press_gesture(true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/gesture/TextFieldScrolledSelectionGestureTest.kt, whenHorizontalScroll_handleGesture_drag
#[test]
fn a_handle_in_a_line_scrolled_to_its_end_drags_its_end() {
    handle_gesture(true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/gesture/TextFieldScrolledSelectionGestureTest.kt, whenVerticalScroll_longPressGesture_selectAndDrag
#[test]
fn a_long_press_in_lines_scrolled_to_their_end_selects_and_drags_by_words() {
    long_press_gesture(false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/gesture/TextFieldScrolledSelectionGestureTest.kt, whenVerticalScroll_handleGesture_drag
#[test]
fn a_handle_in_lines_scrolled_to_their_end_drags_its_end() {
    handle_gesture(false);
}
