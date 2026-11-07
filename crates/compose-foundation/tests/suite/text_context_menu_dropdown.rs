//! The default dropdown of a text context menu: its items in a popup at the data provider's
//! position, closed by its job, a click outside, an item or its provider leaving, replaced by the
//! next (D2037), from Compose's `DefaultTextContextMenuDropdownProviderTest`.
//!
//! The rule's root is 400 by 400 at density 1. Compose's three tests `@Ignore`d upstream
//! (b/414664796) are left out: `whenAnchorLayoutMoves_popupDoesNotMove`,
//! `whenAnchorLayoutScrolled_popupDoesNotMove`, `whenDataProviderPositionUpdates_popupDoesNotMove`.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::dropdown::ProvideDefaultTextContextMenuDropdown;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_runtime::{CoroutineScope, MutableState, mutable_state_of, remember_coroutine_scope};
use compose_test::*;
use compose_ui::{Color, Constraints, Dp, IntOffset, IntSize, Offset, PointerEvent, SizeSpec};

use crate::text_context_menu_support::*;

fn leak<T: Copy + 'static>(value: compose_runtime::OwnedState<T>) -> MutableState<T>
where
    MutableState<T>: Copy,
{
    **Box::leak(Box::new(value))
}

type Outer = Rc<dyn Fn(Rc<dyn Fn()>)>;

/// What a test reads of its content, as Compose's `TestScope`.
#[derive(Default)]
struct Scope {
    provider: RefCell<Option<Rc<dyn TextContextMenuProvider>>>,
    coroutine_scope: Cell<Option<CoroutineScope>>,
}

impl Scope {
    /// Compose's `showTextContextMenu`: launched, then the rule idles.
    fn show(
        &self,
        ui: &mut ComposeTestRule,
        data: Rc<dyn TextContextMenuDataProvider>,
    ) -> JobProbe {
        let provider = self.provider.borrow().clone().expect("no provider");
        let scope = self.coroutine_scope.get().expect("no scope");
        let job = launch(&scope, async move { provider.show_text_context_menu(data).await });
        ui.wait_for_idle();
        job
    }
}

/// Compose's `runTest(outerContent, innerContent)`.
fn run_test(outer: Outer, inner: Rc<dyn Fn()>) -> (ComposeTestRule, Rc<Scope>) {
    let scope = Rc::new(Scope::default());
    let reads = scope.clone();
    let ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        reads.coroutine_scope.set(Some(remember_coroutine_scope()));
        let (reads, inner) = (reads.clone(), inner.clone());
        outer(Rc::new(move || {
            let (reads, inner) = (reads.clone(), inner.clone());
            ProvideDefaultTextContextMenuDropdown!({
                let provider = LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.current();
                assert!(provider.is_some());
                *reads.provider.borrow_mut() = provider;
                inner();
            });
        }));
    });
    (ui, scope)
}

fn default_outer() -> Outer {
    Rc::new(|content| content())
}

fn default_inner() -> Rc<dyn Fn()> {
    Rc::new(|| Box!(modifier = Modifier.fill_max_size()))
}

fn run_default() -> (ComposeTestRule, Rc<Scope>) {
    run_test(default_outer(), default_inner())
}

fn on_item_with_number(ui: &mut ComposeTestRule, number: i32) -> SemanticsNodeInteraction<'_> {
    ui.on_node_with_text(&label_for_number(number))
}

fn assert_context_menu_does_not_exist(ui: &mut ComposeTestRule) {
    ui.on_all_nodes(is_popup()).assert_count_equals(0);
}

fn assert_context_menu_exists_with_numbers(ui: &mut ComposeTestRule, numbers: &[i32]) {
    ui.on_all_nodes(is_popup()).assert_count_equals(1);
    for &number in numbers {
        on_item_with_number(ui, number).assert_is_displayed();
    }
}

fn assert_items_do_not_exist(ui: &mut ComposeTestRule, numbers: &[i32]) {
    for &number in numbers {
        on_item_with_number(ui, number).assert_does_not_exist();
    }
}

fn position_on_screen(ui: &mut ComposeTestRule, number: i32) -> Offset {
    on_item_with_number(ui, number).fetch_semantics_node().position_on_screen()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenDefault_expectedItemsAppear
#[test]
fn whenDefault_expectedItemsAppear() {
    let (mut ui, scope) = run_default();
    scope.show(&mut ui, test_data_provider(&[1, 2, 3]));
    assert_context_menu_exists_with_numbers(&mut ui, &[1, 2, 3]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenCoroutineCancelled_popupDisappears
#[test]
fn whenCoroutineCancelled_popupDisappears() {
    let (mut ui, scope) = run_default();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
    job.job.cancel();
    ui.wait_for_idle();
    assert_context_menu_does_not_exist(&mut ui);
    assert_items_do_not_exist(&mut ui, &[1]);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenCallingTwice_popupIsReplaced
#[test]
fn whenCallingTwice_popupIsReplaced() {
    let (mut ui, scope) = run_default();
    let first = scope.show(&mut ui, test_data_provider(&[1]));
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    assert_items_do_not_exist(&mut ui, &[2]);
    first.assert_active();
    let second = scope.show(&mut ui, test_data_provider(&[2]));
    assert_items_do_not_exist(&mut ui, &[1]);
    assert_context_menu_exists_with_numbers(&mut ui, &[2]);
    first.assert_completed();
    second.assert_active();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenClickOffPopup_popupDisappearsAndCoroutineEnds
/// Compose clicks the root's top left through `UiDevice`, above its test framework; compose-rs's
/// host takes the press as the platform's.
#[test]
fn whenClickOffPopup_popupDisappearsAndCoroutineEnds() {
    let (mut ui, scope) = run_default();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
    ui.host().send_pointer(PointerEvent::press(Offset::ZERO));
    ui.host().send_pointer(PointerEvent::release(Offset::ZERO));
    ui.wait_for_idle();
    assert_context_menu_does_not_exist(&mut ui);
    assert_items_do_not_exist(&mut ui, &[1]);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenClickItemThatClosesPopup_popupDisappearsAndCoroutineEnds
#[test]
fn whenClickItemThatClosesPopup_popupDisappearsAndCoroutineEnds() {
    let (mut ui, scope) = run_default();
    let data = data_provider_with(default_positioner, || {
        TextContextMenuData::new(vec![test_item(1, |session| session.close())])
    });
    let job = scope.show(&mut ui, data);
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
    on_item_with_number(&mut ui, 1).perform_click();
    ui.wait_for_idle();
    assert_context_menu_does_not_exist(&mut ui);
    assert_items_do_not_exist(&mut ui, &[1]);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenClickItemThatDoesNothing_popupRemains
#[test]
fn whenClickItemThatDoesNothing_popupRemains() {
    let (mut ui, scope) = run_default();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
    on_item_with_number(&mut ui, 1).perform_click();
    ui.wait_for_idle();
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenOuterContentRemoved_popupDisappearsAndCoroutineEnds
#[test]
fn whenOuterContentRemoved_popupDisappearsAndCoroutineEnds() {
    let show_outer = leak(mutable_state_of(true));
    let outer: Outer = Rc::new(move |content| {
        if show_outer.get() {
            content();
        }
    });
    let (mut ui, scope) = run_test(outer, default_inner());
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
    show_outer.set(false);
    ui.wait_for_idle();
    assert_context_menu_does_not_exist(&mut ui);
    assert_items_do_not_exist(&mut ui, &[1]);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenInnerContentRemoved_popupRemains
#[test]
fn whenInnerContentRemoved_popupRemains() {
    let show_inner = leak(mutable_state_of(true));
    let inner: Rc<dyn Fn()> = Rc::new(move || {
        if show_inner.get() {
            Box!(modifier = Modifier.fill_max_size());
        }
    });
    let (mut ui, scope) = run_test(default_outer(), inner);
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
    show_inner.set(false);
    ui.wait_for_idle();
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    job.assert_active();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenIncomingMinConstraints_contentReceivesMinConstraints
#[test]
fn whenIncomingMinConstraints_contentReceivesMinConstraints() {
    let (smaller, larger) = (50, 100);
    let expected = Constraints::new(smaller, larger, smaller, larger);
    let actual: Rc<Cell<Option<Constraints>>> = Rc::default();
    let into = actual.clone();
    let outer: Outer = Rc::new(|content| {
        Box!(
            propagate_min_constraints = true,
            modifier = Modifier.background(Color::LIGHT_GRAY).size_in(SizeSpec::bounds(
                Dp(50.0),
                Dp(50.0),
                Dp(100.0),
                Dp(100.0)
            )),
            { content() }
        );
    });
    let inner: Rc<dyn Fn()> = Rc::new(move || {
        let into = into.clone();
        Box!(
            modifier = Modifier
                .layout(move |scope, measurable, constraints| {
                    into.set(Some(constraints));
                    let placeable = scope.measure(measurable, constraints);
                    scope.layout_with(placeable.width, placeable.height, move |placement| {
                        placement.place(&placeable, 0, 0);
                    })
                })
                .test_tag("inner")
        );
    });
    let (mut ui, _scope) = run_test(outer, inner);
    assert_eq!(actual.get(), Some(expected));
    ui.on_node_with_tag("inner").assert_is_displayed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenDataProviderDataUpdates_itemsUpdate
#[test]
fn whenDataProviderDataUpdates_itemsUpdate() {
    let number = leak(mutable_state_of(1));
    let (mut ui, scope) = run_default();
    let data = data_provider_with(default_positioner, move || numbers_to_data(&[number.get()]));
    let job = scope.show(&mut ui, data);
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    assert_items_do_not_exist(&mut ui, &[2]);
    job.assert_active();
    let initial = position_on_screen(&mut ui, 1);
    number.set(2);
    ui.wait_for_idle();
    assert_items_do_not_exist(&mut ui, &[1]);
    assert_context_menu_exists_with_numbers(&mut ui, &[2]);
    job.assert_active();
    assert_eq!(position_on_screen(&mut ui, 2), initial);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/DefaultTextContextMenuDropdownProviderTest.kt, whenDataProviderPositionUpdates_andPopupContentSizeChanges_popupDoesMove
#[test]
fn whenDataProviderPositionUpdates_andPopupContentSizeChanges_popupDoesMove() {
    let offset = leak(mutable_state_of(Offset::ZERO));
    let numbers = leak(mutable_state_of(1));
    let (mut ui, scope) = run_default();
    let data = data_provider_with(
        move |_| offset.get(),
        move || numbers_to_data(&(1..=numbers.get()).collect::<Vec<_>>()),
    );
    let job = scope.show(&mut ui, data);
    assert_context_menu_exists_with_numbers(&mut ui, &[1]);
    assert_items_do_not_exist(&mut ui, &[2]);
    job.assert_active();
    let initial = position_on_screen(&mut ui, 1);
    offset.set(Offset::new(10.0, 10.0));
    numbers.set(2);
    ui.wait_for_idle();
    assert_context_menu_exists_with_numbers(&mut ui, &[1, 2]);
    job.assert_active();
    let delta = position_on_screen(&mut ui, 1) - initial;
    assert_eq!(
        IntOffset::new(delta.x.round() as i32, delta.y.round() as i32),
        IntOffset::new(10, 10)
    );
}
