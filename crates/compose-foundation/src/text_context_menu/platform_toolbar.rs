//! The platform's text toolbar as a text context menu's provider, as Compose's
//! `AndroidTextContextMenuToolbarProvider` and `ProvidePlatformTextContextMenuToolbar`: the menu
//! shown in the platform's floating toolbar (compose-ui's `TextActionModes`, Android's floating
//! `ActionMode`), its items and place following what they read (D2031).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use compose_runtime::composable;
use compose_runtime::platform::ReadObserver;
use compose_ui::{
    LayoutCoordinates, LocalBoxFuture, Modifier, NodeContext, Rect, TextActionMenu, TextActionMode,
    TextActionModeCallback, TextActionModes, text_action_ids,
};

use super::data::{
    TextContextMenuComponent, TextContextMenuData, TextContextMenuKeys, TextContextMenuSession,
};
use super::provider::{
    AnchorCoordinates, CloseSignal, Finally, LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER,
    TextContextMenuDataProvider, TextContextMenuProvider,
};
use crate::{MutatePriority, MutatorMutex};

/// The keys under which the toolbar's reads are observed: its items', and its place's.
const DATA_READS: u64 = 1;
const POSITION_READS: u64 = 2;

/// A text context menu's provider showing it in the platform's floating toolbar, as Compose's
/// `AndroidTextContextMenuToolbarProvider`: a new menu replaces the open one; an item's click
/// runs its action (closing the menu through the session, if it says); the platform closing the
/// toolbar (a touch outside it) ends the session.
#[doc(hidden)]
pub struct PlatformTextContextMenuToolbarProvider {
    inner: Rc<Inner>,
}

struct Inner {
    modes: Rc<dyn TextActionModes>,
    coordinates: AnchorCoordinates,
    mutex: MutatorMutex,
    /// Observes what the shown toolbar's items and place read (Compose's
    /// `SnapshotStateObserver`).
    observer: ReadObserver,
    /// The shown toolbar, invalidated when what it read changes.
    mode: RefCell<Option<Rc<dyn TextActionMode>>>,
}

impl Inner {
    /// What a write invalidated: the toolbar's items, or its place.
    fn refresh(&self) {
        let mode = self.mode.borrow().clone();
        let mut data = false;
        let mut position = false;
        self.observer.drain_invalidated(|key| match key {
            DATA_READS => data = true,
            POSITION_READS => position = true,
            _ => {}
        });
        let Some(mode) = mode else { return };
        if data {
            mode.invalidate();
        }
        if position {
            mode.invalidate_content_rect();
        }
    }
}

impl PlatformTextContextMenuToolbarProvider {
    /// A provider over `modes`, placing the menu in what `coordinates` gives.
    pub fn new(
        modes: Rc<dyn TextActionModes>,
        coordinates: AnchorCoordinates,
    ) -> PlatformTextContextMenuToolbarProvider {
        let inner = Rc::new_cyclic(|weak: &Weak<Inner>| {
            let weak = weak.clone();
            Inner {
                modes,
                coordinates,
                mutex: MutatorMutex::new(),
                // Told at a write, it refreshes the toolbar in a task, as Compose's executor
                // posts it.
                observer: ReadObserver::new(move || {
                    let weak = weak.clone();
                    compose_runtime::platform::launch(async move {
                        if let Some(inner) = weak.upgrade() {
                            inner.refresh();
                        }
                    });
                }),
                mode: RefCell::new(None),
            }
        });
        PlatformTextContextMenuToolbarProvider { inner }
    }

    /// Closes the shown toolbar and stops observing, as Compose's `dispose`.
    pub fn dispose(&self) {
        self.inner.observer.forget(DATA_READS);
        self.inner.observer.forget(POSITION_READS);
        let mode = self.inner.mode.borrow_mut().take();
        if let Some(mode) = mode {
            mode.finish();
        }
    }
}

struct Session(Rc<CloseSignal>);

impl TextContextMenuSession for Session {
    fn close(&self) {
        self.0.close();
    }
}

impl TextContextMenuProvider for PlatformTextContextMenuToolbarProvider {
    fn show_text_context_menu(
        &self,
        data_provider: Rc<dyn TextContextMenuDataProvider>,
    ) -> LocalBoxFuture<'static, ()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let _ = inner
                .mutex
                .mutate(MutatePriority::Default, async {
                    let signal: Rc<CloseSignal> = Rc::default();
                    let callback = Rc::new(ToolbarCallback {
                        session: Rc::new(Session(signal.clone())),
                        data_provider,
                        inner: Rc::downgrade(&inner),
                        previous: RefCell::new(None),
                    });
                    let Some(mode) = inner.modes.start(callback) else { return };
                    *inner.mode.borrow_mut() = Some(mode.clone());
                    // Interrupted, the next menu may be shown already (D850): only its own goes.
                    let own = inner.clone();
                    let _finally = Finally(Some(move || {
                        let current = own.mode.borrow().clone();
                        if current.is_some_and(|current| Rc::ptr_eq(&current, &mode)) {
                            own.observer.forget(DATA_READS);
                            own.observer.forget(POSITION_READS);
                            *own.mode.borrow_mut() = None;
                        }
                        mode.finish();
                    }));
                    signal.wait().await;
                })
                .await;
        })
    }
}

/// The floating toolbar's callback, as Compose's `TextActionModeCallbackImpl`.
struct ToolbarCallback {
    session: Rc<Session>,
    data_provider: Rc<dyn TextContextMenuDataProvider>,
    inner: Weak<Inner>,
    previous: RefCell<Option<TextContextMenuData>>,
}

impl ToolbarCallback {
    /// Fills `menu` with the menu's items: whether they changed. Separators start a new group;
    /// Compose's own items take Android's ids.
    fn update_menu_items(&self, menu: &mut TextActionMenu) -> bool {
        let Some(inner) = self.inner.upgrade() else { return false };
        let data = inner.observer.observe(DATA_READS, || self.data_provider.data());
        let changed =
            !self.previous.borrow().as_ref().is_some_and(|previous| previous.shows_as(&data));
        menu.clear();
        let (mut group, mut order) = (1, 1);
        for component in &data.components {
            match component {
                TextContextMenuComponent::Item(item) => {
                    let id = standard_id(&item.key).unwrap_or(order);
                    let (action, session) = (item.on_click.clone(), self.session.clone());
                    menu.add(group, id, order, &item.label, Rc::new(move || action(&*session)));
                    order += 1;
                }
                TextContextMenuComponent::Separator => group += 1,
            }
        }
        *self.previous.borrow_mut() = Some(data);
        changed
    }
}

/// Android's id of one of Compose's own items.
fn standard_id(key: &super::data::TextContextMenuKey) -> Option<i32> {
    let id = match key.downcast_ref::<TextContextMenuKeys>()? {
        TextContextMenuKeys::Cut => text_action_ids::CUT,
        TextContextMenuKeys::Copy => text_action_ids::COPY,
        TextContextMenuKeys::Paste => text_action_ids::PASTE,
        TextContextMenuKeys::SelectAll => text_action_ids::SELECT_ALL,
        TextContextMenuKeys::Autofill => text_action_ids::AUTOFILL,
    };
    Some(id)
}

impl TextActionModeCallback for ToolbarCallback {
    /// The menu's bounds in the anchor, in the root (Compose's `positionInRoot` translation);
    /// zero while the anchor isn't placed.
    fn on_get_content_rect(&self) -> Rect {
        let Some(inner) = self.inner.upgrade() else { return Rect::ZERO };
        inner.observer.observe(POSITION_READS, || {
            let Some(destination) = (inner.coordinates)().filter(LayoutCoordinates::is_attached)
            else {
                return Rect::ZERO;
            };
            let local = self.data_provider.content_bounds(&destination);
            local.translate(destination.position_in_root())
        })
    }

    fn on_create_action_mode(&self, menu: &mut TextActionMenu) -> bool {
        self.update_menu_items(menu);
        !menu.is_empty()
    }

    fn on_prepare_action_mode(&self, menu: &mut TextActionMenu) -> bool {
        self.update_menu_items(menu)
    }

    fn on_destroy_action_mode(&self) {
        self.session.close();
    }
}

/// Provides `LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER` with the platform's toolbar over `content`,
/// in a box (`propagate_min_constraints`) whose place the menu's bounds are given in, as
/// Compose's `ProvidePlatformTextContextMenuToolbar`. Without a platform toolbar, it provides
/// nothing.
#[doc(hidden)]
#[composable]
pub fn ProvidePlatformTextContextMenuToolbar(modifier: Modifier, content: impl Fn() + 'static) {
    let coordinates = compose_runtime::remember(|| {
        compose_runtime::mutable_state_of_with(
            None::<LayoutCoordinates>,
            compose_runtime::never_equal_policy(),
        )
    });
    let anchor: AnchorCoordinates = compose_runtime::remember(|| {
        let anchor: AnchorCoordinates = Rc::new(move || coordinates.get());
        anchor
    });
    let provider = platform_text_context_menu_toolbar_provider(anchor);
    let content = content.clone();
    let body = move || {
        let content = content.clone();
        crate::layout::Box(
            modifier.clone().on_globally_positioned(move |placed| coordinates.set(Some(placed))),
            crate::Alignment::TOP_START,
            true,
            compose_runtime::callback(move |_| content()),
        );
    };
    match provider {
        Some(provider) => compose_runtime::CompositionLocalProvider(
            LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.provides(Some(provider)),
            body,
        ),
        None => body(),
    }
}

/// Remembers a platform toolbar's provider over the tree's floating toolbars, placing menus in
/// `coordinates`, closing its toolbar when it leaves, as Compose's
/// `platformTextContextMenuToolbarProvider`. None without floating toolbars.
#[doc(hidden)]
pub fn platform_text_context_menu_toolbar_provider(
    coordinates: AnchorCoordinates,
) -> Option<Rc<dyn TextContextMenuProvider>> {
    let modes = compose_ui::platform::text_action_modes_of_composing_tree();
    let provider = compose_runtime::remember_keyed(modes.clone(), move || {
        modes.map(|modes| Rc::new(PlatformTextContextMenuToolbarProvider::new(modes, coordinates)))
    });
    let disposed = provider.clone();
    compose_runtime::DisposableEffect(provider.clone(), move |scope| {
        scope.on_dispose(move || {
            if let Some(provider) = &disposed {
                provider.dispose();
            }
        })
    });
    provider.map(|provider| provider as Rc<dyn TextContextMenuProvider>)
}

/// A tree's floating toolbars (kept weakly) and its default toolbar provider.
type DefaultToolbar = (Weak<dyn TextActionModes>, Rc<PlatformTextContextMenuToolbarProvider>);

thread_local! {
    /// The default toolbar provider of each tree's floating toolbars (D2031).
    static DEFAULT_TOOLBARS: RefCell<Vec<DefaultToolbar>> =
        const { RefCell::new(Vec::new()) };
}

/// The platform's toolbar provider of `context`'s tree, placing menus in its root: what a field
/// or a container shows its toolbar with when none is provided, as Compose's
/// `ProvideDefaultPlatformTextContextMenuProviders` provides it (D2031). None without floating
/// toolbars.
pub(crate) fn default_toolbar_provider(
    context: &mut NodeContext<'_>,
) -> Option<Rc<dyn TextContextMenuProvider>> {
    let modes = context.text_action_modes()?;
    let root = context.require_layout_coordinates().find_root_coordinates();
    DEFAULT_TOOLBARS.with(|toolbars| {
        let mut toolbars = toolbars.borrow_mut();
        toolbars.retain(|(modes, _)| modes.strong_count() > 0);
        if let Some((_, provider)) = toolbars
            .iter()
            .find(|(kept, _)| kept.upgrade().is_some_and(|kept| Rc::ptr_eq(&kept, &modes)))
        {
            return Some(provider.clone() as Rc<dyn TextContextMenuProvider>);
        }
        let anchor: AnchorCoordinates = Rc::new(move || Some(root));
        let provider = Rc::new(PlatformTextContextMenuToolbarProvider::new(modes.clone(), anchor));
        toolbars.push((Rc::downgrade(&modes), provider.clone()));
        Some(provider as Rc<dyn TextContextMenuProvider>)
    })
}
