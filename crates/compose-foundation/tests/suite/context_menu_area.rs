//! The dropdown context menu opened by its state or a right click (D2036), from Compose's
//! `ContextMenuAreaTest` and `ContextMenuGestureTest`.
//!
//! The rule's root is 400 by 400 at density 1.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::context_menu::*;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::mutable_state_of;
use compose_test::*;

use compose_ui::{
    Color, IntSize, LayoutCoordinates, Offset, PointerEvent, PointerEventPass, UnitExt,
};

const TAG: &str = "testTag";
const ITEM_TAG: &str = "itemTag";
const CONTENT_TAG: &str = "contentTag";

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(400, 400), content)
}

fn closed() -> Rc<ContextMenuState> {
    Rc::new(ContextMenuState::new(ContextMenuStatus::Closed))
}

fn open_at(offset: Offset) -> Rc<ContextMenuState> {
    Rc::new(ContextMenuState::new(ContextMenuStatus::open(offset)))
}

/// Compose's `ContextMenuState.open()`.
fn open(state: &ContextMenuState) {
    state.set_status(ContextMenuStatus::open(Offset::ZERO));
}

fn is_open(state: &ContextMenuState) -> bool {
    matches!(state.status(), ContextMenuStatus::Open(_))
}

/// Compose's `testItem(modifier, onClick)`.
fn item(modifier: Modifier, on_click: impl Fn() + 'static) -> ContextMenuBuilderBlock {
    let on_click = Rc::new(on_click);
    Rc::new(move |scope: &mut ContextMenuScope| {
        let on_click = on_click.clone();
        scope.item("Item", modifier.clone(), true, None, move || on_click());
    })
}

/// Compose's `TestMenu`.
fn test_menu(state: Rc<ContextMenuState>, modifier: Modifier, block: ContextMenuBuilderBlock) {
    ContextMenu(state, compose_runtime::callback(|| {}), modifier.test_tag(TAG), block);
}

/// Compose's `TestMenu` dismissed by `on_dismiss`.
fn test_menu_dismissed(state: Rc<ContextMenuState>, on_dismiss: impl Fn() + 'static) {
    let block = item(Modifier, || {});
    ContextMenu(state, compose_runtime::callback(on_dismiss), Modifier.test_tag(TAG), block);
}

/// Compose's `TestArea`.
fn test_area(
    modifier: Modifier,
    state: Rc<ContextMenuState>,
    block: ContextMenuBuilderBlock,
    enabled: bool,
    content: impl Fn() + 'static,
) {
    ContextMenuArea(
        state,
        compose_runtime::callback(|| {}),
        block,
        modifier.test_tag(TAG),
        enabled,
        None::<compose_runtime::Callback<dyn Fn()>>,
        compose_runtime::callback(content),
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenu_defaultStatus_noPopup
#[test]
fn whenContextMenu_defaultStatus_noPopup() {
    let mut ui = rule(|| test_menu(closed(), Modifier, item(Modifier, || {})));
    ui.on_node_with_tag(TAG).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenu_toggleStatus_popupAppearsAndDisappears
#[test]
fn whenContextMenu_toggleStatus_popupAppearsAndDisappears() {
    let state = closed();
    let shown = state.clone();
    let mut ui = rule(move || test_menu(shown.clone(), Modifier, item(Modifier, || {})));
    ui.on_node_with_tag(TAG).assert_does_not_exist();
    for _ in 0..2 {
        open(&state);
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_is_displayed();
        state.close();
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_does_not_exist();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenu_statusOpen_popupAdheresToStatusOffset
#[test]
fn whenContextMenu_statusOpen_popupAdheresToStatusOffset() {
    let popup_offset = Offset::new(10.0, 10.0);
    let state = open_at(popup_offset);
    let content: Rc<Cell<Option<LayoutCoordinates>>> = Rc::default();
    let menu: Rc<Cell<Option<LayoutCoordinates>>> = Rc::default();
    let (content_into, menu_into) = (content.clone(), menu.clone());
    let mut ui = rule(move || {
        let (state, content_into, menu_into) =
            (state.clone(), content_into.clone(), menu_into.clone());
        // Away from the screen's edge.
        Box!(modifier = Modifier.fill_max_size().padding(32.dp()), {
            let (state, content_into, menu_into) =
                (state.clone(), content_into.clone(), menu_into.clone());
            Box!(
                modifier = Modifier
                    .size(100.dp())
                    .test_tag("content")
                    .on_globally_positioned(move |it| content_into.set(Some(it))),
                {
                    let menu_into = menu_into.clone();
                    test_menu(
                        state.clone(),
                        Modifier
                            .test_tag("menu")
                            .on_globally_positioned(move |it| menu_into.set(Some(it))),
                        item(Modifier, || {}),
                    );
                }
            );
        });
    });
    ui.on_node_with_tag("content").assert_is_displayed();
    ui.on_node_with_tag("menu").assert_is_displayed();
    let content = content.get().expect("the content placed").position_on_screen();
    let menu = menu.get().expect("the menu placed").position_on_screen();
    let offset = menu - content;
    assert!(
        (offset.x - popup_offset.x).abs() < 0.5 && (offset.y - popup_offset.y).abs() < 0.5,
        "{offset:?} isn't {popup_offset:?}"
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenu_clickOnItemThatClosesState_popupCloses
#[test]
fn whenContextMenu_clickOnItemThatClosesState_popupCloses() {
    let state = open_at(Offset::ZERO);
    let shown = state.clone();
    let mut ui = rule(move || {
        let closing = shown.clone();
        test_menu(
            shown.clone(),
            Modifier,
            item(Modifier.test_tag(ITEM_TAG), move || closing.close()),
        );
    });
    assert!(is_open(&state));
    ui.on_node_with_tag(ITEM_TAG).assert_is_displayed();
    ui.on_node_with_tag(ITEM_TAG).perform_click();
    ui.wait_for_idle();
    assert!(!is_open(&state));
    ui.on_node_with_tag(ITEM_TAG).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenu_clickOffPopup_closesPopup
/// Compose clicks through `UiDevice`, above its test framework, so the popup's window sees it;
/// compose-rs's host takes the press as the platform's.
#[test]
fn whenContextMenu_clickOffPopup_closesPopup() {
    let state = open_at(Offset::ZERO);
    let shown = state.clone();
    let mut ui = rule(move || {
        let shown = shown.clone();
        Box!(modifier = Modifier.fill_max_size(), {
            let closing = shown.clone();
            test_menu_dismissed(shown.clone(), move || closing.close());
        });
    });
    assert!(is_open(&state));
    ui.on_node_with_tag(TAG).assert_is_displayed();
    // The root's bottom right, less a pixel.
    let at = Offset::new(399.0, 399.0);
    ui.host().send_pointer(PointerEvent::press(at));
    ui.host().send_pointer(PointerEvent::release(at));
    ui.wait_for_idle();
    assert!(!is_open(&state));
    ui.on_node_with_tag(TAG).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenuArea_toggleOpenClose_displayedAsExpected
#[test]
fn whenContextMenuArea_toggleOpenClose_displayedAsExpected() {
    let state = closed();
    let shown = state.clone();
    let mut ui = rule(move || {
        let shown = shown.clone();
        Box!(modifier = Modifier.fill_max_size().wrap_content_size(), {
            test_area(
                Modifier,
                shown.clone(),
                item(Modifier.test_tag(ITEM_TAG), || {}),
                true,
                || {
                    Spacer!(
                        modifier = Modifier
                            .background(Color::LIGHT_GRAY)
                            .size(100.dp())
                            .test_tag(CONTENT_TAG)
                    );
                },
            );
        });
    });
    for _ in 0..2 {
        state.close();
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_is_displayed();
        ui.on_node_with_tag(CONTENT_TAG).assert_is_displayed();
        ui.on_node(is_popup()).assert_does_not_exist();
        ui.on_node_with_tag(ITEM_TAG).assert_does_not_exist();
        open(&state);
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_is_displayed();
        ui.on_node_with_tag(CONTENT_TAG).assert_is_displayed();
        ui.on_node(is_popup()).assert_is_displayed();
        ui.on_node_with_tag(ITEM_TAG).assert_is_displayed();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenuArea_minConstraintsPropagated
#[test]
fn whenContextMenuArea_minConstraintsPropagated() {
    let mut ui = rule(|| {
        test_area(Modifier.size(100.dp()), closed(), item(Modifier, || {}), true, || {
            Box!(modifier = Modifier.background(Color::LIGHT_GRAY).test_tag(CONTENT_TAG));
        });
    });
    ui.on_node_with_tag(CONTENT_TAG)
        .assert_width_is_equal_to(100.dp())
        .assert_height_is_equal_to(100.dp());
}

fn gray_area(state: Rc<ContextMenuState>, enabled: bool) -> ComposeTestRule {
    rule(move || {
        let closing = state.clone();
        test_area(
            Modifier.background(Color::LIGHT_GRAY).size(100.dp()),
            state.clone(),
            item(Modifier.test_tag(ITEM_TAG), move || closing.close()),
            enabled,
            || {},
        );
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenuArea_leftClick_contextMenuDoesNotAppear
#[test]
fn whenContextMenuArea_leftClick_contextMenuDoesNotAppear() {
    let mut ui = gray_area(closed(), true);
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.click());
    ui.on_node_with_tag(ITEM_TAG).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenuArea_rightClick_contextMenuAppears
#[test]
fn whenContextMenuArea_rightClick_contextMenuAppears() {
    let mut ui = gray_area(closed(), true);
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
    ui.on_node_with_tag(ITEM_TAG).assert_is_displayed();
    ui.on_node_with_tag(ITEM_TAG).perform_click();
    ui.on_node_with_tag(ITEM_TAG).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuAreaTest.kt, whenContextMenuArea_disabled_rightClick_contextMenuDoesNotAppear
#[test]
fn whenContextMenuArea_disabled_rightClick_contextMenuDoesNotAppear() {
    let mut ui = gray_area(closed(), false);
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
    ui.on_node_with_tag(ITEM_TAG).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuGestureTest.kt, whenContextMenuGestures_thenRightClick_isOpenAtOffset
#[test]
fn whenContextMenuGestures_thenRightClick_isOpenAtOffset() {
    let state = closed();
    let gestures = state.clone();
    let touch = Offset::new(10.0, 10.0);
    let mut ui = rule(move || {
        Box!(
            modifier =
                Modifier.test_tag(TAG).size(100.dp()).context_menu_gestures_of(gestures.clone())
        );
    });
    assert!(!is_open(&state));
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.right_click(touch));
    ui.wait_for_idle();
    let ContextMenuStatus::Open(offset) = state.status() else { panic!("not open") };
    assert!((offset.x - touch.x).abs() < 0.5 && (offset.y - touch.y).abs() < 0.5, "{offset:?}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuGestureTest.kt, whenContextMenuGestures_insertedAboveOtherPointerInputLate_correctlyDispatchesEvents
/// Fails if the gestures' `pointer_input` key is `()`, as the other one's.
#[test]
fn whenContextMenuGestures_insertedAboveOtherPointerInputLate_correctlyDispatchesEvents() {
    let state = closed();
    let gestures = state.clone();
    let enabled = **Box::leak(Box::new(mutable_state_of(false)));
    let presses = Rc::new(RefCell::new(0));
    let counted = presses.clone();
    let mut ui = rule(move || {
        let maybe = if enabled.get() {
            Modifier.context_menu_gestures_of(gestures.clone())
        } else {
            Modifier
        };
        let counted = counted.clone();
        Box!(
            modifier =
                Modifier.test_tag(TAG).size(100.dp()).then(maybe).pointer_input((), move |scope| {
                    let counted = counted.clone();
                    async move {
                        // Counts a down without consuming it, at each gesture.
                        loop {
                            let event = scope.await_pointer_event(PointerEventPass::Main).await;
                            if event.changes.iter().all(|change| change.changed_to_down()) {
                                *counted.borrow_mut() += 1;
                            }
                            while scope.any_pressed() {
                                scope.await_pointer_event(PointerEventPass::Final).await;
                            }
                        }
                    }
                })
        );
    });
    assert!(!is_open(&state));
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
    assert!(!is_open(&state));
    assert_eq!(*presses.borrow(), 1);
    enabled.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
    assert!(is_open(&state));
    assert_eq!(*presses.borrow(), 2);
}
