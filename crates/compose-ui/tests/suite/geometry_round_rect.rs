//! Compose's `RoundRectTest.kt` and `CornerRadiusTest.kt` (ui-geometry): a rounded rectangle's
//! containment with scaled radii, constructors, dimensions, inner and bounding rects and ellipse
//! test, and a corner radius's copies. Copies are struct update syntax.

use compose_ui::{CornerRadius, Offset, Rect, RoundRect, lerp};

fn corners(radii: [CornerRadius; 4]) -> RoundRect {
    RoundRect::with_radii(Rect::new(1.0, 1.0, 2.0, 2.0), radii[0], radii[1], radii[2], radii[3])
}

fn assert_contains_as_compose(round_rect: RoundRect) {
    assert!(!round_rect.contains(Offset::new(1.0, 1.0)));
    assert!(!round_rect.contains(Offset::new(1.1, 1.1)));
    assert!(round_rect.contains(Offset::new(1.15, 1.15)));
    assert!(!round_rect.contains(Offset::new(2.0, 1.0)));
    assert!(!round_rect.contains(Offset::new(1.93, 1.07)));
    assert!(!round_rect.contains(Offset::new(1.97, 1.7)));
    assert!(round_rect.contains(Offset::new(1.7, 1.97)));
    assert!(round_rect.contains(Offset::new(1.0, 1.99)));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectContains
#[test]
fn a_round_rect_contains_what_its_corners_leave() {
    assert_contains_as_compose(corners([
        CornerRadius::circular(0.5),
        CornerRadius::circular(0.25),
        CornerRadius::new(0.25, 0.75),
        CornerRadius::ZERO,
    ]));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectContainsLargeRadii
#[test]
fn radii_too_large_scale_down_together() {
    assert_contains_as_compose(corners([
        CornerRadius::circular(5000.0),
        CornerRadius::circular(2500.0),
        CornerRadius::new(2500.0, 7500.0),
        CornerRadius::ZERO,
    ]));
}

fn four_radii(left: f32, top: f32, right: f32, bottom: f32) -> RoundRect {
    RoundRect {
        left,
        top,
        right,
        bottom,
        top_left_corner_radius: CornerRadius::new(5.0, 10.0),
        top_right_corner_radius: CornerRadius::new(15.0, 20.0),
        bottom_right_corner_radius: CornerRadius::new(25.0, 30.0),
        bottom_left_corner_radius: CornerRadius::new(35.0, 40.0),
    }
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectWidthComputation
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectHeightComputation
#[test]
fn a_round_rect_measures_its_sides() {
    assert_eq!(four_radii(10.0, 20.0, 30.0, 40.0).width(), 20.0);
    assert_eq!(four_radii(10.0, 20.0, 30.0, 40.0).height(), 20.0);
}

fn same_radius(radius: CornerRadius) -> RoundRect {
    RoundRect {
        left: 10.0,
        top: 15.0,
        right: 20.0,
        bottom: 25.0,
        top_left_corner_radius: radius,
        top_right_corner_radius: radius,
        bottom_right_corner_radius: radius,
        bottom_left_corner_radius: radius,
    }
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectMatchingFloatConstructor
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectRadiusConstructor
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectWithRectAndRadiusConstructor
#[test]
fn the_one_radius_constructors_round_every_corner_alike() {
    let expected = same_radius(CornerRadius::new(30.0, 30.0));
    assert_eq!(
        RoundRect::from_ltrb(10.0, 15.0, 20.0, 25.0, CornerRadius::new(30.0, 30.0)),
        expected
    );
    assert_eq!(
        RoundRect::from_ltrb(10.0, 15.0, 20.0, 25.0, CornerRadius::circular(30.0)),
        expected
    );
    assert_eq!(
        RoundRect::new(Rect::new(10.0, 15.0, 20.0, 25.0), CornerRadius::circular(30.0)),
        expected
    );
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRoundRectWithRectAndSeparateRadii
#[test]
fn a_rect_and_four_radii_make_a_round_rect() {
    let from_rect = RoundRect::with_radii(
        Rect::new(10.0, 15.0, 20.0, 25.0),
        CornerRadius::new(1.0, 2.0),
        CornerRadius::new(3.0, 4.0),
        CornerRadius::new(5.0, 6.0),
        CornerRadius::new(7.0, 8.0),
    );
    let expected = RoundRect {
        left: 10.0,
        top: 15.0,
        right: 20.0,
        bottom: 25.0,
        top_left_corner_radius: CornerRadius::new(1.0, 2.0),
        top_right_corner_radius: CornerRadius::new(3.0, 4.0),
        bottom_right_corner_radius: CornerRadius::new(5.0, 6.0),
        bottom_left_corner_radius: CornerRadius::new(7.0, 8.0),
    };
    assert_eq!(from_rect, expected);
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testRadiusProperties
#[test]
fn each_corner_keeps_its_radius() {
    let round_rect = RoundRect {
        left: 0.0,
        top: 0.0,
        right: 10.0,
        bottom: 10.0,
        top_left_corner_radius: CornerRadius::new(10.0, 15.0),
        top_right_corner_radius: CornerRadius::new(17.0, 20.0),
        bottom_right_corner_radius: CornerRadius::new(25.0, 30.0),
        bottom_left_corner_radius: CornerRadius::new(35.0, 40.0),
    };
    assert_eq!(round_rect.top_left_corner_radius, CornerRadius::new(10.0, 15.0));
    assert_eq!(round_rect.top_right_corner_radius, CornerRadius::new(17.0, 20.0));
    assert_eq!(round_rect.bottom_right_corner_radius, CornerRadius::new(25.0, 30.0));
    assert_eq!(round_rect.bottom_left_corner_radius, CornerRadius::new(35.0, 40.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testMinDimension
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testMaxDimension
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testCenter
#[test]
fn a_round_rects_dimensions_and_center() {
    let radius = CornerRadius::new(7.0, 8.0);
    assert_eq!(RoundRect::from_ltrb(0.0, 0.0, 100.0, 50.0, radius).min_dimension(), 50.0);
    let radius = CornerRadius::new(5.0, 10.0);
    assert_eq!(RoundRect::from_ltrb(0.0, 0.0, 300.0, 100.0, radius).max_dimension(), 300.0);
    let round_rect = RoundRect::from_ltrb(0.0, 0.0, 200.0, 100.0, CornerRadius::ZERO);
    assert_eq!(round_rect.center(), Offset::new(100.0, 50.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testSafeInnerRect
#[test]
fn the_safe_inner_rect_clears_every_corner() {
    let inset_factor = 0.292_893_2_f32;
    let round_rect = RoundRect {
        left: 0.0,
        top: 0.0,
        right: 100.0,
        bottom: 100.0,
        top_left_corner_radius: CornerRadius::new(0.0, 5.0),
        top_right_corner_radius: CornerRadius::new(5.0, 10.0),
        bottom_right_corner_radius: CornerRadius::new(10.0, 15.0),
        bottom_left_corner_radius: CornerRadius::new(15.0, 20.0),
    };
    assert_eq!(
        round_rect.safe_inner_rect(),
        Rect::new(
            15.0 * inset_factor,
            10.0 * inset_factor,
            100.0 - 10.0 * inset_factor,
            100.0 - 20.0 * inset_factor
        )
    );
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testBoundingRect
#[test]
fn the_bounding_rect_ignores_the_corners() {
    let round_rect = RoundRect::from_ltrb(1.0, 2.0, 3.0, 4.0, CornerRadius::new(15.0, 10.0));
    assert_eq!(round_rect.bounding_rect(), Rect::new(1.0, 2.0, 3.0, 4.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/RoundRectTest.kt, testIsEllipse
#[test]
fn an_ellipse_has_one_radius_reaching_its_middle() {
    let square =
        |size: f32, radius: CornerRadius| RoundRect::from_ltrb(0.0, 0.0, size, size, radius);
    assert!(square(32.0, CornerRadius::new(16.0, 16.0)).is_ellipse());
    assert!(square(32.0, CornerRadius::new(16.0, 24.0)).is_ellipse());
    assert!(!square(33.0, CornerRadius::new(16.0, 16.0)).is_ellipse());
    let round = CornerRadius::new(16.0, 16.0);
    // Compose's four cases: one corner (top right, top left, bottom right, bottom left) off.
    let odd_corners = [
        (1, CornerRadius::new(16.0, 17.0)),
        (0, CornerRadius::new(17.0, 16.0)),
        (2, CornerRadius::new(17.0, 16.0)),
        (3, CornerRadius::new(16.0, 17.0)),
    ];
    for (index, odd) in odd_corners {
        let mut radii = [round; 4];
        radii[index] = odd;
        let round_rect = RoundRect::with_radii(
            Rect::new(0.0, 0.0, 33.0, 33.0),
            radii[0],
            radii[1],
            radii[2],
            radii[3],
        );
        assert!(!round_rect.is_ellipse(), "corner {index}");
    }
    assert!(RoundRect::from_ltrb(0.0, 0.0, 12.0, 32.0, CornerRadius::new(16.0, 16.0)).is_ellipse());
    assert!(RoundRect::from_ltrb(0.0, 0.0, 12.0, 32.0, CornerRadius::new(16.0, 24.0)).is_ellipse());
}

// Not one of Compose's tests: the rest of `RoundRect`'s API (`isRect`, `isCircle`, `isSimple`,
// `isEmpty`, `isFinite`, `translate`, `lerp`).
#[test]
fn a_round_rects_shape_queries_moves_and_interpolation() {
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    assert!(RoundRect::new(rect, CornerRadius::ZERO).is_rect());
    assert!(RoundRect::new(rect, CornerRadius::circular(5.0)).is_circle());
    assert!(RoundRect::new(rect, CornerRadius::circular(2.0)).is_simple());
    assert!(!RoundRect::new(rect, CornerRadius::new(2.0, 3.0)).is_simple());
    assert!(RoundRect::ZERO.is_empty());
    assert!(!RoundRect::from_ltrb(0.0, 0.0, f32::INFINITY, 1.0, CornerRadius::ZERO).is_finite());
    assert_eq!(
        RoundRect::new(rect, CornerRadius::circular(2.0)).translate(Offset::new(1.0, 2.0)),
        RoundRect::new(Rect::new(1.0, 2.0, 11.0, 12.0), CornerRadius::circular(2.0))
    );
    assert_eq!(
        lerp(
            RoundRect::new(rect, CornerRadius::ZERO),
            RoundRect::new(rect, CornerRadius::circular(4.0)),
            0.5
        ),
        RoundRect::new(rect, CornerRadius::circular(2.0))
    );
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/CornerRadiusTest.kt, testRadiusCopy
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/CornerRadiusTest.kt, testRadiusCopyOverwriteX
// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/CornerRadiusTest.kt, testRadiusCopyOverwriteY
#[test]
fn a_radius_copy_changes_only_what_it_names() {
    let radius = CornerRadius::new(100.0, 200.0);
    assert_eq!(CornerRadius { ..radius }, radius);
    let copy = CornerRadius { x: 50.0, ..radius };
    assert_eq!((copy.x, copy.y), (50.0, 200.0));
    let copy = CornerRadius { y: 300.0, ..radius };
    assert_eq!((copy.x, copy.y), (100.0, 300.0));
}

// Compose: compose/ui/ui-geometry/src/commonTest/kotlin/androidx/compose/ui/geometry/CornerRadiusTest.kt, testDestructuringAssignment
#[test]
fn a_radius_destructures() {
    let CornerRadius { x, y } = CornerRadius::new(17.0, 42.0);
    assert_eq!((x, y), (17.0, 42.0));
}

// Not one of Compose's tests: `CornerRadius`'s arithmetic and queries.
#[test]
fn radii_add_scale_and_tell_their_shape() {
    let radius = CornerRadius::new(2.0, 4.0);
    assert_eq!(radius + radius, CornerRadius::new(4.0, 8.0));
    assert_eq!(radius - radius, CornerRadius::ZERO);
    assert_eq!(-radius, CornerRadius::new(-2.0, -4.0));
    assert_eq!(radius * 2.0, CornerRadius::new(4.0, 8.0));
    assert_eq!(radius / 2.0, CornerRadius::new(1.0, 2.0));
    assert!(CornerRadius::ZERO.is_zero());
    assert!(CornerRadius::new(0.0, -0.0).is_zero());
    assert!(CornerRadius::new(0.0, 3.0).is_zero());
    assert!(CornerRadius::new(3.0, -0.0).is_zero());
    assert!(!radius.is_zero());
    assert!(CornerRadius::circular(3.0).is_circular());
    assert!(!radius.is_circular());
    assert_eq!(lerp(CornerRadius::ZERO, radius, 0.5), CornerRadius::new(1.0, 2.0));
    assert_eq!(CornerRadius::circular(1.0).to_string(), "CornerRadius.circular(1.0)");
    assert_eq!(radius.to_string(), "CornerRadius.elliptical(2.0, 4.0)");
}
