//! Pinning a lazy list's items, as Compose's `LazyListPinnableContainerTest`, each case with and
//! without a `LookaheadScope` around (its parameter). Items that stop being wanted leave between
//! frames here (D575), so its `waitUntil`s run those requests. A pinned item out of what's laid
//! out is placed beside it, as Compose's `extraItemsBefore` and `extraItemsAfter` (D2024).

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{FocusRequester, IntSize, PinnableContainer, PinnedHandle, UnitExt};

use crate::scroll_support::run;

type Composed = Rc<RefCell<BTreeSet<usize>>>;
type Containers = Rc<RefCell<BTreeMap<usize, Rc<dyn PinnableContainer>>>>;

const LOOKAHEAD: [bool; 2] = [true, false];

/// `content` in a window 1000 square, in a `LookaheadScope` if `lookahead`.
fn rule(lookahead: bool, content: impl Fn() + 'static) -> ComposeTestRule {
    let content = Rc::new(content);
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let content = content.clone();
        if lookahead {
            compose_ui::LookaheadScope(callback(move |_: compose_ui::LookaheadScope| content()));
        } else {
            content();
        }
    });
    ui.wait_for_idle();
    ui
}

/// `Item`: 10 square, tagged with its index, noting it's composed.
fn Item(index: usize, composed: Composed, modifier: Modifier) {
    Box!(modifier = modifier.size(10.dp()).test_tag(index.to_string()));
    DisposableEffect(index, move |effects| {
        composed.borrow_mut().insert(index);
        let composed = composed.clone();
        effects.on_dispose(move || {
            composed.borrow_mut().remove(&index);
        })
    });
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

fn leaked(index: usize) -> LazyListState {
    let owned = LazyListState::new(index, 0);
    let state = *owned;
    std::mem::forget(owned);
    state
}

fn scroll_to(ui: &mut ComposeTestRule, state: LazyListState, index: usize) {
    let _ = run(ui, async move { state.scroll_to_item(index, 0).await });
}

/// Compose's `assertIsNotDisplayed().assertIsPlaced()`: in the tree, out of view.
fn assert_placed_out_of_view(ui: &mut ComposeTestRule, tag: &str) {
    let mut node = ui.on_node_with_tag(tag);
    node.assert_exists();
    assert!(!node.is_displayed(), "{tag} is out of view");
}

struct Pins {
    ui: ComposeTestRule,
    state: LazyListState,
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

    fn top(&mut self, index: usize) -> f32 {
        self.ui.on_node_with_tag(&index.to_string()).fetch_semantics_snapshot().bounds.top
    }
}

/// A column `main` items long of `count` items from `initial`, every item's container kept.
fn pins(
    lookahead: bool,
    main: i32,
    initial: usize,
    count: compose_runtime::MutableState<usize>,
) -> Pins {
    let state = leaked(initial);
    let composed: Composed = Rc::default();
    let containers: Containers = Rc::default();
    let (noted, kept) = (composed.clone(), containers.clone());
    let ui = rule(lookahead, move || {
        let (noted, kept) = (noted.clone(), kept.clone());
        LazyColumn!(
            modifier = Modifier.size((10 * main).dp()),
            state = state,
            move |scope: LazyListScope| {
                let (noted, kept) = (noted.clone(), kept.clone());
                scope.items(count.get(), move |index| {
                    if let Some(container) = compose_ui::LOCAL_PINNABLE_CONTAINER.current() {
                        kept.borrow_mut().insert(index, container);
                    }
                    Item(index, noted.clone(), Modifier);
                });
            }
        );
    });
    Pins { ui, state, composed, containers }
}

fn count_of(count: usize) -> compose_runtime::MutableState<usize> {
    Box::leak(Box::new(mutable_state_of(count))).handle()
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemIsComposedAndPlacedWhenScrolledOut
#[test]
fn pinnedItemIsComposedAndPlacedWhenScrolledOut() {
    for lookahead in LOOKAHEAD {
        let mut pins = pins(lookahead, 2, 0, count_of(100));
        let _handle = pins.pin(1);
        assert!(has(&pins.composed, 1));
        pins.scroll_to(3);
        wait_until(&mut pins.ui, missing(&pins.composed, 0));
        assert!(has(&pins.composed, 1));
        assert_placed_out_of_view(&mut pins.ui, "1");
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, itemsBetweenPinnedAndCurrentVisibleAreNotComposed
#[test]
fn itemsBetweenPinnedAndCurrentVisibleAreNotComposed() {
    for lookahead in LOOKAHEAD {
        let mut pins = pins(lookahead, 2, 0, count_of(100));
        let _handle = pins.pin(1);
        pins.scroll_to(4);
        // Those a round composed too many of leave between frames too (D575).
        let composed = pins.composed.clone();
        wait_until(&mut pins.ui, move || {
            (0..4).filter(|it| composed.borrow().contains(it)).count() == 1
        });
        for (index, expected) in [(0, false), (1, true), (2, false), (3, false), (4, true)] {
            assert_eq!(has(&pins.composed, index), expected, "{index} ({lookahead})");
        }
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemAfterVisibleOnesIsComposedAndPlacedWhenScrolledOut
#[test]
fn pinnedItemAfterVisibleOnesIsComposedAndPlacedWhenScrolledOut() {
    for lookahead in LOOKAHEAD {
        let mut pins = pins(lookahead, 2, 0, count_of(100));
        pins.scroll_to(4);
        wait_until(&mut pins.ui, missing(&pins.composed, 1));
        let _handle = pins.pin(4);
        assert!(has(&pins.composed, 5));
        pins.scroll_to(0);
        if lookahead {
            // As Compose's: another pass ahead, which keeps what the last one composed.
            pins.scroll_to(0);
        }
        wait_until(&mut pins.ui, missing(&pins.composed, 5));
        let expected = [(0, true), (1, true), (2, false), (3, false), (4, true), (5, false)];
        for (index, expected) in expected {
            assert_eq!(has(&pins.composed, index), expected, "{index} ({lookahead})");
        }
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemCanBeUnpinned
#[test]
fn pinnedItemCanBeUnpinned() {
    for lookahead in LOOKAHEAD {
        let mut pins = pins(lookahead, 2, 0, count_of(100));
        let handle = pins.pin(1);
        pins.scroll_to(3);
        wait_until(&mut pins.ui, missing(&pins.composed, 0));
        handle.release();
        wait_until(&mut pins.ui, missing(&pins.composed, 1));
        pins.ui.on_node_with_tag("1").assert_does_not_exist();
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemIsStillPinnedWhenReorderedAndNotVisibleAnymore
#[test]
fn pinnedItemIsStillPinnedWhenReorderedAndNotVisibleAnymore() {
    for lookahead in LOOKAHEAD {
        let state = leaked(0);
        let composed: Composed = Rc::default();
        let containers: Containers = Rc::default();
        let list = mutable_state_of(vec![0usize, 1, 2, 3, 4]);
        let read = list.handle();
        let (noted, kept) = (composed.clone(), containers.clone());
        let mut ui = rule(lookahead, move || {
            let (noted, kept) = (noted.clone(), kept.clone());
            let ids = read.get();
            LazyColumn!(
                modifier = Modifier.size(30.dp()),
                state = state,
                move |scope: LazyListScope| {
                    let (noted, kept) = (noted.clone(), kept.clone());
                    scope.items_list_keyed(
                        ids.clone(),
                        |id: &usize| *id as u64,
                        move |_, id: &usize| {
                            if let Some(container) = compose_ui::LOCAL_PINNABLE_CONTAINER.current()
                            {
                                kept.borrow_mut().insert(*id, container);
                            }
                            Item(*id, noted.clone(), Modifier);
                        },
                    );
                }
            );
        });
        assert_eq!(composed.borrow().iter().copied().collect::<Vec<_>>(), [0, 1, 2]);
        let _handle = containers.borrow()[&2].pin();
        ui.wait_for_idle();
        list.set(vec![0, 3, 4, 1, 2]);
        wait_until(&mut ui, missing(&composed, 1));
        assert_eq!(composed.borrow().iter().copied().collect::<Vec<_>>(), [0, 2, 3, 4]);
        ui.on_node_with_tag("2").assert_exists();
    }
}

/// A column 2 items long of 100 whose state changes, every item's container kept.
fn with_a_new_state(
    lookahead: bool,
    initial: usize,
) -> (ComposeTestRule, compose_runtime::MutableState<LazyListState>, Composed, Containers) {
    let state = Box::leak(Box::new(mutable_state_of(leaked(initial)))).handle();
    let composed: Composed = Rc::default();
    let containers: Containers = Rc::default();
    let (noted, kept) = (composed.clone(), containers.clone());
    let ui = rule(lookahead, move || {
        let (noted, kept) = (noted.clone(), kept.clone());
        LazyColumn!(
            modifier = Modifier.size(20.dp()),
            state = state.get(),
            move |scope: LazyListScope| {
                let (noted, kept) = (noted.clone(), kept.clone());
                scope.items(100, move |index| {
                    if let Some(container) = compose_ui::LOCAL_PINNABLE_CONTAINER.current() {
                        kept.borrow_mut().insert(index, container);
                    }
                    Item(index, noted.clone(), Modifier);
                });
            }
        );
    });
    (ui, state, composed, containers)
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, unpinnedWhenLazyListStateChanges
#[test]
fn unpinnedWhenLazyListStateChanges() {
    for lookahead in LOOKAHEAD {
        let (mut ui, state, composed, containers) = with_a_new_state(lookahead, 2);
        let _handle = containers.borrow()[&2].pin();
        ui.wait_for_idle();
        assert!(has(&composed, 3));
        scroll_to(&mut ui, state.get(), 0);
        if lookahead {
            scroll_to(&mut ui, state.get(), 0);
        }
        wait_until(&mut ui, missing(&composed, 3));
        assert!(has(&composed, 2));
        state.set(leaked(0));
        wait_until(&mut ui, missing(&composed, 2));
        ui.on_node_with_tag("2").assert_does_not_exist();
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinAfterLazyListStateChange
#[test]
fn pinAfterLazyListStateChange() {
    for lookahead in LOOKAHEAD {
        let (mut ui, state, composed, containers) = with_a_new_state(lookahead, 0);
        state.set(leaked(0));
        ui.wait_for_idle();
        let _handle = containers.borrow()[&0].pin();
        ui.wait_for_idle();
        assert!(has(&composed, 1));
        scroll_to(&mut ui, state.get(), 2);
        wait_until(&mut ui, missing(&composed, 1));
        assert!(has(&composed, 0));
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, itemsArePinnedBasedOnGlobalIndexes
#[test]
fn itemsArePinnedBasedOnGlobalIndexes() {
    for lookahead in LOOKAHEAD {
        let state = leaked(3);
        let composed: Composed = Rc::default();
        let containers: Containers = Rc::default();
        let (noted, kept) = (composed.clone(), containers.clone());
        let mut ui = rule(lookahead, move || {
            let (noted, kept) = (noted.clone(), kept.clone());
            LazyColumn!(
                modifier = Modifier.size(20.dp()),
                state = state,
                move |scope: LazyListScope| {
                    for index in 0..100 {
                        let (noted, kept) = (noted.clone(), kept.clone());
                        scope.item(move || {
                            if let Some(container) = compose_ui::LOCAL_PINNABLE_CONTAINER.current()
                            {
                                kept.borrow_mut().insert(index, container);
                            }
                            Item(index, noted.clone(), Modifier);
                        });
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
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemIsRemovedWhenNotVisible
#[test]
fn pinnedItemIsRemovedWhenNotVisible() {
    for lookahead in LOOKAHEAD {
        let count = count_of(10);
        let mut pins = pins(lookahead, 2, 3, count);
        let _handle = pins.pin(3);
        assert!(has(&pins.composed, 4));
        pins.scroll_to(0);
        if lookahead {
            pins.scroll_to(0);
        }
        wait_until(&mut pins.ui, missing(&pins.composed, 4));
        count.set(3);
        wait_until(&mut pins.ui, missing(&pins.composed, 3));
        pins.ui.on_node_with_tag("3").assert_does_not_exist();
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemIsRemovedWhenVisible
#[test]
fn pinnedItemIsRemovedWhenVisible() {
    for lookahead in LOOKAHEAD {
        let composed: Composed = Rc::default();
        let containers: Containers = Rc::default();
        let list = mutable_state_of(vec![0usize, 1, 2]);
        let read = list.handle();
        let (noted, kept) = (composed.clone(), containers.clone());
        let mut ui = rule(lookahead, move || {
            let (noted, kept) = (noted.clone(), kept.clone());
            let ids = read.get();
            LazyColumn!(modifier = Modifier.size(20.dp()), move |scope: LazyListScope| {
                let (noted, kept) = (noted.clone(), kept.clone());
                scope.items_list(ids.clone(), move |_, id: &usize| {
                    if let Some(container) = compose_ui::LOCAL_PINNABLE_CONTAINER.current() {
                        kept.borrow_mut().insert(*id, container);
                    }
                    Item(*id, noted.clone(), Modifier);
                });
            });
        });
        let _handle = containers.borrow()[&1].pin();
        ui.wait_for_idle();
        list.set(vec![0, 2]);
        wait_until(&mut ui, missing(&composed, 1));
        // Compose's `assertIsNotPlaced`: gone, or there and not placed.
        assert!(!crate::lazy_lookahead_support::is_placed(&mut ui, "1"));
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedMultipleTimes
#[test]
fn pinnedMultipleTimes() {
    for lookahead in LOOKAHEAD {
        let mut pins = pins(lookahead, 2, 0, count_of(100));
        let mut handles = vec![pins.pin(1), pins.pin(1)];
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
}

struct Parent(Rc<Cell<bool>>);

impl PinnableContainer for Parent {
    fn pin(&self) -> PinnedHandle {
        self.0.set(true);
        let pinned = self.0.clone();
        PinnedHandle::new(move || pinned.set(false))
    }
}

/// A column of one item under `parents[which]`, the item's container kept.
fn under(
    lookahead: bool,
    parents: Vec<Rc<dyn PinnableContainer>>,
    which: compose_runtime::MutableState<usize>,
    containers: Containers,
) -> ComposeTestRule {
    rule(lookahead, move || {
        let kept = containers.clone();
        CompositionLocalProvider(
            compose_ui::LOCAL_PINNABLE_CONTAINER.provides(Some(parents[which.get()].clone())),
            move || {
                let kept = kept.clone();
                LazyColumn!(move |scope: LazyListScope| {
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

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinningIsPropagatedToParentContainer
#[test]
fn pinningIsPropagatedToParentContainer() {
    for lookahead in LOOKAHEAD {
        let pinned = Rc::new(Cell::new(false));
        let parent: Rc<dyn PinnableContainer> = Rc::new(Parent(pinned.clone()));
        let containers: Containers = Rc::default();
        let mut ui = under(lookahead, vec![parent], count_of(0), containers.clone());
        let handle = containers.borrow()[&0].pin();
        ui.wait_for_idle();
        assert!(pinned.get());
        handle.release();
        ui.wait_for_idle();
        assert!(!pinned.get());
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, parentContainerChange_pinningIsMaintained
#[test]
fn parentContainerChange_pinningIsMaintained() {
    for lookahead in LOOKAHEAD {
        let (one, other) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
        let first: Rc<dyn PinnableContainer> = Rc::new(Parent(one.clone()));
        let second: Rc<dyn PinnableContainer> = Rc::new(Parent(other.clone()));
        let which = count_of(0);
        let containers: Containers = Rc::default();
        let mut ui = under(lookahead, vec![first, second], which, containers.clone());
        let _handle = containers.borrow()[&0].pin();
        ui.wait_for_idle();
        assert!(one.get());
        assert!(!other.get());
        which.set(1);
        ui.wait_for_idle();
        assert!(!one.get());
        assert!(other.get());
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemIsRemovedAfterContainerExitsComposition
#[test]
fn pinnedItemIsRemovedAfterContainerExitsComposition() {
    for lookahead in LOOKAHEAD {
        let active = count_of(1);
        let containers: Containers = Rc::default();
        let kept = containers.clone();
        let mut ui = rule(lookahead, move || {
            if active.get() == 0 {
                return;
            }
            let kept = kept.clone();
            LazyColumn!(modifier = Modifier.size(20.dp()), move |scope: LazyListScope| {
                let kept = kept.clone();
                scope.items(3, move |column| {
                    let kept = kept.clone();
                    LazyRow!(move |scope: LazyListScope| {
                        let kept = kept.clone();
                        scope.items(3, move |row| {
                            if (column, row) == (1, 1)
                                && let Some(container) =
                                    compose_ui::LOCAL_PINNABLE_CONTAINER.current()
                            {
                                kept.borrow_mut().insert(0, container);
                            }
                            Box!(
                                modifier =
                                    Modifier.size(10.dp()).test_tag(format!("{column}:{row}"))
                            );
                        });
                    });
                });
            });
        });
        ui.on_node_with_tag("1:1").assert_exists();
        let _handle = containers.borrow()[&0].pin();
        ui.wait_for_idle();
        active.set(0);
        ui.wait_for_idle();
        ui.on_node_with_tag("1:1").assert_does_not_exist();
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, focusRestorer_shouldRestoreFocusCorrectly
#[test]
fn focusRestorer_shouldRestoreFocusCorrectly() {
    for lookahead in LOOKAHEAD {
        let values = Box::leak(Box::new(mutable_state_of((1..=10).collect::<Vec<u32>>()))).handle();
        let focus_requester = FocusRequester::new();
        let parent_requester = FocusRequester::new();
        let last_item_requester = FocusRequester::new();
        let other_focus_requester = FocusRequester::new();
        let focused: Rc<RefCell<BTreeMap<u32, bool>>> = Rc::default();
        let manager = Rc::new(RefCell::new(None));
        let taken = manager.clone();
        let noted = focused.clone();
        let (fallback_requester, last, parent, other) =
            (focus_requester, last_item_requester, parent_requester, other_focus_requester);
        let mut ui = rule(lookahead, move || {
            *taken.borrow_mut() = Some(compose_ui::LOCAL_FOCUS_MANAGER.current());
            let (noted, fallback_requester, last) = (noted.clone(), fallback_requester, last);
            let state = remember_lazy_list_state!(9);
            Column!({
                Box!(modifier = Modifier.size(50.dp()).focus_requester(other).focusable());
                let (noted, fallback_requester, last) = (noted.clone(), fallback_requester, last);
                LazyRow!(
                    state = state,
                    modifier = Modifier
                        .size(20.dp())
                        .focus_restorer(fallback_requester)
                        .focus_requester(parent)
                        .focusable(),
                    move |scope: LazyListScope| {
                        let (noted, fallback_requester, last) =
                            (noted.clone(), fallback_requester, last);
                        scope.items_list(values.get(), move |_, value: &u32| {
                            let value = *value;
                            let fallback = match value {
                                9 => Modifier.focus_requester(fallback_requester),
                                10 => Modifier.focus_requester(last),
                                _ => Modifier,
                            };
                            let noted = noted.clone();
                            Box!(
                                modifier = Modifier
                                    .size(10.dp())
                                    .then(fallback)
                                    .test_tag(value.to_string())
                                    .on_focus_changed(move |state: compose_ui::FocusState| {
                                        noted.borrow_mut().insert(value, state.is_focused());
                                    })
                                    .focusable()
                            );
                        });
                    }
                );
            });
        });
        assert!(last_item_requester.request_focus());
        ui.wait_for_idle();
        assert!(other_focus_requester.request_focus());
        ui.wait_for_idle();
        let mut shorter = values.get();
        shorter.pop();
        values.set(shorter);
        ui.wait_for_idle();
        assert!(parent_requester.request_focus());
        let manager = manager.borrow().clone().expect("composed");
        assert!(manager.move_focus(compose_ui::FocusDirection::Enter));
        ui.wait_for_idle();
        assert_eq!(focused.borrow().get(&9), Some(&true), "{lookahead}");
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, focusedFocusableItemIsComposedAndPlacedWhenScrolledOut
#[test]
fn focusedFocusableItemIsComposedAndPlacedWhenScrolledOut() {
    for lookahead in LOOKAHEAD {
        let requesters: Rc<Vec<FocusRequester>> =
            Rc::new((0..100).map(|_| FocusRequester::new()).collect());
        let state = leaked(0);
        let composed: Composed = Rc::default();
        let (noted, given) = (composed.clone(), requesters.clone());
        let mut ui = rule(lookahead, move || {
            let (noted, given) = (noted.clone(), given.clone());
            LazyColumn!(
                modifier = Modifier.size(20.dp()),
                state = state,
                move |scope: LazyListScope| {
                    let (noted, given) = (noted.clone(), given.clone());
                    scope.items(100, move |index| {
                        let modifier = Modifier.focus_requester(given[index]).focusable();
                        Item(index, noted.clone(), modifier);
                    });
                }
            );
        });
        requesters[1].request_focus();
        ui.wait_for_idle();
        assert!(has(&composed, 1));
        scroll_to(&mut ui, state, 3);
        wait_until(&mut ui, missing(&composed, 0));
        assert!(has(&composed, 1));
        assert_placed_out_of_view(&mut ui, "1");
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, pinnedItemsAreLaidOutInOrderWhenScrolledOut
#[test]
fn pinnedItemsAreLaidOutInOrderWhenScrolledOut() {
    // Compose's order from `shuffle(Random(0xCAFEBABE))`, any order not ascending standing for it.
    const SHUFFLED: [usize; 10] = [7, 2, 9, 0, 5, 3, 8, 1, 6, 4];
    for lookahead in LOOKAHEAD {
        let mut pins = pins(lookahead, 10, 0, count_of(20));
        let assert_in_order = |pins: &mut Pins, range: std::ops::Range<usize>| {
            let tops: Vec<f32> = range.clone().map(|index| pins.top(index)).collect();
            assert!(tops.windows(2).all(|it| it[0] < it[1]), "{range:?}: {tops:?} ({lookahead})");
        };
        let handles: Vec<PinnedHandle> = SHUFFLED.iter().map(|&it| pins.pin(it)).collect();
        pins.scroll_to(19);
        pins.ui.wait_for_idle();
        assert_in_order(&mut pins, 0..10);
        for handle in handles {
            handle.release();
        }
        let _handles: Vec<PinnedHandle> = SHUFFLED.iter().map(|&it| pins.pin(it + 10)).collect();
        pins.scroll_to(0);
        pins.ui.wait_for_idle();
        assert_in_order(&mut pins, 10..20);
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListPinnableContainerTest.kt, noDuplicatePinnedItemsInPinnedItemsListDuringLayout
#[test]
fn noDuplicatePinnedItemsInPinnedItemsListDuringLayout() {
    for lookahead in LOOKAHEAD {
        let data = Box::leak(Box::new(mutable_state_of(0usize))).handle();
        let state = leaked(0);
        let flag = Rc::new(Cell::new(false));
        let composed: Composed = Rc::default();
        let (noted, set) = (composed.clone(), flag.clone());
        let mut ui = rule(lookahead, move || {
            let (noted, set) = (noted.clone(), set.clone());
            LazyColumn!(
                modifier = Modifier.height(700.dp()),
                state = state,
                move |scope: LazyListScope| {
                    let (noted, set) = (noted.clone(), set.clone());
                    scope.items(data.get(), move |index| {
                        let container = compose_ui::LOCAL_PINNABLE_CONTAINER.current();
                        Item(index, noted.clone(), Modifier.height(300.dp()));
                        if index == 19 {
                            let set = set.clone();
                            DisposableEffect((), move |effects| {
                                set.set(true);
                                let handle = container.as_ref().map(|it| it.pin());
                                let set = set.clone();
                                effects.on_dispose(move || {
                                    // Compose's leaks its pin; the item goes with the data.
                                    std::mem::forget(handle);
                                    set.set(false);
                                })
                            });
                        }
                    });
                }
            );
        });
        for _ in 0..2 {
            data.set(20);
            ui.wait_for_idle();
            scroll_to(&mut ui, state, 19);
            data.set(2);
            ui.wait_for_idle();
            assert!(!flag.get(), "{lookahead}");
        }
    }
}

// Not one of Compose's (its `calculateItemsOffsets`): pinned items out of what's laid out are
// placed side by side before the first item laid out, from the nearest, and after the last, in
// their order, whatever order they were pinned in, and nothing between them and the items laid
// out stays.
#[test]
fn pinned_items_are_placed_side_by_side_beside_those_laid_out() {
    for lookahead in LOOKAHEAD {
        let mut pins = pins(lookahead, 5, 0, count_of(100));
        // Pinned in descending order: 30 first.
        pins.scroll_to(30);
        let _thirty = pins.pin(30);
        pins.scroll_to(20);
        let _twenty = pins.pin(20);
        pins.scroll_to(0);
        wait_until(&mut pins.ui, missing(&pins.composed, 21));
        assert_eq!((pins.top(20), pins.top(30)), (50.0, 60.0), "{lookahead}");
        pins.scroll_to(40);
        let composed = pins.composed.clone();
        wait_until(&mut pins.ui, move || (21..30).all(|it| !composed.borrow().contains(&it)));
        assert_eq!((pins.top(20), pins.top(30)), (-20.0, -10.0), "{lookahead}");
        // The others between stay out.
        let between: Vec<usize> =
            (0..40).filter(|it| has(&pins.composed, *it) && ![20, 30].contains(it)).collect();
        assert!(between.is_empty(), "{between:?} ({lookahead})");
    }
}
