//! Scrolling on both axes, as Compose's `Scrollable2DState.kt` and `Scrollable2D.kt`: the state
//! a 2D scroll's deltas go to, and `Modifier::scrollable_2d`, whose drags (`draggable_2d`'s
//! detection), wheel turns and flings scroll it through the nested-scroll connections around and
//! an overscroll effect. Every offset and velocity here is the pointer's way, as Compose's: no
//! direction is reversed.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;

use compose_animation::{AnimationSpec, animate_value};
use compose_runtime::{ComposeArg, CoroutineScope, Job, OwnedState, composable};
use compose_ui::{
    EventContext, IntSize, Modifier, ModifierNode, ModifierNodeElement, MutableInteractionSource,
    NestedScrollConnection, NestedScrollSource, NodeContext, Offset, PointerEvent,
    PointerEventPass, PointerEventType, PointerInputModifierNode, PointerType, Velocity,
    VelocityTracker1D, platform::NestedScrollChain,
};

use super::drag_gesture::{Deltas, DragBehavior, DragGestureNode};
use super::scrollable::{DefaultFlingBehavior, FlingBehavior, LocalBoxFuture, ScrollScope};
use crate::mutator::{MutatePriority, MutationInterrupted, MutatorMutex};
use crate::overscroll::{OverscrollEffect, PerformFling, same_overscroll};
use crate::scroll::{event_time, same_fling};

// ---- The state ---------------------------------------------------------------------------------

/// What a 2D scroll's block scrolls with, as `Scroll2DScope`: `scroll_by` consumes a delta and
/// returns the part it used. A clone scrolls the same.
#[derive(Clone)]
pub struct Scroll2DScope {
    scroll_by: Rc<dyn Fn(Offset) -> Offset>,
}

impl Scroll2DScope {
    /// A scope whose `scroll_by` calls `scroll_by`, for a state of your own.
    pub fn new(scroll_by: impl Fn(Offset) -> Offset + 'static) -> Scroll2DScope {
        Scroll2DScope { scroll_by: Rc::new(scroll_by) }
    }

    /// Scrolls by `delta`; returns what was used, as `scrollBy`.
    pub fn scroll_by(&self, delta: Offset) -> Offset {
        (self.scroll_by)(delta)
    }
}

/// A 2D scroll's block, boxed: what [`Scrollable2DState::scroll_boxed`] runs with the scope.
pub type Scroll2DBlock<'a> = Box<dyn FnOnce(Scroll2DScope) -> LocalBoxFuture<'a, ()> + 'a>;

/// What scrolls on both axes, as Compose's `Scrollable2DState`: one block at a time by priority,
/// as [`ScrollableState`](super::ScrollableState); the conveniences are in
/// [`Scrollable2DStateExt`], on every state.
pub trait Scrollable2DState {
    /// Runs `block` with this state's scope, as a mutation of `priority`: Compose's `scroll`,
    /// boxed; call [`Scrollable2DStateExt::scroll`].
    fn scroll_boxed<'a>(
        &'a self,
        priority: MutatePriority,
        block: Scroll2DBlock<'a>,
    ) -> LocalBoxFuture<'a, Result<(), MutationInterrupted>>;

    /// Scrolls by `delta` at once, whatever scroll goes on, as `dispatchRawDelta`; returns what
    /// was used.
    fn dispatch_raw_delta(&self, delta: Offset) -> Offset;

    /// Whether a scroll (a block, a drag, a fling) goes on, as `isScrollInProgress` (a state read).
    fn is_scroll_in_progress(&self) -> bool;

    /// Whether it can scroll by `offset`, as `canScroll`: what an overscroll effect and the
    /// wheel ask.
    fn can_scroll(&self, offset: Offset) -> bool;
}

/// `scroll` and the `Scrollable2DState` extensions of `ScrollExtensions.kt`, on every state.
pub trait Scrollable2DStateExt: Scrollable2DState {
    /// Takes the state's scroll for `block`, as `scroll`.
    fn scroll<'a, F, Fut>(
        &'a self,
        priority: MutatePriority,
        block: F,
    ) -> LocalBoxFuture<'a, Result<(), MutationInterrupted>>
    where
        F: FnOnce(Scroll2DScope) -> Fut + 'a,
        Fut: Future<Output = ()> + 'a,
    {
        self.scroll_boxed(priority, Box::new(move |scope| Box::pin(block(scope))))
    }

    /// Scrolls by `value` at once in a scroll of the default priority, as `scrollBy`; returns
    /// what was used.
    fn scroll_by(&self, value: Offset) -> LocalBoxFuture<'_, Result<Offset, MutationInterrupted>> {
        Box::pin(async move {
            let used = Rc::new(Cell::new(Offset::ZERO));
            let into = used.clone();
            self.scroll(MutatePriority::Default, move |scope| async move {
                into.set(scope.scroll_by(value));
            })
            .await?;
            Ok(used.get())
        })
    }

    /// Scrolls by `value` along `spec`, a step a frame, as `animateScrollBy`; returns what was
    /// used.
    fn animate_scroll_by(
        &self,
        value: Offset,
        spec: impl Into<AnimationSpec>,
    ) -> LocalBoxFuture<'_, Result<Offset, MutationInterrupted>> {
        let spec = spec.into();
        Box::pin(async move {
            let used = Rc::new(Cell::new(Offset::ZERO));
            let into = used.clone();
            self.scroll(MutatePriority::Default, move |scope| async move {
                animate_value(Offset::ZERO, value, None, spec, |current, _| {
                    into.set(into.get() + scope.scroll_by(current - into.get()));
                })
                .await;
            })
            .await?;
            Ok(used.get())
        })
    }

    /// Stops the scroll going on (of `priority` or lower), as `stopScroll`.
    fn stop_scroll(
        &self,
        priority: MutatePriority,
    ) -> LocalBoxFuture<'_, Result<(), MutationInterrupted>> {
        self.scroll(priority, |_| async {})
    }
}

impl<T: Scrollable2DState + ?Sized> Scrollable2DStateExt for T {}

/// The state [`scrollable_2d_state`] makes, as `Scrollable2DState(consumeScrollDelta)`. Clones
/// share it.
#[derive(Clone)]
pub struct DefaultScrollable2DState {
    inner: Rc<DefaultInner>,
}

struct DefaultInner {
    on_delta: Rc<dyn Fn(Offset) -> Offset>,
    mutex: MutatorMutex,
    scrolling: OwnedState<bool>,
    /// Bumped by each scroll that runs: an interrupted one, ending later, leaves the flag to the
    /// one that interrupted it.
    generation: Cell<u64>,
}

/// A state whose deltas go to `consume_scroll_delta` (it returns what it used), as
/// `Scrollable2DState(consumeScrollDelta)`.
#[track_caller]
pub fn scrollable_2d_state(
    consume_scroll_delta: impl Fn(Offset) -> Offset + 'static,
) -> DefaultScrollable2DState {
    DefaultScrollable2DState {
        inner: Rc::new(DefaultInner {
            on_delta: Rc::new(consume_scroll_delta),
            mutex: MutatorMutex::new(),
            scrolling: compose_runtime::mutable_state_of(false),
            generation: Cell::new(0),
        }),
    }
}

/// Clears `is_scroll_in_progress` when a scroll's block ends however it ends, unless a later one
/// took over.
struct ScrollingFlag<'a>(&'a DefaultInner, u64);

impl Drop for ScrollingFlag<'_> {
    fn drop(&mut self) {
        if self.0.generation.get() == self.1 {
            let _ = self.0.scrolling.try_set(false);
        }
    }
}

impl Scrollable2DState for DefaultScrollable2DState {
    fn scroll_boxed<'a>(
        &'a self,
        priority: MutatePriority,
        block: Scroll2DBlock<'a>,
    ) -> LocalBoxFuture<'a, Result<(), MutationInterrupted>> {
        let inner = &*self.inner;
        let on_delta = inner.on_delta.clone();
        // A NaN scrolls nothing, as the default scope's `scrollBy`.
        let scope = Scroll2DScope::new(move |delta| {
            if delta.x.is_nan() || delta.y.is_nan() { Offset::ZERO } else { on_delta(delta) }
        });
        Box::pin(inner.mutex.mutate(priority, async move {
            let generation = inner.generation.get().wrapping_add(1);
            inner.generation.set(generation);
            let _ = inner.scrolling.try_set(true);
            let _scrolling = ScrollingFlag(inner, generation);
            block(scope).await;
        }))
    }

    fn dispatch_raw_delta(&self, delta: Offset) -> Offset {
        (self.inner.on_delta)(delta)
    }

    fn is_scroll_in_progress(&self) -> bool {
        self.inner.scrolling.get()
    }

    /// Always, as Compose's default state.
    fn can_scroll(&self, _: Offset) -> bool {
        true
    }
}

impl ComposeArg for DefaultScrollable2DState {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

/// A 2D scrollable state remembered at its call site, as `rememberScrollable2DState`: the newest
/// `consume_scroll_delta` is called.
#[composable(non_restartable)]
pub fn remember_scrollable_2d_state(
    consume_scroll_delta: impl Fn(Offset) -> Offset + 'static,
) -> DefaultScrollable2DState {
    let latest = compose_runtime::remember(|| {
        Rc::new(RefCell::new(None::<compose_runtime::Callback<dyn Fn(Offset) -> Offset>>))
    });
    *latest.borrow_mut() = Some(consume_scroll_delta);
    compose_runtime::remember(move || {
        let latest = latest.clone();
        scrollable_2d_state(move |delta| {
            let consume = latest.borrow().clone();
            consume.map_or(Offset::ZERO, |consume| consume(delta))
        })
    })
}

// ---- The scrolling logic -----------------------------------------------------------------------

fn velocity_offset(velocity: Velocity) -> Offset {
    Offset::new(velocity.x, velocity.y)
}

fn offset_velocity(offset: Offset) -> Velocity {
    Velocity::new(offset.x, offset.y)
}

/// What a 2D scrollable's drags, wheel and flings scroll, owned for tasks to outlive an event, as
/// `ScrollingLogic2D`.
#[derive(Clone)]
struct Logic {
    state: Rc<dyn Scrollable2DState>,
    overscroll: Option<Rc<dyn OverscrollEffect>>,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
    /// The connections around, as the last press or wheel turn found them.
    chain: NestedScrollChain,
    /// Pixels per dp, for the default fling.
    density: f32,
    /// A fling of this node goes on, as `isFlinging`.
    flinging: Rc<Cell<bool>>,
    /// The node left, as `isScrollableNodeAttached` false: its flings end at their next delta.
    detached: Rc<Cell<bool>>,
}

/// Clears `flinging` however a fling ends.
struct Flinging(Rc<Cell<bool>>);

impl Drop for Flinging {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl Logic {
    /// `delta` through the connections around and `scope`, as `performScroll`; returns what all
    /// of them used.
    fn perform(&self, scope: &Scroll2DScope, delta: Offset, source: NestedScrollSource) -> Offset {
        let before = self.chain.pre_scroll(delta, source);
        let left = delta - before;
        let used = scope.scroll_by(left);
        let after = self.chain.post_scroll(used, left - used, source);
        before + used + after
    }

    /// `delta` around the overscroll effect, if the state can scroll by it, as
    /// `scrollByWithOverscroll`.
    fn scroll_with_overscroll(
        &self,
        scope: &Scroll2DScope,
        delta: Offset,
        source: NestedScrollSource,
    ) -> Offset {
        match &self.overscroll {
            Some(effect) if self.state.can_scroll(delta) => {
                effect
                    .apply_to_scroll(delta, source, &mut |delta| self.perform(scope, delta, source))
            }
            _ => self.perform(scope, delta, source),
        }
    }

    /// What a child left scrolls this state, unless a scroll of its own goes on, as
    /// `performRawScroll`.
    fn raw_scroll(&self, delta: Offset) -> Offset {
        if self.state.is_scroll_in_progress() {
            Offset::ZERO
        } else {
            self.state.dispatch_raw_delta(delta)
        }
    }

    /// Flings at `available` in a scroll of its own, as `doFlingAnimation`: the fling behavior
    /// along the velocity's magnitude, each step split along its angle. Returns the velocity
    /// left, or `Err` if another scroll interrupted it.
    async fn do_fling(&self, available: Velocity) -> Result<Velocity, MutationInterrupted> {
        let behavior: Rc<dyn FlingBehavior> = match &self.fling_behavior {
            Some(behavior) => behavior.clone(),
            None => Rc::new(DefaultFlingBehavior::new(self.density)),
        };
        let angle = available.y.atan2(available.x);
        let split = move |magnitude: f32| {
            if angle.is_nan() {
                Offset::new(0.0, magnitude)
            } else {
                Offset::new(
                    (angle.cos() * magnitude).abs() * sign(available.x),
                    (angle.sin() * magnitude).abs() * sign(available.y),
                )
            }
        };
        self.flinging.set(true);
        let _flinging = Flinging(self.flinging.clone());
        let left = Rc::new(Cell::new(available));
        let into = left.clone();
        let logic = self.clone();
        self.state
            .scroll(MutatePriority::Default, move |scope| async move {
                let cancelled = Rc::new(Cell::new(false));
                let saw = cancelled.clone();
                let flung = logic.clone();
                let fling_scope = ScrollScope::of_fling(
                    move |pixels: f32| {
                        let delta = split(pixels);
                        if delta != Offset::ZERO && flung.detached.get() {
                            cancelled.set(true);
                            return 0.0;
                        }
                        let used = flung.scroll_with_overscroll(
                            &scope,
                            delta,
                            NestedScrollSource::SideEffect,
                        );
                        used.x.hypot(used.y)
                    },
                    saw,
                );
                let magnitude = available.x.hypot(available.y);
                let velocity_left = behavior.perform_fling(fling_scope, magnitude).await;
                let left = split(velocity_left);
                into.set(Velocity::new(left.x, left.y));
            })
            .await?;
        Ok(left.get())
    }

    /// The fling with the connections around, as `onScrollStopped`'s `performFling`: `None` if
    /// it was interrupted.
    async fn perform_fling(&self, velocity: Velocity) -> Option<Velocity> {
        let pre = offset_velocity(self.chain.pre_fling(velocity_offset(velocity)).await);
        let available = velocity - pre;
        let left = self.do_fling(available).await.ok()?;
        let post =
            self.chain.post_fling(velocity_offset(available - left), velocity_offset(left)).await;
        Some(velocity - (left - offset_velocity(post)))
    }

    /// A scroll's end at `velocity`, as `onScrollStopped`: around the overscroll effect, if the
    /// state can scroll that way. A wheel's flings only with a custom behavior that asks to.
    async fn on_scroll_stopped(self, velocity: Velocity, wheel: bool) {
        if wheel
            && self
                .fling_behavior
                .as_ref()
                .is_none_or(|behavior| !behavior.should_be_triggered_by_mouse_wheel())
        {
            return;
        }
        let effect =
            self.overscroll.clone().filter(|_| self.state.can_scroll(velocity_offset(velocity)));
        let Some(effect) = effect else {
            let _ = self.perform_fling(velocity).await;
            return;
        };
        let logic = self.clone();
        let perform: PerformFling = Rc::new(move |velocity: Velocity| {
            let logic = logic.clone();
            Box::pin(async move {
                match logic.perform_fling(velocity).await {
                    Some(consumed) => consumed,
                    None => {
                        // As Compose's cancellation through `applyToFling`.
                        compose_runtime::platform::cancel_current_task();
                        std::future::pending().await
                    }
                }
            })
        });
        effect.apply_to_fling(velocity, perform).await;
    }
}

fn sign(value: f32) -> f32 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// A 2D scrollable as the nested-scroll parent of the scrollables inside it, as
/// `ScrollableNestedScrollConnection`.
struct Connection {
    logic: Logic,
    enabled: bool,
}

impl NestedScrollConnection for Connection {
    fn on_post_scroll(&self, _: Offset, available: Offset, _: NestedScrollSource) -> Offset {
        if self.enabled { self.logic.raw_scroll(available) } else { Offset::ZERO }
    }

    fn on_post_fling(&self, _: Offset, available: Offset) -> LocalBoxFuture<'_, Offset> {
        Box::pin(async move {
            if !self.enabled {
                return Offset::ZERO;
            }
            let velocity = offset_velocity(available);
            let left = if self.logic.flinging.get() {
                Velocity::ZERO
            } else {
                match self.logic.do_fling(velocity).await {
                    Ok(left) => left,
                    Err(MutationInterrupted) => {
                        compose_runtime::platform::cancel_current_task();
                        std::future::pending().await
                    }
                }
            };
            velocity_offset(velocity - left)
        })
    }
}

// ---- The wheel ---------------------------------------------------------------------------------

/// How long a wheel gesture holds the scroll for another turn, as `ScrollProgressTimeout`.
const WHEEL_TIMEOUT_MILLIS: u64 = 50;

/// The wheel of a 2D scrollable: each turn scrolls at once, the scroll held while turns come, a
/// custom fling behavior told when they stop (Compose's wheel animates a large turn; this one
/// applies it at once, as a precise wheel's).
#[derive(Default)]
struct Wheel {
    turns: RefCell<std::collections::VecDeque<(Offset, i64)>>,
    waker: RefCell<Option<std::task::Waker>>,
    velocity: RefCell<Option<[VelocityTracker1D; 2]>>,
    scrolling: Cell<bool>,
}

impl Wheel {
    fn send(&self, delta: Offset, time_millis: i64) {
        self.turns.borrow_mut().push_back((delta, time_millis));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }

    fn track(&self, delta: Offset, time_millis: i64) {
        let mut velocity = self.velocity.borrow_mut();
        let trackers = velocity
            .get_or_insert_with(|| [VelocityTracker1D::new(true), VelocityTracker1D::new(true)]);
        trackers[0].add_data_point(time_millis, delta.x);
        trackers[1].add_data_point(time_millis, delta.y);
    }

    async fn receive(&self) -> (Offset, i64) {
        std::future::poll_fn(|context| match self.turns.borrow_mut().pop_front() {
            Some(turn) => std::task::Poll::Ready(turn),
            None => {
                *self.waker.borrow_mut() = Some(context.waker().clone());
                std::task::Poll::Pending
            }
        })
        .await
    }

    /// The task taking the turns: a gesture holds the state's scroll until none came for the
    /// timeout, then the fling behavior hears its velocity.
    async fn run(self: Rc<Self>, logic: Rc<RefCell<Logic>>) {
        loop {
            let first = self.receive().await;
            let current = logic.borrow().clone();
            self.scrolling.set(true);
            let wheel = self.clone();
            let scrolled = current
                .state
                .clone()
                .scroll(MutatePriority::UserInput, |scope| async move {
                    let mut turn = Some(first);
                    while let Some((delta, time)) = turn.take() {
                        wheel.track(delta, time);
                        current.scroll_with_overscroll(
                            &scope,
                            delta,
                            NestedScrollSource::UserInput,
                        );
                        let deadline = compose_runtime::platform::now_nanos()
                            + WHEEL_TIMEOUT_MILLIS * 1_000_000;
                        while turn.is_none() && compose_runtime::platform::now_nanos() < deadline {
                            turn = wheel.turns.borrow_mut().pop_front();
                            if turn.is_none() {
                                compose_runtime::with_frame_nanos(|_| ()).await;
                            }
                        }
                    }
                })
                .await;
            self.scrolling.set(false);
            if scrolled.is_ok() {
                let velocity =
                    self.velocity.borrow_mut().as_mut().map_or(Velocity::ZERO, |trackers| {
                        Velocity::new(
                            trackers[0].calculate_velocity_with(f32::MAX),
                            trackers[1].calculate_velocity_with(f32::MAX),
                        )
                    });
                let current = logic.borrow().clone();
                current.on_scroll_stopped(velocity, true).await;
            }
        }
    }
}

// ---- The modifier ------------------------------------------------------------------------------

/// `Modifier.scrollable2D`, on `Modifier`.
pub trait Scrollable2DModifier {
    /// Drags any way, the wheel and flings scroll `state`, as `Modifier.scrollable2D(state,
    /// enabled, overscrollEffect, flingBehavior, interactionSource)`: the deltas are the
    /// pointer's (a drag down and right gives positive ones), through the nested-scroll
    /// connections around; a mouse doesn't drag it. `fling_behavior` `None` is the default
    /// spline; the overscroll effect isn't drawn here (apply its `overscroll` too).
    fn scrollable_2d<S: Scrollable2DState + ComposeArg + Clone + 'static>(
        self,
        state: S,
        enabled: bool,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        fling_behavior: Option<Rc<dyn FlingBehavior>>,
        interaction_source: Option<MutableInteractionSource>,
    ) -> Modifier;
}

impl Scrollable2DModifier for Modifier {
    fn scrollable_2d<S: Scrollable2DState + ComposeArg + Clone + 'static>(
        self,
        state: S,
        enabled: bool,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        fling_behavior: Option<Rc<dyn FlingBehavior>>,
        interaction_source: Option<MutableInteractionSource>,
    ) -> Modifier {
        let dyn_state: Rc<dyn Scrollable2DState> = Rc::new(state.clone());
        self.then_custom(Scrollable2DElement {
            state,
            dyn_state,
            enabled,
            overscroll: overscroll_effect,
            fling_behavior,
            interaction_source,
        })
    }
}

struct Scrollable2DElement<S> {
    state: S,
    dyn_state: Rc<dyn Scrollable2DState>,
    enabled: bool,
    overscroll: Option<Rc<dyn OverscrollEffect>>,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
    interaction_source: Option<MutableInteractionSource>,
}

/// What the drag reads: the logic as of the last event, and the state as given.
struct Behavior {
    logic: Rc<RefCell<Logic>>,
}

impl DragBehavior for Behavior {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn on_pointer_context(&self, context: &EventContext<'_>) {
        self.logic.borrow_mut().density = context.density().density;
    }

    fn drag(
        self: Rc<Self>,
        deltas: Deltas,
    ) -> LocalBoxFuture<'static, Result<(), MutationInterrupted>> {
        let logic = self.logic.borrow().clone();
        Box::pin(async move {
            let state = logic.state.clone();
            state
                .scroll(MutatePriority::UserInput, |scope| async move {
                    while let Some(delta) = deltas.next().await {
                        logic.scroll_with_overscroll(&scope, delta, NestedScrollSource::UserInput);
                    }
                })
                .await
        })
    }

    fn on_drag_started(&self, _: CoroutineScope, _: Offset) {}

    fn on_drag_stopped(&self, scope: CoroutineScope, velocity: Velocity) {
        let logic = self.logic.borrow().clone();
        if logic.detached.get() {
            return;
        }
        scope.launch(logic.on_scroll_stopped(velocity, false));
    }

    /// A press while it scrolls (a fling, an animation) or its effect shows drags at once, as
    /// `shouldScrollImmediately`.
    fn start_drag_immediately(&self) -> bool {
        let logic = self.logic.borrow();
        logic.state.is_scroll_in_progress()
            || logic.overscroll.as_ref().is_some_and(|effect| effect.is_in_progress())
    }

    /// A mouse doesn't drag it, as `CanDragCalculation`.
    fn can_drag(&self, pointer_type: PointerType) -> bool {
        pointer_type != PointerType::Mouse
    }
}

/// `Modifier::scrollable_2d`'s node, as `Scrollable2DNode`: a 2D drag's detection, the
/// nested-scroll parent of what's inside, and the wheel.
pub(crate) struct Scrollable2DNode {
    /// The state as given, to compare with the next one.
    state_as_given: Box<dyn std::any::Any>,
    logic: Rc<RefCell<Logic>>,
    enabled: bool,
    drag: DragGestureNode,
    wheel: Option<(Rc<Wheel>, Job)>,
}

impl<S: Scrollable2DState + ComposeArg + Clone + 'static> ModifierNodeElement
    for Scrollable2DElement<S>
{
    type Node = Scrollable2DNode;
    fn create(&self) -> Scrollable2DNode {
        let logic = Rc::new(RefCell::new(Logic {
            state: self.dyn_state.clone(),
            overscroll: self.overscroll.clone(),
            fling_behavior: self.fling_behavior.clone(),
            chain: NestedScrollChain::default(),
            density: 1.0,
            flinging: Rc::default(),
            detached: Rc::default(),
        }));
        let behavior = Rc::new(Behavior { logic: logic.clone() });
        Scrollable2DNode {
            state_as_given: Box::new(self.state.clone()),
            drag: DragGestureNode::new(
                behavior,
                self.enabled,
                None,
                self.interaction_source.clone(),
            ),
            logic,
            enabled: self.enabled,
            wheel: None,
        }
    }
    fn update(&self, node: &mut Scrollable2DNode) {
        let reset =
            !node.state_as_given.downcast_ref::<S>().is_some_and(|old| old.same(&self.state));
        node.state_as_given = Box::new(self.state.clone());
        {
            let mut logic = node.logic.borrow_mut();
            logic.state = self.dyn_state.clone();
            logic.overscroll = self.overscroll.clone();
            logic.fling_behavior = self.fling_behavior.clone();
        }
        node.enabled = self.enabled;
        node.drag.update(self.enabled, None, self.interaction_source.clone(), reset);
    }
    fn same(&self, other: &Self) -> bool {
        self.state.same(&other.state)
            && self.enabled == other.enabled
            && same_overscroll(&self.overscroll, &other.overscroll)
            && same_fling(&self.fling_behavior, &other.fling_behavior)
            && self.interaction_source == other.interaction_source
    }
    fn name(&self) -> &'static str {
        "scrollable2D"
    }
}

impl Scrollable2DNode {
    fn detach(&mut self) {
        let mut logic = self.logic.borrow_mut();
        logic.detached.set(true);
        logic.detached = Rc::default();
        drop(logic);
        self.drag.detach();
        if let Some((_, job)) = self.wheel.take() {
            job.cancel();
        }
    }

    /// A wheel turn: taken on the way in while a wheel gesture goes on (this scrollable first),
    /// else on the way out if nothing inside took it, as `NonTouchScrollingLogic`.
    fn on_wheel(
        &mut self,
        event: &mut PointerEvent,
        pass: PointerEventPass,
        bounds: IntSize,
        context: &mut NodeContext<'_>,
    ) {
        if event.consumed {
            return;
        }
        let logic = self.logic.clone();
        let (wheel, _) = self.wheel.get_or_insert_with(|| {
            let wheel = Rc::new(Wheel::default());
            let job = context.coroutine_scope().launch(wheel.clone().run(logic));
            (wheel, job)
        });
        let scrolling = wheel.scrolling.get();
        if (pass == PointerEventPass::Initial && scrolling)
            || (pass == PointerEventPass::Main && !scrolling)
        {
            let density = self.logic.borrow().density;
            // The pixels as a scrollable's: a turn down scrolls forward, as a drag up does.
            let pixels = event.wheel_pixels(density, bounds);
            let delta = Offset::ZERO - pixels;
            // What it can't use is left to what's around (a parent, a Ctrl-wheel zoom), unless a
            // wheel gesture of its own goes on, as `onScrollingEvent` with `canConsumeDelta`.
            if pixels == Offset::ZERO || !self.logic.borrow().state.can_scroll(delta) {
                if scrolling {
                    event.consume();
                }
                return;
            }
            wheel.send(delta, (event_time(event) / 1_000_000) as i64);
            event.consume();
            if let Some((_, job)) = &self.wheel {
                let job = *job;
                compose_runtime::platform::poll_now(job);
            }
        }
    }
}

impl ModifierNode for Scrollable2DNode {
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
    fn as_semantics(&mut self) -> Option<&mut dyn compose_ui::SemanticsModifierNode> {
        Some(self)
    }
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.detach();
    }
    fn on_reset(&mut self, _context: &mut NodeContext<'_>) {
        self.detach();
    }
}

impl compose_ui::SemanticsModifierNode for Scrollable2DNode {
    /// Scrolls by the action's offset at once, as compose-rs's `scrollable` does (D491).
    fn apply_semantics(&self, config: &mut compose_ui::SemanticsConfiguration) {
        if !self.enabled {
            return;
        }
        let state = self.logic.borrow().state.clone();
        config.scroll_by(None, move |x, y| {
            if x != 0.0 || y != 0.0 {
                state.dispatch_raw_delta(Offset::new(x, y));
            }
            true
        });
    }
}

impl PointerInputModifierNode for Scrollable2DNode {
    fn waits_for_gesture(&self, press: &PointerEvent) -> bool {
        self.enabled && press.pointer_type != PointerType::Mouse
    }

    fn first_down_follows_the_host(&self) -> bool {
        true
    }

    fn on_pointer_event(
        &mut self,
        event: &mut PointerEvent,
        pass: PointerEventPass,
        bounds: IntSize,
        context: &mut EventContext<'_>,
    ) {
        // The connections around, found on the way in as a gesture or a turn starts; then this
        // scrollable is one for those inside it, enabled or not.
        if pass == PointerEventPass::Initial
            && matches!(event.kind, PointerEventType::Press | PointerEventType::Scroll)
        {
            let chain = NestedScrollChain::current();
            let logic = {
                let mut logic = self.logic.borrow_mut();
                logic.chain = chain;
                logic.clone()
            };
            NestedScrollChain::enter(Rc::new(Connection { logic, enabled: self.enabled }));
        }
        if event.kind == PointerEventType::Scroll {
            if self.enabled {
                self.on_wheel(event, pass, bounds, context);
            }
            return;
        }
        self.drag.on_pointer_event(event, pass, bounds, context);
    }

    fn on_cancel_pointer_input(&mut self) {
        self.drag.on_cancel_pointer_input();
    }
}
