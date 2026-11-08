//! Compose's `AdvancedBorderRendererTest` (foundation, device tests): `BorderRenderer::draw_border`
//! called from a `draw_with_content` over triangle, generic-path, rounded and non-simple rounded
//! outlines, an alpha tint, outline bounds that don't start at the origin, inset draw scopes,
//! outlines, widths and brushes changed between frames, drawn by Skia and checked pixel by pixel.
//!
//! `drawBorder_changeProvidedGraphicsLayer_usesLatestGraphicsLayer` is n/a (and `@Ignore`d in
//! Compose, b/570069301): compose-rs's `BorderRenderer` takes no graphics layer provider; it
//! clips a generic outline's stroke to the path instead of drawing it in an offscreen layer.

use std::rc::Rc;

use compose::foundation::layout::Box as Container;
use compose::foundation::shape::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Alignment, BorderRenderer, Brush, Color, ContentDrawScope, CornerRadius, CustomShape, Density,
    Dp, IntOffset, IntSize, LOCAL_DENSITY, LayoutDirection, Modifier, Outline, Path, PathOperation,
    Rect, RoundRect, Shape, Size, UnitExt,
};

const TAG: &str = "BorderParent";

/// A test rule capturing with Skia, at a phone's density: some tests give sizes in dp and
/// radii in pixels, and expect a radius smaller than half the side.
fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    compose_test_with(
        DEFAULT_SIZE,
        Density::new(2.0),
        std::boxed::Box::new(compose::ui::platform::FixedTextMeasurer::new()),
        content,
    )
}

type Make = dyn Fn(Size, LayoutDirection) -> Outline;

/// A shape making its outline with a closure, as the test's `object : Shape`s.
struct FnShape(Rc<Make>);

impl CustomShape for FnShape {
    fn create_outline(&self, size: Size, direction: LayoutDirection, _: &Density) -> Outline {
        (self.0)(size, direction)
    }

    fn same(&self, other: &dyn CustomShape) -> bool {
        (other as &dyn std::any::Any)
            .downcast_ref::<FnShape>()
            .is_some_and(|other| Rc::ptr_eq(&self.0, &other.0))
    }
}

fn shape(make: impl Fn(Size, LayoutDirection) -> Outline + 'static) -> Shape {
    Shape::custom(FnShape(Rc::new(make)))
}

fn outline_of(shape: &Shape, scope: &ContentDrawScope<'_, '_>) -> Outline {
    shape.outline_in(scope.size(), &scope.density(), scope.layout_direction())
}

fn whole(size: Size) -> Rect {
    Rect::new(0.0, 0.0, size.width, size.height)
}

/// Pixels as dp in the draw's density, as the test's `Float.toDp()`.
fn px(scope: &ContentDrawScope<'_, '_>, value: f32) -> Dp {
    scope.density().to_dp(value)
}

/// As `IntRect(left, top, right, bottom).contains(pos)`: the right and bottom edges excluded.
fn contains(rect: (i32, i32, i32, i32), pos: IntOffset) -> bool {
    pos.x >= rect.0 && pos.x < rect.2 && pos.y >= rect.1 && pos.y < rect.3
}

/// A box of `size` px (as the test's `Modifier.size(40.0f.toDp(), 40.0f.toDp())`) with a blue
/// background, drawing its content then `draw`, in a box tagged [`TAG`] inside another box (the
/// test's `SemanticParent`).
fn in_semantic_parent(
    size: f32,
    draw: impl Fn(&mut ContentDrawScope<'_, '_>) + 'static,
) -> ComposeTestRule {
    let draw = Rc::new(draw);
    rule(move || {
        let draw = draw.clone();
        Container(
            Modifier,
            Alignment::TOP_START,
            false,
            callback(move |_| {
                let draw = draw.clone();
                Container(
                    Modifier.test_tag(TAG),
                    Alignment::TOP_START,
                    false,
                    callback(move |_| {
                        let side = LOCAL_DENSITY.current().to_dp(size);
                        let draw = draw.clone();
                        Container(
                            Modifier.size((side, side)).background(Color::BLUE).draw_with_content(
                                move |scope| {
                                    scope.draw_content();
                                    draw(scope);
                                },
                            ),
                            Alignment::TOP_START,
                            false,
                            callback(|_| {}),
                        );
                    }),
                );
            }),
        );
    })
}

/// A border of `width` px painted red inside `outline`.
fn red_border(scope: &mut ContentDrawScope<'_, '_>, width: f32, outline: &Outline) {
    let width = px(scope, width);
    BorderRenderer::new().draw_border(scope, width, &Brush::Solid(Color::RED), outline);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_triangle_shape
#[test]
fn draw_border_triangle_shape() {
    let triangle = GenericShape(|path, size, _| {
        path.line_to(size.width, 0.0);
        path.line_to(size.width, size.height);
        path.close();
    });
    let mut ui = rule(move || {
        let triangle = triangle.clone();
        let density = LOCAL_DENSITY.current();
        let side = density.to_dp(100.0);
        Container(
            Modifier
                .test_tag(TAG)
                .required_size((side, side))
                .background(Color::WHITE)
                .draw_with_content(move |scope| {
                    scope.draw_content();
                    let outline = outline_of(&triangle, scope);
                    red_border(scope, 10.0, &outline);
                }),
            Alignment::TOP_START,
            false,
            callback(|_| {}),
        );
    });
    let (offset_left, offset_right, offset_top, offset_bottom) = (5, 2, 2, 5);
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (width, height) = (image.width(), image.height());
    assert_eq!(image.pixel(offset_left, offset_top), Color::RED, "top left");
    assert_eq!(image.pixel(width - offset_right, offset_top), Color::RED, "top right");
    assert_eq!(
        image.pixel(width - offset_right, height - offset_bottom),
        Color::RED,
        "bottom right"
    );
    // Inside the triangle.
    assert_eq!(image.pixel((width as f32 * 3.0 / 4.0).floor() as i32, height / 2), Color::WHITE);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_non_simple_rounded_rect
#[test]
fn draw_border_non_simple_rounded_rect() {
    let (top_left, top_right, bottom_right, bottom_left) = (0.0, 5.0, 10.0, 15.0);
    let border = 10.dp();
    let mut ui = rule(move || {
        Container(
            Modifier
                .test_tag("roundRectTag")
                .size(50.dp())
                .background(Color::WHITE)
                .draw_with_content(move |scope| {
                    scope.draw_content();
                    let outline = outline_of(
                        &RoundedCornerShape((top_left, top_right, bottom_right, bottom_left)),
                        scope,
                    );
                    BorderRenderer::new().draw_border(
                        scope,
                        border,
                        &Brush::Solid(Color::RED),
                        &outline,
                    );
                }),
            Alignment::TOP_START,
            false,
            callback(|_| {}),
        );
    });
    let border_px = ui.density().to_px(border) as i32;
    let image = ui.on_node_with_tag("roundRectTag").capture_to_image();
    let (width, height) = (image.width(), image.height());
    let offset = 2;
    assert_eq!(image.pixel(offset, offset), Color::RED);
    assert_eq!(image.pixel(width - 1, 0), Color::WHITE);
    assert_eq!(image.pixel(width - 1, height - 1), Color::WHITE);
    assert_eq!(image.pixel(0, height - 1), Color::WHITE);

    assert_eq!(image.pixel(border_px + offset, border_px + offset), Color::WHITE);
    assert_eq!(image.pixel(width - border_px - offset, border_px + offset), Color::WHITE);
    assert_eq!(image.pixel(width - border_px - offset, height - border_px - offset), Color::WHITE);
    assert_eq!(image.pixel(border_px + offset, height - border_px - offset), Color::WHITE);

    let top_right_offset = (top_right / 2.0f32).round() as i32;
    assert_eq!(image.pixel(width - 1 - top_right_offset, top_right_offset), Color::RED);
    let bottom_right_offset = (bottom_right / 2.0f32).round() as i32;
    assert_eq!(
        image.pixel(width - 1 - bottom_right_offset, height - 1 - bottom_right_offset),
        Color::RED
    );
    let bottom_left_offset = (bottom_left / 2.0f32).round() as i32;
    assert_eq!(image.pixel(bottom_left_offset, height - 1 - bottom_left_offset), Color::RED);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_color_with_alpha_tinted_properly
#[test]
fn draw_border_color_with_alpha_tinted_properly() {
    let background = Color::RED;
    let border_color = Color::BLUE.with_alpha(0.5);
    let square = GenericShape(|path, size, _| {
        path.line_to(size.width, 0.0);
        path.line_to(size.width, size.height);
        path.line_to(0.0, size.height);
        path.close();
    });
    let mut ui = rule(move || {
        let square = square.clone();
        Container(
            Modifier
                .test_tag(TAG)
                .required_size((100.dp(), 100.dp()))
                .background(background)
                .draw_with_content(move |scope| {
                    scope.draw_content();
                    let outline = outline_of(&square, scope);
                    BorderRenderer::new().draw_border(
                        scope,
                        10.dp(),
                        &Brush::Solid(border_color),
                        &outline,
                    );
                }),
            Alignment::TOP_START,
            false,
            callback(|_| {}),
        );
    });
    let assert_color_equals = |expected: Color, actual: Color| {
        for (expected, actual) in [
            (expected.red(), actual.red()),
            (expected.green(), actual.green()),
            (expected.blue(), actual.blue()),
            (expected.alpha(), actual.alpha()),
        ] {
            assert!((expected - actual).abs() <= 0.01, "{expected} against {actual}");
        }
    };
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (width, height) = (image.width(), image.height());
    let expected = border_color.composite_over(background);
    assert_color_equals(expected, image.pixel(0, 0));
    assert_color_equals(expected, image.pixel(width - 1, 0));
    assert_color_equals(expected, image.pixel(width - 1, height - 1));
    assert_color_equals(expected, image.pixel(0, height - 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_generic_shape_color_to_brush
#[test]
fn draw_border_generic_shape_color_to_brush() {
    let gradient = Brush::vertical_gradient([
        (0.0, Color::RED),
        (0.5, Color::RED),
        (0.5, Color::BLUE),
        (1.0, Color::BLUE),
    ]);
    let border_stroke = 5.dp();
    let toggle = mutable_state_of(false);
    let toggled = toggle.handle();
    let test_shape =
        GenericShape(|path, size, _| path.add_rect(Rect::new(0.0, 0.0, size.width, size.height)));
    let mut ui = rule(move || {
        let (gradient, test_shape) = (gradient.clone(), test_shape.clone());
        Container(
            Modifier
                .test_tag(TAG)
                .required_size((20.dp(), 20.dp()))
                .background(Color::WHITE)
                .draw_with_content(move |scope| {
                    scope.draw_content();
                    let brush =
                        if toggled.get() { gradient.clone() } else { Brush::Solid(Color::GREEN) };
                    let outline = outline_of(&test_shape, scope);
                    BorderRenderer::new().draw_border(scope, border_stroke, &brush, &outline);
                }),
            Alignment::TOP_START,
            false,
            callback(|_| {}),
        );
    });
    let half = (ui.density().to_px(border_stroke) / 2.0) as i32;
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (width, height) = (image.width(), image.height());
    assert_eq!(image.pixel(half, half), Color::GREEN);
    assert_eq!(image.pixel(width - half, half), Color::GREEN);
    assert_eq!(image.pixel(half, height - half), Color::GREEN);
    assert_eq!(image.pixel(width - half, height - half), Color::GREEN);
    assert_eq!(image.pixel(width / 2, height / 2), Color::WHITE);

    ui.run_on_idle(|_| toggle.set(true));

    let image = ui.on_node_with_tag(TAG).capture_to_image();
    assert_eq!(image.pixel(half, half), Color::RED);
    assert_eq!(image.pixel(width - half, half), Color::RED);
    assert_eq!(image.pixel(half, height - half), Color::BLUE);
    assert_eq!(image.pixel(width - half, height - half), Color::BLUE);
    assert_eq!(image.pixel(width / 2, height / 2), Color::WHITE);
}

/// The test's `calculateContainerShape`: a rounded rectangle with an arrow on its top, its tip
/// at half `arrow_tip`.
fn container_shape(
    density: Density,
    arrow_base_width: Dp,
    arrow_tip: Dp,
    arrow_length: Dp,
) -> Shape {
    let corner_radius = density.to_px(8.dp());
    let arrow_base_width = density.to_px(arrow_base_width);
    let arrow_length = density.to_px(arrow_length);
    let arrow_tip = density.to_px(arrow_tip);
    GenericShape(move |path, size, _| {
        let bounding_round_rect = Path::new();
        bounding_round_rect.add_round_rect(RoundRect::from_ltrb(
            0.0,
            0.0,
            size.width,
            size.height,
            CornerRadius::new(corner_radius, corner_radius),
        ));
        // The tip of the arrow is in the middle of the icon above.
        let arrow_tip_x = arrow_tip / 2.0;
        let bounding_arrow = Path::new();
        bounding_arrow.move_to(arrow_tip_x + arrow_base_width / 2.0, 0.0);
        bounding_arrow.relative_line_to(-arrow_base_width, 0.0);
        bounding_arrow.relative_line_to(arrow_base_width / 2.0, -arrow_length);
        bounding_arrow.relative_line_to(arrow_base_width / 2.0, arrow_length);
        path.op(&bounding_round_rect, &bounding_arrow, PathOperation::Union);
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_test_with_fixed_size_generic_shape
#[test]
fn draw_border_test_with_fixed_size_generic_shape() {
    let (bubble_width, bubble_height, border_width) = (80.dp(), 40.dp(), 10.dp());
    let (arrow_base_width, arrow_tip, arrow_length) = (24.dp(), 56.dp(), 12.dp());
    let mut ui = rule(move || {
        let bubble_with_arrow =
            container_shape(LOCAL_DENSITY.current(), arrow_base_width, arrow_tip, arrow_length);
        Container(
            Modifier
                .test_tag(TAG)
                .required_size((bubble_width, bubble_height))
                .background(Color::WHITE)
                .padding(compose::ui::PaddingValues {
                    top: arrow_length,
                    ..compose::ui::PaddingValues::ZERO
                })
                .draw_with_content(move |scope| {
                    scope.draw_content();
                    let outline = outline_of(&bubble_with_arrow, scope);
                    BorderRenderer::new().draw_border(
                        scope,
                        border_width,
                        &Brush::Solid(Color::RED),
                        &outline,
                    );
                }),
            Alignment::TOP_START,
            false,
            callback(|_| {}),
        );
    });
    let density = ui.density();
    let outline = container_shape(density, arrow_base_width, arrow_tip, arrow_length).outline_in(
        Size::new(density.to_px(bubble_width), density.to_px(bubble_height)),
        &density,
        LayoutDirection::Ltr,
    );
    let Outline::Generic(path, _) = &outline else { panic!("a generic outline: {outline:?}") };
    let path_bounds = path.bounds();
    let offset_y = -path_bounds.top;
    let arrow_tip = density.to_px(arrow_tip);
    let arrow_base_width = density.to_px(arrow_base_width);
    let arrow_length = density.to_px(arrow_length);
    let border_stroke = density.to_px(border_width);

    let arrow_tip_x = arrow_tip / 2.0;
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let at = |x: f32, y: f32| image.pixel(x as i32, y as i32);

    // Along the rounded rectangle, before the triangle.
    let mut current_x = arrow_tip_x + arrow_base_width / 2.0 - arrow_base_width;
    assert_eq!(at(current_x, offset_y + border_stroke / 2.0), Color::RED);
    // Halfway up the triangle's start, in the border.
    assert_eq!(
        at(current_x + arrow_base_width / 4.0, offset_y - arrow_length / 2.0 + border_stroke / 2.0),
        Color::RED
    );
    // The triangle's tip, in the border.
    current_x += arrow_base_width / 2.0;
    assert_eq!(at(current_x, offset_y - arrow_length + border_stroke / 2.0), Color::RED);
    // The rounded rectangle right below the triangle has no border.
    assert_eq!(at(current_x, offset_y + border_stroke / 2.0), Color::WHITE);
    // Halfway down the triangle's end, back into the rounded rectangle: in the border.
    assert_eq!(
        at(current_x + arrow_base_width / 4.0, offset_y - arrow_length / 4.0 + border_stroke / 2.0),
        Color::RED
    );
    // The triangle's base on the rounded rectangle, in the border.
    current_x += arrow_base_width / 2.0;
    assert_eq!(at(current_x, offset_y + border_stroke / 2.0), Color::RED);
}

/// A 4 px border red inside `outline_bounds`, blue elsewhere (inside `inset` too).
fn assert_square_border(
    ui: &mut ComposeTestRule,
    outline_bounds: (i32, i32, i32, i32),
    inset: (i32, i32, i32, i32),
) {
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(Some(IntSize::new(40, 40)), |pos| {
        Some(if contains(outline_bounds, pos) && !contains(inset, pos) {
            Color::RED
        } else {
            Color::BLUE
        })
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_rectangleOutline_respectsOutlineBounds
#[test]
fn draw_border_rectangle_outline_respects_outline_bounds() {
    let fixed = shape(|_, _| Outline::Rectangle(Rect::new(10.0, 10.0, 30.0, 30.0)));
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let outline = outline_of(&fixed, scope);
        red_border(scope, 4.0, &outline);
    });
    // A 4 px rectangle from (10, 10).
    assert_square_border(&mut ui, (10, 10, 30, 30), (14, 14, 26, 26));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_simpleRoundedOutline_respectsOutlineBounds
#[test]
fn draw_border_simple_rounded_outline_respects_outline_bounds() {
    let fixed = shape(|_, _| Outline::Rounded(Rect::new(10.0, 10.0, 30.0, 30.0), [8.0; 4]));
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let outline = outline_of(&fixed, scope);
        red_border(scope, 4.0, &outline);
    });
    // A 4 px rounded rectangle from (10, 10).
    let (outline_bounds, inset) = ((10, 10, 30, 30), (14, 14, 26, 26));
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(Some(IntSize::new(40, 40)), |pos| {
        let in_corner_square = !(18..22).contains(&pos.x) && !(18..22).contains(&pos.y);
        let in_extreme_corner = !(12..28).contains(&pos.x) && !(12..28).contains(&pos.y);
        if !contains(outline_bounds, pos) {
            Some(Color::BLUE)
        } else if in_extreme_corner {
            // Outside the ring's rounded corners: the background.
            Some(Color::BLUE)
        } else if in_corner_square {
            // Anti-aliased.
            None
        } else if contains(inset, pos) {
            Some(Color::BLUE)
        } else {
            Some(Color::RED)
        }
    });
}

/// A 4 px border in `outline_bounds` with its bottom right corner rounded (in `corner`, not
/// checked, its tip in `corner_tip` the background).
fn assert_bottom_right_rounded_border(
    ui: &mut ComposeTestRule,
    outline_bounds: (i32, i32, i32, i32),
    inset: (i32, i32, i32, i32),
    corner: (i32, i32, i32, i32),
    corner_tip: (i32, i32, i32, i32),
) {
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(Some(IntSize::new(40, 40)), |pos| {
        if contains(corner_tip, pos) {
            Some(Color::BLUE)
        } else if contains(corner, pos) {
            None
        } else if contains(outline_bounds, pos) && !contains(inset, pos) {
            Some(Color::RED)
        } else {
            Some(Color::BLUE)
        }
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_nonSimpleRoundedOutline_zeroOffsetOriginOutline_respectsOutlineBounds
#[test]
fn draw_border_non_simple_rounded_outline_zero_offset_origin_outline_respects_outline_bounds() {
    let fixed =
        shape(|_, _| Outline::Rounded(Rect::new(0.0, 0.0, 20.0, 20.0), [0.0, 0.0, 8.0, 0.0]));
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let outline = outline_of(&fixed, scope);
        red_border(scope, 4.0, &outline);
    });
    assert_bottom_right_rounded_border(
        &mut ui,
        (0, 0, 20, 20),
        (4, 4, 16, 16),
        (12, 12, 20, 20),
        (18, 18, 20, 20),
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_nonSimpleRoundedOutline_nonZeroOffsetOriginOutline_respectsOutlineBounds
#[test]
fn draw_border_non_simple_rounded_outline_non_zero_offset_origin_outline_respects_outline_bounds() {
    let fixed =
        shape(|_, _| Outline::Rounded(Rect::new(10.0, 10.0, 30.0, 30.0), [0.0, 0.0, 8.0, 0.0]));
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let outline = outline_of(&fixed, scope);
        red_border(scope, 4.0, &outline);
    });
    assert_bottom_right_rounded_border(
        &mut ui,
        (10, 10, 30, 30),
        (14, 14, 26, 26),
        (22, 22, 30, 30),
        (28, 28, 30, 30),
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_outlineGeneric_respectsOutlineSize
#[test]
fn draw_border_outline_generic_respects_outline_size() {
    let fixed = shape(|_, _| {
        let path = Path::new();
        path.add_rect(Rect::new(10.0, 10.0, 30.0, 30.0));
        Outline::generic(&path)
    });
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let outline = outline_of(&fixed, scope);
        red_border(scope, 4.0, &outline);
    });
    assert_square_border(&mut ui, (10, 10, 30, 30), (14, 14, 26, 26));
}

/// Draws a 4 px red border inside `shape`'s outline for the whole 40 px box, in a scope inset by
/// 10 px.
fn inset_border(shape: Shape) -> ComposeTestRule {
    in_semantic_parent(40.0, move |scope| {
        // The outline for the whole 40 x 40 box, not the inset 20 x 20.
        let outline = outline_of(&shape, scope);
        let width = px(scope, 4.0);
        scope.inset(10.0, 10.0, 10.0, 10.0, |scope| {
            BorderRenderer::new().draw_border(scope, width, &Brush::Solid(Color::RED), &outline);
        });
    })
}

/// A 4 px border from (10, 10) at the outline's full 40 x 40, its right and bottom edges past the
/// box; its top left corner rounded when `rounded`.
fn assert_inset_border(ui: &mut ComposeTestRule, rounded: bool) {
    let (outline_bounds, inset) = ((10, 10, 50, 50), (14, 14, 46, 46));
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(Some(IntSize::new(40, 40)), |pos| {
        let in_corner_square = (10..18).contains(&pos.x) && (10..18).contains(&pos.y);
        let in_extreme_corner = (10..12).contains(&pos.x) && (10..12).contains(&pos.y);
        if !rounded {
            return Some(if contains(outline_bounds, pos) && !contains(inset, pos) {
                Color::RED
            } else {
                Color::BLUE
            });
        }
        if !contains(outline_bounds, pos) || in_extreme_corner {
            Some(Color::BLUE)
        } else if in_corner_square {
            // Anti-aliased.
            None
        } else if contains(inset, pos) {
            Some(Color::BLUE)
        } else {
            Some(Color::RED)
        }
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_insetDrawScope_rectangleOutlineCreatedForFullSize_respectsOutlineBounds
#[test]
fn draw_border_inset_draw_scope_rectangle_outline_created_for_full_size_respects_outline_bounds() {
    let mut ui = inset_border(Shape::Rectangle);
    assert_inset_border(&mut ui, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_insetDrawScope_simpleRoundedOutlineCreatedForFullSize_respectsOutlineBounds
#[test]
fn draw_border_inset_draw_scope_simple_rounded_outline_created_for_full_size_respects_outline_bounds()
 {
    let mut ui = inset_border(RoundedCornerShape(8.0));
    assert_inset_border(&mut ui, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_insetDrawScope_nonSimpleRoundedOutlineCreatedForFullSize_respectsOutlineBounds
#[test]
fn draw_border_inset_draw_scope_non_simple_rounded_outline_created_for_full_size_respects_outline_bounds()
 {
    let mut ui = inset_border(shape(|size, _| Outline::Rounded(whole(size), [8.0, 0.0, 0.0, 0.0])));
    assert_inset_border(&mut ui, true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_insetDrawScope_genericOutlineCreatedForFullSize_respectsOutlineBounds
#[test]
fn draw_border_inset_draw_scope_generic_outline_created_for_full_size_respects_outline_bounds() {
    let mut ui = inset_border(GenericShape(|path, size, _| path.add_rect(whole(size))));
    assert_inset_border(&mut ui, false);
}

/// The left edge (and the right one when `right` is given): red down its middle, and at its
/// corners red unless they're rounded (blue there). The last row is `last` (Compose checks the
/// one before on its bigger boxes, b/267371353).
fn assert_edges(
    ui: &mut ComposeTestRule,
    left_rounded: bool,
    right_rounded: Option<bool>,
    last_row_offset: i32,
) {
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (width, height) = (image.width(), image.height());
    let corner = |rounded: bool| if rounded { Color::BLUE } else { Color::RED };
    assert_eq!(image.pixel(0, 0), corner(left_rounded), "top left");
    assert_eq!(image.pixel(0, (height - 1) / 2), Color::RED, "left middle");
    assert_eq!(image.pixel(0, height - last_row_offset), corner(left_rounded), "bottom left");
    if let Some(right_rounded) = right_rounded {
        assert_eq!(image.pixel(width - 1, 0), corner(right_rounded), "top right");
        assert_eq!(image.pixel(width - 1, (height - 1) / 2), Color::RED, "right middle");
        assert_eq!(image.pixel(width - 1, height - 1), corner(right_rounded), "bottom right");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeShape
#[test]
fn draw_border_change_shape() {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let shape = if rounded.get() { RoundedCornerShape(5.0) } else { Shape::Rectangle };
        let outline = outline_of(&shape, scope);
        red_border(scope, 1.0, &outline);
    });
    // Not rounded: the left edge all red.
    assert_edges(&mut ui, false, None, 1);
    ui.run_on_idle(|_| rounded_corners.set(true));
    // Rounded: the left edge's top and bottom blue, its middle still red.
    assert_edges(&mut ui, true, None, 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineRounded_simple_observableShape
#[test]
fn draw_border_change_outline_outline_rounded_simple_observable_shape() {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    // A simple rounded shape (the same radius at each corner), reading the state.
    let observable = shape(move |size, _| {
        Outline::Rounded(whole(size), [if rounded.get() { 5.0 } else { 0.0 }; 4])
    });
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let outline = outline_of(&observable, scope);
        red_border(scope, 1.0, &outline);
    });
    assert_edges(&mut ui, false, None, 1);
    ui.run_on_idle(|_| rounded_corners.set(true));
    assert_edges(&mut ui, true, None, 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineRounded_nonSimple_observableShape
#[test]
fn draw_border_change_outline_outline_rounded_non_simple_observable_shape() {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    // Different radii at each corner: not a simple rounded rectangle.
    let observable = shape(move |size, _| {
        let radii = if rounded.get() { [5.0, 0.0, 0.0, 10.0] } else { [0.0, 5.0, 10.0, 0.0] };
        Outline::Rounded(whole(size), radii)
    });
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let outline = outline_of(&observable, scope);
        red_border(scope, 1.0, &outline);
    });
    // Only the right corners rounded.
    assert_edges(&mut ui, false, Some(true), 1);
    ui.run_on_idle(|_| rounded_corners.set(true));
    // Now the left ones.
    assert_edges(&mut ui, true, Some(false), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineGeneric_samePath_observableShape
#[test]
fn draw_border_change_outline_outline_generic_same_path_observable_shape() {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    // One path, reset and filled again each time.
    let path = Path::new();
    let observable = shape(move |size, _| {
        let radius = if rounded.get() { 50.0 } else { 0.0 };
        path.reset();
        path.add_round_rect(RoundRect::new(whole(size), CornerRadius::new(radius, radius)));
        Outline::generic(&path)
    });
    let mut ui = in_semantic_parent(400.0, move |scope| {
        let outline = outline_of(&observable, scope);
        red_border(scope, 1.0, &outline);
    });
    assert_edges(&mut ui, false, None, 2);
    ui.run_on_idle(|_| rounded_corners.set(true));
    assert_edges(&mut ui, true, None, 2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineGeneric_newPath_observableShape
#[test]
fn draw_border_change_outline_outline_generic_new_path_observable_shape() {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    let observable = shape(move |size, _| {
        let radius = if rounded.get() { 50.0 } else { 0.0 };
        let path = Path::new();
        path.add_round_rect(RoundRect::new(whole(size), CornerRadius::new(radius, radius)));
        Outline::generic(&path)
    });
    let mut ui = in_semantic_parent(400.0, move |scope| {
        let outline = outline_of(&observable, scope);
        red_border(scope, 1.0, &outline);
    });
    assert_edges(&mut ui, false, None, 2);
    ui.run_on_idle(|_| rounded_corners.set(true));
    assert_edges(&mut ui, true, None, 2);
}

/// The outline kinds the `changeOutlineType` tests switch between, each with the corners it
/// rounds (top left, top right, bottom right, bottom left) by 50 px.
#[derive(Clone, Copy)]
enum Kind {
    Rectangle,
    /// All four corners.
    SimpleRounded,
    /// The top left and bottom right corners.
    NonSimpleRounded,
    /// A path rounding the top right and bottom left corners.
    Generic,
}

impl Kind {
    fn outline(self, size: Size) -> Outline {
        match self {
            Kind::Rectangle => Outline::Rectangle(whole(size)),
            Kind::SimpleRounded => Outline::Rounded(whole(size), [50.0; 4]),
            Kind::NonSimpleRounded => Outline::Rounded(whole(size), [50.0, 0.0, 50.0, 0.0]),
            Kind::Generic => {
                let corner = |radius: f32| CornerRadius::new(radius, radius);
                let path = Path::new();
                path.add_round_rect(RoundRect::with_radii(
                    whole(size),
                    corner(0.0),
                    corner(50.0),
                    corner(0.0),
                    corner(50.0),
                ));
                Outline::generic(&path)
            }
        }
    }

    fn rounded(self) -> [bool; 4] {
        match self {
            Kind::Rectangle => [false; 4],
            Kind::SimpleRounded => [true; 4],
            Kind::NonSimpleRounded => [true, false, true, false],
            Kind::Generic => [false, true, false, true],
        }
    }
}

/// The test's `assertRoundedCorners`: the background at the rounded corners, the border at the
/// others and at each edge's middle, the background in the middle.
fn assert_rounded_corners(ui: &mut ComposeTestRule, rounded: [bool; 4]) {
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (width, height) = (image.width(), image.height());
    let corner = |rounded: bool| if rounded { Color::BLUE } else { Color::RED };
    // Compose checks the pixel before the last on the right and bottom edges (b/267371353).
    let (right, bottom) = (width - 2, height - 2);
    assert_eq!(image.pixel(0, 0), corner(rounded[0]), "top left");
    assert_eq!(image.pixel(right, 0), corner(rounded[1]), "top right");
    assert_eq!(image.pixel(right, bottom), corner(rounded[2]), "bottom right");
    assert_eq!(image.pixel(0, bottom), corner(rounded[3]), "bottom left");
    assert_eq!(image.pixel(width / 2, 0), Color::RED, "top");
    assert_eq!(image.pixel(right, height / 2), Color::RED, "right");
    assert_eq!(image.pixel(width / 2, bottom), Color::RED, "bottom");
    assert_eq!(image.pixel(0, height / 2), Color::RED, "left");
    assert_eq!(image.pixel(width / 2, height / 2), Color::BLUE, "middle");
}

/// A 64 dp box (tagged) inside 8 dp of blue padding, centered in the window, with a 2 dp red
/// border inside an outline of kind `from`, then `to`.
fn change_outline_type(from: Kind, to: Kind) {
    let changed = mutable_state_of(false);
    let is_changed = changed.handle();
    let switching = shape(move |size, _| {
        let kind = if is_changed.get() { to } else { from };
        kind.outline(size)
    });
    let mut ui = rule(move || {
        let switching = switching.clone();
        Container(
            Modifier.fill_max_size(),
            Alignment::CENTER,
            false,
            callback(move |_| {
                let switching = switching.clone();
                Container(
                    Modifier
                        .background(Color::BLUE)
                        .padding(8.dp())
                        .test_tag(TAG)
                        .size(64.dp())
                        .draw_with_content(move |scope| {
                            scope.draw_content();
                            let outline = outline_of(&switching, scope);
                            BorderRenderer::new().draw_border(
                                scope,
                                2.dp(),
                                &Brush::Solid(Color::RED),
                                &outline,
                            );
                        }),
                    Alignment::TOP_START,
                    false,
                    callback(|_| {}),
                );
            }),
        );
    });
    assert_rounded_corners(&mut ui, from.rounded());
    ui.run_on_idle(|_| changed.set(true));
    assert_rounded_corners(&mut ui, to.rounded());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_rectangleToSimpleRounded
#[test]
fn draw_border_change_outline_type_rectangle_to_simple_rounded() {
    change_outline_type(Kind::Rectangle, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_rectangleToNonSimpleRounded
#[test]
fn draw_border_change_outline_type_rectangle_to_non_simple_rounded() {
    change_outline_type(Kind::Rectangle, Kind::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_rectangleToGeneric
#[test]
fn draw_border_change_outline_type_rectangle_to_generic() {
    change_outline_type(Kind::Rectangle, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_simpleRoundedToRectangle
#[test]
fn draw_border_change_outline_type_simple_rounded_to_rectangle() {
    change_outline_type(Kind::SimpleRounded, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_simpleRoundedToNonSimpleRounded
#[test]
fn draw_border_change_outline_type_simple_rounded_to_non_simple_rounded() {
    change_outline_type(Kind::SimpleRounded, Kind::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_simpleRoundedToGeneric
#[test]
fn draw_border_change_outline_type_simple_rounded_to_generic() {
    change_outline_type(Kind::SimpleRounded, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_nonSimpleRoundedToRectangle
#[test]
fn draw_border_change_outline_type_non_simple_rounded_to_rectangle() {
    change_outline_type(Kind::NonSimpleRounded, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_nonSimpleRoundedToSimpleRounded
#[test]
fn draw_border_change_outline_type_non_simple_rounded_to_simple_rounded() {
    change_outline_type(Kind::NonSimpleRounded, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_nonSimpleRoundedToGeneric
#[test]
fn draw_border_change_outline_type_non_simple_rounded_to_generic() {
    change_outline_type(Kind::NonSimpleRounded, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_genericToRectangle
#[test]
fn draw_border_change_outline_type_generic_to_rectangle() {
    change_outline_type(Kind::Generic, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_genericToSimpleRounded
#[test]
fn draw_border_change_outline_type_generic_to_simple_rounded() {
    change_outline_type(Kind::Generic, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_genericToNonSimpleRounded
#[test]
fn draw_border_change_outline_type_generic_to_non_simple_rounded() {
    change_outline_type(Kind::Generic, Kind::NonSimpleRounded);
}

/// The outlines of the `sameOutline` tests: the whole 40 px box, a rectangle, with 8 px corners,
/// with an 8 px bottom right corner, or a path.
#[derive(Clone, Copy, PartialEq)]
enum Same {
    Rectangle,
    SimpleRounded,
    NonSimpleRounded,
    Generic,
}

impl Same {
    fn outline(self) -> Outline {
        let bounds = Rect::new(0.0, 0.0, 40.0, 40.0);
        match self {
            Same::Rectangle => Outline::Rectangle(bounds),
            Same::SimpleRounded => Outline::Rounded(bounds, [8.0; 4]),
            Same::NonSimpleRounded => Outline::Rounded(bounds, [0.0, 0.0, 8.0, 0.0]),
            Same::Generic => {
                let path = Path::new();
                path.add_rect(bounds);
                Outline::generic(&path)
            }
        }
    }

    /// Whether `pos` is in a rounded corner's anti-aliased square (`None`: not checked) or its
    /// tip (the background).
    fn corner(self, pos: IntOffset) -> Option<Option<Color>> {
        match self {
            Same::Rectangle | Same::Generic => None,
            Same::SimpleRounded => {
                let in_corner_square = !(8..32).contains(&pos.x) && !(8..32).contains(&pos.y);
                let in_extreme_corner = !(2..38).contains(&pos.x) && !(2..38).contains(&pos.y);
                if in_extreme_corner {
                    Some(Some(Color::BLUE))
                } else if in_corner_square {
                    Some(None)
                } else {
                    None
                }
            }
            Same::NonSimpleRounded => {
                if pos.x >= 38 && pos.y >= 38 {
                    Some(Some(Color::BLUE))
                } else if pos.x >= 32 && pos.y >= 32 {
                    Some(None)
                } else {
                    None
                }
            }
        }
    }

    /// A `width` px border of `color` inside the whole box.
    fn assert_border(self, ui: &mut ComposeTestRule, width: i32, color: Color) {
        let inset = (width, width, 40 - width, 40 - width);
        ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(
            Some(IntSize::new(40, 40)),
            |pos| {
                if let Some(corner) = self.corner(pos) {
                    return corner;
                }
                Some(if contains(inset, pos) { Color::BLUE } else { color })
            },
        );
    }
}

/// The same outline each frame, its border's width changed: 4, 10, then 4 px again.
fn same_outline_change_width(kind: Same) {
    let width_px = mutable_state_of(4.0f32);
    let width = width_px.handle();
    let outline = kind.outline();
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let width = px(scope, width.get());
        BorderRenderer::new().draw_border(scope, width, &Brush::Solid(Color::RED), &outline);
    });
    kind.assert_border(&mut ui, 4, Color::RED);
    ui.run_on_idle(|_| width_px.set(10.0));
    kind.assert_border(&mut ui, 10, Color::RED);
    ui.run_on_idle(|_| width_px.set(4.0));
    kind.assert_border(&mut ui, 4, Color::RED);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_rectangleOutline_sameOutline_changeWidth
#[test]
fn draw_border_rectangle_outline_same_outline_change_width() {
    same_outline_change_width(Same::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_simpleRoundedOutline_sameOutline_changeWidth
#[test]
fn draw_border_simple_rounded_outline_same_outline_change_width() {
    same_outline_change_width(Same::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_nonSimpleRoundedOutline_sameOutline_changeWidth
#[test]
fn draw_border_non_simple_rounded_outline_same_outline_change_width() {
    same_outline_change_width(Same::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_genericOutline_sameOutline_changeWidth
#[test]
fn draw_border_generic_outline_same_outline_change_width() {
    same_outline_change_width(Same::Generic);
}

/// The same outline each frame, its border's brush changed: red, green, then a gradient of red
/// to red.
fn same_outline_change_brush(kind: Same) {
    let brush = mutable_state_of(Brush::Solid(Color::RED));
    let current = brush.handle();
    let outline = kind.outline();
    let mut ui = in_semantic_parent(40.0, move |scope| {
        let width = px(scope, 4.0);
        BorderRenderer::new().draw_border(scope, width, &current.get(), &outline);
    });
    kind.assert_border(&mut ui, 4, Color::RED);
    ui.run_on_idle(|_| brush.set(Brush::Solid(Color::GREEN)));
    kind.assert_border(&mut ui, 4, Color::GREEN);
    ui.run_on_idle(|_| brush.set(Brush::linear_gradient([Color::RED, Color::RED])));
    kind.assert_border(&mut ui, 4, Color::RED);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_rectangleOutline_sameOutline_changeBrush
#[test]
fn draw_border_rectangle_outline_same_outline_change_brush() {
    same_outline_change_brush(Same::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_simpleRoundedOutline_sameOutline_changeBrush
#[test]
fn draw_border_simple_rounded_outline_same_outline_change_brush() {
    same_outline_change_brush(Same::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_nonSimpleRoundedOutline_sameOutline_changeBrush
#[test]
fn draw_border_non_simple_rounded_outline_same_outline_change_brush() {
    same_outline_change_brush(Same::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_genericOutline_sameOutline_changeBrush
#[test]
fn draw_border_generic_outline_same_outline_change_brush() {
    same_outline_change_brush(Same::Generic);
}
