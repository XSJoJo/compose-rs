//! Compose's `SplitButtonTest.kt` and `HorizontalCenterOpticallyTest.kt`: a split button's content
//! and semantics, its checked trailing button, its paddings at each size, its trailing content
//! moved toward the rounder end, and how its two buttons share their height.
//!
//! `horizontalCenterOptically` is internal in Compose; its tests are here through the trailing
//! button, which is where it's used.

use compose_foundation::layout::*;
use compose_foundation::{ScrollModifiers, remember_scroll_state};
use compose_material3::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{CornerSize, Dp, LayoutDirection, Modifier, PaddingValues, Role, Shape, UnitExt};

const LEADING: &str = "leading button";
const TRAILING: &str = "trailing button";

fn bounds(ui: &mut ComposeTestRule, tag: &str) -> compose_ui::Rect {
    ui.on_node_unmerged(has_test_tag(tag)).fetch_semantics_snapshot().bounds
}

fn role(ui: &mut ComposeTestRule, tag: &str) -> Option<Role> {
    let node = ui.on_node_with_tag(tag).fetch_semantics_snapshot();
    node.config.get(&compose_ui::SemanticsProperties::ROLE).copied()
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Filled,
    Tonal,
    Elevated,
    Outlined,
}

/// A split button of `kind`: an edit icon and "My Button", then a 34 dp toggleable arrow.
fn split(kind: Kind, enabled: bool) -> ComposeTestRule {
    compose_test(move || {
        MaterialTheme!({
            let checked = remember(|| mutable_state_of(false));
            let leading = move |_: RowScope| {
                Icon!(icons::filled::edit(), Some("Leading Icon"));
                Spacer!(modifier = Modifier.size(ButtonDefaults::ICON_SPACING));
                Text!("My Button");
            };
            let trailing = || Icon!(icons::filled::keyboard_arrow_down(), Some("Trailing Icon"));
            let (leading_modifier, trailing_modifier) =
                (Modifier.test_tag(LEADING), Modifier.size(34.dp()).test_tag(TRAILING));
            SplitButtonLayout!(
                leading_button = move || match kind {
                    Kind::Filled => SplitButtonLeadingButton!(
                        || {},
                        modifier = leading_modifier.clone(),
                        enabled = enabled,
                        content = leading
                    ),
                    Kind::Tonal => SplitButtonTonalLeadingButton!(
                        || {},
                        modifier = leading_modifier.clone(),
                        enabled = enabled,
                        content = leading
                    ),
                    Kind::Elevated => SplitButtonElevatedLeadingButton!(
                        || {},
                        modifier = leading_modifier.clone(),
                        enabled = enabled,
                        content = leading
                    ),
                    Kind::Outlined => SplitButtonOutlinedLeadingButton!(
                        || {},
                        modifier = leading_modifier.clone(),
                        enabled = enabled,
                        content = leading
                    ),
                },
                trailing_button = move || {
                    let on_change = move |now| checked.set(now);
                    let content = move |_: RowScope| trailing();
                    match kind {
                        Kind::Filled => SplitButtonToggleableTrailingButton!(
                            checked.get(),
                            on_change,
                            modifier = trailing_modifier.clone(),
                            enabled = enabled,
                            content = content
                        ),
                        Kind::Tonal => SplitButtonTonalTrailingButton!(
                            checked.get(),
                            on_change,
                            modifier = trailing_modifier.clone(),
                            enabled = enabled,
                            content = content
                        ),
                        Kind::Elevated => SplitButtonElevatedTrailingButton!(
                            checked.get(),
                            on_change,
                            modifier = trailing_modifier.clone(),
                            enabled = enabled,
                            content = content
                        ),
                        Kind::Outlined => SplitButtonOutlinedTrailingButton!(
                            checked.get(),
                            on_change,
                            modifier = trailing_modifier.clone(),
                            enabled = enabled,
                            content = content
                        ),
                    }
                }
            );
        });
    })
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_contentDisplay
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, TonalSplitButton_contentDisplay
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, ElevatedSplitButton_contentDisplay
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, OutlinedSplitButton_contentDisplay
#[test]
fn each_split_button_shows_its_two_buttons_content() {
    for kind in [Kind::Filled, Kind::Tonal, Kind::Elevated, Kind::Outlined] {
        let mut ui = split(kind, true);
        ui.on_node_with_text("My Button").assert_is_displayed();
        ui.on_node_with_content_description("Leading Icon").assert_is_displayed();
        ui.on_node_with_content_description("Trailing Icon").assert_is_displayed();
        if kind == Kind::Filled {
            ui.on_node_with_tag(LEADING).assert_has_click_action();
            ui.on_node_with_tag(TRAILING).assert_has_click_action();
        }
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_trailingButtonChecked
#[test]
fn the_trailing_button_is_a_button_a_click_checks() {
    let mut ui = split(Kind::Filled, true);
    let node = ui.on_node(is_toggleable()).fetch_semantics_snapshot();
    assert_eq!(
        node.config.get(&compose_ui::SemanticsProperties::ROLE).copied(),
        Some(Role::Button)
    );
    ui.on_node(is_toggleable()).assert_is_enabled().assert_is_off().perform_click().assert_is_on();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_defaultSemantics
#[test]
fn both_buttons_are_enabled_buttons() {
    let mut ui = split(Kind::Filled, true);
    for tag in [LEADING, TRAILING] {
        assert_eq!(role(&mut ui, tag), Some(Role::Button));
        ui.on_node_with_tag(tag).assert_is_enabled();
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_disabledSemantics
#[test]
fn both_buttons_disabled_are_disabled_buttons() {
    let mut ui = split(Kind::Filled, false);
    for tag in [LEADING, TRAILING] {
        assert_eq!(role(&mut ui, tag), Some(Role::Button));
        ui.on_node_with_tag(tag).assert_is_not_enabled();
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, FilledSplitButton_contentPadding
#[test]
fn the_trailing_buttons_icon_moves_toward_its_round_end() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            SplitButtonLayout!(
                leading_button = || {
                    SplitButtonLeadingButton!(
                        || {},
                        modifier = Modifier.test_tag(LEADING),
                        enabled = false,
                        content =
                            |_: RowScope| Text!("My Button", modifier = Modifier.test_tag("text"))
                    );
                },
                trailing_button = || {
                    SplitButtonToggleableTrailingButton!(
                        false,
                        |_| {},
                        modifier = Modifier.test_tag(TRAILING),
                        enabled = false,
                        content = |_: RowScope| {
                            Icon!(
                                icons::filled::keyboard_arrow_down(),
                                Some("Trailing Icon"),
                                modifier = Modifier.test_tag("icon")
                            );
                        }
                    );
                }
            );
        });
    });
    let (leading, text) = (bounds(&mut ui, LEADING), bounds(&mut ui, "text"));
    assert_eq!(text.left - leading.left, 16.0);
    assert_eq!(leading.right - text.right, 12.0);
    let (trailing, icon) = (bounds(&mut ui, TRAILING), bounds(&mut ui, "icon"));
    // The start corners 4 dp, the end ones half the height: 11 % of the difference, to the pixel.
    // Compose's test halves the round corner in its expectation (0.7 dp), within its tolerance at
    // its density; its code, as here, takes the corner whole.
    let correction = (0.11f32 * (4.0 - trailing.height() / 2.0)).round();
    assert_eq!(icon.left - trailing.left, 13.0 + correction);
    assert_eq!(trailing.right - icon.right, 13.0 - correction);
}

/// The leading button's spaces at `height`'s size: the icon's start, the gap to the text, the
/// text's end.
fn leading_spaces(height: Dp) -> (f32, f32, f32) {
    let mut ui = compose_test(move || {
        MaterialTheme!({
            SplitButtonLayout!(
                leading_button = move || {
                    SplitButtonLeadingButton!(
                        || {},
                        modifier = Modifier.test_tag(LEADING).height_in(height, Dp::UNSPECIFIED),
                        shapes = SplitButtonDefaults::leading_button_shapes_for(height),
                        content_padding =
                            SplitButtonDefaults::leading_button_content_padding_for(height),
                        content = move |_: RowScope| {
                            Icon!(
                                icons::filled::edit(),
                                Some("Leading Icon"),
                                modifier = Modifier.test_tag("icon")
                            );
                            Spacer!(
                                modifier = Modifier.size(ButtonDefaults::icon_spacing_for(height))
                            );
                            Text!("My Button", modifier = Modifier.test_tag("text"));
                        }
                    );
                },
                trailing_button = move || {
                    SplitButtonToggleableTrailingButton!(
                        false,
                        |_| {},
                        modifier = Modifier.height_in(height, Dp::UNSPECIFIED),
                        shapes = SplitButtonDefaults::trailing_button_shapes_for(height),
                        content_padding =
                            SplitButtonDefaults::trailing_button_content_padding_for(height),
                        content = |_: RowScope| {
                            Icon!(icons::filled::keyboard_arrow_down(), Some("Trailing Icon"));
                        }
                    );
                }
            );
        });
    });
    let (button, icon, text) =
        (bounds(&mut ui, LEADING), bounds(&mut ui, "icon"), bounds(&mut ui, "text"));
    (icon.left - button.left, text.left - icon.right, button.right - text.right)
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_extraSmall_positioning
#[test]
fn an_extra_small_leading_button_is_12_and_10_dp_in() {
    assert_eq!(
        leading_spaces(SplitButtonDefaults::EXTRA_SMALL_CONTAINER_HEIGHT),
        (12.0, 4.0, 10.0)
    );
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_small_positioning
#[test]
fn a_small_leading_button_is_16_and_12_dp_in() {
    assert_eq!(leading_spaces(SplitButtonDefaults::SMALL_CONTAINER_HEIGHT), (16.0, 8.0, 12.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_medium_positioning
#[test]
fn a_medium_leading_button_is_24_dp_in() {
    assert_eq!(leading_spaces(SplitButtonDefaults::MEDIUM_CONTAINER_HEIGHT), (24.0, 8.0, 24.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_large_positioning
#[test]
fn a_large_leading_button_is_48_dp_in() {
    assert_eq!(leading_spaces(SplitButtonDefaults::LARGE_CONTAINER_HEIGHT), (48.0, 12.0, 48.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, filledSplitButton_extraLarge_positioning
#[test]
fn an_extra_large_leading_button_is_64_dp_in() {
    assert_eq!(
        leading_spaces(SplitButtonDefaults::EXTRA_LARGE_CONTAINER_HEIGHT),
        (64.0, 16.0, 64.0)
    );
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, splitButton_leadingFillMaxWidth_horizontalScroll_noCrash
#[test]
fn a_leading_button_filling_an_unbounded_row_still_shows() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            Row!(modifier = Modifier.horizontal_scroll(remember_scroll_state(0)), {
                SplitButtonLayout!(
                    leading_button = || {
                        SplitButtonLeadingButton!(
                            || {},
                            modifier = Modifier.fill_max_width(),
                            content = |_: RowScope| Text!("Label")
                        );
                    },
                    trailing_button = || {
                        SplitButtonTrailingButton!(
                            || {},
                            content = |_: RowScope| {
                                Icon!(icons::filled::keyboard_arrow_down(), Some("Trailing Icon"));
                            }
                        );
                    }
                );
            });
        });
    });
    ui.on_node_with_text("Label").assert_is_displayed();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, leadingButton_standalone_doesNotStretchHeight
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, trailingButton_standalone_doesNotStretchHeight
#[test]
fn a_button_on_its_own_is_40_dp_high() {
    for leading in [true, false] {
        let mut ui = compose_test(move || {
            MaterialTheme!({
                Box!(modifier = Modifier.height(200.dp()), {
                    if leading {
                        SplitButtonLeadingButton!(
                            || {},
                            modifier = Modifier.test_tag("button"),
                            content = |_: RowScope| Text!("Label")
                        );
                    } else {
                        SplitButtonTrailingButton!(
                            || {},
                            modifier = Modifier.test_tag("button"),
                            content = |_: RowScope| {
                                Icon!(icons::filled::keyboard_arrow_down(), None::<&str>);
                            }
                        );
                    }
                });
            });
        });
        ui.on_node_with_tag("button").assert_height_is_equal_to(40.dp());
    }
}

/// A split button whose layout is `layout_height` high if any, its leading or trailing button
/// `own` high: the two heights.
fn heights(layout_height: Option<f32>, leading: Option<f32>, trailing: Option<f32>) -> (f32, f32) {
    let mut ui = compose_test(move || {
        MaterialTheme!({
            let height = |modifier: Modifier, own: Option<f32>| match own {
                Some(own) => modifier.height(own.dp()),
                None => modifier,
            };
            SplitButtonLayout!(
                leading_button = move || {
                    SplitButtonLeadingButton!(
                        || {},
                        modifier = height(Modifier.test_tag(LEADING), leading),
                        content = |_: RowScope| Text!("Label")
                    );
                },
                trailing_button = move || {
                    SplitButtonTrailingButton!(
                        || {},
                        modifier = height(Modifier.test_tag(TRAILING), trailing),
                        content = |_: RowScope| {
                            Icon!(icons::filled::keyboard_arrow_down(), None::<&str>);
                        }
                    );
                },
                modifier = layout_height.map_or(Modifier, |it| Modifier.height(it.dp()))
            );
        });
    });
    (bounds(&mut ui, LEADING).height(), bounds(&mut ui, TRAILING).height())
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, splitButton_stretchesButtonsToMaxHeight
#[test]
fn a_taller_layout_stretches_both_buttons() {
    assert_eq!(heights(Some(100.0), None, None), (100.0, 100.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, splitButton_trailingMatchLeadingHeight
#[test]
fn the_trailing_button_matches_a_taller_leading_one() {
    assert_eq!(heights(None, Some(100.0), None), (100.0, 100.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SplitButtonTest.kt, splitButton_leadingMatchTrailingHeight
#[test]
fn the_leading_button_matches_a_taller_trailing_one() {
    assert_eq!(heights(None, None, Some(100.0)), (100.0, 100.0));
}

/// A trailing button shaped `shape` with 20 dp each side around a text, in `direction`: the text's
/// start and end from the button's.
fn optically_centered(shape: Shape, direction: LayoutDirection) -> (f32, f32) {
    let mut ui = compose_test(move || {
        let shape = shape.clone();
        MaterialTheme!({
            let shape = shape.clone();
            CompositionLocalProvider(
                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                || {
                    SplitButtonTrailingButton!(
                        || {},
                        modifier = Modifier.test_tag(TRAILING),
                        shapes = SplitButtonShapes {
                            shape: shape.clone(),
                            pressed_shape: None,
                            checked_shape: None
                        },
                        content_padding = PaddingValues::symmetric(20.dp(), 0.dp()),
                        content = |_: RowScope| Text!("Test", modifier = Modifier.test_tag("text"))
                    );
                },
            );
        });
    });
    let (button, text) = (bounds(&mut ui, TRAILING), bounds(&mut ui, "text"));
    (text.left - button.left, button.right - text.right)
}

/// Rounded by 20 dp at the start alone.
fn round_start() -> Shape {
    let (round, square) = (CornerSize::Dp(20.dp()), CornerSize::Dp(0.dp()));
    Shape::Rounded([round, square, square, round])
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/HorizontalCenterOpticallyTest.kt, horizontalCenterOptically_contentPadding_asymmetricShape
#[test]
fn content_moves_toward_the_square_end_by_a_ninth_of_the_rounding() {
    // 11 % of 20 dp is 2.2 dp, 2 pixels at 1 pixel a dp.
    assert_eq!(optically_centered(round_start(), LayoutDirection::Ltr), (22.0, 18.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/HorizontalCenterOpticallyTest.kt, horizontalCenterOptically_contentPadding_symmetricShape
#[test]
fn content_in_a_symmetric_shape_stays_centered() {
    let square = Shape::Rounded([CornerSize::Dp(0.dp()); 4]);
    assert_eq!(optically_centered(square, LayoutDirection::Ltr), (20.0, 20.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/HorizontalCenterOpticallyTest.kt, horizontalCenterOptically_contentPadding_asymmetricShape_rtl
#[test]
fn content_moves_toward_the_square_end_right_to_left() {
    assert_eq!(optically_centered(round_start(), LayoutDirection::Rtl), (18.0, 22.0));
}
