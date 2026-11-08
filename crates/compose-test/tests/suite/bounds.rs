//! Bounds assertions in dp, as Compose's `BoundsAssertions.kt`.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::FixedTextMeasurer;
use compose_ui::{
    Alignment, Color, Density, Dp, DpSize, HorizontalAlignmentLine, IntSize, LOCAL_DENSITY,
    LOCAL_VIEW_CONFIGURATION, Layout, Modifier, PaddingValues, UnitExt, ViewConfiguration,
    measure_policy,
};

const TAG: &str = "box";

/// A black box of `width` by `height` dp, `start` and `top` dp from the root's top left, as
/// Compose's `composeBox`.
#[composable]
fn PlacedBox(start: Dp, top: Dp, width: Dp, height: Dp) {
    Box!(
        modifier = Modifier.fill_max_size().wrap_content_size_with(Alignment::TOP_START, false),
        {
            Box!(
                modifier = Modifier.padding(PaddingValues { start, top, ..PaddingValues::ZERO }),
                {
                    Box!(
                        modifier = Modifier
                            .test_tag(TAG)
                            .required_size((width, height))
                            .background(Color::BLACK)
                    );
                }
            );
        }
    );
}

fn compose_box() -> ComposeTestRule {
    compose_test(|| PlacedBox(50.dp(), 100.dp(), 80.dp(), 100.dp()))
}

/// The same box in whole pixels at density 2, as Compose's `composeBoxWithPixelBounds`.
fn compose_box_with_pixel_bounds() -> ComposeTestRule {
    compose_test_with(
        IntSize::new(400, 800),
        Density::new(2.0),
        Box::new(FixedTextMeasurer::new()),
        || PlacedBox(Dp(25.0), Dp(50.0), Dp(40.0), Dp(50.0)),
    )
}

fn px(ui: &ComposeTestRule, pixels: f32) -> Dp {
    ui.density().to_dp(pixels)
}

#[track_caller]
fn expect_error(action: impl FnOnce()) -> String {
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
    let panic = failed.expect_err("an assertion error");
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()))
        .unwrap_or_default()
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertSizeEquals
#[test]
fn a_nodes_size_is_asserted_in_dp() {
    let mut ui = compose_box();
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(80.dp()).assert_height_is_equal_to(100.dp());
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertSizeAtLeast
#[test]
fn a_nodes_size_is_at_least_its_own_and_less() {
    let mut ui = compose_box();
    ui.on_node_with_tag(TAG)
        .assert_width_is_at_least(80.dp())
        .assert_width_is_at_least(79.dp())
        .assert_height_is_at_least(100.dp())
        .assert_height_is_at_least(99.dp());
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertSizeEquals_fail
#[test]
fn a_wrong_size_fails_naming_it() {
    let mut ui = compose_box();
    let message = expect_error(|| {
        ui.on_node_with_tag(TAG).assert_width_is_equal_to(70.dp());
    });
    assert!(message.contains("Actual width is 80.0.dp, expected 70.0.dp"), "{message}");
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_height_is_equal_to(90.dp());
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertSizeAtLeast_fail
#[test]
fn a_size_below_the_least_fails() {
    let mut ui = compose_box();
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_width_is_at_least(81.dp());
    });
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_height_is_at_least(101.dp());
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertWidthIsEqualTo_withCustomTolerance, assertHeightIsEqualTo_withCustomTolerance
#[test]
fn a_size_is_compared_within_the_tolerance_given() {
    let mut ui = compose_box_with_pixel_bounds();
    let (width, height) = (px(&ui, 81.0), px(&ui, 101.0));
    let (success, failure) = (px(&ui, 1.1), px(&ui, 0.9));
    ui.on_node_with_tag(TAG)
        .assert_width_is_equal_to_with_tolerance(width, success)
        .assert_height_is_equal_to_with_tolerance(height, success);
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_width_is_equal_to_with_tolerance(width, failure);
    });
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_height_is_equal_to_with_tolerance(height, failure);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertPosition
#[test]
fn a_nodes_position_in_the_root_is_asserted_in_dp() {
    let mut ui = compose_box();
    ui.on_node_with_tag(TAG)
        .assert_position_in_root_is_equal_to(50.dp(), 100.dp())
        .assert_left_position_in_root_is_equal_to(50.dp())
        .assert_top_position_in_root_is_equal_to(100.dp());
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertPosition_fail
#[test]
fn a_wrong_position_fails() {
    let mut ui = compose_box();
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_position_in_root_is_equal_to(51.dp(), 101.dp());
    });
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_position_in_root_is_equal_to(49.dp(), 99.dp());
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertPositionInRootIsEqualTo_withCustomTolerance, assertLeftPositionInRootIsEqualTo_withCustomTolerance, assertTopPositionInRootIsEqualTo_withCustomTolerance
#[test]
fn a_position_is_compared_within_the_tolerance_given() {
    let mut ui = compose_box_with_pixel_bounds();
    let (left, top) = (px(&ui, 51.0), px(&ui, 101.0));
    let (success, failure) = (px(&ui, 1.1), px(&ui, 0.9));
    ui.on_node_with_tag(TAG)
        .assert_position_in_root_is_equal_to_with_tolerance(left, top, success)
        .assert_left_position_in_root_is_equal_to_with_tolerance(left, success)
        .assert_top_position_in_root_is_equal_to_with_tolerance(top, success);
    expect_error(|| {
        ui.on_node_with_tag(TAG)
            .assert_position_in_root_is_equal_to_with_tolerance(left, top, failure);
    });
    expect_error(|| {
        ui.on_node_with_tag(TAG)
            .assert_left_position_in_root_is_equal_to_with_tolerance(left, failure);
    });
    expect_error(|| {
        ui.on_node_with_tag(TAG)
            .assert_top_position_in_root_is_equal_to_with_tolerance(top, failure);
    });
}

/// The rule's tolerance applies to the assertions given none, as
/// `ComposeUiTestConfig(boundsAssertionTolerance)`.
#[test]
fn the_rules_tolerance_applies_to_assertions_given_none() {
    let mut ui = compose_box();
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_width_is_equal_to(81.dp());
    });
    ui.set_bounds_assertion_tolerance(1.5.dp());
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(81.dp());
}

/// A box shifted 30 dp left and 10 dp up inside a clipping root: clipped to 50 by 90.
fn compose_clipped_box() -> ComposeTestRule {
    compose_test(|| {
        Box!(
            modifier = Modifier
                .fill_max_size()
                .clip_to_bounds()
                .wrap_content_size_with(Alignment::TOP_START, false),
            {
                Box!(
                    modifier = Modifier
                        .offset(Dp(-30.0), Dp(-10.0))
                        .test_tag(TAG)
                        .required_size((80.dp(), 100.dp()))
                        .background(Color::BLACK)
                );
            }
        );
    })
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertClippedPosition
#[test]
fn a_clipped_nodes_position_is_unclipped() {
    let mut ui = compose_clipped_box();
    ui.on_node_with_tag(TAG)
        .assert_position_in_root_is_equal_to(Dp(-30.0), Dp(-10.0))
        .assert_left_position_in_root_is_equal_to(Dp(-30.0))
        .assert_top_position_in_root_is_equal_to(Dp(-10.0));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertClippedSize
#[test]
fn a_clipped_nodes_size_is_unclipped() {
    let mut ui = compose_clipped_box();
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(80.dp()).assert_height_is_equal_to(100.dp());
}

/// The bounds in the root clip; the unclipped bounds don't, as `getBoundsInRoot` and
/// `getUnclippedBoundsInRoot`.
#[test]
fn the_bounds_in_the_root_are_clipped_and_the_unclipped_ones_not() {
    let mut ui = compose_clipped_box();
    let unclipped = ui.on_node_with_tag(TAG).get_unclipped_bounds_in_root();
    assert_eq!(
        (unclipped.left, unclipped.top, unclipped.right, unclipped.bottom),
        (Dp(-30.0), Dp(-10.0), 50.dp(), 90.dp())
    );
    let clipped = ui.on_node_with_tag(TAG).get_bounds_in_root();
    assert_eq!(
        (clipped.left, clipped.top, clipped.right, clipped.bottom),
        (0.dp(), 0.dp(), 50.dp(), 90.dp())
    );
}

/// A box in a graphics layer: Compose's unclipped bounds are `Rect(positionInRoot, size)`
/// (`BoundsAssertions.kt`, `unclippedBoundsInRoot`): the top left mapped through the layer,
/// the size unscaled; its bounds in the root (`boundsInRoot`) are the layer-mapped bounding
/// box. A 10 dp box scaled 2 around its center: top left at -5, 10 dp unclipped, 20 dp in the
/// root.
#[test]
fn a_scaled_nodes_bounds_in_the_root_are_scaled_and_its_unclipped_size_is_not() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.padding(20.dp()), {
            Box!(modifier = Modifier.scale(2.0).test_tag(TAG).size(10.dp()));
        });
    });
    let clipped = ui.on_node_with_tag(TAG).get_bounds_in_root();
    assert_eq!(
        (clipped.left, clipped.top, clipped.right, clipped.bottom),
        (15.dp(), 15.dp(), 35.dp(), 35.dp())
    );
    let unclipped = ui.on_node_with_tag(TAG).get_unclipped_bounds_in_root();
    assert_eq!(
        (unclipped.left, unclipped.top, unclipped.width(), unclipped.height()),
        (15.dp(), 15.dp(), 10.dp(), 10.dp())
    );
    ui.on_node_with_tag(TAG)
        .assert_width_is_equal_to(10.dp())
        .assert_position_in_root_is_equal_to(15.dp(), 15.dp());
}

/// The same with a quarter turn: a 20 by 10 dp box rotated 90° around its center is 10 by
/// 20 dp in the root; its unclipped bounds start at its top left mapped through the turn
/// (the top right corner of the turned box) and keep its own size.
#[test]
fn a_rotated_nodes_bounds_in_the_root_are_turned() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.padding(20.dp()), {
            Box!(modifier = Modifier.rotate(90.0).test_tag(TAG).size((20.dp(), 10.dp())));
        });
    });
    let clipped = ui.on_node_with_tag(TAG).get_bounds_in_root();
    let rounded = |dp: Dp| Dp(dp.0.round());
    assert_eq!(
        (
            rounded(clipped.left),
            rounded(clipped.top),
            rounded(clipped.right),
            rounded(clipped.bottom)
        ),
        (25.dp(), 15.dp(), 35.dp(), 35.dp())
    );
    let unclipped = ui.on_node_with_tag(TAG).get_unclipped_bounds_in_root();
    assert_eq!(
        (rounded(unclipped.left), rounded(unclipped.top), unclipped.width(), unclipped.height()),
        (35.dp(), 15.dp(), 20.dp(), 10.dp())
    );
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertBoundsInRootUsesLocalDensity (at the root's density)
/// Bounds are in dp at the root's density: 20 dp of padding is 40 px at density 2.
#[test]
fn bounds_are_in_dp_at_the_density() {
    let mut ui = compose_test_with(
        IntSize::new(400, 800),
        Density::new(2.0),
        Box::new(FixedTextMeasurer::new()),
        || {
            Box!(modifier = Modifier.padding(20.dp()).test_tag(TAG).required_size(10.dp()));
        },
    );
    let bounds = ui.on_node_with_tag(TAG).get_bounds_in_root();
    bounds.left.assert_is_equal_to(20.dp(), "left", DEFAULT_BOUNDS_TOLERANCE);
    bounds.top.assert_is_equal_to(20.dp(), "top", DEFAULT_BOUNDS_TOLERANCE);
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(10.dp());
    let pixels = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds;
    assert_eq!(pixels.width(), 20.0);
}

/// `Dp::assert_is_equal_to`: within the tolerance; an unspecified value only matches an
/// unspecified one, an infinite one only itself.
#[test]
fn dp_values_compare_within_a_tolerance() {
    10.dp().assert_is_equal_to(10.4.dp(), "width", 0.5.dp());
    Dp::UNSPECIFIED.assert_is_equal_to(Dp::UNSPECIFIED, "width", 0.5.dp());
    Dp(f32::INFINITY).assert_is_equal_to(Dp(f32::INFINITY), "width", 0.5.dp());
    let message = expect_error(|| 10.dp().assert_is_equal_to(11.dp(), "width", 0.5.dp()));
    assert!(
        message.contains("Actual width is 10.0.dp, expected 11.0.dp (tolerance: 0.5.dp)"),
        "{message}"
    );
    expect_error(|| 10.dp().assert_is_equal_to(Dp::UNSPECIFIED, "width", 0.5.dp()));
}

/// A black box of `size` tagged `tag` after `modifier`, as Compose's `SmallBox`.
#[composable]
fn SmallBox(modifier: Modifier, tag: &'static str, size: Dp) {
    Box!(modifier = modifier.test_tag(tag).required_size(size).background(Color::BLACK));
}

/// `content` where the minimum touch target is `size`, as Compose's
/// `WithMinimumTouchTargetSize`.
fn with_minimum_touch_target_size(size: DpSize, content: impl Fn() + 'static) {
    let configuration =
        ViewConfiguration { minimum_touch_target_size: size, ..LOCAL_VIEW_CONFIGURATION.current() };
    CompositionLocalProvider(LOCAL_VIEW_CONFIGURATION.provides(configuration), content);
}

/// A clickable box of 10 dp tagged `tag`, its minimum touch target 20 dp, as Compose's
/// `TouchTargetTestContent`.
fn touch_target_test_content(tag: &'static str) {
    with_minimum_touch_target_size(DpSize::new(20.dp(), 20.dp()), move || {
        SmallBox(Modifier.clickable(|| {}), tag, 10.dp())
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertTouchSizeEquals
#[test]
fn assert_touch_size_equals() {
    let mut ui = compose_test(|| touch_target_test_content(TAG));
    ui.on_node_with_tag(TAG)
        .assert_touch_width_is_equal_to(20.dp())
        .assert_touch_height_is_equal_to(20.dp());
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertTouchSizeEquals_fail
#[test]
fn assert_touch_size_equals_fail() {
    let mut ui = compose_test(|| touch_target_test_content(TAG));
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_touch_width_is_equal_to(19.dp());
    });
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_touch_height_is_equal_to(21.dp());
    });
}

/// A clickable box of 10 px whose minimum touch target is 20 px, at density 2.
fn touch_target_in_pixels() -> ComposeTestRule {
    compose_test_with(
        IntSize::new(400, 800),
        Density::new(2.0),
        Box::new(FixedTextMeasurer::new()),
        || {
            with_minimum_touch_target_size(DpSize::new(Dp(10.0), Dp(10.0)), || {
                SmallBox(Modifier.clickable(|| {}), TAG, Dp(5.0))
            })
        },
    )
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertTouchWidthIsEqualTo_withCustomTolerance
#[test]
fn assert_touch_width_is_equal_to_with_custom_tolerance() {
    let mut ui = touch_target_in_pixels();
    let (expected, success, failure) = (px(&ui, 21.0), px(&ui, 1.1), px(&ui, 0.9));
    ui.on_node_with_tag(TAG).assert_touch_width_is_equal_to_with_tolerance(expected, success);
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_touch_width_is_equal_to_with_tolerance(expected, failure);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertTouchHeightIsEqualTo_withCustomTolerance
#[test]
fn assert_touch_height_is_equal_to_with_custom_tolerance() {
    let mut ui = touch_target_in_pixels();
    let (expected, success, failure) = (px(&ui, 21.0), px(&ui, 1.1), px(&ui, 0.9));
    ui.on_node_with_tag(TAG).assert_touch_height_is_equal_to_with_tolerance(expected, success);
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_touch_height_is_equal_to_with_tolerance(expected, failure);
    });
}

/// `content` three times: at the rule's density, at density 1 and at density 4, tagged
/// "default-density", "low-density" and "high-density".
fn at_three_densities(content: fn(&'static str)) -> ComposeTestRule {
    compose_test(move || {
        content("default-density");
        CompositionLocalProvider(LOCAL_DENSITY.provides(Density::new(1.0)), move || {
            content("low-density")
        });
        CompositionLocalProvider(LOCAL_DENSITY.provides(Density::new(4.0)), move || {
            content("high-density")
        });
    })
}

const DENSITY_TAGS: [&str; 3] = ["default-density", "low-density", "high-density"];

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertSizeUsesLocalDensity
#[test]
fn assert_size_uses_local_density() {
    let mut ui = at_three_densities(|tag| SmallBox(Modifier, tag, 10.dp()));
    for tag in DENSITY_TAGS {
        ui.on_node_with_tag(tag)
            .assert_width_is_equal_to(10.dp())
            .assert_height_is_equal_to(10.dp());
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertTouchSizeUsesLocalDensity
#[test]
fn assert_touch_size_uses_local_density() {
    let mut ui = at_three_densities(touch_target_test_content);
    for tag in DENSITY_TAGS {
        ui.on_node_with_tag(tag)
            .assert_touch_width_is_equal_to(20.dp())
            .assert_touch_height_is_equal_to(20.dp());
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertBoundsInRootUsesLocalDensity
#[test]
fn assert_bounds_in_root_uses_local_density() {
    let mut ui = at_three_densities(|tag| SmallBox(Modifier.padding(20.dp()), tag, 10.dp()));
    for tag in DENSITY_TAGS {
        let bounds = ui.on_node_with_tag(tag).get_bounds_in_root();
        bounds.left.assert_is_equal_to(20.dp(), "left", DEFAULT_BOUNDS_TOLERANCE);
        bounds.top.assert_is_equal_to(20.dp(), "top", DEFAULT_BOUNDS_TOLERANCE);
    }
}

const TEST_LINE_POSITION: Dp = Dp(10.0);

/// One line for every test, as Compose's `TestLine` (each `new` is another line).
fn test_line() -> HorizontalAlignmentLine {
    static LINE: std::sync::LazyLock<HorizontalAlignmentLine> =
        std::sync::LazyLock::new(|| HorizontalAlignmentLine::new(i32::max));
    *LINE
}

/// A layout filling its constraints that gives [`test_line`] at 10 dp, as Compose's
/// `BoxWithAlignmentLine`.
fn box_with_alignment_line(modifier: Modifier) {
    let position = LOCAL_DENSITY.current().round_to_px(TEST_LINE_POSITION);
    let policy = measure_policy(move |scope, _, constraints| {
        scope.provide_alignment_lines([(test_line(), position)]);
        scope.layout(constraints.max_width, constraints.max_height)
    });
    Layout(modifier, policy, callback(|| {}));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertAlignmentLinesUseLocalDensity
#[test]
fn assert_alignment_lines_use_local_density() {
    let mut ui = at_three_densities(|tag| box_with_alignment_line(Modifier.test_tag(tag)));
    for tag in DENSITY_TAGS {
        ui.on_node_with_tag(tag).get_alignment_line_position(test_line()).assert_is_equal_to(
            TEST_LINE_POSITION,
            "alignment line",
            DEFAULT_BOUNDS_TOLERANCE,
        );
    }
}

/// `content` not measured nor placed, filling the constraints, as Compose's `DoNotMeasure`.
fn do_not_measure(content: impl Fn() + 'static) {
    let policy = measure_policy(|scope, _, constraints| {
        scope.layout(constraints.max_width, constraints.max_height)
    });
    Layout(Modifier, policy, callback(content));
}

/// `content` measured and not placed, as Compose's `DoNotPlace`.
fn do_not_place(content: impl Fn() + 'static) {
    let policy = measure_policy(|scope, measurables, constraints| {
        let placeables: Vec<_> =
            measurables.iter().map(|measurable| scope.measure(*measurable, constraints)).collect();
        let width = placeables.iter().map(|placeable| placeable.width).max().unwrap_or(0);
        let height = placeables.iter().map(|placeable| placeable.height).max().unwrap_or(0);
        scope.layout(width, height)
    });
    Layout(Modifier, policy, callback(content));
}

fn ten_dp_box() {
    Box!(modifier = Modifier.test_tag(TAG).required_size(10.dp()));
}

/// A node not placed has no position: unspecified, as Compose's `getPositionTest`.
#[track_caller]
fn get_position_test(mut ui: ComposeTestRule) {
    let mut node = ui.on_node_with_tag(TAG);
    node.assert_position_in_root_is_equal_to(Dp::UNSPECIFIED, Dp::UNSPECIFIED);
    node.assert_top_position_in_root_is_equal_to(Dp::UNSPECIFIED);
    node.assert_left_position_in_root_is_equal_to(Dp::UNSPECIFIED);
    let bounds = node.get_unclipped_bounds_in_root();
    let specified = [bounds.left, bounds.top, bounds.right, bounds.bottom].map(Dp::is_specified);
    assert_eq!(specified, [false; 4], "{bounds:?}");
    let not_equal =
        |subject: &str| format!("Actual {subject} is Dp.Unspecified, expected 1.0.dp (");
    let message = expect_error(|| {
        node.assert_position_in_root_is_equal_to(1.dp(), 1.dp());
    });
    assert!(message.contains(&not_equal("left")), "{message}");
    let message = expect_error(|| {
        node.assert_top_position_in_root_is_equal_to(1.dp());
    });
    assert!(message.contains(&not_equal("top")), "{message}");
    let message = expect_error(|| {
        node.assert_left_position_in_root_is_equal_to(1.dp());
    });
    assert!(message.contains(&not_equal("left")), "{message}");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, getPosition_measuredNotPlaced
#[test]
#[ignore = "unplaced content: a node its parent doesn't place isn't in compose-rs's semantics tree; Compose's is, with unspecified bounds"]
fn get_position_measured_not_placed() {
    get_position_test(compose_test(|| do_not_place(ten_dp_box)));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, getPosition_notMeasuredNotPlaced
#[test]
#[ignore = "unplaced content: a node its parent doesn't place isn't in compose-rs's semantics tree; Compose's is, with unspecified bounds"]
fn get_position_not_measured_not_placed() {
    get_position_test(compose_test(|| do_not_measure(ten_dp_box)));
}

/// A node not placed has no size: unspecified, as Compose's `getSizeTest`.
#[track_caller]
fn get_size_test(mut ui: ComposeTestRule) {
    let mut node = ui.on_node_with_tag(TAG);
    node.assert_width_is_equal_to(Dp::UNSPECIFIED);
    node.assert_height_is_equal_to(Dp::UNSPECIFIED);
    node.assert_width_is_at_least(Dp::UNSPECIFIED);
    node.assert_height_is_at_least(Dp::UNSPECIFIED);
    let not_equal =
        |subject: &str| format!("Actual {subject} is Dp.Unspecified, expected 10.0.dp (");
    let not_at_least =
        |subject: &str| format!("Actual {subject} is Dp.Unspecified, expected at least 10.0.dp (");
    let message = expect_error(|| {
        node.assert_width_is_equal_to(10.dp());
    });
    assert!(message.contains(&not_equal("width")), "{message}");
    let message = expect_error(|| {
        node.assert_height_is_equal_to(10.dp());
    });
    assert!(message.contains(&not_equal("height")), "{message}");
    let message = expect_error(|| {
        node.assert_width_is_at_least(10.dp());
    });
    assert!(message.contains(&not_at_least("width")), "{message}");
    let message = expect_error(|| {
        node.assert_height_is_at_least(10.dp());
    });
    assert!(message.contains(&not_at_least("height")), "{message}");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, getSize_measuredNotPlaced
#[test]
#[ignore = "unplaced content: a node its parent doesn't place isn't in compose-rs's semantics tree; Compose's is, with unspecified bounds"]
fn get_size_measured_not_placed() {
    get_size_test(compose_test(|| do_not_place(ten_dp_box)));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, getSize_notMeasuredNotPlaced
#[test]
#[ignore = "unplaced content: a node its parent doesn't place isn't in compose-rs's semantics tree; Compose's is, with unspecified bounds"]
fn get_size_not_measured_not_placed() {
    get_size_test(compose_test(|| do_not_measure(ten_dp_box)));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, getAlignmentLine_measuredNotPlaced
#[test]
#[ignore = "unplaced content: a node its parent doesn't place isn't in compose-rs's semantics tree; Compose's is, with unspecified bounds"]
fn get_alignment_line_measured_not_placed() {
    let mut ui = compose_test(|| do_not_place(|| box_with_alignment_line(Modifier.test_tag(TAG))));
    ui.on_node_with_tag(TAG).get_alignment_line_position(test_line()).assert_is_equal_to(
        TEST_LINE_POSITION,
        "alignment line",
        DEFAULT_BOUNDS_TOLERANCE,
    );
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, getAlignmentLine_notMeasuredNotPlaced
#[test]
#[ignore = "unplaced content: a node its parent doesn't place isn't in compose-rs's semantics tree; Compose's is, with unspecified bounds"]
fn get_alignment_line_not_measured_not_placed() {
    let mut ui =
        compose_test(|| do_not_measure(|| box_with_alignment_line(Modifier.test_tag(TAG))));
    ui.on_node_with_tag(TAG).get_alignment_line_position(test_line()).assert_is_equal_to(
        Dp::UNSPECIFIED,
        "alignment line",
        DEFAULT_BOUNDS_TOLERANCE,
    );
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/BoundsAssertionsTest.kt, assertBounds_withConfiguredTolerance
#[test]
fn assert_bounds_with_configured_tolerance() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.test_tag(TAG).size(100.dp()));
    });
    ui.set_bounds_assertion_tolerance(2.dp());
    // 100 dp, expecting 101.5 dp: 1.5 dp off.
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(101.5.dp());
    ui.on_node_with_tag(TAG).assert_height_is_equal_to(101.5.dp());
    ui.on_node_with_tag(TAG).assert_position_in_root_is_equal_to(1.5.dp(), 1.5.dp());
    // 3 dp off, past the tolerance.
    expect_error(|| {
        ui.on_node_with_tag(TAG).assert_width_is_equal_to(103.dp());
    });
}
