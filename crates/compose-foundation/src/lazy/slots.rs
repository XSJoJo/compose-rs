//! Lazy layouts on subcomposition, as Compose's `LazyLayout` (C8, D1312): each item is a slot of a
//! `SubcomposeLayout`, composed when the layout's measurement asks for it and deactivated, kept
//! for reuse, when a measurement no longer does. Lists, grids and staggered grids share this: their
//! measurements ask for items through an [`ItemComposer`].

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use compose_runtime::{Callback, SaveableStateHolder, composable};
use compose_ui::subcompose::{
    SlotIdsSet, SubcomposeLayoutState, SubcomposeLayoutWithState, SubcomposeMeasureScope,
    SubcomposeSlotReusePolicy, subcompose_policy,
};
use compose_ui::{Constraints, Measurable, MeasureResult, Modifier};

use super::{ItemStacking, Items, ItemsRef, LazyItem, LazyListInner, LazyListState, prefetch};

/// The most deactivated items kept for reuse per content type, as Compose's
/// `MaxItemsToRetainForReuse`.
const MAX_ITEMS_TO_RETAIN_FOR_REUSE: usize = 7;

/// An item's slot id, as Compose's: its key. Its content type rides along for the reuse policy,
/// which Compose's asks of the item content factory.
#[derive(Clone, Copy, Debug)]
pub(super) struct LazySlot {
    pub(super) key: u64,
    pub(super) content_type: Option<u64>,
}

impl PartialEq for LazySlot {
    fn eq(&self, other: &LazySlot) -> bool {
        self.key == other.key
    }
}

/// The content type of a slot id: none for one whose layout's content went (Compose's
/// `ReusedSlotId`, which any item takes).
fn content_type_of(slot: &dyn Any) -> Option<u64> {
    slot.downcast_ref::<LazySlot>().and_then(|slot| slot.content_type)
}

/// Which items that left are kept for reuse, and which item may reuse which, as Compose's
/// `LazyLayoutItemReusePolicy`: up to `MAX_ITEMS_TO_RETAIN_FOR_REUSE` of each content type, the
/// last used first; an item reuses one of its content type (`None`, Compose's `null`, being one).
pub(super) struct LazyItemReusePolicy;

impl SubcomposeSlotReusePolicy for LazyItemReusePolicy {
    fn get_slots_to_retain(&self, slots: &mut SlotIdsSet) {
        let mut count_per_type: Vec<(Option<u64>, usize)> = Vec::new();
        slots.retain(|slot| {
            let content_type = content_type_of(slot);
            match count_per_type.iter_mut().find(|(kind, _)| *kind == content_type) {
                Some((_, count)) if *count == MAX_ITEMS_TO_RETAIN_FOR_REUSE => false,
                Some((_, count)) => {
                    *count += 1;
                    true
                }
                None => {
                    count_per_type.push((content_type, 1));
                    true
                }
            }
        });
    }

    fn are_compatible(&self, slot: &dyn Any, reusable: &dyn Any) -> bool {
        content_type_of(slot) == content_type_of(reusable)
    }
}

/// What composes each item of one set of items, as Compose's `LazyLayoutItemContentFactory`.
pub(super) struct ItemFactory {
    pub(super) state: LazyListState,
    pub(super) items: ItemsRef,
    holder: SaveableStateHolder,
    stacking: ItemStacking,
}

/// The items' contents, by key, as Compose's `CachedItemContent`s: an item's content is the same
/// `Callback` while its key is there, so a measurement asking for it again composes nothing; it
/// reads the items as they are when it composes (Compose's reads `itemProvider()`), so an item
/// recomposing for a state of its own with the list's content changed sees the new items.
#[derive(Default)]
pub(super) struct ItemContents {
    contents: RefCell<HashMap<u64, (Rc<std::cell::Cell<usize>>, Callback)>>,
    /// Each item's content's scope, by key: run again when the list's content changes.
    pub(super) scopes: RefCell<HashMap<u64, compose_runtime::RecomposeScope>>,
}

/// The contents kept at most; past it, those of keys no longer composed go.
const MOST_CONTENTS: usize = 256;

impl ItemFactory {
    /// The slot id of item `index`.
    pub(super) fn slot(&self, index: usize) -> LazySlot {
        let items = &self.items.0;
        LazySlot { key: items.key(index), content_type: items.content_type(index) }
    }

    /// Item `index`'s content.
    pub(super) fn content(&self, index: usize) -> Callback {
        let key = self.items.0.key(index);
        let inner = self.state.inner_or_detached();
        let mut contents = inner.contents.contents.borrow_mut();
        if let Some((near, content)) = contents.get(&key) {
            near.set(index);
            return content.clone();
        }
        if contents.len() >= MOST_CONTENTS {
            let composed: Vec<u64> =
                inner.children.borrow().iter().map(|item| self.items.0.key(*item)).collect();
            contents.retain(|key, _| composed.contains(key));
        }
        let state = self.state;
        let near = Rc::new(std::cell::Cell::new(index));
        let at = near.clone();
        // Its item found by its key in the items as they are now: its slot is its key's.
        let content = Callback::new(move || {
            let inner = state.inner_or_detached();
            let Some(factory) = inner.factory.borrow().clone() else { return };
            drop(inner);
            let items = &factory.items.0;
            let guess = at.get();
            let found = if guess < items.count && items.key(guess) == key {
                Some(guess)
            } else {
                items.index_of(key, guess)
            };
            if let Some(found) = found {
                at.set(found);
                LazyItem(found, state, factory.holder, factory.stacking);
            }
        });
        contents.insert(key, (near, content.clone()));
        content
    }

    pub(super) fn count(&self) -> usize {
        self.items.0.count
    }
}

/// The items a measurement composes, as Compose's `LazyLayoutMeasureScope.compose`: each asked for
/// once, its layout returned; in the order asked, which is their slots'.
pub(super) struct ItemComposer {
    factory: Rc<ItemFactory>,
    composed: HashMap<usize, Option<Measurable>>,
    order: Vec<usize>,
}

impl ItemComposer {
    fn new(factory: Rc<ItemFactory>) -> ItemComposer {
        ItemComposer { factory, composed: HashMap::new(), order: Vec::new() }
    }

    /// Item `index`'s layout, composed if this measurement hasn't yet. `None` past the items.
    pub(super) fn measurable(
        &mut self,
        scope: &mut SubcomposeMeasureScope<'_, '_>,
        index: usize,
    ) -> Option<Measurable> {
        if let Some(found) = self.composed.get(&index) {
            return *found;
        }
        if index >= self.factory.count() {
            return None;
        }
        let measurables = scope.subcompose(self.factory.slot(index), self.factory.content(index));
        let measurable = measurables.first().copied();
        self.composed.insert(index, measurable);
        self.order.push(index);
        measurable
    }

    /// Keeps item `index` composed through this measurement, measured or not.
    pub(super) fn keep(&mut self, scope: &mut SubcomposeMeasureScope<'_, '_>, index: usize) {
        self.measurable(scope, index);
    }

    /// Whether this measurement composed item `index`.
    pub(super) fn has(&self, index: usize) -> bool {
        self.composed.contains_key(&index)
    }

    pub(super) fn items(&self) -> &Rc<Items> {
        &self.factory.items.0
    }

    /// Panics if two items composed have the same key, as Compose's lazy layouts do.
    #[cfg(debug_assertions)]
    pub(super) fn check_keys(&self) {
        self.factory.items.0.check_keys(&self.order);
    }
}

/// A lazy layout's measurement, its items composed as it asks for them.
pub(super) trait ItemsMeasurePolicy: 'static {
    fn measure(
        &self,
        scope: &mut SubcomposeMeasureScope<'_, '_>,
        composer: &mut ItemComposer,
        constraints: Constraints,
    ) -> MeasureResult;
}

/// The layout of a lazy list, grid or staggered grid: a `SubcomposeLayout` whose slots are its
/// items, measured by `policy`. The items' composition reads what they're composed under: the
/// list for nested prefetch, and the constraints their lazy lists guess at before they're measured.
#[composable]
#[allow(non_snake_case)]
pub(super) fn LazyItemsLayout(
    modifier: Modifier,
    state: LazyListState,
    items: ItemsRef,
    holder: SaveableStateHolder,
    fallback: Option<Constraints>,
    stacking: ItemStacking,
    policy: Rc<dyn ItemsMeasurePolicy>,
) {
    let inner = state.inner_or_detached();
    // Kept when the layout's in content reused, as Compose keeps a reused node's subcompositions.
    let layout = compose_runtime::platform::remember_across_reuse(|| {
        SubcomposeLayoutState::with_policy(LazyItemReusePolicy)
    });
    *inner.layout_state.borrow_mut() = Some(layout.clone());
    *inner.prefetch.items.borrow_mut() = Rc::downgrade(&items.0);
    let factory = compose_runtime::remember_keyed((state, items.clone(), holder, stacking), || {
        // The items' contents run again with these items, in this frame after this composition.
        for (_, scope) in inner.contents.scopes.borrow_mut().drain() {
            scope.invalidate();
        }
        Rc::new(ItemFactory { state, items: items.clone(), holder, stacking })
    });
    *inner.factory.borrow_mut() = Some(factory.clone());
    // One per list, kept: the local stays the same, its readers aren't invalidated; its list is
    // the state's now.
    let parent = compose_runtime::platform::remember_across_reuse(|| {
        Rc::new(prefetch::ParentList::new(&inner))
    });
    parent.set(&inner);
    let measure = compose_runtime::remember_keyed((policy.clone(), factory.clone()), || {
        let (policy, factory) = (policy.clone(), factory.clone());
        subcompose_policy(move |scope, constraints| {
            let mut composer = ItemComposer::new(factory.clone());
            let result = policy.measure(scope, &mut composer, constraints);
            let inner = factory.state.inner_or_detached();
            *inner.children.borrow_mut() = composer.order;
            result
        })
    });
    // The items' constraints, for what's composed in them that guesses at its own (a nested list).
    let hint = inner.item_constraints.get().or(fallback);
    drop(inner);
    compose_runtime::CompositionLocalProvider(
        compose_ui::platform::LOCAL_CONSTRAINTS_HINT.provides(hint),
        || {
            compose_runtime::CompositionLocalProvider(
                prefetch::LOCAL_PARENT_LIST.provides(Some(parent)),
                || {
                    SubcomposeLayoutWithState!(
                        layout.clone(),
                        modifier = modifier.clone(),
                        measure_policy = measure.clone()
                    );
                },
            );
        },
    );
}
