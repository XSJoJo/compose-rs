//! The toolbar's handler: its requester shows the toolbar through the provider and hides it,
//! across attaching and detaching (D2031), from Compose's `TextContextMenuToolbarHandlerTest`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::modifier::{
    ComputeContentBounds, ToolbarRequester, text_context_menu_toolbar_handler,
};
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_runtime::mutable_state_of;
use compose_test::*;
use compose_ui::{LayoutCoordinates, LocalBoxFuture, Offset, Rect, Size};

use crate::text_context_menu_support::*;

const DEFAULT_RECT: Rect = Rect { left: 0.0, top: 0.0, right: 10.0, bottom: 10.0 };

type OnShow = Rc<dyn Fn(Rc<dyn TextContextMenuDataProvider>) -> LocalBoxFuture<'static, ()>>;

/// A provider calling `on_show` and keeping how its last call went, as Compose's
/// `TestTextContextMenuProvider` with its `previousJob`.
struct TestProvider {
    on_show: OnShow,
    previous: RefCell<Option<Rc<Cell<Outcome>>>>,
}

impl TestProvider {
    fn new(
        on_show: impl Fn(Rc<dyn TextContextMenuDataProvider>) -> LocalBoxFuture<'static, ()> + 'static,
    ) -> Rc<TestProvider> {
        Rc::new(TestProvider { on_show: Rc::new(on_show), previous: RefCell::new(None) })
    }

    fn previous(&self) -> Option<Outcome> {
        self.previous.borrow().as_ref().map(|outcome| outcome.get())
    }
}

/// Sets the outcome when its future completes, or is dropped first.
struct Probe(Rc<Cell<Outcome>>);

impl Drop for Probe {
    fn drop(&mut self) {
        if self.0.get() == Outcome::Active {
            self.0.set(Outcome::Cancelled);
        }
    }
}

impl TextContextMenuProvider for TestProvider {
    fn show_text_context_menu(
        &self,
        data_provider: Rc<dyn TextContextMenuDataProvider>,
    ) -> LocalBoxFuture<'static, ()> {
        let outcome = Rc::new(Cell::new(Outcome::Active));
        *self.previous.borrow_mut() = Some(outcome.clone());
        let shown = (self.on_show)(data_provider);
        Box::pin(async move {
            let probe = Probe(outcome);
            shown.await;
            probe.0.set(Outcome::Completed);
        })
    }
}

/// A future that never ends, as `awaitCancellation()`.
fn await_cancellation() -> LocalBoxFuture<'static, ()> {
    Box::pin(std::future::pending())
}

fn default_bounds() -> ComputeContentBounds {
    Rc::new(|_| Some(DEFAULT_RECT))
}

fn handler(requester: &ToolbarRequester) -> Modifier {
    text_context_menu_toolbar_handler(Modifier, requester.clone(), None, None, default_bounds())
}

fn with_provider(provider: Rc<TestProvider>, content: impl Fn() + 'static) {
    let provider: Rc<dyn TextContextMenuProvider> = provider;
    compose_runtime::CompositionLocalProvider(
        LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.provides(Some(provider)),
        content,
    );
}

fn calls(
    count: &Rc<Cell<i32>>,
) -> impl Fn(Rc<dyn TextContextMenuDataProvider>) -> LocalBoxFuture<'static, ()> + 'static {
    let count = count.clone();
    move |_| {
        count.set(count.get() + 1);
        await_cancellation()
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenCallShow_providerCalled
#[test]
fn show_calls_the_provider() {
    let requester = ToolbarRequester::new();
    let coordinates: Rc<RefCell<Option<LayoutCoordinates>>> = Rc::default();
    let count = Rc::new(Cell::new(0));
    let (c, k) = (coordinates.clone(), count.clone());
    let provider = TestProvider::new(move |data| {
        k.set(k.get() + 1);
        let destination = (*c.borrow()).expect("not placed");
        assert_eq!(data.content_bounds(&destination), DEFAULT_RECT);
        let keys: Vec<_> = data.data().components.iter().map(|it| it.key()).collect();
        assert_eq!(keys, [TextContextMenuKey::new(1)]);
        Box::pin(async {})
    });
    let (p, r, c) = (provider.clone(), requester.clone(), coordinates.clone());
    let mut ui = compose_test(move || {
        let (r, c) = (r.clone(), c.clone());
        with_provider(p.clone(), move || {
            let (c, placed) = (c.clone(), c.clone());
            let modifier = Modifier
                .on_globally_positioned(move |it| *placed.borrow_mut() = Some(it))
                .append_text_context_menu_components(|scope| scope.item(1, "1", None, |_| {}));
            Box!(
                modifier = text_context_menu_toolbar_handler(
                    modifier,
                    r.clone(),
                    None,
                    None,
                    Rc::new(move |destination: &LayoutCoordinates| {
                        let placed = (*c.borrow()).expect("not placed");
                        assert_eq!(*destination, placed);
                        Some(DEFAULT_RECT)
                    }),
                )
            );
        });
    });
    requester.show();
    ui.wait_for_idle();
    assert_eq!(provider.previous(), Some(Outcome::Completed));
    assert_eq!(count.get(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenShow_jobKickedOffAndCompletesGracefully
#[test]
fn a_shown_toolbar_s_job_ends_with_its_provider() {
    let requester = ToolbarRequester::new();
    let count = Rc::new(Cell::new(0));
    let release = Rc::new(compose_foundation_test_signal::Signal::default());
    let (k, s) = (count.clone(), release.clone());
    let provider = TestProvider::new(move |_| {
        k.set(k.get() + 1);
        Box::pin(s.clone().wait())
    });
    let (p, r) = (provider.clone(), requester.clone());
    let mut ui = compose_test(move || {
        let r = r.clone();
        with_provider(p.clone(), move || Box!(modifier = handler(&r)));
    });
    requester.show();
    ui.wait_until(|_| count.get() > 0);
    assert_eq!(provider.previous(), Some(Outcome::Active));
    assert_eq!(count.get(), 1);
    release.send();
    ui.wait_for_idle();
    assert_eq!(provider.previous(), Some(Outcome::Completed));
    assert_eq!(count.get(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenCallHide_providerCancelled
#[test]
fn hide_cancels_the_provider() {
    let requester = ToolbarRequester::new();
    let count = Rc::new(Cell::new(0));
    let provider = TestProvider::new(calls(&count));
    let (p, r) = (provider.clone(), requester.clone());
    let mut ui = compose_test(move || {
        let r = r.clone();
        with_provider(p.clone(), move || Box!(modifier = handler(&r)));
    });
    requester.show();
    ui.wait_until(|_| count.get() > 0);
    assert_eq!(provider.previous(), Some(Outcome::Active));
    requester.hide();
    ui.wait_for_idle();
    assert_eq!(provider.previous(), Some(Outcome::Cancelled));
    assert_eq!(count.get(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenRegularCall_onShowAndOnHideInvoked
#[test]
fn on_show_and_on_hide_are_called() {
    let requester = ToolbarRequester::new();
    let (count, shows, hides) =
        (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let provider = TestProvider::new(calls(&count));
    let (p, r, sh, hi) = (provider.clone(), requester.clone(), shows.clone(), hides.clone());
    let mut ui = compose_test(move || {
        let (r, sh, hi) = (r.clone(), sh.clone(), hi.clone());
        with_provider(p.clone(), move || {
            let (sh, hi) = (sh.clone(), hi.clone());
            Box!(
                modifier = text_context_menu_toolbar_handler(
                    Modifier,
                    r.clone(),
                    Some(Rc::new(move || {
                        sh.set(sh.get() + 1);
                        Box::pin(async {}) as LocalBoxFuture<'static, ()>
                    })),
                    Some(Rc::new(move || hi.set(hi.get() + 1))),
                    default_bounds(),
                )
            );
        });
    });
    requester.show();
    ui.wait_until(|_| count.get() > 0);
    assert_eq!(provider.previous(), Some(Outcome::Active));
    assert_eq!((count.get(), shows.get(), hides.get()), (1, 1, 0));
    requester.hide();
    ui.wait_for_idle();
    assert_eq!(provider.previous(), Some(Outcome::Cancelled));
    assert_eq!((count.get(), shows.get(), hides.get()), (1, 1, 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenNeverAttached_showThrows
#[test]
#[should_panic(expected = "ToolbarRequester is not initialized.")]
fn show_before_any_handler_panics() {
    ToolbarRequester::new().show();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenNeverAttached_hideDoesNotThrow
#[test]
fn hide_before_any_handler_does_nothing() {
    ToolbarRequester::new().hide();
}

/// A handler shown while `show` is on, with no provider.
fn toggled_handler(
    requester: &ToolbarRequester,
    show: compose_runtime::MutableState<bool>,
) -> ComposeTestRule {
    let r = requester.clone();
    compose_test(move || {
        let modifier = if show.get() { handler(&r) } else { Modifier };
        Box!(modifier = modifier);
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenDetached_showDoesNotThrow
#[test]
fn show_once_detached_does_nothing() {
    let requester = ToolbarRequester::new();
    let show = **Box::leak(Box::new(mutable_state_of(true)));
    let mut ui = toggled_handler(&requester, show);
    show.set(false);
    ui.wait_for_idle();
    requester.show();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenDetached_hideDoesNotThrow
#[test]
fn hide_once_detached_does_nothing() {
    let requester = ToolbarRequester::new();
    let show = **Box::leak(Box::new(mutable_state_of(true)));
    let mut ui = toggled_handler(&requester, show);
    show.set(false);
    ui.wait_for_idle();
    requester.hide();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuToolbarHandlerTest.kt, whenReattached_showAndHideSucceed
#[test]
fn show_and_hide_work_once_attached_again() {
    let requester = ToolbarRequester::new();
    let count = Rc::new(Cell::new(0));
    let provider = TestProvider::new(calls(&count));
    let show = **Box::leak(Box::new(mutable_state_of(true)));
    let (p, r) = (provider.clone(), requester.clone());
    let mut ui = compose_test(move || {
        let r = r.clone();
        with_provider(p.clone(), move || {
            let modifier = if show.get() { handler(&r) } else { Modifier };
            Box!(modifier = modifier);
        });
    });
    show.set(false);
    ui.wait_for_idle();
    requester.show();
    show.set(true);
    ui.wait_for_idle();
    requester.show();
    ui.wait_until(|_| count.get() > 0);
    assert_eq!(provider.previous(), Some(Outcome::Active));
    assert_eq!(count.get(), 1);
    requester.hide();
    ui.wait_for_idle();
    assert_eq!(provider.previous(), Some(Outcome::Cancelled));
    assert_eq!(count.get(), 1);
}

/// Its bounds when its owner says none: the last ones, else zero, as Compose's
/// `contentBounds`.
#[test]
fn bounds_said_none_are_the_last() {
    let requester = ToolbarRequester::new();
    let data: Rc<RefCell<Option<Rc<dyn TextContextMenuDataProvider>>>> = Rc::default();
    let d = data.clone();
    let provider = TestProvider::new(move |it| {
        *d.borrow_mut() = Some(it);
        await_cancellation()
    });
    let answer = Rc::new(Cell::new(None::<Rect>));
    let (p, r, a) = (provider.clone(), requester.clone(), answer.clone());
    let destination: Rc<RefCell<Option<LayoutCoordinates>>> = Rc::default();
    let placed = destination.clone();
    let mut ui = compose_test(move || {
        let (r, a, placed) = (r.clone(), a.clone(), placed.clone());
        with_provider(p.clone(), move || {
            let (a, placed) = (a.clone(), placed.clone());
            Box!(
                modifier = text_context_menu_toolbar_handler(
                    Modifier.on_globally_positioned(move |it| *placed.borrow_mut() = Some(it)),
                    r.clone(),
                    None,
                    None,
                    Rc::new(move |_: &LayoutCoordinates| a.get()),
                )
            );
        });
    });
    requester.show();
    ui.wait_for_idle();
    let data = data.borrow().clone().unwrap();
    let destination = (*destination.borrow()).unwrap();
    assert_eq!(data.content_bounds(&destination), Rect::ZERO);
    let bounds = Rect::from_offset_size(Offset::new(1.0, 2.0), Size::new(3.0, 4.0));
    answer.set(Some(bounds));
    assert_eq!(data.content_bounds(&destination), bounds);
    assert_eq!(data.position(&destination), Offset::new(1.0, 2.0));
    answer.set(None);
    assert_eq!(data.content_bounds(&destination), bounds);
}

/// A one-shot signal a test sends, as Compose's tests' `Channel<Unit>`.
mod compose_foundation_test_signal {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::task::Waker;

    #[derive(Default)]
    pub struct Signal {
        sent: Cell<bool>,
        waker: RefCell<Option<Waker>>,
    }

    impl Signal {
        pub fn send(&self) {
            self.sent.set(true);
            if let Some(waker) = self.waker.borrow_mut().take() {
                waker.wake();
            }
        }

        pub async fn wait(self: Rc<Self>) {
            std::future::poll_fn(|context| {
                if self.sent.get() {
                    std::task::Poll::Ready(())
                } else {
                    *self.waker.borrow_mut() = Some(context.waker().clone());
                    std::task::Poll::Pending
                }
            })
            .await
        }
    }
}

/// Shown already, a show asks nothing more of the provider, as Compose's `show` while its job
/// is active.
#[test]
fn showing_while_shown_calls_the_provider_once() {
    let requester = ToolbarRequester::new();
    let count = Rc::new(Cell::new(0));
    let provider = TestProvider::new(calls(&count));
    let (p, r) = (provider.clone(), requester.clone());
    let mut ui = compose_test(move || {
        let r = r.clone();
        with_provider(p.clone(), move || Box!(modifier = handler(&r)));
    });
    requester.show();
    ui.wait_for_idle();
    requester.show();
    ui.wait_for_idle();
    assert_eq!(count.get(), 1);
    assert_eq!(provider.previous(), Some(Outcome::Active));
}
