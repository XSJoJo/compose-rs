//! The modifiers over the layout-rect callbacks, as Compose's `Modifier.onLayoutRectChanged`
//! (`OnLayoutRectChangedModifier.kt`), `Modifier.layoutBounds` (`LayoutBoundsHolder.kt`) and
//! `Modifier.onVisibilityChanged` (`OnVisibilityChangedModifier.kt`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_runtime::{Callback, IntoCallback};

use crate::delegation::Delegates;
use crate::modifier::{InspectorInfo, Modifier, ModifierNode, ModifierNodeElement};
use crate::node_context::NodeContext;
use crate::spatial::{RegistrationHandle, RelativeLayoutBounds};

/// Compose's default debounce for `onLayoutRectChanged`.
pub const DEFAULT_LAYOUT_RECT_DEBOUNCE_MILLIS: i64 = 64;

impl Modifier {
    /// Calls `callback` with where what follows in the chain is, in the root, the window and the
    /// screen, after the layout that moved it, at most once a `throttle_millis` and once it
    /// rested `debounce_millis`, as `Modifier.onLayoutRectChanged(throttleMillis,
    /// debounceMillis, callback)` (Compose's defaults are 0 and
    /// [`DEFAULT_LAYOUT_RECT_DEBOUNCE_MILLIS`]). Both 0: at each change; a throttle with no
    /// debounce debounces by the throttle, so the last place is heard.
    pub fn on_layout_rect_changed(
        self,
        throttle_millis: i64,
        debounce_millis: i64,
        callback: impl IntoCallback<dyn Fn(RelativeLayoutBounds)>,
    ) -> Modifier {
        self.then_custom(OnLayoutRectChangedElement {
            throttle: throttle_millis,
            debounce: debounce_millis,
            callback: callback.into_callback(),
        })
    }
}

struct OnLayoutRectChangedElement {
    throttle: i64,
    debounce: i64,
    callback: Callback<dyn Fn(RelativeLayoutBounds)>,
}

struct OnLayoutRectChangedNode {
    throttle: i64,
    debounce: i64,
    callback: Callback<dyn Fn(RelativeLayoutBounds)>,
    handle: Option<RegistrationHandle>,
    /// For an update's registration, which needs a context.
    delegates: Delegates,
}

impl OnLayoutRectChangedNode {
    /// As `disposeAndRegister`.
    fn register(&mut self, context: &mut NodeContext<'_>) {
        if let Some(handle) = self.handle.take() {
            handle.unregister();
        }
        self.handle = Some(context.register_on_layout_rect_changed(
            self.throttle,
            self.debounce,
            self.callback.clone(),
        ));
    }
}

impl ModifierNodeElement for OnLayoutRectChangedElement {
    type Node = OnLayoutRectChangedNode;
    fn create(&self) -> OnLayoutRectChangedNode {
        OnLayoutRectChangedNode {
            throttle: self.throttle,
            debounce: self.debounce,
            callback: self.callback.clone(),
            handle: None,
            delegates: Delegates::new(),
        }
    }
    fn update(&self, node: &mut OnLayoutRectChangedNode) {
        node.throttle = self.throttle;
        node.debounce = self.debounce;
        node.callback = self.callback.clone();
        node.delegates.ask_for_context();
    }
    fn same(&self, other: &Self) -> bool {
        self.throttle == other.throttle
            && self.debounce == other.debounce
            && self.callback.ptr_eq(&other.callback)
    }
    fn name(&self) -> &'static str {
        "onRectChanged"
    }
    fn inspectable_properties(&self, info: &mut InspectorInfo) {
        info.name = Some(self.name());
        info.properties.push(("throttleMillis", self.throttle.to_string()));
        info.properties.push(("debounceMillis", self.debounce.to_string()));
    }
}

impl ModifierNode for OnLayoutRectChangedNode {
    fn delegates(&mut self) -> Option<&mut Delegates> {
        Some(&mut self.delegates)
    }
    /// Nothing to lay out or draw for a new callback.
    fn should_auto_invalidate(&self) -> bool {
        false
    }
    fn on_update_with_context(&mut self, context: &mut NodeContext<'_>) {
        self.register(context);
    }
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        self.register(context);
    }
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        if let Some(handle) = self.handle.take() {
            handle.unregister();
        }
    }
}

// ---- layoutBounds ----------------------------------------------------------------------------

/// Holds the bounds of the node a `Modifier.layout_bounds` is on, as Compose's
/// `LayoutBoundsHolder`: in a state, so reading them observes them (they may change often:
/// better read in a callback than while composing). For `on_visibility_changed`'s viewport.
/// Clones share their bounds; give one holder to one `layout_bounds`.
#[derive(Clone)]
pub struct LayoutBoundsHolder {
    bounds: Rc<compose_runtime::OwnedState<Option<RelativeLayoutBounds>>>,
}

impl Default for LayoutBoundsHolder {
    fn default() -> Self {
        LayoutBoundsHolder::new()
    }
}

impl LayoutBoundsHolder {
    pub fn new() -> LayoutBoundsHolder {
        LayoutBoundsHolder { bounds: Rc::new(compose_runtime::mutable_state_of(None)) }
    }

    /// The bounds of the node it's given to, `None` until it's laid out or once it left.
    pub fn bounds(&self) -> Option<RelativeLayoutBounds> {
        self.bounds.get()
    }

    fn set(&self, bounds: Option<RelativeLayoutBounds>) {
        self.bounds.set(bounds);
    }

    fn same(&self, other: &LayoutBoundsHolder) -> bool {
        Rc::ptr_eq(&self.bounds, &other.bounds)
    }
}

impl std::fmt::Debug for LayoutBoundsHolder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LayoutBoundsHolder").finish_non_exhaustive()
    }
}

impl Modifier {
    /// Keeps the bounds of what follows in the chain in `holder`, as `Modifier.layoutBounds`.
    pub fn layout_bounds(self, holder: &LayoutBoundsHolder) -> Modifier {
        self.then_custom(LayoutBoundsElement(holder.clone()))
    }
}

struct LayoutBoundsElement(LayoutBoundsHolder);

struct LayoutBoundsNode {
    holder: Rc<RefCell<LayoutBoundsHolder>>,
    last: Rc<Cell<Option<RelativeLayoutBounds>>>,
    handle: Option<RegistrationHandle>,
    delegates: Delegates,
}

impl ModifierNodeElement for LayoutBoundsElement {
    type Node = LayoutBoundsNode;
    fn create(&self) -> LayoutBoundsNode {
        LayoutBoundsNode {
            holder: Rc::new(RefCell::new(self.0.clone())),
            last: Rc::default(),
            handle: None,
            delegates: Delegates::new(),
        }
    }
    fn update(&self, node: &mut LayoutBoundsNode) {
        *node.holder.borrow_mut() = self.0.clone();
        // As `forceUpdate`: the new holder hears the last bounds, outside the update.
        node.delegates.ask_for_context();
    }
    fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    fn name(&self) -> &'static str {
        "layoutBounds"
    }
}

impl ModifierNode for LayoutBoundsNode {
    fn delegates(&mut self) -> Option<&mut Delegates> {
        Some(&mut self.delegates)
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
    fn on_update_with_context(&mut self, context: &mut NodeContext<'_>) {
        let (holder, last) = (self.holder.borrow().clone(), self.last.get());
        context.side_effect(move || holder.set(last));
    }
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        if let Some(handle) = self.handle.take() {
            handle.unregister();
        }
        let (holder, last) = (self.holder.clone(), self.last.clone());
        self.handle = Some(context.register_on_layout_rect_changed(
            0,
            0,
            move |bounds: RelativeLayoutBounds| {
                holder.borrow().set(Some(bounds));
                last.set(Some(bounds));
            },
        ));
    }
    fn on_detach(&mut self, context: &mut NodeContext<'_>) {
        if let Some(handle) = self.handle.take() {
            handle.unregister();
        }
        let holder = self.holder.borrow().clone();
        context.side_effect(move || holder.set(None));
    }
}

// ---- onVisibilityChanged ---------------------------------------------------------------------

impl Modifier {
    /// Calls `callback` with whether what follows in the chain is in the viewport (the window,
    /// or `viewport_bounds`' node, which a `layout_bounds` elsewhere keeps), as
    /// `Modifier.onVisibilityChanged(minDurationMs, minFractionVisible, viewportBounds, callback)`:
    /// `true` once more than `min_fraction_visible` of it is inside (all of it with 1, or all of
    /// the viewport if it's larger) for `min_duration_millis`, `false` at once when it no
    /// longer is, or leaves. Compose's defaults are 0, 1 and `None`.
    pub fn on_visibility_changed(
        self,
        min_duration_millis: i64,
        min_fraction_visible: f32,
        viewport_bounds: Option<&LayoutBoundsHolder>,
        callback: impl IntoCallback<dyn Fn(bool)>,
    ) -> Modifier {
        self.then_custom(OnVisibilityChangedElement {
            min_duration: min_duration_millis,
            min_fraction: min_fraction_visible,
            viewport: viewport_bounds.cloned(),
            callback: callback.into_callback(),
        })
    }
}

/// The node of `on_visibility_changed`, to delegate to, as Compose's `onVisibilityChangedNode`.
/// To change what it's given, undelegate it and delegate a new one.
pub fn on_visibility_changed_node(
    min_duration_millis: i64,
    min_fraction_visible: f32,
    viewport_bounds: Option<&LayoutBoundsHolder>,
    callback: impl IntoCallback<dyn Fn(bool)>,
) -> OnVisibilityChangedNode {
    OnVisibilityChangedNode::new(VisibilityParams {
        min_duration: min_duration_millis,
        min_fraction: min_fraction_visible,
        viewport: viewport_bounds.cloned(),
        callback: callback.into_callback(),
    })
}

struct OnVisibilityChangedElement {
    min_duration: i64,
    min_fraction: f32,
    viewport: Option<LayoutBoundsHolder>,
    callback: Callback<dyn Fn(bool)>,
}

#[derive(Clone)]
struct VisibilityParams {
    min_duration: i64,
    min_fraction: f32,
    viewport: Option<LayoutBoundsHolder>,
    callback: Callback<dyn Fn(bool)>,
}

/// What the node and its rect callback share.
struct VisibilityState {
    params: VisibilityParams,
    last_result: bool,
    last_reported: bool,
    last_bounds: Option<RelativeLayoutBounds>,
    last_viewport: Option<RelativeLayoutBounds>,
    /// The wait for the minimum duration.
    job: Option<compose_runtime::Job>,
    scope: Option<compose_runtime::CoroutineScope>,
}

type SharedVisibility = Rc<RefCell<VisibilityState>>;

/// What to do once the state is let go: call the callback with a result.
type Report = Option<(Callback<dyn Fn(bool)>, bool)>;

impl VisibilityState {
    fn cancel_job(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
    }

    /// As `checkVisibility`.
    fn check(shared: &SharedVisibility, bounds: RelativeLayoutBounds) -> Report {
        let mut state = shared.borrow_mut();
        state.last_bounds = Some(bounds);
        let viewport = state.last_viewport;
        if viewport.is_none() && state.params.viewport.is_some() {
            // A viewport is given but not laid out yet: wait for it.
            return None;
        }
        let fraction = match viewport {
            Some(viewport) => bounds.fraction_visible_in(&viewport),
            None => bounds.fraction_visible_in_window(),
        };
        let result = fraction > state.params.min_fraction || fraction == 1.0;
        if result == state.last_result {
            return None;
        }
        state.last_result = result;
        state.cancel_job();
        if result == state.last_reported {
            return None;
        }
        // Only being visible waits for the minimum duration.
        if result && state.params.min_duration > 0 {
            let duration = std::time::Duration::from_millis(state.params.min_duration as u64);
            let waiting = shared.clone();
            let job = state.scope.map(|scope| {
                scope.launch(async move {
                    compose_runtime::delay(duration).await;
                    let report = VisibilityState::trigger(&waiting);
                    report_now(report);
                })
            });
            state.job = job;
            return None;
        }
        Self::trigger_locked(&mut state)
    }

    /// As `triggerCallback`.
    fn trigger(shared: &SharedVisibility) -> Report {
        Self::trigger_locked(&mut shared.borrow_mut())
    }

    fn trigger_locked(state: &mut VisibilityState) -> Report {
        state.job = None;
        state.last_reported = state.last_result;
        Some((state.params.callback.clone(), state.last_result))
    }

    /// As `forceUpdate`: the last bounds checked again.
    fn force_update(shared: &SharedVisibility) -> Report {
        let bounds = shared.borrow().last_bounds;
        bounds.and_then(|bounds| Self::check(shared, bounds))
    }

    /// As `fireExitIfNeeded`: no longer visible, told if it was reported visible.
    fn exit(shared: &SharedVisibility) -> Report {
        let mut state = shared.borrow_mut();
        state.cancel_job();
        state.last_result = false;
        if state.last_reported { Self::trigger_locked(&mut state) } else { None }
    }
}

fn report_now(report: Report) {
    if let Some((callback, visible)) = report {
        callback(visible);
    }
}

/// Calls the callback once the tree's code under way has returned (it's application code).
fn report_later(context: &mut NodeContext<'_>, report: Report) {
    if report.is_some() {
        context.side_effect(move || report_now(report));
    }
}

/// The node of `on_visibility_changed` (and `on_visibility_changed_node`), as Compose's
/// `OnVisibilityChangedNode`.
pub struct OnVisibilityChangedNode {
    state: SharedVisibility,
    handle: Option<RegistrationHandle>,
    delegates: Delegates,
}

impl OnVisibilityChangedNode {
    fn new(params: VisibilityParams) -> OnVisibilityChangedNode {
        OnVisibilityChangedNode {
            state: Rc::new(RefCell::new(VisibilityState {
                params,
                last_result: false,
                last_reported: false,
                last_bounds: None,
                last_viewport: None,
                job: None,
                scope: None,
            })),
            handle: None,
            delegates: Delegates::new(),
        }
    }

    /// As `updateViewport`: the viewport's bounds read, observed, and checked against if they
    /// changed.
    fn update_viewport(&mut self, context: &mut NodeContext<'_>) {
        let holder = self.state.borrow().params.viewport.clone();
        let Some(holder) = holder else {
            let had = self.state.borrow_mut().last_viewport.take().is_some();
            if had {
                let report = VisibilityState::force_update(&self.state);
                report_later(context, report);
            }
            return;
        };
        let viewport = context.observe_reads(|| holder.bounds());
        let changed = {
            let mut state = self.state.borrow_mut();
            let changed = state.last_viewport != viewport;
            state.last_viewport = viewport;
            changed
        };
        if changed {
            let report = VisibilityState::force_update(&self.state);
            report_later(context, report);
        }
    }
}

impl ModifierNodeElement for OnVisibilityChangedElement {
    type Node = OnVisibilityChangedNode;
    fn create(&self) -> OnVisibilityChangedNode {
        OnVisibilityChangedNode::new(VisibilityParams {
            min_duration: self.min_duration,
            min_fraction: self.min_fraction,
            viewport: self.viewport.clone(),
            callback: self.callback.clone(),
        })
    }
    fn update(&self, node: &mut OnVisibilityChangedNode) {
        {
            let mut state = node.state.borrow_mut();
            state.params = VisibilityParams {
                min_duration: self.min_duration,
                min_fraction: self.min_fraction,
                viewport: self.viewport.clone(),
                callback: self.callback.clone(),
            };
        }
        // The viewport read again and the last bounds checked again, with a context.
        node.delegates.ask_for_context();
    }
    fn same(&self, other: &Self) -> bool {
        self.min_duration == other.min_duration
            && self.min_fraction == other.min_fraction
            && match (&self.viewport, &other.viewport) {
                (Some(a), Some(b)) => a.same(b),
                (None, None) => true,
                _ => false,
            }
            && self.callback.ptr_eq(&other.callback)
    }
    fn name(&self) -> &'static str {
        "onViewportVisibilityChanged"
    }
    fn inspectable_properties(&self, info: &mut InspectorInfo) {
        info.name = Some(self.name());
        info.properties.push(("minDurationMs", self.min_duration.to_string()));
        info.properties.push(("minFractionVisible", self.min_fraction.to_string()));
    }
}

impl ModifierNode for OnVisibilityChangedNode {
    fn delegates(&mut self) -> Option<&mut Delegates> {
        Some(&mut self.delegates)
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
    fn as_unplaced_aware(
        &mut self,
    ) -> Option<&mut dyn crate::layout_callbacks::UnplacedAwareModifierNode> {
        Some(self)
    }
    fn as_observer(&mut self) -> Option<&mut dyn crate::observer_node::ObserverModifierNode> {
        Some(self)
    }
    fn on_update_with_context(&mut self, context: &mut NodeContext<'_>) {
        // `viewportBounds`' setter, then `forceUpdate`.
        self.update_viewport(context);
        let report = VisibilityState::force_update(&self.state);
        report_later(context, report);
    }
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        if let Some(handle) = self.handle.take() {
            handle.unregister();
        }
        self.state.borrow_mut().scope = Some(context.coroutine_scope());
        let state = self.state.clone();
        self.handle = Some(context.register_on_layout_rect_changed(
            0,
            0,
            move |bounds: RelativeLayoutBounds| {
                // The viewport's bounds may have changed in this layout: read them again now.
                let holder = state.borrow().params.viewport.clone();
                if let Some(holder) = holder {
                    let viewport =
                        compose_runtime::snapshots::Snapshot::without_read_observation(|| {
                            holder.bounds()
                        });
                    state.borrow_mut().last_viewport = viewport;
                }
                report_now(VisibilityState::check(&state, bounds));
            },
        ));
        self.update_viewport(context);
    }
    fn on_detach(&mut self, context: &mut NodeContext<'_>) {
        if let Some(handle) = self.handle.take() {
            handle.unregister();
        }
        let report = VisibilityState::exit(&self.state);
        self.state.borrow_mut().scope = None;
        report_later(context, report);
    }
    fn on_reset(&mut self, context: &mut NodeContext<'_>) {
        let report = VisibilityState::exit(&self.state);
        {
            let mut state = self.state.borrow_mut();
            state.last_result = false;
            state.last_bounds = None;
            state.last_viewport = None;
        }
        report_later(context, report);
    }
}

impl crate::layout_callbacks::UnplacedAwareModifierNode for OnVisibilityChangedNode {
    fn on_unplaced(&mut self, context: &mut NodeContext<'_>) {
        let report = VisibilityState::exit(&self.state);
        if report.is_some() {
            context.run_after_layout(move || report_now(report));
        }
    }
}

impl crate::observer_node::ObserverModifierNode for OnVisibilityChangedNode {
    fn on_observed_reads_changed(&mut self, context: &mut NodeContext<'_>) {
        self.update_viewport(context);
    }
}
