//! Compose's `LayoutTest` helpers, for the ports of foundation-layout's device tests
//! (`RowColumnTest`, `BoxTest`, `PaddingTest`, `SizeTest`): a root of [`ROOT`] pixels at a
//! phone's density ([`DENSITY`], not 1, so the tests' `toDp`/`roundToPx` round trips are real),
//! `Container`, `ConstrainedBox`, `WithInfiniteConstraints`, `FixedSizeLayout`, and a
//! [`Probe`] for what Compose's tests save from `onGloballyPositioned`.

#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::Modifier;
use compose_test::*;
use compose_ui::platform::FixedTextMeasurer;
use compose_ui::{
    Alignment, AlignmentLine, Constraints, Density, Dp, IntSize, IntrinsicMeasurable,
    IntrinsicMeasureScope, Layout, LayoutCoordinates, LayoutDirection, Measurable, MeasurePolicy,
    MeasureResult, MeasureScope, Offset, PaddingValues, measure_policy,
};

/// The test device's density, as a phone's (Compose's tests run at the device's).
pub const DENSITY: f32 = 2.625;

/// The root's size, as the test activity's content: neither side divisible by 3.
pub const ROOT: IntSize = IntSize { width: 1000, height: 1600 };

/// `px` pixels in `Dp`, as `Int.toDp()` / `Float.toDp()` at [`DENSITY`].
pub fn px(px: f32) -> Dp {
    density().to_dp(px)
}

pub fn density() -> Density {
    Density::new(DENSITY)
}

/// As `Dp.roundToPx()` at [`DENSITY`].
pub fn round(dp: Dp) -> i32 {
    density().round_to_px(dp)
}

/// As `Dp.toPx()` at [`DENSITY`].
pub fn to_px(dp: Dp) -> f32 {
    density().to_px(dp)
}

/// As `show { content }`: `content` composed in a [`ROOT`] at [`DENSITY`], laid out.
pub fn show(content: impl Fn() + 'static) -> ComposeTestRule {
    show_in(ROOT, content)
}

/// [`show`] in a root of `size`.
pub fn show_in(size: IntSize, content: impl Fn() + 'static) -> ComposeTestRule {
    let mut rule = compose_test_with(size, density(), Box::new(FixedTextMeasurer::new()), content);
    rule.wait_for_idle();
    rule
}

/// Sizes and positions in the root saved by `on_globally_positioned`, per index, as the
/// tests' `childSize`/`childPosition` arrays (`(-1, -1)` until positioned).
#[derive(Clone)]
pub struct Probe(Rc<RefCell<Vec<(IntSize, Offset)>>>);

impl Probe {
    pub fn new(count: usize) -> Probe {
        Probe(Rc::new(RefCell::new(vec![(IntSize::new(-1, -1), Offset::new(-1.0, -1.0)); count])))
    }

    /// Saves its size and position in the root into `index`.
    pub fn at(&self, index: usize) -> Modifier {
        let seen = self.0.clone();
        Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
            seen.borrow_mut()[index] = (coordinates.size(), coordinates.position_in_root());
        })
    }

    pub fn size(&self, index: usize) -> IntSize {
        self.0.borrow()[index].0
    }

    pub fn position(&self, index: usize) -> Offset {
        self.0.borrow()[index].1
    }
}

pub fn offset(x: i32, y: i32) -> Offset {
    Offset::new(x as f32, y as f32)
}

pub fn size(width: i32, height: i32) -> IntSize {
    IntSize::new(width, height)
}

/// As `LayoutTest.DpConstraints`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DpConstraints {
    pub min_width: Dp,
    pub max_width: Dp,
    pub min_height: Dp,
    pub max_height: Dp,
}

impl Default for DpConstraints {
    fn default() -> DpConstraints {
        DpConstraints {
            min_width: Dp(0.0),
            max_width: Dp::INFINITY,
            min_height: Dp(0.0),
            max_height: Dp::INFINITY,
        }
    }
}

impl DpConstraints {
    pub fn fixed(width: Dp, height: Dp) -> DpConstraints {
        DpConstraints { min_width: width, max_width: width, min_height: height, max_height: height }
    }

    /// As `Density.Constraints(dpConstraints)`.
    pub fn to_px(self, density: Density) -> Constraints {
        Constraints::new(
            density.round_to_px(self.min_width),
            density.round_to_px(self.max_width),
            density.round_to_px(self.min_height),
            density.round_to_px(self.max_height),
        )
    }
}

/// `LayoutTest.Container`'s parameters but its modifier and content.
#[derive(Clone, Copy)]
pub struct Spec {
    pub padding: PaddingValues,
    pub alignment: Alignment,
    pub expanded: bool,
    pub constraints: DpConstraints,
    pub width: Option<Dp>,
    pub height: Option<Dp>,
}

impl Default for Spec {
    fn default() -> Spec {
        Spec {
            padding: PaddingValues::ZERO,
            alignment: Alignment::CENTER,
            expanded: false,
            constraints: DpConstraints::default(),
            width: None,
            height: None,
        }
    }
}

impl Spec {
    /// A container of `width` by `height`.
    pub fn sized(width: Dp, height: Dp) -> Spec {
        Spec { width: Some(width), height: Some(height), ..Spec::default() }
    }

    pub fn aligned(alignment: Alignment) -> Spec {
        Spec { alignment, ..Spec::default() }
    }
}

/// As `LayoutTest.Container`: its child, aligned by `spec.alignment` inside `spec.padding`,
/// measured within its own constraints (`width`, `height`, `constraints`); as large as its
/// constraints when fixed (or `expanded`) and bounded, else as its child and padding.
pub fn container(modifier: Modifier, spec: Spec, content: impl Fn() + 'static) {
    let policy = measure_policy(move |scope, measurables, incoming| {
        let density = scope.density();
        let own = spec.constraints.to_px(density);
        let pick = |fixed: Option<Dp>, or: i32| fixed.map_or(or, |dp| density.round_to_px(dp));
        let own = Constraints::new(
            pick(spec.width, own.min_width),
            pick(spec.width, own.max_width),
            pick(spec.height, own.min_height),
            pick(spec.height, own.max_height),
        );
        let container = incoming.constrain_by(own);
        let direction = scope.layout_direction();
        let (left, right) = match direction {
            LayoutDirection::Ltr => (spec.padding.start, spec.padding.end),
            LayoutDirection::Rtl => (spec.padding.end, spec.padding.start),
        };
        let (left, top) = (density.round_to_px(left), density.round_to_px(spec.padding.top));
        let horizontal = left + density.round_to_px(right);
        let vertical = top + density.round_to_px(spec.padding.bottom);
        let child = Constraints::new(0, container.max_width, 0, container.max_height)
            .offset(-horizontal, -vertical);
        let placeable = measurables.first().map(|&measurable| scope.measure(measurable, child));
        let width = if (container.has_fixed_width() || spec.expanded)
            && container.has_bounded_width()
        {
            container.max_width
        } else {
            (placeable.map_or(0, |placeable| placeable.width) + horizontal).max(container.min_width)
        };
        let height = if (container.has_fixed_height() || spec.expanded)
            && container.has_bounded_height()
        {
            container.max_height
        } else {
            (placeable.map_or(0, |placeable| placeable.height) + vertical).max(container.min_height)
        };
        if let Some(placeable) = &placeable {
            let position = spec.alignment.align(
                IntSize::new(placeable.width + horizontal, placeable.height + vertical),
                IntSize::new(width, height),
                direction,
            );
            scope.place(placeable, left + position.x, top + position.y);
        }
        scope.layout(width, height)
    });
    Layout(modifier, policy, compose_runtime::callback(content));
}

/// An empty [`container`].
pub fn empty(modifier: Modifier, spec: Spec) {
    container(modifier, spec, || {});
}

/// As `LayoutTest.FixedSizeLayout`: a leaf of `width` by `height` within its constraints,
/// with `lines`.
pub fn fixed_size_layout(width: i32, height: i32, lines: Vec<(AlignmentLine, i32)>) {
    let policy = measure_policy(move |scope, _, constraints| {
        scope.provide_alignment_lines(lines.iter().copied());
        scope.layout(constraints.constrain_width(width), constraints.constrain_height(height))
    });
    Layout(Modifier, policy, compose_runtime::callback(|| {}));
}

/// As `LayoutTest.WithInfiniteConstraints`: its children measured unbounded, at (0, 0), in a
/// layout of no size.
pub fn with_infinite_constraints(content: impl Fn() + 'static) {
    let policy = measure_policy(|scope, measurables, _| {
        for &measurable in measurables {
            let placeable = scope.measure(measurable, Constraints::unbounded());
            scope.place_relative(&placeable, 0, 0);
        }
        scope.layout(0, 0)
    });
    Layout(Modifier, policy, compose_runtime::callback(content));
}

/// As `LayoutTest.ConstrainedBox`'s policy: its child within `constraints` (and the incoming
/// ones), its intrinsics its child's within `constraints`.
struct ConstrainedPolicy(DpConstraints);

impl MeasurePolicy for ConstrainedPolicy {
    fn measure(
        &self,
        scope: &mut MeasureScope<'_>,
        measurables: &[Measurable],
        incoming: Constraints,
    ) -> MeasureResult {
        let child = incoming.constrain_by(self.0.to_px(scope.density()));
        let placeable = measurables.first().map(|&measurable| scope.measure(measurable, child));
        if let Some(placeable) = &placeable {
            scope.place_relative(placeable, 0, 0);
        }
        let width = placeable.map_or(child.min_width, |placeable| placeable.width);
        let height = placeable.map_or(child.min_height, |placeable| placeable.height);
        scope.layout(width, height)
    }

    fn min_intrinsic_width(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let width =
            measurables.first().map_or(0, |&child| scope.min_intrinsic_width(child, height));
        self.0.to_px(scope.density()).constrain_width(width)
    }

    fn min_intrinsic_height(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let height =
            measurables.first().map_or(0, |&child| scope.min_intrinsic_height(child, width));
        self.0.to_px(scope.density()).constrain_height(height)
    }

    fn max_intrinsic_width(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let width =
            measurables.first().map_or(0, |&child| scope.max_intrinsic_width(child, height));
        self.0.to_px(scope.density()).constrain_width(width)
    }

    fn max_intrinsic_height(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let height =
            measurables.first().map_or(0, |&child| scope.max_intrinsic_height(child, width));
        self.0.to_px(scope.density()).constrain_height(height)
    }
}

/// As `LayoutTest.ConstrainedBox`.
pub fn constrained_box(
    constraints: DpConstraints,
    modifier: Modifier,
    content: impl Fn() + 'static,
) {
    Layout(modifier, Rc::new(ConstrainedPolicy(constraints)), compose_runtime::callback(content));
}

/// As `RowColumnTest`'s `Center`: its child within its maximums only, centered in a layout as
/// large as its bounded constraints (else as its child).
pub fn center(content: impl Fn() + 'static) {
    let policy = measure_policy(|scope, measurables, constraints| {
        let placeable = measurables
            .first()
            .map(|&measurable| scope.measure(measurable, constraints.copy_max_dimensions()));
        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            placeable.map_or(constraints.min_width, |placeable| placeable.width)
        };
        let height = if constraints.has_bounded_height() {
            constraints.max_height
        } else {
            placeable.map_or(constraints.min_height, |placeable| placeable.height)
        };
        if let Some(placeable) = &placeable {
            let position = Alignment::CENTER.align(
                placeable.size(),
                IntSize::new(width, height),
                scope.layout_direction(),
            );
            scope.place_relative(placeable, position.x, position.y);
        }
        scope.layout(width, height)
    });
    Layout(Modifier, policy, compose_runtime::callback(content));
}
