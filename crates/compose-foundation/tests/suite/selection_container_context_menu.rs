//! A `SelectionContainer`'s right-click menu: shown by a mouse's or a trackpad's right click (not
//! a left one), closed by a click off it or an item, its items copy and select all by the
//! selection (D2041), from Compose's `SelectionContainerContextMenuTest`.
//!
//! The rule's root is 400 by 400 at density 1. Compose's `SelectionState` waits on
//! `selection-state`: the selection is read from the tree's anchors (one text: their offsets are
//! Compose's `toTextRange()`). The three tests `@Ignore`d upstream (b/534893012: the trackpad's
//! click off the popup and the two process-text ones) are left out.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::MemoryClipboard;
use compose_ui::{ClipboardManager, IntSize, Offset};

const TEXT_TAG: &str = "text";
const DEFAULT_TEXT: &str = "Text Text Text";
const INITIAL_CLIPBOARD_TEXT: &str = "clip";
const CUT: &str = "Cut";
const COPY: &str = "Copy";
const PASTE: &str = "Paste";
const SELECT_ALL: &str = "Select all";
const AUTOFILL: &str = "Autofill";

fn rule_with(clipboard: Rc<MemoryClipboard>, content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), || {});
    let shared: Rc<dyn ClipboardManager> = clipboard;
    ui.host().with_tree(|tree| tree.set_clipboard(shared));
    ui.set_content(content);
    ui
}

/// `SelectionContainer { BasicText(defaultText, Modifier.testTag(textTag)) }`.
fn container_rule() -> ComposeTestRule {
    rule_with(Rc::new(MemoryClipboard::default()), || {
        SelectionContainer!({
            BasicText!(DEFAULT_TEXT, modifier = Modifier.test_tag(TEXT_TAG));
        });
    })
}

/// The selection's offsets in the one text, as Compose's `state.selection?.toTextRange()`.
fn selection(ui: &mut ComposeTestRule) -> Option<(usize, usize)> {
    ui.host().with_tree(|tree| tree.selection_anchors()).map(|((_, start), (_, end))| (start, end))
}

fn mouse_right_click(ui: &mut ComposeTestRule) {
    ui.on_node_with_tag(TEXT_TAG).perform_mouse_input(|mouse| {
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
    ui.host().send_pointer(compose_ui::PointerEvent::press(at));
    ui.host().send_pointer(compose_ui::PointerEvent::release(at));
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

// region SelectionContainer Context Menu Gesture Tests

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_mouseRightClick_appears
#[test]
fn contextMenu_mouseRightClick_appears() {
    let mut ui = container_rule();
    ui.on_node(is_popup()).assert_does_not_exist();
    mouse_right_click(&mut ui);
    ui.on_node(is_popup()).assert_exists();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_trackpadRightClick_appears
#[test]
fn contextMenu_trackpadRightClick_appears() {
    let mut ui = container_rule();
    ui.on_node(is_popup()).assert_does_not_exist();
    ui.on_node_with_tag(TEXT_TAG).perform_trackpad_input(|trackpad| {
        let center = trackpad.center();
        trackpad.right_click(center);
    });
    ui.on_node(is_popup()).assert_exists();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_mouseLeftClick_doesNotAppear
#[test]
fn contextMenu_mouseLeftClick_doesNotAppear() {
    let mut ui = container_rule();
    ui.on_node(is_popup()).assert_does_not_exist();
    ui.on_node_with_tag(TEXT_TAG).perform_mouse_input(|mouse| mouse.click());
    ui.on_node(is_popup()).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_trackpadLeftClick_doesNotAppear
#[test]
fn contextMenu_trackpadLeftClick_doesNotAppear() {
    let mut ui = container_rule();
    ui.on_node(is_popup()).assert_does_not_exist();
    ui.on_node_with_tag(TEXT_TAG).perform_trackpad_input(|trackpad| trackpad.click());
    ui.on_node(is_popup()).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_disappearsOnMouseClickOffOfPopup
#[test]
fn contextMenu_disappearsOnMouseClickOffOfPopup() {
    let mut ui = container_rule();
    ui.on_node(is_popup()).assert_does_not_exist();
    mouse_right_click(&mut ui);
    ui.on_node(is_popup()).assert_exists();
    // Halfway from the root's top left to its center.
    click_off_popup(&mut ui, Offset::new(100.0, 100.0));
    ui.on_node(is_popup()).assert_does_not_exist();
}

// endregion SelectionContainer Context Menu Gesture Tests

// region Context Menu Item Click Tests

/// Compose's `runClickContextMenuItemTest`: "Text Text Text" with its middle word selected by a
/// finger's long press, "clip" on the clipboard, `label` clicked from the right click's menu,
/// which closes.
fn run_click_context_menu_item_test(
    label: &str,
    expected_selection: (usize, usize),
    expected_clipboard_content: Option<&str>,
) {
    let clipboard = Rc::new(MemoryClipboard::default());
    clipboard.set_text(INITIAL_CLIPBOARD_TEXT);
    let mut ui = rule_with(clipboard.clone(), || {
        SelectionContainer!({
            BasicText!(DEFAULT_TEXT, modifier = Modifier.test_tag(TEXT_TAG));
        });
    });

    // start selection of middle word
    ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| {
        let center = touch.center();
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(center, duration);
    });

    // open context menu
    mouse_right_click(&mut ui);

    item(&mut ui, label).assert_has_click_action().assert_is_enabled().perform_click();
    ui.wait_for_idle();

    // Context menu disappears
    ui.on_node(is_popup()).assert_does_not_exist();
    item(&mut ui, label).assert_does_not_exist();

    // Operation was applied
    assert_eq!(selection(&mut ui), Some(expected_selection));
    let expected = expected_clipboard_content.unwrap_or(INITIAL_CLIPBOARD_TEXT);
    assert_eq!(clipboard.get_text().as_deref(), Some(expected));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_onClickCopy
#[test]
fn contextMenu_onClickCopy() {
    run_click_context_menu_item_test(COPY, (5, 9), Some("Text"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_onClickSelectAll
#[test]
fn contextMenu_onClickSelectAll() {
    run_click_context_menu_item_test(SELECT_ALL, (0, 14), None);
}

// endregion Context Menu Item Click Tests

// region Context Menu Correct Item Tests

#[derive(Clone, Copy)]
enum SelectionAmount {
    None,
    Partial,
    All,
}

/// Compose's `runCorrectItemsTest`: "Text Text Text" selected as said by a finger, then a right
/// click; the selection then.
fn run_correct_items_test(amount: SelectionAmount) -> (ComposeTestRule, Option<(usize, usize)>) {
    let clipboard = Rc::new(MemoryClipboard::default());
    clipboard.set_text("Clipboard Text");
    let mut ui = rule_with(clipboard, || {
        SelectionContainer!({
            BasicText!("Text Text Text", modifier = Modifier.test_tag(TEXT_TAG));
        });
    });

    // set selection
    match amount {
        SelectionAmount::None => {} // already no selection
        SelectionAmount::Partial => {
            // select middle word
            ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| {
                let center = touch.center();
                let duration = touch.view_configuration().long_press_timeout_millis + 100;
                touch.long_click_at(center, duration);
            });
            ui.wait_for_idle();
        }
        SelectionAmount::All => {
            // select everything
            ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| {
                let x_shift = Offset::new(1.0, 0.0);
                let (left, right) = (touch.center_left(), touch.center_right());
                // Compose's `longPress`: held past the long-press timeout.
                let timeout = touch.view_configuration().long_press_timeout_millis;
                touch.down(left + x_shift);
                touch.move_pointers_after(timeout + 10);
                touch.move_to(right - x_shift);
                touch.up();
            });
            ui.wait_for_idle();
        }
    }

    // open context menu
    mouse_right_click(&mut ui);

    let selection = selection(&mut ui);
    (ui, selection)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_noSelection_itemsMatch
#[test]
fn contextMenu_noSelection_itemsMatch() {
    let (mut ui, selection) = run_correct_items_test(SelectionAmount::None);
    assert_eq!(selection, None);
    assert_items(&mut ui, [NO, NO, NO, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_partialSelection_itemsMatch
#[test]
fn contextMenu_partialSelection_itemsMatch() {
    let (mut ui, selection) = run_correct_items_test(SelectionAmount::Partial);
    assert_eq!(selection, Some((5, 9)));
    assert_items(&mut ui, [NO, YES, NO, YES, NO]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuTest.kt, contextMenu_fullSelection_itemsMatch
#[test]
fn contextMenu_fullSelection_itemsMatch() {
    let (mut ui, selection) = run_correct_items_test(SelectionAmount::All);
    assert_eq!(selection, Some((0, 14)));
    assert_items(&mut ui, [NO, YES, NO, NO, NO]);
}

// endregion Context Menu Correct Item Tests

// Not Compose's: a right click on a field inside a container opens the field's menu alone, as
// Compose's container's right click takes only a press nothing below took (`changedToDown`,
// D2042).
#[test]
fn a_right_click_on_a_field_in_a_container_opens_the_fields_menu_alone() {
    let state = **Box::leak(Box::new(compose_foundation::TextFieldState::with_selection(
        "Text",
        compose_ui::TextRange::new(0, 4),
    )));
    let mut ui = rule_with(Rc::new(MemoryClipboard::default()), move || {
        SelectionContainer!({
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
        });
    });
    ui.on_node_with_tag("field").perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
    assert_eq!(ui.on_all_nodes(is_popup()).fetch_semantics_nodes().len(), 1);
    item(&mut ui, CUT).assert_exists();
}

// Not Compose's: the right button pressed during a mouse's selecting drag isn't a right click
// (D2040's chorded buttons, Compose's `changedToDown`): no menu, the drag's selection stays.
#[test]
fn a_secondary_press_during_a_primary_drag_opens_no_menu() {
    let mut ui = container_rule();
    ui.on_node_with_tag(TEXT_TAG).perform_mouse_input(|mouse| {
        let start = mouse.center_left() + Offset::new(1.0, 0.0);
        mouse.move_to(start);
        mouse.press(MouseButton::Primary);
        mouse.move_to(mouse.center_right() - Offset::new(1.0, 0.0));
        mouse.press(MouseButton::Secondary);
        mouse.release(MouseButton::Secondary);
        mouse.release(MouseButton::Primary);
    });
    ui.on_node(is_popup()).assert_does_not_exist();
    assert_eq!(selection(&mut ui), Some((0, 14)));
}

// Not Compose's: the open menu's items follow the selection, as Compose's builder reads it while
// the menu shows. The selected text changes under it: the selection's let go of (Compose's
// `onSelectableChangeCallback`), copy goes; a text not selected stays to select all.
#[test]
fn the_open_menus_items_follow_the_selection() {
    let text =
        Box::leak(Box::new(compose_runtime::mutable_state_of(String::from(DEFAULT_TEXT)))).handle();
    let mut ui = rule_with(Rc::new(MemoryClipboard::default()), move || {
        SelectionContainer!({
            BasicText!(text.get(), modifier = Modifier.test_tag(TEXT_TAG));
        });
    });
    ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| {
        let center = touch.center();
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(center, duration);
    });
    ui.wait_for_idle();
    mouse_right_click(&mut ui);
    assert_items(&mut ui, [NO, YES, NO, YES, NO]);
    text.set(String::from("Other words"));
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), None);
    assert_items(&mut ui, [NO, NO, NO, YES, NO]);
}

// Not Compose's (its tests check the top alone): a long press's toolbar goes around the selected
// word, in the window, its bottom four handles' heights (100 px) lower, as Compose's
// `getContentRect` (D2041).
#[test]
fn the_toolbars_rect_is_the_selected_words_with_room_for_the_handles() {
    let mut ui = container_rule();
    ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| {
        let center = touch.center();
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(center, duration);
    });
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), Some((5, 9)));
    let text = ui.on_node_with_tag(TEXT_TAG).fetch_semantics_snapshot().bounds;
    let mut results = Vec::new();
    ui.on_node_with_tag(TEXT_TAG).perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    let layout = results.pop().expect("a text layout");
    let (first, last) = (layout.get_bounding_box(5), layout.get_bounding_box(8));
    let word = compose_ui::Rect::new(
        first.left.min(last.left),
        first.top.min(last.top),
        first.right.max(last.right),
        first.bottom.max(last.bottom),
    )
    .translate(Offset::new(text.left, text.top));
    let expected = compose_ui::Rect::new(word.left, word.top, word.right, word.bottom + 100.0);
    assert_eq!(ui.text_toolbar().content_rect(), Some(expected));
}
