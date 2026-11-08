//! Compose's `AdvancedBorderRendererTest` (foundation, device tests): `BorderRenderer::draw_border`
//! called from a `draw_with_content` over triangle, generic-path, rounded and non-simple rounded
//! outlines, an alpha tint, outline bounds that don't start at the origin, inset draw scopes,
//! outlines, widths and brushes changed between frames, drawn by Skia and checked pixel by pixel.
//! The scenarios it shares with `AdvancedBorderTest` are in `advanced_border.rs`.
//!
//! `drawBorder_changeProvidedGraphicsLayer_usesLatestGraphicsLayer` is n/a (and `@Ignore`d in
//! Compose, b/570069301): compose-rs's `BorderRenderer` takes no graphics layer provider; it
//! clips a generic outline's stroke to the path instead of drawing it in an offscreen layer.

use compose::foundation::shape::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    BorderRenderer, Brush, Color, ContentDrawScope, Dp, IntOffset, IntSize, Modifier, Outline,
    Path, Rect, Shape,
};

use super::advanced_border::*;

/// Pixels as dp in the draw's density, as the test's `Float.toDp()`.
fn px(scope: &ContentDrawScope<'_, '_>, value: f32) -> Dp {
    scope.density().to_dp(value)
}

/// A box of `size` px with a blue background, drawing its content then `draw` (the test's
/// `SemanticParent` around a `Box` with a `drawWithContent`).
fn drawing(size: f32, draw: impl Fn(&mut ContentDrawScope<'_, '_>) + 'static) -> ComposeTestRule {
    let draw = std::rc::Rc::new(draw);
    in_semantic_parent(size, move || {
        let draw = draw.clone();
        Modifier.draw_with_content(move |scope| {
            scope.draw_content();
            draw(scope);
        })
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_triangle_shape
#[test]
fn draw_border_triangle_shape() {
    triangle_shape(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_non_simple_rounded_rect
#[test]
fn draw_border_non_simple_rounded_rect() {
    non_simple_rounded_rect(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_color_with_alpha_tinted_properly
#[test]
fn draw_border_color_with_alpha_tinted_properly() {
    color_with_alpha_tinted_properly(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_generic_shape_color_to_brush
#[test]
fn draw_border_generic_shape_color_to_brush() {
    generic_shape_color_to_brush(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_test_with_fixed_size_generic_shape
#[test]
fn draw_border_test_with_fixed_size_generic_shape() {
    test_with_fixed_size_generic_shape(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_rectangleOutline_respectsOutlineBounds
#[test]
fn draw_border_rectangle_outline_respects_outline_bounds() {
    rectangle_outline_respects_outline_bounds(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_simpleRoundedOutline_respectsOutlineBounds
#[test]
fn draw_border_simple_rounded_outline_respects_outline_bounds() {
    simple_rounded_outline_respects_outline_bounds(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_nonSimpleRoundedOutline_zeroOffsetOriginOutline_respectsOutlineBounds
#[test]
fn draw_border_non_simple_rounded_outline_zero_offset_origin_outline_respects_outline_bounds() {
    non_simple_rounded_outline_zero_offset_origin(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_nonSimpleRoundedOutline_nonZeroOffsetOriginOutline_respectsOutlineBounds
#[test]
fn draw_border_non_simple_rounded_outline_non_zero_offset_origin_outline_respects_outline_bounds() {
    non_simple_rounded_outline_non_zero_offset_origin(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_outlineGeneric_respectsOutlineSize
#[test]
fn draw_border_outline_generic_respects_outline_size() {
    outline_generic_respects_outline_size(Way::Renderer);
}

/// Draws a 4 px red border inside `shape`'s outline for the whole 40 px box, in a scope inset by
/// 10 px.
fn inset_border(shape: Shape) -> ComposeTestRule {
    drawing(40.0, move |scope| {
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

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeShape
#[test]
fn draw_border_change_shape() {
    change_shape(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineRounded_simple_observableShape
#[test]
fn draw_border_change_outline_outline_rounded_simple_observable_shape() {
    change_outline_rounded_simple(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineRounded_nonSimple_observableShape
#[test]
fn draw_border_change_outline_outline_rounded_non_simple_observable_shape() {
    change_outline_rounded_non_simple(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineGeneric_samePath_observableShape
#[test]
fn draw_border_change_outline_outline_generic_same_path_observable_shape() {
    change_outline_generic_same_path(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutline_outlineGeneric_newPath_observableShape
#[test]
fn draw_border_change_outline_outline_generic_new_path_observable_shape() {
    change_outline_generic_new_path(Way::Renderer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_rectangleToSimpleRounded
#[test]
fn draw_border_change_outline_type_rectangle_to_simple_rounded() {
    change_outline_type(Way::Renderer, Kind::Rectangle, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_rectangleToNonSimpleRounded
#[test]
fn draw_border_change_outline_type_rectangle_to_non_simple_rounded() {
    change_outline_type(Way::Renderer, Kind::Rectangle, Kind::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_rectangleToGeneric
#[test]
fn draw_border_change_outline_type_rectangle_to_generic() {
    change_outline_type(Way::Renderer, Kind::Rectangle, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_simpleRoundedToRectangle
#[test]
fn draw_border_change_outline_type_simple_rounded_to_rectangle() {
    change_outline_type(Way::Renderer, Kind::SimpleRounded, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_simpleRoundedToNonSimpleRounded
#[test]
fn draw_border_change_outline_type_simple_rounded_to_non_simple_rounded() {
    change_outline_type(Way::Renderer, Kind::SimpleRounded, Kind::NonSimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_simpleRoundedToGeneric
#[test]
fn draw_border_change_outline_type_simple_rounded_to_generic() {
    change_outline_type(Way::Renderer, Kind::SimpleRounded, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_nonSimpleRoundedToRectangle
#[test]
fn draw_border_change_outline_type_non_simple_rounded_to_rectangle() {
    change_outline_type(Way::Renderer, Kind::NonSimpleRounded, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_nonSimpleRoundedToSimpleRounded
#[test]
fn draw_border_change_outline_type_non_simple_rounded_to_simple_rounded() {
    change_outline_type(Way::Renderer, Kind::NonSimpleRounded, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_nonSimpleRoundedToGeneric
#[test]
fn draw_border_change_outline_type_non_simple_rounded_to_generic() {
    change_outline_type(Way::Renderer, Kind::NonSimpleRounded, Kind::Generic);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_genericToRectangle
#[test]
fn draw_border_change_outline_type_generic_to_rectangle() {
    change_outline_type(Way::Renderer, Kind::Generic, Kind::Rectangle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_genericToSimpleRounded
#[test]
fn draw_border_change_outline_type_generic_to_simple_rounded() {
    change_outline_type(Way::Renderer, Kind::Generic, Kind::SimpleRounded);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/AdvancedBorderRendererTest.kt, drawBorder_changeOutlineType_genericToNonSimpleRounded
#[test]
fn draw_border_change_outline_type_generic_to_non_simple_rounded() {
    change_outline_type(Way::Renderer, Kind::Generic, Kind::NonSimpleRounded);
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
    let mut ui = drawing(40.0, move |scope| {
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
    let mut ui = drawing(40.0, move |scope| {
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
