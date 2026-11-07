//! Compose's `LazyLayoutTest`, rewritten: the public `LazyLayout`, its item provider and measure
//! scope, items kept for reuse (D1323).

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::lazy::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, DpRect, IntSize, Remeasurement, RemeasurementModifier, UnitExt};

/// A provider of `count()` items, each composed by `content(index)`, its content type its index
/// if `typed`: Compose's test's `itemProvider` helper.
struct TestProvider {
    count: Box<dyn Fn() -> usize>,
    typed: bool,
    content: Box<dyn Fn(usize)>,
}

impl LazyLayoutItemProvider for TestProvider {
    fn item_count(&self) -> usize {
        (self.count)()
    }

    fn item(&self, index: usize, _key: u64) {
        (self.content)(index);
    }

    fn content_type(&self, index: usize) -> Option<u64> {
        self.typed.then_some(index as u64)
    }
}

fn provider(
    count: impl Fn() -> usize + 'static,
    content: impl Fn(usize) + 'static,
) -> Rc<dyn LazyLayoutItemProvider> {
    Rc::new(TestProvider { count: Box::new(count), typed: false, content: Box::new(content) })
}

fn tagged_box(tag: String) {
    Box!(modifier = Modifier.fill_max_size().test_tag(tag));
}

fn bounds(ui: &mut ComposeTestRule, tag: &str) -> DpRect {
    ui.on_node_with_tag(tag).get_bounds_in_root()
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, recompositionWithTheSameInputDoesntCauseRemeasure
#[test]
fn a_caller_recomposing_with_the_same_provider_and_policy_doesnt_measure_it_again() {
    let counter = mutable_state_of(0);
    let measures = Rc::new(Cell::new(0));
    let counted = measures.clone();
    let policy = lazy_layout_measure_policy(move |scope, _| {
        counted.set(counted.get() + 1);
        scope.layout(10, 10)
    });
    let items = provider(|| 0, |_| {});
    let handle = counter.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        handle.get();
        let items = items.clone();
        // A modifier made anew each time, as Compose's `Modifier.composed { Modifier }`.
        LazyLayout!(
            move || items.clone(),
            modifier = Modifier.then(Modifier),
            measure_policy = policy.clone()
        );
    });
    ui.wait_for_idle();
    assert_eq!(measures.get(), 1);
    counter.set(1);
    ui.wait_for_idle();
    assert_eq!(measures.get(), 1);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, measureAndPlaceTwoItems
#[test]
fn a_policy_measures_and_places_the_two_items_it_composes() {
    let items = provider(|| 2, |index| tagged_box(index.to_string()));
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = lazy_layout_measure_policy(|scope, _| {
                let first = scope.compose(0)[0];
                let first = scope.measure(first, Constraints::fixed(50, 50));
                let second = scope.compose(1)[0];
                let second = scope.measure(second, Constraints::fixed(20, 20));
                scope.place(&first, 0, 0);
                scope.place(&second, 80, 80);
                scope.layout(100, 100)
            })
        );
    });
    ui.wait_for_idle();
    assert_eq!(bounds(&mut ui, "0"), DpRect::new(0.dp(), 0.dp(), 50.dp(), 50.dp()));
    assert_eq!(bounds(&mut ui, "1"), DpRect::new(80.dp(), 80.dp(), 100.dp(), 100.dp()));
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, measureAndPlaceMultipleLayoutsInOneItem
#[test]
fn an_item_of_two_layouts_gives_both_to_measure() {
    let items = provider(
        || 1,
        |index| {
            tagged_box(format!("{index}x0"));
            tagged_box(format!("{index}x1"));
        },
    );
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = lazy_layout_measure_policy(|scope, _| {
                let layouts = scope.compose(0);
                assert_eq!(layouts.len(), 2);
                let placeables: Vec<_> = layouts
                    .into_iter()
                    .map(|layout| scope.measure(layout, Constraints::fixed(50, 50)))
                    .collect();
                scope.place(&placeables[0], 0, 0);
                scope.place(&placeables[1], 50, 50);
                scope.layout(100, 100)
            })
        );
    });
    ui.wait_for_idle();
    assert_eq!(bounds(&mut ui, "0x0"), DpRect::new(0.dp(), 0.dp(), 50.dp(), 50.dp()));
    assert_eq!(bounds(&mut ui, "0x1"), DpRect::new(50.dp(), 50.dp(), 100.dp(), 100.dp()));
}

/// A policy composing every item the provider has, each filling the layout.
fn all_items(
    current: impl Fn() -> Rc<dyn LazyLayoutItemProvider> + 'static,
) -> Rc<dyn LazyLayoutMeasurePolicy> {
    lazy_layout_measure_policy(move |scope, _| {
        let constraints = Constraints::fixed(100, 100);
        let mut placeables = Vec::new();
        for index in 0..current().item_count() {
            for layout in scope.compose(index) {
                placeables.push(scope.measure(layout, constraints));
            }
        }
        for placeable in &placeables {
            scope.place(placeable, 0, 0);
        }
        scope.layout(100, 100)
    })
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, updatingitemProvider
#[test]
fn another_provider_shows_its_items() {
    let one = provider(|| 1, |index| tagged_box(index.to_string()));
    let current = mutable_state_of_with(one, never_equal_policy());
    let handle = current.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = handle.get();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| all_items(move || handle.get()))
        );
    });
    ui.wait_for_idle();
    ui.on_node_with_tag("0").assert_is_displayed();
    ui.on_node_with_tag("1").assert_does_not_exist();
    current.set(provider(|| 2, |index| tagged_box(index.to_string())));
    ui.wait_for_idle();
    ui.on_node_with_tag("0").assert_is_displayed();
    ui.on_node_with_tag("1").assert_is_displayed();
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, stateBaseditemProvider
#[test]
fn a_provider_whose_count_reads_a_state_shows_the_items_it_counts() {
    let count = mutable_state_of(1usize);
    let counted = count.handle();
    let items = provider(move || counted.get(), |index| tagged_box(index.to_string()));
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        let current = items.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| all_items(move || current.clone()))
        );
    });
    ui.wait_for_idle();
    ui.on_node_with_tag("0").assert_is_displayed();
    ui.on_node_with_tag("1").assert_does_not_exist();
    count.set(2);
    ui.wait_for_idle();
    ui.on_node_with_tag("0").assert_is_displayed();
    ui.on_node_with_tag("1").assert_is_displayed();
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, getDefaultLazyLayoutKeyIsFollowingClaimedRequirements
#[test]
fn a_default_key_is_the_same_for_an_index_and_not_the_index() {
    assert_eq!(get_default_lazy_layout_key(0), get_default_lazy_layout_key(0));
    assert_ne!(get_default_lazy_layout_key(0), get_default_lazy_layout_key(1));
    assert_ne!(get_default_lazy_layout_key(0), 0);
    assert_ne!(get_default_lazy_layout_key(1), 1);
    // Compose's is `Parcelable`, to be saved: a `u64` is.
}

/// What a box filling what it's given that counts its measurements composes: one modifier for
/// all, as Compose's test makes it once.
fn counted_box(measures: Rc<Cell<i32>>) -> impl Fn(usize) + 'static {
    let modifier = counting(measures).fill_max_size();
    move |_| Box!(modifier = modifier.clone())
}

/// A modifier counting its measurements in `measures`.
fn counting(measures: Rc<Cell<i32>>) -> Modifier {
    Modifier.layout(move |scope, measurable, constraints| {
        let placeable = scope.measure(measurable, constraints);
        measures.set(measures.get() + 1);
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            placement.place(&placeable, 0, 0);
        })
    })
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, nodeIsReusedWithoutExtraRemeasure
#[test]
fn an_item_reusing_anothers_node_doesnt_measure_it_again() {
    let to_compose = mutable_state_of(Some(0usize));
    let measures = Rc::new(Cell::new(0));
    let counted = measures.clone();
    let items = provider(|| 2, counted_box(counted));
    let wanted = to_compose.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| lazy_layout_measure_policy(move |scope, constraints| {
                if let Some(index) = wanted.get() {
                    let layout = scope.compose(index)[0];
                    let placeable = scope.measure(layout, constraints);
                    scope.place(&placeable, 0, 0);
                }
                scope.layout(10, 10)
            }))
        );
    });
    ui.wait_for_idle();
    assert_eq!(measures.get(), 1);
    // Kept for reuse.
    to_compose.set(None);
    ui.wait_for_idle();
    // Item 0's node reused for item 1, at the same constraints.
    to_compose.set(Some(1));
    ui.wait_for_idle();
    assert_eq!(measures.get(), 1);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, nodeIsReusedWhenRemovedFirst
// `@Ignore`d in Compose (b/369188686), and here: the node reactivated for the same key is
// measured again (2 measurements), as Compose's.
#[test]
#[ignore = "as Compose's: b/369188686, a node reactivated for its key is measured again"]
fn an_item_removed_and_added_back_reuses_its_node() {
    let count = mutable_state_of(1usize);
    let measures = Rc::new(Cell::new(0));
    let counted = measures.clone();
    let read = count.handle();
    let items = provider(move || read.get(), counted_box(counted));
    let wanted = count.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| lazy_layout_measure_policy(move |scope, constraints| {
                if wanted.get() == 1 {
                    let layout = scope.compose(0)[0];
                    let placeable = scope.measure(layout, constraints);
                    scope.place(&placeable, 0, 0);
                }
                scope.layout(10, 10)
            }))
        );
    });
    ui.wait_for_idle();
    assert_eq!(measures.get(), 1);
    count.set(0);
    ui.wait_for_idle();
    count.set(1);
    ui.wait_for_idle();
    assert_eq!(measures.get(), 1);
}

/// A provider whose keys are the values of a list in a state.
struct KeyedByState {
    keys: MutableState<Vec<u64>>,
    calls: Rc<Cell<i32>>,
}

impl LazyLayoutItemProvider for KeyedByState {
    fn item_count(&self) -> usize {
        self.keys.with(|keys| keys.len())
    }

    fn item(&self, index: usize, key: u64) {
        assert_eq!(index, 0);
        assert_eq!(key, 0);
        self.calls.set(self.calls.get() + 1);
    }

    fn key(&self, index: usize) -> u64 {
        self.keys.with(|keys| keys[index])
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, skippingItemBlockWhenKeyIsObservableButDidntChange
#[test]
fn an_item_whose_key_reads_a_state_skips_when_its_key_stays() {
    let keys = mutable_state_of(vec![0u64]);
    let calls = Rc::new(Cell::new(0));
    let items: Rc<dyn LazyLayoutItemProvider> =
        Rc::new(KeyedByState { keys: keys.handle(), calls: calls.clone() });
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| lazy_layout_measure_policy(|scope, constraints| {
                for layout in scope.compose(0) {
                    scope.measure(layout, constraints);
                }
                scope.layout(100, 100)
            }))
        );
    });
    ui.wait_for_idle();
    assert_eq!(calls.get(), 1);
    keys.edit(|keys| keys.push(1));
    ui.wait_for_idle();
    assert_eq!(calls.get(), 1);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, subcomposeNodeContentIsResetWhenReused
#[test]
fn a_reused_items_subcomposed_content_is_the_new_items() {
    let to_compose = Rc::new(Cell::new(0usize));
    let remeasurement: Rc<RefCell<Option<Remeasurement>>> = Rc::default();
    let items = provider(
        || 3,
        |index| {
            BoxWithConstraints!(
                modifier = Modifier.test_tag(format!("Box {index}")),
                move |_scope: BoxWithConstraintsScope| tagged_box(index.to_string())
            );
        },
    );
    let wanted = to_compose.clone();
    let told = remeasurement.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        let told = told.clone();
        let modifier: Rc<dyn RemeasurementModifier> =
            remember(|| Rc::new(move |it| *told.borrow_mut() = Some(it)));
        let wanted = wanted.clone();
        LazyLayout!(
            move || items.clone(),
            modifier = Modifier.remeasurement_modifier(modifier),
            measure_policy = remember(|| lazy_layout_measure_policy(move |scope, constraints| {
                let layout = scope.compose(wanted.get())[0];
                let placeable = scope.measure(layout, constraints);
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }))
        );
    });
    ui.wait_for_idle();
    let remeasure = (*remeasurement.borrow()).expect("told its remeasurement");
    to_compose.set(1);
    remeasure.force_remeasure();
    to_compose.set(2);
    remeasure.force_remeasure();
    ui.wait_for_idle();
    ui.on_node_with_tag("Box 0").assert_does_not_exist();
    ui.on_node_with_tag("0").assert_does_not_exist();
    ui.on_node_with_tag("Box 2").assert_exists();
    ui.on_node_with_tag("2").assert_exists();
}

#[test]
fn a_new_provider_runs_the_items_it_keeps_with_its_own_content() {
    let first = provider(|| 1, |_| tagged_box("first".into()));
    let current = mutable_state_of_with(first, never_equal_policy());
    let handle = current.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = handle.get();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| lazy_layout_measure_policy(|scope, constraints| {
                let layout = scope.compose(0)[0];
                let placeable = scope.measure(layout, constraints);
                scope.place(&placeable, 0, 0);
                scope.layout(100, 100)
            }))
        );
    });
    ui.wait_for_idle();
    ui.on_node_with_tag("first").assert_exists();
    // The same key, 0's default: the same slot, its content run with the new provider.
    current.set(provider(|| 1, |_| tagged_box("second".into())));
    ui.wait_for_idle();
    ui.on_node_with_tag("first").assert_does_not_exist();
    ui.on_node_with_tag("second").assert_exists();
}

/// Past the most contents a layout keeps (256), those of keys no longer composed go with their
/// scopes: after 600 keys, a new provider still runs the items shown with its own content.
#[test]
fn after_many_keys_a_new_provider_still_runs_the_items_shown() {
    let tagged = |prefix: &'static str| {
        provider(|| 1000, move |index| tagged_box(format!("{prefix}{index}")))
    };
    let current = mutable_state_of_with(tagged("a"), never_equal_policy());
    let first = mutable_state_of(0usize);
    let (handle, from) = (current.handle(), first.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = handle.get();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| lazy_layout_measure_policy(move |scope, _| {
                let mut y = 0;
                for index in from.get()..from.get() + 3 {
                    let layout = scope.compose(index)[0];
                    let placeable = scope.measure(layout, Constraints::fixed(10, 10));
                    scope.place(&placeable, 0, y);
                    y += 10;
                }
                scope.layout(100, 100)
            }))
        );
    });
    ui.wait_for_idle();
    for at in (3..=600).step_by(3) {
        first.set(at);
        ui.wait_for_idle();
    }
    ui.on_node_with_tag("a600").assert_exists();
    current.set(tagged("b"));
    ui.wait_for_idle();
    for index in 600..603 {
        ui.on_node_with_tag(&format!("b{index}")).assert_exists();
        ui.on_node_with_tag(&format!("a{index}")).assert_does_not_exist();
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, keptForReuseItemIsDisposedWhenCanceled
#[test]
fn an_item_no_longer_composed_and_kept_for_reuse_disposes_its_effects() {
    let need = mutable_state_of(true);
    let composed = Rc::new(Cell::new(false));
    let live = composed.clone();
    let items = provider(
        || 1,
        move |_| {
            let live = live.clone();
            DisposableEffect((), move |scope| {
                live.set(true);
                scope.on_dispose(move || live.set(false))
            });
        },
    );
    let wanted = need.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| lazy_layout_measure_policy(move |scope, constraints| {
                if wanted.get() {
                    for layout in scope.compose(0) {
                        scope.measure(layout, constraints);
                    }
                }
                scope.layout(10, 10)
            }))
        );
    });
    ui.wait_for_idle();
    assert!(composed.get());
    need.set(false);
    ui.wait_for_idle();
    assert!(!composed.get());
}

/// Runs the host's prefetch requests to their end, as Compose's
/// `TestPrefetchScheduler.executeActiveRequests` (no deadline in reach).
fn execute_active_requests(ui: &mut ComposeTestRule) {
    while ui.host().prefetch(compose_ui::prefetch_now() + std::time::Duration::from_secs(60)) {}
}

/// Runs the host's prefetch requests until one has work left for the next idle time, as
/// `TestPrefetchScheduler.executeOneRequest` does with a request that pauses.
fn execute_one_request(ui: &mut ComposeTestRule) {
    ui.host().prefetch(compose_ui::prefetch_now() + std::time::Duration::from_secs(60));
}

/// A layout of `count` items that composes item 0 at `constraints` once `show` says so, its
/// prefetch state `prefetch`; the items count their measurements in `measures`.
fn prefetching_layout(
    typed: bool,
    prefetch: LazyLayoutPrefetchState,
    show: MutableState<bool>,
    measures: Rc<Cell<i32>>,
) -> ComposeTestRule {
    let measured = counting(measures);
    let items: Rc<dyn LazyLayoutItemProvider> = Rc::new(TestProvider {
        count: Box::new(|| 1),
        typed,
        content: Box::new(move |index| {
            Box!(
                modifier =
                    Modifier.fill_max_size().test_tag(index.to_string()).then(measured.clone())
            );
        }),
    });
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            prefetch_state = Some(prefetch.clone()),
            measure_policy = remember(|| lazy_layout_measure_policy(move |scope, _| {
                if show.get() {
                    let layout = scope.compose(0)[0];
                    let placeable = scope.measure(layout, Constraints::fixed(50, 50));
                    scope.place(&placeable, 0, 0);
                }
                scope.layout(100, 100)
            }))
        );
    });
    ui.wait_for_idle();
    ui
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, prefetchItem
// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, prefetchItemWithContentType
#[test]
fn a_prefetched_item_is_measured_ahead_and_not_again_when_shown() {
    for typed in [false, true] {
        let prefetch = LazyLayoutPrefetchState::new();
        let show = mutable_state_of(false);
        let measures = Rc::new(Cell::new(0));
        let mut ui = prefetching_layout(typed, prefetch.clone(), show.handle(), measures.clone());
        assert_eq!(measures.get(), 0);
        prefetch.schedule_precomposition_and_premeasure(0, Constraints::fixed(50, 50));
        execute_active_requests(&mut ui);
        assert_eq!(measures.get(), 1, "typed: {typed}");
        ui.on_node_with_tag("0").assert_is_not_displayed();
        show.set(true);
        ui.wait_for_idle();
        ui.on_node_with_tag("0").assert_is_displayed();
        assert_eq!(measures.get(), 1, "typed: {typed}");
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, prefetchItem_reportMeasuredSizeAfterPremeasure
#[test]
fn a_prefetched_item_reports_its_sizes_once_measured() {
    let prefetch = LazyLayoutPrefetchState::new();
    let show = mutable_state_of(false);
    let measures = Rc::new(Cell::new(0));
    let mut ui = prefetching_layout(false, prefetch.clone(), show.handle(), measures.clone());
    let calls = Rc::new(Cell::new(0));
    let called = calls.clone();
    prefetch.schedule_precomposition_and_premeasure_with(
        0,
        Constraints::fixed(50, 50),
        move |result| {
            called.set(called.get() + 1);
            assert_eq!(result.index(), 0);
            assert_eq!(result.placeables_count(), 1);
            for placeable in 0..result.placeables_count() {
                assert_eq!(result.get_size(placeable), IntSize::new(50, 50));
            }
        },
    );
    execute_active_requests(&mut ui);
    assert_eq!(measures.get(), 1);
    assert_eq!(calls.get(), 1);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, cancelPrefetchedItem
#[test]
fn a_cancelled_prefetch_lets_its_item_go() {
    let composed = Rc::new(Cell::new(false));
    let live = composed.clone();
    let items = provider(
        || 1,
        move |_| {
            Box!(modifier = Modifier.fill_max_size());
            let live = live.clone();
            DisposableEffect((), move |scope| {
                live.set(true);
                scope.on_dispose(move || live.set(false))
            });
        },
    );
    let prefetch = LazyLayoutPrefetchState::new();
    let state = prefetch.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            prefetch_state = Some(state.clone()),
            measure_policy =
                remember(|| lazy_layout_measure_policy(|scope, _| scope.layout(100, 100)))
        );
    });
    ui.wait_for_idle();
    let handle = prefetch.schedule_precomposition_and_premeasure(0, Constraints::fixed(50, 50));
    execute_active_requests(&mut ui);
    assert!(composed.get());
    handle.cancel();
    ui.wait_for_idle();
    assert!(!composed.get());
}

/// A provider of one layout per item filling its constraints, keyed by a list in a state, which
/// says whether it was composed and measured.
struct Lettered {
    keys: MutableState<Vec<u64>>,
    composed: Rc<Cell<bool>>,
    measured: Rc<Cell<bool>>,
}

impl LazyLayoutItemProvider for Lettered {
    fn item_count(&self) -> usize {
        self.keys.with(|keys| keys.len())
    }

    fn item(&self, _index: usize, _key: u64) {
        let composed = self.composed.clone();
        DisposableEffect((), move |scope| {
            composed.set(true);
            scope.on_dispose(move || composed.set(false))
        });
        let measured = self.measured.clone();
        compose_ui::Layout(
            Modifier,
            compose_ui::measure_policy(move |scope, _, constraints| {
                measured.set(true);
                scope.layout(constraints.max_width, constraints.max_height)
            }),
            Callback::new(|| {}),
        );
    }

    fn key(&self, index: usize) -> u64 {
        self.keys.with(|keys| keys[index])
    }

    fn index_of(&self, key: u64) -> Option<usize> {
        self.keys.with(|keys| keys.iter().position(|it| *it == key))
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, changingKeyForPrefetchingItemInTheMiddleOfRequest
#[test]
fn a_prefetch_whose_items_key_changed_while_composed_stops_and_lets_it_go() {
    const A: u64 = 10;
    const B: u64 = 11;
    let keys = mutable_state_of(vec![A, B]);
    let composed = Rc::new(Cell::new(false));
    let measured = Rc::new(Cell::new(false));
    let items: Rc<dyn LazyLayoutItemProvider> = Rc::new(Lettered {
        keys: keys.handle(),
        composed: composed.clone(),
        measured: measured.clone(),
    });
    let prefetch = LazyLayoutPrefetchState::new();
    let state = prefetch.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        LazyLayout!(
            move || items.clone(),
            prefetch_state = Some(state.clone()),
            measure_policy =
                remember(|| lazy_layout_measure_policy(|scope, _| scope.layout(100, 100)))
        );
    });
    ui.wait_for_idle();
    prefetch.pause_between_precomposition_and_premeasure(true);
    prefetch.schedule_precomposition_and_premeasure(0, Constraints::fixed(50, 50));
    // Composed, not measured yet.
    execute_one_request(&mut ui);
    assert!(composed.get());
    assert!(!measured.get());
    keys.set(vec![B, A]);
    ui.wait_for_idle();
    // Its index holds another key: it's no longer valid, and measures nothing.
    execute_active_requests(&mut ui);
    assert!(!measured.get());
    ui.wait_for_idle();
    assert!(!composed.get());
}

/// What item 1's effects saw: how many times it was composed, and disposed.
#[derive(Default)]
struct Lives {
    composed: Cell<i32>,
    disposed: Cell<i32>,
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, prefetchItemNotDisposedAfterApproach
#[test]
fn a_prefetched_item_stays_after_both_passes_measure_again() {
    let composed = Rc::new(RefCell::new(Vec::<usize>::new()));
    let list = composed.clone();
    let size = mutable_state_of(100);
    let items = provider(
        || 10,
        move |index| {
            tagged_box(index.to_string());
            let list = list.clone();
            DisposableEffect((), move |scope| {
                list.borrow_mut().push(index);
                scope.on_dispose(move || list.borrow_mut().retain(|it| *it != index))
            });
        },
    );
    let prefetch = LazyLayoutPrefetchState::new();
    let state = prefetch.clone();
    let side = size.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        let state = state.clone();
        compose_ui::LookaheadScope(callback(move |_: compose_ui::LookaheadScope| {
            let items = items.clone();
            LazyLayout!(
                move || items.clone(),
                prefetch_state = Some(state.clone()),
                measure_policy = remember(|| lazy_layout_measure_policy(move |scope, _| {
                    let size = side.get();
                    let layout = scope.compose(0)[0];
                    let placeable = scope.measure(layout, Constraints::fixed(size, size));
                    scope.place(&placeable, 0, 0);
                    scope.layout(size, size)
                }))
            );
        }));
    });
    ui.wait_for_idle();
    assert_eq!(composed.borrow().len(), 1);
    prefetch.schedule_precomposition_and_premeasure(1, Constraints::fixed(100, 100));
    execute_active_requests(&mut ui);
    ui.wait_for_idle();
    assert_eq!(composed.borrow().len(), 2);
    // Both passes measure again, at other constraints.
    size.set(150);
    ui.wait_for_idle();
    assert_eq!(composed.borrow().len(), 2);
    ui.on_node_with_tag("0").assert_is_displayed();
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutTest.kt, disposePrefetchedItemWhileStillNeededInApproach
#[test]
fn an_item_the_pass_thats_now_composes_outlives_a_cancelled_prefetch_of_it() {
    let in_approach = mutable_state_of(true);
    let lives = Rc::new(Lives::default());
    let seen = lives.clone();
    let items = provider(
        || 10,
        move |index| {
            let seen = seen.clone();
            Box!(modifier = Modifier.fill_max_size().test_tag(index.to_string()), {
                let seen = seen.clone();
                DisposableEffect((), move |scope| {
                    if index == 1 {
                        seen.composed.set(seen.composed.get() + 1);
                    }
                    scope.on_dispose(move || {
                        if index == 1 {
                            seen.disposed.set(seen.disposed.get() + 1);
                        }
                    })
                });
            });
        },
    );
    let prefetch = LazyLayoutPrefetchState::new();
    let state = prefetch.clone();
    let wanted = in_approach.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        let state = state.clone();
        compose_ui::LookaheadScope(callback(move |_: compose_ui::LookaheadScope| {
            let items = items.clone();
            LazyLayout!(
                move || items.clone(),
                prefetch_state = Some(state.clone()),
                measure_policy =
                    remember(|| lazy_layout_measure_policy(move |scope, constraints| {
                        let layout = scope.compose(0)[0];
                        let placeable = scope.measure(layout, constraints);
                        if wanted.get() && !scope.is_looking_ahead() {
                            let other = scope.compose(1)[0];
                            scope.measure(other, constraints);
                        }
                        scope.place(&placeable, 0, 0);
                        scope.layout(placeable.width, placeable.height)
                    }))
            );
        }));
    });
    ui.wait_for_idle();
    // Composed by the pass that's now.
    assert_eq!((lives.composed.get(), lives.disposed.get()), (1, 0));
    let handle = prefetch.schedule_precomposition_and_premeasure(1, Constraints::fixed(100, 100));
    execute_active_requests(&mut ui);
    // Not composed again.
    assert_eq!((lives.composed.get(), lives.disposed.get()), (1, 0));
    handle.cancel();
    ui.wait_for_idle();
    // Compose's pass that's now composes item 1 as a precomposed slot of its own, which the
    // prefetch took and its cancel disposes, the pass composing it again: (2, 1). Here the pass
    // that's now composes it as an active slot (D1316), which a prefetch leaves alone: the
    // cancel lets nothing go, and the item outlives it all the same (D1324).
    assert_eq!((lives.composed.get(), lives.disposed.get()), (1, 0));
    in_approach.set(false);
    ui.wait_for_idle();
    // Let go by the pass that's now, no longer wanting it.
    assert_eq!((lives.composed.get(), lives.disposed.get()), (1, 1));
    ui.on_node_with_tag("0").assert_is_displayed();
}

/// The containers items 0 to 9 of `pinning_layout` are composed in, by index.
type Containers = Rc<RefCell<Vec<Option<Rc<dyn compose_ui::PinnableContainer>>>>>;

/// A layout of ten 10-pixel items in a column, those from `first` to `first + 2` shown and the
/// pinned ones composed and placed after them, each item a `LazyLayoutPinnableItem` noting its
/// container in `containers`, as Compose's lists' items.
fn pinning_layout(
    first: MutableState<usize>,
    pinned: LazyLayoutPinnedItemList,
    containers: Containers,
) -> ComposeTestRule {
    struct Pinnable {
        pinned: LazyLayoutPinnedItemList,
        containers: Containers,
    }
    impl LazyLayoutItemProvider for Pinnable {
        fn item_count(&self) -> usize {
            10
        }
        fn item(&self, index: usize, key: u64) {
            let containers = self.containers.clone();
            LazyLayoutPinnableItem!(key, index, self.pinned.clone(), move || {
                containers.borrow_mut()[index] = compose_ui::LOCAL_PINNABLE_CONTAINER.current();
                Box!(modifier = Modifier.size((10.dp(), 10.dp())).test_tag(index.to_string()));
            });
        }
    }
    let items: Rc<dyn LazyLayoutItemProvider> =
        Rc::new(Pinnable { pinned: pinned.clone(), containers });
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let items = items.clone();
        let pinned = pinned.clone();
        LazyLayout!(
            move || items.clone(),
            measure_policy = remember(|| lazy_layout_measure_policy(move |scope, constraints| {
                let shown = first.get()..first.get() + 3;
                let mut wanted: Vec<usize> = shown.clone().collect();
                for item in pinned.to_vec() {
                    if !shown.contains(&item.index()) {
                        wanted.push(item.index());
                    }
                }
                let mut y = 0;
                let mut placeables = Vec::new();
                for index in wanted {
                    for layout in scope.compose(index) {
                        placeables.push(scope.measure(layout, constraints.loosen()));
                    }
                }
                for placeable in &placeables {
                    scope.place(placeable, 0, y);
                    y += placeable.height;
                }
                scope.layout(100, 100)
            }))
        );
    });
    ui.wait_for_idle();
    ui
}

fn containers() -> Containers {
    Rc::new(RefCell::new(vec![None; 10]))
}

fn container(containers: &Containers, index: usize) -> Rc<dyn compose_ui::PinnableContainer> {
    containers.borrow()[index].clone().expect("the item was composed")
}

// Adapted from Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemIsComposedAndPlacedWhenScrolledOut and pinnedItemCanBeUnpinned
#[test]
fn a_pinned_item_stays_in_the_list_and_composed_until_released() {
    let first = mutable_state_of(0usize);
    let pinned = LazyLayoutPinnedItemList::new();
    let seen = containers();
    let mut ui = pinning_layout(first.handle(), pinned.clone(), seen.clone());
    let handle = container(&seen, 1).pin();
    assert_eq!(pinned.len(), 1);
    let item = &pinned.to_vec()[0];
    assert_eq!((item.index(), item.key()), (1, get_default_lazy_layout_key(1)));
    first.set(5);
    ui.wait_for_idle();
    ui.on_node_with_tag("1").assert_exists();
    ui.on_node_with_tag("0").assert_does_not_exist();
    handle.release();
    assert!(pinned.is_empty());
    ui.wait_for_idle();
    ui.on_node_with_tag("1").assert_does_not_exist();
}

// Adapted from Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedMultipleTimes
#[test]
fn an_item_pinned_twice_stays_until_both_are_released() {
    let first = mutable_state_of(0usize);
    let pinned = LazyLayoutPinnedItemList::new();
    let seen = containers();
    let _ui = pinning_layout(first.handle(), pinned.clone(), seen.clone());
    let one = container(&seen, 0).pin();
    let two = container(&seen, 0).pin();
    assert_eq!(pinned.len(), 1);
    one.release();
    assert_eq!(pinned.len(), 1);
    two.release();
    assert!(pinned.is_empty());
}

/// A container counting what's pinned in it.
#[derive(Default)]
struct CountingContainer(Rc<Cell<i32>>);

impl compose_ui::PinnableContainer for CountingContainer {
    fn pin(&self) -> compose_ui::PinnedHandle {
        self.0.set(self.0.get() + 1);
        let count = self.0.clone();
        compose_ui::PinnedHandle::new(move || count.set(count.get() - 1))
    }
}

// Adapted from Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinningIsPropagatedToParentContainer and pinnedItemIsRemovedWhenNotVisible
#[test]
fn pinning_an_item_pins_the_container_around_it_and_leaving_lets_both_go() {
    let parent_pins = Rc::new(Cell::new(0));
    let parent: Rc<dyn compose_ui::PinnableContainer> =
        Rc::new(CountingContainer(parent_pins.clone()));
    let pinned = LazyLayoutPinnedItemList::new();
    let shown = mutable_state_of(true);
    let live = Rc::new(Cell::new(false));
    let (list, showing, alive) = (pinned.clone(), shown.handle(), live.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        if !showing.get() {
            return;
        }
        let list = list.clone();
        let alive = alive.clone();
        CompositionLocalProvider(
            compose_ui::LOCAL_PINNABLE_CONTAINER.provides(Some(parent.clone())),
            move || {
                let alive = alive.clone();
                LazyLayoutPinnableItem!(7, 0, list.clone(), move || {
                    let container = compose_ui::LOCAL_PINNABLE_CONTAINER.current();
                    alive.set(container.is_some());
                    let _ = remember(move || Rc::new(container.map(|it| it.pin())));
                });
            },
        );
    });
    ui.wait_for_idle();
    assert!(live.get());
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned.to_vec()[0].key(), 7);
    assert_eq!(parent_pins.get(), 1);
    // Gone from the composition: out of the list, the parent released.
    shown.set(false);
    ui.wait_for_idle();
    assert!(pinned.is_empty());
    assert_eq!(parent_pins.get(), 0);
}
