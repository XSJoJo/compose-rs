//! `Modifier.blur` and blur render effects drawn by Skia, as Compose's `BlurTest`.

#![allow(non_snake_case)]

use compose::foundation::CIRCLE_SHAPE;
use compose::foundation::layout::*;
use compose::test::*;
use compose::ui::{
    BlurredEdgeTreatment, Color, Dp, IntSize, Modifier, Paint, RenderEffect, Shape, TileMode,
    UnitExt,
};

/// Two 100 px boxes, black with a 2 px padding, drawing a blue square inset by the 4 px blur:
/// one blurred with `Modifier.blur(4 px, edge_treatment)`, the other by a graphics layer with
/// the blur effect, `clip` and `shape` given, as the test's `testBlurEdgeTreatment`.
fn blurred_and_layered(
    edge_treatment: BlurredEdgeTreatment,
    shape: Shape,
    clip: bool,
    tile_mode: TileMode,
) -> (CapturedImage, CapturedImage) {
    set_capture_renderer(compose_skia::raster_colors);
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 150), move || {
        let draw = |scope: &mut compose::ui::DrawScope<'_>| {
            scope.inset(4.0, 4.0, 4.0, 4.0, |scope| {
                let bounds = scope.bounds();
                scope.draw_rect(bounds, Paint::fill(Color::BLUE));
            });
        };
        let chain = || Modifier.size(100.dp()).background(Color::BLACK).padding(2.dp());
        let (edge_treatment, shape) = (edge_treatment.clone(), shape.clone());
        Row!({
            Box!(
                modifier = Modifier
                    .test_tag("blur")
                    .then(chain())
                    .blur_with(Dp(4.0), Dp(4.0), edge_treatment.clone())
                    .draw_behind(draw)
            );
            let shape = shape.clone();
            Box!(
                modifier = Modifier
                    .test_tag("layer")
                    .then(chain())
                    .graphics_layer(move |layer| {
                        layer.set_render_effect(Some(RenderEffect::blur(4.0, 4.0, tile_mode)));
                        layer.clip = clip;
                        layer.shape = shape.clone();
                    })
                    .draw_behind(draw)
            );
        });
    });
    let blurred = ui.on_node_with_tag("blur").capture_to_image();
    let layered = ui.on_node_with_tag("layer").capture_to_image();
    (blurred, layered)
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/BlurTest.kt, testRectBoundedBlur, testUnboundedBlur, testCircleBoundedBlur
#[test]
fn a_blur_is_its_edge_treatments_layer() {
    for (treatment, shape, clip, tile_mode) in [
        (BlurredEdgeTreatment::RECTANGLE, Shape::Rectangle, true, TileMode::Clamp),
        (BlurredEdgeTreatment::UNBOUNDED, Shape::Rectangle, false, TileMode::Decal),
        (BlurredEdgeTreatment::new(CIRCLE_SHAPE), CIRCLE_SHAPE, true, TileMode::Clamp),
    ] {
        let (blurred, layered) = blurred_and_layered(treatment.clone(), shape, clip, tile_mode);
        assert_eq!(blurred.size(), layered.size());
        assert_eq!(blurred.pixels(), layered.pixels(), "{treatment:?}");
        // Blurred: the square's edge fades from blue to black.
        let edge = blurred.pixel(6, 48);
        assert!(edge.blue() > 20 && edge.blue() < 235, "{treatment:?}: {edge:?}");
        let middle = blurred.pixel(48, 48);
        assert_eq!(middle, Color::BLUE, "{treatment:?}");
    }
}

/// A clamped blur repeats its layer's edge into the blur; an unbounded one blurs from
/// transparent, so its corners fade.
#[test]
fn a_bounded_blur_keeps_its_edges_and_an_unbounded_one_fades_them() {
    set_capture_renderer(compose_skia::raster_colors);
    let corner = |treatment: BlurredEdgeTreatment| {
        let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
            Box!(
                modifier = Modifier
                    .test_tag("blur")
                    .size(50.dp())
                    .blur_with(Dp(8.0), Dp(8.0), treatment.clone())
                    .background(Color::BLUE)
            );
        });
        ui.on_node_with_tag("blur").capture_to_image().pixel(0, 0)
    };
    assert_eq!(corner(BlurredEdgeTreatment::RECTANGLE), Color::BLUE);
    let faded = corner(BlurredEdgeTreatment::UNBOUNDED);
    assert!(faded.red() > 20, "white shows through: {faded:?}");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/BlurTest.kt, testNoopBlur, testRectangleBlurredEdgeTreatmentHasShape, testUnboundedBlurredEdgeTreatmentDoesNotHaveShape
#[test]
fn no_radius_is_no_blur_and_the_treatments_have_their_shapes() {
    let unchanged = Modifier.blur_with(Dp(0.0), Dp(0.0), BlurredEdgeTreatment::UNBOUNDED);
    assert_eq!(format!("{unchanged:?}"), format!("{:?}", Modifier));
    assert!(BlurredEdgeTreatment::RECTANGLE.shape.is_some());
    assert!(BlurredEdgeTreatment::UNBOUNDED.shape.is_none());
}

/// An unbounded blur reaches past its node's edges, over what's around it; a bounded one
/// stays inside them.
#[test]
fn an_unbounded_blur_bleeds_past_its_edges() {
    set_capture_renderer(compose_skia::raster_colors);
    let outside = |treatment: BlurredEdgeTreatment| {
        let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
            let treatment = treatment.clone();
            Box!(
                modifier = Modifier.test_tag("around").size(100.dp()).background(Color::WHITE),
                content_alignment = compose::ui::Alignment::CENTER,
                {
                    Box!(
                        modifier = Modifier
                            .size(50.dp())
                            .blur_with(Dp(8.0), Dp(8.0), treatment.clone())
                            .background(Color::BLUE)
                    );
                }
            );
        });
        // Two pixels left of the blurred box, which spans 25 to 75.
        ui.on_node_with_tag("around").capture_to_image().pixel(23, 50)
    };
    assert_eq!(outside(BlurredEdgeTreatment::RECTANGLE), Color::WHITE);
    let bled = outside(BlurredEdgeTreatment::UNBOUNDED);
    assert!(bled.red() < 235 && bled.blue() > bled.red(), "blue bleeds out: {bled:?}");
}

/// A blur of no radius with a shape blurs nothing but still clips to the shape, as Compose's
/// `Modifier.blur` keeps its layer (`|| clip`, `Blur.kt`): a drawing past the node's bounds is
/// cut at them. Unbounded, it's no layer at all (`testNoopBlur`).
#[test]
fn a_blur_of_no_radius_still_clips_to_its_shape() {
    set_capture_renderer(compose_skia::raster_colors);
    let past_the_edge = |treatment: BlurredEdgeTreatment| {
        let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
            let treatment = treatment.clone();
            Box!(
                modifier = Modifier.test_tag("around").size(100.dp()).background(Color::WHITE),
                content_alignment = compose::ui::Alignment::CENTER,
                {
                    Box!(
                        modifier = Modifier
                            .size(50.dp())
                            .blur_with(Dp(0.0), Dp(0.0), treatment.clone())
                            .draw_behind(|scope| {
                                let size = scope.size();
                                let outside = compose::ui::Rect::new(
                                    -10.0,
                                    -10.0,
                                    size.width + 10.0,
                                    size.height + 10.0,
                                );
                                scope.draw_rect(outside, Paint::fill(Color::BLUE));
                            })
                    );
                }
            );
        });
        let image = ui.on_node_with_tag("around").capture_to_image();
        image.assert_pixel_color(Color::BLUE, 50, 50);
        image.pixel(20, 50)
    };
    assert_eq!(past_the_edge(BlurredEdgeTreatment::RECTANGLE), Color::WHITE);
    assert_eq!(past_the_edge(BlurredEdgeTreatment::UNBOUNDED), Color::BLUE);
}
