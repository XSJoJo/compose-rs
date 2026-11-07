//! Nested scrolling, as Compose's `androidx.compose.ui.input.nestedscroll`: a
//! `NestedScrollConnection` given to `Modifier.nested_scroll` takes part in the scrolls of the
//! scrollables inside it, before them (`on_pre_scroll`, `on_pre_fling`) and after them
//! (`on_post_scroll`, `on_post_fling`).
//!
//! A scrollable finds the connections around it from the pointer's hit path: each nested-scroll
//! node on it adds its connection as an event starts (the initial pass runs from the outside
//! in), and the scrollable takes the ones before it when a gesture or a wheel turn reaches it
//! ([`NestedScrollChain::current`]), then dispatches through them: the outermost first before
//! the scroll, the innermost first after.
//!
//! The nested-scroll nodes are also linked through the tree, each to the nearest one above it,
//! as Compose's `NestedScrollNode`s: a [`NestedScrollDispatcher`] given to
//! `Modifier.nested_scroll_with` dispatches from its node through them, from any code
//! (docs/parity/design/nested-scroll-dispatcher.md, D1917).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use smallvec::SmallVec;

use crate::input::{EventContext, PointerEvent, PointerEventPass, PointerEventType};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, PointerInputModifierNode};
use crate::node_context::{NodeContext, NodeHandle};
use crate::traversal::{TraversableNode, TraverseKey};
use crate::unit::{IntSize, Offset};

/// Where a scroll comes from, as Compose's `NestedScrollSource`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NestedScrollSource {
    /// A drag or a wheel.
    UserInput,
    /// A fling or an animation.
    SideEffect,
}

/// A boxed future borrowing for `'a`, as the object-safe suspending functions of the traits here
/// and in foundation's scrolling return.
pub type LocalBoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;

/// What takes part in the scrolls of the scrollables inside it, as Compose's
/// `NestedScrollConnection`. Offsets and velocities are in pixels (per second) in the pointer's
/// direction: positive towards the bottom and the right (a finger moving down, content
/// scrolling back to its top). Each returns what it consumed. The fling callbacks suspend, as
/// Compose's: the fling's end waits for them, and what they leave goes on to the next.
pub trait NestedScrollConnection {
    /// Before the scrollable scrolls by `available`, as `onPreScroll`.
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        let _ = (available, source);
        Offset::ZERO
    }

    /// After the scrollable (and the connections inside this one) consumed `consumed`, with
    /// `available` left, as `onPostScroll`.
    fn on_post_scroll(
        &self,
        consumed: Offset,
        available: Offset,
        source: NestedScrollSource,
    ) -> Offset {
        let _ = (consumed, available, source);
        Offset::ZERO
    }

    /// Before the scrollable flings at `available`, as `onPreFling`.
    fn on_pre_fling(&self, available: Offset) -> LocalBoxFuture<'_, Offset> {
        let _ = available;
        Box::pin(async { Offset::ZERO })
    }

    /// After the scrollable's fling (or its drag's release, without one), as `onPostFling`.
    fn on_post_fling(&self, consumed: Offset, available: Offset) -> LocalBoxFuture<'_, Offset> {
        let _ = (consumed, available);
        Box::pin(async { Offset::ZERO })
    }
}

thread_local! {
    /// The connections the pointer event being dispatched has passed, outermost first.
    static PASSED: RefCell<Vec<Rc<dyn NestedScrollConnection>>> = const { RefCell::new(Vec::new()) };
}

/// A pointer event's dispatch starts: no connection passed yet.
pub(crate) fn begin_pointer_event() {
    PASSED.with(|passed| passed.borrow_mut().clear());
}

/// The connections around a scrollable, outermost first, as Compose's
/// `NestedScrollDispatcher` reaches them through the parents of its node.
#[derive(Clone, Default)]
pub struct NestedScrollChain {
    connections: Rc<[Rc<dyn NestedScrollConnection>]>,
}

impl NestedScrollChain {
    /// The connections around the node handling the pointer event, during its initial pass:
    /// those the event passed before reaching it.
    pub fn current() -> NestedScrollChain {
        let connections: Vec<_> = PASSED.with(|passed| passed.borrow().clone());
        NestedScrollChain { connections: connections.into() }
    }

    /// Adds `connection` for the nodes inside the one handling the pointer event, during its
    /// initial pass: a scrollable, a nested-scroll parent of the scrollables inside it, as
    /// Compose's `ScrollingLogic`.
    pub fn enter(connection: Rc<dyn NestedScrollConnection>) {
        PASSED.with(|passed| passed.borrow_mut().push(connection));
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Offers `available` before scrolling, the outermost connection first (each gets what the
    /// ones around it left); returns what they consumed, as `dispatchPreScroll`.
    pub fn pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        let mut consumed = Offset::ZERO;
        for connection in self.connections.iter() {
            let used = connection.on_pre_scroll(available - consumed, source);
            consumed = consumed + used;
        }
        consumed
    }

    /// Tells what was scrolled and offers what's left, the innermost connection first; returns
    /// what they consumed, as `dispatchPostScroll`.
    pub fn post_scroll(
        &self,
        consumed: Offset,
        available: Offset,
        source: NestedScrollSource,
    ) -> Offset {
        let mut taken = Offset::ZERO;
        for connection in self.connections.iter().rev() {
            let used = connection.on_post_scroll(consumed + taken, available - taken, source);
            taken = taken + used;
        }
        taken
    }

    /// Offers a fling's velocity before it starts, the outermost first, as `dispatchPreFling`.
    pub async fn pre_fling(&self, available: Offset) -> Offset {
        let mut consumed = Offset::ZERO;
        for connection in self.connections.iter() {
            consumed = consumed + connection.on_pre_fling(available - consumed).await;
        }
        consumed
    }

    /// Tells a fling's end, the innermost first, as `dispatchPostFling`.
    pub async fn post_fling(&self, consumed: Offset, available: Offset) -> Offset {
        let mut taken = Offset::ZERO;
        for connection in self.connections.iter().rev() {
            taken = taken + connection.on_post_fling(consumed + taken, available - taken).await;
        }
        taken
    }

    /// The connections of `links`, given innermost first.
    fn of_links(links: &[Rc<Link>]) -> NestedScrollChain {
        let connections: Vec<_> = links.iter().rev().map(|link| link.connection()).collect();
        NestedScrollChain { connections: connections.into() }
    }
}

thread_local! {
    /// Counts the nested-scroll nodes attached and detached on this thread: a link resolved
    /// before the last of them is resolved again at its next use.
    static EPOCH: Cell<u64> = const { Cell::new(1) };
    /// A scope that's never active, for a dispatcher without a node, as Compose's
    /// `CancelledScope`.
    static CANCELLED: compose_runtime::CoroutineScope = {
        let scope = compose_runtime::CoroutineScope::new();
        scope.cancel();
        *scope
    };
}

fn next_epoch() -> u64 {
    EPOCH.with(|epoch| {
        epoch.set(epoch.get() + 1);
        epoch.get()
    })
}

/// What traversals find the nested-scroll nodes by.
static NESTED_SCROLL: TraverseKey = TraverseKey::new("NestedScrollNode");

/// A nested-scroll node as the nodes inside it and its dispatcher reach it: its connection and
/// the node above it, shared out of the tree so a dispatch needs no tree.
struct Link {
    connection: RefCell<Rc<dyn NestedScrollConnection>>,
    /// The nearest nested-scroll node above, as last resolved.
    parent: RefCell<Option<Rc<Link>>>,
    /// The epoch `parent` was resolved in.
    epoch: Cell<u64>,
    /// The node's handle, while it's attached.
    handle: Cell<Option<NodeHandle>>,
    /// The nearest attached nested-scroll node above when the node was detached, as Compose's
    /// `lastKnownParentNode`.
    last_parent: RefCell<Option<Rc<Link>>>,
}

impl Link {
    fn connection(&self) -> Rc<dyn NestedScrollConnection> {
        self.connection.borrow().clone()
    }

    fn is_attached(&self) -> bool {
        self.handle.get().is_some()
    }

    /// The nearest attached nested-scroll node above, as `parentNestedScrollNode`: resolved
    /// through the tree again if one was attached or detached since, when the tree is free;
    /// otherwise the last one found, past those that left.
    fn parent(&self) -> Option<Rc<Link>> {
        let handle = self.handle.get()?;
        let epoch = EPOCH.with(Cell::get);
        if self.epoch.get() != epoch
            && let Some(found) = with_context(handle, parent_link)
        {
            *self.parent.borrow_mut() = found;
            self.epoch.set(epoch);
        }
        let mut parent = self.parent.borrow().clone();
        while let Some(link) = parent.as_ref().filter(|link| !link.is_attached()) {
            let above = link.last_parent.borrow().clone();
            parent = above;
        }
        parent
    }

    /// Where a fling's end goes on from this node, as `NestedScrollNode.onPostFling`: its parent,
    /// or the last one it knew once it's detached.
    fn post_fling_parent(&self) -> Option<Rc<Link>> {
        if self.is_attached() { self.parent() } else { self.last_parent.borrow().clone() }
    }

    /// The outermost active scope of the nested-scroll nodes from this one up, as
    /// `nestedCoroutineScope`; `None` once detached.
    fn nested_scope(&self) -> Option<compose_runtime::CoroutineScope> {
        let parent = self.parent().and_then(|parent| parent.nested_scope());
        match parent {
            Some(scope) if scope.is_active() => Some(scope),
            _ => self.handle.get()?.coroutine_scope(),
        }
    }
}

/// The link of the nearest nested-scroll node above the one of `context`: a `NestedScrollNode`,
/// or a node taking part as one (a scrollable, `ModifierNode::as_nested_scroll_participant`).
fn parent_link(context: &mut NodeContext<'_>) -> Option<Rc<Link>> {
    let mut found = None;
    context.visit_ancestors(
        crate::node_chain::NodeKinds::TRAVERSABLE,
        true,
        &mut |node, context| {
            found = match node.downcast_mut::<NestedScrollNode>() {
                Some(node) => Some(node.link.clone()),
                None => node.as_nested_scroll_participant(context).map(|it| it.link),
            };
            if found.is_some() {
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        },
    );
    found
}

/// Runs `f` with the context of `handle`'s node now, if the tree is free.
fn with_context<R>(handle: NodeHandle, f: impl FnOnce(&mut NodeContext<'_>) -> R) -> Option<R> {
    let mut f = Some(f);
    let mut result = None;
    let (node, entry) = handle.place();
    crate::node_context::with_free_tree(handle.tree_key(), &mut |tree| {
        let Some(index) = tree.entry_index(node, entry) else { return };
        let Some(f) = f.take() else { return };
        result = Some(tree.with_entry_or_delegate(node, index, entry, |_, context| f(context)));
    });
    result
}

/// A node taking part in nested scrolling as a `NestedScrollNode` would, without being one: a
/// scrollable, made when a nested-scroll node inside it looks for its parent (D1918). Its node
/// returns it from `ModifierNode::as_nested_scroll_participant` and tells it when it's attached
/// and detached.
#[doc(hidden)]
#[derive(Clone)]
pub struct NestedScrollParticipant {
    link: Rc<Link>,
}

impl NestedScrollParticipant {
    /// A participant for the node of `context`, which is attached, with `connection`.
    pub fn new(
        context: &mut NodeContext<'_>,
        connection: Rc<dyn NestedScrollConnection>,
    ) -> NestedScrollParticipant {
        let link = Rc::new(Link {
            connection: RefCell::new(connection),
            parent: RefCell::new(None),
            epoch: Cell::new(0),
            handle: Cell::new(None),
            last_parent: RefCell::new(None),
        });
        let participant = NestedScrollParticipant { link };
        participant.on_attach(context);
        participant
    }

    /// Its node is attached (again): its parent is found at its next use.
    pub fn on_attach(&self, context: &mut NodeContext<'_>) {
        self.link.handle.set(Some(context.handle()));
        self.link.epoch.set(0);
        *self.link.last_parent.borrow_mut() = None;
        next_epoch();
    }

    /// Its node is detached.
    pub fn on_detach(&self, context: &mut NodeContext<'_>) {
        *self.link.last_parent.borrow_mut() = parent_link(context);
        self.link.handle.set(None);
        next_epoch();
    }

    /// The connections above it, outermost first.
    pub fn chain(&self) -> NestedScrollChain {
        NestedScrollChain::of_links(&links_from(self.link.parent()))
    }
}

impl NestedScrollChain {
    /// The connections above `handle`'s node, outermost first, found through the tree: `None`
    /// if the tree is busy or the node gone. What a scrollable dispatches the scrolls through
    /// that no pointer started (D1918).
    #[doc(hidden)]
    pub fn above(handle: NodeHandle) -> Option<NestedScrollChain> {
        let parent = with_context(handle, parent_link)?;
        Some(NestedScrollChain::of_links(&links_from(parent)))
    }
}

/// `start` and the links above it, innermost first.
fn links_from(start: Option<Rc<Link>>) -> SmallVec<[Rc<Link>; 4]> {
    let mut links = SmallVec::new();
    let mut next = start;
    while let Some(link) = next {
        next = link.post_fling_parent();
        links.push(link);
    }
    links
}

/// How a node that isn't a pointer gesture takes part in nested scrolling, as Compose's
/// `NestedScrollDispatcher`: given to [`Modifier::nested_scroll_with`] (or
/// [`nested_scroll_modifier_node`]), it dispatches through the nested-scroll nodes above that
/// node, never to the node's own connection. A clone is the same dispatcher. Velocities are
/// `Offset`s, as the connection's (pixels per second).
#[derive(Clone, Default)]
pub struct NestedScrollDispatcher {
    inner: Rc<DispatcherInner>,
}

#[derive(Default)]
struct DispatcherInner {
    /// The node it's given to, while that's attached.
    node: RefCell<Option<Rc<Link>>>,
    /// The node above its node when that was detached, for a fling's end, as Compose's
    /// `lastKnownParentNode`.
    last_known_parent: RefCell<Option<Rc<Link>>>,
}

impl PartialEq for NestedScrollDispatcher {
    fn eq(&self, other: &NestedScrollDispatcher) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl std::fmt::Debug for NestedScrollDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NestedScrollDispatcher")
    }
}

impl NestedScrollDispatcher {
    pub fn new() -> NestedScrollDispatcher {
        NestedScrollDispatcher::default()
    }

    /// The connections above its node, outermost first, as a chain to dispatch through (none
    /// without a node): what a scrollable dispatches through when no pointer started a scroll.
    pub fn chain(&self) -> NestedScrollChain {
        let parent = self.inner.node.borrow().as_ref().and_then(|node| node.parent());
        NestedScrollChain::of_links(&links_from(parent))
    }

    /// Offers `available` to the nodes above before scrolling, the outermost first; returns
    /// what they consumed, as `dispatchPreScroll`.
    pub fn dispatch_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        self.chain().pre_scroll(available, source)
    }

    /// Tells the nodes above what this node consumed and offers what's left, the innermost
    /// first; returns what they consumed, as `dispatchPostScroll`.
    pub fn dispatch_post_scroll(
        &self,
        consumed: Offset,
        available: Offset,
        source: NestedScrollSource,
    ) -> Offset {
        self.chain().post_scroll(consumed, available, source)
    }

    /// Offers a fling's velocity to the nodes above before it starts, the outermost first, as
    /// `dispatchPreFling`.
    pub async fn dispatch_pre_fling(&self, available: Offset) -> Offset {
        self.chain().pre_fling(available).await
    }

    /// Tells the nodes above a fling's end, the innermost first, as `dispatchPostFling`. Its node
    /// gone, it goes to the node that was above it, so a fling started before it left ends its
    /// cycle.
    pub async fn dispatch_post_fling(&self, consumed: Offset, available: Offset) -> Offset {
        let node = self.inner.node.borrow().clone();
        let parent = node.as_ref().and_then(|node| node.parent());
        let links = match parent {
            Some(parent) => links_from(Some(parent)),
            None => links_from(self.inner.last_known_parent.borrow().clone()),
        };
        NestedScrollChain::of_links(&links).post_fling(consumed, available).await
    }

    /// Where to launch a fling's dispatch so it outlives its node, as `coroutineScope`: the
    /// outermost active scope of the nested-scroll nodes from its node up. Never active
    /// without a node.
    pub fn coroutine_scope(&self) -> compose_runtime::CoroutineScope {
        let node = self.inner.node.borrow().clone();
        node.and_then(|node| node.nested_scope()).unwrap_or_else(|| CANCELLED.with(|scope| *scope))
    }

    /// Whether its node is `link`'s.
    fn is_on(&self, link: &Rc<Link>) -> bool {
        self.inner.node.borrow().as_ref().is_some_and(|node| Rc::ptr_eq(node, link))
    }
}

impl Modifier {
    /// Lets `connection` take part in the scrolls of the scrollables inside, as
    /// `Modifier.nestedScroll(connection)`.
    pub fn nested_scroll(self, connection: Rc<dyn NestedScrollConnection>) -> Modifier {
        self.nested_scroll_with(connection, None)
    }

    /// As [`nested_scroll`](Modifier::nested_scroll), with a dispatcher through which code here
    /// scrolls the nested-scroll nodes above, as `Modifier.nestedScroll(connection, dispatcher)`.
    pub fn nested_scroll_with(
        self,
        connection: Rc<dyn NestedScrollConnection>,
        dispatcher: Option<NestedScrollDispatcher>,
    ) -> Modifier {
        self.then_custom(NestedScrollElement { connection, dispatcher })
    }
}

/// The node of `Modifier::nested_scroll_with`, for a node to delegate to, as
/// `nestedScrollModifierNode(connection, dispatcher)`.
pub fn nested_scroll_modifier_node(
    connection: Rc<dyn NestedScrollConnection>,
    dispatcher: Option<NestedScrollDispatcher>,
) -> impl ModifierNode {
    NestedScrollNode::new(connection, dispatcher)
}

struct NestedScrollElement {
    connection: Rc<dyn NestedScrollConnection>,
    dispatcher: Option<NestedScrollDispatcher>,
}

impl ModifierNodeElement for NestedScrollElement {
    type Node = NestedScrollNode;
    fn create(&self) -> NestedScrollNode {
        NestedScrollNode::new(self.connection.clone(), self.dispatcher.clone())
    }
    fn update(&self, node: &mut NestedScrollNode) {
        *node.link.connection.borrow_mut() = self.connection.clone();
        node.set_dispatcher(self.dispatcher.clone());
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&other.connection, &self.connection) && other.dispatcher == self.dispatcher
    }
    fn name(&self) -> &'static str {
        "nested_scroll"
    }
}

struct NestedScrollNode {
    link: Rc<Link>,
    dispatcher: Option<NestedScrollDispatcher>,
}

impl NestedScrollNode {
    fn new(
        connection: Rc<dyn NestedScrollConnection>,
        dispatcher: Option<NestedScrollDispatcher>,
    ) -> NestedScrollNode {
        let link = Rc::new(Link {
            connection: RefCell::new(connection),
            parent: RefCell::new(None),
            epoch: Cell::new(0),
            handle: Cell::new(None),
            last_parent: RefCell::new(None),
        });
        NestedScrollNode { link, dispatcher }
    }

    /// A new dispatcher (or none), as `updateDispatcher`: the old one lets go of this node,
    /// unless it was given to another since; an attached node gives the new one itself.
    fn set_dispatcher(&mut self, dispatcher: Option<NestedScrollDispatcher>) {
        if self.dispatcher == dispatcher {
            return;
        }
        if let Some(old) = self.dispatcher.take()
            && old.is_on(&self.link)
        {
            *old.inner.node.borrow_mut() = None;
        }
        self.dispatcher = dispatcher;
        if self.link.is_attached() {
            self.give_dispatcher_the_node();
        }
    }

    fn give_dispatcher_the_node(&self) {
        if let Some(dispatcher) = &self.dispatcher {
            *dispatcher.inner.node.borrow_mut() = Some(self.link.clone());
            *dispatcher.inner.last_known_parent.borrow_mut() = None;
        }
    }
}

impl TraversableNode for NestedScrollNode {
    fn traverse_key(&self) -> &'static TraverseKey {
        &NESTED_SCROLL
    }
}

impl ModifierNode for NestedScrollNode {
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
    /// Not hit itself: it hears the events of what's hit inside it.
    fn is_pointer_observer(&self) -> bool {
        true
    }
    fn as_traversable(&mut self) -> Option<&mut dyn TraversableNode> {
        Some(self)
    }
    /// Links to the nested-scroll node above, and gives its dispatcher this node.
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        self.link.handle.set(Some(context.handle()));
        *self.link.parent.borrow_mut() = parent_link(context);
        self.link.epoch.set(next_epoch());
        *self.link.last_parent.borrow_mut() = None;
        self.give_dispatcher_the_node();
    }
    /// Keeps the node above for a fling's end, and lets its dispatcher go.
    fn on_detach(&mut self, context: &mut NodeContext<'_>) {
        let above = parent_link(context);
        *self.link.last_parent.borrow_mut() = above.clone();
        self.link.handle.set(None);
        next_epoch();
        if let Some(dispatcher) = &self.dispatcher {
            *dispatcher.inner.last_known_parent.borrow_mut() = above;
            if dispatcher.is_on(&self.link) {
                *dispatcher.inner.node.borrow_mut() = None;
            }
        }
    }
}

impl PointerInputModifierNode for NestedScrollNode {
    /// On the way in, adds its connection for the scrollables inside to find. It handles
    /// nothing itself.
    fn on_pointer_event(
        &mut self,
        event: &mut PointerEvent,
        pass: PointerEventPass,
        _bounds: IntSize,
        _context: &mut EventContext<'_>,
    ) {
        if pass == PointerEventPass::Initial
            && matches!(event.kind, PointerEventType::Press | PointerEventType::Scroll)
        {
            let connection = self.link.connection();
            PASSED.with(|passed| passed.borrow_mut().push(connection));
        }
    }
}
