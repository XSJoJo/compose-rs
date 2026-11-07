//! Resolving a style, as Compose's `StyleResolver.kt`, `StylePropertyCollector.kt` and
//! `StyleResolverNode.kt`: a [`StyleResolver`] runs its style for its state in a collector, keeps
//! the values as snapshot states its readers observe, and is bound to one
//! `Modifier::style_resolver` node, which collects it again when what it read changes.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use compose_animation::AnimationSpec;
use compose_runtime::snapshots::Snapshot;
use compose_runtime::{
    ComposeArg, CompositionLocalAnchor, CoroutineScope, NeverEqualPolicy, OwnedState,
    RememberObserver, mutable_state_of, mutable_state_of_with,
};
use compose_ui::{
    Density, Listener, Modifier, ModifierNode, ModifierNodeElement, NodeContext,
    ObserverModifierNode, TraversableNode, TraverseKey,
};

use super::animations::{SpecSlot, StyleAnimations};
use super::property::{ErasedProperty, Value};
use super::scope::{
    CommonStyle, CommonStyleScope, CustomStyleScope, NestedStyleKey, StyleBlock,
    StylePropertyAccessorScope,
};
use super::state::StyleState;

/// What a collection set: each property once, in the order first set.
#[derive(Clone, Default)]
pub(crate) struct StyleProperties(Vec<(ErasedProperty, Value)>);

impl StyleProperties {
    pub(crate) fn get(&self, key: u64) -> Option<&Value> {
        self.0.iter().find(|(property, _)| property.key == key).map(|(_, value)| value)
    }

    fn contains(&self, key: u64) -> bool {
        self.get(key).is_some()
    }

    fn set(&mut self, property: ErasedProperty, value: Value) {
        match self.0.iter_mut().find(|(set, _)| set.key == property.key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((property, value)),
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &(ErasedProperty, Value)> {
        self.0.iter()
    }

    /// The properties that differ between this collection and `previous`, as `diffInto`.
    pub(crate) fn changes_from(&self, previous: &StyleProperties) -> Vec<u64> {
        let mut changes = Vec::new();
        for (property, value) in &self.0 {
            match previous.get(property.key) {
                Some(old) if property.vtable.same(old, value) => {}
                _ => changes.push(property.key),
            }
        }
        for (property, _) in &previous.0 {
            if !self.contains(property.key) {
                changes.push(property.key);
            }
        }
        changes
    }
}

/// What a reader of a property observes: its value, and its animation while it runs.
#[derive(Clone)]
pub(crate) struct SlotState {
    pub(crate) value: Value,
    pub(crate) animation: Option<Rc<super::animations::Entry>>,
}

/// A resolved property, as Compose's `AnimatableStylePropertyValue`.
struct Slot {
    property: ErasedProperty,
    /// The value it goes to, kept outside the state so a collection compares without reading
    /// (and so observing) it.
    target: Value,
    state: OwnedState<SlotState>,
}

/// A resolver's properties or locals, as Compose's `ResolvedProperties`: the slots, and a state
/// of their membership (Compose's snapshot map), which every lookup reads.
struct Resolved {
    slots: RefCell<Vec<Slot>>,
    membership: OwnedState<u64>,
}

impl Resolved {
    fn new() -> Resolved {
        Resolved { slots: RefCell::new(Vec::new()), membership: mutable_state_of(0) }
    }

    /// The value shown now, observed: its slot's and the membership's.
    fn get(&self, key: u64) -> Option<Value> {
        self.membership.get();
        let state = {
            let slots = self.slots.borrow();
            slots.iter().find(|slot| slot.property.key == key).map(|slot| slot.state.handle())
        }?;
        Some(state.with(|slot| match &slot.animation {
            Some(animation) => animation.value(),
            None => slot.value.clone(),
        }))
    }

    fn contains(&self, key: u64) -> bool {
        self.membership.get();
        self.contains_untracked(key)
    }

    fn contains_untracked(&self, key: u64) -> bool {
        self.slots.borrow().iter().any(|slot| slot.property.key == key)
    }

    /// Sets `property`'s target, written only if it changed, as `update`.
    fn update(&self, property: ErasedProperty, value: Value, animations: &StyleAnimations) {
        let mut slots = self.slots.borrow_mut();
        match slots.iter_mut().find(|slot| slot.property.key == property.key) {
            Some(slot) => {
                if !property.vtable.same(&slot.target, &value) {
                    slot.target = value.clone();
                    let animation = animations.entry(property.key);
                    slot.state.set(SlotState { value, animation });
                }
            }
            None => {
                let animation = animations.entry(property.key);
                let state = mutable_state_of_with(
                    SlotState { value: value.clone(), animation },
                    NeverEqualPolicy,
                );
                slots.push(Slot { property, target: value, state });
                drop(slots);
                self.bump();
            }
        }
    }

    /// Gives `key`'s readers its animation (or none), as Compose's derived value reading the
    /// animation map.
    fn set_animation(&self, key: u64, animation: Option<Rc<super::animations::Entry>>) {
        let slots = self.slots.borrow();
        if let Some(slot) = slots.iter().find(|slot| slot.property.key == key) {
            let current = Snapshot::without_read_observation(|| {
                slot.state.with(|state| state.animation.clone())
            });
            let same = match (&current, &animation) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            if !same {
                slot.state.set(SlotState { value: slot.target.clone(), animation });
            }
        }
    }

    fn properties(&self) -> Vec<(ErasedProperty, Value)> {
        self.slots.borrow().iter().map(|slot| (slot.property, slot.target.clone())).collect()
    }

    fn retain(&self, keep: impl Fn(u64) -> bool) {
        let mut slots = self.slots.borrow_mut();
        let before = slots.len();
        slots.retain(|slot| keep(slot.property.key));
        if slots.len() != before {
            drop(slots);
            self.bump();
        }
    }

    fn bump(&self) {
        let version = Snapshot::without_read_observation(|| self.membership.get());
        self.membership.set(version.wrapping_add(1));
    }
}

/// Where a resolver's parent is: the resolver a `style_resolver` node holds now (its swap is
/// observed), or one given by a test.
pub(crate) struct ResolverRef {
    resolver: RefCell<StyleResolver>,
    version: OwnedState<u32>,
}

impl ResolverRef {
    fn new(resolver: StyleResolver) -> Rc<ResolverRef> {
        Rc::new(ResolverRef { resolver: RefCell::new(resolver), version: mutable_state_of(0) })
    }

    /// The resolver, observed: a swap reaches the readers.
    fn get(&self) -> StyleResolver {
        self.version.get();
        self.resolver.borrow().clone()
    }

    fn set(&self, resolver: StyleResolver) {
        let changed = !self.resolver.borrow().same(&resolver);
        if changed {
            *self.resolver.borrow_mut() = resolver;
            let version = Snapshot::without_read_observation(|| self.version.get());
            self.version.set(version.wrapping_add(1));
        }
    }
}

/// What a collection runs with.
#[derive(Clone)]
pub(crate) struct CollectContext {
    pub(crate) density: Density,
    pub(crate) locals: Option<CompositionLocalAnchor>,
    /// Where the resolver's animations run (none: they jump to their ends).
    pub(crate) scope: Option<CoroutineScope>,
}

struct ResolverInner {
    style: CommonStyle,
    state: StyleState,
    properties: Resolved,
    locals: Resolved,
    pub(crate) animations: StyleAnimations,
    collector: RefCell<Collector>,
    /// The keys of the nested styles it gives, as a state its descendants read.
    nested_style_keys: OwnedState<Vec<NestedStyleKey>>,
    parent: RefCell<Option<Rc<ResolverRef>>>,
    bound: Cell<bool>,
    resolved_at_least_once: Cell<bool>,
    /// The last collection's context: the density its readers convert with.
    context: RefCell<Option<CollectContext>>,
}

/// A resolver not kept alive.
pub(crate) struct WeakStyleResolver(std::rc::Weak<ResolverInner>);

impl WeakStyleResolver {
    pub(crate) fn upgrade(&self) -> Option<StyleResolver> {
        self.0.upgrade().map(StyleResolver)
    }
}

/// A style resolved for a state, as Compose's `StyleResolver`: remember it (it's a
/// `RememberObserver`), bind it to one [`Modifier::style_resolver`] node, and read its
/// properties with [`resolve`](Self::resolve) from the modifiers that apply them
/// (`style_appearance`, `style_placement`, …), which observe what they read. A handle, cloned to
/// share, compared by identity.
#[derive(Clone)]
pub struct StyleResolver(Rc<ResolverInner>);

impl StyleResolver {
    /// A resolver of `style` for `state`, as `StyleResolver(style, styleState)`.
    pub fn new(style: CommonStyle, state: impl Into<StyleState>) -> StyleResolver {
        StyleResolver(Rc::new(ResolverInner {
            style,
            state: state.into(),
            properties: Resolved::new(),
            locals: Resolved::new(),
            animations: StyleAnimations::default(),
            collector: RefCell::new(Collector::default()),
            nested_style_keys: mutable_state_of(Vec::new()),
            parent: RefCell::new(None),
            bound: Cell::new(false),
            resolved_at_least_once: Cell::new(false),
            context: RefCell::new(None),
        }))
    }

    /// A resolver of `style` for a state of its own, as `StyleResolver(style)`.
    pub fn of(style: CommonStyle) -> StyleResolver {
        StyleResolver::new(style, super::MutableStyleState::new(None))
    }

    /// Its style.
    pub fn style(&self) -> &CommonStyle {
        &self.0.style
    }

    /// Its state.
    pub fn style_state(&self) -> &StyleState {
        &self.0.state
    }

    /// Runs `block` with the resolved properties, as `resolve { }`: what it reads is observed
    /// by the measurement, drawing or observation around it.
    ///
    /// # Panics
    ///
    /// If the resolver isn't bound to a `style_resolver` node, or hasn't been resolved yet
    /// (it's resolved when its node is attached: not during the composition that made it), as
    /// Compose's.
    pub fn resolve<R>(&self, block: impl FnOnce(&StyleResolverScope) -> R) -> R {
        assert!(
            self.0.bound.get(),
            "Unbound style resolver. Cannot read an style resolver before it has been bound to a \
             styleResolver modifier."
        );
        assert!(
            self.0.resolved_at_least_once.get(),
            "Unresolved style properties. A style resolved property cannot be used until it is \
             both bound and resolved."
        );
        let density = self.0.context.borrow().as_ref().map(|context| context.density);
        block(&StyleResolverScope { resolver: self, density: density.unwrap_or(Density::new(1.0)) })
    }

    pub(crate) fn downgrade(&self) -> WeakStyleResolver {
        WeakStyleResolver(Rc::downgrade(&self.0))
    }

    /// Whether it's the same resolver.
    pub fn same(&self, other: &StyleResolver) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn bind(&self) {
        assert!(
            !self.0.bound.get(),
            "Cannot bind a style resolver to more than one styleResolver modifier"
        );
        self.0.bound.set(true);
    }

    pub(crate) fn set_parent(&self, parent: Option<Rc<ResolverRef>>) {
        *self.0.parent.borrow_mut() = parent;
    }

    /// Unbinds it and stops its animations, as `dispose`.
    pub(crate) fn dispose(&self) {
        self.0.bound.set(false);
        *self.0.parent.borrow_mut() = None;
        self.0.animations.close();
    }

    /// Collects its style with `context`, as `collect`: the values written to the states its
    /// readers observe.
    pub(crate) fn collect(&self, context: CollectContext) {
        *self.0.context.borrow_mut() = Some(context.clone());
        let mut collector = std::mem::take(&mut *self.0.collector.borrow_mut());
        collector.collect(self, context);
        *self.0.collector.borrow_mut() = collector;
    }

    /// Binds it if it isn't, and collects its style at `density` without a node, as
    /// `collectForTests`: no locals, no animations. For tests.
    #[doc(hidden)]
    pub fn collect_for_test(&self, density: Density) {
        self.0.bound.set(true);
        self.collect(CollectContext { density, locals: None, scope: None });
    }

    /// Collects as [`collect_for_test`](Self::collect_for_test), with its animations run in
    /// `scope`. For tests.
    #[doc(hidden)]
    pub fn collect_for_test_in(&self, density: Density, scope: CoroutineScope) {
        self.0.bound.set(true);
        self.collect(CollectContext { density, locals: None, scope: Some(scope) });
    }

    /// Makes `parent` its parent, as `bindParent`. For tests.
    #[doc(hidden)]
    pub fn bind_parent_for_test(&self, parent: &StyleResolver) {
        self.set_parent(Some(ResolverRef::new(parent.clone())));
    }

    /// Unbinds it and stops its animations, as `dispose`. For tests.
    #[doc(hidden)]
    pub fn dispose_for_test(&self) {
        self.dispose();
    }

    /// The resolvers above, nearest first, until `f` returns false, as `traverseAncestors`.
    fn traverse_ancestors(&self, mut f: impl FnMut(&StyleResolver) -> bool) {
        let mut current = self.0.parent.borrow().as_ref().map(|parent| parent.get());
        while let Some(resolver) = current {
            if !f(&resolver) {
                return;
            }
            current = resolver.0.parent.borrow().as_ref().map(|parent| parent.get());
        }
    }

    /// The nearest value of `local` above, as `resolveLocal`.
    fn resolve_local(&self, key: u64) -> Option<Value> {
        let mut result = None;
        self.traverse_ancestors(|ancestor| {
            result = ancestor.0.locals.get(key);
            result.is_none()
        });
        result
    }

    fn get_or_null(&self, property: ErasedProperty) -> Option<Value> {
        if property.is_local() {
            self.0.locals.get(property.key).or_else(|| self.resolve_local(property.key))
        } else {
            self.0.properties.get(property.key)
        }
    }

    fn is_set(&self, property: ErasedProperty) -> bool {
        if property.is_local() {
            if self.0.locals.contains(property.key) {
                return true;
            }
            let mut result = false;
            self.traverse_ancestors(|ancestor| {
                result = ancestor.0.locals.contains(property.key);
                !result
            });
            result
        } else {
            self.0.properties.contains(property.key)
        }
    }

    /// Writes a collection's values, as `updateProperties`.
    fn update_properties(&self, properties: &StyleProperties, nested_keys: &[NestedStyleKey]) {
        let inner = &self.0;
        for (property, value) in properties.iter() {
            let resolved = if property.is_local() { &inner.locals } else { &inner.properties };
            resolved.update(*property, value.clone(), &inner.animations);
        }
        let keys = &inner.nested_style_keys;
        if Snapshot::without_read_observation(|| keys.with(|keys| keys.as_slice() != nested_keys)) {
            keys.set(nested_keys.to_vec());
        }
        inner.resolved_at_least_once.set(true);
    }

    /// Calls `block` with the styles and states of the resolvers above that give a nested
    /// style for `key`, outermost first, as `resolveNestedStyle`.
    fn resolve_nested_style(&self, key: &NestedStyleKey, mut block: impl FnMut(&StyleResolver)) {
        let mut resolvers = Vec::new();
        self.traverse_ancestors(|resolver| {
            if resolver.0.nested_style_keys.with(|keys| keys.contains(key)) {
                resolvers.push(resolver.clone());
            }
            true
        });
        for resolver in resolvers.iter().rev() {
            block(resolver);
        }
    }

    /// Takes the properties no longer set back to their defaults (a local to what's above),
    /// and drops those neither of the last two collections set and not animating, as
    /// `discardUnneededProperties`.
    fn discard_unneeded_properties(&self, current: &StyleProperties, previous: &StyleProperties) {
        let inner = &self.0;
        for (property, _) in inner.properties.properties() {
            if !current.contains(property.key) {
                let value = property.vtable.default_value();
                inner.properties.update(property, value, &inner.animations);
            }
        }
        let animations = &inner.animations;
        inner.properties.retain(|key| {
            current.contains(key) || previous.contains(key) || animations.animating(key)
        });
        for (property, _) in inner.locals.properties() {
            if !current.contains(property.key) {
                let value = self
                    .resolve_local(property.key)
                    .unwrap_or_else(|| property.vtable.default_value());
                inner.locals.update(property, value, &inner.animations);
            }
        }
        inner.locals.retain(|key| {
            current.contains(key) || previous.contains(key) || animations.animating(key)
        });
    }

    /// Gives each property's readers its animation, after the animations were recorded.
    pub(crate) fn publish_animations(&self) {
        let inner = &self.0;
        for resolved in [&inner.properties, &inner.locals] {
            for (property, _) in resolved.properties() {
                resolved.set_animation(property.key, inner.animations.entry(property.key));
            }
        }
    }
}

impl RememberObserver for StyleResolver {
    fn on_forgotten(&self) {
        self.dispose();
    }

    fn on_abandoned(&self) {
        self.dispose();
    }
}

impl ComposeArg for StyleResolver {
    fn same(&self, other: &Self) -> bool {
        StyleResolver::same(self, other)
    }
}

impl PartialEq for StyleResolver {
    fn eq(&self, other: &StyleResolver) -> bool {
        self.same(other)
    }
}

impl fmt::Debug for StyleResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "StyleResolver({:?})", self.0.style)
    }
}

/// Where a resolver's properties are read, as Compose's `StyleResolverScope`: a
/// [`StylePropertyAccessorScope`] (read with [`StylePropertyAccessorScopeExt`]) and the density.
///
/// [`StylePropertyAccessorScopeExt`]: super::StylePropertyAccessorScopeExt
pub struct StyleResolverScope<'a> {
    resolver: &'a StyleResolver,
    density: Density,
}

impl StyleResolverScope<'_> {
    /// The density its resolver was resolved at, as the scope's `Density`.
    pub fn density(&self) -> Density {
        self.density
    }

    /// Where composition locals are read where its resolver's node is.
    pub fn composition_locals(&self) -> Option<CompositionLocalAnchor> {
        self.resolver.0.context.borrow().as_ref().and_then(|context| context.locals)
    }
}

impl StylePropertyAccessorScope for StyleResolverScope<'_> {
    fn get_or_null_erased(&self, property: ErasedProperty) -> Option<Value> {
        self.resolver.get_or_null(property)
    }

    fn is_set_erased(&self, property: ErasedProperty) -> bool {
        self.resolver.is_set(property)
    }
}

/// Runs a resolver's style, as Compose's `StylePropertyCollector`: what a write wins by (its
/// layer), which writes animate and with what specs, and the nested styles.
#[derive(Default)]
struct Collector {
    resolver: Option<StyleResolver>,
    context: Option<CollectContext>,
    properties: StyleProperties,
    previous: StyleProperties,
    /// Each property's layer, when deeper than the base.
    layers: Vec<(u64, u32)>,
    layer: u32,
    default_to_spec: SpecSlot,
    default_from_spec: SpecSlot,
    pub(crate) to_specs: Vec<(u64, AnimationSpec)>,
    pub(crate) from_specs: Vec<(u64, AnimationSpec)>,
    pub(crate) previous_from_specs: Vec<(u64, AnimationSpec)>,
    animating: Vec<u64>,
    previous_animating: Vec<u64>,
    /// The nested style being applied (from an ancestor), if any.
    nested_style_key: Option<NestedStyleKey>,
    nested_state_override: Option<StyleState>,
    /// The nested styles this style gives.
    nested_style_keys: Vec<NestedStyleKey>,
}

impl Collector {
    fn collect(&mut self, resolver: &StyleResolver, context: CollectContext) {
        self.start_collect(resolver, context);
        let style = resolver.0.style.clone();
        style.apply_style(self);
        self.done_collect();
    }

    fn start_collect(&mut self, resolver: &StyleResolver, context: CollectContext) {
        self.resolver = Some(resolver.clone());
        self.context = Some(context);
        self.previous = std::mem::take(&mut self.properties);
        self.previous_from_specs = std::mem::take(&mut self.from_specs);
        self.to_specs.clear();
        self.default_to_spec = SpecSlot::Unspecified;
        self.default_from_spec = SpecSlot::Unspecified;
        self.previous_animating = std::mem::take(&mut self.animating);
        self.layers.clear();
        self.layer = 0;
        self.nested_style_key = None;
        self.nested_state_override = None;
        self.nested_style_keys.clear();
    }

    fn done_collect(&mut self) {
        let Some(resolver) = self.resolver.take() else { return };
        let context = self.context.take();
        self.layers.clear();
        resolver.update_properties(&self.properties, &self.nested_style_keys);
        let animations = &resolver.0.animations;
        if !animations.is_empty()
            || !self.animating.is_empty()
            || !self.previous_animating.is_empty()
        {
            let changes = self.properties.changes_from(&self.previous);
            let mut animated = self.animating.clone();
            for key in &self.previous_animating {
                if !animated.contains(key) {
                    animated.push(*key);
                }
            }
            animations.record_animations(
                &animated,
                &changes,
                &self.to_specs,
                &self.from_specs,
                &self.previous_from_specs,
                &self.previous,
                &self.properties,
                &resolver,
                context.and_then(|context| context.scope),
            );
        }
        resolver.discard_unneeded_properties(&self.properties, &self.previous);
        if !animations.is_empty() || !self.previous_animating.is_empty() {
            resolver.publish_animations();
        }
    }

    /// Notes which specs a write of `key` animates with, as `recordWrite`.
    fn record_write(&mut self, key: u64) {
        let animating = self.animating.contains(&key);
        let effective = |slot: &SpecSlot, specs: &[(u64, AnimationSpec)]| match slot {
            SpecSlot::Unspecified => animating.then(|| {
                specs
                    .iter()
                    .find(|(set, _)| *set == key)
                    .map(|(_, spec)| spec.clone())
                    .unwrap_or_else(super::animations::default_spring_spec)
            }),
            SpecSlot::Spec(spec) => Some(spec.clone()),
        };
        let to = effective(&self.default_to_spec, &self.to_specs);
        let from = effective(&self.default_from_spec, &self.from_specs);
        record_spec(&mut self.to_specs, key, to.as_ref());
        record_spec(&mut self.from_specs, key, from.as_ref());
        if to.is_some() && from.is_some() && !animating {
            self.animating.push(key);
        }
    }

    fn state(&self) -> StyleState {
        match (&self.nested_state_override, &self.resolver) {
            (Some(state), _) => state.clone(),
            (None, Some(resolver)) => resolver.0.state.clone(),
            (None, None) => super::MutableStyleState::new(None).state().clone(),
        }
    }
}

/// Keeps `spec` for `key` unless it's none or the default spring (implied), as `recordWrite`.
fn record_spec(specs: &mut Vec<(u64, AnimationSpec)>, key: u64, spec: Option<&AnimationSpec>) {
    specs.retain(|(set, _)| *set != key);
    if let Some(spec) = spec
        && *spec != super::animations::default_spring_spec()
    {
        specs.push((key, spec.clone()));
    }
}

impl CustomStyleScope for Collector {
    fn density(&self) -> Density {
        self.context.as_ref().map(|context| context.density).unwrap_or(Density::new(1.0))
    }

    fn composition_locals(&self) -> Option<CompositionLocalAnchor> {
        self.context.as_ref().and_then(|context| context.locals)
    }

    /// As `provide`: ignored while applying an ancestor's style for a nested key outside the
    /// nested style itself (D1583); a write wins unless an earlier one was in a deeper layer.
    fn provide_erased(&mut self, property: ErasedProperty, value: Value) {
        if self.nested_style_key.is_some() {
            return;
        }
        let key = property.key;
        let effective_layer =
            self.layers.iter().find(|(set, _)| *set == key).map_or(0, |(_, layer)| *layer);
        if self.layer >= effective_layer {
            self.record_write(key);
            self.properties.set(property, value);
            if self.layer > effective_layer {
                self.layers.retain(|(set, _)| *set != key);
                self.layers.push((key, self.layer));
            }
        }
    }
}

impl CommonStyleScope for Collector {
    fn style_state(&self) -> StyleState {
        self.state()
    }

    fn state_dyn(&mut self, active: bool, block: StyleBlock<'_>) {
        if active {
            self.style_layer_dyn(block);
        }
    }

    fn style_layer_dyn(&mut self, block: StyleBlock<'_>) {
        self.layer += 1;
        block(self);
        self.layer -= 1;
    }

    fn animate_dyn(
        &mut self,
        to_spec: AnimationSpec,
        from_spec: AnimationSpec,
        block: StyleBlock<'_>,
    ) {
        let previous_to = std::mem::replace(&mut self.default_to_spec, SpecSlot::Spec(to_spec));
        let previous_from =
            std::mem::replace(&mut self.default_from_spec, SpecSlot::Spec(from_spec));
        block(self);
        self.default_to_spec = previous_to;
        self.default_from_spec = previous_from;
    }

    fn provide_nested_style(&mut self, key: &NestedStyleKey, style: &CommonStyle) {
        if self.nested_style_key.as_ref() != Some(key) {
            if !self.nested_style_keys.contains(key) {
                self.nested_style_keys.push(*key);
            }
        } else {
            self.nested_style_key = None;
            let state_override = self.nested_state_override.take();
            style.apply_style(self);
            self.nested_style_key = Some(*key);
            self.nested_state_override = state_override;
        }
    }

    fn apply_nested_style(&mut self, key: &NestedStyleKey) {
        let Some(resolver) = self.resolver.clone() else { return };
        let previous_key = self.nested_style_key.replace(*key);
        let mut ancestors = Vec::new();
        resolver.resolve_nested_style(key, |ancestor| ancestor_push(&mut ancestors, ancestor));
        for ancestor in ancestors {
            let previous_state = self.nested_state_override.replace(ancestor.0.state.clone());
            ancestor.0.style.apply_style(self);
            self.nested_state_override = previous_state;
        }
        self.nested_style_key = previous_key;
    }
}

fn ancestor_push(ancestors: &mut Vec<StyleResolver>, ancestor: &StyleResolver) {
    ancestors.push(ancestor.clone());
}

/// The `style_resolver` node's traversal key, as `StyleResolverNodeKey`.
static STYLE_RESOLVER_KEY: TraverseKey = TraverseKey::new("StyleResolverNodeKey");

/// `Modifier::style_resolver`'s element, as `StyleResolverElement`.
struct StyleResolverElement(StyleResolver);

impl ModifierNodeElement for StyleResolverElement {
    type Node = StyleResolverNode;

    fn create(&self) -> StyleResolverNode {
        StyleResolverNode {
            current: ResolverRef::new(self.0.clone()),
            pending: None,
            delegates: compose_ui::Delegates::new(),
            source: None,
            listener: None,
        }
    }

    /// As `update`: a new resolver is bound in place of the old once the node has its context.
    fn update(&self, node: &mut StyleResolverNode) {
        let current = node.current.resolver.borrow().clone();
        if !current.same(&self.0) {
            node.pending = Some(self.0.clone());
            node.delegates.ask_for_context();
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }

    fn name(&self) -> &'static str {
        "styleResolver"
    }
}

/// The node binding a resolver, as Compose's `StyleResolverNode`: it collects the style when
/// attached and again when what the collection read changes (D1583), gives its descendants'
/// resolvers their parent, and sets the state's flags from its interaction source.
pub(crate) struct StyleResolverNode {
    current: Rc<ResolverRef>,
    /// A resolver an update gave, bound once the node has its context.
    pending: Option<StyleResolver>,
    delegates: compose_ui::Delegates,
    /// The source its state's flags are listened from.
    source: Option<compose_ui::InteractionSource>,
    listener: Option<Rc<Listener>>,
}

impl StyleResolverNode {
    fn resolver(&self) -> StyleResolver {
        self.current.resolver.borrow().clone()
    }

    /// Collects the resolver's style inside an observation, its parent found again, and
    /// listens to its state's source, as `resolveStyle`.
    fn resolve_style(&mut self, context: &mut NodeContext<'_>) {
        let resolver = self.resolver();
        let mut parent = None;
        context.traverse_ancestors::<StyleResolverNode>(|ancestor, _| {
            parent = Some(ancestor.current.clone());
            false
        });
        resolver.set_parent(parent);
        let collect = CollectContext {
            density: context.require_density(),
            locals: Some(context.composition_locals()),
            scope: Some(context.coroutine_scope()),
        };
        context.observe_reads(|| resolver.collect(collect));
        let source = resolver.style_state().interaction_source().cloned();
        let same = match (&source, &self.source) {
            (Some(a), Some(b)) => a.same(b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.listener = source.as_ref().map(|source| resolver.style_state().listen(source));
            self.source = source;
        }
    }
}

impl ModifierNode for StyleResolverNode {
    fn as_traversable(&mut self) -> Option<&mut dyn TraversableNode> {
        Some(self)
    }

    fn as_observer(&mut self) -> Option<&mut dyn ObserverModifierNode> {
        Some(self)
    }

    fn delegates(&mut self) -> Option<&mut compose_ui::Delegates> {
        Some(&mut self.delegates)
    }

    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        if let Some(pending) = self.pending.take() {
            self.current.set(pending);
        }
        self.resolver().bind();
        self.resolve_style(context);
    }

    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.resolver().dispose();
        self.listener = None;
        self.source = None;
    }

    fn on_update_with_context(&mut self, context: &mut NodeContext<'_>) {
        let Some(pending) = self.pending.take() else { return };
        self.resolver().dispose();
        self.current.set(pending);
        self.resolver().bind();
        self.source = None;
        self.listener = None;
        self.resolve_style(context);
    }
}

impl TraversableNode for StyleResolverNode {
    fn traverse_key(&self) -> &'static TraverseKey {
        &STYLE_RESOLVER_KEY
    }
}

impl ObserverModifierNode for StyleResolverNode {
    fn on_observed_reads_changed(&mut self, context: &mut NodeContext<'_>) {
        self.resolve_style(context);
    }
}

pub(crate) fn style_resolver(modifier: Modifier, resolver: StyleResolver) -> Modifier {
    modifier.then_custom(StyleResolverElement(resolver))
}
