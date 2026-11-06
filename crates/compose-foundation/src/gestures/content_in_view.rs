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

use compose_runtime::CompositionLocalAnchor;
use compose_ui::{
    BringIntoViewBounds, BringIntoViewScope, IntSize, LocalBoxFuture, NodeHandle, Rect,
};

use crate::gestures::bring_into_view_queue::{BringIntoViewRequestPriorityQueue, Request};
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

/// A container's requests and its animation, made at its first request.
pub(crate) struct ContentInView {
    config: RefCell<ContentInViewConfig>,
    locals: Cell<Option<CompositionLocalAnchor>>,
    /// The container's size, as its requests' bounds last said.
    viewport: Rc<Cell<IntSize>>,
    requests: RefCell<BringIntoViewRequestPriorityQueue>,
    animating: Cell<bool>,
}

impl ContentInView {
    pub(crate) fn new(config: ContentInViewConfig) -> Rc<ContentInView> {
        Rc::new(ContentInView {
            config: RefCell::new(config),
            locals: Cell::new(None),
            viewport: Rc::default(),
            requests: RefCell::default(),
            animating: Cell::new(false),
        })
    }

    pub(crate) fn set_config(&self, config: ContentInViewConfig) {
        *self.config.borrow_mut() = config;
    }

    fn spec(&self) -> Rc<dyn BringIntoViewSpec> {
        resolve_spec(&self.config.borrow().spec, self.locals.get())
    }

    fn vertical(&self) -> bool {
        self.config.borrow().vertical
    }

    /// Whether `rect` is as visible as it can be: in view, or spanning the viewport, as
    /// `isMaxVisible`.
    fn is_max_visible(&self, rect: Rect) -> bool {
        let spec = self.spec();
        relocation(&*spec, rect, self.viewport.get(), self.vertical()).abs() <= MIN_SCROLL_THRESHOLD
    }

    /// Asked by the tree to show what `bounds` gives, which doesn't show yet (`shows`), as
    /// `bringChildIntoView`: the future ends once it's shown, or the request was interrupted.
    pub(crate) fn bring_child_into_view(
        self: &Rc<Self>,
        bounds: BringIntoViewBounds,
        scope: &BringIntoViewScope,
    ) -> LocalBoxFuture<'static, ()> {
        self.locals.set(Some(scope.locals()));
        self.viewport.set(scope.viewport());
        let current = self.current_bounds(bounds);
        let this = self.clone();
        let handle = scope.handle();
        Box::pin(async move {
            // What's gone by now is resumed at once.
            let request = Request::new(current);
            let wait = request.continuation.clone().wait();
            let enqueued = this.requests.borrow_mut().enqueue(request);
            if enqueued && !this.animating.get() {
                this.launch_animation(handle);
            }
            wait.await;
        })
    }

    /// What `bounds` gives in the container now, where the scroll asked for will put it.
    fn current_bounds(&self, bounds: BringIntoViewBounds) -> Rc<dyn Fn() -> Option<Rect>> {
        let config = self.config.borrow().clone();
        let viewport = self.viewport.clone();
        Rc::new(move || {
            let (rect, size) = bounds.current()?;
            viewport.set(size);
            Some(shifted(rect, config.pending_shift(), config.vertical))
        })
    }

    /// The largest request that fits in the viewport (the smallest, if none does), as
    /// `findBringIntoViewRequest`.
    fn find_request(&self) -> Option<Rect> {
        let vertical = self.vertical();
        let along = |width: f32, height: f32| if vertical { height } else { width };
        let mut found = None;
        self.requests.borrow_mut().for_each_from_smallest(|bounds| {
            // One detached is left for the frame's end to remove.
            let Some(bounds) = bounds else { return true };
            let viewport = self.viewport.get();
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

    /// How far to scroll to show the requests, as `calculateScrollDelta`.
    fn scroll_delta(&self, spec: &dyn BringIntoViewSpec) -> f32 {
        let Some(rect) = self.find_request() else { return 0.0 };
        relocation(spec, rect, self.viewport.get(), self.vertical())
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
        self.animating.set(true);
        // Dropped with the task however it ends, as Compose's `finally`.
        let ending = AnimationEnd(self.clone());
        scope.launch(async move {
            let ending = ending;
            let this = &*ending.0;
            let config = this.config.borrow().clone();
            let multiplier = config.multiplier();
            let (animation, spec) = (&animation, &*spec);
            let scrolled = config
                .state
                .scroll(MutatePriority::Default, move |scroll| async move {
                    animation.set_value(this.scroll_delta(spec));
                    animation
                        .animate_to_zero(
                            &ExecutorFrames,
                            |delta| {
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
                                this.requests.borrow_mut().resume_and_remove_while(|bounds| {
                                    bounds.is_none_or(|bounds| this.is_max_visible(bounds))
                                });
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
        self.0.animating.set(false);
        if let Ok(mut requests) = self.0.requests.try_borrow_mut() {
            requests.cancel_and_remove_all();
        }
    }
}
