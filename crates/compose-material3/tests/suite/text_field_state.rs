//! Material text fields over a `TextFieldState` (D1136): the value fields' decoration around the
//! state-based field.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::{
    BasicTextFieldOfState, InputTransformation, OutputTransformation, TextFieldBuffer,
    TextFieldLineLimits, TextFieldState,
};
use compose_material3::*;
use compose_test::*;
use compose_ui::{Modifier, SemanticsProperties as P};

fn bounds(ui: &mut ComposeTestRule, tag: &str) -> compose_ui::Rect {
    ui.on_node_unmerged(has_test_tag(tag)).fetch_semantics_snapshot().bounds
}

fn state(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

fn shown(ui: &mut ComposeTestRule) -> String {
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    node.config.get(&P::EDITABLE_TEXT).map(|text| text.to_string()).unwrap_or_default()
}

/// What's typed goes into the state, through the input transformation, and the field shows it
/// through the output transformation; both variants.
#[test]
fn typing_goes_into_the_state_and_shows_transformed() {
    for outlined in [false, true] {
        let held = state("");
        let mut ui = compose_test(move || {
            let caps = InputTransformation::all_caps(compose_ui::Locale::new("en"));
            let suffix = OutputTransformation::from_fn(|buffer| buffer.append("!"));
            MaterialTheme!({
                if outlined {
                    OutlinedTextFieldOfState!(
                        held,
                        modifier = Modifier.test_tag("field"),
                        input_transformation = Some(caps.clone()),
                        output_transformation = Some(suffix.clone()),
                    );
                } else {
                    TextFieldOfState!(
                        held,
                        modifier = Modifier.test_tag("field"),
                        input_transformation = Some(caps.clone()),
                        output_transformation = Some(suffix.clone()),
                    );
                }
            });
        });
        ui.on_node_with_tag("field").request_focus();
        ui.type_text("hi");
        assert_eq!(held.text().as_str(), "HI", "outlined {outlined}");
        assert_eq!(shown(&mut ui), "HI!", "outlined {outlined}");
        held.edit(|buffer| buffer.append(" there"));
        ui.wait_for_idle();
        assert_eq!(shown(&mut ui), "HI there!", "outlined {outlined}");
    }
}

fn prefixed(buffer: &mut TextFieldBuffer) {
    buffer.insert(0, "prefix");
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/TextFieldTest.kt, testTextField_transformedTextIsUsed_toDefineIfPlaceholderNeeded
#[test]
fn the_shown_text_decides_the_placeholder() {
    let (empty, with_prefix) = (state(""), state(""));
    let mut ui = compose_test(move || {
        MaterialTheme!({
            TextFieldOfState!(empty, placeholder = Some(|| Text!("hint one")));
            TextFieldOfState!(
                with_prefix,
                placeholder = Some(|| Text!("hint two")),
                output_transformation = Some(OutputTransformation::from_fn(prefixed)),
            );
        });
    });
    // Without a label, a placeholder shows while the text is empty as it's shown.
    ui.on_node_with_text("hint one").assert_exists();
    ui.on_node_with_text("hint two").assert_does_not_exist();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/TextFieldTest.kt, testTextField_transformedTextIsUsed_toDefineLabelPosition
#[test]
fn the_shown_text_floats_the_label() {
    let (empty, with_prefix) = (state(""), state(""));
    let mut ui = compose_test(move || {
        MaterialTheme!({
            Column!({
                TextFieldOfState!(
                    empty,
                    modifier = Modifier.test_tag("plain"),
                    label = Some(|| Text!("label", modifier = Modifier.test_tag("resting"))),
                );
                TextFieldOfState!(
                    with_prefix,
                    modifier = Modifier.test_tag("prefixed"),
                    label = Some(|| Text!("label", modifier = Modifier.test_tag("floated"))),
                    output_transformation = Some(OutputTransformation::from_fn(prefixed)),
                );
            });
        });
    });
    let (plain, prefixed) = (bounds(&mut ui, "plain"), bounds(&mut ui, "prefixed"));
    let (resting, floated) = (bounds(&mut ui, "resting"), bounds(&mut ui, "floated"));
    assert!(
        floated.top - prefixed.top < resting.top - plain.top,
        "floated {floated:?} in {prefixed:?}, resting {resting:?} in {plain:?}"
    );
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/TextFieldTest.kt, testTextField_outputTransformationPropagated
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/OutlinedTextFieldTest.kt, testOutlinedTextField_visualTransformationPropagated
#[test]
fn the_output_transformation_reaches_the_field() {
    let held = state("qwerty");
    let blank = OutputTransformation::from_fn(|buffer| {
        let length = buffer.length();
        buffer.delete(0, length);
        buffer.insert(0, &" ".repeat(length));
        buffer.place_cursor_at_end();
    });
    let mut ui = compose_test(move || {
        let blank = blank.clone();
        MaterialTheme!({
            OutlinedTextFieldOfState!(
                held,
                modifier = Modifier.test_tag("field"),
                output_transformation = Some(blank.clone()),
            );
        });
    });
    assert_eq!(shown(&mut ui), "      ");
    assert_eq!(held.text().as_str(), "qwerty");
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/TextFieldTest.kt, testTextField_errorSemantics_defaultMessage
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/OutlinedTextFieldTest.kt, testOutlinedTextField_errorSemantics_defaultMessage
#[test]
fn an_error_says_so_in_the_semantics() {
    let (filled, outlined) = (state("test"), state("test"));
    let mut ui = compose_test(move || {
        MaterialTheme!({
            TextFieldOfState!(filled, modifier = Modifier.test_tag("filled"), is_error = true);
            OutlinedTextFieldOfState!(
                outlined,
                modifier = Modifier.test_tag("outlined"),
                is_error = true,
            );
        });
    });
    for tag in ["filled", "outlined"] {
        let node = ui.on_node_with_tag(tag).fetch_semantics_snapshot();
        assert_eq!(node.config.get(&P::ERROR).cloned().as_deref(), Some("Invalid input"), "{tag}");
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/TextFieldTest.kt, testTextField_defaultHeight
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/TextFieldTest.kt, testTextField_defaultWidth
#[test]
fn a_single_line_is_the_value_fields_size() {
    let held = state("input");
    let mut ui = compose_test(move || {
        MaterialTheme!({
            Column!({
                TextField!(
                    "input".to_string(),
                    |_| {},
                    modifier = Modifier.test_tag("value"),
                    single_line = true,
                );
                TextFieldOfState!(
                    held,
                    modifier = Modifier.test_tag("state"),
                    line_limits = TextFieldLineLimits::SingleLine,
                );
            });
        });
    });
    let (value, state) = (bounds(&mut ui, "value"), bounds(&mut ui, "state"));
    assert_eq!((state.width(), state.height()), (value.width(), value.height()));
    assert_eq!((state.width(), state.height()), (280.0, 56.0));
}

/// The state decorators decorate a basic field as the Material fields are decorated: as high,
/// and the label floated by the text as shown.
#[test]
fn a_decorator_decorates_a_basic_field_as_the_material_field() {
    let (decorated, outlined, material) = (state(""), state(""), state(""));
    let mut ui = compose_test(move || {
        MaterialTheme!({
            Column!({
                let source = compose_runtime::remember(compose_ui::MutableInteractionSource::new);
                let prefix = OutputTransformation::from_fn(prefixed);
                let (decorator_source, decorator_prefix) = (source.clone(), prefix.clone());
                BasicTextFieldOfState!(
                    decorated,
                    modifier = Modifier.test_tag("decorated"),
                    interaction_source = Some(source.clone()),
                    output_transformation = Some(prefix.clone()),
                    decorator = move |inner: compose_runtime::Callback| {
                        TextFieldDecorator!(
                            decorated,
                            inner,
                            true,
                            TextFieldLineLimits::DEFAULT,
                            Some(decorator_prefix.clone()),
                            decorator_source.clone(),
                            label =
                                Some(|| Text!("label", modifier = Modifier.test_tag("floated"))),
                        );
                    },
                );
                let other = compose_runtime::remember(compose_ui::MutableInteractionSource::new);
                let other_source = other.clone();
                BasicTextFieldOfState!(
                    outlined,
                    modifier = Modifier.test_tag("outlined"),
                    interaction_source = Some(other.clone()),
                    decorator = move |inner: compose_runtime::Callback| {
                        OutlinedTextFieldDecorator!(
                            outlined,
                            inner,
                            true,
                            TextFieldLineLimits::DEFAULT,
                            None,
                            other_source.clone(),
                        );
                    },
                );
                TextFieldOfState!(
                    material,
                    modifier = Modifier.test_tag("material"),
                    label = Some(|| Text!("label", modifier = Modifier.test_tag("resting"))),
                );
            });
        });
    });
    let (decorated, material) = (bounds(&mut ui, "decorated"), bounds(&mut ui, "material"));
    let outlined = bounds(&mut ui, "outlined");
    // As high: the minimum width is the Material field's own (`defaultMinSize`), not the
    // decoration's.
    assert_eq!(decorated.height(), material.height());
    assert_eq!(outlined.height(), 56.0);
    let (floated, resting) = (bounds(&mut ui, "floated"), bounds(&mut ui, "resting"));
    assert!(floated.top - decorated.top < resting.top - material.top);
}
