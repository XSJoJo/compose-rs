//! A state-based field's `on_text_layout` (D1136).

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{TextLayoutResult, TextRange, TextStyle, UnitExt};

type Heard = Rc<RefCell<Vec<TextLayoutResult>>>;

/// A focused field over `text` whose layouts are heard, with `style` and `output`.
fn field_heard(
    text: &str,
    style: State<TextStyle>,
    output: Option<OutputTransformation>,
) -> (ComposeTestRule, TextFieldState, Heard) {
    let state =
        **Box::leak(Box::new(TextFieldState::with_selection(text, TextRange::at(text.len()))));
    let heard: Heard = Rc::default();
    let into = heard.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            text_style = style.get(),
            output_transformation = output.clone(),
            on_text_layout = Some(move |result: TextLayoutResult| into.borrow_mut().push(result)),
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    (ui, state, heard)
}

fn texts(heard: &Heard) -> Vec<String> {
    heard.borrow().iter().map(|result| result.layout_input.text.to_string()).collect()
}

fn fixed(style: TextStyle) -> State<TextStyle> {
    Box::leak(Box::new(mutable_state_of(style))).handle().into()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_rendersEmptyContent
#[test]
fn an_empty_field_is_laid_out_and_heard() {
    let (_ui, _, heard) = field_heard("", fixed(TextStyle::DEFAULT), None);
    assert_eq!(texts(&heard), [""]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_contentChange_relayouts
#[test]
fn new_text_is_a_new_layout_and_a_cursor_move_is_not() {
    let (mut ui, _, heard) = field_heard("Hello ", fixed(TextStyle::DEFAULT), None);
    ui.type_text("World!");
    assert_eq!(texts(&heard), ["Hello ", "Hello World!"]);
    ui.press_key(compose_ui::Key::DirectionLeft);
    assert_eq!(heard.borrow().len(), 2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_textStyleFontSizeChange_relayouts
#[test]
fn a_new_font_size_is_a_new_layout() {
    let style = Box::leak(Box::new(mutable_state_of(TextStyle {
        font_size: 20.sp(),
        ..TextStyle::DEFAULT
    })))
    .handle();
    let (mut ui, _, heard) = field_heard("Hello ", style.into(), None);
    style.set(TextStyle { font_size: 30.sp(), ..TextStyle::DEFAULT });
    ui.wait_for_idle();
    let sizes: Vec<_> = heard.borrow().iter().map(|it| it.layout_input.style.font_size).collect();
    assert_eq!(sizes, [20.sp(), 30.sp()]);
}

/// What's heard is what's shown: the output transformation's text.
#[test]
fn the_layout_heard_is_of_the_shown_text() {
    let output = OutputTransformation::from_fn(|buffer| buffer.append("!"));
    let (mut ui, state, heard) = field_heard("hi", fixed(TextStyle::DEFAULT), Some(output));
    ui.type_text("s");
    assert_eq!(texts(&heard), ["hi!", "his!"]);
    assert_eq!(state.text().as_str(), "his");
}

/// A field's layout result says the constraints its text was laid out at, as Compose's text
/// under the field's scroll: a single line's unbounded in width, so a text wider than the field
/// is as wide as it, and lines' unbounded in height.
// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/input/internal/TextFieldCoreModifier.kt, measureHorizontalScroll and measureVerticalScroll (the text measured at `maxWidth`, or `maxHeight`, `Constraints.Infinity`)
#[test]
fn the_layout_input_constraints_are_the_scrolled_texts() {
    use compose_ui::Constraints;
    for single in [true, false] {
        let state = **Box::leak(Box::new(TextFieldState::new("wide ".repeat(30))));
        let heard: Heard = Rc::default();
        let into = heard.clone();
        let limits =
            if single { TextFieldLineLimits::SingleLine } else { TextFieldLineLimits::default() };
        let _ui = compose_test(move || {
            let into = into.clone();
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.width(100.dp()).height(40.dp()).test_tag("field"),
                line_limits = limits,
                on_text_layout =
                    Some(move |result: TextLayoutResult| into.borrow_mut().push(result)),
            );
        });
        let result = heard.borrow().last().cloned().expect("a layout");
        let constraints = result.layout_input.constraints;
        if single {
            assert_eq!(constraints.max_width, Constraints::INFINITY);
            assert!(result.size.width > 100, "{:?}", result.size);
        } else {
            assert_eq!(constraints.max_height, Constraints::INFINITY);
            assert!(result.size.height > 40, "{:?}", result.size);
        }
        assert_eq!(constraints.constrain(result.size), result.size, "single {single}");
    }
}
