//! Compose's `MutableRectTest.kt` (ui-geometry): `MutableRect`'s accessors, changes in place,
//! overlaps, anchor points and containment.

use compose_ui::{MutableRect, Offset, Rect, Size};

fn assert_near(expected: f32, actual: f32) {
    assert!((expected - actual).abs() <= 0.01, "expected {expected}, got {actual}");
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testAccessors
#[test]
fn a_mutable_rect_keeps_its_sides() {
    let rect = MutableRect::new(1.0, 3.0, 5.0, 9.0);
    assert_eq!((rect.left, rect.top, rect.right, rect.bottom), (1.0, 3.0, 5.0, 9.0));
    assert_eq!((rect.width(), rect.height()), (4.0, 6.0));
    assert_eq!(rect.size(), Size::new(4.0, 6.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectCreatedByWidthAndHeight
#[test]
fn a_mutable_rect_from_an_offset_and_a_size() {
    let rect = MutableRect::from_offset_size(Offset::new(1.0, 3.0), Size::new(5.0, 7.0));
    assert_near(1.0, rect.left);
    assert_near(3.0, rect.top);
    assert_near(6.0, rect.right);
    assert_near(10.0, rect.bottom);
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectWidth
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectHeight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectSize
#[test]
fn a_mutable_rect_measures_its_sides() {
    let rect = MutableRect::new(70.0, 10.0, 280.0, 300.0);
    assert_eq!(rect.width(), 210.0);
    assert_eq!(rect.height(), 290.0);
    assert_eq!(rect.size(), Size::new(210.0, 290.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectInfinite
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectFinite
#[test]
fn a_side_at_infinity_makes_a_mutable_rect_infinite() {
    assert!(MutableRect::new(f32::INFINITY, 10.0, 200.0, 500.0).is_infinite());
    assert!(MutableRect::new(10.0, f32::INFINITY, 200.0, 500.0).is_infinite());
    assert!(MutableRect::new(10.0, 200.0, f32::INFINITY, 500.0).is_infinite());
    assert!(MutableRect::new(10.0, 200.0, 500.0, f32::INFINITY).is_infinite());
    assert!(!MutableRect::new(0.0, 1.0, 2.0, 3.0).is_infinite());

    assert!(MutableRect::new(0.0, 1.0, 2.0, 3.0).is_finite());
    assert!(!MutableRect::new(0.0, 1.0, 2.0, f32::INFINITY).is_finite());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testEmpty
#[test]
fn a_mutable_rect_is_empty_once_a_side_meets_its_opposite() {
    let mut rect = MutableRect::new(1.0, 3.0, 5.0, 9.0);
    assert!(!rect.is_empty());
    rect.left = 5.0;
    assert!(rect.is_empty());
    rect.left = 1.0;
    rect.bottom = 3.0;
    assert!(rect.is_empty());
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectTranslateOffset
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectTranslate
#[test]
fn translating_moves_every_side_in_place() {
    let mut shifted = MutableRect::new(0.0, 5.0, 10.0, 15.0);
    shifted.translate(Offset::new(10.0, 15.0));
    assert_eq!(shifted.to_rect(), MutableRect::new(10.0, 20.0, 20.0, 30.0).to_rect());

    let mut translated = MutableRect::new(0.0, 5.0, 10.0, 15.0);
    translated.translate_xy(10.0, 15.0);
    assert_eq!(translated.to_rect(), Rect::new(10.0, 20.0, 20.0, 30.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectInflate
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectDeflate
#[test]
fn inflating_and_deflating_in_place() {
    let mut inflated = MutableRect::new(5.0, 10.0, 10.0, 20.0);
    inflated.inflate(5.0);
    assert_eq!(inflated.to_rect(), Rect::new(0.0, 5.0, 15.0, 25.0));

    let mut deflated = MutableRect::new(0.0, 5.0, 15.0, 25.0);
    deflated.deflate(5.0);
    assert_eq!(deflated.to_rect(), Rect::new(5.0, 10.0, 10.0, 20.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectIntersect
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testIntersect
#[test]
fn intersecting_in_place_keeps_what_both_cover() {
    let mut intersected = MutableRect::new(0.0, 0.0, 20.0, 20.0);
    intersected.intersect(10.0, 10.0, 30.0, 30.0);
    assert_eq!(intersected.to_rect(), Rect::new(10.0, 10.0, 20.0, 20.0));

    let mut first = MutableRect::new(0.0, 0.0, 100.0, 100.0);
    first.intersect(50.0, 50.0, 200.0, 200.0);
    assert_eq!((first.left, first.top, first.right, first.bottom), (50.0, 50.0, 100.0, 100.0));
    let mut second = MutableRect::new(50.0, 50.0, 200.0, 200.0);
    second.intersect(0.0, 0.0, 100.0, 100.0);
    assert_eq!((second.left, second.top, second.right, second.bottom), (50.0, 50.0, 100.0, 100.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectOverlap
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectDoesNotOverlap
#[test]
fn mutable_rects_overlap_only_when_they_share_area() {
    let first = MutableRect::new(0.0, 5.0, 10.0, 15.0);
    let second = MutableRect::new(5.0, 10.0, 15.0, 20.0);
    assert!(first.overlaps(&second));
    assert!(second.overlaps(&first));
    assert!(first.overlaps_rect(second.to_rect()));

    let touching = MutableRect::new(10.0, 5.0, 20.0, 15.0);
    assert!(!first.overlaps(&touching));
    assert!(!touching.overlaps(&first));
    assert!(!first.overlaps_rect(touching.to_rect()));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectMinDimension
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectMaxDimension
#[test]
fn a_mutable_rects_dimensions() {
    let rect = MutableRect::new(0.0, 5.0, 100.0, 25.0);
    assert_eq!(rect.min_dimension(), 20.0);
    assert_eq!(rect.max_dimension(), 100.0);
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectTopLeft
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectTopCenter
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectTopRight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectCenterLeft
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectCenter
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectCenterRight
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, restRectBottomLeft
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectBottomCenter
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testRectBottomRight
#[test]
fn a_mutable_rect_has_nine_anchor_points() {
    assert_eq!(MutableRect::new(27.0, 38.0, 100.0, 200.0).top_left(), Offset::new(27.0, 38.0));
    assert_eq!(MutableRect::new(100.0, 15.0, 200.0, 300.0).top_center(), Offset::new(150.0, 15.0));
    assert_eq!(MutableRect::new(100.0, 15.0, 200.0, 300.0).top_right(), Offset::new(200.0, 15.0));
    let rect = MutableRect::new(100.0, 10.0, 200.0, 300.0);
    assert_eq!(rect.center_left(), Offset::new(100.0, 155.0));
    assert_eq!(rect.center(), Offset::new(150.0, 155.0));
    assert_eq!(rect.center_right(), Offset::new(200.0, 155.0));
    assert_eq!(rect.bottom_left(), Offset::new(100.0, 300.0));
    assert_eq!(rect.bottom_center(), Offset::new(150.0, 300.0));
    assert_eq!(rect.bottom_right(), Offset::new(200.0, 300.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testContains
#[test]
fn a_mutable_rect_contains_its_top_left_but_not_its_bottom_right() {
    let rect = MutableRect::new(1.0, 3.0, 5.0, 9.0);
    assert!(rect.contains(Offset::new(1.0, 3.0)));
    assert!(rect.contains(Offset::new(3.0, 3.0)));
    assert!(!rect.contains(Offset::new(5.0, 3.0)));
    assert!(rect.contains(Offset::new(1.0, 6.0)));
    assert!(rect.contains(Offset::new(3.0, 6.0)));
    assert!(!rect.contains(Offset::new(5.0, 6.0)));
    assert!(!rect.contains(Offset::new(1.0, 9.0)));
    assert!(!rect.contains(Offset::new(3.0, 9.0)));
    assert!(!rect.contains(Offset::new(5.0, 9.0)));
    assert!(!rect.contains(Offset::new(0.0, 0.0)));
    assert!(!rect.contains(Offset::new(f32::INFINITY, f32::INFINITY)));
    assert!(!rect.contains(Offset::new(f32::NEG_INFINITY, f32::NEG_INFINITY)));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/MutableRectTest.kt, testSet
#[test]
fn set_replaces_every_side() {
    let mut rect = MutableRect::new(0.0, 0.0, 100.0, 100.0);
    rect.set(10.0, 3.0, 20.0, 6.0);
    assert_eq!((rect.left, rect.top, rect.right, rect.bottom), (10.0, 3.0, 20.0, 6.0));
}

// Not one of Compose's tests: the constructors from corners and a center, and `toString`.
#[test]
fn mutable_rects_from_corners_a_center_and_printing() {
    assert_eq!(
        MutableRect::from_corners(Offset::new(1.0, 2.0), Offset::new(3.0, 4.0)).to_rect(),
        Rect::new(1.0, 2.0, 3.0, 4.0)
    );
    assert_eq!(
        MutableRect::from_center(Offset::new(10.0, 10.0), 2.0).to_rect(),
        Rect::new(8.0, 8.0, 12.0, 12.0)
    );
    assert_eq!(MutableRect::new(1.0, 2.0, 3.0, 4.0).to_string(), "MutableRect(1.0, 2.0, 3.0, 4.0)");
}
