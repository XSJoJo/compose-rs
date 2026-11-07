//! Laying out content beyond a layout's bounds, as Compose's `androidx.compose.ui.layout`
//! `BeyondBoundsLayout`: a layout that shows part of its content (a lazy list) lays out more of it
//! in a direction while a block runs, for code that needs what isn't shown yet (focus moving
//! into items not composed). A node provides one by answering
//! [`ModifierNode::beyond_bounds_layout`](crate::ModifierNode::beyond_bounds_layout); the nodes
//! inside find it with [`NodeContext::find_nearest_beyond_bounds_layout_ancestor`], or the
//! deprecated modifier local [`MODIFIER_LOCAL_BEYOND_BOUNDS_LAYOUT`] (D1919).

use std::ops::ControlFlow;
use std::rc::Rc;

use crate::modifier_local::{ModifierLocal, modifier_local_of};
use crate::node_chain::NodeKinds;
use crate::node_context::NodeContext;

/// Where to lay out content beyond the bounds, as `BeyondBoundsLayout.LayoutDirection`: along
/// the layout's order (`Before`, `After`) or on the screen (the other four, which a layout may
/// not support along its axis).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BeyondBoundsLayoutDirection {
    Before,
    After,
    Left,
    Right,
    Above,
    Below,
}

/// What a beyond-bounds layout's block is told, as `BeyondBoundsLayout.BeyondBoundsScope`.
pub trait BeyondBoundsScope {
    /// Whether there's more content to lay out that way.
    fn has_more_content(&self) -> bool;
}

/// A layout that can lay out content beyond its bounds, as Compose's `BeyondBoundsLayout`.
///
/// Its one method is the type-erased form of Compose's `layout(direction, block)`, which is
/// [`layout`](BeyondBoundsLayout::layout) on `dyn BeyondBoundsLayout`.
pub trait BeyondBoundsLayout {
    /// Lays out more content `direction`, one step at a time, calling `block` after each step
    /// until it returns `true` (it found what it wanted) or there's nothing more to lay out; then
    /// lets go of what it laid out beyond its bounds. A layout with nothing beyond its bounds may
    /// never call `block`.
    fn layout_until(
        &self,
        direction: BeyondBoundsLayoutDirection,
        block: &mut dyn FnMut(&dyn BeyondBoundsScope) -> bool,
    );
}

impl dyn BeyondBoundsLayout {
    /// Lays out content beyond the bounds `direction` until `block` returns something, as
    /// `BeyondBoundsLayout.layout`: returns what it last returned (`None` if it never ran).
    pub fn layout<T>(
        &self,
        direction: BeyondBoundsLayoutDirection,
        mut block: impl FnMut(&dyn BeyondBoundsScope) -> Option<T>,
    ) -> Option<T> {
        let mut found = None;
        self.layout_until(direction, &mut |scope| {
            found = block(scope);
            found.is_some()
        });
        found
    }
}

/// The nearest beyond-bounds layout around, as Compose's deprecated
/// `ModifierLocalBeyondBoundsLayout`: none unless a node provides it. Prefer
/// [`NodeContext::find_nearest_beyond_bounds_layout_ancestor`].
pub static MODIFIER_LOCAL_BEYOND_BOUNDS_LAYOUT: ModifierLocal<Option<Rc<dyn BeyondBoundsLayout>>> =
    modifier_local_of("ModifierLocalBeyondBoundsLayout", || None);

impl NodeContext<'_> {
    /// The beyond-bounds layout of the nearest node above this one providing one, as
    /// `DelegatableNode.findNearestBeyondBoundsLayoutAncestor`.
    pub fn find_nearest_beyond_bounds_layout_ancestor(
        &mut self,
    ) -> Option<Rc<dyn BeyondBoundsLayout>> {
        let mut found = None;
        self.visit_ancestors(NodeKinds::BEYOND_BOUNDS_LAYOUT, true, &mut |node, _| {
            found = node.beyond_bounds_layout();
            if found.is_some() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        });
        found
    }
}
