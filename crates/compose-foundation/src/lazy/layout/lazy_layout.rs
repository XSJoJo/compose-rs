//! The public `LazyLayout`, as Compose's: a `SubcomposeLayout` whose slots are the items an
//! [`LazyLayoutItemProvider`] gives, composed when its [`LazyLayoutMeasurePolicy`] asks for them
//! (`LazyLayoutMeasureScope::compose`), kept for reuse by content type when it no longer does.
//! The lists, grids and staggered grids compose through the same composer (`slots::ItemComposer`),
//! their source statically dispatched (D1323).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use compose_runtime::{Callback, SaveableStateHolder, composable};
use compose_ui::subcompose::{
    SubcomposeLayoutState, SubcomposeLayoutWithState, SubcomposeMeasureScope,
};
use compose_ui::{Constraints, Measurable, MeasureResult, MeasureScope, Modifier};

use super::super::slots::{IndexMap, ItemComposer, ItemContent, LazyItemReusePolicy, SlotSource};
use super::prefetch_state::LazyLayoutPrefetchState;

/// What a [`LazyLayout`] shows, as Compose's `LazyLayoutItemProvider`: how many items, each one's
/// content, key and content type.
///
/// Keys are `u64`s, as the lazy lists' hashed keys (D170): give a key of another type through
/// [`hash_key`](compose_runtime::hash_key). An item's state (what it remembers, its saveable state) follows its key.
pub trait LazyLayoutItemProvider: 'static {
    /// The number of items.
    fn item_count(&self) -> usize;

    /// Composes item `index`, whose key is `key`, as Compose's `Item(index, key)`.
    fn item(&self, index: usize, key: u64);

    /// Item `index`'s key, unique among the items: by default one made of its index
    /// ([`get_default_lazy_layout_key`]), as Compose's.
    fn key(&self, index: usize) -> u64 {
        get_default_lazy_layout_key(index)
    }

    /// Item `index`'s content type: an item composed where none was reuses the composition of
    /// one of its content type that left. `None`, the default, is a type of its own.
    fn content_type(&self, _index: usize) -> Option<u64> {
        None
    }

    /// The index of the item with `key`, as Compose's `getIndex`: an item whose index changed
    /// while composed finds itself through it. `None` (Compose's `-1`), the default, is not found:
    /// such an item composes nothing until its measurement composes its new index.
    fn index_of(&self, _key: u64) -> Option<usize> {
        None
    }
}

/// The key of item `index` of a provider that has no keys, as Compose's
/// `getDefaultLazyLayoutKey`: the same for the same index, another for another, and not the index
/// itself (a hash of a type of its own, so it's unlikely to be any key an app gives).
pub fn get_default_lazy_layout_key(index: usize) -> u64 {
    super::super::default_key(index)
}

/// Measures a [`LazyLayout`], as Compose's `LazyLayoutMeasurePolicy`: composes the items it
/// wants through its scope's [`compose`](LazyLayoutMeasureScope::compose), measures and places
/// them.
pub trait LazyLayoutMeasurePolicy: 'static {
    fn measure(
        &self,
        scope: &mut LazyLayoutMeasureScope<'_, '_, '_>,
        constraints: Constraints,
    ) -> MeasureResult;
}

impl<F> LazyLayoutMeasurePolicy for F
where
    F: Fn(&mut LazyLayoutMeasureScope<'_, '_, '_>, Constraints) -> MeasureResult + 'static,
{
    fn measure(
        &self,
        scope: &mut LazyLayoutMeasureScope<'_, '_, '_>,
        constraints: Constraints,
    ) -> MeasureResult {
        self(scope, constraints)
    }
}

/// A closure as a [`LazyLayoutMeasurePolicy`], its signature inferred. A layout measures again
/// when given another policy (another `Rc`), so a policy made once and kept doesn't measure the
/// layout again when its caller recomposes.
pub fn lazy_layout_measure_policy<F>(policy: F) -> Rc<dyn LazyLayoutMeasurePolicy>
where
    F: Fn(&mut LazyLayoutMeasureScope<'_, '_, '_>, Constraints) -> MeasureResult + 'static,
{
    Rc::new(policy)
}

/// What a [`LazyLayoutMeasurePolicy`] measures in, as Compose's `LazyLayoutMeasureScope`: a
/// [`MeasureScope`] (it derefs to one: `measure`, `place`, `layout`, the density, whether it's
/// looking ahead) that composes items.
pub struct LazyLayoutMeasureScope<'r, 's, 'a> {
    scope: &'r mut SubcomposeMeasureScope<'s, 'a>,
    composer: ItemComposer<ProviderSource>,
}

impl LazyLayoutMeasureScope<'_, '_, '_> {
    /// Item `index`'s layouts, composed in its slot if this measurement hasn't yet, as Compose's
    /// `compose(index)`: one per node its content emits at its top, to measure. Asked again in the
    /// same measurement, the same ones.
    ///
    /// # Panics
    ///
    /// If `index` isn't below the provider's item count.
    pub fn compose(&mut self, index: usize) -> Vec<Measurable> {
        self.composer.measurables(self.scope, index)
    }
}

impl<'a> std::ops::Deref for LazyLayoutMeasureScope<'_, '_, 'a> {
    type Target = MeasureScope<'a>;

    fn deref(&self) -> &MeasureScope<'a> {
        self.scope
    }
}

impl<'a> std::ops::DerefMut for LazyLayoutMeasureScope<'_, '_, 'a> {
    fn deref_mut(&mut self) -> &mut MeasureScope<'a> {
        self.scope
    }
}

/// A provider as the composer's source of slots, as Compose's `LazyLayoutItemContentFactory`:
/// each item's content by key (`CachedItemContent`), reading the provider as it is when it runs.
pub(in crate::lazy) struct ProviderSource {
    this: Weak<ProviderSource>,
    /// The provider the layout was last composed or measured with.
    provider: RefCell<Rc<dyn LazyLayoutItemProvider>>,
    holder: SaveableStateHolder,
    /// Each key's content, and the index it was last found at.
    contents: RefCell<IndexMap<u64, ItemContent>>,
    /// Each key's content's scope: run again when the provider changes, as Compose's content reads
    /// `itemProvider()`, a state. Bounded as `contents` is, and pruned with it: past
    /// `MOST_CONTENTS`, only those of keys the last measurement composed stay (a key dropped gets
    /// a new content, which composes anew).
    scopes: RefCell<IndexMap<u64, compose_runtime::RecomposeScope>>,
    /// The items the last measurement composed: the contents of the others go past
    /// `MOST_CONTENTS`.
    composed: RefCell<Vec<u64>>,
}

/// The contents kept at most; past it, those of keys the last measurement didn't compose go.
const MOST_CONTENTS: usize = 256;

impl ProviderSource {
    fn provider(&self) -> Rc<dyn LazyLayoutItemProvider> {
        self.provider.borrow().clone()
    }

    /// Takes `provider`: the items' contents run again with it if it's another.
    fn set_provider(&self, provider: Rc<dyn LazyLayoutItemProvider>) {
        if Rc::ptr_eq(&*self.provider.borrow(), &provider) {
            return;
        }
        *self.provider.borrow_mut() = provider;
        for (_, scope) in self.scopes.borrow_mut().drain() {
            scope.invalidate();
        }
    }
}

impl SlotSource for ProviderSource {
    fn count(&self) -> usize {
        self.provider.borrow().item_count()
    }

    fn key(&self, index: usize) -> u64 {
        self.provider.borrow().key(index)
    }

    fn content_type(&self, index: usize) -> Option<u64> {
        self.provider.borrow().content_type(index)
    }

    fn content_of(&self, index: usize, key: u64) -> Callback {
        let mut contents = self.contents.borrow_mut();
        if let Some((near, content)) = contents.get(&key) {
            near.set(index);
            return content.clone();
        }
        if contents.len() >= MOST_CONTENTS {
            let composed = self.composed.borrow();
            contents.retain(|key, _| composed.contains(key));
            self.scopes.borrow_mut().retain(|key, _| composed.contains(key));
        }
        let near = Rc::new(Cell::new(index));
        let at = near.clone();
        let this = self.this.clone();
        let content = Callback::new(move || {
            let Some(source) = this.upgrade() else { return };
            {
                // One per key composed, kept as the contents are: past `MOST_CONTENTS`, only
                // those of the keys the last measurement composed stay.
                let mut scopes = source.scopes.borrow_mut();
                if scopes.len() >= MOST_CONTENTS && !scopes.contains_key(&key) {
                    // A key's scope and content go together: one composed again takes a new
                    // content, which composes with the provider as it is then.
                    let composed = source.composed.borrow();
                    scopes.retain(|it, _| *it == key || composed.contains(it));
                    if let Ok(mut contents) = source.contents.try_borrow_mut() {
                        contents.retain(|it, _| *it == key || composed.contains(it));
                    }
                }
                scopes.insert(key, compose_runtime::current_recompose_scope());
            }
            let provider = source.provider();
            let mut index = at.get();
            if index >= provider.item_count() || provider.key(index) != key {
                // Moved: found by its key, or nothing, as Compose's `getIndex(key) == -1`.
                match provider.index_of(key) {
                    Some(found) => {
                        index = found;
                        at.set(found);
                    }
                    None => return,
                }
            }
            ProviderItem(ProviderRef(provider), source.holder, index, key);
        });
        contents.insert(key, (near, content.clone()));
        content
    }

    #[cfg(debug_assertions)]
    fn check_keys(&self, indices: &[usize]) {
        panic!(
            "compose-foundation: items {} and {} of a LazyLayout have the same key: keys must be \
             unique",
            indices[0], indices[1]
        );
    }
}

/// A provider compared by identity, for skipping, as Compose's `@Stable` provider.
#[derive(Clone)]
struct ProviderRef(Rc<dyn LazyLayoutItemProvider>);

impl compose_runtime::ComposeArg for ProviderRef {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// An item, in a `SaveableStateProvider` of its key, as Compose's `SkippableItem`: it skips when
/// its provider, index and key stay, so its content recomposing for a state its key reads doesn't
/// run the item again.
#[composable]
#[allow(non_snake_case)]
fn ProviderItem(provider: ProviderRef, holder: SaveableStateHolder, index: usize, key: u64) {
    holder.SaveableStateProvider(key, || provider.0.item(index, key));
}

/// A layout composing and measuring only the items its policy asks for, as Compose's
/// `LazyLayout(itemProvider, modifier, prefetchState, measurePolicy)`: each item a slot of a
/// `SubcomposeLayout` keyed by the item's key; what a measurement didn't compose is kept for
/// reuse (7 per content type) or disposed.
///
/// `item_provider` is called when the layout composes: another provider (another `Rc`) measures
/// it again and runs the items' contents again with it, as Compose's `rememberUpdatedState` of
/// it. A state its `item_count` reads in the policy measures it again when it changes.
/// `measure_policy` is compared by identity: keep it (`remember`) to keep the layout from
/// measuring again when its caller recomposes.
#[composable(non_restartable)]
#[allow(non_snake_case)]
pub fn LazyLayout(
    item_provider: impl Fn() -> Rc<dyn LazyLayoutItemProvider> + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] prefetch_state: Option<LazyLayoutPrefetchState>,
    measure_policy: Rc<dyn LazyLayoutMeasurePolicy>,
) {
    let provider = item_provider();
    let holder = compose_runtime::remember_saveable_state_holder();
    let source = compose_runtime::remember(|| {
        Rc::new_cyclic(|this| ProviderSource {
            this: this.clone(),
            provider: RefCell::new(provider.clone()),
            holder,
            contents: RefCell::default(),
            scopes: RefCell::default(),
            composed: RefCell::default(),
        })
    });
    source.set_provider(provider.clone());
    // Read by the measurement: another provider measures it again.
    let current = compose_runtime::remember_updated_state(ProviderRef(provider));
    let layout = compose_runtime::platform::remember_across_reuse(|| {
        SubcomposeLayoutState::with_policy(LazyItemReusePolicy)
    });
    // The prefetch state's requests work on this layout while it's composed with it, on the
    // host's scheduler (none outside a host: they do nothing).
    let scheduler = compose_ui::LOCAL_PREFETCH_SCHEDULER.current();
    if let (Some(state), Some(scheduler)) = (prefetch_state, scheduler) {
        let (source, layout) = (source.clone(), layout.clone());
        compose_runtime::DisposableEffect(
            (state.clone(), source.clone(), layout.clone(), scheduler.clone()),
            move |scope| {
                let provider = state.attach(source, layout, scheduler);
                scope.on_dispose(move || state.detach(&provider))
            },
        );
    }
    let measure = compose_runtime::remember_keyed((measure_policy.clone(), source.clone()), || {
        let (policy, source) = (measure_policy.clone(), source.clone());
        compose_ui::subcompose::subcompose_policy(move |scope, constraints| {
            source.set_provider(current.get().0);
            let composer = ItemComposer::new(source.clone(), 8);
            let mut scope = LazyLayoutMeasureScope { scope, composer };
            let result = policy.measure(&mut scope, constraints);
            let composed = scope.composer.composed_keys();
            *source.composed.borrow_mut() = composed;
            result
        })
    });
    SubcomposeLayoutWithState!(layout, modifier = modifier, measure_policy = measure);
}
