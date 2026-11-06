//! The pointer icons on Android (D1678): what the view was last told of the mouse's icon and a
//! hovering stylus's, so it asks again (`ComposeRsNative.pointerIcon`) only when they changed,
//! and how an icon crosses to Kotlin: Android's type, or a bitmap's pixels and hot spot. The
//! view sets the mouse's as its `pointerIcon` (Compose's `PointerIconService.setIcon`) and gives
//! the stylus's in `onResolvePointerIcon`, as `AndroidComposeView`.

use compose_ui::PointerIcon;
use compose_ui::platform::UiHost;

/// What `ComposeRsNative.pointerIcon` answers for no icon: a stylus over none (the view's own
/// then, as Compose's `null`).
pub(crate) const NONE: i32 = -1;
/// What it answers for a bitmap: the view asks for it (`pointerIconBitmap`).
pub(crate) const BITMAP: i32 = -2;

/// The icons the view was last told.
pub(crate) struct ToldIcons {
    mouse: PointerIcon,
    stylus: Option<PointerIcon>,
}

impl Default for ToldIcons {
    /// A new view's: the arrow (its `pointerIcon` unset), no stylus icon.
    fn default() -> ToldIcons {
        ToldIcons { mouse: PointerIcon::DEFAULT, stylus: None }
    }
}

impl ToldIcons {
    /// Takes the host's icons: whether either differs from what the view has, which then asks.
    /// Free while no icon node was hovered (`UiHost::pointer_icon` answers at once).
    pub(crate) fn take_changes(&mut self, ui: &UiHost) -> bool {
        let mouse = ui.pointer_icon();
        let stylus = ui.stylus_hover_icon();
        if mouse == self.mouse && stylus == self.stylus {
            return false;
        }
        self.mouse = mouse;
        self.stylus = stylus;
        true
    }

    /// The mouse's icon, or a hovering `stylus`'s.
    pub(crate) fn icon(&self, stylus: bool) -> Option<&PointerIcon> {
        if stylus { self.stylus.as_ref() } else { Some(&self.mouse) }
    }
}

/// How `icon` crosses: Android's type (`PointerIcon.getSystemIcon`), [`BITMAP`] or [`NONE`].
pub(crate) fn code(icon: Option<&PointerIcon>) -> i32 {
    match icon {
        None => NONE,
        Some(icon) => icon.pointer_icon_type().unwrap_or(BITMAP),
    }
}

/// A bitmap icon's pixels (premultiplied RGBA, as `Bitmap.copyPixelsFromBuffer` reads an
/// `ARGB_8888` bitmap's) and its width, height and hot spot, for `PointerIcon.create`.
pub(crate) fn bitmap(icon: Option<&PointerIcon>) -> Option<(&[u8], [f32; 4])> {
    let (bitmap, hot_spot) = icon?.bitmap()?;
    let shape = [bitmap.width() as f32, bitmap.height() as f32, hot_spot.x, hot_spot.y];
    Some((bitmap.pixels(), shape))
}

#[cfg(test)]
mod tests {
    use super::*;
    use compose_ui::{ImageBitmap, Offset};

    /// A type crosses as itself, a bitmap as `BITMAP` then its pixels and shape, no stylus icon
    /// as `NONE`.
    #[test]
    fn icons_cross_as_their_type_or_their_bitmap() {
        assert_eq!(code(Some(&PointerIcon::HAND)), 1002);
        assert_eq!(code(Some(&PointerIcon::from_type(PointerIcon::TYPE_NULL))), 0);
        assert_eq!(code(None), NONE);
        let image = ImageBitmap::from_rgba_premultiplied(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let icon = PointerIcon::from_bitmap(image, Offset::new(1.0, 0.5));
        assert_eq!(code(Some(&icon)), BITMAP);
        let (pixels, shape) = bitmap(Some(&icon)).unwrap();
        assert_eq!(pixels, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(shape, [2.0, 1.0, 1.0, 0.5]);
        assert!(bitmap(Some(&PointerIcon::HAND)).is_none());
    }

    /// The view hears of a change once: the mouse's icon as the content changes it, a stylus's
    /// apart.
    #[test]
    fn a_change_is_told_once() {
        use compose_ui::Modifier;
        let mut ui = UiHost::new(
            compose_ui::IntSize::new(100, 100),
            compose_ui::Density::new(1.0),
            Box::new(compose_ui::platform::FixedTextMeasurer::new()),
        );
        ui.set_content(|| {
            compose_ui::Layout(
                Modifier
                    .fill_max_size()
                    .pointer_hover_icon(PointerIcon::HAND)
                    .stylus_hover_icon(PointerIcon::TEXT),
                compose_ui::empty_measure_policy(),
                compose_runtime::callback(|| {}),
            );
        });
        ui.frame();
        let mut told = ToldIcons::default();
        assert!(!told.take_changes(&ui), "the arrow, as the view has it");
        ui.send_pointer(compose_ui::PointerEvent::moved(Offset::new(10.0, 10.0), false));
        assert!(told.take_changes(&ui), "the hand");
        assert_eq!(told.icon(false), Some(&PointerIcon::HAND));
        assert_eq!(told.icon(true), None);
        assert!(!told.take_changes(&ui), "told already");
        let stylus = compose_ui::PointerEvent::moved(Offset::new(10.0, 10.0), false)
            .with_pointer(compose_ui::PointerId(1), compose_ui::PointerType::Stylus);
        ui.send_pointer(stylus);
        assert!(told.take_changes(&ui), "the stylus's I-beam");
        assert_eq!(told.icon(true), Some(&PointerIcon::TEXT));
        ui.send_stylus_left();
        ui.send_pointer_left();
        assert!(told.take_changes(&ui));
        assert_eq!((told.icon(false), told.icon(true)), (Some(&PointerIcon::DEFAULT), None));
    }
}
