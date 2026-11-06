//! `Modifier.on_visibility_changed`, `on_visibility_changed_node` and `layout_bounds`: whether a
//! node is in the window, or in another node's bounds, past a fraction and for a time
//! (docs/parity/design/layout-rect-callbacks.md). Adapted from Compose's `OnVisibilityChangedTest`,
//! each test run with the modifier and with a node delegating to `on_visibility_changed_node`, as
//! its `useDelegation` parameter; at density 1, in a window of 1000 by 1000.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Delegates, IntSize, Layout, LayoutBoundsHolder, ModifierNode, ModifierNodeElement, UnitExt,
    measure_policy, on_visibility_changed_node,
};

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(1000, 1000), content)
}

/// The calls a callback had, and the last value.
#[derive(Clone, Default)]
struct Calls(Rc<RefCell<Vec<bool>>>);

impl Calls {
    fn callback(&self) -> Callback<dyn Fn(bool)> {
        let calls = self.0.clone();
        callback(move |visible: bool| calls.borrow_mut().push(visible))
    }
    fn count(&self) -> usize {
        self.0.borrow().len()
    }
    fn last(&self) -> Option<bool> {
        self.0.borrow().last().copied()
    }
    fn all(&self) -> Vec<bool> {
        self.0.borrow().clone()
    }
}

/// A node delegating to `on_visibility_changed_node`, as the test's `DelegatingImplNode`.
struct DelegatingElement {
    min_duration: i64,
    min_fraction: f32,
    viewport: Option<LayoutBoundsHolder>,
    callback: Callback<dyn Fn(bool)>,
}

struct DelegatingNode {
    delegates: Delegates,
}

impl ModifierNodeElement for DelegatingElement {
    type Node = DelegatingNode;
    fn create(&self) -> DelegatingNode {
        let mut delegates = Delegates::new();
        delegates.delegate(on_visibility_changed_node(
            self.min_duration,
            self.min_fraction,
            self.viewport.as_ref(),
            self.callback.clone(),
        ));
        DelegatingNode { delegates }
    }
    fn update(&self, _node: &mut DelegatingNode) {
        panic!("this delegating impl doesn't support updating params");
    }
    fn same(&self, other: &Self) -> bool {
        self.min_duration == other.min_duration
            && self.min_fraction == other.min_fraction
            && self.callback.ptr_eq(&other.callback)
    }
}

impl ModifierNode for DelegatingNode {
    fn delegates(&mut self) -> Option<&mut Delegates> {
        Some(&mut self.delegates)
    }
}

/// The test's `onVisibilityChangedTestImpl`: the modifier, or a node delegating to its node.
fn visibility(
    delegated: bool,
    min_duration: i64,
    min_fraction: f32,
    callback: Callback<dyn Fn(bool)>,
) -> Modifier {
    if delegated {
        Modifier.then_custom(DelegatingElement {
            min_duration,
            min_fraction,
            viewport: None,
            callback,
        })
    } else {
        Modifier.on_visibility_changed(min_duration, min_fraction, None, callback)
    }
}

/// A 100 by 100 box `top` down in two columns, its visibility at `fraction` told to `calls`.
fn moved_box(
    delegated: bool,
    fraction: f32,
    calls: &Calls,
    top: &OwnedState<i32>,
) -> impl Fn() + 'static {
    let (callback, top) = (calls.callback(), top.handle());
    move || {
        let callback = callback.clone();
        Column!(|_| {
            let callback = callback.clone();
            Column!(|_| {
                Box!(
                    modifier = Modifier
                        .offset(0.dp(), (top.get() as f32).dp())
                        .then(visibility(delegated, 0, fraction, callback.clone()))
                        .size(100.dp())
                );
            });
        });
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testOneMinFraction
#[test]
fn a_fraction_of_1_needs_it_all_in_the_window() {
    for delegated in [false, true] {
        let top = mutable_state_of(-1000);
        let calls = Calls::default();
        let mut ui = rule(moved_box(delegated, 1.0, &calls, &top));
        // Off the window.
        assert_eq!(calls.count(), 0);
        top.set(-20);
        ui.wait_for_idle();
        // Not all of it in.
        assert_eq!(calls.count(), 0);
        top.set(0);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        top.set(-20);
        ui.wait_for_idle();
        // Partly in: no longer visible.
        assert_eq!((calls.count(), calls.last()), (2, Some(false)));
        top.set(-1000);
        ui.wait_for_idle();
        // Out: already told.
        assert_eq!(calls.count(), 2);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testZeroMinFraction
#[test]
fn a_fraction_of_0_needs_a_pixel_in_the_window() {
    for delegated in [false, true] {
        let top = mutable_state_of(-1000);
        let calls = Calls::default();
        let mut ui = rule(moved_box(delegated, 0.0, &calls, &top));
        assert_eq!(calls.count(), 0);
        top.set(-101);
        ui.wait_for_idle();
        // Touching the window's edge, still out of it.
        assert_eq!(calls.count(), 0);
        top.set(-99);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        top.set(10);
        ui.wait_for_idle();
        assert_eq!(calls.count(), 1);
        top.set(-99);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        top.set(-101);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (2, Some(false)));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testHalfMinFraction
#[test]
fn a_fraction_of_a_half_needs_more_than_half_in_the_window() {
    for delegated in [false, true] {
        let top = mutable_state_of(-1000);
        let calls = Calls::default();
        let mut ui = rule(moved_box(delegated, 0.5, &calls, &top));
        assert_eq!(calls.count(), 0);
        top.set(-99);
        ui.wait_for_idle();
        assert_eq!(calls.count(), 0);
        top.set(-40);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        top.set(10);
        ui.wait_for_idle();
        assert_eq!(calls.count(), 1);
        top.set(-40);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        top.set(-60);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (2, Some(false)));
        top.set(-1000);
        ui.wait_for_idle();
        assert_eq!(calls.count(), 2);
    }
}

/// A 100 by 100 box (`size` by `size`) in two columns, its visibility at `fraction` told.
fn first_load(delegated: bool, fraction: f32, size: f32) {
    let calls = Calls::default();
    let callback = calls.callback();
    let _ui = rule(move || {
        let callback = callback.clone();
        Column!(|_| {
            let callback = callback.clone();
            Column!(|_| {
                Box!(
                    modifier = visibility(delegated, 0, fraction, callback.clone()).size(size.dp())
                );
            });
        });
    });
    assert_eq!((calls.count(), calls.last()), (1, Some(true)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibleCalledOnFirstLoadZeroFraction
#[test]
fn visible_is_told_at_first_with_a_fraction_of_0() {
    for delegated in [false, true] {
        first_load(delegated, 0.0, 100.0);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibleCalledOnFirstLoadOneFraction
#[test]
fn visible_is_told_at_first_with_a_fraction_of_1() {
    for delegated in [false, true] {
        first_load(delegated, 1.0, 100.0);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testOneFractionWithNodeMuchLargerThanViewport
#[test]
fn a_node_larger_than_the_window_filling_it_is_visible() {
    for delegated in [false, true] {
        first_load(delegated, 1.0, 5000.0);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibleNotCalledWhenNotVisibleInitially
#[test]
fn visible_is_told_once_it_s_all_in() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let parent = mutable_state_of(50.0f32);
        let (callback, p) = (calls.callback(), parent.handle());
        let mut ui = rule(move || {
            let callback = callback.clone();
            Column!(|_| {
                let callback = callback.clone();
                Column!(modifier = Modifier.size(p.get().dp()), |_| {
                    Box!(
                        modifier = Modifier.required_size(100.dp()).then(visibility(
                            delegated,
                            0,
                            1.0,
                            callback.clone()
                        ))
                    );
                });
            });
        });
        assert_eq!(calls.count(), 0);
        parent.set(100.0);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibleCalledWhenRemovedFromComposition
#[test]
fn leaving_the_composition_is_told_as_not_visible() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let composed = mutable_state_of(true);
        let (callback, c) = (calls.callback(), composed.handle());
        let mut ui = rule(move || {
            let callback = callback.clone();
            Column!(|_| {
                let callback = callback.clone();
                Column!(|_| {
                    if c.get() {
                        Box!(
                            modifier =
                                visibility(delegated, 0, 1.0, callback.clone()).size(100.dp())
                        );
                    }
                });
            });
        });
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        composed.set(false);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (2, Some(false)));
    }
}

/// The box with the modifier while `with` holds, in two columns.
fn box_with_or_without(
    delegated: bool,
    calls: &Calls,
    with: &OwnedState<bool>,
) -> impl Fn() + 'static {
    let (callback, w) = (calls.callback(), with.handle());
    move || {
        let modifier =
            if w.get() { visibility(delegated, 0, 1.0, callback.clone()) } else { Modifier };
        Column!(|_| {
            let modifier = modifier.clone();
            Column!(|_| {
                Box!(modifier = modifier.clone().size(100.dp()));
            });
        });
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibleCalledWhenRemovedFromModifierChain
#[test]
fn leaving_the_modifier_chain_is_told_as_not_visible() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let with = mutable_state_of(true);
        let mut ui = rule(box_with_or_without(delegated, &calls, &with));
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        with.set(false);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (2, Some(false)));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibleCalledWhenAddedToModifierChain
#[test]
fn coming_into_the_modifier_chain_is_told_as_visible() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let with = mutable_state_of(false);
        let mut ui = rule(box_with_or_without(delegated, &calls, &with));
        assert_eq!(calls.count(), 0);
        with.set(true);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testPassingParentsBounds_parentAndChildMovesTogether_noExtraCallbacks
#[test]
fn a_child_moving_with_its_viewport_isn_t_told_again() {
    let parent = LayoutBoundsHolder::new();
    let calls = Calls::default();
    let offset = mutable_state_of(0.0f32);
    let (callback, o, holder) = (calls.callback(), offset.handle(), parent.clone());
    let mut ui = rule(move || {
        let (callback, holder) = (callback.clone(), holder.clone());
        Column!(modifier = Modifier.size(200.dp()).offset(o.get().dp(), o.get().dp()), |_| {
            let (callback, holder) = (callback.clone(), holder.clone());
            Column!(modifier = Modifier.size(100.dp()).layout_bounds(&holder), |_| {
                Box!(
                    modifier = Modifier.size(100.dp()).on_visibility_changed(
                        0,
                        1.0,
                        Some(&holder),
                        callback.clone()
                    )
                );
            });
        });
    });
    assert_eq!(calls.all(), vec![true]);
    offset.set(1.0);
    ui.wait_for_idle();
    assert_eq!(calls.all(), vec![true]);
}

/// A viewport's bounds coming after the node's: the node waits for them, then is told.
// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/layout/OnVisibilityChangedModifier.kt, checkVisibility
#[test]
fn a_node_outside_its_viewport_isn_t_visible_in_the_window() {
    let viewport = LayoutBoundsHolder::new();
    let calls = Calls::default();
    let (callback, holder) = (calls.callback(), viewport.clone());
    let _ui = rule(move || {
        let (callback, holder) = (callback.clone(), holder.clone());
        Column!(|_| {
            Box!(modifier = Modifier.size(100.dp()).layout_bounds(&holder));
            // In the window, below its viewport.
            Box!(
                modifier = Modifier.size(100.dp()).on_visibility_changed(
                    0,
                    0.0,
                    Some(&holder),
                    callback.clone()
                )
            );
        });
    });
    assert_eq!(calls.count(), 0);
    let bounds = viewport.bounds().expect("laid out");
    assert_eq!(bounds.bounds_in_root(), compose_ui::IntRect::new(0, 0, 100, 100));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibleCalledWhenUnplacedAndThenPlacedAgain
#[test]
fn being_unplaced_and_placed_again_is_told() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let placed = mutable_state_of(true);
        let (callback, p) = (calls.callback(), placed.handle());
        let mut ui = rule(move || {
            let callback = callback.clone();
            Layout!(
                measure_policy = measure_policy(move |scope, measurables, constraints| {
                    let placeable = scope.measure(measurables[0], constraints);
                    scope.layout_with(placeable.width, placeable.height, move |placement| {
                        if p.get() {
                            placement.place(&placeable, 0, 0);
                        }
                    })
                }),
                {
                    Box!(modifier = visibility(delegated, 0, 1.0, callback.clone()).size(100.dp()));
                }
            );
        });
        assert_eq!((calls.count(), calls.last()), (1, Some(true)));
        placed.set(false);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (2, Some(false)));
        placed.set(true);
        ui.wait_for_idle();
        assert_eq!((calls.count(), calls.last()), (3, Some(true)));
    }
}

/// A box in a box while `composed` holds, waiting `min_duration` to be visible.
fn disposable(
    delegated: bool,
    min_duration: i64,
    calls: &Calls,
    composed: &OwnedState<bool>,
) -> impl Fn() + 'static {
    let (callback, c) = (calls.callback(), composed.handle());
    move || {
        let callback = callback.clone();
        Box!({
            if c.get() {
                Box!(
                    modifier =
                        visibility(delegated, min_duration, 1.0, callback.clone()).size(100.dp())
                );
            }
        });
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testNotVisibleCalledWhenDisposed_nonZeroMinDurationMs
#[test]
fn leaving_after_the_minimum_duration_is_told() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let composed = mutable_state_of(true);
        let mut ui = rule(disposable(delegated, 100, &calls, &composed));
        // Not before the minimum duration.
        assert_eq!(calls.count(), 0);
        ui.wait_until_with(None, Duration::from_millis(5000), |_| calls.count() > 0);
        assert_eq!(calls.all(), vec![true]);
        composed.set(false);
        ui.wait_for_idle();
        assert_eq!(calls.all(), vec![true, false]);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testNoCallbacksCalledWhenDisposedBeforeMinDurationMsPassed
#[test]
fn leaving_before_the_minimum_duration_tells_nothing() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let composed = mutable_state_of(true);
        let mut ui = rule(disposable(delegated, 1_000_000, &calls, &composed));
        assert_eq!(calls.count(), 0);
        composed.set(false);
        ui.wait_for_idle();
        ui.advance_time_by(Duration::from_secs(2000));
        assert_eq!(calls.count(), 0);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testNotVisibleNotCalledWhenWasVisibleForLessThanMinDuration
#[test]
fn hidden_before_the_minimum_duration_tells_nothing() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let parent = mutable_state_of(100.0f32);
        let (callback, p) = (calls.callback(), parent.handle());
        let mut ui = rule(move || {
            let callback = callback.clone();
            Column!(|_| {
                let callback = callback.clone();
                Column!(modifier = Modifier.size(p.get().dp()), |_| {
                    Box!(
                        modifier = Modifier.required_size(100.dp()).then(visibility(
                            delegated,
                            1_000_000,
                            1.0,
                            callback.clone()
                        ))
                    );
                });
            });
        });
        assert_eq!(calls.count(), 0);
        parent.set(50.0);
        ui.wait_for_idle();
        ui.advance_time_by(Duration::from_secs(2000));
        assert_eq!(calls.count(), 0);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnVisibilityChangedTest.kt, testVisibilityChangedWhenReused
#[test]
fn deactivated_content_is_told_not_visible_then_visible_again() {
    for delegated in [false, true] {
        let calls = Calls::default();
        let active = mutable_state_of(true);
        let (callback, a) = (calls.callback(), active.handle());
        let mut ui = rule(move || {
            let callback = callback.clone();
            ReusableContentHost(a.get(), || {
                Box!(
                    modifier = Modifier.size(50.dp()).then(visibility(
                        delegated,
                        0,
                        1.0,
                        callback.clone()
                    ))
                );
            });
        });
        assert_eq!(calls.last(), Some(true));
        active.set(false);
        ui.wait_for_idle();
        assert_eq!(calls.last(), Some(false));
        active.set(true);
        ui.wait_for_idle();
        assert_eq!(calls.last(), Some(true));
    }
}

/// `LayoutBoundsHolder` keeps its node's bounds, and forgets them as it leaves.
// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/layout/LayoutBoundsHolder.kt, LayoutBoundsNode
#[test]
fn a_holder_keeps_its_node_s_bounds_while_it_s_there() {
    let holder = LayoutBoundsHolder::new();
    let composed = mutable_state_of(true);
    let (h, c) = (holder.clone(), composed.handle());
    let mut ui = rule(move || {
        let h = h.clone();
        Column!(|_| {
            Box!(modifier = Modifier.size(30.dp()));
            if c.get() {
                Box!(modifier = Modifier.padding(5.dp()).layout_bounds(&h).size(10.dp()));
            }
        });
    });
    assert_eq!(
        holder.bounds().map(|it| it.bounds_in_root()),
        Some(compose_ui::IntRect::new(5, 35, 15, 45))
    );
    composed.set(false);
    ui.wait_for_idle();
    assert_eq!(holder.bounds(), None);
}
