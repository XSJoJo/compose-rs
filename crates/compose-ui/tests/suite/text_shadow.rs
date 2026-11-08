//! Compose's `ShadowTest` (ui-graphics): the text `Shadow`'s default, its values and `lerp`.

use compose_ui::{Color, Offset, Shadow, lerp};

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ShadowTest.kt, defaultValue
#[test]
fn default_value() {
    let shadow = Shadow::default();
    assert_eq!(shadow.color, Color::from_argb(0xFF000000));
    assert_eq!(shadow.blur_radius, 0.0);
    assert_eq!(shadow.offset, Offset::ZERO);
    assert_eq!(shadow, Shadow::NONE);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ShadowTest.kt, testConstructor
#[test]
fn test_constructor() {
    let color = Color::from_argb(0xFF00FF00);
    let offset = Offset::new(2.0, 3.0);
    let shadow = Shadow { color, offset, blur_radius: 1.0 };
    assert_eq!(shadow.color, color);
    assert_eq!(shadow.offset, offset);
    assert_eq!(shadow.blur_radius, 1.0);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ShadowTest.kt, testLerp
#[test]
fn test_lerp() {
    let (color_a, color_b) = (Color::from_argb(0xFF00FF00), Color::from_argb(0xFF0000FF));
    let (offset_a, offset_b) = (Offset::new(5.0, 10.0), Offset::new(0.0, 5.0));
    let (radius_a, radius_b) = (0.0, 3.0);
    let a = Shadow { color: color_a, offset: offset_a, blur_radius: radius_a };
    let b = Shadow { color: color_b, offset: offset_b, blur_radius: radius_b };
    let t = 0.4;
    let shadow = Shadow::lerp(a, b, t);
    assert_eq!(shadow.color, Color::lerp(color_a, color_b, t));
    assert_eq!(shadow.offset, lerp(offset_a, offset_b, t));
    assert_eq!(shadow.blur_radius, lerp(radius_a, radius_b, t));
}
