//! Compose's `IntOffsetTest.kt` and `IntSizeTest.kt` (ui-unit): integer offsets' interpolation,
//! sums and conversion; integer sizes' arithmetic, center, destructuring and rounding from `Size`.

use compose_ui::{IntOffset, IntSize, Offset, Size, lerp};

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntOffsetTest.kt, lerpPosition
#[test]
fn offsets_interpolate_rounding() {
    let (a, b) = (IntOffset::new(3, 10), IntOffset::new(5, 8));
    assert_eq!(lerp(a, b, 0.5), IntOffset::new(4, 9));
    assert_eq!(lerp(a, b, 0.0), IntOffset::new(3, 10));
    assert_eq!(lerp(a, b, 1.0), IntOffset::new(5, 8));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntOffsetTest.kt, positionMinus
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntOffsetTest.kt, positionPlus
#[test]
fn offsets_add_and_subtract() {
    let (a, b) = (IntOffset::new(3, 10), IntOffset::new(5, 8));
    assert_eq!(a - b, IntOffset::new(-2, 2));
    assert_eq!(b - a, IntOffset::new(2, -2));
    assert_eq!(a + b, IntOffset::new(8, 18));
    assert_eq!(b + a, IntOffset::new(8, 18));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntOffsetTest.kt, toOffset
#[test]
fn an_int_offset_becomes_an_offset() {
    assert_eq!(IntOffset::new(3, 10).to_offset(), Offset::new(3.0, 10.0));
}

// Not one of Compose's tests: the rest of `IntOffset`'s arithmetic (negation, scaling and division
// rounded, remainder, mixed sums with `Offset`, `round`, `Max`).
#[test]
fn int_offsets_scale_round_and_mix_with_offsets() {
    let offset = IntOffset::new(3, -5);
    assert_eq!(-offset, IntOffset::new(-3, 5));
    assert_eq!(offset * 1.5, IntOffset::new(5, -7));
    assert_eq!(offset / 2.0, IntOffset::new(2, -2));
    assert_eq!(offset % 2, IntOffset::new(1, -1));
    assert_eq!(Offset::new(0.5, 0.5) + offset, Offset::new(3.5, -4.5));
    assert_eq!(Offset::new(0.5, 0.5) - offset, Offset::new(-2.5, 5.5));
    assert_eq!(offset + Offset::new(0.5, 0.5), Offset::new(3.5, -4.5));
    assert_eq!(offset - Offset::new(0.5, 0.5), Offset::new(2.5, -5.5));
    assert_eq!(Offset::new(2.5, -2.5).round(), IntOffset::new(3, -2));
    assert_eq!(IntOffset::MAX, IntOffset::new(i32::MAX, i32::MAX));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntSizeTest.kt, constructor
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntSizeTest.kt, components
#[test]
fn a_size_keeps_and_destructures_its_sides() {
    let size = IntSize::new(5, 10);
    assert_eq!((size.width, size.height), (5, 10));
    let extreme = IntSize::new(i32::MAX, i32::MIN);
    assert_eq!((extreme.width, extreme.height), (i32::MAX, i32::MIN));
    let IntSize { width, height } = IntSize::new(10, 20);
    assert_eq!((width, height), (10, 20));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntSizeTest.kt, intSizeTimesInt
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntSizeTest.kt, intSizeDivInt
#[test]
fn sizes_multiply_and_divide() {
    assert_eq!(IntSize::new(2, 2) * 5, IntSize::new(10, 10));
    assert_eq!(5 * IntSize::new(2, 2), IntSize::new(10, 10));
    assert_eq!(IntSize::new(40, 40) / 4, IntSize::new(10, 10));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntSizeTest.kt, sizeCenter
#[test]
fn a_sizes_center_is_half_of_it() {
    assert_eq!(IntSize::new(10, 20).center(), IntOffset::new(5, 10));
    assert_eq!(IntSize::new(-10, -20).center(), IntOffset::new(-5, -10));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntSizeTest.kt, sizeToIntSizeRoundsDown
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntSizeTest.kt, sizeToIntSizeRounded
#[test]
fn a_size_truncates_or_rounds_to_an_int_size() {
    let size = Size::new(10.8, 12.5);
    assert_eq!(size.to_int_size(), IntSize::new(10, 12));
    assert_eq!(size.round_to_int_size(), IntSize::new(11, 13));
}

// Not one of Compose's tests: `IntSize`'s conversions and printing.
#[test]
fn an_int_size_becomes_a_size_and_a_rect() {
    assert_eq!(IntSize::new(3, 4).to_size(), Size::new(3.0, 4.0));
    assert_eq!(IntSize::new(3, 4).to_int_rect(), compose_ui::IntRect::new(0, 0, 3, 4));
    assert_eq!(IntSize::new(3, 4).to_string(), "3 x 4");
}
