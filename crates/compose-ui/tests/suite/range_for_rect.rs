//! `getRangeForRect` with `TextGranularity` and `TextInclusionStrategy` (D2048), adapted from
//! Compose's `AndroidParagraphGetRangeForRectTest` and `MultiParagraphGetRangeForRectTest`: laid
//! out by the fixed measurer with Compose's test font metrics (`FixedFontMetrics::SAMPLE_FONT`:
//! one em wide, lines one em high), as Compose's tests lay out with its `sample_font`.

use std::rc::Rc;

use compose_ui::platform::{
    FixedFontMetrics, FixedTextMeasurer, SharedTextMeasurer, TextLayoutOptions, TextMeasurer,
};
use compose_ui::{
    AnnotatedString, Constraints, Density, IntSize, LayoutDirection, LocaleList, MultiParagraph,
    MultiParagraphIntrinsics, Offset, Paragraph, ParagraphStyle, Rect, TextDirection,
    TextGranularity, TextInclusionStrategy, TextLayoutInput, TextLayoutResult, TextOverflow,
    TextRange, TextStyle, UnitExt, create_font_family_resolver,
};

const FONT_SIZE: f32 = 10.0;

/// `text` at 10 sp, `width` wide (its own width when `None`), going by its first strong
/// character: Compose's `simpleParagraph`.
pub(super) fn paragraph(text: &str, width: Option<f32>) -> Rc<dyn Paragraph> {
    let mut measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let constraints = match width {
        Some(width) => Constraints::new(width as i32, width as i32, 0, Constraints::INFINITY),
        None => Constraints::unbounded(),
    };
    let style = TextStyle {
        font_size: FONT_SIZE.sp(),
        text_direction: Some(TextDirection::Content),
        ..TextStyle::DEFAULT
    };
    let handle = measurer.layout_annotated(
        &AnnotatedString::from(text.to_string()),
        &style,
        TextLayoutOptions::default(),
        constraints,
        &Density::new(1.0),
    );
    measurer.paragraph(handle)
}

/// The smallest rect holding `rects`.
fn bounding_box_of(rects: &[Rect]) -> Rect {
    Rect::new(
        rects.iter().map(|rect| rect.left).fold(f32::INFINITY, f32::min),
        rects.iter().map(|rect| rect.top).fold(f32::INFINITY, f32::min),
        rects.iter().map(|rect| rect.right).fold(f32::NEG_INFINITY, f32::max),
        rects.iter().map(|rect| rect.bottom).fold(f32::NEG_INFINITY, f32::max),
    )
}

/// The smallest rect holding the boxes of the characters at `offsets`.
fn boxes_of(paragraph: &dyn Paragraph, offsets: &[usize]) -> Rect {
    let rects: Vec<Rect> =
        offsets.iter().map(|&offset| paragraph.get_bounding_box(offset)).collect();
    bounding_box_of(&rects)
}

/// A tiny rect at `point`.
fn at_point(point: Offset) -> Rect {
    Rect::new(point.x, point.y, point.x + 0.01, point.y + 0.01)
}

/// Compose's `rangeOf(first, last)`: from `first`'s offset to after `last`.
fn range_of(text: &str, first: char, last: char) -> TextRange {
    let end = text.find(last).unwrap();
    TextRange::new(text.find(first).unwrap(), end + last.len_utf8())
}

fn range(
    paragraph: &dyn Paragraph,
    rect: Rect,
    granularity: TextGranularity,
    strategy: TextInclusionStrategy,
) -> TextRange {
    paragraph.get_range_for_rect(rect, granularity, &strategy)
}

use TextGranularity::{Character, Word};
use TextInclusionStrategy::{AnyOverlap, ContainsAll, ContainsCenter};

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_containsCenter_singleLine
#[test]
fn the_characters_whose_centers_are_in_an_area() {
    let text = "abcdef";
    let paragraph = paragraph(text, None);
    assert_eq!(paragraph.line_count(), 1);
    for start in 0..text.len() {
        for end in start + 1..=text.len() {
            let first = paragraph.get_bounding_box(start).center();
            let last = paragraph.get_bounding_box(end - 1).center();
            // Compose's `boundingBoxOf(offset1, offset2)`: a hundredth past the points.
            let rect = Rect::new(first.x, first.y, last.x + 0.01, last.y + 0.01);
            let found = range(&*paragraph, rect, Character, ContainsCenter);
            assert_eq!(found, TextRange::new(start, end), "{start}..{end}");
        }
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_containsAll_singleLine
#[test]
fn the_characters_all_in_an_area() {
    let text = "abcdef";
    let paragraph = paragraph(text, None);
    for start in 0..text.len() {
        for end in start + 1..=text.len() {
            let rect = boxes_of(&*paragraph, &[start, end - 1]);
            let found = range(&*paragraph, rect, Character, ContainsAll);
            assert_eq!(found, TextRange::new(start, end), "{start}..{end}");
        }
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_anyOverlap_singleLine
#[test]
fn the_characters_overlapping_an_area() {
    let text = "abcdef";
    let paragraph = paragraph(text, None);
    let top = paragraph.get_line_top(0);
    for start in 0..text.len() {
        for end in start + 1..=text.len() {
            let boxes = boxes_of(&*paragraph, &[start, end - 1]);
            let rect = Rect::new(boxes.left, top, boxes.right, top + 1.0);
            let found = range(&*paragraph, rect, Character, AnyOverlap);
            assert_eq!(found, TextRange::new(start, end), "{start}..{end}");
        }
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_multiLine
#[test]
fn the_characters_in_an_area_across_lines() {
    let text = "abcdef";
    let paragraph = paragraph(text, Some(FONT_SIZE * 3.0));
    assert_eq!(paragraph.line_count(), 2);
    let rect = boxes_of(&*paragraph, &[1, 4]);
    assert_eq!(range(&*paragraph, rect, Character, ContainsCenter), range_of(text, 'b', 'e'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_singleLine_compoundCharacter
#[test]
fn a_character_of_two_utf16_units_is_one() {
    let text = "ab\u{1F603}def";
    let paragraph = paragraph(text, None);
    assert_eq!(paragraph.line_count(), 1);
    let emoji = text.find('\u{1F603}').unwrap();
    let rect = boxes_of(&*paragraph, &[1, emoji]);
    assert_eq!(
        range(&*paragraph, rect, Character, ContainsCenter),
        range_of(text, 'b', '\u{1F603}')
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_BiDi
#[test]
fn the_characters_in_an_area_across_directions() {
    let text = "abc\u{05D1}\u{05D2}\u{05D3}";
    let paragraph = paragraph(text, None);
    let rect = boxes_of(&*paragraph, &[text.find('c').unwrap(), text.find('\u{05D3}').unwrap()]);
    assert_eq!(
        range(&*paragraph, rect, Character, ContainsCenter),
        range_of(text, 'c', '\u{05D3}')
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_singleLine_empty
#[test]
fn an_area_holding_no_center_holds_nothing() {
    let paragraph = paragraph("abcdef", None);
    let (top, bottom) = (paragraph.get_line_top(0), paragraph.get_line_bottom(0));
    let left = 2.0 * FONT_SIZE;
    let rect = Rect::new(left, top, left + FONT_SIZE * 0.4, bottom);
    assert_eq!(range(&*paragraph, rect, Character, ContainsCenter), TextRange::ZERO);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_singleLine_containsCenter
#[test]
fn the_word_whose_center_is_in_an_area() {
    let text = "abc def";
    let paragraph = paragraph(text, None);
    let rect = at_point(paragraph.get_bounding_box(1).center());
    assert_eq!(range(&*paragraph, rect, Word, ContainsCenter), range_of(text, 'a', 'c'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_singleLine_containsAll
#[test]
fn the_words_all_in_an_area() {
    let text = "abc def hij";
    let paragraph = paragraph(text, None);
    let rect = boxes_of(&*paragraph, &[text.find('d').unwrap(), text.find('i').unwrap()]);
    assert_eq!(range(&*paragraph, rect, Word, ContainsAll), range_of(text, 'd', 'f'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_singleLine_anyOverlap
#[test]
fn the_word_overlapping_an_area() {
    let text = "abc def hij";
    let paragraph = paragraph(text, None);
    let corner = paragraph.get_bounding_box(text.find('d').unwrap());
    let rect = at_point(Offset::new(corner.left, corner.top));
    assert_eq!(range(&*paragraph, rect, Word, AnyOverlap), range_of(text, 'd', 'f'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_singleLine_excludeSpace
#[test]
fn a_space_is_no_word() {
    let text = "abc def";
    let paragraph = paragraph(text, None);
    let rect = paragraph.get_bounding_box(text.find(' ').unwrap());
    assert_eq!(range(&*paragraph, rect, Word, ContainsCenter), TextRange::ZERO);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_multiLine
#[test]
fn the_words_in_an_area_across_lines() {
    let text = "abc def ghk lmn";
    let paragraph = paragraph(text, Some(FONT_SIZE * 7.0));
    assert_eq!(paragraph.line_count(), 2);
    let rect = boxes_of(&*paragraph, &[text.find('b').unwrap(), text.find('h').unwrap()]);
    assert_eq!(range(&*paragraph, rect, Word, ContainsCenter), range_of(text, 'a', 'k'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_multiLine_excludeSpace
#[test]
fn a_space_on_another_line_is_no_word() {
    let text = "abc def g hi";
    let paragraph = paragraph(text, Some(FONT_SIZE * 7.0));
    assert_eq!(paragraph.line_count(), 2);
    let rect = boxes_of(&*paragraph, &[text.find('b').unwrap(), 9]);
    assert_eq!(range(&*paragraph, rect, Word, ContainsCenter), range_of(text, 'a', 'c'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/AndroidParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_singleLine_BiDi
#[test]
fn the_words_in_an_area_across_directions() {
    let text = "abc \u{05D1}\u{05D2} \u{05D3}\u{05D4}";
    let paragraph = paragraph(text, None);
    let rect = boxes_of(&*paragraph, &[text.find('b').unwrap(), text.find('\u{05D3}').unwrap()]);
    assert_eq!(range(&*paragraph, rect, Word, ContainsCenter), TextRange::new(0, text.len()));
}

#[test]
fn a_strategy_of_the_application_s_decides_what_s_in() {
    let paragraph = paragraph("abcdef", None);
    let line = Rect::new(0.0, 0.0, 60.0, 10.0);
    // Only the characters from 20 pixels on.
    let from_20 = TextInclusionStrategy::new(|bounds, _| bounds.left >= 20.0);
    assert_eq!(paragraph.get_range_for_rect(line, Character, &from_20), TextRange::new(2, 6));
    let none = TextInclusionStrategy::new(|_, _| false);
    assert_eq!(paragraph.get_range_for_rect(line, Character, &none), TextRange::ZERO);
}

/// Each string a paragraph, at 10 sp, `width` wide (unbounded when `None`).
fn multi(texts: &[&str], width: Option<f32>) -> (MultiParagraph, String) {
    let text = AnnotatedString::build(|builder| {
        for text in texts {
            builder.push_paragraph_style(ParagraphStyle::DEFAULT);
            builder.append(text);
            builder.pop();
        }
    });
    let string = text.as_str().to_string();
    let resolver = create_font_family_resolver(&SharedTextMeasurer::new(Box::new(
        FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT),
    )));
    let style = TextStyle { font_size: FONT_SIZE.sp(), ..TextStyle::DEFAULT };
    let intrinsics = MultiParagraphIntrinsics::new(
        text,
        &style,
        &[],
        Density::new(1.0),
        resolver,
        true,
        LocaleList::from_language_tags("en"),
    );
    let max_width = width.map_or(Constraints::INFINITY, |width| width.ceil() as i32);
    let constraints = Constraints::new(0, max_width, 0, Constraints::INFINITY);
    let paragraph =
        MultiParagraph::new(Rc::new(intrinsics), constraints, u32::MAX, TextOverflow::Clip);
    (paragraph, string)
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_rectCoversAllParagraphs
#[test]
fn the_characters_in_an_area_across_paragraphs() {
    let (paragraph, text) = multi(&["abc", "def", "ghi"], None);
    assert_eq!(paragraph.line_count(), 3);
    let rect = Rect::new(10.0, paragraph.get_line_top(0), 20.0, paragraph.get_line_bottom(2));
    let found = paragraph.get_range_for_rect(rect, Character, &ContainsCenter);
    assert_eq!(found, range_of(&text, 'b', 'h'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_rectCoversNothing
#[test]
fn an_area_across_paragraphs_holding_no_center_holds_nothing() {
    let (paragraph, _) = multi(&["abc", "def"], None);
    let rect = Rect::new(10.0, paragraph.get_line_top(0), 14.0, paragraph.get_line_bottom(1));
    assert_eq!(paragraph.get_range_for_rect(rect, Character, &ContainsCenter), TextRange::ZERO);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphGetRangeForRectTest.kt, getRangeForRect_characterGranularity_rectCoversSingleParagraph
#[test]
fn the_characters_in_an_area_in_one_paragraph() {
    let (paragraph, text) = multi(&["abcd", "efg"], Some(3.0 * FONT_SIZE));
    assert_eq!(paragraph.line_count(), 3);
    let rect = Rect::new(10.0, paragraph.get_line_top(1), 20.0, paragraph.get_line_bottom(2));
    let found = paragraph.get_range_for_rect(rect, Character, &ContainsCenter);
    assert_eq!(found, range_of(&text, 'f', 'f'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_rectCoversAllParagraphs
#[test]
fn the_words_in_an_area_across_paragraphs() {
    let (paragraph, text) = multi(&["ab cd", "ef", "gh ij"], None);
    let rect = Rect::new(30.0, paragraph.get_line_top(0), 50.0, paragraph.get_line_bottom(2));
    let found = paragraph.get_range_for_rect(rect, Word, &ContainsCenter);
    assert_eq!(found, range_of(&text, 'c', 'j'));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphGetRangeForRectTest.kt, getRangeForRect_wordLevel_excludeSpaces
#[test]
fn spaces_across_paragraphs_are_no_words() {
    let (paragraph, _) = multi(&["ab cd", "ef gh", "ij kl"], None);
    let rect = Rect::new(20.0, paragraph.get_line_top(0), 30.0, paragraph.get_line_bottom(2));
    assert_eq!(paragraph.get_range_for_rect(rect, Word, &ContainsCenter), TextRange::ZERO);
}

/// What `simple_result` lays out: Compose's `simpleTextLayoutResult`'s arguments.
struct ResultSetup {
    style: TextStyle,
    max_lines: u32,
    overflow: TextOverflow,
    width: Option<f32>,
}

impl Default for ResultSetup {
    fn default() -> Self {
        ResultSetup {
            style: TextStyle { font_size: FONT_SIZE.sp(), ..TextStyle::DEFAULT },
            max_lines: u32::MAX,
            overflow: TextOverflow::Clip,
            width: None,
        }
    }
}

/// `text`'s `MultiParagraph` as a `TextLayoutResult`, as Compose's `simpleTextLayoutResult`.
fn simple_result(text: impl Into<AnnotatedString>, setup: ResultSetup) -> TextLayoutResult {
    let text = text.into();
    let resolver = create_font_family_resolver(&SharedTextMeasurer::new(Box::new(
        FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT),
    )));
    let locales = LocaleList::from_language_tags("en");
    let intrinsics = MultiParagraphIntrinsics::new(
        text.clone(),
        &setup.style,
        &[],
        Density::new(1.0),
        resolver.clone(),
        true,
        locales.clone(),
    );
    let max_width = setup.width.map_or(Constraints::INFINITY, |width| width.ceil() as i32);
    let constraints = Constraints::new(0, max_width, 0, Constraints::INFINITY);
    let paragraph =
        MultiParagraph::new(Rc::new(intrinsics), constraints, setup.max_lines, setup.overflow);
    let size = constraints
        .constrain(IntSize::new(paragraph.width().ceil() as i32, paragraph.height().ceil() as i32));
    let input = TextLayoutInput {
        text,
        style: setup.style,
        max_lines: setup.max_lines,
        soft_wrap: true,
        overflow: setup.overflow,
        density: Density::new(1.0),
        layout_direction: LayoutDirection::Ltr,
        font_family_resolver: resolver,
        default_locale_list: locales,
        constraints,
        placeholders: None,
    };
    TextLayoutResult::new(input, Rc::new(paragraph), size)
}

/// Whether `actual` is within `tolerance` of `expected`.
fn near(actual: f32, expected: f32, tolerance: f32) -> bool {
    (actual - expected).abs() <= tolerance
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_emptyRange_returnsEmptyList
#[test]
fn an_empty_range_has_no_boxes() {
    let result = simple_result("Hello World", ResultSetup::default());
    assert!(result.get_bounding_boxes_for_range(3, 3).is_empty());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_singleLineLtr_returnsSingleMergedBox
#[test]
fn a_range_on_a_left_to_right_line_is_one_box() {
    let result = simple_result("abcdef", ResultSetup::default());
    let boxes = result.get_bounding_boxes_for_range(1, 4);
    assert_eq!(boxes, [Rect::new(10.0, result.get_line_top(0), 40.0, result.get_line_bottom(0))]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_singleLineRtl_returnsSingleMergedBox
#[test]
fn a_range_on_a_right_to_left_line_is_one_box() {
    let text = "\u{05D0}\u{05D1}\u{05D2}\u{05D3}";
    let style = TextStyle {
        text_direction: Some(TextDirection::Rtl),
        locale_list: Some(LocaleList::from_language_tags("ar")),
        ..ResultSetup::default().style
    };
    let result =
        simple_result(text, ResultSetup { style, width: Some(100.0), ..ResultSetup::default() });
    let hebrew = '\u{05D0}'.len_utf8();
    let boxes = result.get_bounding_boxes_for_range(hebrew, 3 * hebrew);
    assert_eq!(boxes, [Rect::new(70.0, result.get_line_top(0), 90.0, result.get_line_bottom(0))]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_mixedBidiBoundary_returnsDisjointVisualBoxesInLtrOrder
#[test]
fn a_range_across_a_direction_change_is_boxes_left_to_right() {
    let text = "abc \u{05D0}\u{05D1}\u{05D2} def";
    let style = TextStyle {
        font_size: 20.sp(),
        text_direction: Some(TextDirection::Ltr),
        ..TextStyle::DEFAULT
    };
    let result =
        simple_result(text, ResultSetup { style, width: Some(1000.0), ..ResultSetup::default() });
    let hebrew = '\u{05D0}'.len_utf8();
    // 'c', ' ', and two of the three Hebrew letters: apart from the third.
    let boxes = result.get_bounding_boxes_for_range(2, 4 + 2 * hebrew);
    assert!(boxes.len() >= 2, "{boxes:?}");
    for pair in boxes.windows(2) {
        assert!(pair[0].right <= pair[1].left, "{boxes:?}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_withWordJoiners_coalescesIntoSingleContinuousBox
#[test]
fn word_joiners_join_the_boxes() {
    let text = "ab\u{2060}cd\u{2060}ef";
    let result = simple_result(text, ResultSetup::default());
    let boxes = result.get_bounding_boxes_for_range(0, text.len());
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    assert!(near(boxes[0].left, 0.0, 0.5) && near(boxes[0].right, 60.0, 0.5), "{boxes:?}");
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_multiLineWrapping_excludesTrailingWhitespace
#[test]
fn a_wrapped_line_s_box_leaves_its_trailing_spaces_out() {
    let result =
        simple_result("abc   def", ResultSetup { width: Some(40.0), ..ResultSetup::default() });
    assert_eq!(result.line_count, 2);
    let boxes = result.get_bounding_boxes_for_range(0, 9);
    assert_eq!(
        boxes,
        [
            Rect::new(0.0, result.get_line_top(0), 30.0, result.get_line_bottom(0)),
            Rect::new(0.0, result.get_line_top(1), 30.0, result.get_line_bottom(1)),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_ellipsizedLines_clampsToVisibleCharacters
#[test]
fn an_ellipsized_line_s_boxes_stop_at_its_ellipsis() {
    let setup = ResultSetup {
        max_lines: 1,
        overflow: TextOverflow::Ellipsis,
        width: Some(50.0),
        ..ResultSetup::default()
    };
    let result = simple_result("abcdefghij", setup);
    assert!(result.is_line_ellipsized(0));
    let visible_end = result.get_line_end(0, true);
    let full = result.get_bounding_boxes_for_range(0, 10);
    assert!(!full.is_empty());
    assert_eq!(full, result.get_bounding_boxes_for_range(0, visible_end));
    assert!(result.get_bounding_boxes_for_range(visible_end, 10).is_empty());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_multiParagraph_translatesVerticalCoordinatesToGlobal
#[test]
fn a_later_paragraph_s_boxes_are_below_the_ones_above() {
    let text = AnnotatedString::build(|builder| {
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("abc");
        });
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("defg");
        });
    });
    let result = simple_result(text, ResultSetup::default());
    assert_eq!(result.line_count, 2);
    assert_eq!(
        result.get_bounding_boxes_for_range(1, 5),
        [
            Rect::new(10.0, result.get_line_top(0), 30.0, result.get_line_bottom(0)),
            Rect::new(0.0, result.get_line_top(1), 20.0, result.get_line_bottom(1)),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_multiParagraphTruncatedByMaxLines_doesNotCrash
#[test]
fn a_paragraph_the_maximum_lines_left_out_has_no_boxes() {
    let text = AnnotatedString::build(|builder| {
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("first");
        });
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("second");
        });
    });
    let result = simple_result(text, ResultSetup { max_lines: 1, ..ResultSetup::default() });
    assert_eq!(result.line_count, 1);
    assert!(result.get_bounding_boxes_for_range(5, 11).is_empty());
    assert!(result.get_bounding_boxes_for_range(7, 10).is_empty());
    assert_eq!(
        result.get_bounding_boxes_for_range(2, 9),
        result.get_bounding_boxes_for_range(2, 5)
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_startGreaterThanEnd_throwsIllegalArgumentException
#[test]
#[should_panic(expected = "out of range")]
fn boxes_of_a_reversed_range_panic() {
    simple_result("abcdef", ResultSetup::default()).get_bounding_boxes_for_range(4, 2);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, getBoundingBoxesForRange_endGreaterThanLength_throwsIllegalArgumentException
#[test]
#[should_panic(expected = "out of range")]
fn boxes_past_the_text_panic() {
    simple_result("abcdef", ResultSetup::default()).get_bounding_boxes_for_range(0, 7);
}

#[test]
fn an_area_below_the_text_holds_nothing_whatever_the_strategy() {
    let paragraph = paragraph("abc", None);
    let everything = TextInclusionStrategy::new(|_, _| true);
    let below = Rect::new(0.0, 20.0, 30.0, 30.0);
    assert_eq!(paragraph.get_range_for_rect(below, Character, &everything), TextRange::ZERO);
    let above = Rect::new(0.0, -30.0, 30.0, -20.0);
    assert_eq!(paragraph.get_range_for_rect(above, Character, &everything), TextRange::ZERO);
    // Beside the line: no run there.
    let beside = Rect::new(100.0, 0.0, 110.0, 10.0);
    assert_eq!(paragraph.get_range_for_rect(beside, Character, &everything), TextRange::ZERO);
}

#[test]
fn a_word_across_lines_is_taken_whole() {
    // "abcdefgh" broken inside the word, three characters a line.
    let paragraph = paragraph("abcdefgh", Some(3.0 * FONT_SIZE));
    assert_eq!(paragraph.line_count(), 3);
    let first_line = Rect::new(0.0, 0.0, 30.0, 10.0);
    assert_eq!(
        paragraph.get_range_for_rect(first_line, Word, &ContainsCenter),
        TextRange::new(0, 8)
    );
    let last_line = Rect::new(0.0, 20.0, 30.0, 30.0);
    assert_eq!(
        paragraph.get_range_for_rect(last_line, Word, &ContainsCenter),
        TextRange::new(0, 8)
    );
}

#[test]
fn the_characters_in_part_of_a_right_to_left_line() {
    let text = "\u{05D0}\u{05D1}\u{05D2}\u{05D3}";
    let paragraph = paragraph(text, None);
    // Laid out right to left: the second and third letters are at 10..30, their centers in
    // 12..28 (an area's edges inside characters, found by a search).
    let rect = Rect::new(12.0, 0.0, 28.0, 10.0);
    let hebrew = '\u{05D0}'.len_utf8();
    let found = paragraph.get_range_for_rect(rect, Character, &ContainsCenter);
    assert_eq!(found, TextRange::new(hebrew, 3 * hebrew));
}

#[test]
fn an_empty_area_contains_nothing_all_in_it() {
    // The word joiner is zero wide at 40, the line's end.
    let paragraph = paragraph("abcd\u{2060}", None);
    let rect = Rect::new(40.0, 0.0, 40.0, 10.0);
    assert_eq!(paragraph.get_range_for_rect(rect, Character, &ContainsAll), TextRange::ZERO);
}

#[test]
fn boxes_on_a_right_to_left_line_are_left_to_right() {
    // "אב ab ג" right to left: "ab" left of "אב".
    let text = "\u{05D0}\u{05D1} ab \u{05D2}";
    let style =
        TextStyle { text_direction: Some(TextDirection::Rtl), ..ResultSetup::default().style };
    let result =
        simple_result(text, ResultSetup { style, width: Some(100.0), ..ResultSetup::default() });
    let hebrew = '\u{05D0}'.len_utf8();
    // The two letters and the space at 70..100, then "ab" left of them at 50..70: the runs come
    // right to left, sorted before they're joined.
    let boxes = result.get_bounding_boxes_for_range(0, 2 * hebrew + 3);
    assert_eq!(boxes, [Rect::new(50.0, 0.0, 100.0, 10.0)]);
}

#[test]
fn boxes_of_runs_that_touch_are_one() {
    // "abc" then two Hebrew letters: c at 20..30, the letters at 30..50 right to left.
    let text = "abc\u{05D0}\u{05D1}";
    let result = simple_result(text, ResultSetup::default());
    let boxes = result.get_bounding_boxes_for_range(2, text.len());
    assert_eq!(boxes, [Rect::new(20.0, 0.0, 50.0, 10.0)]);
}

#[test]
fn a_zero_wide_range_has_no_box() {
    let result = simple_result("ab\u{2060}cd", ResultSetup::default());
    assert!(result.get_bounding_boxes_for_range(2, 2 + '\u{2060}'.len_utf8()).is_empty());
}

#[test]
fn an_ellipsized_line_s_box_ends_where_its_characters_do() {
    let setup = ResultSetup {
        max_lines: 1,
        overflow: TextOverflow::Ellipsis,
        width: Some(50.0),
        ..ResultSetup::default()
    };
    let result = simple_result("abcdefghij", setup);
    let visible_end = result.get_line_end(0, true);
    let right = 10.0 * visible_end as f32;
    assert_eq!(result.get_bounding_boxes_for_range(0, 10), [Rect::new(0.0, 0.0, right, 10.0)]);
}

#[test]
fn the_boxes_of_characters_the_maximum_lines_cut_are_left_as_they_were() {
    let result = simple_result("ab\ncd", ResultSetup { max_lines: 1, ..ResultSetup::default() });
    let mut array = [-1.0; 20];
    result.multi_paragraph.fill_bounding_boxes(TextRange::new(0, 5), &mut array, 0);
    assert_eq!(&array[..4], &[0.0, 0.0, 10.0, 10.0]);
    assert!(array[12..].iter().all(|&value| value == -1.0), "{array:?}");
}
