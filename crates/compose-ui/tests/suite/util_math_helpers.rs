//! Compose's `MathHelpersTest.kt` (ui-util): `lerp` of floats, ints and longs, exact at both ends
//! and on simple fractions. Its tests of `fastCbrt`, `normalizedAngleCos`/`Sin` and `fastIsFinite`
//! are of Kotlin performance helpers compose-rs doesn't port (Rust's `cbrt`, `cos`, `sin` and
//! `is_finite` are the ones to use).

use compose_ui::lerp;

/// The first float where `f + 1 == f`, where floating-point errors become interesting.
const TEST_START: i64 = 16_777_216;
const TEST_END: i64 = TEST_START + 1000;

// Compose: compose/ui/ui-util/src/commonTest/kotlin/androidx/compose/ui/util/MathHelpersTest.kt, testLerpLargeFloats
#[test]
fn float_lerp_is_exact_at_its_ends_for_large_values() {
    let from = 1.0_f32;
    for x in TEST_START..TEST_END {
        let to = x as f32;
        assert_eq!(lerp(from, to, 0.0), from);
        assert_eq!(lerp(from, to, 1.0), to);
    }
}

// Compose: compose/ui/ui-util/src/commonTest/kotlin/androidx/compose/ui/util/MathHelpersTest.kt, testLerpLargeInts
#[test]
fn int_lerp_is_exact_at_its_ends_for_large_values() {
    let from = 1_i32;
    for x in TEST_START..TEST_END {
        let to = x as i32;
        assert_eq!(lerp(from, to, 0.0), from);
        assert_eq!(lerp(from, to, 1.0), to);
    }
}

// Compose: compose/ui/ui-util/src/commonTest/kotlin/androidx/compose/ui/util/MathHelpersTest.kt, testLerpLargeLongs
#[test]
fn long_lerp_is_exact_at_its_ends_for_large_values() {
    let from = 1_i64;
    for to in TEST_START..TEST_END {
        assert_eq!(lerp(from, to, 0.0), from);
        assert_eq!(lerp(from, to, 1.0), to);
    }
}

// Compose: compose/ui/ui-util/src/commonTest/kotlin/androidx/compose/ui/util/MathHelpersTest.kt, testLerpSimpleFloats
#[test]
fn float_lerp_is_exact_on_quarters() {
    for multiplier in 1..=1000 {
        let to = (4 * multiplier) as f32;
        for quarter in 0..=4 {
            let expected = (quarter * multiplier) as f32;
            assert_eq!(lerp(0.0, to, quarter as f32 * 0.25), expected);
        }
    }
}

// Compose: compose/ui/ui-util/src/commonTest/kotlin/androidx/compose/ui/util/MathHelpersTest.kt, testLerpSimpleInts
#[test]
fn int_lerp_is_exact_on_quarters() {
    for multiplier in 1..=1000 {
        let to = 4 * multiplier;
        for quarter in 0..=4 {
            assert_eq!(lerp(0, to, quarter as f32 * 0.25), quarter * multiplier);
        }
    }
}

// Compose: compose/ui/ui-util/src/commonTest/kotlin/androidx/compose/ui/util/MathHelpersTest.kt, testLerpSimpleLongs
#[test]
fn long_lerp_is_exact_on_quarters() {
    for multiplier in 1..=1000_i64 {
        let to = 4 * multiplier;
        for quarter in 0..=4 {
            assert_eq!(lerp(0_i64, to, quarter as f32 * 0.25), quarter * multiplier);
        }
    }
}
