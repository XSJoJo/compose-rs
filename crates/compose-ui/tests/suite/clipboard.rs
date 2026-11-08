//! Compose's `ClipboardTest` (ui, host tests): a `ClipboardManager` that doesn't override
//! `has_text` answers from its text.

use std::cell::RefCell;

use compose_ui::ClipboardManager;

/// A clipboard overriding only the text's reading and writing, as the Kotlin test's
/// `FakeClipboardManager`.
struct FakeClipboardManager {
    text: RefCell<Option<String>>,
}

impl FakeClipboardManager {
    fn new(initial_text: Option<&str>) -> FakeClipboardManager {
        FakeClipboardManager { text: RefCell::new(initial_text.map(str::to_string)) }
    }
}

impl ClipboardManager for FakeClipboardManager {
    fn get_text(&self) -> Option<String> {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = Some(text.to_string());
    }
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/platform/ClipboardTest.kt, clipboardManagerWithoutHasText_returnsTrue_withNonEmptyGetText
#[test]
fn clipboard_manager_without_has_text_returns_true_with_non_empty_get_text() {
    assert!(FakeClipboardManager::new(Some("Something")).has_text());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/platform/ClipboardTest.kt, clipboardManagerWithoutHasText_returnsFalse_withNullGetText
#[test]
fn clipboard_manager_without_has_text_returns_false_with_null_get_text() {
    assert!(!FakeClipboardManager::new(None).has_text());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/platform/ClipboardTest.kt, clipboardManagerWithoutHasText_returnsFalse_withEmptyGetText
#[test]
fn clipboard_manager_without_has_text_returns_false_with_empty_get_text() {
    let clipboard = FakeClipboardManager::new(Some(""));
    assert!(!clipboard.has_text());
    // Text set later counts.
    clipboard.set_text("later");
    assert!(clipboard.has_text());
}
