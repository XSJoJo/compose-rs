//! Compose's `PaddingTest`: `padding`'s overloads applied to a child, with too little room and
//! right to left, through the `LayoutTest` helpers of [`layout_test`](crate::layout_test). Its
//! intrinsics test is in `intrinsics.rs`. Not ported: `absolutePadding` and
//! `PaddingValues.Absolute`, and the negative padding checks (the layout-modifiers feature); its
//! inspector tests are n/a.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_ui::{Alignment, Dp, LOCAL_LAYOUT_DIRECTION, Layout, LayoutDirection, measure_policy};

use crate::layout_test::*;

/// As `PaddingTest.TestBox`: its one child measured with its own constraints.
fn test_box(modifier: Modifier, content: impl Fn() + 'static) {
    let policy = measure_policy(|scope, measurables, constraints| {
        assert_eq!(measurables.len(), 1, "TestBox must have exactly one child");
        let placeable = scope.measure(measurables[0], constraints);
        scope.place_relative(&placeable, 0, 0);
        scope.layout(
            placeable.width.min(constraints.max_width),
            placeable.height.min(constraints.max_height),
        )
    });
    Layout(modifier, policy, callback(content));
}

/// A container in a `TestBox` of `padding`, in a 50 dp `ConstrainedBox` centered in the root:
/// the container's size and position.
fn padded(padding: Modifier) -> Probe {
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let (p, padding) = (p.clone(), padding.clone());
        Box!(modifier = Modifier.fill_max_size(), |scope| {
            let (p, padding) = (p.clone(), padding.clone());
            let fixed = DpConstraints::fixed(Dp(50.0), Dp(50.0));
            constrained_box(fixed, Modifier.align(scope, Alignment::CENTER), move || {
                let p = p.clone();
                test_box(padding.clone(), move || empty(p.at(0), Spec::default()));
            });
        });
    });
    probe
}

/// As `testPaddingWithDifferentInsetsImplementation`: the child of `padding` inset by `left`,
/// `top`, `right` and `bottom`.
fn assert_insets(padding: Modifier, left: Dp, top: Dp, right: Dp, bottom: Dp) {
    let probe = padded(padding);
    let size = round(Dp(50.0));
    let (left, top, right, bottom) = (round(left), round(top), round(right), round(bottom));
    assert_eq!(probe.size(0), self::size(size - left - right, size - top - bottom));
    let x = ((ROOT.width - size) as f32 / 2.0).round() as i32 + left;
    let y = ((ROOT.height - size) as f32 / 2.0).round() as i32 + top;
    assert_eq!(probe.position(0), offset(x, y));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/PaddingTest.kt, allEqualToAbsoluteWithExplicitSides
#[test]
fn all_equal_to_absolute_with_explicit_sides() {
    let ten = Dp(10.0);
    assert!(Modifier.padding((ten, ten, ten, ten)).same(&Modifier.padding(ten)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/PaddingTest.kt, symmetricEqualToAbsoluteWithExplicitSides
#[test]
fn symmetric_equal_to_absolute_with_explicit_sides() {
    let (ten, twenty) = (Dp(10.0), Dp(20.0));
    assert!(Modifier.padding((ten, twenty, ten, twenty)).same(&Modifier.padding((ten, twenty))));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/PaddingTest.kt, paddingAllAppliedToChild
#[test]
fn padding_all_applied_to_child() {
    let padding = Dp(10.0);
    assert_insets(Modifier.padding(padding), padding, padding, padding, padding);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/PaddingTest.kt, paddingPaddingValuesAppliedToChild
#[test]
fn padding_padding_values_applied_to_child() {
    let (start, top, end, bottom) = (Dp(1.0), Dp(3.0), Dp(6.0), Dp(10.0));
    let padding = PaddingValues { start, top, end, bottom };
    assert_insets(Modifier.padding(padding), start, top, end, bottom);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/PaddingTest.kt, insufficientSpaceAvailable
#[test]
fn insufficient_space_available() {
    let padding = Dp(30.0);
    let probe = padded(Modifier.padding(padding));
    let (size, padding) = (round(Dp(50.0)), round(padding));
    assert_eq!(probe.size(0), self::size(0, 0));
    let x = ((ROOT.width - size) as f32 / 2.0).round() as i32 + padding;
    let y = ((ROOT.height - size) as f32 / 2.0).round() as i32 + padding;
    assert_eq!(probe.position(0), offset(x, y));
}

fn rtl(content: impl Fn() + 'static) {
    CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(LayoutDirection::Rtl), content);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/PaddingTest.kt, testPadding_rtl
#[test]
fn padding_rtl() {
    let size_dp = px(100.0);
    let size = round(size_dp);
    let (padding1_dp, padding2_dp, padding3_dp) = (Dp(5.0), Dp(10.0), Dp(15.0));
    let (padding1, padding2, padding3) =
        (round(padding1_dp), round(padding2_dp), round(padding3_dp));
    let probe = Probe::new(3);
    let p = probe.clone();
    // Left to right: P1 S P2 | S P3 | P1 S; right to left: S P1 | P3 S | P2 S P1.
    let _ui = show(move || {
        let p = p.clone();
        rtl(move || {
            let p = p.clone();
            Row!(modifier = Modifier.fill_max_size(), |_| {
                let zero = PaddingValues::ZERO;
                let paddings = [
                    PaddingValues { start: padding1_dp, end: padding2_dp, ..zero },
                    PaddingValues { end: padding3_dp, ..zero },
                    PaddingValues { start: padding1_dp, ..zero },
                ];
                for (index, padding) in paddings.into_iter().enumerate() {
                    let modifier = Modifier.padding(padding).size(size_dp).then(p.at(index));
                    Box!(modifier = modifier);
                }
            });
        });
    });
    let width = ROOT.width;
    assert_eq!(probe.position(0), offset(width - padding1 - size, 0));
    assert_eq!(probe.position(1), offset(width - padding1 - padding2 - size * 2, 0));
    let third = width - size * 3 - padding1 * 2 - padding2 - padding3;
    assert_eq!(probe.position(2), offset(third, 0));
    for index in 0..3 {
        assert_eq!(probe.size(index), self::size(size, size));
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/PaddingTest.kt, testPaddingValuesRtl
#[test]
fn padding_values_rtl() {
    let (box_size, start, end) = (10.0, 1.0, 2.0);
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        rtl(move || {
            let padding = PaddingValues { start: px(start), end: px(end), ..PaddingValues::ZERO };
            Box!(modifier = Modifier.size(px(box_size)).padding(padding).then(p.at(0)));
        });
    });
    assert_eq!(probe.size(0).width, (box_size - start - end) as i32);
}
