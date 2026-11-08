//! Compose's `SubcomposeLayoutTest` (ui, device tests), its drawing-order tests: a slot's layouts
//! drawn in the order they're placed, or by their `z_index`, drawn by Skia and checked at the
//! middle pixel. The rest of the file is in compose-foundation's `subcompose_layout.rs`.

#![allow(non_snake_case)]

use compose::foundation::layout::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::subcompose::*;
use compose::ui::{Color, IntSize, Modifier, Placeable, UnitExt};

/// A red and a green 10 dp square, each a slot, placed on each other: in the order `red_first`
/// says, the red one of a `z_index` of `red_z`.
fn two_squares(red_first: bool, red_z: Option<f32>) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    let square = |color: Color, z: Option<f32>| {
        Callback::new(move || {
            let mut modifier = Modifier.required_size((10.dp(), 10.dp())).background(color);
            if let Some(z) = z {
                modifier = modifier.z_index(z);
            }
            Box!(modifier = modifier);
        })
    };
    let (red, green) = (square(Color::RED, red_z), square(Color::GREEN, None));
    let policy = subcompose_policy(move |scope, constraints| {
        let mut first = |key: u32, content: &Callback| -> Placeable {
            let measurable = scope.subcompose(key, content.clone())[0];
            scope.measure(measurable, constraints)
        };
        let (first, second) = (first(0, &red), first(1, &green));
        if red_first {
            scope.place(&first, 0, 0);
            scope.place(&second, 0, 0);
        } else {
            scope.place(&second, 0, 0);
            scope.place(&first, 0, 0);
        }
        scope.layout(first.width, first.height)
    });
    let mut ui = ComposeTestRule::sized(IntSize::new(30, 30), move || {
        SubcomposeLayout!(modifier = Modifier.test_tag("layout"), measure_policy = policy.clone());
    });
    ui.wait_for_idle();
    ui
}

fn center_pixel(ui: &mut ComposeTestRule) -> Color {
    let image = ui.on_node_with_tag("layout").capture_to_image();
    image.pixel(image.width() / 2, image.height() / 2)
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/SubcomposeLayoutTest.kt, drawingOrderIsControlledByPlaceCalls
#[test]
fn what_s_placed_last_is_drawn_on_top() {
    let mut ui = two_squares(false, None);
    assert_eq!(center_pixel(&mut ui), Color::RED);
    let mut ui = two_squares(true, None);
    assert_eq!(center_pixel(&mut ui), Color::GREEN);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/SubcomposeLayoutTest.kt, drawingOrderCouldBeChangedUsingZIndex
#[test]
fn a_z_index_draws_a_slot_on_top_of_what_s_placed_after_it() {
    let mut ui = two_squares(true, Some(1.0));
    assert_eq!(center_pixel(&mut ui), Color::RED);
}
