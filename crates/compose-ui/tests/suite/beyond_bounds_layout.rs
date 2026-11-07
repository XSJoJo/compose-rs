//! `BeyondBoundsLayout`, found from a node inside its provider and through the deprecated
//! modifier local (D1919): Compose's `BeyondBoundsLayoutProviderModifierNodeTest` and
//! `BeyondBoundsLayoutTest`, rewritten.

use crate::support;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use compose_ui::BeyondBoundsLayoutDirection::After;
use compose_ui::platform::UiHost;
use compose_ui::*;
use support::*;

/// The result of an operation run once the items it needs are laid out.
const OPERATION_RESULT: i32 = 10;

/// A provider with nothing beyond its bounds: it never calls the block.
struct WithoutItems;

impl BeyondBoundsLayout for WithoutItems {
    fn layout_until(
        &self,
        _direction: BeyondBoundsLayoutDirection,
        _block: &mut dyn FnMut(&dyn BeyondBoundsScope) -> bool,
    ) {
    }
}

/// A provider with five items beyond its bounds, laid out one at a time.
struct FiveItems;

struct Left(usize);

impl BeyondBoundsScope for Left {
    fn has_more_content(&self) -> bool {
        self.0 > 0
    }
}

impl BeyondBoundsLayout for FiveItems {
    fn layout_until(
        &self,
        _direction: BeyondBoundsLayoutDirection,
        block: &mut dyn FnMut(&dyn BeyondBoundsScope) -> bool,
    ) {
        for left in (0..5).rev() {
            if block(&Left(left)) {
                return;
            }
        }
    }
}

/// The element of a node providing `layout`.
#[derive(Clone)]
struct Provides(Rc<dyn BeyondBoundsLayout>);

struct ProviderNode(Rc<dyn BeyondBoundsLayout>);

impl ModifierNode for ProviderNode {
    fn is_beyond_bounds_layout(&self) -> bool {
        true
    }
    fn beyond_bounds_layout(&mut self) -> Option<Rc<dyn BeyondBoundsLayout>> {
        Some(self.0.clone())
    }
}

impl ModifierNodeElement for Provides {
    type Node = ProviderNode;
    fn create(&self) -> ProviderNode {
        ProviderNode(self.0.clone())
    }
    fn update(&self, node: &mut ProviderNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

type Parent = Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>>;

/// The element of a node capturing the beyond-bounds layout around it when attached, as the
/// tests' `ParentCapturingNode`.
#[derive(Clone)]
struct Captures(Parent);

struct CapturingNode(Parent);

impl ModifierNode for CapturingNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        if let Some(found) = context.find_nearest_beyond_bounds_layout_ancestor() {
            *self.0.borrow_mut() = Some(found);
        }
    }
}

impl ModifierNodeElement for Captures {
    type Node = CapturingNode;
    fn create(&self) -> CapturingNode {
        CapturingNode(self.0.clone())
    }
    fn update(&self, node: &mut CapturingNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// How the tests reach the provider: through a node inside it, or the modifier local.
#[derive(Clone, Copy, Debug)]
enum Route {
    Node,
    ModifierLocal,
}

const ROUTES: [Route; 2] = [Route::Node, Route::ModifierLocal];

/// A leaf whose modifier provides `layout` and captures it after, by `route`; returns the host
/// and what was captured.
fn parent_of(
    layout: Rc<dyn BeyondBoundsLayout>,
    route: Route,
) -> (UiHost, Rc<dyn BeyondBoundsLayout>) {
    let parent = Parent::default();
    let captured = parent.clone();
    let host = host(move || {
        let modifier = match route {
            Route::Node => Modifier
                .then_custom(Provides(layout.clone()))
                .then_custom(Captures(captured.clone())),
            Route::ModifierLocal => {
                let (layout, captured) = (layout.clone(), captured.clone());
                Modifier
                    .modifier_local_provider(&MODIFIER_LOCAL_BEYOND_BOUNDS_LAYOUT, move || {
                        Some(layout.clone())
                    })
                    .modifier_local_consumer(move |scope| {
                        *captured.borrow_mut() =
                            scope.current(&MODIFIER_LOCAL_BEYOND_BOUNDS_LAYOUT);
                    })
            }
        };
        Leaf!(modifier);
    });
    let found = parent.borrow().clone().expect("the provider was found");
    (host, found)
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, noBeyondBoundsItems
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutTest.kt, noBeyondBoundsItems
#[test]
fn nothing_beyond_the_bounds_never_runs_the_block() {
    for route in ROUTES {
        let (_host, parent) = parent_of(Rc::new(WithoutItems), route);
        let invoked = Cell::new(false);
        let returned = parent.layout(After, |_| {
            invoked.set(true);
            Some(OPERATION_RESULT)
        });
        assert!(!invoked.get(), "{route:?}");
        assert_eq!(returned, None, "{route:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, noItemFound
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutTest.kt, noItemFound
#[test]
fn a_block_finding_nothing_runs_for_each_item() {
    for route in ROUTES {
        let (_host, parent) = parent_of(Rc::new(FiveItems), route);
        let count = Cell::new(0);
        let returned = parent.layout::<i32>(After, |_| {
            count.set(count.get() + 1);
            None
        });
        assert_eq!(count.get(), 5, "{route:?}");
        assert_eq!(returned, None, "{route:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, hasMoreItemsReturnsFalseWhenItemsRunOut
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutTest.kt, hasMoreItemsReturnsFalseWhenItemsRunOut
#[test]
fn has_more_content_is_false_at_the_last_item() {
    for route in ROUTES {
        let (_host, parent) = parent_of(Rc::new(FiveItems), route);
        let calls = RefCell::new(BTreeMap::new());
        let returned = parent.layout(After, |scope| {
            let value = if scope.has_more_content() { None } else { Some(OPERATION_RESULT) };
            let count = calls.borrow().len() + 1;
            calls.borrow_mut().insert(count, value);
            value
        });
        let expected = BTreeMap::from([
            (1, None),
            (2, None),
            (3, None),
            (4, None),
            (5, Some(OPERATION_RESULT)),
        ]);
        assert_eq!(*calls.borrow(), expected, "{route:?}");
        assert_eq!(returned, Some(OPERATION_RESULT), "{route:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, itemFoundOnFirstIteration
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutTest.kt, itemFoundOnFirstIteration
#[test]
fn a_block_finding_its_item_at_once_returns_it() {
    for route in ROUTES {
        let (_host, parent) = parent_of(Rc::new(FiveItems), route);
        assert_eq!(parent.layout(After, |_| Some(OPERATION_RESULT)), Some(OPERATION_RESULT));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, itemFoundOnThirdIteration
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutTest.kt, itemFoundOnThirdIteration
#[test]
fn a_block_finding_its_item_later_returns_it() {
    for route in ROUTES {
        let (_host, parent) = parent_of(Rc::new(FiveItems), route);
        let count = Cell::new(0);
        let returned = parent.layout(After, |_| {
            count.set(count.get() + 1);
            if count.get() <= 3 { None } else { Some(OPERATION_RESULT) }
        });
        assert_eq!(returned, Some(OPERATION_RESULT), "{route:?}");
        assert_eq!(count.get(), 4, "{route:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, iteratorCountWhenCalledMultipleTimes
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutTest.kt, iteratorCountWhenCalledMultipleTimes
#[test]
fn each_layout_starts_over() {
    for route in ROUTES {
        let (_host, parent) = parent_of(Rc::new(FiveItems), route);
        let (first, second) = (Cell::new(0), Cell::new(0));
        let returned_first = parent.layout::<i32>(After, |_| {
            first.set(first.get() + 1);
            None
        });
        let returned_second = parent.layout::<i32>(After, |_| {
            second.set(second.get() + 1);
            None
        });
        assert_eq!((first.get(), second.get()), (5, 5), "{route:?}");
        assert_eq!((returned_first, returned_second), (None, None), "{route:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, reentrantIteratorCount
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutTest.kt, reentrantIteratorCount
#[test]
fn a_layout_inside_a_block_has_its_own_count() {
    for route in ROUTES {
        let (_host, parent) = parent_of(Rc::new(FiveItems), route);
        let (first, second) = (Cell::new(0), Cell::new(0));
        let returned_second = Cell::new(Some(-1));
        let returned_first = parent.layout::<i32>(After, |scope| {
            first.set(first.get() + 1);
            if !scope.has_more_content() {
                returned_second.set(parent.layout::<i32>(After, |_| {
                    second.set(second.get() + 1);
                    None
                }));
            }
            None
        });
        assert_eq!((first.get(), second.get()), (5, 5), "{route:?}");
        assert_eq!((returned_first, returned_second.get()), (None, None), "{route:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/BeyondBoundsLayoutProviderModifierNodeTest.kt, noBeyondBoundsItems (no provider around: none found)
#[test]
fn without_a_provider_around_none_is_found() {
    let parent = Parent::default();
    let captured = parent.clone();
    let _host = host(move || {
        Leaf!(Modifier.then_custom(Captures(captured.clone())));
    });
    assert!(parent.borrow().is_none());
}
