//! The small `BasicText` files of the foundation text ledger, rewritten from their scenarios:
//! `BasicTextLayoutTest` (very long texts), both `BasicTextSemanticsTest`s (the semantics text
//! follows the text), `MultiParagraphLayoutCacheWidthWithLetterSpacingTest` (a text measured at
//! its own width stays on one line), `BasicTextIntrinsicWidthWrappingRegressionTest`,
//! `MinLinesMemoryLeakTest`, `MinLinesConstrainerTest` and `BasicTextGraphicsLayerTest`.
//!
//! n/a: `BasicTextUnexpectedWrappingRegressionTest` (Android's `StaticLayout` builder recycled
//! with `setUseBoundsForWidth`, with a font resource whose last glyph overshoots: compose-rs's
//! measurers have neither); `MinLinesConstrainerTest`'s `minConstrainer_from_reused` and
//! `minConstrainer_from_notCached` (the identity of Compose's internal `MinLinesConstrainer`: a
//! compose-rs text keeps its min-lines height in its node, D233). Waiting on `inline-content`:
//! `text/modifiers/BasicTextSemanticsTest.inlineContentSemantics_matchesInMergedSemantics`.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    AnnotatedRange, AnnotatedString, Color, Density, IntSize, IntrinsicSize, LOCAL_DENSITY,
    ParagraphStyle, SpanStyle, TextLayoutResult, UnitExt,
};

/// A rule in Compose's test font (each character a square of its size) at `density`.
fn rule_at(density: Density, content: impl Fn() + 'static) -> ComposeTestRule {
    rule_sized(IntSize::new(800, 800), density, content)
}

/// The same in a window of `size`.
fn rule_sized(size: IntSize, density: Density, content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(size, density, Box::new(measurer), content);
    ui.wait_for_idle();
    ui
}

/// The last layout `on_text_layout` heard.
#[derive(Clone, Default)]
struct Heard(Rc<RefCell<Option<TextLayoutResult>>>);

impl Heard {
    fn callback(&self) -> impl Fn(TextLayoutResult) + 'static {
        let heard = self.0.clone();
        move |result| *heard.borrow_mut() = Some(result)
    }

    fn last(&self) -> TextLayoutResult {
        self.0.borrow().clone().expect("a layout heard")
    }
}

// ---- BasicTextLayoutTest ----

/// The largest size of a constraint's unfocused dimension, as Compose's `Constraints.kt`.
const UNFOCUSED_DIMENSION_CONSTRAINT_MAX: f32 = (2 << 13) as f32;

/// "a" 2^15 times at 48 sp, as a string or an annotated string: laid out, taller than a
/// constraint's unfocused dimension can be.
fn very_long_text(annotated: bool) {
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let text = "a".repeat(2 << 14);
    let style = TextStyle { font_size: 48.sp(), ..TextStyle::DEFAULT };
    let _ui = rule_at(Density::new(1.0), move || {
        let callback = callback.clone();
        if annotated {
            BasicAnnotatedText!(
                AnnotatedString::new(text.clone()),
                style = style.clone(),
                on_text_layout = Some(move |result| callback(result))
            );
        } else {
            BasicText!(
                text.clone(),
                style = style.clone(),
                on_text_layout = Some(move |result| callback(result))
            );
        }
    });
    let height = heard.last().multi_paragraph.height();
    assert!(height > UNFOCUSED_DIMENSION_CONSTRAINT_MAX, "{height}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextLayoutTest.kt, simple_layoutText_doesNotThrow_when2shl14char
#[test]
fn simple_layoutText_doesNotThrow_when2shl14char() {
    very_long_text(false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextLayoutTest.kt, annotatedString_layoutText_doesNotThrow_when2shl14char
#[test]
fn annotatedString_layoutText_doesNotThrow_when2shl14char() {
    very_long_text(true);
}

// ---- BasicTextSemanticsTest (text and text/modifiers) ----

/// The semantics text follows the text, "before" then "after".
fn semantics_text_changes(annotated: bool) {
    let text = mutable_state_of("before".to_string());
    let current = text.handle();
    let mut ui = rule_at(Density::new(1.0), move || {
        if annotated {
            BasicAnnotatedText!(AnnotatedString::new(current.get()));
        } else {
            BasicText!(current.get());
        }
    });
    ui.on_node_with_text("before").assert_exists();
    text.set("after".to_string());
    ui.on_node_with_text("after").assert_exists();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextSemanticsTest.kt, semanticsTextChanges_String
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/BasicTextSemanticsTest.kt, semanticsTextChanges_String
#[test]
fn semanticsTextChanges_String() {
    semantics_text_changes(false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextSemanticsTest.kt, semanticsTextChanges_AnnotatedString
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/BasicTextSemanticsTest.kt, semanticsTextChanges_AnnotatedString
#[test]
fn semanticsTextChanges_AnnotatedString() {
    semantics_text_changes(true);
}

// ---- MultiParagraphLayoutCacheWidthWithLetterSpacingTest ----

/// "This is a callout message" in `style` at 12 sp, at the Pixel 4's density and font scale of
/// Compose's repro case, in a window wider than it (Compose's `Constraints()`: its width its
/// own): one line. In an empty paragraph
/// style, so it takes Compose's `MultiParagraphLayoutCache` path (as `basic_text_paragraphs.rs`).
fn assert_line_count(style: TextStyle) {
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let text = AnnotatedString::with_paragraph_style(
        "This is a callout message".to_string(),
        ParagraphStyle::DEFAULT,
    );
    let style = TextStyle { font_size: 12.sp(), ..style };
    let _ui =
        rule_sized(IntSize::new(4000, 800), Density::with_font_scale(3.051, 1.15), move || {
            let callback = callback.clone();
            BasicAnnotatedText!(
                text.clone(),
                style = style.clone(),
                on_text_layout = Some(move |result| callback(result))
            );
        });
    assert_eq!(heard.last().line_count, 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/MultiParagraphLayoutCacheWidthWithLetterSpacingTest.kt, letterSpacing_and_lineHeight
#[test]
fn letterSpacing_and_lineHeight() {
    assert_line_count(TextStyle {
        letter_spacing: 0.4.sp(),
        line_height: 16.sp(),
        ..TextStyle::DEFAULT
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/MultiParagraphLayoutCacheWidthWithLetterSpacingTest.kt, only_letterSpacing
#[test]
fn only_letterSpacing() {
    assert_line_count(TextStyle { letter_spacing: 0.4.sp(), ..TextStyle::DEFAULT });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/MultiParagraphLayoutCacheWidthWithLetterSpacingTest.kt, only_lineHeight
#[test]
fn only_lineHeight() {
    assert_line_count(TextStyle { line_height: 16.sp(), ..TextStyle::DEFAULT });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/MultiParagraphLayoutCacheWidthWithLetterSpacingTest.kt, no_lineHeight_or_letterSpacing
#[test]
fn no_lineHeight_or_letterSpacing() {
    assert_line_count(TextStyle::DEFAULT);
}

// ---- BasicTextIntrinsicWidthWrappingRegressionTest (b/346918500) ----

/// The repro case's text: five CJK characters and "...".
const CJK: &str = "\u{8AAD}\u{307F}\u{8FBC}\u{307F}\u{4E2D}...";

/// For each start of a translucent span over the rest of the text (Compose's parameters, 0 to
/// the length), at density 2.625 with a line height: no visual overflow, one line.
fn run_cjk(soft_wrap: bool, max_lines: u32) {
    let boundaries: Vec<usize> =
        CJK.char_indices().map(|(index, _)| index).chain([CJK.len()]).collect();
    for span_start in boundaries {
        let heard = Heard::default();
        let callback = Rc::new(heard.callback());
        let span = SpanStyle { color: Color::BLACK.with_alpha(0.3), ..SpanStyle::default() };
        let text = AnnotatedString::with_styles(
            CJK,
            [AnnotatedRange::new(span, span_start, CJK.len())],
            [],
        );
        let style = TextStyle {
            color: Color::BLACK,
            font_size: 16.sp(),
            line_height: 20.sp(),
            ..TextStyle::DEFAULT
        };
        let _ui = rule_at(Density::new(1.0), move || {
            let (callback, text, style) = (callback.clone(), text.clone(), style.clone());
            CompositionLocalProvider(
                LOCAL_DENSITY.provides(Density::with_font_scale(2.625, 1.0)),
                move || {
                    let callback = callback.clone();
                    BasicAnnotatedText!(
                        text.clone(),
                        style = style.clone(),
                        on_text_layout = Some(move |result| callback(result)),
                        soft_wrap = soft_wrap,
                        max_lines = max_lines
                    );
                },
            );
        });
        let layout = heard.last();
        assert!(!layout.has_visual_overflow(), "span from {span_start}");
        assert_eq!(layout.line_count, 1, "span from {span_start}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextIntrinsicWidthWrappingRegressionTest.kt, whenSoftWrapFalse_lineCountIsOne
#[test]
fn whenSoftWrapFalse_lineCountIsOne() {
    run_cjk(false, u32::MAX);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextIntrinsicWidthWrappingRegressionTest.kt, whenMaxLinesOne_lineCountIsOne
#[test]
fn whenMaxLinesOne_lineCountIsOne() {
    run_cjk(true, 1);
}

// ---- MinLinesMemoryLeakTest ----

/// The paragraphs the host's fixed measurer holds.
fn live_paragraphs(ui: &mut ComposeTestRule) -> usize {
    ui.host().with_tree(|tree| {
        let measurer = tree.text_measurer();
        (&*measurer as &dyn std::any::Any)
            .downcast_ref::<FixedTextMeasurer>()
            .expect("the fixed measurer")
            .live()
    })
}

/// A text with `min_lines` (1, 2 and 100: Compose's parameters), then gone: no paragraph its
/// min-lines height measured is left behind (Compose checks for leaks with LeakCanary).
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/MinLinesMemoryLeakTest.kt, MinLinesMemoryLeakTest
#[test]
fn MinLinesMemoryLeakTest() {
    for num_lines in [1, 2, 100] {
        let shown = mutable_state_of(true);
        let is_shown = shown.handle();
        let mut ui = rule_at(Density::new(1.0), move || {
            if is_shown.get() {
                BasicText!("Lorem ipsum dolor sit amet.", min_lines = num_lines);
            }
        });
        assert!(live_paragraphs(&mut ui) > 0, "min lines {num_lines}");
        shown.set(false);
        ui.wait_for_idle();
        assert_eq!(live_paragraphs(&mut ui), 0, "min lines {num_lines}");
    }
}

// ---- MinLinesConstrainerTest ----

/// A style change gives the min-lines height of the new style, as Compose makes a new
/// `MinLinesConstrainer` (its test changes the color, which leaves the height as it was; a font
/// size shows the height measured again).
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/MinLinesConstrainerTest.kt, minConstrainer_from_new_onStyleChange
#[test]
fn minConstrainer_from_new_onStyleChange() {
    let style = mutable_state_of(TextStyle { color: Color::GREEN, ..TextStyle::DEFAULT });
    let current = style.handle();
    let mut ui = rule_at(Density::new(1.0), move || {
        BasicText!(
            "A",
            modifier = Modifier.test_tag("text").width(200.dp()).height(IntrinsicSize::Min),
            style = current.get(),
            min_lines = 3
        );
    });
    let height = |ui: &mut ComposeTestRule| {
        ui.on_node_with_tag("text").fetch_semantics_snapshot().bounds.height()
    };
    let first = height(&mut ui);
    style.set(TextStyle { color: Color::BLUE, ..TextStyle::DEFAULT });
    ui.wait_for_idle();
    assert_eq!(height(&mut ui), first);
    style.set(TextStyle { color: Color::BLUE, font_size: 28.sp(), ..TextStyle::DEFAULT });
    ui.wait_for_idle();
    assert_eq!(height(&mut ui), first * 2.0, "three lines of 28 sp, where they were 14");
}

// ---- BasicTextGraphicsLayerTest ----

/// The repaint boundaries (graphics layers among them) drawn.
fn boundaries(ui: &mut ComposeTestRule) -> usize {
    ui.host().display_list().groups.len()
}

/// A text in a column draws in no layer of its own: as many boundaries as the column alone.
fn no_graphics_layer(annotated: bool) {
    let with_text = mutable_state_of(false);
    let shown = with_text.handle();
    let mut ui = rule_at(Density::new(1.0), move || {
        Column!({
            if shown.get() {
                if annotated {
                    BasicAnnotatedText!(AnnotatedString::new("Ok"));
                } else {
                    BasicText!("Ok");
                }
            }
        });
    });
    let without = boundaries(&mut ui);
    with_text.set(true);
    ui.wait_for_idle();
    ui.on_node_with_text("Ok").assert_exists();
    assert_eq!(boundaries(&mut ui), without);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextGraphicsLayerTest.kt, modifiersDoNotExposeGraphicsLayer
#[test]
fn modifiersDoNotExposeGraphicsLayer() {
    no_graphics_layer(false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextGraphicsLayerTest.kt, modifiersDoNotExposeGraphicsLayer_annotatedString
#[test]
fn modifiersDoNotExposeGraphicsLayer_annotatedString() {
    no_graphics_layer(true);
}
