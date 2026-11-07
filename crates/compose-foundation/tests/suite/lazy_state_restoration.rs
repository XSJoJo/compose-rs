//! Compose's `LazyLayoutStateRestorationTest`, rewritten on the public `LazyLayout` (D1325):
//! saveable state in lazy items, across recreating the host and items leaving and coming back.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::lazy::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{IntSize, UnitExt};

/// What each item got from `remember_saveable`, by the value it starts from.
type Told = Rc<RefCell<Vec<i32>>>;

fn counter_item(index: usize, told: Told, next: Rc<[Cell<i32>; 3]>) {
    let value = remember_saveable(move || {
        let value = next[index].get();
        next[index].set(value + 1);
        value
    });
    told.borrow_mut()[index] = value;
    Box!(modifier = Modifier.required_size(1.dp()));
}

fn counters() -> Rc<[Cell<i32>; 3]> {
    Rc::new([Cell::new(1), Cell::new(10), Cell::new(100)])
}

fn big_rule() -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(300, 300), || {})
}

/// The items of Compose's test's `LazyLayout` helper: `count()` of them, keyed by `key`, each
/// composed by `content`.
struct Items {
    count: Box<dyn Fn() -> usize>,
    key: Box<dyn Fn(usize) -> u64>,
    content: Box<dyn Fn(usize)>,
}

impl LazyLayoutItemProvider for Items {
    fn item_count(&self) -> usize {
        (self.count)()
    }

    fn item(&self, index: usize, _key: u64) {
        (self.content)(index);
    }

    fn key(&self, index: usize) -> u64 {
        (self.key)(index)
    }
}

/// Compose's test's `LazyLayout(itemCount, itemIsVisible, indexToKey, content)`: the items
/// `visible` says composed, each measured at the layout's constraints and placed at its origin.
fn test_layout(
    count: impl Fn() -> usize + 'static,
    visible: impl Fn(usize) -> bool + 'static,
    key: impl Fn(usize) -> u64 + 'static,
    content: impl Fn(usize) + 'static,
) {
    let items: Rc<dyn LazyLayoutItemProvider> = remember(|| {
        Rc::new(Items { count: Box::new(count), key: Box::new(key), content: Box::new(content) })
            as Rc<dyn LazyLayoutItemProvider>
    });
    let counted = items.clone();
    let policy = remember(|| {
        lazy_layout_measure_policy(move |scope, constraints| {
            let mut placeables = Vec::new();
            for index in 0..counted.item_count() {
                if visible(index) {
                    for layout in scope.compose(index) {
                        placeables.push(scope.measure(layout, constraints));
                    }
                }
            }
            for placeable in &placeables {
                scope.place(placeable, 0, 0);
            }
            scope.layout(constraints.max_width, constraints.max_height)
        })
    });
    LazyLayout!(move || items.clone(), measure_policy = policy);
}

fn all(_: usize) -> bool {
    true
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutStateRestorationTest.kt, visibleItemsStateRestored
#[test]
fn the_items_composed_get_their_state_back() {
    let told: Told = Rc::new(RefCell::new(vec![0; 3]));
    let (given, next) = (told.clone(), counters());
    let mut tester = StateRestorationTester::new();
    let mut ui = big_rule();
    tester.set_content(&mut ui, move || {
        let (given, next) = (given.clone(), next.clone());
        test_layout(
            || 3,
            all,
            get_default_lazy_layout_key,
            move |index| counter_item(index, given.clone(), next.clone()),
        );
    });
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), [1, 10, 100]);
    *told.borrow_mut() = vec![0; 3];
    tester.emulate_save_and_restore(&mut ui);
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), [1, 10, 100]);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutStateRestorationTest.kt, stateRestoredWhenUsedWithCustomKeys
#[test]
fn keyed_items_get_their_state_back() {
    let told: Told = Rc::new(RefCell::new(vec![0; 3]));
    let (given, next) = (told.clone(), counters());
    let mut tester = StateRestorationTester::new();
    let mut ui = big_rule();
    tester.set_content(&mut ui, move || {
        let (given, next) = (given.clone(), next.clone());
        test_layout(
            || 3,
            all,
            |index| hash_key(&format!("{index}")),
            move |index| counter_item(index, given.clone(), next.clone()),
        );
    });
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), [1, 10, 100]);
    *told.borrow_mut() = vec![0; 3];
    tester.emulate_save_and_restore(&mut ui);
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), [1, 10, 100]);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutStateRestorationTest.kt, stateRestoredWhenUsedWithCustomKeysAfterReordering
#[test]
fn keyed_items_reordered_get_their_state_back() {
    let told: Told = Rc::new(RefCell::new(vec![0; 3]));
    let (given, next) = (told.clone(), counters());
    let order = mutable_state_of(vec![0usize, 1, 2]);
    let read = order.handle();
    let mut tester = StateRestorationTester::new();
    let mut ui = big_rule();
    tester.set_content(&mut ui, move || {
        let (given, next) = (given.clone(), next.clone());
        test_layout(
            move || read.with(Vec::len),
            all,
            move |position| hash_key(&format!("{}", read.with(|order| order[position]))),
            move |position| {
                let item = read.with(|order| order[position]);
                counter_item(item, given.clone(), next.clone())
            },
        );
    });
    ui.wait_for_idle();
    order.set(vec![1, 2, 0]);
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), [1, 10, 100]);
    *told.borrow_mut() = vec![0; 3];
    tester.emulate_save_and_restore(&mut ui);
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), [1, 10, 100]);
}

/// Item 0's saveable value, from `next`, told in `told`; `gone` once its effects are disposed.
fn first_item(next: &Rc<Cell<i32>>, told: &Rc<Cell<i32>>, gone: &Rc<Cell<bool>>) {
    let counting = next.clone();
    told.set(remember_saveable(move || {
        let value = counting.get();
        counting.set(value + 1);
        value
    }));
    let gone = gone.clone();
    DisposableEffect((), move |scope| scope.on_dispose(move || gone.set(true)));
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutStateRestorationTest.kt, itemsStateRestoredWhenWeScrolledBackToIt
#[test]
fn an_item_composed_again_gets_its_state_back() {
    let (next, told, disposed) =
        (Rc::new(Cell::new(1)), Rc::new(Cell::new(0)), Rc::new(Cell::new(false)));
    let shown = mutable_state_of(0usize);
    let (counting, telling, gone, showing) =
        (next.clone(), told.clone(), disposed.clone(), shown.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), move || {
        let (counting, telling, gone) = (counting.clone(), telling.clone(), gone.clone());
        test_layout(
            || 2,
            move |index| index == showing.get(),
            get_default_lazy_layout_key,
            move |index| {
                if index == 0 {
                    first_item(&counting, &telling, &gone);
                }
                Box!(modifier = Modifier.required_size(30.dp()));
            },
        );
    });
    ui.wait_for_idle();
    assert_eq!(told.get(), 1);
    shown.set(1);
    ui.wait_for_idle();
    assert!(disposed.get());
    told.set(0);
    shown.set(0);
    ui.wait_for_idle();
    assert_eq!(told.get(), 1);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutStateRestorationTest.kt, nestedLazy_itemsStateRestoredWhenWeScrolledBackToIt
#[test]
fn a_nested_layouts_item_composed_again_gets_its_state_back() {
    let (next, told, disposed) =
        (Rc::new(Cell::new(1)), Rc::new(Cell::new(0)), Rc::new(Cell::new(false)));
    let shown = mutable_state_of(0usize);
    let (counting, telling, gone, showing) =
        (next.clone(), told.clone(), disposed.clone(), shown.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), move || {
        let (counting, telling, gone) = (counting.clone(), telling.clone(), gone.clone());
        test_layout(
            || 2,
            move |index| index == showing.get(),
            get_default_lazy_layout_key,
            move |index| {
                if index == 0 {
                    let (counting, telling, gone) =
                        (counting.clone(), telling.clone(), gone.clone());
                    test_layout(
                        || 1,
                        all,
                        get_default_lazy_layout_key,
                        move |_| {
                            first_item(&counting, &telling, &gone);
                            Box!(modifier = Modifier.required_size(30.dp()));
                        },
                    );
                } else {
                    Box!(modifier = Modifier.required_size(30.dp()));
                }
            },
        );
    });
    ui.wait_for_idle();
    assert_eq!(told.get(), 1);
    shown.set(1);
    ui.wait_for_idle();
    assert!(disposed.get());
    told.set(0);
    shown.set(0);
    ui.wait_for_idle();
    assert_eq!(told.get(), 1);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/layout/LazyLayoutStateRestorationTest.kt, itemsStateRestoredOnlyForVisibleItemsWhenStateSaved_100items
#[test]
fn of_a_hundred_items_those_composed_when_saved_get_their_state_back() {
    let told: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(vec![0; 100]));
    let to_use = Rc::new(Cell::new(1));
    let shown = mutable_state_of(0..90usize);
    let (given, using, showing) = (told.clone(), to_use.clone(), shown.handle());
    let mut tester = StateRestorationTester::new();
    let mut ui = big_rule();
    tester.set_content(&mut ui, move || {
        let (given, using) = (given.clone(), using.clone());
        test_layout(
            || 100,
            move |index| showing.with(|shown| shown.contains(&index)),
            get_default_lazy_layout_key,
            move |index| {
                let using = using.clone();
                given.borrow_mut()[index] = remember_saveable(move || using.get());
                Box!(modifier = Modifier.required_size(30.dp()));
            },
        );
    });
    ui.wait_for_idle();
    shown.set(90..100);
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), vec![1; 100]);
    *told.borrow_mut() = vec![0; 100];
    to_use.set(2);
    tester.emulate_save_and_restore(&mut ui);
    ui.wait_for_idle();
    shown.set(0..90);
    ui.wait_for_idle();
    assert_eq!(told.borrow()[90..100], vec![1; 10][..], "composed when saved");
    // Compose's `LazySaveableStateHolder` drops the state of the keys not composed when it saves
    // (2 here); compose-rs's holder keeps it (D290), as its lists' do.
    assert_eq!(told.borrow()[0..90], vec![1; 90][..], "their state stays with the holder");
}
