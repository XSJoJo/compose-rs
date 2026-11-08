//! Pinning a lazy grid's items, as Compose's `LazyGridPinnableContainerTest`: a vertical grid of
//! one column, items 10 square. As in `lazy_list_pins.rs`, items that stop being wanted leave
//! between frames (D575), so its `waitUntil`s run those requests; a pinned item out of the lines
//! laid out is placed beside them (D2024).

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{IntSize, PinnableContainer, PinnedHandle, UnitExt};

use crate::scroll_support::run;

type Composed = Rc<RefCell<BTreeSet<usize>>>;
type Containers = Rc<RefCell<BTreeMap<usize, Rc<dyn PinnableContainer>>>>;

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), content);
    ui.wait_for_idle();
    ui
}

/// `Item`: 10 square, tagged with its index, noting it's composed.
fn Item(index: usize, composed: Composed) {
    Box!(modifier = Modifier.size(10.dp()).test_tag(index.to_string()));
    DisposableEffect(index, move |effects| {
        composed.borrow_mut().insert(index);
        let composed = composed.clone();
        effects.on_dispose(move || {
            composed.borrow_mut().remove(&index);
        })
    });
}

/// Keeps the item's `LOCAL_PINNABLE_CONTAINER` under `index`, then composes `Item`.
fn KeptItem(index: usize, composed: &Composed, containers: &Containers) {
    if let Some(container) = compose_ui::LOCAL_PINNABLE_CONTAINER.current() {
        containers.borrow_mut().insert(index, container);
    }
    Item(index, composed.clone());
}

/// Compose's `rule.waitUntil { … }`: the requests between frames run until `done`.
fn wait_until(ui: &mut ComposeTestRule, done: impl Fn() -> bool) {
    let later = std::time::Instant::now() + std::time::Duration::from_secs(60);
    for _ in 0..100 {
        ui.wait_for_idle();
        if done() {
            return;
        }
        ui.host().prefetch(later);
    }
    panic!("never true");
}

fn has(composed: &Composed, index: usize) -> bool {
    composed.borrow().contains(&index)
}

fn missing(composed: &Composed, index: usize) -> impl Fn() -> bool + use<> {
    let composed = composed.clone();
    move || !composed.borrow().contains(&index)
}

fn leaked(index: usize) -> LazyGridState {
    let owned = LazyGridState::new(index, 0);
    let state = *owned;
    std::mem::forget(owned);
    state
}

fn scroll_to(ui: &mut ComposeTestRule, state: LazyGridState, index: usize) {
    let _ = run(ui, async move { state.scroll_to_item(index, 0).await });
}

/// Compose's `assertIsPlaced`.
fn is_placed(ui: &mut ComposeTestRule, tag: &str) -> bool {
    crate::lazy_lookahead_support::is_placed(ui, tag)
}

/// Compose's `assertExists().assertIsNotDisplayed().assertIsPlaced()`.
fn assert_placed_out_of_view(ui: &mut ComposeTestRule, tag: &str) {
    let mut node = ui.on_node_with_tag(tag);
    node.assert_exists();
    assert!(!node.is_displayed(), "{tag} is out of view");
    assert!(is_placed(ui, tag), "{tag} is placed");
}

fn state_of<T: PartialEq + 'static>(value: T) -> MutableState<T> {
    Box::leak(Box::new(mutable_state_of(value))).handle()
}

struct Pins {
    ui: ComposeTestRule,
    state: LazyGridState,
    composed: Composed,
    containers: Containers,
}

impl Pins {
    fn pin(&mut self, index: usize) -> PinnedHandle {
        let container = self.containers.borrow().get(&index).cloned().expect("composed");
        let handle = container.pin();
        self.ui.wait_for_idle();
        handle
    }

    fn scroll_to(&mut self, index: usize) {
        scroll_to(&mut self.ui, self.state, index);
    }
}

/// A one-column grid `main` items long of `count` items from `initial`, every item's container
/// kept.
fn pins(main: i32, initial: usize, count: MutableState<usize>) -> Pins {
    let state = leaked(initial);
    let composed: Composed = Rc::default();
    let containers: Containers = Rc::default();
    let (noted, kept) = (composed.clone(), containers.clone());
    let ui = rule(move || {
        let (noted, kept) = (noted.clone(), kept.clone());
        LazyVerticalGrid!(
            GridCells::Fixed(1),
            modifier = Modifier.size((10 * main).dp()),
            state = state,
            move |scope: LazyGridScope| {
                let (noted, kept) = (noted.clone(), kept.clone());
                scope.items(count.get(), move |index| KeptItem(index, &noted, &kept));
            }
        );
    });
    Pins { ui, state, composed, containers }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinnedItemIsComposedAndPlacedWhenScrolledOut
#[test]
fn pinned_item_is_composed_and_placed_when_scrolled_out() {
    let mut pins = pins(2, 0, state_of(100));
    let _handle = pins.pin(1);
    assert!(has(&pins.composed, 1));
    pins.scroll_to(3);
    wait_until(&mut pins.ui, missing(&pins.composed, 0));
    assert!(has(&pins.composed, 1));
    assert_placed_out_of_view(&mut pins.ui, "1");
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, itemsBetweenPinnedAndCurrentVisibleAreNotComposed
#[test]
fn items_between_pinned_and_current_visible_are_not_composed() {
    let mut pins = pins(2, 0, state_of(100));
    let _handle = pins.pin(1);
    pins.scroll_to(4);
    wait_until(&mut pins.ui, missing(&pins.composed, 0));
    for (index, expected) in [(0, false), (1, true), (2, false), (3, false), (4, true)] {
        assert_eq!(has(&pins.composed, index), expected, "{index}");
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinnedItemAfterVisibleOnesIsComposedAndPlacedWhenScrolledOut
#[test]
fn pinned_item_after_visible_ones_is_composed_and_placed_when_scrolled_out() {
    let mut pins = pins(2, 0, state_of(100));
    pins.scroll_to(4);
    wait_until(&mut pins.ui, missing(&pins.composed, 1));
    let _handle = pins.pin(4);
    assert!(has(&pins.composed, 5));
    pins.scroll_to(0);
    wait_until(&mut pins.ui, missing(&pins.composed, 5));
    let expected = [(0, true), (1, true), (2, false), (3, false), (4, true), (5, false)];
    for (index, expected) in expected {
        assert_eq!(has(&pins.composed, index), expected, "{index}");
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinnedItemCanBeUnpinned
#[test]
fn pinned_item_can_be_unpinned() {
    let mut pins = pins(2, 0, state_of(100));
    let handle = pins.pin(1);
    pins.scroll_to(3);
    wait_until(&mut pins.ui, missing(&pins.composed, 0));
    handle.release();
    wait_until(&mut pins.ui, missing(&pins.composed, 1));
    assert!(!is_placed(&mut pins.ui, "1"));
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinnedItemIsStillPinnedWhenReorderedAndNotVisibleAnymore
#[test]
fn pinned_item_is_still_pinned_when_reordered_and_not_visible_anymore() {
    let state = leaked(0);
    let composed: Composed = Rc::default();
    let containers: Containers = Rc::default();
    let list = mutable_state_of(vec![0usize, 1, 2, 3, 4]);
    let read = list.handle();
    let (noted, kept) = (composed.clone(), containers.clone());
    let mut ui = rule(move || {
        let (noted, kept) = (noted.clone(), kept.clone());
        let ids = read.get();
        LazyVerticalGrid!(
            GridCells::Fixed(1),
            modifier = Modifier.size(30.dp()),
            state = state,
            move |scope: LazyGridScope| {
                let (noted, kept) = (noted.clone(), kept.clone());
                scope.items_list_keyed(
                    ids.clone(),
                    |id: &usize| *id as u64,
                    move |id: &usize| KeptItem(*id, &noted, &kept),
                );
            }
        );
    });
    assert_eq!(composed.borrow().iter().copied().collect::<Vec<_>>(), [0, 1, 2]);
    let _handle = containers.borrow()[&2].pin();
    ui.wait_for_idle();
    list.set(vec![0, 3, 4, 1, 2]);
    wait_until(&mut ui, missing(&composed, 1));
    // 2 is pinned.
    assert_eq!(composed.borrow().iter().copied().collect::<Vec<_>>(), [0, 2, 3, 4]);
    assert!(is_placed(&mut ui, "2"));
}

/// A grid 2 items long of 100 whose state changes, every item's container kept.
fn with_a_new_state(
    initial: usize,
) -> (ComposeTestRule, MutableState<LazyGridState>, Composed, Containers) {
    let state = state_of(leaked(initial));
    let composed: Composed = Rc::default();
    let containers: Containers = Rc::default();
    let (noted, kept) = (composed.clone(), containers.clone());
    let ui = rule(move || {
        let (noted, kept) = (noted.clone(), kept.clone());
        LazyVerticalGrid!(
            GridCells::Fixed(1),
            modifier = Modifier.size(20.dp()),
            state = state.get(),
            move |scope: LazyGridScope| {
                let (noted, kept) = (noted.clone(), kept.clone());
                scope.items(100, move |index| KeptItem(index, &noted, &kept));
            }
        );
    });
    (ui, state, composed, containers)
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, unpinnedWhenLazyGridStateChanges
#[test]
fn unpinned_when_lazy_grid_state_changes() {
    let (mut ui, state, composed, containers) = with_a_new_state(2);
    let _handle = containers.borrow()[&2].pin();
    ui.wait_for_idle();
    assert!(has(&composed, 3));
    scroll_to(&mut ui, state.get(), 0);
    wait_until(&mut ui, missing(&composed, 3));
    assert!(has(&composed, 2));
    state.set(leaked(0));
    wait_until(&mut ui, missing(&composed, 2));
    assert!(!is_placed(&mut ui, "2"));
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinAfterLazyGridStateChange
#[test]
fn pin_after_lazy_grid_state_change() {
    let (mut ui, state, composed, containers) = with_a_new_state(0);
    state.set(leaked(0));
    ui.wait_for_idle();
    let _handle = containers.borrow()[&0].pin();
    ui.wait_for_idle();
    assert!(has(&composed, 1));
    scroll_to(&mut ui, state.get(), 2);
    wait_until(&mut ui, missing(&composed, 1));
    assert!(has(&composed, 0));
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, itemsArePinnedBasedOnGlobalIndexes
#[test]
fn items_are_pinned_based_on_global_indexes() {
    let state = leaked(3);
    let composed: Composed = Rc::default();
    let containers: Containers = Rc::default();
    let (noted, kept) = (composed.clone(), containers.clone());
    let mut ui = rule(move || {
        let (noted, kept) = (noted.clone(), kept.clone());
        LazyVerticalGrid!(
            GridCells::Fixed(1),
            modifier = Modifier.size(20.dp()),
            state = state,
            move |scope: LazyGridScope| {
                for index in 0..100 {
                    let (noted, kept) = (noted.clone(), kept.clone());
                    scope.item(move || KeptItem(index, &noted, &kept));
                }
            }
        );
    });
    let _handle = containers.borrow()[&3].pin();
    ui.wait_for_idle();
    assert!(has(&composed, 4));
    scroll_to(&mut ui, state, 6);
    wait_until(&mut ui, missing(&composed, 4));
    assert!(has(&composed, 3));
    assert_placed_out_of_view(&mut ui, "3");
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinnedItemIsRemovedWhenNotVisible
#[test]
fn pinned_item_is_removed_when_not_visible() {
    let count = state_of(10);
    let mut pins = pins(2, 3, count);
    let _handle = pins.pin(3);
    assert!(has(&pins.composed, 4));
    pins.scroll_to(0);
    wait_until(&mut pins.ui, missing(&pins.composed, 4));
    count.set(3);
    wait_until(&mut pins.ui, missing(&pins.composed, 3));
    assert!(!is_placed(&mut pins.ui, "3"));
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinnedItemIsRemovedWhenVisible
#[test]
fn pinned_item_is_removed_when_visible() {
    let state = leaked(0);
    let composed: Composed = Rc::default();
    let containers: Containers = Rc::default();
    let list = mutable_state_of(vec![0usize, 1, 2]);
    let read = list.handle();
    let (noted, kept) = (composed.clone(), containers.clone());
    let mut ui = rule(move || {
        let (noted, kept) = (noted.clone(), kept.clone());
        let ids = read.get();
        LazyVerticalGrid!(
            GridCells::Fixed(1),
            modifier = Modifier.size(20.dp()),
            state = state,
            move |scope: LazyGridScope| {
                let (noted, kept) = (noted.clone(), kept.clone());
                scope.items_list(ids.clone(), move |id: &usize| KeptItem(*id, &noted, &kept));
            }
        );
    });
    let _handle = containers.borrow()[&1].pin();
    ui.wait_for_idle();
    list.set(vec![0, 2]);
    wait_until(&mut ui, missing(&composed, 1));
    assert!(!is_placed(&mut ui, "1"));
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinnedMultipleTimes
#[test]
fn pinned_multiple_times() {
    let mut pins = pins(2, 0, state_of(100));
    let mut handles = vec![pins.pin(1), pins.pin(1)];
    // Pinned 3 times in all.
    handles.push(pins.pin(1));
    assert!(has(&pins.composed, 0));
    pins.scroll_to(3);
    wait_until(&mut pins.ui, missing(&pins.composed, 0));
    while !handles.is_empty() {
        pins.ui.wait_for_idle();
        assert!(has(&pins.composed, 1));
        handles.remove(0).release();
    }
    wait_until(&mut pins.ui, missing(&pins.composed, 1));
}

struct Parent(Rc<Cell<bool>>);

impl PinnableContainer for Parent {
    fn pin(&self) -> PinnedHandle {
        self.0.set(true);
        let pinned = self.0.clone();
        PinnedHandle::new(move || pinned.set(false))
    }
}

/// A grid of one item under `parents[which]`, the item's container kept.
fn under(
    parents: Vec<Rc<dyn PinnableContainer>>,
    which: MutableState<usize>,
    containers: Containers,
) -> ComposeTestRule {
    rule(move || {
        let kept = containers.clone();
        CompositionLocalProvider(
            compose_ui::LOCAL_PINNABLE_CONTAINER.provides(Some(parents[which.get()].clone())),
            move || {
                let kept = kept.clone();
                LazyVerticalGrid!(GridCells::Fixed(1), move |scope: LazyGridScope| {
                    let kept = kept.clone();
                    scope.item(move || {
                        if let Some(container) = compose_ui::LOCAL_PINNABLE_CONTAINER.current() {
                            kept.borrow_mut().insert(0, container);
                        }
                        Box!(modifier = Modifier.size(10.dp()));
                    });
                });
            },
        );
    })
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, pinningIsPropagatedToParentContainer
#[test]
fn pinning_is_propagated_to_parent_container() {
    let pinned = Rc::new(Cell::new(false));
    let parent: Rc<dyn PinnableContainer> = Rc::new(Parent(pinned.clone()));
    let containers: Containers = Rc::default();
    let mut ui = under(vec![parent], state_of(0), containers.clone());
    let handle = containers.borrow()[&0].pin();
    ui.wait_for_idle();
    assert!(pinned.get());
    handle.release();
    ui.wait_for_idle();
    assert!(!pinned.get());
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridPinnableContainerTest.kt, parentContainerChange_pinningIsMaintained
#[test]
fn parent_container_change_pinning_is_maintained() {
    let (one, other) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let first: Rc<dyn PinnableContainer> = Rc::new(Parent(one.clone()));
    let second: Rc<dyn PinnableContainer> = Rc::new(Parent(other.clone()));
    let which = state_of(0);
    let containers: Containers = Rc::default();
    let mut ui = under(vec![first, second], which, containers.clone());
    let _handle = containers.borrow()[&0].pin();
    ui.wait_for_idle();
    assert!(one.get());
    assert!(!other.get());
    which.set(1);
    ui.wait_for_idle();
    assert!(!one.get());
    assert!(other.get());
}

/// Where pinned items out of view go: side by side before the first line and after the last,
/// as Compose's `extraItemsBefore` and `extraItemsAfter` (D1481), not the lines between.
#[test]
fn pinned_items_out_of_view_are_placed_beside_the_lines() {
    let mut pins = pins(2, 0, state_of(100));
    let _first = pins.pin(0);
    let _second = pins.pin(1);
    pins.scroll_to(4);
    wait_until(&mut pins.ui, missing(&pins.composed, 2));
    let top = |ui: &mut ComposeTestRule, tag: &str| {
        ui.on_node_with_tag(tag).fetch_semantics_snapshot().bounds.top
    };
    assert_eq!((top(&mut pins.ui, "0"), top(&mut pins.ui, "1")), (-20.0, -10.0));
    let _after = pins.pin(5);
    pins.scroll_to(0);
    wait_until(&mut pins.ui, missing(&pins.composed, 4));
    assert_eq!(top(&mut pins.ui, "5"), 20.0);
}
