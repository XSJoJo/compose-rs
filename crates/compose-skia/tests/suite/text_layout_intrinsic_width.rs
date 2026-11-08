//! Compose's `android/TextLayoutIntrinsicWidthTest` on SkParagraph, in Compose's sample font:
//! a text laid out at its own max intrinsic width stays on one line, with letter spacing (in
//! px or em, on a span or the whole text) and a line height or not, at the repro's density
//! (Pixel 4, Android 11); and the max intrinsic width is the same either way the text goes.
//! Compose's spans on an Android `Spannable` are compose-rs's span styles and text style.

use compose::test::basic_measure_font;
use compose::ui::platform::{TextLayoutOptions, TextMeasurer};
use compose::ui::{
    AnnotatedString, Constraints, Density, LineHeightStyle, SpanStyle, TextDirection, TextStyle,
    TextUnit, UnitExt,
};
use compose_skia::SkiaTextMeasurer;

const TEXT: &str = "This is a callout message";
const DENSITY: Density = Density { density: 3.051, font_scale: 1.15 };

fn style(letter_spacing: TextUnit, line_height: TextUnit) -> TextStyle {
    TextStyle {
        font_size: 12.sp(),
        font_family: Some(basic_measure_font().to_font_family()),
        letter_spacing,
        line_height,
        // As the test's span: both ends trimmed, proportional, fixed.
        line_height_style: Some(LineHeightStyle::DEFAULT),
        ..TextStyle::DEFAULT
    }
}

fn letter_spacing_px() -> TextUnit {
    0.4.sp()
}

fn letter_spacing_em() -> TextUnit {
    (0.4 / 12.0).em()
}

fn line_height() -> TextUnit {
    16.sp()
}

/// The max intrinsic width of `text` in `style`, and its line count laid out at that width.
fn intrinsic_width_and_lines(text: &AnnotatedString, style: &TextStyle) -> (f32, usize) {
    let mut measurer = SkiaTextMeasurer::new();
    let options = TextLayoutOptions::default();
    let free = Constraints::new(0, Constraints::INFINITY, 0, Constraints::INFINITY);
    let handle = measurer.layout_annotated(text, style, options, free, &DENSITY);
    let width = measurer.paragraph(handle).max_intrinsic_width();
    measurer.release(handle);
    let at = width.ceil() as i32;
    let constraints = Constraints::new(0, at, 0, Constraints::INFINITY);
    let handle = measurer.layout_annotated(text, style, options, constraints, &DENSITY);
    let lines = measurer.paragraph(handle).line_count();
    measurer.release(handle);
    (width, lines)
}

#[track_caller]
fn assert_line_count(text: AnnotatedString, style: TextStyle) {
    let (width, lines) = intrinsic_width_and_lines(&text, &style);
    assert_eq!(lines, 1, "laid out at its max intrinsic width {width}");
}

/// `TEXT` with a span of `spacing` over `ranges`.
fn spaced(spacing: TextUnit, ranges: &[(usize, usize)]) -> AnnotatedString {
    AnnotatedString::build(|builder| {
        builder.append(TEXT);
        for (start, end) in ranges {
            let span = SpanStyle { letter_spacing: spacing, ..SpanStyle::DEFAULT };
            builder.add_style(span, *start, *end);
        }
    })
}

fn plain(text: &str) -> AnnotatedString {
    AnnotatedString::build(|builder| {
        builder.append(text);
    })
}

const UNSPECIFIED: TextUnit = TextUnit::UNSPECIFIED;

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_with_letterSpacing_and_lineHeight_createsOneLine
#[test]
fn intrinsic_width_with_letter_spacing_and_line_height_creates_one_line() {
    assert_line_count(
        spaced(letter_spacing_px(), &[(0, TEXT.len())]),
        style(UNSPECIFIED, line_height()),
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_with_letterSpacing_and_lineHeight_createsOneLine_multipleSpans
/// A span a character over the first nine (compose-rs's line height is the paragraph's).
#[test]
fn intrinsic_width_with_letter_spacing_and_line_height_creates_one_line_multiple_spans() {
    let ranges: Vec<(usize, usize)> = (0..=8).map(|i| (i, i + 1)).collect();
    assert_line_count(spaced(letter_spacing_px(), &ranges), style(UNSPECIFIED, line_height()));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_with_letterSpacingEm_and_lineHeight_createsOneLine
#[test]
fn intrinsic_width_with_letter_spacing_em_and_line_height_creates_one_line() {
    assert_line_count(
        spaced(letter_spacing_em(), &[(0, TEXT.len())]),
        style(UNSPECIFIED, line_height()),
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_with_paintLetterSpacing_and_lineHeight_createsOneLine
/// The paint's letter spacing: the text style's.
#[test]
fn intrinsic_width_with_paint_letter_spacing_and_line_height_creates_one_line() {
    assert_line_count(plain(TEXT), style(letter_spacing_em(), line_height()));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_with_letterSpacing_and_noLineHeight_createsOneLine
#[test]
fn intrinsic_width_with_letter_spacing_and_no_line_height_creates_one_line() {
    assert_line_count(
        spaced(letter_spacing_px(), &[(0, TEXT.len())]),
        style(UNSPECIFIED, UNSPECIFIED),
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_with_noLetterSpacing_and_withLineHeight_createsOneLine
#[test]
fn intrinsic_width_with_no_letter_spacing_and_with_line_height_creates_one_line() {
    assert_line_count(plain(TEXT), style(UNSPECIFIED, line_height()));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_with_noLetterSpacing_and_noLineHeight_createsOneLine
#[test]
fn intrinsic_width_with_no_letter_spacing_and_no_line_height_creates_one_line() {
    assert_line_count(plain(TEXT), style(UNSPECIFIED, UNSPECIFIED));
}

/// The max intrinsic width of "asdf" going `direction`.
fn asdf_width(text: &AnnotatedString, direction: TextDirection) -> f32 {
    let style = TextStyle { text_direction: Some(direction), ..style(UNSPECIFIED, UNSPECIFIED) };
    intrinsic_width_and_lines(text, &style).0
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_sameInLtrAndRtl
#[test]
fn intrinsic_width_same_in_ltr_and_rtl() {
    let text = plain("asdf");
    assert_eq!(asdf_width(&text, TextDirection::Ltr), asdf_width(&text, TextDirection::Rtl));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/TextLayoutIntrinsicWidthTest.kt, intrinsicWidth_sameInLtrAndRtl_withLetterSpacing
#[test]
fn intrinsic_width_same_in_ltr_and_rtl_with_letter_spacing() {
    let text = AnnotatedString::build(|builder| {
        builder.append("asdf");
        builder.add_style(
            SpanStyle { letter_spacing: letter_spacing_px(), ..SpanStyle::DEFAULT },
            0,
            4,
        );
    });
    assert_eq!(asdf_width(&text, TextDirection::Ltr), asdf_width(&text, TextDirection::Rtl));
}
