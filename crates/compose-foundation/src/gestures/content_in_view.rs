//! What a scrolling container does when asked to bring something inside it into view, as
//! Compose's `ContentInViewNode`: the requests wait in a queue (each containing the next, a newer
//! one beside them interrupting them), and one animation scrolls the container towards the
//! largest that fits, a frame at a time, its target worked out again after each frame's layout.
//! A request is resumed once it's as visible as it can be (or its requester is detached); the
//! rest are resumed when the animation ends, and cancelled if it's interrupted (a drag, a scroll
//! of a higher priority) or the container is detached.
//!
//! Nothing is kept until a request comes: a container nobody asks has no queue (D1376).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_runtime::{CancellationHandle, CompositionLocalAnchor};
use compose_ui::{
    BoundsRead, BringIntoViewBounds, BringIntoViewScope, IntSize, LocalBoxFuture, NodeHandle, Rect,
};

use crate::gestures::bring_into_view_queue::{
    BringIntoViewRequestPriorityQueue, ReadBounds, Request,
};
use crate::gestures::updatable_animation_state::{ExecutorFrames, UpdatableAnimationState};
use crate::gestures::{ScrollableState, ScrollableStateExt};
use crate::mutator::MutatePriority;
use crate::relocation::{BringIntoViewSpec, DefaultBringIntoViewSpec, LOCAL_BRING_INTO_VIEW_SPEC};

/// A scroll smaller than this (in pixels) counts as none, as `MinScrollThreshold`.
const MIN_SCROLL_THRESHOLD: f32 = 0.5;

/// What the container scrolls and how.
#[derive(Clone)]
pub(crate) struct ContentInViewConfig {
    pub(crate) state: Rc<dyn ScrollableState>,
    pub(crate) vertical: bool,
    /// As the scrollable's `reverseDirection`: whether the state's pixels run against the
    /// layout's (a `vertical_scroll` not reversed: scrolling forward moves the content up).
    pub(crate) reverse_direction: bool,
    /// The spec given to the scrollable, else `LOCAL_BRING_INTO_VIEW_SPEC`'s.
    pub(crate) spec: Option<Rc<dyn BringIntoViewSpec>>,
    /// The scroll asked for and not laid out yet, in the state's pixels: a `vertical_scroll`'s,
    /// which lays its content out itself (a scroll's bounds are read before its layout).
    pub(crate) pending: Option<Rc<dyn Fn() -> f32>>,
}

impl ContentInViewConfig {
    /// The state's pixels for a pixel the content moves back along the axis.
    fn multiplier(&self) -> f32 {
        if self.reverse_direction { 1.0 } else { -1.0 }
    }

    /// Where the content will be once the pending scroll is laid out, from where it was.
    pub(crate) fn pending_shift(&self) -> f32 {
        self.pending.as_ref().map_or(0.0, |pending| -self.multiplier() * pending())
    }
}

/// The spec a container uses: the one given, else the one where it's composed, as
/// `requireBringIntoViewSpec`.
pub(crate) fn resolve_spec(
    given: &Option<Rc<dyn BringIntoViewSpec>>,
    locals: Option<CompositionLocalAnchor>,
) -> Rc<dyn BringIntoViewSpec> {
    match (given, locals) {
        (Some(spec), _) => spec.clone(),
        (None, Some(locals)) => locals.get(&LOCAL_BRING_INTO_VIEW_SPEC),
        (None, None) => Rc::new(DefaultBringIntoViewSpec),
    }
}

/// `rect` moved by `by` along the axis.
pub(crate) fn shifted(rect: Rect, by: f32, vertical: bool) -> Rect {
    if vertical {
        Rect::new(rect.left, rect.top + by, rect.right, rect.bottom + by)
    } else {
        Rect::new(rect.left + by, rect.top, rect.right + by, rect.bottom)
    }
}

/// How far `spec` says the content is to move back along the axis to show `rect` in `viewport`,
/// as `relocationOffset`.
pub(crate) fn relocation(
    spec: &dyn BringIntoViewSpec,
    rect: Rect,
    viewport: IntSize,
    vertical: bool,
) -> f32 {
    if vertical {
        spec.calculate_scroll_distance(rect.top, rect.bottom - rect.top, viewport.height as f32)
    } else {
        spec.calculate_scroll_distance(rect.left, rect.right - rect.left, viewport.width as f32)
    }
}

/// Where `rect` will be once shown, as `computeDestination`: what the parents are asked to show.
pub(crate) fn destination(
    spec: &dyn BringIntoViewSpec,
    rect: Rect,
    viewport: IntSize,
    vertical: bool,
) -> Rect {
    shifted(rect, -relocation(spec, rect, viewport, vertical), vertical)
}

/// Whether `rect`, where the walk found it, is as visible as it can be in the container
/// (`isMaxVisible`), once moved by `shift` (where the scroll asked for will put it), `parent`
/// being its `destination`: a request for it is done at once. The spec isn't asked again.
pub(crate) fn shows(rect: Rect, parent: Rect, shift: f32, vertical: bool) -> bool {
    let rect = shifted(rect, shift, vertical);
    let relocation = if vertical { rect.top - parent.top } else { rect.left - parent.left };
    relocation.abs() <= MIN_SCROLL_THRESHOLD
}

/// Notes a container measured at `size`, last at `*measured`: what it was if it shrank along its
/// axis (not the first measurement), as `onRemeasured`'s test.
pub(crate) fn shrunk(measured: &mut IntSize, size: IntSize, vertical: bool) -> Option<IntSize> {
    let previous = std::mem::replace(measured, size);
    let along = |size: IntSize| if vertical { size.height } else { size.width };
    (along(size) < along(previous)).then_some(previous)
}

/// A container's `ContentInView` with `config`, made if it has none.
pub(crate) fn with_config(
    content_in_view: &mut Option<Rc<ContentInView>>,
    config: ContentInViewConfig,
) -> Rc<ContentInView> {
    match content_in_view {
        Some(content_in_view) => {
            content_in_view.set_config(config);
            content_in_view.clone()
        }
        None => content_in_view.insert(ContentInView::new(config)).clone(),
    }
}

/// A container's requests and its animation, made at its first request.
///
/// The requests' bounds are read (application code may run: a deprecated
/// `BringIntoViewResponder`'s `calculate_rect_for_parent`) with nothing of it borrowed, then the
/// queue decides from what they read; the queue is borrowed only for that, never across a read or
/// an await, so it's free whenever the animation ends.
pub(crate) struct ContentInView {
    /// Read when it's used (a frame's scroll, a read of a request's bounds): an update of the
    /// container mid-animation is followed, but for its state (the scroll held is of the state it
    /// started with).
    config: Rc<RefCell<ContentInViewConfig>>,
    locals: Cell<Option<CompositionLocalAnchor>>,
    /// The container's size, as its requests' bounds last said.
    viewport: Rc<Cell<IntSize>>,
    requests: RefCell<BringIntoViewRequestPriorityQueue>,
    animating: Cell<bool>,
    /// Keeping the focused child in view after the viewport shrank, as `trackingFocusedChild`.
    tracking: Cell<bool>,
    /// A shrink waiting for its frame's end to see whether the focused child was clipped: the
    /// check's handle, cancelled if the container is detached first.
    checking: RefCell<Option<CancellationHandle>>,
    /// The container's handle, for the focused rect inside it.
    handle: Cell<Option<NodeHandle>>,
}

impl ContentInView {
    pub(crate) fn new(config: ContentInViewConfig) -> Rc<ContentInView> {
        Rc::new(ContentInView {
            config: Rc::new(RefCell::new(config)),
            locals: Cell::new(None),
            viewport: Rc::default(),
            requests: RefCell::default(),
            animating: Cell::new(false),
            tracking: Cell::new(false),
            checking: RefCell::new(None),
            handle: Cell::new(None),
        })
    }

    pub(crate) fn set_config(&self, config: ContentInViewConfig) {
        *self.config.borrow_mut() = config;
    }

    fn spec(&self) -> Rc<dyn BringIntoViewSpec> {
        let given = self.config.borrow().spec.clone();
        resolve_spec(&given, self.locals.get())
    }

    fn vertical(&self) -> bool {
        self.config.borrow().vertical
    }

    /// Whether `rect` is as visible as it can be: in view, or spanning the viewport, as
    /// `isMaxVisible`.
    fn is_max_visible(&self, spec: &dyn BringIntoViewSpec, rect: Rect) -> bool {
        relocation(spec, rect, self.viewport.get(), self.vertical()).abs() <= MIN_SCROLL_THRESHOLD
    }

    /// The focused rect inside the container, where the scroll asked for will put it, as
    /// `getFocusedRect()`.
    fn focused_rect(&self) -> Option<Rect> {
        let rect = self.handle.get()?.focused_rect_inside()?;
        let config = self.config.borrow();
        Some(shifted(rect, config.pending_shift(), config.vertical))
    }

    /// The container was measured smaller along its axis, from `previous` to `size`, as
    /// `onRemeasured`: if that clipped the focused child, which showed before, the container
    /// animates to keep it in view. compose-rs can't read the tree during the measurement, so the
    /// focused child is looked at once the frame's layout is done, where it's placed: one placed
    /// from the container's end (`!reverse_direction`, as Compose's adjustment) was where it is
    /// now plus what the viewport lost.
    pub(crate) fn on_viewport_shrunk(
        self: &Rc<Self>,
        previous: IntSize,
        size: IntSize,
        handle: NodeHandle,
        locals: CompositionLocalAnchor,
    ) {
        self.handle.set(Some(handle));
        self.locals.set(Some(locals));
        self.viewport.set(size);
        if self.animating.get() || self.tracking.get() || self.checking.borrow().is_some() {
            return;
        }
        // At the end of this frame, its layout done and the tree free; the animation it may
        // start takes its first step then.
        let this = self.clone();
        let check = move || {
            this.checking.take();
            this.keep_focused_in_view(previous, size, handle);
        };
        *self.checking.borrow_mut() = locals.schedule_frame_end_callback(check);
    }

    /// The container is detached, as the end of `ContentInViewNode`'s coroutine scope: a check
    /// waiting for the frame's end won't run (it would keep the container until a frame ends).
    pub(crate) fn on_detach(&self) {
        if let Some(check) = self.checking.take() {
            check.cancel();
        }
    }

    /// Starts keeping the focused child in view if the shrink from `previous` to `size` clipped
    /// it, and it showed before.
    fn keep_focused_in_view(self: &Rc<Self>, previous: IntSize, size: IntSize, handle: NodeHandle) {
        if self.animating.get() || self.tracking.get() {
            return;
        }
        let Some(rect) = self.focused_rect() else { return };
        let spec = self.spec();
        let (vertical, from_end) = {
            let config = self.config.borrow();
            (config.vertical, !config.reverse_direction)
        };
        let along = |size: IntSize| if vertical { size.height } else { size.width } as f32;
        let lost = if from_end { along(previous) - along(size) } else { 0.0 };
        let was = shifted(rect, lost, vertical);
        let showed = relocation(&*spec, was, previous, vertical).abs() <= MIN_SCROLL_THRESHOLD;
        if showed && !self.is_max_visible(&*spec, rect) {
            self.tracking.set(true);
            self.launch_animation(handle);
        }
    }

    /// Reads every request's bounds again, outside the queue; `at_frame_end`, where the tree is
    /// free, every read tells.
    fn refresh(&self, at_frame_end: bool) -> Vec<Request> {
        let requests = self.requests.borrow_mut().snapshot();
        for request in &requests {
            let told = request.refresh();
            // A tree busy at a frame's end is a bug; the cap on reads that can't tell
            // (`MOST_BUSY_READS`) would otherwise take the request for gone frames later.
            debug_assert!(
                told || !at_frame_end,
                "a bring-into-view request's bounds couldn't be read at the frame's end"
            );
        }
        requests
    }

    /// Asked by the tree to show what `bounds` gives, which doesn't show yet (`shows`) and is at
    /// `rect` now, as `bringChildIntoView`: the future ends once it's shown, or the request was
    /// interrupted.
    pub(crate) fn bring_child_into_view(
        self: &Rc<Self>,
        rect: Rect,
        bounds: BringIntoViewBounds,
        scope: &BringIntoViewScope,
    ) -> LocalBoxFuture<'static, ()> {
        self.locals.set(Some(scope.locals()));
        self.viewport.set(scope.viewport());
        let first = {
            let config = self.config.borrow();
            shifted(rect, config.pending_shift(), config.vertical)
        };
        let request = Request::new(self.read_bounds(bounds), Some(first));
        let this = self.clone();
        let handle = scope.handle();
        Box::pin(async move {
            let wait = request.continuation.clone().wait();
            // Where it and the others are now (it may be gone already: resumed at once).
            this.refresh(false);
            request.refresh();
            let enqueued = this.requests.borrow_mut().enqueue(request);
            if enqueued && !this.animating.get() {
                this.launch_animation(handle);
            }
            wait.await;
        })
    }

    /// How a request reads what `bounds` gives in the container now, where the scroll asked for
    /// will put it: `Some(None)` once it's gone, `None` while the tree can't be read.
    fn read_bounds(&self, bounds: BringIntoViewBounds) -> ReadBounds {
        let config = self.config.clone();
        let viewport = self.viewport.clone();
        Rc::new(move || match bounds.read() {
            BoundsRead::Now(rect, size) => {
                viewport.set(size);
                let config = config.borrow();
                Some(Some(shifted(rect, config.pending_shift(), config.vertical)))
            }
            BoundsRead::Gone => Some(None),
            BoundsRead::Busy => None,
        })
    }

    /// The largest request that fits in the viewport (the smallest, if none does), as
    /// `findBringIntoViewRequest`, by the bounds last read.
    fn find_request(&self) -> Option<Rect> {
        let vertical = self.vertical();
        let viewport = self.viewport.get();
        let along = |width: f32, height: f32| if vertical { height } else { width };
        let mut found = None;
        self.requests.borrow_mut().for_each_from_smallest(|bounds| {
            // One detached is left for the frame's end to remove.
            let Some(bounds) = bounds else { return true };
            let fits = along(bounds.width(), bounds.height())
                <= along(viewport.width as f32, viewport.height as f32);
            if fits {
                found = Some(bounds);
                true
            } else {
                found.get_or_insert(bounds);
                false
            }
        });
        found
    }

    /// How far to scroll to show the requests, else the focused child being kept in view, as
    /// `calculateScrollDelta`, by the bounds last read (the spec asked with nothing borrowed).
    fn scroll_delta(&self, spec: &dyn BringIntoViewSpec) -> f32 {
        let rect = self
            .find_request()
            .or_else(|| if self.tracking.get() { self.focused_rect() } else { None });
        let Some(rect) = rect else { return 0.0 };
        relocation(spec, rect, self.viewport.get(), self.vertical())
    }

    /// Resumes the requests this frame's layout showed (or whose requester went), the smallest
    /// first, as `resumeAndRemoveWhile { it == null || isMaxVisible(it) }`: each decided with
    /// nothing borrowed, then resumed if it's still the smallest.
    fn resume_shown(&self, spec: &dyn BringIntoViewSpec) {
        let requests = self.refresh(true);
        let shown: Vec<&Request> = requests
            .iter()
            .rev()
            .take_while(|request| {
                request.bounds().is_none_or(|bounds| self.is_max_visible(spec, bounds))
            })
            .collect();
        let mut shown = shown.into_iter();
        self.requests
            .borrow_mut()
            .resume_and_remove_while(|request| shown.next().is_some_and(|it| it.is(request)));
    }

    /// Starts the animation in the container's scope, as `launchAnimation`: it holds the state's
    /// scroll at the default priority (a drag interrupts it), and goes on until nothing is left to
    /// scroll.
    fn launch_animation(self: &Rc<Self>, handle: NodeHandle) {
        let Some(scope) = handle.coroutine_scope() else {
            self.requests.borrow_mut().cancel_and_remove_all();
            return;
        };
        let spec = self.spec();
        #[allow(deprecated)]
        let animation = UpdatableAnimationState::new(spec.scroll_animation_spec());
        let frames = ExecutorFrames::of(self.locals.get());
        self.animating.set(true);
        // Dropped with the task however it ends, as Compose's `finally`.
        let ending = AnimationEnd(self.clone());
        scope.launch(async move {
            let ending = ending;
            let this = &*ending.0;
            let state = this.config.borrow().state.clone();
            let (animation, spec, frames) = (&animation, &*spec, &frames);
            let scrolled = state
                .scroll(MutatePriority::Default, move |scroll| async move {
                    this.refresh(false);
                    animation.set_value(this.scroll_delta(spec));
                    animation
                        .animate_to_zero(
                            frames,
                            |delta| {
                                let multiplier = this.config.borrow().multiplier();
                                let consumed = multiplier * scroll.scroll_by(multiplier * delta);
                                // What the state didn't take this frame it won't take later (an
                                // edge): the animation ends here, its requests cancelled, or it
                                // would ask forever.
                                if consumed.abs() < delta.abs() {
                                    compose_runtime::platform::cancel_current_task();
                                }
                            },
                            || {
                                // The requests this frame's layout showed (or whose requester
                                // went) are done; the target is worked out again from the rest.
                                this.resume_shown(spec);
                                // Kept in view once it shows.
                                if this.tracking.get()
                                    && this
                                        .focused_rect()
                                        .is_some_and(|rect| this.is_max_visible(spec, rect))
                                {
                                    this.tracking.set(false);
                                }
                                animation.set_value(this.scroll_delta(spec));
                            },
                        )
                        .await;
                })
                .await;
            // Those too big to show, or that needed no scroll, once it ended well.
            if scrolled.is_ok() {
                this.requests.borrow_mut().resume_and_remove_all();
            }
        });
    }
}

/// Ends the animation: the requests it leaves are cancelled.
struct AnimationEnd(Rc<ContentInView>);

impl Drop for AnimationEnd {
    fn drop(&mut self) {
        let this = &self.0;
        this.animating.set(false);
        this.tracking.set(false);
        match this.requests.try_borrow_mut() {
            Ok(mut requests) => requests.cancel_and_remove_all(),
            Err(_) => {
                // The queue is never borrowed across an await or a read, so the task can't end
                // while it is; if it ever does, the requests are cancelled once it's free rather
                // than left waiting forever.
                debug_assert!(
                    std::thread::panicking(),
                    "the bring-into-view animation ended with its requests borrowed"
                );
                let this = this.clone();
                compose_runtime::platform::launch(async move {
                    this.requests.borrow_mut().cancel_and_remove_all();
                });
            }
        }
    }
}
