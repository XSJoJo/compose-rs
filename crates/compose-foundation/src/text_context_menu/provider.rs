//! What shows a text context menu, as Compose's `text.contextmenu.provider`.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use compose_runtime::{ComposeArg, CompositionLocal, composable};
use compose_ui::{LayoutCoordinates, LocalBoxFuture, Modifier, Offset, Rect};

use super::data::{TextContextMenuData, TextContextMenuSession};
use crate::{MutatePriority, MutatorMutex};

/// What shows a text context menu and how it looks, as Compose's `TextContextMenuProvider`.
/// Provide another to [`LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER`] or
/// [`LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER`] to replace one.
pub trait TextContextMenuProvider {
    /// Shows the menu `data_provider` describes, as Compose's suspending
    /// `showTextContextMenu(dataProvider)`: the future ends when the menu closes; dropped before
    /// (its job cancelled), it closes the menu.
    fn show_text_context_menu(
        &self,
        data_provider: Rc<dyn TextContextMenuDataProvider>,
    ) -> LocalBoxFuture<'static, ()>;
}

/// What a menu shows and where, as Compose's `TextContextMenuDataProvider`. Each function reads
/// state as it likes: what shows the menu follows what they read.
pub trait TextContextMenuDataProvider {
    /// Where the menu goes, in `destination`'s coordinates.
    fn position(&self, destination: &LayoutCoordinates) -> Offset;
    /// What the menu goes around, in `destination`'s coordinates.
    fn content_bounds(&self, destination: &LayoutCoordinates) -> Rect;
    /// What it shows.
    fn data(&self) -> TextContextMenuData;
}

/// A provider in a composition local: none by default.
pub type TextContextMenuProviderLocal = CompositionLocal<Option<Rc<dyn TextContextMenuProvider>>>;

/// The dropdown's provider (a right click's menu), as Compose's
/// `LocalTextContextMenuDropdownProvider`: none by default, where fields and containers use the
/// default one (D2031).
pub static LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER: TextContextMenuProviderLocal =
    CompositionLocal::new("LocalTextContextMenuDropdownProvider", || None);

/// The toolbar's provider (a finger's selection's), as Compose's
/// `LocalTextContextMenuToolbarProvider`: none by default, where fields and containers use the
/// platform's (D2031).
pub static LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER: TextContextMenuProviderLocal =
    CompositionLocal::new("LocalTextContextMenuToolbarProvider", || None);

/// A composition local of providers, as a composable's parameter: compared by address.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct ProviderLocal(pub &'static TextContextMenuProviderLocal);

impl ComposeArg for ProviderLocal {
    fn same(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

/// Where a menu's anchor is laid out, as Compose's `anchorLayoutCoordinates`: none before it's
/// placed.
pub type AnchorCoordinates = Rc<dyn Fn() -> Option<LayoutCoordinates>>;

/// A menu's composable, as `ProvideBasicTextContextMenu`'s `contextMenu`: composed while a session
/// is open, with it, its data and the anchor's coordinates.
pub type ContextMenuBlock = Rc<
    dyn Fn(Rc<dyn TextContextMenuSession>, Rc<dyn TextContextMenuDataProvider>, AnchorCoordinates),
>;

/// A session's end, as Compose's `Channel<Unit>`: closed once, waited on.
#[derive(Default)]
pub(crate) struct CloseSignal {
    closed: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl CloseSignal {
    pub(crate) fn close(&self) {
        self.closed.set(true);
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }

    /// Ends when it's closed.
    pub(crate) fn wait(self: Rc<Self>) -> impl Future<Output = ()> {
        WaitClose(self)
    }
}

struct WaitClose(Rc<CloseSignal>);

impl Future for WaitClose {
    type Output = ();
    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.0.closed.get() {
            Poll::Ready(())
        } else {
            *self.0.waker.borrow_mut() = Some(context.waker().clone());
            Poll::Pending
        }
    }
}

/// Runs `f` when dropped: a `finally`.
pub(crate) struct Finally<F: FnOnce()>(pub Option<F>);

impl<F: FnOnce()> Drop for Finally<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

/// A provider composing its menu where it's provided, as Compose's
/// `BasicTextContextMenuProvider`: a new menu replaces the open one.
#[doc(hidden)]
pub struct BasicTextContextMenuProvider {
    inner: Rc<BasicInner>,
}

struct BasicInner {
    block: ContextMenuBlock,
    mutex: MutatorMutex,
    session: compose_runtime::OwnedState<Option<Rc<BasicSession>>>,
}

struct BasicSession {
    data_provider: Rc<dyn TextContextMenuDataProvider>,
    signal: Rc<CloseSignal>,
}

impl PartialEq for BasicSession {
    fn eq(&self, other: &BasicSession) -> bool {
        std::ptr::eq(self, other)
    }
}

impl TextContextMenuSession for BasicSession {
    fn close(&self) {
        self.signal.close();
    }
}

impl BasicTextContextMenuProvider {
    pub fn new(block: ContextMenuBlock) -> BasicTextContextMenuProvider {
        BasicTextContextMenuProvider {
            inner: Rc::new(BasicInner {
                block,
                mutex: MutatorMutex::new(),
                session: compose_runtime::mutable_state_of(None),
            }),
        }
    }

    /// Composes the open menu, if any, as Compose's `ContextMenu(anchorLayoutCoordinates)`.
    pub fn context_menu(&self, anchor: AnchorCoordinates) {
        let Some(session) = self.inner.session.get() else { return };
        let data_provider = session.data_provider.clone();
        (self.inner.block)(session, data_provider, anchor);
    }

    /// Closes the open menu, as Compose's `cancel`.
    pub fn cancel(&self) {
        if let Some(session) = self.inner.session.get() {
            session.signal.close();
        }
    }
}

impl TextContextMenuProvider for BasicTextContextMenuProvider {
    fn show_text_context_menu(
        &self,
        data_provider: Rc<dyn TextContextMenuDataProvider>,
    ) -> LocalBoxFuture<'static, ()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let session = Rc::new(BasicSession { data_provider, signal: Rc::default() });
            let _ = inner
                .mutex
                .mutate(MutatePriority::Default, async {
                    let (shown, own) = (inner.clone(), session.clone());
                    // Interrupted, the next session may be open already (D850): only its own
                    // goes.
                    let _finally = Finally(Some(move || {
                        let still = shown
                            .session
                            .with(|open| open.as_ref().is_some_and(|open| Rc::ptr_eq(open, &own)));
                        if still {
                            shown.session.set(None);
                        }
                    }));
                    inner.session.set(Some(session.clone()));
                    session.signal.clone().wait().await;
                })
                .await;
        })
    }
}

/// Remembers a [`BasicTextContextMenuProvider`] composing `block`, closing its menu when it
/// leaves, as Compose's `basicTextContextMenuProvider`.
#[doc(hidden)]
pub fn basic_text_context_menu_provider(
    block: ContextMenuBlock,
) -> Rc<BasicTextContextMenuProvider> {
    let provider = compose_runtime::remember_keyed(block.clone(), || {
        Rc::new(BasicTextContextMenuProvider::new(block))
    });
    let disposed = provider.clone();
    compose_runtime::DisposableEffect(provider.clone(), move |scope| {
        scope.on_dispose(move || disposed.cancel())
    });
    provider
}

/// Provides `local` with a provider composing `context_menu` beside `content`, in a box over it
/// (`propagate_min_constraints`) whose place the menu is anchored to, as Compose's internal
/// `ProvideBasicTextContextMenu(modifier, providableCompositionLocal, contextMenu, content)`.
#[doc(hidden)]
#[composable]
pub fn ProvideBasicTextContextMenu(
    modifier: Modifier,
    local: ProviderLocal,
    context_menu: ContextMenuBlock,
    content: impl Fn() + 'static,
) {
    let coordinates = compose_runtime::remember(|| {
        compose_runtime::mutable_state_of_with(
            None::<LayoutCoordinates>,
            compose_runtime::never_equal_policy(),
        )
    });
    let provider = basic_text_context_menu_provider(context_menu);
    let as_provider: Rc<dyn TextContextMenuProvider> = provider.clone();
    compose_runtime::CompositionLocalProvider(local.0.provides(Some(as_provider)), || {
        let content = content.clone();
        let provider = provider.clone();
        crate::layout::Box(
            modifier.clone().on_globally_positioned(move |placed| coordinates.set(Some(placed))),
            crate::Alignment::TOP_START,
            true,
            compose_runtime::callback(move |_| {
                content();
                let anchor: AnchorCoordinates = Rc::new(move || coordinates.get());
                provider.context_menu(anchor);
            }),
        );
    });
}
