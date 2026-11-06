//! Compose's `DpTest.kt` (ui-unit): `Dp`'s construction, arithmetic, constants, comparisons,
//! clamping, `DpRect`, unspecified value, hashing and negative zero. Compose asserts
//! `assertFalse(a > b)`, which with a NaN isn't `a <= b`, so the tests keep its negations.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::hash::{Hash, Hasher};

use compose_ui::{Dp, DpOffset, DpRect, DpSize, UnitExt, lerp};

fn hash_of(dp: Dp) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    dp.hash(&mut hasher);
    hasher.finish()
}

fn panics(block: impl FnOnce()) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(block)).is_err()
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, constructor
#[test]
fn a_dp_keeps_its_value() {
    assert_eq!(Dp(5.0).value(), 5.0);
    assert_eq!(Dp(f32::INFINITY).value(), f32::INFINITY);
    assert!(Dp(f32::NAN).value().is_nan());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, dpIntegerConstruction
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, dpFloatConstruction
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, dpDoubleConstruction
#[test]
fn integers_floats_and_doubles_make_dps() {
    assert_eq!(10.dp().value(), 10.0);
    assert_eq!(10.0_f32.dp().value(), 10.0);
    assert_eq!(10.0_f64.dp().value(), 10.0);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, subtractOperator
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, addOperator
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, multiplyOperator
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, multiplyOperatorScalar
#[test]
fn dps_add_subtract_and_multiply() {
    assert_eq!((3.dp() - 4.dp()).value(), -1.0);
    assert_eq!((10.dp() - 9.dp()).value(), 1.0);
    assert_eq!((1.dp() + 1.dp()).value(), 2.0);
    assert_eq!((6.dp() + 4.dp()).value(), 10.0);
    assert_eq!((1.dp() * 0.0).value(), 0.0);
    assert_eq!((1.dp() * 10.0).value(), 10.0);
    assert_eq!(10.0 * 1.dp().value(), 10.0);
    assert_eq!((10 * 1.dp()).value(), 10.0);
    assert_eq!((10.0_f64 * 1.dp()).value(), 10.0);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, divideOperator
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, divideToScalar
#[test]
fn a_dp_over_a_dp_is_a_ratio() {
    assert_eq!(100.dp() / 10.0_f32.dp(), 10.0);
    assert_eq!(0.dp() / 10.0_f32.dp(), 0.0);
    assert_eq!(1.dp() / 1.dp(), 1.0);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, hairline
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, infinite
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, isFinite
#[test]
fn hairline_infinity_and_finiteness() {
    assert_eq!(Dp::HAIRLINE.value(), 0.0);
    assert_eq!(Dp::INFINITY.value(), f32::INFINITY);
    assert!(1.dp().is_finite());
    assert!(!Dp::UNSPECIFIED.is_finite());
    assert!(!Dp::INFINITY.is_finite());
    assert!(!f32::NEG_INFINITY.dp().is_finite());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, compare
#[test]
fn dps_compare_by_value_and_nans_compare_equal() {
    use std::cmp::Ordering;
    assert!(0.dp() < f32::MIN_POSITIVE.dp());
    assert!(0.dp() < f32::from_bits(1).dp());
    assert!(1.dp() < 3.dp());
    assert_eq!(1.dp().partial_cmp(&1.dp()), Some(Ordering::Equal));
    assert!(1.dp() > 0.dp());
    assert!(f32::NEG_INFINITY.dp() < Dp::INFINITY);
    assert!(f32::NEG_INFINITY.dp() < 0.dp());
    assert!(Dp::INFINITY > f32::MAX.dp());

    // Two NaNs made two ways, as Compose's test makes them.
    let zero = 0.0_f32;
    let zero_nan = zero / zero;
    let infinity_nan = f32::INFINITY / f32::NEG_INFINITY;
    assert_eq!(zero_nan.dp().partial_cmp(&zero_nan.dp()), Some(Ordering::Equal));
    assert_eq!(infinity_nan.dp().partial_cmp(&infinity_nan.dp()), Some(Ordering::Equal));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, minTest
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, maxTest
#[test]
fn min_and_max_pick_a_side() {
    assert_eq!(10.dp().min(20.dp()).value(), 10.0);
    assert_eq!(20.dp().min(10.dp()).value(), 10.0);
    assert_eq!(10.dp().min(10.dp()).value(), 10.0);
    assert_eq!(10.dp().max(20.dp()).value(), 20.0);
    assert_eq!(20.dp().max(10.dp()).value(), 20.0);
    assert_eq!(20.dp().max(20.dp()).value(), 20.0);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, coerceIn
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, coerceAtLeast
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, coerceAtMost
#[test]
fn coercion_clamps_and_rejects_a_reversed_range() {
    assert_eq!(10.dp().coerce_in(0.dp(), 20.dp()).value(), 10.0);
    assert_eq!(20.dp().coerce_in(0.dp(), 10.dp()).value(), 10.0);
    assert_eq!(0.dp().coerce_in(10.dp(), 20.dp()).value(), 10.0);
    assert!(panics(|| {
        10.dp().coerce_in(20.dp(), 10.dp());
    }));

    assert_eq!(0.dp().coerce_at_least(10.dp()).value(), 10.0);
    assert_eq!(10.dp().coerce_at_least(5.dp()).value(), 10.0);
    assert_eq!(10.dp().coerce_at_least(10.dp()).value(), 10.0);

    assert_eq!(100.dp().coerce_at_most(10.dp()).value(), 10.0);
    assert_eq!(10.dp().coerce_at_most(20.dp()).value(), 10.0);
    assert_eq!(10.dp().coerce_at_most(10.dp()).value(), 10.0);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, dpRectConstructor
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, dpRectWidth
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, dpRectHeight
#[test]
fn a_dp_rect_from_an_origin_and_a_size() {
    assert_eq!(
        DpRect::from_origin_size(DpOffset::new(10.dp(), 5.dp()), DpSize::new(15.dp(), 10.dp())),
        DpRect::new(10.dp(), 5.dp(), 25.dp(), 15.dp())
    );
    let rect = DpRect::new(10.dp(), 5.dp(), 25.dp(), 15.dp());
    assert_eq!(rect.width(), 15.dp());
    assert_eq!(rect.height(), 10.dp());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testIsSpecified
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testIsUnspecified
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testTakeOrElseTrue
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testTakeOrElseFalse
#[test]
fn only_the_unspecified_dp_is_unspecified() {
    assert!(!Dp::UNSPECIFIED.is_specified());
    assert!(Dp(1.0).is_specified());
    assert!(Dp::UNSPECIFIED.is_unspecified());
    assert!(!Dp(1.0).is_unspecified());
    assert!(Dp(1.0).take_or_else(|| Dp::UNSPECIFIED).is_specified());
    assert!(Dp::UNSPECIFIED.take_or_else(|| Dp(1.0)).is_specified());
}

// Compose's `<=` and `>=` with one NaN are true (its `compareTo` calls them equal); compose-rs's
// `partial_cmp` keeps `Equal` for a NaN and a NaN only, as `==` does, so with one NaN every
// comparison is false. The comparisons Compose's test makes (`<`, `>`, `==`, `!=`) agree.
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, compareDp
#[test]
fn equality_and_order_with_unspecified_infinity_and_negative_zero() {
    let one = 1.dp();
    assert!(one == 1.dp());
    assert!(one != 2.dp());
    assert!(one < 2.dp());
    assert!(!(one > 2.dp()));
    assert!(!(2.dp() < one));
    assert!(2.dp() > one);

    assert!(Dp::UNSPECIFIED == Dp::UNSPECIFIED);
    assert!(one != Dp::UNSPECIFIED);
    assert!(Dp::UNSPECIFIED != one);
    assert!(!(Dp::UNSPECIFIED < one));
    assert!(!(Dp::UNSPECIFIED > one));
    assert!(!(one < Dp::UNSPECIFIED));
    assert!(!(one > Dp::UNSPECIFIED));

    assert!(one != Dp::INFINITY);
    assert!(one < Dp::INFINITY);
    assert!(!(one > Dp::INFINITY));
    assert!(Dp::INFINITY > one);
    assert!(!(Dp::INFINITY < one));

    assert_eq!((-0.0_f64).dp().partial_cmp(&0.0_f64.dp()), Some(std::cmp::Ordering::Equal));
    assert!(!((-0.0_f64).dp() < 0.0_f64.dp()));
    assert!(!(0.0_f64.dp() < (-0.0_f64).dp()));
    assert!(-(0.0_f32.dp()) >= 0.0_f32.dp());
    assert!(-(0.0_f32.dp()) == 0.0_f32.dp());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testHashCode
#[test]
fn equal_dps_hash_alike() {
    assert_eq!(hash_of(1.dp()), hash_of(1.dp()));
    assert_eq!(hash_of(Dp::UNSPECIFIED), hash_of(Dp::UNSPECIFIED));
    assert_eq!(hash_of((-0.0_f32).dp()), hash_of(0.0_f32.dp()));
    assert_eq!(hash_of((-0.0_f64).dp()), hash_of(0.0_f64.dp()));
    assert_eq!(hash_of(1.5_f64.dp()), hash_of(1.5_f32.dp()));

    assert_ne!(hash_of(1.dp()), hash_of(2.dp()));
    assert_ne!(hash_of(1.dp()), hash_of(Dp::UNSPECIFIED));
    assert_ne!(hash_of(0.dp()), hash_of(Dp::UNSPECIFIED));
    assert_ne!(hash_of(Dp::INFINITY), hash_of(1.dp()));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testNegativeZeroEquality
#[test]
fn negative_zero_equals_and_hashes_as_zero() {
    let assert_is_equal_zero = |dp: Dp| {
        assert_eq!(dp, 0.dp());
        assert_eq!(hash_of(dp), hash_of(0.dp()));
    };
    assert_is_equal_zero((-0.0_f64).dp());
    assert_is_equal_zero((-0.0_f32).dp());
    assert_is_equal_zero(0.dp());
    assert_is_equal_zero(0.0_f64.dp());
    assert_is_equal_zero(-(0.dp()));
    assert_is_equal_zero(0.dp() / -1);
    assert_is_equal_zero(0.dp() / -1.0);
    assert_is_equal_zero(0.dp() * -1);
    assert_is_equal_zero(0.dp() * -1.0);
    assert_is_equal_zero(-1 * 0.dp());
    assert_is_equal_zero(-1.0_f32 * 0.dp());
    assert_is_equal_zero(-1.0_f64 * 0.dp());
    assert_is_equal_zero((-0.0_f64).dp() + (-0.0_f64).dp());
    assert_is_equal_zero((-0.0_f64).dp() - 0.0_f64.dp());
    assert_is_equal_zero(lerp(-(2.dp()), 2.dp(), 0.5));
}

// Compose compares these by their packed bits, which a negative zero would change: the
// interpolation of two values a hair below zero gives a positive zero.
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testDpOffsetLerpNegativeZero
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpTest.kt, testDpSizeLerpNegativeZero
#[test]
fn interpolating_to_a_negative_zero_gives_zero() {
    let tiny = -f32::from_bits(1);
    let offset =
        lerp(DpOffset::new(tiny.dp(), tiny.dp()), DpOffset::new(tiny.dp(), tiny.dp()), 0.5);
    assert_eq!(offset, DpOffset::new(0.dp(), 0.dp()));
    assert_eq!((offset.x.value().to_bits(), offset.y.value().to_bits()), (0, 0));
    let size = lerp(DpSize::new(tiny.dp(), tiny.dp()), DpSize::new(tiny.dp(), tiny.dp()), 0.5);
    assert_eq!(size, DpSize::new(0.dp(), 0.dp()));
    assert_eq!((size.width.value().to_bits(), size.height.value().to_bits()), (0, 0));
}

// Not one of Compose's tests: `Dp`'s `toString`.
#[test]
fn dps_print_as_composes() {
    assert_eq!(1.dp().to_string(), "1.0.dp");
    assert_eq!(Dp::UNSPECIFIED.to_string(), "Dp.Unspecified");
}
