//! Compose's `ParagraphLayoutCacheTest` (foundation, `text/modifiers`): a plain string text's
//! layout cache, through a `BasicText` of that string and its node (D550): its intrinsics and
//! min-lines intrinsic height, laid out again for another height, density or font scale, its
//! ellipses, the width letter spacing gives it in either direction, its maximum lines' height
//! against one `Paragraph`'s, its layout result once laid out, a huge string, and intrinsics
//! asked after a layout keeping that layout. `basic_text_paragraphs.rs` does the same for
//! `MultiParagraphLayoutCacheTest`.
//!
//! In Compose's test font (each character a square of its size, D1124).
//!
//! n/a: `slowCreate_null_beforeLayout` (the cache's internal `slowCreateTextLayoutResultOrNull`
//! before any layout: a compose-rs text is laid out before its semantics can be asked) and
//! `history_isRecorded` (the cache's internal debug `historyFlag`),
//! `minIntrinsicWidth_withDifferentLayoutDirection_doesNotClearParagraph_afterLayout` (the cache
//! asked its intrinsics in another direction than its layout's: a compose-rs node's intrinsics
//! are asked in its own layout direction).

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, SharedTextMeasurer};
use compose_ui::{
    Constraints, Density, IntSize, IntrinsicSize, LayoutDirection, LocaleList, SemanticsActions,
    TextLayoutResult, TextOverflow, UnitExt,
};

const TAG: &str = "text";
const FONT: f32 = 20.0;

fn style(font_size: f32) -> TextStyle {
    TextStyle { font_size: font_size.sp(), ..TextStyle::DEFAULT }
}

/// A rule in Compose's test font at density 1.
fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(800, 800),
        Density::new(1.0),
        std::boxed::Box::new(measurer),
        content,
    );
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

/// How the text is laid out.
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
            style: style(FONT),
            max_lines: u32::MAX,
            min_lines: 1,
            overflow: TextOverflow::Clip,
            soft_wrap: true,
        }
    }
}

/// `text` laid out with `options`, tagged [`TAG`], and what `on_text_layout` heard.
fn laid_out(text: &'static str, options: Options) -> (ComposeTestRule, Heard) {
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let options = Rc::new(options);
    let ui = rule(move || {
        let callback = callback.clone();
        BasicText!(
            text,
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

fn bounds(ui: &mut ComposeTestRule) -> IntSize {
    let bounds = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    IntSize::new(bounds.width() as i32, bounds.height() as i32)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, minIntrinsicWidth_getter
#[test]
fn minIntrinsicWidth_getter() {
    let text = "Hello";
    let (mut ui, _) = laid_out(
        text,
        Options { modifier: Modifier.width(IntrinsicSize::Min), ..Default::default() },
    );
    assert_eq!(bounds(&mut ui).width, (FONT * text.len() as f32).ceil() as i32);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, maxIntrinsicWidth_getter
#[test]
fn maxIntrinsicWidth_getter() {
    let text = "Hello";
    let (mut ui, _) = laid_out(
        text,
        Options { modifier: Modifier.width(IntrinsicSize::Max), ..Default::default() },
    );
    assert_eq!(bounds(&mut ui).width, (FONT * text.len() as f32).ceil() as i32);
}

/// The text's height through `height(IntrinsicSize::Min)` at 200 px wide.
fn intrinsic_height(text: &'static str, min_lines: u32) -> i32 {
    let modifier = Modifier.width(200.dp()).height(IntrinsicSize::Min);
    let (mut ui, _) = laid_out(text, Options { modifier, min_lines, ..Default::default() });
    bounds(&mut ui).height
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, minIntrinsicsHeight_respectsMinLines
#[test]
fn minIntrinsicsHeight_respectsMinLines() {
    let single = intrinsic_height("A", 1);
    assert_eq!(intrinsic_height("A", 3), single * 3);
}

/// `text` measured with `first`, then with `second`: what was heard each time.
fn relaid(
    text: &'static str,
    options: Options,
    first: Constraints,
    second: Constraints,
    direction: impl Fn(bool) -> LayoutDirection + 'static,
) -> (TextLayoutResult, TextLayoutResult) {
    let second_pass = mutable_state_of(false);
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = second_pass.handle();
    let options = Rc::new(options);
    let direction = Rc::new(direction);
    let mut ui = rule(move || {
        let (callback, options) = (callback.clone(), options.clone());
        let direction = direction(handle.get());
        CompositionLocalProvider(
            compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
            move || {
                let callback = callback.clone();
                BasicText!(
                    text,
                    modifier = Modifier.layout(move |scope, measurable, _| {
                        let constraints = if handle.get() { second } else { first };
                        let placeable = scope.measure(measurable, constraints);
                        scope.place(&placeable, 0, 0);
                        scope.layout(placeable.width, placeable.height)
                    }),
                    style = options.style.clone(),
                    on_text_layout = Some(move |result| callback(result)),
                    overflow = options.overflow,
                    soft_wrap = options.soft_wrap,
                    max_lines = options.max_lines
                );
            },
        );
    });
    let before = heard.last();
    second_pass.set(true);
    ui.wait_for_idle();
    (before, heard.last())
}

fn ltr(_: bool) -> LayoutDirection {
    LayoutDirection::Ltr
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutInput_reLayout_withDifferentHeight
#[test]
fn TextLayoutInput_reLayout_withDifferentHeight() {
    let (first, second) = (Constraints::fixed(200, 100), Constraints::fixed(200, 200));
    let (before, after) = relaid("Hello World", Options::default(), first, second, ltr);
    assert!(before.size.height < after.size.height, "{:?} {:?}", before.size, after.size);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutResult_reLayout_withDifferentHeight
#[test]
fn TextLayoutResult_reLayout_withDifferentHeight() {
    let (first, second) = (Constraints::fixed(200, 100), Constraints::fixed(200, 200));
    let (before, after) = relaid("Hello World", Options::default(), first, second, ltr);
    assert_eq!(before.size.height, 100);
    assert_eq!(after.size.height, 200);
}

/// "Hello World" laid out at `density` after density 1: wider and taller.
fn grows_with(density: Density) {
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let mut ui = rule(move || {
        let callback = callback.clone();
        BasicText!(
            "Hello World",
            style = TextStyle::DEFAULT,
            on_text_layout = Some(move |result| callback(result))
        );
    });
    let before = heard.last().size;
    ui.host().set_density(density);
    ui.wait_for_idle();
    let after = heard.last().size;
    assert!(before.width < after.width && before.height < after.height, "{before:?} {after:?}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutInput_reLayout_withDifferentDensity
#[test]
fn TextLayoutInput_reLayout_withDifferentDensity() {
    grows_with(Density::new(2.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutInput_reLayout_withDifferentFontScale
#[test]
fn TextLayoutInput_reLayout_withDifferentFontScale() {
    grows_with(Density::with_font_scale(1.0, 2.0));
}

const LONG: &str = "Hello World! Hello World! Hello World! Hello World!";

/// `LONG` on one line, without soft wrap, in half its width: its line ellipsized by `overflow`.
fn ellipsized_without_soft_wrap(overflow: TextOverflow) {
    let options = Options { soft_wrap: false, overflow, ..Default::default() };
    let width = (FONT * LONG.len() as f32) as i32 / 2;
    let (_, result) = relaid(
        LONG,
        options,
        Constraints::fixed(0, 0),
        Constraints::new(0, width, 0, Constraints::INFINITY),
        ltr,
    );
    assert_eq!(result.line_count, 1);
    assert!(result.is_line_ellipsized(0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutResult_layout_withEllipsis_withoutSoftWrap
#[test]
fn TextLayoutResult_layout_withEllipsis_withoutSoftWrap() {
    ellipsized_without_soft_wrap(TextOverflow::Ellipsis);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutResult_layout_withStartEllipsis_withoutSoftWrap
#[test]
fn TextLayoutResult_layout_withStartEllipsis_withoutSoftWrap() {
    ellipsized_without_soft_wrap(TextOverflow::StartEllipsis);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutResult_layout_withMiddleEllipsis_withoutSoftWrap
#[test]
fn TextLayoutResult_layout_withMiddleEllipsis_withoutSoftWrap() {
    ellipsized_without_soft_wrap(TextOverflow::MiddleEllipsis);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutResult_layoutWithLimitedHeight_withEllipsis
#[test]
fn TextLayoutResult_layoutWithLimitedHeight_withEllipsis() {
    let options = Options { overflow: TextOverflow::Ellipsis, ..Default::default() };
    let width = (FONT * LONG.len() as f32) as i32 / 4;
    // Two lines fit wholly, at most.
    let height = (FONT * 2.7).round() as i32;
    let (_, result) =
        relaid(LONG, options, Constraints::fixed(0, 0), Constraints::new(0, width, 0, height), ltr);
    assert_eq!(result.line_count, 2);
    assert!(result.is_line_ellipsized(1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, TextLayoutResult_sameWidth_inRtlAndLtr_withLetterSpacing
#[test]
fn TextLayoutResult_sameWidth_inRtlAndLtr_withLetterSpacing() {
    let options = Options {
        style: TextStyle { letter_spacing: 0.5.sp(), ..style(FONT) },
        overflow: TextOverflow::Ellipsis,
        ..Default::default()
    };
    // Laid out left to right, then right to left.
    let unbounded = Constraints::unbounded();
    let (ltr, rtl) = relaid("Hello World", options, unbounded, unbounded, |second| {
        if second { LayoutDirection::Rtl } else { LayoutDirection::Ltr }
    });
    assert_eq!(rtl.layout_input.layout_direction, LayoutDirection::Rtl);
    assert_eq!(ltr.size.width, rtl.size.width);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, maxHeight_hasSameHeight_asParagraph
#[test]
fn maxHeight_hasSameHeight_asParagraph() {
    let text: &'static str = "a\n".repeat(20).leak();
    let (_ui, heard) = laid_out(
        text,
        Options {
            style: style(1.0),
            overflow: TextOverflow::Ellipsis,
            max_lines: 5,
            ..Default::default()
        },
    );
    let actual = heard.last().multi_paragraph.height();
    let resolver = compose_ui::create_font_family_resolver(&SharedTextMeasurer::new(
        std::boxed::Box::new(FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT)),
    ));
    let expected = compose_ui::paragraph_of_text(
        text,
        &style(1.0),
        Constraints::unbounded(),
        Density::new(1.0),
        resolver,
        LocaleList::from_language_tags("en"),
        &[],
        &[],
        5,
        TextOverflow::Ellipsis,
    );
    assert_eq!(actual, expected.height());
}

/// The text's layout result through its semantics, as `slowCreateTextLayoutResultOrNull` (the
/// `GetTextLayoutResult` action's).
fn semantics_layout_results(ui: &mut ComposeTestRule) -> Vec<TextLayoutResult> {
    let mut results = Vec::new();
    ui.on_node_with_tag(TAG).perform_semantics_action_with(
        &SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    results
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, slowCreate_not_null_afterLayout
#[test]
fn slowCreate_not_null_afterLayout() {
    let (mut ui, _) = laid_out("hello", Options { style: style(1.0), ..Default::default() });
    assert_eq!(semantics_layout_results(&mut ui).len(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, slowCreate_not_null_afterLayout_minWidthMinHeight
#[test]
fn slowCreate_not_null_afterLayout_minWidthMinHeight() {
    let modifier = Modifier.layout(|scope, measurable, _| {
        let placeable = scope.measure(
            measurable,
            Constraints::new(5, Constraints::INFINITY, 5, Constraints::INFINITY),
        );
        scope.place(&placeable, 0, 0);
        scope.layout(placeable.width, placeable.height)
    });
    let (mut ui, _) =
        laid_out("hello", Options { modifier, style: style(1.0), ..Default::default() });
    assert_eq!(semantics_layout_results(&mut ui).len(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, hugeString_doesntCrash
#[test]
fn hugeString_doesntCrash() {
    let text: &'static str = "A".repeat(100_000).leak();
    let (_ui, heard) = laid_out(text, Options { style: style(100.0), ..Default::default() });
    assert!(heard.count() > 0);
}

/// A parent measuring the text in each pass, then asking its minimum intrinsic width: the text
/// isn't laid out again (its layout is kept).
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/modifiers/ParagraphLayoutCacheTest.kt, minIntrinsicWidth_doesNotClearParagraph_afterLayout
#[test]
fn minIntrinsicWidth_doesNotClearParagraph_afterLayout() {
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let asked = Rc::new(Cell::new(0));
    let a = asked.clone();
    let pass = mutable_state_of(0);
    let p = pass.handle();
    let mut ui = rule(move || {
        let (callback, a, p) = (callback.clone(), a.clone(), p);
        BasicText!(
            "hello",
            modifier = Modifier.layout(move |scope, measurable, constraints| {
                // Measured again in each pass.
                let _ = p.get();
                let placeable = scope.measure(measurable, constraints);
                a.set(scope.min_intrinsic_width(measurable, Constraints::INFINITY));
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }),
            style = style(1.0),
            on_text_layout = Some(move |result| callback(result))
        );
    });
    assert_eq!(asked.get(), 5);
    let first = heard.last();
    pass.set(1);
    ui.wait_for_idle();
    assert_eq!(asked.get(), 5);
    assert_eq!(heard.count(), 1);
    assert!(Rc::ptr_eq(&heard.last().multi_paragraph, &first.multi_paragraph));
}
