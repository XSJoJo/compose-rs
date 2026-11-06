//! Compose's `foundation.gestures`: `Modifier.draggable` (drags along one axis reported as
//! deltas to a `DraggableState`), and scrolling (`scrollable`: `ScrollableState`,
//! `Modifier.scrollable`, flings), and zooming, panning and rotating (`transformable`).

mod anchored_draggable;
mod bring_into_view_queue;
pub(crate) mod content_in_view;
mod drag_gesture;
mod draggable;
mod mouse_wheel;
mod scrollable;
mod scrollable_2d;
mod snapping;
mod transformable;
mod updatable_animation_state;

pub use anchored_draggable::{
    AnchoredDragScope, AnchoredDraggableDefaults, AnchoredDraggableModifier,
    AnchoredDraggableOptions, AnchoredDraggableState, DraggableAnchors, DraggableAnchorsConfig,
    anchored_fling_behavior, remember_anchored_fling_behavior,
};
pub use draggable::{
    DefaultDraggable2DState, DefaultDraggableState, Drag2DBlock, Drag2DScope, DragBlock, DragScope,
    Draggable2DOptions, Draggable2DState, Draggable2DStateExt, DraggableModifier, DraggableOptions,
    DraggableState, DraggableStateExt, OnDrag2DStopped, OnDragStarted, OnDragStopped,
    draggable_2d_state, draggable_state, remember_draggable_2d_state, remember_draggable_state,
};

pub(crate) use drag_gesture::{DRAG_GESTURES, GestureState, aligned, parent_aligned};
pub(crate) use mouse_wheel::MouseWheel;
pub use scrollable::{
    DefaultFlingBehavior, DefaultScrollableState, FlingBehavior, LocalBoxFuture, ScrollBlock,
    ScrollScope, ScrollableDefaults, ScrollableModifier, ScrollableState, ScrollableStateExt,
    remember_scrollable_state, scrollable_state,
};
pub(crate) use scrollable::{DragSession, ScrollCore};
pub use scrollable_2d::{
    DefaultScrollable2DState, Scroll2DBlock, Scroll2DScope, Scrollable2DModifier,
    Scrollable2DState, Scrollable2DStateExt, remember_scrollable_2d_state, scrollable_2d_state,
};
pub use snapping::{
    CustomSnapPosition, MIN_FLING_VELOCITY, SnapFlingBehavior, SnapLayoutInfoProvider,
    SnapPosition, TargetedFlingBehavior, grid_snap_layout_info_provider,
    list_snap_layout_info_provider, remember_grid_snap_fling_behavior,
    remember_list_snap_fling_behavior, remember_snap_fling_behavior, snap_fling_behavior,
};
pub(crate) use snapping::{FinalSnappingItem, distance_to_snap_position, final_snapping_item};
pub use transformable::{
    DefaultTransformableState, TransformBlock, TransformCalculations, TransformGestureDetector,
    TransformScope, TransformableModifier, TransformableState, TransformableStateExt,
    remember_transformable_state, transformable_state,
};

use compose_runtime::ComposeArg;

/// An axis, as Compose's `Orientation`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

impl ComposeArg for Orientation {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}
