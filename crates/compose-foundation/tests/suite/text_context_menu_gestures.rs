//! A right click showing the dropdown text context menu, and nothing else doing it, as Compose's
//! internal `Modifier.showTextContextMenuOnSecondaryClick` (D2041), from Compose's
//! `TextContextMenuGesturesModifierTest`.
//!
//! As Compose's: the box 100 px square at the rule's density of 1, the provided dropdown a fake
//! keeping the data provider it's given, the destination the box's coordinates (Compose's
//! `onGloballyPositioned` after the gesture's modifier).

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::modifier::show_text_context_menu_on_secondary_click;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_runtime::MutableState;
use compose_test::*;
use compose_ui::{Color, IntOffset, IntSize, LayoutCoordinates, Offset, UnitExt};

use crate::text_context_menu_support::FakeTextContextMenuProvider;

const TAG: &str = "testTag";
const BASE_LENGTH: i32 = 100;

/// Compose's `TestItem`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TestItem {
    One,
    Two,
}

impl TestItem {
    fn label(self) -> &'static str {
        match self {
            TestItem::One => "One",
            TestItem::Two => "Two",
        }
    }
}

/// Compose's `testItem`.
fn test_item(scope: &mut TextContextMenuBuilderScope, item: TestItem) {
    scope.item(item, item.label(), None, |_| {});
}

/// Compose's `TestScope`: the box, the data provider the fake dropdown was given, the
/// destination's coordinates, and how many times `onPreShowContextMenu` ran.
struct Scope {
    ui: ComposeTestRule,
    data_provider: Rc<RefCell<Option<Rc<dyn TextContextMenuDataProvider>>>>,
    destination: Rc<Cell<Option<LayoutCoordinates>>>,
    pre_shows: Rc<Cell<u32>>,
}

/// Compose's `runTest`: `Box(pre.showTextContextMenuOnSecondaryClick { count++ }.then(post)
/// .onGloballyPositioned { destination = it }.testTag(tag))` under the fake dropdown provider.
fn run_test(pre: impl Fn() -> Modifier + 'static, post: impl Fn() -> Modifier + 'static) -> Scope {
    let data_provider: Rc<RefCell<Option<Rc<dyn TextContextMenuDataProvider>>>> = Rc::default();
    let destination: Rc<Cell<Option<LayoutCoordinates>>> = Rc::default();
    let pre_shows = Rc::new(Cell::new(0));
    let (kept, at, count) = (data_provider.clone(), destination.clone(), pre_shows.clone());
    let fake: Rc<dyn TextContextMenuProvider> =
        Rc::new(FakeTextContextMenuProvider::new(move |provider| {
            *kept.borrow_mut() = Some(provider);
            Box::pin(async {})
        }));
    let on_pre_show: Rc<dyn Fn(Offset)> = Rc::new(move |_| count.set(count.get() + 1));
    let ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (fake, on_pre_show, at) = (fake.clone(), on_pre_show.clone(), at.clone());
        let modifier = show_text_context_menu_on_secondary_click(pre(), Some(on_pre_show))
            .then(post())
            .on_globally_positioned(move |coordinates| at.set(Some(coordinates)))
            .test_tag(TAG);
        compose_runtime::CompositionLocalProvider(
            LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.provides(Some(fake)),
            move || {
                Box!(modifier = modifier.clone());
            },
        );
    });
    Scope { ui, data_provider, destination, pre_shows }
}

/// The light gray box of the base length, as the tests' `postGestureModifier`.
fn base_box() -> Modifier {
    Modifier.background(Color::LIGHT_GRAY).size(BASE_LENGTH.dp())
}

impl Scope {
    fn provider(&self) -> Rc<dyn TextContextMenuDataProvider> {
        self.data_provider.borrow().clone().expect("the dropdown wasn't asked to show")
    }

    fn assert_null_data_provider(&self) {
        assert!(self.data_provider.borrow().is_none());
    }

    #[track_caller]
    fn assert_click_position_equals(&self, expected: IntOffset) {
        let destination = self.destination.get().expect("the destination isn't placed");
        assert_eq!(self.provider().position(&destination).round(), expected);
    }

    #[track_caller]
    fn assert_data_contains(&self, items: &[TestItem]) {
        let components = self.provider().data().components;
        assert_eq!(components.len(), items.len());
        for (expected, actual) in items.iter().zip(&components) {
            assert_eq!(actual.key(), TextContextMenuKey::new(*expected));
        }
    }

    #[track_caller]
    fn assert_pre_show_context_menu_invocation_count(&self, n: u32) {
        assert_eq!(self.pre_shows.get(), n);
    }

    /// A right click at `at` in the box, its center by default.
    fn right_click(&mut self, at: Option<Offset>) {
        self.ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
            let position = at.unwrap_or_else(|| mouse.center());
            mouse.right_click(position);
        });
        self.ui.wait_for_idle();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuGesturesModifierTest.kt, whenTouchTap_nothingHappens
#[test]
fn whenTouchTap_nothingHappens() {
    let mut it = run_test(|| Modifier, base_box);
    it.ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click());
    it.assert_null_data_provider();
    it.assert_pre_show_context_menu_invocation_count(0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuGesturesModifierTest.kt, whenLeftClick_nothingHappens
#[test]
fn whenLeftClick_nothingHappens() {
    let mut it = run_test(|| Modifier, base_box);
    it.ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.click());
    it.assert_null_data_provider();
    it.assert_pre_show_context_menu_invocation_count(0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuGesturesModifierTest.kt, whenRightClick_dataProviderTriggers
#[test]
fn whenRightClick_dataProviderTriggers() {
    let mut it = run_test(
        || Modifier.append_text_context_menu_components(|scope| test_item(scope, TestItem::One)),
        base_box,
    );
    let click_offset = IntOffset::new(10, 10);
    it.right_click(Some(Offset::new(10.0, 10.0)));
    it.assert_data_contains(&[TestItem::One]);
    it.assert_click_position_equals(click_offset);
    it.assert_pre_show_context_menu_invocation_count(1);
}

/// A state kept for the test's length (its owner leaked).
fn offset_state() -> MutableState<IntOffset> {
    Box::leak(Box::new(compose_runtime::mutable_state_of(IntOffset::ZERO))).handle()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuGesturesModifierTest.kt, whenDestinationCoordinatesMove_dataProviderPositionsCorrectly
#[test]
fn whenDestinationCoordinatesMove_dataProviderPositionsCorrectly() {
    let final_offset = IntOffset::new(BASE_LENGTH, BASE_LENGTH);
    let offset = offset_state();
    let mut it = run_test(
        || Modifier.size((BASE_LENGTH * 2).dp()),
        move || Modifier.offset_with(move |_| offset.get()).then(base_box()),
    );
    let click_offset = IntOffset::new(10, 10);

    it.right_click(Some(Offset::new(10.0, 10.0)));
    it.assert_click_position_equals(click_offset);
    it.assert_pre_show_context_menu_invocation_count(1);

    offset.set(final_offset);
    it.ui.wait_for_idle();
    // The destinationCoordinates move,
    // but the localCoordinates at the gesture location do not,
    // so the result in the destination coordinates should be subtracting any movement.
    it.assert_click_position_equals(click_offset - final_offset);
    it.assert_pre_show_context_menu_invocation_count(1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuGesturesModifierTest.kt, whenGestureCoordinatesMove_dataProviderPositionsCorrectly
#[test]
fn whenGestureCoordinatesMove_dataProviderPositionsCorrectly() {
    let final_offset = IntOffset::new(BASE_LENGTH, BASE_LENGTH);
    let offset = offset_state();
    let mut it = run_test(
        move || Modifier.size((BASE_LENGTH * 2).dp()).offset_with(move |_| offset.get()),
        base_box,
    );
    let click_offset = IntOffset::new(10, 10);

    it.right_click(Some(Offset::new(10.0, 10.0)));
    it.assert_click_position_equals(click_offset);
    it.assert_pre_show_context_menu_invocation_count(1);

    offset.set(final_offset);
    it.ui.wait_for_idle();
    // Both layouts move the same amount, so the result should be the same
    it.assert_click_position_equals(click_offset);
    it.assert_pre_show_context_menu_invocation_count(1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuGesturesModifierTest.kt, whenDataProviderDataUpdates_dataResultUpdates
#[test]
fn whenDataProviderDataUpdates_dataResultUpdates() {
    let item = Box::leak(Box::new(compose_runtime::mutable_state_of(TestItem::One))).handle();
    let mut it = run_test(
        move || {
            Modifier.append_text_context_menu_components(move |scope| test_item(scope, item.get()))
        },
        base_box,
    );
    it.right_click(None);
    it.assert_data_contains(&[TestItem::One]);
    it.assert_pre_show_context_menu_invocation_count(1);

    item.set(TestItem::Two);
    it.ui.wait_for_idle();
    it.assert_data_contains(&[TestItem::Two]);
    it.assert_pre_show_context_menu_invocation_count(1);
}
