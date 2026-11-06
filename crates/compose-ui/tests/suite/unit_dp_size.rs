//! Compose's `DpSizeTest.kt` (ui-unit): `DpSize`'s arithmetic, center, destructuring,
//! interpolation, unspecified value and printing.

use compose_ui::{DpOffset, DpRect, DpSize, UnitExt, lerp};

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, constructor
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, components
#[test]
fn a_size_keeps_and_destructures_its_sides() {
    let size = DpSize::new(5.dp(), 10.dp());
    assert_eq!((size.width, size.height), (5.dp(), 10.dp()));
    let DpSize { width, height } = DpSize::new(10.dp(), 20.dp());
    assert_eq!((width, height), (10.dp(), 20.dp()));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, dpSizeTimesInt
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, dpSizeTimesFloat
#[test]
fn sizes_multiply_either_way() {
    let ten = DpSize::new(10.dp(), 10.dp());
    assert_eq!(DpSize::new(2.dp(), 2.dp()) * 5, ten);
    assert_eq!(5 * DpSize::new(2.dp(), 2.dp()), ten);
    assert_eq!(DpSize::new(2.dp(), 2.dp()) * 5.0, ten);
    assert_eq!(5.0 * DpSize::new(2.dp(), 2.dp()), ten);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, dpSizeDivInt
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, dpSizeDivFloat
#[test]
fn sizes_divide() {
    assert_eq!(DpSize::new(40.dp(), 40.dp()) / 4, DpSize::new(10.dp(), 10.dp()));
    assert_eq!(DpSize::new(40.dp(), 40.dp()) / 4.0, DpSize::new(10.dp(), 10.dp()));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, sizeCenter
#[test]
fn a_sizes_center_is_half_of_it() {
    assert_eq!(DpSize::new(10.dp(), 20.dp()).center(), DpOffset::new(5.dp(), 10.dp()));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, lerp
#[test]
fn sizes_interpolate() {
    let (start, stop) = (DpSize::new(1.dp(), 2.dp()), DpSize::new(10.dp(), 5.dp()));
    assert_eq!(lerp(start, stop, 0.0), start);
    assert_eq!(lerp(start, stop, 1.0), stop);
    let negative = DpSize::new(-(10.dp()), -(5.dp()));
    assert_eq!(lerp(negative, stop, 0.5), DpSize::new(0.dp(), 0.dp()));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, dpRectSize
#[test]
fn a_rects_size() {
    assert_eq!(DpRect::new(2.dp(), 3.dp(), 12.dp(), 8.dp()).size(), DpSize::new(10.dp(), 5.dp()));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, testIsSpecified
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, testIsUnspecified
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, testTakeOrElseTrue
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, testTakeOrElseFalse
#[test]
fn only_the_unspecified_size_is_unspecified() {
    assert!(!DpSize::UNSPECIFIED.is_specified());
    assert!(DpSize::new(1.dp(), 1.dp()).is_specified());
    assert!(DpSize::UNSPECIFIED.is_unspecified());
    assert!(!DpSize::new(1.dp(), 1.dp()).is_unspecified());
    assert!(DpSize::new(1.dp(), 1.dp()).take_or_else(|| DpSize::UNSPECIFIED).is_specified());
    assert!(DpSize::UNSPECIFIED.take_or_else(|| DpSize::new(1.dp(), 1.dp())).is_specified());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, testToString
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DpSizeTest.kt, testUnspecifiedToString
#[test]
fn sizes_print_as_composes() {
    assert_eq!(DpSize::new(1.dp(), 1.dp()).to_string(), "1.0.dp x 1.0.dp");
    assert_eq!(DpSize::UNSPECIFIED.to_string(), "DpSize.Unspecified");
}

// Not one of Compose's tests: `DpSize`'s sum, difference and zero.
#[test]
fn sizes_add_and_subtract() {
    let size = DpSize::new(3.dp(), 4.dp());
    assert_eq!(size + size, DpSize::new(6.dp(), 8.dp()));
    assert_eq!(size - size, DpSize::ZERO);
}
