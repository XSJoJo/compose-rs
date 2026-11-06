//! Bringing things into view, as Compose's `relocation` package and `BringIntoViewSpec`:
//! `BringIntoViewRequester` and `Modifier::bring_into_view_requester` ask the scrolling
//! containers around a node to show it (or a part of it); `Modifier::bring_into_view_responder`
//! takes part as one of them; `BringIntoViewSpec` says how far a container scrolls to show
//! something, and along what animation (D1371, D1372).

use std::cell::RefCell;
use std::rc::Rc;

use compose_animation::AnimationSpec;
use compose_runtime::{ComposeArg, CompositionLocal};
use compose_ui::{
    BringIntoViewBounds, BringIntoViewResponderNode, BringIntoViewScope, LocalBoxFuture, Modifier,
    ModifierNode, ModifierNodeElement, NodeContext, NodeHandle, Rect,
};

// ---- The spec ----------------------------------------------------------------------------------

/// How far a scrolling container scrolls to show something, as Compose's `BringIntoViewSpec`:
/// `calculate_scroll_distance` is given where it starts (`offset`, from the container's top or
/// left edge, in its layout's coordinates, whatever the scroll's direction), its `size` and the
/// container's, and returns how far the content is to move back (positive: towards the bottom or
/// the right of the content). By default, nothing if it's in view or spans the container, else the least that brings
/// its nearer edge in. Asked again each frame of the container's animation: 0 ends it.
pub trait BringIntoViewSpec {
    /// What a container animates along to show what's asked, as the deprecated
    /// `scrollAnimationSpec`: `spring()` by default.
    #[deprecated(note = "Animation spec customization is no longer supported.")]
    fn scroll_animation_spec(&self) -> AnimationSpec {
        compose_animation::spring().into()
    }

    fn calculate_scroll_distance(&self, offset: f32, size: f32, container_size: f32) -> f32 {
        default_scroll_distance(offset, size, container_size)
    }
}

/// The default spec, as `BringIntoViewSpec.DefaultBringIntoViewSpec`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultBringIntoViewSpec;

impl BringIntoViewSpec for DefaultBringIntoViewSpec {}

/// The default distance, as `defaultCalculateScrollDistance`.
pub(crate) fn default_scroll_distance(offset: f32, size: f32, container_size: f32) -> f32 {
    let (leading, trailing) = (offset, offset + size);
    if (leading >= 0.0 && trailing <= container_size)
        || (leading < 0.0 && trailing > container_size)
    {
        0.0
    } else if leading.abs() < (trailing - container_size).abs() {
        leading
    } else {
        trailing - container_size
    }
}

/// The spec the scrolling containers use where it's read, as `LocalBringIntoViewSpec`. Read by
/// `vertical_scroll`, `horizontal_scroll` and `scrollable` (when given none); the lazy layouts use
/// the default for now.
pub static LOCAL_BRING_INTO_VIEW_SPEC: CompositionLocal<Rc<dyn BringIntoViewSpec>> =
    CompositionLocal::new("LocalBringIntoViewSpec", || Rc::new(DefaultBringIntoViewSpec));

// ---- The requester -----------------------------------------------------------------------------

/// What asks the containers around the nodes it's given to show them, as Compose's
/// `BringIntoViewRequester`: give it to `Modifier::bring_into_view_requester`, then call
/// [`bring_into_view`](Self::bring_into_view). Clones are the same requester.
#[derive(Clone, Default)]
pub struct BringIntoViewRequester {
    nodes: Rc<RefCell<Vec<NodeHandle>>>,
}

impl BringIntoViewRequester {
    /// A requester, as `BringIntoViewRequester()` (remember it).
    pub fn new() -> BringIntoViewRequester {
        BringIntoViewRequester::default()
    }

    /// Asks the containers around each of its nodes to show `rect` (in the node's coordinates;
    /// `None`: all of it), as `bringIntoView(rect)`: returns once they have (they animate to it),
    /// or were interrupted by a newer request. A node not placed yet is asked again at the next
    /// frame; one that isn't then is left.
    pub async fn bring_into_view(&self, rect: Option<Rect>) {
        let nodes = self.nodes.borrow().clone();
        for node in nodes {
            let mut asked = node.bring_into_view(rect);
            if asked.is_none() && node.is_attached() {
                compose_runtime::with_frame_nanos(|_| ()).await;
                asked = node.bring_into_view(rect);
            }
            if let Some(asked) = asked {
                asked.await;
            }
        }
    }
}

impl PartialEq for BringIntoViewRequester {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.nodes, &other.nodes)
    }
}

impl ComposeArg for BringIntoViewRequester {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// `bring_into_view_requester` and `bring_into_view_responder`, on `Modifier`.
pub trait BringIntoViewModifiers {
    /// Lets `requester` bring this node into view, as `Modifier.bringIntoViewRequester`.
    fn bring_into_view_requester(self, requester: BringIntoViewRequester) -> Modifier;

    /// Takes part in bringing the nodes inside into view, as the deprecated
    /// `Modifier.bringIntoViewResponder`: `responder` is told the rect asked for (in this node's
    /// coordinates) and says where its parents are to show it.
    fn bring_into_view_responder(self, responder: Rc<dyn BringIntoViewResponder>) -> Modifier;
}

impl BringIntoViewModifiers for Modifier {
    fn bring_into_view_requester(self, requester: BringIntoViewRequester) -> Modifier {
        self.then_custom(RequesterElement(requester))
    }

    fn bring_into_view_responder(self, responder: Rc<dyn BringIntoViewResponder>) -> Modifier {
        self.then_custom(ResponderElement(responder))
    }
}

struct RequesterElement(BringIntoViewRequester);

struct RequesterNode {
    requester: BringIntoViewRequester,
    handle: Option<NodeHandle>,
}

impl RequesterNode {
    fn leave(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.requester.nodes.borrow_mut().retain(|node| *node != handle);
        }
    }

    fn join(&mut self, handle: NodeHandle) {
        self.requester.nodes.borrow_mut().push(handle);
        self.handle = Some(handle);
    }
}

impl ModifierNodeElement for RequesterElement {
    type Node = RequesterNode;
    fn create(&self) -> RequesterNode {
        RequesterNode { requester: self.0.clone(), handle: None }
    }
    fn update(&self, node: &mut RequesterNode) {
        if node.requester == self.0 {
            return;
        }
        let handle = node.handle;
        node.leave();
        node.requester = self.0.clone();
        if let Some(handle) = handle {
            node.join(handle);
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    fn name(&self) -> &'static str {
        "bringIntoViewRequester"
    }
}

impl ModifierNode for RequesterNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        self.join(context.handle());
    }
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.leave();
    }
}

// ---- The responder -----------------------------------------------------------------------------

/// A node that takes part in bringing what's inside it into view, as Compose's deprecated
/// `BringIntoViewResponder`: told where what's asked is (in this node's coordinates), it says
/// where its parents are to show it, and shows it, its parents asked alongside.
pub trait BringIntoViewResponder {
    /// Where `local_rect` (in this node's coordinates) is to be shown by the nodes around, as
    /// `calculateRectForParent`: where it will be once this node has shown it.
    fn calculate_rect_for_parent(&self, local_rect: Rect) -> Rect;

    /// Shows what `local_rect` gives, in this node's coordinates, as `bringChildIntoView`:
    /// returns once it's shown. `local_rect` may give another rect as time goes on, and `None`
    /// once it can't be worked out (the requester detached). One request at a time is this
    /// node's to handle: a newer one comes while an older one may still run.
    fn bring_child_into_view(
        &self,
        local_rect: Rc<dyn Fn() -> Option<Rect>>,
    ) -> LocalBoxFuture<'_, ()>;
}

struct ResponderElement(Rc<dyn BringIntoViewResponder>);

struct ResponderNode(Rc<dyn BringIntoViewResponder>);

impl ModifierNodeElement for ResponderElement {
    type Node = ResponderNode;
    fn create(&self) -> ResponderNode {
        ResponderNode(self.0.clone())
    }
    fn update(&self, node: &mut ResponderNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    fn name(&self) -> &'static str {
        "bringIntoViewResponder"
    }
}

impl ModifierNode for ResponderNode {
    fn as_bring_into_view_responder(&mut self) -> Option<&mut dyn BringIntoViewResponderNode> {
        Some(self)
    }
}

impl BringIntoViewResponderNode for ResponderNode {
    fn calculate_rect_for_parent(&self, rect: Rect, _scope: &BringIntoViewScope) -> Rect {
        self.0.calculate_rect_for_parent(rect)
    }

    fn bring_child_into_view(
        &mut self,
        bounds: BringIntoViewBounds,
        _scope: &BringIntoViewScope,
    ) -> Option<LocalBoxFuture<'static, ()>> {
        let responder = self.0.clone();
        let local_rect: Rc<dyn Fn() -> Option<Rect>> = Rc::new(move || bounds.rect());
        Some(Box::pin(async move { responder.bring_child_into_view(local_rect).await }))
    }
}
