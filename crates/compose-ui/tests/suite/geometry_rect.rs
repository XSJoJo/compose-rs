//! Compose's `RectTest.kt` (ui-geometry): `Rect`'s accessors, constructors, intersections, moves,
//! insets, overlaps, dimensions, anchor points, containment and interpolation.

use compose_ui::{Offset, Rect, Size, lerp};

fn assert_near(expected: f32, actual: f32) {
    assert!((expected - actual).abs() <= 0.01, "expected {expected}, got {actual}");
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectAccessors
#[test]
fn a_rect_keeps_its_sides() {
    let rect = Rect::new(1.0, 3.0, 5.0, 7.0);
    assert_near(1.0, rect.left);
    assert_near(3.0, rect.top);
    assert_near(5.0, rect.right);
    assert_near(7.0, rect.bottom);
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectCreatedByWidthAndHeight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectFromOffsetAndSize
#[test]
fn a_rect_from_an_offset_and_a_size_ends_where_the_size_does() {
    let rect = Rect::from_offset_size(Offset::new(1.0, 3.0), Size::new(5.0, 7.0));
    assert_near(1.0, rect.left);
    assert_near(3.0, rect.top);
    assert_near(6.0, rect.right);
    assert_near(10.0, rect.bottom);
    assert_eq!(
        Rect::from_offset_size(Offset::new(220.0, 300.0), Size::new(80.0, 200.0)),
        Rect::new(220.0, 300.0, 300.0, 500.0)
    );
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectIntersection
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectIntersect
#[test]
fn an_intersection_is_what_both_cover() {
    let first = Rect::new(0.0, 0.0, 100.0, 100.0);
    let second = Rect::new(50.0, 50.0, 200.0, 200.0);
    let intersection = first.intersect(second);
    assert_near(50.0, intersection.left);
    assert_near(50.0, intersection.top);
    assert_near(100.0, intersection.right);
    assert_near(100.0, intersection.bottom);
    assert_eq!(second.intersect(first), intersection);

    assert_eq!(
        Rect::new(0.0, 0.0, 20.0, 20.0).intersect(Rect::new(10.0, 10.0, 30.0, 30.0)),
        Rect::new(10.0, 10.0, 20.0, 20.0)
    );
    assert_eq!(
        Rect::new(0.0, 0.0, 20.0, 20.0).intersect_ltrb(10.0, 10.0, 30.0, 30.0),
        Rect::new(10.0, 10.0, 20.0, 20.0)
    );
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectWidth
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectHeight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectSize
#[test]
fn a_rect_measures_its_sides() {
    let rect = Rect::new(70.0, 10.0, 280.0, 300.0);
    assert_eq!(rect.width(), 210.0);
    assert_eq!(rect.height(), 290.0);
    assert_eq!(rect.size(), Size::new(210.0, 290.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectInfinite
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectFinite
#[test]
fn a_side_at_infinity_makes_a_rect_infinite() {
    assert!(Rect::new(f32::INFINITY, 10.0, 200.0, 500.0).is_infinite());
    assert!(Rect::new(10.0, f32::INFINITY, 200.0, 500.0).is_infinite());
    assert!(Rect::new(10.0, 200.0, f32::INFINITY, 500.0).is_infinite());
    assert!(Rect::new(10.0, 200.0, 500.0, f32::INFINITY).is_infinite());
    assert!(!Rect::new(0.0, 1.0, 2.0, 3.0).is_infinite());

    assert!(Rect::new(0.0, 1.0, 2.0, 3.0).is_finite());
    assert!(!Rect::new(0.0, 1.0, 2.0, f32::INFINITY).is_finite());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectIsEmpty
#[test]
fn a_rect_without_area_is_empty() {
    assert!(Rect::new(0.0, 0.0, 0.0, 10.0).is_empty());
    assert!(Rect::new(1.0, 0.0, 0.0, 10.0).is_empty());
    assert!(Rect::new(0.0, 1.0, 10.0, 0.0).is_empty());
    assert!(Rect::new(0.0, 1.0, 10.0, 1.0).is_empty());
    assert!(!Rect::new(0.0, 1.0, 2.0, 3.0).is_empty());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectTranslateOffset
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectTranslate
#[test]
fn a_translated_rect_moves_every_side() {
    let rect = Rect::new(0.0, 5.0, 10.0, 15.0);
    assert_eq!(rect.translate(Offset::new(10.0, 15.0)), Rect::new(10.0, 20.0, 20.0, 30.0));
    assert_eq!(rect.translate_xy(10.0, 15.0), Rect::new(10.0, 20.0, 20.0, 30.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectInflate
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectDeflate
#[test]
fn inflating_and_deflating_move_every_side_out_or_in() {
    assert_eq!(Rect::new(5.0, 10.0, 10.0, 20.0).inflate(5.0), Rect::new(0.0, 5.0, 15.0, 25.0));
    assert_eq!(Rect::new(0.0, 5.0, 15.0, 25.0).deflate(5.0), Rect::new(5.0, 10.0, 10.0, 20.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectOverlap
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectDoesNotOverlap
#[test]
fn rects_overlap_only_when_they_share_area() {
    let first = Rect::new(0.0, 5.0, 10.0, 15.0);
    let second = Rect::new(5.0, 10.0, 15.0, 20.0);
    assert!(first.overlaps(second));
    assert!(second.overlaps(first));

    let touching = Rect::new(10.0, 5.0, 20.0, 15.0);
    assert!(!first.overlaps(touching));
    assert!(!touching.overlaps(first));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectMinDimension
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectMaxDimension
#[test]
fn a_rects_dimensions_are_its_shorter_and_longer_sides() {
    let rect = Rect::new(0.0, 5.0, 100.0, 25.0);
    assert_eq!(rect.min_dimension(), 20.0);
    assert_eq!(rect.max_dimension(), 100.0);
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectTopLeft
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectTopCenter
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectTopRight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectCenterLeft
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectCenter
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectCenterRight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectBottomLeft
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectBottomCenter
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectBottomRight
#[test]
fn a_rect_has_nine_anchor_points() {
    assert_eq!(Rect::new(27.0, 38.0, 100.0, 200.0).top_left(), Offset::new(27.0, 38.0));
    assert_eq!(Rect::new(100.0, 15.0, 200.0, 300.0).top_center(), Offset::new(150.0, 15.0));
    assert_eq!(Rect::new(100.0, 15.0, 200.0, 300.0).top_right(), Offset::new(200.0, 15.0));
    let rect = Rect::new(100.0, 10.0, 200.0, 300.0);
    assert_eq!(rect.center_left(), Offset::new(100.0, 155.0));
    assert_eq!(rect.center(), Offset::new(150.0, 155.0));
    assert_eq!(rect.center_right(), Offset::new(200.0, 155.0));
    assert_eq!(rect.bottom_left(), Offset::new(100.0, 300.0));
    assert_eq!(rect.bottom_center(), Offset::new(150.0, 300.0));
    assert_eq!(rect.bottom_right(), Offset::new(200.0, 300.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectContains
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectDoesNotContain
#[test]
fn a_rect_contains_points_inside_it() {
    let rect = Rect::new(100.0, 10.0, 200.0, 300.0);
    assert!(rect.contains(Offset::new(177.0, 288.0)));
    assert!(!rect.contains(Offset::new(201.0, 150.0)));
    assert!(!rect.contains(Offset::new(200.0, 301.0)));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectFromTopleftAndBottomRight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectFromCenterAndRadius
#[test]
fn rects_from_corners_and_from_a_center() {
    assert_eq!(
        Rect::from_corners(Offset::new(27.0, 38.0), Offset::new(130.0, 280.0)),
        Rect::new(27.0, 38.0, 130.0, 280.0)
    );
    assert_eq!(
        Rect::from_center(Offset::new(100.0, 50.0), 25.0),
        Rect::new(75.0, 25.0, 125.0, 75.0)
    );
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RectTest.kt, testRectLerp
#[test]
fn rects_interpolate_side_by_side() {
    let first = Rect::new(0.0, 0.0, 100.0, 100.0);
    let second = Rect::new(50.0, 50.0, 200.0, 200.0);
    assert_eq!(lerp(first, second, 0.5), Rect::new(25.0, 25.0, 150.0, 150.0));
}

// Not one of Compose's tests: `Rect.Zero` and `toString`.
#[test]
fn the_zero_rect_and_printing() {
    assert_eq!(Rect::ZERO, Rect::new(0.0, 0.0, 0.0, 0.0));
    assert_eq!(Rect::new(1.0, 2.25, 3.0, 4.0).to_string(), "Rect.fromLTRB(1.0, 2.3, 3.0, 4.0)");
}
