//! What a text context menu shows, as Compose's `text.contextmenu.data`.

use std::any::Any;
use std::fmt;
use std::rc::Rc;

use compose_ui::Painter;

/// What identifies a component of a text context menu, as Compose's `key: Any`: any value with
/// equality, compared by value and type (Compose's `equals`). Filters find components by it.
#[derive(Clone)]
pub struct TextContextMenuKey(Rc<dyn KeyValue>);

trait KeyValue: Any + fmt::Debug {
    fn equals(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + PartialEq + fmt::Debug> KeyValue for T {
    fn equals(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|other| self == other)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TextContextMenuKey {
    /// The key `value`; a key given is that key.
    pub fn new<T: Any + PartialEq + fmt::Debug>(value: T) -> TextContextMenuKey {
        if let Some(key) = (&value as &dyn Any).downcast_ref::<TextContextMenuKey>() {
            return key.clone();
        }
        TextContextMenuKey(Rc::new(value))
    }

    /// Whether it's the key `value`.
    pub fn is<T: Any + PartialEq + fmt::Debug>(&self, value: &T) -> bool {
        if let Some(key) = (value as &dyn Any).downcast_ref::<TextContextMenuKey>() {
            return self == key;
        }
        self.0.equals(value)
    }

    /// Its value, if it's a `T`.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref::<T>()
    }
}

impl PartialEq for TextContextMenuKey {
    fn eq(&self, other: &TextContextMenuKey) -> bool {
        self.0.equals(other.0.as_any())
    }
}

impl fmt::Debug for TextContextMenuKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The keys of Compose's own items, as `TextContextMenuKeys`'s `CutKey`, `CopyKey`, `PasteKey`,
/// `SelectAllKey` and `AutofillKey`: `TextContextMenuKey::new(TextContextMenuKeys::Cut)`, or
/// [`key`](Self::key).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextContextMenuKeys {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Autofill,
}

impl TextContextMenuKeys {
    pub fn key(self) -> TextContextMenuKey {
        TextContextMenuKey::new(self)
    }
}

/// A session of an open text context menu, as Compose's `TextContextMenuSession`: what closes it.
pub trait TextContextMenuSession {
    /// Closes the menu.
    fn close(&self);
}

/// An item's click: with the menu's session, to close it.
pub type TextContextMenuAction = Rc<dyn Fn(&dyn TextContextMenuSession)>;

/// A clickable item of a text context menu, as Compose's `TextContextMenuItem`: its key, its
/// label, the icon before it (Compose's drawable resource: a painter here), and its click.
#[derive(Clone)]
pub struct TextContextMenuItem {
    pub key: TextContextMenuKey,
    pub label: String,
    pub leading_icon: Option<Painter>,
    pub on_click: TextContextMenuAction,
}

impl TextContextMenuItem {
    pub fn new(
        key: TextContextMenuKey,
        label: impl Into<String>,
        leading_icon: Option<Painter>,
        on_click: impl Fn(&dyn TextContextMenuSession) + 'static,
    ) -> TextContextMenuItem {
        TextContextMenuItem { key, label: label.into(), leading_icon, on_click: Rc::new(on_click) }
    }
}

impl fmt::Debug for TextContextMenuItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "TextContextMenuItem(key={:?}, label={:?}, leadingIcon={})",
            self.key,
            self.label,
            self.leading_icon.is_some()
        )
    }
}

/// A component of a text context menu, as Compose's `TextContextMenuComponent`: an item, or a
/// separator between groups (Compose's `TextContextMenuSeparator`).
#[derive(Clone, Debug)]
pub enum TextContextMenuComponent {
    Item(TextContextMenuItem),
    Separator,
}

impl TextContextMenuComponent {
    /// Its key, as Compose's `key`; a separator's is its own, equal to no other.
    pub fn key(&self) -> TextContextMenuKey {
        match self {
            TextContextMenuComponent::Item(item) => item.key.clone(),
            TextContextMenuComponent::Separator => SEPARATOR_KEY.with(Clone::clone),
        }
    }

    pub fn is_separator(&self) -> bool {
        matches!(self, TextContextMenuComponent::Separator)
    }

    /// Its item, if it's one.
    pub fn as_item(&self) -> Option<&TextContextMenuItem> {
        match self {
            TextContextMenuComponent::Item(item) => Some(item),
            TextContextMenuComponent::Separator => None,
        }
    }
}

/// A separator's key: a value of its own, as Compose's `Any()`.
#[derive(PartialEq, Debug)]
struct SeparatorKey;

thread_local! {
    static SEPARATOR_KEY: TextContextMenuKey = TextContextMenuKey::new(SeparatorKey);
}

/// What a text context menu shows, as Compose's `TextContextMenuData`: its components, in order.
#[derive(Clone, Default)]
pub struct TextContextMenuData {
    pub components: Vec<TextContextMenuComponent>,
}

impl TextContextMenuData {
    /// Compose's `TextContextMenuData.Empty`.
    pub const EMPTY: TextContextMenuData = TextContextMenuData { components: Vec::new() };

    pub fn new(components: Vec<TextContextMenuComponent>) -> TextContextMenuData {
        TextContextMenuData { components }
    }

    /// Whether `other` shows the same: the same keys and labels in order (Compose's data,
    /// derived, is the same object until what it read changes; a toolbar says it changed only
    /// on a change).
    pub(crate) fn shows_as(&self, other: &TextContextMenuData) -> bool {
        self.components.len() == other.components.len()
            && self.components.iter().zip(&other.components).all(|pair| match pair {
                (TextContextMenuComponent::Separator, TextContextMenuComponent::Separator) => true,
                (TextContextMenuComponent::Item(a), TextContextMenuComponent::Item(b)) => {
                    a.key == b.key && a.label == b.label
                }
                _ => false,
            })
    }
}

impl fmt::Debug for TextContextMenuData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "TextContextMenuData(components=[")?;
        for component in &self.components {
            write!(formatter, "\n\t{component:?}")?;
        }
        write!(formatter, "\n])")
    }
}
