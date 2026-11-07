//! `Color`: its components, color spaces, conversions, compositing, lerp, HSV and HSL.

use compose_ui::Color;
use compose_ui::colorspace::ColorSpaces;

/// An orange in sRGB, and about the same in Adobe RGB.
fn srgb_orange() -> Color {
    Color::from_argb(0xFFFF_8000)
}

fn adobe_orange() -> Color {
    Color::new_with(0.8916, 0.4980, 0.1168, 1.0, &ColorSpaces::ADOBE_RGB)
}

/// Half floats in a color aren't exact.
const EPSILON: f32 = 0.0005;

#[track_caller]
fn assert_near(expected: f32, actual: f32, tolerance: f32) {
    assert!((expected - actual).abs() <= tolerance, "expected {expected}, was {actual}");
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, colorSpace, convert
#[test]
fn a_color_converts_to_another_space() {
    assert_eq!(*srgb_orange().color_space(), ColorSpaces::SRGB);
    assert_eq!(*adobe_orange().color_space(), ColorSpaces::ADOBE_RGB);
    let converted = srgb_orange().convert(&ColorSpaces::ADOBE_RGB);
    assert_eq!(converted.color_space(), adobe_orange().color_space());
    let expected = adobe_orange();
    assert_near(expected.red(), converted.red(), EPSILON);
    assert_near(expected.green(), converted.green(), EPSILON);
    assert_near(expected.blue(), converted.blue(), EPSILON);
    assert_near(expected.alpha(), converted.alpha(), EPSILON);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, toArgb_fromSrgb, toArgb_fromAdobeRgb
#[test]
fn either_orange_is_the_same_argb() {
    assert_eq!(srgb_orange().to_argb(), 0xFFFF_8000);
    assert_eq!(adobe_orange().to_argb(), 0xFFFF_8000);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, red, green, blue, alpha
#[test]
fn components_are_read_in_the_colors_space() {
    let (srgb, adobe) = (srgb_orange(), adobe_orange());
    assert_eq!(srgb.red(), 1.0);
    assert_near(0.8916, adobe.red(), EPSILON);
    assert_near(0.5019608, srgb.green(), EPSILON);
    assert_near(0.4980, adobe.green(), EPSILON);
    assert_eq!(srgb.blue(), 0.0);
    assert_near(0.1168, adobe.blue(), EPSILON);
    assert_eq!((srgb.alpha(), adobe.alpha()), (1.0, 1.0));
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, luminance
#[test]
fn luminance_weighs_the_primaries() {
    assert_eq!(Color::BLACK.luminance(), 0.0);
    assert_near(0.0722, Color::BLUE.luminance(), EPSILON);
    assert_near(0.2126, Color::RED.luminance(), EPSILON);
    assert_near(0.7152, Color::GREEN.luminance(), EPSILON);
    assert_eq!(Color::WHITE.luminance(), 1.0);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, luminance
#[test]
#[should_panic(expected = "must be encoded in an RGB color space")]
fn a_lab_color_has_no_luminance() {
    Color::new_with(50.0, 0.0, 0.0, 1.0, &ColorSpaces::CIE_LAB).luminance();
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, testToString
#[test]
fn a_color_prints_its_components_and_space() {
    assert_eq!(srgb_orange().to_string(), "Color(1.0, 0.5019608, 0.0, 1.0, sRGB IEC61966-2.1)");
    // Its debug form stays the ARGB the display-list dumps show.
    assert_eq!(format!("{:?}", srgb_orange()), "#FFFF8000");
    assert_eq!(format!("{:?}", Color::UNSPECIFIED), "Color::UNSPECIFIED");
    let p3 = Color::new_with(1.0, 0.0, 0.0, 1.0, &ColorSpaces::DISPLAY_P3);
    assert_eq!(format!("{p3:?}"), "Color(1.0, 0.0, 0.0, 1.0, Display P3)");
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, lerp
#[test]
fn lerp_runs_through_oklab() {
    let (red, green) = (Color::RED, Color::GREEN);
    let (red_lab, green_lab) =
        (red.convert(&ColorSpaces::OKLAB), green.convert(&ColorSpaces::OKLAB));
    let mix = |start: f32, stop: f32, t: f32| (1.0 - t) * start + t * stop;
    for step in 0..=255 {
        let t = step as f32 / 255.0;
        let expected = Color::new_with(
            mix(red_lab.red(), green_lab.red(), t),
            mix(red_lab.green(), green_lab.green(), t),
            mix(red_lab.blue(), green_lab.blue(), t),
            1.0,
            &ColorSpaces::OKLAB,
        );
        assert_eq!(
            Color::lerp(red, green, t).to_argb(),
            expected.convert(&ColorSpaces::SRGB).to_argb(),
            "at {t}"
        );
    }
    // Toward the same color, transparent: only alpha changes.
    let transparent_red = Color::RED.with_alpha(0.0);
    for step in 0..=255 {
        let t = step as f32 / 255.0;
        assert_eq!(
            Color::lerp(red, transparent_red, t).to_argb(),
            Color::RED.with_alpha(1.0 - t).to_argb(),
            "at {t}"
        );
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, lerp
/// Translucent sRGB colors (lerp's shortcut for two sRGB colors) mix as through the connectors.
#[test]
fn lerp_of_translucent_colors_goes_through_oklab_too() {
    let colors = [0x80FF_0000u32, 0x3300_FF80, 0xFF12_3456, 0x00FF_FFFF, 0xC0AB_CDEF];
    for start in colors.map(Color::from_argb) {
        for stop in colors.map(Color::from_argb) {
            for step in 0..=20 {
                let t = step as f32 / 20.0;
                let (from, to) =
                    (start.convert(&ColorSpaces::OKLAB), stop.convert(&ColorSpaces::OKLAB));
                let mix = |a: f32, b: f32| (1.0 - t) * a + t * b;
                let expected = Color::new_with(
                    mix(from.red(), to.red()),
                    mix(from.green(), to.green()),
                    mix(from.blue(), to.blue()),
                    mix(from.alpha(), to.alpha()),
                    &ColorSpaces::OKLAB,
                )
                .convert(&ColorSpaces::SRGB);
                assert_eq!(Color::lerp(start, stop, t), expected, "{start:?} to {stop:?} at {t}");
            }
        }
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, lerpValues
#[test]
fn lerp_from_red_to_blue_gives_composes_values() {
    // Each of the thousand and one fractions, each channel within one step of Compose's (its
    // test allows as much, for half floats across devices).
    for (step, argb) in LERP_RED_TO_BLUE.iter().copied().enumerate() {
        let color = Color::lerp(Color::RED, Color::BLUE, step as f32 / 1000.0).to_argb();
        for shift in [16, 8, 0] {
            let (want, got) = ((argb >> shift) as u8, (color >> shift) as u8);
            assert!(want.abs_diff(got) < 2, "at {step}/1000: {color:08X}, Compose's {argb:08X}");
        }
    }
}

/// `lerpValues`'s expected colors, `lerp(Red, Blue, n / 1000)` for n from 0 to 1000.
#[rustfmt::skip]
const LERP_RED_TO_BLUE: [u32; 1001] = [    0xFFFF_0000, 0xFFFF_0102, 0xFFFE_0203, 0xFFFE_0305, 0xFFFE_0507, 0xFFFE_0609, 0xFFFE_070A,
    0xFFFD_080C, 0xFFFD_090D, 0xFFFD_0B0F, 0xFFFD_0B10, 0xFFFC_0D11, 0xFFFC_0E13, 0xFFFC_0E13,
    0xFFFC_1015, 0xFFFC_1116, 0xFFFB_1117, 0xFFFB_1218, 0xFFFB_1319, 0xFFFB_1319, 0xFFFA_141A,
    0xFFFA_151B, 0xFFFA_151C, 0xFFFA_161D, 0xFFF9_171D, 0xFFF9_181E, 0xFFF9_181F, 0xFFF9_1920,
    0xFFF9_1920, 0xFFF8_1A21, 0xFFF8_1A22, 0xFFF8_1B22, 0xFFF8_1B23, 0xFFF7_1C24, 0xFFF7_1D24,
    0xFFF7_1D25, 0xFFF7_1D26, 0xFFF7_1E26, 0xFFF6_1E27, 0xFFF6_1F27, 0xFFF6_1F28, 0xFFF6_2028,
    0xFFF5_2029, 0xFFF5_212A, 0xFFF5_212A, 0xFFF5_222B, 0xFFF4_222B, 0xFFF4_222C, 0xFFF4_232C,
    0xFFF4_232D, 0xFFF4_232D, 0xFFF3_242E, 0xFFF3_242E, 0xFFF3_252F, 0xFFF3_252F, 0xFFF2_2530,
    0xFFF2_2630, 0xFFF2_2631, 0xFFF2_2631, 0xFFF2_2732, 0xFFF1_2732, 0xFFF1_2833, 0xFFF1_2833,
    0xFFF1_2833, 0xFFF0_2934, 0xFFF0_2935, 0xFFF0_2935, 0xFFF0_2935, 0xFFEF_2A36, 0xFFEF_2A36,
    0xFFEF_2B37, 0xFFEF_2B37, 0xFFEF_2B37, 0xFFEE_2B38, 0xFFEE_2C38, 0xFFEE_2C39, 0xFFEE_2C39,
    0xFFED_2C3A, 0xFFED_2D3A, 0xFFED_2D3A, 0xFFED_2D3B, 0xFFED_2E3B, 0xFFEC_2E3B, 0xFFEC_2E3C,
    0xFFEC_2F3C, 0xFFEC_2F3D, 0xFFEB_2F3D, 0xFFEB_2F3E, 0xFFEB_2F3E, 0xFFEB_303E, 0xFFEA_303F,
    0xFFEA_303F, 0xFFEA_313F, 0xFFEA_3140, 0xFFEA_3140, 0xFFE9_3141, 0xFFE9_3141, 0xFFE9_3241,
    0xFFE9_3242, 0xFFE8_3242, 0xFFE8_3342, 0xFFE8_3343, 0xFFE8_3343, 0xFFE7_3343, 0xFFE7_3344,
    0xFFE7_3444, 0xFFE7_3444, 0xFFE6_3445, 0xFFE6_3445, 0xFFE6_3546, 0xFFE6_3546, 0xFFE6_3546,
    0xFFE5_3547, 0xFFE5_3547, 0xFFE5_3647, 0xFFE5_3648, 0xFFE4_3648, 0xFFE4_3648, 0xFFE4_3648,
    0xFFE4_3749, 0xFFE4_3749, 0xFFE3_3749, 0xFFE3_374A, 0xFFE3_384A, 0xFFE3_384A, 0xFFE2_384B,
    0xFFE2_384B, 0xFFE2_384B, 0xFFE2_384C, 0xFFE1_394C, 0xFFE1_394C, 0xFFE1_394D, 0xFFE1_394D,
    0xFFE1_394D, 0xFFE0_3A4E, 0xFFE0_3A4E, 0xFFE0_3A4E, 0xFFE0_3A4F, 0xFFDF_3A4F, 0xFFDF_3B4F,
    0xFFDF_3B50, 0xFFDF_3B50, 0xFFDF_3B50, 0xFFDE_3B50, 0xFFDE_3B51, 0xFFDE_3C51, 0xFFDE_3C51,
    0xFFDD_3C52, 0xFFDD_3C52, 0xFFDD_3C52, 0xFFDD_3C53, 0xFFDD_3D53, 0xFFDC_3D53, 0xFFDC_3D53,
    0xFFDC_3D54, 0xFFDC_3D54, 0xFFDB_3D54, 0xFFDB_3D55, 0xFFDB_3E55, 0xFFDB_3E55, 0xFFDA_3E55,
    0xFFDA_3E56, 0xFFDA_3E56, 0xFFDA_3E56, 0xFFDA_3F57, 0xFFD9_3F57, 0xFFD9_3F57, 0xFFD9_3F57,
    0xFFD9_3F58, 0xFFD8_3F58, 0xFFD8_4058, 0xFFD8_4059, 0xFFD8_4059, 0xFFD8_4059, 0xFFD7_4059,
    0xFFD7_405A, 0xFFD7_405A, 0xFFD7_415A, 0xFFD6_415B, 0xFFD6_415B, 0xFFD6_415B, 0xFFD6_415B,
    0xFFD5_415C, 0xFFD5_415C, 0xFFD5_425C, 0xFFD5_425C, 0xFFD5_425D, 0xFFD4_425D, 0xFFD4_425D,
    0xFFD4_425E, 0xFFD4_425E, 0xFFD3_425E, 0xFFD3_435E, 0xFFD3_435F, 0xFFD3_435F, 0xFFD2_435F,
    0xFFD2_435F, 0xFFD2_4360, 0xFFD2_4360, 0xFFD2_4360, 0xFFD1_4460, 0xFFD1_4461, 0xFFD1_4461,
    0xFFD1_4461, 0xFFD0_4461, 0xFFD0_4462, 0xFFD0_4462, 0xFFD0_4462, 0xFFCF_4562, 0xFFCF_4563,
    0xFFCF_4563, 0xFFCF_4563, 0xFFCF_4564, 0xFFCE_4564, 0xFFCE_4564, 0xFFCE_4564, 0xFFCE_4565,
    0xFFCD_4665, 0xFFCD_4665, 0xFFCD_4665, 0xFFCD_4666, 0xFFCC_4666, 0xFFCC_4666, 0xFFCC_4666,
    0xFFCC_4667, 0xFFCC_4767, 0xFFCB_4767, 0xFFCB_4767, 0xFFCB_4768, 0xFFCB_4768, 0xFFCA_4768,
    0xFFCA_4768, 0xFFCA_4769, 0xFFCA_4769, 0xFFCA_4769, 0xFFC9_4769, 0xFFC9_486A, 0xFFC9_486A,
    0xFFC9_486A, 0xFFC8_486A, 0xFFC8_486A, 0xFFC8_486B, 0xFFC8_486B, 0xFFC7_486B, 0xFFC7_486C,
    0xFFC7_496C, 0xFFC7_496C, 0xFFC7_496C, 0xFFC6_496D, 0xFFC6_496D, 0xFFC6_496D, 0xFFC6_496D,
    0xFFC5_496D, 0xFFC5_496E, 0xFFC5_496E, 0xFFC5_496E, 0xFFC5_4A6E, 0xFFC4_4A6F, 0xFFC4_4A6F,
    0xFFC4_4A6F, 0xFFC4_4A6F, 0xFFC3_4A70, 0xFFC3_4A70, 0xFFC3_4A70, 0xFFC3_4A70, 0xFFC2_4A70,
    0xFFC2_4A71, 0xFFC2_4B71, 0xFFC2_4B71, 0xFFC2_4B71, 0xFFC1_4B72, 0xFFC1_4B72, 0xFFC1_4B72,
    0xFFC1_4B72, 0xFFC0_4B73, 0xFFC0_4B73, 0xFFC0_4B73, 0xFFC0_4B73, 0xFFC0_4B74, 0xFFBF_4B74,
    0xFFBF_4C74, 0xFFBF_4C74, 0xFFBF_4C74, 0xFFBE_4C75, 0xFFBE_4C75, 0xFFBE_4C75, 0xFFBE_4C75,
    0xFFBD_4C76, 0xFFBD_4C76, 0xFFBD_4C76, 0xFFBD_4C76, 0xFFBC_4C77, 0xFFBC_4C77, 0xFFBC_4C77,
    0xFFBC_4D77, 0xFFBC_4D78, 0xFFBB_4D78, 0xFFBB_4D78, 0xFFBB_4D78, 0xFFBB_4D78, 0xFFBA_4D79,
    0xFFBA_4D79, 0xFFBA_4D79, 0xFFBA_4D79, 0xFFB9_4D79, 0xFFB9_4D7A, 0xFFB9_4D7A, 0xFFB9_4D7A,
    0xFFB9_4E7A, 0xFFB8_4E7B, 0xFFB8_4E7B, 0xFFB8_4E7B, 0xFFB8_4E7B, 0xFFB7_4E7B, 0xFFB7_4E7C,
    0xFFB7_4E7C, 0xFFB7_4E7C, 0xFFB7_4E7C, 0xFFB6_4E7D, 0xFFB6_4E7D, 0xFFB6_4E7D, 0xFFB6_4E7D,
    0xFFB5_4E7E, 0xFFB5_4F7E, 0xFFB5_4E7E, 0xFFB5_4F7E, 0xFFB4_4F7E, 0xFFB4_4F7F, 0xFFB4_4F7F,
    0xFFB4_4F7F, 0xFFB4_4F7F, 0xFFB3_4F80, 0xFFB3_4F80, 0xFFB3_4F80, 0xFFB3_4F80, 0xFFB2_4F80,
    0xFFB2_4F81, 0xFFB2_4F81, 0xFFB2_4F81, 0xFFB2_4F81, 0xFFB1_4F81, 0xFFB1_4F82, 0xFFB1_5082,
    0xFFB1_5082, 0xFFB0_5082, 0xFFB0_5083, 0xFFB0_5083, 0xFFB0_5083, 0xFFB0_5083, 0xFFAF_5083,
    0xFFAF_5084, 0xFFAF_5084, 0xFFAE_5084, 0xFFAE_5084, 0xFFAE_5084, 0xFFAE_5085, 0xFFAE_5085,
    0xFFAD_5085, 0xFFAD_5085, 0xFFAD_5086, 0xFFAD_5086, 0xFFAC_5086, 0xFFAC_5086, 0xFFAC_5086,
    0xFFAC_5187, 0xFFAB_5087, 0xFFAB_5187, 0xFFAB_5187, 0xFFAB_5187, 0xFFAB_5188, 0xFFAA_5188,
    0xFFAA_5188, 0xFFAA_5188, 0xFFAA_5189, 0xFFA9_5189, 0xFFA9_5189, 0xFFA9_5189, 0xFFA9_5189,
    0xFFA9_518A, 0xFFA8_518A, 0xFFA8_518A, 0xFFA8_518A, 0xFFA8_518A, 0xFFA7_518B, 0xFFA7_518B,
    0xFFA7_518B, 0xFFA7_518B, 0xFFA6_518B, 0xFFA6_518C, 0xFFA6_518C, 0xFFA6_518C, 0xFFA6_528C,
    0xFFA5_528D, 0xFFA5_528D, 0xFFA5_528D, 0xFFA5_528D, 0xFFA4_528D, 0xFFA4_528E, 0xFFA4_528E,
    0xFFA4_528E, 0xFFA4_528E, 0xFFA3_528E, 0xFFA3_528F, 0xFFA3_528F, 0xFFA2_528F, 0xFFA2_528F,
    0xFFA2_528F, 0xFFA2_5290, 0xFFA2_5290, 0xFFA1_5290, 0xFFA1_5290, 0xFFA1_5290, 0xFFA1_5291,
    0xFFA0_5291, 0xFFA0_5291, 0xFFA0_5291, 0xFFA0_5291, 0xFFA0_5292, 0xFF9F_5292, 0xFF9F_5292,
    0xFF9F_5292, 0xFF9F_5292, 0xFF9E_5293, 0xFF9E_5293, 0xFF9E_5293, 0xFF9E_5293, 0xFF9D_5293,
    0xFF9D_5294, 0xFF9D_5394, 0xFF9D_5294, 0xFF9D_5394, 0xFF9C_5395, 0xFF9C_5395, 0xFF9C_5395,
    0xFF9C_5395, 0xFF9B_5395, 0xFF9B_5396, 0xFF9B_5396, 0xFF9B_5396, 0xFF9B_5396, 0xFF9A_5396,
    0xFF9A_5397, 0xFF9A_5397, 0xFF9A_5397, 0xFF99_5397, 0xFF99_5397, 0xFF99_5398, 0xFF99_5398,
    0xFF98_5398, 0xFF98_5398, 0xFF98_5398, 0xFF98_5398, 0xFF97_5399, 0xFF97_5399, 0xFF97_5399,
    0xFF97_5399, 0xFF97_539A, 0xFF96_539A, 0xFF96_539A, 0xFF96_539A, 0xFF96_539A, 0xFF95_539B,
    0xFF95_539B, 0xFF95_539B, 0xFF95_539B, 0xFF94_539B, 0xFF94_539C, 0xFF94_539C, 0xFF94_539C,
    0xFF94_539C, 0xFF93_539C, 0xFF93_539D, 0xFF93_539D, 0xFF93_539D, 0xFF92_539D, 0xFF92_539D,
    0xFF92_539D, 0xFF92_539E, 0xFF92_539E, 0xFF91_539E, 0xFF91_539E, 0xFF91_539F, 0xFF90_539F,
    0xFF90_539F, 0xFF90_539F, 0xFF90_539F, 0xFF90_53A0, 0xFF8F_53A0, 0xFF8F_53A0, 0xFF8F_53A0,
    0xFF8F_53A0, 0xFF8E_53A1, 0xFF8E_53A1, 0xFF8E_53A1, 0xFF8E_53A1, 0xFF8E_53A1, 0xFF8D_53A1,
    0xFF8D_53A2, 0xFF8D_53A2, 0xFF8D_53A2, 0xFF8C_53A2, 0xFF8C_53A2, 0xFF8C_53A3, 0xFF8C_53A3,
    0xFF8B_53A3, 0xFF8B_53A3, 0xFF8B_53A4, 0xFF8B_53A4, 0xFF8B_53A4, 0xFF8A_53A4, 0xFF8A_53A4,
    0xFF8A_53A4, 0xFF8A_53A5, 0xFF89_53A5, 0xFF89_53A5, 0xFF89_53A5, 0xFF89_53A5, 0xFF88_53A6,
    0xFF88_53A6, 0xFF88_53A6, 0xFF88_53A6, 0xFF87_53A6, 0xFF87_53A7, 0xFF87_53A7, 0xFF87_53A7,
    0xFF87_53A7, 0xFF86_53A7, 0xFF86_53A8, 0xFF86_53A8, 0xFF86_53A8, 0xFF85_53A8, 0xFF85_53A8,
    0xFF85_53A8, 0xFF85_53A9, 0xFF84_53A9, 0xFF84_53A9, 0xFF84_53A9, 0xFF84_53A9, 0xFF83_53AA,
    0xFF83_53AA, 0xFF83_53AA, 0xFF83_53AA, 0xFF83_53AB, 0xFF82_52AB, 0xFF82_53AB, 0xFF82_53AB,
    0xFF82_52AB, 0xFF81_52AB, 0xFF81_52AC, 0xFF81_52AC, 0xFF81_52AC, 0xFF80_52AC, 0xFF80_52AC,
    0xFF80_52AD, 0xFF80_52AD, 0xFF80_52AD, 0xFF7F_52AD, 0xFF7F_52AD, 0xFF7F_52AE, 0xFF7F_52AE,
    0xFF7E_52AE, 0xFF7E_52AE, 0xFF7E_52AE, 0xFF7E_52AE, 0xFF7D_52AF, 0xFF7D_52AF, 0xFF7D_52AF,
    0xFF7D_52AF, 0xFF7C_52AF, 0xFF7C_52B0, 0xFF7C_52B0, 0xFF7C_52B0, 0xFF7C_52B0, 0xFF7B_52B0,
    0xFF7B_52B1, 0xFF7B_52B1, 0xFF7B_52B1, 0xFF7A_52B1, 0xFF7A_52B1, 0xFF7A_52B2, 0xFF7A_52B2,
    0xFF79_52B2, 0xFF79_52B2, 0xFF79_52B2, 0xFF79_51B2, 0xFF78_51B3, 0xFF78_51B3, 0xFF78_51B3,
    0xFF78_51B3, 0xFF78_51B4, 0xFF77_51B4, 0xFF77_51B4, 0xFF77_51B4, 0xFF77_51B4, 0xFF76_51B4,
    0xFF76_51B5, 0xFF76_51B5, 0xFF76_51B5, 0xFF75_51B5, 0xFF75_51B5, 0xFF75_51B6, 0xFF75_51B6,
    0xFF74_51B6, 0xFF74_51B6, 0xFF74_51B6, 0xFF74_51B7, 0xFF74_51B7, 0xFF73_51B7, 0xFF73_51B7,
    0xFF73_51B7, 0xFF73_51B7, 0xFF72_51B8, 0xFF72_50B8, 0xFF72_50B8, 0xFF72_50B8, 0xFF71_50B8,
    0xFF71_50B9, 0xFF71_50B9, 0xFF71_50B9, 0xFF70_50B9, 0xFF70_50B9, 0xFF70_50B9, 0xFF70_50BA,
    0xFF6F_50BA, 0xFF6F_50BA, 0xFF6F_50BA, 0xFF6F_50BA, 0xFF6F_50BB, 0xFF6E_50BB, 0xFF6E_50BB,
    0xFF6E_50BB, 0xFF6E_50BB, 0xFF6D_50BC, 0xFF6D_50BC, 0xFF6D_4FBC, 0xFF6D_4FBC, 0xFF6C_4FBC,
    0xFF6C_4FBC, 0xFF6C_4FBD, 0xFF6C_4FBD, 0xFF6B_4FBD, 0xFF6B_4FBD, 0xFF6B_4FBE, 0xFF6B_4FBE,
    0xFF6A_4FBE, 0xFF6A_4FBE, 0xFF6A_4FBE, 0xFF6A_4FBE, 0xFF69_4FBF, 0xFF69_4FBF, 0xFF69_4FBF,
    0xFF69_4FBF, 0xFF69_4FBF, 0xFF68_4FC0, 0xFF68_4EC0, 0xFF68_4EC0, 0xFF68_4EC0, 0xFF67_4EC0,
    0xFF67_4EC1, 0xFF67_4EC1, 0xFF67_4EC1, 0xFF66_4EC1, 0xFF66_4EC1, 0xFF66_4EC1, 0xFF66_4EC2,
    0xFF65_4EC2, 0xFF65_4EC2, 0xFF65_4EC2, 0xFF65_4EC2, 0xFF65_4EC3, 0xFF64_4DC3, 0xFF64_4DC3,
    0xFF64_4DC3, 0xFF63_4DC3, 0xFF63_4DC3, 0xFF63_4DC4, 0xFF63_4DC4, 0xFF63_4DC4, 0xFF62_4DC4,
    0xFF62_4DC4, 0xFF62_4DC5, 0xFF62_4DC5, 0xFF61_4DC5, 0xFF61_4DC5, 0xFF61_4CC5, 0xFF61_4CC6,
    0xFF60_4CC6, 0xFF60_4CC6, 0xFF60_4CC6, 0xFF60_4CC6, 0xFF5F_4CC6, 0xFF5F_4CC7, 0xFF5F_4CC7,
    0xFF5F_4CC7, 0xFF5E_4CC7, 0xFF5E_4CC7, 0xFF5E_4CC8, 0xFF5E_4CC8, 0xFF5D_4CC8, 0xFF5D_4BC8,
    0xFF5D_4BC8, 0xFF5D_4BC8, 0xFF5C_4BC9, 0xFF5C_4BC9, 0xFF5C_4BC9, 0xFF5C_4BC9, 0xFF5B_4BC9,
    0xFF5B_4BCA, 0xFF5B_4BCA, 0xFF5B_4BCA, 0xFF5B_4BCA, 0xFF5A_4BCA, 0xFF5A_4ACA, 0xFF5A_4ACB,
    0xFF5A_4ACB, 0xFF59_4ACB, 0xFF59_4ACB, 0xFF59_4ACB, 0xFF59_4ACC, 0xFF58_4ACC, 0xFF58_4ACC,
    0xFF58_4ACC, 0xFF58_4ACC, 0xFF57_4ACC, 0xFF57_49CD, 0xFF57_49CD, 0xFF57_49CD, 0xFF56_49CD,
    0xFF56_49CD, 0xFF56_49CE, 0xFF56_49CE, 0xFF55_49CE, 0xFF55_49CE, 0xFF55_49CE, 0xFF55_49CF,
    0xFF54_49CF, 0xFF54_48CF, 0xFF54_48CF, 0xFF54_48CF, 0xFF53_48CF, 0xFF53_48D0, 0xFF53_48D0,
    0xFF53_48D0, 0xFF52_48D0, 0xFF52_48D0, 0xFF52_48D1, 0xFF52_47D1, 0xFF51_47D1, 0xFF51_47D1,
    0xFF51_47D1, 0xFF51_47D1, 0xFF50_47D2, 0xFF50_47D2, 0xFF50_47D2, 0xFF50_47D2, 0xFF4F_47D2,
    0xFF4F_46D3, 0xFF4F_46D3, 0xFF4F_46D3, 0xFF4E_46D3, 0xFF4E_46D3, 0xFF4E_46D4, 0xFF4E_46D4,
    0xFF4D_46D4, 0xFF4D_46D4, 0xFF4D_46D4, 0xFF4D_45D4, 0xFF4C_45D5, 0xFF4C_45D5, 0xFF4C_45D5,
    0xFF4C_45D5, 0xFF4B_45D5, 0xFF4B_45D5, 0xFF4B_45D6, 0xFF4B_45D6, 0xFF4A_44D6, 0xFF4A_44D6,
    0xFF4A_44D6, 0xFF4A_44D7, 0xFF49_44D7, 0xFF49_44D7, 0xFF49_44D7, 0xFF49_44D7, 0xFF48_43D8,
    0xFF48_43D8, 0xFF48_43D8, 0xFF48_43D8, 0xFF47_43D8, 0xFF47_43D9, 0xFF47_43D9, 0xFF47_43D9,
    0xFF46_43D9, 0xFF46_42D9, 0xFF46_42D9, 0xFF46_42DA, 0xFF45_42DA, 0xFF45_42DA, 0xFF45_42DA,
    0xFF45_42DA, 0xFF44_42DA, 0xFF44_41DB, 0xFF44_41DB, 0xFF43_41DB, 0xFF43_41DB, 0xFF43_41DB,
    0xFF43_41DC, 0xFF42_41DC, 0xFF42_41DC, 0xFF42_40DC, 0xFF42_40DC, 0xFF41_40DC, 0xFF41_40DD,
    0xFF41_40DD, 0xFF41_40DD, 0xFF40_40DD, 0xFF40_3FDD, 0xFF40_3FDE, 0xFF40_3FDE, 0xFF3F_3FDE,
    0xFF3F_3FDE, 0xFF3F_3FDE, 0xFF3F_3FDF, 0xFF3E_3EDF, 0xFF3E_3EDF, 0xFF3E_3EDF, 0xFF3D_3EDF,
    0xFF3D_3EDF, 0xFF3D_3EE0, 0xFF3D_3EE0, 0xFF3C_3DE0, 0xFF3C_3DE0, 0xFF3C_3DE0, 0xFF3C_3DE0,
    0xFF3B_3DE1, 0xFF3B_3DE1, 0xFF3B_3DE1, 0xFF3B_3CE1, 0xFF3A_3CE1, 0xFF3A_3CE2, 0xFF3A_3CE2,
    0xFF39_3CE2, 0xFF39_3CE2, 0xFF39_3BE2, 0xFF39_3BE3, 0xFF38_3BE3, 0xFF38_3BE3, 0xFF38_3BE3,
    0xFF38_3BE3, 0xFF37_3AE3, 0xFF37_3AE4, 0xFF37_3AE4, 0xFF36_3AE4, 0xFF36_3AE4, 0xFF36_3AE4,
    0xFF36_39E4, 0xFF35_39E5, 0xFF35_39E5, 0xFF35_39E5, 0xFF35_39E5, 0xFF34_39E5, 0xFF34_38E6,
    0xFF34_38E6, 0xFF33_38E6, 0xFF33_38E6, 0xFF33_38E6, 0xFF33_37E6, 0xFF32_37E7, 0xFF32_37E7,
    0xFF32_37E7, 0xFF31_37E7, 0xFF31_37E7, 0xFF31_36E8, 0xFF31_36E8, 0xFF30_36E8, 0xFF30_36E8,
    0xFF30_36E8, 0xFF30_35E8, 0xFF2F_35E9, 0xFF2F_35E9, 0xFF2F_35E9, 0xFF2E_35E9, 0xFF2E_34E9,
    0xFF2E_34EA, 0xFF2E_34EA, 0xFF2D_34EA, 0xFF2D_34EA, 0xFF2D_33EA, 0xFF2C_33EB, 0xFF2C_33EB,
    0xFF2C_33EB, 0xFF2B_32EB, 0xFF2B_32EB, 0xFF2B_32EB, 0xFF2B_32EC, 0xFF2A_32EC, 0xFF2A_31EC,
    0xFF2A_31EC, 0xFF29_31EC, 0xFF29_31EC, 0xFF29_31ED, 0xFF29_30ED, 0xFF28_30ED, 0xFF28_30ED,
    0xFF28_30ED, 0xFF27_2FEE, 0xFF27_2FEE, 0xFF27_2FEE, 0xFF26_2FEE, 0xFF26_2EEE, 0xFF26_2EEF,
    0xFF25_2EEF, 0xFF25_2DEF, 0xFF25_2DEF, 0xFF25_2DEF, 0xFF24_2DEF, 0xFF24_2CF0, 0xFF24_2CF0,
    0xFF23_2CF0, 0xFF23_2CF0, 0xFF23_2BF0, 0xFF22_2BF1, 0xFF22_2BF1, 0xFF22_2BF1, 0xFF21_2AF1,
    0xFF21_2AF1, 0xFF21_2AF1, 0xFF20_2AF1, 0xFF20_29F2, 0xFF20_29F2, 0xFF1F_29F2, 0xFF1F_28F2,
    0xFF1F_28F2, 0xFF1E_28F3, 0xFF1E_27F3, 0xFF1E_27F3, 0xFF1D_27F3, 0xFF1D_27F3, 0xFF1D_26F3,
    0xFF1C_26F4, 0xFF1C_25F4, 0xFF1C_25F4, 0xFF1B_25F4, 0xFF1B_25F4, 0xFF1B_24F5, 0xFF1A_24F5,
    0xFF1A_24F5, 0xFF19_23F5, 0xFF19_23F5, 0xFF19_22F6, 0xFF18_22F6, 0xFF18_22F6, 0xFF18_21F6,
    0xFF17_21F6, 0xFF17_20F6, 0xFF16_20F7, 0xFF16_20F7, 0xFF16_1FF7, 0xFF15_1FF7, 0xFF15_1EF7,
    0xFF14_1EF8, 0xFF14_1EF8, 0xFF14_1DF8, 0xFF13_1DF8, 0xFF13_1CF8, 0xFF12_1CF8, 0xFF12_1BF9,
    0xFF11_1BF9, 0xFF11_1AF9, 0xFF11_1AF9, 0xFF10_19F9, 0xFF10_19FA, 0xFF0F_18FA, 0xFF0F_18FA,
    0xFF0E_17FA, 0xFF0E_17FA, 0xFF0D_16FA, 0xFF0D_15FB, 0xFF0C_15FB, 0xFF0C_14FB, 0xFF0B_14FB,
    0xFF0B_13FB, 0xFF0A_12FC, 0xFF0A_11FC, 0xFF09_11FC, 0xFF08_10FC, 0xFF08_0FFC, 0xFF07_0EFC,
    0xFF07_0DFD, 0xFF06_0DFD, 0xFF06_0CFD, 0xFF05_0BFD, 0xFF05_09FD, 0xFF04_09FE, 0xFF03_08FE,
    0xFF03_07FE, 0xFF02_06FE, 0xFF02_04FE, 0xFF01_03FE, 0xFF01_02FE, 0xFF00_01FF, 0xFF00_00FF,
];

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, compositeColorsSameColorSpace, compositeColorsLowAlpha
#[test]
fn colors_composite_in_srgb() {
    let result = Color::from_argb(0x7F00_7F00).composite_over(Color::from_argb(0x7F7F_0000));
    assert_near(0.16, result.red(), 0.01);
    assert_near(0.33, result.green(), 0.01);
    assert_near(0.0, result.blue(), 0.01);
    assert_near(0.75, result.alpha(), 0.01);

    let faint = Color::new(0.0, 1.0, 0.0, 0.01).composite_over(Color::new(1.0, 0.0, 0.0, 0.01));
    assert_near(0.5, faint.red(), 0.01);
    assert_near(0.5, faint.green(), 0.01);
    assert_near(0.0, faint.blue(), 0.01);
    assert_near(0.01, faint.alpha(), 0.02);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, compositeColorsDifferentColorSpace
#[test]
fn a_color_composites_in_the_backgrounds_space() {
    let background = Color::new_with(0.5, 0.0, 0.0, 0.5, &ColorSpaces::DISPLAY_P3);
    let result = Color::from_argb(0x7F00_7F00).composite_over(background);
    assert_eq!(*result.color_space(), ColorSpaces::DISPLAY_P3);
    assert_near(0.31, result.red(), 0.01);
    assert_near(0.33, result.green(), 0.01);
    assert_near(0.09, result.blue(), 0.01);
    assert_near(0.75, result.alpha(), 0.01);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, compositeColorsZeroAlpha
#[test]
fn two_transparent_colors_composite_to_transparent_black() {
    let result = Color::from_argb(0x0000_7F00).composite_over(Color::from_argb(0x007F_0000));
    assert_eq!(result, Color::new(0.0, 0.0, 0.0, 0.0));
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, srgbOutOfBounds_highRed, srgbOutOfBounds_lowRed, srgbOutOfBounds_highGreen, srgbOutOfBounds_lowGreen, srgbOutOfBounds_highBlue, srgbOutOfBounds_lowBlue, srgbOutOfBounds_highAlpha, srgbOutOfBounds_lowAlpha
#[test]
fn srgb_components_are_kept_in_range() {
    assert_eq!(Color::new(2.0, 0.0, 0.0, 1.0).red(), 1.0);
    assert_eq!(Color::new(-1.0, 0.0, 0.0, 1.0).red(), 0.0);
    assert_eq!(Color::new(0.0, 2.0, 0.0, 1.0).green(), 1.0);
    assert_eq!(Color::new(0.0, -1.0, 0.0, 1.0).green(), 0.0);
    assert_eq!(Color::new(0.0, 0.0, 2.0, 1.0).blue(), 1.0);
    assert_eq!(Color::new(0.0, 0.0, -1.0, 1.0).blue(), 0.0);
    assert_eq!(Color::new(0.0, 0.0, 0.0, 2.0).alpha(), 1.0);
    assert_eq!(Color::new(0.0, 0.0, 0.0, -1.0).alpha(), 0.0);
    // Another space keeps them in its own range.
    let extended = Color::new_with(3.0, -1.0, 0.5, 1.0, &ColorSpaces::EXTENDED_SRGB);
    assert_near(2.399, extended.red(), 0.002);
    assert_near(-0.799, extended.green(), 0.002);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, noneColor, unspecifiedConstantValue
#[test]
fn unspecified_is_its_own_transparent_color() {
    assert!(Color::UNSPECIFIED.is_unspecified() && !Color::UNSPECIFIED.is_specified());
    assert!(!Color::RED.is_unspecified() && Color::RED.is_specified());
    assert_eq!(Color::TRANSPARENT.to_argb(), Color::UNSPECIFIED.to_argb());
    assert_ne!(Color::TRANSPARENT, Color::UNSPECIFIED);
    assert_eq!(Color::UNSPECIFIED.value(), 0x10);
    assert_eq!(Color::UNSPECIFIED.take_or_else(|| Color::RED), Color::RED);
    assert_eq!(Color::BLUE.take_or_else(|| Color::RED), Color::BLUE);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, testDestructuring, testDestructuringSubset, testDestructuringMiddleSubset
#[test]
fn a_color_gives_its_components_and_space_at_once() {
    let (red, green, blue, alpha, space) =
        Color::new_with(0.15, 0.2, 0.3, 0.5, &ColorSpaces::DISPLAY_P3).components();
    assert_near(0.15, red, EPSILON);
    assert_near(0.2, green, EPSILON);
    assert_near(0.3, blue, EPSILON);
    assert_near(0.5, alpha, EPSILON);
    assert_eq!(*space, ColorSpaces::DISPLAY_P3);
    let (red, green, blue, alpha, _) =
        Color::new_with(0.2, 0.3, 0.4, 0.6, &ColorSpaces::ACES).components();
    assert_near(0.2, red, EPSILON);
    assert_near(0.3, green, EPSILON);
    assert_near(0.4, blue, EPSILON);
    assert_near(0.6, alpha, EPSILON);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, testHsvInSrgb
#[test]
fn hsv_in_srgb_makes_the_named_colors() {
    assert_eq!(Color::hsv_with(0.0, 0.0, 0.0, 0.0, &ColorSpaces::SRGB), Color::TRANSPARENT);
    for hue in [0.0, 120.0] {
        assert_eq!(Color::hsv(hue, 0.0, 0.0), Color::BLACK);
    }
    assert_eq!(Color::hsv(120.0, 1.0, 0.0), Color::BLACK);
    for hue in [0.0, 120.0, 240.0] {
        assert_eq!(Color::hsv(hue, 0.0, 1.0), Color::WHITE);
        assert_eq!(Color::hsv(hue, 0.0, 0.5), Color::from_argb(0xFF80_8080));
    }
    let hues = [
        (0.0, Color::RED),
        (60.0, Color::YELLOW),
        (120.0, Color::GREEN),
        (180.0, Color::CYAN),
        (240.0, Color::BLUE),
        (300.0, Color::MAGENTA),
        (360.0, Color::RED),
    ];
    for (hue, color) in hues {
        assert_eq!(Color::hsv(hue, 1.0, 1.0), color, "{hue}");
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, testHsvInLinearSrgb
#[test]
fn hsv_in_linear_srgb_stays_in_it() {
    let linear = &*ColorSpaces::LINEAR_SRGB;
    let hsv = |hue: f32, saturation: f32, value: f32| {
        Color::hsv_with(hue, saturation, value, 1.0, linear)
    };
    for (hue, saturation) in [(0.0, 0.0), (120.0, 0.0), (120.0, 1.0)] {
        assert_eq!(hsv(hue, saturation, 0.0).convert(&ColorSpaces::SRGB), Color::BLACK);
    }
    for hue in [0.0, 120.0, 240.0] {
        assert_eq!(hsv(hue, 0.0, 1.0).convert(&ColorSpaces::SRGB), Color::WHITE);
        assert_eq!(hsv(hue, 0.0, 0.5), Color::new_with(0.5, 0.5, 0.5, 1.0, linear));
    }
    let hues = [
        (0.0, [1.0, 0.0, 0.0]),
        (60.0, [1.0, 1.0, 0.0]),
        (120.0, [0.0, 1.0, 0.0]),
        (180.0, [0.0, 1.0, 1.0]),
        (240.0, [0.0, 0.0, 1.0]),
        (300.0, [1.0, 0.0, 1.0]),
        (360.0, [1.0, 0.0, 0.0]),
    ];
    for (hue, [r, g, b]) in hues {
        assert_eq!(hsv(hue, 1.0, 1.0), Color::new_with(r, g, b, 1.0, linear), "{hue}");
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, testHslInSrgb
#[test]
fn hsl_in_srgb_makes_the_named_colors() {
    assert_eq!(Color::hsl_with(0.0, 0.0, 0.0, 0.0, &ColorSpaces::SRGB), Color::TRANSPARENT);
    assert_eq!(Color::hsl(0.0, 0.0, 0.0), Color::BLACK);
    assert_eq!(Color::hsl(120.0, 0.0, 0.0), Color::BLACK);
    assert_eq!(Color::hsl(120.0, 1.0, 0.0), Color::BLACK);
    assert_eq!(Color::hsl(0.0, 0.0, 1.0), Color::WHITE);
    assert_eq!(Color::hsl(120.0, 1.0, 1.0), Color::WHITE);
    assert_eq!(Color::hsl(240.0, 0.5, 1.0), Color::WHITE);
    for hue in [0.0, 120.0, 240.0] {
        assert_eq!(Color::hsl(hue, 0.0, 0.5), Color::from_argb(0xFF80_8080));
    }
    let hues = [
        (0.0, Color::RED),
        (60.0, Color::YELLOW),
        (120.0, Color::GREEN),
        (180.0, Color::CYAN),
        (240.0, Color::BLUE),
        (300.0, Color::MAGENTA),
        (360.0, Color::RED),
    ];
    for (hue, color) in hues {
        assert_eq!(Color::hsl(hue, 1.0, 0.5), color, "{hue}");
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, testHslInLinearSrgb
#[test]
fn hsl_in_linear_srgb_stays_in_it() {
    let linear = &*ColorSpaces::LINEAR_SRGB;
    let hsl = |hue: f32, saturation: f32, lightness: f32| {
        Color::hsl_with(hue, saturation, lightness, 1.0, linear)
    };
    for (hue, saturation) in [(0.0, 0.0), (120.0, 0.0), (120.0, 1.0)] {
        assert_eq!(hsl(hue, saturation, 0.0).convert(&ColorSpaces::SRGB), Color::BLACK);
    }
    for hue in [0.0, 120.0, 240.0] {
        assert_eq!(hsl(hue, 0.0, 1.0).convert(&ColorSpaces::SRGB), Color::WHITE);
        assert_eq!(hsl(hue, 0.0, 0.5), Color::new_with(0.5, 0.5, 0.5, 1.0, linear));
    }
    let hues = [
        (0.0, [1.0, 0.0, 0.0]),
        (60.0, [1.0, 1.0, 0.0]),
        (120.0, [0.0, 1.0, 0.0]),
        (180.0, [0.0, 1.0, 1.0]),
        (240.0, [0.0, 0.0, 1.0]),
        (300.0, [1.0, 0.0, 1.0]),
        (360.0, [1.0, 0.0, 0.0]),
    ];
    for (hue, [r, g, b]) in hues {
        assert_eq!(hsl(hue, 1.0, 0.5), Color::new_with(r, g, b, 1.0, linear), "{hue}");
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, testHsvInSrgb
#[test]
#[should_panic(expected = "must be in range")]
fn a_hue_past_a_turn_is_refused() {
    Color::hsv(361.0, 1.0, 1.0);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, convertCieLab
#[test]
fn a_color_goes_to_lab_and_back() {
    let green = Color::from_rgb(100, 180, 50);
    let back = green.convert(&ColorSpaces::CIE_LAB).convert(&ColorSpaces::SRGB);
    assert_near(100.0 / 255.0, back.red(), 0.01);
    assert_near(180.0 / 255.0, back.green(), 0.01);
    assert_near(50.0 / 255.0, back.blue(), 0.01);
    // A dark gray, whose lightness is on the curve's linear part, comes back too.
    let dark = Color::new_with(5.0, 0.0, 0.0, 1.0, &ColorSpaces::CIE_LAB);
    let lab = dark.convert(&ColorSpaces::SRGB).convert(&ColorSpaces::CIE_LAB);
    assert_near(5.0, lab.red(), 0.1);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, toColorLong, fromColorLong
#[test]
fn a_color_of_another_space_packs_half_floats_and_its_id() {
    // 0x1F2B3D4E converted to each named space keeps its id in the low six bits, and the same
    // packed value comes back.
    let color = Color::from_argb(0x1F2B_3D4E);
    for space in ColorSpaces::all().iter().filter(|space| space.component_count() == 3) {
        let converted = color.convert(space);
        if space.is_srgb() {
            assert_eq!(converted, color);
            continue;
        }
        assert_eq!(converted.value() & 0x3F, space.id() as u64, "{space}");
        assert_eq!(Color::from_value(converted.value()), converted);
        assert_near(0x1F as f32 / 255.0, converted.alpha(), 1.0 / 1023.0);
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, red, green, blue, alpha
#[test]
fn a_wide_color_packs_rounded_half_floats() {
    // 0.1, 0.2 and 0.3 as half floats rounded to nearest (0x2E66, 0x3266, 0x34CD), alpha 0.4 as
    // 409 of 1023, Display P3's id 7: Android's color long of the same color.
    let color = Color::new_with(0.1, 0.2, 0.3, 0.4, &ColorSpaces::DISPLAY_P3);
    assert_eq!(color.value(), 0x2E66_3266_34CD_6647);
    assert_eq!(color.red(), 0.099975586);
    assert_eq!(color.alpha(), 409.0 / 1023.0);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/ColorTest.kt, lerp
#[test]
fn lerp_with_unspecified_ends() {
    assert_eq!(Color::lerp(Color::UNSPECIFIED, Color::UNSPECIFIED, 0.5), Color::UNSPECIFIED);
    // An unspecified start is transparent black.
    let half = Color::lerp(Color::UNSPECIFIED, Color::WHITE, 0.5);
    assert_eq!(*half.color_space(), ColorSpaces::SRGB);
    assert_eq!(half.to_argb() >> 24, 0x80);
}
