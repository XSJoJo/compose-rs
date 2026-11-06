//! `TextFieldState`, its `edit` and `TextFieldBuffer`, and the state-based field over it
//! (docs/parity/design/text-field-state.md, step 1). Compose's tests of what isn't here yet
//! (undo, transformations, styles, an input method's internals) are the ledger's.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::flow::next;
use compose_runtime::platform::run_ready;
use compose_runtime::snapshots::{Snapshot, SnapshotStateObserver};
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    CommitTextCommand, Key, SemanticsProperties as P, SetComposingTextCommand, SetSelectionCommand,
    TextRange, UnitExt,
};

/// An edit of a buffer, for tables of cases.
type Edit = fn(&mut TextFieldBuffer);

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(start, end)
}

/// What `state` holds: its text and selection.
fn held(state: &TextFieldState) -> (String, TextRange) {
    (state.text().to_string(), state.selection())
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, defaultInitialTextAndSelection
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, customInitialTextAndDefaultSelection
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, customInitialTextAndSelection
#[test]
fn a_new_state_puts_the_cursor_at_its_end_unless_told() {
    let empty = TextFieldState::new("");
    assert_eq!(held(&empty), (String::new(), TextRange::ZERO));
    let hello = TextFieldState::new("hello");
    assert_eq!(held(&hello), ("hello".to_string(), TextRange::at(5)));
    let selected = TextFieldState::with_selection("hello", range(0, 1));
    assert_eq!(held(&selected), ("hello".to_string(), range(0, 1)));
    // Kept within the text.
    let past = TextFieldState::with_selection("hi", range(1, 9));
    assert_eq!(past.selection(), range(1, 2));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_doesNotAllow_reentrantBehavior
#[test]
fn an_edit_inside_an_edit_panics_and_changes_nothing() {
    let state = TextFieldState::new("");
    let state = *state;
    let result = catch_unwind(AssertUnwindSafe(|| {
        state.edit(|buffer| {
            buffer.replace(0, 0, "hello");
            state.edit(|inner| inner.replace(0, 0, "hello"));
        });
    }));
    let message = result.expect_err("nested editing panics");
    let message = message
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| message.downcast_ref::<&str>().map(|text| text.to_string()))
        .unwrap_or_default();
    assert!(message.contains("does not support concurrent or nested editing"), "{message}");
    assert_eq!(state.text().as_str(), "");
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_doesNotChange_whenThrows
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_canEditAgain_ifFirstOneThrows
#[test]
fn an_edit_that_panics_changes_nothing_and_another_can_follow() {
    let state = TextFieldState::new("");
    let result = catch_unwind(AssertUnwindSafe(|| {
        state.edit(|buffer| {
            buffer.replace(0, 0, "hello");
            panic!("expected");
        });
    }));
    assert!(result.is_err());
    assert_eq!(state.text().as_str(), "");
    state.edit(|buffer| buffer.replace(0, 0, "hello"));
    assert_eq!(state.text().as_str(), "hello");
}

/// How many times an observation of `state.text()` was told of a change after `act`.
fn invalidations_of_text_after(state: TextFieldState, act: impl FnOnce()) -> usize {
    let count = Rc::new(Cell::new(0));
    let observer = SnapshotStateObserver::new(|run| run());
    let counted = count.clone();
    let on_changed: Rc<dyn Fn(&u32)> = Rc::new(move |_| counted.set(counted.get() + 1));
    observer.start();
    observer.observe_reads(0u32, &on_changed, || {
        state.text();
    });
    act();
    Snapshot::send_apply_notifications();
    observer.stop();
    count.get()
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_invalidates_whenSelectionChanged
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_invalidates_whenTextChanged
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_doesNotInvalidate_whenNoChangesMade
#[test]
fn an_edit_invalidates_its_readers_only_if_it_changed_something() {
    let selected = TextFieldState::with_selection("hello", TextRange::at(0));
    let selected = *selected;
    let count = invalidations_of_text_after(selected, || {
        selected.edit(|buffer| buffer.set_selection(range(0, buffer.length())));
    });
    assert_eq!(count, 1, "a new selection");
    let appended = TextFieldState::with_selection("hello", TextRange::at(0));
    let appended = *appended;
    let count =
        invalidations_of_text_after(appended, || appended.edit(|buffer| buffer.append("1")));
    assert_eq!(count, 1, "a new text");
    let unchanged = TextFieldState::with_selection("hello", TextRange::at(0));
    let unchanged = *unchanged;
    let count = invalidations_of_text_after(unchanged, || {
        unchanged.edit(|buffer| {
            let original = buffer.selection();
            buffer.set_selection(range(0, buffer.length()));
            buffer.set_selection(original);
        });
    });
    assert_eq!(count, 0, "nothing changed");
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_replace_changesValueInPlace
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_replace_changesStateAfterReturn
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_replace_doesNotChangeStateUntilReturn
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_multipleOperations
#[test]
fn an_edit_changes_its_buffer_at_once_and_the_state_once_done() {
    let state = TextFieldState::new("");
    let state = *state;
    state.edit(|buffer| {
        buffer.replace(0, 0, "hello");
        assert_eq!(buffer.to_string(), "hello");
        assert_eq!(buffer.length(), 5);
        buffer.replace(5, 5, "world");
        buffer.replace(5, 5, " ");
        buffer.replace(6, 11, "Compose");
        assert_eq!(buffer.as_str(), "hello Compose");
        assert_eq!(state.text().as_str(), "", "not until the edit is done");
        buffer.place_cursor_at_end();
    });
    assert_eq!(state.text().as_str(), "hello Compose");
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_placeCursorAtEnd
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_placeCursorBeforeChar_simpleCase
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_selectAll
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_selectChars_simpleCase
#[test]
fn an_edit_places_the_cursor_or_selects() {
    let edits: [(Edit, TextRange); 4] = [
        (|buffer| buffer.place_cursor_at_end(), TextRange::at(5)),
        (|buffer| buffer.place_cursor_before_char_at(2), TextRange::at(2)),
        (|buffer| buffer.select_all(), range(0, 5)),
        (|buffer| buffer.set_selection(range(1, 4)), range(1, 4)),
    ];
    for (place, expected) in edits {
        let state = TextFieldState::new("");
        state.edit(|buffer| {
            buffer.replace(0, 0, "hello");
            place(buffer);
        });
        assert_eq!(state.selection(), expected);
    }
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_placeCursorBeforeChar_throws_whenInvalid
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_selectChars_throws_whenInvalid
#[test]
fn placing_past_the_text_panics() {
    let state = TextFieldState::new("");
    state.edit(|buffer| {
        let panics = |place: &dyn Fn(&mut TextFieldBuffer), buffer: &mut TextFieldBuffer| {
            catch_unwind(AssertUnwindSafe(|| place(buffer))).is_err()
        };
        assert!(panics(&|buffer| buffer.place_cursor_before_char_at(500), buffer));
        assert!(panics(&|buffer| buffer.set_selection(range(500, 501)), buffer));
        assert!(panics(&|buffer| buffer.set_selection(range(0, 500)), buffer));
        assert!(panics(&|buffer| buffer.set_selection(range(500, 0)), buffer));
        buffer.place_cursor_at_end();
    });
    assert_eq!(state.selection(), TextRange::ZERO);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, edit_afterEdit
#[test]
fn an_edit_starts_from_the_last() {
    let state = TextFieldState::new("");
    state.edit(|buffer| {
        buffer.replace(0, 0, "hello");
        buffer.place_cursor_at_end();
    });
    state.edit(|buffer| {
        assert_eq!(buffer.as_str(), "hello");
        buffer.replace(5, 5, " world");
        assert_eq!(buffer.as_str(), "hello world");
        buffer.place_cursor_at_end();
    });
    assert_eq!(state.text().as_str(), "hello world");
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, append_char
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, append_charSequence
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, append_charSequence_range
#[test]
fn appending_a_character_a_text_or_part_of_one() {
    let edits: [(Edit, &str); 4] = [
        (|buffer| buffer.append_char('c'), "c"),
        (|buffer| buffer.append("hello"), "hello"),
        (|buffer| buffer.append(&"hello world"[0..5]), "hello"),
        (|buffer| std::fmt::Write::write_fmt(buffer, format_args!("{}{}", 4, 2)).unwrap(), "42"),
    ];
    for (append, expected) in edits {
        let state = TextFieldState::new("");
        state.edit(|buffer| {
            append(buffer);
            buffer.place_cursor_at_end();
        });
        assert_eq!(state.text().as_str(), expected);
    }
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, setTextAndPlaceCursorAtEnd_works
#[test]
fn the_helpers_replace_the_text_and_place_the_cursor() {
    let state = TextFieldState::new("");
    state.set_text_and_place_cursor_at_end("Hello");
    assert_eq!(held(&state), ("Hello".to_string(), TextRange::at(5)));
    state.set_text_and_select_all("World!");
    assert_eq!(held(&state), ("World!".to_string(), range(0, 6)));
    state.clear_text();
    assert_eq!(held(&state), (String::new(), TextRange::ZERO));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, replace_changesAreTracked
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, appendChar_changesAreTracked
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, appendCharSequence_changesAreTracked
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, appendCharSequenceRange_changesAreTracked
#[test]
fn an_edits_changes_are_tracked() {
    let cases: [(&str, Edit, &str, TextRange, TextRange); 4] = [
        (
            "hello world",
            |buffer| buffer.replace(6, 11, "Compose"),
            "hello Compose",
            range(6, 13),
            range(6, 11),
        ),
        ("hello ", |buffer| buffer.append_char('c'), "hello c", range(6, 7), TextRange::at(6)),
        ("hello ", |buffer| buffer.append("world"), "hello world", range(6, 11), TextRange::at(6)),
        (
            "hello ",
            |buffer| buffer.append(&"hello world"[6..11]),
            "hello world",
            range(6, 11),
            TextRange::at(6),
        ),
    ];
    for (initial, edit, expected, now, original) in cases {
        let state = TextFieldState::new(initial);
        state.edit(|buffer| {
            edit(buffer);
            assert_eq!(buffer.as_str(), expected);
            assert_eq!(buffer.changes().change_count(), 1);
            assert_eq!(buffer.changes().get_range(0), now);
            assert_eq!(buffer.changes().get_original_range(0), original);
            buffer.place_cursor_at_end();
        });
    }
}

/// Collects what `block` reads of the state as it changes, as `snapshotFlow { … }.collect`, on
/// an unconfined dispatcher: each send of the apply notifications runs the collector.
fn collect<T: Clone + PartialEq + 'static>(
    block: impl Fn() -> T + 'static,
) -> (Rc<RefCell<Vec<T>>>, Owned<CoroutineScope>) {
    let seen: Rc<RefCell<Vec<T>>> = Rc::default();
    let scope = CoroutineScope::new();
    let log = seen.clone();
    let mut flow = snapshot_flow(block);
    scope.launch(async move {
        while let Some(value) = next(&mut flow).await {
            log.borrow_mut().push(value);
        }
    });
    run_ready();
    (seen, scope)
}

/// Sends the apply notifications and runs what they woke, as the runtime does after a write.
fn sent() {
    Snapshot::send_apply_notifications();
    run_ready();
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlow_fires_immediately
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlow_fires_whenTextChanged
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlow_fires_whenSelectionChanged
#[test]
fn a_snapshot_flow_of_the_state_gives_its_value_and_each_change() {
    let state = TextFieldState::with_selection("hello", TextRange::at(5));
    let handle = *state;
    let (seen, _scope) = collect(move || held(&handle));
    assert_eq!(*seen.borrow(), [("hello".to_string(), TextRange::at(5))]);
    handle.edit(|buffer| {
        buffer.append(" world");
        buffer.place_cursor_before_char_at(0);
    });
    sent();
    assert_eq!(seen.borrow().last(), Some(&("hello world".to_string(), TextRange::at(0))));
    handle.edit(|buffer| buffer.place_cursor_at_end());
    sent();
    assert_eq!(seen.borrow().len(), 3);
    assert_eq!(seen.borrow().last(), Some(&("hello world".to_string(), TextRange::at(11))));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlow_firesTwice_whenEditCalledTwice
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlow_firesOnce_whenMultipleChangesMadeInSingleEdit
#[test]
fn a_snapshot_flow_hears_once_per_edit() {
    let state = TextFieldState::new("");
    let handle = *state;
    let (seen, _scope) = collect(move || handle.text().to_string());
    handle.edit(|buffer| {
        buffer.append("hello");
        buffer.append(" there");
        buffer.place_cursor_at_end();
    });
    sent();
    handle.edit(|buffer| {
        buffer.append(" world");
        buffer.place_cursor_at_end();
    });
    sent();
    assert_eq!(*seen.borrow(), ["", "hello there", "hello there world"]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlow_fires_whenChangeMadeInSnapshotIsApplied
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlow_notFired_whenChangeMadeInSnapshotThenDisposed
#[test]
fn an_edit_in_a_snapshot_shows_once_applied_and_never_if_disposed() {
    for apply in [true, false] {
        let state = TextFieldState::new("");
        let handle = *state;
        let (seen, _scope) = collect(move || handle.text().to_string());
        let snapshot = Snapshot::take_mutable_snapshot(None, None);
        snapshot.enter(|| {
            handle.edit(|buffer| {
                buffer.append("hello");
                buffer.place_cursor_at_end();
            });
        });
        if apply {
            snapshot.apply().check();
        }
        snapshot.dispose();
        sent();
        if apply {
            assert_eq!(*seen.borrow(), ["", "hello"]);
        } else {
            assert_eq!(*seen.borrow(), [""]);
            assert_eq!(handle.text().as_str(), "");
        }
    }
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, snapshotFlowOfText_onlyFiresIfContentChanges
#[test]
fn a_snapshot_flow_of_the_text_hears_only_new_texts() {
    let state = TextFieldState::new("");
    let handle = *state;
    let (seen, _scope) = collect(move || handle.text().to_string());
    let edits: [fn(&mut TextFieldBuffer); 5] = [
        |buffer| buffer.append("a"),
        |buffer| buffer.append("b"),
        |buffer| buffer.place_cursor_before_char_at(0),
        |buffer| buffer.place_cursor_at_end(),
        |buffer| buffer.append("c"),
    ];
    for edit in edits {
        handle.edit(edit);
        sent();
    }
    assert_eq!(*seen.borrow(), ["", "a", "ab", "abc"]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, toString_doesNotReadSnapshotState
#[test]
fn printing_a_state_reads_no_state() {
    let state = TextFieldState::new("hello");
    let read = Rc::new(Cell::new(false));
    let told = read.clone();
    let printed =
        Snapshot::observe(Some(Rc::new(move |_| told.set(true))), None, || state.to_string());
    assert!(!read.get());
    assert_eq!(printed, "TextFieldState(selection=TextRange(5, 5), text=\"hello\")");
}

// ---- TextFieldBufferTest's, through the public API ---------------------------------------

/// A buffer of `marked`'s text, with a cursor at one `_` or a selection between two, as
/// Compose's test's `parseAsTextEditState`.
fn marked_buffer(marked: &str) -> (Owned<TextFieldState>, TextFieldBuffer) {
    let marks: Vec<usize> =
        marked.match_indices('_').enumerate().map(|(index, (at, _))| at - index).collect();
    let text = marked.replace('_', "");
    let selection = match marks.as_slice() {
        [] => TextRange::ZERO,
        [at] => TextRange::at(*at),
        [start, end] => range(*start, *end),
        _ => panic!("too many marks in {marked:?}"),
    };
    let state = TextFieldState::with_selection(text, selection);
    let buffer = state.to_text_field_buffer();
    (state, buffer)
}

/// `buffer`'s text with its cursor or selection marked, as Compose's `toParsableString`.
fn marked(buffer: &TextFieldBuffer) -> String {
    let mut text = buffer.to_string();
    if !text.is_empty() {
        let selection = buffer.selection();
        if !selection.collapsed() {
            text.insert(selection.max(), '_');
        }
        text.insert(selection.min(), '_');
    }
    text
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_empty_textInserted
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorAtStart_textInsertedAtCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorAtStart_textInsertedAfterCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorAtStart_textReplacedAroundCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorAtEnd_textInsertedAtCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorAtEnd_textInsertedBeforeCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorAtEnd_textReplacedAroundCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorInMiddle_textInsertedAtCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorInMiddle_textReplacedJustBeforeCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorInMiddle_textReplacedJustAfterCursor
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenCursorInMiddle_textReplacedAroundCursor
#[test]
fn a_cursor_moves_with_the_edits_around_it() {
    let cases: [(&str, Edit, &str); 11] = [
        ("", |buffer| buffer.append("hello"), "hello_"),
        ("_hello", |buffer| buffer.insert(0, "world"), "world_hello"),
        ("_hello", |buffer| buffer.append("world"), "_helloworld"),
        ("_hello", |buffer| buffer.replace(0, buffer.length(), "foo"), "_foo"),
        ("hello_", |buffer| buffer.append("world"), "helloworld_"),
        ("hello_", |buffer| buffer.insert(0, "world"), "worldhello_"),
        ("hello_", |buffer| buffer.replace(0, buffer.length(), "foo"), "foo_"),
        ("he_llo", |buffer| buffer.insert(2, "foo"), "hefoo_llo"),
        ("he_llo", |buffer| buffer.replace(0, 2, "foo"), "foo_llo"),
        ("he_llo", |buffer| buffer.replace(2, 3, "foo"), "he_foolo"),
        ("he_llo", |buffer| buffer.replace(1, 3, "foo"), "hfoo_lo"),
    ];
    for (initial, edit, expected) in cases {
        let (_state, mut buffer) = marked_buffer(initial);
        edit(&mut buffer);
        assert_eq!(marked(&buffer), expected, "from {initial}");
    }
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_allReplacedWithShorter
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_allReplacedWithLonger
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_textReplacedInsideSelection_withLonger
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_textReplacedInsideSelection_withShorter
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_textReplacedInsideFromStart_withLonger
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_textReplacedInsideFromStart_withShorter
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_textReplacedInsideToEnd_withLonger
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenAllSelected_textReplacedInsideToEnd_withShorter
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenInsideSelected_textReplacedJustBeforeSelection
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenInsideSelected_textReplacedJustAfterSelection
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenInsideSelected_textReplacedAroundStart
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectionAdjusted_whenInsideSelected_textReplacedAroundEnd
#[test]
fn a_selection_moves_and_shrinks_with_the_edits_around_and_in_it() {
    let cases: [(&str, Edit, &str); 12] = [
        ("_hello_", |buffer| buffer.replace(0, buffer.length(), "foo"), "_foo_"),
        ("_hello_", |buffer| buffer.replace(0, buffer.length(), "abracadabra"), "_abracadabra_"),
        ("_hello_", |buffer| buffer.replace(1, 4, "world"), "_hworldo_"),
        ("_hello_", |buffer| buffer.replace(1, 4, "w"), "_hwo_"),
        ("_hello_", |buffer| buffer.replace(0, 3, "world"), "_worldlo_"),
        ("_hello_", |buffer| buffer.replace(1, 3, "w"), "_hwlo_"),
        (
            "_hello_",
            |buffer| buffer.replace(buffer.length() - 3, buffer.length(), "world"),
            "_heworld_",
        ),
        ("_hello_", |buffer| buffer.replace(buffer.length() - 3, buffer.length(), "w"), "_hew_"),
        ("hel_lo_", |buffer| buffer.replace(0, 3, "world"), "world_lo_"),
        ("_he_llo", |buffer| buffer.replace(2, buffer.length(), "world"), "_he_world"),
        ("h_ello_", |buffer| buffer.replace(0, 3, "world"), "world_lo_"),
        ("_hell_o", |buffer| buffer.replace(2, buffer.length(), "world"), "_he_world"),
    ];
    for (initial, edit, expected) in cases {
        let (_state, mut buffer) = marked_buffer(initial);
        edit(&mut buffer);
        assert_eq!(marked(&buffer), expected, "from {initial}");
    }
}

fn panics(edit: impl FnOnce()) -> bool {
    catch_unwind(AssertUnwindSafe(edit)).is_err()
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, placeCursorBeforeCharAt_emptyBuffer
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, placeCursorBeforeCharAt_nonEmptyBuffer
#[test]
fn a_cursor_goes_before_any_character_or_at_the_end() {
    let empty = TextFieldState::new("");
    let mut buffer = empty.to_text_field_buffer();
    buffer.place_cursor_before_char_at(0);
    assert_eq!(buffer.selection(), TextRange::at(0));
    assert!(panics(|| buffer.place_cursor_before_char_at(1)));
    let hello = TextFieldState::new("hello");
    let mut buffer = hello.to_text_field_buffer();
    for index in [0, 1, 5] {
        buffer.place_cursor_before_char_at(index);
        assert_eq!(buffer.selection(), TextRange::at(index));
    }
    assert!(panics(|| buffer.place_cursor_before_char_at(6)));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, placeCursorAfterCharAt_emptyBuffer
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, placeCursorAfterCharAt_nonEmptyBuffer
#[test]
fn a_cursor_goes_after_any_character() {
    let empty = TextFieldState::new("");
    let mut buffer = empty.to_text_field_buffer();
    assert!(panics(|| buffer.place_cursor_after_char_at(0)));
    assert!(panics(|| buffer.place_cursor_after_char_at(1)));
    let hello = TextFieldState::new("hello");
    let mut buffer = hello.to_text_field_buffer();
    for (index, expected) in [(0, 1), (1, 2), (4, 5)] {
        buffer.place_cursor_after_char_at(index);
        assert_eq!(buffer.selection(), TextRange::at(expected));
    }
    assert!(panics(|| buffer.place_cursor_after_char_at(5)));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectCharsIn_emptyBuffer
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, selectCharsIn_nonEmptyBuffer
#[test]
fn a_selection_is_anything_within_the_text() {
    let empty = TextFieldState::new("");
    let mut buffer = empty.to_text_field_buffer();
    buffer.set_selection(TextRange::at(0));
    assert_eq!(buffer.selection(), TextRange::at(0));
    assert!(panics(|| buffer.set_selection(range(0, 1))));
    let hello = TextFieldState::new("hello");
    let mut buffer = hello.to_text_field_buffer();
    for selection in [TextRange::at(0), range(0, 1), range(0, 5), range(4, 5), range(5, 5)] {
        buffer.set_selection(selection);
        assert_eq!(buffer.selection(), selection);
    }
    assert!(panics(|| buffer.set_selection(range(5, 6))));
    assert!(panics(|| buffer.set_selection(range(6, 6))));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, charAt_throws_whenEmpty
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, charAt_throws_whenOutOfBounds
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, charAt_returnsChars
#[test]
fn a_character_is_read_within_the_text_only() {
    let empty = TextFieldState::new("");
    let buffer = empty.to_text_field_buffer();
    assert!(panics(|| {
        buffer.char_at(0);
    }));
    let ab = TextFieldState::new("ab");
    let buffer = ab.to_text_field_buffer();
    assert_eq!((buffer.char_at(0), buffer.char_at(1)), ('a', 'b'));
    assert!(panics(|| {
        buffer.char_at(2);
    }));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, asCharSequence_isViewOfBuffer
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, changeList_alwaysReturnsTheLatestChanges
#[test]
fn the_text_and_changes_read_are_the_latest() {
    let state = TextFieldState::new("");
    let mut buffer = state.to_text_field_buffer();
    assert_eq!(buffer.as_str(), "");
    buffer.insert(0, "hello");
    assert_eq!(buffer.as_str(), "hello");
    let changes = buffer.changes();
    assert_eq!(changes.change_count(), 1);
    assert_eq!(changes.get_original_range(0), TextRange::at(0));
    assert_eq!(changes.get_range(0), range(0, 5));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, replace_throws_whenStartGreaterThanEnd
#[test]
#[should_panic(expected = "Expected start=3 <= end=2")]
fn replacing_a_backward_range_panics() {
    let state = TextFieldState::new("ABCD");
    state.to_text_field_buffer().replace(3, 2, "");
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, createFromTextFieldState
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldBufferTest.kt, changesToBuffer_doesNotPropagateToTextFieldState
#[test]
fn a_buffer_of_a_state_starts_from_it_and_leaves_it_alone() {
    let state = TextFieldState::with_selection("Hello", TextRange::at(3));
    let mut buffer = state.to_text_field_buffer();
    assert_eq!(buffer.as_str(), "Hello");
    assert_eq!(buffer.selection(), TextRange::at(3));
    assert_eq!(buffer.changes().change_count(), 0);
    assert_eq!((buffer.original_text(), buffer.original_selection()), ("Hello", TextRange::at(3)));
    buffer.replace(0, 5, "World");
    assert_eq!(buffer.as_str(), "World");
    assert_eq!(state.text().as_str(), "Hello");
}

/// A transformation-shaped edit: deleting what each change inserted past a length, iterating
/// while it edits, as Compose's `maxLengthTrim` does with `forEachChangeReversed`.
#[test]
fn iterating_the_changes_can_edit_the_buffer() {
    let state = TextFieldState::new("ab");
    state.edit(|buffer| {
        buffer.append("cd");
        buffer.insert(0, "xy");
        let mut visited = Vec::new();
        buffer.changes().for_each_change(|now, original| visited.push((now, original)));
        assert_eq!(visited, [(range(0, 2), TextRange::at(0)), (range(4, 6), TextRange::at(2))]);
        buffer.for_each_change_reversed(|buffer, now, _| buffer.delete(now.min(), now.max()));
        assert_eq!(buffer.as_str(), "ab");
    });
    assert_eq!(state.text().as_str(), "ab");
}

// ---- Saving --------------------------------------------------------------------------------

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateSaverTest.kt, savesAndRestoresTextAndSelection
#[test]
fn the_saver_keeps_the_text_and_selection() {
    let state = TextFieldState::with_selection("hello, world", range(0, 5));
    let saver = TextFieldState::saver();
    let saved = saver.save(&state).expect("saved");
    let restored = saver.restore(&saved).expect("restored");
    assert_eq!(held(&restored), ("hello, world".to_string(), range(0, 5)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/RememberTextFieldStateTest.kt, rememberTextFieldState_withInitialTextAndSelection
#[test]
fn a_remembered_state_starts_with_its_initial_text_and_selection() {
    let remembered: Rc<Cell<Option<TextFieldState>>> = Rc::default();
    let kept = remembered.clone();
    let mut ui = compose_test(move || {
        kept.set(Some(remember_text_field_state!("hello", Some(TextRange::at(2)))));
    });
    ui.wait_for_idle();
    let state = remembered.get().expect("composed");
    assert_eq!(held(&state), ("hello".to_string(), TextRange::at(2)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/RememberTextFieldStateTest.kt, rememberTextFieldState_restoresTextAndSelection
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/RememberTextFieldStateTest.kt, rememberTextFieldState_withInitialTextAndSelection_restoresTextAndSelection
#[test]
fn a_remembered_state_comes_back_after_recreation_whatever_its_initial_text() {
    for initial in ["", "this should be ignored"] {
        let states: Rc<RefCell<Vec<TextFieldState>>> = Rc::default();
        let kept = states.clone();
        let mut tester = StateRestorationTester::new();
        let mut ui = compose_test(|| {});
        tester.set_content(&mut ui, move || {
            let state = remember_text_field_state!(initial, Some(TextRange::ZERO));
            let mut kept = kept.borrow_mut();
            if kept.last() != Some(&state) {
                kept.push(state);
            }
        });
        ui.wait_for_idle();
        let original = states.borrow()[0];
        original.edit(|buffer| {
            buffer.replace(0, buffer.length(), "hello, world");
            buffer.select_all();
        });
        tester.emulate_save_and_restore(&mut ui);
        let restored = *states.borrow().last().expect("restored");
        assert_ne!(restored, original, "a new state");
        assert_eq!(held(&restored), ("hello, world".to_string(), range(0, 12)), "{initial:?}");
    }
}

// ---- The field over a state ----------------------------------------------------------------

/// A state-based field over `state`, 200 by 40, tagged "field".
#[composable]
fn Field(state: TextFieldState, line_limits: TextFieldLineLimits) {
    BasicTextFieldOfState!(
        state,
        modifier = Modifier.size((200.dp(), 40.dp())).test_tag("field"),
        line_limits = line_limits,
    );
}

fn field(text: &str, line_limits: TextFieldLineLimits) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::new(text)));
    let mut ui = compose_test(move || Field(state, line_limits));
    ui.wait_for_idle();
    (ui, state)
}

fn focused(text: &str) -> (ComposeTestRule, TextFieldState) {
    let (mut ui, state) = field(text, TextFieldLineLimits::DEFAULT);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    (ui, state)
}

/// What the field's semantics say: its text and selection.
fn shown(ui: &mut ComposeTestRule) -> (String, TextRange) {
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    let text = node.config.get(&P::EDITABLE_TEXT).map(|text| text.to_string()).unwrap_or_default();
    (text, *node.config.get(&P::TEXT_SELECTION_RANGE).expect("a field's selection"))
}

/// Typing, deleting and moving the cursor go into the state at once.
#[test]
fn what_is_typed_goes_into_the_state() {
    let (mut ui, state) = focused("ab");
    assert_eq!(state.selection(), TextRange::at(2), "the state's cursor, at the end");
    ui.type_text("c");
    assert_eq!(held(&state), ("abc".to_string(), TextRange::at(3)));
    ui.press_key(Key::DirectionLeft);
    assert_eq!(state.selection(), TextRange::at(2));
    ui.press_key(Key::Backspace);
    assert_eq!(held(&state), ("ac".to_string(), TextRange::at(1)));
    assert_eq!(shown(&mut ui), ("ac".to_string(), TextRange::at(1)));
}

/// An edit of the state shows in the field, which types on from its selection.
#[test]
fn an_edit_of_the_state_shows_and_typing_follows_it() {
    let (mut ui, state) = focused("hello");
    state.edit(|buffer| {
        buffer.replace(0, 5, "world");
        buffer.place_cursor_before_char_at(0);
    });
    ui.wait_for_idle();
    assert_eq!(shown(&mut ui), ("world".to_string(), TextRange::at(0)));
    ui.type_text("a ");
    assert_eq!(state.text().as_str(), "a world");
}

/// An edit of the state between two keys of one frame is what the second key edits.
#[test]
fn an_edit_between_two_keys_is_what_the_next_key_edits() {
    let (mut ui, state) = focused("ab");
    ui.run_without_implicit_wait(|ui| {
        ui.host().send_text("c");
        state.set_text_and_place_cursor_at_end("xyz");
        ui.host().send_text("!");
    });
    ui.wait_for_idle();
    assert_eq!(held(&state), ("xyz!".to_string(), TextRange::at(4)));
    assert_eq!(shown(&mut ui).0, "xyz!");
}

/// Two fields over one state show the same text: what one types, the other shows.
#[test]
fn two_fields_over_one_state_agree() {
    let state = **Box::leak(Box::new(TextFieldState::new("")));
    let mut ui = compose_test(move || {
        Column!({
            BasicTextFieldOfState!(state, modifier = Modifier.width(200.dp()).test_tag("first"));
            BasicTextFieldOfState!(state, modifier = Modifier.width(200.dp()).test_tag("second"));
        });
    });
    ui.on_node_with_tag("first").request_focus();
    ui.type_text("hi");
    ui.wait_for_idle();
    let second = ui.on_node_with_tag("second").fetch_semantics_snapshot();
    assert_eq!(
        second.config.get(&P::EDITABLE_TEXT).map(|text| text.to_string()).as_deref(),
        Some("hi")
    );
}

/// An input method's edits go into the state, its composition too, and don't restart it.
#[test]
fn an_input_methods_edits_and_composition_go_into_the_state() {
    let (mut ui, state) = focused("");
    ui.host().take_input_method_restart();
    assert!(ui.host().send_edit_commands(&[&SetComposingTextCommand::new("hel", 1)]));
    ui.wait_for_idle();
    assert_eq!(held(&state), ("hel".to_string(), TextRange::at(3)));
    assert_eq!(state.composition(), Some(range(0, 3)));
    assert!(ui.host().send_edit_commands(&[&CommitTextCommand::new("hello", 1)]));
    ui.wait_for_idle();
    assert_eq!(held(&state), ("hello".to_string(), TextRange::at(5)));
    assert_eq!(state.composition(), None);
    assert!(!ui.host().take_input_method_restart(), "its own edits");
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!((value.text(), value.selection()), ("hello", TextRange::at(5)));
}

/// An edit of the state under a composition ends it and restarts the input method, as Compose's
/// rule; one without a composition only tells it the new text.
#[test]
fn an_edit_of_the_state_restarts_the_input_method_only_under_a_composition() {
    let (mut ui, state) = focused("");
    ui.host().take_input_method_restart();
    state.set_text_and_place_cursor_at_end("plain");
    ui.wait_for_idle();
    assert!(!ui.host().take_input_method_restart(), "nothing was composed");
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!(value.text(), "plain", "what the keyboard reads");
    assert!(ui.host().send_edit_commands(&[&SetComposingTextCommand::new("wor", 1)]));
    ui.wait_for_idle();
    state.edit(|buffer| buffer.append("ld"));
    ui.wait_for_idle();
    assert!(ui.host().take_input_method_restart(), "under a composition");
    assert_eq!(state.composition(), None);
    assert_eq!(ui.host().input_method_value().expect("a value").composition(), None);
}

/// A selection the input method sets is the state's.
#[test]
fn an_input_methods_selection_is_the_states() {
    let (mut ui, state) = focused("hello");
    assert!(ui.host().send_edit_commands(&[&SetSelectionCommand::new(1, 3)]));
    ui.wait_for_idle();
    assert_eq!(state.selection(), range(1, 3));
}

/// Losing the focus collapses the selection to its end, in the state too.
#[test]
fn losing_the_focus_collapses_the_states_selection() {
    let (mut ui, state) = focused("hello");
    state.edit(|buffer| buffer.set_selection(range(1, 3)));
    ui.wait_for_idle();
    assert_eq!(shown(&mut ui).1, range(1, 3));
    ui.host().with_tree(|tree| tree.clear_focus());
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::at(3));
}

/// A single line is one line high however many lines its text has; a multi-line field is between
/// its fewest and most lines high, as the value-based field's `min_lines` and `max_lines`.
#[test]
fn line_limits_bound_the_height() {
    let height_of = |ui: &mut ComposeTestRule| {
        ui.wait_for_idle();
        ui.on_node_with_tag("field").get_unclipped_bounds_in_root().height()
    };
    let state_height = |text: &str, limits: TextFieldLineLimits| {
        let state = **Box::leak(Box::new(TextFieldState::new(text)));
        let mut ui = compose_test(move || {
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.test_tag("field"),
                line_limits = limits
            );
        });
        height_of(&mut ui)
    };
    let value_height = |text: &'static str, single_line: bool, min_lines: u32, max_lines: u32| {
        let mut ui = compose_test(move || {
            BasicTextField!(
                text.to_string(),
                |_| {},
                modifier = Modifier.test_tag("field"),
                single_line = single_line,
                min_lines = min_lines,
                max_lines = max_lines,
            );
        });
        height_of(&mut ui)
    };
    let one = state_height("a", TextFieldLineLimits::SingleLine);
    assert!(one.0 > 0.0);
    assert_eq!(state_height("a\nb\nc", TextFieldLineLimits::SingleLine), one);
    assert_eq!(one, value_height("a\nb\nc", true, 1, 1));
    let three = state_height("a", TextFieldLineLimits::multi_line(3, 5));
    assert_eq!(three, value_height("a", false, 3, 5));
    assert!(three > one);
    assert_eq!(
        state_height("a\nb\nc\nd\ne\nf", TextFieldLineLimits::multi_line(1, 2)),
        state_height("a\nb", TextFieldLineLimits::DEFAULT)
    );
}

#[test]
#[should_panic(expected = "Expected 1 ≤ minHeightInLines ≤ maxHeightInLines, were 3, 2")]
fn line_limits_out_of_order_panic() {
    TextFieldLineLimits::multi_line(3, 2);
}

/// A read-only field takes no typing, and its state stays.
#[test]
fn a_read_only_field_leaves_its_state() {
    let state = **Box::leak(Box::new(TextFieldState::new("fixed")));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(200.dp()).test_tag("field"),
            read_only = true
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui.type_text("x");
    ui.wait_for_idle();
    assert_eq!(state.text().as_str(), "fixed");
}

/// Undo goes back over what was typed, in the state too (the field's own history for now, D1131).
#[test]
fn undo_goes_back_in_the_state() {
    let (mut ui, state) = focused("");
    ui.type_text("abc");
    shortcut(&mut ui, 'z');
    assert_eq!(state.text().as_str(), "");
}

/// Cmd+`c` on macOS, Ctrl+`c` elsewhere, down and up.
fn shortcut(ui: &mut ComposeTestRule, c: char) {
    let modifiers = if cfg!(target_os = "macos") {
        compose_ui::KeyModifiers { meta: true, ..Default::default() }
    } else {
        compose_ui::KeyModifiers { ctrl: true, ..Default::default() }
    };
    for kind in [compose_ui::KeyEventType::KeyDown, compose_ui::KeyEventType::KeyUp] {
        let key = Key::Character(c);
        ui.host().send_key(compose_ui::KeyEvent {
            key,
            kind,
            modifiers,
            from_soft_keyboard: false,
        });
    }
    ui.wait_for_idle();
}

/// An edit of the state ends the field's history there: undo doesn't go back past it, and the
/// edit stays (D1131).
#[test]
fn undo_stops_at_an_edit_of_the_state() {
    let (mut ui, state) = focused("");
    ui.type_text("abc");
    state.edit(|buffer| buffer.append("X"));
    ui.wait_for_idle();
    ui.type_text("d");
    shortcut(&mut ui, 'z');
    shortcut(&mut ui, 'z');
    assert_eq!(state.text().as_str(), "abcX");
}

/// Cut and paste without a composition don't restart the input method: the state's rule
/// restarts it only under one, unlike the value-based field (D1664).
#[test]
fn cut_and_paste_restart_nothing_without_a_composition() {
    let (mut ui, state) = focused("hello");
    state.edit(|buffer| buffer.select_all());
    ui.wait_for_idle();
    ui.host().take_input_method_restart();
    shortcut(&mut ui, 'x');
    assert_eq!(state.text().as_str(), "");
    assert!(!ui.host().take_input_method_restart(), "cut");
    shortcut(&mut ui, 'v');
    assert_eq!(state.text().as_str(), "hello");
    assert!(!ui.host().take_input_method_restart(), "paste");
}

/// Backspace under a composition restarts the input method when a hardware keyboard sent it, not
/// when the on-screen keyboard did, as Compose's `restartImeIfContentChanges =
/// !isFromSoftKeyboard`.
#[test]
fn a_soft_keyboards_key_under_a_composition_restarts_nothing() {
    for soft in [true, false] {
        let (mut ui, state) = focused("");
        assert!(ui.host().send_edit_commands(&[&SetComposingTextCommand::new("hel", 1)]));
        ui.wait_for_idle();
        ui.host().take_input_method_restart();
        for kind in [compose_ui::KeyEventType::KeyDown, compose_ui::KeyEventType::KeyUp] {
            let modifiers = compose_ui::KeyModifiers::default();
            let key = Key::Backspace;
            ui.host().send_key(compose_ui::KeyEvent {
                key,
                kind,
                modifiers,
                from_soft_keyboard: soft,
            });
        }
        ui.wait_for_idle();
        assert_eq!(state.text().as_str(), "he");
        assert_eq!(ui.host().take_input_method_restart(), !soft, "soft: {soft}");
    }
}

/// A field turned read-only ends what was composed, in its state at once: what's composed after
/// it in the same pass reads no composition.
#[test]
fn a_field_turned_read_only_ends_the_states_composition() {
    let state = **Box::leak(Box::new(TextFieldState::new("")));
    let read_only = Box::leak(Box::new(mutable_state_of(false))).handle();
    let seen: Readings = Rc::default();
    let log = seen.clone();
    let mut ui = compose_test(move || {
        let log = log.clone();
        Column!({
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.width(200.dp()).test_tag("field"),
                read_only = read_only.get(),
            );
            Reader(read_only, state, log.clone());
        });
    });
    ui.on_node_with_tag("field").request_focus();
    assert!(ui.host().send_edit_commands(&[&SetComposingTextCommand::new("hel", 1)]));
    ui.wait_for_idle();
    assert_eq!(state.composition(), Some(range(0, 3)));
    read_only.set(true);
    ui.wait_for_idle();
    assert_eq!(state.composition(), None);
    assert_eq!(state.text().as_str(), "hel");
    let stale: Vec<_> = seen
        .borrow()
        .iter()
        .filter(|(read_only, composition)| *read_only && composition.is_some())
        .cloned()
        .collect();
    assert!(stale.is_empty(), "read while read-only: {:?}", seen.borrow());
}

/// What `Reader` saw, each time: read-only, and the state's composition.
type Readings = Rc<RefCell<Vec<(bool, Option<TextRange>)>>>;

/// Records whether the field is read-only and what its state composes, each time it's composed.
#[composable]
fn Reader(read_only: MutableState<bool>, state: TextFieldState, log: Readings) {
    log.borrow_mut().push((read_only.get(), state.composition()));
}
