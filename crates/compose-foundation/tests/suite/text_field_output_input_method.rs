//! A state-based field's input method over its output transformation: it reads the text shown,
//! and its offsets are mapped back to the state's, as Compose's `ImeEditCommandScope` (D1135).

#![allow(non_snake_case)]

use compose_foundation::*;
use compose_test::*;
use compose_ui::{
    CommitTextCommand, DeleteSurroundingTextCommand, DeleteSurroundingTextInCodePointsCommand,
    EditCommand, Key, KeyEvent, KeyEventType, KeyModifiers, SetComposingRegionCommand,
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

/// Each character shown as `with`.
fn masked(with: &'static str) -> impl Fn(&mut TextFieldBuffer) + 'static {
    move |buffer: &mut TextFieldBuffer| {
        let mut at = 0;
        while at < buffer.length() {
            let length = buffer.as_str()[at..].chars().next().map_or(1, char::len_utf8);
            buffer.replace(at, at + length, with);
            at += with.len();
        }
    }
}

/// "123456" shown "123-456".
fn phone(buffer: &mut TextFieldBuffer) {
    if buffer.length() > 3 {
        buffer.insert(3, "-");
    }
}

/// Android's cursor position after a commit counts units of the field's own text, as Compose's
/// buffer does: not the bullets shown.
#[test]
fn the_cursor_after_a_commit_counts_in_the_fields_own_text() {
    let (mut ui, state) = field_showing("abcdefgh", TextRange::at(2), masked("\u{2022}"));
    send(&mut ui, &[&CommitTextCommand::in_utf16("x", 3)]);
    assert_eq!(held(state), ("abxcdefgh".to_string(), TextRange::at(5), None));
    let (mut ui, state) = field_showing("\u{e9}\u{e9}\u{e9}", TextRange::ZERO, masked("*"));
    send(&mut ui, &[&CommitTextCommand::in_utf16("x", 2)]);
    assert_eq!(state.selection(), TextRange::at(3), "after the first \u{e9}");
    let (mut ui, state) = field_showing("abcdefgh", TextRange::at(2), masked("\u{2022}"));
    send(&mut ui, &[&SetComposingTextCommand::in_utf16("x", 3)]);
    assert_eq!(state.selection(), TextRange::at(5));
}

/// Around a selection, the lengths count from its ends: after its end, before its start.
#[test]
fn surrounding_text_around_a_selection_is_counted_from_its_ends() {
    let (mut ui, state) = field_showing("123456", TextRange::new(1, 2), phone);
    send(&mut ui, &[&DeleteSurroundingTextCommand::new(1, 1)]);
    assert_eq!(held(state), ("2456".to_string(), TextRange::new(0, 1), None));
}

/// Across what's inserted: the dash counts as shown, deletes nothing of the state, and code
/// points count in the state's own text (Compose doesn't map them).
#[test]
fn surrounding_text_across_an_insertion() {
    for (cursor, before, after, expected) in
        [(3, 1, 0, ("12456", 2)), (3, 0, 1, ("12356", 3)), (5, 3, 0, ("1236", 3))]
    {
        let (mut ui, state) = field_showing("123456", TextRange::at(cursor), phone);
        send(&mut ui, &[&DeleteSurroundingTextCommand::new(before, after)]);
        let expected = (expected.0.to_string(), TextRange::at(expected.1), None);
        assert_eq!(held(state), expected, "{cursor}: {before}, {after}");
    }
    let (mut ui, state) = field_showing("123456", TextRange::at(5), phone);
    send(&mut ui, &[&DeleteSurroundingTextInCodePointsCommand::new(3, 0)]);
    assert_eq!(held(state), ("126".to_string(), TextRange::at(2), None));
}

/// A hidden range: the cursor shown after it, a deletion before it takes it, a selection at it
/// selects it, a region over it covers it.
#[test]
fn offsets_at_a_hidden_range() {
    let hide = |buffer: &mut TextFieldBuffer| {
        if buffer.as_str().starts_with("abc") {
            buffer.delete(1, 3);
        }
    };
    let (mut ui, state) = field_showing("abcdef", TextRange::at(4), hide);
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.selection()), ("adef", TextRange::at(2)));
    send(&mut ui, &[&DeleteSurroundingTextCommand::new(1, 0)]);
    assert_eq!(held(state), ("aef".to_string(), TextRange::at(1), None));
    let (mut ui, state) = field_showing("abcdef", TextRange::at(4), hide);
    send(&mut ui, &[&SetSelectionCommand::new(1, 1)]);
    assert_eq!(state.selection(), TextRange::new(1, 3));
    let (mut ui, state) = field_showing("abcdef", TextRange::at(4), hide);
    send(&mut ui, &[&SetComposingRegionCommand::new(0, 2)]);
    assert_eq!(state.composition(), Some(TextRange::new(0, 4)));
}

fn press(ui: &mut ComposeTestRule, key: Key) {
    let modifiers = KeyModifiers::default();
    for kind in [KeyEventType::KeyDown, KeyEventType::KeyUp] {
        ui.host().send_key(KeyEvent { key, kind, modifiers, from_soft_keyboard: false });
    }
    ui.wait_for_idle();
}

/// Keys over two replacements side by side, and with something composed: what's composed ends
/// and the key deletes in what's shown.
#[test]
fn keys_delete_by_what_is_shown_next_to_replacements_and_compositions() {
    let two = |buffer: &mut TextFieldBuffer| {
        if buffer.length() >= 3 {
            buffer.replace(2, 3, "CC");
            buffer.replace(1, 2, "BB");
        }
    };
    let (mut ui, state) = field_showing("abcd", TextRange::at(3), two);
    press(&mut ui, Key::Backspace);
    assert_eq!(held(state), ("abd".to_string(), TextRange::at(2), None));
    let (mut ui, state) = field_showing("abcd", TextRange::at(1), two);
    press(&mut ui, Key::Delete);
    assert_eq!(held(state), ("acd".to_string(), TextRange::at(1), None));
    let (mut ui, state) = field_showing("123456", TextRange::at(6), phone);
    send(&mut ui, &[&SetComposingTextCommand::new("78", 1)]);
    press(&mut ui, Key::Backspace);
    assert_eq!(held(state), ("1234567".to_string(), TextRange::at(7), None));
}

/// A batch's later offsets map by the text it began with, which may have shown nothing
/// transformed.
#[test]
fn a_batch_starting_untransformed_maps_as_it_began() {
    let (mut ui, state) = field_showing("123456", TextRange::at(6), phone);
    send(&mut ui, &[&CommitTextCommand::new("7", 1), &SetSelectionCommand::new(4, 4)]);
    assert_eq!(held(state), ("1234567".to_string(), TextRange::at(3), None));
    let (mut ui, state) = field_showing("12", TextRange::at(2), phone);
    send(&mut ui, &[&CommitTextCommand::new("34", 1), &SetSelectionCommand::new(3, 3)]);
    assert_eq!(held(state), ("1234".to_string(), TextRange::at(3), None));
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.selection()), ("123-4", TextRange::at(3)));
}
