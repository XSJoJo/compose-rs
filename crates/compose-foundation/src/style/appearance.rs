//! `Modifier::appearance` and `style_appearance`, as Compose's `Appearance.kt`: a drawing node
//! painting a shape's background, foreground, border and shadows from a block it runs while
//! drawing (what the block reads draws it again, and only that).

use std::rc::Rc;

use compose_ui::{
    BorderRenderer, Brush, ContentDrawScope, CustomPainter, Dp, DrawModifierNode, Modifier,
    ModifierNode, ModifierNodeElement, PaintStyle, Shape,
};

use super::properties::{
    BACKGROUND, BORDER_FILL, BORDER_WIDTH, DROP_SHADOW, FOREGROUND, INNER_SHADOW, SHAPE,
};
use super::property::{Fill, Shadows};
use super::resolver::StyleResolver;
use super::scope::StylePropertyAccessorScopeExt;

/// What an `appearance` block sets, as Compose's `AppearanceScope`: nothing drawn but the
/// content by default.
#[derive(Clone, PartialEq, Debug)]
pub struct AppearanceScope {
    pub background: Fill,
    pub foreground: Fill,
    /// Unspecified: no border.
    pub border_width: Dp,
    /// Black if a width is set and it's none, as Compose's.
    pub border: Fill,
    pub drop_shadow: Shadows,
    pub inner_shadow: Shadows,
    pub shape: Shape,
}

impl Default for AppearanceScope {
    fn default() -> AppearanceScope {
        AppearanceScope {
            background: Fill::None,
            foreground: Fill::None,
            border_width: Dp::UNSPECIFIED,
            border: Fill::None,
            drop_shadow: Shadows::None,
            inner_shadow: Shadows::None,
            shape: Shape::Rectangle,
        }
    }
}

type AppearanceBlock = Rc<dyn Fn(&mut AppearanceScope)>;

/// `appearance`'s element: the same block is the same element, as Compose's (by identity).
struct AppearanceElement(AppearanceBlock);

impl ModifierNodeElement for AppearanceElement {
    type Node = AppearanceNode;

    fn create(&self) -> AppearanceNode {
        AppearanceNode { block: self.0.clone() }
    }

    fn update(&self, node: &mut AppearanceNode) {
        node.block = self.0.clone();
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    fn name(&self) -> &'static str {
        "appearance"
    }
}

/// As Compose's `AppearanceNode`.
struct AppearanceNode {
    block: AppearanceBlock,
}

impl ModifierNode for AppearanceNode {
    fn as_draw(&mut self) -> Option<&mut dyn DrawModifierNode> {
        Some(self)
    }
}

impl DrawModifierNode for AppearanceNode {
    fn draws_content(&self) -> bool {
        true
    }

    /// As `draw`: the drop shadow, the background, the content, the foreground, the border,
    /// the inner shadow.
    fn draw_with_content(&mut self, scope: &mut ContentDrawScope<'_, '_>) {
        let mut appearance = AppearanceScope::default();
        (self.block)(&mut appearance);
        let shape = &appearance.shape;
        draw_shadows(scope, &appearance.drop_shadow, shape, false);
        let size = scope.size();
        let (density, direction) = (scope.density(), scope.layout_direction());
        let outline = shape.outline_in(size, &density, direction);
        if let Some(brush) = fill_brush(&appearance.background) {
            let paint = scope.brush_paint(&brush, 1.0, PaintStyle::Fill);
            scope.draw_outline(outline.clone(), paint);
        }
        scope.draw_content();
        if let Some(brush) = fill_brush(&appearance.foreground) {
            let paint = scope.brush_paint(&brush, 1.0, PaintStyle::Fill);
            scope.draw_outline(outline.clone(), paint);
        }
        let width = appearance.border_width;
        // As Compose's `(borderWidth / 2).roundToPx() > 0`.
        let has_border = width.is_specified() && density.round_to_px(Dp(width.0 / 2.0)) > 0;
        if has_border {
            let brush = match &appearance.border {
                Fill::Brush(brush) => brush.clone(),
                Fill::Color(color) => Brush::Solid(*color),
                Fill::None => Brush::Solid(compose_ui::Color::BLACK),
            };
            BorderRenderer.draw_border(scope, width, &brush, &outline);
        }
        draw_shadows(scope, &appearance.inner_shadow, shape, true);
    }
}

/// What a fill paints with: none for none or an unspecified color.
fn fill_brush(fill: &Fill) -> Option<Brush> {
    match fill {
        Fill::None => None,
        Fill::Color(color) => color.is_specified().then_some(Brush::Solid(*color)),
        Fill::Brush(brush) => Some(brush.clone()),
    }
}

fn draw_shadows(
    scope: &mut ContentDrawScope<'_, '_>,
    shadows: &Shadows,
    shape: &Shape,
    inner: bool,
) {
    let mut draw = |shadow: &compose_ui::shadow::Shadow| {
        if inner {
            compose_ui::shadow::InnerShadowPainter::new(shape.clone(), shadow.clone())
                .on_draw(scope);
        } else {
            compose_ui::shadow::DropShadowPainter::new(shape.clone(), shadow.clone())
                .on_draw(scope);
        }
    };
    match shadows {
        Shadows::None => {}
        Shadows::Simple(shadow) => draw(shadow),
        Shadows::Compound(all) => all.iter().for_each(draw),
    }
}

pub(crate) fn appearance(
    modifier: Modifier,
    block: impl Fn(&mut AppearanceScope) + 'static,
) -> Modifier {
    modifier.then_custom(AppearanceElement(Rc::new(block)))
}

pub(crate) fn style_appearance(modifier: Modifier, resolver: StyleResolver) -> Modifier {
    appearance(modifier, move |appearance| {
        resolver.resolve(|scope| {
            scope.if_set(&SHAPE, |shape| appearance.shape = shape);
            scope.if_set(&BACKGROUND, |fill| appearance.background = fill);
            scope.if_set(&FOREGROUND, |fill| appearance.foreground = fill);
            scope.if_set(&BORDER_FILL, |fill| appearance.border = fill);
            scope.if_set(&BORDER_WIDTH, |width| appearance.border_width = width);
            scope.if_set(&DROP_SHADOW, |shadows| appearance.drop_shadow = shadows);
            scope.if_set(&INNER_SHADOW, |shadows| appearance.inner_shadow = shadows);
        })
    })
}
