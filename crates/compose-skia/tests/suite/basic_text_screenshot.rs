//! Compose's `BasicTextScreenshotTest` (foundation, device tests), its goldens rewritten as
//! raster checks drawn by Skia: a centered ellipsized line's leading margin doesn't move as the
//! layout around it is laid out again (b/389707025: the same pixels before and after), and a span
//! changing the weight, family or style of "World" in "Hello World" draws "World" otherwise than
//! the text without the span, "Hello" the same.
//!
//! The fonts are the platform's (Skia's system fonts), as the goldens' were the device's.

use compose::foundation::layout::*;
use compose::foundation::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Alignment, AnnotatedString, Color, Density, FontFamily, FontStyle, FontWeight, IntSize,
    LineHeightAlignment, LineHeightMode, LineHeightStyle, LineHeightTrim, Modifier, PaddingValues,
    SpanStyle, TextAlign, TextOverflow, UnitExt,
};
use compose_skia::SkiaTextMeasurer;

const TEXT_TAG: &str = "text";

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    let mut ui = compose_test_with(
        IntSize::new(400, 400),
        Density::new(1.0),
        std::boxed::Box::new(SkiaTextMeasurer::new()),
        content,
    );
    ui.wait_for_idle();
    ui
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextScreenshotTest.kt, basicTextEllipsisCentered_leadingMarginCorrect_doesntMarch_b389707025
#[test]
fn basic_text_ellipsis_centered_leading_margin_correct_doesnt_march_b389707025() {
    let padding = mutable_state_of(0.0f32);
    let top = padding.handle();
    let mut ui = rule(move || {
        Box!(
            modifier = Modifier
                .padding(PaddingValues { top: top.get().dp(), ..PaddingValues::ZERO })
                .fill_max_size()
                .border(1.dp(), Color::BLUE),
            content_alignment = Alignment::CENTER,
            {
                Column!(vertical_arrangement = Arrangement::spaced_by(12.dp()), {
                    BasicText!(
                        "I will definitely fill the screen!".repeat(10),
                        modifier = Modifier.border(1.dp(), Color::RED).test_tag(TEXT_TAG),
                        style = TextStyle {
                            // Compose's `color = { Color.Black }`.
                            color: Color::BLACK,
                            text_align: Some(TextAlign::Center),
                            letter_spacing: 1.sp(),
                            line_height: 24.sp(),
                            line_height_style: Some(LineHeightStyle {
                                alignment: LineHeightAlignment::CENTER,
                                trim: LineHeightTrim::None,
                                mode: LineHeightMode::Fixed,
                            }),
                            ..TextStyle::DEFAULT
                        },
                        overflow = TextOverflow::Ellipsis,
                        max_lines = 1,
                    );
                });
            }
        );
    });
    let before = ui.on_node_with_tag(TEXT_TAG).capture_to_image();
    // These layouts again reproduced b/389707025.
    for _ in 0..2 {
        ui.wait_for_idle();
        padding.set(padding.get() + 1.0);
    }
    ui.wait_for_idle();
    let after = ui.on_node_with_tag(TEXT_TAG).capture_to_image();
    assert_eq!(before.size(), after.size());
    assert!(before == after, "the line's pixels moved");
}

/// "Hello World" in `base`, with "World" in `span` or not: drawn otherwise, the same over "Hello"
/// when `same_line` (another family's metrics move the line's baseline, so "Hello" too).
fn span_changes_world(base: TextStyle, span: SpanStyle, same_line: bool) {
    let draw = |span: Option<SpanStyle>| {
        let text = AnnotatedString::build(|builder| {
            builder.append("Hello ");
            match &span {
                Some(span) => {
                    builder.with_style(span.clone(), |builder| {
                        builder.append("World");
                    });
                }
                None => {
                    builder.append("World");
                }
            }
        });
        let base = base.clone();
        let mut ui = rule(move || {
            BasicAnnotatedText!(
                text.clone(),
                modifier = Modifier.test_tag(TEXT_TAG),
                style = base.clone()
            );
        });
        ui.on_node_with_tag(TEXT_TAG).capture_to_image()
    };
    let (with_span, without) = (draw(Some(span)), draw(None));
    assert!(with_span != without, "the span changes nothing");
    if !same_line {
        return;
    }
    // "Hello" is the first five of eleven characters: its left part is drawn the same.
    let hello = without.width().min(with_span.width()) * 4 / 11;
    let height = without.height().min(with_span.height());
    for y in 0..height {
        for x in 0..hello {
            assert_eq!(with_span.pixel(x, y), without.pixel(x, y), "({x}, {y}) in \"Hello\"");
        }
    }
}

fn monospace(build: impl FnOnce(&mut TextStyle)) -> TextStyle {
    let mut style = TextStyle {
        font_size: 24.sp(),
        font_family: Some(FontFamily::Monospace),
        ..TextStyle::DEFAULT
    };
    build(&mut style);
    style
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextScreenshotTest.kt, multiStyleText_setFontWeight
#[test]
fn multi_style_text_set_font_weight() {
    span_changes_world(
        monospace(|style| style.font_style = Some(FontStyle::Italic)),
        SpanStyle { font_weight: Some(FontWeight::BOLD), ..SpanStyle::default() },
        true,
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextScreenshotTest.kt, multiStyleText_setFontFamily
#[test]
fn multi_style_text_set_font_family() {
    span_changes_world(
        monospace(|style| {
            style.font_style = Some(FontStyle::Italic);
            style.font_weight = Some(FontWeight::BOLD);
        }),
        SpanStyle { font_family: Some(FontFamily::SansSerif), ..SpanStyle::default() },
        false,
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextScreenshotTest.kt, multiStyleText_setFontStyle
#[test]
fn multi_style_text_set_font_style() {
    span_changes_world(
        monospace(|style| style.font_weight = Some(FontWeight::BOLD)),
        SpanStyle { font_style: Some(FontStyle::Italic), ..SpanStyle::default() },
        true,
    );
}
