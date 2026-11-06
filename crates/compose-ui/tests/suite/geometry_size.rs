//! Compose's `SizeTest.kt` (ui-geometry): `Size`'s arithmetic, copies, interpolation, unspecified
//! value, printing and emptiness. Copies are struct update syntax.

use compose_ui::{Size, lerp};

// Compose's `Int` and `Double` variants pass floats too (`4f`, `2.5f`), so all six are the `Float`
// operators.
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, sizeTimesInt
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, sizeTimesFloat
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, sizeTimesDouble
#[test]
fn sizes_multiply_either_way() {
    assert_eq!(Size::new(2.5, 2.5) * 4.0, Size::new(10.0, 10.0));
    assert_eq!(4.0 * Size::new(2.5, 2.5), Size::new(10.0, 10.0));
    assert_eq!(Size::new(4.0, 4.0) * 2.5, Size::new(10.0, 10.0));
    assert_eq!(2.5 * Size::new(4.0, 4.0), Size::new(10.0, 10.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, sizeDivInt
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, sizeDivFloat
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, sizeDivDouble
#[test]
fn sizes_divide() {
    assert_eq!(Size::new(40.0, 40.0) / 4.0, Size::new(10.0, 10.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testSizeCopy
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testSizeCopyOverwriteWidth
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testSizeCopyOverwriteHeight
#[test]
fn a_copy_changes_only_what_it_names() {
    let size = Size::new(100.0, 200.0);
    assert_eq!(Size { ..size }, size);
    let copy = Size { width: 50.0, ..size };
    assert_eq!((copy.width, copy.height), (50.0, 200.0));
    let copy = Size { height: 300.0, ..size };
    assert_eq!((copy.width, copy.height), (100.0, 300.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testSizeLerp
#[test]
fn sizes_interpolate() {
    assert_eq!(
        lerp(Size::new(100.0, 200.0), Size::new(300.0, 500.0), 0.5),
        Size::new(200.0, 350.0)
    );
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testIsSpecified
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testIsUnspecified
#[test]
fn only_the_unspecified_size_is_unspecified() {
    assert!(!Size::UNSPECIFIED.is_specified());
    assert!(Size::new(1.0, 1.0).is_specified());
    assert!(Size::UNSPECIFIED.is_unspecified());
    assert!(!Size::new(1.0, 1.0).is_unspecified());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testTakeOrElseTrue
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testTakeOrElseFalse
#[test]
fn take_or_else_replaces_only_an_unspecified_size() {
    assert!(Size::new(1.0, 1.0).take_or_else(|| Size::UNSPECIFIED).is_specified());
    assert!(Size::UNSPECIFIED.take_or_else(|| Size::new(1.0, 1.0)).is_specified());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testUnspecifiedSizeToString
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testSpecifiedSizeToString
#[test]
fn sizes_print_as_composes() {
    assert_eq!(Size::UNSPECIFIED.to_string(), "Size.Unspecified");
    assert_eq!(Size::new(10.0, 20.0).to_string(), "Size(10.0, 20.0)");
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/SizeTest.kt, testIsEmpty
#[test]
fn a_size_without_area_is_empty() {
    assert!(!Size::new(10.0, 20.0).is_empty());
    assert!(!Size::new(10.0, f32::INFINITY).is_empty());
    assert!(!Size::new(f32::INFINITY, 20.0).is_empty());

    assert!(Size::new(0.0, 20.0).is_empty());
    assert!(Size::new(10.0, 0.0).is_empty());
    assert!(Size::new(0.0, 0.0).is_empty());
    assert!(Size::new(-10.0, 20.0).is_empty());
    assert!(Size::new(10.0, -20.0).is_empty());
    assert!(Size::new(0.0, f32::INFINITY).is_empty());
    assert!(Size::new(f32::INFINITY, 0.0).is_empty());
    assert!(Size::new(0.0, f32::NEG_INFINITY).is_empty());
    assert!(Size::new(f32::NEG_INFINITY, 0.0).is_empty());
    assert!(Size::new(f32::NEG_INFINITY, 20.0).is_empty());
    assert!(Size::new(10.0, f32::NEG_INFINITY).is_empty());
    assert!(!Size::new(1.0, 2.0).is_empty());
    assert!(!Size::new(114.87094, 0.00793457).is_empty());

    assert!(Size::UNSPECIFIED.is_empty());
}

// Not one of Compose's tests: the rest of `Size`'s API (`minDimension`, `maxDimension`, `center`,
// `toRect`).
#[test]
fn a_size_has_dimensions_a_center_and_a_rect() {
    let size = Size::new(-30.0, 20.0);
    assert_eq!(size.min_dimension(), 20.0);
    assert_eq!(size.max_dimension(), 30.0);
    assert_eq!(Size::new(30.0, 20.0).center(), compose_ui::Offset::new(15.0, 10.0));
    assert_eq!(Size::new(30.0, 20.0).to_rect(), compose_ui::Rect::new(0.0, 0.0, 30.0, 20.0));
}
