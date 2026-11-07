//! A floating text toolbar for tests: what a test rule's tree shows its text toolbars in, as
//! Compose's device tests show them in Android's floating action mode and read them through
//! their spy callback (`SpyTextActionModeCallback`)
//! (docs/parity/design/text-context-menu.md).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use compose_ui::{
    Rect, TextActionMenu, TextActionMenuItem, TextActionMode, TextActionModeCallback,
};

/// The floating toolbars of a test's tree, behaving as Android's floating action mode does: a
/// toolbar is created with its callback's menu (one with an empty menu doesn't start), prepared
/// again when invalidated, placed again when its rect is invalidated, destroyed when finished,
/// dismissed or replaced by another (a view shows one floating toolbar).
#[derive(Default)]
pub struct TestTextToolbar {
    inner: Rc<Inner>,
}

#[derive(Default)]
struct Inner {
    shown: RefCell<Option<Shown>>,
    next_id: Cell<u64>,
    started: Cell<usize>,
}

struct Shown {
    id: u64,
    callback: Rc<dyn TextActionModeCallback>,
    menu: TextActionMenu,
    rect: Rect,
    /// The rect it started with.
    first_rect: Rect,
}

struct Mode {
    inner: Weak<Inner>,
    id: u64,
}

impl Inner {
    /// The shown toolbar's callback, if it's `id`'s.
    fn callback_of(&self, id: u64) -> Option<Rc<dyn TextActionModeCallback>> {
        self.shown
            .borrow()
            .as_ref()
            .filter(|shown| shown.id == id)
            .map(|shown| shown.callback.clone())
    }

    /// Closes the shown toolbar, if any (and if it's `id`'s, given one), telling its callback.
    fn destroy(&self, id: Option<u64>) {
        let shown = {
            let mut slot = self.shown.borrow_mut();
            if slot.as_ref().is_some_and(|shown| id.is_none_or(|id| shown.id == id)) {
                slot.take()
            } else {
                None
            }
        };
        if let Some(shown) = shown {
            shown.callback.on_destroy_action_mode();
        }
    }
}

impl TextActionMode for Mode {
    fn invalidate(&self) {
        let Some(inner) = self.inner.upgrade() else { return };
        let Some(callback) = inner.callback_of(self.id) else { return };
        let mut menu = TextActionMenu::new();
        if let Some(shown) = inner.shown.borrow_mut().as_mut() {
            menu = std::mem::take(&mut shown.menu);
        }
        callback.on_prepare_action_mode(&mut menu);
        if let Some(shown) = inner.shown.borrow_mut().as_mut().filter(|shown| shown.id == self.id) {
            shown.menu = menu;
        }
    }

    fn invalidate_content_rect(&self) {
        let Some(inner) = self.inner.upgrade() else { return };
        let Some(callback) = inner.callback_of(self.id) else { return };
        let rect = callback.on_get_content_rect();
        if let Some(shown) = inner.shown.borrow_mut().as_mut().filter(|shown| shown.id == self.id) {
            shown.rect = rect;
        }
    }

    fn finish(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.destroy(Some(self.id));
        }
    }
}

impl compose_ui::TextActionModes for TestTextToolbar {
    fn start(&self, callback: Rc<dyn TextActionModeCallback>) -> Option<Rc<dyn TextActionMode>> {
        // A view shows one floating toolbar: a new one replaces it.
        self.inner.destroy(None);
        let mut menu = TextActionMenu::new();
        if !callback.on_create_action_mode(&mut menu) {
            return None;
        }
        let rect = callback.on_get_content_rect();
        let id = self.inner.next_id.get() + 1;
        self.inner.next_id.set(id);
        self.inner.started.set(self.inner.started.get() + 1);
        *self.inner.shown.borrow_mut() = Some(Shown { id, callback, menu, rect, first_rect: rect });
        Some(Rc::new(Mode { inner: Rc::downgrade(&self.inner), id }))
    }
}

impl TestTextToolbar {
    pub fn new() -> TestTextToolbar {
        TestTextToolbar::default()
    }

    /// Whether a toolbar shows (its spy's `menu` isn't null).
    pub fn is_shown(&self) -> bool {
        self.inner.shown.borrow().is_some()
    }

    /// The shown toolbar's items, in order; none while hidden.
    pub fn items(&self) -> Vec<TextActionMenuItem> {
        self.inner
            .shown
            .borrow()
            .as_ref()
            .map(|shown| shown.menu.items().to_vec())
            .unwrap_or_default()
    }

    /// The shown toolbar's titles, in order.
    pub fn titles(&self) -> Vec<String> {
        self.items().into_iter().map(|item| item.title).collect()
    }

    /// Whether the shown toolbar has an item titled `title`.
    pub fn has_item(&self, title: &str) -> bool {
        self.items().iter().any(|item| item.title == title)
    }

    /// Where the shown toolbar goes, as its callback last said (in the root).
    pub fn content_rect(&self) -> Option<Rect> {
        self.inner.shown.borrow().as_ref().map(|shown| shown.rect)
    }

    /// Where the shown toolbar went when it started: what Android's floating toolbar keeps when
    /// told of a new place before it first shows (D2033).
    pub fn first_content_rect(&self) -> Option<Rect> {
        self.inner.shown.borrow().as_ref().map(|shown| shown.first_rect)
    }

    /// How many toolbars started since the rule began.
    pub fn started_count(&self) -> usize {
        self.inner.started.get()
    }

    /// Clicks the shown toolbar's item titled `title`, without closing it (Android's
    /// `performIdentifierAction(id, FLAG_PERFORM_NO_CLOSE)`): its action decides. Panics if no
    /// item has that title.
    pub fn click(&self, title: &str) {
        let item =
            self.items().into_iter().find(|item| item.title == title).unwrap_or_else(|| {
                panic!("no toolbar item titled {title:?} in {:?}", self.titles())
            });
        (item.on_click)();
    }

    /// Closes the shown toolbar as the platform does (a touch outside it, back): its callback is
    /// told it's destroyed.
    pub fn dismiss(&self) {
        self.inner.destroy(None);
    }
}
