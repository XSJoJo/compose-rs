//! Compose's `IntRectTest.kt` (ui-unit): integer rectangles' constructors, intersections, sizes,
//! moves, insets, overlaps, dimensions, anchor points, containment, interpolation and conversions.

use compose_ui::{IntOffset, IntRect, IntSize, Rect, lerp};

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectCreatedByWidthAndHeight
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectFromIntOffsetAndSize
#[test]
fn a_rect_from_an_offset_and_a_size() {
    let rect = IntRect::from_offset_size(IntOffset::new(1, 3), IntSize::new(5, 7));
    assert_eq!((rect.left, rect.top, rect.right, rect.bottom), (1, 3, 6, 10));
    assert_eq!(
        IntRect::from_offset_size(IntOffset::new(220, 300), IntSize::new(80, 200)),
        IntRect::new(220, 300, 300, 500)
    );
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectIntersection
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectIntersect
#[test]
fn an_intersection_is_what_both_cover() {
    let first = IntRect::new(0, 0, 100, 100);
    let second = IntRect::new(50, 50, 200, 200);
    let intersection = first.intersect(second);
    assert_eq!(intersection, IntRect::new(50, 50, 100, 100));
    assert_eq!(second.intersect(first), intersection);
    assert_eq!(
        IntRect::new(0, 0, 20, 20).intersect(IntRect::new(10, 10, 30, 30)),
        IntRect::new(10, 10, 20, 20)
    );
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectWidth
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectHeight
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectSize
#[test]
fn a_rect_measures_its_sides() {
    let rect = IntRect::new(70, 10, 280, 300);
    assert_eq!(rect.width(), 210);
    assert_eq!(rect.height(), 290);
    assert_eq!(rect.size(), IntSize::new(210, 290));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectIsEmpty
#[test]
fn a_rect_without_area_is_empty() {
    assert!(IntRect::new(0, 0, 0, 10).is_empty());
    assert!(IntRect::new(1, 0, 0, 10).is_empty());
    assert!(IntRect::new(0, 1, 10, 0).is_empty());
    assert!(IntRect::new(0, 1, 10, 1).is_empty());
    assert!(!IntRect::new(0, 1, 2, 3).is_empty());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectTranslateIntOffset
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectTranslate
#[test]
fn a_translated_rect_moves_every_side() {
    let rect = IntRect::new(0, 5, 10, 15);
    assert_eq!(rect.translate(IntOffset::new(10, 15)), IntRect::new(10, 20, 20, 30));
    assert_eq!(rect.translate_xy(10, 15), IntRect::new(10, 20, 20, 30));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectInflate
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectDeflate
#[test]
fn inflating_and_deflating() {
    assert_eq!(IntRect::new(5, 10, 10, 20).inflate(5), IntRect::new(0, 5, 15, 25));
    assert_eq!(IntRect::new(0, 5, 15, 25).deflate(5), IntRect::new(5, 10, 10, 20));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectOverlap
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectDoesNotOverlap
#[test]
fn rects_overlap_only_when_they_share_area() {
    let first = IntRect::new(0, 5, 10, 15);
    let second = IntRect::new(5, 10, 15, 20);
    assert!(first.overlaps(second));
    assert!(second.overlaps(first));
    let touching = IntRect::new(10, 5, 20, 15);
    assert!(!first.overlaps(touching));
    assert!(!touching.overlaps(first));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectMinDimension
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectMaxDimension
#[test]
fn a_rects_dimensions() {
    let rect = IntRect::new(0, 5, 100, 25);
    assert_eq!(rect.min_dimension(), 20);
    assert_eq!(rect.max_dimension(), 100);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectTopLeft
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectTopCenter
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectTopRight
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectCenterLeft
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectCenter
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectCenterRight
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectBottomLeft
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectBottomCenter
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectBottomRight
#[test]
fn a_rect_has_nine_anchor_points() {
    assert_eq!(IntRect::new(27, 38, 100, 200).top_left(), IntOffset::new(27, 38));
    assert_eq!(IntRect::new(100, 15, 200, 300).top_center(), IntOffset::new(150, 15));
    assert_eq!(IntRect::new(100, 15, 200, 300).top_right(), IntOffset::new(200, 15));
    let rect = IntRect::new(100, 10, 200, 300);
    assert_eq!(rect.center_left(), IntOffset::new(100, 155));
    assert_eq!(rect.center(), IntOffset::new(150, 155));
    assert_eq!(rect.center_right(), IntOffset::new(200, 155));
    assert_eq!(rect.bottom_left(), IntOffset::new(100, 300));
    assert_eq!(rect.bottom_center(), IntOffset::new(150, 300));
    assert_eq!(rect.bottom_right(), IntOffset::new(200, 300));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectContains
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectDoesNotContain
#[test]
fn a_rect_contains_points_inside_it() {
    let rect = IntRect::new(100, 10, 200, 300);
    assert!(rect.contains(IntOffset::new(177, 288)));
    assert!(!rect.contains(IntOffset::new(201, 150)));
    assert!(!rect.contains(IntOffset::new(200, 301)));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectFromTopleftAndBottomRight
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectFromCenterAndRadius
#[test]
fn rects_from_corners_and_from_a_center() {
    assert_eq!(
        IntRect::from_corners(IntOffset::new(27, 38), IntOffset::new(130, 280)),
        IntRect::new(27, 38, 130, 280)
    );
    assert_eq!(IntRect::from_center(IntOffset::new(100, 50), 25), IntRect::new(75, 25, 125, 75));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRectLerp
#[test]
fn rects_interpolate() {
    let (first, second) = (IntRect::new(0, 0, 100, 100), IntRect::new(50, 50, 200, 200));
    assert_eq!(lerp(first, second, 0.5), IntRect::new(25, 25, 150, 150));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testToRect
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/IntRectTest.kt, testRoundRectToIntRect
#[test]
fn int_rects_and_rects_convert() {
    assert_eq!(IntRect::new(25, 25, 150, 150).to_rect(), Rect::new(25.0, 25.0, 150.0, 150.0));
    assert_eq!(Rect::new(2.4, 2.5, 3.9, 5.3).round_to_int_rect(), IntRect::new(2, 3, 4, 5));
}

// Not one of Compose's tests: `IntRect.Zero` and `toString`.
#[test]
fn the_zero_rect_and_printing() {
    assert_eq!(IntRect::ZERO, IntRect::new(0, 0, 0, 0));
    assert_eq!(IntRect::new(1, 2, 3, 4).to_string(), "IntRect.fromLTRB(1, 2, 3, 4)");
}
