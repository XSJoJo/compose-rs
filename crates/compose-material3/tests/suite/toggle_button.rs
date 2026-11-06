//! Compose's `ToggleButtonTest.kt`: the toggle buttons' semantics, where their content sits at each
//! size, their default colors, how their shapes move, a border that changes with the state, and
//! their sizes.
//!
//! `hasRoundedCornerShapes` is internal in Compose: what it decides, whether the shape moves to its
//! new corners or jumps there, is what the two shape tests here check.

use compose_foundation::layout::*;
use compose_material3::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    BorderStroke, Color, CornerSize, Dp, Modifier, Offset, Outline, Press, PressInteraction, Role,
    Shape, UnitExt,
};

const BUTTON: &str = "toggle button";
const TEXT: &str = "text";
const ICON: &str = "icon";

fn bounds(ui: &mut ComposeTestRule, tag: &str) -> compose_ui::Rect {
    ui.on_node_unmerged(has_test_tag(tag)).fetch_semantics_snapshot().bounds
}

fn role(ui: &mut ComposeTestRule) -> Option<Role> {
    let node = ui.on_node_with_tag(BUTTON).fetch_semantics_snapshot();
    node.config.get(&compose_ui::SemanticsProperties::ROLE).copied()
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, default_semantics
#[test]
fn a_toggle_button_is_a_checkbox_a_click_turns_on() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            let checked = remember(|| mutable_state_of(false));
            Box!({
                ToggleButton!(
                    checked.get(),
                    move |now| checked.set(now),
                    modifier = Modifier.test_tag(BUTTON),
                    |_: RowScope| Text!("test")
                );
            });
        });
    });
    assert_eq!(role(&mut ui), Some(Role::Checkbox));
    ui.on_node_with_tag(BUTTON).assert_is_enabled().assert_is_off().perform_click().assert_is_on();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, disabledSemantics
#[test]
fn a_disabled_toggle_button_stays_off_when_clicked() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            let checked = remember(|| mutable_state_of(false));
            Box!({
                ToggleButton!(
                    checked.get(),
                    move |now| checked.set(now),
                    modifier = Modifier.test_tag(BUTTON),
                    enabled = false,
                    |_: RowScope| Text!("test")
                );
            });
        });
    });
    assert_eq!(role(&mut ui), Some(Role::Checkbox));
    ui.on_node_with_tag(BUTTON)
        .assert_is_not_enabled()
        .assert_is_off()
        .perform_click()
        .assert_is_off();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_contentPositioning
#[test]
fn a_toggle_buttons_text_is_16_dp_from_each_side() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            Box!({
                ToggleButton!(
                    false,
                    |_| {},
                    modifier = Modifier.test_tag(BUTTON),
                    enabled = false,
                    |_: RowScope| Text!("test", modifier = Modifier.test_tag(TEXT))
                );
            });
        });
    });
    let (button, text) = (bounds(&mut ui, BUTTON), bounds(&mut ui, TEXT));
    assert_eq!(text.left - button.left, 16.0);
    assert_eq!(button.right - text.right, 16.0);
}

/// A toggle button of `size` with an icon (the edit icon if `material_icon`, else an empty box) and
/// a label: the icon's start from the button's, the gap to the text and the text's end from the
/// button's.
fn icon_and_text_spaces(size: Option<ToggleButtonSize>, material_icon: bool) -> (f32, f32, f32) {
    let mut ui = compose_test(move || {
        MaterialTheme!({
            Box!({
                let icon = move || {
                    if material_icon {
                        Icon!(
                            icons::filled::edit(),
                            Some("Localized description"),
                            modifier = Modifier.test_tag(ICON)
                        );
                    } else {
                        Box!(modifier = Modifier.test_tag(ICON));
                    }
                };
                let label = |_: RowScope| Text!("Label", modifier = Modifier.test_tag(TEXT));
                match size {
                    Some(size) => ToggleButton!(
                        false,
                        |_| {},
                        modifier = Modifier.test_tag(BUTTON),
                        button_size = size,
                        icon = icon,
                        content = label
                    ),
                    None => ToggleButton!(
                        false,
                        |_| {},
                        modifier = Modifier.test_tag(BUTTON),
                        enabled = false,
                        icon = icon,
                        content = label
                    ),
                }
            });
        });
    });
    let (button, icon, text) =
        (bounds(&mut ui, BUTTON), bounds(&mut ui, ICON), bounds(&mut ui, TEXT));
    (icon.left - button.left, text.left - icon.right, button.right - text.right)
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButtonWithIcon_contentPositioning
#[test]
fn a_toggle_buttons_icon_is_16_dp_in_and_8_dp_before_its_text() {
    assert_eq!(icon_and_text_spaces(None, false), (16.0, 8.0, 16.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_XSmall_positioning
#[test]
fn an_extra_small_toggle_buttons_content_is_12_dp_in_its_icon_4_dp_before_its_text() {
    assert_eq!(icon_and_text_spaces(Some(ToggleButtonSize::EXTRA_SMALL), true), (12.0, 4.0, 12.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_Medium_positioning
#[test]
fn a_medium_toggle_buttons_content_is_24_dp_in_its_icon_8_dp_before_its_text() {
    assert_eq!(icon_and_text_spaces(Some(ToggleButtonSize::MEDIUM), true), (24.0, 8.0, 24.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_Large_positioning
#[test]
fn a_large_toggle_buttons_content_is_48_dp_in_its_icon_12_dp_before_its_text() {
    assert_eq!(icon_and_text_spaces(Some(ToggleButtonSize::LARGE), true), (48.0, 12.0, 48.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_XLarge_positioning
#[test]
fn an_extra_large_toggle_buttons_content_is_64_dp_in_its_icon_16_dp_before_its_text() {
    assert_eq!(icon_and_text_spaces(Some(ToggleButtonSize::EXTRA_LARGE), true), (64.0, 16.0, 64.0));
}

/// What `read` returns in a light theme's composition.
fn in_theme<T: 'static>(read: fn() -> T) -> T {
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let _ui = compose_test({
        let slot = slot.clone();
        move || {
            let slot = slot.clone();
            MaterialTheme!({
                *slot.borrow_mut() = Some(read());
            });
        }
    });
    slot.take().expect("composed")
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_defaultColors
#[test]
fn a_toggle_buttons_colors_are_the_filled_button_tokens() {
    let scheme = light_color_scheme();
    assert_eq!(
        in_theme(ToggleButtonDefaults::colors),
        ToggleButtonColors {
            container_color: scheme.surface_container,
            content_color: scheme.on_surface_variant,
            disabled_container_color: scheme.on_surface.with_alpha(0.1),
            disabled_content_color: scheme.on_surface_variant.with_alpha(0.38),
            checked_container_color: scheme.primary,
            checked_content_color: scheme.on_primary,
        }
    );
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, elevatedToggleButton_defaultColors
#[test]
fn an_elevated_toggle_buttons_colors_are_the_elevated_button_tokens() {
    let scheme = light_color_scheme();
    assert_eq!(
        in_theme(ElevatedToggleButtonDefaults::colors),
        ToggleButtonColors {
            container_color: scheme.surface_container_low,
            content_color: scheme.primary,
            disabled_container_color: scheme.on_surface.with_alpha(0.1),
            disabled_content_color: scheme.on_surface_variant.with_alpha(0.38),
            checked_container_color: scheme.primary,
            checked_content_color: scheme.on_primary,
        }
    );
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, tonalToggleButton_defaultColors
#[test]
fn a_tonal_toggle_buttons_colors_are_the_tonal_button_tokens() {
    let scheme = light_color_scheme();
    assert_eq!(
        in_theme(FilledTonalToggleButtonDefaults::colors),
        ToggleButtonColors {
            container_color: scheme.secondary_container,
            content_color: scheme.on_secondary_container,
            disabled_container_color: scheme.on_surface.with_alpha(0.1),
            disabled_content_color: scheme.on_surface_variant.with_alpha(0.38),
            checked_container_color: scheme.secondary,
            checked_content_color: scheme.on_secondary,
        }
    );
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, outlinedToggleButton_defaultColors
#[test]
fn an_outlined_toggle_buttons_colors_are_the_outlined_button_tokens() {
    let scheme = light_color_scheme();
    assert_eq!(
        in_theme(OutlinedToggleButtonDefaults::colors),
        ToggleButtonColors {
            container_color: Color::TRANSPARENT,
            content_color: scheme.on_surface_variant,
            disabled_container_color: scheme.outline_variant.with_alpha(0.1),
            disabled_content_color: scheme.on_surface_variant.with_alpha(0.38),
            checked_container_color: scheme.inverse_surface,
            checked_content_color: scheme.inverse_on_surface,
        }
    );
}

fn rounded(dp: f32) -> Shape {
    Shape::Rounded([CornerSize::Dp(dp.dp()); 4])
}

/// The corner a toggle button with `shapes` clips to at rest, then the frame after a press and
/// once it has settled.
fn corners_when_pressed(shapes: ToggleButtonShapes) -> (f32, f32, f32) {
    let source = compose_ui::MutableInteractionSource::new();
    let mut ui = compose_test({
        let source = source.clone();
        move || {
            let (source, shapes) = (source.clone(), shapes.clone());
            MaterialTheme!({
                ToggleButton!(
                    false,
                    |_| {},
                    shapes = shapes.clone(),
                    interaction_source = Some(source.clone()),
                    |_: RowScope| Text!("test")
                );
            });
        }
    });
    let clip = |ui: &mut ComposeTestRule| {
        let list = ui.host().display_list();
        let layers = list.groups.iter().filter_map(|group| group.layer.as_ref());
        match layers.filter(|layer| layer.clip).find_map(|layer| layer.outline.clone()) {
            Some(Outline::Rounded(_, radii)) => radii[0],
            Some(Outline::Rectangle(_)) => 0.0,
            other => panic!("{other:?}"),
        }
    };
    let resting = clip(&mut ui);
    ui.set_auto_advance(false);
    source.try_emit(PressInteraction::Press(Press::new(Offset::ZERO)));
    for _ in 0..4 {
        ui.advance_time_by_frame();
    }
    let moving = clip(&mut ui);
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    (resting, moving, clip(&mut ui))
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, buttonShapes_AllRounded_hasRoundedShapesIsTrue
#[test]
fn rounded_shapes_move_from_one_to_the_next() {
    let shapes = ToggleButtonShapes {
        shape: rounded(10.0),
        pressed_shape: rounded(2.0),
        checked_shape: rounded(4.0),
    };
    let (resting, moving, pressed) = corners_when_pressed(shapes);
    assert_eq!((resting, pressed), (10.0, 2.0));
    assert!(moving < 10.0 && moving > 2.0, "{moving}");
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, buttonShapes_mixedShapes_hasRoundedShapesIsFalse
#[test]
fn shapes_not_all_with_corners_jump_from_one_to_the_next() {
    let shapes = ToggleButtonShapes {
        shape: compose_ui::RECTANGLE_SHAPE,
        pressed_shape: rounded(10.0),
        checked_shape: rounded(4.0),
    };
    assert_eq!(corners_when_pressed(shapes), (0.0, 10.0, 10.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_customStatefulBorder_widthUpdatedWhenChecked
#[test]
fn a_border_given_for_the_state_follows_it() {
    let given = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut ui = compose_test({
        let given = given.clone();
        move || {
            let given = given.clone();
            MaterialTheme!({
                let checked = remember(|| mutable_state_of(false));
                let border = if checked.get() {
                    BorderStroke::new(2.dp(), Color::RED)
                } else {
                    BorderStroke::new(1.dp(), Color::BLUE)
                };
                *given.borrow_mut() = Some(border.clone());
                ToggleButton!(
                    checked.get(),
                    move |now| checked.set(now),
                    border = Some(border),
                    modifier = Modifier.test_tag(BUTTON),
                    |_: RowScope| Text!("test")
                );
            });
        }
    });
    assert_eq!(given.borrow().clone(), Some(BorderStroke::new(1.dp(), Color::BLUE)));
    ui.on_node_with_tag(BUTTON).perform_click().assert_is_on();
    assert_eq!(given.borrow().clone(), Some(BorderStroke::new(2.dp(), Color::RED)));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ToggleButtonTest.kt, toggleButton_buttonSize_sizesAndDimensions
#[test]
fn each_size_is_its_height_and_its_paddings_icon_and_content_wide() {
    let sizes = [
        ToggleButtonSize::EXTRA_SMALL,
        ToggleButtonSize::SMALL,
        ToggleButtonSize::MEDIUM,
        ToggleButtonSize::LARGE,
        ToggleButtonSize::EXTRA_LARGE,
    ];
    let content_width = 50.0;
    let mut ui = compose_test(move || {
        MaterialTheme!({
            CompositionLocalProvider(
                LOCAL_MINIMUM_INTERACTIVE_COMPONENT_SIZE.provides(0.dp()),
                || {
                    Column!({
                        for (index, size) in sizes.into_iter().enumerate() {
                            ToggleButton!(
                                true,
                                |_| {},
                                button_size = size,
                                icon = || Box!(modifier = Modifier.fill_max_size()),
                                modifier = Modifier.test_tag(format!("{BUTTON} {index}")),
                                |_: RowScope| Box!(modifier = Modifier.size((50.dp(), 10.dp())))
                            );
                        }
                    });
                },
            );
        });
    });
    for (index, size) in sizes.into_iter().enumerate() {
        let button = bounds(&mut ui, &format!("{BUTTON} {index}"));
        let padding = ToggleButtonDefaults::content_padding_for(size, true, false);
        let expected = padding.start.0
            + ButtonDefaults::icon_size_for(size.height()).0
            + ButtonDefaults::icon_spacing_for(size.height()).0
            + content_width
            + padding.end.0;
        assert!((button.height() - size.height().0).abs() <= 1.0, "{index}: {button:?}");
        assert!((button.width() - expected).abs() <= 1.0, "{index}: {button:?} {expected}");
    }
    let _: Dp = ToggleButtonDefaults::MIN_HEIGHT;
}
