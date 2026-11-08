//! Compose's `platform/SpannableExtensionsTest`'s span flattening (`flattenFontStylesAndApply`),
//! through `AnnotatedString::runs`, compose-rs's flattening: Compose's calls its block with each
//! piece between two span ends that a span covers (or every piece, with a context style),
//! merged over the context style; compose-rs's runs cover the whole text, each piece in the base
//! style with the spans covering it merged in their order, so the pieces no span covers are
//! there too, in the base style. The texts are ASCII: offsets are Compose's.

use std::ops::Range;

use compose_ui::{AnnotatedString, Color, FontStyle, FontWeight, SpanStyle, TextStyle, UnitExt};

const TEXT: &str = "abcdefghijklmnopqrst";
const END: usize = TEXT.len();

fn span(start: usize, end: usize, style: SpanStyle) -> (SpanStyle, usize, usize) {
    (style, start, end)
}

/// The runs of `TEXT` with `spans` (in their order), over `base`: each piece and its style.
fn flatten(
    base: &TextStyle,
    spans: &[(SpanStyle, usize, usize)],
) -> Vec<(Range<usize>, TextStyle)> {
    let text = AnnotatedString::build(|builder| {
        builder.append(TEXT);
        for (style, start, end) in spans {
            builder.add_style(style.clone(), *start, *end);
        }
    });
    text.runs(base).into_iter().map(|(range, style, _)| (range, style)).collect()
}

fn weight(weight: u16) -> SpanStyle {
    SpanStyle { font_weight: Some(FontWeight::new(weight)), ..SpanStyle::DEFAULT }
}

fn italic() -> SpanStyle {
    SpanStyle { font_style: Some(FontStyle::Italic), ..SpanStyle::DEFAULT }
}

/// The default text style with `weight` and `italic` set.
fn styled(weight: Option<u16>, italic: bool) -> TextStyle {
    TextStyle {
        font_weight: weight.map(FontWeight::new),
        font_style: italic.then_some(FontStyle::Italic),
        ..TextStyle::DEFAULT
    }
}

fn plain() -> TextStyle {
    TextStyle::DEFAULT
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_emptyList
#[test]
fn flatten_styles_and_apply_empty_list() {
    assert_eq!(flatten(&plain(), &[]), [(0..END, plain())]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_oneStyle
#[test]
fn flatten_styles_and_apply_one_style() {
    assert_eq!(
        flatten(&plain(), &[span(4, 10, weight(123))]),
        [(0..4, plain()), (4..10, styled(Some(123), false)), (10..END, plain())]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_containedByOldStyle
#[test]
fn flatten_styles_and_apply_contained_by_old_style() {
    assert_eq!(
        flatten(&plain(), &[span(3, 10, weight(123)), span(4, 6, italic())]),
        [
            (0..3, plain()),
            (3..4, styled(Some(123), false)),
            (4..6, styled(Some(123), true)),
            (6..10, styled(Some(123), false)),
            (10..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_containedByOldStyle_sharedStart
#[test]
fn flatten_styles_and_apply_contained_by_old_style_shared_start() {
    assert_eq!(
        flatten(&plain(), &[span(3, 10, weight(123)), span(3, 6, italic())]),
        [
            (0..3, plain()),
            (3..6, styled(Some(123), true)),
            (6..10, styled(Some(123), false)),
            (10..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_containedByOldStyle_sharedEnd
#[test]
fn flatten_styles_and_apply_contained_by_old_style_shared_end() {
    assert_eq!(
        flatten(&plain(), &[span(3, 10, weight(123)), span(5, 10, italic())]),
        [
            (0..3, plain()),
            (3..5, styled(Some(123), false)),
            (5..10, styled(Some(123), true)),
            (10..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_sameRange
#[test]
fn flatten_styles_and_apply_same_range() {
    assert_eq!(
        flatten(&plain(), &[span(3, 10, weight(123)), span(3, 10, italic())]),
        [(0..3, plain()), (3..10, styled(Some(123), true)), (10..END, plain())]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_overlappingStyles
#[test]
fn flatten_styles_and_apply_overlapping_styles() {
    assert_eq!(
        flatten(&plain(), &[span(3, 10, weight(123)), span(6, 19, italic())]),
        [
            (0..3, plain()),
            (3..6, styled(Some(123), false)),
            (6..10, styled(Some(123), true)),
            (10..19, styled(None, true)),
            (19..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_notIntersectedStyles
#[test]
fn flatten_styles_and_apply_not_intersected_styles() {
    assert_eq!(
        flatten(&plain(), &[span(3, 4, weight(123)), span(8, 10, italic())]),
        [
            (0..3, plain()),
            (3..4, styled(Some(123), false)),
            (4..8, plain()),
            (8..10, styled(None, true)),
            (10..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_containedByOldStyle_appliedInOrder
#[test]
fn flatten_styles_and_apply_contained_by_old_style_applied_in_order() {
    // The second overrides the first's weight in [5, 9).
    assert_eq!(
        flatten(&plain(), &[span(3, 10, weight(123)), span(5, 9, weight(200))]),
        [
            (0..3, plain()),
            (3..5, styled(Some(123), false)),
            (5..9, styled(Some(200), false)),
            (9..10, styled(Some(123), false)),
            (10..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_containsOldStyle_appliedInOrder
#[test]
fn flatten_styles_and_apply_contains_old_style_applied_in_order() {
    // The second, applied after, overrides the first where they meet: three pieces of it.
    assert_eq!(
        flatten(&plain(), &[span(5, 7, weight(123)), span(3, 10, weight(200))]),
        [
            (0..3, plain()),
            (3..5, styled(Some(200), false)),
            (5..7, styled(Some(200), false)),
            (7..10, styled(Some(200), false)),
            (10..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_notIntersected_appliedInIndexOrder
#[test]
fn flatten_styles_and_apply_not_intersected_applied_in_index_order() {
    // Given last to first, the pieces come in the text's order.
    assert_eq!(
        flatten(
            &plain(),
            &[span(7, 8, weight(300)), span(3, 4, weight(200)), span(1, 2, weight(100))]
        ),
        [
            (0..1, plain()),
            (1..2, styled(Some(100), false)),
            (2..3, plain()),
            (3..4, styled(Some(200), false)),
            (4..7, plain()),
            (7..8, styled(Some(300), false)),
            (8..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_intersected_appliedInIndexOrder
#[test]
fn flatten_styles_and_apply_intersected_applied_in_index_order() {
    // Applied in the list's order: the second's weight wins where they meet.
    assert_eq!(
        flatten(&plain(), &[span(5, 9, weight(100)), span(3, 6, weight(200))]),
        [
            (0..3, plain()),
            (3..5, styled(Some(200), false)),
            (5..6, styled(Some(200), false)),
            (6..9, styled(Some(100), false)),
            (9..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_allEmptyRanges_notApplied
/// Empty spans style nothing: every piece is in the context's style.
#[test]
fn flatten_styles_and_apply_all_empty_ranges_not_applied() {
    let context = styled(Some(400), false);
    let runs = flatten(
        &context,
        &[span(2, 2, weight(100)), span(4, 4, weight(200)), span(0, 0, weight(300))],
    );
    assert!(runs.iter().all(|(_, style)| *style == context), "{runs:?}");
    let ranges: Vec<Range<usize>> = runs.into_iter().map(|(range, _)| range).collect();
    assert_eq!(ranges.first().map(|range| range.start), Some(0));
    assert_eq!(ranges.last().map(|range| range.end), Some(END));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_emptySpanRange_shouldNotApply
#[test]
fn flatten_styles_and_apply_empty_span_range_should_not_apply() {
    assert_eq!(
        flatten(
            &plain(),
            &[span(4, 10, weight(200)), span(1, 7, italic()), span(3, 3, weight(100))]
        ),
        [
            (0..1, plain()),
            (1..3, styled(None, true)),
            (3..4, styled(None, true)),
            (4..7, styled(Some(200), true)),
            (7..10, styled(Some(200), false)),
            (10..END, plain()),
        ]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_emptySpanRangeBeginning_shouldNotApply
#[test]
fn flatten_styles_and_apply_empty_span_range_beginning_should_not_apply() {
    assert_eq!(
        flatten(&plain(), &[span(0, 0, weight(100)), span(0, 7, italic())]),
        [(0..7, styled(None, true)), (7..END, plain())]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_withContextSpanStyle_inheritContext
#[test]
fn flatten_styles_and_apply_with_context_span_style_inherit_context() {
    let context =
        TextStyle { color: Color::RED, font_style: Some(FontStyle::Italic), ..TextStyle::DEFAULT };
    let runs = flatten(&context, &[span(3, 6, weight(200))]);
    assert_eq!(
        runs[1],
        (
            3..6,
            TextStyle {
                color: Color::RED,
                font_style: Some(FontStyle::Italic),
                font_weight: Some(FontWeight::new(200)),
                ..TextStyle::DEFAULT
            }
        )
    );
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert_eq!((runs[0].1.clone(), runs[2].1.clone()), (context.clone(), context));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/platform/SpannableExtensionsTest.kt, flattenStylesAndApply_withContextSpanStyle_multipleSpanStyles_inheritContext
#[test]
fn flatten_styles_and_apply_with_context_span_style_multiple_span_styles_inherit_context() {
    let context = TextStyle {
        color: Color::RED,
        font_weight: Some(FontWeight::LIGHT),
        font_style: Some(FontStyle::Normal),
        font_size: 18.sp(),
        ..TextStyle::DEFAULT
    };
    let bold = SpanStyle { font_weight: Some(FontWeight::BOLD), ..SpanStyle::DEFAULT };
    let big = SpanStyle { font_size: 24.sp(), ..SpanStyle::DEFAULT };
    let with = |weight: FontWeight, style: FontStyle, size: f32| TextStyle {
        font_weight: Some(weight),
        font_style: Some(style),
        font_size: size.sp(),
        ..context.clone()
    };
    let (light, bold_weight) = (FontWeight::LIGHT, FontWeight::BOLD);
    let (normal, italic_style) = (FontStyle::Normal, FontStyle::Italic);
    // Five pieces: each the context with what covers it.
    assert_eq!(
        flatten(&context, &[span(2, 8, bold), span(4, 10, italic()), span(6, 12, big)]),
        [
            (0..2, context.clone()),
            (2..4, with(bold_weight, normal, 18.0)),
            (4..6, with(bold_weight, italic_style, 18.0)),
            (6..8, with(bold_weight, italic_style, 24.0)),
            (8..10, with(light, italic_style, 24.0)),
            (10..12, with(light, normal, 24.0)),
            (12..END, context.clone()),
        ]
    );
}
