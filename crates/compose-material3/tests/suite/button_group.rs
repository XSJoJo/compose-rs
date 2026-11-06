//! Compose's `ButtonGroupTest.kt`: where a group places its buttons, how a pressed one grows and
//! its neighbours give way (within what each can give), its overflow indicator and menu, its
//! arrangements and alignments, right to left.
//!
//! The pressed widths are measured at 3 pixels a dp, near the density Compose's run at, where the
//! growth's rounding to pixels stays within its half-dp tolerance.

use compose_foundation::layout::*;
use compose_foundation::{ScrollModifiers, remember_scroll_state};
use compose_material3::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Alignment, Density, Dp, IntSize, LayoutDirection, Modifier, MutableInteractionSource, Offset,
    PaddingValues, TextOverflow, UnitExt,
};

const WRAPPER: &str = "wrapper";
const INDICATOR: &str = "overflow indicator";
const TAGS: [&str; 4] = ["A", "B", "C", "D"];

fn bounds(ui: &mut ComposeTestRule, tag: &str) -> compose_ui::Rect {
    ui.on_node_unmerged(has_test_tag(tag)).fetch_semantics_snapshot().bounds
}

fn text_bounds(ui: &mut ComposeTestRule, text: &str) -> compose_ui::Rect {
    ui.on_node_with_text(text).fetch_semantics_snapshot().bounds
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, default_positioning
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, differentHorizontalSpacing_positioning
#[test]
fn a_groups_buttons_are_12_dp_apart_and_the_group_wraps_them() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            Box!(modifier = Modifier.test_tag(WRAPPER), {
                ButtonGroup!(
                    |_| {},
                    vertical_alignment = Alignment::TOP,
                    content = |scope: ButtonGroupScope| {
                        for label in TAGS {
                            scope.clickable_item(|| {}, label);
                        }
                    }
                );
            });
        });
    });
    let wrapper = bounds(&mut ui, WRAPPER);
    let [a, b, c, d] = TAGS.map(|tag| text_bounds(&mut ui, tag));
    // The text's bounds are the buttons': their semantics merge.
    assert_eq!(a.left - wrapper.left, 0.0);
    assert_eq!(b.left - a.right, 12.0);
    assert_eq!(c.left - b.right, 12.0);
    assert_eq!(d.left - c.right, 12.0);
    assert_eq!(wrapper.right - d.right, 0.0);
}

/// Four 75 dp buttons whose widths follow their presses, `ratio` the group's expanded ratio, at 3
/// pixels a dp; `pressed` is pressed and held for 200 ms.
fn pressed_widths(ratio: f32, pressed: usize) -> [f32; 4] {
    let sources: [MutableInteractionSource; 4] =
        std::array::from_fn(|_| MutableInteractionSource::new());
    let mut ui = compose_test_with(
        IntSize::new(1200, 2400),
        Density { density: 3.0, font_scale: 1.0 },
        Box::new(compose_ui::platform::FixedTextMeasurer::new()),
        {
            let sources = sources.clone();
            move || {
                let sources = sources.clone();
                MaterialTheme!({
                    let sources = sources.clone();
                    Box!(modifier = Modifier.test_tag(WRAPPER), {
                        let sources = sources.clone();
                        ButtonGroup!(
                            |_| {},
                            expanded_ratio = ratio,
                            vertical_alignment = Alignment::TOP,
                            content = move |scope: ButtonGroupScope| {
                                for (tag, source) in TAGS.into_iter().zip(sources.clone()) {
                                    let inner = scope.clone();
                                    scope.custom_item(
                                        move || {
                                            Button!(
                                                || {},
                                                modifier = Modifier
                                                    .width(75.dp())
                                                    .animate_width(&inner, &source)
                                                    .test_tag(tag),
                                                interaction_source = Some(source.clone()),
                                                |_: RowScope| Text!(tag)
                                            );
                                        },
                                        |_| {},
                                    );
                                }
                            }
                        );
                    });
                });
            }
        },
    );
    ui.set_auto_advance(false);
    ui.on_node_with_tag(TAGS[pressed]).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    ui.advance_time_by(std::time::Duration::from_millis(200));
    ui.wait_for_idle();
    TAGS.map(|tag| bounds(&mut ui, tag).width() / 3.0)
}

/// Each width within Compose's half-dp tolerance.
fn assert_widths(actual: [f32; 4], expected: [f32; 4]) {
    for (actual, expected) in actual.iter().zip(expected) {
        assert!((actual - expected).abs() <= 0.5, "{actual:?} against {expected:?}");
    }
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, default_firstPressed_buttonSizing
#[test]
fn a_pressed_first_button_grows_into_the_second() {
    let (grown, given) = (75.0 * 1.15, 75.0 * 0.85);
    assert_widths(pressed_widths(0.15, 0), [grown, given, 75.0, 75.0]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, default_secondPressed_buttonSizing
#[test]
fn a_pressed_second_button_grows_into_both_neighbours() {
    let (grown, given) = (75.0 * 1.15, 75.0 * (1.0 - 0.075));
    assert_widths(pressed_widths(0.15, 1), [given, grown, given, 75.0]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, default_thirdPressed_buttonSizing
#[test]
fn a_pressed_third_button_grows_into_both_neighbours() {
    let (grown, given) = (75.0 * 1.15, 75.0 * (1.0 - 0.075));
    assert_widths(pressed_widths(0.15, 2), [75.0, given, grown, given]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, default_fourthPressed_buttonSizing
#[test]
fn a_pressed_last_button_grows_into_the_one_before() {
    let (grown, given) = (75.0 * 1.15, 75.0 * 0.85);
    assert_widths(pressed_widths(0.15, 3), [75.0, 75.0, given, grown]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, customAnimateFraction_firstPressed_buttonSizing
#[test]
fn a_larger_ratio_grows_the_first_button_further() {
    let (grown, given) = (75.0 * 1.3, 75.0 * 0.7);
    assert_widths(pressed_widths(0.3, 0), [grown, given, 75.0, 75.0]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, customAnimateFraction_secondPressed_buttonSizing
#[test]
fn a_larger_ratio_grows_the_second_button_further() {
    let (grown, given) = (75.0 * 1.3, 75.0 * 0.85);
    assert_widths(pressed_widths(0.3, 1), [given, grown, given, 75.0]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, customAnimateFraction_thirdPressed_buttonSizing
#[test]
fn a_larger_ratio_grows_the_third_button_further() {
    let (grown, given) = (75.0 * 1.3, 75.0 * 0.85);
    assert_widths(pressed_widths(0.3, 2), [75.0, given, grown, given]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, customAnimateFraction_fourthPressed_buttonSizing
#[test]
fn a_larger_ratio_grows_the_last_button_further() {
    let (grown, given) = (75.0 * 1.3, 75.0 * 0.7);
    assert_widths(pressed_widths(0.3, 3), [75.0, 75.0, given, grown]);
}

/// A group of `count` clickable items labelled `Item {i}` (`{i}` alone if not `long`), with an
/// icon button as its indicator, in a box of `width` (none if `None`).
fn group_of(count: usize, long: bool, width: Option<Dp>) -> ComposeTestRule {
    compose_test(move || {
        MaterialTheme!({
            let modifier = match width {
                Some(width) => Modifier.width(width).test_tag(WRAPPER),
                None => Modifier.test_tag(WRAPPER),
            };
            Box!(modifier = modifier, {
                ButtonGroup!(
                    |_| IconButton!(|| {}, modifier = Modifier.test_tag(INDICATOR), {}),
                    modifier = if width.is_some() { Modifier.fill_max_width() } else { Modifier },
                    vertical_alignment = Alignment::TOP,
                    content = move |scope: ButtonGroupScope| {
                        for index in 0..count {
                            let label =
                                if long { format!("Item {index}") } else { format!("{index}") };
                            scope.clickable_item(|| {}, label);
                        }
                    }
                );
            });
        });
    })
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, overflowIndicator_tooManyItems_Exists
#[test]
fn too_many_items_show_the_overflow_indicator() {
    group_of(100, false, None).on_node_with_tag(INDICATOR).assert_is_displayed();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, overflowIndicator_fitsOnScreen_doesNotExists
#[test]
fn items_that_fit_hide_the_overflow_indicator() {
    group_of(4, false, None).on_node_with_tag(INDICATOR).assert_is_not_displayed();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, overflowIndicator_fixedWidthConstraints_doesNotCrash
#[test]
fn a_fixed_width_overflows_into_the_indicator() {
    group_of(10, true, Some(150.dp())).on_node_with_tag(INDICATOR).assert_is_displayed();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, overflowMenu_tooManyItems_exists
#[test]
fn the_indicator_opens_a_menu_of_the_items_that_dont_fit() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            Box!(modifier = Modifier.test_tag(WRAPPER), {
                ButtonGroup!(
                    |menu: ButtonGroupMenuState| {
                        IconButton!(
                            move || if menu.is_showing() { menu.dismiss() } else { menu.show() },
                            modifier = Modifier.test_tag(INDICATOR),
                            {}
                        );
                    },
                    vertical_alignment = Alignment::TOP,
                    content = |scope: ButtonGroupScope| {
                        for index in 0..10 {
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
                        }
                    }
                );
            });
        });
    });
    ui.on_node_with_tag("9 MenuItem").assert_does_not_exist();
    ui.on_node_with_tag(INDICATOR).perform_click();
    ui.on_node_with_tag("9 MenuItem").assert_is_displayed();
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, horizontalScroll_handlingScroll_buttonDisplay
#[test]
fn a_group_in_a_horizontal_scroll_shows_all_its_items_as_it_scrolls() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            Box!(modifier = Modifier.test_tag(WRAPPER), {
                ButtonGroup!(
                    |_| {},
                    modifier = Modifier.horizontal_scroll(remember_scroll_state(0)),
                    vertical_alignment = Alignment::TOP,
                    content = |scope: ButtonGroupScope| {
                        for index in 0..10 {
                            scope.clickable_item(|| {}, format!("{index}"));
                        }
                    }
                );
            });
        });
    });
    ui.on_node_with_text("9").assert_is_not_displayed();
    ui.on_node_with_tag(WRAPPER).perform_touch_input(|touch| {
        let center = touch.center();
        touch.swipe(center, Offset::new(center.x - 1500.0, center.y), 100);
    });
    ui.on_node_with_text("9").assert_is_displayed();
}

/// Three items in a full-width group arranged by `arrangement`: whether `fraction` of the root's
/// width falls between the first's start and the last's end.
fn arranged_over(arrangement: HorizontalArrangement, fraction: f32) -> bool {
    let mut ui = compose_test(move || {
        let arrangement = arrangement.clone();
        MaterialTheme!({
            let arrangement = arrangement.clone();
            Box!(modifier = Modifier.test_tag(WRAPPER), {
                ButtonGroup!(
                    |_| {},
                    modifier = Modifier.fill_max_width(),
                    horizontal_arrangement = arrangement.clone(),
                    vertical_alignment = Alignment::TOP,
                    content = |scope: ButtonGroupScope| {
                        for index in 0..3 {
                            scope.clickable_item(|| {}, format!("{index}"));
                        }
                    }
                );
            });
        });
    });
    let (first, last) = (text_bounds(&mut ui, "0"), text_bounds(&mut ui, "2"));
    (first.left..=last.right).contains(&(400.0 * fraction))
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, horizontalArrangement_startArrangement_buttonGroupPositioning
#[test]
fn a_group_arranged_at_the_start_covers_a_third_of_the_width() {
    assert!(arranged_over(Arrangement::START, 1.0 / 3.0));
    assert!(!arranged_over(Arrangement::START, 2.0 / 3.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, horizontalArrangement_centerArrangement_buttonGroupPositioning
#[test]
fn a_centered_group_covers_the_middle() {
    assert!(arranged_over(Arrangement::CENTER.into(), 0.5));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, horizontalArrangement_endArrangement_buttonGroupPositioning
#[test]
fn a_group_arranged_at_the_end_covers_two_thirds_of_the_width() {
    assert!(arranged_over(Arrangement::END, 2.0 / 3.0));
    assert!(!arranged_over(Arrangement::END, 1.0 / 3.0));
}

/// Two buttons of `first` and `second` dp high (the first aligned by itself if `own`), in a group
/// aligned by `alignment`: their tops.
fn tops(
    alignment: compose_ui::VerticalAlignment,
    first: f32,
    second: f32,
    own: Option<compose_ui::VerticalAlignment>,
) -> (f32, f32) {
    let mut ui = compose_test(move || {
        MaterialTheme!({
            Box!({
                ButtonGroup!(
                    |_| {},
                    vertical_alignment = alignment,
                    content = move |scope: ButtonGroupScope| {
                        let inner = scope.clone();
                        scope.custom_item(
                            move || {
                                let modifier = Modifier.height(first.dp());
                                let modifier = match own {
                                    Some(own) => modifier.align(&inner, own),
                                    None => modifier,
                                };
                                Button!(
                                    || {},
                                    modifier = modifier.test_tag("first"),
                                    |_: RowScope| Text!("A")
                                );
                            },
                            |_| {},
                        );
                        scope.custom_item(
                            move || {
                                Button!(
                                    || {},
                                    modifier = Modifier.height(second.dp()).test_tag("second"),
                                    |_: RowScope| Text!("B")
                                );
                            },
                            |_| {},
                        );
                    }
                );
            });
        });
    });
    (bounds(&mut ui, "first").top, bounds(&mut ui, "second").top)
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_verticalAlignment_isTop
#[test]
fn a_group_aligned_to_the_top_tops_its_buttons_together() {
    let (first, second) = tops(Alignment::TOP, 48.0, 64.0, None);
    assert_eq!(first, second);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_verticalAlignment_isCenter
#[test]
fn a_group_aligned_to_the_center_centers_its_buttons() {
    let (first, second) = tops(Alignment::CENTER_VERTICALLY, 48.0, 64.0, None);
    assert_eq!(first + 24.0, second + 32.0);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_verticalAlignment_isBottom
#[test]
fn a_group_aligned_to_the_bottom_bottoms_its_buttons_together() {
    let (first, second) = tops(Alignment::BOTTOM, 48.0, 64.0, None);
    assert_eq!(first + 48.0, second + 64.0);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_individualAlignment_alignTop
#[test]
fn a_button_aligned_to_the_top_is_at_the_top() {
    assert_eq!(tops(Alignment::BOTTOM, 50.0, 100.0, Some(Alignment::TOP)), (0.0, 0.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_individualAlignment_alignCenter
#[test]
fn a_button_aligned_to_the_center_is_centered() {
    assert_eq!(tops(Alignment::TOP, 50.0, 100.0, Some(Alignment::CENTER_VERTICALLY)), (25.0, 0.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_individualAlignment_alignBottom
#[test]
fn a_button_aligned_to_the_bottom_is_at_the_bottom() {
    assert_eq!(tops(Alignment::TOP, 50.0, 100.0, Some(Alignment::BOTTOM)), (50.0, 0.0));
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_individualAlignment_overridesGroupAlignment
#[test]
fn a_buttons_own_alignment_wins_over_the_groups() {
    assert_eq!(tops(Alignment::BOTTOM, 50.0, 100.0, Some(Alignment::TOP)), (0.0, 0.0));
    assert_eq!(tops(Alignment::BOTTOM, 50.0, 100.0, None), (50.0, 0.0));
}

/// Two weighted buttons in 100 dp, each able to give `limit` dp, A pressed with `ratio`: B's
/// width.
fn neighbours_width_with_a_limit(ratio: Option<f32>) -> f32 {
    let mut ui = compose_test(move || {
        MaterialTheme!({
            Box!(modifier = Modifier.width(100.dp()), {
                let content = move |scope: ButtonGroupScope| {
                    for label in ["A", "B"] {
                        let inner = scope.clone();
                        scope.custom_item(
                            move || {
                                let source = remember(MutableInteractionSource::new);
                                Button!(
                                    || {},
                                    modifier = Modifier
                                        .weight(&inner, 1.0)
                                        .animate_width_with(&inner, &source, 10.dp())
                                        .test_tag(label),
                                    interaction_source = Some(source.clone()),
                                    content_padding = PaddingValues::symmetric(10.dp(), 0.dp()),
                                    |_: RowScope| Text!(
                                        label,
                                        soft_wrap = false,
                                        max_lines = 1,
                                        overflow = TextOverflow::Visible
                                    )
                                );
                            },
                            |_| {},
                        );
                    }
                };
                match ratio {
                    Some(ratio) => ButtonGroup!(
                        |_| {},
                        expanded_ratio = ratio,
                        horizontal_arrangement = Arrangement::spaced_by(0.dp()),
                        content = content
                    ),
                    None => ButtonGroup!(
                        |_| {},
                        horizontal_arrangement = Arrangement::spaced_by(0.dp()),
                        content = content
                    ),
                }
            });
        });
    });
    ui.set_auto_advance(false);
    ui.on_node_with_tag("A").perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    ui.advance_time_by(std::time::Duration::from_millis(200));
    ui.wait_for_idle();
    bounds(&mut ui, "B").width()
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_widthAnimation_exceedPaddingLimit
#[test]
fn a_neighbour_gives_no_more_than_its_limit() {
    // The press asks for all of A's 50 dp; B gives its 10.
    assert_eq!(neighbours_width_with_a_limit(Some(1.0)), 40.0);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_widthAnimation_withinPaddingLimit
#[test]
fn a_neighbour_gives_what_the_ratio_asks_within_its_limit() {
    // 15 % of 50 dp, 7.5 dp, rounded to the pixel at 1 pixel a dp.
    let width = neighbours_width_with_a_limit(None);
    assert!((width - 42.5).abs() <= 0.5, "{width}");
}

/// Three 100 dp buttons, the outer ones able to give 10 dp, the middle one pressed with a ratio of
/// a half, in `direction`: the three widths.
fn asymmetric_widths(direction: LayoutDirection) -> [f32; 3] {
    let sources: [MutableInteractionSource; 3] =
        std::array::from_fn(|_| MutableInteractionSource::new());
    let mut ui = compose_test({
        let sources = sources.clone();
        move || {
            let sources = sources.clone();
            MaterialTheme!({
                let sources = sources.clone();
                CompositionLocalProvider(
                    compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                    || {
                        let sources = sources.clone();
                        ButtonGroup!(
                            |_| {},
                            expanded_ratio = 0.5,
                            horizontal_arrangement = Arrangement::spaced_by(0.dp()),
                            content = move |scope: ButtonGroupScope| {
                                for (index, source) in sources.clone().into_iter().enumerate() {
                                    let inner = scope.clone();
                                    let tag = TAGS[index];
                                    scope.custom_item(
                                        move || {
                                            let modifier = Modifier.width(100.dp());
                                            let (modifier, padding) = if index == 1 {
                                                (
                                                    modifier.animate_width(&inner, &source),
                                                    ButtonDefaults::CONTENT_PADDING,
                                                )
                                            } else {
                                                (
                                                    modifier.animate_width_with(
                                                        &inner,
                                                        &source,
                                                        10.dp(),
                                                    ),
                                                    PaddingValues::new(
                                                        40.dp(),
                                                        0.dp(),
                                                        10.dp(),
                                                        0.dp(),
                                                    ),
                                                )
                                            };
                                            Button!(
                                                || {},
                                                modifier = modifier.test_tag(tag),
                                                interaction_source = Some(source.clone()),
                                                content_padding = padding,
                                                |_: RowScope| Text!(tag)
                                            );
                                        },
                                        |_| {},
                                    );
                                }
                            }
                        );
                    },
                );
            });
        }
    });
    ui.set_auto_advance(false);
    ui.on_node_with_tag("B").perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    ui.advance_time_by(std::time::Duration::from_millis(200));
    ui.wait_for_idle();
    ["A", "B", "C"].map(|tag| bounds(&mut ui, tag).width())
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_asymmetricPadding_ltr_buttonSizing
#[test]
fn a_pressed_button_takes_what_each_neighbour_can_give() {
    assert_eq!(asymmetric_widths(LayoutDirection::Ltr), [90.0, 120.0, 90.0]);
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/ButtonGroupTest.kt, buttonGroup_asymmetricPadding_rtl_buttonSizing
#[test]
fn a_pressed_button_takes_what_each_neighbour_can_give_right_to_left() {
    assert_eq!(asymmetric_widths(LayoutDirection::Rtl), [90.0, 120.0, 90.0]);
}

/// The widths come back once the press is released.
#[test]
fn a_released_button_goes_back_to_its_width() {
    let source = MutableInteractionSource::new();
    let mut ui = compose_test({
        let source = source.clone();
        move || {
            let source = source.clone();
            MaterialTheme!({
                let source = source.clone();
                ButtonGroup!(
                    |_| {},
                    content = move |scope: ButtonGroupScope| {
                        for (index, tag) in TAGS.into_iter().enumerate() {
                            let (inner, source) = (scope.clone(), source.clone());
                            scope.custom_item(
                                move || {
                                    let own = remember(MutableInteractionSource::new);
                                    let source = if index == 0 { source.clone() } else { own };
                                    Button!(
                                        || {},
                                        modifier = Modifier
                                            .width(75.dp())
                                            .animate_width(&inner, &source)
                                            .test_tag(tag),
                                        interaction_source = Some(source.clone()),
                                        |_: RowScope| Text!(tag)
                                    );
                                },
                                |_| {},
                            );
                        }
                    }
                );
            });
        }
    });
    let press = compose_ui::Press::new(Offset::ZERO);
    source.try_emit(compose_ui::PressInteraction::Press(press));
    ui.wait_for_idle();
    assert_eq!(bounds(&mut ui, "A").width(), 86.0);
    source.try_emit(compose_ui::PressInteraction::Release(press));
    ui.wait_for_idle();
    assert_eq!(bounds(&mut ui, "A").width(), 75.0);
    assert_eq!(bounds(&mut ui, "B").width(), 75.0);
}

/// A 300 dp group in `direction` of `count` 100 dp buttons tagged by their index (the first
/// emitting nothing if `empty_first`), each growing as its source in `sources` is pressed, and a
/// 40 dp box as its indicator.
fn hundreds(
    direction: LayoutDirection,
    count: usize,
    empty_first: bool,
    sources: Vec<MutableInteractionSource>,
) -> ComposeTestRule {
    compose_test(move || {
        let sources = sources.clone();
        MaterialTheme!({
            let sources = sources.clone();
            CompositionLocalProvider(
                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                || {
                    let sources = sources.clone();
                    ButtonGroup!(
                        |_| Box!(modifier = Modifier.size(40.dp()).test_tag(INDICATOR)),
                        modifier = Modifier.width(300.dp()),
                        content = move |scope: ButtonGroupScope| {
                            if empty_first {
                                scope.custom_item(|| {}, |_| {});
                            }
                            for (index, source) in sources.iter().take(count).enumerate() {
                                let (inner, source) = (scope.clone(), source.clone());
                                scope.custom_item(
                                    move || {
                                        Button!(
                                            || {},
                                            modifier = Modifier
                                                .width(100.dp())
                                                .animate_width(&inner, &source)
                                                .test_tag(format!("{index}")),
                                            interaction_source = Some(source.clone()),
                                            |_: RowScope| Text!("{index}")
                                        );
                                    },
                                    |_| {},
                                );
                            }
                        }
                    );
                },
            );
        });
    })
}

fn sources(count: usize) -> Vec<MutableInteractionSource> {
    (0..count).map(|_| MutableInteractionSource::new()).collect()
}

/// Right to left, what fits starts at the right and the indicator follows it to the left: after
/// two 100 dp items and their 12 dp spaces (224 dp), 40 dp wide, in 300 dp.
#[test]
fn the_indicator_follows_what_fits_right_to_left() {
    let mut ui = hundreds(LayoutDirection::Rtl, 5, false, sources(5));
    assert_eq!(bounds(&mut ui, INDICATOR).left, 36.0);
    assert_eq!(bounds(&mut ui, "0").right, 300.0);
    let mut ui = hundreds(LayoutDirection::Ltr, 5, false, sources(5));
    assert_eq!(bounds(&mut ui, INDICATOR).left, 224.0);
}

/// An item emitting no node takes no place: each node is an item to the layout, as Compose's
/// content measurables, and the indicator is still the last.
#[test]
fn an_item_emitting_nothing_takes_no_place() {
    let mut ui = hundreds(LayoutDirection::Ltr, 5, true, sources(5));
    ui.on_node_with_tag(INDICATOR).assert_is_displayed();
    assert_eq!(bounds(&mut ui, INDICATOR).left, 224.0);
    assert_eq!(bounds(&mut ui, "0").left, 0.0);
    let mut ui = hundreds(LayoutDirection::Ltr, 2, true, sources(2));
    ui.on_node_with_tag(INDICATOR).assert_is_not_displayed();
    assert_eq!(bounds(&mut ui, "1").left, 112.0);
}

/// The last item that fits grows into the one before it alone: the items after it are in the menu.
#[test]
fn the_last_item_shown_grows_into_the_one_before() {
    let sources = sources(5);
    let mut ui = hundreds(LayoutDirection::Ltr, 5, false, sources.clone());
    let press = compose_ui::Press::new(Offset::ZERO);
    sources[1].try_emit(compose_ui::PressInteraction::Press(press));
    ui.wait_for_idle();
    // 15 % of 100 dp, within the first's 24 dp of padding.
    assert_eq!(bounds(&mut ui, "1").width(), 115.0);
    assert_eq!(bounds(&mut ui, "0").width(), 85.0);
    assert_eq!(bounds(&mut ui, INDICATOR).left, 224.0);
}

/// Released early, it goes on growing to three quarters of the way before shrinking back.
#[test]
fn a_release_waits_for_three_quarters_of_the_growth() {
    let sources = sources(2);
    let mut ui = hundreds(LayoutDirection::Ltr, 2, false, sources.clone());
    ui.set_auto_advance(false);
    let press = compose_ui::Press::new(Offset::ZERO);
    sources[0].try_emit(compose_ui::PressInteraction::Press(press));
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    let at_release = bounds(&mut ui, "0").width();
    sources[0].try_emit(compose_ui::PressInteraction::Release(press));
    let mut widest = at_release;
    for _ in 0..60 {
        ui.advance_time_by_frame();
        widest = widest.max(bounds(&mut ui, "0").width());
    }
    // Three quarters of 15 dp at least.
    assert!(at_release < 111.25 && widest >= 111.0, "{at_release} {widest}");
    assert_eq!(bounds(&mut ui, "0").width(), 100.0);
}

/// A weighted item takes what the others and the spaces leave.
#[test]
fn a_weighted_item_takes_what_a_fixed_one_leaves() {
    let mut ui = compose_test(|| {
        MaterialTheme!({
            ButtonGroup!(
                |_| {},
                modifier = Modifier.width(312.dp()),
                content = |scope: ButtonGroupScope| {
                    scope.custom_item(
                        || {
                            Button!(
                                || {},
                                modifier = Modifier.width(100.dp()),
                                |_: RowScope| Text!("A")
                            )
                        },
                        |_| {},
                    );
                    scope.clickable_item_with(|| {}, "B", None::<fn()>, Some(1.0), true);
                }
            );
        });
    });
    assert_eq!(text_bounds(&mut ui, "B").width(), 200.0);
    assert_eq!(text_bounds(&mut ui, "B").left - text_bounds(&mut ui, "A").right, 12.0);
}
