//! The text context menu, as Compose's `text.contextmenu` packages: what a right click on a
//! field or a selection opens, and the floating toolbar a finger's selection shows; its
//! components, built and filtered by modifiers up the hierarchy, shown by replaceable providers
//! (docs/parity/design/text-context-menu.md, D2031).

pub mod builder;
pub mod data;
pub mod default_providers;
pub mod dropdown;
pub mod modifier;
pub mod platform_toolbar;
pub mod provider;

pub use builder::{TextContextMenuBuilderScope, TextContextMenuFilter};
pub use data::{
    TextContextMenuAction, TextContextMenuComponent, TextContextMenuData, TextContextMenuItem,
    TextContextMenuKey, TextContextMenuKeys, TextContextMenuSession,
};
pub use modifier::{TextContextMenuBuilder, TextContextMenuModifiers};
pub use provider::{
    LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER, LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER,
    TextContextMenuDataProvider, TextContextMenuProvider, TextContextMenuProviderLocal,
};
