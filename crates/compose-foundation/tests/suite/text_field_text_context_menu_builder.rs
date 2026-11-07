//! A state-based field's right-click menu's data, by the clipboard, the selection, a password and
//! read-only, as a provided dropdown gets it (D2040), from the `btf2` half of Compose's
//! `TextFieldTextContextMenuBuilderTest`.
//!
//! The rule's platform has autofill, as Android 8 and up (D1682): the `beforeApi26` tests say it
//! hasn't.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::MemoryClipboard;
use compose_ui::{ClipboardManager, IntSize, TextRange};

use crate::text_context_menu_support::FakeTextContextMenuProvider;

const TEXT: &str = "Text Text Text";
const TAG: &str = "BTF";

#[derive(Clone, Copy)]
enum SelectionAmount {
    None,
    Partial,
    All,
}

use TextContextMenuKeys::{Autofill, Copy, Cut, Paste, SelectAll};

thread_local! {
    /// Whether the platform's API level is 26 or more, where autofill is (D1682).
    static API_26: Cell<bool> = const { Cell::new(true) };
}

/// `run_correct_items_test` before API 26: no autofill, so no autofill item.
fn run_correct_items_test_before_api26(
    password: bool,
    read_only: bool,
    empty_clipboard: bool,
    amount: SelectionAmount,
    expected: &[TextContextMenuKeys],
) {
    API_26.with(|it| it.set(false));
    // Nor a platform autofill manager, whatever `COMPOSE_TEST_AUTOFILL` says (D1688).
    compose_test::use_autofill(false);
    run_correct_items_test(password, read_only, empty_clipboard, amount, expected);
    API_26.with(|it| it.set(true));
}

/// Compose's `runBtf2CorrectItemsTest`: the data the provided dropdown gets from a right click on
/// "Text Text Text", selected as said, the clipboard empty or holding text, a secure field for a
/// password; its keys are `expected`, in order.
fn run_correct_items_test(
    password: bool,
    read_only: bool,
    empty_clipboard: bool,
    amount: SelectionAmount,
    expected: &[TextContextMenuKeys],
) {
    assert!(!(password && read_only), "Can't be a password field and read-only.");
    let selection = match amount {
        SelectionAmount::None => TextRange::new(0, 0),
        SelectionAmount::Partial => TextRange::new(5, 9),
        SelectionAmount::All => TextRange::new(0, 14),
    };
    let state: TextFieldState =
        **Box::leak(Box::new(TextFieldState::with_selection(TEXT, selection)));
    let data: Rc<RefCell<Option<TextContextMenuData>>> = Rc::default();
    let into = data.clone();
    let fake: Rc<dyn TextContextMenuProvider> =
        Rc::new(FakeTextContextMenuProvider::new(move |provider| {
            *into.borrow_mut() = Some(provider.data());
            Box::pin(async {})
        }));
    let clipboard = Rc::new(MemoryClipboard::default());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), || {});
    let shared: Rc<dyn ClipboardManager> = clipboard.clone();
    ui.host().with_tree(|tree| tree.set_clipboard(shared));
    // Before API 26, Android has no autofill (Compose's `beforeApi26` tests run there only).
    ui.host().with_tree(|tree| tree.set_autofill_available(API_26.with(Cell::get)));
    ui.set_content(move || {
        compose_runtime::CompositionLocalProvider(
            LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.provides(Some(fake.clone())),
            || {
                if password {
                    BasicSecureTextField!(state, modifier = Modifier.test_tag(TAG));
                } else {
                    BasicTextFieldOfState!(
                        state,
                        modifier = Modifier.test_tag(TAG),
                        read_only = read_only
                    );
                }
            },
        );
    });
    // Set after the content, as Compose's clip entry's job.
    if !empty_clipboard {
        clipboard.set_text("Clipboard Text");
    }
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
    let data = data.borrow().clone().expect("the dropdown wasn't asked to show");
    let keys: Vec<TextContextMenuKey> = data.components.iter().map(|it| it.key()).collect();
    let expected: Vec<TextContextMenuKey> = expected.iter().map(|it| it.key()).collect();
    assert_eq!(keys, expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_beforeApi26
#[test]
fn btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_beforeApi26() {
    run_correct_items_test_before_api26(false, false, true, SelectionAmount::None, &[SelectAll]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_afterApi26
#[test]
fn btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_afterApi26() {
    run_correct_items_test(false, false, true, SelectionAmount::None, &[SelectAll, Autofill]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_emptyClipboard_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_emptyClipboard_partialSelection_itemsMatch() {
    run_correct_items_test(false, false, true, SelectionAmount::Partial, &[Cut, Copy, SelectAll]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_emptyClipboard_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_emptyClipboard_fullSelection_itemsMatch() {
    run_correct_items_test(false, false, true, SelectionAmount::All, &[Cut, Copy]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_beforeApi26
#[test]
fn btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_beforeApi26() {
    let expected = [Paste, SelectAll];
    run_correct_items_test_before_api26(false, false, false, SelectionAmount::None, &expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_afterApi26
#[test]
fn btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_afterApi26() {
    let expected = [Paste, SelectAll, Autofill];
    run_correct_items_test(false, false, false, SelectionAmount::None, &expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_nonEmptyClipboard_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_nonEmptyClipboard_partialSelection_itemsMatch() {
    let expected = [Cut, Copy, Paste, SelectAll];
    run_correct_items_test(false, false, false, SelectionAmount::Partial, &expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_nonEmptyClipboard_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_nonEmptyClipboard_fullSelection_itemsMatch() {
    run_correct_items_test(false, false, false, SelectionAmount::All, &[Cut, Copy, Paste]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_password_noSelection_itemsMatch_beforeApi26
#[test]
fn btf2_contextMenu_password_noSelection_itemsMatch_beforeApi26() {
    let expected = [Paste, SelectAll];
    run_correct_items_test_before_api26(true, false, false, SelectionAmount::None, &expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_password_noSelection_itemsMatch_afterApi26
#[test]
fn btf2_contextMenu_password_noSelection_itemsMatch_afterApi26() {
    let expected = [Paste, SelectAll, Autofill];
    run_correct_items_test(true, false, false, SelectionAmount::None, &expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_password_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_password_partialSelection_itemsMatch() {
    run_correct_items_test(true, false, false, SelectionAmount::Partial, &[Paste, SelectAll]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_password_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_password_fullSelection_itemsMatch() {
    run_correct_items_test(true, false, false, SelectionAmount::All, &[Paste]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_readOnly_noSelection_itemsMatch
#[test]
fn btf2_contextMenu_readOnly_noSelection_itemsMatch() {
    run_correct_items_test(false, true, true, SelectionAmount::None, &[SelectAll]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_readOnly_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_readOnly_partialSelection_itemsMatch() {
    run_correct_items_test(false, true, true, SelectionAmount::Partial, &[Copy, SelectAll]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldTextContextMenuBuilderTest.kt, btf2_contextMenu_readOnly_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_readOnly_fullSelection_itemsMatch() {
    run_correct_items_test(false, true, true, SelectionAmount::All, &[Copy]);
}
