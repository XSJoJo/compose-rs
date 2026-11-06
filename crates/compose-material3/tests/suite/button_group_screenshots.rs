//! Compose's `ButtonGroupScreenshotTest.kt`, its goldens as what the layout and the display list
//! say: the group's buttons and their colors, the connected groups' corners, the widths as each
//! button is pressed, the overflow indicator after what fits and its menu of the rest. Also what
//! the goldens show without a test of their own: the default indicator and the items' menu entries.

use compose_foundation::layout::*;
use compose_material3::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::DrawOp;
use compose_ui::{Alignment, Modifier, Outline, PaintStyle};

const LABELS: [&str; 5] = ["A", "B", "C", "D", "E"];

fn text_bounds(ui: &mut ComposeTestRule, text: &str) -> compose_ui::Rect {
    ui.on_node_with_text(text).fetch_semantics_snapshot().bounds
}

/// Five clickable items in `scheme`.
fn five(scheme: ColorScheme) -> ComposeTestRule {
    compose_test(move || {
        MaterialTheme!(color_scheme = scheme, {
            ButtonGroup!(
                |_| {},
                vertical_alignment = Alignment::TOP,
                content = |scope: ButtonGroupScope| {
                    for label in LABELS {
                        scope.clickable_item(|| {}, label);
                    }
                }
            );
        });
    })
}

/// The fills the display list draws.
fn fills(ui: &mut ComposeTestRule) -> Vec<compose_ui::Color> {
    let list = ui.host().display_list();
    list.ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::DrawRoundRect(_, _, paint) if paint.style == PaintStyle::Fill => {
                Some(paint.color)
            }
            _ => None,
        })
        .collect()
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_darkTheme
#[test]
fn a_groups_items_are_filled_buttons_in_a_row() {
    for scheme in [light_color_scheme(), dark_color_scheme()] {
        let mut ui = five(scheme);
        assert_eq!(fills(&mut ui), vec![scheme.primary; 5]);
        let bounds = LABELS.map(|label| text_bounds(&mut ui, label));
        for pair in bounds.windows(2) {
            assert_eq!(pair[1].left - pair[0].right, 12.0);
            assert_eq!(pair[1].top, pair[0].top);
        }
    }
}

/// The corners of each of three connected toggle buttons, checked as `checked` says.
fn connected_corners(checked: [bool; 3]) -> Vec<[f32; 4]> {
    let mut ui = compose_test(move || {
        MaterialTheme!({
            Row!(
                horizontal_arrangement =
                    Arrangement::spaced_by(ButtonGroupDefaults::CONNECTED_SPACE_BETWEEN),
                {
                    let shapes = [
                        ButtonGroupDefaults::connected_leading_button_shapes(),
                        ButtonGroupDefaults::connected_middle_button_shapes(),
                        ButtonGroupDefaults::connected_trailing_button_shapes(),
                    ];
                    for (index, (label, shapes)) in
                        ["Work", "Restaurant", "Coffee"].into_iter().zip(shapes).enumerate()
                    {
                        ToggleButton!(
                            checked[index],
                            |_| {},
                            shapes = shapes,
                            |_: RowScope| Text!(label)
                        );
                    }
                }
            );
        });
    });
    let list = ui.host().display_list();
    let layers = list.groups.iter().filter_map(|group| group.layer.as_ref());
    layers
        .filter(|layer| layer.clip)
        .filter_map(|layer| match &layer.outline {
            Some(Outline::Rounded(_, radii)) => Some(*radii),
            _ => None,
        })
        .collect()
}

/// Round outside, the small corners inside (top left, top right, bottom right, bottom left).
const LEADING: [f32; 4] = [20.0, 8.0, 8.0, 20.0];
const MIDDLE: [f32; 4] = [8.0; 4];
const TRAILING: [f32; 4] = [8.0, 20.0, 20.0, 8.0];
const PILL: [f32; 4] = [20.0; 4];

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, connectedButtonGroup_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, connectedButtonGroup_darkTheme
#[test]
fn a_connected_group_is_round_at_its_ends_and_square_inside() {
    assert_eq!(connected_corners([false; 3]), vec![LEADING, MIDDLE, TRAILING]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, connectedButtonGroup_startSelected_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, connectedButtonGroup_middleSelected_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, connectedButtonGroup_endSelected_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, connectedButtonGroup_allSelected_lightTheme
#[test]
fn a_connected_groups_checked_buttons_are_pills() {
    assert_eq!(connected_corners([true, false, false]), vec![PILL, MIDDLE, TRAILING]);
    assert_eq!(connected_corners([false, true, false]), vec![LEADING, PILL, TRAILING]);
    assert_eq!(connected_corners([false, false, true]), vec![LEADING, MIDDLE, PILL]);
    assert_eq!(connected_corners([true; 3]), vec![PILL; 3]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_firstPressed_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_secondPressed_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_thirdPressed_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_fourthPressed_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_fifthPressed_lightTheme
#[test]
fn a_pressed_item_grows_by_what_its_neighbours_give() {
    for pressed in 0..LABELS.len() {
        let mut ui = five(light_color_scheme());
        let before = LABELS.map(|label| text_bounds(&mut ui, label).width());
        ui.set_auto_advance(false);
        ui.on_node_with_text(LABELS[pressed]).perform_touch_input(|touch| {
            let center = touch.center();
            touch.down(center);
        });
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        ui.advance_time_by(std::time::Duration::from_millis(500));
        ui.wait_for_idle();
        let after = LABELS.map(|label| text_bounds(&mut ui, label).width());
        // A middle one takes half its 15 % from each side, an end one all of it from its one
        // neighbour, none giving more than its end padding, 24 dp.
        let neighbours: Vec<usize> = [pressed.checked_sub(1), Some(pressed + 1)]
            .into_iter()
            .flatten()
            .filter(|&index| index < LABELS.len())
            .collect();
        let share = if neighbours.len() == 2 { 0.075 } else { 0.15 };
        let given = (before[pressed] * share).min(24.0).round();
        for index in 0..LABELS.len() {
            let expected = if index == pressed {
                before[index] + given * neighbours.len() as f32
            } else if neighbours.contains(&index) {
                before[index] - given
            } else {
                before[index]
            };
            assert_eq!(after[index], expected, "{pressed} pressed, {index}: {before:?} {after:?}");
        }
    }
}

/// Ten items and an indicator: its menu's entries for those that don't fit.
fn ten_with_a_menu(scheme: ColorScheme, custom: bool) -> ComposeTestRule {
    compose_test(move || {
        MaterialTheme!(color_scheme = scheme, {
            ButtonGroup!(
                |menu: ButtonGroupMenuState| {
                    ButtonGroupOverflowIndicator!(menu, modifier = Modifier.test_tag("indicator"));
                },
                vertical_alignment = Alignment::TOP,
                content = move |scope: ButtonGroupScope| {
                    for index in 0..10 {
                        if custom {
                            scope.custom_item(
                                move || Button!(|| {}, |_: RowScope| Text!("{index}")),
                                move |_| {
                                    DropdownMenuItem!(
                                        || Text!("{index}"),
                                        || {},
                                        modifier = Modifier.test_tag(format!("{index} MenuItem"))
                                    );
                                },
                            );
                        } else {
                            scope.clickable_item(|| {}, format!("Item {index}"));
                        }
                    }
                }
            );
        });
    })
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_overflowIndicator_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_overflowIndicator_darkTheme
#[test]
fn the_indicator_stands_after_the_items_that_fit() {
    for scheme in [light_color_scheme(), dark_color_scheme()] {
        let mut ui = ten_with_a_menu(scheme, true);
        let indicator = ui.on_node_with_tag("indicator").fetch_semantics_snapshot().bounds;
        assert!(indicator.right <= 400.0, "{indicator:?}");
        let shown: Vec<usize> =
            (0..10).filter(|index| ui.count_nodes(&has_text(&format!("{index}"))) > 0).collect();
        assert!(!shown.is_empty() && shown.len() < 10, "{shown:?}");
        let last = text_bounds(&mut ui, &format!("{}", shown.len() - 1));
        // 12 dp after it, the 40 dp button centered in its 48 dp touch target, which the
        // semantics bounds leave out, as Compose's (its clickable's merging node is inside it).
        assert_eq!(indicator.left - last.right, 12.0 + 4.0);
        // Its default: three dots in the filled icon button's colors.
        assert!(fills(&mut ui).contains(&scheme.primary));
        ui.on_node_with_content_description("More options").assert_exists();
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_overflowMenuExpanded_lightTheme
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupScreenshotTest.kt, buttonGroup_overflowMenuExpanded_darkTheme
#[test]
fn the_indicators_menu_lists_the_items_that_dont_fit() {
    for scheme in [light_color_scheme(), dark_color_scheme()] {
        let mut ui = ten_with_a_menu(scheme, true);
        let shown = (0..10).filter(|index| ui.count_nodes(&has_text(&format!("{index}"))) > 0);
        let shown = shown.count();
        ui.on_node_with_tag("indicator").perform_click();
        for index in 0..10 {
            let entries = ui.count_nodes(&has_test_tag(&format!("{index} MenuItem")));
            assert_eq!(entries, usize::from(index >= shown), "{index} of {shown}");
        }
        // A second click hides it.
        ui.on_node_with_tag("indicator").perform_click();
        ui.on_node_with_tag("9 MenuItem").assert_does_not_exist();
    }
}

#[test]
fn a_clickable_items_menu_entry_clicks_it_and_closes_the_menu() {
    let clicked = std::rc::Rc::new(std::cell::Cell::new(None));
    let mut ui = compose_test({
        let clicked = clicked.clone();
        move || {
            let clicked = clicked.clone();
            MaterialTheme!({
                let clicked = clicked.clone();
                let checked = remember(|| mutable_state_of(false));
                ButtonGroup!(
                    |menu: ButtonGroupMenuState| {
                        ButtonGroupOverflowIndicator!(
                            menu,
                            modifier = Modifier.test_tag("indicator")
                        );
                    },
                    content = move |scope: ButtonGroupScope| {
                        for index in 0..9 {
                            let clicked = clicked.clone();
                            scope.clickable_item(
                                move || clicked.set(Some(index)),
                                format!("Item {index}"),
                            );
                        }
                        scope.toggleable_item(checked.get(), "Toggle", move |now| checked.set(now));
                    }
                );
            });
        }
    });
    ui.on_node_with_tag("indicator").perform_click();
    ui.on_node_with_text("Item 8").perform_click();
    assert_eq!(clicked.get(), Some(8));
    ui.on_node_with_text("Toggle").assert_does_not_exist();
    ui.on_node_with_tag("indicator").perform_click();
    ui.on_node_with_text("Toggle").perform_click();
    ui.on_node_with_text("Toggle").assert_does_not_exist();
    // Checked now: shown in the group, the toggle would be on.
    ui.on_node_with_tag("indicator").perform_click();
    ui.on_node_with_text("Toggle").assert_exists();
}

#[test]
#[should_panic(expected = "invalid weight")]
fn a_weight_of_zero_is_refused() {
    let _ui = compose_test(|| {
        MaterialTheme!({
            ButtonGroup!(
                |_| {},
                content = |scope: ButtonGroupScope| {
                    scope.clickable_item_with(|| {}, "A", None::<fn()>, Some(0.0), true);
                }
            );
        });
    });
}

#[test]
fn weighted_items_share_the_width() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            ButtonGroup!(
                |_| {},
                modifier = Modifier.width(compose_ui::Dp(312.0)),
                content = |scope: ButtonGroupScope| {
                    scope.clickable_item_with(|| {}, "A", None::<fn()>, Some(1.0), true);
                    scope.clickable_item_with(|| {}, "B", None::<fn()>, Some(2.0), true);
                }
            );
        });
    });
    // 300 dp after the 12 between them, a third and two.
    assert_eq!(text_bounds(&mut ui, "A").width(), 100.0);
    assert_eq!(text_bounds(&mut ui, "B").width(), 200.0);
}
