//! Compose's `BasicTextBrushTest` (foundation, device tests): a `BasicText` whose style's brush
//! is toggled draws in the new brush, solid or a gradient, drawn by Skia in the sample font (as
//! Compose's test font) and checked for the colors its pixels hold.
//!
//! `toggleCustomShaderBrush` waits on a custom `ShaderBrush` (a brush making its own shader,
//! reading state as it does): compose-rs's `Brush` is solid or one of the gradients.

use compose::foundation::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{Brush, Color, Density, IntSize, Modifier, UnitExt};
use compose_skia::SkiaTextMeasurer;

const TAG: &str = "TAG";

/// "Hello" at 20 px in the sample font, painted with `brush()` (read as it's composed) over
/// black, at density 1.
fn hello(brush: impl Fn() -> Brush + 'static) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    let mut ui = compose_test_with(
        IntSize::new(400, 200),
        Density::new(1.0),
        std::boxed::Box::new(SkiaTextMeasurer::new()),
        move || {
            BasicText!(
                "Hello",
                style = TextStyle {
                    brush: Some(brush()),
                    font_family: Some(basic_measure_font().to_font_family()),
                    font_size: 20.sp(),
                    ..TextStyle::DEFAULT
                },
                modifier = Modifier.background(Color::BLACK).test_tag(TAG),
            );
        },
    );
    ui.wait_for_idle();
    ui
}

/// Black and `shown` in the text's pixels, not `hidden`.
fn assert_colors(ui: &mut ComposeTestRule, shown: Color, hidden: Color) {
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    image.assert_contains_color(Color::BLACK);
    image.assert_contains_color(shown);
    image.assert_does_not_contain_color(hidden);
}

/// The brush `first`, then `second`: red, then blue.
fn toggled(first: Brush, second: Brush) {
    let brush = mutable_state_of(first);
    let current = brush.handle();
    let mut ui = hello(move || current.get());
    assert_colors(&mut ui, Color::RED, Color::BLUE);
    brush.set(second);
    ui.wait_for_idle();
    assert_colors(&mut ui, Color::BLUE, Color::RED);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextBrushTest.kt, toggleSolidColorBrush
#[test]
fn toggle_solid_color_brush() {
    toggled(Brush::Solid(Color::RED), Brush::Solid(Color::BLUE));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/BasicTextBrushTest.kt, togglePredefinedShaderBrush
#[test]
fn toggle_predefined_shader_brush() {
    toggled(
        Brush::horizontal_gradient([Color::RED, Color::RED]),
        Brush::horizontal_gradient([Color::BLUE, Color::BLUE]),
    );
}
