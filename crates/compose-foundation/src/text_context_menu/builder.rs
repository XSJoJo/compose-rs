//! Building a text context menu, as Compose's `text.contextmenu.builder`.

use std::rc::Rc;

use compose_ui::Painter;

use super::data::{
    TextContextMenuComponent, TextContextMenuData, TextContextMenuItem, TextContextMenuKey,
    TextContextMenuSession,
};

/// A component's filter: whether it shows.
pub type TextContextMenuFilter = Rc<dyn Fn(&TextContextMenuComponent) -> bool>;

/// What a builder of [`append_text_context_menu_components`] adds to, as Compose's
/// `TextContextMenuBuilderScope`: items and separators, in order.
///
/// [`append_text_context_menu_components`]: super::modifier::TextContextMenuModifiers::append_text_context_menu_components
#[derive(Default)]
pub struct TextContextMenuBuilderScope {
    components: Vec<TextContextMenuComponent>,
    filters: Vec<TextContextMenuFilter>,
}

impl TextContextMenuBuilderScope {
    /// A scope with nothing in it. Compose's is internal; compose-rs's tests build with it.
    #[doc(hidden)]
    pub fn new() -> TextContextMenuBuilderScope {
        TextContextMenuBuilderScope::default()
    }

    /// Adds an item, as Compose's `item(key, label, leadingIcon, onClick)`: `key` identifies it
    /// (any value with equality, or a [`TextContextMenuKey`]), `label` is its text, `leading_icon`
    /// the icon before it, `on_click` what a click does (closing the menu through its session, if
    /// it should).
    pub fn item<K: std::any::Any + PartialEq + std::fmt::Debug>(
        &mut self,
        key: K,
        label: impl Into<String>,
        leading_icon: Option<Painter>,
        on_click: impl Fn(&dyn TextContextMenuSession) + 'static,
    ) {
        self.add_component(TextContextMenuComponent::Item(TextContextMenuItem::new(
            TextContextMenuKey::new(key),
            label,
            leading_icon,
            on_click,
        )));
    }

    /// Adds a separator, as Compose's `separator()`: successive ones are one.
    pub fn separator(&mut self) {
        self.components.push(TextContextMenuComponent::Separator);
    }

    #[doc(hidden)]
    pub fn add_component(&mut self, component: TextContextMenuComponent) {
        self.components.push(component);
    }

    /// Adds a filter every component but separators must pass, as Compose's `addFilter`.
    #[doc(hidden)]
    pub fn add_filter(&mut self, filter: TextContextMenuFilter) {
        self.filters.push(filter);
    }

    /// What it built, as Compose's `build()`: no separator first, last or after another, and only
    /// the components every filter passes (separators aren't filtered).
    #[doc(hidden)]
    pub fn build(self) -> TextContextMenuData {
        let mut result: Vec<TextContextMenuComponent> = Vec::new();
        let mut head = true;
        for current in self.components {
            if head && current.is_separator() {
                continue;
            }
            head = false;
            if current.is_separator() && result.last().is_some_and(|last| last.is_separator()) {
                continue;
            }
            if !current.is_separator() && self.filters.iter().any(|filter| !filter(&current)) {
                continue;
            }
            result.push(current);
        }
        if result.last().is_some_and(|last| last.is_separator()) {
            result.pop();
        }
        TextContextMenuData::new(result)
    }
}
