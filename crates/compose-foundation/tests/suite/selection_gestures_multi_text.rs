//! A `SelectionContainer`'s gestures over several texts and over clipped ones, from Compose's
//! `MultiText2dSelectionGesturesTest` (a long press dragged over a 3×3 grid of texts),
//! `MultiTextRowRegressionTest` (an end handle dragged over a row of texts),
//! `MultiTextWithSpaceSelectionGesturesRegressionTest` (a double click dragged past texts of
//! different widths) and `ClippedTextSelectionGesturesTest` (a long press and a handle dragged
//! below a text cut at two lines).
//!
//! As Compose's tests (`AbstractSelectionGesturesTest`): the test font at 15 sp, density 1, a touch
//! slop of 0.1, the container in a box inset by 32 dp that wraps it, in a window of a phone's
//! pixels (1080 by 1920: the grid's rows, 549 px wide, fit as on Compose's test devices, where a
//! density of 1 makes pixels of the device's). A selection is checked as
//! Compose's `Selection`: each end's text and offset, the start where it began (its anchor).

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, NodeId};
use compose_ui::{
    Alignment, Density, HapticFeedbackType, InputMode, IntSize, Offset, Rect, TextLayoutResult,
    TextOverflow, UnitExt,
};

use crate::text_field_touch_support::*;
use crate::text_selection_gestures::{CURSORS, Cursor, CursorScope};

const TAG: &str = "selectionContainer";
const FONT: f32 = 15.0;

thread_local! {
    static MOVES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

struct CountingHaptics;

impl compose_ui::HapticFeedback for CountingHaptics {
    fn perform_haptic_feedback(&self, feedback: HapticFeedbackType) {
        if feedback == HapticFeedbackType::TextHandleMove {
            MOVES.with(|count| count.set(count.get() + 1));
        }
    }
}

fn style() -> TextStyle {
    TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT }
}

/// As Compose's `setTextFieldTestContent` around `Content()`, the container tagged.
fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    MOVES.with(|count| count.set(0));
    let content = Rc::new(content);
    let mut ui = compose_test_with(
        IntSize::new(1080, 1920),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let haptics: Rc<dyn compose_ui::HapticFeedback> = Rc::new(CountingHaptics);
            let content = content.clone();
            compose_runtime::CompositionLocalProvider(
                compose_ui::LOCAL_HAPTIC_FEEDBACK.provides(haptics),
                move || {
                    let content = content.clone();
                    Box!(
                        modifier = Modifier.padding(32.dp()).fill_max_size().wrap_content_size(),
                        {
                            content();
                        }
                    );
                },
            );
        },
    );
    ui.host().set_input_mode(InputMode::Touch);
    ui.host().set_view_configuration(Some(compose_ui::ViewConfiguration {
        touch_slop: 0.1,
        minimum_touch_target_size: compose_ui::DpSize::ZERO,
        ..compose_ui::ViewConfiguration::default()
    }));
    ui.wait_for_idle();
    ui
}

/// The container's bounds in its coordinates.
fn bounds(ui: &mut ComposeTestRule) -> Rect {
    let size = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds.size();
    Rect::new(0.0, 0.0, size.width, size.height)
}

fn layout(ui: &mut ComposeTestRule, tag: &str) -> TextLayoutResult {
    let mut results = Vec::new();
    ui.on_node_with_tag(tag).perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    results.pop().expect("a text layout")
}

/// The box of `tag`'s character at `offset` (bytes, one per character here), in the container's
/// coordinates (`translate(nodePosition - pointerAreaPosition)`).
fn character_box(ui: &mut ComposeTestRule, tag: &str, offset: usize) -> Rect {
    let text = ui.on_node_with_tag(tag).fetch_semantics_snapshot().bounds;
    let area = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    let rect = layout(ui, tag).get_bounding_box(offset);
    rect.translate(Offset::new(text.left - area.left, text.top - area.top))
}

/// The middle of the character's left side, two pixels in, as the tests' `characterPosition`.
fn character(ui: &mut ComposeTestRule, tag: &str, offset: usize) -> Offset {
    let rect = character_box(ui, tag, offset);
    Offset::new(rect.left + 2.0, rect.center().y)
}

fn node(ui: &mut ComposeTestRule, tag: &str) -> NodeId {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().id
}

/// The selection, each end's text and offset, as Compose's `Selection` (`start`, `end`).
fn selection(ui: &mut ComposeTestRule) -> Option<((NodeId, usize), (NodeId, usize))> {
    ui.host().with_tree(|tree| tree.selection_anchors())
}

#[track_caller]
fn assert_selection(ui: &mut ComposeTestRule, start: (&str, usize), end: (&str, usize)) {
    let expected = ((node(ui, start.0), start.1), (node(ui, end.0), end.1));
    assert_eq!(selection(ui), Some(expected));
}

fn touch(ui: &mut ComposeTestRule, block: impl FnOnce(&mut TouchInjectionScope<'_>)) {
    ui.on_node_with_tag(TAG).perform_touch_input(block);
    ui.wait_for_idle();
}

/// A finger held at `at` past the long-press timeout, as Compose's `longPress`.
fn long_press(ui: &mut ComposeTestRule, at: Offset) {
    touch(ui, |touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(at);
        touch.move_pointers_after(timeout + 10);
    });
}

/// The finger moved to `to` over 200 ms, a move each event period, as `touchDragNodeTo`.
fn touch_drag_to(ui: &mut ComposeTestRule, to: Offset) {
    touch(ui, |touch| {
        let from = touch.current_position(0).expect("a finger down");
        let period = touch.event_period_millis();
        let steps = ((200.0 / period as f32).round() as u64).max(1);
        let mut previous = 0;
        for step in 1..=steps {
            let time = 200 * step / steps;
            let fraction = time as f32 / 200.0;
            touch.move_pointer_to(0, from + (to - from) * fraction, time - previous);
            previous = time;
        }
    });
}

fn up(ui: &mut ComposeTestRule) {
    touch(ui, |touch| touch.up());
}

/// The `TextHandleMove` haptics since the last call.
fn moves() -> u32 {
    MOVES.with(|count| count.replace(0))
}

/// The handles shown, each as its semantics say (`SelectionHandleInfo.visible`).
fn handles_shown(ui: &mut ComposeTestRule) -> (bool, bool) {
    let mut shown = |handle: Handle| {
        let nodes = ui.on_all_nodes(is_selection_handle(Some(handle))).fetch_semantics_nodes();
        match nodes.as_slice() {
            [] => false,
            [node] => node.config().get(&SELECTION_HANDLE_INFO_KEY).expect("info").visible,
            _ => panic!("found several {handle:?} handles"),
        }
    };
    (shown(Handle::SelectionStart), shown(Handle::SelectionEnd))
}

/// A handle pressed, as Compose's `withHandlePressed`: down at its center; the gesture's place is
/// where the handle points, in the container's coordinates (`HandlePressedScope`).
fn press_handle(ui: &mut ComposeTestRule, handle: Handle) -> Offset {
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.wait_for_idle();
    handle_info(ui, handle).position
}

/// The pressed handle's finger moved so the gesture goes to `to`, as `moveHandleTo`.
fn move_handle_to(ui: &mut ComposeTestRule, handle: Handle, gesture: &mut Offset, to: Offset) {
    let delta = to - *gesture;
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        touch.move_by(delta);
    });
    ui.wait_for_idle();
    *gesture = to;
}

fn release_handle(ui: &mut ComposeTestRule, handle: Handle) {
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
}

// ---- MultiText2dSelectionGesturesTest ------------------------------------------------------

const LINE: &str = "Test Text";

/// A 3×3 grid of texts of two lines, `1` to `9` row by row, centered in a container filling the
/// window, each text padded by 24 dp.
fn grid() -> ComposeTestRule {
    rule(|| {
        SelectionContainer!(modifier = Modifier.test_tag(TAG), {
            Column!(
                modifier = Modifier.fill_max_size(),
                vertical_arrangement = Arrangement::CENTER,
                horizontal_alignment = Alignment::CENTER_HORIZONTALLY,
                {
                    for i in 0..3 {
                        Row!(
                            vertical_alignment = Alignment::CENTER_VERTICALLY,
                            horizontal_arrangement = Arrangement::CENTER,
                            {
                                for j in 0..3 {
                                    let tag = format!("{}", i * 3 + j + 1);
                                    BasicText!(
                                        format!("{LINE}\n{LINE}"),
                                        style = style(),
                                        modifier = Modifier.padding(24.dp()).test_tag(&tag),
                                    );
                                }
                            }
                        );
                    }
                }
            );
        });
    })
}

/// Halfway between the centers of texts `a` and `b`, as the test's `betweenSelectables`.
fn between(ui: &mut ComposeTestRule, a: usize, b: usize) -> Offset {
    let area = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    let mut center = |id: usize| {
        let rect = ui.on_node_with_tag(&id.to_string()).fetch_semantics_snapshot().bounds;
        rect.center() - Offset::new(area.left, area.top)
    };
    let (a, b) = (center(a), center(b));
    a + (b - a) * 0.5
}

/// A long press on text 5's "Text" (its offset 6), dragged to `to`: the selection from text 5's
/// word to `end` (text, offset), crossed (begun at the word's end) or not, as the test's
/// `dragTest`.
fn drag_test(to: impl FnOnce(&mut ComposeTestRule) -> Offset, end: (usize, usize), crossed: bool) {
    let mut ui = grid();
    let at = character(&mut ui, "5", 6);
    long_press(&mut ui, at);
    assert_selection(&mut ui, ("5", 5), ("5", 9));
    let to = to(&mut ui);
    touch_drag_to(&mut ui, to);
    let start = ("5", if crossed { 9 } else { 5 });
    let end_tag = end.0.to_string();
    assert_selection(&mut ui, start, (&end_tag, end.1));
    up(&mut ui);
    assert_selection(&mut ui, start, (&end_tag, end.1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragUpperLeftText
#[test]
fn dragUpperLeftText() {
    drag_test(|ui| character(ui, "1", 6), (1, 5), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragUpperCenterText
#[test]
fn dragUpperCenterText() {
    drag_test(|ui| character(ui, "2", 6), (2, 5), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragUpperRightText
#[test]
fn dragUpperRightText() {
    drag_test(|ui| character(ui, "3", 6), (3, 5), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragLeftText
#[test]
fn dragLeftText() {
    drag_test(|ui| character(ui, "4", 6), (4, 5), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragSameText
#[test]
fn dragSameText() {
    drag_test(|ui| character(ui, "5", 10), (5, 14), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragRightText
#[test]
fn dragRightText() {
    drag_test(|ui| character(ui, "6", 5), (6, 9), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragLowerLeftText
#[test]
fn dragLowerLeftText() {
    drag_test(|ui| character(ui, "7", 5), (7, 9), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragLowerCenterText
#[test]
fn dragLowerCenterText() {
    drag_test(|ui| character(ui, "8", 5), (8, 9), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragLowerRightText
#[test]
fn dragLowerRightText() {
    drag_test(|ui| character(ui, "9", 5), (9, 9), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragTopContainer
#[test]
fn dragTopContainer() {
    drag_test(
        |ui| {
            let bounds = bounds(ui);
            Offset::new(bounds.right - 1.0, bounds.top + 1.0)
        },
        (1, 0),
        true,
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragBetweenFirstAndSecondRow
#[test]
fn dragBetweenFirstAndSecondRow() {
    drag_test(|ui| between(ui, 2, 5), (4, 0), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragBetweenSecondAndThirdRow
#[test]
fn dragBetweenSecondAndThirdRow() {
    drag_test(|ui| between(ui, 5, 8), (6, 19), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragBottomContainer
#[test]
fn dragBottomContainer() {
    drag_test(
        |ui| {
            let bounds = bounds(ui);
            Offset::new(bounds.left + 1.0, bounds.bottom - 1.0)
        },
        (9, 19),
        false,
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragLeftContainer
#[test]
fn dragLeftContainer() {
    // Between the lines: two pixels up, on the upper one.
    drag_test(
        |ui| {
            let bounds = bounds(ui);
            Offset::new(bounds.left + 1.0, bounds.center().y - 2.0)
        },
        (4, 0),
        true,
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragBetweenLeftAndCenterTexts
#[test]
fn dragBetweenLeftAndCenterTexts() {
    drag_test(|ui| between(ui, 4, 5) + Offset::new(0.0, -2.0), (5, 0), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragBetweenCenterAndRightTexts
#[test]
fn dragBetweenCenterAndRightTexts() {
    drag_test(|ui| between(ui, 5, 6) + Offset::new(0.0, -2.0), (5, 9), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiText2dSelectionGesturesTest.kt, dragRightContainer
#[test]
fn dragRightContainer() {
    drag_test(
        |ui| {
            let bounds = bounds(ui);
            Offset::new(bounds.right - 1.0, bounds.center().y - 2.0)
        },
        (6, 9),
        false,
    );
}

// ---- MultiTextRowRegressionTest ------------------------------------------------------------

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiTextRowRegressionTest.kt, selectMiddle_moveEndHandleToLeft
#[test]
fn selectMiddle_moveEndHandleToLeft() {
    let mut ui = rule(|| {
        SelectionContainer!(modifier = Modifier.test_tag(TAG), {
            Column!(
                modifier = Modifier.fill_max_size(),
                vertical_arrangement = Arrangement::CENTER,
                horizontal_alignment = Alignment::CENTER_HORIZONTALLY,
                {
                    Row!(horizontal_arrangement = Arrangement::CENTER, {
                        BasicText!(
                            LINE.to_string(),
                            style = style(),
                            modifier =
                                Modifier.padding((0.dp(), 8.dp(), 0.dp(), 0.dp())).test_tag("1"),
                        );
                        BasicText!(
                            format!("{LINE}\n{LINE}\n{LINE}"),
                            style = style(),
                            modifier = Modifier.test_tag("2"),
                        );
                        BasicText!(
                            LINE.to_string(),
                            style = style(),
                            modifier =
                                Modifier.padding((0.dp(), 24.dp(), 0.dp(), 0.dp())).test_tag("3"),
                        );
                    });
                }
            );
        });
    });
    let at = character(&mut ui, "2", 0);
    long_press(&mut ui, at);
    assert_selection(&mut ui, ("2", 0), ("2", 4));
    // Compose's finger stays down in the container's window while the handle's popup, a window
    // of its own, takes another; here one injection holds both, so the finger lifts first.
    up(&mut ui);
    assert_selection(&mut ui, ("2", 0), ("2", 4));

    let handle = Handle::SelectionEnd;
    let mut gesture = press_handle(&mut ui, handle);
    let to = character(&mut ui, "2", 19);
    move_handle_to(&mut ui, handle, &mut gesture, to);
    release_handle(&mut ui, handle);
    assert_selection(&mut ui, ("2", 0), ("2", 19));

    // Now to the top left: no crash, and the selection expected.
    let mut gesture = press_handle(&mut ui, handle);
    let to = character(&mut ui, "1", 0);
    move_handle_to(&mut ui, handle, &mut gesture, to);
    release_handle(&mut ui, handle);
    assert_selection(&mut ui, ("2", 0), ("1", 0));
}

// ---- MultiTextWithSpaceSelectionGesturesRegressionTest -------------------------------------

const SPACED: &str = "line1\nline2 text1 text2!\nline3";

/// The texts of `SPACED`'s lines, `testTag0` to `testTag2`, in a column wrapping them.
fn spaced() -> ComposeTestRule {
    rule(|| {
        SelectionContainer!(modifier = Modifier.test_tag(TAG), {
            Column!({
                for (index, line) in SPACED.split('\n').enumerate() {
                    let tag = format!("testTag{index}");
                    BasicText!(
                        line.to_string(),
                        style = style(),
                        modifier = Modifier.test_tag(&tag)
                    );
                }
            });
        });
    })
}

/// Offset `offset` of `SPACED` as its text's tag and offset, each line's to its new line
/// (`offsetToSelectableId`, `offsetToLocalOffset`).
fn spaced_place(offset: usize) -> (String, usize) {
    let mut start = 0;
    for (index, line) in SPACED.split('\n').enumerate() {
        if offset <= start + line.len() {
            return (format!("testTag{index}"), offset - start);
        }
        start += line.len() + 1;
    }
    panic!("offset {offset} past the text");
}

#[track_caller]
fn assert_spaced_selection(ui: &mut ComposeTestRule, start: usize, end: usize) {
    let (start, end) = (spaced_place(start), spaced_place(end));
    assert_selection(ui, (&start.0, start.1), (&end.0, end.1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/MultiTextWithSpaceSelectionGesturesRegressionTest.kt, whenMouse_withDoubleClickThenDragUpAndDown_selectsWords, whenTrackpad_withDoubleClickThenDragUpAndDown_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragUpAndDown_selectsWords() {
    for kind in CURSORS {
        let mut ui = spaced();
        let (tag, offset) = spaced_place(18);
        // The character's left side's middle, not nudged, as this test's `characterPosition`.
        let rect = character_box(&mut ui, &tag, offset);
        let at = Offset::new(rect.left, rect.center().y);
        cursor(&mut ui, kind, |cursor| {
            cursor.move_to(at);
            cursor.press();
            cursor.advance_event_time(16);
            cursor.release();
            cursor.advance_event_time(16);
            cursor.press();
        });
        assert_spaced_selection(&mut ui, 18, 23);
        let bounds = bounds(&mut ui);
        cursor_drag_to(&mut ui, kind, Offset::new(bounds.right - 1.0, bounds.top + 1.0));
        assert_spaced_selection(&mut ui, 24, 6);
        cursor_drag_to(&mut ui, kind, Offset::new(bounds.right - 1.0, bounds.bottom - 1.0));
        assert_spaced_selection(&mut ui, 18, 30);
        cursor(&mut ui, kind, |cursor| cursor.release());
        assert_spaced_selection(&mut ui, 18, 30);
    }
}

/// A mouse's or a trackpad's input on the container.
fn cursor(ui: &mut ComposeTestRule, kind: Cursor, block: impl FnOnce(&mut CursorScope<'_, '_>)) {
    match kind {
        Cursor::Mouse => {
            ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
                block(&mut CursorScope::Mouse(mouse));
            });
        }
        Cursor::Trackpad => {
            ui.on_node_with_tag(TAG).perform_trackpad_input(|trackpad| {
                block(&mut CursorScope::Trackpad(trackpad));
            });
        }
    }
    ui.wait_for_idle();
}

/// The cursor moved to `to` over 200 ms, as `mouseDragNodeTo` and `trackpadDragNodeTo`.
fn cursor_drag_to(ui: &mut ComposeTestRule, kind: Cursor, to: Offset) {
    cursor(ui, kind, |cursor| {
        let from = cursor.current_position();
        let steps = ((200.0 / 16.0f32).round() as u64).max(1);
        let mut previous = 0;
        for step in 1..=steps {
            let time = 200 * step / steps;
            let fraction = time as f32 / 200.0;
            cursor.move_to_after(from + (to - from) * fraction, time - previous);
            previous = time;
        }
    });
}

// ---- ClippedTextSelectionGesturesTest ------------------------------------------------------

const TEXT_TAG: &str = "textTag";

fn clipped_text() -> String {
    format!("Text {}", "Text".repeat(19))
}

/// A text 100 dp wide cut at two lines with `overflow`, centered in a box padded by 32 dp.
fn clipped(overflow: TextOverflow) -> ComposeTestRule {
    rule(move || {
        SelectionContainer!(modifier = Modifier.test_tag(TAG), {
            Box!(modifier = Modifier.padding(32.dp()), content_alignment = Alignment::CENTER, {
                BasicText!(
                    clipped_text(),
                    style = style(),
                    max_lines = 2,
                    overflow = overflow,
                    modifier = Modifier.width(100.dp()).test_tag(TEXT_TAG),
                );
            });
        });
    })
}

#[track_caller]
fn assert_clipped_selection(ui: &mut ComposeTestRule, start: usize, end: usize) {
    assert_selection(ui, (TEXT_TAG, start), (TEXT_TAG, end));
}

/// As the test's `dragBelowTextTest`.
fn drag_below_text(overflow: TextOverflow) {
    let mut ui = clipped(overflow);
    assert_eq!(selection(&mut ui), None);
    assert_eq!(handles_shown(&mut ui), (false, false));
    let at = character(&mut ui, TEXT_TAG, 2);
    long_press(&mut ui, at);
    assert_clipped_selection(&mut ui, 0, 4);
    assert_eq!(handles_shown(&mut ui), (true, true));
    moves();
    let bounds = bounds(&mut ui);
    touch_drag_to(&mut ui, Offset::new(bounds.left + 1.0, bounds.bottom - 1.0));
    assert_clipped_selection(&mut ui, 0, clipped_text().len());
    assert!(moves() >= 1);
    // Dragged back: the gesture goes on.
    let to = character(&mut ui, TEXT_TAG, 2);
    touch_drag_to(&mut ui, to);
    assert_clipped_selection(&mut ui, 0, 4);
    assert!(moves() >= 1);
    up(&mut ui);
    assert_clipped_selection(&mut ui, 0, 4);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/ClippedTextSelectionGesturesTest.kt, whenDragBelowText_withOverflowClip_entireTextSelected
#[test]
fn whenDragBelowText_withOverflowClip_entireTextSelected() {
    drag_below_text(TextOverflow::Clip);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/ClippedTextSelectionGesturesTest.kt, whenDragBelowText_withOverflowVisible_entireTextSelected
#[test]
fn whenDragBelowText_withOverflowVisible_entireTextSelected() {
    drag_below_text(TextOverflow::Visible);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/ClippedTextSelectionGesturesTest.kt, whenDragEndHandleOutOfBounds_selectionAndHandleUpdates
#[test]
fn whenDragEndHandleOutOfBounds_selectionAndHandleUpdates() {
    let mut ui = clipped(TextOverflow::Clip);
    assert_eq!(selection(&mut ui), None);
    let at = character(&mut ui, TEXT_TAG, 4);
    long_press(&mut ui, at);
    let bounds = bounds(&mut ui);
    touch_drag_to(&mut ui, Offset::new(bounds.left + 1.0, bounds.bottom - 1.0));
    assert_clipped_selection(&mut ui, 0, clipped_text().len());
    assert_eq!(handles_shown(&mut ui), (true, true));
    assert!(moves() >= 1);
    let to = character(&mut ui, TEXT_TAG, 2);
    touch_drag_to(&mut ui, to);
    assert_clipped_selection(&mut ui, 0, 4);
    assert_eq!(handles_shown(&mut ui), (true, true));
    up(&mut ui);
    assert_clipped_selection(&mut ui, 0, 4);
    moves();

    let handle = Handle::SelectionEnd;
    let mut gesture = press_handle(&mut ui, handle);
    let to = Offset::new(bounds.right - 1.0, bounds.bottom - 1.0);
    move_handle_to(&mut ui, handle, &mut gesture, to);
    assert_clipped_selection(&mut ui, 0, clipped_text().len());
    assert!(moves() >= 1);
    // To character 2: the end handle's place, the box's bottom left nudged to the start.
    let corner = character_box(&mut ui, TEXT_TAG, 2).bottom_left();
    move_handle_to(&mut ui, handle, &mut gesture, corner + Offset::new(-2.0, 0.0));
    assert_clipped_selection(&mut ui, 0, 4);
    assert!(moves() >= 1);
    release_handle(&mut ui, handle);
    assert_clipped_selection(&mut ui, 0, 4);
    assert_eq!(handles_shown(&mut ui), (true, true));
}
