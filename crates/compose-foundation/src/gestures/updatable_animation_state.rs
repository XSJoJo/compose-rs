//! A float animated to zero whose value may change as it runs, as Compose's internal
//! `UpdatableAnimationState`: what a scrolling container's bring-into-view animation runs on.
//! Each frame continues from the value as it is then, as though the animation were started again
//! from it with the velocity it had: set it higher and it falls from there.

use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use compose_animation::{AnimationSpec, AnimationVector, VectorizedAnimationSpec};
use compose_runtime::{Recomposer, RecomposerState};

/// Where the frames come from: one is awaited (its time in nanoseconds), then its end, after the
/// frame's composition and layout.
pub(crate) trait AnimationFrames {
    /// The next frame's time, as `withFrameNanos`.
    fn frame(&self) -> impl Future<Output = u64>;

    /// The end of the frame under way (or the next), its layout done.
    fn frame_end(&self) -> impl Future<Output = ()>;
}

/// The executor's frames, as the host runs them: a frame's tasks run before it composes and lays
/// out, its end after it drew.
pub(crate) struct ExecutorFrames;

impl AnimationFrames for ExecutorFrames {
    fn frame(&self) -> impl Future<Output = u64> {
        compose_runtime::with_frame_nanos(|time| time)
    }

    fn frame_end(&self) -> impl Future<Output = ()> {
        FrameEnd { shared: None, handle: None }
    }
}

/// The end of the frame, as a frame-end callback of the thread's recomposer wakes it.
struct FrameEnd {
    shared: Option<Rc<FrameEndShared>>,
    handle: Option<compose_runtime::CancellationHandle>,
}

#[derive(Default)]
struct FrameEndShared {
    done: Cell<bool>,
    waker: Cell<Option<Waker>>,
}

impl Future for FrameEnd {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if let Some(shared) = &self.shared {
            if shared.done.get() {
                self.handle = None;
                return Poll::Ready(());
            }
            shared.waker.set(Some(context.waker().clone()));
            return Poll::Pending;
        }
        let recomposer = Recomposer::current_thread();
        // One that's over runs no callback: nothing to wait for.
        if matches!(
            recomposer.current_state().get(),
            RecomposerState::ShutDown | RecomposerState::ShuttingDown
        ) {
            return Poll::Ready(());
        }
        let shared = Rc::new(FrameEndShared::default());
        shared.waker.set(Some(context.waker().clone()));
        let told = shared.clone();
        self.handle = Some(recomposer.schedule_frame_end_callback(move || {
            told.done.set(true);
            if let Some(waker) = told.waker.take() {
                waker.wake();
            }
        }));
        self.shared = Some(shared);
        Poll::Pending
    }
}

impl Drop for FrameEnd {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.cancel();
        }
    }
}

const VISIBILITY_THRESHOLD: f32 = 0.01;

fn is_zeroish(value: f32) -> bool {
    value.abs() < VISIBILITY_THRESHOLD
}

/// A value animated to zero along a spec, which may be changed while it runs, as Compose's
/// `UpdatableAnimationState`.
pub(crate) struct UpdatableAnimationState {
    spec: AnimationSpec,
    value: Cell<f32>,
    last_frame_time: Cell<Option<u64>>,
    last_velocity: Cell<f32>,
    running: Cell<bool>,
}

impl UpdatableAnimationState {
    pub(crate) fn new(spec: AnimationSpec) -> UpdatableAnimationState {
        UpdatableAnimationState {
            spec,
            value: Cell::new(0.0),
            last_frame_time: Cell::new(None),
            last_velocity: Cell::new(0.0),
            running: Cell::new(false),
        }
    }

    /// The value: changed each frame while `animate_to_zero` runs, exactly 0 once it returns.
    #[cfg(test)]
    pub(crate) fn value(&self) -> f32 {
        self.value.get()
    }

    /// Sets the value: the next frame animates from it. Setting it doesn't start the animation.
    pub(crate) fn set_value(&self, value: f32) {
        self.value.set(value);
    }

    /// Animates the value to 0, as `animateToZero`: each frame, `before_frame` is told how much
    /// it fell (before the frame composes and lays out), then `after_frame` runs once the frame
    /// ended (it may set the value again, from what the layout did). Returns once the value is 0,
    /// at once if it is. With animations off (a duration scale of 0) it ends after one frame.
    ///
    /// # Panics
    ///
    /// If it's running already.
    pub(crate) async fn animate_to_zero(
        &self,
        frames: &impl AnimationFrames,
        mut before_frame: impl FnMut(f32),
        mut after_frame: impl FnMut(),
    ) {
        assert!(!self.running.get(), "animate_to_zero called while previous animation is running");
        let scale = compose_ui::motion_duration_scale();
        self.running.set(true);
        let _reset = Reset(self);
        // Not the spec's duration: the value may be changed after each frame, and then it goes on.
        while !is_zeroish(self.value.get()) {
            let frame_time = frames.frame().await;
            let last = *self.last_frame_time.get().get_or_insert(frame_time);
            let value = self.value.get();
            let initial = AnimationVector::from(value);
            let target = AnimationVector::from(0.0);
            let velocity = AnimationVector::from(self.last_velocity.get());
            let play_time = if scale == 0.0 {
                // Animations off (accessibility or developer settings): at once.
                self.spec.duration_nanos(initial, target, velocity)
            } else {
                (frame_time.saturating_sub(last) as f64 / scale as f64).round() as i64
            };
            let new_value = self.spec.value_from_nanos(play_time, initial, target, velocity)[0];
            let new_velocity =
                self.spec.velocity_from_nanos(play_time, initial, target, velocity)[0];
            self.last_velocity.set(new_velocity);
            self.last_frame_time.set(Some(frame_time));
            self.value.set(new_value);
            before_frame(value - new_value);
            frames.frame_end().await;
            after_frame();
            if scale == 0.0 {
                break;
            }
        }
        // The last frame may have left less than the threshold: it's given to one more, so the
        // value ends at exactly 0.
        if self.value.get() != 0.0 {
            frames.frame().await;
            let delta = self.value.get();
            self.value.set(0.0);
            before_frame(delta);
            frames.frame_end().await;
            after_frame();
        }
    }
}

/// Forgets the frames once the animation ends, however it ends.
struct Reset<'a>(&'a UpdatableAnimationState);

impl Drop for Reset<'_> {
    fn drop(&mut self) {
        self.0.last_frame_time.set(None);
        self.0.last_velocity.set(0.0);
        self.0.running.set(false);
    }
}

#[cfg(test)]
mod tests {
    //! Compose's `UpdatableAnimationStateTest`, rewritten, over frames 16 ms apart that come at
    //! once.

    use std::cell::RefCell;
    use std::pin::pin;

    use super::*;

    /// Frames 16 ms apart, each there as soon as it's awaited, as the test's `TestFrameClock`.
    struct ImmediateFrames(Cell<u64>);

    impl AnimationFrames for ImmediateFrames {
        fn frame(&self) -> impl Future<Output = u64> {
            let time = self.0.get();
            self.0.set(time + 16_000_000);
            std::future::ready(time)
        }

        fn frame_end(&self) -> impl Future<Output = ()> {
            std::future::ready(())
        }
    }

    /// Runs `future`, which never waits, to its end, as `runBlocking` with that clock.
    fn run<T>(future: impl Future<Output = T>) -> T {
        let mut future = pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("the frames come at once"),
        }
    }

    fn state() -> UpdatableAnimationState {
        UpdatableAnimationState::new(compose_animation::spring().into())
    }

    fn frames() -> ImmediateFrames {
        ImmediateFrames(Cell::new(0))
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/UpdatableAnimationStateTest.kt, animateToZero_doesNothing_whenValueIsZero
    #[test]
    fn at_zero_it_returns_without_a_frame() {
        let state = state();
        let frames = frames();
        run(state.animate_to_zero(&frames, |_| panic!("no frame"), || panic!("no frame")));
        assert_eq!(frames.0.get(), 0);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/UpdatableAnimationStateTest.kt, animateToZero_animatesToZero_fromPositiveValue
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/UpdatableAnimationStateTest.kt, animateToZero_animatesToZero_fromNegativeValue
    #[test]
    fn it_falls_to_exactly_zero_from_either_side() {
        for start in [10.0, -10.0] {
            let state = state();
            state.set_value(start);
            let deltas = RefCell::new(Vec::new());
            run(state.animate_to_zero(&frames(), |delta| deltas.borrow_mut().push(delta), || {}));
            assert_eq!(state.value(), 0.0);
            let deltas = deltas.into_inner();
            assert!(deltas.len() > 1);
            // What it fell by, all of it.
            assert!((deltas.iter().sum::<f32>() - start).abs() < 1e-4);
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/UpdatableAnimationStateTest.kt, animateToZero_handlesZeroAnimationScale
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/UpdatableAnimationStateTest.kt, animateToZero_handlesDoubleAnimationScale
    #[test]
    fn a_duration_scale_of_zero_ends_it_at_once_and_of_two_slows_it() {
        let frame_count = |scale: f32| {
            let state = state();
            state.set_value(10.0);
            let deltas = RefCell::new(Vec::new());
            run(compose_ui::with_motion_duration_scale(
                scale,
                state.animate_to_zero(&frames(), |delta| deltas.borrow_mut().push(delta), || {}),
            ));
            assert_eq!(state.value(), 0.0);
            deltas.into_inner().len()
        };
        // Its frame, then one giving what the spring's end left below the threshold.
        assert!(frame_count(0.0) <= 2);
        assert!(frame_count(1.0) > 2);
        assert!(frame_count(2.0) > frame_count(1.0));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/UpdatableAnimationStateTest.kt, animateToZero_animatesToZero_whenValueIncreasedAfterFrame
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/UpdatableAnimationStateTest.kt, animateToZero_animatesToZero_whenValueDecreasedAfterFrame
    #[test]
    fn a_value_set_after_a_frame_is_animated_from() {
        for (start, later) in [(10.0, [20.0, 30.0, 40.0]), (100.0, [50.0, 25.0, 1.0])] {
            let state = state();
            state.set_value(start);
            let later = RefCell::new(later.to_vec());
            run(state.animate_to_zero(
                &frames(),
                |_| {},
                || {
                    if !later.borrow().is_empty() {
                        state.set_value(later.borrow_mut().remove(0));
                    }
                },
            ));
            assert_eq!(state.value(), 0.0);
            assert!(later.borrow().is_empty());
        }
    }
}
