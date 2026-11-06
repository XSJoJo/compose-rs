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
    // A sample of Compose's thousand and one values, each channel within one step.
    let expected = [
        (0, 0xFFFF_0000u32),
        (1, 0xFFFF_0102),
        (100, 0xFFE8_3342),
        (200, 0xFFD1_4460),
        (300, 0xFFBA_4D79),
        (400, 0xFFA4_528E),
        (500, 0xFF8C_53A2),
        (600, 0xFF75_51B6),
        (700, 0xFF5D_4BC8),
        (800, 0xFF44_41DB),
        (900, 0xFF29_30ED),
        (999, 0xFF00_01FF),
        (1000, 0xFF00_00FF),
    ];
    for (step, argb) in expected {
        let color = Color::lerp(Color::RED, Color::BLUE, step as f32 / 1000.0).to_argb();
        for shift in [16, 8, 0] {
            let (want, got) = ((argb >> shift) as u8, (color >> shift) as u8);
            assert!(want.abs_diff(got) < 2, "at {step}/1000: {color:08X}, Compose's {argb:08X}");
        }
    }
}

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
