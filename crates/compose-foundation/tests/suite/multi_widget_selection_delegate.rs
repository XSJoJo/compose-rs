//! What a text tells the selection of its container, as Compose's `MultiWidgetSelectionDelegate`
//! (compose-rs's `Selectable` of `BasicText`): where a handle points, a character's box, the line
//! around an offset, the last offset shown, a line's height. From Compose's
//! `MultiWidgetSelectionDelegateTest`.
//!
//! As Compose's tests: the test font (each character a square of its size), density 1. Compose
//! lays a `TextLayoutResult` out alone; here a `BasicText` is laid out in a selection container
//! under the same constraints and its node asked.

#![allow(non_snake_case)]

use std::ops::Range;

use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, SharedTextMeasurer};
use compose_ui::{
    AnnotatedString, Density, IntSize, Offset, Rect, Selectable, SpanStyle, TextOverflow, UnitExt,
};

const TAG: &str = "text";
const FONT: f32 = 20.0;

/// How a text is laid out: Compose's `simpleTextLayout` (`width: None`, unconstrained) or
/// `constrainedTextLayout` (a fixed width in characters, a height in lines at most).
#[derive(Clone)]
struct Layout {
    max_lines: u32,
    overflow: TextOverflow,
    soft_wrap: bool,
    width: Option<u32>,
    height: Option<u32>,
}

/// Compose's `simpleTextLayout`.
const SIMPLE: Layout = Layout {
    max_lines: u32::MAX,
    overflow: TextOverflow::Clip,
    soft_wrap: true,
    width: None,
    height: None,
};

/// Compose's `constrainedTextLayout`'s defaults: 20 characters wide.
const CONSTRAINED: Layout = Layout { width: Some(20), ..SIMPLE };

fn style() -> TextStyle {
    TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT }
}

/// `text` laid out as `layout` says, in a selection container.
fn laid_out(text: AnnotatedString, layout: Layout) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(800, 800),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let mut modifier = Modifier.test_tag(TAG);
            if let Some(width) = layout.width {
                modifier = modifier.required_width((width as f32 * FONT).dp());
            }
            if let Some(height) = layout.height {
                modifier = modifier.height_in(0.dp(), (height as f32 * FONT).dp());
            }
            let (text, modifier, layout) = (text.clone(), modifier.clone(), layout.clone());
            SelectionContainer!({
                BasicAnnotatedText!(
                    text.clone(),
                    modifier = modifier.clone(),
                    style = style(),
                    max_lines = layout.max_lines,
                    overflow = layout.overflow,
                    soft_wrap = layout.soft_wrap
                );
            });
        },
    );
    ui.wait_for_idle();
    ui
}

/// Asks the text's `Selectable`.
fn ask<R>(
    ui: &mut ComposeTestRule,
    question: impl FnOnce(&mut dyn Selectable, &SharedTextMeasurer) -> R,
) -> R {
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().id;
    let mut question = Some(question);
    let mut answer = None;
    ui.host().with_tree(|tree| {
        tree.with_selectable(node, &mut |selectable, measurer| {
            answer = question.take().map(|question| question(selectable, measurer));
        });
    });
    answer.expect("a selectable text")
}

/// Where a handle points in `text` laid out as `layout`, at `offset`.
fn handle(text: &str, layout: Layout, offset: usize, start: bool, crossed: bool) -> Option<Offset> {
    let mut ui = laid_out(AnnotatedString::new(text.to_string()), layout);
    ask(&mut ui, |selectable, measurer| {
        selectable.handle_position(offset, start, crossed, measurer)
    })
}

fn index(text: &str, c: char) -> usize {
    text.find(c).expect("in the text")
}

/// The handle point at `characters` from the left, on the first line's bottom.
fn at(characters: usize) -> Option<Offset> {
    Some(Offset::new(FONT * characters as f32, FONT))
}

/// The number of characters (not bytes) before `offset`.
fn chars(text: &str, offset: usize) -> usize {
    text[..offset].chars().count()
}

const HELLO: &str = "hello world\n";
const RTL: &str = "\u{05D0}\u{05D1}\u{05D2} \u{05D3}\u{05D4}\u{05D5}\n";
const BIDI_LTR: &str = "Hello";
const BIDI: &str = "Hello\u{05D0}\u{05D1}\u{05D2}";

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_StartHandle_not_cross_ltr
#[test]
fn the_start_handle_of_left_to_right_text_is_at_its_offset() {
    let start = index(HELLO, 'h');
    assert_eq!(handle(HELLO, SIMPLE, start, true, false), at(start));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_StartHandle_cross_ltr
#[test]
fn the_crossed_start_handle_of_left_to_right_text_is_at_its_offset() {
    let start = index(HELLO, 'o');
    assert_eq!(handle(HELLO, SIMPLE, start, true, true), at(start));
}

/// An offset of the right-to-left text from the right: Compose's `text.length - 1 - offset`.
fn from_right(offset: usize) -> Option<Offset> {
    at(RTL.chars().count() - 1 - chars(RTL, offset))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_StartHandle_not_cross_rtl
#[test]
fn the_start_handle_of_right_to_left_text_is_at_its_offset() {
    let start = index(RTL, '\u{05D1}');
    assert_eq!(handle(RTL, SIMPLE, start, true, false), from_right(start));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_StartHandle_cross_rtl
#[test]
fn the_crossed_start_handle_of_right_to_left_text_is_at_its_offset() {
    let start = index(RTL, '\u{05D5}');
    assert_eq!(handle(RTL, SIMPLE, start, true, true), from_right(start));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_StartHandle_not_cross_bidi
#[test]
fn the_start_handle_on_a_right_to_left_run_in_left_to_right_text_is_at_the_runs_start() {
    let start = index(BIDI, '\u{05D0}');
    assert_eq!(handle(BIDI, SIMPLE, start, true, false), at(BIDI.chars().count()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_StartHandle_cross_bidi
#[test]
fn the_crossed_start_handle_on_a_right_to_left_run_is_at_the_left_runs_end() {
    let start = index(BIDI, '\u{05D0}');
    assert_eq!(handle(BIDI, SIMPLE, start, true, true), at(BIDI_LTR.len()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_not_cross_ltr
#[test]
fn the_end_handle_of_left_to_right_text_is_at_its_offset() {
    let end = index(HELLO, 'o');
    assert_eq!(handle(HELLO, SIMPLE, end, false, false), at(end));
}

/// Compose's `simpleTextLayout(maxLines = 1)`.
const ONE_LINE: Layout = Layout { max_lines: 1, ..SIMPLE };

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_not_cross_ltr_overflowed
#[test]
fn the_end_handle_past_the_lines_shown_is_at_the_first_lines_end() {
    let text = "hello\nworld";
    assert_eq!(handle(text, ONE_LINE, index(text, 'r'), false, false), at(5));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_cross_ltr
#[test]
fn the_crossed_end_handle_of_left_to_right_text_is_at_its_offset() {
    let end = index(HELLO, 'h');
    assert_eq!(handle(HELLO, SIMPLE, end, false, true), at(end));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_cross_ltr_overflowed
#[test]
fn the_crossed_end_handle_past_the_lines_shown_is_at_the_first_lines_end() {
    let text = "hello\nworld";
    assert_eq!(handle(text, ONE_LINE, index(text, 'w'), false, true), at(5));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_not_cross_rtl
#[test]
fn the_end_handle_of_right_to_left_text_is_at_its_offset() {
    let end = index(RTL, '\u{05D5}');
    assert_eq!(handle(RTL, SIMPLE, end, false, false), from_right(end));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_cross_rtl
#[test]
fn the_crossed_end_handle_of_right_to_left_text_is_at_its_offset() {
    let end = index(RTL, '\u{05D1}');
    assert_eq!(handle(RTL, SIMPLE, end, false, true), from_right(end));
}

const RTL_LINES: &str = "\u{05D0}\u{05D1}\u{05D2}\n\u{05D3}\u{05D4}\u{05D5}";

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_not_cross_rtl_overflowed
#[test]
fn the_end_handle_past_the_right_to_left_lines_shown_is_at_the_first_lines_end() {
    let end = index(RTL_LINES, '\u{05D5}');
    assert_eq!(handle(RTL_LINES, ONE_LINE, end, false, false), at(0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_cross_rtl_overflowed
#[test]
fn the_crossed_end_handle_past_the_right_to_left_lines_shown_is_at_the_first_lines_end() {
    let end = index(RTL_LINES, '\u{05D3}');
    assert_eq!(handle(RTL_LINES, ONE_LINE, end, false, true), at(3));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_not_cross_bidi
#[test]
fn the_end_handle_at_a_right_to_left_runs_start_is_at_the_left_runs_end() {
    let end = index(BIDI, '\u{05D0}');
    assert_eq!(handle(BIDI, SIMPLE, end, false, false), at(BIDI_LTR.len()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getHandlePosition_EndHandle_cross_bidi
#[test]
fn the_crossed_end_handle_at_a_right_to_left_runs_start_is_at_the_texts_end() {
    let end = index(BIDI, '\u{05D0}');
    assert_eq!(handle(BIDI, SIMPLE, end, false, true), at(BIDI.chars().count()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getText_textLayoutResult_NotNull_Return_AnnotatedString
#[test]
fn a_texts_selectable_text_is_its_text() {
    let mut ui = laid_out(AnnotatedString::new(BIDI.to_string()), SIMPLE);
    let text = ask(&mut ui, |selectable, _| selectable.text());
    assert_eq!(text.as_str(), BIDI);
}

fn bounding_box(text: &str, offset: usize) -> Rect {
    let mut ui = laid_out(AnnotatedString::new(text.to_string()), SIMPLE);
    ask(&mut ui, |selectable, measurer| selectable.bounding_box(offset, measurer))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getBoundingBox_valid
#[test]
fn a_characters_box_is_where_it_is_laid_out() {
    let text = "hello\nworld\n";
    assert_eq!(bounding_box(text, index(text, 'w')), Rect::new(0.0, FONT, FONT, 2.0 * FONT));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getBoundingBox_zero_length_text_return_zero_rect
#[test]
fn an_empty_texts_box_is_empty() {
    assert_eq!(bounding_box("", 0), Rect::ZERO);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getBoundingBox_offset_larger_than_range_should_return_largest
#[test]
fn the_box_past_the_text_is_its_last_characters() {
    let text = "hello\nworld";
    let box_ = bounding_box(text, index(text, 'd') + 5);
    assert_eq!(box_, Rect::new(4.0 * FONT, FONT, 5.0 * FONT, 2.0 * FONT));
}

fn line_range(text: &str, layout: Layout, offset: usize) -> Range<usize> {
    let mut ui = laid_out(AnnotatedString::new(text.to_string()), layout);
    ask(&mut ui, |selectable, measurer| selectable.range_of_line_containing(offset, measurer))
}

const LINES: &str = "hello\nworld\n";

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_zeroOffset
#[test]
fn the_line_at_the_start_is_the_first() {
    assert_eq!(line_range(LINES, SIMPLE, 0), 0..5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_secondLine
#[test]
fn the_line_of_an_offset_on_the_second_is_the_second() {
    assert_eq!(line_range(LINES, SIMPLE, 7), 6..11);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_offsetPastTextLength_returnsLastLine
#[test]
fn the_line_past_the_text_is_the_last_with_text() {
    assert_eq!(line_range(LINES, SIMPLE, usize::MAX), 6..11);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_offsetAtNewline_returnsPreviousLine
#[test]
fn the_line_at_a_new_line_is_the_one_it_ends() {
    assert_eq!(line_range(LINES, SIMPLE, 5), 0..5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_emptyString_returnsEmptyRange
#[test]
fn the_line_of_an_empty_text_is_empty() {
    assert_eq!(line_range("", SIMPLE, 5), 0..0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_emptyLine_returnsEmptyNonZeroRange
#[test]
fn an_empty_line_is_an_empty_range_where_it_is() {
    assert_eq!(line_range("hello\n\nworld", SIMPLE, 6), 6..6);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_overflowed_returnsLastVisibleLine
#[test]
fn the_line_of_an_offset_past_the_most_lines_is_the_last_shown() {
    assert_eq!(line_range("hello\nworld", ONE_LINE, 6), 0..5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getRangeOfLineContaining_overflowedDueToMaxHeight_returnsLastVisibleLine
#[test]
fn the_line_of_an_offset_below_the_height_is_the_last_shown() {
    // Compose's constraints: as high as a line.
    let layout = Layout { height: Some(1), ..SIMPLE };
    assert_eq!(line_range("hello\nworld", layout, 6), 0..5);
}

fn last_visible(text: &str, layout: Layout) -> usize {
    let mut ui = laid_out(AnnotatedString::new(text.to_string()), layout);
    ask(&mut ui, |selectable, measurer| selectable.last_visible_offset(measurer))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_everythingVisible_returnsTextLength
#[test]
fn all_shown_the_last_visible_offset_is_the_texts_end() {
    assert_eq!(last_visible("hello\nworld", CONSTRAINED), 11);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_changesWhenTextLayoutChanges
#[test]
fn the_last_visible_offset_follows_a_new_layout() {
    let text = compose_runtime::mutable_state_of("hello\nworld".to_string());
    let text: &'static compose_runtime::OwnedState<String> = Box::leak(Box::new(text));
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(800, 800),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let shown = text.get();
            SelectionContainer!({
                BasicText!(
                    shown.clone(),
                    modifier = Modifier.test_tag(TAG).required_width((20.0 * FONT).dp()),
                    style = style()
                );
            });
        },
    );
    ui.wait_for_idle();
    let last = |ui: &mut ComposeTestRule| {
        ask(ui, |selectable, measurer| selectable.last_visible_offset(measurer))
    };
    assert_eq!(last(&mut ui), 11);
    text.set("hello\nworldhello\nworld".to_string());
    ui.wait_for_idle();
    assert_eq!(last(&mut ui), 22);
}

/// Compose's `constrainedTextLayout` at `width` characters, at most `max_lines` lines of
/// `overflow`, soft-wrapped or not.
fn cut(width: u32, max_lines: u32, overflow: TextOverflow, soft_wrap: bool) -> Layout {
    Layout { max_lines, overflow, soft_wrap, width: Some(width), height: None }
}

/// The same, at most `lines` lines high.
fn cut_high(width: u32, lines: u32, overflow: TextOverflow, soft_wrap: bool) -> Layout {
    Layout { height: Some(lines), ..cut(width, u32::MAX, overflow, soft_wrap) }
}

const CLIP: TextOverflow = TextOverflow::Clip;
const ELLIPSIS: TextOverflow = TextOverflow::Ellipsis;

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_clip_enabledSoftwrap_multiLineContent
#[test]
fn one_clipped_wrapped_line_of_two_shows_up_to_its_end() {
    assert_eq!(last_visible("hello\nworld", cut(20, 1, CLIP, true)), 5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_clip_enabledSoftwrap_singleLineContent
#[test]
fn one_clipped_wrapped_line_of_a_wrapped_text_shows_its_first_word() {
    assert_eq!(last_visible("hello world", cut(10, 1, CLIP, true)), 5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_clip_disabledSoftwrap_multiLineContent
#[test]
fn one_clipped_unwrapped_line_of_two_shows_up_to_its_end() {
    assert_eq!(last_visible("hello\nworld", cut(20, 1, CLIP, false)), 5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_clip_disabledSoftwrap_singleLineContent
#[test]
fn one_clipped_unwrapped_line_shows_all_its_text_but_the_last_space() {
    let text = "hello world ".repeat(10);
    assert_eq!(last_visible(&text, cut(10, 1, CLIP, false)), text.len() - 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_ellipsis_enabledSoftwrap_multiLineContent
#[test]
fn one_ellipsized_wrapped_line_shows_up_to_the_ellipsis() {
    assert_eq!(last_visible("hello\nworld", cut(4, 1, ELLIPSIS, true)), 3);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_ellipsis_enabledSoftwrap_singleLineContent
#[test]
fn one_ellipsized_wrapped_line_of_a_long_text_shows_up_to_the_ellipsis() {
    assert_eq!(last_visible(&"hello world ".repeat(10), cut(10, 1, ELLIPSIS, true)), 9);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_ellipsis_disabledSoftwrap_multiLineContent
#[test]
fn one_ellipsized_unwrapped_line_of_two_shows_up_to_the_ellipsis() {
    assert_eq!(last_visible("hello\nworld", cut(5, 1, ELLIPSIS, false)), 4);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_maxLines1_ellipsis_disabledSoftwrap_singleLineContent
#[test]
fn one_ellipsized_unwrapped_line_shows_up_to_the_ellipsis() {
    assert_eq!(last_visible(&"hello world ".repeat(10), cut(20, 1, ELLIPSIS, false)), 19);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_clip_enabledSoftwrap_multiLineContent
#[test]
fn one_clipped_wrapped_line_high_of_two_shows_up_to_its_end() {
    assert_eq!(last_visible("hello\nworld", cut_high(20, 1, CLIP, true)), 5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_clip_enabledSoftwrap_singleLineContent
#[test]
fn two_clipped_wrapped_lines_high_show_two_words() {
    let text = "helloworld helloworld helloworld";
    assert_eq!(last_visible(text, cut_high(10, 2, CLIP, true)), 21);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_clip_disabledSoftwrap_multiLineContent
#[test]
fn one_clipped_unwrapped_line_high_of_two_shows_up_to_its_end() {
    assert_eq!(last_visible("hello\nworld", cut_high(20, 1, CLIP, false)), 5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_clip_disabledSoftwrap_singleLineContent
#[test]
fn one_clipped_unwrapped_line_high_shows_all_its_text_but_the_last_space() {
    let text = "hello world ".repeat(10);
    assert_eq!(last_visible(&text, cut_high(10, 1, CLIP, false)), text.len() - 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_ellipsis_enabledSoftwrap_multiLineContent
#[test]
fn two_ellipsized_wrapped_lines_high_show_up_to_the_second_ones_end() {
    let text = "hello\nworld\nhello\nworld";
    assert_eq!(last_visible(text, cut_high(10, 2, ELLIPSIS, true)), 11);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_ellipsis_enabledSoftwrap_singleLineContent
#[test]
fn one_ellipsized_wrapped_line_high_shows_up_to_the_ellipsis() {
    assert_eq!(last_visible(&"hello world ".repeat(10), cut_high(10, 1, ELLIPSIS, true)), 9);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_ellipsis_disabledSoftwrap_multiLineContent
#[test]
fn one_ellipsized_unwrapped_line_high_of_two_shows_up_to_its_end() {
    assert_eq!(last_visible("hello\nworld", cut_high(20, 1, ELLIPSIS, false)), 5);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLastVisibleOffset_limitHeight_ellipsis_disabledSoftwrap_singleLineContent
#[test]
fn one_ellipsized_unwrapped_line_high_shows_up_to_the_ellipsis() {
    assert_eq!(last_visible(&"hello world ".repeat(10), cut_high(20, 1, ELLIPSIS, false)), 19);
}

/// "text\n" at 10 sp, "hello\n" at 20 sp, "world" at 30 sp, as Compose's line height tests.
fn three_sizes() -> AnnotatedString {
    AnnotatedString::build(|text| {
        for (part, size) in [("text\n", 10.0), ("hello\n", 20.0), ("world", 30.0)] {
            text.push_style(SpanStyle { font_size: size.sp(), ..SpanStyle::default() });
            text.append(part);
            text.pop();
        }
    })
}

fn line_height(text: AnnotatedString, max_lines: u32, offset: usize) -> f32 {
    let mut ui = laid_out(text, Layout { max_lines, ..SIMPLE });
    ask(&mut ui, |selectable, measurer| selectable.line_height(offset, measurer))
}

const SIZES: &str = "text\nhello\nworld";

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLineHeight_line_in_the_middle
#[test]
fn a_lines_height_is_its_fonts() {
    assert_eq!(line_height(three_sizes(), u32::MAX, index(SIZES, 'h')), 20.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLineHeight_offset_next_after_last_character
#[test]
fn the_height_at_the_texts_end_is_the_last_lines() {
    assert_eq!(line_height(three_sizes(), u32::MAX, SIZES.len()), 30.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLineHeight_offset_next_after_last_visible_character
#[test]
fn the_height_just_past_the_lines_shown_is_the_last_shown_lines() {
    assert_eq!(line_height(three_sizes(), 2, index(SIZES, 'w')), 20.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLineHeight_zero_length_text_return_zero
#[test]
fn an_empty_texts_line_height_is_zero() {
    assert_eq!(line_height(AnnotatedString::new(""), u32::MAX, 0), 0.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLineHeight_offset_larger_than_range_should_return_zero
#[test]
fn the_line_height_past_the_text_is_zero() {
    assert_eq!(line_height(three_sizes(), u32::MAX, index(SIZES, 'd') + 5), 0.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/MultiWidgetSelectionDelegateTest.kt, getLineHeight_character_out_of_visible_range_return_zero
#[test]
fn the_line_height_of_a_character_past_the_lines_shown_is_zero() {
    assert_eq!(line_height(three_sizes(), 2, index(SIZES, 'd')), 0.0);
}
