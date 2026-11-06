//! `TextFieldState`, what the state-based text field edits, as Compose's `TextFieldState`,
//! `rememberTextFieldState` and `TextFieldLineLimits` (docs/parity/design/text-field-state.md).

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use compose_runtime::snapshots::Snapshot;
use compose_runtime::{
    ComposeArg, MutableState, ObjectHandle, Owned, OwnedState, Saveable, SavedValue, Saver,
    composable, mutable_state_of,
};
use compose_ui::{SharedString, TextRange};

use crate::text_field_buffer::{TextFieldBuffer, TextFieldCharSequence};
use crate::text_undo::{TextFieldEditUndoBehavior, TextUndoManager, UndoState};

/// Told of each new value, as Compose's `NotifyImeListener`: the value before, the value now, and
/// whether the input method must start over.
pub(crate) type NotifyImeListener =
    Rc<dyn Fn(&TextFieldCharSequence, &TextFieldCharSequence, bool)>;

/// What a `TextFieldState`'s owner keeps.
struct TextFieldStates {
    value: OwnedState<TextFieldCharSequence>,
    /// An `edit` is running: another, nested, panics. A state, as Compose's: two snapshots may
    /// each edit.
    is_editing: OwnedState<bool>,
    listeners: RefCell<Vec<(u64, NotifyImeListener)>>,
    next_listener: Cell<u64>,
    /// Its history, as Compose's `textUndoManager`.
    undo: TextUndoManager,
}

/// The text, selection and composition of a text field, as Compose's `TextFieldState`: what
/// [`BasicTextFieldOfState`](crate::BasicTextFieldOfState) edits, and an application reads (state:
/// reading it in composition recomposes on each change) and changes with
/// [`edit`](Self::edit).
///
/// A `Copy` handle (D240): [`new`](Self::new) and [`with_selection`](Self::with_selection)
/// return its owner, [`remember_text_field_state`] keeps it in the composition and saves it. Once
/// its owner went, it reads empty and edits do nothing. Offsets are UTF-8 byte offsets (D1100).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TextFieldState {
    value: MutableState<TextFieldCharSequence>,
    is_editing: MutableState<bool>,
    object: ObjectHandle<TextFieldStates>,
}

impl TextFieldState {
    /// A state of `initial_text`, the cursor at its end, as `TextFieldState(initialText)`.
    #[track_caller]
    pub fn new(initial_text: impl Into<String>) -> Owned<TextFieldState> {
        let text = initial_text.into();
        let end = text.len();
        TextFieldState::with_selection(text, TextRange::at(end))
    }

    /// A state of `initial_text` with `initial_selection` (kept within the text), as
    /// `TextFieldState(initialText, initialSelection)`.
    #[track_caller]
    pub fn with_selection(
        initial_text: impl Into<String>,
        initial_selection: TextRange,
    ) -> Owned<TextFieldState> {
        TextFieldState::restored(initial_text.into(), initial_selection, TextUndoManager::default())
    }

    /// A state with a history, as Compose's internal constructor.
    #[track_caller]
    fn restored(
        initial_text: String,
        initial_selection: TextRange,
        undo: TextUndoManager,
    ) -> Owned<TextFieldState> {
        let text: Rc<str> = Rc::from(initial_text);
        let value = TextFieldCharSequence::new(text, initial_selection, None);
        let states = TextFieldStates {
            value: mutable_state_of(value),
            is_editing: mutable_state_of(false),
            listeners: RefCell::default(),
            next_listener: Cell::new(0),
            undo,
        };
        let (value, is_editing) = (states.value.handle(), states.is_editing.handle());
        Owned::new(states, move |object| TextFieldState { value, is_editing, object })
    }

    /// The value, read as state; `None` once the owner went (said in debug builds).
    pub(crate) fn current(&self) -> Option<TextFieldCharSequence> {
        let value = self.value.try_get();
        if value.is_none() {
            self.object.warn_if_freed();
        }
        value
    }

    /// The value, read without observing it.
    pub(crate) fn current_untracked(&self) -> Option<TextFieldCharSequence> {
        Snapshot::without_read_observation(|| self.current())
    }

    /// The text, as `text`.
    pub fn text(&self) -> SharedString {
        self.current().map_or(SharedString::EMPTY, |value| SharedString::from(value.text))
    }

    /// The selection; a collapsed one is the cursor.
    pub fn selection(&self) -> TextRange {
        self.current().map_or(TextRange::ZERO, |value| value.selection)
    }

    /// What an input method is composing, if anything.
    pub fn composition(&self) -> Option<TextRange> {
        self.current().and_then(|value| value.composition)
    }

    /// Edits the text and selection in `block`, then commits what it changed, as Compose's `edit`:
    /// the state changes once `block` returns, and only if something changed; a change of the
    /// text or selection ends what an input method composed. Panics if called inside another
    /// `edit` of this state, as Compose's throws; a panic in `block` leaves the state as it was.
    pub fn edit(&self, block: impl FnOnce(&mut TextFieldBuffer)) {
        let Some(value) = self.current() else { return };
        let editing = Snapshot::without_read_observation(|| self.is_editing.get());
        assert!(!editing, "TextFieldState does not support concurrent or nested editing.");
        self.is_editing.set(true);
        /// Ends the edit, whether `block` returned or panicked, as Compose's `finishEditing`.
        struct Finish(MutableState<bool>);
        impl Drop for Finish {
            fn drop(&mut self) {
                self.0.try_set(false);
            }
        }
        let _finish = Finish(self.is_editing);
        let mut buffer = TextFieldBuffer::new(value);
        block(&mut buffer);
        self.commit_edit(&buffer);
    }

    /// Takes in what an `edit`'s buffer changed, as Compose's `commitEdit` and
    /// `syncMainBufferToTemporaryBuffer`.
    pub(crate) fn commit_edit(&self, buffer: &TextFieldBuffer) {
        let Some(old) = self.current_untracked() else { return };
        let text_changed = buffer.changes().change_count() > 0;
        let selection_changed = buffer.selection() != old.selection;
        let text = if text_changed { Rc::from(buffer.as_str()) } else { old.text.clone() };
        let selection =
            if text_changed || selection_changed { buffer.selection() } else { old.selection };
        // The input method composes again over a text or selection changed without it.
        let composition =
            if text_changed || selection_changed || old.composition != buffer.composition() {
                None
            } else {
                old.composition
            };
        let new = TextFieldCharSequence::new(text, selection, composition);
        if text_changed {
            // A programmatic edit enters the history, never merging (Compose's `NeverMerge`).
            let post = buffer.to_text_field_char_sequence();
            self.with_undo(|undo| undo.record_changes(&old, &post, buffer.changes(), false));
        }
        self.update_value_and_notify(&old, new, true);
    }

    /// Runs `block` with the history; `None` once the owner went.
    pub(crate) fn with_undo<R>(&self, block: impl FnOnce(&TextUndoManager) -> R) -> Option<R> {
        let states = self.object.get()?;
        Some(block(&states.undo))
    }

    /// The undo and redo of this state, as Compose's `undoState`.
    pub fn undo_state(&self) -> UndoState {
        UndoState { state: *self }
    }

    /// Edits outside the history and any transformation, as Compose's `editWithNoSideEffects`:
    /// undo and redo. The input method hears of it.
    pub(crate) fn edit_with_no_side_effects(&self, block: impl FnOnce(&mut TextFieldBuffer)) {
        let Some(before) = self.current_untracked() else { return };
        let mut buffer = TextFieldBuffer::new(before.clone());
        block(&mut buffer);
        let after = buffer.to_text_field_char_sequence();
        self.update_value_and_notify(&before, after, true);
    }

    /// Edits for the user (keys, the pointer, an input method, semantics), as Compose's
    /// `editAsUser` without an input transformation: `block` edits a buffer of the value, whose
    /// change, composition included, is taken as it is. The input method starts over only if the
    /// text changed under a composition and `restart_ime_if_content_changes` (false for its own
    /// edits). The edit enters the history by `undo_behavior`.
    pub(crate) fn edit_as_user(
        &self,
        restart_ime_if_content_changes: bool,
        undo_behavior: TextFieldEditUndoBehavior,
        block: impl FnOnce(&mut TextFieldBuffer),
    ) {
        let Some(before) = self.current_untracked() else { return };
        let mut buffer = TextFieldBuffer::new(before.clone());
        block(&mut buffer);
        let changes = buffer.changes().change_count();
        if changes == 0 && before.selection == buffer.selection() {
            if before.composition != buffer.composition() {
                let after = TextFieldCharSequence::new(
                    before.text.clone(),
                    buffer.selection(),
                    buffer.composition(),
                );
                self.update_value_and_notify(&before, after, restart_ime_if_content_changes);
            }
            return;
        }
        let text = if changes > 0 { Rc::from(buffer.as_str()) } else { before.text.clone() };
        let after = TextFieldCharSequence::new(text, buffer.selection(), buffer.composition());
        self.update_value_and_notify(
            &before,
            after.clone(),
            changes > 0 && restart_ime_if_content_changes,
        );
        self.with_undo(|undo| match undo_behavior {
            TextFieldEditUndoBehavior::MergeIfPossible => {
                undo.record_changes(&before, &after, buffer.changes(), true)
            }
            TextFieldEditUndoBehavior::NeverMerge => {
                undo.record_changes(&before, &after, buffer.changes(), false)
            }
        });
    }

    /// Sets the value and tells the listeners, as Compose's `updateValueAndNotifyListeners`.
    fn update_value_and_notify(
        &self,
        old: &TextFieldCharSequence,
        new: TextFieldCharSequence,
        restart_ime_if_content_changes: bool,
    ) {
        let restart_ime = restart_ime_if_content_changes
            && !old.content_equals(&new)
            && old.composition.is_some();
        self.value.try_set(new.clone());
        let Some(states) = self.object.try_get() else { return };
        if states.listeners.borrow().is_empty() {
            return;
        }
        let listeners: Vec<NotifyImeListener> =
            states.listeners.borrow().iter().map(|(_, listener)| listener.clone()).collect();
        for listener in listeners {
            listener(old, &new, restart_ime);
        }
    }

    /// Adds `listener`, told of each new value; returns its id, for
    /// [`remove_notify_ime_listener`](Self::remove_notify_ime_listener).
    pub(crate) fn add_notify_ime_listener(
        &self,
        listener: impl Fn(&TextFieldCharSequence, &TextFieldCharSequence, bool) + 'static,
    ) -> Option<u64> {
        let states = self.object.try_get()?;
        let id = states.next_listener.get();
        states.next_listener.set(id + 1);
        states.listeners.borrow_mut().push((id, Rc::new(listener)));
        Some(id)
    }

    pub(crate) fn remove_notify_ime_listener(&self, id: u64) {
        if let Some(states) = self.object.try_get() {
            states.listeners.borrow_mut().retain(|(listener, _)| *listener != id);
        }
    }

    /// Replaces the text with `text` and puts the cursor at its end, as
    /// `setTextAndPlaceCursorAtEnd`.
    pub fn set_text_and_place_cursor_at_end(&self, text: &str) {
        self.edit(|buffer| {
            buffer.replace(0, buffer.length(), text);
            buffer.place_cursor_at_end();
        });
    }

    /// Replaces the text with `text` and selects it all, as `setTextAndSelectAll`.
    pub fn set_text_and_select_all(&self, text: &str) {
        self.edit(|buffer| {
            buffer.replace(0, buffer.length(), text);
            buffer.select_all();
        });
    }

    /// Deletes the whole text, as `clearText`.
    pub fn clear_text(&self) {
        self.edit(|buffer| {
            buffer.delete(0, buffer.length());
            buffer.place_cursor_at_end();
        });
    }

    /// A buffer of the value, not tied to the state, as `toTextFieldBuffer`: what it's given
    /// doesn't change the state.
    pub fn to_text_field_buffer(&self) -> TextFieldBuffer {
        let value = self
            .current()
            .unwrap_or_else(|| TextFieldCharSequence::new(Rc::from(""), TextRange::ZERO, None));
        TextFieldBuffer::new(value)
    }

    /// Saves the text, selection and history, as Compose's `TextFieldState.Saver` (its styles
    /// come with them in Compose; they're not here yet).
    pub fn saver() -> impl Saver<Owned<TextFieldState>> {
        compose_runtime::saver(
            |state: &Owned<TextFieldState>| {
                let value = state.current_untracked()?;
                Some(SavedValue::List(vec![
                    SavedValue::Text(value.text.to_string()),
                    value.selection.start.save(),
                    value.selection.end.save(),
                    state.with_undo(TextUndoManager::save)?,
                ]))
            },
            |saved| {
                let SavedValue::List(items) = saved else { return None };
                let [SavedValue::Text(text), start, end, undo, ..] = items.as_slice() else {
                    return None;
                };
                let selection = TextRange::new(usize::restore(start)?, usize::restore(end)?);
                let undo = TextUndoManager::restore(undo)?;
                Some(TextFieldState::restored(text.clone(), selection, undo))
            },
        )
    }

    /// The editing flag's state, for the tests of what reads it.
    #[cfg(test)]
    pub(crate) fn is_editing_state(&self) -> MutableState<bool> {
        self.is_editing
    }
}

/// `TextFieldState(selection=TextRange(0, 5), text="hello")`, as Compose's `toString`, read
/// without observing the state.
impl fmt::Display for TextFieldState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.current_untracked() {
            Some(value) => write!(
                formatter,
                "TextFieldState(selection={}, text=\"{}\")",
                value.selection, value.text
            ),
            None => formatter.write_str("TextFieldState(freed)"),
        }
    }
}

impl fmt::Debug for TextFieldState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl ComposeArg for TextFieldState {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// A text field state kept across recompositions and saved across the host's recreation, as
/// `rememberTextFieldState(initialText, initialSelection)` (a `rememberSaveable` of its saver).
/// `initial_selection` `None` puts the cursor at the text's end, Compose's default. Values given
/// on later recompositions are ignored: change the state with its methods.
#[composable(non_restartable)]
pub fn remember_text_field_state(
    #[default("")] initial_text: impl Into<String>,
    #[default(None)] initial_selection: Option<TextRange>,
) -> TextFieldState {
    compose_runtime::remember_saveable_with(TextFieldState::saver(), move || {
        let text: String = initial_text.into();
        let selection = initial_selection.unwrap_or(TextRange::at(text.len()));
        TextFieldState::with_selection(text, selection)
    })
}

/// How a text field wraps, scrolls and how high it is, as Compose's `TextFieldLineLimits`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextFieldLineLimits {
    /// One line high; the text doesn't wrap and scrolls sideways, as `SingleLine`.
    SingleLine,
    /// At least `min_height_in_lines` lines high, growing with its text up to
    /// `max_height_in_lines` lines, then scrolling down, as `MultiLine`. Make it with
    /// [`multi_line`](TextFieldLineLimits::multi_line), which checks `1 ≤ min ≤ max`.
    MultiLine { min_height_in_lines: u32, max_height_in_lines: u32 },
}

impl TextFieldLineLimits {
    /// Compose's default: multi-line, from one line high, without a maximum.
    pub const DEFAULT: TextFieldLineLimits =
        TextFieldLineLimits::MultiLine { min_height_in_lines: 1, max_height_in_lines: u32::MAX };

    /// `MultiLine(minHeightInLines, maxHeightInLines)`. Panics unless `1 ≤ min ≤ max`, as
    /// Compose's constructor throws.
    pub fn multi_line(min_height_in_lines: u32, max_height_in_lines: u32) -> TextFieldLineLimits {
        let limits = TextFieldLineLimits::MultiLine { min_height_in_lines, max_height_in_lines };
        limits.validate();
        limits
    }

    /// Panics if it's a `MultiLine` without `1 ≤ min ≤ max`.
    pub(crate) fn validate(self) {
        if let TextFieldLineLimits::MultiLine { min_height_in_lines, max_height_in_lines } = self {
            assert!(
                (1..=max_height_in_lines).contains(&min_height_in_lines),
                "Expected 1 ≤ minHeightInLines ≤ maxHeightInLines, were \
                 {min_height_in_lines}, {max_height_in_lines}"
            );
        }
    }

    /// As the value-based field's parameters: single line, fewest lines, most lines.
    pub(crate) fn lines(self) -> (bool, u32, u32) {
        match self {
            TextFieldLineLimits::SingleLine => (true, 1, 1),
            TextFieldLineLimits::MultiLine { min_height_in_lines, max_height_in_lines } => {
                (false, min_height_in_lines, max_height_in_lines)
            }
        }
    }
}

impl Default for TextFieldLineLimits {
    fn default() -> TextFieldLineLimits {
        TextFieldLineLimits::DEFAULT
    }
}

impl ComposeArg for TextFieldLineLimits {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Edit = fn(&mut TextFieldBuffer);

    /// What a listener heard: the texts before and after, and whether to restart.
    type Heard = Rc<RefCell<Vec<(String, String, bool)>>>;

    fn listen(state: &TextFieldState) -> Heard {
        let heard: Heard = Rc::default();
        let log = heard.clone();
        state.add_notify_ime_listener(move |old, new, restart| {
            log.borrow_mut().push((old.text.to_string(), new.text.to_string(), restart));
        });
        heard
    }

    /// An input method composing `text` at the cursor, as `setComposingText(text, 1)`.
    fn compose_text(state: &TextFieldState, text: &str) {
        state.edit_as_user(false, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
            let at = buffer.selection().min();
            buffer.replace(at, buffer.selection().max(), text);
            buffer.set_composition(at, at + text.len());
        });
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, noOpEdit_doesNot_commitComposition
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, editContentChange_commitsComposition
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, editSelectionChange_commitsComposition
    #[test]
    fn an_edit_ends_a_composition_only_if_it_changed_something() {
        let edits: [(Edit, Option<TextRange>); 3] = [
            (|_| {}, Some(TextRange::new(0, 5))),
            (|buffer| buffer.append(" world"), None),
            (|buffer| buffer.set_selection(TextRange::at(3)), None),
        ];
        for (edit, expected) in edits {
            let state = TextFieldState::new("");
            compose_text(&state, "Hello");
            assert_eq!(state.composition(), Some(TextRange::new(0, 5)));
            state.edit(edit);
            assert_eq!(state.composition(), expected);
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, isEditing_doesNotRegisterSnapshotRead
    #[test]
    fn editing_reads_no_editing_flag() {
        let state = TextFieldState::new("hello");
        let flag = state.is_editing_state().state_object_id();
        let read = Rc::new(Cell::new(false));
        let told = read.clone();
        Snapshot::observe(
            Some(Rc::new(move |id| told.set(told.get() || id == flag))),
            None,
            || {
                state.edit(|buffer| buffer.append(" world"));
            },
        );
        assert!(!read.get());
        assert_eq!(state.text().as_str(), "hello world");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, notifyImeListener_firesAfterProgrammaticEdit
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, notifyImeListener_firesAfterProgrammaticEdit_restartsImeIfComposing
    #[test]
    fn a_programmatic_edit_restarts_the_input_method_only_over_a_composition() {
        for composing in [false, true] {
            let state = TextFieldState::new("Hello");
            if composing {
                state.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
                    buffer.set_composition(0, 5)
                });
            }
            let heard = listen(&state);
            state.edit(|buffer| buffer.append(" World"));
            assert_eq!(
                *heard.borrow(),
                [("Hello".to_string(), "Hello World".to_string(), composing)]
            );
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, notifyImeListener_firesAfterProgrammaticEdit_doesNotRestartIfContentIsSame
    #[test]
    fn a_programmatic_edit_leaving_the_text_restarts_nothing() {
        let state = TextFieldState::new("Hello");
        state.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
            buffer.set_composition(0, 5)
        });
        let heard = listen(&state);
        state.edit(|buffer| {
            buffer.append(" World");
            buffer.delete(5, buffer.length());
        });
        assert_eq!(*heard.borrow(), [("Hello".to_string(), "Hello".to_string(), false)]);
        assert_eq!(state.composition(), None, "the text was edited all the same");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, notifyImeListener_firesAfterUserEdit
    #[test]
    fn an_input_methods_own_edit_restarts_nothing() {
        let state = TextFieldState::new("Hello");
        state.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
            buffer.set_composition(0, 5)
        });
        let heard = listen(&state);
        state.edit_as_user(false, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
            buffer.replace(0, 5, "World");
            buffer.set_composition(0, 5);
        });
        assert_eq!(*heard.borrow(), [("Hello".to_string(), "World".to_string(), false)]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, noOpUserEdit_duringComposition_doesNotUpdateValueOrNotifyIme
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, noOpUserEdit_withoutComposition_doesNotNotifyIme
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, noOpUserEdit_afterProgrammaticEdit_doesNotNotifyIme
    #[test]
    fn a_user_edit_changing_nothing_is_heard_by_nobody() {
        let composing = TextFieldState::new("Hello ");
        compose_text(&composing, "ab");
        assert_eq!(composing.composition(), Some(TextRange::new(6, 8)));
        let committed = TextFieldState::new("Hello");
        committed.edit_as_user(false, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
            buffer.append(" World")
        });
        committed.edit_as_user(false, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
            buffer.place_cursor_at_end()
        });
        let programmatic = TextFieldState::new("Hello");
        programmatic.edit(|buffer| buffer.replace(5, 5, " World"));
        for (state, cursor) in [(&composing, 8), (&committed, 11), (&programmatic, 11)] {
            let before = state.current_untracked();
            let heard = listen(state);
            state.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
                buffer.set_selection(TextRange::at(cursor))
            });
            assert!(heard.borrow().is_empty(), "{}", **state);
            assert_eq!(state.current_untracked(), before);
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, notifyImeListener_restartImeIsFalse_ifOnlySelectionIsChanged
    #[test]
    fn a_new_selection_alone_restarts_nothing() {
        let state = TextFieldState::with_selection("Hello", TextRange::at(3));
        let heard = listen(&state);
        state.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |buffer| {
            buffer.set_selection(TextRange::ZERO)
        });
        assert_eq!(*heard.borrow(), [("Hello".to_string(), "Hello".to_string(), false)]);
        assert_eq!(state.selection(), TextRange::ZERO);
    }

    /// A listener removed hears nothing more.
    #[test]
    fn a_removed_listener_hears_nothing() {
        let state = TextFieldState::new("");
        let heard: Heard = Rc::default();
        let log = heard.clone();
        let id = state
            .add_notify_ime_listener(move |_, _, _| log.borrow_mut().push(Default::default()))
            .expect("a live state");
        state.remove_notify_ime_listener(id);
        state.set_text_and_place_cursor_at_end("x");
        assert!(heard.borrow().is_empty());
    }

    /// Once the owner went, the handle reads empty and edits do nothing (D240).
    #[test]
    fn a_stale_state_reads_empty() {
        let owner = TextFieldState::new("hello");
        let state = *owner;
        drop(owner);
        assert_eq!(state.text().as_str(), "");
        state.edit(|_| panic!("not run"));
        assert_eq!(state.to_string(), "TextFieldState(freed)");
    }
}
