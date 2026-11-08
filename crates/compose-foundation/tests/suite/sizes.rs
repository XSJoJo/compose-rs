//! Compose's `SizeTest`: `size`, `width`, `height`, the `*_in` and `required*` modifiers,
//! `default_min_size`, `fill_max_*`, `wrap_content*`, through the `LayoutTest` helpers of
//! [`layout_test`](crate::layout_test). Its intrinsics tests are in `intrinsics.rs`.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_ui::{
    Alignment, Constraints, Dp, IntSize, Layout, LayoutCoordinates, SizeSpec, measure_policy,
};

use crate::layout_test::*;

const NONE: Dp = Dp::UNSPECIFIED;

/// `count` containers in a column (or a row) in a box, each with its modifier: their sizes and
/// positions.
fn stacked(vertical: bool, modifiers: Vec<Modifier>) -> Probe {
    let probe = Probe::new(modifiers.len());
    let p = probe.clone();
    let modifiers = Rc::new(modifiers);
    let _ui = show(move || {
        let (p, modifiers) = (p.clone(), modifiers.clone());
        Box!(|_| {
            let (p, modifiers) = (p.clone(), modifiers.clone());
            let children = move || {
                for (index, modifier) in modifiers.iter().enumerate() {
                    empty(modifier.clone().then(p.at(index)), Spec::default());
                }
            };
            if vertical {
                Column!(|_| children());
            } else {
                Row!(|_| children());
            }
        });
    });
    probe
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testPreferredSize_withWidthSizeModifiers
#[test]
fn preferred_size_with_width_size_modifiers() {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let height = Modifier.height(size_dp);
    let probe = stacked(
        true,
        vec![
            Modifier.width_in(size_dp, size_dp * 2.0).then(height.clone()),
            Modifier.width_in(NONE, size_dp * 2.0).then(height.clone()),
            Modifier.width_in(size_dp, NONE).then(height.clone()),
            Modifier.width_in(NONE, size_dp).width_in(size_dp * 2.0, NONE).then(height.clone()),
            Modifier.width_in(size_dp * 2.0, NONE).width_in(NONE, size_dp).then(height),
            Modifier.size(size_dp),
        ],
    );
    let widths = [size, 0, size, size, round(size_dp * 2.0), size];
    for (index, width) in widths.into_iter().enumerate() {
        assert_eq!(probe.size(index), self::size(width, size), "child {index}");
        assert_eq!(probe.position(index), offset(0, size * index as i32), "child {index}");
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testPreferredSize_withHeightSizeModifiers
#[test]
fn preferred_size_with_height_size_modifiers() {
    let size_dp = px(10.0);
    let size = round(size_dp);
    let width = Modifier.width(size_dp);
    let probe = stacked(
        false,
        vec![
            Modifier.height_in(size_dp, size_dp * 2.0).then(width.clone()),
            Modifier.height_in(NONE, size_dp * 2.0).then(width.clone()),
            Modifier.height_in(size_dp, NONE).then(width.clone()),
            Modifier.height_in(NONE, size_dp).height_in(size_dp * 2.0, NONE).then(width.clone()),
            Modifier.height_in(size_dp * 2.0, NONE).height_in(NONE, size_dp).then(width.clone()),
            Modifier.height(size_dp).then(width),
        ],
    );
    let heights = [size, 0, size, size, round(size_dp * 2.0), size];
    for (index, height) in heights.into_iter().enumerate() {
        assert_eq!(probe.size(index), self::size(size, height), "child {index}");
        assert_eq!(probe.position(index), offset(size * index as i32, 0), "child {index}");
    }
}

fn size_in(min_width: Dp, min_height: Dp, max_width: Dp, max_height: Dp) -> SizeSpec {
    SizeSpec::bounds(min_width, min_height, max_width, max_height)
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testPreferredSize_withSizeModifiers
#[test]
fn preferred_size_with_size_modifiers() {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let double = size_dp * 2.0;
    let probe = stacked(
        false,
        vec![
            Modifier
                .size_in(size_in(NONE, NONE, double, double))
                .size_in(size_in(size_dp, size_dp, NONE, NONE)),
            Modifier
                .size_in(size_in(NONE, NONE, size_dp, size_dp))
                .size_in(size_in(double, size_dp, NONE, NONE)),
            Modifier
                .size_in(size_in(size_dp, size_dp, NONE, NONE))
                .size_in(size_in(NONE, NONE, double, double)),
            Modifier
                .size_in(size_in(double, double, NONE, NONE))
                .size_in(size_in(NONE, NONE, size_dp, size_dp)),
            Modifier.size(size_dp),
        ],
    );
    let expected = [
        (size, size, 0),
        (size, size, size),
        (size, size, size * 2),
        (size * 2, size * 2, size * 3),
        (size, size, size * 5),
    ];
    for (index, (width, height, x)) in expected.into_iter().enumerate() {
        assert_eq!(probe.size(index), self::size(width, height), "child {index}");
        assert_eq!(probe.position(index), offset(x, 0), "child {index}");
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testPreferredSizeModifiers_respectMaxConstraint
#[test]
fn preferred_size_modifiers_respect_max_constraint() {
    let size_dp = px(100.0);
    let size = round(size_dp);
    let (probe, constrained) = (Probe::new(1), Rc::new(Cell::new(IntSize::new(-1, -1))));
    let (p, c) = (probe.clone(), constrained.clone());
    let _ui = show(move || {
        let (p, c) = (p.clone(), c.clone());
        Box!(|_| {
            let (p, c) = (p.clone(), c.clone());
            container(Modifier, Spec::sized(size_dp, size_dp), move || {
                let (p, c) = (p.clone(), c.clone());
                let seen =
                    Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
                        c.set(coordinates.size())
                    });
                let modifier = Modifier.width(size_dp * 2.0).height(size_dp * 3.0).then(seen);
                container(modifier, Spec::default(), move || {
                    empty(p.at(0), Spec { expanded: true, ..Spec::default() });
                });
            });
        });
    });
    assert_eq!(constrained.get(), self::size(size, size));
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(0, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testMaxModifiers_withInfiniteValue
#[test]
fn max_modifiers_with_infinite_value() {
    let size_dp = px(20.0);
    let size = round(size_dp);
    let probe = Probe::new(4);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        Box!(|_| {
            let p = p.clone();
            Row!(|_| {
                let inner = |index: usize| {
                    let p = p.clone();
                    move || empty(p.at(index), Spec::sized(size_dp, size_dp))
                };
                container(Modifier.width_in(NONE, Dp::INFINITY), Spec::default(), inner(0));
                container(Modifier.height_in(NONE, Dp::INFINITY), Spec::default(), inner(1));
                empty(
                    Modifier
                        .width(size_dp)
                        .height(size_dp)
                        .width_in(NONE, Dp::INFINITY)
                        .height_in(NONE, Dp::INFINITY)
                        .then(p.at(2)),
                    Spec::default(),
                );
                container(
                    Modifier.size_in(size_in(NONE, NONE, Dp::INFINITY, Dp::INFINITY)),
                    Spec::default(),
                    inner(3),
                );
            });
        });
    });
    for index in 0..4 {
        assert_eq!(probe.size(index), self::size(size, size), "child {index}");
    }
}

/// A box wrapping its content at the root's top-start, of `modifier`: its size and position.
fn wrapped_box(modifier: Modifier) -> (IntSize, compose_ui::Offset) {
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let modifier = Modifier
            .wrap_content_size_with(Alignment::TOP_START, false)
            .then(modifier.clone())
            .then(p.at(0));
        Box!(modifier = modifier);
    });
    (probe.size(0), probe.position(0))
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testSize_smallerInLarger
#[test]
fn size_smaller_in_larger() {
    let size = 64;
    let size_dp = px(size as f32);
    let (seen, at) = wrapped_box(Modifier.required_size(size_dp * 2.0).required_size(size_dp));
    assert_eq!(seen, self::size(size, size));
    assert_eq!(at, offset(size / 2, size / 2));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testSize_smallerBoxInLargerBox
#[test]
fn size_smaller_box_in_larger_box() {
    let size = 64;
    let size_dp = px(size as f32);
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        Box!(
            modifier = Modifier
                .wrap_content_size_with(Alignment::TOP_START, false)
                .required_size(size_dp * 2.0),
            propagate_min_constraints = true,
            |_| Box!(modifier = Modifier.required_size(size_dp).then(p.at(0)))
        );
    });
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(size / 2, size / 2));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testSize_largerInSmaller
#[test]
fn size_larger_in_smaller() {
    let size = 64;
    let size_dp = px(size as f32);
    let (seen, at) = wrapped_box(Modifier.required_size(size_dp).required_size(size_dp * 2.0));
    assert_eq!(seen, self::size(size * 2, size * 2));
    assert_eq!(at, offset(-size / 2, -size / 2));
}

/// The constraints a `BoxWithConstraints` of `modifier` gets, measured with `incoming`, as
/// `SizeTest.assertConstraints`.
fn assert_constraints(incoming: Constraints, modifier: Modifier, expected: Constraints) {
    let seen = Rc::new(Cell::new(None));
    let s = seen.clone();
    let _ui = show(move || {
        let (s, modifier) = (s.clone(), modifier.clone());
        let policy = measure_policy(move |scope, measurables, _| {
            scope.measure(measurables[0], incoming);
            scope.layout(0, 0)
        });
        Layout(
            Modifier,
            policy,
            callback(move || {
                let s = s.clone();
                BoxWithConstraints!(
                    modifier = modifier.clone(),
                    |scope: BoxWithConstraintsScope| { s.set(Some(scope.constraints)) }
                );
            }),
        );
    });
    assert_eq!(seen.get(), Some(expected), "incoming {incoming:?}");
}

fn c(min_width: i32, max_width: i32, min_height: i32, max_height: i32) -> Constraints {
    Constraints::new(min_width, max_width, min_height, max_height)
}

fn dp(pixels: f32) -> Dp {
    px(pixels)
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testMeasurementConstraints_preferredSatisfiable
#[test]
fn measurement_constraints_preferred_satisfiable() {
    let incoming = c(10, 30, 15, 35);
    assert_constraints(incoming, Modifier.width(dp(20.0)), c(20, 20, 15, 35));
    assert_constraints(incoming, Modifier.height(dp(20.0)), c(10, 30, 20, 20));
    assert_constraints(incoming, Modifier.size(dp(20.0)), c(20, 20, 20, 20));
    assert_constraints(incoming, Modifier.width_in(dp(20.0), dp(25.0)), c(20, 25, 15, 35));
    assert_constraints(incoming, Modifier.height_in(dp(20.0), dp(25.0)), c(10, 30, 20, 25));
    let spec = size_in(dp(20.0), dp(20.0), dp(25.0), dp(25.0));
    assert_constraints(incoming, Modifier.size_in(spec), c(20, 25, 20, 25));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testMeasurementConstraints_preferredUnsatisfiable
#[test]
fn measurement_constraints_preferred_unsatisfiable() {
    assert_constraints(c(20, 40, 15, 35), Modifier.width(dp(15.0)), c(20, 20, 15, 35));
    let incoming = c(10, 30, 15, 35);
    assert_constraints(incoming, Modifier.height(dp(10.0)), c(10, 30, 15, 15));
    assert_constraints(incoming, Modifier.size(dp(40.0)), c(30, 30, 35, 35));
    assert_constraints(c(20, 30, 15, 35), Modifier.width_in(dp(10.0), dp(15.0)), c(20, 20, 15, 35));
    assert_constraints(incoming, Modifier.height_in(dp(5.0), dp(10.0)), c(10, 30, 15, 15));
    let spec = size_in(dp(40.0), dp(50.0), dp(45.0), dp(55.0));
    assert_constraints(incoming, Modifier.size_in(spec), c(30, 30, 35, 35));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testMeasurementConstraints_compulsorySatisfiable
#[test]
fn measurement_constraints_compulsory_satisfiable() {
    let incoming = c(10, 30, 15, 35);
    assert_constraints(incoming, Modifier.required_width(dp(20.0)), c(20, 20, 15, 35));
    assert_constraints(incoming, Modifier.required_height(dp(20.0)), c(10, 30, 20, 20));
    assert_constraints(incoming, Modifier.required_size(dp(20.0)), c(20, 20, 20, 20));
    assert_constraints(incoming, Modifier.required_width_in(dp(20.0), dp(25.0)), c(20, 25, 15, 35));
    assert_constraints(
        incoming,
        Modifier.required_height_in(dp(20.0), dp(25.0)),
        c(10, 30, 20, 25),
    );
    let spec = size_in(dp(20.0), dp(20.0), dp(25.0), dp(25.0));
    assert_constraints(incoming, Modifier.required_size_in(spec), c(20, 25, 20, 25));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testMeasurementConstraints_compulsoryUnsatisfiable
#[test]
fn measurement_constraints_compulsory_unsatisfiable() {
    assert_constraints(c(20, 40, 15, 35), Modifier.required_width(dp(15.0)), c(15, 15, 15, 35));
    let incoming = c(10, 30, 15, 35);
    assert_constraints(incoming, Modifier.required_height(dp(10.0)), c(10, 30, 10, 10));
    assert_constraints(incoming, Modifier.required_size(dp(40.0)), c(40, 40, 40, 40));
    assert_constraints(
        c(20, 30, 15, 35),
        Modifier.required_width_in(dp(10.0), dp(15.0)),
        c(10, 15, 15, 35),
    );
    assert_constraints(incoming, Modifier.required_height_in(dp(5.0), dp(10.0)), c(10, 30, 5, 10));
    let spec = size_in(dp(40.0), dp(50.0), dp(45.0), dp(55.0));
    assert_constraints(incoming, Modifier.required_size_in(spec), c(40, 45, 50, 55));
    // One dimension unspecified, the other contradicting the incoming constraints.
    let spec = size_in(dp(20.0), dp(30.0), NONE, NONE);
    assert_constraints(c(10, 10, 10, 10), Modifier.required_size_in(spec), c(20, 20, 30, 30));
    let spec = size_in(NONE, NONE, dp(20.0), dp(30.0));
    assert_constraints(c(40, 40, 40, 40), Modifier.required_size_in(spec), c(20, 20, 30, 30));
}

/// The constraints a leaf of `modifier` is measured with.
fn leaf_constraints(modifier: Modifier) -> Constraints {
    let seen = Rc::new(Cell::new(None));
    let s = seen.clone();
    let _ui = show(move || {
        let s = s.clone();
        let policy = measure_policy(move |scope, _, constraints| {
            s.set(Some(constraints));
            scope.layout(0, 0)
        });
        Layout(modifier.clone(), policy, callback(|| {}));
    });
    seen.get().expect("measured")
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testDefaultMinSize
#[test]
fn default_min_size() {
    // Constraints are applied.
    let applied = leaf_constraints(
        Modifier
            .wrap_content_size()
            .required_size_in(size_in(NONE, NONE, dp(30.0), dp(40.0)))
            .default_min_size(dp(10.0), dp(20.0)),
    );
    assert_eq!(applied, c(10, 30, 20, 40));
    // Constraints are not applied.
    let bounds = size_in(dp(10.0), dp(20.0), dp(100.0), dp(110.0));
    let not_applied =
        leaf_constraints(Modifier.required_size_in(bounds).default_min_size(dp(50.0), dp(50.0)));
    assert_eq!(not_applied, c(10, 100, 20, 110));
    // The defaults change nothing.
    let defaults = leaf_constraints(Modifier.required_size_in(bounds).default_min_size(NONE, NONE));
    assert_eq!(defaults, c(10, 100, 20, 110));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testDefaultMinSize_withCoercingMaxConstraints
#[test]
fn default_min_size_with_coercing_max_constraints() {
    let constraints = leaf_constraints(
        Modifier
            .wrap_content_size()
            .required_size_in(size_in(NONE, NONE, dp(30.0), dp(40.0)))
            .default_min_size(dp(70.0), dp(80.0)),
    );
    assert_eq!(constraints, c(30, 30, 40, 40));
}
