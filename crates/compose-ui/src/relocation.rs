//! Bringing things into view through the tree, as Compose's `BringIntoViewModifierNode` chain:
//! a request goes up from the node asking (a requester, a focused node) through the nodes around
//! it, innermost first. Each responding node says at once where what it shows will be once it has
//! shown it (`calculate_rect_for_parent`), what the ones outside are asked to show, and gives a
//! future that ends once it has shown it or was interrupted (`bring_child_into_view`); the
//! request's future waits for all of them, as Compose's responders run alongside each other.
//!
//! What a responder shows is given by [`BringIntoViewBounds`], read again whenever it's asked, as
//! Compose's `() -> Rect?`: the requester's bounds where they are now, through the rects the
//! responders inside give their parents. Nodes that answer at once (a lazy list jumping to an item,
//! `ModifierNode::bring_into_view`) are asked with where it is during the walk.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use compose_runtime::CompositionLocalAnchor;
use smallvec::SmallVec;

use crate::nested_scroll::LocalBoxFuture;
use crate::node_chain::{EntryId, NodeKinds};
use crate::node_context::NodeHandle;
use crate::tree::{NodeId, UiTree};
use crate::unit::{IntSize, Offset, Rect};

/// A node taking part in bringing what's inside it into view, as Compose's
/// `BringIntoViewResponder` behind `BringIntoViewModifierNode`: scrolling containers, and
/// `Modifier::bring_into_view_responder`. compose-rs's tree passes the request on to the parents
/// itself, each told where the ones inside said (Compose's node calls its parent).
pub trait BringIntoViewResponderNode {
    /// Where `rect` (in the coordinates of the layer this node is in, `scope.viewport()` its
    /// size) will be once this node has shown it: what its parents are asked to show, as
    /// `calculateRectForParent`.
    fn calculate_rect_for_parent(&self, rect: Rect, scope: &BringIntoViewScope) -> Rect;

    /// Shows what `bounds` gives (read it again as the request goes on: it moves; `rect` is
    /// what it gives now, `parent` what `calculate_rect_for_parent` said of it), as
    /// `bringChildIntoView`: the future ends once it's shown, or the request was interrupted;
    /// `None` when there's nothing to wait for (it shows already). Dropped, the request is
    /// withdrawn.
    fn bring_child_into_view(
        &mut self,
        rect: Rect,
        parent: Rect,
        bounds: BringIntoViewBounds,
        scope: &BringIntoViewScope,
    ) -> Option<LocalBoxFuture<'static, ()>>;
}

/// Where a responder is asked: its layer's size, the composition locals where its layout node
/// is composed, and a handle on its chain entry (its coroutine scope).
#[derive(Clone, Copy)]
pub struct BringIntoViewScope {
    viewport: IntSize,
    locals: CompositionLocalAnchor,
    handle: NodeHandle,
}

impl BringIntoViewScope {
    /// The size of the layer the responder is in: a scrolling container's viewport.
    pub fn viewport(&self) -> IntSize {
        self.viewport
    }

    /// Where the responder's layout node reads composition locals (a container's
    /// `LOCAL_BRING_INTO_VIEW_SPEC`), as `currentValueOf`.
    pub fn locals(&self) -> CompositionLocalAnchor {
        self.locals
    }

    /// A handle on the responder's chain entry: its coroutine scope, cancelled once it's detached.
    pub fn handle(&self) -> NodeHandle {
        self.handle
    }
}

/// What a responder is to show, read again each time, as Compose's `localRect: () -> Rect?`:
/// the requester's bounds where they are now, through the rects the responders inside give their
/// parents, in the responder's coordinates. `None` once something on the way is detached.
/// Clones read the same.
#[derive(Clone)]
pub struct BringIntoViewBounds(Rc<BoundsPath>);

impl BringIntoViewBounds {
    /// The rect now, in the responder's coordinates, as `localRect()`: `None` once the requester
    /// or a responder on the way is detached (or the tree is busy).
    pub fn rect(&self) -> Option<Rect> {
        self.current().map(|(rect, _)| rect)
    }

    /// The rect now and the size of the responder's layer.
    pub fn current(&self) -> Option<(Rect, IntSize)> {
        let mut current = None;
        crate::node_context::peek_tree(self.0.tree, &mut |tree| {
            current = tree.bounds_along(&self.0);
        });
        current
    }
}

/// What a chain entry does when asked to show a rect.
pub(crate) enum Answer<'a> {
    /// It responds over time.
    Responder(&'a mut dyn BringIntoViewResponderNode),
    /// It showed it at once; where it is for the ones outside.
    Shown(Rect),
    /// Not a node that shows things.
    Nothing,
}

/// Where the requester's bounds start: a node's bounds (a focused node), or a rect in the layer
/// of a chain entry (a requester's, or where a responder answering at once left it).
#[derive(Clone)]
enum Base {
    Node { node: NodeId, rect: Option<Rect> },
    Entry { node: NodeId, entry: EntryId, rect: Option<Rect> },
}

/// From the base, through the responders `via` (innermost first), to the responder `at`.
struct BoundsPath {
    tree: u32,
    base: Base,
    via: Vec<(NodeId, EntryId)>,
    at: (NodeId, EntryId),
}

fn moved(rect: Rect, by: Offset) -> Rect {
    Rect::new(rect.left + by.x, rect.top + by.y, rect.right + by.x, rect.bottom + by.y)
}

impl UiTree {
    /// Asks the scrolling (and bring-into-view responding) nodes around chain entry `entry` of
    /// `node` to show `rect`, in the coordinates of the layer the entry is in (`None`: all of
    /// that layer), as `BringIntoViewModifierNode.bringIntoView`: the entries before it in its own
    /// chain, then each ancestor's, innermost first, each told where the ones inside it will put
    /// it. With no entry, the node's ancestors are asked, about its bounds (or `rect` in them).
    /// Returns what the responders give to wait for.
    pub(crate) fn bring_into_view_from(
        &mut self,
        node: NodeId,
        entry: Option<usize>,
        rect: Option<Rect>,
    ) -> Walked {
        let (mut base, first) = match entry {
            Some(index) => {
                let entry = self.node(node).chain[index].id;
                (Base::Entry { node, entry, rect }, index)
            }
            None => (Base::Node { node, rect }, 0),
        };
        let mut walked = Walked::default();
        let mut via: Vec<(NodeId, EntryId)> = Vec::new();
        // Where it is in the root, as the walk goes.
        let Some(mut current) = self.base_rect(&base) else { return walked };
        // The nodes walked, innermost first: an overlay's containers are its own, so the ones
        // around its host don't show it.
        let mut path: SmallVec<[NodeId; 32]> = SmallVec::new();
        let mut at = Some(node);
        while let Some(id) = at {
            path.push(id);
            let value = self.node(id);
            at = if value.overlay { None } else { value.parent };
        }
        // Where each node's first layer starts in the root, from the outermost down, as
        // `layer_origin` finds it: a walk's worth once, not each entry's.
        let mut starts: SmallVec<[Offset; 32]> = SmallVec::from_elem(Offset::ZERO, path.len());
        for (depth, &id) in path.iter().enumerate().rev() {
            let value = self.node(id);
            starts[depth] = match value.parent {
                _ if value.overlay => value.overlay_origin,
                Some(parent) if depth + 1 < path.len() => {
                    let outer = self.node(parent);
                    let skip = usize::from(outer.overlay);
                    outer.layers.iter().skip(skip).fold(starts[depth + 1], |origin, layer| {
                        origin + Offset::new(layer.offset.x as f32, layer.offset.y as f32)
                    })
                }
                Some(parent) => self.layer_origin(parent, self.node(parent).layers.len() - 1),
                None => Offset::ZERO,
            };
        }
        for (depth, &id) in path.iter().enumerate() {
            let end = if depth == 0 { first } else { self.node(id).chain.len() };
            if end == 0 {
                continue;
            }
            let value = self.node(id);
            let skip = usize::from(value.overlay);
            // Each entry's layer: the layout entries before it, counted down as the walk goes
            // from the chain's end.
            let mut layer = value.chain[..end]
                .iter()
                .filter(|entry| entry.kinds.any(NodeKinds::LAYOUT))
                .count();
            for index in (0..end).rev() {
                let value = self.node(id);
                if value.chain[index].kinds.any(NodeKinds::LAYOUT) {
                    layer -= 1;
                }
                let layer = layer.min(value.layers.len() - 1);
                let origin = value.layers[..=layer].iter().skip(skip).fold(
                    starts[depth],
                    |origin, layer| {
                        origin + Offset::new(layer.offset.x as f32, layer.offset.y as f32)
                    },
                );
                self.ask_to_bring_into_view(
                    id,
                    index,
                    layer,
                    origin,
                    &mut base,
                    &mut via,
                    &mut current,
                    &mut walked,
                );
            }
        }
        walked
    }

    /// Asks chain entry `index` of `id` (in layer `layer`, at `origin` in the root) to show
    /// `current` (in the root), which becomes where it will then be: a responder is given bounds
    /// to read and asked where it will put them; a node answering at once
    /// (`ModifierNode::bring_into_view`) is asked with where it is, and becomes the base of what
    /// the ones outside read.
    #[allow(clippy::too_many_arguments)]
    fn ask_to_bring_into_view(
        &mut self,
        id: NodeId,
        index: usize,
        layer: usize,
        origin: Offset,
        base: &mut Base,
        via: &mut Vec<(NodeId, EntryId)>,
        current: &mut Rect,
        walked: &mut Walked,
    ) {
        let value = self.node(id);
        let viewport = value.layers[layer].measured;
        let locals = value.locals;
        let entry = value.chain[index].id;
        let local = moved(*current, Offset::new(-origin.x, -origin.y));
        let key = self.key;
        // Its handle, registered below if it's given a request to keep.
        let handle = crate::node_context::NodeHandle::of_entry(key, id, entry);
        let Some(value) = self.get_mut(id) else { return };
        match value.chain[index].bring_into_view_answer(local, viewport, locals) {
            Answer::Responder(responder) => {
                walked.answered = true;
                let scope = BringIntoViewScope { viewport, locals, handle };
                let bounds = BringIntoViewBounds(Rc::new(BoundsPath {
                    tree: key,
                    base: base.clone(),
                    via: via.clone(),
                    at: (id, entry),
                }));
                let parent = responder.calculate_rect_for_parent(local, &scope);
                if let Some(future) = responder.bring_child_into_view(local, parent, bounds, &scope)
                {
                    walked.futures.push(future);
                    crate::node_context::entry_handle(self, id, index);
                }
                via.push((id, entry));
                *current = moved(parent, origin);
            }
            Answer::Shown(shown) => {
                walked.answered = true;
                *current = moved(shown, origin);
                *base = Base::Entry { node: id, entry, rect: Some(shown) };
                via.clear();
            }
            Answer::Nothing => {}
        }
    }

    /// Chain entry `entry` of `node`: its index and the layer it's in, if it's there.
    fn entry_place(&self, node: NodeId, entry: EntryId) -> Option<(usize, usize)> {
        let value = self.get(node)?;
        let index = value.chain.iter().position(|it| it.id == entry)?;
        Some((index, value.layer_of(index).min(value.layers.len().checked_sub(1)?)))
    }

    /// Where `base` is in the root now.
    fn base_rect(&self, base: &Base) -> Option<Rect> {
        match *base {
            Base::Node { node, rect } => {
                self.get(node)?;
                let bounds = self.bounds_in_root(node);
                Some(match rect {
                    Some(rect) => moved(rect, Offset::new(bounds.left, bounds.top)),
                    None => bounds,
                })
            }
            Base::Entry { node, entry, rect } => {
                let (_, layer) = self.entry_place(node, entry)?;
                let origin = self.layer_origin(node, layer);
                let size = self.node(node).layers[layer].measured;
                let local =
                    rect.unwrap_or(Rect::new(0.0, 0.0, size.width as f32, size.height as f32));
                Some(moved(local, origin))
            }
        }
    }

    /// What `path` reads now: the rect in the responder's coordinates and its layer's size.
    fn bounds_along(&mut self, path: &BoundsPath) -> Option<(Rect, IntSize)> {
        let mut rect = self.base_rect(&path.base)?;
        for &(node, entry) in &path.via {
            let (index, layer) = self.entry_place(node, entry)?;
            let origin = self.layer_origin(node, layer);
            let viewport = self.node(node).layers[layer].measured;
            let locals = self.node(node).locals;
            let handle = crate::node_context::entry_handle(self, node, index);
            let scope = BringIntoViewScope { viewport, locals, handle };
            let local = moved(rect, Offset::new(-origin.x, -origin.y));
            let responder = self.node_mut(node).chain[index].as_bring_into_view_responder()?;
            rect = moved(responder.calculate_rect_for_parent(local, &scope), origin);
        }
        let (node, entry) = path.at;
        let (_, layer) = self.entry_place(node, entry)?;
        let origin = self.layer_origin(node, layer);
        let viewport = self.node(node).layers[layer].measured;
        Some((moved(rect, Offset::new(-origin.x, -origin.y)), viewport))
    }

    /// Asks the scrolling ancestors of `node` to show it, innermost first, each shown where
    /// the scrolling inside it will put it. What they animate runs as a task of its own.
    pub fn bring_into_view(&mut self, node: NodeId) {
        self.bring_rect_into_view(node, None);
    }

    /// The same for `rect` within `node` (its own coordinates; `None`: all of it).
    pub fn bring_rect_into_view(&mut self, node: NodeId, rect: Option<Rect>) {
        self.bring_into_view_from(node, None, rect).launch();
    }

    /// `bring_into_view`, saying whether a node around answered: one that didn't asked nothing
    /// of any.
    pub(crate) fn bring_node_into_view(&mut self, node: NodeId) -> bool {
        self.bring_into_view_from(node, None, None).launch()
    }

    /// The same for `rect` in the layer of chain entry `entry` of `node`, asking the entries
    /// before it in its chain first.
    pub(crate) fn bring_entry_rect_into_view(
        &mut self,
        node: NodeId,
        entry: usize,
        rect: Rect,
    ) -> bool {
        self.bring_into_view_from(node, Some(entry), Some(rect)).launch()
    }
}

/// What a walk bringing into view found: the futures the responders give to wait for, and
/// whether any node answered (none: the walk asked nothing of anything).
#[derive(Default)]
pub(crate) struct Walked {
    pub(crate) futures: Vec<LocalBoxFuture<'static, ()>>,
    pub(crate) answered: bool,
}

impl Walked {
    /// Runs what the responders animate as a task of its own; returns whether a node answered.
    fn launch(self) -> bool {
        if !self.futures.is_empty() {
            compose_runtime::platform::launch(join_all(self.futures));
        }
        self.answered
    }
}

impl crate::node_chain::ChainEntry {
    /// What its node or delegate does when asked to show `rect`.
    pub(crate) fn bring_into_view_answer(
        &mut self,
        rect: Rect,
        viewport: IntSize,
        locals: CompositionLocalAnchor,
    ) -> Answer<'_> {
        if self.kinds.any(NodeKinds::DELEGATING) {
            return crate::delegation::bring_into_view_answer_of(
                &mut *self.node,
                rect,
                viewport,
                locals,
            );
        }
        if self.node.as_bring_into_view_responder().is_some() {
            return self
                .node
                .as_bring_into_view_responder()
                .map_or(Answer::Nothing, Answer::Responder);
        }
        match self.node.bring_into_view_in(rect, viewport, locals) {
            Some(shown) => Answer::Shown(shown),
            None => Answer::Nothing,
        }
    }

    /// Its node or delegate that responds to bringing into view, as a `BringIntoViewModifierNode`.
    pub(crate) fn as_bring_into_view_responder(
        &mut self,
    ) -> Option<&mut dyn BringIntoViewResponderNode> {
        if self.kinds.any(NodeKinds::DELEGATING) {
            crate::delegation::responder_of(&mut *self.node)
        } else {
            self.node.as_bring_into_view_responder()
        }
    }
}

/// Waits for all of `futures`, polling each, as Compose's `coroutineScope { launch … }`.
pub(crate) fn join_all(futures: Vec<LocalBoxFuture<'static, ()>>) -> JoinAll {
    JoinAll(futures.into_iter().map(Some).collect())
}

pub(crate) struct JoinAll(Vec<Option<LocalBoxFuture<'static, ()>>>);

impl Future for JoinAll {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let mut pending = false;
        for slot in self.0.iter_mut() {
            if let Some(future) = slot {
                if future.as_mut().poll(context).is_ready() {
                    *slot = None;
                } else {
                    pending = true;
                }
            }
        }
        if pending { Poll::Pending } else { Poll::Ready(()) }
    }
}
