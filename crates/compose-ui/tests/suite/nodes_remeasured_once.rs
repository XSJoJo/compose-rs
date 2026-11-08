//! Compose's `NodesRemeasuredOnceTest` (ui, device tests): a child whose height state changes
//! is measured once more, and its parent sees the new height once, through layers of wrapping
//! layouts, a parent not placing it, and parents measuring it in their placement block (with
//! intrinsics asked first in one).

use std::cell::Cell;
use std::rc::Rc;

use crate::support::*;

use compose_runtime::*;
use compose_ui::*;

type OnMeasured = Rc<dyn Fn(i32)>;

/// Its first child measured at any height, and as large, as the test's `WrapChild`.
fn wrap_child(on_measured: Option<OnMeasured>, content: impl Fn() + 'static) {
    Layout(
        Modifier,
        measure_policy(move |scope, measurables, constraints| {
            let loose = Constraints::new(
                constraints.min_width,
                constraints.max_width,
                0,
                Constraints::INFINITY,
            );
            let placeable = scope.measure(measurables[0], loose);
            if let Some(on_measured) = &on_measured {
                on_measured(placeable.height);
            }
            scope.place(&placeable, 0, 0);
            scope.layout(placeable.width, placeable.height)
        }),
        callback(content),
    );
}

/// The largest size it may take, its first child measured (at any height) in its placement
/// block, as the test's `WrapChildMeasureDuringLayout`; with `intrinsics`, its size is the
/// child's maximum intrinsic size instead, as `IntrinsicSizeAndMeasureDuringLayout`.
fn measure_during_layout(intrinsics: bool, on_measured: OnMeasured, content: impl Fn() + 'static) {
    Layout(
        Modifier,
        measure_policy(move |scope, measurables, constraints| {
            let child = measurables[0];
            let (width, height) = if intrinsics {
                (
                    scope.max_intrinsic_width(child, constraints.max_width),
                    scope.max_intrinsic_height(child, constraints.max_height),
                )
            } else {
                (
                    if constraints.has_bounded_width() {
                        constraints.max_width
                    } else {
                        constraints.min_width
                    },
                    if constraints.has_bounded_height() {
                        constraints.max_height
                    } else {
                        constraints.min_height
                    },
                )
            };
            let on_measured = on_measured.clone();
            scope.layout_with(width, height, move |placement| {
                let loose = Constraints::new(
                    constraints.min_width,
                    constraints.max_width,
                    0,
                    Constraints::INFINITY,
                );
                let placeable = placement.measure(child, loose);
                on_measured(placeable.height);
                placement.place(&placeable, 0, 0);
            })
        }),
        callback(content),
    );
}

/// Measures its first child in its placement block and never places it, its own height the
/// state's, as the test's `NotPlaceChild`.
fn not_place_child(height: MutableState<i32>, content: impl Fn() + 'static) {
    Layout(
        Modifier,
        measure_policy(move |scope, measurables, constraints| {
            let child = measurables[0];
            let width = if constraints.has_bounded_width() {
                constraints.max_width
            } else {
                constraints.min_width
            };
            scope.layout_with(width, height.get(), move |placement| {
                let loose = Constraints::new(
                    constraints.min_width,
                    constraints.max_width,
                    0,
                    Constraints::INFINITY,
                );
                placement.measure(child, loose);
            })
        }),
        callback(content),
    );
}

/// As wide as it may be, the state's height, as the test's `Child`.
fn child(height: MutableState<i32>) {
    Layout(
        Modifier,
        measure_policy(move |scope, _, constraints| {
            let width = if constraints.has_bounded_width() {
                constraints.max_width
            } else {
                constraints.min_width
            };
            scope.layout(width, height.get())
        }),
        callback(|| {}),
    );
}

/// How many times the outer parent saw its child measured, before and after the height goes
/// from 10 to 20; each time the height it saw must be `expected`'s.
fn remeasurements(
    content: impl Fn(MutableState<i32>, OnMeasured) + 'static,
    expected: impl Fn(i32) -> i32 + 'static,
) -> (i32, i32) {
    let height = mutable_state_of(10);
    let count = Rc::new(Cell::new(0));
    let (h, c) = (height.handle(), count.clone());
    let expected = Rc::new(expected);
    let mut host = host(move || {
        let (c, expected) = (c.clone(), expected.clone());
        let on_measured: OnMeasured = Rc::new(move |actual| {
            assert_eq!(actual, expected(h.get()));
            c.set(c.get() + 1);
        });
        content(h, on_measured);
    });
    let before = count.get();
    height.set(20);
    host.frame();
    (before, count.get())
}

fn same(height: i32) -> i32 {
    height
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/NodesRemeasuredOnceTest.kt, remeasuringDirectChild
#[test]
fn remeasuring_direct_child() {
    let counts = remeasurements(
        |height, on_measured| wrap_child(Some(on_measured), move || child(height)),
        same,
    );
    assert_eq!(counts, (1, 2));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/NodesRemeasuredOnceTest.kt, remeasuringChildWithExtraLayer
#[test]
fn remeasuring_child_with_extra_layer() {
    let counts = remeasurements(
        |height, on_measured| {
            wrap_child(Some(on_measured), move || wrap_child(None, move || child(height)))
        },
        same,
    );
    assert_eq!(counts, (1, 2));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/NodesRemeasuredOnceTest.kt, remeasuringChildWith3ExtraLayers
#[test]
fn remeasuring_child_with3_extra_layers() {
    let counts = remeasurements(
        |height, on_measured| {
            wrap_child(Some(on_measured), move || {
                wrap_child(None, move || {
                    wrap_child(None, move || wrap_child(None, move || child(height)))
                })
            })
        },
        same,
    );
    assert_eq!(counts, (1, 2));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/NodesRemeasuredOnceTest.kt, remeasuringChildWithExtraLayer_notPlacedChild
#[test]
fn remeasuring_child_with_extra_layer_not_placed_child() {
    let counts = remeasurements(
        |height, on_measured| {
            wrap_child(Some(on_measured), move || {
                not_place_child(height, move || wrap_child(None, move || child(height)))
            })
        },
        same,
    );
    assert_eq!(counts, (1, 2));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/NodesRemeasuredOnceTest.kt, remeasuringChildDuringLayoutWithExtraLayer
#[test]
fn remeasuring_child_during_layout_with_extra_layer() {
    let counts = remeasurements(
        |height, on_measured| {
            measure_during_layout(false, on_measured, move || {
                wrap_child(None, move || child(height))
            })
        },
        same,
    );
    assert_eq!(counts, (1, 2));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/NodesRemeasuredOnceTest.kt, remeasuringChildDuringLayout
#[test]
fn remeasuring_child_during_layout() {
    let counts = remeasurements(
        |height, on_measured| measure_during_layout(false, on_measured, move || child(height)),
        same,
    );
    assert_eq!(counts, (1, 2));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/NodesRemeasuredOnceTest.kt, remeasuringChildDuringLayoutWithExtraLayerUsingIntrinsics
#[test]
fn remeasuring_child_during_layout_with_extra_layer_using_intrinsics() {
    let counts = remeasurements(
        |height, on_measured| {
            measure_during_layout(true, on_measured, move || {
                wrap_child(None, move || child(height))
            })
        },
        same,
    );
    assert_eq!(counts, (1, 2));
}
