//! Compose's `SplitButtonScreenshotTest.kt` (each scenario in the light and dark schemes), its
//! goldens as what the display list draws: each kind's fills and outline, enabled and disabled,
//! the two buttons' corners at rest, pressed and checked, the checked one's tint, and the layout.

use compose_foundation::layout::*;
use compose_material3::*;
use compose_test::*;
use compose_ui::platform::DrawOp;
use compose_ui::{Color, Modifier, Offset, Outline, PaintStyle, Press, PressInteraction, UnitExt};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Filled,
    Tonal,
    Elevated,
    Outlined,
}

/// What a split button draws.
#[derive(Debug, Default)]
struct Drawn {
    fills: Vec<Color>,
    outlines: Vec<Color>,
    /// The clips' corners: the leading button's, then the trailing one's.
    corners: Vec<[f32; 4]>,
    leading: compose_ui::Rect,
    trailing: compose_ui::Rect,
    layout: compose_ui::Rect,
}

struct Scenario {
    kind: Kind,
    enabled: bool,
    checked: bool,
    /// Which to press: 0 the leading button, 1 the trailing one.
    pressed: Option<usize>,
    fill_width: bool,
}

const PLAIN: Scenario = Scenario {
    kind: Kind::Filled,
    enabled: true,
    checked: false,
    pressed: None,
    fill_width: false,
};

fn draw(scheme: ColorScheme, scenario: Scenario) -> Drawn {
    let sources =
        [compose_ui::MutableInteractionSource::new(), compose_ui::MutableInteractionSource::new()];
    let Scenario { kind, enabled, checked, pressed, fill_width } = scenario;
    let mut ui = compose_test({
        let sources = sources.clone();
        move || {
            let sources = sources.clone();
            MaterialTheme!(color_scheme = scheme, {
                let [leading_source, trailing_source] = sources.clone();
                let wrap = if fill_width { Modifier.width(200.dp()) } else { Modifier };
                Box!(modifier = wrap, {
                    let [leading_source, trailing_source] =
                        [leading_source.clone(), trailing_source.clone()];
                    SplitButtonLayout!(
                        leading_button = move || {
                            let modifier = Modifier.test_tag("leading");
                            let modifier =
                                if fill_width { modifier.fill_max_width() } else { modifier };
                            let source = Some(leading_source.clone());
                            let content = |_: RowScope| {
                                Icon!(
                                    icons::filled::edit(),
                                    Some("Localized description"),
                                    modifier =
                                        Modifier.size(SplitButtonDefaults::LEADING_ICON_SIZE)
                                );
                                Spacer!(modifier = Modifier.size(ButtonDefaults::ICON_SPACING));
                                Text!("My Button");
                            };
                            match kind {
                                Kind::Filled => SplitButtonLeadingButton!(
                                    || {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                                Kind::Tonal => SplitButtonTonalLeadingButton!(
                                    || {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                                Kind::Elevated => SplitButtonElevatedLeadingButton!(
                                    || {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                                Kind::Outlined => SplitButtonOutlinedLeadingButton!(
                                    || {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                            }
                        },
                        trailing_button = move || {
                            let modifier = Modifier.test_tag("trailing");
                            let source = Some(trailing_source.clone());
                            let content = |_: RowScope| {
                                Icon!(
                                    icons::filled::keyboard_arrow_down(),
                                    Some("Localized description"),
                                    modifier =
                                        Modifier.size(SplitButtonDefaults::TRAILING_ICON_SIZE)
                                );
                            };
                            match kind {
                                Kind::Filled => SplitButtonToggleableTrailingButton!(
                                    checked,
                                    |_| {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                                Kind::Tonal => SplitButtonTonalTrailingButton!(
                                    checked,
                                    |_| {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                                Kind::Elevated => SplitButtonElevatedTrailingButton!(
                                    checked,
                                    |_| {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                                Kind::Outlined => SplitButtonOutlinedTrailingButton!(
                                    checked,
                                    |_| {},
                                    modifier = modifier,
                                    enabled = enabled,
                                    interaction_source = source,
                                    content = content
                                ),
                            }
                        },
                        modifier = Modifier.test_tag("layout")
                    );
                });
            });
        }
    });
    if let Some(index) = pressed {
        sources[index].try_emit(PressInteraction::Press(Press::new(Offset::ZERO)));
        ui.wait_for_idle();
    }
    let list = ui.host().display_list();
    let mut drawn = Drawn::default();
    for op in &list.ops {
        match op {
            DrawOp::DrawRoundRect(_, _, paint) | DrawOp::DrawRect(_, paint) => match paint.style {
                PaintStyle::Fill => drawn.fills.push(paint.color),
                PaintStyle::Stroke(_) => drawn.outlines.push(paint.color),
            },
            // A border of corners that differ is a filled path, as the icons are.
            DrawOp::DrawPath { paint, .. } => drawn.outlines.push(paint.color),
            _ => {}
        }
    }
    let layers = list.groups.iter().filter_map(|group| group.layer.as_ref());
    drawn.corners = layers
        .filter(|layer| layer.clip)
        .filter_map(|layer| match &layer.outline {
            Some(Outline::Rounded(_, radii)) => Some(*radii),
            _ => None,
        })
        .collect();
    let mut bounds =
        |tag: &str| ui.on_node_unmerged(has_test_tag(tag)).fetch_semantics_snapshot().bounds;
    drawn.leading = bounds("leading");
    drawn.trailing = bounds("trailing");
    drawn.layout = bounds("layout");
    drawn
}

const SCHEMES: [fn() -> ColorScheme; 2] = [light_color_scheme, dark_color_scheme];

/// Round outside, 4 dp inside; the trailing button's pill when checked.
const LEADING: [f32; 4] = [20.0, 4.0, 4.0, 20.0];
const TRAILING: [f32; 4] = [4.0, 20.0, 20.0, 4.0];

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, filledSplitButton
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, splitButton_iconLeadingButton
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, splitButton_textLeadingButton
#[test]
fn a_filled_split_button_is_two_primary_buttons_2_dp_apart() {
    for scheme in SCHEMES.map(|scheme| scheme()) {
        let drawn = draw(scheme, PLAIN);
        assert_eq!(drawn.fills, vec![scheme.primary; 2]);
        assert_eq!(drawn.corners, vec![LEADING, TRAILING]);
        assert_eq!(drawn.trailing.left - drawn.leading.right, 2.0);
        assert_eq!((drawn.leading.height(), drawn.trailing.height()), (40.0, 40.0));
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, filledSplitButton_leadingFillMaxWidth
#[test]
fn a_full_width_split_button_gives_the_leading_button_the_rest() {
    let drawn = draw(light_color_scheme(), Scenario { fill_width: true, ..PLAIN });
    assert_eq!(drawn.layout.width(), 200.0);
    assert_eq!(drawn.trailing.right, 200.0);
    assert_eq!(drawn.leading.width() + 2.0 + drawn.trailing.width(), 200.0);
}

/// The colors `kind`'s buttons take in `scheme`.
fn colors(scheme: ColorScheme, kind: Kind) -> ButtonColors {
    let read = std::rc::Rc::new(std::cell::Cell::new(None));
    let _ui = compose_test({
        let read = read.clone();
        move || {
            let read = read.clone();
            MaterialTheme!(color_scheme = scheme, {
                read.set(Some(match kind {
                    Kind::Filled => ButtonDefaults::button_colors(),
                    Kind::Tonal => ButtonDefaults::filled_tonal_button_colors(),
                    Kind::Elevated => ButtonDefaults::elevated_button_colors(),
                    Kind::Outlined => ButtonDefaults::outlined_button_colors(),
                }));
            });
        }
    });
    read.get().expect("composed")
}

/// Each kind's two fills (none for an outlined one) and outlines.
fn check_kind(kind: Kind, enabled: bool) {
    for scheme in SCHEMES.map(|scheme| scheme()) {
        let drawn = draw(scheme, Scenario { kind, enabled, ..PLAIN });
        let colors = colors(scheme, kind);
        let fill = colors.container_color(enabled);
        let fills = if fill == Color::TRANSPARENT { vec![] } else { vec![fill; 2] };
        assert_eq!(drawn.fills, fills, "{kind:?} enabled {enabled}");
        let outline = scheme.outline_variant;
        let outline = if enabled { outline } else { outline.with_alpha(0.1) };
        let outlines = drawn.outlines.iter().filter(|&&color| color == outline).count();
        let expected = if kind == Kind::Outlined { 2 } else { 0 };
        assert_eq!(outlines, expected, "{kind:?} enabled {enabled}: {:?}", drawn.outlines);
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, filledSplitButton_disabled
#[test]
fn a_disabled_filled_split_button_is_faded() {
    check_kind(Kind::Filled, false);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, tonalSplitButton
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, tonalSplitButton_disabled
#[test]
fn a_tonal_split_button_is_in_the_secondary_container() {
    check_kind(Kind::Tonal, true);
    check_kind(Kind::Tonal, false);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, elevatedSplitButton
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, elevatedSplitButton_disabled
#[test]
fn an_elevated_split_button_is_in_the_low_surface_container() {
    check_kind(Kind::Elevated, true);
    check_kind(Kind::Elevated, false);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, outlinedSplitButton
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, outlinedSplitButton_disabled
#[test]
fn an_outlined_split_button_is_two_outlines() {
    check_kind(Kind::Outlined, true);
    check_kind(Kind::Outlined, false);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, filledSplitButtonChecked
#[test]
fn a_checked_trailing_button_is_a_pill_tinted_with_its_content_color() {
    for scheme in SCHEMES.map(|scheme| scheme()) {
        let drawn = draw(scheme, Scenario { checked: true, ..PLAIN });
        assert_eq!(drawn.corners, vec![LEADING, [20.0; 4]]);
        assert_eq!(
            drawn.fills,
            vec![scheme.primary, scheme.primary, scheme.on_primary.with_alpha(0.1)]
        );
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, splitButton_leadingButton_pressed
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonScreenshotTest.kt, splitButton_trailingButton_pressed
#[test]
fn a_pressed_buttons_inner_corners_round_to_12_dp() {
    for scheme in SCHEMES.map(|scheme| scheme()) {
        let leading = draw(scheme, Scenario { pressed: Some(0), ..PLAIN });
        assert_eq!(leading.corners, vec![[20.0, 12.0, 12.0, 20.0], TRAILING]);
        let trailing = draw(scheme, Scenario { pressed: Some(1), ..PLAIN });
        assert_eq!(trailing.corners, vec![LEADING, [12.0, 20.0, 20.0, 12.0]]);
    }
}

/// The trailing button's size by default, in its layout's 40 dp: at least 48 dp wide.
#[test]
fn the_trailing_button_is_at_least_48_dp_wide() {
    let drawn = draw(light_color_scheme(), PLAIN);
    assert_eq!(drawn.trailing.width(), 48.0);
}
