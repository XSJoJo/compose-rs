//! Color spaces, connectors and adaptation, as Compose's `colorspace` package.

use compose_ui::colorspace::{
    Adaptation, ColorModel, ColorSpace, ColorSpaces, Illuminant, MAX_ID, MIN_ID, RenderIntent, Rgb,
    TransferParameters, WhitePoint,
};

/// sRGB's linear RGB to XYZ (D65), column-major.
const SRGB_TO_XYZ: [f32; 9] =
    [0.412391, 0.212639, 0.019331, 0.357584, 0.715169, 0.119195, 0.180481, 0.072192, 0.950532];
/// Its inverse.
const XYZ_TO_SRGB: [f32; 9] =
    [3.240_97, -0.969244, 0.055630, -1.537383, 1.875968, -0.203977, -0.498611, 0.041555, 1.056971];
/// sRGB's linear RGB to XYZ adapted to D50.
const SRGB_TO_XYZ_D50: [f32; 9] = [
    0.4360747, 0.2225045, 0.0139322, 0.3850649, 0.7168786, 0.0971045, 0.1430804, 0.0606169,
    0.7141733,
];
const SRGB_PRIMARIES_XY: [f32; 6] = [0.640, 0.330, 0.300, 0.600, 0.150, 0.060];
const SRGB_WHITE_XY: WhitePoint = WhitePoint::new(0.3127, 0.3290);
/// sRGB's primaries as XYZ triples (Y = 1).
const SRGB_PRIMARIES_XYZ: [f32; 9] =
    [1.939394, 1.0, 0.090909, 0.5, 1.0, 0.166667, 2.5, 1.0, 13.166667];

fn identity(x: f64) -> f64 {
    x
}

/// Each value within `tolerance` of the expected one (or both the same, NaN included).
#[track_caller]
fn assert_close(expected: &[f32], actual: &[f32], tolerance: f32) {
    for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        assert!(
            expected.total_cmp(actual).is_eq() || (expected - actual).abs() <= tolerance,
            "at {index}: expected {expected}, was {actual} (all: {actual_all:?})",
            actual_all = actual
        );
    }
}

/// Each value further than `tolerance` from the other.
#[track_caller]
fn assert_far(one: &[f32], other: &[f32], tolerance: f32) {
    for (index, (one, other)) in one.iter().zip(other).enumerate() {
        assert!(
            !(one.total_cmp(other).is_eq() || (one - other).abs() < tolerance),
            "at {index}: {one} and {other} are the same"
        );
    }
}

fn test_space(primaries: &[f32], min: f32, max: f32) -> Rgb {
    Rgb::from_primaries("Test", primaries, WhitePoint::new(0.0, 0.0), identity, identity, min, max)
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testNamedColorSpaces, testIDs, testGetColorSpace
#[test]
fn the_named_spaces_are_listed_by_their_ids() {
    let all = ColorSpaces::all();
    assert_eq!(all.len(), 20);
    for (index, space) in all.iter().enumerate() {
        assert!(!space.name().is_empty());
        assert_eq!(space.id(), index as i32);
        assert!((1..=4).contains(&space.component_count()));
        assert!(ColorSpaces::get(index).unwrap().same_instance(space));
    }
    assert_eq!(ColorSpaces::SRGB.id(), 0);
    assert_eq!((MIN_ID, MAX_ID), (-1, 63));
    let by_id: [&ColorSpace; 20] = [
        &ColorSpaces::SRGB,
        &ColorSpaces::LINEAR_SRGB,
        &ColorSpaces::EXTENDED_SRGB,
        &ColorSpaces::LINEAR_EXTENDED_SRGB,
        &ColorSpaces::BT709,
        &ColorSpaces::BT2020,
        &ColorSpaces::DCI_P3,
        &ColorSpaces::DISPLAY_P3,
        &ColorSpaces::NTSC1953,
        &ColorSpaces::SMPTE_C,
        &ColorSpaces::ADOBE_RGB,
        &ColorSpaces::PRO_PHOTO_RGB,
        &ColorSpaces::ACES,
        &ColorSpaces::ACESCG,
        &ColorSpaces::CIE_XYZ,
        &ColorSpaces::CIE_LAB,
        &ColorSpaces::UNSPECIFIED,
        &ColorSpaces::BT2020_HLG,
        &ColorSpaces::BT2020_PQ,
        &ColorSpaces::OKLAB,
    ];
    for (id, space) in by_id.into_iter().enumerate() {
        assert_eq!(ColorSpaces::get(id), Some(space), "{id}");
    }
    assert_eq!(ColorSpaces::get(20), None);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testEmptyName
#[test]
#[should_panic(expected = "the name of a color space")]
fn a_space_without_a_name_is_refused() {
    Rgb::from_primaries("", &[0.0; 6], WhitePoint::new(0.0, 0.0), identity, identity, 0.0, 1.0);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testName
#[test]
fn a_space_keeps_its_name() {
    assert_eq!(test_space(&[0.0; 6], 0.0, 1.0).name(), "Test");
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testPrimariesLength
#[test]
#[should_panic(expected = "6 floats in xyY or 9 floats in XYZ")]
fn seven_primaries_are_refused() {
    test_space(&[0.0; 7], 0.0, 1.0);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testOETF, testEOTF
#[test]
fn a_space_runs_its_own_transfer_functions() {
    let encoding = Rgb::from_primaries(
        "Test",
        &[0.0; 6],
        WhitePoint::new(0.0, 0.0),
        f64::sqrt,
        identity,
        0.0,
        1.0,
    );
    assert!((encoding.oetf(0.25) - 0.5).abs() < 1e-5);
    let decoding = Rgb::from_primaries(
        "Test",
        &[0.0; 6],
        WhitePoint::new(0.0, 0.0),
        identity,
        |x| x * x,
        0.0,
        1.0,
    );
    assert!((decoding.eotf(0.25) - 0.0625).abs() < 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testInvalidRange
#[test]
#[should_panic(expected = "min must be strictly < max")]
fn a_range_upside_down_is_refused() {
    test_space(&[0.0; 6], 2.0, 1.0);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testRanges
#[test]
fn each_kind_of_space_has_its_ranges() {
    let ranges = |space: &ColorSpace| -> Vec<(f32, f32)> {
        (0..3).map(|component| (space.min_value(component), space.max_value(component))).collect()
    };
    let expected: Vec<(f32, f32)> = vec![(0.0, 1.0); 3];
    assert_eq!(ranges(&ColorSpaces::SRGB), expected);
    let lab: Vec<(f32, f32)> = vec![(0.0, 100.0), (-128.0, 128.0), (-128.0, 128.0)];
    assert_eq!(ranges(&ColorSpaces::CIE_LAB), lab);
    let xyz: Vec<(f32, f32)> = vec![(-2.0, 2.0); 3];
    assert_eq!(ranges(&ColorSpaces::CIE_XYZ), xyz);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testMat3x3, testMat3x3Inverse, testMat3x3Primaries, testMat3x3WhitePoint
#[test]
fn a_space_from_its_matrix_finds_its_primaries_and_white_point() {
    let space = Rgb::new("Test", SRGB_TO_XYZ, identity, identity);
    assert_close(&SRGB_TO_XYZ, &space.transform(), 1e-5);
    assert_close(&XYZ_TO_SRGB, &space.inverse_transform(), 1e-5);
    assert_close(&SRGB_PRIMARIES_XY, &space.primaries(), 1e-5);
    let white = space.white_point();
    assert_close(&[SRGB_WHITE_XY.x, SRGB_WHITE_XY.y], &[white.x, white.y], 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testXYZFromPrimaries_xyY
#[test]
fn a_space_from_xy_primaries_computes_its_matrices() {
    let space = Rgb::from_primaries(
        "Test",
        &SRGB_PRIMARIES_XY,
        SRGB_WHITE_XY,
        identity,
        identity,
        0.0,
        1.0,
    );
    assert_close(&SRGB_TO_XYZ, &space.transform(), 1e-5);
    assert_close(&XYZ_TO_SRGB, &space.inverse_transform(), 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testXYZFromPrimaries_XYZ
#[test]
fn a_space_from_xyz_primaries_turns_them_to_xy() {
    let space = Rgb::from_primaries(
        "Test",
        &SRGB_PRIMARIES_XYZ,
        WhitePoint::from_xyz(0.950456, 1.0, 1.089058),
        identity,
        identity,
        0.0,
        1.0,
    );
    // The xy values above have three decimals.
    assert_close(&SRGB_PRIMARIES_XY, &space.primaries(), 1e-3);
    let white = space.white_point();
    assert_close(&[SRGB_WHITE_XY.x, SRGB_WHITE_XY.y], &[white.x, white.y], 1e-3);
    assert_close(&SRGB_TO_XYZ, &space.transform(), 1e-5);
    assert_close(&XYZ_TO_SRGB, &space.inverse_transform(), 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testGetComponentCount
// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/colorspace/ColorModelTest.kt, testRgb, testXyz, testLab, testCmyk
#[test]
fn spaces_and_models_count_their_components() {
    let spaces: [&ColorSpace; 5] = [
        &ColorSpaces::SRGB,
        &ColorSpaces::LINEAR_SRGB,
        &ColorSpaces::EXTENDED_SRGB,
        &ColorSpaces::LINEAR_EXTENDED_SRGB,
        &ColorSpaces::DISPLAY_P3,
    ];
    for space in spaces {
        assert_eq!(space.component_count(), 3, "{space}");
    }
    assert_eq!(ColorSpaces::CIE_LAB.component_count(), 3);
    assert_eq!(ColorSpaces::CIE_XYZ.component_count(), 3);
    let models = [ColorModel::Rgb, ColorModel::Xyz, ColorModel::Lab, ColorModel::Cmyk];
    assert_eq!(models.map(ColorModel::component_count), [3, 3, 3, 4]);
    assert_eq!(models.map(|model| model.to_string()), ["Rgb", "Xyz", "Lab", "Cmyk"]);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testIsSRGB
#[test]
fn only_srgb_is_srgb() {
    for space in ColorSpaces::all() {
        assert_eq!(space.is_srgb(), space.same_instance(&ColorSpaces::SRGB), "{space}");
    }
    let almost = Rgb::new("Almost sRGB", SRGB_TO_XYZ, |x| x.powf(1.0 / 2.2), |x| x.powf(2.2));
    assert!(!almost.is_srgb());
    // The same curve as sRGB's, made anew, is sRGB.
    let again = Rgb::from_primaries_with_transfer_parameters(
        "Again",
        &SRGB_PRIMARIES_XY,
        Illuminant::D65,
        TransferParameters::new(2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045),
    );
    assert!(again.is_srgb());
    // sRGB's curve over another range isn't.
    let srgb = ColorSpaces::SRGB.clone();
    let below = Rgb::from_primaries(
        "Below",
        &SRGB_PRIMARIES_XY,
        Illuminant::D65,
        move |x| srgb.oetf(x),
        |x| ColorSpaces::SRGB.eotf(x),
        -0.5,
        1.0,
    );
    assert!(!below.is_srgb());
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testIsWideGamut
#[test]
fn wide_spaces_say_so() {
    assert!(!ColorSpaces::SRGB.is_wide_gamut());
    assert!(!ColorSpaces::BT709.is_wide_gamut());
    let spaces: [&ColorSpace; 6] = [
        &ColorSpaces::EXTENDED_SRGB,
        &ColorSpaces::DCI_P3,
        &ColorSpaces::BT2020,
        &ColorSpaces::ACES,
        &ColorSpaces::CIE_LAB,
        &ColorSpaces::CIE_XYZ,
    ];
    for space in spaces {
        assert!(space.is_wide_gamut(), "{space}");
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testWhitePoint, testPrimaries, testRGBtoXYZMatrix, testXYZtoRGBMatrix
#[test]
fn srgb_gives_copies_of_its_values() {
    let srgb = &*ColorSpaces::SRGB;
    assert_eq!(srgb.white_point(), Illuminant::D65);
    // Kotlin hands out copies of its arrays; Rust gives arrays by value: changing one changes
    // nothing in the space.
    let mut primaries = srgb.primaries();
    assert_close(&SRGB_PRIMARIES_XY, &primaries, 1e-5);
    primaries.fill(f32::NAN);
    assert_far(&primaries, &srgb.primaries(), 1e-5);
    let mut transform = srgb.transform();
    // From primaries of three decimals.
    assert_close(&SRGB_TO_XYZ, &transform, 1e-3);
    transform.fill(f32::NAN);
    assert_far(&transform, &srgb.transform(), 1e-5);
    let mut inverse = srgb.inverse_transform();
    assert_close(&XYZ_TO_SRGB, &inverse, 1e-3);
    inverse.fill(f32::NAN);
    assert_far(&inverse, &srgb.inverse_transform(), 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testRGBtoXYZ, testXYZtoRGB
#[test]
fn srgb_converts_to_xyz_and_back() {
    let srgb = &*ColorSpaces::SRGB;
    let rgb = [0.75, 0.5, 0.25];
    let xyz = [0.3012, 0.2679, 0.0840];
    let to = srgb.to_xyz(rgb[0], rgb[1], rgb[2]);
    assert_far(&rgb, &to, 1e-5);
    assert_close(&xyz, &to, 1e-3);
    let mut in_place = rgb;
    srgb.to_xyz_in_place(&mut in_place);
    assert_close(&to, &in_place, 1e-5);

    let from = srgb.from_xyz(xyz[0], xyz[1], xyz[2]);
    assert_far(&xyz, &from, 1e-5);
    assert_close(&rgb, &from, 1e-3);
    let mut in_place = xyz;
    srgb.from_xyz_in_place(&mut in_place);
    assert_close(&from, &in_place, 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testConnect, testImplicitSRGBConnector
#[test]
fn a_connector_knows_its_ends_and_intent() {
    let connector = ColorSpaces::SRGB.connect(&ColorSpaces::DCI_P3);
    assert!(connector.source().same_instance(&ColorSpaces::SRGB));
    assert!(connector.destination().same_instance(&ColorSpaces::DCI_P3));
    assert_eq!(connector.render_intent(), RenderIntent::Perceptual);
    // A space to itself: relative, as an identity.
    let itself = ColorSpaces::SRGB.connect(&ColorSpaces::SRGB);
    assert!(itself.source().same_instance(itself.destination()));
    assert_eq!(itself.render_intent(), RenderIntent::Relative);
    // Compose's default destination is sRGB.
    let to_srgb = ColorSpaces::DCI_P3.connect(&ColorSpaces::SRGB);
    assert!(to_srgb.destination().same_instance(&ColorSpaces::SRGB));
    let explicit = ColorSpaces::DCI_P3.connect_with(&ColorSpaces::SRGB, RenderIntent::Perceptual);
    assert_close(&to_srgb.transform(0.6, 0.9, 0.7), &explicit.transform(0.6, 0.9, 0.7), 1e-7);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testConnector
#[test]
fn srgb_and_adobe_rgb_connect_both_ways() {
    let srgb_orange = [1.0, 0.5, 0.0];
    let adobe_orange = [0.8912, 0.4962, 0.1164];
    for (connector, source, expected) in [
        (ColorSpaces::SRGB.connect(&ColorSpaces::ADOBE_RGB), srgb_orange, adobe_orange),
        (ColorSpaces::ADOBE_RGB.connect(&ColorSpaces::SRGB), adobe_orange, srgb_orange),
    ] {
        let result = connector.transform(source[0], source[1], source[2]);
        assert_far(&source, &result, 1e-5);
        assert_close(&expected, &result, 1e-3);
        let mut in_place = source;
        connector.transform_in_place(&mut in_place);
        assert_close(&result, &in_place, 1e-5);
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testAdaptedConnector
#[test]
fn spaces_of_other_white_points_connect_adapted() {
    let connector = ColorSpaces::SRGB.connect(&ColorSpaces::PRO_PHOTO_RGB);
    let red = connector.transform(1.0, 0.0, 0.0);
    assert_far(&[1.0, 0.0, 0.0], &red, 1e-5);
    assert_close(&[0.70226, 0.2757, 0.1036], &red, 1e-4);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testAdaptedConnectorWithRenderIntent
#[test]
fn an_absolute_intent_keeps_the_white_point() {
    let gray = [0.9, 0.9, 0.9];
    let relative = ColorSpaces::DCI_P3
        .connect_with(&ColorSpaces::SRGB, RenderIntent::Relative)
        .transform(gray[0], gray[1], gray[2]);
    assert_far(&gray, &relative, 1e-5);
    assert_close(&[0.8862, 0.8862, 0.8862], &relative, 1e-4);
    let absolute = ColorSpaces::DCI_P3
        .connect_with(&ColorSpaces::SRGB, RenderIntent::Absolute)
        .transform(gray[0], gray[1], gray[2]);
    assert_far(&gray, &absolute, 1e-5);
    assert_far(&relative, &absolute, 1e-5);
    assert_close(&[0.8475, 0.9217, 0.8203], &absolute, 1e-4);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testIdentityConnector, testConnectorTransformIdentity
#[test]
fn a_space_connected_to_itself_changes_nothing() {
    let srgb = ColorSpaces::SRGB.connect(&ColorSpaces::SRGB);
    assert!(srgb.source().same_instance(srgb.destination()));
    assert_eq!(srgb.render_intent(), RenderIntent::Relative);
    let values = [0.11112, 0.22227, 0.444448];
    assert_close(&values, &srgb.transform(values[0], values[1], values[2]), 1e-5);

    let p3 = ColorSpaces::DCI_P3.connect(&ColorSpaces::DCI_P3);
    let red = p3.transform(1.0, 0.0, 0.0);
    assert_close(&[1.0, 0.0, 0.0], &red, 1e-3);
    let mut in_place = [1.0, 0.0, 0.0];
    p3.transform_in_place(&mut in_place);
    assert_close(&red, &in_place, 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testAdaptation
#[test]
fn srgb_adapted_to_d50_takes_bradfords_matrix() {
    let expected = [
        0.43602175,
        0.22247513,
        0.01392813,
        0.38510883,
        0.71690667,
        0.09710153,
        0.14308129,
        0.06061824,
        0.714_158_8,
    ];
    let adapted = ColorSpaces::SRGB.adapt(Illuminant::D50);
    assert_close(&expected, &adapted.as_rgb().unwrap().transform(), 1e-7);
    let bradford = ColorSpaces::SRGB.adapt_with(Illuminant::D50, Adaptation::BRADFORD);
    assert_close(&expected, &bradford.as_rgb().unwrap().transform(), 1e-7);
    // Another transform adapts otherwise; a space at its own white point is itself.
    let von_kries = ColorSpaces::SRGB.adapt_with(Illuminant::D50, Adaptation::VON_KRIES);
    assert_far(&expected[..1], &von_kries.as_rgb().unwrap().transform()[..1], 1e-6);
    assert!(ColorSpaces::SRGB.adapt(Illuminant::D65).same_instance(&ColorSpaces::SRGB));
    assert!(ColorSpaces::CIE_LAB.adapt(Illuminant::D65).same_instance(&ColorSpaces::CIE_LAB));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testLab
#[test]
fn lab_converts_to_srgb_relative_and_absolute() {
    let relative = ColorSpaces::CIE_LAB.connect(&ColorSpaces::SRGB);
    assert_close(&[1.0, 1.0, 1.0], &relative.transform(100.0, 0.0, 0.0), 1e-3);
    assert_close(&[1.0, 0.9925, 0.5762], &relative.transform(100.0, 0.0, 54.0), 1e-3);
    let absolute = ColorSpaces::CIE_LAB.connect_with(&ColorSpaces::SRGB, RenderIntent::Absolute);
    assert_close(&[1.0, 0.9910, 0.8651], &absolute.transform(100.0, 0.0, 0.0), 1e-3);
    assert_close(&[1.0, 0.9853, 0.4652], &absolute.transform(100.0, 0.0, 54.0), 1e-3);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testOkLab
#[test]
fn oklab_converts_to_srgb() {
    let connector = ColorSpaces::OKLAB.connect(&ColorSpaces::SRGB);
    let cases = [
        ([1.0, -0.065, 0.241], [1.0, 1.0, 0.0]),
        ([1.0, -0.125, -0.217], [0.0, 1.0, 1.0]),
        ([0.5, 0.25, 0.0], [0.768, 0.0, 0.366]),
        ([1.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
    ];
    for (oklab, srgb) in cases {
        assert_close(&srgb, &connector.transform(oklab[0], oklab[1], oklab[2]), 1e-3);
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testXYZ
#[test]
fn xyz_is_itself_in_xyz() {
    let xyz = &*ColorSpaces::CIE_XYZ;
    let values = [0.32, 0.43, 0.54];
    assert_close(&values, &xyz.to_xyz(values[0], values[1], values[2]), 1e-7);
    assert_close(&values, &xyz.from_xyz(values[0], values[1], values[2]), 1e-7);
    let srgb = xyz.connect(&ColorSpaces::SRGB).transform(values[0], values[1], values[2]);
    assert_close(&[0.2280, 0.7541, 0.8453], &srgb, 1e-3);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testFromLinear, testToLinear
#[test]
fn srgb_encodes_and_decodes_linear_values() {
    let srgb = &*ColorSpaces::SRGB;
    let encoded = srgb.from_linear(0.0, 0.5, 1.0);
    assert_close(&[0.0, 0.7354, 1.0], &encoded, 1e-3);
    let mut in_place = [0.0, 0.5, 1.0];
    srgb.from_linear_in_place(&mut in_place);
    assert_close(&encoded, &in_place, 1e-5);

    let decoded = srgb.to_linear(0.0, 0.5, 1.0);
    assert_close(&[0.0, 0.2140, 1.0], &decoded, 1e-3);
    let mut in_place = [0.0, 0.5, 1.0];
    srgb.to_linear_in_place(&mut in_place);
    assert_close(&decoded, &in_place, 1e-5);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testTransferParameters
#[test]
fn extended_srgb_shares_srgbs_curve_parameters() {
    let srgb = ColorSpaces::SRGB.transfer_parameters().expect("sRGB's curve");
    assert_eq!(ColorSpaces::EXTENDED_SRGB.transfer_parameters(), Some(srgb));
    assert_eq!((srgb.gamma, srgb.d), (2.4, 0.04045));
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testIdempotentTransferFunctions
#[test]
fn every_named_rgb_space_decodes_and_encodes_back() {
    for space in ColorSpaces::all() {
        let Some(rgb) = space.as_rgb() else { continue };
        let [r, g, b] = rgb.to_linear(0.0, 0.5, 1.0);
        assert_close(&[0.0, 0.5, 1.0], &rgb.from_linear(r, g, b), 1e-3);
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, testMatch
#[test]
fn a_matrix_and_curve_find_their_named_space() {
    for space in ColorSpaces::all() {
        let Some(rgb) = space.as_rgb() else { continue };
        // Extended sRGB shares sRGB's matrix and curve: sRGB is found first.
        if rgb == *ColorSpaces::EXTENDED_SRGB || rgb == *ColorSpaces::LINEAR_EXTENDED_SRGB {
            continue;
        }
        let adapted = rgb.adapt(Illuminant::D50).as_rgb().unwrap();
        let found = ColorSpaces::matching(
            &adapted.transform(),
            &adapted.transfer_parameters().expect("a named curve"),
        );
        assert!(found.is_some_and(|found| found.same_instance(space)), "{space}: {found:?}");
    }
    let srgb = TransferParameters::new(2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045);
    assert!(
        ColorSpaces::matching(&SRGB_TO_XYZ_D50, &srgb)
            .is_some_and(|found| found.same_instance(&ColorSpaces::SRGB))
    );
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ColorSpaceTest.kt, identitySrgbConnector, srgbToOklabConnector, oklabToSrgbConnector
#[test]
fn connectors_between_named_spaces_are_made_once() {
    let same = ColorSpaces::SRGB.connect(&ColorSpaces::SRGB);
    assert_eq!(same, ColorSpaces::SRGB.connect(&ColorSpaces::SRGB));
    assert_eq!(*same.source(), ColorSpaces::SRGB);
    assert_eq!(*same.destination(), ColorSpaces::SRGB);

    let to_oklab = ColorSpaces::SRGB.connect(&ColorSpaces::OKLAB);
    assert_eq!(to_oklab, ColorSpaces::SRGB.connect(&ColorSpaces::OKLAB));
    assert_eq!(*to_oklab.source(), ColorSpaces::SRGB);
    assert_eq!(*to_oklab.destination(), ColorSpaces::OKLAB);
    assert_eq!(to_oklab.render_intent(), RenderIntent::Perceptual);

    let from_oklab = ColorSpaces::OKLAB.connect(&ColorSpaces::SRGB);
    assert_eq!(from_oklab, ColorSpaces::OKLAB.connect(&ColorSpaces::SRGB));
    assert_eq!(*from_oklab.source(), ColorSpaces::OKLAB);
    assert_eq!(*from_oklab.destination(), ColorSpaces::SRGB);
    assert_eq!(from_oklab.render_intent(), RenderIntent::Perceptual);

    // Another intent is another connector; a space of the app's own isn't kept.
    assert_ne!(
        to_oklab,
        ColorSpaces::SRGB.connect_with(&ColorSpaces::OKLAB, RenderIntent::Absolute)
    );
    let own = Rgb::new("Own", SRGB_TO_XYZ, identity, identity);
    assert_ne!(own.connect(&ColorSpaces::SRGB), own.connect(&ColorSpaces::SRGB));
}

// Compose: compose/ui/ui-graphics/src/commonMain/kotlin/androidx/compose/ui/graphics/colorspace/TransferParameters.kt, its init (no Compose test)
#[test]
fn transfer_parameters_refuse_constant_curves() {
    let refused = |gamma: f64, a: f64, c: f64, d: f64| {
        std::panic::catch_unwind(|| TransferParameters::new(gamma, a, 0.0, c, d)).is_err()
    };
    assert!(refused(f64::NAN, 1.0, 0.0, 0.0), "a NaN");
    assert!(refused(2.2, 1.0, 0.0, 1.5), "d past 1");
    assert!(refused(2.2, 0.0, 0.0, 0.0), "a of 0 from 0");
    assert!(refused(2.2, 1.0, -1.0, 0.5), "decreasing");
    assert!(!refused(2.2, 1.0, 0.0, 0.0));
    // PQ and HLG curves aren't checked.
    assert!(!refused(-2.0, -1.0, 0.0, 20.0));
}
