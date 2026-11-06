//! The modifiers over the layout-rect callbacks, as Compose's `Modifier.onLayoutRectChanged`
//! (`OnLayoutRectChangedModifier.kt`), `Modifier.layoutBounds` (`LayoutBoundsHolder.kt`) and
//! `Modifier.onVisibilityChanged` (`OnVisibilityChangedModifier.kt`).

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
