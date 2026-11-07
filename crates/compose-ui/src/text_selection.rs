//! Selecting non-editable text, as Compose's `SelectionContainer`: a press and drag inside a
//! container selects from where it went down to the pointer, across the texts in it (in
//! order); a double-click selects a word, a triple-click a whole text; Cmd+A (Ctrl+A) selects
//! all and Cmd+C (Ctrl+C) copies, the texts joined by new lines. A click elsewhere in it, or
//! the container losing focus, clears the selection. `DisableSelection` leaves its content out.
//!
//! The texts are `Selectable` modifier nodes (foundation's `BasicText`); the tree finds them
//! below the container when a gesture asks, as it does focus targets.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use compose_runtime::OwnedState;
use smallvec::SmallVec;

use crate::input::{
    EventContext, Key, KeyEvent, KeyEventType, PointerEvent, PointerEventPass, PointerEventType,
};
use crate::modifier::{
    FocusTargetModifierNode, KeyInputModifierNode, Modifier, ModifierNode, ModifierNodeElement,
    PointerInputModifierNode,
};
use crate::node_chain::NodeKinds;
use crate::text::{SharedString, SharedTextMeasurer};
use crate::text_attributes::ResolvedTextDirection;
use crate::tree::{NodeId, UiTree};
use crate::unit::{IntSize, Offset, Rect};

/// A text a `SelectionContainer` can select in, as Compose's `Selectable`: a modifier node
/// returning itself from `ModifierNode::as_selectable`. Offsets are byte offsets at character
/// boundaries; positions are in the node's coordinates.
pub trait Selectable {
    /// Its text: what's selected and copied.
    fn text(&self) -> SharedString;
    /// The offset nearest to `position`, from the measurer (`None` before it's laid out).
    fn offset_at(&self, position: Offset, measurer: &SharedTextMeasurer) -> Option<usize>;
    /// The word at `offset`, as `getWordBoundary` (`platform::word_boundary`): what a
    /// double-click selects.
    fn word_at(&self, offset: usize) -> Range<usize>;
    /// Bumped when its text changes: a selection ending in it is dropped then.
    fn version(&self) -> u64 {
        0
    }
    /// Shows `range` selected (highlighted), or nothing. Returns whether that changed.
    fn set_selection(&mut self, range: Option<Range<usize>>, measurer: &SharedTextMeasurer)
    -> bool;
    /// Where a selection handle at `offset` points, in the node's coordinates, as Compose's
    /// `Selectable.getHandlePosition`: the start handle (`start`) or the end's, the selection
    /// reversed or not (`crossed`). `None` before it's laid out.
    fn handle_position(
        &self,
        _offset: usize,
        _start: bool,
        _crossed: bool,
        _measurer: &SharedTextMeasurer,
    ) -> Option<Offset> {
        None
    }
    /// The direction of the text at `offset`, as `getBidiRunDirection`: what a selection's
    /// anchor there says (`Selection.AnchorInfo.direction`).
    fn direction_at(
        &self,
        _offset: usize,
        _measurer: &SharedTextMeasurer,
    ) -> ResolvedTextDirection {
        ResolvedTextDirection::Ltr
    }
    /// Character `offset`'s box, as `getBoundingBox`: the offset kept within the text (past it,
    /// its last character's); empty for an empty text or before it's laid out.
    fn bounding_box(&self, _offset: usize, _measurer: &SharedTextMeasurer) -> Rect {
        Rect::ZERO
    }
    /// The line around `offset`, as `getRangeOfLineContaining`: from its start to its visible
    /// end, the offset kept to what's shown; empty for an empty text.
    fn range_of_line_containing(
        &self,
        _offset: usize,
        _measurer: &SharedTextMeasurer,
    ) -> Range<usize> {
        0..0
    }
    /// The end of what's shown, as `getLastVisibleOffset`: the last line shown's visible end
    /// (the most lines, or the height, cut the rest).
    fn last_visible_offset(&self, _measurer: &SharedTextMeasurer) -> usize {
        0
    }
    /// The height of the line at `offset`, as `getLineHeight`: none past the lines shown or an
    /// empty text.
    fn line_height(&self, _offset: usize, _measurer: &SharedTextMeasurer) -> f32 {
        0.0
    }
}

/// What a press selects, and a drag after it extends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Unit {
    Character,
    Word,
    /// A whole text.
    Text,
}

/// A selection gesture or command, handled by the tree once the event's dispatch is over.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum SelectionRequest {
    /// A press at `position` (in the root) starts a selection.
    Start {
        container: NodeId,
        position: Offset,
        unit: Unit,
    },
    /// The pointer, dragged to `position`, moves the selection's end there.
    Extend {
        container: NodeId,
        position: Offset,
    },
    SelectAll {
        container: NodeId,
    },
    Copy {
        container: NodeId,
    },
    Clear {
        container: NodeId,
    },
    /// A finger's long press at `position` (in the root) selects the word of the text under it,
    /// as Compose's `SelectionController`'s `longPressDragObserver`; a later press of a multi-tap
    /// selects its word or more (`unit`) at once.
    LongPress {
        container: NodeId,
        position: Offset,
        unit: Unit,
    },
    /// The container's focus or touch mode changed: its handles follow.
    RefreshHandles {
        container: NodeId,
    },
    /// A selection handle of the container is dragged: the finger's movement since the drag
    /// passed the touch slop, or `None` once it ends.
    DragHandle {
        container: NodeId,
        start: bool,
        moved: Option<Offset>,
    },
}

/// Which selection handles a container shows, as the condition Compose's `SelectionContainer`
/// composes them under (touch mode, focus, a selection not trivially collapsed): the direction
/// at each end (`Selection.AnchorInfo.direction`) and whether they're crossed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ContainerHandlesShown {
    pub start_direction: ResolvedTextDirection,
    pub end_direction: ResolvedTextDirection,
    pub crossed: bool,
}

/// What a `SelectionContainer`'s handles share with the tree that holds its selection, as
/// Compose's `SelectionManager`'s `startHandlePosition` and `endHandlePosition`: which show (read
/// by composition), where each points in the container's coordinates (`None`: hidden, as
/// `Offset.Unspecified`), and the drag under way. Foundation's `SelectionContainer` makes one
/// and composes the handles; the tree writes it after each selection change and layout.
#[doc(hidden)]
pub struct ContainerHandles {
    shown: OwnedState<Option<ContainerHandlesShown>>,
    positions: OwnedState<[Option<Offset>; 2]>,
    /// The container's node, once its handles were first refreshed.
    container: Cell<Option<NodeId>>,
    /// The handle dragged (the start's?), where its drag began and where it was when the
    /// selection last followed it, in the container's coordinates, as `handleDragObserver`'s
    /// first and later `dragBeginPosition`.
    drag: Cell<Option<(bool, Offset, Offset)>>,
    /// The point in the texts the selection last followed the drag to (the previous layout's
    /// raw offset, Compose's `givenRawPreviousHandleOffset`), none before the first move.
    drag_point: Cell<Option<Point>>,
    /// What a selection's change vibrates with in touch mode (`LocalHapticFeedback`'s where the
    /// container is composed), as `SelectionManager.hapticFeedBack`.
    haptics: std::cell::RefCell<Option<Rc<dyn crate::platform_services::HapticFeedback>>>,
}

impl ContainerHandles {
    #[allow(clippy::new_without_default)]
    pub fn new() -> ContainerHandles {
        ContainerHandles {
            shown: compose_runtime::mutable_state_of(None),
            positions: compose_runtime::mutable_state_of([None, None]),
            container: Cell::new(None),
            drag: Cell::new(None),
            drag_point: Cell::new(None),
            haptics: std::cell::RefCell::new(None),
        }
    }

    /// What its selection's changes vibrate with in touch mode, as the container composed reads
    /// `LocalHapticFeedback`.
    pub fn set_haptic_feedback(&self, haptics: Rc<dyn crate::platform_services::HapticFeedback>) {
        let mut slot = self.haptics.borrow_mut();
        if !slot.as_ref().is_some_and(|old| Rc::ptr_eq(old, &haptics)) {
            *slot = Some(haptics);
        }
    }

    /// Which handles show (a read of its state).
    pub fn shown(&self) -> Option<ContainerHandlesShown> {
        self.shown.get()
    }

    /// Where the start handle (`start`) or the end's points, in the container's coordinates, or
    /// `None` when it's hidden (a read of its state).
    pub fn position(&self, start: bool) -> Option<Offset> {
        self.positions.with(|positions| positions[if start { 0 } else { 1 }])
    }

    /// A handle's drag: the finger's movement since it passed the touch slop, as
    /// `handleDragObserver`'s `onDrag`.
    pub fn drag(&self, context: &mut EventContext<'_>, start: bool, moved: Offset) {
        let Some(container) = self.container.get() else { return };
        let moved = Some(moved);
        context.requests.selection.push(SelectionRequest::DragHandle { container, start, moved });
    }

    /// The handle's drag ended (lifted, cancelled, or its node gone), as `onStop`: its next drag
    /// begins anew where it points.
    pub fn end_drag(&self) {
        self.drag.set(None);
        self.drag_point.set(None);
    }

    /// The handle dragged, if any (`SelectionManager.draggingHandle`).
    pub fn dragging(&self) -> Option<bool> {
        self.drag.get().map(|(start, _, _)| start)
    }
}

impl compose_runtime::ComposeArg for ContainerHandles {
    fn same(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

/// A point in a container's texts: a text (its node, the chain index of its selectable, and
/// the version of its text) and an offset in it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Point {
    node: NodeId,
    index: usize,
    version: u64,
    offset: usize,
}

/// The selection in a container: from `anchor` (where it started) to `focus` (where it ends).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct TextSelection {
    container: NodeId,
    anchor: Point,
    focus: Point,
    /// What the press selected (a word, a text): a drag keeps it whole.
    initial: (Point, Point),
    unit: Unit,
}

/// Where a selection's container and the texts of its ends were, in the root, with their sizes:
/// what its handles follow, as Compose's `updateHandleOffsets` runs when the container's
/// position or a selected text's coordinates change.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct HandlesPlacedFor([(NodeId, Offset, IntSize); 3]);

/// A text below a container: its node, the chain index of its selectable, its bounds in the
/// root, and its text's version.
#[derive(Clone, Copy, Debug)]
struct Entry {
    node: NodeId,
    index: usize,
    origin: Offset,
    size: IntSize,
    version: u64,
}

impl Entry {
    fn point(&self, offset: usize) -> Point {
        Point { node: self.node, index: self.index, version: self.version, offset }
    }

    fn holds(&self, point: &Point) -> bool {
        self.node == point.node && self.index == point.index && self.version == point.version
    }
}

/// A selection's ends as places in a container's texts: `(order, offset)`, first to last.
type Span = ((usize, usize), (usize, usize));

impl UiTree {
    /// The selectable texts below `container`, in order: its subtree's nodes in placement
    /// order, without what a nested container or a `DisableSelection` modifier holds.
    fn selectables(&mut self, container: NodeId) -> Vec<Entry> {
        let mut entries = Vec::new();
        let mut stack = vec![container];
        while let Some(node) = stack.pop() {
            let Some(value) = self.get(node) else { continue };
            if !value.placed {
                continue;
            }
            if node != container
                && value.chain.iter().any(|entry| {
                    entry.node.downcast_ref::<SelectionContainerNode>().is_some()
                        || entry.node.downcast_ref::<DisableSelectionNode>().is_some()
                })
            {
                continue;
            }
            let mut layer = 0;
            for index in 0..self.node(node).chain.len() {
                let bits = self.node(node).chain[index].kinds;
                let version = self
                    .get_mut(node)
                    .and_then(|value| value.chain[index].selectable().map(|text| text.version()));
                if let Some(version) = version {
                    let origin = self.layer_origin(node, layer);
                    let size = self.node(node).layers[layer].measured;
                    entries.push(Entry { node, index, origin, size, version });
                }
                if bits.any(NodeKinds::LAYOUT) {
                    layer += 1;
                }
            }
            let order = self.placement_order(node);
            let count = order.as_ref().map_or(self.node(node).children.len(), |order| order.len());
            // Pushed last first, so the first placed comes off the stack first.
            for position in (0..count).rev() {
                let index = order.as_ref().map_or(position, |order| order[position] as usize);
                stack.push(self.node(node).children[index].node);
            }
        }
        entries
    }

    fn selectable_text(&mut self, node: NodeId, index: usize) -> SharedString {
        self.get_mut(node)
            .and_then(|value| value.chain[index].selectable().map(|text| text.text()))
            .unwrap_or(SharedString::EMPTY)
    }

    /// The offset nearest to `position` (in the root) in `entry`'s text.
    fn offset_in(&mut self, entry: &Entry, position: Offset) -> usize {
        let local = position - entry.origin;
        self.node_and_measurer(entry.node)
            .and_then(|(value, (measurer, _))| {
                value.chain[entry.index].selectable()?.offset_at(local, measurer)
            })
            .unwrap_or(0)
    }

    /// The point nearest to `position` (in the root), as Compose's selection finds it: in the
    /// text under it; else, on a text's line (a row of texts), in the last text starting
    /// before it; else at the start of the first text below it, or the end of the last.
    fn point_at(&mut self, entries: &[Entry], position: Offset) -> Option<Point> {
        let last = entries.last()?;
        let within = |entry: &Entry| {
            position.y >= entry.origin.y && position.y < entry.origin.y + entry.size.height as f32
        };
        if let Some(entry) = entries.iter().find(|entry| {
            within(entry)
                && position.x >= entry.origin.x
                && position.x < entry.origin.x + entry.size.width as f32
        }) {
            let offset = self.offset_in(entry, position);
            return Some(entry.point(offset));
        }
        let row: Vec<&Entry> = entries.iter().filter(|entry| within(entry)).collect();
        if let Some(first) = row.first() {
            // Past its end (or in a gap after it), or before the first: the offset there on the
            // line beside it, as Compose's `getOffsetForPosition` (that line's end, or start).
            let found = row.iter().rev().find(|entry| entry.origin.x <= position.x);
            let entry = **found.unwrap_or(first);
            let offset = self.offset_in(&entry, position);
            return Some(entry.point(offset));
        }
        if let Some(entry) = entries.iter().find(|entry| position.y < entry.origin.y) {
            return Some(entry.point(0));
        }
        let length = self.selectable_text(last.node, last.index).len();
        Some(last.point(length))
    }

    /// The selection's ends in `entries`' order, or `None` if a text it ends in is gone or
    /// changed (the selection is then dropped, as Compose's).
    fn span(selection: &TextSelection, entries: &[Entry]) -> Option<Span> {
        let place = |point: &Point| {
            entries.iter().position(|entry| entry.holds(point)).map(|order| (order, point.offset))
        };
        let (anchor, focus) = (place(&selection.anchor)?, place(&selection.focus)?);
        Some(if anchor <= focus { (anchor, focus) } else { (focus, anchor) })
    }

    /// The word around `point`.
    fn word(&mut self, point: Point) -> Range<usize> {
        self.get_mut(point.node)
            .and_then(|value| {
                value.chain[point.index].selectable().map(|text| text.word_at(point.offset))
            })
            .unwrap_or(point.offset..point.offset)
    }

    fn order_of(entries: &[Entry], point: &Point) -> Option<(usize, usize)> {
        entries.iter().position(|entry| entry.holds(point)).map(|order| (order, point.offset))
    }

    /// Handles a selection gesture or command; the handles of the containers it touched follow.
    pub(crate) fn handle_selection(&mut self, request: SelectionRequest) {
        let before = self.text_selection;
        let container = request.container();
        self.apply_selection_request(request);
        self.after_selection_change(container, before);
        if let Some(old) =
            before.map(|selection| selection.container).filter(|old| *old != container)
        {
            self.after_selection_change(old, before);
        }
    }

    /// `container`'s handles follow its selection, and a change of it vibrates in touch mode, as
    /// Compose's `TextHandleMove` feedback when a gesture changes the selection or `onRelease`
    /// clears it.
    fn after_selection_change(&mut self, container: NodeId, before: Option<TextSelection>) {
        let of = |selection: Option<TextSelection>| {
            selection
                .filter(|selection| selection.container == container)
                .map(|selection| (selection.anchor, selection.focus))
        };
        if of(before) != of(self.text_selection)
            && let Some(found) = self.container_handles(container).filter(|found| found.touch)
            && let Some(haptics) = found.handles.haptics.borrow().clone()
        {
            haptics.perform_haptic_feedback(
                crate::platform_services::HapticFeedbackType::TextHandleMove,
            );
        }
        self.refresh_handles(container);
    }

    fn apply_selection_request(&mut self, request: SelectionRequest) {
        match request {
            SelectionRequest::Start { container, position, unit } => {
                let entries = self.selectables(container);
                self.clear_other_selection(container);
                let Some(point) = self.point_at(&entries, position) else { return };
                let (anchor, focus) = match unit {
                    Unit::Character => (point, point),
                    Unit::Word => {
                        let word = self.word(point);
                        (Point { offset: word.start, ..point }, Point { offset: word.end, ..point })
                    }
                    Unit::Text => {
                        let length = self.selectable_text(point.node, point.index).len();
                        (Point { offset: 0, ..point }, Point { offset: length, ..point })
                    }
                };
                self.text_selection = Some(TextSelection {
                    container,
                    anchor,
                    focus,
                    initial: (anchor, focus),
                    unit,
                });
                self.show_selection(&entries);
            }
            SelectionRequest::Extend { container, position } => {
                let Some(mut selection) =
                    self.text_selection.filter(|selection| selection.container == container)
                else {
                    return;
                };
                let entries = self.selectables(container);
                let Some(point) = self.point_at(&entries, position) else { return };
                let (Some(start), Some(end), Some(at)) = (
                    Self::order_of(&entries, &selection.initial.0),
                    Self::order_of(&entries, &selection.initial.1),
                    Self::order_of(&entries, &point),
                ) else {
                    self.text_selection = None;
                    self.show_selection(&entries);
                    return;
                };
                // A word selection extends by words, a text selection by texts.
                let snap = |tree: &mut UiTree, forward: bool| match selection.unit {
                    Unit::Character => point,
                    Unit::Word => {
                        let word = tree.word(point);
                        Point { offset: if forward { word.end } else { word.start }, ..point }
                    }
                    Unit::Text => {
                        let length = tree.selectable_text(point.node, point.index).len();
                        Point { offset: if forward { length } else { 0 }, ..point }
                    }
                };
                // From the side of what the press selected away from the pointer.
                (selection.anchor, selection.focus) = if at >= start {
                    let focus = snap(self, true);
                    let focus_at = (at.0, focus.offset);
                    (selection.initial.0, if focus_at >= end { focus } else { selection.initial.1 })
                } else {
                    (selection.initial.1, snap(self, false))
                };
                self.text_selection = Some(selection);
                self.show_selection(&entries);
                self.bring_dragged_into_view(container, &entries, position, point);
            }
            SelectionRequest::SelectAll { container } => {
                let entries = self.selectables(container);
                let (Some(first), Some(last)) = (entries.first(), entries.last()) else { return };
                let length = self.selectable_text(last.node, last.index).len();
                let (anchor, focus) = (first.point(0), last.point(length));
                self.text_selection = Some(TextSelection {
                    container,
                    anchor,
                    focus,
                    initial: (anchor, anchor),
                    unit: Unit::Character,
                });
                self.show_selection(&entries);
            }
            SelectionRequest::Copy { container } => {
                if self.text_selection.is_some_and(|selection| selection.container == container)
                    && let Some(text) = self.selected_text()
                {
                    self.clipboard.set_text(&text);
                }
            }
            SelectionRequest::Clear { container } => {
                if self.text_selection.is_some_and(|selection| selection.container == container) {
                    let entries = self.selectables(container);
                    self.text_selection = None;
                    self.show_selection(&entries);
                }
            }
            SelectionRequest::LongPress { container, position, unit } => {
                // On a text only: Compose's long press is each text's (`SelectionController`).
                let entries = self.selectables(container);
                let on_text = entries.iter().any(|entry| {
                    let local = position - entry.origin;
                    local.x >= 0.0
                        && local.y >= 0.0
                        && local.x < entry.size.width as f32
                        && local.y < entry.size.height as f32
                });
                if on_text {
                    self.apply_selection_request(SelectionRequest::Start {
                        container,
                        position,
                        unit,
                    });
                }
            }
            SelectionRequest::RefreshHandles { .. } => {}
            SelectionRequest::DragHandle { container, start, moved } => {
                self.drag_handle(container, start, moved)
            }
        }
    }

    /// A handle of `container` dragged by `moved` since its drag began (`None`: it ended), as
    /// Compose's `handleDragObserver`: the drag begins where the handle points, a pixel up
    /// (`getAdjustedCoordinates`), and its end follows the point under the finger by
    /// `CharacterWithWordAccelerate` across the texts (`accelerated`), never collapsing the
    /// selection.
    fn drag_handle(&mut self, container: NodeId, start: bool, moved: Option<Offset>) {
        let Some(found) = self.container_handles(container) else { return };
        let handles = found.handles;
        let Some(moved) = moved else {
            handles.end_drag();
            return;
        };
        let Some(mut selection) =
            self.text_selection.filter(|selection| selection.container == container)
        else {
            return;
        };
        let (begin, previous, previous_point) = match handles.drag.get() {
            Some((dragged, begin, previous)) if dragged == start => {
                (begin, previous, handles.drag_point.get())
            }
            _ => {
                let pointed =
                    compose_runtime::snapshots::Snapshot::without_read_observation(|| {
                        handles.position(start)
                    });
                // A hidden handle isn't dragged, as Compose's `onDown`.
                let Some(pointed) = pointed else { return };
                let begin = Offset::new(pointed.x, pointed.y - 1.0);
                (begin, begin, None)
            }
        };
        let origin = self.layer_origin(container, found.layer);
        let entries = self.selectables(container);
        let target = origin + begin + moved;
        let Some(point) = self.point_at(&entries, target) else { return };
        // The selection follows the finger when its place in the texts changed since it last
        // did (the texts may have moved under a still finger), as Compose's
        // `shouldRecomputeSelection`; the place it was then is the next move's previous one.
        if previous_point == Some(point) {
            handles.drag.set(Some((start, begin, previous)));
            self.bring_dragged_into_view(container, &entries, target, point);
            return;
        }
        handles.drag.set(Some((start, begin, begin + moved)));
        handles.drag_point.set(Some(point));
        let (end, other) = if start {
            (selection.anchor, selection.focus)
        } else {
            (selection.focus, selection.anchor)
        };
        let previous = (origin + previous, previous_point);
        let moved_to = self.accelerated(&entries, start, point, target, previous, end, other);
        // Never collapsed by a handle, as Compose's `ensureAtLeastOneChar` keeps a character.
        if moved_to != other {
            if start {
                selection.anchor = moved_to;
            } else {
                selection.focus = moved_to;
            }
        }
        selection.initial = (selection.anchor, selection.focus);
        selection.unit = Unit::Character;
        self.text_selection = Some(selection);
        self.show_selection(&entries);
        self.bring_dragged_into_view(container, &entries, target, point);
    }

    /// Scrolls what's around the text under a drag toward the finger, as Compose's
    /// `bringDraggedPositionIntoView` (`isSelectionAutoScrollEnabled`): the text whose point
    /// the drag at `position` (in the root) is at asks its scrolling ancestors for the drag's
    /// place, in touch mode with the box of the character there (Compose's cursor at the
    /// handle's offset), stretched to the nearest point of the text's bounds, so a list scrolls
    /// a little each move, never past what it composes next.
    fn bring_dragged_into_view(
        &mut self,
        container: NodeId,
        entries: &[Entry],
        position: Offset,
        point: Point,
    ) {
        let Some(entry) = entries.iter().find(|entry| entry.holds(&point)).copied() else {
            return;
        };
        let local = position - entry.origin;
        let mut rect = Rect::new(local.x, local.y, local.x, local.y);
        if self.container_in_touch_mode(container) {
            let edge = self
                .node_and_measurer(entry.node)
                .and_then(|(value, (measurer, _))| {
                    let text = value.chain[entry.index].selectable()?;
                    Some(text.bounding_box(point.offset, measurer))
                })
                .unwrap_or(rect);
            rect = union(rect, edge);
        }
        let (width, height) = (entry.size.width as f32, entry.size.height as f32);
        let nearest = Offset::new(local.x.clamp(0.0, width), local.y.clamp(0.0, height));
        rect = union(rect, Rect::new(nearest.x, nearest.y, nearest.x, nearest.y));
        self.bring_entry_rect_into_view(entry.node, entry.index, rect);
    }

    /// Whether `container`'s node is in touch mode (its last pointer a finger's).
    fn container_in_touch_mode(&mut self, container: NodeId) -> bool {
        let Some(value) = self.get_mut(container) else { return false };
        value.chain.iter_mut().find_map(|entry| {
            entry.node.downcast_mut::<SelectionContainerNode>().map(|node| node.touch == Some(true))
        }) == Some(true)
    }

    /// Pins the pinnable containers (a lazy layout's items) of the texts `selected` holds a
    /// part of, and lets go of those of the texts among `entries` it no longer does, as
    /// Compose's `updatePinnedSelectables`: a selected text scrolled away stays composed. A
    /// pinned text not among `entries` (not placed) keeps its pin while a selection exists.
    fn update_selection_pins(&mut self, entries: &[Entry], selected: &[NodeId]) {
        if self.text_selection.is_none() {
            self.release_selection_pins();
            return;
        }
        let mut index = 0;
        while index < self.selection_pins.len() {
            let node = self.selection_pins[index].0;
            let shown = entries.iter().any(|entry| entry.node == node);
            if shown && !selected.contains(&node) {
                self.selection_pins.swap_remove(index).1.release();
            } else {
                index += 1;
            }
        }
        for &node in selected {
            if self.selection_pins.iter().any(|(pinned, _)| *pinned == node) {
                continue;
            }
            let container = self.node(node).locals.get(&crate::pinnable::LOCAL_PINNABLE_CONTAINER);
            if let Some(container) = container {
                self.selection_pins.push((node, container.pin()));
            }
        }
    }

    /// Lets go of every selected text's pin.
    pub(crate) fn release_selection_pins(&mut self) {
        for (_, handle) in self.selection_pins.drain(..) {
            handle.release();
        }
    }

    /// Where a handle's drag moves its end (`end`, the other being `other`) when the finger is at
    /// `target` (in the root), over `point`, having been at `previous`, as Compose's
    /// `SelectionAdjustment.CharacterWithWordAccelerate`: between texts, the point; into another
    /// text or onto another line, the word under it (`snapped`); shrinking, the point; growing
    /// from a word's boundary, by words, and from inside a word, by characters.
    #[allow(clippy::too_many_arguments)]
    fn accelerated(
        &mut self,
        entries: &[Entry],
        start: bool,
        point: Point,
        target: Offset,
        previous: (Offset, Option<Point>),
        end: Point,
        other: Point,
    ) -> Point {
        let Some(entry) = entries.iter().find(|entry| entry.holds(&point)).copied() else {
            return point;
        };
        let inside = |entry: &Entry| {
            let local = target - entry.origin;
            local.x >= 0.0
                && local.y >= 0.0
                && local.x < entry.size.width as f32
                && local.y < entry.size.height as f32
        };
        // Between texts (none under the finger): the point, as `updateSelectionBoundary`.
        if !entries.iter().any(inside) {
            return point;
        }
        let order = |at: &Point| Self::order_of(entries, at);
        let crossed =
            if start { order(&point) > order(&other) } else { order(&other) > order(&point) };
        // The other end in this text, or this text's edge toward it (`rawEndHandleOffset`).
        let other_raw = if entry.holds(&other) {
            other.offset
        } else if order(&other) > order(&point) {
            self.selectable_text(point.node, point.index).len()
        } else {
            0
        };
        let line = |tree: &mut UiTree, offset: usize| tree.line_of(&entry, offset);
        let snapped = |tree: &mut UiTree| {
            // The word under the finger, kept to its line (a word broken across lines).
            let word = tree.word(point);
            let current = line(tree, point.offset);
            let word_start = if line(tree, word.start).start == current.start {
                word.start
            } else {
                current.start
            };
            let word_end =
                if line(tree, word.end).start == current.start { word.end } else { current.end };
            let offset = if word_start == other_raw {
                word_end
            } else if word_end == other_raw {
                word_start
            } else if start != crossed {
                if point.offset <= word_end { word_start } else { word_end }
            } else if point.offset >= word_start {
                word_end
            } else {
                word_start
            };
            Point { offset, ..point }
        };
        if !entry.holds(&end) {
            // Into another text: by words.
            return snapped(self);
        }
        // Where the finger was in this text when the selection last followed it, as the previous
        // layout says it, else as its place there now says it.
        let previous_raw = match previous {
            (_, Some(point)) if entry.holds(&point) => point.offset,
            (position, _) => self.offset_in(&entry, position),
        };
        if point.offset == previous_raw {
            return end;
        }
        if line(self, previous_raw).start != line(self, point.offset).start {
            return snapped(self);
        }
        let expanding = if start != crossed {
            point.offset < previous_raw
        } else {
            point.offset > previous_raw
        };
        if !expanding {
            return point;
        }
        let word = self.word(end);
        if end.offset == word.start || end.offset == word.end { snapped(self) } else { point }
    }

    /// The line around `offset` in `entry`'s text, as `getLineForOffset` and its bounds.
    fn line_of(&mut self, entry: &Entry, offset: usize) -> Range<usize> {
        self.node_and_measurer(entry.node)
            .and_then(|(value, (measurer, _))| {
                let text = value.chain[entry.index].selectable()?;
                Some(text.range_of_line_containing(offset, measurer))
            })
            .unwrap_or(offset..offset)
    }

    /// `container`'s node, if it has handles: they, its touch mode and focus, its chain index
    /// and layer.
    fn container_handles(&mut self, container: NodeId) -> Option<FoundHandles> {
        let value = self.get(container)?;
        let mut layer = 0;
        for (index, entry) in value.chain.iter().enumerate() {
            if let Some(node) = entry.node.downcast_ref::<SelectionContainerNode>() {
                let handles = node.handles.clone()?;
                let touch = node.touch.unwrap_or_else(|| {
                    self.input_mode.input_mode() == crate::input_mode::InputMode::Touch
                });
                return Some(FoundHandles { handles, touch, focused: node.focused, index, layer });
            }
            if entry.kinds.any(NodeKinds::LAYOUT) {
                layer += 1;
            }
        }
        None
    }

    /// Tells `container`'s handles which show and where, as Compose's `SelectionContainer`'s
    /// condition and `SelectionManager.updateHandleOffsets`: in touch mode, with the focus, over
    /// a selection not trivially collapsed; each where its text says (`getHandlePosition`), in
    /// the container's coordinates, hidden outside its visible bounds unless it's dragged.
    /// Nothing for a container without handles.
    pub(crate) fn refresh_handles(&mut self, container: NodeId) {
        self.handles_placed_for = self.text_selection.and_then(|it| self.handles_placed(&it));
        let Some(found) = self.container_handles(container) else { return };
        let handles = found.handles.clone();
        handles.container.set(Some(container));
        let selection = self.text_selection.filter(|selection| {
            selection.container == container && selection.anchor != selection.focus
        });
        let mut shown = None;
        let mut positions = [None, None];
        if let Some(selection) = selection.filter(|_| found.touch && found.focused) {
            let entries = self.selectables(container);
            if let (Some(anchor), Some(focus)) = (
                Self::order_of(&entries, &selection.anchor),
                Self::order_of(&entries, &selection.focus),
            ) {
                let crossed = anchor > focus;
                let origin = self.layer_origin(container, found.layer);
                // Compose's `visibleBounds()`: clipped by the clips around it and the window.
                let clipped = self.coordinate_values(container, found.layer, found.index);
                let window = self.window_bounds_of(container);
                let visible = crate::layout_callbacks::intersect(clipped.clipped_bounds, window)
                    .translate(-clipped.position_in_root);
                let dragging = handles.dragging();
                let mut directions = [ResolvedTextDirection::Ltr; 2];
                for (slot, (point, start)) in
                    [(selection.anchor, true), (selection.focus, false)].into_iter().enumerate()
                {
                    let Some(entry) = entries.iter().find(|entry| entry.holds(&point)) else {
                        continue;
                    };
                    let asked =
                        self.node_and_measurer(entry.node).and_then(|(value, (measurer, _))| {
                            let text = value.chain[entry.index].selectable()?;
                            let position =
                                text.handle_position(point.offset, start, crossed, measurer);
                            Some((position, text.direction_at(point.offset, measurer)))
                        });
                    let Some((position, direction)) = asked else { continue };
                    directions[slot] = direction;
                    let Some(position) = position else { continue };
                    let position = position + entry.origin - origin;
                    if dragging == Some(start) || contains_inclusive(visible, position) {
                        positions[slot] = Some(position);
                    }
                }
                shown = Some(ContainerHandlesShown {
                    start_direction: directions[0],
                    end_direction: directions[1],
                    crossed,
                });
            }
        }
        let (was_shown, was_at) =
            compose_runtime::snapshots::Snapshot::without_read_observation(|| {
                (handles.shown.get(), handles.positions.get())
            });
        if was_shown != shown {
            handles.shown.set(shown);
        }
        if was_at != positions {
            handles.positions.set(positions);
        }
    }

    /// After a layout or graphics layers moving (`moved`), tells `selection`'s handles where they
    /// point if its container or the texts of its ends moved or changed size, as Compose's
    /// container updates them when its position changes and a selected text when its
    /// coordinates do; else nothing.
    pub(crate) fn refresh_handles_if_moved(&mut self, selection: &TextSelection, moved: &[NodeId]) {
        let nodes = [selection.container, selection.anchor.node, selection.focus.node];
        let layer_moved =
            moved.iter().any(|&layer| nodes.iter().any(|&node| self.is_at_or_below(node, layer)));
        if layer_moved || self.handles_placed(selection) != self.handles_placed_for {
            self.refresh_handles(selection.container);
        }
    }

    /// Where `selection`'s container and the texts of its ends are now, in the root, with their
    /// sizes (graphics layers left out: their moves are told apart).
    fn handles_placed(&self, selection: &TextSelection) -> Option<HandlesPlacedFor> {
        let at = |node: NodeId| {
            let value = self.get(node)?;
            Some((node, self.layer_origin(node, 0), value.layers.first()?.size))
        };
        Some(HandlesPlacedFor([
            at(selection.container)?,
            at(selection.anchor.node)?,
            at(selection.focus.node)?,
        ]))
    }

    /// A selection started in another container clears this one's.
    fn clear_other_selection(&mut self, container: NodeId) {
        if let Some(old) = self.text_selection
            && old.container != container
        {
            self.text_selection = None;
            if self.get(old.container).is_some() {
                let entries = self.selectables(old.container);
                self.show_selection(&entries);
            }
        }
    }

    /// Tells each text of `entries` its part of the selection (dropping a selection whose
    /// texts are gone or changed).
    fn show_selection(&mut self, entries: &[Entry]) {
        let span = self.text_selection.and_then(|selection| Self::span(&selection, entries));
        if span.is_none() {
            self.text_selection = None;
        }
        let mut selected: SmallVec<[NodeId; 8]> = SmallVec::new();
        for (order, entry) in entries.iter().enumerate() {
            let range = span.and_then(|span| range_of(span, order));
            let text = self.selectable_text(entry.node, entry.index);
            let range = range.map(|range| clamp(&text, range));
            if range.as_ref().is_some_and(|range| range.start < range.end) {
                selected.push(entry.node);
            }
            let changed =
                self.node_and_measurer(entry.node).is_some_and(|(value, (measurer, _))| {
                    value.chain[entry.index]
                        .selectable()
                        .is_some_and(|text| text.set_selection(range, measurer))
                });
            if changed {
                self.invalidate_draw_of(entry.node);
            }
        }
        if !selected.is_empty() || !self.selection_pins.is_empty() {
            self.update_selection_pins(entries, &selected);
        }
    }

    /// The bounds of what's selected in `container`, in its coordinates, as Compose's
    /// `getSelectedRegionRect` (where the text toolbar goes): for each text the selection covers
    /// a part of, its first and last selected characters' boxes, all together. `None` without a
    /// selection there, or with a collapsed one.
    #[doc(hidden)]
    pub fn selected_region_rect(&mut self, container: NodeId) -> Option<Rect> {
        let selection = self.text_selection.filter(|selection| selection.container == container)?;
        let mut layer = 0;
        for entry in self.get(container)?.chain.iter() {
            if entry.node.downcast_ref::<SelectionContainerNode>().is_some() {
                break;
            }
            if entry.kinds.any(NodeKinds::LAYOUT) {
                layer += 1;
            }
        }
        let origin = self.layer_origin(container, layer);
        let entries = self.selectables(container);
        let span = Self::span(&selection, &entries)?;
        let mut region: Option<Rect> = None;
        for (order, entry) in entries.iter().enumerate() {
            let Some(range) = range_of(span, order) else { continue };
            let text = self.selectable_text(entry.node, entry.index);
            let range = clamp(&text, range);
            if range.is_empty() {
                continue;
            }
            let last = text[..range.end].char_indices().next_back().map_or(0, |(at, _)| at);
            let Some((value, (measurer, _))) = self.node_and_measurer(entry.node) else { continue };
            let Some(selectable) = value.chain[entry.index].selectable() else { continue };
            for offset in [range.start, last] {
                let found =
                    selectable.bounding_box(offset, measurer).translate(entry.origin - origin);
                region = Some(match region {
                    Some(region) => Rect::new(
                        region.left.min(found.left),
                        region.top.min(found.top),
                        region.right.max(found.right),
                        region.bottom.max(found.bottom),
                    ),
                    None => found,
                });
            }
        }
        region
    }

    /// Runs `ask` with the `Selectable` of `node` (its first), if it has one. For tests.
    #[doc(hidden)]
    pub fn with_selectable(
        &mut self,
        node: NodeId,
        ask: &mut dyn FnMut(&mut dyn Selectable, &SharedTextMeasurer),
    ) {
        let Some((value, (measurer, _))) = self.node_and_measurer(node) else { return };
        for entry in value.chain.iter_mut() {
            if let Some(selectable) = entry.selectable() {
                ask(selectable, measurer);
                return;
            }
        }
    }

    /// Where the selection starts and ends: each end's text node and offset, its start handle's
    /// first, as Compose's `Selection.start` and `end` (`None` without a selection). For tests.
    #[doc(hidden)]
    pub fn selection_anchors(&self) -> Option<((NodeId, usize), (NodeId, usize))> {
        let selection = self.text_selection?;
        let (anchor, focus) = (selection.anchor, selection.focus);
        Some(((anchor.node, anchor.offset), (focus.node, focus.offset)))
    }

    /// The text selected in a container, if any (what Cmd+C copies there): `None` when nothing
    /// is, or a text it ends in is gone or changed.
    pub fn selected_text(&mut self) -> Option<String> {
        let selection =
            self.text_selection.filter(|selection| selection.anchor != selection.focus)?;
        let entries = self.selectables(selection.container);
        let Some(span) = Self::span(&selection, &entries) else {
            self.text_selection = None;
            self.show_selection(&entries);
            return None;
        };
        let mut parts = Vec::new();
        for (order, entry) in entries.iter().enumerate() {
            if let Some(range) = range_of(span, order) {
                let text = self.selectable_text(entry.node, entry.index);
                parts.push(text[clamp(&text, range)].to_string());
            }
        }
        Some(parts.join("\n"))
    }
}

/// A container's handles, as `container_handles` finds them.
struct FoundHandles {
    handles: Rc<ContainerHandles>,
    touch: bool,
    focused: bool,
    index: usize,
    layer: usize,
}

/// `rect` holds `point`, its edges included, as Compose's `containsInclusive`.
fn contains_inclusive(rect: Rect, point: Offset) -> bool {
    point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

impl SelectionRequest {
    fn container(&self) -> NodeId {
        match *self {
            SelectionRequest::Start { container, .. }
            | SelectionRequest::Extend { container, .. }
            | SelectionRequest::SelectAll { container }
            | SelectionRequest::Copy { container }
            | SelectionRequest::Clear { container }
            | SelectionRequest::LongPress { container, .. }
            | SelectionRequest::RefreshHandles { container }
            | SelectionRequest::DragHandle { container, .. } => container,
        }
    }
}

/// The part of the text at `order` a selection spanning `span` covers, if any.
fn range_of(span: Span, order: usize) -> Option<Range<usize>> {
    let (start, end) = span;
    if start == end || order < start.0 || order > end.0 {
        return None;
    }
    let from = if order == start.0 { start.1 } else { 0 };
    let to = if order == end.0 { end.1 } else { usize::MAX };
    Some(from..to)
}

/// The smallest rectangle holding both, as Compose's `Rect.union`.
fn union(a: Rect, b: Rect) -> Rect {
    Rect::new(a.left.min(b.left), a.top.min(b.top), a.right.max(b.right), a.bottom.max(b.bottom))
}

/// `range` within `text`, at character boundaries.
fn clamp(text: &str, range: Range<usize>) -> Range<usize> {
    let floor = |offset: usize| {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    floor(range.start)..floor(range.end)
}

// ---- The container's node ------------------------------------------------------------------

/// A press within this long of the last, and this close to it, is a double- or triple-click.
const MULTI_CLICK_NANOS: u64 = 500_000_000;
const MULTI_CLICK_DISTANCE: f32 = 8.0;

#[derive(Clone)]
struct SelectionContainerElement(Option<Rc<ContainerHandles>>);

/// `SelectionContainer`'s node: it turns presses, drags and keys into selection requests, and
/// takes the focus a press gives it (Cmd+C reaches it). A mouse selects as it presses and drags;
/// a finger's long press selects a word and its drag extends by words, a finger's tap clears
/// the selection (Compose's `SelectionController` and `onClearSelectionRequested`).
pub(crate) struct SelectionContainerNode {
    /// A press is selecting.
    dragging: bool,
    focused: bool,
    /// When and where the last press was, and how many came in a row.
    last_press: Option<(u64, Offset, u32)>,
    /// The handles foundation's `SelectionContainer` composes, if any.
    handles: Option<Rc<ContainerHandles>>,
    /// The last pointer was a finger's (or a stylus'), as `SelectionManager.isInTouchMode`
    /// (`updateSelectionTouchMode`); the host's input mode until a pointer comes.
    touch: Option<bool>,
    /// A finger's press: where it went down, its long-press timer, whether it became one.
    finger: Option<FingerPress>,
    /// The last finger's down, when (the event's time) and where, and how many came in a row,
    /// as Compose's `ClicksCounter`.
    last_down: Option<(u64, Offset, u32)>,
}

struct FingerPress {
    /// The finger pressing: a long press's drag follows it alone, as Compose's
    /// `drag(pointerId)`.
    pointer: crate::input::PointerId,
    at: Offset,
    job: Option<compose_runtime::Job>,
    /// A long press, or a later press of a multi-tap: its moves drag the selection.
    long: bool,
    /// A later press of a multi-tap whose finger hasn't passed the touch slop yet, nor been held
    /// past the long-press timeout (Compose's `touchSelectionSubsequentPress`).
    slop_pending: bool,
    /// What the drag leaves out of the finger's place: the touch slop a later press passed
    /// (Compose's over-slop).
    slop: Offset,
}

impl SelectionContainerNode {
    fn end_finger(&mut self) {
        if let Some(job) = self.finger.take().and_then(|finger| finger.job) {
            job.cancel();
        }
    }
}

impl ModifierNodeElement for SelectionContainerElement {
    type Node = SelectionContainerNode;
    fn create(&self) -> SelectionContainerNode {
        SelectionContainerNode {
            dragging: false,
            focused: false,
            last_press: None,
            handles: self.0.clone(),
            touch: None,
            finger: None,
            last_down: None,
        }
    }
    fn update(&self, node: &mut SelectionContainerNode) {
        node.handles = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
    fn name(&self) -> &'static str {
        "selection_container"
    }
}

impl ModifierNode for SelectionContainerNode {
    /// Reused: no press selecting, no focus, no click run (the tree drops its selection).
    fn on_reset(&mut self, _context: &mut crate::node_context::NodeContext<'_>) {
        self.dragging = false;
        self.focused = false;
        self.last_press = None;
        self.touch = None;
        self.last_down = None;
        self.end_finger();
    }
    fn on_detach(&mut self, _context: &mut crate::node_context::NodeContext<'_>) {
        self.end_finger();
    }
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
    fn as_focus_target(&mut self) -> Option<&mut dyn FocusTargetModifierNode> {
        Some(self)
    }
    fn as_key_input(&mut self) -> Option<&mut dyn KeyInputModifierNode> {
        Some(self)
    }
}

impl PointerInputModifierNode for SelectionContainerNode {
    fn on_pointer_event(
        &mut self,
        event: &mut PointerEvent,
        pass: PointerEventPass,
        _bounds: IntSize,
        context: &mut EventContext<'_>,
    ) {
        if pass == PointerEventPass::Initial {
            // A long press's drag is the selection's before what's inside sees it, as Compose's
            // texts take it ahead of the scrollables around them (a lazy column's items).
            let dragging = self.finger.as_ref().filter(|finger| {
                finger.long && !finger.slop_pending && finger.pointer == event.pointer
            });
            if let Some(finger) = dragging
                && event.kind == PointerEventType::Move
                && event.pressed
                && !event.consumed
            {
                let Some(position) = context.pointer_position() else { return };
                let position = position - finger.slop;
                event.consume();
                let container = context.node;
                context.requests.selection.push(SelectionRequest::Extend { container, position });
            }
            return;
        }
        if pass != PointerEventPass::Main {
            return;
        }
        let container = context.node;
        let Some(position) = context.pointer_position() else { return };
        // As Compose's `updateSelectionTouchMode`: each event says whether it's a finger's.
        let touch = event.pointer_type != crate::input::PointerType::Mouse;
        if self.touch != Some(touch) {
            self.touch = Some(touch);
            if self.handles.is_some() {
                context.requests.selection.push(SelectionRequest::RefreshHandles { container });
            }
        }
        if touch {
            self.on_finger_event(event, position, context);
            return;
        }
        match event.kind {
            PointerEventType::Press => {
                if event.consumed {
                    // A clickable inside took it: the selection goes, as Compose's tap.
                    context.requests.selection.push(SelectionRequest::Clear { container });
                    return;
                }
                let now = compose_runtime::platform::now_nanos();
                let clicks = match self.last_press {
                    Some((at, where_, count))
                        if now.saturating_sub(at) <= MULTI_CLICK_NANOS
                            && (where_ - position).x.hypot((where_ - position).y)
                                <= MULTI_CLICK_DISTANCE =>
                    {
                        count + 1
                    }
                    _ => 1,
                };
                self.last_press = Some((now, position, clicks));
                let unit = match clicks {
                    1 => Unit::Character,
                    2 => Unit::Word,
                    _ => Unit::Text,
                };
                self.dragging = true;
                context.request_focus();
                context.requests.selection.push(SelectionRequest::Start {
                    container,
                    position,
                    unit,
                });
            }
            PointerEventType::Move if self.dragging && event.pressed && !event.consumed => {
                event.consume();
                context.requests.selection.push(SelectionRequest::Extend { container, position });
            }
            PointerEventType::Release => self.dragging = false,
            _ => {}
        }
    }

    fn on_cancel_pointer_input(&mut self) {
        self.dragging = false;
        self.end_finger();
    }
}

impl SelectionContainerNode {
    /// A finger's event, as Compose's `touchSelectionFirstPress` over the texts and the
    /// container's `onClearSelectionRequested`: held past the long-press timeout without moving
    /// past the touch slop, it selects the word under it and its drag extends by words; lifted
    /// before, it's a tap, which clears the selection.
    fn on_finger_event(
        &mut self,
        event: &mut PointerEvent,
        position: Offset,
        context: &mut EventContext<'_>,
    ) {
        let container = context.node;
        // Another finger of the gesture under way isn't its.
        if self.finger.as_ref().is_some_and(|finger| finger.pointer != event.pointer) {
            return;
        }
        match event.kind {
            PointerEventType::Press => {
                self.end_finger();
                self.dragging = false;
                if event.consumed {
                    context.requests.selection.push(SelectionRequest::Clear { container });
                    return;
                }
                // As Compose's `ClicksCounter`: a down soon after the last and near it is the
                // next of a multi-tap.
                let configuration = context.view_configuration();
                let clicks = match self.last_down {
                    Some((time, at, clicks))
                        if event.time_nanos.saturating_sub(time)
                            < configuration.double_tap_timeout_millis * 1_000_000
                            && (position - at).x.hypot((position - at).y)
                                < configuration.touch_slop =>
                    {
                        clicks + 1
                    }
                    _ => 1,
                };
                self.last_down = Some((event.time_nanos, position, clicks));
                let later = clicks > 1;
                if later {
                    // Compose's `touchSelectionSubsequentPress`: its word (or its text, for a
                    // third tap) at once, on a text; dragged once past the slop or held.
                    let unit = if clicks == 2 { Unit::Word } else { Unit::Text };
                    context.request_focus();
                    context.requests.selection.push(SelectionRequest::LongPress {
                        container,
                        position,
                        unit,
                    });
                }
                let handle = context.handle();
                let job = handle.coroutine_scope().map(|scope| {
                    let timeout = configuration.long_press_timeout_millis;
                    scope.launch(async move {
                        compose_runtime::delay(std::time::Duration::from_millis(timeout)).await;
                        let key = handle.tree_key();
                        crate::node_context::with_free_tree(key, &mut |tree| {
                            tree.container_long_pressed(container);
                        });
                    })
                });
                let pointer = event.pointer;
                self.finger = Some(FingerPress {
                    pointer,
                    at: position,
                    job,
                    long: later,
                    slop_pending: later,
                    slop: Offset::ZERO,
                });
            }
            PointerEventType::Move if event.pressed => {
                let Some(finger) = &mut self.finger else { return };
                if finger.slop_pending {
                    // A later press's drag starts past the slop, the slop left out of it.
                    let moved = position - finger.at;
                    let distance = moved.x.hypot(moved.y);
                    let slop = context.view_configuration().touch_slop;
                    if !event.consumed && distance > slop {
                        finger.slop_pending = false;
                        finger.slop = moved * (slop / distance);
                        if let Some(job) = finger.job.take() {
                            job.cancel();
                        }
                        event.consume();
                        let position = position - finger.slop;
                        context
                            .requests
                            .selection
                            .push(SelectionRequest::Extend { container, position });
                    }
                } else if !finger.long {
                    // A long press's moves were taken in the initial pass.
                    let moved = position - finger.at;
                    let slop = context.view_configuration().touch_slop;
                    if event.consumed || moved.x.hypot(moved.y) > slop {
                        self.end_finger();
                    }
                }
            }
            PointerEventType::Release => {
                let Some(finger) = self.finger.take() else { return };
                if let Some(job) = finger.job {
                    job.cancel();
                }
                if finger.long {
                    // The long press's up is its, as Compose's drag consumes it.
                    event.consume();
                } else if !event.consumed {
                    context.requests.selection.push(SelectionRequest::Clear { container });
                }
            }
            _ => {}
        }
    }
}

impl UiTree {
    /// A finger held still on `container` past the long-press timeout: its press becomes a long
    /// press, selecting the word under it.
    fn container_long_pressed(&mut self, container: NodeId) {
        let Some(value) = self.get_mut(container) else { return };
        let mut at = None;
        for entry in value.chain.iter_mut() {
            if let Some(node) = entry.node.downcast_mut::<SelectionContainerNode>() {
                if let Some(finger) = node.finger.as_mut() {
                    finger.job = None;
                    if finger.long {
                        // A later press held past the timeout drags without the slop, as
                        // Compose's `DownResolution.Timeout`.
                        finger.slop_pending = false;
                    } else {
                        finger.long = true;
                        at = Some(finger.at);
                    }
                }
                break;
            }
        }
        if let Some(position) = at {
            let unit = Unit::Word;
            self.handle_selection(SelectionRequest::LongPress { container, position, unit });
            // The container takes the focus as its selection starts (`startSelection`).
            if self.text_selection.is_some_and(|selection| selection.container == container) {
                let direction = crate::focus_manager::FocusDirection::Enter;
                self.request_focus_at_or_below(container, direction);
            }
        }
    }
}

impl FocusTargetModifierNode for SelectionContainerNode {
    fn on_focus_changed(
        &mut self,
        _previous: crate::focus::FocusState,
        current: crate::focus::FocusState,
        context: &mut EventContext<'_>,
    ) {
        let focused = current.has_focus();
        if self.focused && !focused {
            context.requests.selection.push(SelectionRequest::Clear { container: context.node });
        } else if !self.focused && focused && self.handles.is_some() {
            let container = context.node;
            context.requests.selection.push(SelectionRequest::RefreshHandles { container });
        }
        self.focused = focused;
    }
}

impl KeyInputModifierNode for SelectionContainerNode {
    fn on_key_event(&mut self, event: &KeyEvent, context: &mut EventContext<'_>) -> bool {
        let shortcut =
            if cfg!(target_os = "macos") { event.modifiers.meta } else { event.modifiers.ctrl };
        if event.kind != KeyEventType::KeyDown || !shortcut {
            return false;
        }
        let container = context.node;
        let request = match event.key {
            Key::Character(c) if c.eq_ignore_ascii_case(&'c') => {
                SelectionRequest::Copy { container }
            }
            Key::Character(c) if c.eq_ignore_ascii_case(&'a') => {
                SelectionRequest::SelectAll { container }
            }
            _ => return false,
        };
        context.requests.selection.push(request);
        true
    }
}

/// `DisableSelection`'s node: a mark the container's search stops at.
pub(crate) struct DisableSelectionNode;

struct DisableSelectionElement;

impl ModifierNodeElement for DisableSelectionElement {
    type Node = DisableSelectionNode;
    fn create(&self) -> DisableSelectionNode {
        DisableSelectionNode
    }
    fn update(&self, _node: &mut DisableSelectionNode) {}
    fn same(&self, _other: &Self) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        "disable_selection"
    }
}

impl ModifierNode for DisableSelectionNode {}

impl Modifier {
    /// Makes this node's texts selectable, as `SelectionContainer` does for its content.
    pub fn selection_container(self) -> Modifier {
        self.then_custom(SelectionContainerElement(None))
    }

    /// A selection container whose handles foundation's `SelectionContainer` composes, told
    /// through `handles` (`SelectionManager`'s handle states).
    #[doc(hidden)]
    pub fn selection_container_with_handles(self, handles: Rc<ContainerHandles>) -> Modifier {
        self.then_custom(SelectionContainerElement(Some(handles)))
    }

    /// Leaves this node's texts out of a selection container, as `DisableSelection`.
    pub fn disable_selection(self) -> Modifier {
        self.then_custom(DisableSelectionElement)
    }
}

impl TextSelection {
    /// The container it's in.
    pub(crate) fn container(&self) -> NodeId {
        self.container
    }
}
