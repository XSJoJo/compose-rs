//! Zooming, panning and rotating with several fingers (or Ctrl and the mouse wheel), as Compose's
//! `Transformable.kt`, `TransformableState.kt` and `TransformGestureDetector.kt`: the
//! `PointerEvents` helpers that measure a gesture (`calculate_zoom`, …), the detector on a
//! `pointer_input` block, the state the transformations go to and `Modifier::transformable`.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::future::Future;
use std::rc::Rc;
use std::task::{Poll, Waker};

use compose_animation::{
    AnimationSpec, AnimationState, AnimationVector, Spring, VectorizedAnimationSpec, spring_with,
};
use compose_runtime::{ComposeArg, Job, OwnedState, composable};
use compose_ui::platform::PointerInputDriver;
use compose_ui::{
    EventContext, IntSize, Modifier, ModifierNode, ModifierNodeElement, NodeContext, Offset,
    PointerEventPass, PointerEventType, PointerEvents, PointerInputChange,
    PointerInputModifierNode, PointerInputScope, PointerType,
};

use super::scrollable::LocalBoxFuture;
use crate::mutator::{MutatePriority, MutationInterrupted, MutatorMutex};

// ---- Measuring a gesture -----------------------------------------------------------------------

/// What a multi-pointer event says of a transformation, as Compose's `PointerEvent` extensions in
/// `TransformGestureDetector.kt`. Only the pointers down before and after the event count.
pub trait TransformCalculations {
    /// The rotation of the pointers around their centroid, in degrees (clockwise positive),
    /// weighed by their distance to it, as `calculateRotation`: 0 with fewer than two.
    fn calculate_rotation(&self) -> f32;
    /// How much farther from their centroid the pointers are, as `calculateZoom`: 1 without
    /// distance.
    fn calculate_zoom(&self) -> f32;
    /// How far the centroid moved, as `calculatePan`.
    fn calculate_pan(&self) -> Offset;
    /// The pointers' average distance to their centroid, now or before the event, as
    /// `calculateCentroidSize`.
    fn calculate_centroid_size(&self, use_current: bool) -> f32;
    /// The pointers' average position, now or before the event (`Offset::UNSPECIFIED` with none
    /// down), as `calculateCentroid`.
    fn calculate_centroid(&self, use_current: bool) -> Offset;
}

fn still_down(change: &PointerInputChange) -> bool {
    change.pressed && change.previous_pressed
}

/// The average position of the changes `counted`, as the internal `calculateCentroid(matcher)`.
fn centroid_of(
    event: &PointerEvents,
    use_current: bool,
    counted: fn(&PointerInputChange) -> bool,
) -> Offset {
    let (mut sum, mut count) = (Offset::ZERO, 0);
    for change in event.changes.iter().filter(|change| counted(change)) {
        sum = sum + if use_current { change.position } else { change.previous_position };
        count += 1;
    }
    if count == 0 {
        Offset::UNSPECIFIED
    } else {
        Offset::new(sum.x / count as f32, sum.y / count as f32)
    }
}

fn distance(offset: Offset) -> f32 {
    offset.x.hypot(offset.y)
}

/// The angle of `offset`, in degrees, as `angleDeg`.
fn angle_degrees(offset: Offset) -> f32 {
    if offset.x == 0.0 && offset.y == 0.0 { 0.0 } else { -offset.x.atan2(offset.y) * 180.0 / PI }
}

impl TransformCalculations for PointerEvents {
    fn calculate_rotation(&self) -> f32 {
        if self.changes.iter().filter(|change| still_down(change)).count() < 2 {
            return 0.0;
        }
        let current = self.calculate_centroid(true);
        let previous = self.calculate_centroid(false);
        let (mut rotation, mut weights) = (0.0, 0.0);
        for change in self.changes.iter().filter(|change| still_down(change)) {
            let before = change.previous_position - previous;
            let now = change.position - current;
            let difference = angle_degrees(now) - angle_degrees(before);
            // Farther from the centroid, a move is a steadier angle: it weighs more.
            let weight = distance(now + before) / 2.0;
            let difference = if difference > 180.0 {
                difference - 360.0
            } else if difference < -180.0 {
                difference + 360.0
            } else {
                difference
            };
            rotation += difference * weight;
            weights += weight;
        }
        if weights == 0.0 { 0.0 } else { rotation / weights }
    }

    fn calculate_zoom(&self) -> f32 {
        let current = self.calculate_centroid_size(true);
        let previous = self.calculate_centroid_size(false);
        if current == 0.0 || previous == 0.0 { 1.0 } else { current / previous }
    }

    fn calculate_pan(&self) -> Offset {
        let current = self.calculate_centroid(true);
        if !current.is_specified() {
            return Offset::ZERO;
        }
        current - self.calculate_centroid(false)
    }

    fn calculate_centroid_size(&self, use_current: bool) -> f32 {
        let centroid = self.calculate_centroid(use_current);
        if !centroid.is_specified() {
            return 0.0;
        }
        let (mut sum, mut count) = (0.0, 0);
        for change in self.changes.iter().filter(|change| still_down(change)) {
            let position = if use_current { change.position } else { change.previous_position };
            sum += distance(position - centroid);
            count += 1;
        }
        sum / count as f32
    }

    fn calculate_centroid(&self, use_current: bool) -> Offset {
        centroid_of(self, use_current, still_down)
    }
}

/// Runs `gesture` for each gesture, from a first press to every pointer up, as
/// `awaitEachGesture`.
async fn each_gesture<F: Future<Output = ()>>(scope: &PointerInputScope, gesture: impl Fn() -> F) {
    loop {
        gesture().await;
        while scope.any_pressed() {
            scope.await_pointer_event(PointerEventPass::Final).await;
        }
    }
}

/// A pointer moved: past the slop, the moves that changed something are consumed.
struct Slop {
    rotation: f32,
    zoom: f32,
    pan: Offset,
    past: bool,
    locked_to_pan_zoom: bool,
}

/// What a step of a gesture past the slop changes, as the detectors' `onGesture` arguments.
struct Step {
    centroid: Offset,
    zoom: f32,
    pan: Offset,
    rotation: f32,
}

impl Slop {
    fn new() -> Slop {
        Slop { rotation: 0.0, zoom: 1.0, pan: Offset::ZERO, past: false, locked_to_pan_zoom: false }
    }

    /// Takes `event`: once past the slop, the step it makes, if it makes one, and its moves
    /// consumed. Whether a pan alone may start or continue it is `can_pan`'s.
    fn step(
        &mut self,
        event: &PointerEvents,
        touch_slop: f32,
        pan_zoom_lock: bool,
        can_pan: &dyn Fn(Offset) -> bool,
    ) -> Option<Step> {
        let zoom_change = event.calculate_zoom();
        let rotation_change = event.calculate_rotation();
        let pan_change = event.calculate_pan();
        if !self.past {
            self.zoom *= zoom_change;
            self.rotation += rotation_change;
            self.pan = self.pan + pan_change;
            let centroid_size = event.calculate_centroid_size(false);
            let zoom_motion = (1.0 - self.zoom).abs() * centroid_size;
            let rotation_motion = (self.rotation * PI * centroid_size / 180.0).abs();
            let pan_motion = distance(self.pan);
            if zoom_motion > touch_slop
                || rotation_motion > touch_slop
                || (pan_motion > touch_slop && can_pan(pan_change))
            {
                self.past = true;
                self.locked_to_pan_zoom = pan_zoom_lock && rotation_motion < touch_slop;
            }
        }
        if !self.past {
            return None;
        }
        let centroid = event.calculate_centroid(false);
        let rotation = if self.locked_to_pan_zoom { 0.0 } else { rotation_change };
        let step = (rotation != 0.0
            || zoom_change != 1.0
            || (pan_change != Offset::ZERO && can_pan(pan_change)))
        .then_some(Step { centroid, zoom: zoom_change, pan: pan_change, rotation });
        for change in &event.changes {
            if change.position_changed() {
                change.consume();
            }
        }
        step
    }
}

/// Zooms, pans and rotations, as Compose's `PointerInputScope.detectTransformGestures`.
pub trait TransformGestureDetector {
    /// Reports each step of a gesture past the touch slop to `on_gesture` (its centroid, pan,
    /// zoom and rotation in degrees), its moves consumed; a gesture whose events another handler
    /// consumes ends. With `pan_zoom_lock`, a gesture that starts as a pan or a zoom doesn't
    /// rotate. Never returns.
    fn detect_transform_gestures(
        &self,
        pan_zoom_lock: bool,
        on_gesture: impl Fn(Offset, Offset, f32, f32) + 'static,
    ) -> LocalBoxFuture<'_, ()>;
}

impl TransformGestureDetector for PointerInputScope {
    fn detect_transform_gestures(
        &self,
        pan_zoom_lock: bool,
        on_gesture: impl Fn(Offset, Offset, f32, f32) + 'static,
    ) -> LocalBoxFuture<'_, ()> {
        let on_gesture = Rc::new(on_gesture);
        Box::pin(each_gesture(self, move || {
            let on_gesture = on_gesture.clone();
            async move {
                let mut slop = Slop::new();
                let touch_slop = self.touch_slop();
                self.await_first_down(false).await;
                loop {
                    let event = self.await_pointer_event(PointerEventPass::Main).await;
                    if event.changes.iter().any(|change| change.is_consumed()) {
                        return;
                    }
                    if let Some(step) = slop.step(&event, touch_slop, pan_zoom_lock, &|_| true) {
                        let on_gesture = on_gesture.clone();
                        self.run_after_dispatch(move || {
                            on_gesture(step.centroid, step.pan, step.zoom, step.rotation)
                        });
                    }
                    if !event.changes.iter().any(|change| change.pressed) {
                        return;
                    }
                }
            }
        }))
    }
}

// ---- The state ---------------------------------------------------------------------------------

/// Where a transformation's changes go, as `TransformScope`: valid in the state's `transform`.
#[derive(Clone)]
pub struct TransformScope {
    on_transformation: Rc<dyn Fn(Offset, f32, Offset, f32)>,
}

impl TransformScope {
    /// A scope calling `on_transformation` (centroid, zoom, pan, rotation) with each change, for
    /// a state of your own.
    pub fn new(on_transformation: impl Fn(Offset, f32, Offset, f32) + 'static) -> TransformScope {
        TransformScope { on_transformation: Rc::new(on_transformation) }
    }

    /// Zooms by `zoom_change` (a factor), pans by `pan_change` and rotates by `rotation_change`
    /// degrees, around no point, as `transformBy`.
    pub fn transform_by(&self, zoom_change: f32, pan_change: Offset, rotation_change: f32) {
        self.transform_by_with_centroid(
            Offset::UNSPECIFIED,
            zoom_change,
            pan_change,
            rotation_change,
        )
    }

    /// The same around `centroid` (`Offset::UNSPECIFIED` for none), as
    /// `transformByWithCentroid`.
    pub fn transform_by_with_centroid(
        &self,
        centroid: Offset,
        zoom_change: f32,
        pan_change: Offset,
        rotation_change: f32,
    ) {
        (self.on_transformation)(centroid, zoom_change, pan_change, rotation_change)
    }
}

/// What a state's `transform` runs.
pub type TransformBlock<'a> = Box<dyn FnOnce(TransformScope) -> LocalBoxFuture<'a, ()> + 'a>;

/// Where zooms, pans and rotations go, as Compose's `TransformableState`, an interface for a state
/// of your own. The state `TransformableState(onTransformation)` makes is
/// [`DefaultTransformableState`], from [`transformable_state`]; `transform` and the animations
/// are in [`TransformableStateExt`], on every state.
pub trait TransformableState {
    /// Runs `block` with this state's scope, as a mutation of `priority`: Compose's
    /// `transform`, boxed so the trait is object-safe; call [`TransformableStateExt::transform`].
    fn transform_boxed<'a>(
        &'a self,
        priority: MutatePriority,
        block: TransformBlock<'a>,
    ) -> LocalBoxFuture<'a, Result<(), MutationInterrupted>>;

    /// Whether a transformation (a gesture, an animation) goes on, as `isTransformInProgress` (a
    /// state read).
    fn is_transform_in_progress(&self) -> bool;
}

/// Compose's default animation of a transformation, `SpringSpec(stiffness = StiffnessLow)`.
fn default_spec() -> AnimationSpec {
    spring_with(Spring::DAMPING_RATIO_NO_BOUNCY, Spring::STIFFNESS_LOW).into()
}

/// `transform`, the steps and the animations on every [`TransformableState`], as Compose's
/// extensions. Each is a transformation of `MutatePriority::Default` (`stop_transformation`'s
/// is given) that a gesture interrupts; interrupted, it returns [`MutationInterrupted`].
pub trait TransformableStateExt: TransformableState {
    /// Takes the transformation for `block`, as `transform`: one going on of the same or a lower
    /// priority is interrupted, one of a higher priority makes this one return
    /// [`MutationInterrupted`].
    fn transform<'a, F, Fut>(
        &'a self,
        priority: MutatePriority,
        block: F,
    ) -> LocalBoxFuture<'a, Result<(), MutationInterrupted>>
    where
        F: FnOnce(TransformScope) -> Fut + 'a,
        Fut: Future<Output = ()> + 'a,
    {
        self.transform_boxed(priority, Box::new(move |scope| Box::pin(block(scope))))
    }

    /// Zooms by `zoom_factor` at once, around `centroid` (`Offset::UNSPECIFIED` for none), as
    /// `zoomBy`.
    fn zoom_by(
        &self,
        zoom_factor: f32,
        centroid: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.transform(MutatePriority::Default, move |scope| async move {
            scope.transform_by_with_centroid(centroid, zoom_factor, Offset::ZERO, 0.0);
        })
    }

    /// Rotates by `degrees` at once, as `rotateBy`.
    fn rotate_by(
        &self,
        degrees: f32,
        centroid: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.transform(MutatePriority::Default, move |scope| async move {
            scope.transform_by_with_centroid(centroid, 1.0, Offset::ZERO, degrees);
        })
    }

    /// Pans by `offset` at once, as `panBy`.
    fn pan_by(
        &self,
        offset: Offset,
        centroid: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.transform(MutatePriority::Default, move |scope| async move {
            scope.transform_by_with_centroid(centroid, 1.0, offset, 0.0);
        })
    }

    /// Stops the transformation going on, by taking it with nothing to do, as
    /// `stopTransformation(terminationPriority)`.
    fn stop_transformation(
        &self,
        termination_priority: MutatePriority,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.transform(termination_priority, |_| async {})
    }

    /// Zooms by `zoom_factor` along Compose's default spring (no bounce, low stiffness), as
    /// `animateZoomBy(zoomFactor)`.
    fn animate_zoom_by(
        &self,
        zoom_factor: f32,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.animate_zoom_by_with(zoom_factor, default_spec(), Offset::UNSPECIFIED)
    }

    /// Zooms by `zoom_factor` along `spec`, around `centroid`, as `animateZoomBy`.
    ///
    /// # Panics
    ///
    /// If `zoom_factor` isn't above 0, as Compose's `requirePrecondition`.
    fn animate_zoom_by_with(
        &self,
        zoom_factor: f32,
        spec: impl Into<AnimationSpec>,
        centroid: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        assert!(zoom_factor > 0.0, "zoom value should be greater than 0");
        let spec = spec.into();
        self.transform(MutatePriority::Default, move |scope| async move {
            let previous = Cell::new(1.0f32);
            AnimationState::new(1.0f32)
                .animate_to(zoom_factor, spec, false, |frame| {
                    let value = frame.value();
                    let factor = if previous.get() == 0.0 { 1.0 } else { value / previous.get() };
                    scope.transform_by_with_centroid(centroid, factor, Offset::ZERO, 0.0);
                    previous.set(value);
                })
                .await;
        })
    }

    /// Rotates by `degrees` along Compose's default spring, as `animateRotateBy(degrees)`.
    fn animate_rotate_by(
        &self,
        degrees: f32,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.animate_rotate_by_with(degrees, default_spec(), Offset::UNSPECIFIED)
    }

    /// Rotates by `degrees` along `spec`, around `centroid`, as `animateRotateBy`.
    fn animate_rotate_by_with(
        &self,
        degrees: f32,
        spec: impl Into<AnimationSpec>,
        centroid: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        let spec = spec.into();
        self.transform(MutatePriority::Default, move |scope| async move {
            let previous = Cell::new(0.0f32);
            AnimationState::new(0.0f32)
                .animate_to(degrees, spec, false, |frame| {
                    let value = frame.value();
                    scope.transform_by_with_centroid(
                        centroid,
                        1.0,
                        Offset::ZERO,
                        value - previous.get(),
                    );
                    previous.set(value);
                })
                .await;
        })
    }

    /// Pans by `offset` along Compose's default spring, as `animatePanBy(offset)`.
    fn animate_pan_by(
        &self,
        offset: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.animate_pan_by_with(offset, default_spec(), Offset::UNSPECIFIED)
    }

    /// Pans by `offset` along `spec`, around `centroid`, as `animatePanBy`.
    fn animate_pan_by_with(
        &self,
        offset: Offset,
        spec: impl Into<AnimationSpec>,
        centroid: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        let spec = spec.into();
        self.transform(MutatePriority::Default, move |scope| async move {
            let previous = Cell::new(Offset::ZERO);
            AnimationState::new(Offset::ZERO)
                .animate_to(offset, spec, false, |frame| {
                    let value = frame.value();
                    scope.transform_by_with_centroid(centroid, 1.0, value - previous.get(), 0.0);
                    previous.set(value);
                })
                .await;
        })
    }

    /// Zooms, pans and rotates at once along Compose's default springs, as
    /// `animateBy(zoomFactor, panOffset, rotationDegrees)`.
    fn animate_by(
        &self,
        zoom_factor: f32,
        pan_offset: Offset,
        rotation_degrees: f32,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        let spec = default_spec();
        self.animate_by_with(
            zoom_factor,
            pan_offset,
            rotation_degrees,
            [spec.clone(), spec.clone(), spec],
            Offset::UNSPECIFIED,
        )
    }

    /// The same, each along its spec (`specs`: the zoom's, the pan's and the rotation's), around
    /// `centroid`, as `animateBy`: one animation, as long as the longest.
    ///
    /// # Panics
    ///
    /// If `zoom_factor` isn't above 0.
    fn animate_by_with(
        &self,
        zoom_factor: f32,
        pan_offset: Offset,
        rotation_degrees: f32,
        specs: [AnimationSpec; 3],
        centroid: Offset,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        assert!(zoom_factor > 0.0, "zoom value should be greater than 0");
        let spec = AnimationSpec::custom(Delegating(specs));
        self.transform(MutatePriority::Default, move |scope| async move {
            // Zoom, pan x, pan y, degrees, as `AnimationData`.
            let previous = Cell::new([1.0f32, 0.0, 0.0, 0.0]);
            let target =
                AnimationVector::from([zoom_factor, pan_offset.x, pan_offset.y, rotation_degrees]);
            AnimationState::with_velocity(
                AnimationVector::from(previous.get()),
                AnimationVector::zero(4),
            )
            .animate_to(target, spec, false, |frame| {
                let value = frame.value().to_array();
                let before = previous.get();
                let zoom = if before[0] == 0.0 { 1.0 } else { value[0] / before[0] };
                let pan = Offset::new(value[1] - before[1], value[2] - before[2]);
                scope.transform_by_with_centroid(centroid, zoom, pan, value[3] - before[3]);
                previous.set(value);
            })
            .await;
        })
    }
}

impl<T: TransformableState + ?Sized> TransformableStateExt for T {}

/// The zoom's, the pan's and the rotation's specs as one over their four components, as
/// `DelegatingAnimationSpec`.
struct Delegating([AnimationSpec; 3]);

impl Delegating {
    /// Each spec's part of `vectors` through `f`, packed again.
    fn each(
        &self,
        vectors: [AnimationVector; 3],
        f: impl Fn(&AnimationSpec, [AnimationVector; 3]) -> AnimationVector,
    ) -> AnimationVector {
        let part = |range: std::ops::Range<usize>| {
            vectors.map(|vector| {
                let all = vector.to_array();
                match range.len() {
                    1 => AnimationVector::from([all[range.start]]),
                    _ => AnimationVector::from([all[range.start], all[range.start + 1]]),
                }
            })
        };
        let zoom = f(&self.0[0], part(0..1)).to_array();
        let pan = f(&self.0[1], part(1..3)).to_array();
        let rotation = f(&self.0[2], part(3..4)).to_array();
        AnimationVector::from([zoom[0], pan[0], pan[1], rotation[0]])
    }
}

impl VectorizedAnimationSpec for Delegating {
    fn value_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: AnimationVector,
        target_value: AnimationVector,
        initial_velocity: AnimationVector,
    ) -> AnimationVector {
        self.each([initial_value, target_value, initial_velocity], |spec, [from, to, velocity]| {
            spec.value_from_nanos(play_time_nanos, from, to, velocity)
        })
    }

    fn velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: AnimationVector,
        target_value: AnimationVector,
        initial_velocity: AnimationVector,
    ) -> AnimationVector {
        self.each([initial_value, target_value, initial_velocity], |spec, [from, to, velocity]| {
            spec.velocity_from_nanos(play_time_nanos, from, to, velocity)
        })
    }

    fn duration_nanos(
        &self,
        initial_value: AnimationVector,
        target_value: AnimationVector,
        initial_velocity: AnimationVector,
    ) -> i64 {
        let longest = Cell::new(0i64);
        self.each([initial_value, target_value, initial_velocity], |spec, [from, to, velocity]| {
            longest.set(longest.get().max(spec.duration_nanos(from, to, velocity)));
            from
        });
        longest.get()
    }
}

/// The state [`transformable_state`] makes, as `TransformableState(onTransformation)`. Clones
/// share it.
#[derive(Clone)]
pub struct DefaultTransformableState {
    inner: Rc<DefaultInner>,
}

struct DefaultInner {
    scope: TransformScope,
    mutex: MutatorMutex,
    transforming: OwnedState<bool>,
    /// Bumped by each transformation that runs: an interrupted one, ending later, leaves the flag
    /// to the one that interrupted it.
    generation: Cell<u64>,
}

/// A state calling `on_transformation` (centroid, zoom factor, pan, rotation in degrees) with
/// each change, as `TransformableState(onTransformation)`.
#[track_caller]
pub fn transformable_state(
    on_transformation: impl Fn(Offset, f32, Offset, f32) + 'static,
) -> DefaultTransformableState {
    DefaultTransformableState {
        inner: Rc::new(DefaultInner {
            scope: TransformScope::new(on_transformation),
            mutex: MutatorMutex::new(),
            transforming: compose_runtime::mutable_state_of(false),
            generation: Cell::new(0),
        }),
    }
}

/// Clears `is_transform_in_progress` when a transformation's block ends however it ends, unless
/// a later one took over.
struct TransformingFlag<'a> {
    inner: &'a DefaultInner,
    generation: u64,
}

impl Drop for TransformingFlag<'_> {
    fn drop(&mut self) {
        if self.inner.generation.get() == self.generation {
            let _ = self.inner.transforming.try_set(false);
        }
    }
}

impl TransformableState for DefaultTransformableState {
    fn transform_boxed<'a>(
        &'a self,
        priority: MutatePriority,
        block: TransformBlock<'a>,
    ) -> LocalBoxFuture<'a, Result<(), MutationInterrupted>> {
        let inner = &*self.inner;
        Box::pin(inner.mutex.mutate(priority, async move {
            let generation = inner.generation.get().wrapping_add(1);
            inner.generation.set(generation);
            let _ = inner.transforming.try_set(true);
            let _transforming = TransformingFlag { inner, generation };
            block(inner.scope.clone()).await;
        }))
    }

    fn is_transform_in_progress(&self) -> bool {
        self.inner.transforming.get()
    }
}

impl ComposeArg for DefaultTransformableState {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

/// A transformable state remembered at its call site, as
/// `rememberTransformableState(onTransformation)`: the newest `on_transformation` is called.
#[composable(non_restartable)]
pub fn remember_transformable_state(
    on_transformation: impl Fn(Offset, f32, Offset, f32) + 'static,
) -> DefaultTransformableState {
    let latest = compose_runtime::remember(|| {
        Rc::new(RefCell::new(None::<compose_runtime::Callback<dyn Fn(Offset, f32, Offset, f32)>>))
    });
    *latest.borrow_mut() = Some(on_transformation);
    compose_runtime::remember(move || {
        let latest = latest.clone();
        transformable_state(move |centroid, zoom, pan, rotation| {
            let on_transformation = latest.borrow().clone();
            if let Some(on_transformation) = on_transformation {
                on_transformation(centroid, zoom, pan, rotation);
            }
        })
    })
}

// ---- The modifier ------------------------------------------------------------------------------

/// What the gesture detection sends the task holding the state's transformation, as
/// `TransformEvent`.
enum TransformEvent {
    Started,
    Stopped,
    Delta(Step),
}

/// The node's unlimited channel of transform events.
#[derive(Default)]
struct Queue {
    events: RefCell<VecDeque<TransformEvent>>,
    waker: RefCell<Option<Waker>>,
}

impl Queue {
    fn send(&self, event: TransformEvent) {
        self.events.borrow_mut().push_back(event);
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }

    async fn receive(&self) -> TransformEvent {
        std::future::poll_fn(|context| match self.events.borrow_mut().pop_front() {
            Some(event) => Poll::Ready(event),
            None => {
                *self.waker.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}

/// Sends `Stopped` when a gesture's detection ends however it ends, as the detectors' `finally`.
struct StopOnDrop(Rc<Queue>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.send(TransformEvent::Stopped);
    }
}

/// Every 545 pixels of a Ctrl-wheel scroll zoom twice, as `SCROLL_FACTOR` (fitted to ChromeOS's
/// zooms).
const SCROLL_FACTOR: f32 = 545.0;

/// Zooms, pans and rotations go to `state`, as `Modifier.transformable`: a gesture of one or more
/// fingers past the touch slop (a pan too, if `can_pan` allows it) takes the state's
/// transformation at `UserInput` priority until its fingers lift; Ctrl with the mouse wheel zooms
/// (each 545 pixels of scroll twice, up zooming in). With `lock_rotation_on_zoom_pan`, a gesture
/// that starts as a pan or a zoom doesn't rotate. A new state, lock or `enabled` ends a
/// transformation going on.
pub trait TransformableModifier {
    fn transformable<S: TransformableState + ComposeArg + Clone + 'static>(
        self,
        state: S,
        lock_rotation_on_zoom_pan: bool,
        enabled: bool,
    ) -> Modifier;

    /// The same, a pan starting or continuing a gesture only if `can_pan` allows its change, as
    /// `transformable(state, canPan, …)`.
    fn transformable_with_can_pan<S: TransformableState + ComposeArg + Clone + 'static>(
        self,
        state: S,
        can_pan: impl Fn(Offset) -> bool + 'static,
        lock_rotation_on_zoom_pan: bool,
        enabled: bool,
    ) -> Modifier;
}

/// The `can_pan` of `transformable`: always, compared by its type as Compose's shared `{ true }`.
fn always(_: Offset) -> bool {
    true
}

impl TransformableModifier for Modifier {
    fn transformable<S: TransformableState + ComposeArg + Clone + 'static>(
        self,
        state: S,
        lock_rotation_on_zoom_pan: bool,
        enabled: bool,
    ) -> Modifier {
        thread_local! {
            static ALWAYS: Rc<dyn Fn(Offset) -> bool> = Rc::new(always);
        }
        let can_pan = ALWAYS.with(Rc::clone);
        self.then_custom(TransformableElement {
            state,
            can_pan,
            lock: lock_rotation_on_zoom_pan,
            enabled,
        })
    }

    fn transformable_with_can_pan<S: TransformableState + ComposeArg + Clone + 'static>(
        self,
        state: S,
        can_pan: impl Fn(Offset) -> bool + 'static,
        lock_rotation_on_zoom_pan: bool,
        enabled: bool,
    ) -> Modifier {
        let can_pan: Rc<dyn Fn(Offset) -> bool> = Rc::new(can_pan);
        self.then_custom(TransformableElement {
            state,
            can_pan,
            lock: lock_rotation_on_zoom_pan,
            enabled,
        })
    }
}

struct TransformableElement<S> {
    state: S,
    can_pan: Rc<dyn Fn(Offset) -> bool>,
    lock: bool,
    enabled: bool,
}

/// What the node's gestures read: its newest inputs.
struct Inputs {
    /// The state as given, to compare with the next one.
    state_as_given: Box<dyn std::any::Any>,
    state: Rc<dyn TransformableState>,
    can_pan: Rc<dyn Fn(Offset) -> bool>,
    lock: bool,
    enabled: bool,
}

impl<S: TransformableState + ComposeArg + Clone + 'static> TransformableElement<S> {
    fn inputs(&self) -> Inputs {
        Inputs {
            state_as_given: Box::new(self.state.clone()),
            state: Rc::new(self.state.clone()),
            can_pan: self.can_pan.clone(),
            lock: self.lock,
            enabled: self.enabled,
        }
    }
}

impl<S: TransformableState + ComposeArg + Clone + 'static> ModifierNodeElement
    for TransformableElement<S>
{
    type Node = TransformableNode;
    fn create(&self) -> TransformableNode {
        let inputs = Rc::new(RefCell::new(self.inputs()));
        let queue = Rc::new(Queue::default());
        TransformableNode {
            touch: touch_driver(inputs.clone(), queue.clone()),
            inputs,
            queue,
            mouse: None,
            consumer: None,
        }
    }
    fn update(&self, node: &mut TransformableNode) {
        let reset = {
            let inputs = node.inputs.borrow();
            !inputs.state_as_given.downcast_ref::<S>().is_some_and(|old| old.same(&self.state))
                || inputs.enabled != self.enabled
                || inputs.lock != self.lock
        };
        *node.inputs.borrow_mut() = self.inputs();
        if reset {
            node.reset();
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.state.same(&other.state)
            && Rc::ptr_eq(&self.can_pan, &other.can_pan)
            && self.lock == other.lock
            && self.enabled == other.enabled
    }
    fn name(&self) -> &'static str {
        "transformable"
    }
    fn inspectable_properties(&self, info: &mut compose_ui::InspectorInfo) {
        info.name = Some("transformable");
        for (name, value) in [
            ("state", "TransformableState".to_string()),
            ("canPan", "Function".to_string()),
            ("enabled", self.enabled.to_string()),
            ("lockRotationOnZoomPan", self.lock.to_string()),
        ] {
            info.properties.push((name, value));
        }
    }
}

/// The detection of touch gestures, as `TransformableNode`'s pointer input: nothing while
/// disabled.
fn touch_driver(inputs: Rc<RefCell<Inputs>>, queue: Rc<Queue>) -> PointerInputDriver {
    PointerInputDriver::new(move |scope| {
        let (inputs, queue) = (inputs.clone(), queue.clone());
        async move {
            if !inputs.borrow().enabled {
                return;
            }
            each_gesture(&scope, || detect_zoom(&scope, &inputs, &queue)).await
        }
    })
}

/// One gesture, as `detectZoom`: its events go to `queue`, ended by `Stopped` however it ends.
async fn detect_zoom(scope: &PointerInputScope, inputs: &RefCell<Inputs>, queue: &Rc<Queue>) {
    let _stopped = StopOnDrop(queue.clone());
    let lock = inputs.borrow().lock;
    let touch_slop = scope.touch_slop();
    let mut slop = Slop::new();
    scope.await_first_down(false).await;
    loop {
        let event = scope.await_pointer_event(PointerEventPass::Main).await;
        let cancelled = event.changes.iter().any(|change| change.is_consumed());
        if cancelled {
            queue.send(TransformEvent::Stopped);
        } else {
            let can_pan = inputs.borrow().can_pan.clone();
            let was_past = slop.past;
            let step = slop.step(&event, touch_slop, lock, &*can_pan);
            if slop.past && !was_past {
                queue.send(TransformEvent::Started);
            }
            if let Some(step) = step {
                queue.send(TransformEvent::Delta(step));
            }
        }
        let after = scope.await_pointer_event(PointerEventPass::Final).await;
        // Another handler took the gesture while it waited for the slop.
        let cancelled_after = after.changes.iter().any(|change| change.is_consumed()) && !slop.past;
        if cancelled || cancelled_after || !event.changes.iter().any(|change| change.pressed) {
            return;
        }
    }
}

/// The detection of Ctrl-wheel zooms, as `detectNonTouchGestures` (a trackpad's pans and scales
/// aren't events here).
fn mouse_driver(queue: Rc<Queue>) -> PointerInputDriver {
    PointerInputDriver::new(move |scope| {
        let queue = queue.clone();
        async move {
            loop {
                let _stopped = StopOnDrop(queue.clone());
                let (mut event, mut scroll) = loop {
                    let event = scope.await_pointer_event(PointerEventPass::Main).await;
                    if let Some(scroll) = ctrl_scroll(&scope, &event) {
                        break (event, scroll);
                    }
                };
                queue.send(TransformEvent::Started);
                loop {
                    let centroid = centroid_of(&event, true, |_| true);
                    let zoom = 2f32.powf(scroll.y / SCROLL_FACTOR);
                    queue.send(TransformEvent::Delta(Step {
                        centroid,
                        zoom,
                        pan: Offset::ZERO,
                        rotation: 0.0,
                    }));
                    event = scope.await_pointer_event(PointerEventPass::Main).await;
                    match ctrl_scroll(&scope, &event) {
                        Some(next) => scroll = next,
                        None => break,
                    }
                }
            }
        }
    })
}

/// A wheel scroll with Ctrl held, consumed, as `consumePointerEventAsCtrlScrollOrNull`: its
/// pixels as Compose's `calculateMouseWheelScroll` says them, whose sign is the opposite of a
/// scroll's here (a turn down scrolls forward there through the scrollable's reversal).
fn ctrl_scroll(scope: &PointerInputScope, event: &PointerEvents) -> Option<Offset> {
    if !event.keyboard_modifiers.is_ctrl_pressed() || event.kind != PointerEventType::Scroll {
        return None;
    }
    let density = scope.density().density;
    let pixels = event
        .changes
        .iter()
        .fold(Offset::ZERO, |sum, change| sum - change.wheel_pixels(density, scope.size()));
    if pixels == Offset::ZERO {
        return None;
    }
    event.changes.iter().for_each(PointerInputChange::consume);
    Some(pixels)
}

/// `Modifier.transformable`'s node, as `TransformableNode`.
pub(crate) struct TransformableNode {
    inputs: Rc<RefCell<Inputs>>,
    queue: Rc<Queue>,
    touch: PointerInputDriver,
    /// Made at the first mouse event, as Compose's `pointerInputModifierMouse`.
    mouse: Option<PointerInputDriver>,
    /// The task taking the gestures' events to the state, as the launched loop of Compose's
    /// pointer input.
    consumer: Option<Job>,
}

impl TransformableNode {
    /// A new state, lock or `enabled`, as `resetPointerInputHandler`: the gesture and the
    /// transformation going on end.
    fn reset(&mut self) {
        self.touch.stop();
        if let Some(job) = self.consumer.take() {
            job.cancel();
        }
        self.queue.events.borrow_mut().clear();
    }

    fn start_consumer(&mut self, context: &mut NodeContext<'_>) {
        if self.consumer.is_some() {
            return;
        }
        let (inputs, queue) = (self.inputs.clone(), self.queue.clone());
        self.consumer = Some(context.coroutine_scope().launch(async move {
            loop {
                let TransformEvent::Started = queue.receive().await else { continue };
                let state = inputs.borrow().state.clone();
                let queue = &queue;
                // Interrupted, the gesture's later events are dropped until the next one starts.
                let _ = state
                    .transform(MutatePriority::UserInput, |scope| async move {
                        loop {
                            match queue.receive().await {
                                TransformEvent::Stopped => return,
                                TransformEvent::Delta(step) => scope.transform_by_with_centroid(
                                    step.centroid,
                                    step.zoom,
                                    step.pan,
                                    step.rotation,
                                ),
                                TransformEvent::Started => {}
                            }
                        }
                    })
                    .await;
            }
        }));
    }
}

impl ModifierNode for TransformableNode {
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.reset();
        if let Some(mouse) = &mut self.mouse {
            mouse.stop();
        }
    }
    fn on_reset(&mut self, _context: &mut NodeContext<'_>) {
        self.reset();
    }
}

impl PointerInputModifierNode for TransformableNode {
    fn takes_whole_events(&self) -> bool {
        true
    }

    fn hears_pass(&self, pass: PointerEventPass) -> bool {
        self.touch.hears_pass(pass)
            || self.mouse.as_ref().is_some_and(|mouse| mouse.hears_pass(pass))
    }

    fn on_pointer_events(
        &mut self,
        event: &PointerEvents,
        pass: PointerEventPass,
        bounds: IntSize,
        context: &mut EventContext<'_>,
    ) {
        let enabled = self.inputs.borrow().enabled;
        if enabled {
            self.start_consumer(context);
            if self.mouse.is_none()
                && event.changes.iter().any(|change| change.pointer_type == PointerType::Mouse)
            {
                self.mouse = Some(mouse_driver(self.queue.clone()));
            }
        }
        self.touch.on_pointer_events(event, pass, bounds, context);
        if let Some(mouse) = &mut self.mouse {
            mouse.on_pointer_events(event, pass, bounds, context);
        }
        // The state hears the events right after the dispatch, not at the executor's next turn.
        if let Some(job) = self.consumer
            && !self.queue.events.borrow().is_empty()
        {
            context.run(move || {
                compose_runtime::platform::poll_now(job);
            });
        }
    }

    fn on_cancel_pointer_input(&mut self) {
        self.touch.stop();
        if let Some(mouse) = &mut self.mouse {
            mouse.stop();
        }
    }
}
