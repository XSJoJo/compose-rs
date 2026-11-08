//! Text selection's magnifier, from Compose's `AbstractSelectionMagnifierTests` as
//! `SelectionContainerMagnifierTest` and the state-based field's `TextFieldMagnifierTest` run
//! them (each check over a container and over a field, `container_*` and `field_*`), and those two
//! classes' own tests (D2073).
//!
//! As Compose's: the magnifiers found by their semantics (`MagnifierPositionInRoot`); the
//! platform's magnifier the test rule's (as big as Android's text magnifier, 100 by 48 dp); the
//! test font at 20 sp (each character 20 px square), density 1, touch input mode.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    Density, InputMode, IntSize, IntrinsicSize, LayoutDirection, Offset, TextAlign,
    TextLayoutResult, TextRange, UnitExt,
};

use crate::text_field_touch_support::*;

const TAG: &str = "tag";
const FONT: f32 = 20.0;
/// Compose's `RtlChar`, a Hebrew letter.
const RTL_CHAR: char = '\u{05D1}';

/// A selection container over a text, or a state-based field.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Container,
    Field,
}

/// Where the content's last text layout goes.
type Layout = Rc<RefCell<Option<TextLayoutResult>>>;

fn style() -> TextStyle {
    TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT }
}

fn centered() -> TextStyle {
    TextStyle { text_align: Some(TextAlign::Center), ..style() }
}

fn rule(direction: LayoutDirection, content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let content = Rc::new(content);
    let mut ui = compose_test_with(
        IntSize::new(600, 600),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let content = content.clone();
            compose_runtime::CompositionLocalProvider(
                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                move || content(),
            );
        },
    );
    ui.host().set_input_mode(InputMode::Touch);
    ui.wait_for_idle();
    ui
}

/// The test's content, as `TestContent`: a container over a `BasicText`, or a field over a
/// state remembered with `text`.
fn content(
    kind: Kind,
    text: &str,
    modifier: Modifier,
    style: TextStyle,
    layout: &Layout,
    max_lines: u32,
) {
    let told = layout.clone();
    let on_layout = move |result: TextLayoutResult| *told.borrow_mut() = Some(result);
    match kind {
        Kind::Container => {
            let text = text.to_string();
            SelectionContainer!(modifier = modifier, {
                BasicText!(
                    text.clone(),
                    style = style.clone(),
                    on_text_layout = Some(on_layout.clone()),
                    max_lines = max_lines,
                );
            });
        }
        Kind::Field => {
            let initial = text.to_string();
            let state = compose_runtime::remember(move || TextFieldState::new(&initial));
            BasicTextFieldOfState!(
                state,
                modifier = modifier,
                text_style = style,
                on_text_layout = Some(on_layout),
            );
        }
    }
}

fn state_with(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
}

fn scroll_state() -> ScrollState {
    **Box::leak(Box::new(ScrollState::new(0)))
}

/// A rule over `kind`'s content of `text`, `modifier` made for it, laid out left to right.
fn content_rule(
    kind: Kind,
    text: &str,
    modifier: impl Fn() -> Modifier + 'static,
    style: TextStyle,
    max_lines: u32,
) -> (ComposeTestRule, Layout) {
    let layout: Layout = Rc::default();
    let (kept, text) = (layout.clone(), text.to_string());
    let ui = rule(LayoutDirection::Ltr, move || {
        content(kind, &text, modifier(), style.clone(), &kept, max_lines);
    });
    (ui, layout)
}

/// Centered in the window, as most of Compose's checks put it: room for the magnifier to move.
fn centered_in_window() -> Modifier {
    Modifier.fill_max_size().wrap_content_size().test_tag(TAG)
}

fn layout_of(layout: &Layout) -> TextLayoutResult {
    layout.borrow().clone().expect("laid out")
}

/// The magnifiers' positions in the root, as Compose's `getMagnifierPositions`: each node with
/// a magnifier says it, unspecified while it shows nothing.
fn magnifier_positions(ui: &mut ComposeTestRule) -> Vec<Offset> {
    let nodes =
        ui.on_all_nodes(key_is_defined(&MAGNIFIER_POSITION_IN_ROOT)).fetch_semantics_nodes();
    let positions: Vec<MagnifierPositionInRoot> = nodes
        .iter()
        .filter_map(|node| node.config().get(&MAGNIFIER_POSITION_IN_ROOT).cloned())
        .collect();
    ui.wait_for_idle();
    positions.iter().map(MagnifierPositionInRoot::get).collect()
}

/// As Compose's `getMagnifierCenterOffset`: the first magnifier's position, or the one
/// specified (`require_specified`).
#[track_caller]
fn magnifier_center(ui: &mut ComposeTestRule, require_specified: bool) -> Offset {
    let positions = magnifier_positions(ui);
    if require_specified {
        let specified: Vec<Offset> =
            positions.iter().copied().filter(|position| position.is_specified()).collect();
        assert_eq!(specified.len(), 1, "one specified magnifier position: {positions:?}");
        specified[0]
    } else {
        *positions.first().expect("a magnifier position")
    }
}

/// The first magnifier's position, unspecified where none is: what Compose's
/// `getMagnifierCenterOffset(rule).isUnspecified` asks of a magnifier it expects hidden. Compose's
/// field always has its magnifier's node; compose-rs's composes it only while a drag shows it.
fn magnifier_center_or_unspecified(ui: &mut ComposeTestRule) -> Offset {
    magnifier_positions(ui).first().copied().unwrap_or(Offset::UNSPECIFIED)
}

#[track_caller]
fn assert_magnifier_exists(ui: &mut ComposeTestRule) {
    let positions = magnifier_positions(ui);
    assert!(
        positions.iter().any(|position| position.is_specified()),
        "a magnifier with specified coordinates: {positions:?}"
    );
    // The platform's shows, where the semantics say.
    assert!(!ui.magnifiers().showing().is_empty(), "the platform's magnifier shows");
}

/// As Compose's `assertNoMagnifierExists`: none, or none with specified coordinates.
#[track_caller]
fn assert_no_magnifier_exists(ui: &mut ComposeTestRule) {
    let positions = magnifier_positions(ui);
    assert!(
        positions.iter().all(|position| position.is_unspecified()),
        "no magnifier with specified coordinates: {positions:?}"
    );
    assert!(ui.magnifiers().showing().is_empty(), "the platform's magnifier doesn't show");
}

#[track_caller]
fn assert_offset_near(actual: Offset, expected: Offset, tolerance: f32) {
    assert!(
        (actual.x - expected.x).abs() <= tolerance && (actual.y - expected.y).abs() <= tolerance,
        "expected {expected:?} within {tolerance}, found {actual:?}"
    );
}

/// Shows `handle`, as Compose's `showHandle`: the cursor's by a click, the selection's by a
/// long click.
fn show_handle(ui: &mut ComposeTestRule, handle: Handle) {
    if handle == Handle::Cursor {
        ui.on_node_with_tag(TAG).perform_click();
    } else {
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.long_click());
    }
    ui.wait_for_idle();
}

/// Moves the finger by `delta` past the touch slop on each axis it moves on, as Compose's
/// `movePastSlopBy`.
fn move_past_slop_by(touch: &mut TouchInjectionScope<'_>, delta: Offset) {
    let slop = touch.view_configuration().touch_slop;
    let sign = |value: f32| {
        if value > 0.0 {
            1.0
        } else if value < 0.0 {
            -1.0
        } else {
            0.0
        }
    };
    touch.move_by(delta + Offset::new(slop * sign(delta.x), slop * sign(delta.y)));
}

/// A finger held at `at` past the long-press timeout, as Compose's `longPress`.
fn long_press(touch: &mut TouchInjectionScope<'_>, at: Offset) {
    let timeout = touch.view_configuration().long_press_timeout_millis;
    touch.down(at);
    touch.advance_event_time(timeout + 10);
    touch.move_to(at);
}

fn word(rtl: bool) -> String {
    let char = if rtl { RTL_CHAR } else { 'a' };
    std::iter::repeat_n(char, 4).collect()
}

fn line_center(layout: &TextLayoutResult, line: usize) -> f32 {
    let (top, bottom) = (layout.get_line_top(line), layout.get_line_bottom(line));
    (bottom - top) / 2.0 + top
}

fn position_in_root(ui: &mut ComposeTestRule) -> Offset {
    let bounds = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    Offset::new(bounds.left, bounds.top)
}

// ---- The checks of `AbstractSelectionMagnifierTests` ---------------------------------------

/// Compose's `centerIsUnspecified_whenNoSelection`, through what shows: a text with no selection
/// has no magnifier (Compose's asks a new `SelectionManager` its center).
fn check_center_unspecified_without_selection(kind: Kind) {
    let (mut ui, _) = content_rule(kind, "aaaa aaaa", centered_in_window, style(), u32::MAX);
    assert_no_magnifier_exists(&mut ui);
    assert_eq!(ui.magnifiers().creation_count(), 0);
}

/// Compose's `centerIsUnspecified_whenNotDragging`: a selection no gesture moves has none.
fn check_center_unspecified_when_not_dragging(kind: Kind) {
    let (mut ui, _) = content_rule(kind, "aaaa aaaa", centered_in_window, style(), u32::MAX);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.long_click());
    ui.wait_for_idle();
    assert_no_magnifier_exists(&mut ui);
}

/// `magnifier_centeredToEndOfLine_whenBidiEndOffsetInMiddleOfLine`: dragged past the end of a
/// line whose last offset is in its middle (bidi), the magnifier stays at the line's right.
/// `first_click` is the abstract test's click before; the field's own version has none.
fn check_centered_to_end_of_line_with_bidi(kind: Kind, first_click: bool) {
    let (ltr, rtl) = ("hello", std::iter::repeat_n(RTL_CHAR, 5).collect::<String>());
    let text = format!("{rtl} {ltr}\n{ltr} {rtl}\n{rtl} {ltr}");
    let (mut ui, layout) = content_rule(
        kind,
        &text,
        || Modifier.fill_max_size().wrap_content_height().test_tag(TAG),
        style(),
        u32::MAX,
    );
    let text_layout = layout_of(&layout);
    if first_click {
        let first = text_layout.get_bounding_box(0).center_left() + Offset::new(1.0, 0.0);
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(first));
        ui.wait_for_idle();
    }
    let placed = position_in_root(&mut ui);
    let far_right = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds.right - 1.0;
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        // Not a second tap of the click.
        touch.advance_event_time(1000);
        long_press(touch, Offset::new(far_right, line_center(&text_layout, 0)));
    });
    assert!(magnifier_center_or_unspecified(&mut ui).is_unspecified(), "not shown");
    let second = Offset::new(far_right, line_center(&text_layout, 1));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.move_to(second));
    assert!(magnifier_center_or_unspecified(&mut ui).is_unspecified(), "not shown");
    let line_right = text_layout.get_line_right(1);
    let third = Offset::new(line_right + 1.0, second.y);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.move_to(third));
    let actual = magnifier_center(&mut ui, true) - placed;
    assert_offset_near(actual, Offset::new(line_right, second.y), 0.001);
}

/// `magnifier_centeredOnCorrectLine_whenLinesAreEmpty`: dragged down empty lines, the magnifier is
/// on each, at its left. `first_click` as above.
fn check_centered_on_empty_lines(kind: Kind, first_click: bool) {
    let (mut ui, layout) = content_rule(kind, "a\n\n", centered_in_window, style(), u32::MAX);
    let text_layout = layout_of(&layout);
    let first = text_layout.get_bounding_box(0).center_left() + Offset::new(1.0, 0.0);
    if first_click {
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(first));
        ui.wait_for_idle();
    }
    let placed = position_in_root(&mut ui);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let timeout = touch.view_configuration().double_tap_timeout_millis;
        touch.advance_event_time(timeout * 2);
        long_press(touch, first);
    });
    let assert_at = |ui: &mut ComposeTestRule, expected: Offset| {
        let actual = magnifier_center(ui, true) - placed;
        assert_offset_near(actual, expected, 0.001);
    };
    assert_at(&mut ui, first);
    for line in [1, 2] {
        let at = Offset::new(first.x, line_center(&text_layout, line));
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.move_to(at));
        assert_at(&mut ui, Offset::new(0.0, at.y));
    }
}

/// `magnifier_hidden_whenTextIsEmpty`.
fn check_hidden_when_text_empty(kind: Kind) {
    let (mut ui, _) = content_rule(kind, "", || Modifier.test_tag(TAG), style(), u32::MAX);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.long_click());
    assert_no_magnifier_exists(&mut ui);
}

/// `magnifier_hidden_whenSelectionWithoutHandleTouch`.
fn check_hidden_without_handle_touch(kind: Kind) {
    let (mut ui, _) =
        content_rule(kind, "aaaa aaaa aaaa", || Modifier.test_tag(TAG), style(), u32::MAX);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.long_click());
    assert_no_magnifier_exists(&mut ui);
}

/// `checkMagnifierShowsDuringInitialLongPressDrag`: the magnifier shows once the long press
/// starts and follows its drag. The container's is the abstract check (a click first); the
/// field's is `TextFieldMagnifierTest`'s override (none: its long press differs).
fn check_shows_during_initial_long_press_drag(kind: Kind, forwards: bool, rtl: bool) {
    let drag = Offset::new(if forwards { 10.0 } else { -10.0 }, 0.0);
    let word = word(rtl);
    let text = format!("{word} {word} {word}");
    let (mut ui, layout) = content_rule(kind, &text, centered_in_window, style(), u32::MAX);
    if kind == Kind::Container {
        let first = layout_of(&layout).get_bounding_box(0).center_left() + Offset::new(1.0, 0.0);
        ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(first));
        ui.wait_for_idle();
    }
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        // Not a second tap of the click.
        touch.advance_event_time(1000);
        let center = touch.center();
        touch.down(center);
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.advance_event_time(timeout + 100);
        touch.move_by(Offset::ZERO);
    });
    let initial = magnifier_center(&mut ui, true);
    // No slop: the drag starts with the long press.
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.move_by(drag));
    assert_eq!(magnifier_center(&mut ui, false), initial + drag);
}

/// `checkMagnifierAppears_whileHandleTouched`: shown while the handle is held, gone once lifted.
fn check_appears_while_handle_touched(kind: Kind, handle: Handle) {
    let (mut ui, _) =
        content_rule(kind, "aaaa aaaa aaaa", || Modifier.test_tag(TAG), style(), u32::MAX);
    show_handle(&mut ui, handle);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    assert_magnifier_exists(&mut ui);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| touch.up());
    assert_no_magnifier_exists(&mut ui);
}

/// `checkMagnifierAppears_whenCursorHandleDragged`: the cursor's only once dragged.
fn check_appears_when_cursor_handle_dragged(kind: Kind) {
    let (mut ui, _) =
        content_rule(kind, "aaaa aaaa aaaa", || Modifier.test_tag(TAG), style(), u32::MAX);
    show_handle(&mut ui, Handle::Cursor);
    let cursor = || is_selection_handle(Some(Handle::Cursor));
    ui.on_node(cursor()).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    assert_no_magnifier_exists(&mut ui);
    ui.on_node(cursor())
        .perform_touch_input(|touch| move_past_slop_by(touch, Offset::new(1.0, 0.0)));
    assert_magnifier_exists(&mut ui);
    ui.on_node(cursor()).perform_touch_input(|touch| touch.up());
    assert_no_magnifier_exists(&mut ui);
}

/// `checkMagnifierFollowsHandleHorizontally`.
fn check_follows_handle_horizontally(kind: Kind, handle: Handle, rtl: bool) {
    let drag = Offset::new(1.0, 0.0);
    let word = word(rtl);
    let text = format!("{word} {word} {word}");
    let (mut ui, _) = content_rule(kind, &text, centered_in_window, style(), u32::MAX);
    show_handle(&mut ui, handle);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
        move_past_slop_by(touch, drag);
    });
    let initial = magnifier_center(&mut ui, true);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| touch.move_by(drag));
    assert_eq!(magnifier_center(&mut ui, false), initial + drag);
}

/// `checkMagnifierConstrainedToLineHorizontalBounds`: dragged a little past its line's end (the
/// start's, or the end's: `check_start`), the magnifier stays.
fn check_constrained_to_line(kind: Kind, handle: Handle, check_start: bool, rtl: bool) {
    let direction = if check_start != rtl { -1.0 } else { 1.0 };
    let moved = Offset::new(direction, 0.0);
    let filler = word(rtl);
    // The cursor's on an empty line, with no room either way; a selection's needs a word.
    let middle = if handle == Handle::Cursor { String::new() } else { filler.clone() };
    let text = format!("{filler}{filler}\n{middle}\n{filler}{filler}");
    let (mut ui, _) = content_rule(kind, &text, centered_in_window, centered(), u32::MAX);
    show_handle(&mut ui, handle);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
        move_past_slop_by(touch, moved);
    });
    let initial = magnifier_center(&mut ui, true);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| touch.move_by(moved));
    assert_eq!(magnifier_center(&mut ui, false), initial);
}

/// `checkMagnifierHiddenWhenDraggedTooFar`: dragged a window's width past its line, it's gone.
fn check_hidden_when_dragged_too_far(kind: Kind, handle: Handle, check_start: bool, rtl: bool) {
    let screen_width = Rc::new(Cell::new(0));
    let direction = if check_start { -1.0 } else { 1.0 };
    let word = word(rtl);
    let text = format!("{word} {word}\n{word}\n{word} {word}");
    let width = screen_width.clone();
    let (mut ui, _) = content_rule(
        kind,
        &text,
        move || {
            let width = width.clone();
            Modifier
                .on_size_changed(move |size: IntSize| width.set(size.width))
                .fill_max_size()
                .wrap_content_size()
                .test_tag(TAG)
        },
        centered(),
        u32::MAX,
    );
    show_handle(&mut ui, handle);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    let far = Offset::new(screen_width.get() as f32 * direction, 0.0);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| touch.move_by(far));
    assert_no_magnifier_exists(&mut ui);
}

/// `checkMagnifierFollowsHandleVerticallyBetweenLines`: dragged to the next line, it moves there
/// (animated: within a pixel once idle).
fn check_follows_handle_to_next_line(kind: Kind, handle: Handle) {
    let drag = Offset::new(0.0, 1.0);
    let line_height = Rc::new(Cell::new(0.0));
    let height = line_height.clone();
    let layout: Layout = Rc::default();
    let kept = layout.clone();
    let mut ui = rule(LayoutDirection::Ltr, move || {
        content(
            kind,
            "aaaa aaaa aaaa\naaaa aaaa aaaa\naaaa aaaa aaaa",
            centered_in_window(),
            style(),
            &kept,
            u32::MAX,
        );
    });
    if let Some(text_layout) = layout.borrow().as_ref() {
        height.set(text_layout.get_line_bottom(2) - text_layout.get_line_bottom(1));
    }
    show_handle(&mut ui, handle);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    let initial = magnifier_center(&mut ui, true);
    ui.on_node(is_selection_handle(Some(handle)))
        .perform_touch_input(|touch| move_past_slop_by(touch, drag));
    let moved = magnifier_center(&mut ui, false);
    assert_eq!(moved.x, initial.x);
    assert!((moved.y - (initial.y + line_height.get())).abs() <= 1.0, "{moved:?} {initial:?}");
}

/// `checkMagnifierAsHandleGoesOutOfBoundsUsingMaxLines`: dragged into a line cut by the most
/// lines, it's gone.
fn check_out_of_bounds_with_max_lines(kind: Kind, handle: Handle) {
    let (mut ui, layout) =
        content_rule(kind, "aaaa aaaa aaaa\naaaa aaaa aaaa", centered_in_window, style(), 1);
    let text_layout = layout_of(&layout);
    let line_height = text_layout.get_line_bottom(0) - text_layout.get_line_top(0);
    show_handle(&mut ui, handle);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.on_node(is_selection_handle(Some(handle)))
        .perform_touch_input(|touch| move_past_slop_by(touch, Offset::new(0.0, line_height)));
    assert_no_magnifier_exists(&mut ui);
}

/// `checkMagnifierDoesNotFollowHandleVerticallyWithinLine`: dragged up within its line, it
/// stays.
fn check_does_not_follow_within_line(kind: Kind, handle: Handle) {
    let (mut ui, _) = content_rule(
        kind,
        "aaaa aaaa aaaa\naaaa aaaa aaaa\naaaa aaaa aaaa",
        centered_in_window,
        style(),
        u32::MAX,
    );
    show_handle(&mut ui, handle);
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    let initial = magnifier_center(&mut ui, true);
    // Down would move it to the next line: up only.
    ui.on_node(is_selection_handle(Some(handle)))
        .perform_touch_input(|touch| move_past_slop_by(touch, Offset::new(0.0, -1.0)));
    assert_eq!(magnifier_center(&mut ui, false), initial);
}

/// Each of `AbstractSelectionMagnifierTests`' tests, over a container and a field.
macro_rules! abstract_tests {
    ($($name:ident => |$kind:ident| $body:expr;)*) => {
        mod container {
            use super::*;
            $(
                #[test]
                fn $name() {
                    let $kind = Kind::Container;
                    $body
                }
            )*
        }
        mod field {
            use super::*;
            $(
                #[test]
                fn $name() {
                    let $kind = Kind::Field;
                    $body
                }
            )*
        }
    };
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/AbstractSelectionMagnifierTests.kt, centerIsUnspecified_whenNoSelection, centerIsUnspecified_whenNotDragging, magnifier_centeredToEndOfLine_whenBidiEndOffsetInMiddleOfLine, magnifier_centeredOnCorrectLine_whenLinesAreEmpty, magnifier_hidden_whenTextIsEmpty, magnifier_hidden_whenSelectionWithoutHandleTouch, magnifier_appears_duringInitialLongPressDrag_expandingForwards, magnifier_appears_duringInitialLongPressDrag_expandingBackwards, magnifier_appears_duringInitialLongPressDrag_expandingForwards_rtl, magnifier_appears_duringInitialLongPressDrag_expandingBackwards_rtl, magnifier_appears_whileStartHandleTouched, magnifier_appears_whileEndHandleTouched, magnifier_followsStartHandleHorizontally_whenDragged, magnifier_followsEndHandleHorizontally_whenDragged, magnifier_staysAtLineStart_whenDraggedPastStart, magnifier_staysAtLineEnd_whenDraggedPastEnd, magnifier_hidden_whenDraggedFarPastStartOfLine, magnifier_hidden_whenDraggedFarPastEndOfLine, magnifier_followsStartHandleHorizontally_whenDragged_rtl, magnifier_followsEndHandleHorizontally_whenDragged_rtl, magnifier_staysAtLineStart_whenDraggedPastStart_rtl, magnifier_staysAtLineEnd_whenDraggedPastEnd_rtl, magnifier_hidden_whenDraggedFarPastStartOfLine_rtl, magnifier_hidden_whenDraggedFarPastEndOfLine_rtl, magnifier_doesNotFollowStartHandleVertically_whenDraggedWithinLine, magnifier_doesNotFollowEndHandleVertically_whenDraggedWithinLine, magnifier_followsStartHandle_whenDraggedToNextLine, magnifier_followsEndHandle_whenDraggedToNextLine
abstract_tests! {
    centerIsUnspecified_whenNoSelection => |kind| check_center_unspecified_without_selection(kind);
    centerIsUnspecified_whenNotDragging => |kind| check_center_unspecified_when_not_dragging(kind);
    magnifier_centeredToEndOfLine_whenBidiEndOffsetInMiddleOfLine =>
        |kind| check_centered_to_end_of_line_with_bidi(kind, true);
    magnifier_centeredOnCorrectLine_whenLinesAreEmpty =>
        |kind| check_centered_on_empty_lines(kind, true);
    magnifier_hidden_whenTextIsEmpty => |kind| check_hidden_when_text_empty(kind);
    magnifier_hidden_whenSelectionWithoutHandleTouch =>
        |kind| check_hidden_without_handle_touch(kind);
    magnifier_appears_duringInitialLongPressDrag_expandingForwards =>
        |kind| check_shows_during_initial_long_press_drag(kind, true, false);
    magnifier_appears_duringInitialLongPressDrag_expandingBackwards =>
        |kind| check_shows_during_initial_long_press_drag(kind, false, false);
    magnifier_appears_duringInitialLongPressDrag_expandingForwards_rtl =>
        |kind| check_shows_during_initial_long_press_drag(kind, true, true);
    magnifier_appears_duringInitialLongPressDrag_expandingBackwards_rtl =>
        |kind| check_shows_during_initial_long_press_drag(kind, false, true);
    magnifier_appears_whileStartHandleTouched =>
        |kind| check_appears_while_handle_touched(kind, Handle::SelectionStart);
    magnifier_appears_whileEndHandleTouched =>
        |kind| check_appears_while_handle_touched(kind, Handle::SelectionEnd);
    magnifier_followsStartHandleHorizontally_whenDragged =>
        |kind| check_follows_handle_horizontally(kind, Handle::SelectionStart, false);
    magnifier_followsEndHandleHorizontally_whenDragged =>
        |kind| check_follows_handle_horizontally(kind, Handle::SelectionEnd, false);
    magnifier_staysAtLineStart_whenDraggedPastStart =>
        |kind| check_constrained_to_line(kind, Handle::SelectionStart, true, false);
    magnifier_staysAtLineEnd_whenDraggedPastEnd =>
        |kind| check_constrained_to_line(kind, Handle::SelectionEnd, false, false);
    magnifier_hidden_whenDraggedFarPastStartOfLine =>
        |kind| check_hidden_when_dragged_too_far(kind, Handle::SelectionStart, true, false);
    magnifier_hidden_whenDraggedFarPastEndOfLine =>
        |kind| check_hidden_when_dragged_too_far(kind, Handle::SelectionEnd, false, false);
    magnifier_followsStartHandleHorizontally_whenDragged_rtl =>
        |kind| check_follows_handle_horizontally(kind, Handle::SelectionStart, true);
    magnifier_followsEndHandleHorizontally_whenDragged_rtl =>
        |kind| check_follows_handle_horizontally(kind, Handle::SelectionEnd, true);
    magnifier_staysAtLineStart_whenDraggedPastStart_rtl =>
        |kind| check_constrained_to_line(kind, Handle::SelectionStart, true, true);
    magnifier_staysAtLineEnd_whenDraggedPastEnd_rtl =>
        |kind| check_constrained_to_line(kind, Handle::SelectionEnd, false, true);
    magnifier_hidden_whenDraggedFarPastStartOfLine_rtl =>
        |kind| check_hidden_when_dragged_too_far(kind, Handle::SelectionStart, true, true);
    magnifier_hidden_whenDraggedFarPastEndOfLine_rtl =>
        |kind| check_hidden_when_dragged_too_far(kind, Handle::SelectionEnd, false, true);
    magnifier_doesNotFollowStartHandleVertically_whenDraggedWithinLine =>
        |kind| check_does_not_follow_within_line(kind, Handle::SelectionStart);
    magnifier_doesNotFollowEndHandleVertically_whenDraggedWithinLine =>
        |kind| check_does_not_follow_within_line(kind, Handle::SelectionEnd);
    magnifier_followsStartHandle_whenDraggedToNextLine =>
        |kind| check_follows_handle_to_next_line(kind, Handle::SelectionStart);
    magnifier_followsEndHandle_whenDraggedToNextLine =>
        |kind| check_follows_handle_to_next_line(kind, Handle::SelectionEnd);
}

// ---- `SelectionContainerMagnifierTest`'s own -----------------------------------------------

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerMagnifierTest.kt, magnifier_goesToLastLine_whenSelectionEndDraggedBelowTextBounds_whenTextOverflowed
#[test]
fn magnifier_goesToLastLine_whenSelectionEndDraggedBelowTextBounds_whenTextOverflowed() {
    check_out_of_bounds_with_max_lines(Kind::Container, Handle::SelectionEnd);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerMagnifierTest.kt, magnifier_hidden_whenSelectionStartDraggedBelowTextBounds_whenTextOverflowed
#[test]
fn magnifier_hidden_whenSelectionStartDraggedBelowTextBounds_whenTextOverflowed() {
    check_out_of_bounds_with_max_lines(Kind::Container, Handle::SelectionStart);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerMagnifierTest.kt, magnifier_whenRtlLayoutWithEmptyLine_positionedOnRightSide
#[test]
fn magnifier_whenRtlLayoutWithEmptyLine_positionedOnRightSide() {
    const NON_EMPTY: &str = "nonEmpty";
    const EMPTY: &str = "empty";
    let layouts: Rc<RefCell<Vec<Option<TextLayoutResult>>>> =
        Rc::new(RefCell::new(vec![None, None]));
    let kept = layouts.clone();
    let mut ui = rule(LayoutDirection::Rtl, move || {
        let kept = kept.clone();
        SelectionContainer!(modifier = centered_in_window(), {
            let kept = kept.clone();
            Column!(modifier = Modifier.width(IntrinsicSize::Max), {
                for (index, (text, tag)) in [
                    (std::iter::repeat_n(RTL_CHAR, 4).collect::<String>(), NON_EMPTY),
                    (String::new(), EMPTY),
                ]
                .into_iter()
                .enumerate()
                {
                    let kept = kept.clone();
                    BasicText!(
                        text,
                        modifier = Modifier.fill_max_width().test_tag(tag),
                        style = style(),
                        on_text_layout = Some(move |result: TextLayoutResult| {
                            kept.borrow_mut()[index] = Some(result);
                        }),
                    );
                }
            });
        });
    });
    let placed = position_in_root(&mut ui);
    let layout_of = |index: usize| layouts.borrow()[index].clone().expect("laid out");
    // At the first character.
    let first = layout_of(0).get_bounding_box(0).center_right() - Offset::new(1.0, 0.0);
    let non_empty = ui.on_node_with_tag(NON_EMPTY).fetch_semantics_snapshot().bounds;
    let first_in_container = first + Offset::new(non_empty.left, non_empty.top) - placed;
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| long_press(touch, first_in_container));
    let actual = magnifier_center(&mut ui, true) - placed;
    assert_offset_near(actual, first_in_container, 0.001);
    let empty = ui.on_node_with_tag(EMPTY).fetch_semantics_snapshot().bounds;
    let empty_center_y = layout_of(1).size.height as f32 / 2.0 + empty.top - placed.y;
    let second = Offset::new(first_in_container.x, empty_center_y);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.move_to(second));
    let expected_x = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds.width();
    let actual = magnifier_center(&mut ui, true) - placed;
    assert_offset_near(actual, Offset::new(expected_x, empty_center_y), 0.001);
}

// ---- `TextFieldMagnifierTest`'s own --------------------------------------------------------

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_followsCursorHorizontally_whenDragged
#[test]
fn magnifier_followsCursorHorizontally_whenDragged() {
    check_follows_handle_horizontally(Kind::Field, Handle::Cursor, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtLineEnd_whenCursorDraggedPastStart
#[test]
fn magnifier_staysAtLineEnd_whenCursorDraggedPastStart() {
    check_constrained_to_line(Kind::Field, Handle::Cursor, true, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtLineEnd_whenCursorDraggedPastEnd
#[test]
fn magnifier_staysAtLineEnd_whenCursorDraggedPastEnd() {
    check_constrained_to_line(Kind::Field, Handle::Cursor, false, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_hidden_whenCursorDraggedFarPastStartOfLine
#[test]
fn magnifier_hidden_whenCursorDraggedFarPastStartOfLine() {
    check_hidden_when_dragged_too_far(Kind::Field, Handle::Cursor, true, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_hidden_whenCursorDraggedFarPastEndOfLine
#[test]
fn magnifier_hidden_whenCursorDraggedFarPastEndOfLine() {
    check_hidden_when_dragged_too_far(Kind::Field, Handle::Cursor, false, false);
}

/// `checkMagnifierStayAtEndWhenDraggedBeyondScroll`: a one-line field as wide as the window,
/// its handle dragged a window's width toward its end: the magnifier stays at the field's edge.
fn check_stays_at_end_beyond_scroll(handle: Handle, rtl: bool) {
    let screen = Rc::new(Cell::new(IntSize::ZERO));
    let direction = Offset::new(if rtl { -1.0 } else { 1.0 }, 0.0);
    let filler = if rtl { "\u{05D0}\u{05D1}\u{05D2}\u{05D3}" } else { "aaaa" };
    let text = format!("{filler} {filler} {filler} ").repeat(10);
    let state = state_with(&text, TextRange::new(0, 0));
    let (given, size) = (state, screen.clone());
    let direction_of_layout = if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
    let mut ui = rule(direction_of_layout, move || {
        let size = size.clone();
        BasicTextFieldOfState!(
            given,
            modifier = Modifier
                .fill_max_width()
                .on_size_changed(move |measured: IntSize| size.set(measured))
                .wrap_content_size()
                .test_tag(TAG),
            text_style = TextStyle::DEFAULT,
            line_limits = TextFieldLineLimits::SingleLine,
        );
    });
    if handle == Handle::Cursor {
        ui.on_node_with_tag(TAG).perform_click();
    } else {
        ui.on_node_with_tag(TAG).request_focus();
        ui.on_node_with_tag(TAG).perform_semantics_action_with(
            &compose_ui::SemanticsActions::SET_SELECTION,
            |set| {
                set(5, 9, false);
            },
        );
    }
    ui.wait_for_idle();
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
        // The cursor's shows only once dragged.
        move_past_slop_by(touch, direction);
    });
    let initial = magnifier_center(&mut ui, true);
    let screen = screen.get();
    ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        touch.move_by(Offset::new(screen.width as f32 * direction.x, 0.0));
    });
    let x = if rtl { 0.0 } else { screen.width as f32 };
    assert_eq!(magnifier_center(&mut ui, false), Offset::new(x, initial.y));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtVisibleRegion_whenCursorDraggedPastScrollThreshold_Ltr
#[test]
fn magnifier_staysAtVisibleRegion_whenCursorDraggedPastScrollThreshold_Ltr() {
    check_stays_at_end_beyond_scroll(Handle::Cursor, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtVisibleRegion_whenCursorDraggedPastScrollThreshold_Rtl
#[test]
fn magnifier_staysAtVisibleRegion_whenCursorDraggedPastScrollThreshold_Rtl() {
    check_stays_at_end_beyond_scroll(Handle::Cursor, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtVisibleRegion_whenSelectionStartDraggedPastScrollThreshold_Ltr
#[test]
fn magnifier_staysAtVisibleRegion_whenSelectionStartDraggedPastScrollThreshold_Ltr() {
    check_stays_at_end_beyond_scroll(Handle::SelectionStart, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtVisibleRegion_whenSelectionStartDraggedPastScrollThreshold_Rtl
#[test]
fn magnifier_staysAtVisibleRegion_whenSelectionStartDraggedPastScrollThreshold_Rtl() {
    check_stays_at_end_beyond_scroll(Handle::SelectionStart, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtVisibleRegion_whenSelectionEndDraggedPastScrollThreshold_Ltr
#[test]
fn magnifier_staysAtVisibleRegion_whenSelectionEndDraggedPastScrollThreshold_Ltr() {
    check_stays_at_end_beyond_scroll(Handle::SelectionEnd, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_staysAtVisibleRegion_whenSelectionEndDraggedPastScrollThreshold_Rtl
#[test]
fn magnifier_staysAtVisibleRegion_whenSelectionEndDraggedPastScrollThreshold_Rtl() {
    check_stays_at_end_beyond_scroll(Handle::SelectionEnd, true);
}

/// A field of the test font at 20 sp in a decoration of 8 dp padding, as the decoration box
/// tests' (`lines`, `width`: `None` for its own).
fn decorated_field(
    state: TextFieldState,
    lines: TextFieldLineLimits,
    width: Option<f32>,
    scroll: Option<ScrollState>,
) -> ComposeTestRule {
    rule(LayoutDirection::Ltr, move || {
        let modifier = Modifier.test_tag(TAG);
        let modifier = match width {
            Some(width) => modifier.width(width.dp()),
            None => modifier,
        };
        BasicTextFieldOfState!(
            state,
            modifier = modifier,
            text_style = style(),
            line_limits = lines,
            scroll_state = scroll,
            decorator = |inner: compose_runtime::Callback| {
                Box!(modifier = Modifier.padding(8.dp()), { inner() });
            },
        );
    })
}

/// Drags the cursor handle by `by` past the slop, from its center.
fn drag_cursor(ui: &mut ComposeTestRule, by: Offset) {
    ui.on_node(is_selection_handle(Some(Handle::Cursor))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
        move_past_slop_by(touch, by);
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_insideDecorationBox
#[test]
fn magnifier_insideDecorationBox() {
    let state = state_with("aaaa", TextRange::new(0, 0));
    let mut ui = decorated_field(state, TextFieldLineLimits::SingleLine, None, None);
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let top_left = touch.top_left();
        touch.click_at(top_left);
    });
    ui.wait_for_idle();
    drag_cursor(&mut ui, Offset::new(-0.1, 0.1));
    assert_eq!(magnifier_center(&mut ui, false), Offset::new(0.0, 10.0) + Offset::new(8.0, 8.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_insideDecorationBox_scrolledVertically
#[test]
fn magnifier_insideDecorationBox_scrolledVertically() {
    let state = state_with(&"aaaa\naaaa\naaaa\n".repeat(5), TextRange::new(0, 0));
    let scroll = scroll_state();
    let lines = TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: 2 };
    let mut ui = decorated_field(state, lines, None, Some(scroll));
    let max = scroll.max_value();
    compose_runtime::platform::launch(async move {
        let _ = scroll.scroll_to(max).await;
    });
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let bottom_left = touch.bottom_left();
        touch.click_at(bottom_left);
    });
    ui.wait_for_idle();
    drag_cursor(&mut ui, Offset::new(0.1, 0.1));
    assert_eq!(magnifier_center(&mut ui, false), Offset::new(0.0, 30.0) + Offset::new(8.0, 8.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, magnifier_insideDecorationBox_scrolledHorizontally
#[test]
fn magnifier_insideDecorationBox_scrolledHorizontally() {
    // Compose's text ends with a space, which its line's right takes in on a device; the test
    // measurer's line ends before it: the text here has none, its line's right the text's end.
    let text = "aaaa aaaa aaaa ".repeat(5);
    let state = state_with(text.trim_end(), TextRange::new(0, 0));
    let scroll = scroll_state();
    let mut ui = decorated_field(state, TextFieldLineLimits::SingleLine, Some(100.0), Some(scroll));
    let max = scroll.max_value();
    compose_runtime::platform::launch(async move {
        let _ = scroll.scroll_to(max).await;
    });
    ui.wait_for_idle();
    // Compose taps the field's right: its cursor at the text's end, its handle at the visible
    // right. compose-rs's field gives the cursor its width past the text, so a cursor at the end
    // of a field scrolled to its end is a pixel past its visible bounds, its handle hidden: the
    // tap puts it a character before, and the drag goes past the end (D2073).
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let at = touch.center_right() - Offset::new(8.0 + 20.0, 0.0);
        touch.click_at(at);
    });
    ui.wait_for_idle();
    drag_cursor(&mut ui, Offset::new(30.0, 0.1));
    // Across: kept to the inner field's visible right; down: its line's middle; then the padding.
    assert_eq!(
        magnifier_center(&mut ui, false),
        Offset::new(100.0 - 16.0, 10.0) + Offset::new(8.0, 8.0)
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, textField_magnifier_centeredOnCorrectLine_whenLinesAreEmpty
#[test]
fn textField_magnifier_centeredOnCorrectLine_whenLinesAreEmpty() {
    check_centered_on_empty_lines(Kind::Field, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldMagnifierTest.kt, textField_magnifier_centeredToEndOfLine_whenBidiEndOffsetInMiddleOfLine
#[test]
fn textField_magnifier_centeredToEndOfLine_whenBidiEndOffsetInMiddleOfLine() {
    check_centered_to_end_of_line_with_bidi(Kind::Field, false);
}

// Not one of Compose's: its `checkMagnifierAppears_whenCursorHandleDragged`, which none of its
// tests calls, over a field: the cursor handle shows the magnifier only once dragged.
#[test]
fn field_cursor_handle_shows_the_magnifier_only_once_dragged() {
    check_appears_when_cursor_handle_dragged(Kind::Field);
}

// Not one of Compose's: a mouse's drag selects without a magnifier, as Compose's selection
// gestures tests' asserter checks after each mouse gesture (`shouldShowMagnifier` needs touch
// mode), over a container and a field.
#[test]
fn a_mouse_drag_shows_no_magnifier() {
    for kind in [Kind::Container, Kind::Field] {
        let (mut ui, layout) =
            content_rule(kind, "aaaa aaaa aaaa", centered_in_window, style(), u32::MAX);
        let text_layout = layout_of(&layout);
        let from = text_layout.get_bounding_box(0).center();
        let to = text_layout.get_bounding_box(7).center();
        ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
            mouse.move_to(from);
            mouse.press(MouseButton::Primary);
            mouse.move_to(to);
        });
        ui.wait_for_idle();
        if kind == Kind::Container {
            let selected = ui.host().with_tree(|tree| tree.selected_text());
            assert!(selected.is_some_and(|text| !text.is_empty()), "selected");
        }
        assert_no_magnifier_exists(&mut ui);
        ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.release(MouseButton::Primary));
        assert_no_magnifier_exists(&mut ui);
    }
}
