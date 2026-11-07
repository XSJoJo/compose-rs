//! The platform's floating toolbar as the menu's provider: its items, its place following the
//! anchor and the data, replaced, closed by its job, an item or its leaving (D2031), from
//! Compose's `AndroidTextContextMenuToolbarProviderTest`, on the test rule's toolbar (Compose's
//! spy on the platform's).

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::platform_toolbar::ProvidePlatformTextContextMenuToolbar;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_runtime::{CoroutineScope, mutable_state_of};
use compose_test::*;
use compose_ui::{Color, Constraints, IntOffset, Offset, UnitExt};

use crate::text_context_menu_support::*;

struct Scope {
    provider: Rc<dyn TextContextMenuProvider>,
    coroutine_scope: CoroutineScope,
    toolbar: Rc<TestTextToolbar>,
}

impl Scope {
    fn show(
        &self,
        ui: &mut ComposeTestRule,
        data: Rc<dyn TextContextMenuDataProvider>,
    ) -> JobProbe {
        let provider = self.provider.clone();
        let job =
            launch(
                &self.coroutine_scope,
                async move { provider.show_text_context_menu(data).await },
            );
        ui.wait_for_idle();
        job
    }

    fn numbers(&self) -> Vec<i32> {
        assert!(self.toolbar.is_shown(), "The toolbar context menu should exist.");
        self.toolbar.titles().iter().map(|title| number_for_label(title)).collect()
    }

    fn assert_with(&self, numbers: &[i32]) {
        let actual = self.numbers();
        for number in numbers {
            assert!(actual.contains(number), "{number} not in {actual:?}");
        }
    }

    fn assert_without(&self, numbers: &[i32]) {
        let actual = self.numbers();
        for number in numbers {
            assert!(!actual.contains(number), "{number} in {actual:?}");
        }
    }

    fn assert_none(&self) {
        assert!(!self.toolbar.is_shown(), "The toolbar context menu should not exist.");
    }

    fn content_position(&self) -> IntOffset {
        let rect = self.toolbar.content_rect().expect("no toolbar");
        IntOffset::new(rect.left.round() as i32, rect.top.round() as i32)
    }

    fn click(&self, number: i32) {
        self.toolbar.click(&label_for_number(number));
    }
}

fn fill() {
    Box!(modifier = Modifier.fill_max_size());
}

/// The provider over `inner`, inside what `outer` makes, as Compose's `runTest`.
fn run_test(
    outer: impl Fn(Rc<dyn Fn()>) + 'static,
    inner: impl Fn() + 'static,
) -> (ComposeTestRule, Scope) {
    let provider: Rc<RefCell<Option<Rc<dyn TextContextMenuProvider>>>> = Rc::default();
    let scope: Rc<RefCell<Option<CoroutineScope>>> = Rc::default();
    let (p, s) = (provider.clone(), scope.clone());
    let inner = Rc::new(inner);
    let ui = compose_test(move || {
        *s.borrow_mut() = Some(compose_runtime::remember_coroutine_scope());
        let (p, inner) = (p.clone(), inner.clone());
        outer(Rc::new(move || {
            let (p, inner) = (p.clone(), inner.clone());
            ProvidePlatformTextContextMenuToolbar!(Modifier, {
                let local = LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.current();
                assert!(local.is_some());
                *p.borrow_mut() = local;
                inner();
            });
        }));
    });
    let scope = Scope {
        provider: provider.borrow().clone().unwrap(),
        coroutine_scope: (*scope.borrow()).unwrap(),
        toolbar: ui.text_toolbar(),
    };
    (ui, scope)
}

fn run_default() -> (ComposeTestRule, Scope) {
    run_test(|content| content(), fill)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenDefault_expectedItemsAppear
#[test]
fn the_items_show() {
    let (mut ui, scope) = run_default();
    scope.show(&mut ui, test_data_provider(&[1, 2, 3]));
    scope.assert_with(&[1, 2, 3]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenCoroutineCancelled_toolbarDisappears
#[test]
fn cancelling_the_job_closes_the_toolbar() {
    let (mut ui, scope) = run_default();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    scope.assert_with(&[1]);
    job.assert_active();
    job.job.cancel();
    ui.wait_for_idle();
    scope.assert_none();
    job.assert_cancelled();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenCallingTwice_toolbarIsReplaced
#[test]
fn a_second_toolbar_replaces_the_first() {
    let (mut ui, scope) = run_default();
    let first = scope.show(&mut ui, test_data_provider(&[1]));
    scope.assert_with(&[1]);
    scope.assert_without(&[2]);
    first.assert_active();
    let second = scope.show(&mut ui, test_data_provider(&[2]));
    scope.assert_without(&[1]);
    scope.assert_with(&[2]);
    first.assert_completed();
    second.assert_active();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenClickItemThatClosesToolbar_toolbarDisappearsAndCoroutineEnds
#[test]
fn an_item_closing_the_toolbar_ends_it() {
    let (mut ui, scope) = run_default();
    let data = data_provider_with(default_positioner, || {
        TextContextMenuData::new(vec![test_item(1, |session| session.close())])
    });
    let job = scope.show(&mut ui, data);
    scope.assert_with(&[1]);
    job.assert_active();
    scope.click(1);
    ui.wait_for_idle();
    scope.assert_none();
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenClickItemThatDoesNothing_toolbarRemains
#[test]
fn an_item_doing_nothing_leaves_the_toolbar() {
    let (mut ui, scope) = run_default();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    scope.assert_with(&[1]);
    scope.click(1);
    ui.wait_for_idle();
    scope.assert_with(&[1]);
    job.assert_active();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenOuterContentRemoved_toolbarDisappearsAndCoroutineEnds
#[test]
fn removing_the_provider_closes_the_toolbar() {
    let show = **Box::leak(Box::new(mutable_state_of(true)));
    let (mut ui, scope) = run_test(
        move |content| {
            if show.get() {
                content()
            }
        },
        fill,
    );
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    scope.assert_with(&[1]);
    job.assert_active();
    show.set(false);
    ui.wait_for_idle();
    scope.assert_none();
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenInnerContentRemoved_toolbarRemains
#[test]
fn removing_the_content_leaves_the_toolbar() {
    let show = **Box::leak(Box::new(mutable_state_of(true)));
    let (mut ui, scope) = run_test(
        |content| content(),
        move || {
            if show.get() {
                fill()
            }
        },
    );
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    scope.assert_with(&[1]);
    show.set(false);
    ui.wait_for_idle();
    scope.assert_with(&[1]);
    job.assert_active();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenIncomingMinConstraints_contentReceivesMinConstraints
#[test]
fn the_content_gets_the_minimum_constraints() {
    let (smaller, larger) = (50, 100);
    let actual = Rc::new(std::cell::Cell::new(None::<Constraints>));
    let a = actual.clone();
    let (mut ui, _scope) = run_test(
        move |content| {
            Box!(
                propagate_min_constraints = true,
                modifier =
                    Modifier.background(Color::LIGHT_GRAY).size_in(compose_ui::SizeSpec::bounds(
                        (smaller as f32).dp(),
                        (smaller as f32).dp(),
                        (larger as f32).dp(),
                        (larger as f32).dp(),
                    )),
                { content() }
            );
        },
        move || {
            let a = a.clone();
            Box!(
                modifier = Modifier
                    .layout(move |scope, measurable, constraints| {
                        a.set(Some(constraints));
                        let placeable = scope.measure(measurable, constraints);
                        scope.layout_with(placeable.width, placeable.height, move |placement| {
                            placement.place(&placeable, 0, 0);
                        })
                    })
                    .test_tag("inner")
            );
        },
    );
    assert_eq!(actual.get(), Some(Constraints::new(smaller, larger, smaller, larger)));
    ui.on_node_with_tag("inner").assert_is_displayed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenAnchorLayoutMoves_toolbarMoves
#[test]
fn the_toolbar_follows_its_anchor() {
    let length = **Box::leak(Box::new(mutable_state_of(0)));
    let (mut ui, scope) = run_test(
        move |content| {
            Box!(modifier = Modifier.fill_max_size(), {
                let content = content.clone();
                Box!(
                    modifier = Modifier
                        .offset_with(move |_| IntOffset::new(length.get(), length.get()))
                        .background(Color::LIGHT_GRAY)
                        .size(150.dp()),
                    { content() }
                );
            });
        },
        fill,
    );
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    scope.assert_with(&[1]);
    let initial = scope.content_position();
    length.set(50);
    ui.wait_for_idle();
    scope.assert_with(&[1]);
    job.assert_active();
    assert_eq!(scope.content_position() - initial, IntOffset::new(50, 50));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenAnchorLayoutScrolled_toolbarMoves
#[test]
fn the_toolbar_follows_its_anchor_scrolled() {
    let scroll = **Box::leak(Box::new(ScrollState::new(0)));
    let base = 100.0;
    let (mut ui, scope) = run_test(
        move |content| {
            Box!(modifier = Modifier.fill_max_size(), {
                let content = content.clone();
                Box!(
                    modifier = Modifier
                        .background(Color::LIGHT_GRAY)
                        .size(((base * 2.0).dp(), (base * 2.0).dp()))
                        .vertical_scroll(scroll)
                        .size(((base * 2.0).dp(), (base * 3.0).dp())),
                    { content() }
                );
            });
        },
        fill,
    );
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    scope.assert_with(&[1]);
    let initial = scope.content_position();
    let coroutine_scope = scope.coroutine_scope;
    coroutine_scope.launch(async move {
        let _ = scroll.scroll_to(scroll.max_value()).await;
    });
    ui.wait_for_idle();
    scope.assert_with(&[1]);
    job.assert_active();
    assert_eq!(scope.content_position() - initial, IntOffset::new(0, -100));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenDataProviderPositionUpdates_toolbarMoves
#[test]
fn the_toolbar_follows_its_position() {
    let offset = **Box::leak(Box::new(mutable_state_of(Offset::ZERO)));
    let (mut ui, scope) = run_default();
    let data = data_provider_with(move |_| offset.get(), || numbers_to_data(&[1]));
    let job = scope.show(&mut ui, data);
    scope.assert_with(&[1]);
    let initial = scope.content_position();
    offset.set(Offset::new(10.0, 10.0));
    ui.wait_for_idle();
    scope.assert_with(&[1]);
    job.assert_active();
    assert_eq!(scope.content_position() - initial, IntOffset::new(10, 10));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenDataProviderDataUpdates_positionDoesNotMove
#[test]
fn new_items_leave_the_toolbar_in_place() {
    let number = **Box::leak(Box::new(mutable_state_of(1)));
    let (mut ui, scope) = run_default();
    let data = data_provider_with(default_positioner, move || numbers_to_data(&[number.get()]));
    let job = scope.show(&mut ui, data);
    scope.assert_with(&[1]);
    scope.assert_without(&[2, 3]);
    let initial = scope.content_position();
    number.set(2);
    ui.wait_for_idle();
    scope.assert_without(&[1, 3]);
    scope.assert_with(&[2]);
    job.assert_active();
    assert_eq!(scope.content_position(), initial);
    number.set(3);
    ui.wait_for_idle();
    scope.assert_without(&[1, 2]);
    scope.assert_with(&[3]);
    job.assert_active();
    assert_eq!(scope.content_position(), initial);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/AndroidTextContextMenuToolbarProviderTest.kt, whenDataProviderDataUpdates_itemsUpdate
#[test]
fn the_toolbar_follows_its_items() {
    let number = **Box::leak(Box::new(mutable_state_of(1)));
    let (mut ui, scope) = run_default();
    let data = data_provider_with(default_positioner, move || numbers_to_data(&[number.get()]));
    let job = scope.show(&mut ui, data);
    scope.assert_with(&[1]);
    scope.assert_without(&[2, 3]);
    number.set(2);
    ui.wait_for_idle();
    scope.assert_without(&[1, 3]);
    scope.assert_with(&[2]);
    job.assert_active();
    number.set(3);
    ui.wait_for_idle();
    scope.assert_without(&[1, 2]);
    scope.assert_with(&[3]);
    job.assert_active();
}

/// The platform closing the toolbar (a touch outside it) ends the session.
#[test]
fn the_platform_closing_the_toolbar_ends_it() {
    let (mut ui, scope) = run_default();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    scope.toolbar.dismiss();
    ui.wait_for_idle();
    scope.assert_none();
    job.assert_completed_successfully();
}

/// A separator starts a new group of items; Compose's own keys take Android's ids.
#[test]
fn separators_make_groups_and_compose_s_items_take_android_s_ids() {
    let (mut ui, scope) = run_default();
    let data = data_provider_with(default_positioner, || {
        let mut builder = TextContextMenuBuilderScope::new();
        builder.item(TextContextMenuKeys::Copy, "Copy", None, |_| {});
        builder.separator();
        builder.item(7, "Seven", None, |_| {});
        builder.build()
    });
    scope.show(&mut ui, data);
    let items = scope.toolbar.items();
    let shape: Vec<_> = items.iter().map(|item| (item.group, item.id, item.order)).collect();
    assert_eq!(shape, [(1, compose_ui::text_action_ids::COPY, 1), (2, 2, 2)]);
}

/// The test toolbar is a view's floating toolbar: one at a time, a new one destroying the
/// shown one; one with nothing in its menu doesn't start.
#[test]
fn the_test_toolbar_shows_one_toolbar_at_a_time() {
    use compose_ui::{Rect, TextActionMenu, TextActionModeCallback, TextActionModes};
    use std::cell::Cell;
    struct Callback(Rc<Cell<bool>>, bool);
    impl TextActionModeCallback for Callback {
        fn on_get_content_rect(&self) -> Rect {
            Rect::ZERO
        }
        fn on_create_action_mode(&self, menu: &mut TextActionMenu) -> bool {
            if self.1 {
                menu.add(0, 1, 1, "Item", Rc::new(|| {}));
            }
            !menu.is_empty()
        }
        fn on_prepare_action_mode(&self, _: &mut TextActionMenu) -> bool {
            false
        }
        fn on_destroy_action_mode(&self) {
            self.0.set(true);
        }
    }
    let toolbar = TestTextToolbar::new();
    let first = Rc::new(Cell::new(false));
    assert!(toolbar.start(Rc::new(Callback(first.clone(), true))).is_some());
    assert!(toolbar.start(Rc::new(Callback(Rc::default(), false))).is_none());
    assert!(first.get(), "the first toolbar wasn't destroyed");
    assert!(!toolbar.is_shown());
}

/// `LOCAL_TEXT_TOOLBAR` shows the actions given in the platform's toolbar, in Compose's order
/// with Android's ids, around the rect given; shown again, it follows; hidden, it's gone, as
/// Compose's `AndroidTextToolbar`.
#[test]
fn the_text_toolbar_shows_the_actions_given() {
    use compose_ui::{
        LOCAL_TEXT_TOOLBAR, Rect, TextToolbar, TextToolbarStatus, text_action_ids as ids,
    };
    use std::cell::Cell;
    let kept: Rc<RefCell<Option<Rc<dyn TextToolbar>>>> = Rc::default();
    let k = kept.clone();
    let ui = compose_test(move || *k.borrow_mut() = Some(LOCAL_TEXT_TOOLBAR.current()));
    let text_toolbar = kept.borrow().clone().unwrap();
    let toolbar = ui.text_toolbar();
    assert_eq!(text_toolbar.status(), TextToolbarStatus::Hidden);
    let copied = Rc::new(Cell::new(0));
    let c = copied.clone();
    let rect = Rect::new(1.0, 2.0, 3.0, 4.0);
    let action = |f: Rc<dyn Fn()>| Some(f);
    text_toolbar.show_menu(
        rect,
        action(Rc::new(move || c.set(c.get() + 1))),
        None,
        action(Rc::new(|| {})),
        None,
        None,
    );
    assert_eq!(text_toolbar.status(), TextToolbarStatus::Shown);
    assert_eq!(toolbar.titles(), ["Copy", "Cut"]);
    let ids: Vec<_> = toolbar.items().iter().map(|item| item.id).collect();
    assert_eq!(ids, [ids::COPY, ids::CUT]);
    assert_eq!(toolbar.content_rect(), Some(rect));
    toolbar.click("Copy");
    assert_eq!(copied.get(), 1);
    let moved = Rect::new(5.0, 6.0, 7.0, 8.0);
    let all = |_: ()| Some(Rc::new(|| {}) as Rc<dyn Fn()>);
    text_toolbar.show_menu(moved, all(()), all(()), all(()), all(()), all(()));
    assert_eq!(toolbar.titles(), ["Copy", "Paste", "Cut", "Select all", "Autofill"]);
    assert_eq!(toolbar.content_rect(), Some(moved));
    assert_eq!(toolbar.started_count(), 1);
    text_toolbar.hide();
    assert!(!toolbar.is_shown());
    assert_eq!(text_toolbar.status(), TextToolbarStatus::Hidden);
    // Closed by the platform, it's hidden too.
    text_toolbar.show_menu(rect, all(()), None, None, None, None);
    toolbar.dismiss();
    assert_eq!(text_toolbar.status(), TextToolbarStatus::Hidden);
}
