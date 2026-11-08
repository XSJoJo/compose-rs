//! Compose's `TestRuleExecutesLayoutPassesWhenWaitingForIdleTest` (ui, device tests): waiting
//! for idle runs the measurements and placements an animation's frames ask for, in a layout and
//! in a `SubcomposeLayout`'s slot composed while it measures.
//!
//! The three tests through an `AndroidView` are n/a (Android views). The two composing a slot
//! in a placement block aren't ported: compose-rs's `SubcomposeLayout` composes while it
//! measures only (its placement scope can measure, not compose).

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::subcompose::*;
use compose_ui::{Layout, MeasureResult, MeasureScope, measure_policy};

const UPDATES: i32 = 5;

type Count = Rc<Cell<i32>>;

fn bump(count: &Count) {
    count.set(count.get() + 1);
}

/// Bumps `state` at each of the next `UPDATES` frames, as the test's `LaunchedEffect`.
fn animate(state: MutableState<i32>) {
    LaunchedEffect((), async move {
        for _ in 0..UPDATES {
            with_frame_nanos(move |_| state.set(state.get() + 1)).await;
        }
    });
}

/// A 1 × 1 layout counting its measurements and placements, reading `state` in its
/// measurement or (`in_placement`) its placement block.
fn counting_layout(state: MutableState<i32>, in_placement: bool, counts: &(Count, Count)) {
    let (measures, placements) = counts.clone();
    Layout(
        Modifier,
        measure_policy(move |scope: &mut MeasureScope<'_>, _, _| -> MeasureResult {
            bump(&measures);
            if !in_placement {
                state.get();
            }
            let placements = placements.clone();
            scope.layout_with(1, 1, move |_| {
                bump(&placements);
                if in_placement {
                    state.get();
                }
            })
        }),
        callback(|| {}),
    );
}

/// The measurements and placements of a counting layout reading the animated state in its
/// measurement or its placement block, in a `SubcomposeLayout`'s slot composed while it
/// measures if `subcomposed`.
fn run(in_placement: bool, subcomposed: bool) -> (i32, i32) {
    let state = mutable_state_of(0);
    let counts: (Count, Count) = Default::default();
    let (s, c) = (state.handle(), counts.clone());
    let mut ui = compose_test(move || {
        if subcomposed {
            let c = c.clone();
            SubcomposeLayout!(
                measure_policy = subcompose_policy(move |scope, constraints| {
                    let c = c.clone();
                    let content = callback(move || counting_layout(s, in_placement, &c));
                    let measurables = scope.subcompose((), content);
                    let [measurable] = measurables[..] else { panic!("one layout") };
                    let placeable = scope.measure(measurable, constraints);
                    scope.layout_with(1, 1, move |placement| placement.place(&placeable, 0, 0))
                })
            );
        } else {
            counting_layout(s, in_placement, &c);
        }
        animate(s);
    });
    ui.run_on_idle(|_| (counts.0.get(), counts.1.get()))
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/TestRuleExecutesLayoutPassesWhenWaitingForIdleTest.kt, measure_animation
#[test]
fn measure_animation() {
    assert_eq!(run(false, false), (UPDATES + 1, UPDATES + 1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/TestRuleExecutesLayoutPassesWhenWaitingForIdleTest.kt, placement_animation
#[test]
fn placement_animation() {
    assert_eq!(run(true, false), (1, UPDATES + 1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/TestRuleExecutesLayoutPassesWhenWaitingForIdleTest.kt, SubcomposeLayout_compose_in_measure_animate_measure
#[test]
fn subcompose_layout_compose_in_measure_animate_measure() {
    assert_eq!(run(false, true), (UPDATES + 1, UPDATES + 1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/TestRuleExecutesLayoutPassesWhenWaitingForIdleTest.kt, SubcomposeLayout_compose_in_measure_animate_placement
#[test]
fn subcompose_layout_compose_in_measure_animate_placement() {
    assert_eq!(run(true, true), (1, UPDATES + 1));
}
