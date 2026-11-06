//! A text field state's undo history, as Compose's `UndoState`, `TextUndoManager`,
//! `TextUndoOperation` and `UndoManager`: edits recorded as operations, merged while typing goes on
//! the way a user expects, kept in the state and saved with it (D1132).

use compose_runtime::snapshots::{OwnedStateList, Snapshot, mutable_state_list_from};
use compose_runtime::{OwnedState, Saveable, SavedValue, mutable_state_of};
use compose_ui::TextRange;

use crate::TextFieldState;
use crate::text_field_buffer::{ChangeList, TextFieldCharSequence};

/// Edits closer than this to the first of their group merge, as Compose's
/// `SNAPSHOTS_INTERVAL_MILLIS`.
const SNAPSHOTS_INTERVAL_MILLIS: u64 = 5000;

/// The most operations a text field's history keeps, as Compose's `TEXT_UNDO_CAPACITY`.
const TEXT_UNDO_CAPACITY: usize = 100;

/// Now, by the executor's clock (a test's clock in tests), as Compose's `timeNowMillis`.
fn now_millis() -> u64 {
    compose_runtime::platform::now_nanos() / 1_000_000
}

/// What an edit is, forwards, as Compose's `TextEditType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TextEditType {
    Insert,
    Delete,
    Replace,
}

/// Which way a deletion went, as Compose's `TextDeleteType`: before the cursor (Backspace), after
/// it (Delete), a selection, or something no key does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TextDeleteType {
    Start,
    End,
    Inner,
    NotByUser,
}

/// How an edit enters the history, as Compose's `TextFieldEditUndoBehavior` (its `ClearHistory`
/// is used nowhere).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TextFieldEditUndoBehavior {
    MergeIfPossible,
    NeverMerge,
}

/// One edit, applied forwards or backwards, as Compose's `TextUndoOperation`: `pre_text` at `index`
/// became `post_text`, the selection `pre_selection` became `post_selection`. Offsets are bytes.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct TextUndoOperation {
    pub(crate) index: usize,
    pub(crate) pre_text: String,
    pub(crate) post_text: String,
    pub(crate) pre_selection: TextRange,
    pub(crate) post_selection: TextRange,
    /// When it was first committed.
    pub(crate) time_millis: u64,
    /// Whether it may merge at all (a cut or a paste may not).
    pub(crate) can_merge: bool,
}

impl TextUndoOperation {
    /// An operation made now. Panics if both texts are empty, as Compose's.
    pub(crate) fn new(
        index: usize,
        pre_text: String,
        post_text: String,
        pre_selection: TextRange,
        post_selection: TextRange,
        can_merge: bool,
    ) -> TextUndoOperation {
        assert!(
            !pre_text.is_empty() || !post_text.is_empty(),
            "Either pre or post text must not be empty"
        );
        TextUndoOperation {
            index,
            pre_text,
            post_text,
            pre_selection,
            post_selection,
            time_millis: now_millis(),
            can_merge,
        }
    }

    pub(crate) fn text_edit_type(&self) -> TextEditType {
        match (self.pre_text.is_empty(), self.post_text.is_empty()) {
            (true, _) => TextEditType::Insert,
            (false, true) => TextEditType::Delete,
            (false, false) => TextEditType::Replace,
        }
    }

    /// Only for deciding whether two deletions merge, as Compose's `deletionType`.
    pub(crate) fn deletion_type(&self) -> TextDeleteType {
        if self.text_edit_type() != TextEditType::Delete || !self.post_selection.collapsed() {
            return TextDeleteType::NotByUser;
        }
        if self.pre_selection.collapsed() {
            if self.pre_selection.start > self.post_selection.start {
                TextDeleteType::Start
            } else {
                TextDeleteType::End
            }
        } else if self.pre_selection.min() == self.post_selection.start
            && self.pre_selection.min() == self.index
        {
            TextDeleteType::Inner
        } else {
            TextDeleteType::NotByUser
        }
    }

    /// A hard line break, which never merges.
    fn is_new_line_insert(&self) -> bool {
        self.post_text == "\n" || self.post_text == "\r\n"
    }

    /// This operation and `next`, which came after it, as one, if they merge, as Compose's
    /// `merge`: both may merge, `next` within 5 s of this one's start, neither a new line, of the
    /// same type; an insertion followed by one at its end, or deletions the same way sharing a
    /// boundary.
    pub(crate) fn merge(&self, next: &TextUndoOperation) -> Option<TextUndoOperation> {
        if !self.can_merge || !next.can_merge {
            return None;
        }
        if next.time_millis < self.time_millis
            || next.time_millis - self.time_millis >= SNAPSHOTS_INTERVAL_MILLIS
        {
            return None;
        }
        if self.is_new_line_insert() || next.is_new_line_insert() {
            return None;
        }
        let edit_type = self.text_edit_type();
        if edit_type != next.text_edit_type() {
            return None;
        }
        let merged = |index: usize, pre_text: String, post_text: String| TextUndoOperation {
            index,
            pre_text,
            post_text,
            pre_selection: self.pre_selection,
            post_selection: next.post_selection,
            time_millis: self.time_millis,
            can_merge: true,
        };
        match edit_type {
            TextEditType::Insert if self.index + self.post_text.len() == next.index => Some(
                merged(self.index, String::new(), format!("{}{}", self.post_text, next.post_text)),
            ),
            TextEditType::Delete => {
                let deletion = self.deletion_type();
                if deletion != next.deletion_type()
                    || !matches!(deletion, TextDeleteType::Start | TextDeleteType::End)
                {
                    return None;
                }
                if self.index == next.index + next.pre_text.len() {
                    Some(merged(
                        next.index,
                        format!("{}{}", next.pre_text, self.pre_text),
                        String::new(),
                    ))
                } else if self.index == next.index {
                    Some(merged(
                        self.index,
                        format!("{}{}", self.pre_text, next.pre_text),
                        String::new(),
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn save(&self) -> SavedValue {
        SavedValue::List(vec![
            self.index.save(),
            self.pre_text.save(),
            self.post_text.save(),
            self.pre_selection.start.save(),
            self.pre_selection.end.save(),
            self.post_selection.start.save(),
            self.post_selection.end.save(),
            self.time_millis.save(),
        ])
    }

    fn restore(saved: &SavedValue) -> Option<TextUndoOperation> {
        let SavedValue::List(items) = saved else { return None };
        let [index, pre_text, post_text, pre_start, pre_end, post_start, post_end, time] =
            items.as_slice()
        else {
            return None;
        };
        let offset = |saved: &SavedValue| usize::restore(saved);
        Some(TextUndoOperation {
            index: offset(index)?,
            pre_text: String::restore(pre_text)?,
            post_text: String::restore(post_text)?,
            pre_selection: TextRange::new(offset(pre_start)?, offset(pre_end)?),
            post_selection: TextRange::new(offset(post_start)?, offset(post_end)?),
            time_millis: u64::restore(time)?,
            can_merge: true,
        })
    }
}

/// An undo and a redo stack, as Compose's generic `UndoManager`: state lists, so `can_undo` and
/// `can_redo` are observed; together at most `capacity` long, the oldest going first.
pub(crate) struct UndoManager<T: Clone + 'static> {
    undo_stack: OwnedStateList<T>,
    redo_stack: OwnedStateList<T>,
    capacity: usize,
}

impl<T: Clone + 'static> UndoManager<T> {
    /// Panics if the stacks are longer than `capacity`, as Compose's.
    pub(crate) fn new(undo_stack: Vec<T>, redo_stack: Vec<T>, capacity: usize) -> UndoManager<T> {
        assert!(
            undo_stack.len() + redo_stack.len() <= capacity,
            "Initial list of undo and redo operations have a size greater than the given capacity."
        );
        UndoManager {
            undo_stack: mutable_state_list_from(undo_stack),
            redo_stack: mutable_state_list_from(redo_stack),
            capacity,
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo_stack.handle().is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo_stack.handle().is_empty()
    }

    fn size(&self) -> usize {
        self.undo_stack.handle().len() + self.redo_stack.handle().len()
    }

    /// Adds `action`, forgetting what could be redone and, past the capacity, the oldest.
    pub(crate) fn record(&self, action: T) {
        let (undo, redo) = (self.undo_stack.handle(), self.redo_stack.handle());
        redo.clear();
        while self.size() > self.capacity.saturating_sub(1) && !undo.is_empty() {
            undo.remove(0);
        }
        if self.capacity > 0 {
            undo.push(action);
        }
    }

    /// The last action, moved to the redo stack. Panics if there's none, as Compose's.
    pub(crate) fn undo(&self) -> T {
        assert!(
            self.can_undo(),
            "It's an error to call undo while there is nothing to undo. Please first check \
             `canUndo` value before calling the `undo` function."
        );
        let undo = self.undo_stack.handle();
        let action = undo.remove(undo.len() - 1);
        self.redo_stack.handle().push(action.clone());
        action
    }

    /// The last action undone, moved back. Panics if there's none, as Compose's.
    pub(crate) fn redo(&self) -> T {
        assert!(
            self.can_redo(),
            "It's an error to call redo while there is nothing to redo. Please first check \
             `canRedo` value before calling the `redo` function."
        );
        let redo = self.redo_stack.handle();
        let action = redo.remove(redo.len() - 1);
        self.undo_stack.handle().push(action.clone());
        action
    }

    pub(crate) fn clear_history(&self) {
        self.undo_stack.handle().clear();
        self.redo_stack.handle().clear();
    }

    /// `[capacity, undo count, redo count, the undo stack…, the redo stack…]`, as Compose's
    /// `createSaver`.
    pub(crate) fn save(&self, save: impl Fn(&T) -> SavedValue) -> SavedValue {
        let (undo, redo) = (self.undo_stack.handle(), self.redo_stack.handle());
        let mut saved = vec![self.capacity.save(), undo.len().save(), redo.len().save()];
        saved.extend(undo.iter_cloned().map(|item| save(&item)));
        saved.extend(redo.iter_cloned().map(|item| save(&item)));
        SavedValue::List(saved)
    }

    pub(crate) fn restore(
        saved: &SavedValue,
        restore: impl Fn(&SavedValue) -> Option<T>,
    ) -> Option<UndoManager<T>> {
        let SavedValue::List(items) = saved else { return None };
        let [capacity, undo_count, redo_count, rest @ ..] = items.as_slice() else { return None };
        let (capacity, undo_count) = (usize::restore(capacity)?, usize::restore(undo_count)?);
        let redo_count = usize::restore(redo_count)?;
        if rest.len() != undo_count + redo_count || undo_count + redo_count > capacity {
            return None;
        }
        let items: Vec<T> = rest.iter().map(restore).collect::<Option<_>>()?;
        let (undo, redo) = items.split_at(undo_count);
        Some(UndoManager::new(undo.to_vec(), redo.to_vec(), capacity))
    }
}

/// A text field's history, as Compose's `TextUndoManager`: an operation waits in a staging place,
/// where the next merges into it, until one doesn't.
pub(crate) struct TextUndoManager {
    staging: OwnedState<Option<TextUndoOperation>>,
    manager: UndoManager<TextUndoOperation>,
}

impl Default for TextUndoManager {
    fn default() -> TextUndoManager {
        TextUndoManager::new(None, UndoManager::new(Vec::new(), Vec::new(), TEXT_UNDO_CAPACITY))
    }
}

impl TextUndoManager {
    fn new(
        staging: Option<TextUndoOperation>,
        manager: UndoManager<TextUndoOperation>,
    ) -> TextUndoManager {
        TextUndoManager { staging: mutable_state_of(staging), manager }
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.manager.can_undo() || self.staging.handle().with(Option::is_some)
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.manager.can_redo() && self.staging.handle().with(Option::is_none)
    }

    /// The operation to undo, or `None` if there's none.
    pub(crate) fn take_undo(&self) -> Option<TextUndoOperation> {
        if !self.can_undo() {
            return None;
        }
        self.flush();
        Some(self.manager.undo())
    }

    /// The operation to redo, or `None` if there's none.
    pub(crate) fn take_redo(&self) -> Option<TextUndoOperation> {
        if !self.can_redo() {
            return None;
        }
        Some(self.manager.redo())
    }

    /// Records `operation`, merged into the staged one if they merge, as Compose's `record`.
    pub(crate) fn record(&self, operation: TextUndoOperation) {
        let staging = self.staging.handle();
        let staged = Snapshot::without_read_observation(|| staging.get());
        let Some(staged) = staged else {
            staging.set(Some(operation));
            return;
        };
        if let Some(merged) = staged.merge(&operation) {
            staging.set(Some(merged));
            return;
        }
        self.flush();
        staging.set(Some(operation));
    }

    pub(crate) fn clear_history(&self) {
        self.staging.handle().set(None);
        self.manager.clear_history();
    }

    fn flush(&self) {
        let staging = self.staging.handle();
        if let Some(staged) = Snapshot::without_read_observation(|| staging.get()) {
            self.manager.record(staged);
        }
        staging.set(None);
    }

    /// Records the change from `pre` to `post` that `changes` made, as Compose's `recordChanges`:
    /// one change in the text's bounds an operation of its own, several or one out of bounds the
    /// whole text replaced, never merging.
    pub(crate) fn record_changes(
        &self,
        pre: &TextFieldCharSequence,
        post: &TextFieldCharSequence,
        changes: &ChangeList,
        allow_merge: bool,
    ) {
        let whole = || {
            TextUndoOperation::new(
                0,
                pre.text.to_string(),
                post.text.to_string(),
                pre.selection,
                post.selection,
                false,
            )
        };
        match changes.change_count() {
            0 => {}
            1 => {
                let (original_pre, original_post) =
                    (changes.get_original_range(0), changes.get_range(0));
                let pre_range = original_pre.coerce_in(0, pre.text.len());
                let post_range = original_post.coerce_in(0, post.text.len());
                let in_bounds = original_pre == pre_range
                    && original_post == post_range
                    && pre_range.min() == post_range.min();
                if !in_bounds {
                    self.record(whole());
                } else if !pre_range.collapsed() || !post_range.collapsed() {
                    self.record(TextUndoOperation::new(
                        pre_range.min(),
                        pre_range.substring(&pre.text).to_string(),
                        post_range.substring(&post.text).to_string(),
                        pre.selection,
                        post.selection,
                        allow_merge,
                    ));
                }
            }
            _ => self.record(whole()),
        }
    }

    /// `[the staged operation or nothing, the stacks]`, as Compose's `TextUndoManager.Saver`.
    pub(crate) fn save(&self) -> SavedValue {
        let staged = Snapshot::without_read_observation(|| self.staging.handle().get());
        SavedValue::List(vec![
            SavedValue::List(staged.iter().map(TextUndoOperation::save).collect()),
            self.manager.save(TextUndoOperation::save),
        ])
    }

    pub(crate) fn restore(saved: &SavedValue) -> Option<TextUndoManager> {
        let SavedValue::List(items) = saved else { return None };
        let [SavedValue::List(staged), manager] = items.as_slice() else { return None };
        let staged = match staged.as_slice() {
            [] => None,
            [operation] => Some(TextUndoOperation::restore(operation)?),
            _ => return None,
        };
        let manager = UndoManager::restore(manager, TextUndoOperation::restore)?;
        Some(TextUndoManager::new(staged, manager))
    }
}

/// A text field state's undo and redo, as Compose's `UndoState`
/// ([`TextFieldState::undo_state`]): `can_undo` and `can_redo` are state, reading them in
/// composition recomposes when they change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UndoState {
    pub(crate) state: TextFieldState,
}

impl UndoState {
    /// Whether there's something to undo, as `canUndo`.
    pub fn can_undo(&self) -> bool {
        self.state.with_undo(TextUndoManager::can_undo).unwrap_or(false)
    }

    /// Whether there's something undone to redo, as `canRedo`.
    pub fn can_redo(&self) -> bool {
        self.state.with_undo(TextUndoManager::can_redo).unwrap_or(false)
    }

    /// Reverts the last edit, or group of edits merged together, as `undo`; nothing if there's
    /// none.
    pub fn undo(&self) {
        if let Some(operation) = self.state.with_undo(TextUndoManager::take_undo).flatten() {
            self.state.edit_with_no_side_effects(|buffer| {
                let end = operation.index + operation.post_text.len();
                buffer.replace(operation.index, end, &operation.pre_text);
                buffer.set_selection_coerced(operation.pre_selection);
            });
        }
    }

    /// Applies again what was last undone, as `redo`; nothing if there's none.
    pub fn redo(&self) {
        if let Some(operation) = self.state.with_undo(TextUndoManager::take_redo).flatten() {
            self.state.edit_with_no_side_effects(|buffer| {
                let end = operation.index + operation.pre_text.len();
                buffer.replace(operation.index, end, &operation.post_text);
                buffer.set_selection_coerced(operation.post_selection);
            });
        }
    }

    /// Forgets everything to undo or redo, as `clearHistory`.
    pub fn clear_history(&self) {
        self.state.with_undo(TextUndoManager::clear_history);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;

    use compose_runtime::snapshots::SnapshotStateObserver;

    use super::*;
    use crate::TextFieldBuffer;

    const MERGE: TextFieldEditUndoBehavior = TextFieldEditUndoBehavior::MergeIfPossible;

    fn manager(capacity: usize) -> UndoManager<i32> {
        UndoManager::new(Vec::new(), Vec::new(), capacity)
    }

    fn panics(block: impl FnOnce()) -> bool {
        catch_unwind(AssertUnwindSafe(block)).is_err()
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, initialLowCapacityThrows_undoStack
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, initialLowCapacityThrows_redoStack
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, initialLowCapacityThrows_bothStacks
    #[test]
    fn stacks_longer_than_the_capacity_panic() {
        assert!(panics(|| {
            UndoManager::new(vec![1, 2], Vec::new(), 1);
        }));
        assert!(panics(|| {
            UndoManager::new(Vec::new(), vec![1, 2], 1);
        }));
        assert!(panics(|| {
            UndoManager::new(vec![1, 2], vec![1, 2], 3);
        }));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, commitSingleItem_canUndo
    #[test]
    fn what_is_recorded_can_be_undone_then_redone() {
        let undo = manager(100);
        undo.record(1);
        assert!(undo.can_undo() && !undo.can_redo());
        undo.undo();
        assert!(!undo.can_undo() && undo.can_redo());
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, cannotRedoWithoutFirstUndo
    #[test]
    fn nothing_undone_has_nothing_to_redo() {
        let undo = manager(100);
        for item in 1..=3 {
            undo.record(item);
        }
        assert!(!undo.can_redo());
        assert!(panics(|| {
            undo.redo();
        }));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, commitItem_clearsRedoStack
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, clearHistoryRemovesUndoAndRedo
    #[test]
    fn a_record_forgets_the_redo_and_clearing_forgets_all() {
        let undo = manager(100);
        for item in 1..=3 {
            undo.record(item);
        }
        undo.undo();
        assert!(undo.can_redo());
        undo.record(4);
        assert!(!undo.can_redo());
        undo.undo();
        assert!(undo.can_undo() && undo.can_redo());
        undo.clear_history();
        assert!(!undo.can_undo() && !undo.can_redo());
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, capacityOverflow_removesFromTheBottomOfStack
    #[test]
    fn past_the_capacity_the_oldest_goes() {
        let undo = manager(2);
        for item in 1..=3 {
            undo.record(item);
        }
        assert_eq!((undo.undo(), undo.undo()), (3, 2));
        assert!(!undo.can_undo());
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerTest.kt, capacityOverflow_shouldRemoveRedoActionsFirst
    #[test]
    fn past_the_capacity_the_redo_goes_first() {
        let undo = manager(20);
        for item in 1..=4 {
            undo.record(item);
        }
        undo.undo();
        undo.undo();
        undo.record(5);
        assert!(undo.can_undo() && !undo.can_redo());
        assert_eq!((undo.undo(), undo.undo(), undo.undo()), (5, 2, 1));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/UndoManagerSaverTest.kt, saveAndRestore
    #[test]
    fn the_stacks_are_saved_and_restored() {
        let undo = manager(100);
        for item in 1..=3 {
            undo.record(item);
        }
        undo.undo();
        let saved = undo.save(Saveable::save);
        let restored = UndoManager::<i32>::restore(&saved, i32::restore).expect("restored");
        assert!(restored.can_undo() && restored.can_redo());
        assert_eq!(restored.redo(), 3);
        let mut undone = Vec::new();
        while restored.can_undo() {
            undone.push(restored.undo());
        }
        assert_eq!(undone, [3, 2, 1]);
    }

    /// Text edits a user makes, as Compose's test's helpers over `editAsUser`.
    trait Typing {
        fn place_cursor_at(&self, index: usize);
        fn select(&self, start: usize, end: usize);
        fn type_at(&self, index: usize, text: &str);
        fn type_at_end(&self, text: &str);
        fn type_at_start(&self, text: &str);
        fn delete_at(&self, index: usize);
        fn replace_at(&self, start: usize, end: usize, text: &str);
        fn type_over_selection(&self, text: &str);
        fn held(&self) -> (String, TextRange);
    }

    impl Typing for TextFieldState {
        fn place_cursor_at(&self, index: usize) {
            self.select(index, index);
        }
        fn select(&self, start: usize, end: usize) {
            self.edit_as_user(true, MERGE, |buffer| {
                buffer.set_selection_coerced(TextRange::new(start, end))
            });
        }
        fn type_at(&self, index: usize, text: &str) {
            self.place_cursor_at(index);
            self.edit_as_user(true, MERGE, |buffer| buffer.replace(index, index, text));
        }
        fn type_at_end(&self, text: &str) {
            self.type_at(self.text().len(), text);
        }
        fn type_at_start(&self, text: &str) {
            self.type_at(0, text);
        }
        fn delete_at(&self, index: usize) {
            self.edit_as_user(true, MERGE, |buffer| buffer.delete(index, index + 1));
        }
        fn replace_at(&self, start: usize, end: usize, text: &str) {
            self.edit_as_user(true, MERGE, |buffer| buffer.replace(start, end, text));
        }
        /// As an input method's commit over the selection, the cursor after it.
        fn type_over_selection(&self, text: &str) {
            self.edit_as_user(false, MERGE, |buffer| {
                let selection = buffer.selection();
                buffer.replace(selection.min(), selection.max(), text);
                buffer.place_cursor_before_char_at(selection.min() + text.len());
            });
        }
        fn held(&self) -> (String, TextRange) {
            (self.text().to_string(), self.selection())
        }
    }

    fn texts_undone(state: &TextFieldState, count: usize) -> Vec<String> {
        (0..count)
            .map(|_| {
                state.undo_state().undo();
                state.text().to_string()
            })
            .collect()
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, insertionFromEndPointCanMerge
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, insertionFromStartPointCannotMerge
    #[test]
    fn insertions_merge_only_forwards() {
        let state = TextFieldState::new("");
        for text in ["a", "b", "c"] {
            state.type_at_end(text);
        }
        assert_eq!(state.text().as_str(), "abc");
        assert!(state.undo_state().can_undo());
        assert_eq!(texts_undone(&state, 1), [""]);
        let backwards = TextFieldState::new("");
        for text in ["a", "b", "c"] {
            backwards.type_at_start(text);
        }
        assert_eq!(backwards.text().as_str(), "cba");
        assert_eq!(texts_undone(&backwards, 2), ["ba", "a"]);
    }

    /// Nothing can be redone while an operation is staged: it would be lost under the redo.
    #[test]
    fn nothing_is_redone_while_an_edit_is_staged() {
        let state = TextFieldState::new("");
        state.type_at_end("a");
        state.undo_state().undo();
        assert!(state.undo_state().can_redo());
        state.type_at_end("b");
        assert!(!state.undo_state().can_redo(), "an edit is staged");
        state.undo_state().redo();
        assert_eq!(state.text().as_str(), "b");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, insertionFromMiddleCannotMerge
    #[test]
    fn an_insertion_in_the_middle_is_a_step_of_its_own() {
        let state = TextFieldState::new("");
        state.type_at_end("a");
        state.type_at_end("c");
        state.type_at(1, "b");
        assert_eq!(state.text().as_str(), "abc");
        state.undo_state().undo();
        assert_eq!(state.held(), ("ac".to_string(), TextRange::at(1)));
        assert_eq!(texts_undone(&state, 1), [""]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, deletionFromEndPointCanMerge
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, deletionFromStartPointCanMerge
    #[test]
    fn deletions_one_way_merge() {
        let backspaced = TextFieldState::new("abc");
        backspaced.place_cursor_at(3);
        for index in [2, 1, 0] {
            backspaced.delete_at(index);
        }
        assert_eq!(backspaced.text().as_str(), "");
        assert_eq!(texts_undone(&backspaced, 1), ["abc"]);
        let deleted = TextFieldState::new("abc");
        deleted.place_cursor_at(0);
        for _ in 0..3 {
            deleted.delete_at(0);
        }
        assert_eq!(deleted.text().as_str(), "");
        assert_eq!(texts_undone(&deleted, 1), ["abc"]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, deletionFromMiddleCannotMerge
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, deletionsWithDifferentDirectionsCannotMerge
    #[test]
    fn deletions_changing_way_or_place_are_steps_of_their_own() {
        let state = TextFieldState::new("abc");
        state.place_cursor_at(2);
        for index in [1, 1, 0] {
            state.delete_at(index);
        }
        assert_eq!(state.text().as_str(), "");
        assert_eq!(texts_undone(&state, 3), ["a", "ac", "abc"]);
        let both_ways = TextFieldState::new("abcdef");
        both_ways.place_cursor_at(3);
        both_ways.delete_at(2);
        both_ways.delete_at(2);
        assert_eq!(both_ways.text().as_str(), "abef");
        assert_eq!(texts_undone(&both_ways, 2), ["abdef", "abcdef"]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, insertionAndDeletionNeverMerge
    #[test]
    fn insertions_and_deletions_never_merge() {
        let state = TextFieldState::new("");
        state.type_at_end("a");
        state.type_at_end("b");
        state.delete_at(1);
        state.type_at_start("c");
        assert_eq!(state.held(), ("ca".to_string(), TextRange::at(1)));
        state.undo_state().undo();
        assert_eq!(state.held(), ("a".to_string(), TextRange::at(0)));
        state.undo_state().undo();
        assert_eq!(state.held(), ("ab".to_string(), TextRange::at(2)));
        assert_eq!(texts_undone(&state, 1), [""]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, replaceDoesNotMergeWithInsertion
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, replaceDoesNotMergeWithDeletion
    #[test]
    fn a_replacement_never_merges() {
        let inserted = TextFieldState::new("abc");
        inserted.type_at_end("d");
        inserted.replace_at(0, 4, "def");
        inserted.type_at_end("g");
        assert_eq!(inserted.held(), ("defg".to_string(), TextRange::at(4)));
        inserted.undo_state().undo();
        assert_eq!(inserted.held(), ("def".to_string(), TextRange::at(3)));
        inserted.undo_state().undo();
        assert_eq!(inserted.held(), ("abcd".to_string(), TextRange::at(4)));
        let deleted = TextFieldState::new("abc");
        deleted.place_cursor_at(3);
        deleted.delete_at(2);
        deleted.replace_at(0, 2, "def");
        deleted.delete_at(2);
        assert_eq!(deleted.held(), ("de".to_string(), TextRange::at(2)));
        deleted.undo_state().undo();
        assert_eq!(deleted.held(), ("def".to_string(), TextRange::at(3)));
        deleted.undo_state().undo();
        assert_eq!(deleted.held(), ("ab".to_string(), TextRange::at(2)));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, newLineInsert_doesNotMerge
    #[test]
    fn a_new_line_is_a_step_of_its_own() {
        let state = TextFieldState::new("");
        for text in ["a", "b", "\n", "c"] {
            state.type_at_end(text);
        }
        assert_eq!(state.held(), ("ab\nc".to_string(), TextRange::at(4)));
        state.undo_state().undo();
        assert_eq!(state.held(), ("ab\n".to_string(), TextRange::at(3)));
        state.undo_state().undo();
        assert_eq!(state.held(), ("ab".to_string(), TextRange::at(2)));
        assert_eq!(texts_undone(&state, 1), [""]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, undoRecoversSelectionState
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, redoDoesNotRecoverSelectionState
    #[test]
    fn undo_brings_the_selection_back_and_redo_the_one_after() {
        let state = TextFieldState::new("abc");
        state.select(2, 0);
        state.type_over_selection("d");
        assert_eq!(state.held(), ("dc".to_string(), TextRange::at(1)));
        state.undo_state().undo();
        assert_eq!(state.held(), ("abc".to_string(), TextRange::new(2, 0)));
        let redone = TextFieldState::new("abc");
        redone.type_at_end("d");
        redone.select(2, 0);
        redone.type_over_selection("e");
        assert_eq!(redone.held(), ("ecd".to_string(), TextRange::at(1)));
        redone.undo_state().undo();
        redone.undo_state().undo();
        redone.undo_state().redo();
        assert_eq!(redone.held(), ("abcd".to_string(), TextRange::at(4)));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, directEdits_doeNotClearTheUndoHistory
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, directEdit_onlySelectionChange_doesNotClearUndoHistory
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, directEdit_replaceButNoContentChange_doesNotClearUndoHistory
    #[test]
    fn programmatic_edits_keep_the_history() {
        let edits: [fn(&mut TextFieldBuffer); 3] = [
            |buffer| buffer.replace(0, 1, "x"),
            |buffer| buffer.set_selection(TextRange::ZERO),
            |buffer| buffer.replace(0, 6, "eabcdf"),
        ];
        for edit in edits {
            let state = TextFieldState::new("abc");
            state.type_at_end("d");
            state.type_at_start("e");
            state.type_at_end("f");
            state.edit(edit);
            assert!(state.undo_state().can_undo());
            assert!(!state.undo_state().can_redo());
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, commitEdit_withMismatchedOriginalRange_undoRestoresOriginalText
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/undo/TextUndoTest.kt, commitEdit_withMismatchedOriginalRange_nonZeroOffset_undoRestoresOriginalText
    #[test]
    fn a_change_out_of_the_texts_bounds_records_the_whole_text() {
        for (initial, other, (start, end, with), expected) in [
            ("1234", "123456789012345", (0, 15, "abcd"), "abcd"),
            ("12", "1234567890", (5, 10, "xyz"), "12345xyz"),
        ] {
            let state = TextFieldState::new(initial);
            assert!(!state.undo_state().can_undo() && !state.undo_state().can_redo());
            let other = TextFieldState::new(other);
            let mut buffer = other.to_text_field_buffer();
            buffer.replace(start, end, with);
            state.commit_edit(&buffer);
            assert_eq!(state.text().as_str(), expected);
            assert!(state.undo_state().can_undo() && !state.undo_state().can_redo());
            state.undo_state().undo();
            assert_eq!(state.text().as_str(), initial);
            assert!(!state.undo_state().can_undo() && state.undo_state().can_redo());
            state.undo_state().redo();
            assert_eq!(state.text().as_str(), expected);
            assert!(state.undo_state().can_undo() && !state.undo_state().can_redo());
        }
    }

    /// Edits 5 s apart don't merge: the group's first edit is what counts, as Compose's.
    #[test]
    fn edits_five_seconds_after_their_groups_first_are_a_new_step() {
        let first = TextUndoOperation {
            index: 0,
            pre_text: String::new(),
            post_text: "a".to_string(),
            pre_selection: TextRange::at(0),
            post_selection: TextRange::at(1),
            time_millis: 1_000,
            can_merge: true,
        };
        let soon = TextUndoOperation {
            index: 1,
            post_text: "b".to_string(),
            pre_selection: TextRange::at(1),
            post_selection: TextRange::at(2),
            time_millis: 5_999,
            ..first.clone()
        };
        let merged = first.merge(&soon).expect("within 5 s");
        assert_eq!((merged.post_text.as_str(), merged.time_millis), ("ab", 1_000));
        let late = TextUndoOperation { index: 2, time_millis: 6_000, ..soon.clone() };
        assert_eq!(merged.merge(&late), None, "5 s after the group's first");
        let earlier = TextUndoOperation { time_millis: 999, ..soon };
        assert_eq!(first.merge(&earlier), None, "before it");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateTest.kt, notifyImeListener_firesAfterUndoRedo
    #[test]
    fn an_undo_tells_the_input_method_without_restarting_it() {
        let state = TextFieldState::new("Hello");
        state.edit_as_user(true, MERGE, |buffer| buffer.append(" World"));
        let heard = Rc::new(std::cell::RefCell::new(Vec::new()));
        let log = heard.clone();
        state.add_notify_ime_listener(move |old, new, restart| {
            log.borrow_mut().push((old.text.to_string(), new.text.to_string(), restart));
        });
        state.undo_state().undo();
        assert_eq!(*heard.borrow(), [("Hello World".to_string(), "Hello".to_string(), false)]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/TextFieldStateSaverTest.kt, savesAndRestoresUndo
    #[test]
    fn the_saver_keeps_the_history() {
        let state = TextFieldState::with_selection("hello, world", TextRange::new(0, 5));
        state.type_over_selection("hi");
        let saver = TextFieldState::saver();
        let saved = compose_runtime::Saver::save(&saver, &state).expect("saved");
        let restored = compose_runtime::Saver::restore(&saver, &saved).expect("restored");
        assert_eq!(restored.text().as_str(), "hi, world");
        assert!(restored.undo_state().can_undo());
        restored.undo_state().undo();
        assert_eq!(restored.held(), ("hello, world".to_string(), TextRange::new(0, 5)));
    }

    // Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/undo/BasicTextFieldUndoTest.kt, undoDoesNotReadSnapshotState_whileInitializing
    #[test]
    fn making_a_state_and_recording_edits_reads_nothing_observed() {
        let count = Rc::new(Cell::new(0));
        let observer = SnapshotStateObserver::new(|run| run());
        let counted = count.clone();
        let on_changed: Rc<dyn Fn(&u32)> = Rc::new(move |_| counted.set(counted.get() + 1));
        observer.start();
        let state = observer.observe_reads(0u32, &on_changed, || TextFieldState::new("Hello"));
        Snapshot::with_mutable_snapshot(|| {
            state.edit_as_user(true, MERGE, |buffer| buffer.append(" World!"));
            state.edit_as_user(true, MERGE, |buffer| buffer.replace(0, buffer.length(), "abc"));
        });
        Snapshot::send_apply_notifications();
        observer.stop();
        assert_eq!(count.get(), 0);
    }
}
