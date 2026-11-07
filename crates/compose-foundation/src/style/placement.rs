//! `Modifier::placement` and `style_placement`, as Compose's `Placement.kt`: a layout node
//! sizing and offsetting the content from a block it runs while measuring (what the block reads
//! measures it again).

use std::rc::Rc;

use compose_ui::{
    Constraints, Dp, LayoutModifierNode, Measurable, MeasureResult, MeasureScope, Modifier,
    ModifierNode, ModifierNodeElement,
};

use super::properties::{
    BOTTOM, HEIGHT, LEFT, MAX_HEIGHT, MAX_WIDTH, MIN_HEIGHT, MIN_WIDTH, RIGHT, TOP, WIDTH,
};
use super::property::Breadth;
use super::resolver::StyleResolver;
use super::scope::StylePropertyAccessorScopeExt;

/// What a `placement` block sets, as Compose's `PlacementModifierScope`: all unspecified (none)
/// by default.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PlacementModifierScope {
    pub left: Dp,
    pub right: Dp,
    pub top: Dp,
    pub bottom: Dp,
    pub min_width: Dp,
    pub max_width: Dp,
    pub min_height: Dp,
    pub max_height: Dp,
    pub width: Breadth,
    pub height: Breadth,
}

impl Default for PlacementModifierScope {
    fn default() -> PlacementModifierScope {
        PlacementModifierScope {
            left: Dp::UNSPECIFIED,
            right: Dp::UNSPECIFIED,
            top: Dp::UNSPECIFIED,
            bottom: Dp::UNSPECIFIED,
            min_width: Dp::UNSPECIFIED,
            max_width: Dp::UNSPECIFIED,
            min_height: Dp::UNSPECIFIED,
            max_height: Dp::UNSPECIFIED,
            width: Breadth::None,
            height: Breadth::None,
        }
    }
}

type PlacementBlock = Rc<dyn Fn(&mut PlacementModifierScope)>;

struct PlacementElement(PlacementBlock);

impl ModifierNodeElement for PlacementElement {
    type Node = PlacementNode;

    fn create(&self) -> PlacementNode {
        PlacementNode { block: self.0.clone() }
    }

    fn update(&self, node: &mut PlacementNode) {
        node.block = self.0.clone();
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    fn name(&self) -> &'static str {
        "placement"
    }
}

/// As Compose's `PlacementNode`.
struct PlacementNode {
    block: PlacementBlock,
}

impl ModifierNode for PlacementNode {
    fn as_layout(&mut self) -> Option<&mut dyn LayoutModifierNode> {
        Some(self)
    }
}

/// `px` rounded half up, as `fastRoundToInt`.
fn round(px: f32) -> i32 {
    compose_ui::platform::round_to_int(px)
}

/// A finite maximum grown by `value` (not below 0), as `addMaxWithMinimum`.
fn add_max_with_minimum(max: i32, value: i32) -> i32 {
    if max == Constraints::INFINITY { max } else { (max + value).max(0) }
}

/// One direction's constraints, as Compose's measure: the style's minimum, maximum and size
/// within the incoming ones (less the offsets).
#[allow(clippy::too_many_arguments)]
fn resolve_axis(
    to_px: &dyn Fn(Dp) -> f32,
    mut min: i32,
    mut max: i32,
    style_min: Dp,
    style_max: Dp,
    size: Breadth,
    bounded: bool,
    both_offsets: bool,
) -> (i32, i32) {
    let mut style_min_px = 0;
    let mut style_max_px = Constraints::INFINITY;
    if style_max.is_specified() {
        style_max_px = round(to_px(style_max)).max(0);
    }
    if style_min.is_specified() {
        style_min_px = round(to_px(style_min)).clamp(0, style_max_px);
    }
    if let Breadth::Distance(distance) = size {
        style_min_px = round(to_px(distance)).clamp(style_min_px, style_max_px);
        style_max_px = style_min_px;
    }
    min = if style_min_px == 0 { min } else { style_min_px.clamp(min, max) };
    max = if style_max_px == Constraints::INFINITY { max } else { style_max_px.clamp(min, max) };
    if let Breadth::Fraction(fraction) = size {
        if bounded {
            let size = round(max as f32 * fraction).clamp(min, max);
            min = size;
            max = size;
        } else if both_offsets {
            min = max;
        }
    }
    (min, max)
}

impl LayoutModifierNode for PlacementNode {
    fn measure(
        &mut self,
        scope: &mut MeasureScope<'_>,
        content: Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let mut placement = PlacementModifierScope::default();
        (self.block)(&mut placement);
        let density = scope.density();
        let to_px = |dp: Dp| density.to_px(dp);
        let start = to_px(placement.left);
        let end = to_px(placement.right);
        let top = to_px(placement.top);
        let bottom = to_px(placement.bottom);
        // An unspecified side is NaN, and so is a sum with it, which rounds to 0 (`as i32`), as
        // `fastRoundToInt`: a side given alone moves the content without growing the node, as
        // Compose's (D1587).
        let horizontal = round(start + end);
        let vertical = round(top + bottom);
        let (min_width, max_width) = resolve_axis(
            &to_px,
            (constraints.min_width - horizontal).max(0),
            add_max_with_minimum(constraints.max_width, horizontal),
            placement.min_width,
            placement.max_width,
            placement.width,
            constraints.has_bounded_width(),
            placement.left.is_specified() && placement.right.is_specified(),
        );
        let (min_height, max_height) = resolve_axis(
            &to_px,
            (constraints.min_height - vertical).max(0),
            add_max_with_minimum(constraints.max_height, vertical),
            placement.min_height,
            placement.max_height,
            placement.height,
            constraints.has_bounded_height(),
            placement.top.is_specified() && placement.bottom.is_specified(),
        );
        let placeable =
            scope.measure(content, Constraints::new(min_width, max_width, min_height, max_height));
        let x = if !placement.left.is_specified() && placement.right.is_specified() {
            constraints.max_width - placeable.width - round(end)
        } else {
            round(start)
        };
        let y = if placement.bottom.is_specified() && !placement.top.is_specified() {
            constraints.max_height - placeable.height - round(bottom)
        } else {
            round(top)
        };
        scope.place(&placeable, x, y);
        scope.layout(placeable.width + horizontal, placeable.height + vertical)
    }
}

pub(crate) fn placement(
    modifier: Modifier,
    block: impl Fn(&mut PlacementModifierScope) + 'static,
) -> Modifier {
    modifier.then_custom(PlacementElement(Rc::new(block)))
}

pub(crate) fn style_placement(modifier: Modifier, resolver: StyleResolver) -> Modifier {
    placement(modifier, move |placement| {
        resolver.resolve(|scope| {
            scope.if_set(&TOP, |value| placement.top = value);
            scope.if_set(&BOTTOM, |value| placement.bottom = value);
            scope.if_set(&LEFT, |value| placement.left = value);
            scope.if_set(&RIGHT, |value| placement.right = value);
            scope.if_set(&MIN_HEIGHT, |value| placement.min_height = value);
            scope.if_set(&MAX_HEIGHT, |value| placement.max_height = value);
            scope.if_set(&MIN_WIDTH, |value| placement.min_width = value);
            scope.if_set(&MAX_WIDTH, |value| placement.max_width = value);
            scope.if_set(&WIDTH, |value| placement.width = value);
            scope.if_set(&HEIGHT, |value| placement.height = value);
        })
    })
}
