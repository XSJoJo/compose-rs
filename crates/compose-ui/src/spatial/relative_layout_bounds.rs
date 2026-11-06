//! What a layout-rect callback is told, as Compose's `RelativeLayoutBounds`.

use crate::tree::NodeId;
use crate::unit::{IntOffset, IntRect, IntSize};

/// The axis-aligned bounding rectangle of an element, in the coordinates of the root of its
/// composition, of the window, or of the screen, as Compose's `RelativeLayoutBounds`.
///
/// compose-rs's root fills the window: the root's coordinates are the window's, but for a popup's
/// content, whose root is the popup's. The screen's are the window's moved by the origin the host
/// gives (`UiHost::set_window_origin`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RelativeLayoutBounds {
    /// In the root.
    pub(crate) rect: IntRect,
    /// Where the window is on the screen.
    pub(crate) window_offset: IntOffset,
    /// Where the root is on the screen.
    pub(crate) screen_offset: IntOffset,
    pub(crate) window_size: IntSize,
    /// The tree's key and the layout node, for `calculate_occlusions`.
    pub(crate) tree: u32,
    pub(crate) node: NodeId,
}

impl RelativeLayoutBounds {
    /// Bounds with nothing behind them, for unit tests.
    #[cfg(test)]
    pub(crate) fn for_test(rect: IntRect) -> RelativeLayoutBounds {
        RelativeLayoutBounds {
            rect,
            window_offset: IntOffset::ZERO,
            screen_offset: IntOffset::ZERO,
            window_size: IntSize::ZERO,
            tree: 0,
            node: NodeId::from_bits(0),
        }
    }

    /// The top left in the root, as `positionInRoot`.
    pub fn position_in_root(&self) -> IntOffset {
        self.rect.top_left()
    }

    /// The top left in the window, as `positionInWindow`.
    pub fn position_in_window(&self) -> IntOffset {
        self.rect.top_left() + self.root_in_window()
    }

    /// The top left on the screen, as `positionInScreen`.
    pub fn position_in_screen(&self) -> IntOffset {
        self.rect.top_left() + self.screen_offset
    }

    /// Its width in pixels.
    pub fn width(&self) -> i32 {
        self.rect.width()
    }

    /// Its height in pixels.
    pub fn height(&self) -> i32 {
        self.rect.height()
    }

    /// The rectangle in the root, as `boundsInRoot`.
    pub fn bounds_in_root(&self) -> IntRect {
        self.rect
    }

    /// The rectangle in the window, as `boundsInWindow`.
    pub fn bounds_in_window(&self) -> IntRect {
        self.rect.translate(self.root_in_window())
    }

    /// The rectangle on the screen, as `boundsInScreen`.
    pub fn bounds_in_screen(&self) -> IntRect {
        self.rect.translate(self.screen_offset)
    }

    fn root_in_window(&self) -> IntOffset {
        self.screen_offset - self.window_offset
    }

    /// The fraction of its pixels inside `viewport`'s bounds, as `fractionVisibleIn`: 1 if the
    /// viewport is entirely inside it.
    pub fn fraction_visible_in(&self, viewport: &RelativeLayoutBounds) -> f32 {
        let rect = viewport.rect;
        self.fraction_visible_in_rect(rect.left, rect.top, rect.right, rect.bottom)
    }

    /// The fraction of its pixels inside the rectangle, as `fractionVisibleInRect`: of the
    /// smaller of the two, so 1 if the rectangle is entirely inside it.
    pub fn fraction_visible_in_rect(&self, left: i32, top: i32, right: i32, bottom: i32) -> f32 {
        let IntRect { left: l, top: t, right: r, bottom: b } = self.rect;
        let clipped_left = l.max(left).min(right);
        let clipped_top = t.max(top).min(bottom);
        let clipped_right = r.min(right).max(left);
        let clipped_bottom = b.min(bottom).max(top);
        let area = |width: i32, height: i32| width as i64 * height as i64;
        let viewport_area = area(right - left, bottom - top);
        let rect_area = area(r - l, b - t);
        let clipped_area = area(clipped_right - clipped_left, clipped_bottom - clipped_top).max(0);
        let max_area = viewport_area.min(rect_area);
        clipped_area as f32 / max_area as f32
    }

    /// The fraction of its pixels inside the window, as `fractionVisibleInWindow`.
    pub fn fraction_visible_in_window(&self) -> f32 {
        self.fraction_visible_in_rect(0, 0, self.window_size.width, self.window_size.height)
    }

    /// The fraction of its pixels inside the window less the insets, as
    /// `fractionVisibleInWindowWithInsets`.
    pub fn fraction_visible_in_window_with_insets(
        &self,
        top_left_inset: IntOffset,
        bottom_right_inset: IntOffset,
    ) -> f32 {
        let IntOffset { x, y } = self.window_offset;
        self.fraction_visible_in_rect(
            x + top_left_inset.x,
            y + top_left_inset.y,
            x + self.window_size.width - bottom_right_inset.x,
            y + self.window_size.height - bottom_right_inset.y,
        )
    }

    /// The rectangles of the layouts that may draw over it, as they're now, as
    /// `calculateOcclusions`: those intersecting it (edges touching count) that draw after it,
    /// transparent or not. Its ancestors and descendants never occlude it. Empty if it's gone or
    /// its tree is busy.
    pub fn calculate_occlusions(&self) -> Vec<IntRect> {
        let mut occlusions = Vec::new();
        crate::node_context::peek_tree(self.tree, &mut |tree| {
            occlusions = tree.occlusions_of(self.node);
        });
        occlusions
    }
}
