//! Review of `multi-paragraph` (D2047, D2048): a text split into paragraphs by its paragraph
//! styles (`MultiParagraph`), and the range queries (`get_range_for_rect`, `fill_bounding_boxes`,
//! `get_bounding_boxes_for_range`). Failing tests are ignored with the finding they show.

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, SharedTextMeasurer, UiHost};
use compose_ui::{
    AnnotatedString, Constraints, Density, DrawTextOptions, FontFamilyResolver, IntSize,
    LocaleList, MultiParagraph, MultiParagraphIntrinsics, Offset, ParagraphStyle, Rect,
    ResolvedTextDirection, TextDirection, TextGranularity, TextInclusionStrategy,
    TextMeasureOptions, TextMeasurer, TextOverflow, TextRange, TextStyle, UnitExt,
    create_font_family_resolver, remember_text_measurer,
};

fn resolver() -> FontFamilyResolver {
    create_font_family_resolver(&SharedTextMeasurer::new(Box::new(
        FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT),
    )))
}

/// Each string a paragraph of its own.
fn paragraphs(texts: &[&str]) -> AnnotatedString {
    AnnotatedString::build(|builder| {
        for text in texts {
            builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
                builder.append(text);
            });
        }
    })
}

/// `text` laid out at `width` (unbounded when `None`) in 10 px characters and lines.
fn multi(text: AnnotatedString, width: Option<i32>, max_lines: u32) -> MultiParagraph {
    let style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT };
    let intrinsics = MultiParagraphIntrinsics::new(
        text,
        &style,
        &[],
        Density::new(1.0),
        resolver(),
        true,
        LocaleList::from_language_tags("en"),
    );
    let max_width = width.unwrap_or(Constraints::INFINITY);
    MultiParagraph::new(
        Rc::new(intrinsics),
        Constraints::new(0, max_width, 0, Constraints::INFINITY),
        max_lines,
        TextOverflow::Clip,
    )
}

/// The direction of a neutral text whose paragraph's direction is `Content`, in `locale`.
fn content_direction_in(locale: &str) -> ResolvedTextDirection {
    let style = TextStyle {
        text_direction: Some(TextDirection::Content),
        locale_list: Some(LocaleList::from_language_tags(locale)),
        ..TextStyle::DEFAULT
    };
    let intrinsics = MultiParagraphIntrinsics::new(
        AnnotatedString::new("123"),
        &style,
        &[],
        Density::new(1.0),
        resolver(),
        true,
        LocaleList::from_language_tags("en"),
    );
    let paragraph = MultiParagraph::new(
        Rc::new(intrinsics),
        Constraints::unbounded(),
        u32::MAX,
        TextOverflow::Clip,
    );
    paragraph.get_paragraph_direction(0)
}

fn measurer() -> TextMeasurer {
    TextMeasurer::new(
        resolver(),
        LocaleList::from_language_tags("en"),
        Density::new(1.0),
        compose_ui::LayoutDirection::Ltr,
        8,
    )
}

// ---- Locale direction (D2047's list) ----

#[test]
fn right_to_left_locales_by_script_or_language_are_right_to_left() {
    for locale in ["ar", "he", "iw", "fa", "ur", "yi", "en-Arab", "en-arab", "en-Hebr", "ff-Adlm"] {
        assert_eq!(content_direction_in(locale), ResolvedTextDirection::Rtl, "{locale}");
    }
    for locale in ["en", "ar-Latn", "sd-Deva", "ks-Deva", "ja", ""] {
        assert_eq!(content_direction_in(locale), ResolvedTextDirection::Ltr, "{locale}");
    }
}

/// Android's `TextUtils.getLayoutDirectionFromLocale` asks ICU's `ULocale.isRightToLeft`, which
/// takes a locale without a script in its likely script: Kurdish's is Latin (`ku_Latn_TR`).
#[test]
fn kurdish_without_a_script_is_left_to_right() {
    assert_eq!(content_direction_in("ku"), ResolvedTextDirection::Ltr);
}

/// ICU's likely subtags give these Arabic or Hebrew script (by region: `pa_Arab_PK`,
/// `az_Arab_IR`, `uz_Arab_AF`; by language: Mazanderani, Northern Luri, Ladino), and its script
/// metadata marks Imperial Aramaic and Phoenician right to left.
#[test]
fn locales_right_to_left_by_their_likely_script_are_right_to_left() {
    for locale in ["pa-PK", "az-IR", "uz-AF", "mzn", "lrc", "lad", "arc-Armi", "phn-Phnx"] {
        assert_eq!(content_direction_in(locale), ResolvedTextDirection::Rtl, "{locale}");
    }
}

// ---- The plain text's MultiParagraph (`of_paragraph`) ----

/// Compose's `multiParagraph.maxLines` is the maximum the text was laid out with.
#[test]
fn a_plain_text_s_multi_paragraph_has_its_maximum_lines() {
    let options = TextMeasureOptions { max_lines: 2, ..TextMeasureOptions::default() };
    let result = measurer().measure("one\ntwo\nthree", options);
    assert_eq!(result.multi_paragraph.max_lines(), 2);
}

#[test]
fn a_text_of_paragraphs_multi_paragraph_has_its_maximum_lines() {
    let options = TextMeasureOptions { max_lines: 2, ..TextMeasureOptions::default() };
    let result = measurer().measure(paragraphs(&["one", "two", "three"]), options);
    assert_eq!(result.multi_paragraph.max_lines(), 2);
    assert_eq!(result.multi_paragraph.paragraph_info_list().len(), 2);
    assert!(result.multi_paragraph.did_exceed_max_lines());
}

/// As Compose's `TextMeasurer.layout`: as wide as the widest paragraph within the constraints;
/// an ellipsis without wrapping is one line, so the first paragraph alone.
#[test]
fn a_text_of_paragraphs_is_measured_as_compose_s_text_measurer_does() {
    let style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT };
    let at_most_200 = Constraints::new(0, 200, 0, Constraints::INFINITY);
    let options = TextMeasureOptions {
        style: style.clone(),
        constraints: at_most_200,
        ..TextMeasureOptions::default()
    };
    let result = measurer().measure(paragraphs(&["ab", "abc"]), options);
    assert_eq!(result.size, IntSize::new(30, 20));
    assert_eq!(result.multi_paragraph.width(), 30.0);
    let options = TextMeasureOptions {
        style,
        constraints: Constraints::new(0, 25, 0, Constraints::INFINITY),
        soft_wrap: false,
        overflow: TextOverflow::Ellipsis,
        ..TextMeasureOptions::default()
    };
    let result = measurer().measure(paragraphs(&["abcd", "efgh"]), options);
    assert_eq!(result.line_count, 1);
    assert_eq!(result.multi_paragraph.paragraph_info_list().len(), 1);
    assert!(result.multi_paragraph.did_exceed_max_lines());
}

/// `paragraph_info_list` is public, and documented as the paragraphs' lines and extent.
#[test]
fn a_plain_text_s_paragraph_info_has_its_lines_and_height() {
    let result = measurer().measure("one\ntwo", TextMeasureOptions::default());
    let paragraph = &result.multi_paragraph;
    let info = &paragraph.paragraph_info_list()[0];
    assert_eq!(info.end_line_index, paragraph.line_count());
    assert_eq!(info.bottom, paragraph.height());
}

// ---- Drawing ----

/// A host counting its layouts, whose one node draws `draw` on each frame `tick` asks.
fn counted_drawing(
    tick: MutableState<u32>,
    draw: impl Fn(&mut compose_ui::DrawScope<'_>, &TextMeasurer, u32) + 'static,
) -> (UiHost, TextLayoutCounter) {
    let (measurer, counter) = CountingTextMeasurer::new(FixedTextMeasurer::new());
    let mut host = UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer));
    let draw = Rc::new(draw);
    host.set_content(move || {
        let measurer = remember_text_measurer!();
        let draw = draw.clone();
        Box!(
            modifier = Modifier.fill_max_size().draw_behind(move |scope| {
                draw(scope, &measurer, tick.get());
            })
        );
    });
    host.frame();
    (host, counter)
}

#[test]
fn drawing_a_plain_text_in_its_own_style_paints_nothing_new() {
    let owner = mutable_state_of(1u32);
    let tick = owner.handle();
    let (mut host, counter) = counted_drawing(tick, |scope, measurer, _| {
        scope.draw_text(measurer, "Hello", DrawTextOptions::default());
    });
    for step in 2..4 {
        tick.set(step);
        host.frame();
    }
    let counts = counter.of("Hello");
    assert_eq!((counts.fresh, counts.restyled), (1, 0), "{counts:?}");
}

/// Each paragraph is laid out in its style resolved (black, no shadow nor decoration), while a
/// drawing's paint is the input's unresolved style: the two never match, so each paragraph is
/// painted anew on its first drawing (a `painted` copy, a JNI call and a new handle on Android).
#[test]
fn drawing_a_text_of_paragraphs_in_its_own_style_paints_nothing_new() {
    let owner = mutable_state_of(1u32);
    let tick = owner.handle();
    let (mut host, counter) = counted_drawing(tick, |scope, measurer, _| {
        scope.draw_text(measurer, paragraphs(&["First", "Second"]), DrawTextOptions::default());
    });
    for step in 2..4 {
        tick.set(step);
        host.frame();
    }
    for text in ["First", "Second"] {
        let counts = counter.of(text);
        assert_eq!((counts.fresh, counts.restyled), (1, 0), "{text}: {counts:?}");
    }
}

/// A text of paragraphs measured on a stack of its own, drawn by a host: each paragraph is laid
/// out again there once, in the paint the drawing asks for (D2049).
#[test]
fn drawing_a_text_of_paragraphs_measured_elsewhere_paints_nothing_new() {
    let result =
        measurer().measure(paragraphs(&["First", "Second"]), TextMeasureOptions::default());
    let owner = mutable_state_of(1u32);
    let tick = owner.handle();
    let (mut host, counter) = counted_drawing(tick, move |scope, _, _| {
        compose_ui::TextPainter::paint(scope, &result);
    });
    for step in 2..4 {
        tick.set(step);
        host.frame();
    }
    for text in ["First", "Second"] {
        let counts = counter.of(text);
        assert_eq!((counts.fresh, counts.restyled), (1, 0), "{text}: {counts:?}");
    }
}

// ---- Queries at paragraph boundaries ----

/// The end of a paragraph is the start of the next: its cursor, position and word are there.
#[test]
fn an_offset_at_a_paragraph_s_end_is_in_the_next_paragraph() {
    let paragraph = multi(paragraphs(&["ab", "cd"]), Some(100), u32::MAX);
    assert_eq!(paragraph.get_cursor_rect(2), Rect::new(0.0, 10.0, 0.0, 20.0));
    assert_eq!(paragraph.get_horizontal_position(2, true), 0.0);
    assert_eq!(paragraph.get_line_for_offset(2), 1);
    assert_eq!(paragraph.get_word_boundary(2), TextRange::new(2, 4));
    assert_eq!(paragraph.get_bounding_box(2), Rect::new(0.0, 10.0, 10.0, 20.0));
}

/// A height exactly at a paragraph's bottom is in the next.
#[test]
fn a_height_at_a_paragraph_s_bottom_is_in_the_next_paragraph() {
    let paragraph = multi(paragraphs(&["ab", "cd"]), Some(100), u32::MAX);
    assert_eq!(paragraph.get_line_for_vertical_position(10.0), 1);
    assert_eq!(paragraph.get_offset_for_position(Offset::new(0.0, 10.0)), 2);
    assert_eq!(paragraph.get_offset_for_position(Offset::new(0.0, 9.9)), 0);
}

/// An empty paragraph between two: its line is its own, an offset in it its start.
#[test]
fn an_empty_paragraph_between_two_has_its_line() {
    let paragraph = multi(paragraphs(&["ab", "", "cd"]), Some(100), u32::MAX);
    assert_eq!(paragraph.line_count(), 3);
    assert_eq!(paragraph.get_line_for_vertical_position(15.0), 1);
    assert_eq!(paragraph.get_offset_for_position(Offset::new(50.0, 15.0)), 2);
    assert_eq!(paragraph.get_line_top(2), 20.0);
    assert_eq!(paragraph.get_cursor_rect(4), Rect::new(20.0, 20.0, 20.0, 30.0));
}

/// A paragraph ending in a line feed has an empty last line of its own.
#[test]
fn a_paragraph_ending_in_a_line_feed_has_an_empty_last_line() {
    let paragraph = multi(paragraphs(&["ab\n", "cd"]), Some(100), u32::MAX);
    assert_eq!(paragraph.line_count(), 3);
    assert_eq!(paragraph.get_line_for_offset(3), 2);
    assert_eq!(paragraph.get_line_top(2), 20.0);
    assert_eq!(paragraph.height(), 30.0);
}

/// The boxes of characters outside the basic plane: one each, each paragraph's below the ones
/// above, from a range starting in the second paragraph.
#[test]
fn boxes_of_characters_of_many_bytes_across_paragraphs() {
    let text = paragraphs(&["é😀", "a😀b"]);
    let paragraph = multi(text.clone(), Some(100), u32::MAX);
    let start = "é😀".len();
    let mut array = vec![-1.0; 4 * 4];
    paragraph.fill_bounding_boxes(TextRange::new(start, text.len()), &mut array, 4);
    assert_eq!(&array[..4], &[-1.0; 4]);
    assert_eq!(&array[4..8], &[0.0, 10.0, 10.0, 20.0]);
    assert_eq!(&array[8..12], &[10.0, 10.0, 20.0, 20.0]);
    assert_eq!(&array[12..16], &[20.0, 10.0, 30.0, 20.0]);
    let boxes = paragraph.get_bounding_boxes_for_range(start - "😀".len(), start + 1);
    assert_eq!(boxes, vec![Rect::new(10.0, 0.0, 20.0, 10.0), Rect::new(0.0, 10.0, 10.0, 20.0)]);
}

/// The narrowest width is the widest paragraph's narrowest (its widest word with the space after
/// it, as Compose's `minIntrinsicWidth`), not their sum.
#[test]
fn the_narrowest_width_is_the_widest_paragraph_s() {
    let paragraph = multi(paragraphs(&["abc d", "ef"]), Some(100), u32::MAX);
    assert_eq!(paragraph.min_intrinsic_width(), 40.0);
    assert_eq!(paragraph.max_intrinsic_width(), 50.0);
}

/// The end of a text cut by the maximum lines is the end of the last paragraph laid out.
#[test]
fn the_end_of_a_text_cut_by_the_maximum_lines_is_the_last_paragraph_s_end() {
    let text = paragraphs(&["a\nb", "c"]);
    let length = text.len();
    let paragraph = multi(text, Some(100), 1);
    assert_eq!(paragraph.paragraph_info_list().len(), 1);
    assert_eq!(paragraph.get_cursor_rect(length), paragraph.get_cursor_rect(3));
    assert_eq!(
        paragraph.get_horizontal_position(length, true),
        paragraph.get_horizontal_position(3, true)
    );
}

/// Unbounded, a text of paragraphs is as wide as its widest (D2047; Compose's is
/// `Int.MAX_VALUE` wide).
#[test]
fn an_unbounded_text_of_paragraphs_is_as_wide_as_its_widest() {
    let paragraph = multi(paragraphs(&["a", "abc", "ab"]), None, u32::MAX);
    assert_eq!(paragraph.width(), 30.0);
}

/// A rectangle down into a paragraph with nothing in it ends in the one above.
#[test]
fn a_rectangle_ending_in_a_paragraph_with_nothing_in_it_ends_in_the_one_above() {
    let paragraph = multi(paragraphs(&["abc", "def", "g"]), Some(100), u32::MAX);
    let rect = Rect::new(15.0, 5.0, 25.0, 25.0);
    let range = paragraph.get_range_for_rect(
        rect,
        TextGranularity::Character,
        &TextInclusionStrategy::AnyOverlap,
    );
    assert_eq!(range, TextRange::new(1, 6));
}

/// The boxes of a range across an empty paragraph: it has none (Compose skips it).
#[test]
fn boxes_across_an_empty_paragraph() {
    let text = paragraphs(&["ab", "", "cd"]);
    let paragraph = multi(text, Some(100), u32::MAX);
    let mut array = vec![0.0; 4 * 4];
    paragraph.fill_bounding_boxes(TextRange::new(0, 4), &mut array, 0);
    let tops: Vec<f32> = array.chunks(4).map(|chunk| chunk[1]).collect();
    assert_eq!(tops, [0.0, 0.0, 20.0, 20.0]);
    let path = paragraph.get_path_for_range(1, 3);
    assert_eq!(path.get_bounds(), Rect::new(0.0, 0.0, 20.0, 30.0));
}

/// A rectangle exactly as tall as the first paragraph is in it alone, whatever the strategy.
#[test]
fn a_rectangle_down_to_a_paragraph_s_bottom_is_in_that_paragraph() {
    let paragraph = multi(paragraphs(&["abc", "def"]), Some(100), u32::MAX);
    let everything = TextInclusionStrategy::new(|_, _| true);
    let rect = Rect::new(0.0, 0.0, 30.0, 10.0);
    let range = paragraph.get_range_for_rect(rect, TextGranularity::Character, &everything);
    assert_eq!(range, TextRange::new(0, 3));
}

/// A rectangle from exactly the last line's bottom down: Compose's check is `top > bottom`, so
/// the last line is still asked, and a strategy taking everything takes it.
#[test]
fn a_rectangle_from_the_last_line_s_bottom_still_asks_the_last_line() {
    let paragraph = multi(paragraphs(&["abc"]), Some(100), u32::MAX);
    let everything = TextInclusionStrategy::new(|_, _| true);
    let rect = Rect::new(0.0, 10.0, 30.0, 20.0);
    let range = paragraph.get_range_for_rect(rect, TextGranularity::Character, &everything);
    assert_eq!(range, TextRange::new(0, 3));
    // Above the first line: nothing, whatever the strategy.
    let above = Rect::new(0.0, -20.0, 30.0, -10.0);
    let none = paragraph.get_range_for_rect(above, TextGranularity::Character, &everything);
    assert_eq!(none, TextRange::ZERO);
}

/// A no-break space isn't Java's whitespace: a word piece of its own.
#[test]
fn a_no_break_space_is_a_word_piece() {
    let text = "ab\u{00A0}cd";
    let paragraph = multi(paragraphs(&[text]), Some(100), u32::MAX);
    let rect = Rect::new(24.0, 0.0, 26.0, 10.0);
    let range = paragraph.get_range_for_rect(
        rect,
        TextGranularity::Word,
        &TextInclusionStrategy::ContainsCenter,
    );
    assert_eq!(range, TextRange::new(2, 4));
}

/// Each paragraph's room is the maximum height less the paragraphs above rounded up, as
/// Compose's: 10.25 px lines in 31 px leave 20 px (one line, ellipsized) to the second, where
/// 21 px would hold two.
#[test]
fn the_height_left_to_a_paragraph_is_rounded_down() {
    let style = TextStyle { font_size: 10.25.sp(), ..TextStyle::DEFAULT };
    let intrinsics = MultiParagraphIntrinsics::new(
        paragraphs(&["a", "b\nc\nd"]),
        &style,
        &[],
        Density::new(1.0),
        resolver(),
        true,
        LocaleList::from_language_tags("en"),
    );
    let paragraph = MultiParagraph::new(
        Rc::new(intrinsics),
        Constraints::new(0, 100, 0, 31),
        u32::MAX,
        TextOverflow::Ellipsis,
    );
    assert_eq!(paragraph.line_count(), 2);
    assert!(paragraph.did_exceed_max_lines());
}

/// The placeholders of the paragraphs the maximum lines leave out are `None`, one for each.
#[test]
fn the_placeholders_of_paragraphs_left_out_are_none() {
    use compose_ui::{AnnotatedRange, Placeholder, PlaceholderVerticalAlign};
    let placeholder = || Placeholder::new(1.em(), 1.em(), PlaceholderVerticalAlign::AboveBaseline);
    let placeholders = vec![
        AnnotatedRange::new(placeholder(), 0, 1),
        AnnotatedRange::new(placeholder(), 2, 3),
        AnnotatedRange::new(placeholder(), 4, 5),
    ];
    let style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT };
    let intrinsics = MultiParagraphIntrinsics::new(
        paragraphs(&["ab", "cd", "ef"]),
        &style,
        &placeholders,
        Density::new(1.0),
        resolver(),
        true,
        LocaleList::from_language_tags("en"),
    );
    let paragraph = MultiParagraph::new(
        Rc::new(intrinsics),
        Constraints::new(0, 100, 0, Constraints::INFINITY),
        1,
        TextOverflow::Clip,
    );
    let rects = paragraph.placeholder_rects();
    assert_eq!(rects.len(), 3);
    assert!(rects[0].is_some());
    assert_eq!(&rects[1..], &[None, None]);
}

/// A rectangle across paragraphs: from the first character in it to the last.
#[test]
fn the_range_in_a_rectangle_across_paragraphs() {
    let paragraph = multi(paragraphs(&["abc", "", "def"]), Some(100), u32::MAX);
    let rect = Rect::new(15.0, 5.0, 25.0, 25.0);
    let range = paragraph.get_range_for_rect(
        rect,
        TextGranularity::Character,
        &TextInclusionStrategy::AnyOverlap,
    );
    assert_eq!(range, TextRange::new(1, 6));
    // Only the empty paragraph: nothing.
    let empty = Rect::new(0.0, 12.0, 100.0, 18.0);
    let none = paragraph.get_range_for_rect(
        empty,
        TextGranularity::Character,
        &TextInclusionStrategy::AnyOverlap,
    );
    assert_eq!(none, TextRange::ZERO);
}
