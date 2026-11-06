//! A field of one line is as high as its first line's bottom, as Compose's
//! `heightForSingleLineField` (`TextLayoutResult.getLineBottom(0)`, rounded up), with
//! SkParagraph's real fonts and line heights (D2004). A line height trimmed at both ends (a style
//! without a line height style, D571) leaves the last line at its font's bottom: SkParagraph's
//! line metrics keep the whole line height's descent there, 4 px lower at 16 sp in 40 sp lines.

#![allow(non_snake_case)]

use compose::foundation::layout::*;
use compose::foundation::*;
use compose::material3::{TextField, Typography};
use compose::test::*;
use compose::ui::{
    Constraints, Density, IntSize, LineHeightStyle, TextStyle, TextUnit,
    platform::{TextLayoutOptions, TextMeasurer},
};
use compose_skia::SkiaTextMeasurer;

/// `text`'s first line's bottom, as the paragraph's queries (`TextLayoutResult`'s) give it,
/// laid out in `max_lines`: an empty text's one line is the paragraph.
fn line_bottom(text: &str, style: &TextStyle, density: f32, max_lines: u32) -> f32 {
    let mut measurer = SkiaTextMeasurer::new();
    let paragraph = measurer.layout(
        text,
        style,
        TextLayoutOptions { max_lines, ..TextLayoutOptions::default() },
        Constraints::new(0, 10_000, 0, i32::MAX),
        &Density::new(density),
    );
    let queries = measurer.paragraph(paragraph);
    if queries.line_count() == 0 {
        measurer.metrics(paragraph).size.height as f32
    } else {
        queries.get_line_bottom(0)
    }
}

fn height_of(ui: &mut ComposeTestRule, tag: &str) -> f32 {
    let bounds = ui.on_node_with_tag(tag).fetch_semantics_snapshot().bounds;
    bounds.bottom - bounds.top
}

fn skia(density: f32, content: impl Fn() + 'static) -> ComposeTestRule {
    compose_test_with(
        IntSize::new(800, 600),
        Density::new(density),
        Box::new(SkiaTextMeasurer::new()),
        content,
    )
}

/// 16 sp text in 40 sp lines, trimmed at both ends (no line height style: Compose's default).
const TRIMMED: TextStyle = TextStyle {
    font_size: TextUnit::sp(16.0),
    line_height: TextUnit::sp(40.0),
    ..TextStyle::DEFAULT
};

/// A trimmed line's bottom is the paragraph's, not its line height's descent.
#[test]
fn a_trimmed_last_line_ends_at_the_paragraphs_bottom() {
    for density in [1.0, 2.0, 2.625] {
        for (text, max_lines) in [("Hello", u32::MAX), ("a\nb", u32::MAX), ("a\nb", 1)] {
            let mut measurer = SkiaTextMeasurer::new();
            let paragraph = measurer.layout(
                text,
                &TRIMMED,
                TextLayoutOptions { max_lines, ..TextLayoutOptions::default() },
                Constraints::new(0, 10_000, 0, i32::MAX),
                &Density::new(density),
            );
            let height = measurer.metrics(paragraph).size.height as f32;
            let queries = measurer.paragraph(paragraph);
            let last = queries.line_count() - 1;
            assert!(
                (queries.get_line_bottom(last) - height).abs() < 1.0,
                "{text:?} in {max_lines} lines at {density}: {} in {height}",
                queries.get_line_bottom(last)
            );
            assert_eq!(measurer.line_bottom(paragraph, last), queries.get_line_bottom(last));
            // A line before the last keeps its descent: the next starts there.
            if last > 0 {
                assert!((queries.get_line_bottom(0) - queries.get_line_top(1)).abs() < 1.0);
                assert_eq!(measurer.line_bottom(paragraph, 0), queries.get_line_bottom(0));
            }
        }
    }
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/CoreTextField.kt, heightForSingleLineField (maxLines == 1: result.getLineBottom(0).ceilToIntPx())
#[test]
fn a_value_field_of_one_line_is_as_high_as_its_line_bottom() {
    for density in [1.0, 2.0, 2.625] {
        for text in ["Hello", "\nb", ""] {
            let mut ui = skia(density, move || {
                Column!({
                    BasicTextField!(
                        value = text.to_string(),
                        on_value_change = |_| {},
                        text_style = TRIMMED,
                        max_lines = 1,
                        modifier = Modifier.test_tag("max_lines"),
                    );
                    BasicTextField!(
                        value = text.to_string(),
                        on_value_change = |_| {},
                        text_style = TRIMMED,
                        single_line = true,
                        modifier = Modifier.test_tag("single_line"),
                    );
                });
            });
            // A field of `max_lines = 1` lays its text out whole, as Compose's: "\nb"'s first
            // line isn't its last, and keeps its line height's descent.
            let whole = line_bottom(text, &TRIMMED, density, u32::MAX).ceil();
            assert_eq!(height_of(&mut ui, "max_lines"), whole, "{text:?} at {density}");
            // A single line lays out one line (Compose's, its text whole: they differ with a
            // new line in the text, D2004).
            let one = line_bottom(text, &TRIMMED, density, 1).ceil();
            assert_eq!(height_of(&mut ui, "single_line"), one, "{text:?} at {density}");
        }
    }
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/input/internal/TextFieldTextLayoutModifier.kt, heightForSingleLineField (singleLine: result.getLineBottom(0).ceilToIntPx())
#[test]
fn a_state_field_of_a_single_line_is_as_high_as_its_line_bottom() {
    for density in [1.0, 2.625] {
        for text in ["Hello", "\nb"] {
            // A single line shows its new lines as spaces.
            let expected = line_bottom(&text.replace('\n', " "), &TRIMMED, density, 1).ceil();
            let mut ui = skia(density, move || {
                let state = remember_text_field_state(text, None);
                BasicTextFieldOfState!(
                    state,
                    text_style = TRIMMED,
                    line_limits = TextFieldLineLimits::SingleLine,
                    modifier = Modifier.test_tag("field"),
                );
            });
            assert_eq!(height_of(&mut ui, "field"), expected, "{text:?} at {density}");
        }
    }
}

/// Material's fields, without padding and in a font large enough for the text to be taller than
/// the container's least height: the field is its text's line. Material's own style (centered,
/// untrimmed, `body_large`'s proportions at a larger size) and a trimmed one.
// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/CoreTextField.kt, heightForSingleLineField (maxLines == 1: result.getLineBottom(0).ceilToIntPx())
#[test]
fn a_material_field_of_one_line_is_as_high_as_its_line_bottom() {
    let body_large = Typography::default().body_large;
    let material = TextStyle {
        font_size: TextUnit::sp(64.0),
        line_height: TextUnit::sp(96.0),
        ..body_large.clone()
    };
    let trimmed =
        TextStyle { line_height_style: Some(LineHeightStyle::DEFAULT), ..material.clone() };
    for style in [material, trimmed] {
        for text in ["Hello", "\nb"] {
            let shown = style.clone();
            let mut ui = skia(1.0, move || {
                let style = shown.clone();
                Column!({
                    TextField!(
                        value = text.to_string(),
                        on_value_change = |_| {},
                        text_style = style.clone(),
                        max_lines = 1,
                        content_padding = Some(PaddingValues::ZERO),
                        modifier = Modifier.test_tag("max_lines"),
                    );
                    TextField!(
                        value = text.to_string(),
                        on_value_change = |_| {},
                        text_style = style.clone(),
                        single_line = true,
                        content_padding = Some(PaddingValues::ZERO),
                        modifier = Modifier.test_tag("single_line"),
                    );
                });
            });
            let whole = line_bottom(text, &style, 1.0, u32::MAX).ceil();
            let one = line_bottom(text, &style, 1.0, 1).ceil();
            assert!(whole.min(one) > 56.0, "taller than the container's least height");
            assert_eq!(height_of(&mut ui, "max_lines"), whole, "{text:?} in {style:?}");
            assert_eq!(height_of(&mut ui, "single_line"), one, "{text:?} in {style:?}");
        }
    }
}
