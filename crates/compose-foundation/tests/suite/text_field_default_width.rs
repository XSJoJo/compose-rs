//! A state-based field's default width of ten "H" (D1137).

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{TextAlign, TextLayoutResult, TextStyle, UnitExt};

fn width_of(ui: &mut ComposeTestRule, tag: &str) -> f32 {
    ui.on_node_unmerged(has_test_tag(tag)).fetch_semantics_snapshot().bounds.width()
}

/// A field of `text` with `modifier`, centered, and where its single line's text starts.
fn centered(text: &'static str, modifier: Modifier) -> (ComposeTestRule, Rc<Cell<f32>>) {
    let left = Rc::new(Cell::new(f32::NAN));
    let heard = left.clone();
    let ui = compose_test(move || {
        let state = remember_text_field_state(text, None);
        let (heard, modifier) = (heard.clone(), modifier.clone());
        Column!({
            let heard = heard.clone();
            BasicTextFieldOfState!(
                state,
                modifier = modifier.clone().test_tag("field"),
                text_style =
                    TextStyle { text_align: Some(TextAlign::Center), ..TextStyle::DEFAULT },
                line_limits = TextFieldLineLimits::SingleLine,
                on_text_layout = Some(move |result: TextLayoutResult| {
                    heard.set(result.get_line_left(0));
                }),
            );
            BasicText!("HHHHHHHHHH", modifier = Modifier.test_tag("ten"));
        });
    });
    (ui, left)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_textAlignCenter_defaultWidth
#[test]
fn a_short_text_is_centered_in_ten_hs() {
    let (mut ui, left) = centered("A", Modifier);
    let ten = width_of(&mut ui, "ten");
    assert_eq!(width_of(&mut ui, "field"), ten);
    // "A" is one "H" wide in the test measurer: centered, about four and a half on each side
    // (the text's width leaves out the cursor's few pixels, D1137).
    let centered = left.get() * 2.0 + ten / 10.0;
    assert!((ten - centered) >= 0.0 && (ten - centered) <= 4.0, "the line's left {}", left.get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_textAlignCenter_widthSmallerThanDefaultWidth
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_textAlignCenter_widthLargerThanDefaultWidth
#[test]
fn a_width_given_is_the_fields_either_way() {
    for width in [20.0, 400.0] {
        let (mut ui, left) = centered("A", Modifier.width(width.dp()));
        let field = width_of(&mut ui, "field");
        assert_eq!(field, width, "the width given");
        let ten = width_of(&mut ui, "ten");
        if width > ten {
            assert!(left.get() > ten * 0.45, "centered in the wider field: {}", left.get());
        }
    }
}

/// A longer text makes the field wider; a value-based field keeps no default width.
#[test]
fn a_long_text_is_wider_and_a_value_field_has_none() {
    let mut ui = compose_test(|| {
        let state = remember_text_field_state("HHHHHHHHHHHHHHHHHHHH", None);
        Column!({
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("long"));
            BasicTextField!(String::new(), |_| {}, modifier = Modifier.test_tag("value"));
            BasicText!("HHHHHHHHHH", modifier = Modifier.test_tag("ten"));
        });
    });
    let ten = width_of(&mut ui, "ten");
    assert!(width_of(&mut ui, "long") > 2.0 * ten - 1.0);
    assert!(width_of(&mut ui, "value") < ten);
}

/// Its intrinsic widths are its text's and the cursor's, as Compose's (`textFieldMinSize` sets
/// the least width when measured, its intrinsics are the default ones: D1140): a column as wide
/// as its narrowest or widest content is as wide as "ab" and the cursor around a field over "ab".
#[test]
fn the_intrinsic_widths_are_the_texts() {
    for (tag, size) in
        [("min", compose_ui::IntrinsicSize::Min), ("max", compose_ui::IntrinsicSize::Max)]
    {
        let mut ui = compose_test(move || {
            let state = remember_text_field_state("ab", None);
            Column!({
                Column!(modifier = Modifier.width(size).test_tag("column"), {
                    BasicTextFieldOfState!(state);
                });
                BasicText!("ab", modifier = Modifier.test_tag("ab"));
                BasicText!("HHHHHHHHHH", modifier = Modifier.test_tag("ten"));
            });
        });
        let cursor = (2.0 * ui.density().density).round();
        let ab = width_of(&mut ui, "ab");
        assert_eq!(width_of(&mut ui, "column"), ab + cursor, "{tag}");
        assert!(ab + cursor < width_of(&mut ui, "ten"), "{tag}");
    }
}
