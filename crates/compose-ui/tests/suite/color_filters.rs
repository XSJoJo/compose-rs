//! Color matrices, color filters and blend modes, as Compose's `ColorMatrix`, `ColorFilter` and
//! `BlendMode`, and how a display list keeps them.

use compose_ui::platform::*;
use compose_ui::*;

/// 0 to 19, row-major.
fn counting() -> ColorMatrix {
    ColorMatrix::from_values(std::array::from_fn(|index| index as f32))
}

fn assert_near(expected: f32, actual: f32) {
    assert!((expected - actual).abs() <= 1e-6, "expected {expected}, was {actual}");
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testGetOperator, testSetOperator
#[test]
fn a_matrix_reads_and_writes_by_row_and_column() {
    let mut matrix = counting();
    for row in 0..4 {
        for column in 0..5 {
            assert_eq!(matrix.get(row, column), (row * 5 + column) as f32);
            matrix.set(row, column, matrix.get(row, column) + 1.0);
            assert_eq!(matrix.values[row * 5 + column], (row * 5 + column) as f32 + 1.0);
        }
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testColorMatrix, testReset
#[test]
fn a_new_matrix_is_the_identity_and_reset_goes_back_to_it() {
    let identity = ColorMatrix::new();
    for (index, value) in identity.values.iter().enumerate() {
        assert_eq!(*value, if index % 6 == 0 && index < 19 { 1.0 } else { 0.0 }, "{index}");
    }
    let mut matrix = counting();
    assert_eq!(matrix.values, counting().values);
    matrix.reset();
    assert_eq!(matrix, identity);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testSet1, testSet2
#[test]
fn a_matrix_takes_anothers_values() {
    let mut matrix = ColorMatrix::new();
    matrix.set_from(&counting());
    assert_eq!(matrix, counting());
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testSetRotate
#[test]
fn half_turns_around_each_channel_negate_the_other_two() {
    let mut red = ColorMatrix::new();
    red.set_to_rotate_red(180.0);
    for (index, expected) in [(6, -1.0), (12, -1.0), (7, 0.0), (11, 0.0)] {
        assert_near(expected, red.values[index]);
    }
    let mut green = ColorMatrix::new();
    green.set_to_rotate_green(180.0);
    for (index, expected) in [(0, -1.0), (12, -1.0), (2, 0.0), (10, 0.0)] {
        assert_near(expected, green.values[index]);
    }
    let mut blue = ColorMatrix::new();
    blue.set_to_rotate_blue(180.0);
    for (index, expected) in [(0, -1.0), (6, -1.0), (1, 0.0), (5, 0.0)] {
        assert_near(expected, blue.values[index]);
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testSetSaturation
#[test]
fn half_saturation_mixes_each_channel_with_the_luminance() {
    let mut matrix = counting();
    matrix.set_to_saturation(0.5);
    let expected = [
        0.6065,
        0.3575,
        0.036,
        0.0,
        0.0, //
        0.1065,
        0.857_499_96,
        0.036,
        0.0,
        0.0, //
        0.1065,
        0.3575,
        0.536,
        0.0,
        0.0, //
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ];
    assert_eq!(matrix.values, expected);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testSetScale
#[test]
fn a_scale_is_on_the_diagonal_only() {
    let mut matrix = counting();
    matrix.set_to_scale(2.0, 3.0, 4.0, 5.0);
    let values = matrix.values;
    assert_eq!([values[0], values[6], values[12], values[18]], [2.0, 3.0, 4.0, 5.0]);
    for (index, value) in values.iter().enumerate() {
        if index % 6 != 0 {
            assert_eq!(*value, 0.0, "{index}");
        }
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testSetRGB2YUV, testSetYUV2RGB
#[test]
fn rgb_and_yuv_convert_with_the_usual_coefficients() {
    let mut to_yuv = counting();
    to_yuv.convert_rgb_to_yuv();
    assert_eq!(
        to_yuv.values,
        [
            0.299, 0.587, 0.114, 0.0, 0.0, //
            -0.16874, -0.33126, 0.5, 0.0, 0.0, //
            0.5, -0.41869, -0.08131, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ]
    );
    let mut to_rgb = counting();
    to_rgb.convert_yuv_to_rgb();
    assert_eq!(
        to_rgb.values,
        [
            1.0, 0.0, 1.402, 0.0, 0.0, //
            1.0, -0.34414, -0.71414, 0.0, 0.0, //
            1.0, 1.772, 0.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ]
    );
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testTimesAssignPostConcat, testTimesAssignPreConcat
#[test]
fn the_identity_changes_nothing_on_either_side() {
    let mut matrix = counting();
    matrix *= ColorMatrix::new();
    assert_eq!(matrix, counting());
    let mut identity = ColorMatrix::new();
    identity *= counting();
    assert_eq!(identity, counting());
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorMatrixTest.kt, testTimesAssign
#[test]
fn a_product_applies_the_right_matrix_first() {
    let mut matrix = ColorMatrix::from_values([
        0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0,
        1.0, 0.0,
    ]);
    matrix *= ColorMatrix::from_values([1.0; 20]);
    assert_eq!(
        matrix.values,
        [
            6.0, 6.0, 6.0, 6.0, 10.0, 26.0, 26.0, 26.0, 26.0, 35.0, 30.0, 30.0, 30.0, 30.0, 35.0,
            10.0, 10.0, 10.0, 10.0, 10.0,
        ]
    );
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidColorFilterTest.kt, testBlendModeColorFilterEquals, testColorMatrixColorFilterEquals, testLightingColorFilterEquals
#[test]
fn filters_of_the_same_kind_and_values_are_equal() {
    assert_eq!(
        ColorFilter::tint_with(Color::YELLOW, BlendMode::Softlight),
        ColorFilter::BlendMode { color: Color::YELLOW, blend_mode: BlendMode::Softlight }
    );
    assert_ne!(
        ColorFilter::tint_with(Color::YELLOW, BlendMode::Softlight),
        ColorFilter::tint(Color::YELLOW)
    );
    assert_eq!(ColorFilter::color_matrix(counting()), ColorFilter::color_matrix(counting()));
    assert_eq!(
        ColorFilter::lighting(Color::BLUE, Color::YELLOW),
        ColorFilter::lighting(Color::BLUE, Color::YELLOW)
    );
    let ColorFilter::ColorMatrix(copied) = ColorFilter::color_matrix(counting()) else {
        panic!("a matrix filter");
    };
    assert_eq!(copied, counting(), "its matrix, as copyColorMatrix");
}

/// `ColorFilter.tint`'s mode is `SrcIn` by default, and a paint blends `SrcOver`, as Compose's.
#[test]
fn the_defaults_are_composes() {
    assert_eq!(ColorFilter::tint(Color::RED), ColorFilter::tint_with(Color::RED, BlendMode::SrcIn));
    assert_eq!(BlendMode::default(), BlendMode::SrcOver);
    assert_eq!(Paint::fill(Color::RED).blend_mode, BlendMode::SrcOver);
    assert_eq!(Paint::fill(Color::RED).color_filter(), None);
    assert_eq!(BlendMode::Luminosity as u8, 28, "Compose's 29 modes, in its order");
}

/// A filtered or blended paint is kept in the display list with its filter, which a flattened
/// list (and a dump) names; a plain paint's dump is as it was.
#[test]
fn a_frame_keeps_each_recordings_filters_and_dumps_them() {
    let mut list = DisplayList::default();
    let tint = list.push_color_filter(ColorFilter::tint(Color::RED));
    assert_eq!(list.push_color_filter(ColorFilter::tint(Color::RED)), tint, "kept once");
    let lighting = list.push_color_filter(ColorFilter::lighting(Color::WHITE, Color::BLUE));
    assert_ne!(lighting, tint);
    let paint = Paint::fill(Color::BLACK).with_color_filter(lighting);
    list.push(DrawOp::DrawRect(Rect::new(0.0, 0.0, 1.0, 1.0), paint));
    let cleared = Paint::fill(Color::TRANSPARENT).with_blend_mode(BlendMode::Clear);
    list.push(DrawOp::DrawRect(Rect::new(0.0, 0.0, 1.0, 1.0), cleared));
    let flat = list.flatten();
    assert_eq!(flat.color_filter(paint.color_filter()), Some(&list.color_filters[1]));
    assert_eq!(
        flat.dump(),
        "rect 0,0 1x1 #FF000000 lighting #FFFFFFFF #FF0000FF\nrect 0,0 1x1 #00000000 blend Clear\n"
    );
}
