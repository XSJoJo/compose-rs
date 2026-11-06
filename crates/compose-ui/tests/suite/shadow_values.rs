//! Drop and inner shadows' values, as Compose's `shadow.Shadow` and `shadow.lerp`.

use compose_ui::shadow::{Shadow, lerp};
use compose_ui::*;

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/shadow/ShadowParamsTest.kt, testShadowParamsWithColor, testShadowParamsWithGradient, testShadowParamsWithSolidColorBrush
#[test]
fn a_shadow_has_a_color_or_a_brush_a_solid_brush_being_a_color() {
    let colored = Shadow::colored(Dp(4.0), Color::RED);
    assert_eq!((colored.color, colored.brush.clone()), (Color::RED, None));
    assert_eq!(Shadow::colored(Dp(4.0), Color::UNSPECIFIED).color, Color::BLACK);
    let gradient = Brush::horizontal_gradient([Color::RED, Color::BLUE]);
    let brushed = Shadow::brushed(Dp(4.0), gradient.clone());
    assert_eq!((brushed.color, brushed.brush), (Color::BLACK, Some(gradient)));
    let solid = Shadow::brushed(Dp(4.0), Brush::solid(Color::GREEN));
    assert_eq!((solid.color, solid.brush), (Color::GREEN, None));
    assert_eq!(Shadow::new(Dp(1.0)).with_alpha(3.0).alpha, 1.0, "alpha kept in 0 to 1");
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/shadow/ShadowParamsTest.kt, testShadowParamsEquals
#[test]
fn shadows_are_equal_when_their_values_are() {
    let shadow = || Shadow {
        spread: Dp(2.0),
        offset: DpOffset::new(Dp(1.0), Dp(1.0)),
        alpha: 0.5,
        ..Shadow::colored(Dp(4.0), Color::RED)
    };
    assert_eq!(shadow(), shadow());
    assert_ne!(shadow(), Shadow { blend_mode: BlendMode::Xor, ..shadow() });
    assert_ne!(shadow(), Shadow { radius: Dp(5.0), ..shadow() });
}

fn red() -> Shadow {
    Shadow { spread: Dp(1.0), alpha: 0.5, ..Shadow::colored(Dp(1.0), Color::RED) }
}

fn blue() -> Shadow {
    Shadow {
        spread: Dp(3.0),
        offset: DpOffset::new(Dp(1.0), Dp(1.0)),
        blend_mode: BlendMode::DstOver,
        ..Shadow::colored(Dp(2.0), Color::BLUE)
    }
}

/// `start` and `stop` mixed at `fraction`, each value as its own `lerp`.
fn mixed(start: &Shadow, stop: &Shadow, fraction: f32) -> Shadow {
    let dp = |a: Dp, b: Dp| Dp(a.0 + (b.0 - a.0) * fraction);
    Shadow {
        radius: dp(start.radius, stop.radius),
        spread: dp(start.spread, stop.spread),
        offset: DpOffset::lerp(start.offset, stop.offset, fraction),
        color: Color::lerp(start.color, stop.color, fraction),
        brush: None,
        alpha: start.alpha + (stop.alpha - start.alpha) * fraction,
        blend_mode: if fraction < 0.5 { start.blend_mode } else { stop.blend_mode },
    }
}

// Compose: compose/ui/ui-graphics/src/androidHostTest/kotlin/androidx/compose/ui/graphics/ShadowInterpolationTests.kt, testShadowToShadow, testShadowToShadow_fraction
#[test]
fn a_shadow_mixes_into_another() {
    for fraction in [0.5, 0.25, 0.75] {
        assert_eq!(
            lerp(Some(&red()), Some(&blue()), fraction),
            Some(mixed(&red(), &blue(), fraction))
        );
    }
    assert_eq!(lerp(None, None, 0.5), None);
}

// Compose: compose/ui/ui-graphics/src/androidHostTest/kotlin/androidx/compose/ui/graphics/ShadowInterpolationTests.kt, testShadowToNull, testNullToShadow, testShadowToNull_fraction, testNullToShadow_fraction
#[test]
fn a_shadow_fades_to_and_from_none() {
    let transparent = |shadow: Shadow| Shadow { color: Color::TRANSPARENT, ..shadow };
    for fraction in [0.5, 0.25] {
        assert_eq!(
            lerp(Some(&red()), None, fraction),
            Some(mixed(&red(), &transparent(red()), fraction))
        );
        assert_eq!(
            lerp(None, Some(&blue()), fraction),
            Some(mixed(&transparent(blue()), &blue(), fraction))
        );
    }
}
