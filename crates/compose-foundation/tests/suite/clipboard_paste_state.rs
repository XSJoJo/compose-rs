//! Whether the clipboard holds something to paste, known at each update without reading the clip
//! (D2035), from Compose's `ClipboardPasteStateTest`.
//!
//! Compose's `FakeClipboard` counts `getClipEntry` calls and its native manager is a mock whose
//! `hasPrimaryClip` and `primaryClip` are verified; the fake here counts `get_text` (reading the
//! clip) and `has_clip`.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};

use compose_foundation::ClipboardPasteState;
use compose_ui::ClipboardManager;

/// A clipboard of text that answers `has_text` and `has_clip` without reading its text, as
/// Android's, counting what's asked.
#[derive(Default)]
struct FakeClipboard {
    text: RefCell<Option<String>>,
    get_text_calls: Cell<u32>,
    has_clip_calls: Cell<u32>,
}

impl FakeClipboard {
    fn new(text: &str) -> FakeClipboard {
        let clipboard = FakeClipboard::default();
        *clipboard.text.borrow_mut() = Some(text.to_owned());
        clipboard
    }
}

impl ClipboardManager for FakeClipboard {
    fn get_text(&self) -> Option<String> {
        self.get_text_calls.set(self.get_text_calls.get() + 1);
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = Some(text.to_owned());
    }

    fn has_text(&self) -> bool {
        self.text.borrow().as_ref().is_some_and(|text| !text.is_empty())
    }

    fn has_clip(&self) -> bool {
        self.has_clip_calls.set(self.has_clip_calls.get() + 1);
        self.text.borrow().is_some()
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/ClipboardPasteStateTest.kt, clipboardPasteState_updates
#[test]
fn clipboardPasteState_updates() {
    let clipboard = FakeClipboard::default();
    let mut state = ClipboardPasteState::default();

    state.update(&clipboard);
    assert!(!state.has_text());
    assert!(!state.has_clip());

    clipboard.set_text("hello");
    state.update(&clipboard);

    assert!(state.has_text());
    assert!(state.has_clip());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/ClipboardPasteStateTest.kt, clipboardPasteState_initiallyFalse
#[test]
fn clipboardPasteState_initiallyFalse() {
    let _clipboard = FakeClipboard::new("hello, world");
    let state = ClipboardPasteState::default();

    // Not updated.
    assert!(!state.has_text());
    assert!(!state.has_clip());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/ClipboardPasteStateTest.kt, clipboardPasteState_update_doesNotReadPrimaryClip
#[test]
fn clipboardPasteState_update_doesNotReadPrimaryClip() {
    let clipboard = FakeClipboard::new("hello, world");
    let mut state = ClipboardPasteState::default();

    state.update(&clipboard);

    assert_eq!(clipboard.get_text_calls.get(), 0);
    assert!(clipboard.has_clip_calls.get() >= 1);
}
