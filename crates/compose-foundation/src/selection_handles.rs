//! Touch selection's handles, as Compose's `SelectionHandle` and `CursorHandle` on Android: a
//! popup each (`HandlePopup`), at least 40 dp square to touch, drawn in
//! `LOCAL_TEXT_SELECTION_COLORS`' handle color, saying in its semantics which handle it is and
//! where it points (`SelectionHandleInfo`), and dragged by a finger
//! (docs/parity/design/touch-text-selection.md).
//!
//! A field's node decides which show and where; it tells them through [`TouchHandles`], which the
//! field's composable remembers and gives both to the node and to [`TextFieldHandles`], composed
//! inside the inner field.

use std::cell::Cell;
use std::rc::Rc;

use compose_runtime::snapshots::Snapshot as RuntimeSnapshot;
use compose_runtime::{OwnedState, composable};
use compose_ui::{
    Color, Dp, DrawModifierNode, DrawScope, EventContext, IntOffset, IntSize, Layout,
    LayoutDirection, Modifier, ModifierNode, ModifierNodeElement, NodeHandle, Offset, Paint,
    PointerEvent, PointerEventPass, PointerEventType, PointerInputModifierNode,
    PopupPositionProvider, PopupProperties, Rect, SemanticsConfiguration, SemanticsModifierNode,
    SemanticsPropertyKey, SemanticsValue, empty_measure_policy,
};

/// Which handle, as Compose's `Handle`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Handle {
    /// The cursor's, below a collapsed selection.
    Cursor,
    SelectionStart,
    SelectionEnd,
}

impl compose_runtime::ComposeArg for Handle {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Handle {
    pub(crate) fn index(self) -> usize {
        match self {
            Handle::Cursor => 0,
            Handle::SelectionStart => 1,
            Handle::SelectionEnd => 2,
        }
    }
}

/// How a handle sits on the point it marks, as Compose's `SelectionHandleAnchor`: a selection's
/// start pointing left is anchored left (its top right on the point), the cursor's in the middle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectionHandleAnchor {
    Left,
    Middle,
    Right,
}

/// What a handle's node says of it, as Compose's `SelectionHandleInfo`: which handle, the point it
/// marks in the text's coordinates (the cursor's bottom center, a selection end's line bottom),
/// how it's anchored there, and whether its icon shows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SelectionHandleInfo {
    pub handle: Handle,
    pub position: Offset,
    pub anchor: SelectionHandleAnchor,
    pub visible: bool,
}

impl SemanticsValue for SelectionHandleInfo {}

/// The key of a handle's [`SelectionHandleInfo`], as Compose's `SelectionHandleInfoKey`: what
/// tests find handles by (Compose's is internal, read by its tests; compose-rs's tests are
/// outside the crate).
pub static SELECTION_HANDLE_INFO_KEY: SemanticsPropertyKey<SelectionHandleInfo> =
    SemanticsPropertyKey::new("SelectionHandleInfo");

/// A handle's smallest size to touch, as Compose's `MinTouchTargetSizeForHandles` (Android's
/// `TextView`'s).
const MIN_TOUCH_TARGET: Dp = Dp(40.0);
/// A selection handle's icon, as Compose's `HandleWidth` and `HandleHeight`.
const HANDLE_SIZE: Dp = Dp(25.0);
/// The cursor handle's icon: as high as a selection handle, and as wide as that turned 45°
/// (Compose's `CursorHandleWidth`).
const CURSOR_HANDLE_WIDTH: Dp = Dp(25.0 * 2.0 / (1.0 + std::f32::consts::SQRT_2));

/// Which handles show: the cursor's, and each selection end's with whether it points left
/// (Compose's `isLeftSelectionHandle`).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) struct ShownHandles {
    pub cursor: bool,
    pub start: Option<bool>,
    pub end: Option<bool>,
}

impl ShownHandles {
    pub(crate) fn any(&self) -> bool {
        self.cursor || self.start.is_some() || self.end.is_some()
    }
}

/// Where each handle points (by [`Handle::index`]): in the text's coordinates, what its
/// semantics say, and in its anchor's (the inner field, its scroll taken off), where its popup
/// goes.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub(crate) struct HandlePositions {
    pub text: [Offset; 3],
    pub anchor: [Offset; 3],
}

/// What a field's node and its handles share: which show and where (states, so composition and
/// the handles' semantics follow them), the drag under way, and the field to tell of it.
pub(crate) struct TouchHandles {
    pub shown: OwnedState<ShownHandles>,
    pub positions: OwnedState<HandlePositions>,
    /// A handle's drag the field hasn't followed yet: the handle, and the finger's movement in the
    /// window since the drag passed the touch slop (Compose's `dragTotalDistance`, the slop taken
    /// off).
    pub moved: Cell<Option<(Handle, Offset)>>,
    /// The drag ended (lifted or cancelled): the field forgets it, after its last move.
    pub ended: Cell<bool>,
    /// The cursor handle was tapped: what the text toolbar will toggle on (`text-context-menu`).
    pub cursor_tapped: Cell<bool>,
    /// The field's node, measured again when a handle is dragged: the one focused last (a
    /// decoration moving the inner field makes the node anew).
    pub field: Cell<Option<NodeHandle>>,
    /// While handles may show: the inner field's bounds clipped by its ancestors are followed,
    /// as Compose's `textLayoutCoordinates.visibleBounds()`.
    pub active: OwnedState<bool>,
    /// The inner field's visible bounds, in its coordinates, after its last placement: a handle's
    /// point outside them hides it (one frame behind, as Compose's).
    pub visible: Cell<Option<Rect>>,
}

impl TouchHandles {
    pub(crate) fn new() -> TouchHandles {
        TouchHandles {
            shown: compose_runtime::mutable_state_of(ShownHandles::default()),
            positions: compose_runtime::mutable_state_of(HandlePositions::default()),
            moved: Cell::new(None),
            ended: Cell::new(false),
            cursor_tapped: Cell::new(false),
            field: Cell::new(None),
            active: compose_runtime::mutable_state_of(false),
            visible: Cell::new(None),
        }
    }

    /// The field's node `field` is gone: nothing it told shows any more, and drags aren't told
    /// to it.
    pub(crate) fn forget(&self, field: Option<NodeHandle>) {
        if field.is_some() && self.field.get() == field {
            self.field.set(None);
        }
        if self.shown.with(ShownHandles::any) {
            self.shown.set(ShownHandles::default());
        }
        if self.active.with(|active| *active) {
            self.active.set(false);
        }
    }

    fn measure_field(&self) {
        if let Some(field) = self.field.get() {
            field.invalidate_measurement();
        }
    }

    fn tell_field(&self, moved: Option<(Handle, Offset)>) {
        match moved {
            Some(moved) => self.moved.set(Some(moved)),
            None => self.ended.set(true),
        }
        self.measure_field();
    }
}

impl compose_runtime::ComposeArg for TouchHandles {
    fn same(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

/// The handles of a field, composed inside its inner field, as Compose's `TextFieldCursorHandle`
/// and `TextFieldSelectionHandles`: each only while its node says it shows.
#[composable]
#[allow(non_snake_case)]
pub(crate) fn TextFieldHandles(handles: Rc<TouchHandles>) {
    if handles.active.get() {
        VisibleBoundsReader(handles.clone());
    }
    let shown = handles.shown.get();
    if !shown.any() {
        return;
    }
    if shown.cursor {
        HandlePopup(handles.clone(), Handle::Cursor, false);
    }
    if let Some(left) = shown.start {
        HandlePopup(handles.clone(), Handle::SelectionStart, left);
    }
    if let Some(left) = shown.end {
        HandlePopup(handles, Handle::SelectionEnd, left);
    }
}

/// A node of no size in the inner field that follows the field's bounds clipped by its
/// ancestors after each layout, for the field's handles, while they may show.
#[composable]
#[allow(non_snake_case)]
fn VisibleBoundsReader(handles: Rc<TouchHandles>) {
    Layout(
        Modifier.then_custom(VisibleBoundsElement(handles)),
        empty_measure_policy(),
        compose_runtime::callback(|| {}),
    );
}

#[derive(Clone)]
struct VisibleBoundsElement(Rc<TouchHandles>);

struct VisibleBoundsNode(Rc<TouchHandles>);

impl ModifierNodeElement for VisibleBoundsElement {
    type Node = VisibleBoundsNode;
    fn create(&self) -> VisibleBoundsNode {
        VisibleBoundsNode(self.0.clone())
    }
    fn update(&self, node: &mut VisibleBoundsNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    fn name(&self) -> &'static str {
        "selection_handles_visible_bounds"
    }
}

impl ModifierNode for VisibleBoundsNode {
    fn as_global_position_aware(
        &mut self,
    ) -> Option<&mut dyn compose_ui::GlobalPositionAwareModifierNode> {
        Some(self)
    }
    fn on_detach(&mut self, _context: &mut compose_ui::NodeContext<'_>) {
        self.0.visible.set(None);
    }
}

impl compose_ui::GlobalPositionAwareModifierNode for VisibleBoundsNode {
    /// The inner field's bounds in the window, clipped by its ancestors and the window, back in
    /// its coordinates, as Compose's `LayoutCoordinates.visibleBounds()`; the field measures
    /// again when they changed.
    fn on_globally_positioned(
        &mut self,
        coordinates: compose_ui::LayoutCoordinates,
        _context: &mut compose_ui::NodeContext<'_>,
    ) {
        let Some(field) = coordinates.parent_layout_coordinates() else { return };
        let window = field.bounds_in_window();
        let top_left = field.window_to_local(Offset::new(window.left, window.top));
        let bottom_right = field.window_to_local(Offset::new(window.right, window.bottom));
        let visible = Rect::new(top_left.x, top_left.y, bottom_right.x, bottom_right.y);
        if self.0.visible.get() != Some(visible) {
            self.0.visible.set(Some(visible));
            self.0.measure_field();
        }
    }
}

/// One handle's popup, as Compose's `HandlePopup` with its content: not focusable, never
/// dismissed, its node at least 40 dp square.
#[composable]
#[allow(non_snake_case)]
fn HandlePopup(handles: Rc<TouchHandles>, handle: Handle, left: bool) {
    let provider = compose_runtime::remember_keyed((handle, left), || {
        Rc::new(HandlePositionProvider { handles: handles.clone(), handle, left })
            as Rc<dyn PopupPositionProvider>
    });
    let element = HandleElement { handles: handles.clone(), handle, left };
    compose_ui::PopupWithPositionProvider(
        provider,
        None,
        PopupProperties::default(),
        compose_runtime::callback(move || {
            let size = (MIN_TOUCH_TARGET, MIN_TOUCH_TARGET);
            Layout(
                Modifier.required_size(size).then_custom(element.clone()),
                empty_measure_policy(),
                compose_runtime::callback(|| {}),
            );
        }),
    );
}

/// Places a handle's popup on the point it marks, as Compose's `HandlePositionProvider`: a left
/// handle's top right there, a right handle's top left, the cursor handle's top center.
struct HandlePositionProvider {
    handles: Rc<TouchHandles>,
    handle: Handle,
    left: bool,
}

impl PopupPositionProvider for HandlePositionProvider {
    fn position(
        &self,
        anchor: Rect,
        _window: IntSize,
        _direction: LayoutDirection,
        popup: IntSize,
    ) -> IntOffset {
        let point = RuntimeSnapshot::without_read_observation(|| {
            self.handles.positions.with(|positions| positions.anchor[self.handle.index()])
        });
        let adjustment = match (self.handle, self.left) {
            (Handle::Cursor, _) => -(popup.width / 2),
            (_, true) => -popup.width,
            (_, false) => 0,
        };
        IntOffset::new(
            anchor.left.round() as i32 + point.x.round() as i32 + adjustment,
            anchor.top.round() as i32 + point.y.round() as i32,
        )
    }
}

#[derive(Clone)]
struct HandleElement {
    handles: Rc<TouchHandles>,
    handle: Handle,
    left: bool,
}

/// A handle's node: its semantics, its icon, and its drag.
struct HandleNode {
    handles: Rc<TouchHandles>,
    handle: Handle,
    left: bool,
    /// Where the finger went down, in the window, while it's held.
    down: Option<Offset>,
    /// Where the drag counts from once past the touch slop: the press, moved by the slop toward
    /// the finger, as Compose's `awaitTouchSlopOrCancellation` leaves the drag's first delta.
    origin: Option<Offset>,
}

impl ModifierNodeElement for HandleElement {
    type Node = HandleNode;
    fn create(&self) -> HandleNode {
        HandleNode {
            handles: self.handles.clone(),
            handle: self.handle,
            left: self.left,
            down: None,
            origin: None,
        }
    }
    fn update(&self, node: &mut HandleNode) {
        node.handles = self.handles.clone();
        node.handle = self.handle;
        node.left = self.left;
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.handles, &other.handles)
            && self.handle == other.handle
            && self.left == other.left
    }
    fn name(&self) -> &'static str {
        "selection_handle"
    }
}

impl ModifierNode for HandleNode {
    fn as_draw(&mut self) -> Option<&mut dyn DrawModifierNode> {
        Some(self)
    }
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
    fn as_semantics(&mut self) -> Option<&mut dyn SemanticsModifierNode> {
        Some(self)
    }
    fn observes_reads(&self) -> bool {
        true
    }
    fn on_detach(&mut self, _context: &mut compose_ui::NodeContext<'_>) {
        self.end_drag();
    }
}

impl HandleNode {
    fn end_drag(&mut self) {
        self.down = None;
        if self.origin.take().is_some() {
            self.handles.tell_field(None);
        }
    }
}

impl SemanticsModifierNode for HandleNode {
    fn apply_semantics(&self, config: &mut SemanticsConfiguration) {
        let position = self.handles.positions.with(|positions| positions.text[self.handle.index()]);
        let anchor = match (self.handle, self.left) {
            (Handle::Cursor, _) => SelectionHandleAnchor::Middle,
            (_, true) => SelectionHandleAnchor::Left,
            (_, false) => SelectionHandleAnchor::Right,
        };
        config.set(
            &SELECTION_HANDLE_INFO_KEY,
            SelectionHandleInfo { handle: self.handle, position, anchor, visible: true },
        );
    }
}

impl DrawModifierNode for HandleNode {
    /// The handle's shape, as Compose's `createHandleImage`: a circle with the square of its top
    /// left corner filled, so it points up and left from its top left; flipped for a left handle,
    /// turned 45° for the cursor's, at the side or the center of its node where the point is.
    fn draw(&mut self, scope: &mut DrawScope<'_>) {
        let color: Color = scope.current_value_of(&crate::LOCAL_TEXT_SELECTION_COLORS).handle_color;
        let density = scope.density();
        let size = scope.size();
        match self.handle {
            Handle::Cursor => {
                let width = density.to_px(CURSOR_HANDLE_WIDTH);
                let radius = width / 2.0;
                let left = (size.width - width) / 2.0;
                scope.translate(left + radius, 0.0, |scope| {
                    scope.rotate(45.0, Offset::ZERO, |scope| draw_shape(scope, radius, color));
                });
            }
            _ => {
                let edge = density.to_px(HANDLE_SIZE);
                let radius = edge / 2.0;
                if self.left {
                    // Its top right on the point: at the node's right, mirrored.
                    let left = size.width - edge;
                    scope.scale(-1.0, 1.0, Offset::new(left + radius, radius), |scope| {
                        scope.translate(left, 0.0, |scope| draw_shape(scope, radius, color));
                    });
                } else {
                    draw_shape(scope, radius, color);
                }
            }
        }
    }
}

/// A circle of `radius` at (radius, radius) and the square of its top left quarter.
fn draw_shape(scope: &mut DrawScope<'_>, radius: f32, color: Color) {
    scope.draw_rect(Rect::new(0.0, 0.0, radius, radius), Paint::fill(color));
    scope.draw_circle(Offset::new(radius, radius), radius, Paint::fill(color));
}

impl PointerInputModifierNode for HandleNode {
    /// A press, then moves past the touch slop: a drag, each move told the field; a press lifted
    /// before the slop is a tap (the cursor handle's toggles the toolbar).
    fn on_pointer_event(
        &mut self,
        event: &mut PointerEvent,
        pass: PointerEventPass,
        _bounds: IntSize,
        context: &mut EventContext<'_>,
    ) {
        if pass != PointerEventPass::Main {
            return;
        }
        let Some(position) = context.pointer_position() else { return };
        match event.kind {
            PointerEventType::Press if !event.consumed => {
                event.consume();
                self.end_drag();
                self.down = Some(position);
            }
            PointerEventType::Move if event.pressed => {
                let Some(down) = self.down else { return };
                if self.origin.is_none() {
                    let moved = position - down;
                    let distance = moved.x.hypot(moved.y);
                    let slop = context.view_configuration().touch_slop;
                    if distance < slop {
                        return;
                    }
                    let toward = Offset::new(moved.x / distance * slop, moved.y / distance * slop);
                    self.origin = Some(down + toward);
                }
                event.consume();
                let origin = self.origin.expect("past the slop");
                self.handles.tell_field(Some((self.handle, position - origin)));
            }
            PointerEventType::Release => {
                if self.down.is_some() && self.origin.is_none() && self.handle == Handle::Cursor {
                    self.handles.cursor_tapped.set(!self.handles.cursor_tapped.get());
                }
                if self.down.is_some() {
                    event.consume();
                }
                self.end_drag();
            }
            _ => {}
        }
    }

    fn on_cancel_pointer_input(&mut self) {
        self.end_drag();
    }
}
