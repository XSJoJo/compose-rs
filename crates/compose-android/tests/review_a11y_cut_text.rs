//! Review of `a11y-cut-text` (D2070): line and page traversal stopping where a text's maximum
//! lines cut it. Right-to-left texts, an ellipsis on the last line, an empty text, an empty last
//! line shown, and pages lower than a line, each over a plain text and a text of paragraphs.

extern crate compose_runtime as compose;

use std::cell::RefCell;
use std::rc::Rc;

use compose_android::text_segments::{Granularity, TextSegments};

use compose_foundation::*;
use compose_ui::platform::UiHost;
use compose_ui::*;

/// Each of `parts` in a paragraph of its own, in `style`: a text of paragraphs (D2053).
fn paragraphs_in(parts: &[&str], style: ParagraphStyle) -> AnnotatedString {
    AnnotatedString::build(|text| {
        for part in parts {
            text.with_paragraph_style(style, |text| {
                text.append(part);
            });
        }
    })
}

fn paragraphs(parts: &[&str]) -> AnnotatedString {
    paragraphs_in(parts, ParagraphStyle::DEFAULT)
}

fn rtl() -> ParagraphStyle {
    ParagraphStyle { text_direction: Some(TextDirection::Rtl), ..Default::default() }
}

/// `text` laid out `width` wide in `max_lines`, in `style`, with `overflow`.
fn laid_out(
    text: AnnotatedString,
    width: f32,
    max_lines: u32,
    style: TextStyle,
    overflow: TextOverflow,
) -> (UiHost, TextLayoutResult) {
    let result: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
    let heard = result.clone();
    let mut host = UiHost::headless(400, 800);
    host.set_content(move || {
        let heard = heard.clone();
        BasicAnnotatedText!(
            text.clone(),
            modifier = Modifier.width(Dp(width)),
            style = style.clone(),
            max_lines = max_lines,
            overflow = overflow,
            on_text_layout = Some(move |layout| *heard.borrow_mut() = Some(layout))
        );
    });
    host.settle(5);
    let layout = result.borrow().clone().expect("laid out");
    (host, layout)
}

fn cut(text: AnnotatedString, max_lines: u32) -> (UiHost, TextLayoutResult) {
    laid_out(text, 30.0, max_lines, TextStyle::DEFAULT, TextOverflow::Clip)
}

/// Where the layout's last line ends, in UTF-16 units of `text`.
fn shown_end(text: &str, layout: &TextLayoutResult) -> i32 {
    let last = layout.line_count as usize - 1;
    text[..layout.get_line_end(last, false)].encode_utf16().count() as i32
}

/// Every move from every place in and around `text` by lines and pages (each page `page` high):
/// none panics, and each range found is inside what the lines show.
fn every_move_is_inside_what_is_shown(
    name: &str,
    text: &str,
    layout: &TextLayoutResult,
    page: f32,
) {
    let segments = TextSegments::new(text);
    let end = shown_end(text, layout);
    for granularity in [Granularity::Line(layout), Granularity::Page(layout, page)] {
        for current in -1..=segments.len() + 1 {
            for range in
                [segments.following(granularity, current), segments.preceding(granularity, current)]
                    .into_iter()
                    .flatten()
            {
                assert!(
                    (0..=end).contains(&range.0) && (0..=end).contains(&range.1),
                    "{name}, from {current}: {range:?} past {end}"
                );
            }
            if current >= end {
                assert_eq!(segments.following(granularity, current), None, "{name}, {current}");
            }
        }
    }
}

const HEBREW: &str =
    "\u{5d0}\u{5d1}\u{5d2}\u{5d3}\u{5d4}\u{5d5}\u{5d6}\u{5d7}\u{5d8}\u{5d9}\u{5da}\u{5db}";

/// The same twelve Hebrew letters as a plain text in a right-to-left style, a text of
/// right-to-left paragraphs whose last is left out, and one whose second is cut inside: cut after
/// two lines, the first four letters and the next four.
fn rtl_cut_texts() -> Vec<(&'static str, UiHost, TextLayoutResult)> {
    let letters: Vec<String> = HEBREW.chars().map(String::from).collect();
    let part = |range: std::ops::Range<usize>| letters[range].concat();
    let rtl_style = TextStyle { text_direction: Some(TextDirection::Rtl), ..TextStyle::DEFAULT };
    [
        ("plain", AnnotatedString::new(HEBREW), rtl_style),
        (
            "the last paragraph left out",
            paragraphs_in(&[&part(0..4), &part(4..8), &part(8..12)], rtl()),
            TextStyle::DEFAULT,
        ),
        (
            "the second cut inside",
            paragraphs_in(&[&part(0..4), &part(4..10), &part(10..12)], rtl()),
            TextStyle::DEFAULT,
        ),
    ]
    .into_iter()
    .map(|(name, text, style)| {
        let (host, layout) = laid_out(text, 30.0, 2, style, TextOverflow::Clip);
        assert_eq!(layout.line_count, 2, "{name}");
        assert_eq!(shown_end(HEBREW, &layout), 8, "{name}");
        assert_eq!(layout.get_paragraph_direction(0), ResolvedTextDirection::Rtl, "{name}");
        (name, host, layout)
    })
    .collect()
}

/// Right to left, a line's edges are the other way round (`TextView`'s, as Compose's); a cut
/// text's moves stay inside what's shown, nothing follows from the cut or past it, in a text of
/// paragraphs without a panic.
#[test]
fn a_right_to_left_cut_text_is_moved_through_up_to_its_cut() {
    for (name, _host, layout) in rtl_cut_texts() {
        let line_height = layout.get_line_bottom(0);
        every_move_is_inside_what_is_shown(name, HEBREW, &layout, line_height);
        every_move_is_inside_what_is_shown(name, HEBREW, &layout, 1000.0);
    }
}

/// A page following from the cut in a right-to-left text: none, as left to right. Compose's
/// iterator gives `(8, 5)` there (its end is the last line's start plus one); the guard's `>=`
/// is what keeps it out (a `>` mutant lets it through).
#[test]
fn a_page_following_from_the_cut_of_a_right_to_left_text_is_none() {
    let segments = TextSegments::new(HEBREW);
    for (name, _host, layout) in rtl_cut_texts() {
        let line_height = layout.get_line_bottom(0);
        let page = Granularity::Page(&layout, line_height);
        assert_eq!(segments.following(page, 8), None, "{name}");
        assert_eq!(segments.following(page, 9), None, "{name}");
    }
}

/// A line preceding in a right-to-left text: from the cut, Compose's answer for a text's end (its
/// last line's start edge plus one isn't the cut, so the line before), from past it the last
/// line. What TalkBack moves back from first, the text's end, is past the cut. Pinned so a `>=`
/// in `line_preceding` (the same left to right) is seen to change right to left.
#[test]
fn a_line_preceding_in_a_right_to_left_cut_text_from_the_cut_and_past_it() {
    let segments = TextSegments::new(HEBREW);
    for (name, _host, layout) in rtl_cut_texts() {
        let line = Granularity::Line(&layout);
        // Right to left, a line's range is (its end - 1, its start + 1), as Compose's.
        assert_eq!(segments.preceding(line, 12), Some((7, 5)), "{name}");
        assert_eq!(segments.preceding(line, 9), Some((7, 5)), "{name}");
        assert_eq!(segments.preceding(line, 8), Some((3, 1)), "{name}");
    }
}

/// The same twelve letters cut after two lines with an ellipsis: lines and pages stop at the
/// cut, without a panic.
#[test]
fn an_ellipsis_on_the_last_line_is_moved_through_without_a_panic() {
    let text = "abcdefghijkl";
    for (name, annotated) in [
        ("plain", AnnotatedString::new(text)),
        ("the last paragraph left out", paragraphs(&["abcd", "efgh", "ijkl"])),
        ("the second cut inside", paragraphs(&["abcd", "efghij", "kl"])),
    ] {
        let (_host, layout) =
            laid_out(annotated, 30.0, 2, TextStyle::DEFAULT, TextOverflow::Ellipsis);
        assert_eq!(layout.line_count, 2, "{name}");
        let line_height = layout.get_line_bottom(0);
        every_move_is_inside_what_is_shown(name, text, &layout, line_height);
        let segments = TextSegments::new(text);
        let end = shown_end(text, &layout);
        // Back from the text's end, the last line shown.
        let last = segments.preceding(Granularity::Line(&layout), 12).expect("a line");
        assert_eq!(last, (4, end), "{name}");
    }
}

/// An empty text: nothing to move through, whatever its maximum lines.
#[test]
fn an_empty_text_has_no_lines_or_pages_to_move_through() {
    for max_lines in [1, u32::MAX] {
        let (_host, layout) = cut(AnnotatedString::new(""), max_lines);
        let segments = TextSegments::new("");
        for granularity in [Granularity::Line(&layout), Granularity::Page(&layout, 10.0)] {
            for current in -1..=1 {
                assert_eq!(segments.following(granularity, current), None);
                assert_eq!(segments.preceding(granularity, current), None);
            }
        }
    }
}

/// Cut after an empty line: the first paragraph's line feed leaves an empty line, the last one
/// shown, and the next paragraph is left out. Nothing panics, nothing follows past the cut.
#[test]
fn a_cut_after_an_empty_line_is_moved_through_without_a_panic() {
    let text = "abc\ndefg";
    let (_host, layout) =
        laid_out(paragraphs(&["abc\n", "defg"]), 200.0, 2, TextStyle::DEFAULT, TextOverflow::Clip);
    assert_eq!(layout.line_count, 2);
    assert_eq!(shown_end(text, &layout), 4);
    let line_height = layout.get_line_bottom(0);
    every_move_is_inside_what_is_shown("an empty last line", text, &layout, line_height);
}

/// The same, moving back from the text's end (where TalkBack starts): the last line shown is
/// empty, so a move to it is none (`non_empty`), and the traversal ends without reading the line
/// before it — TalkBack goes to the node before. Compose's own text ending in a line feed does
/// the same from its end (`getRange` of an empty line), so this is Compose's answer moved to the
/// cut; the line before (`abc\n`) is what a reader would expect. Pinned as found.
#[test]
fn a_line_preceding_the_end_of_a_text_cut_after_an_empty_line() {
    let text = "abc\ndefg";
    let (_host, layout) =
        laid_out(paragraphs(&["abc\n", "defg"]), 200.0, 2, TextStyle::DEFAULT, TextOverflow::Clip);
    let segments = TextSegments::new(text);
    let line = Granularity::Line(&layout);
    assert_eq!(segments.preceding(line, 8), None);
    // From before the first line's line feed: inside the first line, so the line before it, none.
    assert_eq!(segments.preceding(line, 3), None);
}

/// A page lower than one line (a node shown a few pixels high): each page following is at least
/// what's left of a line, never reversed, and stops at the cut; each page preceding ends where
/// it was asked from, or at the cut.
#[test]
fn pages_lower_than_a_line_move_by_lines_up_to_the_cut() {
    let text = "abcdefghijkl";
    let segments = TextSegments::new(text);
    for (name, annotated) in [
        ("plain", AnnotatedString::new(text)),
        ("the last paragraph left out", paragraphs(&["abcd", "efgh", "ijkl"])),
        ("the second cut inside", paragraphs(&["abcd", "efghij", "kl"])),
    ] {
        let (_host, layout) = cut(annotated, 2);
        for page in [0.4, 3.0] {
            every_move_is_inside_what_is_shown(name, text, &layout, page);
            let next = |current| segments.following(Granularity::Page(&layout, page), current);
            let previous = |current| segments.preceding(Granularity::Page(&layout, page), current);
            assert_eq!(next(0), Some((0, 4)), "{name}, {page}");
            assert_eq!(next(2), Some((2, 4)), "{name}, {page}");
            assert_eq!(next(4), Some((4, 8)), "{name}, {page}");
            assert_eq!(next(8), None, "{name}, {page}");
            assert_eq!(previous(12), Some((4, 8)), "{name}, {page}");
            assert_eq!(previous(6).map(|range| range.1), Some(6), "{name}, {page}");
        }
    }
}

/// A text cut after a line that ends in a line feed: its lines end after the line feed, and so
/// does what's shown (`get_line_end(_, false)`, not the visible end): a page back from the
/// text's end ends there, as the line back from it does. A visible end would leave the line feed
/// out of the page but not of the line.
#[test]
fn a_cut_after_a_line_feed_ends_after_it() {
    let text = "abc\ndef\nghi";
    for (name, annotated) in
        [("plain", AnnotatedString::new(text)), ("paragraphs", paragraphs(&["abc\ndef\n", "ghi"]))]
    {
        let (_host, layout) = laid_out(annotated, 200.0, 2, TextStyle::DEFAULT, TextOverflow::Clip);
        assert_eq!(layout.line_count, 2, "{name}");
        assert_eq!(shown_end(text, &layout), 8, "{name}");
        let segments = TextSegments::new(text);
        let line_height = layout.get_line_bottom(0);
        let end = segments.len();
        assert_eq!(segments.preceding(Granularity::Line(&layout), end), Some((4, 8)), "{name}");
        let page = Granularity::Page(&layout, line_height);
        assert_eq!(segments.preceding(page, end), Some((4, 8)), "{name}");
        assert_eq!(segments.following(Granularity::Line(&layout), 7), None, "{name}");
        every_move_is_inside_what_is_shown(name, text, &layout, line_height);
    }
}
