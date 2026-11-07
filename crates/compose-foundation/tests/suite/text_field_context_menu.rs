//! A state-based field's right-click menu: shown by a right click (not a left one, not while
//! disabled), closed by a click off it or an item, its items by the clipboard, the selection, a
//! password and read-only (D2040), from the `btf2` half of Compose's `TextFieldContextMenuTest`.
//!
//! The rule's root is 400 by 400 at density 1. The rule's platform has autofill, as Android 8
//! and up (D1682): the `beforeApi26` tests say it hasn't.
//! Compose's two process-text tests are `@Ignore`d upstream (b/472497302), left out.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::MemoryClipboard;
use compose_ui::{ClipboardManager, IntSize, Offset, PointerEvent, PointerEventPass, TextRange};

const TAG: &str = "BTF";
const CUT: &str = "Cut";
const COPY: &str = "Copy";
const PASTE: &str = "Paste";
const SELECT_ALL: &str = "Select all";
const AUTOFILL: &str = "Autofill";

/// A state kept for the test's length (its owner leaked).
fn state(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
}

fn default_full_width_text() -> String {
    "M".repeat(20)
}

/// A rule over `content` whose tree's clipboard is `clipboard`, as Compose's `LocalClipboard
/// provides clipboard` around `setTextFieldTestContent`.
fn rule_with(clipboard: Rc<MemoryClipboard>, content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), || {});
    let shared: Rc<dyn ClipboardManager> = clipboard;
    ui.host().with_tree(|tree| tree.set_clipboard(shared));
    ui.set_content(content);
    ui
}

fn field_rule(state: TextFieldState, enabled: bool) -> ComposeTestRule {
    rule_with(Rc::new(MemoryClipboard::default()), move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG), enabled = enabled);
    })
}

fn right_click_field(ui: &mut ComposeTestRule) {
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
}

/// Compose's `contextMenuItemInteraction`: the item labeled `label` in a popup.
fn item<'a>(ui: &'a mut ComposeTestRule, label: &str) -> SemanticsNodeInteraction<'a> {
    ui.on_node(has_any_ancestor(is_popup()).and(has_text(label)))
}

/// Compose's `clickOffPopup`, above its test framework (`UiDevice`): the host takes the click as
/// the platform's.
fn click_off_popup(ui: &mut ComposeTestRule, at: Offset) {
    ui.host().send_pointer(PointerEvent::press(at));
    ui.host().send_pointer(PointerEvent::release(at));
    ui.wait_for_idle();
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ItemState {
    Enabled,
    DoesNotExist,
}

/// Compose's `assertContextMenuItem`.
fn assert_item(ui: &mut ComposeTestRule, label: &str, state: ItemState) {
    match state {
        ItemState::DoesNotExist => {
            item(ui, label).assert_does_not_exist();
        }
        ItemState::Enabled => {
            item(ui, label)
                .assert_exists()
                .assert_is_displayed()
                .assert_has_click_action()
                .assert_is_enabled();
        }
    }
}

/// Compose's `assertContextMenuItems`: the popup exists, each item as said.
fn assert_items(
    ui: &mut ComposeTestRule,
    [cut, copy, paste, select_all, autofill]: [ItemState; 5],
) {
    ui.on_node(is_popup()).assert_exists();
    assert_item(ui, CUT, cut);
    assert_item(ui, COPY, copy);
    assert_item(ui, PASTE, paste);
    assert_item(ui, SELECT_ALL, select_all);
    assert_item(ui, AUTOFILL, autofill);
}

use ItemState::{DoesNotExist as NO, Enabled as YES};

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_rightClick_appears
#[test]
fn btf2_contextMenu_rightClick_appears() {
    let state = state(&default_full_width_text(), TextRange::new(20, 20));
    let mut ui = field_rule(state, true);
    ui.on_node(is_popup()).assert_does_not_exist();
    right_click_field(&mut ui);
    ui.on_node(is_popup()).assert_exists();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_leftClick_doesNotAppear
#[test]
fn btf2_contextMenu_leftClick_doesNotAppear() {
    let state = state(&default_full_width_text(), TextRange::new(20, 20));
    let mut ui = field_rule(state, true);
    ui.on_node(is_popup()).assert_does_not_exist();
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.click());
    ui.on_node(is_popup()).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_disabled_rightClick_doesNotAppear
#[test]
fn btf2_contextMenu_disabled_rightClick_doesNotAppear() {
    let state = state(&default_full_width_text(), TextRange::new(20, 20));
    let mut ui = field_rule(state, false);
    ui.on_node(is_popup()).assert_does_not_exist();
    right_click_field(&mut ui);
    ui.on_node(is_popup()).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_disappearsOnClickOffOfPopup
#[test]
fn btf2_contextMenu_disappearsOnClickOffOfPopup() {
    let state = state(&default_full_width_text(), TextRange::new(20, 20));
    let mut ui = field_rule(state, true);
    ui.on_node(is_popup()).assert_does_not_exist();
    right_click_field(&mut ui);
    ui.on_node(is_popup()).assert_exists();
    // Halfway from the root's top left to its center.
    click_off_popup(&mut ui, Offset::new(100.0, 100.0));
    ui.on_node(is_popup()).assert_does_not_exist();
}

/// Compose's `runBtf2ClickContextMenuItemTest`: "Text Text Text" with its middle word selected,
/// "clip" on the clipboard, `label` clicked from the right click's menu, which closes.
fn run_click_item_test(
    label: &str,
    expected_text: &str,
    expected_selection: TextRange,
    expected_clipboard: &str,
) {
    let state = state("Text Text Text", TextRange::new(5, 9));
    let clipboard = Rc::new(MemoryClipboard::default());
    clipboard.set_text("clip");
    let mut ui = rule_with(clipboard.clone(), move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG));
    });
    right_click_field(&mut ui);
    item(&mut ui, label).assert_has_click_action().assert_is_enabled().perform_click();
    ui.wait_for_idle();
    ui.on_node(is_popup()).assert_does_not_exist();
    item(&mut ui, label).assert_does_not_exist();
    assert_eq!(state.text().as_str(), expected_text);
    assert_eq!(state.selection(), expected_selection);
    assert_eq!(clipboard.get_text().as_deref(), Some(expected_clipboard));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_onClickCut
#[test]
fn btf2_contextMenu_onClickCut() {
    run_click_item_test(CUT, "Text  Text", TextRange::new(5, 5), "Text");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_onClickCopy
#[test]
fn btf2_contextMenu_onClickCopy() {
    run_click_item_test(COPY, "Text Text Text", TextRange::new(5, 9), "Text");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_onClickPaste
#[test]
fn btf2_contextMenu_onClickPaste() {
    run_click_item_test(PASTE, "Text clip Text", TextRange::new(9, 9), "clip");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_onClickSelectAll
#[test]
fn btf2_contextMenu_onClickSelectAll() {
    run_click_item_test(SELECT_ALL, "Text Text Text", TextRange::new(0, 14), "clip");
}

#[derive(Clone, Copy)]
enum SelectionAmount {
    None,
    Partial,
    All,
}

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
) -> ComposeTestRule {
    API_26.with(|it| it.set(false));
    // Nor a platform autofill manager, whatever `COMPOSE_TEST_AUTOFILL` says (D1688).
    compose_test::use_autofill(false);
    let ui = run_correct_items_test(password, read_only, empty_clipboard, amount);
    API_26.with(|it| it.set(true));
    ui
}

/// Compose's `runBtf2CorrectItemsTest`: "Text Text Text", selected as said, the clipboard empty
/// or holding text, a secure field for a password, then a right click.
fn run_correct_items_test(
    password: bool,
    read_only: bool,
    empty_clipboard: bool,
    amount: SelectionAmount,
) -> ComposeTestRule {
    assert!(!(password && read_only), "Can't be a password field and read-only.");
    let selection = match amount {
        SelectionAmount::None => TextRange::new(0, 0),
        SelectionAmount::Partial => TextRange::new(5, 9),
        SelectionAmount::All => TextRange::new(0, 14),
    };
    let state = state("Text Text Text", selection);
    let clipboard = Rc::new(MemoryClipboard::default());
    if !empty_clipboard {
        clipboard.set_text("Clipboard Text");
    }
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), || {});
    let shared: Rc<dyn ClipboardManager> = clipboard;
    ui.host().with_tree(|tree| tree.set_clipboard(shared));
    // Before API 26, Android has no autofill (Compose's `beforeApi26` tests run there only).
    ui.host().with_tree(|tree| tree.set_autofill_available(API_26.with(Cell::get)));
    ui.set_content(move || {
        if password {
            BasicSecureTextField!(state, modifier = Modifier.test_tag(TAG));
        } else {
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG), read_only = read_only);
        }
    });
    right_click_field(&mut ui);
    ui
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_beforeApi26
#[test]
fn btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_beforeApi26() {
    let mut ui = run_correct_items_test_before_api26(false, false, true, SelectionAmount::None);
    assert_items(&mut ui, [NO, NO, NO, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_afterApi26
#[test]
fn btf2_contextMenu_emptyClipboard_noSelection_itemsMatch_afterApi26() {
    let mut ui = run_correct_items_test(false, false, true, SelectionAmount::None);
    assert_items(&mut ui, [NO, NO, NO, YES, YES]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_emptyClipboard_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_emptyClipboard_partialSelection_itemsMatch() {
    let mut ui = run_correct_items_test(false, false, true, SelectionAmount::Partial);
    assert_items(&mut ui, [YES, YES, NO, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_emptyClipboard_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_emptyClipboard_fullSelection_itemsMatch() {
    let mut ui = run_correct_items_test(false, false, true, SelectionAmount::All);
    assert_items(&mut ui, [YES, YES, NO, NO, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_beforeApi26
#[test]
fn btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_beforeApi26() {
    let mut ui = run_correct_items_test_before_api26(false, false, false, SelectionAmount::None);
    assert_items(&mut ui, [NO, NO, YES, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_afterApi26
#[test]
fn btf2_contextMenu_nonEmptyClipboard_noSelection_itemsMatch_afterApi26() {
    let mut ui = run_correct_items_test(false, false, false, SelectionAmount::None);
    assert_items(&mut ui, [NO, NO, YES, YES, YES]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_nonEmptyClipboard_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_nonEmptyClipboard_partialSelection_itemsMatch() {
    let mut ui = run_correct_items_test(false, false, false, SelectionAmount::Partial);
    assert_items(&mut ui, [YES, YES, YES, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_nonEmptyClipboard_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_nonEmptyClipboard_fullSelection_itemsMatch() {
    let mut ui = run_correct_items_test(false, false, false, SelectionAmount::All);
    assert_items(&mut ui, [YES, YES, YES, NO, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_password_noSelection_itemsMatch_beforeApi26
#[test]
fn btf2_contextMenu_password_noSelection_itemsMatch_beforeApi26() {
    let mut ui = run_correct_items_test_before_api26(true, false, false, SelectionAmount::None);
    assert_items(&mut ui, [NO, NO, YES, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_password_noSelection_itemsMatch_afterApi26
#[test]
fn btf2_contextMenu_password_noSelection_itemsMatch_afterApi26() {
    let mut ui = run_correct_items_test(true, false, false, SelectionAmount::None);
    assert_items(&mut ui, [NO, NO, YES, YES, YES]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_password_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_password_partialSelection_itemsMatch() {
    let mut ui = run_correct_items_test(true, false, false, SelectionAmount::Partial);
    assert_items(&mut ui, [NO, NO, YES, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_password_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_password_fullSelection_itemsMatch() {
    let mut ui = run_correct_items_test(true, false, false, SelectionAmount::All);
    assert_items(&mut ui, [NO, NO, YES, NO, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_readOnly_noSelection_itemsMatch
#[test]
fn btf2_contextMenu_readOnly_noSelection_itemsMatch() {
    let mut ui = run_correct_items_test(false, true, true, SelectionAmount::None);
    assert_items(&mut ui, [NO, NO, NO, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_readOnly_partialSelection_itemsMatch
#[test]
fn btf2_contextMenu_readOnly_partialSelection_itemsMatch() {
    let mut ui = run_correct_items_test(false, true, true, SelectionAmount::Partial);
    assert_items(&mut ui, [NO, YES, NO, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/textfield/TextFieldContextMenuTest.kt, btf2_contextMenu_readOnly_fullSelection_itemsMatch
#[test]
fn btf2_contextMenu_readOnly_fullSelection_itemsMatch() {
    let mut ui = run_correct_items_test(false, true, true, SelectionAmount::All);
    assert_items(&mut ui, [NO, YES, NO, NO, NO]);
}

/// compose-rs's: the open menu's items follow the field, as Compose's builder reads its state
/// while the menu shows: the text cleared under it, cut and copy go, select all too (nothing to
/// select); text and a selection set back, they come back.
#[test]
fn openMenu_itemsFollowTheField() {
    let state = state("Text Text Text", TextRange::new(5, 9));
    let mut ui = rule_with(Rc::new(MemoryClipboard::default()), move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG));
    });
    right_click_field(&mut ui);
    assert_items(&mut ui, [YES, YES, NO, YES, NO]);
    state.clear_text();
    ui.wait_for_idle();
    // Autofill shows on an editable field with no selection (D1684).
    assert_items(&mut ui, [NO, NO, NO, NO, YES]);
    state.set_text_and_select_all("Text");
    ui.wait_for_idle();
    assert_items(&mut ui, [YES, YES, NO, NO, NO]);
    state.edit(|buffer| buffer.set_selection(TextRange::new(1, 1)));
    ui.wait_for_idle();
    assert_items(&mut ui, [NO, NO, NO, YES, YES]);
}

/// The field, its middle word selected, inside a box telling whether each press and release
/// reached it consumed (after the field, in the main pass).
fn observed_field(consumed: Rc<RefCell<Vec<bool>>>) -> (ComposeTestRule, TextFieldState) {
    let state = state("Text Text Text", TextRange::new(5, 9));
    let ui = rule_with(Rc::new(MemoryClipboard::default()), move || {
        let consumed = consumed.clone();
        Box!(
            modifier = Modifier.pointer_input((), move |scope| {
                let consumed = consumed.clone();
                async move {
                    loop {
                        let event = scope.await_pointer_event(PointerEventPass::Main).await;
                        let pressed_or_lifted = |it: &&compose_ui::PointerInputChange| {
                            it.changed_to_down_ignore_consumed()
                                || it.changed_to_up_ignore_consumed()
                        };
                        if let Some(change) = event.changes.iter().find(pressed_or_lifted) {
                            consumed.borrow_mut().push(change.is_consumed());
                        }
                    }
                }
            }),
            {
                BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG));
            }
        );
    });
    (ui, state)
}

/// compose-rs's: the right click is the menu's, as Compose's `onRightClickDown` consumes its
/// press and release before the selection's gestures, which take the primary button alone: the
/// selection stays, the field isn't focused, and what's around sees both consumed.
#[test]
fn rightClick_isTheMenus_selectionAndFocusStay() {
    let consumed: Rc<RefCell<Vec<bool>>> = Rc::default();
    let (mut ui, state) = observed_field(consumed.clone());
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.right_click(Offset::new(1.0, 5.0)));
    ui.on_node(is_popup()).assert_exists();
    assert_eq!(state.selection(), TextRange::new(5, 9));
    ui.on_node_with_tag(TAG).assert_is_not_focused();
    assert_eq!(*consumed.borrow(), vec![true, true]);
}

/// compose-rs's: the right button pressed during a primary press isn't a right click (D2040's
/// chorded buttons, Compose's `changedToDown`): no menu; the primary press placed the cursor and
/// focused the field.
#[test]
fn secondaryPressedWhilePrimaryHeld_opensNoMenu() {
    let (mut ui, state) = observed_field(Rc::default());
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        mouse.move_to(Offset::new(1.0, 5.0));
        mouse.press(MouseButton::Primary);
        mouse.press(MouseButton::Secondary);
        mouse.release(MouseButton::Secondary);
        mouse.release(MouseButton::Primary);
    });
    ui.on_node(is_popup()).assert_does_not_exist();
    assert_eq!(state.selection(), TextRange::new(0, 0));
    ui.on_node_with_tag(TAG).assert_is_focused();
}

/// compose-rs's, not Compose's (D2044): the primary button pressed while a right click is held is
/// the first down of what hears the primary button alone, so the field's decoration takes its
/// release as a tap and focuses the field; Compose's tap detector waits for a pointer's down
/// (`changedToDown`), and the pointer came down with the right click, consumed by the menu. The
/// field's own gestures hear every button: the press goes by them, the cursor and the menu stay.
#[test]
fn rightClick_thenPrimaryWhileHeld_focusesOnTheRelease() {
    let (mut ui, state) = observed_field(Rc::default());
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        mouse.move_to(Offset::new(1.0, 5.0));
        mouse.press(MouseButton::Secondary);
        mouse.press(MouseButton::Primary);
    });
    ui.on_node(is_popup()).assert_exists();
    ui.on_node_with_tag(TAG).assert_is_not_focused();
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        mouse.release(MouseButton::Primary);
        mouse.release(MouseButton::Secondary);
    });
    ui.on_node(is_popup()).assert_exists();
    assert_eq!(state.selection(), TextRange::new(5, 9));
    ui.on_node_with_tag(TAG).assert_is_focused();
}

/// compose-rs's: a middle click does nothing to the field, its selection's gestures taking the
/// primary button alone (Compose's `isPrimaryPressed`); a left click there moves the cursor.
#[test]
fn middleClick_leavesTheSelection() {
    let (mut ui, state) = observed_field(Rc::default());
    ui.on_node_with_tag(TAG)
        .perform_mouse_input(|mouse| mouse.click_at(Offset::new(1.0, 5.0), MouseButton::Tertiary));
    ui.on_node(is_popup()).assert_does_not_exist();
    assert_eq!(state.selection(), TextRange::new(5, 9));
    ui.on_node_with_tag(TAG)
        .perform_mouse_input(|mouse| mouse.click_at(Offset::new(1.0, 5.0), MouseButton::Primary));
    assert_eq!(state.selection(), TextRange::new(0, 0));
}
