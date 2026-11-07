//! A `SelectionContainer` around a lazy column of texts, from Compose's
//! `LazyColumnMultiTextRegressionTest`: the selected items stay composed scrolled away (pinned,
//! D2023), a selection is made again after its texts left and came back, copy reads texts
//! scrolled away, the handles hide and show as their texts scroll.
//!
//! As Compose's tests: density 1, the test font at 15 sp, touch input mode, no minimum touch
//! target; the container 100 dp high in the middle of the window.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, NodeId};
use compose_ui::{
    Alignment, Density, InputMode, IntSize, Key, KeyEvent, KeyEventType, KeyModifiers, Offset,
    Rect, TextAlign, TextLayoutResult, UnitExt,
};

use crate::text_field_touch_support::*;

const TAG: &str = "tag";
const TEXTS: usize = 20;

type Selection = Option<((NodeId, usize), (NodeId, usize))>;

/// As Compose's `runTest`'s content.
fn content() {
    Box!(modifier = Modifier.fill_max_size(), content_alignment = Alignment::CENTER, {
        SelectionContainer!(modifier = Modifier.height(100.dp()), {
            LazyColumn!(modifier = Modifier.test_tag(TAG), |scope: LazyListScope| {
                scope.items(TEXTS, |index| {
                    BasicText!(
                        index.to_string(),
                        style = TextStyle {
                            font_size: 15.sp(),
                            text_align: Some(TextAlign::Center),
                            ..TextStyle::DEFAULT
                        },
                        modifier = Modifier.fill_max_width().test_tag(index.to_string()),
                    );
                });
            });
        });
    });
}

/// The rule over `content`, set by `restoration` if given (`StateRestorationTester`).
fn rule_with(restoration: Option<&mut StateRestorationTester>) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let size = IntSize::new(400, 400);
    let mut ui = match restoration {
        None => compose_test_with(size, Density::new(1.0), Box::new(measurer), content),
        Some(tester) => {
            let mut ui = compose_test_with(size, Density::new(1.0), Box::new(measurer), || {});
            tester.set_content(&mut ui, content);
            ui
        }
    };
    ui.host().set_input_mode(InputMode::Touch);
    ui.host().set_view_configuration(Some(compose_ui::ViewConfiguration {
        minimum_touch_target_size: compose_ui::DpSize::ZERO,
        ..compose_ui::ViewConfiguration::default()
    }));
    ui.wait_for_idle();
    ui
}

fn rule() -> ComposeTestRule {
    rule_with(None)
}

fn selection(ui: &mut ComposeTestRule) -> Selection {
    ui.host().with_tree(|tree| tree.selection_anchors())
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

/// The box of line `line`'s first line in the pointer area, as `boundingBoxForLineInPointerArea`.
fn line_box(ui: &mut ComposeTestRule, line: usize) -> Rect {
    let tag = line.to_string();
    let text = ui.on_node_with_tag(&tag).fetch_semantics_snapshot().bounds;
    let area = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    let layout = layout(ui, &tag);
    let (start, end) = (layout.get_line_start(0), layout.get_line_end(0, false));
    let first = layout.get_bounding_box(start);
    let last = layout.get_bounding_box(end.saturating_sub(1).max(start));
    let rect = Rect::new(
        first.left.min(last.left),
        first.top.min(last.top),
        first.right.max(last.right),
        first.bottom.max(last.bottom),
    );
    rect.translate(Offset::new(text.left - area.left, text.top - area.top))
}

fn line_center(ui: &mut ComposeTestRule, line: usize) -> Offset {
    line_box(ui, line).center()
}

fn touch(ui: &mut ComposeTestRule, block: impl FnOnce(&mut TouchInjectionScope<'_>)) {
    ui.on_node_with_tag(TAG).perform_touch_input(block);
    ui.wait_for_idle();
}

fn create_selection(ui: &mut ComposeTestRule, line: usize) {
    let at = line_center(ui, line);
    touch(ui, |touch| {
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(at, duration);
    });
}

fn create_selection_over(ui: &mut ComposeTestRule, start: usize, end: usize) {
    let (from, to) = (line_center(ui, start), line_center(ui, end));
    touch(ui, |touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(from);
        touch.move_pointers_after(timeout + 10);
        touch.move_to(to);
        touch.up();
    });
}

fn scroll_down(ui: &mut ComposeTestRule) {
    touch(ui, |touch| {
        let (top, bottom) = (touch.top_center(), touch.bottom_center());
        touch.swipe(bottom - Offset::new(0.0, 1.0), top + Offset::new(0.0, 1.0), 200);
    });
}

fn scroll_up(ui: &mut ComposeTestRule) {
    touch(ui, |touch| {
        let (top, bottom) = (touch.top_center(), touch.bottom_center());
        touch.swipe(top + Offset::new(0.0, 1.0), bottom - Offset::new(0.0, 1.0), 200);
    });
}

fn scroll_lines(ui: &mut ComposeTestRule, from: usize, to: usize) {
    let (from, to) = (line_center(ui, from), line_center(ui, to));
    touch(ui, |touch| touch.swipe(from, to, 200));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/LazyColumnMultiTextRegressionTest.kt, whenTextScrollsOutOfCompositionAndThenBackIn_creatingSelectionStillPossible
#[test]
fn whenTextScrollsOutOfCompositionAndThenBackIn_creatingSelectionStillPossible() {
    let mut ui = rule();
    scroll_down(&mut ui);
    scroll_up(&mut ui);
    create_selection(&mut ui, 0);
    assert!(selection(&mut ui).is_some());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/LazyColumnMultiTextRegressionTest.kt, whenSelectionScrollsOutOfCompositionAndThenBackIn_selectionRemains
#[test]
fn whenSelectionScrollsOutOfCompositionAndThenBackIn_selectionRemains() {
    let mut ui = rule();
    create_selection(&mut ui, 0);
    let initial = selection(&mut ui);
    assert!(initial.is_some());
    scroll_down(&mut ui);
    assert_eq!(selection(&mut ui), initial);
    scroll_up(&mut ui);
    assert_eq!(selection(&mut ui), initial);
}

fn copy(ui: &mut ComposeTestRule) {
    let modifiers = if cfg!(target_os = "macos") {
        KeyModifiers { meta: true, ..Default::default() }
    } else {
        KeyModifiers { ctrl: true, ..Default::default() }
    };
    for kind in [KeyEventType::KeyDown, KeyEventType::KeyUp] {
        let key = Key::Character('c');
        ui.host().send_key(KeyEvent { key, kind, modifiers, from_soft_keyboard: false });
    }
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/LazyColumnMultiTextRegressionTest.kt, whenTextScrollsOutOfLazyLayoutBounds_copyCorrectlySetsClipboard
#[test]
fn whenTextScrollsOutOfLazyLayoutBounds_copyCorrectlySetsClipboard() {
    let mut ui = rule();
    ui.host().with_tree(|tree| tree.clipboard().set_text("Initial text"));
    create_selection_over(&mut ui, 0, 4);
    assert!(selection(&mut ui).is_some());
    scroll_down(&mut ui);
    copy(&mut ui);
    let copied = ui.host().with_tree(|tree| tree.clipboard().get_text());
    assert_eq!(copied.as_deref(), Some("0\n1\n2\n3\n4"));
}

fn handle_position(ui: &mut ComposeTestRule, handle: Handle) -> Option<Offset> {
    let nodes = ui.on_all_nodes(is_selection_handle(Some(handle))).fetch_semantics_nodes();
    match nodes.as_slice() {
        [node] => node.config().get(&SELECTION_HANDLE_INFO_KEY).map(|info| info.position),
        _ => None,
    }
}

#[track_caller]
fn assert_shown(ui: &mut ComposeTestRule, start: bool, end: bool) {
    for (handle, shown) in [(Handle::SelectionStart, start), (Handle::SelectionEnd, end)] {
        let nodes = ui.on_all_nodes(is_selection_handle(Some(handle))).fetch_semantics_nodes();
        let visible = match nodes.as_slice() {
            [] => false,
            [node] => node.config().get(&SELECTION_HANDLE_INFO_KEY).expect("info").visible,
            _ => panic!("several {handle:?} handles"),
        };
        assert_eq!(visible, shown, "{handle:?} shown");
    }
}

#[track_caller]
fn assert_moved(previous: Option<Offset>, current: Option<Offset>, up: bool) {
    let (previous, current) = (previous.expect("a previous place"), current.expect("a place"));
    assert!((current.x - previous.x).abs() <= 0.1, "x moved: {previous:?} to {current:?}");
    assert!(if up { current.y < previous.y } else { current.y > previous.y }, "{up}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/LazyColumnMultiTextRegressionTest.kt, whenScrollingTextOutOfViewUpwards_handlesDisappear
#[test]
fn whenScrollingTextOutOfViewUpwards_handlesDisappear() {
    let mut ui = rule();
    let start = Handle::SelectionStart;
    let end = Handle::SelectionEnd;
    assert_shown(&mut ui, false, false);
    create_selection_over(&mut ui, 1, 3);
    assert_shown(&mut ui, true, true);
    let positions = |ui: &mut ComposeTestRule| {
        (handle_position(ui, Handle::SelectionStart), handle_position(ui, Handle::SelectionEnd))
    };
    let (previous_start, previous_end) = positions(&mut ui);

    scroll_lines(&mut ui, 3, 2);
    assert_shown(&mut ui, true, true);
    assert_moved(previous_start, handle_position(&mut ui, start), true);
    assert_moved(previous_end, handle_position(&mut ui, end), true);
    let (_, previous_end) = positions(&mut ui);

    scroll_lines(&mut ui, 4, 2);
    assert_shown(&mut ui, false, true);
    assert_moved(previous_end, handle_position(&mut ui, end), true);

    scroll_lines(&mut ui, 6, 4);
    assert_shown(&mut ui, false, false);

    scroll_lines(&mut ui, 6, 8);
    assert_shown(&mut ui, false, true);
    let (_, previous_end) = positions(&mut ui);

    scroll_lines(&mut ui, 4, 6);
    assert_shown(&mut ui, true, true);
    assert_moved(previous_end, handle_position(&mut ui, end), false);
    let (previous_start, previous_end) = positions(&mut ui);

    scroll_lines(&mut ui, 2, 5);
    assert_shown(&mut ui, true, true);
    assert_moved(previous_start, handle_position(&mut ui, start), false);
    assert_moved(previous_end, handle_position(&mut ui, end), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/LazyColumnMultiTextRegressionTest.kt, whenTextIsSavedRestored_clearsSelection
#[test]
fn whenTextIsSavedRestored_clearsSelection() {
    let mut tester = StateRestorationTester::new();
    let mut ui = rule_with(Some(&mut tester));
    create_selection(&mut ui, 0);
    assert!(selection(&mut ui).is_some());
    tester.emulate_save_and_restore(&mut ui);
    assert_eq!(selection(&mut ui), None);
}

/// The box of line `line`'s first line in the root, as `boundingBoxForLineInRoot`.
fn line_box_in_root(ui: &mut ComposeTestRule, line: usize) -> Rect {
    let area = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    line_box(ui, line).translate(Offset::new(area.left, area.top))
}

/// Where the toolbar goes, its top, as `assertTextToolbarTopAt` (the spy's `contentRect`, in the
/// window: the root here).
#[track_caller]
fn assert_text_toolbar_top_at(ui: &mut ComposeTestRule, y: f32) {
    let top = ui.text_toolbar().content_rect().map(|rect| rect.top);
    let top = top.expect("a toolbar's rect");
    assert!((top - y).abs() <= 0.1, "the toolbar's top {top}, expected {y}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/LazyColumnMultiTextRegressionTest.kt, whenScrollingTextOutOfViewUpwards_textToolbarCoercedToTop
#[test]
fn whenScrollingTextOutOfViewUpwards_textToolbarCoercedToTop() {
    let mut ui = rule();
    let toolbar = ui.text_toolbar();
    let pointer_area_top =
        |ui: &mut ComposeTestRule| ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds.top;
    assert!(!toolbar.is_shown());

    create_selection_over(&mut ui, 1, 3);
    assert!(toolbar.is_shown());
    let top = line_box_in_root(&mut ui, 1).top;
    assert_text_toolbar_top_at(&mut ui, top);

    scroll_lines(&mut ui, 3, 1);
    assert!(toolbar.is_shown());
    let top = pointer_area_top(&mut ui);
    assert_text_toolbar_top_at(&mut ui, top);

    scroll_lines(&mut ui, 5, 3);
    let top = pointer_area_top(&mut ui);
    assert_text_toolbar_top_at(&mut ui, top);

    scroll_lines(&mut ui, 5, 7);
    assert!(toolbar.is_shown());
    let top = pointer_area_top(&mut ui);
    assert_text_toolbar_top_at(&mut ui, top);

    scroll_lines(&mut ui, 3, 5);
    assert!(toolbar.is_shown());
    let top = line_box_in_root(&mut ui, 1).top;
    assert_text_toolbar_top_at(&mut ui, top);
}
