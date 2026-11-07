//! A `SelectionContainer`'s text context menu items, copy and select all, collected from a text
//! inside it as any menu collects them, by touch and by mouse (D2041), from Compose's
//! `SelectionContainerContextMenuBuilderTest`.
//!
//! Compose's `SelectionState` waits on `selection-state`: the selection is read from the tree's
//! anchors (one text: their offsets are Compose's `toTextRange()`). The clipboard is the tree's,
//! holding the initial text as Compose's `FakeClipboard`.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::MemoryClipboard;
use compose_ui::{ClipboardManager, IntSize, Offset};

use crate::text_context_menu_traversal::DataInvoker;

const TEXT_TAG: &str = "text";
const DEFAULT_TEXT: &str = "Text Text Text";
const INITIAL_CLIPBOARD_TEXT: &str = "initialClipboardText";

/// A session counting its closes, as the test's `fakeSession`.
#[derive(Default)]
struct FakeSession(Cell<bool>);

impl TextContextMenuSession for FakeSession {
    fn close(&self) {
        self.0.set(true);
    }
}

/// The selection's offsets in the one text, as Compose's `state.selection?.toTextRange()`.
fn selection(ui: &mut ComposeTestRule) -> Option<(usize, usize)> {
    ui.host().with_tree(|tree| tree.selection_anchors()).map(|((_, start), (_, end))| (start, end))
}

fn rule_with(clipboard: Rc<MemoryClipboard>, content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), || {});
    let shared: Rc<dyn ClipboardManager> = clipboard;
    ui.host().with_tree(|tree| tree.set_clipboard(shared));
    ui.set_content(content);
    ui
}

/// Compose's `runClickContextMenuItemTest`: the middle word selected by a mouse's double click or
/// a finger's long press, the item of `key` collected at the text and clicked with a session.
fn run_click_context_menu_item_test(
    is_mouse: bool,
    expected_session_closed: bool,
    item_to_invoke: TextContextMenuKeys,
    expected_selection: Option<(usize, usize)>,
    expected_clipboard_content: &str,
) {
    let clipboard = Rc::new(MemoryClipboard::default());
    clipboard.set_text(INITIAL_CLIPBOARD_TEXT);
    let reader = DataInvoker::default();
    let modifier = Modifier.test_tag(TEXT_TAG).then(reader.reader());
    let mut ui = rule_with(clipboard.clone(), move || {
        let modifier = modifier.clone();
        SelectionContainer!({
            BasicText!(DEFAULT_TEXT, modifier = modifier.clone());
        });
    });

    // start selection of middle word
    if is_mouse {
        ui.on_node_with_tag(TEXT_TAG).perform_mouse_input(|mouse| {
            let center = mouse.center();
            mouse.double_click(center);
        });
    } else {
        ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| touch.long_click());
    }
    ui.wait_for_idle();

    // collect our context menu items.
    let data = reader.invoke_traversal();

    // get and verify component
    let key = item_to_invoke.key();
    let found: Vec<_> = data.components.iter().filter(|it| it.key() == key).collect();
    assert_eq!(found.len(), 1, "Component with key {key:?} not found");
    let TextContextMenuComponent::Item(item) = found[0] else {
        panic!("{key:?} isn't an item");
    };

    // simulate clicking the item.
    let session = FakeSession::default();
    (item.on_click)(&session);
    ui.wait_for_idle();

    // verify whether close called
    assert_eq!(session.0.get(), expected_session_closed);

    // verify selection updated
    assert_eq!(selection(&mut ui), expected_selection);

    // verify clipboard contents
    assert_eq!(clipboard.get_text().as_deref(), Some(expected_clipboard_content));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuBuilderTest.kt, whenTouch_onClick_copy
#[test]
fn whenTouch_onClick_copy() {
    run_click_context_menu_item_test(false, true, TextContextMenuKeys::Copy, None, "Text");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuBuilderTest.kt, whenTouch_onClick_selectAll
#[test]
fn whenTouch_onClick_selectAll() {
    run_click_context_menu_item_test(
        false,
        false,
        TextContextMenuKeys::SelectAll,
        Some((0, 14)),
        INITIAL_CLIPBOARD_TEXT,
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuBuilderTest.kt, whenMouse_onClick_copy
#[test]
fn whenMouse_onClick_copy() {
    run_click_context_menu_item_test(true, true, TextContextMenuKeys::Copy, Some((5, 9)), "Text");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuBuilderTest.kt, whenMouse_onClick_selectAll
#[test]
fn whenMouse_onClick_selectAll() {
    run_click_context_menu_item_test(
        true,
        true,
        TextContextMenuKeys::SelectAll,
        Some((0, 14)),
        INITIAL_CLIPBOARD_TEXT,
    );
}

/// Compose's `runItemMatchTest`: after `actions`, the keys collected in a box around the text.
fn run_item_match_test(
    actions: Option<fn(&mut ComposeTestRule)>,
    expected_items: &[TextContextMenuKeys],
) {
    let reader = DataInvoker::default();
    let modifier = reader.reader();
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let modifier = modifier.clone();
        SelectionContainer!({
            let modifier = modifier.clone();
            Box!(modifier = modifier, {
                BasicText!(DEFAULT_TEXT, modifier = Modifier.test_tag(TEXT_TAG));
            });
        });
    });

    if let Some(actions) = actions {
        actions(&mut ui);
        ui.wait_for_idle();
    }

    let actual = reader.invoke_traversal();
    let keys: Vec<TextContextMenuKey> = actual.components.iter().map(|it| it.key()).collect();
    let expected: Vec<TextContextMenuKey> = expected_items.iter().map(|it| it.key()).collect();
    assert_eq!(keys, expected);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuBuilderTest.kt, whenNoSelection_itemsMatch
#[test]
fn whenNoSelection_itemsMatch() {
    run_item_match_test(None, &[TextContextMenuKeys::SelectAll]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuBuilderTest.kt, whenPartialSelection_itemsMatch
#[test]
fn whenPartialSelection_itemsMatch() {
    run_item_match_test(
        Some(|ui| {
            ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| {
                let center = touch.center();
                let duration = touch.view_configuration().long_press_timeout_millis + 100;
                touch.long_click_at(center, duration);
            });
        }),
        &[TextContextMenuKeys::Copy, TextContextMenuKeys::SelectAll],
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerContextMenuBuilderTest.kt, whenFullSelection_itemsMatch
#[test]
fn whenFullSelection_itemsMatch() {
    run_item_match_test(
        Some(|ui| {
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
        }),
        &[TextContextMenuKeys::Copy],
    );
}
