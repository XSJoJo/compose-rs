//! `Modifier.animate_bounds`, as Compose's `animateBounds` (`AnimateBoundsModifier.kt`): two
//! approach nodes around a modifier, animating a layout's size and position in a
//! `LookaheadScope` from where it was to where the pass ahead puts it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_runtime::{CoroutineScope, OwnedState, mutable_state_of};
use compose_ui::{
    ApproachLayoutModifierNode, ApproachMeasureScope, Constraints, IntSize, LayoutCoordinates,
    LayoutModifierNode, LookaheadCoordinates, LookaheadScope, Measurable, MeasureResult,
    MeasureScope, Modifier, ModifierNode, ModifierNodeElement, NodeContext, Offset,
    PlacementInProgress, PlacementScope, Rect, Size,
};

use crate::animatable::Animatable;
use crate::shared::{BoundsTransform, SharedTransitionDefaults};

/// The bounds-animating modifier, on `Modifier`.
pub trait BoundsModifiers {
    /// Animates this layout's size and position in `lookahead_scope` whenever the pass ahead
    /// changes them, along Compose's default spring, as `Modifier.animateBounds(lookaheadScope)`.
    fn animate_bounds(self, lookahead_scope: LookaheadScope) -> Modifier;

    /// The same with `modifier` between the animation's two nodes (what's measured at the
    /// animated size), the animation's spec from `bounds_transform`, and what a motion frame of
    /// reference (a scroll) moves animated too if `animate_motion_frame_of_reference`, as
    /// `Modifier.animateBounds(lookaheadScope, modifier, boundsTransform,
    /// animateMotionFrameOfReference)`.
    fn animate_bounds_with(
        self,
        lookahead_scope: LookaheadScope,
        modifier: Modifier,
        bounds_transform: BoundsTransform,
        animate_motion_frame_of_reference: bool,
    ) -> Modifier;
}

impl BoundsModifiers for Modifier {
    fn animate_bounds(self, lookahead_scope: LookaheadScope) -> Modifier {
        self.animate_bounds_with(
            lookahead_scope,
            Modifier,
            SharedTransitionDefaults::bounds_transform(),
            false,
        )
    }

    fn animate_bounds_with(
        self,
        lookahead_scope: LookaheadScope,
        modifier: Modifier,
        bounds_transform: BoundsTransform,
        animate_motion_frame_of_reference: bool,
    ) -> Modifier {
        let element = |inner| BoundsElement {
            scope: lookahead_scope,
            transform: bounds_transform.clone(),
            inner,
            motion: animate_motion_frame_of_reference,
        };
        self.then_custom(element(false)).then(modifier).then_custom(element(true))
    }
}

/// `animate_bounds`' element, as `BoundsAnimationElement`: the outer node (`inner` false) passes
/// its constraints through, the inner one measures at the animated size.
struct BoundsElement {
    scope: LookaheadScope,
    transform: BoundsTransform,
    inner: bool,
    motion: bool,
}

impl ModifierNodeElement for BoundsElement {
    type Node = BoundsNode;

    fn create(&self) -> BoundsNode {
        let state = Rc::new(BoundsState {
            params: RefCell::new(Params {
                scope: self.scope,
                transform: self.transform.clone(),
                motion: self.motion,
            }),
            animation: RefCell::new(DeferredBounds::default()),
            animated_value: mutable_state_of(None),
            parents_dirty: Cell::new(true),
            coroutines: Cell::new(None),
        });
        let in_progress = {
            let state = state.clone();
            Rc::new(move |placement: &mut PlacementScope<'_>, _: LookaheadCoordinates| {
                state.placement_in_progress(placement)
            }) as PlacementInProgress
        };
        BoundsNode { state, inner: self.inner, in_progress, coroutines: CoroutineScope::new() }
    }

    fn update(&self, node: &mut BoundsNode) {
        *node.state.params.borrow_mut() =
            Params { scope: self.scope, transform: self.transform.clone(), motion: self.motion };
        node.inner = self.inner;
    }

    fn same(&self, other: &Self) -> bool {
        self.scope == other.scope
            && self.transform == other.transform
            && self.inner == other.inner
            && self.motion == other.motion
    }

    fn name(&self) -> &'static str {
        "boundsAnimation"
    }
}

/// What the element gives, read where the node's blocks run.
struct Params {
    scope: LookaheadScope,
    transform: BoundsTransform,
    motion: bool,
}

/// `BoundsAnimationModifierNode`.
struct BoundsNode {
    state: Rc<BoundsState>,
    inner: bool,
    in_progress: PlacementInProgress,
    coroutines: compose_runtime::Owned<CoroutineScope>,
}

/// What the node and its placement blocks share.
struct BoundsState {
    params: RefCell<Params>,
    animation: RefCell<DeferredBounds>,
    /// The animation's value, as `animatedValue` (a state: read where it's measured).
    animated_value: OwnedState<Option<Rect>>,
    /// The motion frames of reference it went through are to be looked at again, as
    /// `directManipulationParentsDirty`.
    parents_dirty: Cell<bool>,
    /// The node's coroutine scope while it's attached.
    coroutines: Cell<Option<CoroutineScope>>,
}

impl BoundsState {
    /// The animation's value, none while it's idle, as `value`.
    fn value(&self) -> Option<Rect> {
        let idle = self.animation.borrow().is_idle();
        if idle { None } else { self.animated_value.get() }
    }

    /// As `isPlacementApproachInProgress`: notes where it goes and animates there.
    fn placement_in_progress(&self, placement: &mut PlacementScope<'_>) -> bool {
        let (scope, transform, motion) = {
            let params = self.params.borrow();
            (params.scope, params.transform.clone(), params.motion)
        };
        let dirty = self.parents_dirty.get();
        let value = self.animation.borrow_mut().update_target_offset_and_animate(
            scope,
            placement,
            self.coroutines.get(),
            dirty,
            motion,
            &transform,
        );
        if let Some(value) = value {
            self.animated_value.set(Some(value));
        }
        self.parents_dirty.set(motion);
        !self.animation.borrow().is_idle()
    }
}

impl ModifierNode for BoundsNode {
    fn as_layout(&mut self) -> Option<&mut dyn LayoutModifierNode> {
        Some(self)
    }

    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        context.approach_attached();
        self.state.parents_dirty.set(true);
        self.state.coroutines.set(Some(*self.coroutines));
    }

    /// Detached, its animation stops, as with Compose's node scope.
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.coroutines.cancel();
        self.coroutines = CoroutineScope::new();
        self.state.coroutines.set(None);
    }
}

impl LayoutModifierNode for BoundsNode {
    fn measure(
        &mut self,
        scope: &mut MeasureScope<'_>,
        content: Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        compose_ui::measure_approach(self, scope, content, constraints)
    }
}

impl ApproachLayoutModifierNode for BoundsNode {
    fn is_measurement_approach_in_progress(&mut self, lookahead_size: IntSize) -> bool {
        let mut animation = self.state.animation.borrow_mut();
        animation.update_target_size(to_size(lookahead_size));
        !animation.is_idle()
    }

    fn placement_approach_in_progress(&self) -> PlacementInProgress {
        self.in_progress.clone()
    }

    fn approach_measure(
        &mut self,
        scope: &mut ApproachMeasureScope<'_, '_>,
        content: Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let fallback = {
            let animation = self.state.animation.borrow();
            if animation.current_size.is_unspecified() {
                to_size(scope.lookahead_size())
            } else {
                animation.current_size
            }
        };
        let animated = self.state.value().map_or(fallback, |it| it.size()).round_to_int_size();
        let chosen = if self.inner {
            Constraints::fixed(animated.width, animated.height)
        } else {
            constraints
        };
        let placeable = scope.measure(content, chosen);
        let size = chosen.constrain(animated);
        let state = self.state.clone();
        scope.layout_with(size.width, size.height, move |placement| {
            let animated = state.value();
            let (lookahead_scope, motion) = {
                let params = state.params.borrow();
                (params.scope, params.motion)
            };
            let in_scope = placement.coordinates().and_then(|coordinates| {
                let scope_coordinates = lookahead_scope.lookahead_scope_coordinates(placement)?;
                Some(scope_coordinates.local_position_of_with(&coordinates, Offset::ZERO, motion))
            });
            let top_left = match animated {
                Some(bounds) => {
                    state
                        .animation
                        .borrow_mut()
                        .update_current_bounds(bounds.top_left(), bounds.size());
                    bounds.top_left()
                }
                None => state
                    .animation
                    .borrow()
                    .current_bounds()
                    .map_or(Offset::ZERO, |it| it.top_left()),
            };
            let at = in_scope.map_or(Offset::ZERO, |it| top_left - it);
            placement.place(&placeable, at.x.round() as i32, at.y.round() as i32);
        })
    }
}

fn to_size(size: IntSize) -> Size {
    Size::new(size.width as f32, size.height as f32)
}

/// The animation of the bounds toward the target the pass ahead gives, as
/// `BoundsTransformDeferredAnimation`.
struct DeferredBounds {
    animatable: Option<Animatable<Rect>>,
    target_size: Size,
    target_offset: Offset,
    /// The target changed: an animation is to start.
    pending: bool,
    current_position: Offset,
    current_size: Size,
    /// The motion frames of reference between it and the scope, as `directManipulationParents`.
    parents: Vec<LayoutCoordinates>,
    /// What they moved it by, which the target leaves out, as `additionalOffset`.
    additional_offset: Offset,
}

impl Default for DeferredBounds {
    fn default() -> DeferredBounds {
        DeferredBounds {
            animatable: None,
            target_size: Size::UNSPECIFIED,
            target_offset: Offset::UNSPECIFIED,
            pending: false,
            current_position: Offset::UNSPECIFIED,
            current_size: Size::UNSPECIFIED,
            parents: Vec::new(),
            additional_offset: Offset::ZERO,
        }
    }
}

impl DeferredBounds {
    fn update_target_size(&mut self, size: Size) {
        if self.target_size.is_specified()
            && size.round_to_int_size() != self.target_size.round_to_int_size()
        {
            self.pending = true;
        }
        self.target_size = size;
        if self.current_size.is_unspecified() {
            self.current_size = size;
        }
    }

    fn update_target_offset(&mut self, offset: Offset) {
        if self.target_offset.is_specified() && offset.round() != self.target_offset.round() {
            self.pending = true;
        }
        self.target_offset = offset;
        if self.current_position.is_unspecified() {
            self.current_position = offset;
        }
    }

    fn current_bounds(&self) -> Option<Rect> {
        let (size, position) = (self.current_size, self.current_position);
        (position.is_specified() && size.is_specified())
            .then(|| Rect::from_offset_size(position, size))
    }

    fn update_current_bounds(&mut self, position: Offset, size: Size) {
        self.current_position = position;
        self.current_size = size;
    }

    /// As `isIdle` (reads whether the animation runs).
    fn is_idle(&self) -> bool {
        !self.pending && !self.animatable.as_ref().is_some_and(Animatable::is_running)
    }

    /// As `updateTargetOffsetAndAnimate`: the animation's value, if the node has coordinates.
    fn update_target_offset_and_animate(
        &mut self,
        lookahead_scope: LookaheadScope,
        placement: &mut PlacementScope<'_>,
        coroutines: Option<CoroutineScope>,
        parents_dirty: bool,
        include_motion_frame_of_reference: bool,
        transform: &BoundsTransform,
    ) -> Option<Rect> {
        let coordinates = placement.coordinates()?;
        let scope_coordinates = lookahead_scope.lookahead_scope_coordinates(placement)?;
        let mut delta = Offset::ZERO;
        if !include_motion_frame_of_reference && parents_dirty {
            let mut current = coordinates;
            let mut index = 0;
            while current != scope_coordinates {
                if current.introduces_motion_frame_of_reference() {
                    if self.parents.len() == index {
                        self.parents.push(current);
                        delta = delta + current.position_in_parent();
                    } else if self.parents[index] != current {
                        delta = delta - self.parents[index].position_in_parent();
                        self.parents[index] = current;
                        delta = delta + current.position_in_parent();
                    }
                    index += 1;
                }
                match current.parent_coordinates() {
                    Some(parent) => current = parent,
                    None => break,
                }
            }
            while self.parents.len() > index {
                let parent = self.parents.pop().expect("a parent past the index");
                delta = delta - parent.position_in_parent();
            }
        }
        self.additional_offset = self.additional_offset + delta;
        let target = lookahead_scope.local_lookahead_position_of_with(
            scope_coordinates,
            coordinates,
            Offset::ZERO,
            include_motion_frame_of_reference,
        );
        self.update_target_offset(target + self.additional_offset);
        let value = self.animate(coroutines, transform);
        Some(value.translate(Offset::ZERO - self.additional_offset))
    }

    fn animate(&mut self, coroutines: Option<CoroutineScope>, transform: &BoundsTransform) -> Rect {
        if self.target_offset.is_specified() && self.target_size.is_specified() {
            let target = Rect::from_offset_size(self.target_offset, self.target_size);
            let animatable = self
                .animatable
                .get_or_insert_with(|| Animatable::new_with(target, None, "Animatable"))
                .clone();
            if self.pending {
                self.pending = false;
                let current = self.current_bounds().unwrap_or(target);
                let spec = transform.create_animation_spec(current, target);
                if let Some(coroutines) = coroutines {
                    let running = animatable.clone();
                    let job = coroutines.launch(async move {
                        running.animate_to(target, spec).await;
                    });
                    // Started at once, as Compose's `CoroutineStart.UNDISPATCHED`: it runs from
                    // this frame.
                    compose_runtime::platform::poll_now(job);
                }
            }
            return animatable.value();
        }
        self.animatable.as_ref().map_or(Rect::ZERO, Animatable::value)
    }
}
