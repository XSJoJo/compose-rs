//! A `SelectionContainer` under a finger: its long press selects a word, a tap clears the
//! selection, and its handles show and drag the selection's ends across texts, from Compose's
//! `SelectionContainerTest` and `SelectionContainerFocusTest` (their touch parts).
//!
//! As Compose's tests: the test font at 20 sp (each character 20 px square), density 1, touch
//! input mode, the clock advanced by hand.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, NodeId};
use compose_ui::{
    Density, EventContext, InputMode, IntSize, LayoutDirection, ModifierNode, ModifierNodeElement,
    Offset, PointerEvent, PointerEventPass, PointerEventType, PointerInputModifierNode, Rect,
    TextLayoutResult, UnitExt,
};

use crate::text_field_touch_support::*;

const TEXT: &str = "Text Demo Text";
const FONT: f32 = 20.0;
const TAG1: &str = "tag1";
const TAG2: &str = "tag2";
const CONTAINER: &str = "selectionContainer";

thread_local! {
    /// The `TextHandleMove` feedbacks performed, as Compose's tests' mocked `HapticFeedback`.
    static HAPTICS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

struct CountingHaptics;

impl compose_ui::HapticFeedback for CountingHaptics {
    fn perform_haptic_feedback(&self, feedback: compose_ui::HapticFeedbackType) {
        if feedback == compose_ui::HapticFeedbackType::TextHandleMove {
            HAPTICS.with(|count| count.set(count.get() + 1));
        }
    }
}

fn haptics() -> u32 {
    HAPTICS.with(|count| count.get())
}

/// Provides the counting haptics around `content`.
fn with_haptics(content: impl Fn() + 'static) {
    let haptics: Rc<dyn compose_ui::HapticFeedback> = Rc::new(CountingHaptics);
    compose_runtime::CompositionLocalProvider(
        compose_ui::LOCAL_HAPTIC_FEEDBACK.provides(haptics),
        content,
    );
}

fn style() -> TextStyle {
    TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT }
}

/// A rule over `content`, as Compose's `createSelectionContainer`: its parent tagged, the
/// container in it, the layout direction `rtl` or not.
fn container_with(rtl: bool, content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let content = Rc::new(content);
    HAPTICS.with(|count| count.set(0));
    let mut ui = compose_test_with(
        IntSize::new(400, 400),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let content = content.clone();
            let direction = if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            with_haptics(move || {
                let content = content.clone();
                compose_runtime::CompositionLocalProvider(
                    compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                    move || {
                        let content = content.clone();
                        Box!(modifier = Modifier.test_tag(CONTAINER), {
                            let content = content.clone();
                            SelectionContainer!({ content() });
                        });
                    },
                );
            });
        },
    );
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_auto_advance(false);
    ui.wait_for_idle();
    ui
}

/// The container over one text filling it, as Compose's default content (`TestText`).
fn container(rtl: bool) -> ComposeTestRule {
    container_with(rtl, || {
        BasicText!(TEXT, modifier = Modifier.fill_max_size(), style = style());
    })
}

/// Two texts in a column, each filling the width, the second under `DisableSelection` or not.
fn two_texts(disabled: bool) -> ComposeTestRule {
    container_with(false, move || {
        Column!({
            BasicText!(TEXT, modifier = Modifier.fill_max_width().test_tag(TAG1), style = style());
            if disabled {
                DisableSelection!({
                    BasicText!(
                        TEXT,
                        modifier = Modifier.fill_max_width().test_tag(TAG2),
                        style = style()
                    );
                });
            } else {
                BasicText!(
                    TEXT,
                    modifier = Modifier.fill_max_width().test_tag(TAG2),
                    style = style()
                );
            }
        });
    })
}

/// The selection's ends: each end's text and offset, as Compose's `state.selection`.
fn selection(ui: &mut ComposeTestRule) -> Option<((NodeId, usize), (NodeId, usize))> {
    ui.host().with_tree(|tree| tree.selection_anchors())
}

fn node_of(ui: &mut ComposeTestRule, tag: &str) -> NodeId {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().id
}

/// As Compose's `assertAnchorInfo`: the end is `offset` in the text tagged `tag`.
#[track_caller]
fn assert_anchor(ui: &mut ComposeTestRule, end: Option<(NodeId, usize)>, offset: usize, tag: &str) {
    let node = node_of(ui, tag);
    assert_eq!(end, Some((node, offset)), "expected offset {offset} in {tag}");
}

fn text_layout(ui: &mut ComposeTestRule, tag: &str) -> TextLayoutResult {
    let mut results = Vec::new();
    ui.on_node_with_tag(tag).perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    results.pop().expect("a text layout")
}

/// Character `offset`'s box in the root, as Compose's `characterBox`.
fn character_box(ui: &mut ComposeTestRule, tag: &str, offset: usize) -> Rect {
    let position = ui.on_node_with_tag(tag).fetch_semantics_snapshot().bounds;
    let layout = text_layout(ui, tag);
    layout.get_bounding_box(offset).translate(Offset::new(position.left, position.top))
}

/// A long press on character `offset` of the text tagged `tag`, as Compose's `startSelection`.
fn start_selection(ui: &mut ComposeTestRule, tag: &str, offset: usize) {
    let center = text_layout(ui, tag).get_bounding_box(offset).center();
    ui.on_node_with_tag(tag).perform_touch_input(|touch| touch.long_click_at(center, 600));
    ui.wait_for_idle();
}

/// Drags `handle` so that where it points goes to `to` (in the root), as Compose's
/// `dragHandleTo`: down at its center, then moved by the distance and the touch slop on each
/// axis it moves along (`movePastSlopBy`).
fn drag_handle_to(ui: &mut ComposeTestRule, handle: Handle, to: Offset) {
    let position = handle_info(ui, handle).position;
    // The semantics' position is in the container's coordinates, the container at the root's
    // top left here.
    let delta = to - position;
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let slop = touch.view_configuration().touch_slop;
        let center = touch.center();
        touch.down(center);
        let slop = Offset::new(slop * delta.x.signum_or_zero(), slop * delta.y.signum_or_zero());
        touch.move_by(delta + slop);
        touch.up();
    });
    ui.wait_for_idle();
}

trait SignumOrZero {
    fn signum_or_zero(self) -> f32;
}

impl SignumOrZero for f32 {
    fn signum_or_zero(self) -> f32 {
        if self == 0.0 { 0.0 } else { self.signum() }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, long_press_select_a_word
#[test]
fn a_long_press_selects_a_word_and_its_handles_point_at_its_ends() {
    let mut ui = container(false);
    let at = Offset::new(TEXT.find('m').unwrap() as f32 * FONT, 0.5 * FONT);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.long_click_at(at, 600));
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let ((_, start), (_, end)) = selection(&mut ui).expect("a selection");
    assert_eq!((start, end), (TEXT.find('D').unwrap(), TEXT.find('o').unwrap() + 1));
    assert_eq!(haptics(), 1);
    // Relative to the container, as the semantics say.
    let left = FONT * TEXT.find('D').unwrap() as f32;
    let right = FONT * (TEXT.find('o').unwrap() + 1) as f32;
    assert_handle_position(&mut ui, Handle::SelectionStart, left, FONT);
    assert_handle_position(&mut ui, Handle::SelectionEnd, right, FONT);
    // Own: a left-to-right selection's start handle points left, its end right.
    assert_eq!(handle_info(&mut ui, Handle::SelectionStart).anchor, SelectionHandleAnchor::Left);
    assert_eq!(handle_info(&mut ui, Handle::SelectionEnd).anchor, SelectionHandleAnchor::Right);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, long_press_select_a_word_rtl_layout
#[test]
fn a_long_press_in_a_right_to_left_layout_selects_a_word() {
    let mut ui = container(true);
    let root_width = 400.0;
    let at = Offset::new(root_width - "xt Demo Text".len() as f32 * FONT, 0.5 * FONT);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.long_click_at(at, 600));
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let ((_, start), (_, end)) = selection(&mut ui).expect("a selection");
    // As Compose's: `indexOf`, the first "Text" (one left-to-right run, at the line's left).
    assert_eq!((start, end), (TEXT.find('T').unwrap(), TEXT.find('t').unwrap() + 1));
    assert_eq!(haptics(), 1);
    let left = root_width - FONT * TEXT.len() as f32;
    let right = root_width - FONT * " Demo Text".len() as f32;
    assert_handle_position(&mut ui, Handle::SelectionStart, left, FONT);
    assert_handle_position(&mut ui, Handle::SelectionEnd, right, FONT);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, press_to_cancel
#[test]
fn a_tap_after_a_long_press_clears_the_selection() {
    let mut ui = container(false);
    let at = Offset::new(50.0, 50.0);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.long_click_at(at, 600));
    ui.wait_for_idle();
    assert!(selection(&mut ui).is_some());
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.click_at(at));
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), None);
    assert_eq!(haptics(), 2);
}

/// Records the passes and kinds of the events it sees, as Compose's `gestureSpy`.
#[derive(Clone)]
struct Spy(Rc<RefCell<Vec<(PointerEventPass, PointerEventType)>>>);

struct SpyNode(Spy);

impl ModifierNodeElement for Spy {
    type Node = SpyNode;
    fn create(&self) -> SpyNode {
        SpyNode(self.clone())
    }
    fn update(&self, node: &mut SpyNode) {
        node.0 = self.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    fn name(&self) -> &'static str {
        "gesture_spy"
    }
}

impl ModifierNode for SpyNode {
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
}

impl PointerInputModifierNode for SpyNode {
    fn on_pointer_event(
        &mut self,
        event: &mut PointerEvent,
        pass: PointerEventPass,
        _bounds: IntSize,
        _context: &mut EventContext<'_>,
    ) {
        self.0.0.borrow_mut().push((pass, event.kind));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, tapToCancelDoesNotBlockUp
#[test]
fn the_tap_clearing_a_selection_lets_its_up_reach_the_parent() {
    let log = Spy(Rc::new(RefCell::new(Vec::new())));
    let spy = log.clone();
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(400, 400),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            Box!(modifier = Modifier.test_tag(CONTAINER).then_custom(spy.clone()), {
                SelectionContainer!({
                    BasicText!(TEXT, modifier = Modifier.fill_max_size(), style = style());
                });
            });
        },
    );
    ui.host().set_input_mode(InputMode::Touch);
    let at = Offset::new(50.0, 50.0);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.long_click_at(at, 600));
    ui.wait_for_idle();
    log.0.borrow_mut().clear();
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.click_at(at));
    ui.wait_for_idle();
    let last = *log.0.borrow().last().expect("events");
    assert_eq!(last, (PointerEventPass::Final, PointerEventType::Release));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, selectionContinues_toBelowText
#[test]
fn the_end_handle_dragged_to_the_text_below_continues_the_selection_there() {
    let mut ui = two_texts(false);
    start_selection(&mut ui, TAG1, 0);
    let to = character_box(&mut ui, TAG2, 3).bottom_right();
    drag_handle_to(&mut ui, Handle::SelectionEnd, to);
    let (start, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(start), 0, TAG1);
    assert_anchor(&mut ui, Some(end), 4, TAG2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, selectionContinues_toAboveText
#[test]
fn the_start_handle_dragged_to_the_text_above_continues_the_selection_there() {
    let mut ui = two_texts(false);
    // The second word.
    start_selection(&mut ui, TAG2, 6);
    let to = character_box(&mut ui, TAG1, 5).bottom_left();
    drag_handle_to(&mut ui, Handle::SelectionStart, to);
    let (start, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(start), 5, TAG1);
    assert_anchor(&mut ui, Some(end), 9, TAG2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, selectionContinues_toNextText_skipsDisableSelection
#[test]
fn the_end_handle_dragged_over_a_disabled_text_stays_in_the_first() {
    let mut ui = two_texts(true);
    start_selection(&mut ui, TAG1, 0);
    let to = character_box(&mut ui, TAG2, 3).bottom_right();
    drag_handle_to(&mut ui, Handle::SelectionEnd, to);
    let (start, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(start), 0, TAG1);
    assert_anchor(&mut ui, Some(end), TEXT.len(), TAG1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, selectionHandle_remainsInComposition_whenTextIsOverflowed_clipped_softwrapDisabled
#[test]
fn a_handle_dragged_from_an_overflowing_text_stays_and_reaches_the_next() {
    let long = format!("{TEXT} ").repeat(100);
    let mut ui = container_with(false, move || {
        let long = long.clone();
        Column!({
            BasicText!(
                long.clone(),
                modifier = Modifier.fill_max_width().test_tag(TAG1),
                style = style(),
                soft_wrap = false
            );
            DisableSelection!({
                BasicText!(
                    TEXT,
                    modifier = Modifier.fill_max_width(),
                    style = style(),
                    soft_wrap = false
                );
            });
            BasicText!(
                long.clone(),
                modifier = Modifier.fill_max_width().test_tag(TAG2),
                style = style(),
                soft_wrap = false
            );
        });
    });
    start_selection(&mut ui, TAG1, 0);
    let to = character_box(&mut ui, TAG2, 3).bottom_right();
    drag_handle_to(&mut ui, Handle::SelectionEnd, to);
    let (start, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(start), 0, TAG1);
    assert_anchor(&mut ui, Some(end), 4, TAG2);
}

fn one_line_cut(overflow: compose_ui::TextOverflow) {
    let long = format!("{TEXT} ").repeat(100);
    let length = long.len();
    let mut ui = container_with(false, move || {
        let long = long.clone();
        Column!({
            BasicText!(
                long.clone(),
                modifier = Modifier.fill_max_width().test_tag(TAG1),
                style = style(),
                max_lines = 1,
                overflow = overflow
            );
        });
    });
    start_selection(&mut ui, TAG1, 0);
    let to = character_box(&mut ui, TAG1, 4).bottom_right() + Offset::new(0.0, FONT);
    drag_handle_to(&mut ui, Handle::SelectionEnd, to);
    let (start, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(start), 0, TAG1);
    assert_anchor(&mut ui, Some(end), length, TAG1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, allTextIsSelected_whenTextIsOverflowed_clipped_maxLines1
#[test]
fn the_end_handle_dragged_below_a_clipped_line_selects_all_its_text() {
    one_line_cut(compose_ui::TextOverflow::Clip);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, allTextIsSelected_whenTextIsOverflowed_ellipsized_maxLines1
#[test]
fn the_end_handle_dragged_below_an_ellipsized_line_selects_all_its_text() {
    one_line_cut(compose_ui::TextOverflow::Ellipsis);
}

/// "Hello", a text of no height or no width, "World".
fn with_an_empty_text(no_height: bool) {
    let mut ui = container_with(false, move || {
        Column!({
            BasicText!("Hello", modifier = Modifier.fill_max_width().test_tag(TAG1));
            let empty = if no_height { Modifier.height(0.dp()) } else { Modifier.width(0.dp()) };
            BasicText!("THIS SHOULD NOT CAUSE CRASH", modifier = empty.test_tag("empty"));
            BasicText!("World", modifier = Modifier.fill_max_width().test_tag(TAG2));
        });
    });
    start_selection(&mut ui, TAG1, 0);
    let to = character_box(&mut ui, TAG2, 4).bottom_right();
    drag_handle_to(&mut ui, Handle::SelectionEnd, to);
    let (start, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(start), 0, TAG1);
    assert_anchor(&mut ui, Some(end), 5, TAG2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, selectionIncludes_noHeightText
#[test]
fn a_selection_dragged_over_a_text_of_no_height_goes_on() {
    with_an_empty_text(true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, selectionIncludes_noWidthText
#[test]
fn a_selection_dragged_over_a_text_of_no_width_goes_on() {
    with_an_empty_text(false);
}

/// Two containers in a column, as Compose's `SelectionContainerFocusTest`: the first's text over
/// a box of 40 dp, the second's text.
fn two_containers() -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    HAPTICS.with(|count| count.set(0));
    let mut ui =
        compose_test_with(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer), || {
            with_haptics(|| {
                Column!({
                    SelectionContainer!(modifier = Modifier.test_tag("selectionContainer1"), {
                        Column!({
                            BasicText!(TEXT, modifier = Modifier.fill_max_width(), style = style());
                            Box!(modifier = Modifier.size(40.dp()).test_tag("box"));
                        });
                    });
                    SelectionContainer!(modifier = Modifier.test_tag("selectionContainer2"), {
                        BasicText!(TEXT, modifier = Modifier.fill_max_width(), style = style());
                    });
                });
            });
        });
    ui.host().set_input_mode(InputMode::Touch);
    ui.wait_for_idle();
    ui
}

/// Whether the text of the container tagged `tag` has the selection.
fn selected_in(ui: &mut ComposeTestRule, tag: &str) -> bool {
    let container = node_of(ui, tag);
    let Some(((node, _), _)) = selection(ui) else { return false };
    ui.host().with_tree(|tree| tree.is_at_or_below(node, container))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerFocusTest.kt, tap_to_cancel
#[test]
fn a_tap_on_a_box_in_the_container_clears_its_selection() {
    let mut ui = two_containers();
    let at = Offset::new(FONT / 2.0, FONT / 2.0);
    ui.on_node_with_tag("selectionContainer1").perform_touch_input(|touch| {
        touch.long_click_at(at, 600);
    });
    ui.wait_for_idle();
    assert!(selected_in(&mut ui, "selectionContainer1"));
    ui.on_node_with_tag("box").perform_touch_input(|touch| touch.click());
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), None);
    assert_eq!(haptics(), 2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerFocusTest.kt, select_anotherContainer_cancelOld
#[test]
fn a_long_press_in_another_container_clears_the_first_ones_selection() {
    let mut ui = two_containers();
    let at = Offset::new(FONT / 2.0, FONT / 2.0);
    ui.on_node_with_tag("selectionContainer1").perform_touch_input(|touch| {
        touch.long_click_at(at, 600);
    });
    ui.wait_for_idle();
    assert!(selected_in(&mut ui, "selectionContainer1"));
    ui.on_node_with_tag("selectionContainer2").perform_touch_input(|touch| {
        touch.long_click_at(at, 600);
    });
    ui.wait_for_idle();
    assert!(!selected_in(&mut ui, "selectionContainer1"));
    assert!(selected_in(&mut ui, "selectionContainer2"));
    // Two from the first container, one from the second.
    assert_eq!(haptics(), 3);
}

/// A finger's long press below a container's text, where no text is, selects nothing: Compose's
/// long press is each text's (`SelectionController`), not the container's.
#[test]
fn a_long_press_where_no_text_is_selects_nothing() {
    let mut ui = container_with(false, || {
        Column!(modifier = Modifier.fill_max_size(), {
            BasicText!(TEXT, modifier = Modifier.test_tag(TAG1), style = style());
        });
    });
    let below = Offset::new(FONT / 2.0, 5.0 * FONT);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.long_click_at(below, 600));
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), None);
    assert_eq!(haptics(), 0);
}

/// A handle dragged past the container's visible bounds still shows while it's dragged, as
/// Compose's `updateHandleOffsets` keeps the dragged handle: the container clipped to its first
/// text's line, the end handle dragged to the second text.
#[test]
fn a_dragged_handle_outside_the_visible_bounds_still_shows() {
    let mut ui = container_with(false, || {
        Box!(modifier = Modifier.height(FONT.dp()).clip_to_bounds(), {
            // Laid out whole, the container 20 px high around it.
            let whole = Modifier.wrap_content_height_with(compose_ui::Alignment::TOP, true);
            Column!(modifier = whole, {
                BasicText!(TEXT, modifier = Modifier.test_tag(TAG1), style = style());
                BasicText!(TEXT, modifier = Modifier.test_tag(TAG2), style = style());
            });
        });
    });
    start_selection(&mut ui, TAG1, 0);
    assert!(handle_info(&mut ui, Handle::SelectionEnd).visible);
    let to = character_box(&mut ui, TAG2, 3).bottom_right();
    let delta = to - handle_info(&mut ui, Handle::SelectionEnd).position;
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).perform_touch_input(|touch| {
        let slop = touch.view_configuration().touch_slop;
        touch.down(touch.center());
        let slop = Offset::new(slop * delta.x.signum_or_zero(), slop * delta.y.signum_or_zero());
        touch.move_by(delta + slop);
    });
    ui.wait_for_idle();
    let (_, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(end), 4, TAG2);
    assert!(handle_info(&mut ui, Handle::SelectionEnd).visible, "shown while dragged");
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).perform_touch_input(|touch| {
        touch.up();
    });
}
