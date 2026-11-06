//! Compose's `OffsetTest.kt` (ui-geometry): `Offset`'s copies, unspecified value, negation,
//! finiteness and validity. Copies are struct update syntax.

use compose_ui::Offset;

/// A NaN with another payload than the canonical one, as the test's `AnotherNaN`.
fn another_nan() -> f32 {
    f32::from_bits(0x7f80_0001)
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testOffsetCopy
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testOffsetCopyOverwriteX
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testOffsetCopyOverwriteY
#[test]
fn a_copy_changes_only_what_it_names() {
    let offset = Offset::new(100.0, 200.0);
    assert_eq!(Offset { ..offset }, offset);
    let copy = Offset { x: 50.0, ..offset };
    assert_eq!((copy.x, copy.y), (50.0, 200.0));
    let copy = Offset { y: 300.0, ..offset };
    assert_eq!((copy.x, copy.y), (100.0, 300.0));
}

// Compose's four tests catch the `fail` they call, so they assert nothing: reading an unspecified
// offset's coordinates, copying it or destructuring it is allowed, and gives NaN.
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testUnspecifiedWidthQueryThrows
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testUnspecifiedHeightQueryThrows
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testUnspecifiedCopyThrows
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testUnspecifiedComponentAssignmentThrows
#[test]
fn an_unspecified_offsets_coordinates_are_nan() {
    let Offset { x, y } = Offset::UNSPECIFIED;
    assert!(x.is_nan() && y.is_nan());
    assert!(Offset { x: 100.0, ..Offset::UNSPECIFIED }.y.is_nan());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testIsSpecified
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testIsUnspecified
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testUnspecifiedEquals
#[test]
fn only_the_unspecified_offset_is_unspecified() {
    let offset = Offset::new(10.0, 20.0);
    assert!(offset.is_specified());
    assert!(!offset.is_unspecified());
    assert!(Offset::UNSPECIFIED.is_unspecified());
    assert!(!Offset::UNSPECIFIED.is_specified());
    assert_eq!(Offset::UNSPECIFIED, Offset::UNSPECIFIED);
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testTakeOrElseTrue
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testTakeOrElseFalse
#[test]
fn take_or_else_replaces_only_an_unspecified_offset() {
    assert!(Offset::new(1.0, 1.0).take_or_else(|| Offset::UNSPECIFIED).is_specified());
    assert!(Offset::UNSPECIFIED.take_or_else(|| Offset::new(1.0, 1.0)).is_specified());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testUnspecifiedOffsetToString
#[test]
fn the_unspecified_offset_prints_its_name() {
    assert_eq!(Offset::UNSPECIFIED.to_string(), "Offset.Unspecified");
    assert_eq!(Offset::new(1.04, 2.06).to_string(), "Offset(1.0, 2.1)");
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testUnaryMinus
#[test]
fn negation_flips_both_signs() {
    assert_eq!(-Offset::new(10.0, 20.0), Offset::new(-10.0, -20.0));
    assert_eq!(-Offset::new(-10.0, -20.0), Offset::new(10.0, 20.0));
    assert_eq!(-Offset::new(10.0, f32::INFINITY), Offset::new(-10.0, f32::NEG_INFINITY));
    assert_eq!(-Offset::new(10.0, f32::NEG_INFINITY), Offset::new(-10.0, f32::INFINITY));

    let minus_unspecified = -Offset::new(f32::NAN, f32::NAN);
    assert!(minus_unspecified.x.is_nan());
    assert!(minus_unspecified.y.is_nan());
    assert!(minus_unspecified.is_unspecified());
    assert!(!minus_unspecified.is_specified());
    assert!(!minus_unspecified.is_finite());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testIsFinite
#[test]
fn finite_offsets_have_no_infinity_and_no_nan() {
    assert!(Offset::new(10.0, 20.0).is_finite());
    assert!(Offset::new(0.0, 0.0).is_finite());
    assert!(Offset::new(10.0, -20.0).is_finite());

    assert!(!Offset::new(10.0, f32::INFINITY).is_finite());
    assert!(!Offset::new(10.0, f32::NEG_INFINITY).is_finite());
    assert!(!Offset::new(f32::INFINITY, 20.0).is_finite());
    assert!(!Offset::new(f32::NEG_INFINITY, 20.0).is_finite());
    assert!(!Offset::new(f32::INFINITY, f32::INFINITY).is_finite());

    assert!(!Offset::UNSPECIFIED.is_finite());
    assert!(!Offset::new(f32::NAN, 10.0).is_finite());
    assert!(!Offset::new(10.0, f32::NAN).is_finite());
    assert!(!Offset::new(another_nan(), another_nan()).is_finite());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/OffsetTest.kt, testIsValid
#[test]
fn valid_offsets_have_no_nan() {
    assert!(Offset::new(10.0, 20.0).is_valid());
    assert!(Offset::new(0.0, 0.0).is_valid());
    assert!(Offset::new(10.0, -20.0).is_valid());

    assert!(Offset::new(10.0, f32::INFINITY).is_valid());
    assert!(Offset::new(10.0, f32::NEG_INFINITY).is_valid());
    assert!(Offset::new(f32::INFINITY, 20.0).is_valid());
    assert!(Offset::new(f32::NEG_INFINITY, 20.0).is_valid());
    assert!(Offset::new(f32::INFINITY, f32::INFINITY).is_valid());

    assert!(!Offset::UNSPECIFIED.is_valid());
    assert!(!Offset::new(f32::NAN, 10.0).is_valid());
    assert!(!Offset::new(10.0, f32::NAN).is_valid());
    assert!(!Offset::new(another_nan(), another_nan()).is_valid());
}

// Not one of Compose's tests: the arithmetic `Offset` has beside them (`times`, `div`, `rem`,
// `getDistance`, `lerp`), which Compose's tests of other types lean on.
#[test]
fn offsets_scale_divide_and_measure() {
    let offset = Offset::new(3.0, -4.0);
    assert_eq!(offset * 2.0, Offset::new(6.0, -8.0));
    assert_eq!(2.0 * offset, Offset::new(6.0, -8.0));
    assert_eq!(offset / 2.0, Offset::new(1.5, -2.0));
    assert_eq!(Offset::new(7.0, -7.0) % 3.0, Offset::new(1.0, -1.0));
    assert_eq!(offset.get_distance(), 5.0);
    assert_eq!(offset.get_distance_squared(), 25.0);
    assert_eq!(
        compose_ui::lerp(Offset::new(0.0, 10.0), Offset::new(10.0, 30.0), 0.5),
        Offset::new(5.0, 20.0)
    );
}
