//! A provider composing its menu where it's provided: shown, closed by its session, its job or
//! its leaving, replaced by the next (D2031), from Compose's `BasicTextContextMenuProviderTest`.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::provider::{
    AnchorCoordinates, ContextMenuBlock, ProvideBasicTextContextMenu, ProviderLocal,
};
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_runtime::{CompositionLocal, CoroutineScope, mutable_state_of};
use compose_test::*;
use compose_ui::{Color, IntOffset, UnitExt};

use crate::text_context_menu_support::*;

static LOCAL_TEST_CONTEXT_MENU_PROVIDER: TextContextMenuProviderLocal =
    CompositionLocal::new("LocalTestContextMenuProvider", || None);

const ANCHOR_LAYOUT_TAG: &str = "AnchorLayout";
const CONTEXT_MENU_TAG: &str = "ContextMenu";

/// What a test reads of its content, as Compose's `TestScope`.
#[derive(Default)]
struct Scope {
    coroutine_scope: RefCell<Option<CoroutineScope>>,
    provider: RefCell<Option<Rc<dyn TextContextMenuProvider>>>,
    session: RefCell<Option<Rc<dyn TextContextMenuSession>>>,
    anchor: RefCell<Option<AnchorCoordinates>>,
}

impl Scope {
    fn show(
        &self,
        ui: &mut ComposeTestRule,
        data: Rc<dyn TextContextMenuDataProvider>,
    ) -> JobProbe {
        let provider = self.provider.borrow().clone().expect("no provider");
        let scope = (*self.coroutine_scope.borrow()).expect("no scope");
        let job = launch(&scope, async move { provider.show_text_context_menu(data).await });
        ui.wait_for_idle();
        job
    }
}

fn outer_box(modifier: Modifier, content: Rc<dyn Fn()>) {
    Box!(modifier = modifier, content_alignment = Alignment::CENTER, { content() });
}

fn inner_box() {
    Box!(
        modifier = Modifier.background(Color::LIGHT_GRAY.with_alpha(0.3)).size_in(
            compose_ui::SizeSpec::bounds(
                200.dp(),
                200.dp(),
                compose_ui::Dp::UNSPECIFIED,
                compose_ui::Dp::UNSPECIFIED
            )
        )
    );
}

/// The menu: a column of the items' labels, each clicked with the session.
fn test_context_menu(
    session: Rc<dyn TextContextMenuSession>,
    data: Rc<dyn TextContextMenuDataProvider>,
) {
    Column!(
        vertical_arrangement = Arrangement::spaced_by(4.dp()),
        modifier =
            Modifier.test_tag(CONTEXT_MENU_TAG).background(Color::LIGHT_GRAY).padding(4.dp()),
        {
            for component in data.data().components {
                if let TextContextMenuComponent::Item(item) = component {
                    let session = session.clone();
                    let label = item.label.clone();
                    BasicText!(
                        label.clone(),
                        modifier =
                            Modifier.test_tag(label).clickable(move || (item.on_click)(&*session)),
                    );
                }
            }
        }
    );
}

fn test_block(scope: Rc<Scope>) -> ContextMenuBlock {
    Rc::new(move |session, data, anchor| {
        *scope.session.borrow_mut() = Some(session.clone());
        *scope.anchor.borrow_mut() = Some(anchor);
        test_context_menu(session, data);
    })
}

/// Content `outer` makes around the provider (or the inner box alone while `enabled` is off),
/// as Compose's `runProviderTest`.
fn run_provider_test(
    outer: impl Fn(Rc<dyn Fn()>) + 'static,
    enabled: compose_runtime::MutableState<bool>,
) -> (ComposeTestRule, Rc<Scope>) {
    let scope = Rc::new(Scope::default());
    let test_scope = scope.clone();
    let block = test_block(scope.clone());
    let ui = compose_test(move || {
        *test_scope.coroutine_scope.borrow_mut() =
            Some(compose_runtime::remember_coroutine_scope());
        let (test_scope, block) = (test_scope.clone(), block.clone());
        outer(Rc::new(move || {
            if enabled.get() {
                let test_scope = test_scope.clone();
                ProvideBasicTextContextMenu!(
                    Modifier.test_tag(ANCHOR_LAYOUT_TAG),
                    ProviderLocal(&LOCAL_TEST_CONTEXT_MENU_PROVIDER),
                    block.clone(),
                    {
                        *test_scope.provider.borrow_mut() =
                            LOCAL_TEST_CONTEXT_MENU_PROVIDER.current();
                        inner_box();
                    }
                );
            } else {
                inner_box();
            }
        }));
    });
    (ui, scope)
}

fn default_outer(content: Rc<dyn Fn()>) {
    outer_box(Modifier.fill_max_size(), content);
}

fn assert_menu_with(ui: &mut ComposeTestRule, numbers: &[i32]) {
    ui.on_node_with_tag(CONTEXT_MENU_TAG).assert_is_displayed();
    for number in numbers {
        ui.on_node_with_tag(&label_for_number(*number)).assert_is_displayed();
    }
}

fn assert_items_absent(ui: &mut ComposeTestRule, numbers: &[i32]) {
    for number in numbers {
        ui.on_node_with_tag(&label_for_number(*number)).assert_does_not_exist();
    }
}

fn assert_no_menu(ui: &mut ComposeTestRule) {
    ui.on_node_with_tag(CONTEXT_MENU_TAG).assert_does_not_exist();
}

fn enabled() -> compose_runtime::MutableState<bool> {
    **Box::leak(Box::new(mutable_state_of(true)))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenDefault_expectedItemsAppear
#[test]
fn the_items_show() {
    let (mut ui, scope) = run_provider_test(default_outer, enabled());
    scope.show(&mut ui, test_data_provider(&[1, 2]));
    assert_menu_with(&mut ui, &[1, 2]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenSessionCloseCalled_contextMenuDisappears
#[test]
fn closing_the_session_closes_the_menu() {
    let (mut ui, scope) = run_provider_test(default_outer, enabled());
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_menu_with(&mut ui, &[1]);
    job.assert_active();
    scope.session.borrow().clone().expect("no session").close();
    ui.wait_for_idle();
    assert_no_menu(&mut ui);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenCoroutineCancelled_contextMenuDisappears
#[test]
fn cancelling_the_job_closes_the_menu() {
    let (mut ui, scope) = run_provider_test(default_outer, enabled());
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_menu_with(&mut ui, &[1]);
    job.assert_active();
    job.job.cancel();
    ui.wait_for_idle();
    assert_no_menu(&mut ui);
    job.assert_cancelled();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenCallingShowTwice_contextMenuIsReplaced
#[test]
fn a_second_menu_replaces_the_first() {
    let (mut ui, scope) = run_provider_test(default_outer, enabled());
    let first = scope.show(&mut ui, test_data_provider(&[1]));
    assert_menu_with(&mut ui, &[1]);
    assert_items_absent(&mut ui, &[2]);
    first.assert_active();
    let second = scope.show(&mut ui, test_data_provider(&[2]));
    assert_items_absent(&mut ui, &[1]);
    assert_menu_with(&mut ui, &[2]);
    first.assert_completed();
    second.assert_active();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenRemovingAnchorLayout_contextMenuIsClosed
#[test]
fn removing_the_anchor_closes_the_menu() {
    let show = enabled();
    let (mut ui, scope) = run_provider_test(
        move |content| {
            outer_box(
                Modifier.fill_max_size(),
                Rc::new(move || {
                    if show.get() {
                        content()
                    }
                }),
            )
        },
        enabled(),
    );
    ui.on_node_with_tag(ANCHOR_LAYOUT_TAG).assert_is_displayed();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    ui.on_node_with_tag(ANCHOR_LAYOUT_TAG).assert_is_displayed();
    assert_menu_with(&mut ui, &[1]);
    job.assert_active();
    show.set(false);
    ui.wait_for_idle();
    ui.on_node_with_tag(ANCHOR_LAYOUT_TAG).assert_does_not_exist();
    assert_no_menu(&mut ui);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenRemovingProvider_contextMenuIsClosed
#[test]
fn removing_the_provider_closes_the_menu() {
    let enabled = enabled();
    let (mut ui, scope) = run_provider_test(default_outer, enabled);
    ui.on_node_with_tag(ANCHOR_LAYOUT_TAG).assert_is_displayed();
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    ui.on_node_with_tag(ANCHOR_LAYOUT_TAG).assert_is_displayed();
    assert_menu_with(&mut ui, &[1]);
    job.assert_active();
    enabled.set(false);
    ui.wait_for_idle();
    ui.on_node_with_tag(ANCHOR_LAYOUT_TAG).assert_does_not_exist();
    assert_no_menu(&mut ui);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenShowingThenInstantlyClosing_coroutineDoesNotHang
#[test]
fn closing_right_after_showing_does_not_hang() {
    let enabled = enabled();
    let (mut ui, scope) = run_provider_test(default_outer, enabled);
    let provider = scope.provider.borrow().clone().unwrap();
    let coroutine_scope = (*scope.coroutine_scope.borrow()).unwrap();
    let data = test_data_provider(&[1]);
    let job = launch(&coroutine_scope, async move { provider.show_text_context_menu(data).await });
    enabled.set(false);
    ui.wait_for_idle();
    ui.on_node_with_tag(ANCHOR_LAYOUT_TAG).assert_does_not_exist();
    assert_no_menu(&mut ui);
    job.assert_completed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenMovingAnchorLayout_contextMenuReceivesUpdate
#[test]
fn a_moved_anchor_is_told() {
    let length = **Box::leak(Box::new(mutable_state_of(0)));
    let (mut ui, scope) = run_provider_test(
        move |content| {
            outer_box(
                Modifier
                    .offset_with(move |_| IntOffset::new(length.get(), length.get()))
                    .size(150.dp()),
                content,
            )
        },
        enabled(),
    );
    let job = scope.show(&mut ui, test_data_provider(&[1]));
    assert_menu_with(&mut ui, &[1]);
    job.assert_active();
    let anchor = scope.anchor.borrow().clone().expect("no anchor");
    let initial = anchor().expect("not placed").bounds_in_root();
    length.set(50);
    ui.wait_for_idle();
    assert_menu_with(&mut ui, &[1]);
    job.assert_active();
    let moved = anchor().expect("not placed").bounds_in_root();
    assert_eq!(moved, initial.translate(compose_ui::Offset::new(50.0, 50.0)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/provider/BasicTextContextMenuProviderTest.kt, whenContextMenuChanges_contextMenuUpdates
#[test]
fn a_new_menu_composable_closes_the_open_menu() {
    fn menu_with_tag(tag: &'static str) -> ContextMenuBlock {
        Rc::new(move |_, _, _| {
            Box!(modifier = Modifier.background(Color::LIGHT_GRAY).size(50.dp()).test_tag(tag));
        })
    }
    let (tag1, tag2) = ("ContextMenu1", "ContextMenu2");
    let menu = **Box::leak(Box::new(compose_runtime::mutable_state_of_with(
        menu_with_tag(tag1),
        compose_runtime::never_equal_policy(),
    )));
    let scope = Rc::new(Scope::default());
    let test_scope = scope.clone();
    let mut ui = compose_test(move || {
        *test_scope.coroutine_scope.borrow_mut() =
            Some(compose_runtime::remember_coroutine_scope());
        let test_scope = test_scope.clone();
        outer_box(
            Modifier.fill_max_size(),
            Rc::new(move || {
                let test_scope = test_scope.clone();
                ProvideBasicTextContextMenu!(
                    Modifier.test_tag(ANCHOR_LAYOUT_TAG),
                    ProviderLocal(&LOCAL_TEST_CONTEXT_MENU_PROVIDER),
                    menu.get(),
                    {
                        *test_scope.provider.borrow_mut() =
                            LOCAL_TEST_CONTEXT_MENU_PROVIDER.current();
                        inner_box();
                    }
                );
            }),
        );
    });
    let job1 = scope.show(&mut ui, test_data_provider(&[1]));
    ui.on_node_with_tag(tag1).assert_is_displayed();
    ui.on_node_with_tag(tag2).assert_does_not_exist();
    job1.assert_active();

    menu.set(menu_with_tag(tag2));
    ui.wait_for_idle();
    ui.on_node_with_tag(tag1).assert_does_not_exist();
    ui.on_node_with_tag(tag2).assert_does_not_exist();
    job1.assert_completed();

    let job2 = scope.show(&mut ui, test_data_provider(&[1]));
    ui.on_node_with_tag(tag1).assert_does_not_exist();
    ui.on_node_with_tag(tag2).assert_is_displayed();
    job2.assert_active();
}

/// An item's click gets the session, which closes the menu.
#[test]
fn an_item_closes_the_menu_through_its_session() {
    let (mut ui, scope) = run_provider_test(default_outer, enabled());
    let data = data_provider_with(default_positioner, || {
        TextContextMenuData::new(vec![test_item(1, |session| session.close())])
    });
    let job = scope.show(&mut ui, data);
    ui.on_node_with_tag(&label_for_number(1)).perform_click();
    ui.wait_for_idle();
    assert_no_menu(&mut ui);
    job.assert_completed_successfully();
}
