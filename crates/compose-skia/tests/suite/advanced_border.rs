//! Compose's `AdvancedBorderTest` (foundation, device tests): `Modifier.border` over triangle,
//! generic-path, rounded and non-simple rounded outlines, an alpha tint, outline bounds that don't
//! start at the origin, shapes and outlines changed between frames, and the modifier's equality,
//! drawn by Skia and checked pixel by pixel. The scenarios are shared with
//! `AdvancedBorderRendererTest` (`advanced_border_renderer.rs`), which draws the same borders with
//! `BorderRenderer::draw_border` in a `draw_with_content`: each body here takes the [`Way`].
//!
//! Compose runs each test with its old border implementation too
//! (`isNewBorderImplementationEnabled` false): n/a, compose-rs has only the new one.

use std::rc::Rc;

use compose::foundation::layout::Box as Container;
use compose::foundation::shape::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Alignment, BorderRenderer, BorderStroke, Brush, Color, ContentDrawScope, CornerRadius,
    CustomShape, Density, Dp, IntOffset, IntSize, LOCAL_DENSITY, LayoutDirection, Modifier,
    Outline, Path, PathOperation, Rect, RoundRect, Shape, Size, UnitExt,
};

pub(super) const TAG: &str = "BorderParent";

/// A test rule capturing with Skia, at a phone's density: some tests give sizes in dp and radii
/// in pixels, and expect a radius smaller than half the side.
pub(super) fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    compose_test_with(
        DEFAULT_SIZE,
        Density::new(2.0),
        std::boxed::Box::new(compose::ui::platform::FixedTextMeasurer::new()),
        content,
    )
}

type Make = dyn Fn(Size, LayoutDirection) -> Outline;

/// A shape making its outline with a closure, as the tests' `object : Shape`s.
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

pub(super) fn shape(make: impl Fn(Size, LayoutDirection) -> Outline + 'static) -> Shape {
    Shape::custom(FnShape(Rc::new(make)))
}

pub(super) fn outline_of(shape: &Shape, scope: &ContentDrawScope<'_, '_>) -> Outline {
    shape.outline_in(scope.size(), &scope.density(), scope.layout_direction())
}

pub(super) fn whole(size: Size) -> Rect {
    Rect::new(0.0, 0.0, size.width, size.height)
}

/// As `IntRect(left, top, right, bottom).contains(pos)`: the right and bottom edges excluded.
pub(super) fn contains(rect: (i32, i32, i32, i32), pos: IntOffset) -> bool {
    pos.x >= rect.0 && pos.x < rect.2 && pos.y >= rect.1 && pos.y < rect.3
}

/// How a test draws its border.
#[derive(Clone, Copy)]
pub(super) enum Way {
    /// `BorderRenderer::draw_border` in a `draw_with_content` after the content, its width,
    /// brush and shape read as it draws (`AdvancedBorderRendererTest`).
    Renderer,
    /// `Modifier.border_with`, its width, brush and shape read as it's composed
    /// (`AdvancedBorderTest`).
    Modifier,
}

/// A border of `width` (given the density), `brush` and `shape`, drawn the `way` way.
pub(super) fn border(
    way: Way,
    width: impl Fn(Density) -> Dp + 'static,
    brush: impl Fn() -> Brush + 'static,
    shape: impl Fn() -> Shape + 'static,
) -> Modifier {
    match way {
        Way::Renderer => Modifier.draw_with_content(move |scope| {
            scope.draw_content();
            let outline = outline_of(&shape(), scope);
            let (width, brush) = (width(scope.density()), brush());
            BorderRenderer::new().draw_border(scope, width, &brush, &outline);
        }),
        Way::Modifier => Modifier.border_with(width(LOCAL_DENSITY.current()), brush(), shape()),
    }
}

/// A red border of `width` px inside `shape`.
fn red_px(way: Way, width: f32, shape: Shape) -> Modifier {
    border(
        way,
        move |density| density.to_dp(width),
        || Brush::Solid(Color::RED),
        move || shape.clone(),
    )
}

/// A box of `size` px (as the tests' `Modifier.size(40.0f.toDp(), 40.0f.toDp())`) with a blue
/// background and `modifier` (made as it's composed) after, in a box tagged [`TAG`] inside
/// another box (the tests' `SemanticParent`).
pub(super) fn in_semantic_parent(
    size: f32,
    modifier: impl Fn() -> Modifier + 'static,
) -> ComposeTestRule {
    let modifier = Rc::new(modifier);
    rule(move || {
        let modifier = modifier.clone();
        Container(
            Modifier,
            Alignment::TOP_START,
            false,
            callback(move |_| {
                let modifier = modifier.clone();
                Container(
                    Modifier.test_tag(TAG),
                    Alignment::TOP_START,
                    false,
                    callback(move |_| {
                        let side = LOCAL_DENSITY.current().to_dp(size);
                        Container(
                            Modifier.size((side, side)).background(Color::BLUE).then(modifier()),
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

/// One box tagged `tag` with `modifier` (made as it's composed).
fn single(tag: &'static str, modifier: impl Fn() -> Modifier + 'static) -> ComposeTestRule {
    rule(move || {
        Container(
            Modifier.test_tag(tag).then(modifier()),
            Alignment::TOP_START,
            false,
            callback(|_| {}),
        );
    })
}

/// `border_triangle_shape`, `drawBorder_triangle_shape`.
pub(super) fn triangle_shape(way: Way) {
    let triangle = GenericShape(|path, size, _| {
        path.line_to(size.width, 0.0);
        path.line_to(size.width, size.height);
        path.close();
    });
    let mut ui = single(TAG, move || {
        let side = LOCAL_DENSITY.current().to_dp(100.0);
        Modifier.required_size((side, side)).background(Color::WHITE).then(red_px(
            way,
            10.0,
            triangle.clone(),
        ))
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

/// `border_non_simple_rounded_rect`, `drawBorder_non_simple_rounded_rect`.
pub(super) fn non_simple_rounded_rect(way: Way) {
    let (top_left, top_right, bottom_right, bottom_left) = (0.0, 5.0, 10.0, 15.0);
    let border_width = 10.dp();
    let mut ui = single("roundRectTag", move || {
        Modifier.size(50.dp()).background(Color::WHITE).then(border(
            way,
            move |_| border_width,
            || Brush::Solid(Color::RED),
            move || RoundedCornerShape((top_left, top_right, bottom_right, bottom_left)),
        ))
    });
    let border_px = ui.density().to_px(border_width) as i32;
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

/// `border_color_with_alpha_tinted_properly`, `drawBorder_color_with_alpha_tinted_properly`.
pub(super) fn color_with_alpha_tinted_properly(way: Way) {
    let background = Color::RED;
    let border_color = Color::BLUE.with_alpha(0.5);
    let square = GenericShape(|path, size, _| {
        path.line_to(size.width, 0.0);
        path.line_to(size.width, size.height);
        path.line_to(0.0, size.height);
        path.close();
    });
    let mut ui = single(TAG, move || {
        let square = square.clone();
        Modifier.required_size((100.dp(), 100.dp())).background(background).then(border(
            way,
            |_| 10.dp(),
            move || Brush::Solid(border_color),
            move || square.clone(),
        ))
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

/// `border_generic_shape_color_to_brush`, `drawBorder_generic_shape_color_to_brush`: a solid
/// border, then a gradient one.
pub(super) fn generic_shape_color_to_brush(way: Way) {
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
    let mut ui = single(TAG, move || {
        let (gradient, test_shape) = (gradient.clone(), test_shape.clone());
        Modifier.required_size((20.dp(), 20.dp())).background(Color::WHITE).then(border(
            way,
            move |_| border_stroke,
            move || if toggled.get() { gradient.clone() } else { Brush::Solid(Color::GREEN) },
            move || test_shape.clone(),
        ))
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

/// The tests' `calculateContainerShape`: a rounded rectangle with an arrow on its top, its tip at
/// half `arrow_tip`.
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

/// `border_test_with_fixed_size_generic_shape`, `drawBorder_test_with_fixed_size_generic_shape`:
/// a bubble with an arrow drawn into the padding above it.
pub(super) fn test_with_fixed_size_generic_shape(way: Way) {
    let (bubble_width, bubble_height, border_width) = (80.dp(), 40.dp(), 10.dp());
    let (arrow_base_width, arrow_tip, arrow_length) = (24.dp(), 56.dp(), 12.dp());
    let mut ui = single(TAG, move || {
        let bubble_with_arrow =
            container_shape(LOCAL_DENSITY.current(), arrow_base_width, arrow_tip, arrow_length);
        Modifier
            .required_size((bubble_width, bubble_height))
            .background(Color::WHITE)
            .padding(compose::ui::PaddingValues {
                top: arrow_length,
                ..compose::ui::PaddingValues::ZERO
            })
            .then(border(
                way,
                move |_| border_width,
                || Brush::Solid(Color::RED),
                move || bubble_with_arrow.clone(),
            ))
    });
    let density = ui.density();
    let outline = container_shape(density, arrow_base_width, arrow_tip, arrow_length).outline_in(
        Size::new(density.to_px(bubble_width), density.to_px(bubble_height)),
        &density,
        LayoutDirection::Ltr,
    );
    let Outline::Generic(path, _) = &outline else { panic!("a generic outline: {outline:?}") };
    let offset_y = -path.bounds().top;
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
pub(super) fn assert_square_border(
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

/// `border_rectangleOutline_respectsOutlineBounds` and the renderer's: a 4 px border in a fixed
/// rectangle from (10, 10).
pub(super) fn rectangle_outline_respects_outline_bounds(way: Way) {
    let fixed = shape(|_, _| Outline::Rectangle(Rect::new(10.0, 10.0, 30.0, 30.0)));
    let mut ui = in_semantic_parent(40.0, move || red_px(way, 4.0, fixed.clone()));
    assert_square_border(&mut ui, (10, 10, 30, 30), (14, 14, 26, 26));
}

/// `border_simpleRoundedOutline_respectsOutlineBounds` and the renderer's: a 4 px rounded
/// rectangle from (10, 10).
pub(super) fn simple_rounded_outline_respects_outline_bounds(way: Way) {
    let fixed = shape(|_, _| Outline::Rounded(Rect::new(10.0, 10.0, 30.0, 30.0), [8.0; 4]));
    let mut ui = in_semantic_parent(40.0, move || red_px(way, 4.0, fixed.clone()));
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
/// checked; its tip, `corner_tip`, the background).
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

/// `border_nonSimpleRoundedOutline_zeroOffsetOriginOutline_respectsOutlineBounds` and the
/// renderer's: a 20 px outline at the origin, its bottom right corner rounded.
pub(super) fn non_simple_rounded_outline_zero_offset_origin(way: Way) {
    let fixed =
        shape(|_, _| Outline::Rounded(Rect::new(0.0, 0.0, 20.0, 20.0), [0.0, 0.0, 8.0, 0.0]));
    let mut ui = in_semantic_parent(40.0, move || red_px(way, 4.0, fixed.clone()));
    assert_bottom_right_rounded_border(
        &mut ui,
        (0, 0, 20, 20),
        (4, 4, 16, 16),
        (12, 12, 20, 20),
        (18, 18, 20, 20),
    );
}

/// `border_nonSimpleRoundedOutline_nonZeroOffsetOriginOutline_respectsOutlineBounds` and the
/// renderer's: the same from (10, 10) (b/546074983).
pub(super) fn non_simple_rounded_outline_non_zero_offset_origin(way: Way) {
    let fixed =
        shape(|_, _| Outline::Rounded(Rect::new(10.0, 10.0, 30.0, 30.0), [0.0, 0.0, 8.0, 0.0]));
    let mut ui = in_semantic_parent(40.0, move || red_px(way, 4.0, fixed.clone()));
    assert_bottom_right_rounded_border(
        &mut ui,
        (10, 10, 30, 30),
        (14, 14, 26, 26),
        (22, 22, 30, 30),
        (28, 28, 30, 30),
    );
}

/// `border_outlineGeneric_respectsOutlineSize` and the renderer's: a path's rectangle from
/// (10, 10).
pub(super) fn outline_generic_respects_outline_size(way: Way) {
    let fixed = shape(|_, _| {
        let path = Path::new();
        path.add_rect(Rect::new(10.0, 10.0, 30.0, 30.0));
        Outline::generic(&path)
    });
    let mut ui = in_semantic_parent(40.0, move || red_px(way, 4.0, fixed.clone()));
    assert_square_border(&mut ui, (10, 10, 30, 30), (14, 14, 26, 26));
}

/// The left edge (and the right one when `right_rounded` is given): red down its middle, and at
/// its corners red unless they're rounded (blue there). The bottom left pixel checked is
/// `last_row_offset` up from the bottom (Compose checks the one before the last on its bigger
/// boxes, b/267371353).
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

/// `border_changeShape`, `drawBorder_changeShape`: a rectangle, then 5 px rounded corners.
pub(super) fn change_shape(way: Way) {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    let mut ui = in_semantic_parent(40.0, move || {
        border(
            way,
            |density| density.to_dp(1.0),
            || Brush::Solid(Color::RED),
            move || if rounded.get() { RoundedCornerShape(5.0) } else { Shape::Rectangle },
        )
    });
    // Not rounded: the left edge all red.
    assert_edges(&mut ui, false, None, 1);
    ui.run_on_idle(|_| rounded_corners.set(true));
    // Rounded: the left edge's top and bottom blue, its middle still red.
    assert_edges(&mut ui, true, None, 1);
}

/// A 1 px red border inside `observable`, in a `size` px box: its outline reads `rounded`.
fn observable_shape(
    way: Way,
    size: f32,
    observable: Shape,
    rounded: OwnedState<bool>,
    before: (bool, Option<bool>),
    after: (bool, Option<bool>),
) {
    let last_row_offset = if size > 40.0 { 2 } else { 1 };
    let mut ui = in_semantic_parent(size, move || red_px(way, 1.0, observable.clone()));
    assert_edges(&mut ui, before.0, before.1, last_row_offset);
    ui.run_on_idle(|_| rounded.set(true));
    assert_edges(&mut ui, after.0, after.1, last_row_offset);
}

/// `border_changeOutline_outlineRounded_simple_observableShape` and the renderer's: the same
/// radius at each corner, 0 then 5 px.
pub(super) fn change_outline_rounded_simple(way: Way) {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    let observable = shape(move |size, _| {
        Outline::Rounded(whole(size), [if rounded.get() { 5.0 } else { 0.0 }; 4])
    });
    observable_shape(way, 40.0, observable, rounded_corners, (false, None), (true, None));
}

/// `border_changeOutline_outlineRounded_nonSimple_observableShape` and the renderer's: the right
/// corners rounded, then the left ones.
pub(super) fn change_outline_rounded_non_simple(way: Way) {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    // Different radii at each corner: not a simple rounded rectangle.
    let observable = shape(move |size, _| {
        let radii = if rounded.get() { [5.0, 0.0, 0.0, 10.0] } else { [0.0, 5.0, 10.0, 0.0] };
        Outline::Rounded(whole(size), radii)
    });
    observable_shape(
        way,
        40.0,
        observable,
        rounded_corners,
        (false, Some(true)),
        (true, Some(false)),
    );
}

/// `border_changeOutline_outlineGeneric_samePath_observableShape` and the renderer's: one path,
/// reset and filled again with a rounded rectangle, 0 then 50 px.
pub(super) fn change_outline_generic_same_path(way: Way) {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    let path = Path::new();
    let observable = shape(move |size, _| {
        let radius = if rounded.get() { 50.0 } else { 0.0 };
        path.reset();
        path.add_round_rect(RoundRect::new(whole(size), CornerRadius::new(radius, radius)));
        Outline::generic(&path)
    });
    observable_shape(way, 400.0, observable, rounded_corners, (false, None), (true, None));
}

/// `border_changeOutline_outlineGeneric_newPath_observableShape` and the renderer's: a new path
/// each time.
pub(super) fn change_outline_generic_new_path(way: Way) {
    let rounded_corners = mutable_state_of(false);
    let rounded = rounded_corners.handle();
    let observable = shape(move |size, _| {
        let radius = if rounded.get() { 50.0 } else { 0.0 };
        let path = Path::new();
        path.add_round_rect(RoundRect::new(whole(size), CornerRadius::new(radius, radius)));
        Outline::generic(&path)
    });
    observable_shape(way, 400.0, observable, rounded_corners, (false, None), (true, None));
}

/// The outline kinds the `changeOutlineType` tests switch between, rounding corners by 50 px.
#[derive(Clone, Copy)]
pub(super) enum Kind {
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

    /// Which corners it rounds: top left, top right, bottom right, bottom left.
    fn rounded(self) -> [bool; 4] {
        match self {
            Kind::Rectangle => [false; 4],
            Kind::SimpleRounded => [true; 4],
            Kind::NonSimpleRounded => [true, false, true, false],
            Kind::Generic => [false, true, false, true],
        }
    }
}

/// The tests' `assertRoundedCorners`: the background at the rounded corners, the border at the
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

/// The `changeOutlineType` tests: a 64 dp box (tagged) inside 8 dp of blue padding, centered in
/// the window, with a 2 dp red border inside an outline of kind `from`, then `to`.
pub(super) fn change_outline_type(way: Way, from: Kind, to: Kind) {
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
                        .then(border(
                            way,
                            |_| 2.dp(),
                            || Brush::Solid(Color::RED),
                            move || switching.clone(),
                        )),
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

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_triangle_shape
#[test]
fn border_triangle_shape() {
    triangle_shape(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_non_simple_rounded_rect
#[test]
fn border_non_simple_rounded_rect() {
    non_simple_rounded_rect(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_color_with_alpha_tinted_properly
#[test]
fn border_color_with_alpha_tinted_properly() {
    color_with_alpha_tinted_properly(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_generic_shape_color_to_brush
#[test]
fn border_generic_shape_color_to_brush() {
    generic_shape_color_to_brush(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_test_with_fixed_size_generic_shape
#[test]
fn border_test_with_fixed_size_generic_shape() {
    test_with_fixed_size_generic_shape(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_rectangleOutline_respectsOutlineBounds
#[test]
fn border_rectangle_outline_respects_outline_bounds() {
    rectangle_outline_respects_outline_bounds(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_simpleRoundedOutline_respectsOutlineBounds
#[test]
fn border_simple_rounded_outline_respects_outline_bounds() {
    simple_rounded_outline_respects_outline_bounds(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_nonSimpleRoundedOutline_zeroOffsetOriginOutline_respectsOutlineBounds
#[test]
fn border_non_simple_rounded_outline_zero_offset_origin_outline_respects_outline_bounds() {
    non_simple_rounded_outline_zero_offset_origin(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_nonSimpleRoundedOutline_nonZeroOffsetOriginOutline_respectsOutlineBounds
#[test]
fn border_non_simple_rounded_outline_non_zero_offset_origin_outline_respects_outline_bounds() {
    non_simple_rounded_outline_non_zero_offset_origin(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_outlineGeneric_respectsOutlineSize
#[test]
fn border_outline_generic_respects_outline_size() {
    outline_generic_respects_outline_size(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeShape
#[test]
fn border_change_shape() {
    change_shape(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutline_outlineRounded_simple_observableShape
#[test]
fn border_change_outline_outline_rounded_simple_observable_shape() {
    change_outline_rounded_simple(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutline_outlineRounded_nonSimple_observableShape
#[test]
fn border_change_outline_outline_rounded_non_simple_observable_shape() {
    change_outline_rounded_non_simple(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutline_outlineGeneric_samePath_observableShape
#[test]
fn border_change_outline_outline_generic_same_path_observable_shape() {
    change_outline_generic_same_path(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutline_outlineGeneric_newPath_observableShape
#[test]
fn border_change_outline_outline_generic_new_path_observable_shape() {
    change_outline_generic_new_path(Way::Modifier);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_rectangleToSimpleRounded
#[test]
fn border_change_outline_type_rectangle_to_simple_rounded() {
    change_outline_type(Way::Modifier, Kind::Rectangle, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_rectangleToNonSimpleRounded
#[test]
fn border_change_outline_type_rectangle_to_non_simple_rounded() {
    change_outline_type(Way::Modifier, Kind::Rectangle, Kind::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_rectangleToGeneric
#[test]
fn border_change_outline_type_rectangle_to_generic() {
    change_outline_type(Way::Modifier, Kind::Rectangle, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_simpleRoundedToRectangle
#[test]
fn border_change_outline_type_simple_rounded_to_rectangle() {
    change_outline_type(Way::Modifier, Kind::SimpleRounded, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_simpleRoundedToNonSimpleRounded
#[test]
fn border_change_outline_type_simple_rounded_to_non_simple_rounded() {
    change_outline_type(Way::Modifier, Kind::SimpleRounded, Kind::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_simpleRoundedToGeneric
#[test]
fn border_change_outline_type_simple_rounded_to_generic() {
    change_outline_type(Way::Modifier, Kind::SimpleRounded, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_nonSimpleRoundedToRectangle
#[test]
fn border_change_outline_type_non_simple_rounded_to_rectangle() {
    change_outline_type(Way::Modifier, Kind::NonSimpleRounded, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_nonSimpleRoundedToSimpleRounded
#[test]
fn border_change_outline_type_non_simple_rounded_to_simple_rounded() {
    change_outline_type(Way::Modifier, Kind::NonSimpleRounded, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_nonSimpleRoundedToGeneric
#[test]
fn border_change_outline_type_non_simple_rounded_to_generic() {
    change_outline_type(Way::Modifier, Kind::NonSimpleRounded, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_genericToRectangle
#[test]
fn border_change_outline_type_generic_to_rectangle() {
    change_outline_type(Way::Modifier, Kind::Generic, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_genericToSimpleRounded
#[test]
fn border_change_outline_type_generic_to_simple_rounded() {
    change_outline_type(Way::Modifier, Kind::Generic, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, border_changeOutlineType_genericToNonSimpleRounded
#[test]
fn border_change_outline_type_generic_to_non_simple_rounded() {
    change_outline_type(Way::Modifier, Kind::Generic, Kind::NonSimpleRounded);
}

/// The test utilities' `assertModifierIsPure`: the same inputs make equal modifiers, different
/// ones different modifiers.
fn assert_modifier_is_pure(create: impl Fn(bool) -> Modifier) {
    let (first, second, third) = (create(true), create(false), create(true));
    assert!(first.same(&third), "Modifier with same inputs should resolve true to equals call");
    assert!(
        !first.same(&second),
        "Modifier with different inputs should resolve false to equals call"
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, equalInputs_shouldResolveToEquals_withColor
#[test]
fn equal_inputs_should_resolve_to_equals_with_color() {
    assert_modifier_is_pure(|toggle| {
        if toggle {
            Modifier.border(10.dp(), Color::RED)
        } else {
            Modifier.border(5.dp(), Color::RED)
        }
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, equalInputs_shouldResolveToEquals_withBorderStroke
#[test]
fn equal_inputs_should_resolve_to_equals_with_border_stroke() {
    let border_stroke1 = BorderStroke::new(10.dp(), Color::BLUE);
    let border_stroke2 = BorderStroke::new(5.dp(), Color::BLUE);
    assert_modifier_is_pure(|toggle| {
        // `Modifier.border(borderStroke)`: a rectangle.
        let stroke = if toggle { border_stroke1.clone() } else { border_stroke2.clone() };
        Modifier.border_stroke(stroke, Shape::Rectangle)
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderTest.kt, equalInputs_shouldResolveToEquals_withBrush
#[test]
fn equal_inputs_should_resolve_to_equals_with_brush() {
    // Compose's gradients have no colors; these differ by their direction alone too.
    let brush1 = Brush::horizontal_gradient([Color::RED, Color::BLUE]);
    let brush2 = Brush::vertical_gradient([Color::RED, Color::BLUE]);
    assert_modifier_is_pure(|toggle| {
        let brush = if toggle { brush1.clone() } else { brush2.clone() };
        Modifier.border_with(10.dp(), brush, Shape::Rectangle)
    });
}
