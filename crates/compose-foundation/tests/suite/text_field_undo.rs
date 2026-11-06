//! A state-based field's undo, which is its state's (D1132): edits from the input method, keys,
//! semantics and the application recorded, merged by Compose's rules.

#![allow(non_snake_case)]

use compose_foundation::*;
use compose_test::*;
use compose_ui::{
    CommitTextCommand, FinishComposingTextCommand, Key, SemanticsActions as A,
    SetComposingTextCommand, TextRange, UnitExt,
};

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(start, end)
}

/// A field over a state of `text` with `selection`, tagged "field".
fn field(text: &str, selection: TextRange) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::with_selection(text, selection)));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.width(300.dp()).test_tag("field"));
    });
    ui.wait_for_idle();
    (ui, state)
}

fn held(state: &TextFieldState) -> (String, TextRange) {
    (state.text().to_string(), state.selection())
}

/// Each character as an input of its own, as Compose's test's `typeText`.
fn type_text(ui: &mut ComposeTestRule, text: &str) {
    for c in text.chars() {
        ui.on_node_with_tag("field").perform_text_input(&c.to_string());
    }
}

/// `performTextInputSelection`.
fn select(ui: &mut ComposeTestRule, selection: TextRange) {
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_SELECTION, |set| {
        set(selection.start as i32, selection.end as i32, false);
    });
}

fn undo(ui: &mut ComposeTestRule, state: &TextFieldState) -> (String, TextRange) {
    state.undo_state().undo();
    ui.wait_for_idle();
    held(state)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, canUndo_imeInsert
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, canRedo_imeInsert
#[test]
fn an_input_is_undone_and_redone_in_the_field_too() {
    let (mut ui, state) = field("Hello", TextRange::at(5));
    ui.on_node_with_tag("field").perform_text_input(", World");
    assert_eq!(state.text().as_str(), "Hello, World");
    state.undo_state().undo();
    ui.wait_for_idle();
    ui.on_node_with_tag("field").assert_text_equals(&["Hello"]);
    state.undo_state().redo();
    ui.wait_for_idle();
    ui.on_node_with_tag("field").assert_text_equals(&["Hello, World"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, undoMerges_imeInserts
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, undoMerges_imeInserts_onlyInForwardsDirection
#[test]
fn inputs_one_after_another_merge_forwards_only() {
    let (mut ui, state) = field("Hello", TextRange::at(5));
    type_text(&mut ui, ", World");
    assert_eq!(undo(&mut ui, &state).0, "Hello");
    let (mut ui, state) = field("Hello", TextRange::at(5));
    ui.on_node_with_tag("field").perform_text_input(", World");
    select(&mut ui, TextRange::at(5));
    ui.on_node_with_tag("field").perform_text_input(" Compose");
    assert_eq!(state.text().as_str(), "Hello Compose, World");
    assert_eq!(undo(&mut ui, &state).0, "Hello, World");
    ui.on_node_with_tag("field").assert_text_equals(&["Hello, World"]);
    assert_eq!(undo(&mut ui, &state).0, "Hello");
}

/// Typing at the end of repeated characters merges: the change is where it was typed, not where a
/// difference of the texts would put it (the review of D1130's finding).
#[test]
fn typing_after_repeated_characters_merges() {
    let (mut ui, state) = field("aa", TextRange::at(2));
    // A difference of "aa" and "aaa" is an insertion at 1, which "b" at 3 wouldn't continue.
    type_text(&mut ui, "ab");
    assert_eq!(state.text().as_str(), "aaab");
    assert_eq!(undo(&mut ui, &state), ("aa".to_string(), TextRange::at(2)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, undoMerges_deletes
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, undoDoesNotMerge_deletes_inBothDirections
#[test]
fn deletions_merge_one_way_only() {
    let (mut ui, state) = field("Hello, World", TextRange::at(12));
    ui.on_node_with_tag("field").request_focus();
    for _ in 0..12 {
        ui.press_key(Key::Backspace);
    }
    assert_eq!(held(&state), (String::new(), TextRange::ZERO));
    assert_eq!(undo(&mut ui, &state), ("Hello, World".to_string(), TextRange::at(12)));
    let (mut ui, state) = field("Hello, World", TextRange::at(6));
    ui.on_node_with_tag("field").request_focus();
    for key in [Key::Backspace, Key::Delete] {
        for _ in 0..6 {
            ui.press_key(key);
        }
    }
    assert_eq!(held(&state), (String::new(), TextRange::ZERO));
    assert_eq!(undo(&mut ui, &state), (" World".to_string(), TextRange::at(0)));
    assert_eq!(undo(&mut ui, &state), ("Hello, World".to_string(), TextRange::at(6)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, undo_revertsSelection
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, redo_revertsSelection
#[test]
fn undo_and_redo_bring_their_selections_back() {
    let (mut ui, state) = field("Hello", TextRange::at(5));
    select(&mut ui, range(0, 5));
    ui.on_node_with_tag("field").perform_text_input("a");
    assert_eq!(held(&state), ("a".to_string(), TextRange::at(1)));
    assert_eq!(undo(&mut ui, &state), ("Hello".to_string(), range(0, 5)));
    let (mut ui, state) = field("Hello", TextRange::at(5));
    ui.on_node_with_tag("field").request_focus();
    select(&mut ui, TextRange::at(2));
    ui.on_node_with_tag("field").perform_text_input(" abc ");
    assert_eq!(held(&state), ("He abc llo".to_string(), TextRange::at(7)));
    state.undo_state().undo();
    ui.wait_for_idle();
    assert_eq!(held(&state), ("Hello".to_string(), TextRange::at(2)));
    state.undo_state().redo();
    ui.wait_for_idle();
    assert_eq!(held(&state), ("He abc llo".to_string(), TextRange::at(7)));
}

/// `typeText("abc def")`, a selection at 4, `typeText("123 ")`, a selection of 0..3,
/// `typeText("ghi")`, then the text cleared, as Compose's test.
fn various_edits() -> (ComposeTestRule, TextFieldState) {
    let (mut ui, state) = field("", TextRange::ZERO);
    type_text(&mut ui, "abc def");
    select(&mut ui, TextRange::at(4));
    type_text(&mut ui, "123 ");
    select(&mut ui, range(0, 3));
    type_text(&mut ui, "ghi");
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_TEXT, |set| {
        set("".into());
    });
    (ui, state)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, variousEditOperations
#[test]
fn various_edits_are_undone_step_by_step() {
    let (mut ui, state) = various_edits();
    assert_eq!(held(&state), (String::new(), TextRange::ZERO));
    let undone: Vec<_> = (0..5).map(|_| undo(&mut ui, &state)).collect();
    assert_eq!(
        undone,
        [
            ("ghi 123 def".to_string(), TextRange::at(3)),
            ("g 123 def".to_string(), TextRange::at(1)),
            ("abc 123 def".to_string(), range(0, 3)),
            ("abc def".to_string(), TextRange::at(4)),
            (String::new(), TextRange::ZERO),
        ]
    );
    assert!(!state.undo_state().can_undo());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, clearHistory_removesAllUndoAndRedo
#[test]
fn clearing_the_history_leaves_nothing_to_undo_or_redo() {
    let (mut ui, state) = various_edits();
    for _ in 0..3 {
        undo(&mut ui, &state);
    }
    assert!(state.undo_state().can_undo() && state.undo_state().can_redo());
    state.undo_state().clear_history();
    ui.wait_for_idle();
    assert!(!state.undo_state().can_undo() && !state.undo_state().can_redo());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, paste_neverMerges
#[test]
fn a_paste_is_a_step_of_its_own() {
    let (mut ui, state) = field("", TextRange::ZERO);
    ui.host().with_tree(|tree| tree.clipboard().set_text("ghi"));
    type_text(&mut ui, "abc def ");
    ui.on_node_with_tag("field").perform_semantics_action(&A::PASTE_TEXT);
    type_text(&mut ui, " jkl");
    assert_eq!(undo(&mut ui, &state), ("abc def ghi".to_string(), TextRange::at(11)));
    assert_eq!(undo(&mut ui, &state), ("abc def ".to_string(), TextRange::at(8)));
    assert_eq!(undo(&mut ui, &state), (String::new(), TextRange::ZERO));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, cut_neverMerges
#[test]
fn a_cut_is_a_step_of_its_own() {
    let (mut ui, state) = field("abc def ghi", TextRange::at(11));
    ui.on_node_with_tag("field").request_focus();
    for _ in 0..4 {
        ui.press_key(Key::Backspace);
    }
    select(&mut ui, range(4, 7));
    ui.on_node_with_tag("field").perform_semantics_action(&A::CUT_TEXT);
    for _ in 0..4 {
        ui.press_key(Key::Backspace);
    }
    assert_eq!(held(&state), (String::new(), TextRange::ZERO));
    assert_eq!(undo(&mut ui, &state), ("abc ".to_string(), TextRange::at(4)));
    assert_eq!(undo(&mut ui, &state), ("abc def".to_string(), range(4, 7)));
    assert_eq!(undo(&mut ui, &state), ("abc def ghi".to_string(), TextRange::at(11)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, programmaticEdits_contributeToUndoHistory
#[test]
fn an_edit_of_the_state_is_undone_and_redone() {
    let (mut ui, state) = field("", TextRange::ZERO);
    state.edit(|buffer| buffer.append("abc"));
    ui.wait_for_idle();
    assert!(state.undo_state().can_undo() && !state.undo_state().can_redo());
    assert_eq!(undo(&mut ui, &state), (String::new(), TextRange::ZERO));
    assert!(!state.undo_state().can_undo() && state.undo_state().can_redo());
    state.undo_state().redo();
    ui.wait_for_idle();
    assert_eq!(held(&state), ("abc".to_string(), TextRange::at(3)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, programmaticEdits_contributeToUndoHistoryUnmerged
#[test]
fn edits_of_the_state_never_merge() {
    let (mut ui, state) = field("", TextRange::ZERO);
    type_text(&mut ui, "aaa ");
    assert_eq!(held(&state), ("aaa ".to_string(), TextRange::at(4)));
    for _ in 0..3 {
        state.edit(|buffer| buffer.append("a"));
    }
    ui.wait_for_idle();
    assert!(state.undo_state().can_undo() && !state.undo_state().can_redo());
    let undone: Vec<_> = (0..4).map(|_| undo(&mut ui, &state)).collect();
    assert_eq!(
        undone,
        [
            ("aaa aa".to_string(), TextRange::at(6)),
            ("aaa a".to_string(), TextRange::at(5)),
            ("aaa ".to_string(), TextRange::at(4)),
            (String::new(), TextRange::ZERO),
        ]
    );
}

/// `can_undo` is state: a composable reading it recomposes when it changes.
#[test]
fn can_undo_is_read_as_state() {
    let state = **Box::leak(Box::new(TextFieldState::new("")));
    let mut ui = compose_test(move || {
        let label = if state.undo_state().can_undo() { "undo" } else { "nothing" };
        BasicText!(label);
    });
    ui.on_node_with_text("nothing").assert_exists();
    state.edit(|buffer| buffer.append("x"));
    ui.wait_for_idle();
    ui.on_node_with_text("undo").assert_exists();
}

/// An input method composing a word letter by letter is one step, as Compose's `imeReplace`
/// keeps each step as the letter it added (the review of D1132's finding).
#[test]
fn a_composed_word_is_one_step() {
    let (mut ui, state) = field("", TextRange::ZERO);
    ui.on_node_with_tag("field").request_focus();
    for text in ["h", "he", "hel"] {
        assert!(ui.host().send_edit_commands(&[&SetComposingTextCommand::new(text, 1)]));
    }
    assert!(ui.host().send_edit_commands(&[&FinishComposingTextCommand]));
    ui.wait_for_idle();
    assert_eq!(state.text().as_str(), "hel");
    assert_eq!(undo(&mut ui, &state).0, "");
    assert!(!state.undo_state().can_undo());
    let (mut ui, state) = field("", TextRange::ZERO);
    ui.on_node_with_tag("field").request_focus();
    assert!(ui.host().send_edit_commands(&[&SetComposingTextCommand::new("hel", 1)]));
    assert!(ui.host().send_edit_commands(&[&CommitTextCommand::new("hello", 1)]));
    ui.wait_for_idle();
    assert_eq!(state.text().as_str(), "hello");
    assert_eq!(undo(&mut ui, &state).0, "");
    assert!(!state.undo_state().can_undo());
}

/// A cut or paste that changes nothing leaves the next typing to merge with the typing before.
#[test]
fn a_paste_of_nothing_leaves_typing_to_merge() {
    let (mut ui, state) = field("", TextRange::ZERO);
    ui.host().with_tree(|tree| tree.clipboard().set_text(""));
    type_text(&mut ui, "ab");
    ui.on_node_with_tag("field").perform_semantics_action(&A::PASTE_TEXT);
    type_text(&mut ui, "c");
    assert_eq!(state.text().as_str(), "abc");
    assert_eq!(undo(&mut ui, &state).0, "");
}
