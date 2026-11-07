//! compose-rs's regression tests of a field's toolbar, from a review (D2035), not ports.
//!
//! Where no provider is given, the tree's platform toolbar shows it, its rect in the window (the
//! host's view) for a field in a popup and in the screen alike. Compose wraps each field in its
//! own default providers (`ProvideDefaultPlatformTextContextMenuProviders`), anchored where the
//! field is; its tests all provide the toolbar.
//!
//! "Select all" and "Copy" ask whether the toolbar shows, not which one was asked for, as
//! Compose's `textToolbarShown`: with the selection scrolled out of view, the toolbar hides while
//! its state stays.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_runtime::{MutableState, mutable_state_of};
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, MemoryClipboard, UiHost};
use compose_ui::{
    ClipboardManager, Density, InputMode, IntSize, LocalBoxFuture, Offset, Popup, Rect,
    SemanticsActions, TextRange, UnitExt,
};

const FONT: f32 = 10.0;

fn leak<T: Copy + 'static>(value: compose_runtime::OwnedState<T>) -> MutableState<T>
where
    MutableState<T>: Copy,
{
    **Box::leak(Box::new(value))
}

fn new_state() -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new("Hello")))
}

fn field(state: TextFieldState, tag: &'static str) {
    BasicTextFieldOfState!(
        state,
        modifier = Modifier.width(100.dp()).test_tag(tag),
        text_style = TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT },
    );
}

/// A box of 100 dp, a box of 100 dp holding a popup with a field (while `show_popup`), then a
/// field: no toolbar provided.
fn run(
    show_popup: MutableState<bool>,
    popup: TextFieldState,
    main: TextFieldState,
) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let host = UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer));
    let mut ui = ComposeTestRule::empty(host);
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_content(move || {
        Column!({
            Box!(modifier = Modifier.size(100.dp()));
            Box!(modifier = Modifier.size(100.dp()), {
                if show_popup.get() {
                    Popup!(on_dismiss_request = || {}, {
                        field(popup, "popup");
                    });
                }
            });
            field(main, "main");
        });
    });
    ui
}

fn select(ui: &mut ComposeTestRule, tag: &str, (start, end): (i32, i32)) {
    ui.on_node_with_tag(tag).request_focus();
    ui.on_node_with_tag(tag).perform_semantics_action_with(
        &SemanticsActions::SET_SELECTION,
        |set| {
            set(start, end, false);
        },
    );
    ui.wait_for_idle();
}

#[test]
fn a_popup_fields_toolbar_is_placed_in_the_window() {
    let show_popup = leak(mutable_state_of(true));
    let (popup, main) = (new_state(), new_state());
    let mut ui = run(show_popup, popup, main);
    let toolbar = ui.text_toolbar();
    select(&mut ui, "popup", (2, 4));
    assert!(toolbar.is_shown());
    // The selection from offset 2 to 4 of the popup's field, the popup at the second box's top.
    assert_eq!(toolbar.content_rect(), Some(Rect::new(20.0, 100.0, 40.0, 110.0)));
}

#[test]
fn a_fields_toolbar_after_a_popup_closed_is_placed_in_the_window() {
    let show_popup = leak(mutable_state_of(true));
    let (popup, main) = (new_state(), new_state());
    let mut ui = run(show_popup, popup, main);
    let toolbar = ui.text_toolbar();
    select(&mut ui, "popup", (2, 4));
    assert!(toolbar.is_shown());
    show_popup.set(false);
    ui.wait_for_idle();
    select(&mut ui, "main", (2, 4));
    assert!(toolbar.is_shown());
    assert_eq!(toolbar.content_rect(), Some(Rect::new(20.0, 200.0, 40.0, 210.0)));
}

#[test]
fn a_fields_toolbar_before_a_popups_is_placed_in_the_window() {
    let show_popup = leak(mutable_state_of(true));
    let (popup, main) = (new_state(), new_state());
    let mut ui = run(show_popup, popup, main);
    let toolbar = ui.text_toolbar();
    select(&mut ui, "main", (2, 4));
    assert_eq!(toolbar.content_rect(), Some(Rect::new(20.0, 200.0, 40.0, 210.0)));
    select(&mut ui, "popup", (2, 4));
    assert_eq!(toolbar.content_rect(), Some(Rect::new(20.0, 100.0, 40.0, 110.0)));
}

/// A toolbar provider keeping the last menu's data provider, so a test can click its items after
/// it hid.
#[derive(Default)]
struct KeepingProvider {
    data: RefCell<Option<Rc<dyn TextContextMenuDataProvider>>>,
    shows: Cell<u32>,
    shown: Rc<Cell<bool>>,
}

struct Hides(Rc<Cell<bool>>);

impl Drop for Hides {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl TextContextMenuProvider for KeepingProvider {
    fn show_text_context_menu(
        &self,
        data_provider: Rc<dyn TextContextMenuDataProvider>,
    ) -> LocalBoxFuture<'static, ()> {
        *self.data.borrow_mut() = Some(data_provider);
        self.shows.set(self.shows.get() + 1);
        self.shown.set(true);
        let hides = Hides(self.shown.clone());
        Box::pin(async move {
            let _hides = hides;
            std::future::pending::<()>().await
        })
    }
}

#[derive(Default)]
struct Session(Cell<bool>);

impl TextContextMenuSession for Session {
    fn close(&self) {
        self.0.set(true);
    }
}

impl KeepingProvider {
    /// Clicks the kept menu's item titled `label`: whether it closed the menu.
    fn click(&self, label: &str) -> bool {
        let data = self.data.borrow().clone().expect("no menu shown").data();
        let item = data
            .components
            .iter()
            .filter_map(TextContextMenuComponent::as_item)
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("no {label}"))
            .clone();
        let session = Session::default();
        (item.on_click)(&session);
        session.0.get()
    }
}

const TAG: &str = "field";

/// A single-line field of "Hello " 20 times, 100 wide, its toolbar `provider`'s.
fn run_scrolled(provider: Rc<KeepingProvider>) -> (ComposeTestRule, TextFieldState) {
    let state: TextFieldState = **Box::leak(Box::new(TextFieldState::new("Hello ".repeat(20))));
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let host = UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer));
    let clipboard: Rc<dyn ClipboardManager> = Rc::new(MemoryClipboard::default());
    host.with_tree(|tree| tree.set_clipboard(clipboard));
    let mut ui = ComposeTestRule::empty(host);
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_content(move || {
        let provider: Rc<dyn TextContextMenuProvider> = provider.clone();
        compose_runtime::CompositionLocalProvider(
            LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.provides(Some(provider)),
            move || {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.width(100.dp()).test_tag(TAG),
                    text_style = TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT },
                    line_limits = TextFieldLineLimits::SingleLine,
                );
            },
        );
    });
    (ui, state)
}

/// Selects 2 to 4, showing the toolbar, then scrolls it out of view: the toolbar hides, its state
/// stays.
fn select_and_scroll_away(ui: &mut ComposeTestRule, provider: &KeepingProvider) {
    // A tap focuses it, its cursor in view at the start.
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(Offset::new(20.0, 5.0)));
    ui.wait_for_idle();
    select(ui, TAG, (2, 4));
    assert!(provider.shown.get());
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let double_tap = touch.view_configuration().double_tap_timeout_millis;
        touch.advance_event_time(double_tap * 2);
        touch.swipe_left_with(FONT * 8.0, 0.0, 200);
    });
    ui.wait_for_idle();
    assert!(!provider.shown.get(), "the selection should be out of view");
}

// "Select all" closes the menu unless the toolbar shows, and asks for the selection's toolbar
// (Compose's `addBasicTextFieldTextContextMenuComponents`, TextFieldSelectionState.android.kt).
#[test]
fn select_all_with_the_toolbar_hidden_closes_the_menu_and_shows_the_selections() {
    let provider = Rc::new(KeepingProvider::default());
    let (mut ui, state) = run_scrolled(provider.clone());
    select_and_scroll_away(&mut ui, &provider);
    assert!(provider.click("Select all"), "the menu should close: no toolbar shows");
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(0, 120));
    // The whole text's selection is in view: its toolbar shows.
    assert!(provider.shown.get());
    assert_eq!(provider.shows.get(), 2);
}

// "Copy" cancels the selection only while the toolbar shows (Compose's `copy(cancelSelection =
// textToolbarShown)`).
#[test]
fn copy_with_the_toolbar_hidden_keeps_the_selection() {
    let provider = Rc::new(KeepingProvider::default());
    let (mut ui, state) = run_scrolled(provider.clone());
    select_and_scroll_away(&mut ui, &provider);
    assert!(provider.click("Copy"));
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(2, 4));
}

/// A clipboard counting how often it's asked whether it holds anything.
#[derive(Default)]
struct CountingClipboard {
    text: RefCell<Option<String>>,
    asked: Cell<u32>,
}

impl ClipboardManager for CountingClipboard {
    fn get_text(&self) -> Option<String> {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = Some(text.to_owned());
    }

    fn has_text(&self) -> bool {
        self.text.borrow().as_ref().is_some_and(|text| !text.is_empty())
    }

    fn has_clip(&self) -> bool {
        self.asked.set(self.asked.get() + 1);
        self.text.borrow().is_some()
    }
}

// The clipboard is asked when the toolbar shows, not at each measurement, as Compose's
// `updateClipboardEntry` in the handler's `onShow`: paste follows it from one showing to the next.
#[test]
fn the_clipboard_is_asked_when_the_toolbar_shows() {
    let clipboard = Rc::new(CountingClipboard::default());
    let state = new_state();
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let host = UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer));
    let shared: Rc<dyn ClipboardManager> = clipboard.clone();
    host.with_tree(|tree| tree.set_clipboard(shared));
    let mut ui = ComposeTestRule::empty(host);
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_content(move || field(state, TAG));
    let toolbar = ui.text_toolbar();
    select(&mut ui, TAG, (2, 4));
    assert!(toolbar.is_shown());
    assert!(!toolbar.has_item("Paste"));
    let asked = clipboard.asked.get();
    assert!(asked >= 1);
    // Measured again with the toolbar shown: not asked.
    select(&mut ui, TAG, (1, 4));
    select(&mut ui, TAG, (1, 3));
    assert!(toolbar.is_shown());
    assert_eq!(clipboard.asked.get(), asked);
    // Hidden by a tap, shown again after the clipboard changed: paste.
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click_at(Offset::new(10.0, 5.0)));
    ui.wait_for_idle();
    assert!(!toolbar.is_shown());
    clipboard.set_text("world");
    select(&mut ui, TAG, (2, 4));
    assert!(toolbar.is_shown());
    assert!(toolbar.has_item("Paste"), "{:?}", toolbar.titles());
}
