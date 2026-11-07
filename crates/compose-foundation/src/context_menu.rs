//! The dropdown context menu's internals, as Compose's internal `foundation.contextmenu`: its
//! state and where its popup goes (D2034). Hidden: compose-rs's tests are outside the crate.

use std::rc::Rc;

use compose_ui::{
    IntOffset, IntRect, IntSize, LayoutDirection, Offset, PopupPositionProvider, Rect,
};

/// Whether a context menu is open, and where, as Compose's `ContextMenuState.Status`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ContextMenuStatus {
    /// Open at an offset, which must be specified (finite).
    Open(Offset),
    Closed,
}

impl ContextMenuStatus {
    /// An open status at `offset`, as Compose's `Status.Open(offset)`.
    ///
    /// Panics if `offset` is unspecified (not finite), as Compose's precondition.
    pub fn open(offset: Offset) -> ContextMenuStatus {
        assert!(
            offset.x.is_finite() && offset.y.is_finite(),
            "ContextMenuState.Status should never be open with an unspecified offset. Use \
             ContextMenuState.Status.Closed instead."
        );
        ContextMenuStatus::Open(offset)
    }
}

/// The state of a context menu, as Compose's `ContextMenuState`: its status, a state.
pub struct ContextMenuState {
    status: compose_runtime::OwnedState<ContextMenuStatus>,
}

impl PartialEq for ContextMenuState {
    /// Equal when their statuses are, as Compose's.
    fn eq(&self, other: &ContextMenuState) -> bool {
        self.status() == other.status()
    }
}

impl std::fmt::Debug for ContextMenuState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "ContextMenuState(status={:?})", self.status())
    }
}

impl ContextMenuState {
    pub fn new(status: ContextMenuStatus) -> ContextMenuState {
        ContextMenuState { status: compose_runtime::mutable_state_of(status) }
    }

    pub fn status(&self) -> ContextMenuStatus {
        self.status.get()
    }

    pub fn set_status(&self, status: ContextMenuStatus) {
        self.status.set(status);
    }

    /// Closes it, as Compose's `close`.
    pub fn close(&self) {
        self.status.set(ContextMenuStatus::Closed);
    }
}

/// A popup position provider placing a context menu fully in the window, a corner at the
/// anchor's position, as Compose's `ContextMenuPopupPositionProvider`: along each axis, at the
/// window's start when the menu is larger than it; else after the position if it fits there,
/// before it if it fits there, else at the window's end. Horizontally, "after" is the layout
/// direction's.
pub struct ContextMenuPopupPositionProvider {
    anchor_position: Rc<dyn Fn() -> IntOffset>,
    on_position_calculated: Option<Rc<dyn Fn(IntOffset, IntRect)>>,
}

impl ContextMenuPopupPositionProvider {
    /// At what `anchor_position` gives (in the anchor's coordinates, read at each placement), with
    /// `on_position_calculated` told the position and the menu's bounds after each.
    pub fn new(
        anchor_position: impl Fn() -> IntOffset + 'static,
        on_position_calculated: Option<Rc<dyn Fn(IntOffset, IntRect)>>,
    ) -> ContextMenuPopupPositionProvider {
        ContextMenuPopupPositionProvider {
            anchor_position: Rc::new(anchor_position),
            on_position_calculated,
        }
    }

    /// At `position`, as Compose's constructor taking an `IntOffset`.
    pub fn at(
        position: IntOffset,
        on_position_calculated: Option<Rc<dyn Fn(IntOffset, IntRect)>>,
    ) -> ContextMenuPopupPositionProvider {
        ContextMenuPopupPositionProvider::new(move || position, on_position_calculated)
    }

    /// Compose's `calculatePosition(anchorBounds, windowSize, layoutDirection, popupContentSize)`.
    pub fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let anchor_position = (self.anchor_position)();
        let result = IntOffset::new(
            align_popup_axis(
                anchor_bounds.left + anchor_position.x,
                popup_content_size.width,
                window_size.width,
                layout_direction == LayoutDirection::Ltr,
            ),
            align_popup_axis(
                anchor_bounds.top + anchor_position.y,
                popup_content_size.height,
                window_size.height,
                true,
            ),
        );
        if let Some(on_position_calculated) = &self.on_position_calculated {
            on_position_calculated(
                anchor_position,
                IntRect::new(
                    result.x,
                    result.y,
                    result.x + popup_content_size.width,
                    result.y + popup_content_size.height,
                ),
            );
        }
        result
    }
}

impl PopupPositionProvider for ContextMenuPopupPositionProvider {
    fn position(
        &self,
        anchor: Rect,
        window: IntSize,
        layout_direction: LayoutDirection,
        popup: IntSize,
    ) -> IntOffset {
        let anchor = IntRect::new(
            anchor.left.round() as i32,
            anchor.top.round() as i32,
            anchor.right.round() as i32,
            anchor.bottom.round() as i32,
        );
        self.calculate_position(anchor, window, layout_direction, popup)
    }
}

/// Where a popup of `popup_length` goes along an axis of the window's `window_length` to align
/// with `position`, as Compose's `alignPopupAxis`: `close_affinity` says the axis' start is the
/// close edge (0), else the far one.
pub fn align_popup_axis(
    position: i32,
    popup_length: i32,
    window_length: i32,
    close_affinity: bool,
) -> i32 {
    let fits_before = |close: bool| {
        if close { popup_length <= position } else { window_length - popup_length > position }
    };
    let start_to_position = |close: bool| if close { position } else { position - popup_length };
    let start_edges = |close: bool| if close { 0 } else { window_length - popup_length };
    if popup_length >= window_length {
        start_edges(close_affinity)
    } else if fits_before(!close_affinity) {
        start_to_position(close_affinity)
    } else if fits_before(close_affinity) {
        start_to_position(!close_affinity)
    } else {
        start_edges(!close_affinity)
    }
}
