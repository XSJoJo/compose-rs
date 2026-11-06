//! A state-based field's input method over its output transformation: it reads the text shown,
//! and its offsets are mapped back to the state's, as Compose's `ImeEditCommandScope` (D1135).

#![allow(non_snake_case)]

use compose_foundation::*;
use compose_test::*;
use compose_ui::{
    CommitTextCommand, DeleteSurroundingTextCommand, EditCommand, SetComposingRegionCommand,
    SetComposingTextCommand, SetSelectionCommand, TextRange, UnitExt,
};

/// A focused field over `text` with `selection`, showing it through `transform`.
fn field_showing(
    text: &str,
    selection: TextRange,
    transform: impl Fn(&mut TextFieldBuffer) + 'static,
) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::with_selection(text, selection)));
    let output = OutputTransformation::from_fn(transform);
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            output_transformation = Some(output.clone()),
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    (ui, state)
}

/// The commands sent as one batch.
fn send(ui: &mut ComposeTestRule, commands: &[&dyn EditCommand]) {
    assert!(ui.host().send_edit_commands(commands));
    ui.wait_for_idle();
}

/// The state's text, selection and composition.
fn held(state: TextFieldState) -> (String, TextRange, Option<TextRange>) {
    (state.text().to_string(), state.selection(), state.composition())
}

/// Replaces the first "123" with "x".
fn shorten_numbers(buffer: &mut TextFieldBuffer) {
    if let Some(index) = buffer.as_str().find("123") {
        buffer.replace(index, index + 3, "x");
    }
}

/// The input method reads the text shown, its selection and composition there, and is told
/// again when what's shown changes with the state.
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, collectImeNotifications_usesVisualText
#[test]
fn the_input_method_reads_the_shown_text() {
    let (mut ui, state) = field_showing("abc", TextRange::at(3), |buffer| buffer.insert(0, ">"));
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.selection()), (">abc", TextRange::at(4)));
    send(&mut ui, &[&SetComposingRegionCommand::new(2, 4)]);
    assert_eq!(state.composition(), Some(TextRange::new(1, 3)));
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!(value.composition(), Some(TextRange::new(2, 4)));
    assert!(ui.host().input_method_value_is(&value).unwrap_or(false));
    state.edit(|buffer| buffer.append("d"));
    ui.wait_for_idle();
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.selection()), (">abcd", TextRange::at(5)));
    let (mut ui, state) = field_showing("hello", TextRange::at(5), |buffer| {
        buffer.insert(0, "a");
        buffer.append("a");
    });
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.selection()), ("ahelloa", TextRange::at(6)));
    state.edit(|buffer| buffer.append(" world"));
    ui.wait_for_idle();
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.selection()), ("ahello worlda", TextRange::at(12)));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/SetComposingRegionCommandTest.kt, test_set_too_small_with_output_transformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/SetComposingRegionCommandTest.kt, test_set_too_large_with_output_transformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/SetComposingRegionCommandTest.kt, test_set_too_small_and_too_large_with_output_transformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/SetComposingRegionCommandTest.kt, test_set_too_small_and_too_large_reversed_with_output_transformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/SetComposingRegionCommandTest.kt, test_output_transformation_additive_untransform_given_range
#[test]
fn a_composing_region_is_in_the_shown_text() {
    for (start, end, expected) in [
        (-1000, -1000, None),
        (1000, 1000, None),
        (-1000, 1000, Some(TextRange::new(0, 5))),
        (1000, -1000, Some(TextRange::new(0, 5))),
        (1, 3, Some(TextRange::new(0, 2))),
    ] {
        let (mut ui, state) =
            field_showing("ABCDE", TextRange::ZERO, |buffer| buffer.insert(0, "F"));
        send(&mut ui, &[&SetComposingRegionCommand::new(start, end)]);
        let held = held(state);
        assert_eq!(held, ("ABCDE".to_string(), TextRange::ZERO, expected), "{start}..{end}");
    }
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/SetComposingRegionCommandTest.kt, test_set_too_small_and_too_large_with_reductive_output_transformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/SetComposingRegionCommandTest.kt, test_output_transformation_reductive_untransform_given_range
#[test]
fn a_composing_region_over_a_deletion_covers_it() {
    let (mut ui, state) = field_showing("ABCDE", TextRange::ZERO, |buffer| buffer.delete(0, 3));
    send(&mut ui, &[&SetComposingRegionCommand::new(-1000, 1000)]);
    assert_eq!(state.composition(), Some(TextRange::new(0, 5)));
    let (mut ui, state) = field_showing("AB", TextRange::ZERO, |buffer| buffer.delete(0, 1));
    send(&mut ui, &[&SetComposingRegionCommand::new(0, 1)]);
    assert_eq!(held(state), ("AB".to_string(), TextRange::ZERO, Some(TextRange::new(0, 2))));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/DeleteSurroundingTextCommandTest.kt, test_delete_withOutputTransformation_deletesBeforeCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/DeleteSurroundingTextCommandTest.kt, test_delete_withOutputTransformation_deletesAfterCursor
#[test]
fn surrounding_text_is_counted_in_the_shown_text() {
    // Shown "abc x| def": five before the cursor are "abc x".
    let (mut ui, state) = field_showing("abc 123 def", TextRange::at(7), shorten_numbers);
    send(&mut ui, &[&DeleteSurroundingTextCommand::new(5, 0)]);
    assert_eq!(held(state), (" def".to_string(), TextRange::ZERO, None));
    // Shown "|abc x def": six after it are "abc x ".
    let (mut ui, state) = field_showing("abc 123 def", TextRange::at(0), shorten_numbers);
    send(&mut ui, &[&DeleteSurroundingTextCommand::new(0, 6)]);
    assert_eq!(held(state), ("def".to_string(), TextRange::ZERO, None));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/DeleteSurroundingTextCommandTest.kt, test_delete_withOutputTransformation_deletesBeforeCursor_additive
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/DeleteSurroundingTextCommandTest.kt, test_delete_withOutputTransformation_deletesBeforeCursor_additive_intMax
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/DeleteSurroundingTextCommandTest.kt, test_delete_withOutputTransformation_deletesAfterCursor_additive_intMax
#[test]
fn surrounding_text_counts_what_is_inserted() {
    let spaced = |buffer: &mut TextFieldBuffer| buffer.insert(2, " ");
    for (cursor, before, after) in [(6, 7, 0), (6, usize::MAX, 0), (0, 0, usize::MAX)] {
        let (mut ui, state) = field_showing("abcdef", TextRange::at(cursor), spaced);
        send(&mut ui, &[&DeleteSurroundingTextCommand::new(before, after)]);
        assert_eq!(held(state), (String::new(), TextRange::ZERO, None), "{before}, {after}");
    }
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/BatchEditTest.kt, batchEdit_withPrefixTransformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/BatchEditTest.kt, batchEdit_withSuffixTransformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/BatchEditTest.kt, batchEdit_withMaskingTransformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/BatchEditTest.kt, batchEdit_withCompositeTransformation
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/BatchEditTest.kt, batchEdit_transformationDependentOnContent
#[test]
fn a_batch_maps_its_offsets_by_the_text_it_began_with() {
    let commit = |text: &str| CommitTextCommand::new(text, 1);
    let delete_one = DeleteSurroundingTextCommand::new(1, 0);
    let (mut ui, state) =
        field_showing("ABC", TextRange::at(0), |buffer| buffer.insert(0, "PREFIX"));
    send(&mut ui, &[&commit("D")]);
    assert_eq!(state.text().as_str(), "DABC");
    let (mut ui, state) = field_showing("ABC", TextRange::at(3), |buffer| buffer.append("SUFFIX"));
    send(&mut ui, &[&commit("D")]);
    assert_eq!(state.text().as_str(), "ABCD");
    let (mut ui, state) = field_showing("ABC", TextRange::at(3), |buffer| buffer.append("!"));
    send(&mut ui, &[&commit("D"), &delete_one]);
    assert_eq!(state.text().as_str(), "ABC");
    let (mut ui, state) = field_showing("ABC", TextRange::at(3), |buffer| {
        buffer.insert(0, "PREFIX");
        buffer.append("SUFFIX");
    });
    send(&mut ui, &[&delete_one, &commit("D")]);
    assert_eq!(state.text().as_str(), "ABD");
    let (mut ui, state) = field_showing("12345", TextRange::at(5), |buffer| {
        if buffer.length() > 3 {
            buffer.insert(3, "-");
        }
    });
    send(&mut ui, &[&commit("6"), &delete_one]);
    assert_eq!(state.text().as_str(), "12345");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/StatelessInputConnectionTest.kt, setSelection_respectsOutputTransformation
#[test]
fn a_selection_the_input_method_sets_is_in_the_shown_text() {
    let (mut ui, state) =
        field_showing("abc def", TextRange::ZERO, |buffer| buffer.insert(4, "ghi "));
    send(&mut ui, &[&SetSelectionCommand::new(5, 5)]);
    assert_eq!(state.selection(), TextRange::at(4));
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!(value.selection(), TextRange::at(4));
    // Past the shown text, its end.
    send(&mut ui, &[&SetSelectionCommand::new(2, 100)]);
    assert_eq!(state.selection(), TextRange::new(2, 7));
    // Kept in the shown text first: past "ab" shown of "abc", its end, where "c" was deleted.
    let (mut ui, state) = field_showing("abc", TextRange::ZERO, |buffer| buffer.delete(2, 3));
    send(&mut ui, &[&SetSelectionCommand::new(5, 5)]);
    assert_eq!(state.selection(), TextRange::new(2, 3));
}

/// What the input method composes and commits goes where the state's cursor is, whatever is
/// shown around it.
#[test]
fn composing_goes_to_the_states_cursor() {
    let (mut ui, state) = field_showing("ab", TextRange::at(1), |buffer| buffer.insert(1, "--"));
    send(&mut ui, &[&SetComposingTextCommand::new("x", 1)]);
    assert_eq!(held(state), ("axb".to_string(), TextRange::at(2), Some(TextRange::new(1, 2))));
    // Shown, the composition starts before the insertion at its start (the wedge affinity's).
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.composition()), ("a--xb", Some(TextRange::new(1, 4))));
    send(&mut ui, &[&CommitTextCommand::new("yz", 1)]);
    assert_eq!(held(state), ("ayzb".to_string(), TextRange::at(3), None));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, whenWindowFocusLost_outputTransformationChange_restartsInputSession
#[test]
fn a_new_output_transformation_restarts_the_input_method() {
    let which = Box::leak(Box::new(compose_runtime::mutable_state_of(0usize))).handle();
    let transformations = [
        OutputTransformation::from_fn(|buffer| buffer.append(" World")),
        OutputTransformation::from_fn(|buffer| buffer.append(" Compose")),
    ];
    let state = **Box::leak(Box::new(TextFieldState::new("Hello")));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            output_transformation = Some(transformations[which.get()].clone()),
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    ui.host().take_input_method_restart();
    which.set(1);
    ui.wait_for_idle();
    assert!(ui.host().take_input_method_restart());
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!(value.text(), "Hello Compose");
}
