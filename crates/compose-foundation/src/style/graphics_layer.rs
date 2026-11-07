//! The style graphics layer, as Compose's `StyleGraphicsLayerModifier.kt`: content placed with
//! a layer whose properties a block gives (read where the layer computes them, so what it reads
//! updates the layer only), when a predicate says a layer is needed.

use std::rc::Rc;

use compose_ui::{
    Constraints, GraphicsLayerScope, LayerBlock, LayoutModifierNode, Measurable, MeasureResult,
    MeasureScope, Modifier, ModifierNode, ModifierNodeElement, TransformOrigin,
};

use super::properties::{
    ALPHA, CAMERA_DISTANCE, CLIP, COLOR_FILTER, ROTATION_X, ROTATION_Y, ROTATION_Z, SCALE_X,
    SCALE_Y, SHAPE, TRANSFORM_ORIGIN_X, TRANSFORM_ORIGIN_Y, TRANSLATION_X, TRANSLATION_Y,
};
use super::property::AnyStyleProperty;
use super::resolver::StyleResolver;
use super::scope::StylePropertyAccessorScopeExt;

type LayerRequired = Rc<dyn Fn() -> bool>;

struct StyleGraphicsLayerElement {
    layer_required: Option<LayerRequired>,
    block: LayerBlock,
}

impl ModifierNodeElement for StyleGraphicsLayerElement {
    type Node = StyleGraphicsLayerNode;

    fn create(&self) -> StyleGraphicsLayerNode {
        StyleGraphicsLayerNode {
            layer_required: self.layer_required.clone(),
            block: self.block.clone(),
        }
    }

    /// As `update`: a new predicate measures again; a new block is the layer's.
    fn update(&self, node: &mut StyleGraphicsLayerNode) {
        node.layer_required = self.layer_required.clone();
        node.block = self.block.clone();
    }

    fn same(&self, other: &Self) -> bool {
        let same_required = match (&self.layer_required, &other.layer_required) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        same_required && self.block.ptr_eq(&other.block)
    }

    fn name(&self) -> &'static str {
        "graphicsLayer"
    }
}

/// As Compose's `StyleGraphicsLayerNode`.
struct StyleGraphicsLayerNode {
    layer_required: Option<LayerRequired>,
    block: LayerBlock,
}

impl ModifierNode for StyleGraphicsLayerNode {
    fn as_layout(&mut self) -> Option<&mut dyn LayoutModifierNode> {
        Some(self)
    }
}

impl LayoutModifierNode for StyleGraphicsLayerNode {
    fn measure(
        &mut self,
        scope: &mut MeasureScope<'_>,
        content: Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = scope.measure(content, constraints);
        let required = self.layer_required.clone();
        let block = self.block.clone();
        // Placed in a block, as Compose's: what the predicate reads places again.
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            if required.as_ref().is_none_or(|required| required()) {
                placement.place_with_layer(&placeable, 0, 0, block.clone());
            } else {
                placement.place(&placeable, 0, 0);
            }
        })
    }
}

/// The properties a style layer applies.
static LAYER_PROPERTIES: [&dyn AnyStyleProperty; 12] = [
    &ALPHA,
    &TRANSLATION_X,
    &TRANSLATION_Y,
    &TRANSFORM_ORIGIN_X,
    &TRANSFORM_ORIGIN_Y,
    &ROTATION_X,
    &ROTATION_Y,
    &ROTATION_Z,
    &SCALE_X,
    &SCALE_Y,
    &CAMERA_DISTANCE,
    &COLOR_FILTER,
];

pub(crate) fn graphics_layer_when(
    modifier: Modifier,
    layer_required: Option<Rc<dyn Fn() -> bool>>,
    block: impl Fn(&mut GraphicsLayerScope) + 'static,
) -> Modifier {
    modifier
        .then_custom(StyleGraphicsLayerElement { layer_required, block: LayerBlock::new(block) })
}

pub(crate) fn style_graphics_layer(modifier: Modifier, resolver: StyleResolver) -> Modifier {
    let required = resolver.clone();
    let layer_required: Rc<dyn Fn() -> bool> = Rc::new(move || {
        required.resolve(|scope| scope.any_set(&LAYER_PROPERTIES) || scope.value(&CLIP))
    });
    graphics_layer_when(modifier, Some(layer_required), move |layer| {
        resolver.resolve(|scope| {
            layer.clip = scope.value(&CLIP);
            layer.shape = scope.value(&SHAPE);
            layer.alpha = scope.value(&ALPHA);
            layer.translation_x = scope.value(&TRANSLATION_X);
            layer.translation_y = scope.value(&TRANSLATION_Y);
            layer.transform_origin = TransformOrigin {
                pivot_fraction_x: scope.value(&TRANSFORM_ORIGIN_X),
                pivot_fraction_y: scope.value(&TRANSFORM_ORIGIN_Y),
            };
            layer.rotation_z = scope.value(&ROTATION_Z);
            layer.scale_x = scope.value(&SCALE_X);
            layer.scale_y = scope.value(&SCALE_Y);
        })
    })
}
