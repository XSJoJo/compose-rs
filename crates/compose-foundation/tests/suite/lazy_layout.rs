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
    let modifier = Modifier
        .layout(move |scope, measurable, constraints| {
            let placeable = scope.measure(measurable, constraints);
            measures.set(measures.get() + 1);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        })
        .fill_max_size();
    move |_| Box!(modifier = modifier.clone())
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
