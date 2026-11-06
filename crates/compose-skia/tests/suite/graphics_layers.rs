//! Graphics layers as objects, recorded and drawn by drawings, rendered by Skia, as Compose's
//! `AndroidGraphicsLayerTest` checks them on a device.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose::foundation::layout::*;
use compose::runtime::composable;
use compose::test::*;
use compose::ui::{
    BlendMode, Color, ColorFilter, CompositingStrategy, DrawScope, GraphicsLayer, IntOffset,
    IntSize, Modifier, Offset, Paint, Path, Rect, Size, UnitExt, remember_graphics_layer,
};

const TAG: &str = "layers";
const WIDTH: i32 = 600;
const HEIGHT: i32 = 400;

/// A 600 × 400 node over black drawing with `block`, given a layer remembered for it, as the
/// test's `graphicsLayerTest`.
fn layer_test(block: impl Fn(&mut DrawScope<'_>, &GraphicsLayer) + 'static) -> CapturedImage {
    set_capture_renderer(compose_skia::raster_colors);
    let block = Rc::new(block);
    let mut ui = ComposeTestRule::sized(IntSize::new(800, 600), move || {
        let layer = remember_graphics_layer();
        let block = block.clone();
        Box!(
            modifier = Modifier
                .test_tag(TAG)
                .size((WIDTH.dp(), HEIGHT.dp()))
                .background(Color::BLACK)
                .draw_behind(move |scope| block(scope, &layer))
        );
    });
    ui.on_node_with_tag(TAG).capture_to_image()
}

/// Each quadrant of `image` is the color given, at points inside it.
fn assert_quadrants(image: &CapturedImage, colors: [Color; 4]) {
    let (half_width, half_height) = (WIDTH / 2, HEIGHT / 2);
    for (index, color) in colors.into_iter().enumerate() {
        let (left, top) = ((index as i32 % 2) * half_width, (index as i32 / 2) * half_height);
        for (x, y) in [
            (left + 2, top + 2),
            (left + half_width - 3, top + 2),
            (left + 2, top + half_height - 3),
            (left + half_width - 3, top + half_height - 3),
            (left + half_width / 2, top + half_height / 2),
        ] {
            image.assert_pixel_color(color, x, y);
        }
    }
}

fn full(scope: &DrawScope<'_>) -> IntSize {
    IntSize::new(scope.size().width as i32, scope.size().height as i32)
}

fn fill(scope: &mut DrawScope<'_>, color: Color) {
    let bounds = scope.bounds();
    scope.draw_rect(bounds, Paint::fill(color));
}

/// The top left quarter of the drawing, in `color`.
fn quarter(scope: &mut DrawScope<'_>, color: Color) {
    let size = scope.size();
    scope.draw_rect(Rect::new(0.0, 0.0, size.width / 2.0, size.height / 2.0), Paint::fill(color));
}

const RED: Color = Color::RED;
const BLACK: Color = Color::BLACK;

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testDrawLayer, testRecordOverwritesPreviousRecord
#[test]
fn a_recorded_layer_draws_its_last_recording_where_it_is_drawn() {
    assert_eq!(GraphicsLayer::new().size(), IntSize::ZERO);
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        layer.record(scope, full(scope), |scope| fill(scope, Color::BLUE));
        assert_eq!((layer.size(), layer.top_left()), (full(scope), IntOffset::new(0, 0)));
        scope.draw_layer(layer);
    });
    assert_quadrants(&image, [Color::BLUE; 4]);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testRecordLayerWithSize, testRecordLayerWithOffset, testSetOffset
#[test]
fn a_layer_has_its_recorded_size_and_is_drawn_at_its_top_left() {
    let image = layer_test(|scope, layer| {
        layer.record(scope, IntSize::new(WIDTH / 2, HEIGHT / 2), |scope| fill(scope, RED));
        scope.draw_layer(layer);
    });
    assert_quadrants(&image, [RED, BLACK, BLACK, BLACK]);
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        layer.set_top_left(IntOffset::new(WIDTH / 2, HEIGHT / 2));
        scope.draw_layer(layer);
    });
    assert_quadrants(&image, [BLACK, BLACK, BLACK, RED]);
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| {
            scope.inset(0.0, 0.0, -4.0, -4.0, |scope| fill(scope, RED));
        });
        layer.set_top_left(IntOffset::new(4, 4));
        scope.draw_layer(layer);
    });
    image.assert_pixel_color(RED, 5, 5);
    image.assert_pixel_color(BLACK, 5, 3);
    image.assert_pixel_color(BLACK, 3, 5);
    image.assert_pixel_color(RED, WIDTH - 2, HEIGHT - 2);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testSetAlpha
#[test]
fn a_layers_alpha_fades_it() {
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        layer.set_alpha(0.5);
        scope.draw_layer(layer);
    });
    let pixel = image.pixel(WIDTH / 2, HEIGHT / 2);
    let expected = RED.with_alpha(0.5).composite_over(BLACK);
    assert!(pixel.red().abs_diff(expected.red()) <= 2, "{pixel:?} against {expected:?}");
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testSetScaleX, testSetScaleY, testTranslationX, testTranslationY
#[test]
fn a_layer_scales_and_moves() {
    let scaled = |x: f32, y: f32| {
        layer_test(move |scope, layer| {
            layer.record(scope, full(scope), |scope| quarter(scope, RED));
            layer.set_scale_x(x);
            layer.set_scale_y(y);
            layer.set_pivot_offset(Offset::ZERO);
            scope.draw_layer(layer);
        })
    };
    assert_quadrants(&scaled(2.0, 1.0), [RED, RED, BLACK, BLACK]);
    assert_quadrants(&scaled(1.0, 2.0), [RED, BLACK, RED, BLACK]);
    let moved = |x: bool| {
        layer_test(move |scope, layer| {
            layer.record(scope, full(scope), |scope| quarter(scope, RED));
            if x {
                layer.set_translation_x(WIDTH as f32 / 2.0);
            } else {
                layer.set_translation_y(HEIGHT as f32 / 2.0);
            }
            scope.draw_layer(layer);
        })
    };
    assert_quadrants(&moved(true), [BLACK, RED, BLACK, BLACK]);
    assert_quadrants(&moved(false), [BLACK, BLACK, RED, BLACK]);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testDefaultPivot, testBottomRightPivot, testResettingToDefaultPivot
#[test]
fn a_layer_scales_around_its_pivot_its_center_by_default() {
    let middle_doubled = |reset: bool| {
        layer_test(move |scope, layer| {
            layer.record(scope, full(scope), |scope| {
                let size = scope.size();
                scope.inset(
                    size.width / 4.0,
                    size.height / 4.0,
                    size.width / 4.0,
                    size.height / 4.0,
                    |scope| fill(scope, RED),
                );
            });
            layer.set_scale_x(2.0);
            layer.set_scale_y(2.0);
            if reset {
                layer.set_pivot_offset(Offset::new(WIDTH as f32, HEIGHT as f32));
                layer.set_pivot_offset(Offset::UNSPECIFIED);
            }
            scope.draw_layer(layer);
        })
    };
    assert_quadrants(&middle_doubled(false), [RED; 4]);
    assert_quadrants(&middle_doubled(true), [RED; 4]);
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        layer.set_scale_x(0.5);
        layer.set_scale_y(0.5);
        layer.set_pivot_offset(Offset::new(WIDTH as f32, HEIGHT as f32));
        scope.draw_layer(layer);
    });
    assert_quadrants(&image, [BLACK, BLACK, BLACK, RED]);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testRotationZ
#[test]
fn a_layer_rotates_around_its_center() {
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| {
            let center = scope.center();
            let rect =
                Rect::new(center.x - 50.0, center.y - 50.0, center.x + 50.0, center.y + 50.0);
            scope.draw_rect(rect, Paint::fill(RED));
        });
        layer.set_rotation_z(45.0);
        scope.draw_layer(layer);
    });
    let (x, y) = (WIDTH / 2, HEIGHT / 2);
    for (dx, dy) in [(0, 0), (0, -50), (0, 49), (-50, 0), (49, 0)] {
        image.assert_pixel_color(RED, x + dx, y + dy);
    }
    for (dx, dy) in [(-33, -46), (-33, 46), (33, -46), (33, 46)] {
        image.assert_pixel_color(BLACK, x + dx, y + dy);
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testUnboundedClip, testBoundedClip
#[test]
fn a_layer_clips_to_its_bounds_only_if_asked() {
    let huge = |clip: bool| {
        layer_test(move |scope, layer| {
            layer.record(scope, IntSize::new(WIDTH / 2, HEIGHT / 2), |scope| {
                scope.draw_rect(Rect::new(0.0, 0.0, 100_000.0, 100_000.0), Paint::fill(RED));
            });
            layer.set_clip(clip);
            scope.draw_layer(layer);
        })
    };
    assert_quadrants(&huge(false), [RED; 4]);
    assert_quadrants(&huge(true), [RED, BLACK, BLACK, BLACK]);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testRectOutlineClip, testRoundRectOutlineClip, testPathOutlineClip, testRectOutlineWithNonZeroTopLeft, testRoundRectOutlineWithNonZeroTopLeft
#[test]
fn a_layer_clips_to_its_outline() {
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        let center = scope.center();
        layer.set_rect_outline(center, Size::new(center.x, center.y));
        layer.set_clip(true);
        scope.draw_layer(layer);
    });
    assert_quadrants(&image, [BLACK, BLACK, BLACK, RED]);
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        layer.set_round_rect_outline(Offset::ZERO, Size::UNSPECIFIED, 50.0);
        layer.set_clip(true);
        scope.draw_layer(layer);
    });
    image.assert_pixel_color(BLACK, 2, 2);
    image.assert_pixel_color(RED, 50, 50);
    image.assert_pixel_color(BLACK, WIDTH - 3, HEIGHT - 3);
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        let triangle = Path::new();
        triangle.move_to(0.0, 0.0);
        triangle.line_to(WIDTH as f32, 0.0);
        triangle.line_to(0.0, HEIGHT as f32);
        triangle.close();
        layer.set_path_outline(&triangle);
        layer.set_clip(true);
        scope.draw_layer(layer);
    });
    image.assert_pixel_color(RED, 10, 10);
    image.assert_pixel_color(BLACK, WIDTH - 10, HEIGHT - 10);
    let outline = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| quarter(scope, RED));
        layer.set_top_left(IntOffset::new(20, 30));
        layer.set_rect_outline(Offset::ZERO, Size::UNSPECIFIED);
        assert_eq!(layer.outline().bounds(), Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32));
        layer.set_round_rect_outline(Offset::ZERO, Size::UNSPECIFIED, 0.0);
        assert_eq!(layer.outline().bounds(), Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32));
        scope.draw_layer(layer);
    });
    outline.assert_pixel_color(RED, 25, 35);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testElevation
#[test]
fn a_layer_with_an_elevation_casts_a_shadow() {
    let image = layer_test(|scope, layer| {
        let half = IntSize::new(WIDTH / 2, HEIGHT / 2);
        layer.record(scope, half, |scope| fill(scope, Color::WHITE));
        layer.set_shadow_elevation(10.0);
        fill(scope, Color::WHITE);
        scope.translate(WIDTH as f32 / 4.0, HEIGHT as f32 / 4.0, |scope| scope.draw_layer(layer));
    });
    let shaded = (WIDTH / 4..WIDTH * 3 / 4 + 20)
        .flat_map(|x| (HEIGHT / 4..HEIGHT * 3 / 4 + 20).map(move |y| (x, y)))
        .filter(|(x, y)| image.pixel(*x, *y) != Color::WHITE)
        .count();
    assert!(shaded > 0);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testCompositingStrategyAuto, testCompositingStrategyOffscreen, testCompositingStrategyModulateAlpha
#[test]
fn a_layers_compositing_strategy_decides_how_its_alpha_is_applied() {
    let overlapping = |alpha: f32, strategy: CompositingStrategy, background: Color| {
        layer_test(move |scope, layer| {
            layer.record(scope, full(scope), |scope| {
                let size = scope.size();
                let (third_x, third_y) = (size.width / 3.0, size.height / 3.0);
                scope.inset(0.0, 0.0, third_x, third_y, |scope| fill(scope, RED));
                let xor = Paint::fill(Color::BLUE).with_blend_mode(BlendMode::Xor);
                let blend = strategy == CompositingStrategy::Offscreen;
                scope.inset(third_x, third_y, 0.0, 0.0, |scope| {
                    let bounds = scope.bounds();
                    scope.draw_rect(bounds, if blend { xor } else { Paint::fill(Color::BLUE) });
                });
            });
            layer.set_alpha(alpha);
            layer.set_compositing_strategy(strategy);
            fill(scope, background);
            scope.draw_layer(layer);
        })
    };
    let near = |image: &CapturedImage, x: i32, y: i32, expected: Color| {
        let pixel = image.pixel(x, y);
        let close = |a: u8, b: u8| a.abs_diff(b) <= 2;
        assert!(
            close(pixel.red(), expected.red())
                && close(pixel.green(), expected.green())
                && close(pixel.blue(), expected.blue()),
            "({x}, {y}): {pixel:?} against {expected:?}"
        );
    };
    let (red, blue) = (RED.with_alpha(0.5), Color::BLUE.with_alpha(0.5));
    let center = (WIDTH / 2, HEIGHT / 2);
    let auto = overlapping(0.5, CompositingStrategy::Auto, BLACK);
    near(&auto, 0, 0, red.composite_over(BLACK));
    near(&auto, center.0, center.1, blue.composite_over(BLACK));
    near(&auto, WIDTH - 1, 0, BLACK);
    let modulated = overlapping(0.5, CompositingStrategy::ModulateAlpha, BLACK);
    near(&modulated, 0, 0, red.composite_over(BLACK));
    near(&modulated, center.0, center.1, blue.composite_over(red).composite_over(BLACK));
    let gray = Color::LIGHT_GRAY;
    let offscreen = overlapping(1.0, CompositingStrategy::Offscreen, gray);
    near(&offscreen, 0, 0, RED);
    near(&offscreen, WIDTH - 1, HEIGHT - 1, Color::BLUE);
    near(&offscreen, center.0, center.1, gray);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/layer/AndroidGraphicsLayerTest.kt, testTintColorFilter, testBlendMode
#[test]
fn a_layer_is_composited_through_its_color_filter_and_blend_mode() {
    let image = layer_test(|scope, layer| {
        layer.record(scope, full(scope), |scope| fill(scope, RED));
        layer.set_color_filter(Some(ColorFilter::tint(Color::BLUE)));
        scope.draw_layer(layer);
    });
    assert_quadrants(&image, [Color::BLUE; 4]);
    let image = layer_test(|scope, layer| {
        layer.record(scope, IntSize::new(WIDTH / 2, HEIGHT / 2), |scope| fill(scope, RED));
        layer.set_top_left(IntOffset::new(WIDTH / 4, HEIGHT / 4));
        layer.set_blend_mode(BlendMode::Xor);
        fill(scope, Color::GREEN);
        scope.draw_layer(layer);
        let blue = Paint::fill(Color::BLUE).with_blend_mode(BlendMode::DstOver);
        let bounds = scope.bounds();
        scope.draw_rect(bounds, blue);
    });
    image.assert_pixel_color(Color::GREEN, 0, 0);
    image.assert_pixel_color(Color::GREEN, WIDTH - 1, HEIGHT - 1);
    image.assert_pixel_color(Color::BLUE, WIDTH / 4 + 2, HEIGHT / 4 + 2);
    image.assert_pixel_color(Color::BLUE, WIDTH / 2, HEIGHT / 2);
}

/// A layer drawn twice draws twice; a change of its properties draws again what drew it,
/// without composing it again; a released layer draws nothing.
#[test]
fn a_layer_drawn_twice_and_changed_later_draws_again() {
    set_capture_renderer(compose_skia::raster_colors);
    let kept: Rc<Cell<Option<GraphicsLayer>>> = Rc::new(Cell::new(None));
    let keep = kept.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 100), move || {
        let layer = remember_graphics_layer();
        keep.set(Some(layer.clone()));
        Box!(
            modifier =
                Modifier.test_tag(TAG).size((200.dp(), 100.dp())).background(BLACK).draw_behind(
                    move |scope| {
                        if layer.size() == IntSize::ZERO {
                            layer.record(scope, IntSize::new(100, 100), |scope| fill(scope, RED));
                        }
                        scope.draw_layer(&layer);
                        scope.translate(100.0, 0.0, |scope| scope.draw_layer(&layer));
                    }
                )
        );
    });
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    image.assert_pixel_color(RED, 50, 50);
    image.assert_pixel_color(RED, 150, 50);
    let layer = kept.take().expect("the layer");
    layer.set_color_filter(Some(ColorFilter::tint(Color::GREEN)));
    ui.wait_for_idle();
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    image.assert_pixel_color(Color::GREEN, 50, 50);
    image.assert_pixel_color(Color::GREEN, 150, 50);
    layer.release();
    assert!(layer.is_released());
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixel_color(BLACK, 50, 50);
}

/// A box of `color()` drawn into `layer` and the layer drawn, as the test's `ColoredBox`
/// (`drawIntoLayer`).
#[composable]
fn ColoredBox(layer: GraphicsLayer, color: Rc<dyn Fn() -> Color>) {
    Box!(
        modifier = Modifier
            .size(10.dp())
            .test_tag("colored")
            .draw_with_content(move |scope| {
                scope.record_layer(&layer, |scope| scope.draw_content());
                scope.draw_layer(&layer);
            })
            .draw_behind(move |scope| fill(scope, color()))
    );
}

/// A white box drawing `layer`, as the test's `LayerDrawingBox`.
#[composable]
fn LayerDrawingBox(layer: GraphicsLayer) {
    Box!(
        modifier = Modifier.size(10.dp()).test_tag("drawing").draw_behind(move |scope| {
            fill(scope, Color::WHITE);
            scope.draw_layer(&layer);
        })
    );
}

/// The two boxes: the coloring one while `colored` or until `drawing`, the drawing one once
/// `drawing`.
fn prebuilt(
    colored: bool,
    color: Rc<dyn Fn() -> Color>,
    drawing: compose::runtime::MutableState<bool>,
) -> (ComposeTestRule, GraphicsLayer) {
    set_capture_renderer(compose_skia::raster_colors);
    let layer = GraphicsLayer::new();
    let kept = layer.clone();
    let ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        let (layer, color) = (kept.clone(), color.clone());
        Column!({
            if colored || !drawing.get() {
                ColoredBox(layer.clone(), color.clone());
            }
            if drawing.get() {
                LayerDrawingBox(layer.clone());
            }
        });
    });
    (ui, layer)
}

fn drawing_box(ui: &mut ComposeTestRule) -> CapturedImage {
    ui.on_node_with_tag("drawing").capture_to_image()
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/DrawingPrebuiltGraphicsLayerTest.kt, continueDrawingPrebuiltLayer, sizeIsCorrect, drawingWithAlpha, updateLayerProperties
#[test]
fn a_layer_recorded_by_a_node_is_drawn_by_another_after_it_is_gone() {
    let drawing = compose::runtime::mutable_state_of(false);
    let (mut ui, layer) = prebuilt(false, Rc::new(|| RED), drawing.handle());
    assert_eq!(layer.size(), IntSize::new(10, 10));
    drawing.set(true);
    ui.wait_for_idle();
    drawing_box(&mut ui).assert_pixels(Some(IntSize::new(10, 10)), |_| Some(RED));
    layer.set_alpha(0.5);
    ui.wait_for_idle();
    let pixel = drawing_box(&mut ui).pixel(5, 5);
    let expected = RED.with_alpha(0.5).composite_over(Color::WHITE);
    assert!(pixel.green().abs_diff(expected.green()) <= 2, "{pixel:?} against {expected:?}");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/DrawingPrebuiltGraphicsLayerTest.kt, keepComposingTheNodeWeTookLayerFrom
#[test]
fn a_layer_recorded_again_by_its_node_is_drawn_again_by_another() {
    let color = compose::runtime::mutable_state_of(Color::BLUE);
    let handle = color.handle();
    let drawing = compose::runtime::mutable_state_of(false);
    let (mut ui, _layer) = prebuilt(true, Rc::new(move || handle.get()), drawing.handle());
    drawing.set(true);
    ui.wait_for_idle();
    drawing_box(&mut ui).assert_pixels(None, |_| Some(Color::BLUE));
    color.set(Color::GREEN);
    ui.wait_for_idle();
    ui.on_node_with_tag("colored").capture_to_image().assert_pixels(None, |_| Some(Color::GREEN));
    drawing_box(&mut ui).assert_pixels(None, |_| Some(Color::GREEN));
}
