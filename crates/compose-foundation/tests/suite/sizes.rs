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

// ---- Fill ---------------------------------------------------------------------------------

/// The size of a box of `modifier` in a box of `parent`, as `SizeTest.calculateSizeFor`.
fn size_for(parent: Modifier, modifier: Modifier) -> IntSize {
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let modifier = modifier.clone().then(p.at(0));
        Box!(modifier = parent.clone(), |_| Box!(modifier = modifier.clone()));
    });
    probe.size(0)
}

/// `testFillModifier_correctSize`'s checks, the sizes given as `size`'s two forms.
fn fill_sizes(parent: Modifier, child: Modifier) {
    let (parent_width, parent_height, child_width, child_height) = (100, 80, 40, 30);
    let fill = |fill: Modifier| size_for(parent.clone(), fill.then(child.clone()));
    assert_eq!(fill(Modifier), size(child_width, child_height));
    assert_eq!(fill(Modifier.fill_max_width()), size(parent_width, child_height));
    assert_eq!(fill(Modifier.fill_max_height()), size(child_width, parent_height));
    assert_eq!(fill(Modifier.fill_max_size()), size(parent_width, parent_height));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testFillModifier_correctSize
#[test]
fn fill_modifier_correct_size() {
    fill_sizes(Modifier.required_size((dp(100.0), dp(80.0))), Modifier.size((dp(40.0), dp(30.0))));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testFillModifier_correctDpSize
#[test]
fn fill_modifier_correct_dp_size() {
    fill_sizes(
        Modifier.required_size(compose_ui::DpSize::new(dp(100.0), dp(80.0))),
        Modifier.size(compose_ui::DpSize::new(dp(40.0), dp(30.0))),
    );
}

/// A child of `child_width` by `child_height` filling half of a 100 by 80 px parent.
fn half_fills(child_width: i32, child_height: i32) {
    let parent = Modifier.required_size((dp(100.0), dp(80.0)));
    let child = Modifier.size((dp(child_width as f32), dp(child_height as f32)));
    let fill = |fill: Modifier| size_for(parent.clone(), fill.then(child.clone()));
    assert_eq!(fill(Modifier), size(child_width, child_height));
    assert_eq!(fill(Modifier.fill_max_width_with(0.5)), size(50, child_height));
    assert_eq!(fill(Modifier.fill_max_height_with(0.5)), size(child_width, 40));
    assert_eq!(fill(Modifier.fill_max_size_with(0.5)), size(50, 40));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testFractionalFillModifier_correctSize_whenSmallerChild
#[test]
fn fractional_fill_modifier_correct_size_when_smaller_child() {
    half_fills(40, 30);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testFractionalFillModifier_correctSize_whenLargerChild
#[test]
fn fractional_fill_modifier_correct_size_when_larger_child() {
    half_fills(70, 50);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testFractionalFillModifier_coerced
#[test]
fn fractional_fill_modifier_coerced() {
    let child = Modifier.required_size_in(size_in(dp(40.0), dp(30.0), dp(60.0), dp(50.0)));
    let fill = |fraction: f32| size_for(Modifier, child.clone().fill_max_size_with(fraction));
    assert_eq!(fill(-0.1), size(40, 30));
    assert_eq!(fill(0.1), size(40, 30));
    assert_eq!(fill(1.2), size(60, 50));
}

// ---- Wrapping the content -----------------------------------------------------------------

/// In a container, a container of `modifier`: the outer one's size and position, the inner's.
fn wrapped(modifier: Modifier) -> Probe {
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let (p, modifier) = (p.clone(), modifier.clone());
        container(p.at(0), Spec::default(), move || {
            empty(modifier.clone().then(p.at(1)), Spec::default());
        });
    });
    probe
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, test2DWrapContentSize
#[test]
fn wrap_content_size_2d() {
    let size_dp = Dp(50.0);
    let size = round(size_dp);
    let probe = wrapped(
        Modifier.fill_max_size().wrap_content_size_with(Alignment::BOTTOM_END, false).size(size_dp),
    );
    assert_eq!(probe.size(0), ROOT);
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.size(1), self::size(size, size));
    assert_eq!(probe.position(1), offset(ROOT.width - size, ROOT.height - size));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, test1DWrapContentSize
#[test]
fn wrap_content_size_1d() {
    let size_dp = Dp(50.0);
    let size = round(size_dp);
    let probe = wrapped(
        Modifier.fill_max_size().wrap_content_width_with(Alignment::END, false).width(size_dp),
    );
    assert_eq!(probe.size(0), ROOT);
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.size(1), self::size(size, ROOT.height));
    assert_eq!(probe.position(1), offset(ROOT.width - size, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testWrapContentSize_rtl
#[test]
fn wrap_content_size_rtl() {
    let size_dp = px(200.0);
    let size = round(size_dp);
    let probe = Probe::new(3);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        let rtl = compose_ui::LOCAL_LAYOUT_DIRECTION.provides(compose_ui::LayoutDirection::Rtl);
        CompositionLocalProvider(rtl, move || {
            let p = p.clone();
            Box!(modifier = Modifier.fill_max_size(), |_| {
                let wraps = [
                    Modifier.wrap_content_size_with(Alignment::TOP_START, false),
                    Modifier.wrap_content_height_with(Alignment::CENTER_VERTICALLY, false),
                    Modifier.wrap_content_size_with(Alignment::BOTTOM_END, false),
                ];
                for (index, wrap) in wraps.into_iter().enumerate() {
                    let p = p.clone();
                    Box!(modifier = Modifier.fill_max_size().then(wrap), |_| {
                        Box!(modifier = Modifier.size(size_dp).then(p.at(index)));
                    });
                }
            });
        });
    });
    let (width, height) = (ROOT.width, ROOT.height);
    assert_eq!(probe.position(0), offset(width - size, 0));
    assert_eq!(probe.position(1), offset(width - size, (height - size) / 2));
    assert_eq!(probe.position(2), offset(0, height - size));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testModifier_wrapsContent
#[test]
fn modifier_wraps_content() {
    let content = Dp(50.0);
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        container(Modifier, Spec::default(), move || {
            container(p.at(0), Spec::default(), move || {
                empty(
                    Modifier.wrap_content_size_with(Alignment::TOP_START, false).size(content),
                    Spec::default(),
                );
            });
        });
    });
    assert_eq!(probe.size(0), size(round(content), round(content)));
}

/// `content` measured with no constraints, in a layout as large as its own maximums.
fn unconstrained(content: impl Fn() + 'static) {
    let policy = measure_policy(|scope, measurables, constraints| {
        let placeable = scope.measure(measurables[0], Constraints::unbounded());
        scope.place_relative(&placeable, 0, 0);
        scope.layout(constraints.max_width, constraints.max_height)
    });
    Layout(Modifier, policy, callback(content));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testWrapContentSize_wrapsContent_whenMeasuredWithInfiniteConstraints
#[test]
fn wrap_content_size_wraps_content_when_measured_with_infinite_constraints() {
    let size_dp = Dp(50.0);
    let size = round(size_dp);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        unconstrained(move || {
            let p = p.clone();
            container(p.at(0), Spec::default(), move || {
                empty(
                    Modifier
                        .wrap_content_size_with(Alignment::BOTTOM_END, false)
                        .size(size_dp)
                        .then(p.at(1)),
                    Spec::default(),
                );
            });
        });
    });
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.size(1), self::size(size, size));
    assert_eq!(probe.position(1), offset(0, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testWrapContentSize_respectsMinConstraints
#[test]
fn wrap_content_size_respects_min_constraints() {
    let size_dp = Dp(50.0);
    let (size, double) = (round(size_dp), round(size_dp * 2.0));
    let (probe, wrap) = (Probe::new(1), Rc::new(Cell::new(IntSize::new(-1, -1))));
    let (p, w) = (probe.clone(), wrap.clone());
    let _ui = show(move || {
        let (p, w) = (p.clone(), w.clone());
        let wrapper = Modifier.wrap_content_size_with(Alignment::TOP_START, false);
        container(wrapper, Spec::default(), move || {
            let (p, w) = (p.clone(), w.clone());
            let policy = measure_policy(move |scope, measurables, incoming| {
                let constraints = incoming.constrain_by(Constraints::new(double, INF, double, INF));
                let placeable = scope.measure(measurables[0], constraints);
                scope.place_relative(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            });
            let seen = Modifier.on_globally_positioned(move |c: LayoutCoordinates| w.set(c.size()));
            Layout(
                seen,
                policy,
                callback(move || {
                    empty(
                        Modifier
                            .wrap_content_size_with(Alignment::CENTER, false)
                            .size(size_dp)
                            .then(p.at(0)),
                        Spec::default(),
                    );
                }),
            );
        });
    });
    let half = ((double - size) as f32 / 2.0).round() as i32;
    assert_eq!(wrap.get(), self::size(double, double));
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(half, half));
}

const INF: i32 = Constraints::INFINITY;

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testWrapContentSize_unbounded
#[test]
fn wrap_content_size_unbounded() {
    let (outer, inner) = (10.0, 20.0);
    let probe = Probe::new(4);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        Box!(modifier = Modifier.required_size(px(outer)).then(p.at(3)), |_| {
            let wraps = [
                Modifier.wrap_content_size_with(Alignment::BOTTOM_END, true),
                Modifier.wrap_content_width_with(Alignment::END, true),
                Modifier.wrap_content_height_with(Alignment::BOTTOM, true),
            ];
            for (index, wrap) in wraps.into_iter().enumerate() {
                Box!(modifier = wrap.required_size(px(inner)).then(p.at(index)));
            }
        });
    });
    assert_eq!(probe.size(3).width, outer as i32);
    let parent = probe.position(3);
    let at = |index: usize| probe.position(index) - parent;
    let end = outer - inner;
    assert_eq!(at(0), compose_ui::Offset::new(end, end));
    assert_eq!(at(1).x, end);
    assert_eq!(at(2).y, end);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testAlignedModifier_alignsCorrectly_whenOddDimensions_endAligned
#[test]
fn aligned_modifier_aligns_correctly_when_odd_dimensions_end_aligned() {
    let (parent, child) = (px(100.0), px(1.0));
    let child_size = round(child);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        unconstrained(move || {
            let p = p.clone();
            container(Modifier.size(parent).then(p.at(0)), Spec::default(), move || {
                empty(
                    Modifier
                        .fill_max_size()
                        .wrap_content_size_with(Alignment::BOTTOM_END, false)
                        .size(child)
                        .then(p.at(1)),
                    Spec::default(),
                );
            });
        });
    });
    let outer = probe.size(0);
    assert_eq!(probe.size(1), size(child_size, child_size));
    assert_eq!(probe.position(1), offset(outer.width - child_size, outer.height - child_size));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, testModifiers_equals
#[test]
fn modifiers_equals() {
    let same = |a: Modifier, b: Modifier| a.same(&b);
    let (ten, twenty) = (Dp(10.0), Dp(20.0));
    assert!(same(Modifier.size((ten, twenty)), Modifier.size((ten, twenty))));
    assert!(same(Modifier.required_size((ten, twenty)), Modifier.required_size((ten, twenty))));
    let bottom_end = || Modifier.wrap_content_size_with(Alignment::BOTTOM_END, false);
    assert!(same(bottom_end(), bottom_end()));
    assert!(same(Modifier.fill_max_size_with(0.8), Modifier.fill_max_size_with(0.8)));
    assert!(same(Modifier.default_min_size(ten, twenty), Modifier.default_min_size(ten, twenty)));
    assert!(!same(Modifier.size((ten, twenty)), Modifier.size((twenty, ten))));
    assert!(!same(Modifier.required_size((ten, twenty)), Modifier.required_size((twenty, ten))));
    assert!(!same(bottom_end(), Modifier.wrap_content_size_with(Alignment::BOTTOM_CENTER, false)));
    assert!(!same(Modifier.fill_max_size_with(0.8), Modifier.fill_max_size()));
    assert!(!same(Modifier.default_min_size(ten, twenty), Modifier.default_min_size(twenty, ten)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/SizeTest.kt, sizeModifiers_doNotCauseCrashesWhenCreatingConstraints
#[test]
fn size_modifiers_do_not_cause_crashes_when_creating_constraints() {
    let mut ui = show(|| {
        Box!(modifier = Modifier.size_in(size_in(Dp(-1.0), NONE, NONE, NONE)));
        Box!(modifier = Modifier.size_in(size_in(Dp(10.0), NONE, Dp(5.0), NONE)));
        Box!(modifier = Modifier.size_in(size_in(NONE, Dp(-1.0), NONE, NONE)));
        Box!(modifier = Modifier.size_in(size_in(NONE, Dp(10.0), NONE, Dp(5.0))));
        let infinite = Dp::INFINITY;
        Box!(modifier = Modifier.size_in(size_in(infinite, infinite, infinite, infinite)));
        Box!(modifier = Modifier.default_min_size(Dp(-1.0), Dp(-1.0)));
    });
    ui.wait_for_idle();
}
