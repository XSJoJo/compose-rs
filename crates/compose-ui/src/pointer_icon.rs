//! Pointer icons, as Compose's `PointerIcon`, `Modifier.pointerHoverIcon` and
//! `Modifier.stylusHoverIcon` (D1676): what the mouse pointer (or a hovering stylus) looks like
//! over a node. The icon is what the hovered path says, read when a host asks: the innermost
//! hovered node's, unless one around it overrides its descendants (the outermost such one wins),
//! the arrow over nothing ([06](../../../docs/06-ui.md#input-focus-and-semantics)).

use crate::image::ImageBitmap;
use crate::input::{EventContext, PointerEvent, PointerEventPass, PointerEventType};
use crate::modifier::{
    InspectorInfo, Modifier, ModifierNode, ModifierNodeElement, PointerInputModifierNode,
};
use crate::unit::{IntSize, Offset};

/// What a pointer looks like, as Compose's `PointerIcon`: one of the platform's system icons
/// (Android's types, which the desktop maps onto its cursors) or a bitmap.
///
/// The built-in ones are Compose's: [`DEFAULT`](Self::DEFAULT) (the arrow),
/// [`CROSSHAIR`](Self::CROSSHAIR), [`TEXT`](Self::TEXT) (an I-beam) and [`HAND`](Self::HAND).
/// The others are made with [`from_type`](Self::from_type), as Compose's
/// `PointerIcon(pointerIconType)`, and [`from_bitmap`](Self::from_bitmap), as
/// `PointerIcon(android.view.PointerIcon.create(bitmap, hotSpotX, hotSpotY))`. Two icons are
/// equal when they're the same type, or the same bitmap (a clone of it) at the same hot spot:
/// `PointerIcon::from_type(PointerIcon::TYPE_HAND) == PointerIcon::HAND`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PointerIcon(IconKind);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum IconKind {
    Type(i32),
    Bitmap(BitmapIcon),
}

/// A bitmap and its hot spot: the pixel at the pointer's position.
#[derive(Clone, Debug)]
struct BitmapIcon {
    bitmap: ImageBitmap,
    hot_spot: Offset,
}

impl PartialEq for BitmapIcon {
    fn eq(&self, other: &Self) -> bool {
        self.bitmap.same_image(&other.bitmap)
            && self.hot_spot.x.to_bits() == other.hot_spot.x.to_bits()
            && self.hot_spot.y.to_bits() == other.hot_spot.y.to_bits()
    }
}

impl Eq for BitmapIcon {}

impl std::hash::Hash for BitmapIcon {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.bitmap.id().hash(state);
        self.hot_spot.x.to_bits().hash(state);
        self.hot_spot.y.to_bits().hash(state);
    }
}

impl PointerIcon {
    /// The arrow, as `PointerIcon.Default`.
    pub const DEFAULT: PointerIcon = PointerIcon::from_type(PointerIcon::TYPE_ARROW);
    /// For picking a precise point, as `PointerIcon.Crosshair`.
    pub const CROSSHAIR: PointerIcon = PointerIcon::from_type(PointerIcon::TYPE_CROSSHAIR);
    /// The I-beam, over text that can be edited or selected, as `PointerIcon.Text`.
    pub const TEXT: PointerIcon = PointerIcon::from_type(PointerIcon::TYPE_TEXT);
    /// A hand, over what can be clicked, as `PointerIcon.Hand`.
    pub const HAND: PointerIcon = PointerIcon::from_type(PointerIcon::TYPE_HAND);

    // Android's system icon types (`android.view.PointerIcon.TYPE_*`), for `from_type`.
    /// No icon: the pointer is hidden.
    pub const TYPE_NULL: i32 = 0;
    pub const TYPE_ARROW: i32 = 1000;
    pub const TYPE_CONTEXT_MENU: i32 = 1001;
    pub const TYPE_HAND: i32 = 1002;
    pub const TYPE_HELP: i32 = 1003;
    pub const TYPE_WAIT: i32 = 1004;
    pub const TYPE_CELL: i32 = 1006;
    pub const TYPE_CROSSHAIR: i32 = 1007;
    pub const TYPE_TEXT: i32 = 1008;
    pub const TYPE_VERTICAL_TEXT: i32 = 1009;
    pub const TYPE_ALIAS: i32 = 1010;
    pub const TYPE_COPY: i32 = 1011;
    pub const TYPE_NO_DROP: i32 = 1012;
    pub const TYPE_ALL_SCROLL: i32 = 1013;
    pub const TYPE_HORIZONTAL_DOUBLE_ARROW: i32 = 1014;
    pub const TYPE_VERTICAL_DOUBLE_ARROW: i32 = 1015;
    pub const TYPE_TOP_RIGHT_DIAGONAL_DOUBLE_ARROW: i32 = 1016;
    pub const TYPE_TOP_LEFT_DIAGONAL_DOUBLE_ARROW: i32 = 1017;
    pub const TYPE_ZOOM_IN: i32 = 1018;
    pub const TYPE_ZOOM_OUT: i32 = 1019;
    pub const TYPE_GRAB: i32 = 1020;
    pub const TYPE_GRABBING: i32 = 1021;
    pub const TYPE_HANDWRITING: i32 = 1022;

    /// The platform's icon of `pointer_icon_type` (a `TYPE_*`), as Compose's
    /// `PointerIcon(pointerIconType)`. A host that doesn't know a type shows the arrow.
    pub const fn from_type(pointer_icon_type: i32) -> PointerIcon {
        PointerIcon(IconKind::Type(pointer_icon_type))
    }

    /// `bitmap` with its `hot_spot` (the pixel, from its top left, at the pointer's position),
    /// as Compose's `PointerIcon(android.view.PointerIcon.create(bitmap, hotSpotX, hotSpotY))`.
    pub fn from_bitmap(bitmap: ImageBitmap, hot_spot: Offset) -> PointerIcon {
        PointerIcon(IconKind::Bitmap(BitmapIcon { bitmap, hot_spot }))
    }

    /// Its system type (a `TYPE_*`), if it's one: what a host shows.
    pub fn pointer_icon_type(&self) -> Option<i32> {
        match &self.0 {
            IconKind::Type(pointer_icon_type) => Some(*pointer_icon_type),
            IconKind::Bitmap(_) => None,
        }
    }

    /// Its bitmap and hot spot, if it's one: what a host shows.
    pub fn bitmap(&self) -> Option<(&ImageBitmap, Offset)> {
        match &self.0 {
            IconKind::Type(_) => None,
            IconKind::Bitmap(icon) => Some((&icon.bitmap, icon.hot_spot)),
        }
    }
}

impl Default for PointerIcon {
    fn default() -> PointerIcon {
        PointerIcon::DEFAULT
    }
}

impl compose_runtime::ComposeArg for PointerIcon {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// What a hover icon node shows, as Compose's `HoverIconModifierNode`: its icon, whether it
/// overrides its descendants' and whose pointer it's for.
#[derive(Clone, PartialEq, Debug)]
pub struct HoverIcon {
    pub icon: PointerIcon,
    /// Its descendants' icons don't show while it's hovered: its does, as `overrideDescendants`.
    pub override_descendants: bool,
    /// A hovering stylus's or eraser's (`stylusHoverIcon`), not the mouse's (`pointerHoverIcon`).
    pub stylus: bool,
}

/// `Modifier.pointer_hover_icon`'s and `stylus_hover_icon`'s element.
struct HoverIconElement(HoverIcon);

/// `PointerHoverIconModifierNode` and `StylusHoverIconModifierNode`: hit by the pointer, it's on
/// the hovered path the icon is read from.
struct HoverIconNode(HoverIcon);

impl ModifierNodeElement for HoverIconElement {
    type Node = HoverIconNode;
    fn create(&self) -> HoverIconNode {
        HoverIconNode(self.0.clone())
    }
    fn update(&self, node: &mut HoverIconNode) {
        node.0.clone_from(&self.0);
    }
    fn same(&self, other: &Self) -> bool {
        other.0 == self.0
    }
    fn name(&self) -> &'static str {
        if self.0.stylus { "stylus_hover_icon" } else { "pointer_hover_icon" }
    }
    fn inspectable_properties(&self, info: &mut InspectorInfo) {
        info.name = Some(self.name());
        info.properties.push(("icon", format!("{:?}", self.0.icon)));
        info.properties.push(("overrideDescendants", self.0.override_descendants.to_string()));
    }
}

impl ModifierNode for HoverIconNode {
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
    /// The icon is read when a host asks: nothing to draw or measure again.
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl PointerInputModifierNode for HoverIconNode {
    fn on_pointer_event(
        &mut self,
        event: &mut PointerEvent,
        pass: PointerEventPass,
        _bounds: IntSize,
        context: &mut EventContext<'_>,
    ) {
        // Hovered: the hosts' icon has something to read from now on (until then it's the
        // arrow, read for nothing).
        if pass == PointerEventPass::Main && event.kind == PointerEventType::Enter {
            context.requests.hover_icons = true;
        }
    }

    fn hover_icon(&self) -> Option<&HoverIcon> {
        Some(&self.0)
    }
}

impl Modifier {
    /// The mouse pointer looks like `icon` over the node, as `Modifier.pointerHoverIcon(icon)`:
    /// a descendant's icon shows over the descendant.
    pub fn pointer_hover_icon(self, icon: PointerIcon) -> Modifier {
        self.pointer_hover_icon_with(icon, false)
    }

    /// As `Modifier.pointerHoverIcon(icon, overrideDescendants)`: with `override_descendants`,
    /// `icon` shows over the descendants too, whatever theirs (the outermost overriding node
    /// wins).
    pub fn pointer_hover_icon_with(
        self,
        icon: PointerIcon,
        override_descendants: bool,
    ) -> Modifier {
        self.then_custom(HoverIconElement(HoverIcon { icon, override_descendants, stylus: false }))
    }

    /// A hovering stylus (or eraser) looks like `icon` over the node, as
    /// `Modifier.stylusHoverIcon(icon)`. Read with `UiHost::stylus_hover_icon`.
    pub fn stylus_hover_icon(self, icon: PointerIcon) -> Modifier {
        self.stylus_hover_icon_with(icon, false)
    }

    /// As `Modifier.stylusHoverIcon(icon, overrideDescendants)`. Compose's third parameter,
    /// `touchBoundsExpansion`, waits for touch bounds expansion in hit testing.
    pub fn stylus_hover_icon_with(self, icon: PointerIcon, override_descendants: bool) -> Modifier {
        self.then_custom(HoverIconElement(HoverIcon { icon, override_descendants, stylus: true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A type's icon is the built-in one of that type, as Compose's `AndroidPointerIconType`
    /// equality; a bitmap's is the same image at the same hot spot.
    #[test]
    fn icons_are_equal_by_type_or_by_image_and_hot_spot() {
        assert_eq!(PointerIcon::from_type(PointerIcon::TYPE_HAND), PointerIcon::HAND);
        assert_ne!(PointerIcon::from_type(PointerIcon::TYPE_GRAB), PointerIcon::HAND);
        assert_eq!(PointerIcon::default(), PointerIcon::DEFAULT);
        assert_eq!(PointerIcon::DEFAULT.pointer_icon_type(), Some(1000));
        let image = ImageBitmap::from_rgba_premultiplied(1, 1, vec![0, 0, 0, 255]);
        let icon = PointerIcon::from_bitmap(image.clone(), Offset::new(0.0, 0.0));
        assert_eq!(icon, PointerIcon::from_bitmap(image.clone(), Offset::new(0.0, 0.0)));
        assert_ne!(icon, PointerIcon::from_bitmap(image.clone(), Offset::new(1.0, 0.0)));
        let other = ImageBitmap::from_rgba_premultiplied(1, 1, vec![0, 0, 0, 255]);
        assert_ne!(icon, PointerIcon::from_bitmap(other, Offset::new(0.0, 0.0)));
        assert_eq!(icon.pointer_icon_type(), None);
        assert!(icon.bitmap().is_some_and(|(bitmap, _)| bitmap.same_image(&image)));
    }
}
