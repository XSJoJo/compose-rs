//! The platform's text toolbar, as Compose's `TextToolbar` and `LocalTextToolbar`, and the
//! floating toolbar under it, shaped as Android's `ActionMode` of `TYPE_FLOATING`, which both
//! Compose's `AndroidTextToolbar` and its newer `AndroidTextContextMenuToolbarProvider` drive
//! (docs/parity/design/text-context-menu.md).
//!
//! The host sets its floating toolbars on the tree (`UiTree::set_text_action_modes`): Android's
//! view starts action modes through JNI, the test rule a fake. Desktop and headless trees have
//! none, and a toolbar shown there shows nothing.

use std::cell::RefCell;
use std::rc::Rc;

use compose_runtime::CompositionLocal;

use crate::unit::Rect;

/// Whether a text toolbar shows, as Compose's `TextToolbarStatus`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextToolbarStatus {
    Shown,
    Hidden,
}

/// An action of a toolbar's item.
pub type TextToolbarAction = Rc<dyn Fn()>;

/// A text toolbar, as Compose's `TextToolbar`: the floating toolbar of copy, paste, cut, select
/// all and autofill.
pub trait TextToolbar {
    /// Shows the toolbar around `rect` (in the root), with an item for each action given, as
    /// `showMenu(rect, onCopyRequested, onPasteRequested, onCutRequested, onSelectAllRequested,
    /// onAutofillRequested)`: a missing action has no item. Showing it again updates it.
    fn show_menu(
        &self,
        rect: Rect,
        on_copy: Option<TextToolbarAction>,
        on_paste: Option<TextToolbarAction>,
        on_cut: Option<TextToolbarAction>,
        on_select_all: Option<TextToolbarAction>,
        on_autofill: Option<TextToolbarAction>,
    );

    /// Hides it.
    fn hide(&self);

    fn status(&self) -> TextToolbarStatus;
}

/// The platform's floating toolbars, as Android's `View.startActionMode(callback,
/// ActionMode.TYPE_FLOATING)`.
pub trait TextActionModes {
    /// Starts a floating toolbar that asks `callback` for its items and where it goes: `None` if
    /// it didn't start (its menu was empty, or the platform refused).
    fn start(&self, callback: Rc<dyn TextActionModeCallback>) -> Option<Rc<dyn TextActionMode>>;
}

/// A floating toolbar shown, as Android's `ActionMode`.
pub trait TextActionMode {
    /// Asks its callback to prepare its menu again (`ActionMode.invalidate`).
    fn invalidate(&self);
    /// Asks its callback where it goes again (`ActionMode.invalidateContentRect`).
    fn invalidate_content_rect(&self);
    /// Closes it: its callback is told it's destroyed (`ActionMode.finish`).
    fn finish(&self);
}

/// What a floating toolbar asks of what shows it, as Android's `ActionMode.Callback2` (Compose's
/// internal `TextActionModeCallback`).
pub trait TextActionModeCallback {
    /// Where it goes around: a rect in the root (the view), as `onGetContentRect`.
    fn on_get_content_rect(&self) -> Rect;
    /// Fills its menu when it starts: whether it shows (`onCreateActionMode`).
    fn on_create_action_mode(&self, menu: &mut TextActionMenu) -> bool;
    /// Fills its menu again when invalidated: whether it changed (`onPrepareActionMode`).
    fn on_prepare_action_mode(&self, menu: &mut TextActionMenu) -> bool;
    /// It closed, by its owner or by the platform (a touch outside, back), as
    /// `onDestroyActionMode`.
    fn on_destroy_action_mode(&self);
}

/// A floating toolbar's menu, as Android's `Menu`: its items, in order.
#[derive(Default)]
pub struct TextActionMenu {
    items: Vec<TextActionMenuItem>,
}

/// An item of a floating toolbar's menu, as a `MenuItem` shown as an action: its group (items of
/// a group are side by side, Compose's separators start a new one), its id (Android's
/// `android.R.id.copy` and others for the standard items), its order, its title and what a click
/// does.
#[derive(Clone)]
pub struct TextActionMenuItem {
    pub group: i32,
    pub id: i32,
    pub order: i32,
    pub title: String,
    pub on_click: TextToolbarAction,
}

impl std::fmt::Debug for TextActionMenuItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TextActionMenuItem")
            .field("group", &self.group)
            .field("id", &self.id)
            .field("order", &self.order)
            .field("title", &self.title)
            .finish()
    }
}

impl TextActionMenu {
    pub fn new() -> TextActionMenu {
        TextActionMenu::default()
    }

    /// Removes its items (`Menu.clear`).
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Adds an item (`Menu.add(groupId, itemId, order, title)` with its click listener), kept in
    /// order by `order`, as Android's menu sorts them.
    pub fn add(
        &mut self,
        group: i32,
        id: i32,
        order: i32,
        title: &str,
        on_click: TextToolbarAction,
    ) {
        let at = self.items.partition_point(|item| item.order <= order);
        self.items
            .insert(at, TextActionMenuItem { group, id, order, title: title.into(), on_click });
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[TextActionMenuItem] {
        &self.items
    }
}

/// The ids of the standard items, Android's (`android.R.id.cut` and the others), which the
/// platform gives their icons and order.
pub mod text_action_ids {
    pub const CUT: i32 = 0x0102_0020;
    pub const COPY: i32 = 0x0102_0021;
    pub const PASTE: i32 = 0x0102_0022;
    pub const SELECT_ALL: i32 = 0x0102_001f;
    pub const AUTOFILL: i32 = 0x0102_002a;
}

/// The labels of the standard items, Compose's English strings (Android's `android.R.string`
/// values in English).
pub mod text_action_labels {
    pub const CUT: &str = "Cut";
    pub const COPY: &str = "Copy";
    pub const PASTE: &str = "Paste";
    pub const SELECT_ALL: &str = "Select all";
    pub const AUTOFILL: &str = "Autofill";
}

/// The tree's [`TextToolbar`]: Compose's `AndroidTextToolbar` over the platform's floating
/// toolbars, its items in Compose's order (copy, paste, cut, select all, autofill), each with
/// Android's id. Without floating toolbars it shows nothing and stays hidden.
pub struct PlatformTextToolbar {
    modes: Option<Rc<dyn TextActionModes>>,
    state: Rc<RefCell<PlatformTextToolbarState>>,
}

#[derive(Default)]
struct PlatformTextToolbarState {
    mode: Option<Rc<dyn TextActionMode>>,
    rect: Rect,
    items: Vec<(i32, &'static str, TextToolbarAction)>,
}

struct PlatformTextToolbarCallback(Rc<RefCell<PlatformTextToolbarState>>);

impl PlatformTextToolbarCallback {
    fn fill(&self, menu: &mut TextActionMenu) {
        menu.clear();
        let state = self.0.borrow();
        for (order, (id, title, action)) in state.items.iter().enumerate() {
            menu.add(0, *id, order as i32 + 1, title, action.clone());
        }
    }
}

impl TextActionModeCallback for PlatformTextToolbarCallback {
    fn on_get_content_rect(&self) -> Rect {
        self.0.borrow().rect
    }

    fn on_create_action_mode(&self, menu: &mut TextActionMenu) -> bool {
        self.fill(menu);
        !menu.is_empty()
    }

    fn on_prepare_action_mode(&self, menu: &mut TextActionMenu) -> bool {
        self.fill(menu);
        true
    }

    fn on_destroy_action_mode(&self) {
        self.0.borrow_mut().mode = None;
    }
}

impl PlatformTextToolbar {
    pub fn new(modes: Option<Rc<dyn TextActionModes>>) -> PlatformTextToolbar {
        PlatformTextToolbar { modes, state: Rc::default() }
    }
}

impl TextToolbar for PlatformTextToolbar {
    fn show_menu(
        &self,
        rect: Rect,
        on_copy: Option<TextToolbarAction>,
        on_paste: Option<TextToolbarAction>,
        on_cut: Option<TextToolbarAction>,
        on_select_all: Option<TextToolbarAction>,
        on_autofill: Option<TextToolbarAction>,
    ) {
        use text_action_ids as id;
        use text_action_labels as label;
        let Some(modes) = &self.modes else { return };
        let items = [
            (id::COPY, label::COPY, on_copy),
            (id::PASTE, label::PASTE, on_paste),
            (id::CUT, label::CUT, on_cut),
            (id::SELECT_ALL, label::SELECT_ALL, on_select_all),
            (id::AUTOFILL, label::AUTOFILL, on_autofill),
        ];
        let mode = {
            let mut state = self.state.borrow_mut();
            state.rect = rect;
            state.items = items
                .into_iter()
                .filter_map(|(id, title, action)| Some((id, title, action?)))
                .collect();
            state.mode.clone()
        };
        match mode {
            Some(mode) => {
                mode.invalidate_content_rect();
                mode.invalidate();
            }
            None => {
                let callback = Rc::new(PlatformTextToolbarCallback(self.state.clone()));
                let mode = modes.start(callback);
                self.state.borrow_mut().mode = mode;
            }
        }
    }

    fn hide(&self) {
        let mode = self.state.borrow_mut().mode.take();
        if let Some(mode) = mode {
            mode.finish();
        }
    }

    fn status(&self) -> TextToolbarStatus {
        if self.state.borrow().mode.is_some() {
            TextToolbarStatus::Shown
        } else {
            TextToolbarStatus::Hidden
        }
    }
}

/// The text toolbar, as Compose's `LocalTextToolbar`: the tree's ([`PlatformTextToolbar`] over
/// its floating toolbars), or one showing nothing outside a tree's composition.
pub static LOCAL_TEXT_TOOLBAR: CompositionLocal<Rc<dyn TextToolbar>> =
    CompositionLocal::new("LocalTextToolbar", || {
        crate::platform_services::of_composing_tree(|tree| tree.text_toolbar())
            .unwrap_or_else(|| Rc::new(PlatformTextToolbar::new(None)))
    });

/// The floating toolbars of the tree being composed, if it has any: what a composable showing a
/// toolbar uses where none is provided.
pub fn text_action_modes_of_composing_tree() -> Option<Rc<dyn TextActionModes>> {
    crate::platform_services::of_composing_tree(|tree| tree.text_action_modes()).flatten()
}

impl crate::tree::UiTree {
    /// Sets the platform's floating toolbars (Android's action modes), before composing: what
    /// the text toolbar and the text context menu's toolbar show.
    pub fn set_text_action_modes(&mut self, modes: Option<Rc<dyn TextActionModes>>) {
        self.text_action_modes = modes;
        self.text_toolbar = None;
    }

    pub fn text_action_modes(&self) -> Option<Rc<dyn TextActionModes>> {
        self.text_action_modes.clone()
    }

    /// The tree's text toolbar (`LOCAL_TEXT_TOOLBAR`'s default), made at the first ask.
    pub fn text_toolbar(&mut self) -> Rc<dyn TextToolbar> {
        let modes = self.text_action_modes.clone();
        self.text_toolbar.get_or_insert_with(|| Rc::new(PlatformTextToolbar::new(modes))).clone()
    }
}
