//! Placing with a layer (`place_with_layer`, D1909) drawn by Skia, as Compose's
//! `GraphicsLayerTest` checks it on a device.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose::foundation::layout::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Color, GraphicsLayerScope, IntSize, LayerBlock, Layout, Modifier, Paint, UnitExt,
    measure_policy,
};

const TAG: &str = "placing";
const YELLOW: Color = Color::from_argb(0xFFFF_FF00);
const RED: Color = Color::from_argb(0xFF80_0000);

/// A 30 × 30 yellow layout placing a red box of `size` at `at` with the layer `block` gives.
fn placing(size: i32, at: i32, block: fn(&mut GraphicsLayerScope)) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    ComposeTestRule::sized(IntSize::new(100, 100), move || {
        Layout!(
            modifier = Modifier.test_tag(TAG).draw_behind(|scope| {
                let bounds = scope.bounds();
                scope.draw_rect(bounds, Paint::fill(YELLOW));
            }),
            measure_policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                scope.layout_with(30, 30, move |placement| {
                    placement.place_with_layer(&placeable, at, at, block);
                })
            }),
            {
                Box!(modifier = Modifier.size(size.dp()).background(RED));
            }
        );
    })
}

/// The 30 × 30 capture is `inner` in its 10 × 10 middle square and `outer` around it, as the
/// test's `validateSquareColors`.
fn assert_square(ui: &mut ComposeTestRule, outer: Color, inner: Color) {
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    image.assert_pixels(Some(IntSize::new(30, 30)), |at| {
        let inside = (10..20).contains(&at.x) && (10..20).contains(&at.y);
        Some(if inside { inner } else { outer })
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/GraphicsLayerTest.kt, layoutUsesPlaceWithLayer
#[test]
fn a_child_placed_with_a_layer_is_drawn_where_it_is_placed() {
    let mut ui = placing(10, 10, |_| {});
    assert_square(&mut ui, YELLOW, RED);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/GraphicsLayerTest.kt, layoutUsesPlaceWithLayerWithScale
#[test]
fn a_child_placed_with_a_scaled_layer_is_drawn_scaled() {
    let mut ui = placing(20, 5, |layer| {
        layer.scale_x = 0.5;
        layer.scale_y = 0.5;
    });
    assert_square(&mut ui, YELLOW, RED);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/GraphicsLayerTest.kt, layoutMovesPlacedWithLayerChild_noInvalidations
#[test]
fn moving_a_child_placed_with_a_layer_draws_it_not_again() {
    let (parent, child) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let offset = mutable_state_of(0);
    let (p, c, o) = (parent.clone(), child.clone(), offset.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        let (p, c) = (p.clone(), c.clone());
        let block = LayerBlock::new(|_| {});
        Layout!(
            modifier = Modifier.draw_with_content(move |scope| {
                scope.draw_content();
                p.set(p.get() + 1);
            }),
            measure_policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                let block = block.clone();
                scope.layout_with(30, 30, move |placement| {
                    placement.place_with_layer(&placeable, o.get(), o.get(), block.clone());
                })
            }),
            {
                let c = c.clone();
                Box!(modifier = Modifier.size(20.dp()).draw_behind(move |_| c.set(c.get() + 1)));
            }
        );
    });
    ui.wait_for_idle();
    assert_eq!((parent.get(), child.get()), (1, 1));
    offset.set(10);
    ui.wait_for_idle();
    // The child's layer moved without drawing it again. The parent draws again here (Compose's
    // doesn't): it holds the move, as a repaint boundary's parent does (docs/06-ui.md, "Repaint
    // boundaries").
    assert_eq!(child.get(), 1);
    assert_eq!(parent.get(), 2);
}
