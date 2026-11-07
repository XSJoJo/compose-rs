//! Review of `bidi-selection` (D2062): `BasicText`'s `direction_at` as Compose's
//! `getTextDirectionForOffset` (the paragraph's direction on an empty line or in an empty text,
//! else the run's), at paragraph ends, after a trailing line feed, in a text cut by its maximum
//! lines, and across paragraphs of different directions.
//!
//! In Compose's test font (each character a square of its size, D1124), 20 px a character, ten a
//! line.

use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    AnnotatedString, Density, IntSize, ParagraphStyle, ResolvedTextDirection, TextDirection,
    UnitExt,
};

use ResolvedTextDirection::{Ltr, Rtl};

const TAG: &str = "text";

fn style() -> TextStyle {
    TextStyle { font_size: 20.0.sp(), ..TextStyle::DEFAULT }
}

fn rtl() -> ParagraphStyle {
    ParagraphStyle { text_direction: Some(TextDirection::Rtl), ..Default::default() }
}

fn ltr() -> ParagraphStyle {
    ParagraphStyle { text_direction: Some(TextDirection::Ltr), ..Default::default() }
}

/// Each `(text, style)` in a paragraph of its own.
fn paragraphs(parts: &[(&str, ParagraphStyle)]) -> AnnotatedString {
    AnnotatedString::build(|text| {
        for (part, style) in parts {
            text.with_paragraph_style(*style, |text| {
                text.append(part);
            });
        }
    })
}

/// `text` in a selection container, 200 px wide, in `style`, `max_lines` at most.
fn selectable(text: AnnotatedString, style: TextStyle, max_lines: u32) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(800, 800),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let text = text.clone();
            let style = style.clone();
            SelectionContainer!({
                BasicAnnotatedText!(
                    text.clone(),
                    modifier = Modifier.test_tag(TAG).width(200.dp()),
                    style = style.clone(),
                    max_lines = max_lines
                );
            });
        },
    );
    ui.wait_for_idle();
    ui
}

/// The text's direction at byte `offset`, as its `Selectable` says.
fn direction(ui: &mut ComposeTestRule, offset: usize) -> ResolvedTextDirection {
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().id;
    let mut answer = None;
    ui.host().with_tree(|tree| {
        tree.with_selectable(node, &mut |selectable, measurer| {
            answer = Some(selectable.direction_at(offset, measurer));
        });
    });
    answer.expect("a selectable text")
}

/// A plain text in a right-to-left style ending in a line feed: the empty last line is the
/// paragraph's, right to left; the letters' run is left to right.
#[test]
fn a_trailing_line_feed_of_a_right_to_left_text_says_its_paragraphs_direction() {
    let rtl_style = TextStyle { text_direction: Some(TextDirection::Rtl), ..style() };
    let mut ui = selectable(AnnotatedString::new("abc\n"), rtl_style, u32::MAX);
    assert_eq!(direction(&mut ui, 4), Rtl);
    assert_eq!(direction(&mut ui, 1), Ltr);
}

/// A left-to-right paragraph ending in a line feed, then a right-to-left one of neutral
/// characters. At the second's start, Compose's `MultiParagraph` finds the second paragraph
/// (`findParagraphByIndex`: an offset belongs to the paragraph it starts), whose first line also
/// holds the next character: not an empty line, its run's direction, right to left.
#[test]
fn the_start_of_a_paragraph_after_a_line_feed_is_that_paragraphs() {
    let mut ui = selectable(paragraphs(&[("abc\n", ltr()), ("!!", rtl())]), style(), u32::MAX);
    assert_eq!(direction(&mut ui, 4), Rtl);
    assert_eq!(direction(&mut ui, 5), Rtl);
    assert_eq!(direction(&mut ui, 1), Ltr);
}

/// Where a left-to-right paragraph meets a right-to-left one with no line feed between them: the
/// offset is the second's start, its run right to left, as Compose's.
#[test]
fn a_paragraph_boundary_is_the_next_paragraphs() {
    let mut ui = selectable(paragraphs(&[("Hello", ltr()), ("!!", rtl())]), style(), u32::MAX);
    assert_eq!(direction(&mut ui, 5), Rtl);
    assert_eq!(direction(&mut ui, 4), Ltr);
}

/// A right-to-left last paragraph ending in a line feed: at the text's end, its empty last line,
/// that paragraph's direction.
#[test]
fn the_empty_last_line_of_a_right_to_left_last_paragraph_is_right_to_left() {
    let mut ui = selectable(paragraphs(&[("ab", ltr()), ("!!\n", rtl())]), style(), u32::MAX);
    assert_eq!(direction(&mut ui, 5), Rtl);
}

/// Cut after an empty line: the right-to-left paragraph's line feed leaves an empty line, the
/// last shown, and the next paragraph is left out. From the cut to just before the text's end,
/// each offset is kept to the cut (D2054), on that empty line: its paragraph's direction.
#[test]
fn offsets_from_a_cut_after_an_empty_line_say_its_paragraphs_direction() {
    let text = paragraphs(&[("abc", ltr()), ("!!\n", rtl()), ("def", ltr())]);
    let mut ui = selectable(text, style(), 3);
    for offset in 6..=8 {
        assert_eq!(direction(&mut ui, offset), Rtl, "at {offset}");
    }
}

/// The same text's own end, where a select-all's end is: the one offset `laid_out_offset` leaves
/// alone (it keeps `text.len()`), which `text_direction_for_offset` answers past `end` with the
/// run's, left to right, where the offset just before it says right to left. Compose's
/// `MultiParagraph` throws there (the offset before is in a paragraph left out), so neither is
/// Compose's answer; the empty last line's paragraph's is D2062's own rule.
#[test]
fn the_end_of_a_text_cut_after_an_empty_line_says_the_same_as_the_cut() {
    let text = paragraphs(&[("abc", ltr()), ("!!\n", rtl()), ("def", ltr())]);
    let mut ui = selectable(text, style(), 3);
    assert_eq!(direction(&mut ui, 9), direction(&mut ui, 6));
}

/// The same text not cut: the empty line isn't the last, the next paragraph starts on its own
/// line, so the line feed's end is the next paragraph's start, its run's (left to right).
#[test]
fn the_same_text_not_cut_answers_at_the_next_paragraph() {
    let text = paragraphs(&[("abc", ltr()), ("!!\n", rtl()), ("def", ltr())]);
    let mut ui = selectable(text, style(), u32::MAX);
    assert_eq!(direction(&mut ui, 6), Ltr);
    assert_eq!(direction(&mut ui, 4), Rtl);
}

/// An empty text says its paragraph's direction, right to left in a right-to-left style.
#[test]
fn an_empty_text_says_its_paragraphs_direction() {
    let rtl_style = TextStyle { text_direction: Some(TextDirection::Rtl), ..style() };
    let mut ui = selectable(AnnotatedString::new(""), rtl_style, u32::MAX);
    assert_eq!(direction(&mut ui, 0), Rtl);
    let mut ui = selectable(AnnotatedString::new(""), style(), u32::MAX);
    assert_eq!(direction(&mut ui, 0), Ltr);
}

/// An empty line between two Hebrew lines of a plain right-to-left text: the paragraph's, and
/// the lines around it their runs'. A character of two bytes before and after it (the neighbours
/// are characters, not bytes).
#[test]
fn an_empty_line_between_two_byte_characters_is_found() {
    let rtl_style = TextStyle { text_direction: Some(TextDirection::Rtl), ..style() };
    let text = "\u{5d0}\n\n\u{5d1}";
    let mut ui = selectable(AnnotatedString::new(text), rtl_style, u32::MAX);
    // The second line feed's offset starts the empty line.
    assert_eq!(direction(&mut ui, 3), Rtl);
    assert_eq!(direction(&mut ui, 0), Rtl);
    assert_eq!(direction(&mut ui, 4), Rtl);
}
