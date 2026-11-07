//! The platform's default providers: each made only where none is provided, under one added
//! layout (D2038), from Compose's `PlatformDefaultTextContextMenuProvidersTest`.
//!
//! The rule's floating toolbars are the platform's (`TestTextToolbar`), so the toolbar's default
//! is the platform toolbar's provider, as Compose's `AndroidTextContextMenuToolbarProvider`.

#![allow(non_snake_case)]

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::default_providers::ProvideDefaultPlatformTextContextMenuProviders;
use compose_foundation::text_context_menu::platform_toolbar::PlatformTextContextMenuToolbarProvider;
use compose_foundation::text_context_menu::provider::BasicTextContextMenuProvider;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_test::*;

use crate::text_context_menu_support::*;

type Outer = Rc<dyn Fn(Rc<dyn Fn()>)>;

const BOX_TAG: &str = "box";

/// What the content read, as Compose's `TestScope`.
struct Scope {
    ui: ComposeTestRule,
    toolbar: Rc<RefCell<Option<Rc<dyn TextContextMenuProvider>>>>,
    dropdown: Rc<RefCell<Option<Rc<dyn TextContextMenuProvider>>>>,
}

fn fake_provider() -> Rc<dyn TextContextMenuProvider> {
    Rc::new(FakeTextContextMenuProvider::new(|_| {
        panic!("FakeTextContextMenuProvider.showTextContextMenu called unexpectedly.")
    }))
}

/// Compose's `runTest(outerContent)`.
fn run_test(outer: Outer) -> Scope {
    let toolbar: Rc<RefCell<Option<Rc<dyn TextContextMenuProvider>>>> = Rc::default();
    let dropdown: Rc<RefCell<Option<Rc<dyn TextContextMenuProvider>>>> = Rc::default();
    let (toolbar_read, dropdown_read) = (toolbar.clone(), dropdown.clone());
    let ui = compose_test(move || {
        let (toolbar_read, dropdown_read) = (toolbar_read.clone(), dropdown_read.clone());
        outer(Rc::new(move || {
            let (toolbar_read, dropdown_read) = (toolbar_read.clone(), dropdown_read.clone());
            ProvideDefaultPlatformTextContextMenuProviders!({
                *toolbar_read.borrow_mut() = LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.current();
                *dropdown_read.borrow_mut() = LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.current();
                Box!(modifier = Modifier.test_tag(BOX_TAG));
            });
        }));
    });
    Scope { ui, toolbar, dropdown }
}

fn provider_is<T: Any>(provider: &Option<Rc<dyn TextContextMenuProvider>>) -> bool {
    let provider = provider.as_ref().expect("no provider");
    let provider: &dyn Any = &**provider;
    provider.is::<T>()
}

impl Scope {
    fn assert_toolbar_instance_of<T: Any>(&self) {
        assert!(provider_is::<T>(&self.toolbar.borrow()), "the toolbar isn't the expected type");
    }

    fn assert_dropdown_instance_of<T: Any>(&self) {
        assert!(provider_is::<T>(&self.dropdown.borrow()), "the dropdown isn't the expected type");
    }

    /// The root's layout, the provider's, the box with the test tag.
    fn assert_only_one_added_layout(&mut self) {
        let node = self.ui.on_node_with_tag(BOX_TAG).fetch_semantics_node();
        let mut depth = 0;
        let mut info = Some(node.layout_info());
        while let Some(current) = info {
            info = current.parent_info();
            depth += 1;
        }
        assert_eq!(depth, 3);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/PlatformDefaultTextContextMenuProvidersTest.kt, testProvidersCreated_noneDefined
#[test]
fn testProvidersCreated_noneDefined() {
    let mut scope = run_test(Rc::new(|content| content()));
    scope.assert_toolbar_instance_of::<PlatformTextContextMenuToolbarProvider>();
    scope.assert_dropdown_instance_of::<BasicTextContextMenuProvider>();
    scope.assert_only_one_added_layout();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/PlatformDefaultTextContextMenuProvidersTest.kt, testDropdownProviderCreated_toolbarDefined
#[test]
fn testDropdownProviderCreated_toolbarDefined() {
    let fake = fake_provider();
    let mut scope = run_test(Rc::new(move |content| {
        compose_runtime::CompositionLocalProvider(
            LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.provides(Some(fake.clone())),
            || content(),
        );
    }));
    scope.assert_toolbar_instance_of::<PlatformTextContextMenuToolbarProvider>();
    scope.assert_dropdown_instance_of::<FakeTextContextMenuProvider>();
    scope.assert_only_one_added_layout();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/PlatformDefaultTextContextMenuProvidersTest.kt, testToolbarProviderCreated_dropdownDefined
#[test]
fn testToolbarProviderCreated_dropdownDefined() {
    let fake = fake_provider();
    let mut scope = run_test(Rc::new(move |content| {
        compose_runtime::CompositionLocalProvider(
            LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.provides(Some(fake.clone())),
            || content(),
        );
    }));
    scope.assert_dropdown_instance_of::<BasicTextContextMenuProvider>();
    scope.assert_toolbar_instance_of::<FakeTextContextMenuProvider>();
    scope.assert_only_one_added_layout();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/internal/PlatformDefaultTextContextMenuProvidersTest.kt, testNoProvidersCreated_bothDefined
#[test]
fn testNoProvidersCreated_bothDefined() {
    let fake = fake_provider();
    let mut scope = run_test(Rc::new(move |content| {
        compose_runtime::CompositionLocalProvider(
            (
                LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.provides(Some(fake.clone())),
                LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.provides(Some(fake.clone())),
            ),
            || content(),
        );
    }));
    scope.assert_toolbar_instance_of::<FakeTextContextMenuProvider>();
    scope.assert_dropdown_instance_of::<FakeTextContextMenuProvider>();
    scope.assert_only_one_added_layout();
}
