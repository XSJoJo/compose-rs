//! Drop and inner shadows drawn by Skia, as Compose's `DropShadowTest`, `InnerShadowTest` and
//! their painters' tests check them.

#![allow(non_snake_case)]

use compose::foundation::CIRCLE_SHAPE;
use compose::foundation::layout::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::shadow::{DropShadowPainter, InnerShadowPainter, Shadow};
use compose::ui::{
    Alignment, Brush, Color, ColorFilter, ContentScale, Dp, DpOffset, IntSize, Modifier, Painter,
    Shape, UnitExt,
};

const TAG: &str = "shadowed";

/// A 100 px white node with a 40 px red box in its middle casting `shadow` of `shape`.
fn dropped(shape: Shape, shadow: Shadow) -> CapturedImage {
    set_capture_renderer(compose_skia::raster_colors);
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        let (shape, shadow) = (shape.clone(), shadow.clone());
        Box!(
            modifier = Modifier.test_tag(TAG).size(100.dp()).background(Color::WHITE),
            content_alignment = Alignment::CENTER,
            {
                Box!(
                    modifier = Modifier
                        .size(40.dp())
                        .drop_shadow(shape.clone(), shadow.clone())
                        .background(Color::RED)
                );
            }
        );
    });
    ui.on_node_with_tag(TAG).capture_to_image()
}

/// How dark a pixel is: 0 white, 255 black.
fn darkness(color: Color) -> u8 {
    255 - color.green()
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/DropShadowTest.kt, dropShadowDrawn
#[test]
fn a_drop_shadow_darkens_around_its_shape() {
    let image = dropped(Shape::Rectangle, Shadow::new(Dp(8.0)));
    image.assert_pixel_color(Color::RED, 50, 50);
    image.assert_pixel_color(Color::WHITE, 2, 2);
    let near = darkness(image.pixel(28, 50));
    assert!(near > 30 && near < 255, "the blurred edge: {}", near);
    assert!(darkness(image.pixel(20, 50)) < near, "fading out");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/DropShadowScreenShotTest.kt, dropShadowSpread, dropShadowOffset
#[test]
fn a_drop_shadow_spreads_and_moves() {
    let plain = dropped(Shape::Rectangle, Shadow::new(Dp(4.0)));
    let spread = dropped(Shape::Rectangle, Shadow { spread: Dp(6.0), ..Shadow::new(Dp(4.0)) });
    assert!(darkness(spread.pixel(25, 50)) > darkness(plain.pixel(25, 50)));
    let moved = dropped(
        Shape::Rectangle,
        Shadow { offset: DpOffset::new(Dp(8.0), Dp(0.0)), ..Shadow::new(Dp(4.0)) },
    );
    assert!(darkness(moved.pixel(73, 50)) > darkness(moved.pixel(27, 50)));
}

/// A circle's shadow follows the circle: the box's corner, outside it, stays light.
#[test]
fn a_drop_shadow_follows_its_shape() {
    let image = dropped(CIRCLE_SHAPE, Shadow::new(Dp(4.0)));
    let (edge, corner) = (darkness(image.pixel(50, 27)), darkness(image.pixel(28, 28)));
    assert!(edge as u16 > corner as u16 + 20, "{edge} {corner}");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/DropShadowScreenShotTest.kt, dropShadowGradientBrush
#[test]
fn a_drop_shadow_is_painted_by_its_brush() {
    let brush = Brush::horizontal_gradient([Color::RED, Color::BLUE]);
    let image =
        dropped(Shape::Rectangle, Shadow { spread: Dp(4.0), ..Shadow::brushed(Dp(6.0), brush) });
    let (left, right) = (image.pixel(27, 50), image.pixel(73, 50));
    assert!(left.red() > left.blue(), "{left:?}");
    assert!(right.blue() > right.red(), "{right:?}");
}

/// A 60 px white box with `shadow` inside its rectangle.
fn inner(shadow: Shadow) -> CapturedImage {
    set_capture_renderer(compose_skia::raster_colors);
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        Box!(
            modifier = Modifier
                .test_tag(TAG)
                .size(60.dp())
                .background(Color::WHITE)
                .inner_shadow(Shape::Rectangle, shadow.clone())
        );
    });
    ui.on_node_with_tag(TAG).capture_to_image()
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/InnerShadowTest.kt, innerShadowDrawn
#[test]
fn an_inner_shadow_darkens_inside_its_edges() {
    let image = inner(Shadow::new(Dp(8.0)));
    let edge = darkness(image.pixel(1, 30));
    assert!(edge > 60, "{edge}");
    assert!(darkness(image.pixel(8, 30)) < edge, "fading in");
    image.assert_pixel_color(Color::WHITE, 30, 30);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/InnerShadowScreenShotTest.kt, innerShadowSpread, innerShadowOffset
#[test]
fn an_inner_shadow_spreads_inward_and_moves() {
    let plain = inner(Shadow::new(Dp(4.0)));
    let spread = inner(Shadow { spread: Dp(8.0), ..Shadow::new(Dp(4.0)) });
    assert!(darkness(spread.pixel(8, 30)) > darkness(plain.pixel(8, 30)) + 50);
    let moved = inner(Shadow { offset: DpOffset::new(Dp(8.0), Dp(0.0)), ..Shadow::new(Dp(4.0)) });
    assert!(darkness(moved.pixel(4, 30)) > darkness(moved.pixel(56, 30)) + 50);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/DropShadowTest.kt, dropShadowRedrawnOnChange
#[test]
fn a_new_shadow_draws_again() {
    set_capture_renderer(compose_skia::raster_colors);
    let color = mutable_state_of(Color::BLACK);
    let handle = color.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        Box!(
            modifier = Modifier.test_tag(TAG).size(100.dp()).background(Color::WHITE),
            content_alignment = Alignment::CENTER,
            {
                Box!(
                    modifier = Modifier
                        .size(40.dp())
                        .drop_shadow(Shape::Rectangle, Shadow::colored(Dp(4.0), handle.get()))
                        .background(Color::RED)
                );
            }
        );
    });
    let before = ui.on_node_with_tag(TAG).capture_to_image().pixel(28, 50);
    color.set(Color::BLUE);
    ui.wait_for_idle();
    let after = ui.on_node_with_tag(TAG).capture_to_image().pixel(28, 50);
    assert!(after.blue() > after.red() && before.blue() == before.red(), "{before:?} {after:?}");
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/shadow/DropShadowPainterTest.kt, testDropShadowPainterAlpha, testDropShadowPainterColorFilter
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/shadow/InnerShadowPainterTest.kt, testInnerShadowPainterColor
#[test]
fn the_shadow_painters_take_their_alpha_and_filter() {
    set_capture_renderer(compose_skia::raster_colors);
    let painted = |painter: Painter, alpha: f32, filter: Option<ColorFilter>| {
        let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
            Box!(
                modifier =
                    Modifier.test_tag(TAG).size(60.dp()).background(Color::WHITE).paint_with(
                        painter.clone(),
                        true,
                        Alignment::CENTER,
                        ContentScale::FillBounds,
                        alpha,
                        filter
                    )
            );
        });
        ui.on_node_with_tag(TAG).capture_to_image()
    };
    let inset = |shadow: Shadow| Painter::new(InnerShadowPainter::new(Shape::Rectangle, shadow));
    let opaque = painted(inset(Shadow::new(Dp(6.0))), 1.0, None).pixel(1, 30);
    let faded = painted(inset(Shadow::new(Dp(6.0))), 0.5, None).pixel(1, 30);
    assert!(darkness(faded) < darkness(opaque), "{faded:?} {opaque:?}");
    let tinted = painted(inset(Shadow::new(Dp(6.0))), 1.0, Some(ColorFilter::tint(Color::BLUE)));
    let edge = tinted.pixel(1, 30);
    assert!(edge.blue() > edge.red() + 50, "{edge:?}");
    let dropped = Painter::new(DropShadowPainter::new(CIRCLE_SHAPE, Shadow::new(Dp(4.0))));
    painted(dropped, 1.0, None).assert_pixel_color(Color::BLACK, 30, 30);
}
