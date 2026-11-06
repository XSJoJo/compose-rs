//! Drop and inner shadows, as Compose's `androidx.compose.ui.graphics.shadow`: a shape's shadow
//! blurred by a radius, spread, moved, in a color or a brush, drawn by `Modifier.drop_shadow`
//! and `Modifier.inner_shadow` or by their painters.

use std::cell::Cell;

use crate::brush::Brush;
use crate::color_filter::{BlendMode, ColorFilter};
use crate::draw::DrawScope;
use crate::graphics::{Color, Outline, Paint, PaintStyle, Shape};
use crate::modifier::{DrawModifierNode, Modifier, ModifierNode, ModifierNodeElement};
use crate::painter::CustomPainter;
use crate::path::{CornerRadius, Path, PathFillType, RoundRect};
use crate::unit::{Dp, DpOffset, Offset, Rect, Size};

/// A shadow's look, as Compose's `shadow.Shadow`: its blur `radius`, how far it `spread`s past
/// (or, inside, into) its shape, its `offset`, its color or brush (a solid brush is its color),
/// `alpha` and `blend_mode`. Compose's defaults: black, no spread or offset, opaque, `SrcOver`.
#[derive(Clone, PartialEq, Debug)]
pub struct Shadow {
    pub radius: Dp,
    pub spread: Dp,
    pub offset: DpOffset,
    pub color: Color,
    pub brush: Option<Brush>,
    pub alpha: f32,
    pub blend_mode: BlendMode,
}

impl Default for Shadow {
    fn default() -> Self {
        Shadow::new(Dp(0.0))
    }
}

impl Shadow {
    /// A black shadow of `radius`, as `Shadow(radius)`.
    pub fn new(radius: Dp) -> Shadow {
        Shadow {
            radius,
            spread: Dp(0.0),
            offset: DpOffset::default(),
            color: Color::BLACK,
            brush: None,
            alpha: 1.0,
            blend_mode: BlendMode::SrcOver,
        }
    }

    /// A shadow of `radius` in `color` (an unspecified color is black), as `Shadow(radius,
    /// color)`.
    pub fn colored(radius: Dp, color: Color) -> Shadow {
        Shadow { color: color.or(Color::BLACK), ..Shadow::new(radius) }
    }

    /// A shadow of `radius` painted by `brush` (a solid brush is its color), as `Shadow(radius,
    /// brush)`.
    pub fn brushed(radius: Dp, brush: Brush) -> Shadow {
        match brush {
            Brush::Solid(color) => Shadow::colored(radius, color),
            brush => Shadow { brush: Some(brush), ..Shadow::new(radius) },
        }
    }

    /// The same shadow at `alpha` (kept in 0 to 1), as the `alpha` parameter.
    pub fn with_alpha(self, alpha: f32) -> Shadow {
        Shadow { alpha: alpha.clamp(0.0, 1.0), ..self }
    }

    /// Its paint at `alpha` (the painter's times its own), `style`d, blurred by its radius.
    fn paint(
        &self,
        scope: &mut DrawScope<'_>,
        alpha: f32,
        style: PaintStyle,
        filter: Option<&ColorFilter>,
    ) -> Paint {
        let alpha = (alpha * self.alpha).clamp(0.0, 1.0);
        let mut paint = match &self.brush {
            Some(brush) => scope.brush_paint(brush, alpha, style),
            None => {
                let mut paint = Paint::fill(self.color).with_alpha(alpha);
                paint.style = style;
                paint
            }
        };
        paint = paint.with_blend_mode(self.blend_mode);
        paint = scope.paint_with_color_filter(paint, filter);
        paint.with_blur(scope.density().to_px(self.radius))
    }
}

impl compose_runtime::ComposeArg for Shadow {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// Between `start` and `stop`, as Compose's `shadow.lerp`: a missing end is the other one,
/// transparent; brushes switch halfway, as the blend mode.
pub fn lerp(start: Option<&Shadow>, stop: Option<&Shadow>, fraction: f32) -> Option<Shadow> {
    let transparent =
        |shadow: &Shadow| Shadow { color: Color::TRANSPARENT, brush: None, ..shadow.clone() };
    let (start, stop) = match (start, stop) {
        (None, None) => return None,
        (None, Some(stop)) => (transparent(stop), stop.clone()),
        (Some(start), None) => (start.clone(), transparent(start)),
        (Some(start), Some(stop)) => (start.clone(), stop.clone()),
    };
    let dp = |a: Dp, b: Dp| Dp(crate::unit::lerp_f32(a.0, b.0, fraction));
    let half = fraction < 0.5;
    Some(Shadow {
        radius: dp(start.radius, stop.radius),
        spread: dp(start.spread, stop.spread),
        offset: DpOffset::lerp(start.offset, stop.offset, fraction),
        color: Color::lerp(start.color, stop.color, fraction),
        brush: if start.brush == stop.brush || half { start.brush } else { stop.brush },
        alpha: crate::unit::lerp_f32(start.alpha, stop.alpha, fraction),
        blend_mode: if half { start.blend_mode } else { stop.blend_mode },
    })
}

/// `rect` grown by `by` on each side (shrunk for a negative one).
fn inflate(rect: Rect, by: f32) -> Rect {
    Rect::new(rect.left - by, rect.top - by, rect.right + by, rect.bottom + by)
}

/// Draws `shape`'s drop shadow at `size`, as `DropShadowPainter`: the shape grown by the spread,
/// moved by the offset, filled blurred by the radius.
fn draw_drop_shadow(
    scope: &mut DrawScope<'_>,
    shape: &Shape,
    shadow: &Shadow,
    alpha: f32,
    filter: Option<&ColorFilter>,
) {
    let size = scope.size();
    let outline = shape.outline_in(size, &scope.density(), scope.layout_direction());
    let density = scope.density();
    let spread = density.to_px(shadow.spread);
    let offset = Offset::new(density.to_px(shadow.offset.x), density.to_px(shadow.offset.y));
    let paint = shadow.paint(scope, alpha, PaintStyle::Fill, filter);
    scope.translate(offset.x, offset.y, |scope| match &outline {
        Outline::Rectangle(rect) => scope.draw_rect(inflate(*rect, spread), paint),
        Outline::Rounded(rect, radii) => scope.draw_round_rect(
            inflate(*rect, spread),
            radii.map(|radius| radius + spread),
            paint,
        ),
        Outline::Generic(path, at) => {
            scope.draw_path_scaled(path, *at, (1.0, 1.0), paint);
            if spread > 0.0 {
                let mut stroke = paint;
                stroke.style = PaintStyle::Stroke(spread * 2.0);
                scope.draw_path_scaled(path, *at, (1.0, 1.0), stroke);
            }
        }
    });
}

/// Draws `shape`'s inner shadow at `size`, as `InnerShadowPainter`: clipped to the shape, what's
/// outside the shape (shrunk by the spread, moved by the offset) filled blurred by the radius,
/// so its blur reaches in from the edges.
fn draw_inner_shadow(
    scope: &mut DrawScope<'_>,
    shape: &Shape,
    shadow: &Shadow,
    alpha: f32,
    filter: Option<&ColorFilter>,
) {
    let size = scope.size();
    let density = scope.density();
    let outline = shape.outline_in(size, &density, scope.layout_direction());
    let (radius, spread) = (density.to_px(shadow.radius), density.to_px(shadow.spread));
    let offset = Offset::new(density.to_px(shadow.offset.x), density.to_px(shadow.offset.y));
    let paint = shadow.paint(scope, alpha, PaintStyle::Fill, filter);
    // Around the shape, far enough that the blur doesn't reach its outer edge.
    let reach = radius * 3.0 + spread + offset.x.abs().max(offset.y.abs()) + 1.0;
    let around = Path::new();
    around.add_rect(inflate(outline.bounds(), reach));
    let mut band = None;
    match &outline {
        Outline::Rectangle(rect) => around.add_rect(inflate(*rect, -spread)),
        Outline::Rounded(rect, radii) => {
            let mut round_rect = RoundRect::new(inflate(*rect, -spread), CornerRadius::default());
            let corners = radii.map(|radius| {
                let radius = (radius - spread).max(0.0);
                CornerRadius { x: radius, y: radius }
            });
            [
                round_rect.top_left_corner_radius,
                round_rect.top_right_corner_radius,
                round_rect.bottom_right_corner_radius,
                round_rect.bottom_left_corner_radius,
            ] = corners;
            around.add_round_rect(round_rect);
        }
        Outline::Generic(path, at) => {
            around.add_path(&Path::from_shared_data(std::sync::Arc::clone(path)), *at);
            // The spread as a band along the edge, as Compose strokes it.
            band = (spread > 0.0).then(|| (std::sync::Arc::clone(path), *at));
        }
    }
    around.set_fill_type(PathFillType::EvenOdd);
    scope.save();
    scope.clip_outline(outline);
    scope.draw_path_scaled(&around, offset, (1.0, 1.0), paint);
    if let Some((path, at)) = band {
        let mut stroke = paint;
        stroke.style = PaintStyle::Stroke(spread * 2.0);
        scope.draw_path_scaled(&path, at + offset, (1.0, 1.0), stroke);
    }
    scope.restore();
}

/// A painter of a shape's drop shadow, as Compose's `DropShadowPainter`: no intrinsic size,
/// faded and filtered itself.
pub struct DropShadowPainter {
    shape: Shape,
    shadow: Shadow,
    alpha: Cell<f32>,
    color_filter: Cell<Option<ColorFilter>>,
}

/// A painter of a shape's inner shadow, as Compose's `InnerShadowPainter`.
pub struct InnerShadowPainter {
    shape: Shape,
    shadow: Shadow,
    alpha: Cell<f32>,
    color_filter: Cell<Option<ColorFilter>>,
}

macro_rules! shadow_painter {
    ($painter:ident, $draw:ident) => {
        impl $painter {
            pub fn new(shape: Shape, shadow: Shadow) -> $painter {
                $painter { shape, shadow, alpha: Cell::new(1.0), color_filter: Cell::new(None) }
            }
        }

        impl CustomPainter for $painter {
            fn intrinsic_size(&self) -> Size {
                Size::UNSPECIFIED
            }
            fn on_draw(&self, scope: &mut DrawScope<'_>) {
                let filter = self.color_filter.get();
                $draw(scope, &self.shape, &self.shadow, self.alpha.get(), filter.as_ref());
            }
            fn apply_alpha(&self, alpha: f32) -> bool {
                self.alpha.set(alpha);
                true
            }
            fn apply_color_filter(&self, color_filter: Option<&ColorFilter>) -> bool {
                self.color_filter.set(color_filter.copied());
                true
            }
            fn apply_layout_direction(&self, _: crate::alignment::LayoutDirection) -> bool {
                true
            }
        }
    };
}

shadow_painter!(DropShadowPainter, draw_drop_shadow);
shadow_painter!(InnerShadowPainter, draw_inner_shadow);

impl Modifier {
    /// Draws `shape`'s drop shadow behind the content, as `Modifier.dropShadow(shape, shadow)`.
    pub fn drop_shadow(self, shape: Shape, shadow: Shadow) -> Modifier {
        self.then_custom(ShadowElement { shape, shadow, inner: false })
    }

    /// Draws `shape`'s inner shadow (before the content, as Compose's), as
    /// `Modifier.innerShadow(shape, shadow)`.
    pub fn inner_shadow(self, shape: Shape, shadow: Shadow) -> Modifier {
        self.then_custom(ShadowElement { shape, shadow, inner: true })
    }
}

#[derive(Clone)]
struct ShadowElement {
    shape: Shape,
    shadow: Shadow,
    inner: bool,
}

impl ModifierNodeElement for ShadowElement {
    type Node = ShadowElement;
    fn create(&self) -> ShadowElement {
        self.clone()
    }
    fn update(&self, node: &mut ShadowElement) {
        *node = self.clone();
    }
    fn same(&self, other: &Self) -> bool {
        self.shape == other.shape && self.shadow == other.shadow && self.inner == other.inner
    }
    fn name(&self) -> &'static str {
        if self.inner { "innerShadow" } else { "dropShadow" }
    }
}

impl ModifierNode for ShadowElement {
    fn as_draw(&mut self) -> Option<&mut dyn DrawModifierNode> {
        Some(self)
    }
}

impl DrawModifierNode for ShadowElement {
    fn draw(&mut self, scope: &mut DrawScope<'_>) {
        if self.inner {
            draw_inner_shadow(scope, &self.shape, &self.shadow, 1.0, None);
        } else {
            draw_drop_shadow(scope, &self.shape, &self.shadow, 1.0, None);
        }
    }
}
