//! A text of many paragraphs measured and drawn through Skia (D2047): each paragraph laid out
//! in its own style at the widest one's width, drawn below the one before.

use compose::ui::{
    AnnotatedString, Color, DrawTextLayoutOptions, DrawTextOptions, MultiParagraph, Offset,
    ParagraphStyle, Size, TextAlign, TextMeasureOptions, TextStyle, UnitExt,
};

use super::text_painter::draw_with;

fn style() -> TextStyle {
    TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT }
}

/// "Lorem ipsum" in its own paragraph, then "dolor" aligned right.
fn two_paragraphs() -> AnnotatedString {
    AnnotatedString::build(|builder| {
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("Lorem ipsum");
        });
        let right =
            ParagraphStyle { text_align: Some(TextAlign::Right), ..ParagraphStyle::DEFAULT };
        builder.with_paragraph_style(right, |builder| {
            builder.append("dolor");
        });
    })
}

#[test]
fn a_text_of_paragraphs_draws_each_below_the_one_before_in_its_style() {
    let together = draw_with((300, 120), (300, 120), 1.0, 0, |scope, measurer| {
        scope.draw_text(
            measurer,
            two_paragraphs(),
            DrawTextOptions { style: style(), ..DrawTextOptions::default() },
        );
    });
    let apart = draw_with((300, 120), (300, 120), 1.0, 0, |scope, measurer| {
        let options = || TextMeasureOptions { style: style(), ..TextMeasureOptions::default() };
        let first = measurer.measure("Lorem ipsum", options());
        let width = first.size.width as f32;
        let height = first.multi_paragraph.height();
        scope.draw_text_layout(&first, Color::UNSPECIFIED, DrawTextLayoutOptions::default());
        let right = TextStyle { text_align: Some(TextAlign::Right), ..style() };
        scope.draw_text(
            measurer,
            "dolor",
            DrawTextOptions {
                style: right,
                top_left: Offset::new(0.0, height),
                size: Size::new(width, f32::NAN),
                ..DrawTextOptions::default()
            },
        );
    });
    assert!(!together.is_blank());
    assert_eq!(together, apart);
}

#[test]
fn a_text_of_paragraphs_measures_as_its_widest_and_all_its_lines() {
    let _ = draw_with((300, 120), (300, 120), 1.0, 0, |_, measurer| {
        let options = || TextMeasureOptions { style: style(), ..TextMeasureOptions::default() };
        let first = measurer.measure("Lorem ipsum", options());
        let second = measurer.measure("dolor", options());
        let both = measurer.measure(two_paragraphs(), options());
        let paragraph: &MultiParagraph = &both.multi_paragraph;
        assert_eq!(paragraph.paragraph_info_list().len(), 2);
        assert_eq!(both.size.width, first.size.width);
        let height = first.multi_paragraph.height() + second.multi_paragraph.height();
        assert_eq!(paragraph.height(), height);
        assert_eq!(both.size.height, height.ceil() as i32);
        assert_eq!(both.line_count, 2);
        let top = first.multi_paragraph.height() + second.multi_paragraph.get_line_top(0);
        assert_eq!(paragraph.get_line_top(1), top);
        // "dolor" ends at the right edge.
        assert_eq!(paragraph.get_line_right(1), first.size.width as f32);
        assert_eq!(both.last_baseline, first.multi_paragraph.height() + second.last_baseline);
    });
}

// ---- `paint` (D2052) ----

use compose::ui::{
    BlendMode, Brush, Constraints, DrawStyle, SpanStyle, TextDecoration, TextMeasurer,
};

/// `text` in `style`, laid out `width` wide by the host's measurer: Compose's
/// `simpleMultiParagraph`.
fn measured(
    measurer: &TextMeasurer,
    text: AnnotatedString,
    style: TextStyle,
    width: i32,
) -> std::rc::Rc<MultiParagraph> {
    let options = TextMeasureOptions {
        style: sample(style),
        constraints: Constraints::new(width, width, 0, Constraints::INFINITY),
        ..TextMeasureOptions::default()
    };
    measurer.measure(text, options).multi_paragraph
}

/// "Lorem" aligned right in a paragraph of its own, then "Ipsum".
fn lorem_ipsum() -> AnnotatedString {
    AnnotatedString::build(|builder| {
        let right =
            ParagraphStyle { text_align: Some(TextAlign::Right), ..ParagraphStyle::DEFAULT };
        builder.with_paragraph_style(right, |builder| {
            builder.append("Lorem");
        });
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("Ipsum");
        });
    })
}

/// `style` in Compose's test font, a square em a character (D1124): "Lorem" fills 100 px at
/// 20 px, and every line is 20 px high, as one paragraph's lines or many paragraphs.
fn sample(style: TextStyle) -> TextStyle {
    let font_family = Some(compose::test::basic_measure_font().to_font_family());
    TextStyle { font_family, ..style }
}

fn gradient() -> Brush {
    Brush::vertical_gradient([Color::BLUE, Color::RED].as_slice())
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, drawText_withUnderlineStyle_equalToUnderlinePaint
#[test]
fn an_underline_painted_is_an_underline_span() {
    let text = "\u{30EC}\u{30F3}\u{30BA}(\u{5358}\u{7126}\u{70B9})";
    let style = TextStyle { font_size: 30.sp(), ..TextStyle::DEFAULT };
    let spanned = {
        let style = style.clone();
        draw_with((600, 100), (600, 100), 1.0, 0, move |scope, measurer| {
            let underline = SpanStyle {
                text_decoration: Some(TextDecoration::UNDERLINE),
                ..Default::default()
            };
            let text = AnnotatedString::with_styles(
                text.to_string(),
                [compose::ui::AnnotatedRange::new(underline, 0, text.len())],
                [],
            );
            let paragraph = measured(measurer, text, style.clone(), 600);
            paragraph.paint(scope, Color::UNSPECIFIED, None, None, None, BlendMode::SrcOver);
        })
    };
    let painted = draw_with((600, 100), (600, 100), 1.0, 0, move |scope, measurer| {
        let style = TextStyle { text_decoration: Some(TextDecoration::UNDERLINE), ..style.clone() };
        let paragraph = measured(measurer, AnnotatedString::new(text), style, 600);
        let underline = Some(TextDecoration::UNDERLINE);
        paragraph.paint(scope, Color::UNSPECIFIED, None, underline, None, BlendMode::SrcOver);
    });
    assert!(!spanned.is_blank());
    assert_eq!(spanned, painted);
}

/// A gradient over paragraphs is one gradient over the whole text: as a text of the same lines
/// in one paragraph.
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, multiParagraph_appliesBrush_toTheWholeText
#[test]
fn a_gradient_painted_spans_every_paragraph() {
    let style = TextStyle { brush: Some(gradient()), font_size: 20.sp(), ..TextStyle::DEFAULT };
    let paragraphs = {
        let style = style.clone();
        draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
            let paragraph = measured(measurer, lorem_ipsum(), style.clone(), 100);
            paragraph.paint_with_brush(
                scope,
                &gradient(),
                f32::NAN,
                None,
                None,
                None,
                BlendMode::SrcOver,
            );
        })
    };
    let lines = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let paragraph =
            measured(measurer, AnnotatedString::new("Lorem\nIpsum"), style.clone(), 100);
        paragraph.paint_with_brush(
            scope,
            &gradient(),
            f32::NAN,
            None,
            None,
            None,
            BlendMode::SrcOver,
        );
    });
    assert!(!paragraphs.is_blank());
    assert_eq!(paragraphs, lines);
}

/// A text drawn by its measurer in a gradient spans every paragraph too.
#[test]
fn a_text_of_paragraphs_drawn_in_a_gradient_spans_every_paragraph() {
    let style = TextStyle { brush: Some(gradient()), font_size: 20.sp(), ..TextStyle::DEFAULT };
    let draw = |text: AnnotatedString, style: TextStyle| {
        draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
            let options = DrawTextOptions {
                style: sample(style.clone()),
                size: Size::new(100.0, 60.0),
                ..DrawTextOptions::default()
            };
            scope.draw_text(measurer, text.clone(), options);
        })
    };
    let paragraphs = draw(lorem_ipsum(), style.clone());
    let lines = draw(AnnotatedString::new("Lorem\nIpsum"), style);
    assert_eq!(paragraphs, lines);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, multiParagraph_overridesAlphaDuringDraw
#[test]
fn a_gradient_painted_without_an_alpha_keeps_the_style_s() {
    let paragraphs = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let style = TextStyle {
            brush: Some(gradient()),
            alpha: 0.5,
            font_size: 20.sp(),
            ..TextStyle::DEFAULT
        };
        let paragraph = measured(measurer, lorem_ipsum(), style, 100);
        let brush = gradient();
        paragraph.paint_with_brush(scope, &brush, f32::NAN, None, None, None, BlendMode::SrcOver);
    });
    let lines = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let style = TextStyle { brush: Some(gradient()), font_size: 20.sp(), ..TextStyle::DEFAULT };
        let paragraph = measured(measurer, AnnotatedString::new("Lorem\nIpsum"), style, 100);
        let brush = gradient();
        paragraph.paint_with_brush(scope, &brush, 0.5, None, None, None, BlendMode::SrcOver);
    });
    assert!(!paragraphs.is_blank());
    assert_eq!(paragraphs, lines);
}

/// A draw style painted is every paragraph's: as the text laid out in it.
// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, multiParagraph_appliesDrawStyle_toAllParagraphs
#[test]
fn a_draw_style_painted_is_every_paragraph_s() {
    let stroke = || DrawStyle::Stroke(compose::ui::Stroke::default());
    let painted = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let style = TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT };
        let paragraph = measured(measurer, lorem_ipsum(), style, 100);
        paragraph.paint(scope, Color::UNSPECIFIED, None, None, Some(stroke()), BlendMode::SrcOver);
    });
    let styled = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let style =
            TextStyle { draw_style: Some(stroke()), font_size: 20.sp(), ..TextStyle::DEFAULT };
        let paragraph = measured(measurer, lorem_ipsum(), style, 100);
        paragraph.paint(scope, Color::UNSPECIFIED, None, None, None, BlendMode::SrcOver);
    });
    let filled = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let style = TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT };
        let paragraph = measured(measurer, lorem_ipsum(), style, 100);
        paragraph.paint(scope, Color::UNSPECIFIED, None, None, None, BlendMode::SrcOver);
    });
    assert_eq!(painted, styled);
    assert_ne!(painted, filled);
}

/// A color painted over each paragraph; a paragraph of the list paints alone, at the origin.
#[test]
fn a_color_painted_is_every_paragraph_s_and_a_paragraph_paints_alone() {
    let style = TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT };
    let red = {
        let style = style.clone();
        draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
            let paragraph = measured(measurer, lorem_ipsum(), style.clone(), 100);
            paragraph.paint(scope, Color::RED, None, None, None, BlendMode::SrcOver);
        })
    };
    assert!(red.contains(Color::RED));
    assert!(!red.contains(Color::BLACK));
    // The second paragraph alone, at the top.
    let second = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let paragraph = measured(measurer, lorem_ipsum(), style.clone(), 100);
        let info = &paragraph.paragraph_info_list()[1];
        info.paragraph.paint(scope, Color::RED, None, None, None, BlendMode::SrcOver);
    });
    assert!(second.contains(Color::RED));
    // Nothing below the first line's height.
    assert!(second.is_empty_from(0, 21));
}

/// Composited with a blend mode: behind the white window (`DstOver`), nothing shows.
#[test]
fn a_blend_mode_composites_the_paint() {
    let style = TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT };
    let cleared = draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
        let paragraph = measured(measurer, lorem_ipsum(), style.clone(), 100);
        paragraph.paint(scope, Color::RED, None, None, None, BlendMode::DstOver);
    });
    // Behind the white already there: nothing shows.
    assert!(cleared.is_blank());
}

/// A blend mode is on each glyph's paint, a span's too (D2060): `Src` replaces the red under a
/// glyph with its half transparent color, where `SrcOver` would mix them, and leaves the red
/// around the glyphs.
#[test]
fn a_blend_mode_is_on_each_glyph_s_paint_a_span_s_too() {
    let half_blue = Color::BLUE.with_alpha(0.5);
    let painted = |blend_mode: BlendMode| {
        draw_with((100, 60), (100, 60), 1.0, 0, move |scope, measurer| {
            scope.draw_rect(
                compose::ui::Rect::new(0.0, 0.0, 100.0, 60.0),
                compose::ui::Paint::fill(Color::RED),
            );
            let span = SpanStyle { color: half_blue, ..Default::default() };
            let text = AnnotatedString::build(|builder| {
                builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
                    builder.append("Lorem");
                });
                builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
                    builder.with_style(span, |builder| {
                        builder.append("Ipsum");
                    });
                });
            });
            let style = TextStyle { color: half_blue, font_size: 20.sp(), ..TextStyle::DEFAULT };
            let paragraph = measured(measurer, text, style, 100);
            paragraph.paint(scope, Color::UNSPECIFIED, None, None, None, blend_mode);
        })
    };
    let replaced = painted(BlendMode::Src);
    let mixed = painted(BlendMode::SrcOver);
    // Inside the first character of each paragraph (the test font's glyph is its em square).
    for (x, y) in [(10, 10), (10, 30)] {
        assert_ne!(replaced.pixel(x, y), Color::RED, "{x}, {y}");
        assert_ne!(replaced.pixel(x, y), mixed.pixel(x, y), "{x}, {y}");
        assert!(replaced.pixel(x, y).alpha() < 0.6, "{x}, {y}: {:?}", replaced.pixel(x, y));
    }
    // Below the text, still red.
    assert_eq!(replaced.pixel(50, 50), Color::RED);
}

/// A paragraph of the list painted alone in a blend mode draws all of it, wider than the drawing:
/// nothing clips it to the drawing's size.
#[test]
fn a_paragraph_painted_alone_in_a_blend_mode_is_not_clipped_to_the_drawing() {
    let style = TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT };
    let alone = draw_with((100, 60), (10, 10), 1.0, 0, move |scope, measurer| {
        let paragraph = measured(measurer, lorem_ipsum(), style.clone(), 100);
        let info = &paragraph.paragraph_info_list()[1];
        info.paragraph.paint(scope, Color::RED, None, None, None, BlendMode::Src);
    });
    // "Ipsum" is 100 px wide, the drawing 10.
    assert!(!alone.is_empty_from(50, 0));
}
