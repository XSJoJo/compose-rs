//! The style paddings, as Compose's `StylePadding.kt`: `Modifier.padding` with the values a
//! resolver resolves, read while measuring (what they read measures again).

use compose_ui::{
    Constraints, Density, Dp, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
    Modifier, ModifierNode, ModifierNodeElement,
};

use super::properties::{
    BORDER_WIDTH, CONTENT_PADDING_BOTTOM, CONTENT_PADDING_END, CONTENT_PADDING_START,
    CONTENT_PADDING_TOP, EXTERNAL_PADDING_BOTTOM, EXTERNAL_PADDING_END, EXTERNAL_PADDING_START,
    EXTERNAL_PADDING_TOP,
};
use super::resolver::{StyleResolver, StyleResolverScope};
use super::scope::StylePropertyAccessorScopeExt;

/// Which padding a node reads.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Which {
    Content,
    External,
    Border,
}

struct StylePaddingElement(StyleResolver, Which);

impl ModifierNodeElement for StylePaddingElement {
    type Node = StylePaddingNode;

    fn create(&self) -> StylePaddingNode {
        StylePaddingNode(self.0.clone(), self.1)
    }

    fn update(&self, node: &mut StylePaddingNode) {
        *node = StylePaddingNode(self.0.clone(), self.1);
    }

    fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0) && self.1 == other.1
    }

    fn name(&self) -> &'static str {
        "padding"
    }
}

struct StylePaddingNode(StyleResolver, Which);

impl ModifierNode for StylePaddingNode {
    fn as_layout(&mut self) -> Option<&mut dyn LayoutModifierNode> {
        Some(self)
    }
}

/// The border's width as a padding, as `effectiveBorderWidth`: a hairline is a pixel, a
/// fraction of a pixel rounds up.
fn effective_border_width(scope: &StyleResolverScope<'_>, density: Density) -> Dp {
    match scope.get_or_null(&BORDER_WIDTH) {
        None => Dp(0.0),
        Some(width) => {
            let px = if width == Dp::HAIRLINE { 1.0 } else { density.to_px(width).ceil() };
            density.to_dp(px)
        }
    }
}

impl LayoutModifierNode for StylePaddingNode {
    /// As `Modifier.padding(paddingValues)`'s measure.
    fn measure(
        &mut self,
        scope: &mut MeasureScope<'_>,
        content: Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let density = scope.density();
        let (start, top, end, bottom) = self.0.resolve(|resolved| match self.1 {
            Which::Content => (
                resolved.value(&CONTENT_PADDING_START),
                resolved.value(&CONTENT_PADDING_TOP),
                resolved.value(&CONTENT_PADDING_END),
                resolved.value(&CONTENT_PADDING_BOTTOM),
            ),
            Which::External => (
                resolved.value(&EXTERNAL_PADDING_START),
                resolved.value(&EXTERNAL_PADDING_TOP),
                resolved.value(&EXTERNAL_PADDING_END),
                resolved.value(&EXTERNAL_PADDING_BOTTOM),
            ),
            Which::Border => {
                let width = effective_border_width(resolved, density);
                (width, width, width, width)
            }
        });
        let (start, top) = (density.round_to_px(start), density.round_to_px(top));
        let horizontal = start + density.round_to_px(end);
        let vertical = top + density.round_to_px(bottom);
        let placeable = scope.measure(content, constraints.offset(-horizontal, -vertical));
        let width = constraints.constrain_width(placeable.width + horizontal);
        let height = constraints.constrain_height(placeable.height + vertical);
        scope.place_relative(&placeable, start, top);
        scope.layout(width, height)
    }
}

pub(crate) fn style_content_padding(modifier: Modifier, resolver: StyleResolver) -> Modifier {
    modifier.then_custom(StylePaddingElement(resolver, Which::Content))
}

pub(crate) fn style_external_padding(modifier: Modifier, resolver: StyleResolver) -> Modifier {
    modifier.then_custom(StylePaddingElement(resolver, Which::External))
}

pub(crate) fn style_border_padding(modifier: Modifier, resolver: StyleResolver) -> Modifier {
    modifier.then_custom(StylePaddingElement(resolver, Which::Border))
}
