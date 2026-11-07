//! Adding to and filtering a text context menu from where it's shown, as Compose's
//! `text.contextmenu.modifier`, and the toolbar's handler (Compose's internal
//! `textContextMenuToolbarHandler`).

use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use compose_ui::{
    LayoutCoordinates, LocalBoxFuture, Modifier, ModifierNode, ModifierNodeElement, NodeContext,
    NodeHandle, Offset, Rect, TraversableNode, TraverseKey,
};

use super::builder::{TextContextMenuBuilderScope, TextContextMenuFilter};
use super::data::{TextContextMenuComponent, TextContextMenuData};
use super::provider::{
    Finally, LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER, TextContextMenuDataProvider,
    TextContextMenuProvider,
};

/// A builder of [`TextContextMenuModifiers::append_text_context_menu_components`].
pub type TextContextMenuBuilder = Rc<dyn Fn(&mut TextContextMenuBuilderScope)>;

/// The key of both modifiers' nodes, as Compose's `TextContextMenuDataTraverseKey`.
static TEXT_CONTEXT_MENU_DATA: TraverseKey = TraverseKey::new("TextContextMenuData");

/// The text context menu's modifiers.
pub trait TextContextMenuModifiers {
    /// Adds `builder`'s components to the text context menus shown below, as Compose's
    /// `Modifier.appendTextContextMenuComponents`: the builders of a hierarchy run from the
    /// bottom up, then every filter of [`filter_text_context_menu_components`]
    /// (Self::filter_text_context_menu_components) applies. `builder` may read state: the menu
    /// follows it.
    fn append_text_context_menu_components(
        self,
        builder: impl Fn(&mut TextContextMenuBuilderScope) + 'static,
    ) -> Modifier;

    /// Shows only the components `filter` passes in the text context menus shown below, as
    /// Compose's `Modifier.filterTextContextMenuComponents`: after every builder, separators
    /// passing always; every filter of a hierarchy must pass, in no order to depend on.
    fn filter_text_context_menu_components(
        self,
        filter: impl Fn(&TextContextMenuComponent) -> bool + 'static,
    ) -> Modifier;
}

impl TextContextMenuModifiers for Modifier {
    fn append_text_context_menu_components(
        self,
        builder: impl Fn(&mut TextContextMenuBuilderScope) + 'static,
    ) -> Modifier {
        self.then_custom(AppendElement(Rc::new(builder)))
    }

    fn filter_text_context_menu_components(
        self,
        filter: impl Fn(&TextContextMenuComponent) -> bool + 'static,
    ) -> Modifier {
        self.then_custom(FilterElement(Rc::new(filter)))
    }
}

struct AppendElement(TextContextMenuBuilder);

struct AppendNode(TextContextMenuBuilder);

impl ModifierNodeElement for AppendElement {
    type Node = AppendNode;
    fn create(&self) -> AppendNode {
        AppendNode(self.0.clone())
    }
    fn update(&self, node: &mut AppendNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    fn name(&self) -> &'static str {
        "addTextContextMenuDataComponents"
    }
}

impl ModifierNode for AppendNode {
    fn as_traversable(&mut self) -> Option<&mut dyn TraversableNode> {
        Some(self)
    }
}

impl TraversableNode for AppendNode {
    fn traverse_key(&self) -> &'static TraverseKey {
        &TEXT_CONTEXT_MENU_DATA
    }
}

struct FilterElement(TextContextMenuFilter);

struct FilterNode(TextContextMenuFilter);

impl ModifierNodeElement for FilterElement {
    type Node = FilterNode;
    fn create(&self) -> FilterNode {
        FilterNode(self.0.clone())
    }
    fn update(&self, node: &mut FilterNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    fn name(&self) -> &'static str {
        "filterTextContextMenuDataComponents"
    }
}

impl ModifierNode for FilterNode {
    fn as_traversable(&mut self) -> Option<&mut dyn TraversableNode> {
        Some(self)
    }
}

impl TraversableNode for FilterNode {
    fn traverse_key(&self) -> &'static TraverseKey {
        &TEXT_CONTEXT_MENU_DATA
    }
}

/// The text context menu's data from the node of `context`, as Compose's
/// `DelegatableNode.collectTextContextMenuData`: the builders above it, nearest first, then the
/// filters above it.
#[doc(hidden)]
pub fn collect_text_context_menu_data(context: &mut NodeContext<'_>) -> TextContextMenuData {
    collect_into(TextContextMenuBuilderScope::new(), context)
}

/// The same, after what `scope` has: the node's own components, nearest of all.
fn collect_into(
    mut scope: TextContextMenuBuilderScope,
    context: &mut NodeContext<'_>,
) -> TextContextMenuData {
    let mut builders: Vec<TextContextMenuBuilder> = Vec::new();
    let mut filters: Vec<TextContextMenuFilter> = Vec::new();
    context.traverse_ancestors_with_key(&TEXT_CONTEXT_MENU_DATA, |node, _| {
        if let Some(append) = node.downcast_ref::<AppendNode>() {
            builders.push(append.0.clone());
        } else if let Some(filter) = node.downcast_ref::<FilterNode>() {
            filters.push(filter.0.clone());
        }
        true
    });
    for builder in builders {
        builder(&mut scope);
    }
    for filter in filters {
        scope.add_filter(filter);
    }
    scope.build()
}

/// Whether a toolbar handler is attached, as Compose's `ToolbarHandlerState`.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolbarHandlerState {
    Uninitialized,
    Detached,
    Attached,
}

/// What asks a toolbar handler to show or hide its toolbar, as Compose's `ToolbarRequesterImpl`:
/// tied to the handler of [`text_context_menu_toolbar_handler`] given it. Clones are the same.
#[doc(hidden)]
#[derive(Clone)]
pub struct ToolbarRequester(Rc<RequesterInner>);

struct RequesterInner {
    state: Cell<ToolbarHandlerState>,
    node: Cell<Option<NodeHandle>>,
}

impl Default for ToolbarRequester {
    fn default() -> ToolbarRequester {
        ToolbarRequester::new()
    }
}

impl ToolbarRequester {
    pub fn new() -> ToolbarRequester {
        ToolbarRequester(Rc::new(RequesterInner {
            state: Cell::new(ToolbarHandlerState::Uninitialized),
            node: Cell::new(None),
        }))
    }

    pub fn state(&self) -> ToolbarHandlerState {
        self.0.state.get()
    }

    /// Shows the toolbar, as Compose's `show`.
    ///
    /// Panics if no handler was ever given it, as Compose's precondition.
    pub fn show(&self) {
        assert!(
            self.0.state.get() != ToolbarHandlerState::Uninitialized,
            "ToolbarRequester is not initialized."
        );
        if let Some(node) = self.0.node.get() {
            node.with_node::<ToolbarHandlerNode, _>(|node, context| node.show(context));
        }
    }

    /// Hides the toolbar, as Compose's `hide`.
    pub fn hide(&self) {
        if let Some(node) = self.0.node.get() {
            node.with_node::<ToolbarHandlerNode, _>(|node, _| node.handler.hide());
        }
    }

    fn same(&self, other: &ToolbarRequester) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A toolbar's `on_show`, run before it shows: Compose's suspending `onShow`.
pub type OnShowToolbar = Rc<dyn Fn() -> LocalBoxFuture<'static, ()>>;

/// What a toolbar goes around, in the destination's coordinates, as Compose's
/// `computeContentBounds`: `None` keeps the last bounds.
pub type ComputeContentBounds = Rc<dyn Fn(&LayoutCoordinates) -> Option<Rect>>;

/// Lets `requester` show or hide the text toolbar from here, as Compose's internal
/// `Modifier.textContextMenuToolbarHandler(requester, onShow, onHide, computeContentBounds)`:
/// the toolbar provider (`LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER`'s, else the platform's, D2031)
/// shows the menu collected from here around what `compute_content_bounds` says, `on_show`
/// before, `on_hide` when it closes.
#[doc(hidden)]
pub fn text_context_menu_toolbar_handler(
    modifier: Modifier,
    requester: ToolbarRequester,
    on_show: Option<OnShowToolbar>,
    on_hide: Option<Rc<dyn Fn()>>,
    compute_content_bounds: ComputeContentBounds,
) -> Modifier {
    modifier.then_custom(ToolbarHandlerElement {
        requester,
        on_show,
        on_hide,
        compute: compute_content_bounds,
    })
}

struct ToolbarHandlerElement {
    requester: ToolbarRequester,
    on_show: Option<OnShowToolbar>,
    on_hide: Option<Rc<dyn Fn()>>,
    compute: ComputeContentBounds,
}

#[doc(hidden)]
pub struct ToolbarHandlerNode {
    requester: ToolbarRequester,
    on_show: Option<OnShowToolbar>,
    on_hide: Option<Rc<dyn Fn()>>,
    compute: ComputeContentBounds,
    /// Its handle, while attached.
    handle: Option<NodeHandle>,
    handler: ToolbarHandler,
}

fn same_option<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

impl ModifierNodeElement for ToolbarHandlerElement {
    type Node = ToolbarHandlerNode;
    fn create(&self) -> ToolbarHandlerNode {
        ToolbarHandlerNode {
            requester: self.requester.clone(),
            on_show: self.on_show.clone(),
            on_hide: self.on_hide.clone(),
            compute: self.compute.clone(),
            handle: None,
            handler: ToolbarHandler::default(),
        }
    }
    fn update(&self, node: &mut ToolbarHandlerNode) {
        // As Compose's `update(requester)`: the old requester forgets it, the new one has it.
        node.requester.0.node.set(None);
        node.requester = self.requester.clone();
        node.requester.0.node.set(node.handle);
        node.requester.0.state.set(if node.handle.is_some() {
            ToolbarHandlerState::Attached
        } else {
            ToolbarHandlerState::Detached
        });
        node.on_show = self.on_show.clone();
        node.on_hide = self.on_hide.clone();
        node.compute = self.compute.clone();
    }
    fn same(&self, other: &Self) -> bool {
        self.requester.same(&other.requester)
            && same_option(&self.on_show, &other.on_show)
            && same_option(&self.on_hide, &other.on_hide)
            && Rc::ptr_eq(&self.compute, &other.compute)
    }
    fn name(&self) -> &'static str {
        "textContextMenuToolbarHandler"
    }
}

impl ModifierNode for ToolbarHandlerNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        let handle = context.handle();
        self.handle = Some(handle);
        self.requester.0.state.set(ToolbarHandlerState::Attached);
        self.requester.0.node.set(Some(handle));
    }

    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.handle = None;
        self.requester.0.state.set(ToolbarHandlerState::Detached);
        self.requester.0.node.set(None);
        // Its coroutine scope's cancelled: the toolbar closes.
        self.handler.forget();
    }
}

impl ToolbarHandlerNode {
    fn show(&mut self, context: &mut NodeContext<'_>) {
        let (on_show, on_hide) = (self.on_show.clone(), self.on_hide.clone());
        self.handler.show::<ToolbarHandlerNode>(context, on_show, on_hide);
    }
}

impl ToolbarHost for ToolbarHandlerNode {
    fn toolbar_content_bounds(
        &mut self,
        destination: &LayoutCoordinates,
        _context: &mut NodeContext<'_>,
    ) -> Option<Rect> {
        (self.compute)(destination)
    }
}

/// A node showing a toolbar of the text context menu collected from it: the handler's node, a
/// field's.
pub(crate) trait ToolbarHost: ModifierNode {
    /// What the toolbar goes around, in `destination`'s coordinates; `None` keeps the last.
    fn toolbar_content_bounds(
        &mut self,
        destination: &LayoutCoordinates,
        context: &mut NodeContext<'_>,
    ) -> Option<Rect>;

    /// Its own components, before its ancestors' (a field's: Compose's are a builder on its
    /// decoration box, the nearest; a node's own costs nothing at each composition, D2032).
    fn own_text_context_menu_components(&mut self, _scope: &mut TextContextMenuBuilderScope) {}
}

/// A toolbar's showing, as Compose's `TextContextMenuToolbarHandlerNode`'s job: a node holding
/// it shows a toolbar of its menu through the toolbar provider.
#[derive(Default)]
pub(crate) struct ToolbarHandler {
    job: Option<compose_runtime::Job>,
}

impl ToolbarHandler {
    /// Whether its toolbar's job runs (shown, or about to be).
    pub(crate) fn is_showing(&self) -> bool {
        self.job.as_ref().is_some_and(compose_runtime::Job::is_active)
    }

    /// Shows the toolbar of the menu collected from `context`'s node (an `N`), unless it shows
    /// already, as Compose's `show`.
    pub(crate) fn show<N: ToolbarHost>(
        &mut self,
        context: &mut NodeContext<'_>,
        on_show: Option<OnShowToolbar>,
        on_hide: Option<Rc<dyn Fn()>>,
    ) {
        if !context.is_attached() || self.is_showing() {
            return;
        }
        let Some(provider) = toolbar_provider(context) else { return };
        let data: Rc<dyn TextContextMenuDataProvider> = Rc::new(NodeDataProvider::<N> {
            handle: context.handle(),
            previous: Cell::new(Rect::ZERO),
            _node: PhantomData,
        });
        self.job = Some(context.coroutine_scope().launch(async move {
            let _hide = Finally(Some(move || {
                if let Some(on_hide) = on_hide {
                    on_hide();
                }
            }));
            if let Some(on_show) = on_show {
                on_show().await;
            }
            provider.show_text_context_menu(data).await;
        }));
    }

    /// Hides it, as Compose's `hide`: its job's cancelled.
    pub(crate) fn hide(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
    }

    /// Its node left: its scope's jobs are cancelled with it.
    pub(crate) fn forget(&mut self) {
        self.hide();
    }
}

/// The toolbar provider where `context`'s node is: the one provided, else the platform's
/// (D2031).
pub(crate) fn toolbar_provider(
    context: &mut NodeContext<'_>,
) -> Option<Rc<dyn TextContextMenuProvider>> {
    context
        .current_value_of(&LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER)
        .or_else(|| super::platform_toolbar::default_toolbar_provider(context))
}

/// A toolbar handler's node as the menu's data provider, as Compose's handler node is: its data
/// collected from it, its bounds its host's, the last kept while the host says none or is gone.
struct NodeDataProvider<N> {
    handle: NodeHandle,
    previous: Cell<Rect>,
    _node: PhantomData<fn() -> N>,
}

impl<N: ToolbarHost> TextContextMenuDataProvider for NodeDataProvider<N> {
    fn position(&self, destination: &LayoutCoordinates) -> Offset {
        self.content_bounds(destination).top_left()
    }

    fn content_bounds(&self, destination: &LayoutCoordinates) -> Rect {
        let bounds = self
            .handle
            .with_node::<N, _>(|node, context| node.toolbar_content_bounds(destination, context))
            .flatten();
        match bounds {
            Some(bounds) => {
                self.previous.set(bounds);
                bounds
            }
            None => self.previous.get(),
        }
    }

    fn data(&self) -> TextContextMenuData {
        // Gone, it has nothing, as Compose's (it may be asked while it's disposed).
        self.handle
            .with_node::<N, _>(|node, context| {
                let mut scope = TextContextMenuBuilderScope::new();
                node.own_text_context_menu_components(&mut scope);
                collect_into(scope, context)
            })
            .unwrap_or_default()
    }
}

/// `root_bounds` (from `local`'s root) in `destination`'s coordinates, through the screen, as
/// Compose's `translateRootToDestination`: the destination may be in another window (a dialog's
/// field). Zero if either isn't attached.
#[doc(hidden)]
pub fn translate_root_to_destination(
    root_bounds: Rect,
    local: &LayoutCoordinates,
    destination: &LayoutCoordinates,
) -> Rect {
    if !local.is_attached() || !destination.is_attached() {
        return Rect::ZERO;
    }
    let root = local.find_root_coordinates();
    let screen = root.local_to_screen(root_bounds.top_left());
    let position = destination.screen_to_local(screen);
    Rect::from_offset_size(position, root_bounds.size())
}
