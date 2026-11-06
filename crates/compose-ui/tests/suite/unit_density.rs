//! Compose's `DensityTest.kt` (ui-unit) and `AndroidDensityTest.kt` (its device tests, the
//! `FontScaling` conversions, linear as Compose's below non-linear font scaling): pixels from and to
//! `Dp`, `Sp`, `DpRect`, `DpSize` and `Size`.

use compose_ui::{Density, DpRect, DpSize, Size, TextUnit, UnitExt};

fn density() -> Density {
    Density { density: 2.0, font_scale: 3.0 }
}

fn assert_near(expected: f32, actual: f32) {
    assert!((expected - actual).abs() <= 0.001, "expected {expected}, got {actual}");
}

fn panic_message(block: impl FnOnce()) -> String {
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(block)).unwrap_err();
    payload
        .downcast_ref::<&str>()
        .map(|message| message.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testValues
#[test]
fn a_density_keeps_its_values() {
    let density = Density::with_font_scale(2.0, 3.0);
    assert_near(2.0, density.density);
    assert_near(3.0, density.font_scale);
    assert_eq!(density, self::density());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testDpToPx
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testDpRoundToPx
#[test]
fn dps_become_pixels_rounded_half_up() {
    assert_near(2.0, density().to_px(1.dp()));
    assert_eq!(density().round_to_px(4.95.dp()), 10);
    assert_eq!(density().round_to_px(4.75.dp()), 10);
    assert_eq!(density().round_to_px(4.74.dp()), 9);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testIntToDp
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testFloatToDp
#[test]
fn pixels_become_dps() {
    let pixels: i32 = 2;
    assert_eq!(density().to_dp(pixels as f32), 1.dp());
    assert_eq!(density().to_dp(2.0), 1.dp());
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testDpRectToRect
#[test]
fn a_dp_rect_becomes_a_rect_in_pixels() {
    let rect = density().to_rect(DpRect::new(1.dp(), 2.dp(), 3.dp(), 4.dp()));
    assert_near(2.0, rect.left);
    assert_near(4.0, rect.top);
    assert_near(6.0, rect.right);
    assert_near(8.0, rect.bottom);
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testDpSizeToSize
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testSizeToDpSize
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testDpSizeUnspecifiedToSize
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/DensityTest.kt, testSizeUnspecifiedToDpSize
#[test]
fn sizes_convert_both_ways_and_unspecified_stays_unspecified() {
    assert_eq!(density().to_size(DpSize::new(1.dp(), 3.dp())), Size::new(2.0, 6.0));
    assert_eq!(density().to_dp_size(Size::new(2.0, 6.0)), DpSize::new(1.dp(), 3.dp()));
    assert!(density().to_size(DpSize::UNSPECIFIED).is_unspecified());
    assert_eq!(density().to_dp_size(Size::UNSPECIFIED), DpSize::UNSPECIFIED);
}

// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testDpToSp
// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testTextUnitToDp
#[test]
fn sps_and_dps_differ_by_the_font_scale() {
    assert_eq!(density().dp_to_sp(6.dp()), 2.sp());
    assert_eq!(density().sp_to_dp(1.sp()), 3.dp());
}

// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testTextUnitToPx
// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testTextRoundUnitToPx
#[test]
fn sps_become_pixels() {
    assert_near(6.0, density().sp_to_px(1.sp()));
    assert_eq!(density().sp_round_to_px(1.sp()), 6);
    assert_eq!(density().sp_round_to_px(1.05.sp()), 6);
    assert_eq!(density().sp_round_to_px(0.95.sp()), 6);
}

// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testTextUnitToPxFail
// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testTextUnitRoundToPxFail
#[test]
fn an_em_has_no_pixels() {
    let message = "Only Sp can convert to Px";
    assert_eq!(panic_message(|| _ = density().sp_to_px(1.em())), message);
    assert_eq!(panic_message(|| _ = density().sp_round_to_px(1.em())), message);
    assert_eq!(panic_message(|| _ = density().sp_to_dp(TextUnit::UNSPECIFIED)), message);
}

// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testIntToSp
// Compose: compose/ui/ui-unit/src/androidDeviceTest/kotlin/androidx/compose/ui/unit/AndroidDensityTest.kt, testFloatToSp
#[test]
fn pixels_become_sps() {
    let pixels: i32 = 6;
    assert_eq!(density().to_sp(pixels as f32), 1.sp());
    assert_eq!(density().to_sp(6.0), 1.sp());
}
