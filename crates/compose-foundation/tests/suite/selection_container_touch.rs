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

/// Whether `handle` shows, as Compose's `assertSelectionHandlesShown`: there and visible.
fn shown(ui: &mut ComposeTestRule, handle: Handle) -> bool {
    let nodes = ui.on_all_nodes(is_selection_handle(Some(handle))).fetch_semantics_nodes();
    match nodes.as_slice() {
        [] => false,
        [node] => node.config().get(&SELECTION_HANDLE_INFO_KEY).expect("its info").visible,
        _ => panic!("more than one {handle:?} handle"),
    }
}

/// Where the popup of `handle` is drawn, in the root.
fn handle_bounds(ui: &mut ComposeTestRule, handle: Handle) -> Rect {
    ui.on_node(is_selection_handle(Some(handle))).fetch_semantics_snapshot().bounds
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/selection/SelectionManager.kt, updateHandleOffsets (a new selection moves the handles, nothing having moved)
#[test]
fn a_new_selection_moves_the_handles_where_nothing_moved() {
    let mut ui = container(false);
    let long_press = |ui: &mut ComposeTestRule, at: usize| {
        let at = Offset::new(at as f32 * FONT + 1.0, 0.5 * FONT);
        ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| touch.long_click_at(at, 600));
        ui.advance_time_by_frame();
        ui.wait_for_idle();
    };
    long_press(&mut ui, TEXT.find('m').unwrap());
    let start = handle_bounds(&mut ui, Handle::SelectionStart);
    let end = handle_bounds(&mut ui, Handle::SelectionEnd);
    // The last word, five characters right of "Demo".
    long_press(&mut ui, TEXT.rfind('x').unwrap());
    let ((_, from), (_, to)) = selection(&mut ui).expect("a selection");
    assert_eq!((from, to), (10, 14));
    let start_now = handle_bounds(&mut ui, Handle::SelectionStart);
    let end_now = handle_bounds(&mut ui, Handle::SelectionEnd);
    assert_eq!(start_now.left - start.left, 5.0 * FONT, "{start:?} {start_now:?}");
    assert_eq!(end_now.left - end.left, 5.0 * FONT, "{end:?} {end_now:?}");
}

const LINES: usize = 20;
const LINE: f32 = 15.0;

/// A container 100 px high over a column of twenty texts of 15 px ("0" to "19", each tagged by
/// its text) scrolling by `state`, as Compose's `LazyColumnMultiTextRegressionTest` over a lazy
/// column (whose texts leave the composition as they scroll away, so Compose pins those
/// selected: the auto-scroll's part).
fn scrolling_texts(state: ScrollState) -> ComposeTestRule {
    container_with(false, move || {
        Box!(modifier = Modifier.height(100.dp()), {
            Column!(modifier = Modifier.vertical_scroll(state), {
                for index in 0..LINES {
                    BasicText!(
                        index.to_string(),
                        modifier = Modifier.fill_max_width().test_tag(index.to_string()),
                        style = TextStyle { font_size: LINE.sp(), ..TextStyle::DEFAULT }
                    );
                }
            });
        });
    })
}

/// A long press on text `from`, dragged to text `to`, as Compose's `createSelection`.
fn select_lines(ui: &mut ComposeTestRule, from: usize, to: usize) {
    let center =
        |ui: &mut ComposeTestRule, line: usize| character_box(ui, &line.to_string(), 0).center();
    let (from, to) = (center(ui, from), center(ui, to));
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(from);
        touch.advance_event_time(timeout + 10);
        touch.move_to(from);
        touch.move_to(to);
        touch.up();
    });
    ui.wait_for_idle();
}

fn scroll_to(ui: &mut ComposeTestRule, state: ScrollState, to: i32) {
    crate::scroll_support::run(ui, async move {
        let _ = state.scroll_to(to).await;
    });
}

// After Compose's compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/LazyColumnMultiTextRegressionTest.kt, whenScrollingTextOutOfViewUpwards_handlesDisappear,
// over a column scrolled by its state: each handle hides once where it points scrolls out of the
// container's view (Compose's `updateHandleOffsets`), and shows again as it comes back.
#[test]
fn handles_hide_as_their_texts_scroll_out_of_view_and_come_back_with_them() {
    let state = ScrollState::new(0);
    let state = *state;
    let mut ui = scrolling_texts(state);
    let start = |ui: &mut ComposeTestRule| shown(ui, Handle::SelectionStart);
    let end = |ui: &mut ComposeTestRule| shown(ui, Handle::SelectionEnd);
    assert!(!start(&mut ui) && !end(&mut ui));
    select_lines(&mut ui, 1, 3);
    let (first, last) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(first), 0, "1");
    assert_anchor(&mut ui, Some(last), 1, "3");
    assert!(start(&mut ui) && end(&mut ui));
    // The start handle at line 1's bottom (30), the end at line 3's (60).
    let at = |ui: &mut ComposeTestRule, handle| handle_info(ui, handle).position.y;
    assert_eq!(
        (at(&mut ui, Handle::SelectionStart), at(&mut ui, Handle::SelectionEnd)),
        (30.0, 60.0)
    );
    scroll_to(&mut ui, state, 30);
    assert!(start(&mut ui) && end(&mut ui), "on the edge, still in view");
    assert_eq!(at(&mut ui, Handle::SelectionEnd), 30.0);
    scroll_to(&mut ui, state, 31);
    assert!(!start(&mut ui) && end(&mut ui));
    scroll_to(&mut ui, state, 61);
    assert!(!start(&mut ui) && !end(&mut ui));
    scroll_to(&mut ui, state, 45);
    assert!(!start(&mut ui) && end(&mut ui));
    scroll_to(&mut ui, state, 0);
    assert!(start(&mut ui) && end(&mut ui));
    assert_eq!(
        (at(&mut ui, Handle::SelectionStart), at(&mut ui, Handle::SelectionEnd)),
        (30.0, 60.0)
    );
}

// Not one of Compose's: a finger's long press dragged over texts in a scrolling column selects
// them, the column not scrolling (Compose's texts take the drag ahead of the scrollables around
// them).
#[test]
fn a_long_press_dragged_in_a_scrolling_column_selects_and_does_not_scroll() {
    let state = ScrollState::new(0);
    let state = *state;
    let mut ui = scrolling_texts(state);
    select_lines(&mut ui, 3, 1);
    let (first, last) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(first), 1, "3");
    assert_anchor(&mut ui, Some(last), 0, "1");
    assert_eq!(state.value(), 0);
}

// Not one of Compose's: the visible bounds are clipped by the window too, as Compose's
// `visibleBounds()` (`boundsInWindow`): a container running past the window's bottom hides the
// handle pointing past it.
#[test]
fn a_handle_pointing_past_the_window_hides() {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer), || {
            // The window is 400 px high: the second text's bottom is 10 px past it.
            Box!(modifier = Modifier.offset(0.dp(), 370.dp()).test_tag(CONTAINER), {
                SelectionContainer!({
                    Column!({
                        BasicText!("aaa", modifier = Modifier.test_tag(TAG1), style = style());
                        BasicText!("bbb", modifier = Modifier.test_tag(TAG2), style = style());
                    });
                });
            });
        });
    ui.host().set_input_mode(InputMode::Touch);
    ui.wait_for_idle();
    start_selection(&mut ui, TAG1, 0);
    assert!(shown(&mut ui, Handle::SelectionStart) && shown(&mut ui, Handle::SelectionEnd));
    // Everything selected: the end handle points at the second text's bottom, past the window.
    select_all(&mut ui);
    let (_, last) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(last), 3, TAG2);
    assert!(shown(&mut ui, Handle::SelectionStart));
    assert!(!shown(&mut ui, Handle::SelectionEnd));
}

// Not one of Compose's: a graphics layer moving the container, with no layout, moves its visible
// bounds (Compose's container's position in the window changes): the handle it takes past the
// window hides.
#[test]
fn a_container_moved_by_a_graphics_layer_past_the_window_hides_its_handle() {
    let translation = compose_runtime::mutable_state_of(0.0f32);
    let moved = translation.handle();
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(400, 400),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            // The texts' bottoms at 360 and 380, the window 400 px high.
            let modifier = Modifier
                .offset(0.dp(), 340.dp())
                .graphics_layer(move |layer| layer.translation_y = moved.get())
                .test_tag(CONTAINER);
            Box!(modifier = modifier, {
                SelectionContainer!({
                    Column!({
                        BasicText!("aaa", modifier = Modifier.test_tag(TAG1), style = style());
                        BasicText!("bbb", modifier = Modifier.test_tag(TAG2), style = style());
                    });
                });
            });
        },
    );
    ui.host().set_input_mode(InputMode::Touch);
    ui.wait_for_idle();
    start_selection(&mut ui, TAG1, 0);
    select_all(&mut ui);
    assert!(shown(&mut ui, Handle::SelectionStart) && shown(&mut ui, Handle::SelectionEnd));
    // 30 px down: the end handle 10 px past the window, the start one 10 px in it.
    translation.set(30.0);
    ui.wait_for_idle();
    assert!(shown(&mut ui, Handle::SelectionStart));
    assert!(!shown(&mut ui, Handle::SelectionEnd));
    translation.set(0.0);
    ui.wait_for_idle();
    assert!(shown(&mut ui, Handle::SelectionEnd));
}

/// The two texts of the container tests in a `SelectionContainer`, the box around it tagged.
fn selectable_box(modifier: Modifier) {
    Box!(modifier = modifier.test_tag(CONTAINER), {
        SelectionContainer!({
            Column!({
                BasicText!("aaa", modifier = Modifier.test_tag(TAG1), style = style());
                BasicText!("bbb", modifier = Modifier.test_tag(TAG2), style = style());
            });
        });
    });
}

/// A rule 400 px square over `content`, as the tests above, its clock moving as asked.
fn window_of_400(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer), content);
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_auto_advance(false);
    ui.wait_for_idle();
    ui
}

// Not one of Compose's: an `animate_item` item places its content with its animation's layer
// (D1910), so its move, which lays nothing out after its first frame, moves a selection
// container in it as a graphics layer does: the end handle, taken past the window by the item's
// place, shows once the layer brings it back in, as Compose's `updateHandleOffsets` on the
// container's moves.
#[test]
fn a_selection_in_an_animated_item_follows_the_item_s_layer() {
    let items = compose_runtime::mutable_state_of(vec![0, 1, 2]);
    let handle = items.handle();
    let mut ui = window_of_400(move || {
        let items = handle.get();
        LazyColumn!(
            modifier = Modifier.fill_max_size(),
            content = move |scope: LazyListScope| {
                scope.items_list_keyed(
                    items.clone(),
                    |it: &i32| *it,
                    |item_scope, it: &i32| {
                        let spec = compose_animation::tween(64)
                            .easing(compose_animation::LINEAR_EASING)
                            .into();
                        let modifier =
                            Modifier.animate_item_with(&item_scope, None, Some(spec), None);
                        match *it {
                            0 => Box!(modifier = modifier.fill_max_width().height(60.dp())),
                            1 => Box!(modifier = modifier.fill_max_width().height(320.dp())),
                            _ => selectable_box(modifier),
                        }
                    },
                );
            }
        );
    });
    // The texts' bottoms at 400 and 420, the window 400 px high.
    start_selection(&mut ui, TAG1, 0);
    select_all(&mut ui);
    assert!(shown(&mut ui, Handle::SelectionStart));
    assert!(!shown(&mut ui, Handle::SelectionEnd));
    // The first item gone: the item is laid out 60 px up, its layer moving it from where it was
    // over 64 ms.
    let handle = items.handle();
    ui.run_on_ui_thread(move |_| handle.set(vec![1, 2]));
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    assert!(!shown(&mut ui, Handle::SelectionEnd), "the layer keeps it where it was");
    let mut frames = 0;
    while !shown(&mut ui, Handle::SelectionEnd) {
        frames += 1;
        assert!(frames <= 8, "the end handle never came back in");
        ui.advance_time_by_frame();
    }
    // In by a third of the way (20 px), not before the layer's move was told.
    let top = ui.on_node_with_tag(CONTAINER).fetch_semantics_node().position_in_root().y;
    assert!((320.0..=360.0).contains(&top), "{top}");
    ui.wait_for_idle();
    assert!(shown(&mut ui, Handle::SelectionStart) && shown(&mut ui, Handle::SelectionEnd));
}

// Not one of Compose's: a layer placed with comes with no layout of what it moves, so a selection
// container under it is told of it (Compose's layer made calls `onLayoutChange`): the end handle
// it takes past the window hides, and shows again once placed without it.
#[test]
fn a_selection_under_a_layer_placed_with_follows_it_coming_and_going() {
    let with_layer = compose_runtime::mutable_state_of(false);
    let handle = with_layer.handle();
    let block = compose_ui::LayerBlock::new(|layer| layer.translation_y = 30.0);
    let mut ui = window_of_400(move || {
        let block = block.clone();
        let policy = compose_ui::measure_policy(move |scope, measurables, constraints| {
            let placeable = scope.measure(measurables[0], constraints);
            let block = block.clone();
            scope.layout_with(constraints.max_width, constraints.max_height, move |placement| {
                if handle.get() {
                    placement.place_with_layer(&placeable, 0, 340, block.clone());
                } else {
                    placement.place(&placeable, 0, 340);
                }
            })
        });
        compose_ui::Layout(
            Modifier.fill_max_size(),
            policy,
            compose_runtime::callback(|| selectable_box(Modifier)),
        );
    });
    // The texts' bottoms at 360 and 380, the window 400 px high.
    start_selection(&mut ui, TAG1, 0);
    select_all(&mut ui);
    assert!(shown(&mut ui, Handle::SelectionStart) && shown(&mut ui, Handle::SelectionEnd));
    // 30 px down: the end handle 10 px past the window, the start one 10 px in it.
    with_layer.set(true);
    ui.wait_for_idle();
    assert!(shown(&mut ui, Handle::SelectionStart));
    assert!(!shown(&mut ui, Handle::SelectionEnd));
    with_layer.set(false);
    ui.wait_for_idle();
    assert!(shown(&mut ui, Handle::SelectionEnd));
}

/// Cmd+A (Ctrl+A): everything in the focused container selected.
fn select_all(ui: &mut ComposeTestRule) {
    let modifiers = if cfg!(target_os = "macos") {
        compose_ui::KeyModifiers { meta: true, ..Default::default() }
    } else {
        compose_ui::KeyModifiers { ctrl: true, ..Default::default() }
    };
    for kind in [compose_ui::KeyEventType::KeyDown, compose_ui::KeyEventType::KeyUp] {
        let key = compose_ui::Key::Character('a');
        ui.host().send_key(compose_ui::KeyEvent {
            key,
            kind,
            modifiers,
            from_soft_keyboard: false,
        });
    }
    ui.wait_for_idle();
}

// Not one of Compose's: a long press's drag follows its own finger, as Compose's `drag(pointerId)`
// in `touchSelectionFirstPress`: a second finger going down, moving and lifting neither moves the
// selection nor clears it.
#[test]
fn a_second_finger_neither_drags_nor_clears_a_long_press() {
    let mut ui = two_texts(false);
    let first = text_layout(&mut ui, TAG1);
    let demo = first.get_bounding_box(TEXT.find('D').unwrap()).center();
    let last = first.get_bounding_box(TEXT.len() - 1).center();
    let second = Offset::new(demo.x, demo.y + FONT);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| {
        touch.down(demo);
        touch.move_pointers_after(600);
    });
    ui.wait_for_idle();
    let (start, end) = selection(&mut ui).expect("a selection");
    assert_anchor(&mut ui, Some(start), TEXT.find('D').unwrap(), TAG1);
    assert_anchor(&mut ui, Some(end), TEXT.find('o').unwrap() + 1, TAG1);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| {
        touch.down_pointer(1, second);
        touch.move_pointer_to(1, Offset::new(last.x, second.y), 16);
        touch.up_pointer(1);
    });
    ui.wait_for_idle();
    let (start, end) = selection(&mut ui).expect("still a selection");
    assert_anchor(&mut ui, Some(start), TEXT.find('D').unwrap(), TAG1);
    assert_anchor(&mut ui, Some(end), TEXT.find('o').unwrap() + 1, TAG1);
    ui.on_node_with_tag(CONTAINER).perform_touch_input(|touch| {
        touch.move_to(last);
        touch.up();
    });
    ui.wait_for_idle();
    let (start, end) = selection(&mut ui).expect("a selection dragged");
    assert_anchor(&mut ui, Some(start), TEXT.find('D').unwrap(), TAG1);
    assert_anchor(&mut ui, Some(end), TEXT.len(), TAG1);
}
