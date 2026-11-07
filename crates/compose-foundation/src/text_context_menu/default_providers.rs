//! The platform's default providers where none is provided, as Compose's
//! `ProvideDefaultPlatformTextContextMenuProviders` (D2038). compose-rs's fields and containers
//! don't compose it: they show the same defaults without a composition until a menu opens
//! (D2031); it's here for its tests and for content wanting Compose's shape.

use std::rc::Rc;

use compose_runtime::composable;
use compose_ui::{LayoutCoordinates, Modifier};

use super::dropdown::{ProvideDefaultTextContextMenuDropdown, default_text_context_menu_dropdown};
use super::platform_toolbar::{
    ProvidePlatformTextContextMenuToolbar, platform_text_context_menu_toolbar_provider,
};
use super::provider::{
    AnchorCoordinates, LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER,
    LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER, TextContextMenuProvider,
};

/// Provides the default dropdown and the platform's toolbar to `content` where none is provided,
/// in one box (`propagate_min_constraints`) their menus are anchored to, as Compose's internal
/// `ProvideDefaultPlatformTextContextMenuProviders(modifier, content)`: both provided, the box
/// only.
#[doc(hidden)]
#[composable]
pub fn ProvideDefaultPlatformTextContextMenuProviders(
    #[default(crate::Modifier)] modifier: Modifier,
    content: impl Fn() + 'static,
) {
    let dropdown_defined = LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.current().is_some();
    let toolbar_defined = LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.current().is_some();
    match (dropdown_defined, toolbar_defined) {
        (true, true) => {
            let content = content.clone();
            crate::layout::Box(
                modifier,
                crate::Alignment::TOP_START,
                true,
                compose_runtime::callback(move |_| content()),
            );
        }
        (true, false) => ProvidePlatformTextContextMenuToolbar(modifier, content),
        (false, true) => ProvideDefaultTextContextMenuDropdown(modifier, content),
        (false, false) => ProvideBothDefaultProviders(modifier, content),
    }
}

/// Both defaults, sharing the box and its coordinates, as Compose's `ProvideBothDefaultProviders`.
#[composable]
#[allow(non_snake_case)]
fn ProvideBothDefaultProviders(modifier: Modifier, content: impl Fn() + 'static) {
    // A move gives the same coordinates with another place: never equal, so it's a change.
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
    let dropdown = default_text_context_menu_dropdown();
    let toolbar = platform_text_context_menu_toolbar_provider(anchor.clone());
    let as_dropdown: Rc<dyn TextContextMenuProvider> = dropdown.clone();
    compose_runtime::CompositionLocalProvider(
        (
            LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.provides(toolbar),
            LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.provides(Some(as_dropdown)),
        ),
        || {
            let content = content.clone();
            crate::layout::Box(
                modifier
                    .clone()
                    .on_globally_positioned(move |placed| coordinates.set(Some(placed))),
                crate::Alignment::TOP_START,
                true,
                compose_runtime::callback(move |_| {
                    content();
                    dropdown.context_menu(anchor.clone());
                }),
            );
        },
    );
}
