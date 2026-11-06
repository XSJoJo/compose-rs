# The public `LazyLayout` (C8, stages 2 to 4)

Status: being built (2026-11-06): steps 1 (D1322), 2 (D1323) and 3 (D1324) are in. Step 3 is the public prefetch state on its own (`lazy/layout/prefetch_state.rs`): the lists keep theirs (`prefetch::Prefetch`) for now, so the split it describes, nested prefetch and `onNestedPrefetch` are still to do. Where step 2 differs from this note: `LazyLayout` takes its policy as an `Rc<dyn LazyLayoutMeasurePolicy>` (compared by identity, as `SubcomposeLayout`'s), calls `item_provider` when it composes, and has no deprecated `measure(index, constraints)`; the dead paths are still there (D1323 says why). C8's stage 1 (D1312 to D1317) moved lists, grids and staggered grids onto per-item subcomposition: each item is a slot of a `SubcomposeLayout`, composed when the measurement reaches it. This note is how that becomes Compose's public `lazy.layout` API, so apps can write their own lazy layouts, and how the existing layouts sit on it.

## What Compose has at tip

`foundation/api/current.txt`, package `androidx.compose.foundation.lazy.layout`:

- `LazyLayout(itemProvider: () -> LazyLayoutItemProvider, modifier, prefetchState: LazyLayoutPrefetchState?, measurePolicy: LazyLayoutMeasurePolicy)`.
- `LazyLayoutItemProvider`: `itemCount`, `@Composable Item(index, key)`, `getKey(index)` (default `getDefaultLazyLayoutKey(index)`), `getContentType(index)`, `getIndex(key)`.
- `LazyLayoutMeasurePolicy.measure(LazyLayoutMeasureScope, Constraints): MeasureResult`; `LazyLayoutMeasureScope : MeasureScope` with `compose(index): List<Measurable>` (and the deprecated `measure(index, constraints): List<Placeable>`).
- `LazyLayoutPrefetchState()`: `schedulePrecomposition(index)`, `schedulePrecompositionAndPremeasure(index, constraints, onItemPremeasured)`, returning a `PrefetchHandle` (`cancel`, `markAsUrgent`); `PrefetchResultScope` (`index`, `placeablesCount`, `getSize(i)`); the experimental `onNestedPrefetch` constructor and `NestedPrefetchScope`.
- `LazyLayoutPinnableItem(key, index, pinnedItemList, content)`, `LazyLayoutPinnedItemList` (a `List<PinnedItem>`, `PinnedItem.key`/`index`).
- `LazyLayoutIntervalContent` (`intervals`, `itemCount`, `getKey`, `getContentType`, `withInterval`) with `Interval.key`/`type`; `IntervalList`/`MutableIntervalList` (`addInterval`, `get`, `forEach(from, to)`); `LazyLayoutKeyIndexMap(range, content)`; `getDefaultLazyLayoutKey`.
- `LazyLayoutCacheWindow` and `LazyLayoutScrollScope`: already public here (D892, D858).

## What compose-rs has

- `lazy/slots.rs`: `LazyItemsLayout` (a `SubcomposeLayoutWithState` whose policy runs an `ItemsMeasurePolicy`), `ItemComposer::measurable(scope, index)` (Compose's `compose`, one `Measurable` per item: an item's several nodes are stacked by `ItemStacking`, D866), `ItemFactory`/`ItemContents` (Compose's `LazyLayoutItemContentFactory` and `CachedItemContent`), `LazyItemReusePolicy`.
- `Items` (the private interval list: intervals with key, type and span functions, binary search, a key window) and `ItemsRef`.
- `prefetch.rs`: requests (`Handle`) that precompose a slot with `create_paused_precomposition` and premeasure it, timed per content type; they read the list's `LazyListInner` (its items, its `SubcomposeLayoutState`, its metrics, its nested lists).
- Pinning: `LOCAL_PINNABLE_CONTAINER` and lazy items pinned by key (`pinned`, D1282).

Everything is keyed by `LazyListState`/`LazyListInner`, which grids and staggered grids share. That's the coupling to cut.

## Design

**One composer, two providers.** `ItemComposer` and `ItemFactory` become generic over a `SlotSource` trait (slot id, content callback, count, key, content type for an index). The lists' source is today's `ItemFactory` (statically dispatched, so the lists' measurement compiles to what it is now); the public `LazyLayout`'s is an adapter over `Rc<dyn LazyLayoutItemProvider>`, one virtual call per item composed. `LazyItemsLayout` becomes the body of `LazyLayout`, with the list-only parts (the `LOCAL_PARENT_LIST` for nested prefetch, the constraints hint) passed in by the lists.

**The public types** (`compose_foundation::lazy::layout`):

```rust
pub trait LazyLayoutItemProvider {
    fn item_count(&self) -> usize;
    fn item(&self, index: usize, key: u64);                 // composed in the item's slot
    fn key(&self, index: usize) -> u64 { get_default_lazy_layout_key(index) }
    fn content_type(&self, index: usize) -> Option<u64> { None }
    fn index_of(&self, key: u64) -> Option<usize> { None }
}
#[composable] pub fn LazyLayout(item_provider: impl Fn() -> Rc<dyn LazyLayoutItemProvider> + 'static,
    modifier: Modifier, prefetch_state: Option<LazyLayoutPrefetchState>,
    measure_policy: impl LazyLayoutMeasurePolicy + 'static);
pub trait LazyLayoutMeasurePolicy { fn measure(&self, scope: &mut LazyLayoutMeasureScope<'_, '_>, constraints: Constraints) -> MeasureResult; }
pub struct LazyLayoutMeasureScope<'a, 'b> { /* SubcomposeMeasureScope + ItemComposer */ }
impl LazyLayoutMeasureScope<'_, '_> { pub fn compose(&mut self, index: usize) -> Vec<Measurable>; pub fn measure(&mut self, index: usize, constraints: Constraints) -> Vec<Placeable>; }
```

Keys are `u64`, as the lists' (D170: hashed, a key function's `Hash` value; debug builds compare full keys only through the lists' `Items`). `compose` returns the item's top-level nodes, as Compose's list; the lists keep asking for one stacked `Measurable` (their `ItemStacking`), so their measurement doesn't change. The scope derefs to `SubcomposeMeasureScope`'s `MeasureScope` (`layout`, density, `is_looking_ahead`).

**Prefetch.** `prefetch::Prefetch` splits into what any lazy layout needs (the queue of `Handle`s, the per-content-type averages, the `SubcomposeLayoutState` it precomposes in, the slot source) and what a list adds (the cache window, nested lists, `KEEP_ITEMS_BEHIND`). `LazyLayoutPrefetchState` owns the first; lists own a `LazyLayoutPrefetchState` plus their window, as Compose's states own one. `PrefetchHandle` is today's `Handle` (`cancel`, `mark_as_urgent`); `on_item_premeasured` gets a `PrefetchResultScope` from the premeasure step. The executor stays `LOCAL_PREFETCH_SCHEDULER` (Compose's deprecated `PrefetchScheduler` parameter isn't ported).

**Pinning.** `LazyLayoutPinnedItemList` is a `Vec<PinnedItem>` behind an `Rc<RefCell<…>>`; `LazyLayoutPinnableItem(key, index, list, content)` provides a `PinnableContainer` (ui's) that adds and removes its item. The lists' own pinning (D1282) moves onto it: their measurement keeps the list's pinned items composed and placed, as `pinned` does now.

**Intervals.** `IntervalList<T>`/`MutableIntervalList<T>` are public versions of `Items`' interval search (`add_interval(size, value)`, `get(index) -> Interval<'_, T>`, `for_each(from, to, f)`); `LazyLayoutIntervalContent` is a trait over one (`intervals()`, `item_count`, `key`, `content_type`, `with_interval`), which `LazyListScope`'s content already is in all but name. `LazyLayoutKeyIndexMap` is the key window `Items::index_of` has, made public.

## Order and tests

1. `MutableIntervalList` and `IntervalList` public, `Items` on them: `MutableIntervalListTest` (19). No behaviour changes.
2. The generic composer, `LazyLayout`, `LazyLayoutItemProvider`, `LazyLayoutMeasureScope`: `LazyLayoutTest`'s composing and measuring cases (`measureAndPlaceTwoItems`, `measureAndPlaceMultipleLayoutsInOneItem`, `updatingitemProvider`, `stateBaseditemProvider`, `recompositionWithTheSameInputDoesntCauseRemeasure`, `getDefaultLazyLayoutKeyIsFollowingClaimedRequirements`, reuse: `nodeIsReusedWithoutExtraRemeasure`, `nodeIsReusedWhenRemovedFirst`, `subcomposeNodeContentIsResetWhenReused`, `skippingItemBlockWhenKeyIsObservableButDidntChange`). Lists, grids and staggered grids then compose through it; the lazy scenarios of `compare_instructions.py` must stay within spread.
3. `LazyLayoutPrefetchState` public, lists' prefetch on it: `LazyLayoutTest`'s prefetch cases (`prefetchItem*`, `cancelPrefetchedItem`, `changingKeyForPrefetchingItemInTheMiddleOfRequest`, `keptForReuseItemIsDisposedWhenCanceled`, the two approach ones), `TestPrefetchScheduler` as a scheduler the test drives.
4. `LazyLayoutPinnableItem`/`LazyLayoutPinnedItemList`, the lists' pinning on them; `LazyLayoutStateRestorationTest` (6, on `LazyColumn` today) moved onto `LazyLayout`.

Stage 1's dead paths (the lacking-item branches, `FIRST_STEP`, `MOST_IN_A_ROUND`, `LOCAL_CONSTRAINTS_HINT` for items, `detached`) go in step 2, when the composer is rewritten anyway.

## Costs

Lists keep their statically dispatched source: no added call per item, the same slots. The public path pays one `dyn` call per item composed and per key asked, as Compose's interface calls. Prefetch's split moves fields, not work. Each step runs `compare_instructions.py` against its base at fat LTO, `--dumps` too.
