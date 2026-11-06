//! Compose's `MatrixTest.kt` (ui-graphics) and the Compose half of `AndroidMatrixTest.kt` (its
//! device tests): `Matrix`'s identity, reset, pivoted transform, mapping of points and rects,
//! rotations, scale, translation, inversion and their combinations. Android's half converts to
//! `android.graphics.Matrix`, which compose-rs never does: it maps points itself.

use compose_ui::{Matrix, MutableRect, Offset, Rect};

fn assert_near(expected: f32, actual: f32, tolerance: f32) {
    assert!((expected - actual).abs() <= tolerance, "expected {expected}, got {actual}");
}

fn assert_offset(expected: Offset, actual: Offset) {
    assert_near(expected.x, actual.x, 2e-2);
    assert_near(expected.y, actual.y, 2e-2);
}

fn assert_mutable_rect(expected: MutableRect, actual: MutableRect) {
    assert_near(expected.left, actual.left, 3e-2);
    assert_near(expected.top, actual.top, 3e-2);
    assert_near(expected.right, actual.right, 3e-2);
    assert_near(expected.bottom, actual.bottom, 3e-2);
}

fn is_near_identity(matrix: &Matrix) -> bool {
    (0..4).all(|row| {
        (0..4).all(|column| {
            let expected = if row == column { 1.0 } else { 0.0 };
            (matrix[(row, column)] - expected).abs() <= 0.0001
        })
    })
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, identity
#[test]
fn the_identity_has_ones_on_its_diagonal_only() {
    let mut matrix = Matrix::new();
    assert!(matrix.is_identity());
    for row in 0..4 {
        for column in 0..4 {
            let expected = if row == column { 1.0 } else { 0.0 };
            assert_eq!(matrix[(row, column)], expected);
            matrix[(row, column)] = expected + 0.1;
            assert!(!matrix.is_identity());
            matrix[(row, column)] = expected;
        }
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, reset
#[test]
fn reset_makes_the_identity() {
    let mut matrix = Matrix::new();
    matrix.translate(5.0, 0.0, 0.0);
    matrix.reset();
    assert!(matrix.is_identity());
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, resetToPivotedTransform
#[test]
fn a_pivoted_transform_is_its_steps_around_the_pivot() {
    let mut matrix = Matrix::new();
    matrix.translate(5.0, 0.0, 0.0);
    matrix.reset_to_pivoted_transform(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
    assert!(matrix.is_identity());

    matrix.values = [2.0; 16];
    matrix.reset_to_pivoted_transform(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
    assert!(matrix.is_identity());

    let (px, py) = (64.0, 32.0);
    let (tx, ty, tz) = (200.0, 300.0, 1.0);
    let (rx, ry, rz) = (15.0, 25.0, 35.0);
    let (sx, sy, sz) = (1.2, 1.4, 1.1);
    matrix.reset_to_pivoted_transform(px, py, tx, ty, tz, rx, ry, rz, sx, sy, sz);

    let mut expected = Matrix::new();
    expected.translate(-px, -py, 0.0);
    let mut steps = Matrix::new();
    steps.translate(tx, ty, tz);
    steps.rotate_x(rx);
    steps.rotate_y(ry);
    steps.rotate_z(rz);
    steps.scale(sx, sy, sz);
    expected *= steps;
    let mut back = Matrix::new();
    back.translate(px, py, 0.0);
    expected *= back;

    for index in 0..16 {
        assert_near(expected.values[index], matrix.values[index], 1e-4);
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, mapPoint
#[test]
fn rotations_map_points_around_each_axis() {
    let mut matrix = Matrix::new();
    matrix.rotate_z(45.0);
    let z_point = matrix.map(Offset::new(10.0, 0.0));
    assert_near(7.071, z_point.x, 0.02);
    assert_near(7.071, z_point.y, 0.02);

    matrix.reset();
    matrix.rotate_x(45.0);
    let x_point = matrix.map(Offset::new(0.0, 10.0));
    assert_near(0.0, x_point.x, 0.02);
    assert_near(7.071, x_point.y, 0.02);

    matrix.reset();
    matrix.rotate_y(45.0);
    let y_point = matrix.map(Offset::new(10.0, 0.0));
    assert_near(7.071, y_point.x, 0.02);
    assert_near(0.0, y_point.y, 0.02);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, mapPointInfinite
#[test]
fn a_point_mapped_to_infinity_is_the_origin() {
    let mut matrix = Matrix::new();
    matrix[(3, 3)] = 0.0;
    assert_eq!(matrix.map(Offset::new(1.0, 1.0)), Offset::ZERO);
    matrix[(3, 3)] = f32::from_bits(1);
    assert_eq!(matrix.map(Offset::new(1.0, 1.0)), Offset::ZERO);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, mapRect
#[test]
fn a_mapped_rect_bounds_its_mapped_corners() {
    let mut matrix = Matrix::new();
    let square = Rect::new(0.0, 0.0, 10.0, 10.0);
    assert_eq!(matrix.map_rect(square), square);

    matrix.rotate_z(90.0);
    matrix.scale(2.0, 2.0, 1.0);
    let rotated = matrix.map_rect(square);
    assert_near(-20.0, rotated.left, 0.0001);
    assert_near(0.0, rotated.top, 0.0001);
    assert_near(0.0, rotated.right, 0.0001);
    assert_near(20.0, rotated.bottom, 0.0001);

    matrix.reset();
    matrix.rotate_z(45.0);
    let rotated = matrix.map_rect(square);
    let sqrt2_times_10 = 10.0 * 2.0_f32.sqrt();
    assert_near(-sqrt2_times_10 / 2.0, rotated.left, 3e-2);
    assert_near(0.0, rotated.top, 3e-2);
    assert_near(sqrt2_times_10 / 2.0, rotated.right, 3e-2);
    assert_near(sqrt2_times_10, rotated.bottom, 3e-2);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, mapMutableRect
#[test]
fn a_mutable_rect_maps_in_place() {
    let mut matrix = Matrix::new();
    let mut rect = MutableRect::new(0.0, 0.0, 10.0, 10.0);
    matrix.map_mutable_rect(&mut rect);
    assert_mutable_rect(MutableRect::new(0.0, 0.0, 10.0, 10.0), rect);

    matrix.rotate_z(90.0);
    matrix.scale(2.0, 2.0, 1.0);
    matrix.map_mutable_rect(&mut rect);
    assert_mutable_rect(MutableRect::new(-20.0, 0.0, 0.0, 20.0), rect);

    matrix.reset();
    matrix.rotate_z(45.0);
    rect.set(0.0, 0.0, 10.0, 10.0);
    matrix.map_mutable_rect(&mut rect);
    let sqrt2_times_10 = 10.0 * 2.0_f32.sqrt();
    assert_mutable_rect(
        MutableRect::new(-sqrt2_times_10 / 2.0, 0.0, sqrt2_times_10 / 2.0, sqrt2_times_10),
        rect,
    );
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, rotateZ
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, rotate90
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, rotate30
#[test]
fn rotating_about_z_turns_points_clockwise() {
    let mut matrix = Matrix::new();
    assert_offset(Offset::new(0.0, 0.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(10.0, 10.0), matrix.map(Offset::new(10.0, 10.0)));

    matrix.rotate_z(90.0);
    assert_offset(Offset::new(0.0, 0.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(0.0, 10.0), matrix.map(Offset::new(10.0, 0.0)));
    assert_offset(Offset::new(-10.0, 10.0), matrix.map(Offset::new(10.0, 10.0)));
    assert_offset(Offset::new(-100.0, 100.0), matrix.map(Offset::new(100.0, 100.0)));

    let mut thirty = Matrix::new();
    thirty.rotate_z(30.0);
    assert_offset(Offset::new(0.0, 0.0), thirty.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(86.60254, 50.0), thirty.map(Offset::new(100.0, 0.0)));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, rotateX
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, rotateX45
#[test]
fn rotating_about_x_shortens_y() {
    let mut matrix = Matrix::new();
    matrix.rotate_x(90.0);
    assert_offset(Offset::new(0.0, 0.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(10.0, 0.0), matrix.map(Offset::new(10.0, 0.0)));
    assert_offset(Offset::new(0.0, 0.0), matrix.map(Offset::new(0.0, 10.0)));
    assert_offset(Offset::new(10.0, 0.0), matrix.map(Offset::new(10.0, 10.0)));

    let mut forty_five = Matrix::new();
    forty_five.rotate_x(45.0);
    assert_offset(Offset::new(0.0, 0.0), forty_five.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(1.0, 2.0_f32.sqrt() / 2.0), forty_five.map(Offset::new(1.0, 1.0)));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, rotateY
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, rotateY45
#[test]
fn rotating_about_y_shortens_x() {
    let mut matrix = Matrix::new();
    matrix.rotate_y(90.0);
    assert_offset(Offset::new(0.0, 0.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(0.0, 0.0), matrix.map(Offset::new(10.0, 0.0)));
    assert_offset(Offset::new(0.0, 10.0), matrix.map(Offset::new(0.0, 10.0)));
    assert_offset(Offset::new(0.0, 10.0), matrix.map(Offset::new(10.0, 10.0)));

    let mut forty_five = Matrix::new();
    forty_five.rotate_y(45.0);
    assert_offset(Offset::new(0.0, 0.0), forty_five.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(2.0_f32.sqrt() / 2.0, 1.0), forty_five.map(Offset::new(1.0, 1.0)));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, scale
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, scale
#[test]
fn scaling_multiplies_each_axis() {
    let mut matrix = Matrix::new();
    matrix.scale(2.0, 3.0, 4.0);
    assert_offset(Offset::new(0.0, 0.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(0.0, 30.0), matrix.map(Offset::new(0.0, 10.0)));
    assert_offset(Offset::new(20.0, 0.0), matrix.map(Offset::new(10.0, 0.0)));
    assert_offset(Offset::new(20.0, 30.0), matrix.map(Offset::new(10.0, 10.0)));
    assert_offset(Offset::new(200.0, 300.0), matrix.map(Offset::new(100.0, 100.0)));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, translate
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, translateX
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, translateY
#[test]
fn translating_moves_every_point() {
    let mut matrix = Matrix::new();
    matrix.translate(5.0, 20.0, 30.0);
    assert_offset(Offset::new(5.0, 20.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(5.0, 30.0), matrix.map(Offset::new(0.0, 10.0)));
    assert_offset(Offset::new(15.0, 20.0), matrix.map(Offset::new(10.0, 0.0)));
    assert_offset(Offset::new(15.0, 30.0), matrix.map(Offset::new(10.0, 10.0)));

    let mut along_x = Matrix::new();
    along_x.translate(10.0, 0.0, 0.0);
    assert_offset(Offset::new(10.0, 0.0), along_x.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(110.0, 100.0), along_x.map(Offset::new(100.0, 100.0)));

    let mut along_y = Matrix::new();
    along_y.translate(0.0, 10.0, 0.0);
    assert_offset(Offset::new(100.0, 110.0), along_y.map(Offset::new(100.0, 100.0)));
    along_y.translate(0.0, 10.0, 0.0);
    assert_offset(Offset::new(0.0, 20.0), along_y.map(Offset::new(0.0, 0.0)));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, invert
#[test]
fn a_matrix_times_its_inverse_is_the_identity() {
    let mut first = Matrix::new();
    first.invert();
    assert!(first.is_identity());

    first.rotate_z(12.0);
    let mut second = Matrix::new();
    second.set_from(&first);
    second.invert();
    second *= first;
    assert!(is_near_identity(&second));

    first.reset();
    first.translate(1.0, 200.0, 3.0);
    first.rotate_z(30.0);
    first.rotate_x(18.0);
    first.scale(1.2, 1.5, 1.1);
    first.rotate_y(1.0);
    second.set_from(&first);
    second.invert();
    second *= first;
    assert!(is_near_identity(&second));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, combineScaleTranslate
#[test]
fn a_scale_then_a_translation_scales_the_translation() {
    let mut matrix = Matrix::new();
    matrix.scale(2.0, 3.0, 1.0);
    matrix.translate(10.0, 10.0, 0.0);
    assert_offset(Offset::new(20.0, 30.0), matrix.map(Offset::new(0.0, 0.0)));

    matrix.reset();
    matrix.translate(10.0, 10.0, 0.0);
    matrix.scale(2.0, 3.0, 1.0);
    assert_offset(Offset::new(10.0, 10.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(30.0, 40.0), matrix.map(Offset::new(10.0, 10.0)));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/MatrixTest.kt, combineRotateTranslate
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidMatrixTest.kt, rotate90Scale
#[test]
fn a_rotation_then_a_translation_turns_the_translation() {
    let mut matrix = Matrix::new();
    matrix.rotate_z(90.0);
    matrix.translate(10.0, 10.0, 0.0);
    assert_offset(Offset::new(-10.0, 10.0), matrix.map(Offset::new(0.0, 0.0)));

    matrix.reset();
    matrix.translate(10.0, 10.0, 0.0);
    matrix.rotate_z(90.0);
    assert_offset(Offset::new(10.0, 10.0), matrix.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(3.0, 17.0), matrix.map(Offset::new(7.0, 7.0)));

    let mut rotated_scaled = Matrix::new();
    rotated_scaled.rotate_z(90.0);
    rotated_scaled.scale(2.0, 3.0, 1.0);
    assert_offset(Offset::new(0.0, 0.0), rotated_scaled.map(Offset::new(0.0, 0.0)));
    assert_offset(Offset::new(-300.0, 200.0), rotated_scaled.map(Offset::new(100.0, 100.0)));
}

// Not one of Compose's tests: the named indices of `values`, and `toString`.
#[test]
fn the_named_indices_and_printing() {
    let mut matrix = Matrix::new();
    matrix.translate(3.0, 4.0, 0.0);
    assert_eq!(matrix.values[Matrix::TRANSLATE_X], 3.0);
    assert_eq!(matrix.values[Matrix::TRANSLATE_Y], 4.0);
    assert_eq!(matrix.values[Matrix::SCALE_X], 1.0);
    assert_eq!(
        Matrix::new().to_string(),
        "|1.0 0.0 0.0 0.0|\n|0.0 1.0 0.0 0.0|\n|0.0 0.0 1.0 0.0|\n|0.0 0.0 0.0 1.0|"
    );
}
