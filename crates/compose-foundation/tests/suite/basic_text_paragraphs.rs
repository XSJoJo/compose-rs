//! `BasicText` laying a text with paragraph styles out as paragraphs, as Compose's
//! `TextAnnotatedStringNode` with its `MultiParagraphLayoutCache` (D2053): each paragraph in its
//! own style (alignment, direction, indent, line height), drawn at its top, measured, its
//! intrinsics and baselines the paragraphs', the maximum lines and an ellipsis across them, and
//! `on_text_layout` hearing the real `MultiParagraph`. A text without paragraph styles stays one
//! platform paragraph.
//!
//! In Compose's test font (each character a square of its size, D1124), 20 px a character: a
//! line is 20 px high, its baseline at 16.
//!
//! Also ports, through a text of paragraphs: Compose's `MultiParagraphLayoutCacheTest` (its cases
//! without auto size, the text in an empty `ParagraphStyle` so it takes Compose's path),
//! `TextOverflowTest` and `BasicTextDensityTest`.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{DrawOp, FixedFontMetrics, FixedTextMeasurer, ParagraphHandle};
use compose_ui::{
    AnnotatedString, BlendMode, Constraints, Density, FIRST_BASELINE, IntSize, IntrinsicSize,
    LAST_BASELINE, LayoutCoordinates, LayoutDirection, Offset, ParagraphStyle,
    ResolvedTextDirection, SpanStyle, TextAlign, TextDirection, TextIndent, TextLayoutResult,
    TextOverflow, TextPainter, UnitExt,
};

const TAG: &str = "text";
const FONT: f32 = 20.0;

fn style() -> TextStyle {
    TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT }
}

fn paragraph(build: impl FnOnce(&mut ParagraphStyle)) -> ParagraphStyle {
    let mut style = ParagraphStyle::DEFAULT;
    build(&mut style);
    style
}

/// "Hello", then "World" in its own paragraph styled `second`.
fn hello_world(second: ParagraphStyle) -> AnnotatedString {
    AnnotatedString::build(|text| {
        text.append("Hello");
        text.with_paragraph_style(second, |text| {
            text.append("World");
        });
    })
}

/// Each of `texts` in a paragraph of its own, in no style of its own.
fn paragraphs(texts: &[&str]) -> AnnotatedString {
    AnnotatedString::build(|text| {
        for part in texts {
            text.with_paragraph_style(ParagraphStyle::DEFAULT, |text| {
                text.append(part);
            });
        }
    })
}

/// A rule in Compose's test font at density 1.
fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(800, 800), Density::new(1.0), Box::new(measurer), content);
    ui.wait_for_idle();
    ui
}

/// The results `on_text_layout` heard.
#[derive(Clone, Default)]
struct Heard(Rc<RefCell<Vec<TextLayoutResult>>>);

impl Heard {
    fn callback(&self) -> impl Fn(TextLayoutResult) + 'static {
        let heard = self.0.clone();
        move |result| heard.borrow_mut().push(result)
    }

    fn last(&self) -> TextLayoutResult {
        self.0.borrow().last().cloned().expect("a layout heard")
    }

    fn count(&self) -> usize {
        self.0.borrow().len()
    }
}

/// `text` laid out in `modifier` with `options`, what `on_text_layout` heard and its rule.
struct Options {
    modifier: Modifier,
    style: TextStyle,
    max_lines: u32,
    min_lines: u32,
    overflow: TextOverflow,
    soft_wrap: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            modifier: Modifier,
            style: style(),
            max_lines: u32::MAX,
            min_lines: 1,
            overflow: TextOverflow::Clip,
            soft_wrap: true,
        }
    }
}

fn laid_out(text: AnnotatedString, options: Options) -> (ComposeTestRule, Heard) {
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let options = Rc::new(options);
    let ui = rule(move || {
        let callback = callback.clone();
        BasicAnnotatedText!(
            text.clone(),
            modifier = options.modifier.clone().test_tag(TAG),
            style = options.style.clone(),
            on_text_layout = Some(move |result| callback(result)),
            overflow = options.overflow,
            soft_wrap = options.soft_wrap,
            max_lines = options.max_lines,
            min_lines = options.min_lines
        );
    });
    (ui, heard)
}

/// The paragraphs drawn, and where.
fn drawn(ui: &mut ComposeTestRule) -> Vec<(ParagraphHandle, Offset)> {
    ui.host()
        .display_list()
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::DrawParagraph(handle, at) => Some((*handle, *at)),
            _ => None,
        })
        .collect()
}

fn clips(ui: &mut ComposeTestRule) -> bool {
    ui.host().display_list().ops.iter().any(|op| matches!(op, DrawOp::Save))
}

fn bounds(ui: &mut ComposeTestRule) -> IntSize {
    let bounds = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    IntSize::new(bounds.width() as i32, bounds.height() as i32)
}

// Layout and drawing.

#[test]
fn each_paragraph_is_laid_out_in_its_own_style() {
    let centered = paragraph(|style| style.text_align = Some(TextAlign::Center));
    let (mut ui, heard) = laid_out(
        hello_world(centered),
        Options { modifier: Modifier.width(200.dp()), ..Default::default() },
    );
    let result = heard.last();
    let multi_paragraph = &result.multi_paragraph;
    assert_eq!(multi_paragraph.paragraph_info_list().len(), 2);
    assert_eq!(result.line_count, 2);
    assert_eq!(result.size, IntSize::new(200, 40));
    assert_eq!(bounds(&mut ui), IntSize::new(200, 40));
    // The first at the start, the second centered in the width.
    assert_eq!(result.get_line_left(0), 0.0);
    assert_eq!(result.get_line_left(1), 50.0);
    assert_eq!(result.get_line_top(1), 20.0);
    assert_eq!(result.get_line_start(1), 5);
}

#[test]
fn a_text_without_paragraph_styles_stays_one_platform_paragraph() {
    let spans = AnnotatedString::with_span_style("Hello World", SpanStyle::default(), None);
    for (text, split) in [
        (AnnotatedString::new("Hello World"), false),
        (spans, false),
        (paragraphs(&["Hello World"]), true),
    ] {
        let (_ui, heard) = laid_out(text, Options::default());
        let result = heard.last();
        assert_eq!(result.multi_paragraph.intrinsics().is_some(), split);
        assert_eq!(result.size, IntSize::new(220, 20));
    }
}

#[test]
fn each_paragraph_is_drawn_at_its_top_in_the_text_style() {
    let (mut ui, _) = laid_out(paragraphs(&["Hello", "World"]), Options::default());
    let drawn = drawn(&mut ui);
    assert_eq!(drawn.len(), 2, "{}", ui.host().display_list().dump());
    assert_eq!((drawn[0].1, drawn[1].1), (Offset::new(0.0, 0.0), Offset::new(0.0, 20.0)));
    assert!(!clips(&mut ui));
}

#[test]
fn a_paragraph_runs_in_its_own_direction() {
    let rtl = paragraph(|style| style.text_direction = Some(TextDirection::Rtl));
    let (_ui, heard) = laid_out(
        hello_world(rtl),
        Options { modifier: Modifier.width(200.dp()), ..Default::default() },
    );
    let result = heard.last();
    assert_eq!(result.get_paragraph_direction(0), ResolvedTextDirection::Ltr);
    assert_eq!(result.get_paragraph_direction(7), ResolvedTextDirection::Rtl);
    // At its start, which is the right.
    assert_eq!(result.get_line_left(0), 0.0);
    assert_eq!(result.get_line_left(1), 100.0);
}

#[test]
fn a_paragraph_has_its_own_indent_and_line_height() {
    let indented = paragraph(|style| {
        style.text_indent = Some(TextIndent { first_line: 40.sp(), rest_line: 0.sp() });
        style.line_height = 40.sp();
    });
    let text = AnnotatedString::build(|text| {
        text.append("Hello");
        text.with_paragraph_style(indented, |text| {
            // Three lines at 300 px: two words, two words and one.
            text.append("World World World World World");
        });
    });
    let (mut ui, heard) =
        laid_out(text, Options { modifier: Modifier.width(300.dp()), ..Default::default() });
    let result = heard.last();
    // Its first line indented, not the others, nor the paragraph above.
    assert_eq!(result.get_line_left(0), 0.0);
    assert_eq!(result.get_line_left(1), 40.0);
    assert_eq!(result.get_line_left(2), 0.0);
    // Its lines 40 high (the first's top and the last's bottom trimmed, as the default
    // `LineHeightStyle`'s: 16 and 4 of the 20 more), not the paragraph above.
    let heights: Vec<f32> =
        (0..4).map(|line| result.multi_paragraph.get_line_height(line)).collect();
    assert_eq!(heights, vec![20.0, 24.0, 40.0, 36.0]);
    assert_eq!(result.size.height, 120);
    assert_eq!(bounds(&mut ui).height, 120);
}

#[test]
fn the_intrinsic_widths_are_the_widest_paragraphs() {
    let text = paragraphs(&["Hi", "Hello World"]);
    let (mut ui, _) = laid_out(
        text.clone(),
        Options { modifier: Modifier.width(IntrinsicSize::Max), ..Default::default() },
    );
    assert_eq!(bounds(&mut ui).width, 220);
    let (mut ui, _) = laid_out(
        text,
        Options { modifier: Modifier.width(IntrinsicSize::Min), ..Default::default() },
    );
    // Its longest word, with the space after it, as Android's line breaks measure it; the same
    // as the text alone.
    assert_eq!(bounds(&mut ui).width, 120);
    let (mut ui, _) = laid_out(
        AnnotatedString::new("Hello World"),
        Options { modifier: Modifier.width(IntrinsicSize::Min), ..Default::default() },
    );
    assert_eq!(bounds(&mut ui).width, 120);
}

#[test]
fn the_intrinsic_height_is_the_paragraphs_laid_out_at_the_width() {
    let text = paragraphs(&["Hi", "Hello World"]);
    let modifier = Modifier.width(100.dp()).height(IntrinsicSize::Min);
    let (mut ui, _) = laid_out(text, Options { modifier, ..Default::default() });
    // "Hi", then "Hello" and "World".
    assert_eq!(bounds(&mut ui), IntSize::new(100, 60));
}

#[test]
fn the_baselines_are_the_first_paragraphs_first_and_the_last_paragraphs_last() {
    let seen = Rc::new(Cell::new((0, 0)));
    let s = seen.clone();
    let _ui = rule(move || {
        let s = s.clone();
        Row!(|scope| {
            let s = s.clone();
            BasicAnnotatedText!(
                paragraphs(&["Hello", "World"]),
                modifier = Modifier.align_by_block(scope, move |measured| {
                    let first = measured.alignment_line(FIRST_BASELINE);
                    s.set((first, measured.alignment_line(LAST_BASELINE)));
                    first
                }),
                style = style()
            );
        });
    });
    assert_eq!(seen.get(), (16, 36));
}

#[test]
fn the_maximum_lines_count_across_paragraphs() {
    let (mut ui, heard) =
        laid_out(paragraphs(&["A", "B", "C"]), Options { max_lines: 2, ..Default::default() });
    let result = heard.last();
    assert_eq!(result.line_count, 2);
    assert!(result.did_overflow_height);
    assert_eq!(result.multi_paragraph.paragraph_info_list().len(), 2);
    assert_eq!(result.size.height, 40);
    // The third paragraph isn't laid out, so not drawn; what's drawn is clipped, as Compose's
    // `hasVisualOverflow`.
    assert_eq!(drawn(&mut ui).len(), 2);
    assert!(clips(&mut ui));
}

#[test]
fn the_last_line_the_maximum_lines_leave_is_ellipsized() {
    let (_ui, heard) = laid_out(
        paragraphs(&["Hello", "World World World"]),
        Options {
            modifier: Modifier.width(100.dp()),
            max_lines: 2,
            overflow: TextOverflow::Ellipsis,
            ..Default::default()
        },
    );
    let result = heard.last();
    assert_eq!(result.line_count, 2);
    assert!(!result.is_line_ellipsized(0));
    assert!(result.is_line_ellipsized(1));
}

#[test]
fn the_heard_layout_paints() {
    let held: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
    let show = mutable_state_of(false);
    let (h, s) = (held.clone(), show.handle());
    let mut ui = rule(move || {
        let (h, s) = (h.clone(), s);
        Column!({
            let (h, held, s) = (h.clone(), h.clone(), s);
            BasicAnnotatedText!(
                paragraphs(&["Hello", "World"]),
                style = style(),
                on_text_layout = Some(move |result| *h.borrow_mut() = Some(result))
            );
            Canvas(Modifier.width(200.dp()).height(100.dp()), move |scope| {
                if s.get() {
                    let held = held.borrow();
                    let result = held.as_ref().expect("heard");
                    TextPainter::paint(scope, result);
                    result.multi_paragraph.paint(
                        scope,
                        compose_ui::Color::RED,
                        None,
                        None,
                        None,
                        BlendMode::SrcOver,
                    );
                }
            });
        });
    });
    assert_eq!(drawn(&mut ui).len(), 2);
    show.set(true);
    ui.wait_for_idle();
    let drawn = drawn(&mut ui);
    // The text's own two, then the painter's and `paint`'s, each paragraph at its top in the
    // canvas.
    assert_eq!(drawn.len(), 6, "{}", ui.host().display_list().dump());
    let tops: Vec<f32> = drawn[2..].iter().map(|(_, at)| at.y).collect();
    assert_eq!(tops, vec![0.0, 20.0, 0.0, 20.0]);
    // The painter draws them in their own paint, as the text: the same paragraphs.
    assert_eq!((drawn[2].0, drawn[3].0), (drawn[0].0, drawn[1].0));
    // In red: copies in that paint.
    assert_ne!(drawn[4].0, drawn[0].0);
}

#[test]
fn a_new_paint_keeps_the_layout_and_draws_it_in_the_paint() {
    let color = mutable_state_of(compose_ui::Color::BLACK);
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = color.handle();
    let mut ui = rule(move || {
        let callback = callback.clone();
        BasicAnnotatedText!(
            paragraphs(&["Hello", "World"]),
            style = TextStyle { color: handle.get(), ..style() },
            on_text_layout = Some(move |result| callback(result))
        );
    });
    let before = drawn(&mut ui);
    color.set(compose_ui::Color::RED);
    ui.wait_for_idle();
    let after = drawn(&mut ui);
    assert_eq!(heard.count(), 1, "laid out again");
    assert_eq!(after.len(), 2);
    assert_ne!(after[0].0, before[0].0);
    assert_ne!(after[1].0, before[1].0);
    // Back to its first paint: the paragraphs it was laid out in.
    color.set(compose_ui::Color::BLACK);
    ui.wait_for_idle();
    assert_eq!(drawn(&mut ui), before);
}

#[test]
fn a_new_text_lays_the_paragraphs_out_again() {
    let text = mutable_state_of(paragraphs(&["Hello", "World"]));
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = text.handle();
    let mut ui = rule(move || {
        let callback = callback.clone();
        BasicAnnotatedText!(
            handle.get(),
            modifier = Modifier.test_tag(TAG),
            style = style(),
            on_text_layout = Some(move |result| callback(result))
        );
    });
    text.set(paragraphs(&["Hello", "World", "Again"]));
    ui.wait_for_idle();
    assert_eq!(heard.count(), 2);
    assert_eq!(heard.last().line_count, 3);
    assert_eq!(bounds(&mut ui).height, 60);
    // Without paragraph styles: one platform paragraph again.
    text.set(AnnotatedString::new("Hello"));
    ui.wait_for_idle();
    assert!(heard.last().multi_paragraph.intrinsics().is_none());
    assert_eq!(drawn(&mut ui).len(), 1);
}

// Compose's `TextOverflowTest`: a text in `TextOverflow::Visible` draws past its bounds.

/// Compose's text in a box 200 by 100, at most 40 high with 20 of padding below: 20 high.
fn visible_overflow(text: AnnotatedString) -> ComposeTestRule {
    rule(move || {
        let text = text.clone();
        Box!(modifier = Modifier.width(200.dp()).height(100.dp()), {
            BasicAnnotatedText!(
                text.clone(),
                modifier = Modifier.fill_max_width().height_in(0.dp(), 40.dp()).padding((
                    0.dp(),
                    0.dp(),
                    0.dp(),
                    20.dp()
                )),
                style = TextStyle { line_height: 20.sp(), ..style() },
                overflow = TextOverflow::Visible
            );
        });
    })
}

#[test]
fn paint_singleParagraph_withVisibleOverflow() {
    let mut ui = visible_overflow(AnnotatedString::new("Hello\nHello\nHello\nHello"));
    let drawn = drawn(&mut ui);
    assert_eq!(drawn.len(), 1);
    assert!(!clips(&mut ui));
    // Its four lines go past the 40 px the box's crop starts at.
    let height =
        ui.host().with_tree(|tree| tree.text_measurer_ref().metrics(drawn[0].0).size.height);
    assert_eq!(height, 80);
}

#[test]
fn paint_multiParagraph_withVisibleOverflow() {
    let text = AnnotatedString::build(|text| {
        text.append("Hello\nHello");
        text.with_paragraph_style(
            paragraph(|style| style.text_align = Some(TextAlign::Center)),
            |text| {
                text.append("Hello\nHello");
            },
        );
    });
    let mut ui = visible_overflow(text);
    let drawn = drawn(&mut ui);
    assert!(!clips(&mut ui));
    // The second paragraph drawn at 40, where the box's crop starts.
    assert_eq!(drawn.iter().map(|(_, at)| at.y).collect::<Vec<_>>(), vec![0.0, 40.0]);
}

// Compose's `BasicTextDensityTest`: a new density lays the text out again.

fn grows_with_density(content: impl Fn(Rc<Cell<IntSize>>) + 'static) {
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let s = size.clone();
    let mut ui = compose_test(move || content(s.clone()));
    ui.wait_for_idle();
    let before = size.get();
    ui.host().set_density(Density::new(2.0));
    ui.wait_for_idle();
    let after = size.get();
    assert!(after.width > before.width && after.height > before.height, "{before:?} {after:?}");
}

fn measured(size: Rc<Cell<IntSize>>) -> Modifier {
    Modifier
        .on_globally_positioned(move |coordinates: LayoutCoordinates| size.set(coordinates.size()))
}

#[test]
fn simpleMultiParagraph_densityChange() {
    grows_with_density(|size| {
        let text = AnnotatedString::build(|text| {
            text.with_paragraph_style(paragraph(|style| style.line_height = 20.sp()), |text| {
                text.append("Hello");
            });
            text.with_paragraph_style(paragraph(|style| style.line_height = 15.sp()), |text| {
                text.append("World");
            });
        });
        BasicAnnotatedText!(text, modifier = measured(size));
    });
}

#[test]
fn selectableText_densityChange() {
    grows_with_density(|size| {
        SelectionContainer!({
            BasicText!("Hello World!", modifier = measured(size.clone()));
        });
    });
}

// Compose's `MultiParagraphLayoutCacheTest`, through a text of paragraphs (the text in an empty
// paragraph style) and its node; its auto size cases aren't ported (no `TextAutoSize`).

fn one_paragraph(text: &str) -> AnnotatedString {
    AnnotatedString::with_paragraph_style(text.to_string(), ParagraphStyle::DEFAULT)
}

#[test]
fn minIntrinsicWidth_getter() {
    let (mut ui, _) = laid_out(
        one_paragraph("Hello"),
        Options { modifier: Modifier.width(IntrinsicSize::Min), ..Default::default() },
    );
    assert_eq!(bounds(&mut ui).width, 100);
}

#[test]
fn maxIntrinsicWidth_getter() {
    let (mut ui, _) = laid_out(
        one_paragraph("Hello"),
        Options { modifier: Modifier.width(IntrinsicSize::Max), ..Default::default() },
    );
    assert_eq!(bounds(&mut ui).width, 100);
}

/// The text's height at 20 px wide, through `height(IntrinsicSize::Min)`.
fn intrinsic_height_at_20(text: AnnotatedString, min_lines: u32, width: f32) -> i32 {
    let modifier = Modifier.width(width.dp()).height(IntrinsicSize::Min);
    let (mut ui, _) = laid_out(text, Options { modifier, min_lines, ..Default::default() });
    bounds(&mut ui).height
}

#[test]
fn intrinsicHeight_invalidates() {
    let text = mutable_state_of(one_paragraph("Hello"));
    let handle = text.handle();
    let mut ui = rule(move || {
        BasicAnnotatedText!(
            handle.get(),
            modifier = Modifier.test_tag(TAG).width(20.dp()).height(IntrinsicSize::Min),
            style = style()
        );
    });
    let original = bounds(&mut ui).height;
    text.set(one_paragraph("Longer\ntext\ngoes\nhere\n\n\n."));
    ui.wait_for_idle();
    assert!(original < bounds(&mut ui).height);
}

#[test]
fn minIntrinsicsHeight_respectsMinLines() {
    let single = intrinsic_height_at_20(one_paragraph("A"), 1, 200.0);
    assert_eq!(intrinsic_height_at_20(one_paragraph("A"), 3, 200.0), single * 3);
}

/// A text of paragraphs laid out at `first`, then at `second`: what was heard each time.
fn relaid(
    text: AnnotatedString,
    options: Options,
    first: Constraints,
    second: Constraints,
) -> (TextLayoutResult, TextLayoutResult) {
    let constraints = mutable_state_of(first);
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = constraints.handle();
    let options = Rc::new(options);
    let mut ui = rule(move || {
        let callback = callback.clone();
        let handle = handle;
        BasicAnnotatedText!(
            text.clone(),
            modifier = Modifier.layout(move |scope, measurable, _| {
                let placeable = scope.measure(measurable, handle.get());
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }),
            style = options.style.clone(),
            on_text_layout = Some(move |result| callback(result)),
            overflow = options.overflow,
            soft_wrap = options.soft_wrap,
            max_lines = options.max_lines
        );
    });
    let before = heard.last();
    constraints.set(second);
    ui.wait_for_idle();
    (before, heard.last())
}

#[test]
fn TextLayoutInput_reLayout_withDifferentHeight() {
    let (first, second) = (Constraints::fixed(200, 100), Constraints::fixed(200, 200));
    let (before, after) = relaid(one_paragraph("Hello World"), Options::default(), first, second);
    assert_eq!(before.layout_input.constraints, first);
    assert_eq!(after.layout_input.constraints, second);
}

#[test]
fn TextLayoutResult_reLayout_withDifferentHeight() {
    let (first, second) = (Constraints::fixed(200, 100), Constraints::fixed(200, 200));
    let (before, after) = relaid(one_paragraph("Hello World"), Options::default(), first, second);
    assert_eq!(before.size.height, 100);
    assert_eq!(after.size.height, 200);
}

const LONG: &str = "Hello World! Hello World! Hello World! Hello World!";

#[test]
fn TextLayoutResult_layout_withEllipsis_withoutSoftWrap() {
    let options =
        Options { soft_wrap: false, overflow: TextOverflow::Ellipsis, ..Default::default() };
    // Half its own width.
    let width = LONG.len() as i32 * 20 / 2;
    let (_, result) = relaid(
        one_paragraph(LONG),
        options,
        Constraints::fixed(0, 0),
        Constraints::new(0, width, 0, Constraints::INFINITY),
    );
    assert_eq!(result.line_count, 1);
    assert!(result.is_line_ellipsized(0));
}

#[test]
fn TextLayoutResult_layoutWithLimitedHeight_withEllipsis() {
    let options = Options { overflow: TextOverflow::Ellipsis, ..Default::default() };
    let width = LONG.len() as i32 * 20 / 4;
    // Two lines fit wholly, at most.
    let height = (20.0f32 * 2.7).round() as i32;
    let (_, result) = relaid(
        one_paragraph(LONG),
        options,
        Constraints::fixed(0, 0),
        Constraints::new(0, width, 0, height),
    );
    assert_eq!(result.line_count, 2);
    assert!(result.is_line_ellipsized(1));
}

fn grows_with(density: Density) {
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let mut ui = compose_test(move || {
        let callback = callback.clone();
        BasicAnnotatedText!(
            one_paragraph("Hello World"),
            on_text_layout = Some(move |result| callback(result))
        );
    });
    ui.wait_for_idle();
    let before = heard.last().size;
    ui.host().set_density(density);
    ui.wait_for_idle();
    let after = heard.last().size;
    assert!(before.width < after.width && before.height < after.height, "{before:?} {after:?}");
}

#[test]
fn TextLayoutResult_reLayout_withDifferentDensity() {
    grows_with(Density::new(2.0));
}

#[test]
fn TextLayoutResult_reLayout_withDifferentFontScale() {
    grows_with(Density::with_font_scale(1.0, 2.0));
}

#[test]
fn TextLayoutResult_sameWidth_inRtlAndLtr_withLetterSpacing() {
    let width = |direction: LayoutDirection| {
        let heard = Heard::default();
        let callback = Rc::new(heard.callback());
        let _ui = rule(move || {
            let callback = callback.clone();
            CompositionLocalProvider(
                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                move || {
                    let callback = callback.clone();
                    BasicAnnotatedText!(
                        one_paragraph("Hello World"),
                        style = TextStyle { letter_spacing: 0.5.sp(), ..style() },
                        on_text_layout = Some(move |result| callback(result)),
                        overflow = TextOverflow::Ellipsis
                    );
                },
            );
        });
        heard.last().size.width
    };
    assert_eq!(width(LayoutDirection::Ltr), width(LayoutDirection::Rtl));
}

#[test]
fn maxHeight_hasSameHeight_asParagraph() {
    // Compose compares with the text laid out as one `Paragraph`: here, the same text without
    // the paragraph style, which `BasicText` lays out as one platform paragraph.
    let lines = AnnotatedString::build(|text| {
        for size in (1..100).step_by(10) {
            text.with_style(
                SpanStyle { font_size: (size as f32).sp(), ..SpanStyle::default() },
                |text| {
                    text.append(&format!("{size}.sp\n"));
                },
            );
        }
    });
    let height = |text: AnnotatedString| {
        let options = Options {
            style: TextStyle { font_size: 1.sp(), ..TextStyle::DEFAULT },
            overflow: TextOverflow::Ellipsis,
            max_lines: 5,
            ..Default::default()
        };
        let (_ui, heard) = laid_out(text, options);
        heard.last().multi_paragraph.height()
    };
    let whole = AnnotatedString::build(|text| {
        text.with_paragraph_style(ParagraphStyle::DEFAULT, |text| {
            text.append_annotated(&lines);
        });
    });
    assert!(whole.has_paragraph_styles());
    assert_eq!(height(whole), height(lines));
}

#[test]
fn hugeString_doesntCrash() {
    let text = "A".repeat(100_000);
    let (_ui, heard) = laid_out(
        one_paragraph(&text),
        Options {
            style: TextStyle { font_size: 100.sp(), ..TextStyle::DEFAULT },
            ..Default::default()
        },
    );
    assert!(heard.count() > 0);
}

#[test]
fn minIntrinsicWidth_doesNotClearLayoutCache_afterLayout() {
    // A parent that measures the text, then asks its minimum intrinsic width.
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let asked = Rc::new(Cell::new(0));
    let a = asked.clone();
    let pass = mutable_state_of(0);
    let p = pass.handle();
    let mut ui = rule(move || {
        let (callback, a, p) = (callback.clone(), a.clone(), p);
        BasicAnnotatedText!(
            one_paragraph("hello"),
            modifier = Modifier.layout(move |scope, measurable, constraints| {
                // Measured again in each pass.
                let _ = p.get();
                let placeable = scope.measure(measurable, constraints);
                a.set(scope.min_intrinsic_width(measurable, Constraints::INFINITY));
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }),
            style = style(),
            on_text_layout = Some(move |result| callback(result))
        );
    });
    assert_eq!(asked.get(), 100);
    let first = heard.last();
    pass.set(1);
    ui.wait_for_idle();
    assert_eq!(asked.get(), 100);
    assert_eq!(heard.count(), 1);
    assert!(Rc::ptr_eq(&heard.last().multi_paragraph, &first.multi_paragraph));
}

// A text's own layout paints, whatever its kind (D2053): a plain text's and a field's one
// platform paragraph only answers queries (its node keeps the layout), so its input is laid out
// again to paint, once.

/// `content`, then a canvas painting in red what `held` holds once `show` is set.
fn with_a_canvas(
    content: impl Fn() + 'static,
    held: Rc<RefCell<Option<TextLayoutResult>>>,
    show: MutableState<bool>,
) -> ComposeTestRule {
    let s = show;
    rule(move || {
        let (s, held) = (s, held.clone());
        content();
        Canvas(Modifier.width(200.dp()).height(100.dp()), move |scope| {
            if s.get() {
                let held = held.borrow();
                let result = held.as_ref().expect("a layout");
                let red = compose_ui::Color::RED;
                result.multi_paragraph.paint(scope, red, None, None, None, BlendMode::SrcOver);
                result.multi_paragraph.paint(scope, red, None, None, None, BlendMode::SrcOver);
            }
        });
    })
}

#[test]
fn a_plain_texts_heard_layout_paints() {
    let held: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
    let show = mutable_state_of(false);
    let h = held.clone();
    let mut ui = with_a_canvas(
        move || {
            let h = h.clone();
            BasicText!(
                "Hello",
                style = style(),
                on_text_layout = Some(move |result| *h.borrow_mut() = Some(result))
            );
        },
        held.clone(),
        show.handle(),
    );
    assert!(held.borrow().as_ref().unwrap().multi_paragraph.intrinsics().is_none());
    let before = drawn(&mut ui);
    show.set(true);
    ui.wait_for_idle();
    let after = drawn(&mut ui);
    assert_eq!(after.len(), before.len() + 2);
    // Laid out again once, its copy in red drawn twice.
    let (first, second) = (after[before.len()], after[before.len() + 1]);
    assert_eq!(first, second);
    assert_ne!(first.0, before[0].0);
}

#[test]
fn a_fields_layout_paints() {
    let state = **Box::leak(Box::new(TextFieldState::new("Hello")));
    let held: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
    let show = mutable_state_of(false);
    let mut ui = with_a_canvas(
        move || {
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.test_tag("field"),
                text_style = style()
            );
        },
        held.clone(),
        show.handle(),
    );
    let mut results = Vec::new();
    ui.on_node_with_tag("field").perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    *held.borrow_mut() = results.pop();
    let before = drawn(&mut ui).len();
    show.set(true);
    ui.wait_for_idle();
    assert_eq!(drawn(&mut ui).len(), before + 2);
}

// Selection and semantics: the queries are the paragraphs', over the whole text.

/// "Hello", then "World" in a paragraph styled `second`, 200 px wide, in a selection container.
fn selectable(second: ParagraphStyle) -> ComposeTestRule {
    rule(move || {
        SelectionContainer!({
            BasicAnnotatedText!(
                hello_world(second.clone()),
                modifier = Modifier.test_tag(TAG).width(200.dp()),
                style = style()
            );
        });
    })
}

/// Asks the text's `Selectable`.
fn ask<R>(
    ui: &mut ComposeTestRule,
    question: impl FnOnce(
        &mut dyn compose_ui::Selectable,
        &compose_ui::platform::SharedTextMeasurer,
    ) -> R,
) -> R {
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().id;
    let mut question = Some(question);
    let mut answer = None;
    ui.host().with_tree(|tree| {
        tree.with_selectable(node, &mut |selectable, measurer| {
            answer = question.take().map(|question| question(selectable, measurer));
        });
    });
    answer.expect("a selectable text")
}

#[test]
fn the_semantics_layout_is_the_text_of_paragraphs() {
    let mut ui = selectable(ParagraphStyle::DEFAULT);
    let mut results = Vec::new();
    ui.on_node_with_tag(TAG).perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    let result = results.pop().expect("a layout");
    assert_eq!(result.multi_paragraph.paragraph_info_list().len(), 2);
    assert_eq!(result.size, IntSize::new(200, 40));
    assert_eq!(result.get_offset_for_position(Offset::new(25.0, 30.0)), 6);
    assert_eq!(result.get_bounding_box(6), compose_ui::Rect::new(20.0, 20.0, 40.0, 40.0));
    assert_eq!(result.get_line_for_offset(7), 1);
    // The text it reads is the whole text.
    assert_eq!(result.layout_input.text.as_str(), "HelloWorld");
    ui.on_node_with_tag(TAG).assert_text_equals(&["HelloWorld"]);
}

#[test]
fn a_selection_finds_offsets_and_boxes_in_each_paragraph() {
    let mut ui = selectable(ParagraphStyle::DEFAULT);
    let offset = ask(&mut ui, |text, measurer| text.offset_at(Offset::new(25.0, 30.0), measurer));
    assert_eq!(offset, Some(6));
    let first = ask(&mut ui, |text, measurer| text.offset_at(Offset::new(25.0, 5.0), measurer));
    assert_eq!(first, Some(1));
    let rect = ask(&mut ui, |text, measurer| text.bounding_box(6, measurer));
    assert_eq!(rect, compose_ui::Rect::new(20.0, 20.0, 40.0, 40.0));
    let cursor = ask(&mut ui, |text, measurer| text.cursor_rect(7, measurer));
    assert_eq!((cursor.left, cursor.top, cursor.bottom), (40.0, 20.0, 40.0));
    let line = ask(&mut ui, |text, measurer| text.range_of_line_containing(7, measurer));
    assert_eq!(line, 5..10);
    let line = ask(&mut ui, |text, measurer| text.range_of_line_containing(2, measurer));
    assert_eq!(line, 0..5);
    assert_eq!(ask(&mut ui, |text, measurer| text.line_height(7, measurer)), 20.0);
    assert_eq!(ask(&mut ui, |text, measurer| text.last_visible_offset(measurer)), 10);
    // A handle at the bottom of the line its offset is on.
    let handle = ask(&mut ui, |text, measurer| text.handle_position(7, false, false, measurer));
    assert_eq!(handle, Some(Offset::new(40.0, 40.0)));
}

#[test]
fn a_word_ends_at_its_paragraph() {
    // "HelloWorld" is one word in one paragraph; in two, "World" is a word of its own, as
    // Compose's `MultiParagraph.getWordBoundary`.
    let mut ui = selectable(ParagraphStyle::DEFAULT);
    assert_eq!(ask(&mut ui, |text, _| text.word_at(7)), 5..10);
    assert_eq!(ask(&mut ui, |text, _| text.word_at(2)), 0..5);
    // At the end of the text.
    assert_eq!(ask(&mut ui, |text, _| text.word_at(10)), 5..10);
}

#[test]
fn a_selection_runs_in_each_paragraphs_direction() {
    // "Hello", then "!!" in a right-to-left paragraph: neutral characters, which run in their
    // paragraph's direction (the same text in one paragraph runs left to right).
    let rtl = paragraph(|style| style.text_direction = Some(TextDirection::Rtl));
    let text = AnnotatedString::build(|text| {
        text.append("Hello");
        text.with_paragraph_style(rtl, |text| {
            text.append("!!");
        });
    });
    let mut ui = rule(move || {
        let text = text.clone();
        SelectionContainer!({
            BasicAnnotatedText!(text.clone(), modifier = Modifier.test_tag(TAG), style = style());
        });
    });
    let direction = |ui: &mut ComposeTestRule, offset| {
        ask(ui, |text, measurer| text.direction_at(offset, measurer))
    };
    assert_eq!(direction(&mut ui, 2), ResolvedTextDirection::Ltr);
    assert_eq!(direction(&mut ui, 6), ResolvedTextDirection::Rtl);
    // Past the end: kept to it.
    assert_eq!(direction(&mut ui, 99), direction(&mut ui, 7));
}

#[test]
fn a_selection_across_paragraphs_is_highlighted_in_each() {
    let mut ui = selectable(ParagraphStyle::DEFAULT);
    // A drag from after "Hel" to after "Wo".
    ui.host().send_pointer(compose_ui::PointerEvent::press(Offset::new(61.0, 10.0)));
    ui.host().send_pointer(compose_ui::PointerEvent::moved(Offset::new(39.0, 30.0), true));
    ui.host().send_pointer(compose_ui::PointerEvent::release(Offset::new(39.0, 30.0)));
    ui.wait_for_idle();
    let highlights: Vec<compose_ui::Rect> = ui
        .host()
        .display_list()
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::DrawRect(rect, _) => Some(*rect),
            _ => None,
        })
        .collect();
    // "lo" on the first line, "Wo" on the second, below it.
    assert_eq!(highlights.len(), 2, "{}", ui.host().display_list().dump());
    assert_eq!((highlights[0].left, highlights[0].top), (60.0, 0.0));
    assert_eq!((highlights[0].right, highlights[0].bottom), (100.0, 20.0));
    assert_eq!((highlights[1].left, highlights[1].top), (0.0, 20.0));
    assert_eq!((highlights[1].right, highlights[1].bottom), (40.0, 40.0));
}
