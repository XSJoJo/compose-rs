//! `fill_bounding_boxes` (D2048), adapted from Compose's `ParagraphFillBoundingBoxesTest`,
//! `MultiParagraphFillBoundingBoxesTest` and `TextLayoutFillBoundingBoxesTest` (its internal
//! `TextLayout`'s, through the paragraph; its line spacing multiplier through a line height): laid out by the fixed measurer with Compose's test
//! font metrics (`FixedFontMetrics::SAMPLE_FONT`: one em wide, lines one em high), as Compose's
//! tests lay out with its `sample_font`. Four floats a character, where Compose has four a UTF-16
//! unit: its texts here are all in the basic plane.

use std::rc::Rc;

use compose_ui::platform::{
    FixedFontMetrics, FixedTextMeasurer, SharedTextMeasurer, TextLayoutOptions, TextMeasurer,
};
use compose_ui::{
    AnnotatedRange, AnnotatedString, BaselineShift, Constraints, Density, LineHeightAlignment,
    LineHeightStyle, LineHeightTrim, LocaleList, MultiParagraph, MultiParagraphIntrinsics,
    Paragraph, ParagraphStyle, Placeholder, PlaceholderVerticalAlign, Rect, SpanStyle, TextAlign,
    TextDirection, TextGeometricTransform, TextIndent, TextOverflow, TextRange, TextStyle, UnitExt,
    create_font_family_resolver,
};

const SIZE: f32 = 10.0;

/// `text` at 10 sp in `style`, `width` wide (its own width when `None`), going by its first
/// strong character: Compose's test's `Paragraph`.
fn laid_out(
    text: AnnotatedString,
    style: TextStyle,
    width: Option<f32>,
    placeholders: &[AnnotatedRange<Placeholder>],
) -> Rc<dyn Paragraph> {
    let mut measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let constraints = match width {
        Some(width) => Constraints::new(width as i32, width as i32, 0, Constraints::INFINITY),
        None => Constraints::unbounded(),
    };
    let style = TextStyle {
        font_size: SIZE.sp(),
        text_direction: Some(TextDirection::Content),
        ..TextStyle::DEFAULT
    }
    .merge(&style);
    let (options, density) = (TextLayoutOptions::default(), Density::new(1.0));
    let handle = if placeholders.is_empty() {
        measurer.layout_annotated(&text, &style, options, constraints, &density)
    } else {
        measurer.layout_with_placeholders(
            &text,
            &style,
            placeholders,
            options,
            constraints,
            &density,
        )
    };
    measurer.paragraph(handle)
}

fn plain(text: &str) -> Rc<dyn Paragraph> {
    laid_out(AnnotatedString::new(text.to_string()), TextStyle::DEFAULT, None, &[])
}

fn styled(text: &str, style: TextStyle, width: Option<f32>) -> Rc<dyn Paragraph> {
    laid_out(AnnotatedString::new(text.to_string()), style, width, &[])
}

/// Every character's box of `range`.
fn boxes(paragraph: &dyn Paragraph, range: TextRange) -> Vec<Rect> {
    let count = paragraph.text()[range.min()..range.max()].chars().count();
    let mut array = vec![0.0; count * 4];
    paragraph.fill_bounding_boxes(range, &mut array, 0);
    array.chunks(4).map(|b| Rect::new(b[0], b[1], b[2], b[3])).collect()
}

fn all_boxes(paragraph: &dyn Paragraph) -> Vec<Rect> {
    boxes(paragraph, TextRange::new(0, paragraph.text().len()))
}

/// Compose's `getLtrCharacterBoundariesForTestFont`: one box a character from the left, a line
/// feed zero wide at its line's end.
fn ltr_boxes(text: &str, size: f32, line_height: f32) -> Vec<Rect> {
    let (mut top, mut left) = (0.0, 0.0);
    text.chars()
        .map(|c| {
            let right = if c == '\n' { left } else { left + size };
            let rect = Rect::new(left, top, right, top + line_height);
            if c == '\n' {
                left = 0.0;
                top += line_height;
            } else {
                left = right;
            }
            rect
        })
        .collect()
}

/// Compose's `getRtlCharacterBoundariesForTestFont`: from the right of `width`.
fn rtl_boxes(text: &str, width: f32, size: f32) -> Vec<Rect> {
    let (mut top, mut right) = (0.0, width);
    text.chars()
        .map(|c| {
            let left = if c == '\n' { right } else { right - size };
            let rect = Rect::new(left, top, right, top + size);
            if c == '\n' {
                right = width;
                top += size;
            } else {
                right = left;
            }
            rect
        })
        .collect()
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, startEqualToLength
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, negativeStart
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, startEqualToLength
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, negativeStart
#[test]
#[should_panic(expected = "startOffset must be less than text length")]
fn a_start_at_the_text_s_end_panics() {
    boxes(&*plain("a"), TextRange::new(1, 1));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, endGreaterThanLength
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, endGreaterThanLength
#[test]
#[should_panic(expected = "endOffset must be smaller or equal to text length")]
fn an_end_past_the_text_panics() {
    plain("a").fill_bounding_boxes(TextRange::new(0, 2), &mut [0.0; 8], 0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, endEqualToStart
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, endEqualToStart
#[test]
#[should_panic(expected = "endOffset must be greater than startOffset")]
fn an_empty_range_panics() {
    boxes(&*plain("a"), TextRange::new(0, 0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, arraySizeSmallerThanTextLength
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, arraySizeSmallerThanTextLength
#[test]
#[should_panic(expected = "array.size - arrayStart must be greater or equal")]
fn an_array_smaller_than_the_text_panics() {
    plain("abc").fill_bounding_boxes(TextRange::new(0, 3), &mut [0.0; 11], 0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, arraySizeSmallerThanRange
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, arraySizeSmallerThanRange
#[test]
#[should_panic(expected = "array.size - arrayStart must be greater or equal")]
fn an_array_smaller_than_the_range_panics() {
    plain("abc").fill_bounding_boxes(TextRange::new(1, 3), &mut [0.0; 7], 0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, arraySizeSmallerThanTextLengthWithStart
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, arraySizeSmallerThanTextLengthWithStart
#[test]
#[should_panic(expected = "array.size - arrayStart must be greater or equal")]
fn an_array_too_small_after_its_start_panics() {
    plain("abc").fill_bounding_boxes(TextRange::new(0, 3), &mut [0.0; 24], 13);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, singleCharacter
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, singleCharacter
#[test]
fn a_character_s_box() {
    assert_eq!(all_boxes(&*plain("a")), ltr_boxes("a", SIZE, SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, arrayFillStartsFromStartOffsetEndsAtEndOffset
#[test]
fn the_boxes_fill_the_array_from_its_start_only() {
    let paragraph = plain("abc");
    let section = 3 * 4;
    let mut array = vec![f32::MIN_POSITIVE; section * 3];
    paragraph.fill_bounding_boxes(TextRange::new(0, 3), &mut array, section);
    assert!(array[..section].iter().all(|&value| value == f32::MIN_POSITIVE));
    assert!(array[section..2 * section].iter().all(|&value| value != f32::MIN_POSITIVE));
    assert!(array[2 * section..].iter().all(|&value| value == f32::MIN_POSITIVE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, overridesArray
#[test]
fn the_boxes_replace_what_the_array_held() {
    let range = TextRange::new(0, 3);
    let mut array = [0.0; 12];
    styled("abc", TextStyle::DEFAULT, None).fill_bounding_boxes(range, &mut array, 0);
    let rects: Vec<Rect> = array.chunks(4).map(|b| Rect::new(b[0], b[1], b[2], b[3])).collect();
    assert_eq!(rects, ltr_boxes("abc", SIZE, SIZE));
    let double = TextStyle { font_size: (2.0 * SIZE).sp(), ..TextStyle::DEFAULT };
    styled("abc", double, None).fill_bounding_boxes(range, &mut array, 0);
    let rects: Vec<Rect> = array.chunks(4).map(|b| Rect::new(b[0], b[1], b[2], b[3])).collect();
    assert_eq!(rects, ltr_boxes("abc", 2.0 * SIZE, 2.0 * SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, singleCharacterLineHeight
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, singleCharacterLineHeight
#[test]
fn a_single_line_s_line_height_is_trimmed() {
    let style = TextStyle { line_height: (2.0 * SIZE).sp(), ..TextStyle::DEFAULT };
    assert_eq!(all_boxes(&*styled("a", style, None)), ltr_boxes("a", SIZE, SIZE));
}

fn untrimmed(line_height: f32) -> TextStyle {
    TextStyle {
        line_height: line_height.sp(),
        line_height_style: Some(LineHeightStyle {
            alignment: LineHeightAlignment::PROPORTIONAL,
            trim: LineHeightTrim::None,
            ..LineHeightStyle::DEFAULT
        }),
        ..TextStyle::DEFAULT
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, singleCharacterLineHeight_includeFontPaddingIsFalse
#[test]
fn an_untrimmed_line_height_is_the_box_s() {
    let boxes = all_boxes(&*styled("a", untrimmed(2.0 * SIZE), None));
    assert_eq!(boxes, ltr_boxes("a", SIZE, 2.0 * SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, multiLineCharacterLineHeight
#[test]
fn every_line_s_boxes_are_its_line_height() {
    let boxes = all_boxes(&*styled("a\na\na", untrimmed(2.0 * SIZE), None));
    assert_eq!(boxes, ltr_boxes("a\na\na", SIZE, 2.0 * SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, singleCharacterRtl
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, singleCharacterRtl
#[test]
fn a_right_to_left_character_s_box() {
    let width = 2.0 * SIZE;
    let boxes = all_boxes(&*styled("\u{05D0}", TextStyle::DEFAULT, Some(width)));
    assert_eq!(boxes, rtl_boxes("\u{05D0}", width, SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, singleLineLtr
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, singleLineLtr
#[test]
fn a_left_to_right_line_s_boxes() {
    assert_eq!(all_boxes(&*plain("abc")), ltr_boxes("abc", SIZE, SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, singleLineRtl
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, singleLineRtl
#[test]
fn a_right_to_left_line_s_boxes() {
    let text = "\u{05D0}\u{05D1}\u{05D2}";
    let width = 6.0 * SIZE;
    assert_eq!(
        all_boxes(&*styled(text, TextStyle::DEFAULT, Some(width))),
        rtl_boxes(text, width, SIZE)
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, bidiLtrLine
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, bidiLtrLine
#[test]
fn a_left_to_right_line_s_boxes_with_a_right_to_left_run() {
    let text = "a\u{05D0}\u{05D1}b";
    let expected = ltr_boxes(text, SIZE, SIZE);
    // Laid out as 0 2 1 3.
    let shown = vec![expected[0], expected[2], expected[1], expected[3]];
    assert_eq!(all_boxes(&*plain(text)), shown);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, bidiRtlLine
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, bidiRtlLine
#[test]
fn a_right_to_left_line_s_boxes_with_a_left_to_right_run() {
    let text = "\u{05D0}ab\u{05D1}";
    let width = 8.0 * SIZE;
    let expected = rtl_boxes(text, width, SIZE);
    // Laid out as 3 1 2 0.
    let shown = vec![expected[0], expected[2], expected[1], expected[3]];
    assert_eq!(all_boxes(&*styled(text, TextStyle::DEFAULT, Some(width))), shown);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, multiLineLtr
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, multiLineLtr
#[test]
fn left_to_right_lines_boxes() {
    assert_eq!(all_boxes(&*plain("a\nb\nc")), ltr_boxes("a\nb\nc", SIZE, SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, multiLineRtl
#[test]
fn right_to_left_lines_boxes() {
    let text = "\u{05D0}\n\u{05D1}\n\u{05D2}";
    let width = 3.0 * SIZE;
    assert_eq!(
        all_boxes(&*styled(text, TextStyle::DEFAULT, Some(width))),
        rtl_boxes(text, width, SIZE)
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, multiLineRtl
#[test]
fn right_to_left_lines_boxes_as_wide_as_two() {
    let text = "\u{05D0}\n\u{05D1}\n\u{05D2}";
    let width = 2.0 * SIZE;
    assert_eq!(
        all_boxes(&*styled(text, TextStyle::DEFAULT, Some(width))),
        rtl_boxes(text, width, SIZE)
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, multiLineCenterAligned
#[test]
fn centered_lines_boxes() {
    let style = TextStyle { text_align: Some(TextAlign::Center), ..TextStyle::DEFAULT };
    let boxes = all_boxes(&*styled("a\nb\nc", style, Some(3.0 * SIZE)));
    // Each letter in the middle third, its line feed zero wide at its right.
    let (left, right) = (SIZE, 2.0 * SIZE);
    assert_eq!(
        boxes,
        [
            Rect::new(left, 0.0, right, SIZE),
            Rect::new(right, 0.0, right, SIZE),
            Rect::new(left, SIZE, right, 2.0 * SIZE),
            Rect::new(right, SIZE, right, 2.0 * SIZE),
            Rect::new(left, 2.0 * SIZE, right, 3.0 * SIZE),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, multiLineRtlCenterAligned
#[test]
fn centered_right_to_left_lines_boxes() {
    let style = TextStyle { text_align: Some(TextAlign::Center), ..TextStyle::DEFAULT };
    let text = "\u{05D0}\n\u{05D1}\n\u{05D2}";
    let boxes = all_boxes(&*styled(text, style, Some(3.0 * SIZE)));
    // Each letter in the middle third, its line feed zero wide at its left.
    let (left, right) = (SIZE, 2.0 * SIZE);
    assert_eq!(
        boxes,
        [
            Rect::new(left, 0.0, right, SIZE),
            Rect::new(left, 0.0, left, SIZE),
            Rect::new(left, SIZE, right, 2.0 * SIZE),
            Rect::new(left, SIZE, left, 2.0 * SIZE),
            Rect::new(left, 2.0 * SIZE, right, 3.0 * SIZE),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, zwjEmoji
#[test]
fn a_cluster_s_later_characters_are_zero_wide_at_its_first_s_right() {
    // Compose's is an emoji of four joined by zero width joiners, one glyph in Android's emoji
    // font; the fixed measurer's font has no such glyph and draws each emoji. A letter with two
    // combining marks is one cluster in any font.
    let text = "e\u{0301}\u{0323}";
    let boxes = all_boxes(&*plain(text));
    assert_eq!(boxes.len(), 3);
    let first = boxes[0];
    assert!(first.width() > 0.0, "{first:?}");
    for rect in &boxes[1..] {
        assert_eq!(*rect, Rect::new(first.right, first.top, first.right, first.bottom));
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, withIndent
#[test]
fn indented_lines_boxes() {
    let indent = TextIndent { first_line: (2.0 * SIZE).sp(), rest_line: SIZE.sp() };
    let style = TextStyle { text_indent: Some(indent), ..TextStyle::DEFAULT };
    let boxes = all_boxes(&*styled("abcd\ne", style, Some(3.0 * SIZE)));
    let (first, rest) = (2.0 * SIZE, SIZE);
    // __a / _bc / _d\n / __e.
    assert_eq!(
        boxes,
        [
            Rect::new(first, 0.0, first + SIZE, SIZE),
            Rect::new(rest, SIZE, rest + SIZE, 2.0 * SIZE),
            Rect::new(rest + SIZE, SIZE, rest + 2.0 * SIZE, 2.0 * SIZE),
            Rect::new(rest, 2.0 * SIZE, rest + SIZE, 3.0 * SIZE),
            Rect::new(rest + SIZE, 2.0 * SIZE, rest + SIZE, 3.0 * SIZE),
            Rect::new(first, 3.0 * SIZE, first + SIZE, 4.0 * SIZE),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, variableFontSize
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutFillBoundingBoxesTest.kt, withStyling
#[test]
fn a_larger_span_s_box_is_wider_and_the_line_taller() {
    let text = AnnotatedString::build(|builder| {
        builder.append("a");
        builder.with_style(
            SpanStyle { font_size: (2.0 * SIZE).sp(), ..SpanStyle::DEFAULT },
            |builder| {
                builder.append("b");
            },
        );
        builder.append("c");
    });
    let double = 2.0 * SIZE;
    assert_eq!(
        all_boxes(&*laid_out(text, TextStyle::DEFAULT, None, &[])),
        [
            Rect::new(0.0, 0.0, SIZE, double),
            Rect::new(SIZE, 0.0, 3.0 * SIZE, double),
            Rect::new(3.0 * SIZE, 0.0, 4.0 * SIZE, double),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, letterSpacing
#[test]
fn letter_spacing_widens_each_box() {
    let style = TextStyle { letter_spacing: 1.em(), ..TextStyle::DEFAULT };
    // Without Android's fix for the spacing at a line's edges (the fixed measurer's).
    assert_eq!(
        all_boxes(&*styled("abc\nde", style, None)),
        [
            Rect::new(0.0, 0.0, 2.0 * SIZE, SIZE),
            Rect::new(2.0 * SIZE, 0.0, 4.0 * SIZE, SIZE),
            Rect::new(4.0 * SIZE, 0.0, 6.0 * SIZE, SIZE),
            Rect::new(6.0 * SIZE, 0.0, 6.0 * SIZE, SIZE),
            Rect::new(0.0, SIZE, 2.0 * SIZE, 2.0 * SIZE),
            Rect::new(2.0 * SIZE, SIZE, 4.0 * SIZE, 2.0 * SIZE),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, textAlignCenter
#[test]
fn centered_boxes() {
    let style = TextStyle { text_align: Some(TextAlign::Center), ..TextStyle::DEFAULT };
    assert_eq!(
        all_boxes(&*styled("ab", style, Some(4.0 * SIZE))),
        [Rect::new(SIZE, 0.0, 2.0 * SIZE, SIZE), Rect::new(2.0 * SIZE, 0.0, 3.0 * SIZE, SIZE)]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, textAlignEnd
#[test]
fn end_aligned_boxes() {
    let style = TextStyle { text_align: Some(TextAlign::End), ..TextStyle::DEFAULT };
    assert_eq!(
        all_boxes(&*styled("ab", style, Some(4.0 * SIZE))),
        [
            Rect::new(2.0 * SIZE, 0.0, 3.0 * SIZE, SIZE),
            Rect::new(3.0 * SIZE, 0.0, 4.0 * SIZE, SIZE)
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, withTextGeometricTransformScaleX
#[test]
fn a_horizontal_scale_widens_each_box() {
    let style = TextStyle {
        text_geometric_transform: Some(TextGeometricTransform { scale_x: 2.0, skew_x: 0.0 }),
        ..TextStyle::DEFAULT
    };
    assert_eq!(
        all_boxes(&*styled("ab", style, None)),
        [Rect::new(0.0, 0.0, 2.0 * SIZE, SIZE), Rect::new(2.0 * SIZE, 0.0, 4.0 * SIZE, SIZE)]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, textGeometricTransformSkewX
#[test]
fn a_skew_leaves_the_boxes() {
    let style = TextStyle {
        text_geometric_transform: Some(TextGeometricTransform { scale_x: 1.0, skew_x: -1.0 }),
        ..TextStyle::DEFAULT
    };
    assert_eq!(all_boxes(&*styled("ab", style, None)), ltr_boxes("ab", SIZE, SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, baselineShift
#[test]
fn a_shifted_character_s_box_spans_its_line() {
    let half = SIZE / 2.0;
    let text = AnnotatedString::build(|builder| {
        builder.append("a");
        let superscript = SpanStyle {
            baseline_shift: Some(BaselineShift::SUPERSCRIPT),
            font_size: half.sp(),
            ..SpanStyle::DEFAULT
        };
        builder.with_style(superscript, |builder| {
            builder.append("b");
        });
        builder.append("c");
    });
    let paragraph = laid_out(text, TextStyle::DEFAULT, Some(6.0 * SIZE), &[]);
    let (start, end) = (SIZE, SIZE + half);
    assert_eq!(
        all_boxes(&*paragraph),
        [
            Rect::new(0.0, 0.0, SIZE, SIZE),
            Rect::new(start, 0.0, end, SIZE),
            Rect::new(end, 0.0, end + SIZE, SIZE),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphFillBoundingBoxesTest.kt, inlineElement
#[test]
fn a_placeholder_s_box_is_its_width_and_the_line_its_height() {
    let double = 2.0 * SIZE;
    let placeholder = Placeholder::new(double.sp(), double.sp(), PlaceholderVerticalAlign::Top);
    let paragraph = laid_out(
        AnnotatedString::new("abc"),
        TextStyle::DEFAULT,
        None,
        &[AnnotatedRange::new(placeholder, 1, 2)],
    );
    assert_eq!(
        all_boxes(&*paragraph),
        [
            Rect::new(0.0, 0.0, SIZE, double),
            Rect::new(SIZE, 0.0, 3.0 * SIZE, double),
            Rect::new(3.0 * SIZE, 0.0, 4.0 * SIZE, double),
        ]
    );
}

/// Each string a paragraph, at 10 sp, `width` wide (unbounded when `None`).
fn multi(texts: &[&str], width: Option<f32>) -> MultiParagraph {
    let text = AnnotatedString::build(|builder| {
        for text in texts {
            builder.push_paragraph_style(ParagraphStyle::DEFAULT);
            builder.append(text);
            builder.pop();
        }
    });
    let resolver = create_font_family_resolver(&SharedTextMeasurer::new(Box::new(
        FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT),
    )));
    let style = TextStyle { font_size: SIZE.sp(), ..TextStyle::DEFAULT };
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
    MultiParagraph::new(Rc::new(intrinsics), constraints, u32::MAX, TextOverflow::Clip)
}

/// `other` below `boxes`' last, appended: Compose's `offsetVerticalAndAppend`.
fn and_below(mut boxes: Vec<Rect>, other: Vec<Rect>) -> Vec<Rect> {
    let below = boxes.last().unwrap().bottom;
    boxes.extend(other.into_iter().map(|rect| rect.translate_xy(0.0, below)));
    boxes
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphFillBoundingBoxesTest.kt, singleParagraphLtr
#[test]
fn one_left_to_right_paragraph_s_boxes() {
    let paragraph = multi(&["ab"], None);
    assert_eq!(all_boxes(&paragraph), ltr_boxes("ab", SIZE, SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphFillBoundingBoxesTest.kt, singleParagraphRtl
#[test]
fn one_right_to_left_paragraph_s_boxes() {
    let width = 3.0 * SIZE;
    let paragraph = multi(&["\u{05D0}\u{05D1}"], Some(width));
    assert_eq!(all_boxes(&paragraph), rtl_boxes("\u{05D0}\u{05D1}", width, SIZE));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphFillBoundingBoxesTest.kt, ltr
#[test]
fn left_to_right_paragraphs_boxes() {
    let expected = and_below(ltr_boxes("a", SIZE, SIZE), ltr_boxes("b", SIZE, SIZE));
    assert_eq!(all_boxes(&multi(&["a", "b"], None)), expected);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphFillBoundingBoxesTest.kt, rtl
#[test]
fn right_to_left_paragraphs_boxes() {
    let width = 3.0 * SIZE;
    let expected =
        and_below(rtl_boxes("\u{05D0}", width, SIZE), rtl_boxes("\u{05D1}", width, SIZE));
    assert_eq!(all_boxes(&multi(&["\u{05D0}", "\u{05D1}"], Some(width))), expected);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphFillBoundingBoxesTest.kt, multiLineLtr
#[test]
fn left_to_right_paragraphs_of_lines_boxes() {
    let expected = and_below(ltr_boxes("a\nb", SIZE, SIZE), ltr_boxes("c\nd", SIZE, SIZE));
    assert_eq!(all_boxes(&multi(&["a\nb", "c\nd"], None)), expected);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphFillBoundingBoxesTest.kt, multiLineRtl
#[test]
fn right_to_left_paragraphs_of_lines_boxes() {
    let width = 3.0 * SIZE;
    let (first, second) = ("\u{05D0}\n\u{05D1}", "\u{05D2}\n\u{05D3}");
    let expected = and_below(rtl_boxes(first, width, SIZE), rtl_boxes(second, width, SIZE));
    assert_eq!(all_boxes(&multi(&[first, second], Some(width))), expected);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphFillBoundingBoxesTest.kt, ltrAndRtlParagraphs
#[test]
fn paragraphs_of_both_directions_boxes() {
    let width = 3.0 * SIZE;
    let (first, second) = ("a\nb", "\u{05D0}\n\u{05D1}");
    let expected = and_below(ltr_boxes(first, SIZE, SIZE), rtl_boxes(second, width, SIZE));
    assert_eq!(all_boxes(&multi(&[first, second], Some(width))), expected);
}

#[test]
fn a_range_from_a_later_paragraph_is_below_the_ones_above() {
    // Compose's `MultiParagraph.fillBoundingBoxes` counts heights from the range's first
    // paragraph, so its boxes would be at the top; they're where the characters are (D2048).
    let paragraph = multi(&["a", "b", "c"], None);
    assert_eq!(
        boxes(&paragraph, TextRange::new(1, 3)),
        [Rect::new(0.0, SIZE, SIZE, 2.0 * SIZE), Rect::new(0.0, 2.0 * SIZE, SIZE, 3.0 * SIZE)]
    );
}
