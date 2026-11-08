//! Compose's `TextActionModeCallbackTest` (ui, host tests): the menu `AndroidTextToolbar`'s
//! floating `ActionMode` callback fills, each standard item with Android's id, Compose's order
//! and its title.
//!
//! Compose's callback is internal; compose-rs's is `PlatformTextToolbar`'s, reached here as a
//! platform reaches it: the toolbar shows its menu, the floating toolbars it starts (a recording
//! fake) hand over the callback, and the test fills menus through it.

use std::cell::RefCell;
use std::rc::Rc;

use compose_ui::{
    PlatformTextToolbar, Rect, TextActionMenu, TextActionMode, TextActionModeCallback,
    TextActionModes, TextToolbar, TextToolbarAction, text_action_ids, text_action_labels,
};

/// Floating toolbars keeping the callback of the one started.
#[derive(Default)]
struct RecordingModes {
    callback: RefCell<Option<Rc<dyn TextActionModeCallback>>>,
}

struct Mode;

impl TextActionMode for Mode {
    fn invalidate(&self) {}
    fn invalidate_content_rect(&self) {}
    fn finish(&self) {}
}

impl TextActionModes for RecordingModes {
    fn start(&self, callback: Rc<dyn TextActionModeCallback>) -> Option<Rc<dyn TextActionMode>> {
        *self.callback.borrow_mut() = Some(callback);
        Some(Rc::new(Mode))
    }
}

/// The callbacks given, as the Kotlin constructor's named arguments.
#[derive(Default)]
struct Requested {
    copy: bool,
    paste: bool,
    cut: bool,
    select_all: bool,
    autofill: bool,
}

const ALL: Requested =
    Requested { copy: true, paste: true, cut: true, select_all: true, autofill: true };
const COPY_ONLY: Requested =
    Requested { copy: true, paste: false, cut: false, select_all: false, autofill: false };
const NONE: Requested =
    Requested { copy: false, paste: false, cut: false, select_all: false, autofill: false };

fn action(given: bool) -> Option<TextToolbarAction> {
    given.then(|| Rc::new(|| {}) as TextToolbarAction)
}

/// A toolbar shown with `requested`, and its callback.
fn callback(requested: &Requested) -> (PlatformTextToolbar, Rc<dyn TextActionModeCallback>) {
    let modes = Rc::new(RecordingModes::default());
    let toolbar = PlatformTextToolbar::new(Some(modes.clone()));
    show(&toolbar, requested);
    let callback = modes.callback.borrow().clone().expect("the toolbar started a floating one");
    (toolbar, callback)
}

fn show(toolbar: &PlatformTextToolbar, requested: &Requested) {
    toolbar.show_menu(
        Rect::ZERO,
        action(requested.copy),
        action(requested.paste),
        action(requested.cut),
        action(requested.select_all),
        action(requested.autofill),
    );
}

/// An item's id, order and title.
fn shape(menu: &TextActionMenu) -> Vec<(i32, i32, String)> {
    menu.items().iter().map(|item| (item.id, item.order, item.title.clone())).collect()
}

/// A menu holding a copy item, as the Kotlin tests' `FakeMenuItem(MenuItemOption.Copy)`.
fn menu_with_copy() -> TextActionMenu {
    let mut menu = TextActionMenu::new();
    menu.add(0, text_action_ids::COPY, 0, text_action_labels::COPY, Rc::new(|| {}));
    menu
}

// `onCreateActionMode_beforeApi26` is n/a: compose-rs's Android support starts at API 29,
// where Compose always adds the autofill item.

/// From API 26, the menu's last item is autofill.
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/text/TextActionModeCallbackTest.kt, onCreateActionMode_afterApi26
#[test]
fn on_create_action_mode_after_api26() {
    let (_toolbar, callback) = callback(&ALL);
    let mut menu = TextActionMenu::new();
    assert!(callback.on_create_action_mode(&mut menu));
    let last = menu.items().last().unwrap();
    assert_eq!(last.id, text_action_ids::AUTOFILL);
    assert_eq!(last.order, 4);
    assert_eq!(last.title, text_action_labels::AUTOFILL);
    // And every item in Compose's order.
    assert_eq!(
        shape(&menu),
        [
            (text_action_ids::COPY, 0, "Copy".into()),
            (text_action_ids::PASTE, 1, "Paste".into()),
            (text_action_ids::CUT, 2, "Cut".into()),
            (text_action_ids::SELECT_ALL, 3, "Select all".into()),
            (text_action_ids::AUTOFILL, 4, "Autofill".into()),
        ]
    );
}

/// Copy's item: its id, order and title.
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/text/TextActionModeCallbackTest.kt, addMenuItem_correctValues
#[test]
fn add_menu_item_correct_values() {
    let (_toolbar, callback) = callback(&COPY_ONLY);
    let mut menu = TextActionMenu::new();
    callback.on_create_action_mode(&mut menu);
    assert_eq!(shape(&menu)[0], (text_action_ids::COPY, 0, text_action_labels::COPY.into()));
}

/// Prepared again with no copy callback, the menu loses its copy item.
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/text/TextActionModeCallbackTest.kt, updateMenuItems_hasItem_shouldDeleteItem
#[test]
fn update_menu_items_has_item_should_delete_item() {
    let (toolbar, callback) = callback(&COPY_ONLY);
    let mut menu = menu_with_copy();
    show(&toolbar, &NONE);
    callback.on_prepare_action_mode(&mut menu);
    assert!(menu.is_empty());
}

/// Prepared with a copy callback, an empty menu gets one copy item.
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/text/TextActionModeCallbackTest.kt, updateMenuItems_shouldAddItem
#[test]
fn update_menu_items_should_add_item() {
    let (_toolbar, callback) = callback(&COPY_ONLY);
    let mut menu = TextActionMenu::new();
    callback.on_prepare_action_mode(&mut menu);
    let copies = menu.items().iter().filter(|item| item.id == text_action_ids::COPY).count();
    assert_eq!(copies, 1);
}

/// Prepared with a copy callback, a menu already holding the copy item still holds it alone.
/// Compose's menu keeps the item object; compose-rs's callback fills the menu again, its copy
/// item the same (group, id, order, title).
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/text/TextActionModeCallbackTest.kt, updateMenuItems_hasItem_shouldAddItem_doesNothing
#[test]
fn update_menu_items_has_item_should_add_item_does_nothing() {
    let (_toolbar, callback) = callback(&COPY_ONLY);
    let mut menu = menu_with_copy();
    assert_eq!(menu.len(), 1);
    let previous = menu.items()[0].clone();
    callback.on_prepare_action_mode(&mut menu);
    assert_eq!(menu.len(), 1);
    let current = &menu.items()[0];
    assert_eq!(
        (current.group, current.id, current.order, &current.title),
        (previous.group, previous.id, previous.order, &previous.title)
    );
}
