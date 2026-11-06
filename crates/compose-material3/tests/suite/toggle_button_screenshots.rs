//! Compose's `ToggleButtonScreenshotTest.kt`, its goldens as what the display list draws: each
//! toggle button's container, outline, content and icon colors in the light and dark schemes,
//! checked, unchecked and disabled, and its corners at rest, checked and pressed.

use compose_foundation::layout::*;
use compose_material3::*;
use compose_test::*;
use compose_ui::platform::DrawOp;
use compose_ui::{Color, Offset, Outline, PaintStyle, Press, PressInteraction};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Filled,
    Elevated,
    Tonal,
    Outlined,
}

const KINDS: [Kind; 4] = [Kind::Filled, Kind::Elevated, Kind::Tonal, Kind::Outlined];

/// What a toggle button draws.
#[derive(PartialEq, Debug, Default)]
struct Drawn {
    container: Option<Color>,
    outline: Option<Color>,
    content: Option<Color>,
    icon: Option<Color>,
    corner: f32,
}

/// A toggle button of `kind` in `scheme`, pressed if `pressed`, with an icon if `icon`.
fn draw(
    scheme: ColorScheme,
    kind: Kind,
    checked: bool,
    enabled: bool,
    pressed: bool,
    icon: bool,
) -> Drawn {
    let content = std::rc::Rc::new(std::cell::Cell::new(None));
    let source = compose_ui::MutableInteractionSource::new();
    let mut ui = compose_test({
        let (content, source) = (content.clone(), source.clone());
        move || {
            let (content, source) = (content.clone(), Some(source.clone()));
            MaterialTheme!(color_scheme = scheme, {
                let label = {
                    let content = content.clone();
                    move |_: RowScope| {
                        content.set(Some(LOCAL_CONTENT_COLOR.current()));
                        Text!("Button");
                    }
                };
                let icon = icon.then_some(|| {
                    Icon!(icons::filled::favorite(), Some("Localized description"));
                });
                match kind {
                    Kind::Filled => ToggleButton!(
                        checked,
                        |_| {},
                        enabled = enabled,
                        icon = icon,
                        interaction_source = source.clone(),
                        content = label
                    ),
                    Kind::Elevated => ElevatedToggleButton!(
                        checked,
                        |_| {},
                        enabled = enabled,
                        icon = icon,
                        interaction_source = source.clone(),
                        content = label
                    ),
                    Kind::Tonal => FilledTonalToggleButton!(
                        checked,
                        |_| {},
                        enabled = enabled,
                        icon = icon,
                        interaction_source = source.clone(),
                        content = label
                    ),
                    Kind::Outlined => OutlinedToggleButton!(
                        checked,
                        |_| {},
                        enabled = enabled,
                        icon = icon,
                        interaction_source = source.clone(),
                        content = label
                    ),
                }
            });
        }
    });
    if pressed {
        source.try_emit(PressInteraction::Press(Press::new(Offset::ZERO)));
        ui.wait_for_idle();
    }
    let list = ui.host().display_list();
    let mut drawn = Drawn { content: content.get(), ..Drawn::default() };
    for op in &list.ops {
        match op {
            DrawOp::DrawRoundRect(_, _, paint) | DrawOp::DrawRect(_, paint) => match paint.style {
                PaintStyle::Fill => drawn.container = Some(paint.color),
                PaintStyle::Stroke(_) => drawn.outline = Some(paint.color),
            },
            DrawOp::DrawPath { paint, .. } => drawn.icon = Some(paint.color),
            _ => {}
        }
    }
    let layers = list.groups.iter().filter_map(|group| group.layer.as_ref());
    drawn.corner = match layers.filter(|layer| layer.clip).find_map(|layer| layer.outline.clone()) {
        Some(Outline::Rounded(_, radii)) => radii[0],
        other => panic!("{other:?}"),
    };
    drawn
}

/// The colors `kind`'s defaults give in `scheme`.
fn colors(scheme: ColorScheme, kind: Kind) -> ToggleButtonColors {
    let read = std::rc::Rc::new(std::cell::Cell::new(None));
    let _ui = compose_test({
        let read = read.clone();
        move || {
            let read = read.clone();
            MaterialTheme!(color_scheme = scheme, {
                read.set(Some(match kind {
                    Kind::Filled => ToggleButtonDefaults::colors(),
                    Kind::Elevated => ElevatedToggleButtonDefaults::colors(),
                    Kind::Tonal => FilledTonalToggleButtonDefaults::colors(),
                    Kind::Outlined => OutlinedToggleButtonDefaults::colors(),
                }));
            });
        }
    });
    read.get().expect("composed")
}

/// Each kind draws its colors for its state, and an outlined one its outline until checked.
fn check_colors(scheme: ColorScheme, checked: bool, enabled: bool, icon: bool, kinds: &[Kind]) {
    for &kind in kinds {
        let colors = colors(scheme, kind);
        let drawn = draw(scheme, kind, checked, enabled, false, icon);
        let container = colors.container_color(enabled, checked);
        let content = colors.content_color(enabled, checked);
        let outline = (kind == Kind::Outlined && !checked).then(|| {
            if enabled { scheme.outline_variant } else { scheme.outline_variant.with_alpha(0.1) }
        });
        assert_eq!(
            (drawn.container, drawn.outline, drawn.content, drawn.icon),
            ((container != Color::TRANSPARENT).then_some(container), outline, Some(content), {
                icon.then_some(content)
            }),
            "{kind:?} checked {checked} enabled {enabled}"
        );
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, elevatedToggleButton_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, filledTonalToggleButton_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, outlinedToggleButton_lightTheme
#[test]
fn unchecked_toggle_buttons_draw_their_unchecked_colors() {
    check_colors(light_color_scheme(), false, true, false, &KINDS);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, elevatedToggleButton_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, filledTonalToggleButton_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, outlinedToggleButton_darkTheme
#[test]
fn unchecked_dark_toggle_buttons_draw_their_unchecked_colors() {
    check_colors(dark_color_scheme(), false, true, false, &KINDS);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_checked_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, elevatedToggleButton_checked_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, filledTonalToggleButton_checked_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, outlinedToggleButton_checked_lightTheme
#[test]
fn checked_toggle_buttons_draw_their_checked_colors_without_an_outline() {
    check_colors(light_color_scheme(), true, true, false, &KINDS);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_checked_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, elevatedToggleButton_checked_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, filledTonalToggleButton_checked_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, outlinedToggleButton_checked_darkTheme
#[test]
fn checked_dark_toggle_buttons_draw_their_checked_colors() {
    check_colors(dark_color_scheme(), true, true, false, &KINDS);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_lightTheme_disabled
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, elevatedToggleButton_lightTheme_disabled
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, filledTonalToggleButton_lightTheme_disabled
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, outlinedToggleButton_lightTheme_disabled
#[test]
fn disabled_toggle_buttons_draw_their_disabled_colors() {
    check_colors(light_color_scheme(), false, false, false, &KINDS);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_withIcon_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_withIcon_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_withIcon_checked_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_withIcon_checked_darkTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_withIcon_lightTheme_disabled
#[test]
fn a_toggle_buttons_icon_is_in_its_content_color() {
    for scheme in [light_color_scheme(), dark_color_scheme()] {
        check_colors(scheme, false, true, true, &[Kind::Filled]);
        check_colors(scheme, true, true, true, &[Kind::Filled]);
    }
    check_colors(light_color_scheme(), false, false, true, &[Kind::Filled]);
}

/// The theme's small, medium and full corners on a 40 dp button.
const PRESSED: f32 = 8.0;
const CHECKED: f32 = 12.0;
const ROUND: f32 = 20.0;

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_lightTheme_defaultToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, elevatedToggleButton_lightTheme_defaultToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, filledTonalToggleButton_lightTheme_defaultToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, outlinedToggleButton_lightTheme_defaultToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_withIcon_lightTheme_defaultToPressed
#[test]
fn a_toggle_button_is_round_until_pressed() {
    for kind in KINDS {
        let scheme = light_color_scheme();
        assert_eq!(draw(scheme, kind, false, true, false, false).corner, ROUND, "{kind:?}");
        assert_eq!(draw(scheme, kind, false, true, true, false).corner, PRESSED, "{kind:?}");
    }
    assert_eq!(draw(light_color_scheme(), Kind::Filled, false, true, true, true).corner, PRESSED);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_lightTheme_checkedToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, elevatedToggleButton_lightTheme_checkedToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, filledTonalToggleButton_lightTheme_checkedToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, outlinedToggleButton_lightTheme_checkedToPressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonScreenshotTest.kt, toggleButton_withIcon_lightTheme_checkedToPressed
#[test]
fn a_checked_toggle_button_is_square_and_squarer_pressed() {
    for kind in KINDS {
        let scheme = light_color_scheme();
        assert_eq!(draw(scheme, kind, true, true, false, false).corner, CHECKED, "{kind:?}");
        assert_eq!(draw(scheme, kind, true, true, true, false).corner, PRESSED, "{kind:?}");
    }
    assert_eq!(draw(light_color_scheme(), Kind::Filled, true, true, true, true).corner, PRESSED);
}
