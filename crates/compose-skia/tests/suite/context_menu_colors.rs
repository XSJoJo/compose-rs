//! The dropdown context menu's colors and shadow drawn by Skia (D2036), from the pixel tests of
//! Compose's `ContextMenuUiTest`: its other tests are compose-foundation's `context_menu_ui.rs`.
//!
//! At density 1 in a 500 px root, the labels in Skia's default font.

#![allow(non_snake_case)]

use std::collections::HashMap;
use std::rc::Rc;

use compose::foundation::context_menu::*;
use compose::foundation::layout::*;
use compose::foundation::*;
use compose::runtime::mutable_state_of;
use compose::test::*;
use compose::ui::platform::UiHost;
use compose::ui::{
    Color, Density, IntOffset, IntSize, LayoutDirection, PopupPositionProvider, Rect,
};
use compose_skia::SkiaTextMeasurer;

const TAG: &str = "testTag";

/// Compose's `TestColors`.
const TEST_COLORS: ContextMenuColors = ContextMenuColors {
    background_color: Color::WHITE,
    text_color: Color::BLACK,
    icon_color: Color::BLUE,
    disabled_text_color: Color::RED,
    disabled_icon_color: Color::GREEN,
};

const TEST_COLOR: Color = Color::RED;
/// Without a background, the background's color isn't known.
const TEST_BACKGROUND_COLOR: Color = Color::WHITE;

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    let host =
        UiHost::new(IntSize::new(500, 500), Density::new(1.0), Box::new(SkiaTextMeasurer::new()));
    let mut ui = ComposeTestRule::empty(host);
    ui.set_content(content);
    ui
}

/// Compose's `transparentColors`: all transparent but the background, unless given.
fn transparent_colors() -> ContextMenuColors {
    ContextMenuColors {
        background_color: TEST_BACKGROUND_COLOR,
        text_color: Color::TRANSPARENT,
        icon_color: Color::TRANSPARENT,
        disabled_text_color: Color::TRANSPARENT,
        disabled_icon_color: Color::TRANSPARENT,
    }
}

fn filling_icon() -> ContextMenuIcon {
    Rc::new(|color| Box!(modifier = Modifier.background(color).fill_max_size()))
}

/// Compose's `TestColumn` with one `testItem`.
fn test_column(
    colors: ContextMenuColors,
    label: &'static str,
    enabled: bool,
    icon: Option<ContextMenuIcon>,
) {
    let block: ContextMenuBuilderBlock = Rc::new(move |scope: &mut ContextMenuScope| {
        scope.item(label, Modifier, enabled, icon.clone(), || {});
    });
    ContextMenuColumnBuilder(Modifier.test_tag(TAG), colors, block);
}

/// Compose's `getColors`: the colors of at least 2% of the tagged node's pixels, past the
/// shadow's and corners' padding at its start.
fn get_colors(ui: &mut ComposeTestRule) -> Vec<Color> {
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let padding = (ContextMenuSpec::CORNER_RADIUS.0 * 4.0).ceil() as i32;
    // As Compose's `toPixelMap(startX = padding, startY = padding, width = width - padding,
    // height = height - padding)`.
    let (width, height) = (image.width() - padding, image.height() - padding);
    let mut histogram: HashMap<Color, usize> = HashMap::new();
    for x in padding..image.width() {
        for y in padding..image.height() {
            *histogram.entry(image.pixel(x, y)).or_default() += 1;
        }
    }
    let threshold = (width * height) as f32 * 0.02;
    histogram.into_iter().filter(|&(_, count)| count as f32 >= threshold).map(|(c, _)| c).collect()
}

fn get_non_background_colors(ui: &mut ComposeTestRule) -> Vec<Color> {
    get_colors(ui).into_iter().filter(|color| *color != TEST_BACKGROUND_COLOR).collect()
}

/// Compose's `assertThatColors(colors).containsExactly(expected)`, within 0.02 a channel.
#[track_caller]
fn assert_colors(actual: Vec<Color>, expected: &[Color]) {
    let close = |a: Color, b: Color| {
        (a.red() - b.red()).abs() <= 0.02
            && (a.green() - b.green()).abs() <= 0.02
            && (a.blue() - b.blue()).abs() <= 0.02
            && (a.alpha() - b.alpha()).abs() <= 0.02
    };
    assert_eq!(actual.len(), expected.len(), "{actual:?} isn't {expected:?}");
    for color in expected {
        assert!(actual.iter().any(|it| close(*it, *color)), "{actual:?} isn't {expected:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColumn_usedNormally_hasExpectedShadow
#[test]
fn whenContextMenuColumn_usedNormally_hasExpectedShadow() {
    let mut ui = rule(|| {
        Box!(
            modifier = Modifier
                .test_tag("containerTag")
                .padding(ContextMenuSpec::MENU_CONTAINER_ELEVATION),
            {
                test_column(TEST_COLORS, "Item", true, None);
            }
        );
    });
    let image = ui.on_node_with_tag("containerTag").capture_to_image();
    let outer = ui.on_node_with_tag("containerTag").fetch_semantics_node().bounds_in_root();
    let inner = ui.on_node_with_tag(TAG).fetch_semantics_node().bounds_in_root();
    let column = inner.translate(compose::ui::Offset::ZERO - outer.top_left());
    let (left, top) = (column.left.round() as i32, column.top.round() as i32);
    let (right, bottom) = (column.right.round() as i32, column.bottom.round() as i32);
    let (center_x, center_y) = ((left + right) / 2, (top + bottom) / 2);
    // One pixel past each side's center, the shadow, not the background.
    for (x, y) in [(left - 1, center_y), (center_x, top - 1), (right, center_y), (center_x, bottom)]
    {
        let color = image.pixel(x, y);
        assert_ne!(
            color, TEST_COLORS.background_color,
            "Expected pixel at [{x}, {y}] to not be the background."
        );
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColors_transparentExceptBackground_backgroundColorIsAsExpected
#[test]
fn whenContextMenuColors_transparentExceptBackground_backgroundColorIsAsExpected() {
    let colors = ContextMenuColors { background_color: TEST_COLOR, ..transparent_colors() };
    let mut ui = rule(move || test_column(colors, "Item", true, None));
    assert_colors(get_colors(&mut ui), &[TEST_COLOR]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColors_transparentExceptText_textColorIsAsExpected
#[test]
fn whenContextMenuColors_transparentExceptText_textColorIsAsExpected() {
    let colors = ContextMenuColors { text_color: TEST_COLOR, ..transparent_colors() };
    let mut ui = rule(move || test_column(colors, "MMMMMMMMMM", true, None));
    assert_colors(get_non_background_colors(&mut ui), &[TEST_COLOR]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColors_transparentExceptDisabledText_textColorIsAsExpected
#[test]
fn whenContextMenuColors_transparentExceptDisabledText_textColorIsAsExpected() {
    let colors = ContextMenuColors { disabled_text_color: TEST_COLOR, ..transparent_colors() };
    let mut ui = rule(move || test_column(colors, "MMMMMMMMMM", false, None));
    assert_colors(get_non_background_colors(&mut ui), &[TEST_COLOR]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColors_transparentExceptIcon_iconColorIsAsExpected
#[test]
fn whenContextMenuColors_transparentExceptIcon_iconColorIsAsExpected() {
    let colors = ContextMenuColors { icon_color: TEST_COLOR, ..transparent_colors() };
    let mut ui = rule(move || test_column(colors, "Item", true, Some(filling_icon())));
    assert_colors(get_non_background_colors(&mut ui), &[TEST_COLOR]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColors_transparentExceptDisabledIcon_iconColorIsAsExpected
#[test]
fn whenContextMenuColors_transparentExceptDisabledIcon_iconColorIsAsExpected() {
    let colors = ContextMenuColors { disabled_icon_color: TEST_COLOR, ..transparent_colors() };
    let mut ui = rule(move || test_column(colors, "Item", false, Some(filling_icon())));
    assert_colors(get_non_background_colors(&mut ui), &[TEST_COLOR]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColors_enabledToggled_colorsChangeAsExpected
#[test]
fn whenContextMenuColors_enabledToggled_colorsChangeAsExpected() {
    let (enabled_color, disabled_color) = (Color::RED, Color::BLUE);
    let colors = ContextMenuColors {
        text_color: enabled_color,
        icon_color: enabled_color,
        disabled_text_color: disabled_color,
        disabled_icon_color: disabled_color,
        ..transparent_colors()
    };
    let enabled = **Box::leak(Box::new(mutable_state_of(true)));
    let mut ui = rule(move || test_column(colors, "MMMMM", enabled.get(), Some(filling_icon())));
    for _ in 0..2 {
        enabled.set(true);
        ui.wait_for_idle();
        assert_colors(get_non_background_colors(&mut ui), &[enabled_color]);
        enabled.set(false);
        ui.wait_for_idle();
        assert_colors(get_non_background_colors(&mut ui), &[disabled_color]);
    }
}

/// Compose's `centeringPopupPositionProvider`.
struct Centering;

impl PopupPositionProvider for Centering {
    fn position(
        &self,
        _anchor: Rect,
        window: IntSize,
        _direction: LayoutDirection,
        popup: IntSize,
    ) -> IntOffset {
        IntOffset::new(window.width / 2 - popup.width / 2, window.height / 2 - popup.height / 2)
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuPopup_enabledDisabled_colorsAreAsExpected
#[test]
fn whenContextMenuPopup_enabledDisabled_colorsAreAsExpected() {
    let enabled = **Box::leak(Box::new(mutable_state_of(true)));
    let mut ui = rule(move || {
        let block: ContextMenuBuilderBlock = Rc::new(move |scope: &mut ContextMenuScope| {
            scope.item("MMMMMMMMMM", Modifier, enabled.get(), Some(filling_icon()), || {});
        });
        ContextMenuPopup(
            Rc::new(Centering),
            compose::runtime::callback(|| {}),
            Modifier.test_tag(TAG),
            TEST_COLORS,
            block,
        );
    });
    for _ in 0..2 {
        enabled.set(true);
        ui.wait_for_idle();
        assert_colors(
            get_colors(&mut ui),
            &[TEST_COLORS.background_color, TEST_COLORS.text_color, TEST_COLORS.icon_color],
        );
        enabled.set(false);
        ui.wait_for_idle();
        assert_colors(
            get_colors(&mut ui),
            &[
                TEST_COLORS.background_color,
                TEST_COLORS.disabled_text_color,
                TEST_COLORS.disabled_icon_color,
            ],
        );
    }
}
