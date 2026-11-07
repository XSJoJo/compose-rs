//! Review of `multi-paragraph-2` (D2049 to D2052): a locale's direction (ICU's
//! `ULocale.isRightToLeft` over CLDR's likely subtags), `fill_path_for_range`, the constructors and
//! `paint`/`paint_with_brush` on `Paragraph` and `MultiParagraph`. Pictures are Skia's, in Compose's
//! test font (a square em a character, D1124). Failing tests are ignored with the finding they
//! show.

use std::cell::RefCell;
use std::rc::Rc;

use compose::foundation::layout::*;
use compose::foundation::*;
use compose::runtime::*;
use compose::ui::platform::{FixedFontMetrics, FixedTextMeasurer, SharedTextMeasurer, UiHost};
use compose::ui::{
    AnnotatedString, BlendMode, Brush, Color, Constraints, CornerRadius, Density, IntSize,
    LayoutDirection, LocaleList, Modifier, MultiParagraph, MultiParagraphIntrinsics, Paint,
    ParagraphStyle, Path, PathCommand, Rect, ResolvedTextDirection, Shadow, Size, TextDirection,
    TextLayoutResult, TextMeasureOptions, TextMeasurer, TextOverflow, TextStyle, UnitExt,
    create_font_family_resolver,
};
use compose_skia::SkiaTextMeasurer;

use super::text_painter::{draw_with, render};

// ---- Helpers ----

/// `style` in Compose's test font: "Lorem" is 100 px wide at 20 px, every line 20 px high.
fn sample(style: TextStyle) -> TextStyle {
    let font_family = Some(compose::test::basic_measure_font().to_font_family());
    TextStyle { font_family, font_size: 20.sp(), ..style }
}

/// `text` in `style` (in the test font), laid out `width` wide by the host's measurer.
fn measured(
    measurer: &TextMeasurer,
    text: AnnotatedString,
    style: TextStyle,
    width: i32,
) -> TextLayoutResult {
    let options = TextMeasureOptions {
        style: sample(style),
        constraints: Constraints::new(width, width, 0, Constraints::INFINITY),
        ..TextMeasureOptions::default()
    };
    measurer.measure(text, options)
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

fn gradient() -> Brush {
    Brush::vertical_gradient([Color::BLUE, Color::RED].as_slice())
}

/// Paints `text` laid out 100 px wide in `style` with `brush` at `alpha` into a 100 by 80 window.
fn painted_with_brush(
    text: AnnotatedString,
    style: TextStyle,
    brush: Brush,
    alpha: f32,
) -> super::text_painter::Picture {
    draw_with((100, 80), (100, 80), 1.0, 0, move |scope, measurer| {
        let result = measured(measurer, text.clone(), style.clone(), 100);
        result.multi_paragraph.paint_with_brush(
            scope,
            &brush,
            alpha,
            None,
            None,
            None,
            BlendMode::SrcOver,
        );
    })
}

// ---- D2052: painting a text's own result (focus 1) ----

/// Compose lets an app draw the `MultiParagraph` its `BasicText` laid out
/// (`layoutResult.multiParagraph.paint(canvas, …)` in a draw callback, after `onTextLayout`):
/// `TextLayoutResult.multiParagraph` is public and paints whatever laid it out. Here the paragraph
/// a text node hands `on_text_layout` only answers queries, and `paint` panics in the frame's
/// drawing, while `draw_text_layout` of the same result draws (it lays the text out again from
/// the result's input).
#[test]
fn a_text_s_own_multi_paragraph_paints_after_on_text_layout() {
    let kept: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
    let heard_owner = mutable_state_of(0);
    let heard = heard_owner.handle();
    let mut host =
        UiHost::new(IntSize::new(400, 200), Density::new(1.0), Box::new(SkiaTextMeasurer::new()));
    let (write, read) = (kept.clone(), kept.clone());
    host.set_content(move || {
        let (write, read) = (write.clone(), read.clone());
        Column!(modifier = Modifier.fill_max_size(), {
            let write = write.clone();
            BasicText!(
                "Hello",
                modifier = Modifier.height(20.0.dp()),
                on_text_layout = move |result: TextLayoutResult| {
                    *write.borrow_mut() = Some(result);
                    heard.set(1);
                }
            );
            let read = read.clone();
            Box!(
                modifier = Modifier.fill_max_size().draw_behind(move |scope| {
                    let _ = heard.get();
                    if let Some(result) = &*read.borrow() {
                        result.multi_paragraph.paint(
                            scope,
                            Color::RED,
                            None,
                            None,
                            None,
                            BlendMode::SrcOver,
                        );
                    }
                })
            );
        });
    });
    host.frame();
    host.frame();
    let picture = render(&mut host, (400, 200));
    assert!(picture.contains(Color::RED));
}

// ---- D2052: blend modes ----

/// Compose paints each paragraph with the blend mode in its paint: Skia blends a glyph's paint
/// only where the glyph covers, so `Src` replaces what's under the glyphs and leaves the rest.
/// Here the paragraphs are drawn into a layer as big as the drawing (or the text), composited
/// with `Src` as a whole: the layer's transparent pixels replace everything around the glyphs too.
/// Likewise `Clear`, `SrcIn`, `DstIn`, `SrcOut` and `DstATop`.
#[test]
#[ignore = "review: a non-SrcOver blend mode composites a layer the drawing's size, so `Src` (and Clear, SrcIn, DstIn, SrcOut, DstATop) wipes what's around the glyphs; Compose blends each glyph's paint only where it covers"]
fn painting_with_src_replaces_only_what_the_glyphs_cover() {
    let picture = draw_with((100, 80), (100, 80), 1.0, 0, move |scope, measurer| {
        scope.draw_rect(Rect::new(0.0, 0.0, 100.0, 80.0), Paint::fill(Color::RED));
        let result = measured(measurer, paragraphs(&["Lorem", "Ipsum"]), TextStyle::DEFAULT, 100);
        result.multi_paragraph.paint(scope, Color::BLACK, None, None, None, BlendMode::Src);
    });
    assert!(picture.contains(Color::BLACK));
    // Below the text (40 px high) nothing was drawn: still red.
    assert_eq!(picture.pixel(50, 70), Color::RED);
}

// ---- D2052: gradients ----

/// A paragraph of a text's list painted with a gradient alone, as Compose's
/// `ParagraphInfo.paragraph.paint(brush)`: the gradient over its own size, as the same text in a
/// text of its own drawn by `draw_text_layout_with_brush` (which `paint`'s spanning never touches).
#[test]
fn a_paragraph_of_the_list_painted_alone_spans_its_gradient_over_itself() {
    let alone = draw_with((100, 80), (100, 80), 1.0, 0, move |scope, measurer| {
        let result = measured(measurer, paragraphs(&["Lorem", "Ipsum"]), TextStyle::DEFAULT, 100);
        let info = &result.multi_paragraph.paragraph_info_list()[1];
        info.paragraph.paint_with_brush(
            scope,
            &gradient(),
            f32::NAN,
            None,
            None,
            None,
            BlendMode::SrcOver,
        );
    });
    let own = draw_with((100, 80), (100, 80), 1.0, 0, move |scope, measurer| {
        let result = measured(measurer, AnnotatedString::new("Ipsum"), TextStyle::DEFAULT, 100);
        scope.draw_text_layout_with_brush(&result, gradient(), Default::default());
    });
    assert!(!alone.is_blank());
    assert_eq!(alone, own);
}

/// Over three paragraphs the gradient is still the whole text's: Compose's `drawMultiParagraph`
/// sets each shader's local matrix to the previous paragraph's height only (`setTranslate`, not
/// the sum), off from the third paragraph on; compose-rs places it at each paragraph's top, the
/// picture of the same lines in one paragraph (a Compose bug not copied, worth a line in D2052).
#[test]
fn a_gradient_spans_three_paragraphs_as_one() {
    let style = TextStyle { brush: Some(gradient()), ..TextStyle::DEFAULT };
    let three = painted_with_brush(
        paragraphs(&["Lorem", "Ipsum", "Dolor"]),
        style.clone(),
        gradient(),
        1.0,
    );
    let lines =
        painted_with_brush(AnnotatedString::new("Lorem\nIpsum\nDolor"), style, gradient(), 1.0);
    assert!(!three.is_blank());
    assert!(three == lines, "the third paragraph's gradient is off");
}

/// A radial gradient (its centre and radius the text's by default) spans every paragraph too.
#[test]
fn a_radial_gradient_spans_every_paragraph() {
    let radial = || Brush::radial_gradient([Color::BLUE, Color::RED].as_slice());
    let two =
        painted_with_brush(paragraphs(&["Lorem", "Ipsum"]), TextStyle::DEFAULT, radial(), 1.0);
    let lines =
        painted_with_brush(AnnotatedString::new("Lorem\nIpsum"), TextStyle::DEFAULT, radial(), 1.0);
    assert!(!two.is_blank());
    assert!(two == lines);
}

/// A text of one paragraph style over all of it is one paragraph: its gradient over its size,
/// as the plain text's (the spanning changes nothing for one paragraph).
#[test]
fn a_text_of_one_paragraph_style_draws_its_gradient_as_a_plain_text() {
    let style = TextStyle { brush: Some(gradient()), ..TextStyle::DEFAULT };
    let draw = |text: AnnotatedString| {
        let style = style.clone();
        draw_with((100, 80), (100, 80), 1.0, 0, move |scope, measurer| {
            let options = compose::ui::DrawTextOptions {
                style: sample(style.clone()),
                size: Size::new(100.0, 80.0),
                ..Default::default()
            };
            scope.draw_text(measurer, text.clone(), options);
        })
    };
    let one = draw(paragraphs(&["Lorem\nIpsum"]));
    let plain = draw(AnnotatedString::new("Lorem\nIpsum"));
    assert!(!one.is_blank());
    assert!(one == plain);
}

/// A gradient painted without an alpha (NaN) keeps the paint's: on Android the text paint's
/// alpha, set by the style's color (`setColor`) when the style has no brush. A half transparent
/// color is as the gradient at half alpha.
#[test]
fn a_gradient_painted_without_an_alpha_keeps_the_style_color_s() {
    let half = TextStyle { color: Color::BLACK.with_alpha(0.5), ..TextStyle::DEFAULT };
    let kept = painted_with_brush(paragraphs(&["Lorem", "Ipsum"]), half, gradient(), f32::NAN);
    let given =
        painted_with_brush(paragraphs(&["Lorem", "Ipsum"]), TextStyle::DEFAULT, gradient(), 0.5);
    assert!(!kept.is_blank());
    assert!(kept == given);
}

/// A shadow painted is drawn over the paragraph's own (Android's `setShadow` with one given).
#[test]
fn a_shadow_painted_replaces_the_style_s() {
    let shadow = |color: Color| Shadow {
        color,
        offset: compose::ui::Offset::new(0.0, 30.0),
        blur_radius: 0.0,
    };
    let picture = draw_with((100, 80), (100, 80), 1.0, 0, move |scope, measurer| {
        let style = TextStyle { shadow: Some(shadow(Color::BLUE)), ..TextStyle::DEFAULT };
        let result = measured(measurer, paragraphs(&["Lorem", "Ipsum"]), style, 100);
        result.multi_paragraph.paint(
            scope,
            Color::BLACK,
            Some(shadow(Color::RED)),
            None,
            None,
            BlendMode::SrcOver,
        );
    });
    assert!(picture.contains(Color::RED));
    assert!(!picture.contains(Color::BLUE));
}

/// `paint_with_brush` with an unspecified solid color: Android's `setBrush(SolidColor)` sets the
/// color, which an unspecified one leaves as it was (`setColor` ignores it), so the style's red
/// stays.
#[test]
#[ignore = "review: `paint_with_brush` with `Brush::Solid(Color::UNSPECIFIED)` paints over the paragraph's own color with an unspecified one (drawn black); Android's `setColor` ignores an unspecified color"]
fn an_unspecified_solid_brush_keeps_the_paragraph_s_color() {
    let style = TextStyle { color: Color::RED, ..TextStyle::DEFAULT };
    let picture = painted_with_brush(
        paragraphs(&["Lorem", "Ipsum"]),
        style,
        Brush::Solid(Color::UNSPECIFIED),
        f32::NAN,
    );
    assert!(picture.contains(Color::RED));
}

// ---- D2050: fill_path_for_range ----

/// `text` laid out by the fixed measurer, 10 px characters and lines, `width` wide.
fn fixed_layout(
    text: impl Into<AnnotatedString>,
    style: TextStyle,
    width: i32,
) -> TextLayoutResult {
    let resolver = create_font_family_resolver(&SharedTextMeasurer::new(Box::new(
        FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT),
    )));
    let measurer = TextMeasurer::new(
        resolver,
        LocaleList::from_language_tags("en"),
        Density::new(1.0),
        LayoutDirection::Ltr,
        0,
    );
    let style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT }.merge(&style);
    measurer.measure(
        text,
        TextMeasureOptions {
            style,
            constraints: Constraints::new(width, width, 0, Constraints::INFINITY),
            ..TextMeasureOptions::default()
        },
    )
}

fn path_of(layout: &TextLayoutResult, start: usize, end: usize, radius: CornerRadius) -> Path {
    let path = Path::new();
    layout.fill_path_for_range(start, end, &path, radius, true);
    path
}

fn moves(path: &Path) -> usize {
    path.data().commands().iter().filter(|c| matches!(c, PathCommand::MoveTo(_))).count()
}

/// Boxes of consecutive lines that only touch (one's right the other's left) don't overlap
/// across: Compose's `max(left) < min(right)` keeps them two rectangles.
#[test]
fn boxes_of_consecutive_lines_that_only_touch_are_not_merged() {
    let layout = fixed_layout("abcdef\nabcdef", TextStyle::DEFAULT, 100);
    // "def" of line 0 is 30..60, "abc" of line 1 is 0..30.
    let boxes = layout.get_bounding_boxes_for_range(3, 10);
    assert_eq!(boxes, vec![Rect::new(30.0, 0.0, 60.0, 10.0), Rect::new(0.0, 10.0, 30.0, 20.0)]);
    assert_eq!(moves(&path_of(&layout, 3, 10, CornerRadius::ZERO)), 2);
    // One pixel more of line 1 overlaps: one outline.
    assert_eq!(moves(&path_of(&layout, 3, 11, CornerRadius::ZERO)), 1);
}

/// A radius only across is a radius (Compose's `x > 0 || y > 0`): the corners are curves.
#[test]
fn a_radius_only_across_rounds_the_corners() {
    let layout = fixed_layout("abcdef\nabcd", TextStyle::DEFAULT, 100);
    let length = layout.layout_input.text.len();
    let path = path_of(&layout, 0, length, CornerRadius::new(4.0, 0.0));
    let curves =
        path.data().commands().iter().filter(|c| matches!(c, PathCommand::QuadTo(..))).count();
    assert_eq!(curves, 6);
}

/// Lines of consecutive paragraphs are consecutive lines: merged into one outline, as Compose's
/// (its line numbers are the whole text's).
#[test]
fn lines_of_consecutive_paragraphs_merge() {
    let layout = fixed_layout(paragraphs(&["abcdef", "abcd"]), TextStyle::DEFAULT, 100);
    let length = layout.layout_input.text.len();
    let path = path_of(&layout, 0, length, CornerRadius::ZERO);
    assert_eq!(moves(&path), 1);
    assert_eq!(path.get_bounds(), Rect::new(0.0, 0.0, 60.0, 20.0));
}

/// Right to left lines are right aligned: the outline steps on the left.
#[test]
fn right_to_left_lines_merge_stepping_on_the_left() {
    let style = TextStyle { text_direction: Some(TextDirection::Rtl), ..TextStyle::DEFAULT };
    let text = "\u{05D0}\u{05D1}\u{05D2}\u{05D3}\n\u{05D0}\u{05D1}";
    let layout = fixed_layout(text, style, 100);
    let path = path_of(&layout, 0, text.len(), CornerRadius::ZERO);
    use PathCommand::{Close, LineTo, MoveTo};
    let point = compose::ui::Offset::new;
    assert_eq!(
        path.data().commands(),
        [
            MoveTo(point(60.0, 0.0)),
            LineTo(point(100.0, 0.0)),
            LineTo(point(100.0, 20.0)),
            LineTo(point(80.0, 20.0)),
            LineTo(point(80.0, 10.0)),
            LineTo(point(60.0, 10.0)),
            Close,
        ]
    );
}

#[test]
#[should_panic(expected = "out of range")]
fn a_path_for_a_range_past_the_text_panics() {
    let layout = fixed_layout("abc", TextStyle::DEFAULT, 100);
    layout.fill_path_for_range(0, 4, &Path::new(), CornerRadius::ZERO, true);
}

// ---- D2049: a locale's direction against ICU ----

/// The direction a neutral paragraph whose direction is `Content` takes in `locale`.
fn direction_in(locale: &str) -> ResolvedTextDirection {
    let resolver =
        create_font_family_resolver(&SharedTextMeasurer::new(Box::new(FixedTextMeasurer::new())));
    let style = TextStyle {
        text_direction: Some(TextDirection::Content),
        locale_list: Some(LocaleList::from_language_tags(locale)),
        ..TextStyle::DEFAULT
    };
    let intrinsics = MultiParagraphIntrinsics::new(
        AnnotatedString::new("1"),
        &style,
        &[],
        Density::new(1.0),
        resolver,
        true,
        LocaleList::from_language_tags("en"),
    );
    MultiParagraph::new(Rc::new(intrinsics), Constraints::unbounded(), u32::MAX, TextOverflow::Clip)
        .get_paragraph_direction(0)
}

/// Underscores, any case: a tag is a tag.
#[test]
fn a_locale_s_direction_reads_its_tag_in_any_case_and_separator() {
    for locale in
        ["AR_eg", "Ur-pk", "PA_pk", "pa-Arab-IN", "UND-eg", "ckb_IR", "ms-ARAB", "ff-adlm-GN"]
    {
        assert_eq!(direction_in(locale), ResolvedTextDirection::Rtl, "{locale}");
    }
    for locale in ["SD_in", "ks-Deva", "uz-Cyrl-AF", "az-Latn-IR", "ku_TR", "EN_ae"] {
        assert_eq!(direction_in(locale), ResolvedTextDirection::Ltr, "{locale}");
    }
}

/// Languages and regions whose likely script is right to left in ICU's own likely subtags
/// (ICU 68.2's `likelySubtags.txt`, in androidx-main's `external/icu`; CLDR 38) that the table
/// misses: Comorian's two (`zdj`, `wni`; `swb` is in), Kurdish of Lebanon and Iran (`ku_LB`,
/// `ku_IR`), Kazakh of Mongolia (`kk_MN`), Adamawa Fulfulde (`fub_Arab_CM`), and the
/// macro-regions of North Africa and Western Asia (`und_015`, `und_145`).
#[test]
#[ignore = "review: the likely-script table misses zdj, wni, fub, ku-LB, ku-IR, kk-MN, und-015, und-145 (right to left in ICU's likely subtags)"]
fn locales_right_to_left_in_icu_s_likely_subtags_are_right_to_left() {
    for locale in ["zdj", "wni", "fub", "ku-LB", "ku-IR", "kk-MN", "und-015", "und-145"] {
        assert_eq!(direction_in(locale), ResolvedTextDirection::Rtl, "{locale}");
    }
}

/// ICU's `UScript.isRightToLeft` is false for the script variants without characters of their
/// own (Nastaliq `Aran`, the Syriac variants `Syre`, `Syrj`, `Syrn`, Book Pahlavi `Phlv`): their
/// script properties are 0 in `uscript_props.cpp` (ICU 68.2; `Aran` isn't even a script code
/// there). So `ur-Aran` is left to right on Android; D2049 lists them as right to left.
#[test]
#[ignore = "review: Aran, Syre, Syrj, Syrn and Phlv are right to left here; ICU's `UScript.isRightToLeft` says not (no script properties), so Android lays a Content paragraph in `ur-Aran` out left to right"]
fn script_variants_without_properties_are_left_to_right_as_icu_s() {
    for locale in ["ur-Aran", "syr-Syre", "syr-Syrj", "syr-Syrn", "pal-Phlv"] {
        assert_eq!(direction_in(locale), ResolvedTextDirection::Ltr, "{locale}");
    }
}
