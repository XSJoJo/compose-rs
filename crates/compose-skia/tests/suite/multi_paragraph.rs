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
