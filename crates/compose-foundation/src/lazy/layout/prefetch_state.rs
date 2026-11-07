//! The public `LazyLayoutPrefetchState`, as Compose's: requests to compose a [`LazyLayout`]'s
//! items, and measure them, between frames, before a measurement asks for them (D1324).
//!
//! A request runs in the host's idle time (`LOCAL_PREFETCH_SCHEDULER`), each of its steps only
//! while the time left exceeds that step's running average for the item's content type, as
//! Compose's `PrefetchMetrics` (`current / 4 * 3 + new / 4`, the first sample taken as is), or any
//! time left once it's urgent: the item's composition (paused when the time runs out, resumed
//! after the next frame), applying it, then measuring each of its layouts. Its slot is the item's
//! key's, so the measurement composing the item takes it as it is.
//!
//! [`LazyLayout`]: super::LazyLayout

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use compose_ui::subcompose::{PausedPrecomposition, PrecomposedSlotHandle, SubcomposeLayoutState};
use compose_ui::{Constraints, IntSize, PrefetchRequest, PrefetchRequestScope, PrefetchScheduler};

use super::super::slots::{IndexMap, LazySlot, SlotSource};
use super::lazy_layout::ProviderSource;

/// What a [`LazyLayout`](super::LazyLayout) is told to compose and measure ahead, as Compose's
/// `LazyLayoutPrefetchState`: given to one layout (`prefetch_state`), it schedules requests for
/// its items, which do nothing while it isn't composed.
#[derive(Clone, Default)]
pub struct LazyLayoutPrefetchState {
    inner: Rc<PrefetchStateInner>,
}

#[derive(Default)]
struct PrefetchStateInner {
    /// The layout's, while one is composed with this state, as Compose's `prefetchHandleProvider`.
    provider: RefCell<Option<Rc<HandleProvider>>>,
    /// The steps' running averages, by content type, as Compose's `PrefetchMetrics`.
    averages: RefCell<IndexMap<Option<u64>, Averages>>,
}

impl compose_runtime::ComposeArg for LazyLayoutPrefetchState {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl LazyLayoutPrefetchState {
    pub fn new() -> LazyLayoutPrefetchState {
        LazyLayoutPrefetchState::default()
    }

    /// Schedules the composition of item `index`, as Compose's `schedulePrecomposition`. An item
    /// composed already when the request runs is left as it is. With no layout composed with
    /// this state, the handle does nothing.
    pub fn schedule_precomposition(&self, index: usize) -> PrefetchHandle {
        self.schedule(index, None, None)
    }

    /// Schedules the composition of item `index` and the measurement of its layouts with
    /// `constraints`, as Compose's `schedulePrecompositionAndPremeasure`.
    pub fn schedule_precomposition_and_premeasure(
        &self,
        index: usize,
        constraints: Constraints,
    ) -> PrefetchHandle {
        self.schedule(index, Some(constraints), None)
    }

    /// [`schedule_precomposition_and_premeasure`](Self::schedule_precomposition_and_premeasure),
    /// `on_item_premeasured` called once the item is measured (not if the request is cancelled or
    /// measures nothing), as Compose's `onItemPremeasured`.
    pub fn schedule_precomposition_and_premeasure_with(
        &self,
        index: usize,
        constraints: Constraints,
        on_item_premeasured: impl FnOnce(&PrefetchResultScope<'_>) + 'static,
    ) -> PrefetchHandle {
        self.schedule(index, Some(constraints), Some(Box::new(on_item_premeasured)))
    }

    fn schedule(
        &self,
        index: usize,
        constraints: Option<Constraints>,
        on_item_premeasured: Option<OnPremeasured>,
    ) -> PrefetchHandle {
        let Some(provider) = self.inner.provider.borrow().clone() else {
            return PrefetchHandle(None);
        };
        let request = Rc::new(Request {
            index,
            constraints,
            on_item_premeasured: RefCell::new(on_item_premeasured),
            provider: Rc::downgrade(&provider),
            state: Rc::downgrade(&self.inner),
            paused: RefCell::new(None),
            precomposed: RefCell::new(None),
            key: Cell::new(None),
            measured: Cell::new(false),
            canceled: Cell::new(false),
            urgent: Cell::new(false),
        });
        provider.scheduler.schedule_prefetch(request.clone());
        PrefetchHandle(Some(request))
    }

    /// For Compose's `changingKeyForPrefetchingItemInTheMiddleOfRequest`, as its
    /// `shouldPauseBetweenPrecompositionAndPremeasure`: a request that composed its item ends
    /// that idle time before measuring it.
    #[doc(hidden)]
    pub fn pause_between_precomposition_and_premeasure(&self, pause: bool) {
        if let Some(provider) = &*self.inner.provider.borrow() {
            provider.pause_before_premeasure.set(pause);
        }
    }

    /// How many item contents and their scopes the layout composed with this state keeps by key,
    /// and how many times they were trimmed, for tests.
    #[doc(hidden)]
    pub fn item_contents_held(&self) -> (usize, usize, usize) {
        self.inner.provider.borrow().as_ref().map_or((0, 0, 0), |it| it.source.contents_held())
    }

    /// The layout `source` and `layout` are a lazy layout's, composed with this state.
    pub(super) fn attach(
        &self,
        source: Rc<ProviderSource>,
        layout: Rc<SubcomposeLayoutState>,
        scheduler: PrefetchScheduler,
    ) -> Rc<HandleProvider> {
        let provider = Rc::new(HandleProvider {
            source,
            layout,
            scheduler,
            pause_before_premeasure: Cell::new(false),
        });
        *self.inner.provider.borrow_mut() = Some(provider.clone());
        provider
    }

    /// The layout `provider` went: its requests do nothing more.
    pub(super) fn detach(&self, provider: &Rc<HandleProvider>) {
        let mut current = self.inner.provider.borrow_mut();
        if current.as_ref().is_some_and(|current| Rc::ptr_eq(current, provider)) {
            *current = None;
        }
    }
}

/// What a request works on, while its layout's composed, as Compose's `PrefetchHandleProvider`.
pub(super) struct HandleProvider {
    source: Rc<ProviderSource>,
    layout: Rc<SubcomposeLayoutState>,
    scheduler: PrefetchScheduler,
    pause_before_premeasure: Cell<bool>,
}

type OnPremeasured = Box<dyn FnOnce(&PrefetchResultScope<'_>)>;

/// A request, as Compose's `PrefetchHandle`: to cancel it, or mark it urgent.
#[derive(Clone)]
pub struct PrefetchHandle(Option<Rc<Request>>);

impl PrefetchHandle {
    /// The item's no longer wanted: the request stops, and what it composed goes (kept for reuse
    /// or disposed, as the layout's reuse policy says) unless a measurement took it, as Compose's
    /// `cancel`.
    pub fn cancel(&self) {
        if let Some(request) = &self.0
            && !request.canceled.replace(true)
        {
            request.clean_up();
        }
    }

    /// The item's expected in the next frame: each step runs with any time left, not only when
    /// its average fits, as Compose's `markAsUrgent`.
    pub fn mark_as_urgent(&self) {
        if let Some(request) = &self.0 {
            request.urgent.set(true);
        }
    }
}

/// What `on_item_premeasured` reads, as Compose's `PrefetchResultScope`.
pub struct PrefetchResultScope<'a> {
    index: usize,
    handle: &'a PrecomposedSlotHandle,
}

impl PrefetchResultScope<'_> {
    /// The item's index.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The number of layouts the item emitted, as `placeablesCount`.
    pub fn placeables_count(&self) -> usize {
        self.handle.placeables_count()
    }

    /// The size layout `placeable_index` was measured at, in pixels, as `getSize`.
    pub fn get_size(&self, placeable_index: usize) -> IntSize {
        self.handle.get_size(placeable_index)
    }
}

/// The running averages of a content type's steps, in nanoseconds (0: none yet).
#[derive(Clone, Copy, Default)]
struct Averages {
    resume: u64,
    pause: u64,
    apply: u64,
    measure: u64,
}

/// `current / 4 * 3 + new / 4`, the first sample as is, as Compose's `calculateAverageTime`.
fn averaged(current: u64, new: u64) -> u64 {
    if current == 0 { new } else { current / 4 * 3 + new / 4 }
}

struct Request {
    index: usize,
    constraints: Option<Constraints>,
    on_item_premeasured: RefCell<Option<OnPremeasured>>,
    provider: Weak<HandleProvider>,
    state: Weak<PrefetchStateInner>,
    paused: RefCell<Option<Rc<PausedPrecomposition>>>,
    precomposed: RefCell<Option<PrecomposedSlotHandle>>,
    /// The key it composed, as `keyUsedForComposition`: another key at its index ends it.
    key: Cell<Option<u64>>,
    measured: Cell<bool>,
    canceled: Cell<bool>,
    urgent: Cell<bool>,
}

impl Request {
    fn clean_up(&self) {
        if let Some(paused) = self.paused.borrow_mut().take() {
            paused.cancel();
        }
        if let Some(handle) = self.precomposed.borrow_mut().take() {
            handle.dispose();
        }
    }

    fn is_composed(&self) -> bool {
        self.precomposed.borrow().is_some()
            || self.paused.borrow().as_ref().is_some_and(|paused| paused.is_complete())
    }

    /// Whether a step of `average` nanoseconds runs with `available` left, as `shouldExecute`.
    fn runs(&self, available: u64, average: u64) -> bool {
        available > if self.urgent.get() { 0 } else { average }
    }

    fn averages(&self, content_type: Option<u64>) -> Averages {
        let state = self.state.upgrade();
        state
            .and_then(|state| state.averages.borrow().get(&content_type).copied())
            .unwrap_or_default()
    }

    fn save(&self, content_type: Option<u64>, update: impl FnOnce(&mut Averages)) {
        if let Some(state) = self.state.upgrade() {
            update(state.averages.borrow_mut().entry(content_type).or_default());
        }
    }
}

impl PrefetchRequest for Request {
    fn execute(&self, scope: &mut PrefetchRequestScope<'_>) -> bool {
        let Some(provider) = self.provider.upgrade() else { return false };
        let source = &provider.source;
        if self.canceled.get() || self.index >= source.count() {
            self.clean_up();
            return false;
        }
        let key = source.key(self.index);
        if self.key.get().is_some_and(|composed| composed != key) {
            // Another item at its index now: the request is no longer valid.
            self.clean_up();
            return false;
        }
        let content_type = source.content_type(self.index);
        let averages = self.averages(content_type);
        let composed_at_start = self.is_composed();
        let elapsed = |since: std::time::Instant| {
            compose_ui::prefetch_now().saturating_duration_since(since).as_nanos() as u64
        };
        if !composed_at_start {
            if self.runs(scope.available_time_nanos(), averages.resume + averages.pause) {
                let paused = self
                    .paused
                    .borrow_mut()
                    .get_or_insert_with(|| {
                        self.key.set(Some(key));
                        Rc::new(provider.layout.create_paused_precomposition(
                            LazySlot { key, content_type },
                            source.content_of(self.index, key),
                        ))
                    })
                    .clone();
                let began = compose_ui::prefetch_now();
                let deadline = scope.deadline();
                let urgent = self.urgent.get();
                let needed = averages.resume + averages.pause;
                let complete = paused.resume(move || {
                    let left = deadline
                        .saturating_duration_since(compose_ui::prefetch_now())
                        .as_nanos() as u64;
                    left <= if urgent { 0 } else { needed }
                });
                let took = elapsed(began);
                self.save(content_type, |averages| {
                    if complete {
                        averages.resume = averaged(averages.resume, took);
                    } else {
                        averages.pause = averaged(averages.pause, took);
                    }
                });
            }
            if !self.is_composed() {
                return true;
            }
        }
        let paused = self.paused.borrow().clone();
        if let Some(paused) = paused {
            if !self.runs(scope.available_time_nanos(), averages.apply) {
                return true;
            }
            let began = compose_ui::prefetch_now();
            *self.precomposed.borrow_mut() = Some(paused.apply());
            *self.paused.borrow_mut() = None;
            let took = elapsed(began);
            self.save(content_type, |averages| averages.apply = averaged(averages.apply, took));
        }
        if let Some(constraints) = self.constraints
            && !self.measured.get()
        {
            if provider.pause_before_premeasure.get() && !composed_at_start {
                return true;
            }
            if !self.runs(scope.available_time_nanos(), averages.measure) {
                return true;
            }
            let began = compose_ui::prefetch_now();
            self.measured.set(true);
            let precomposed = self.precomposed.borrow();
            if let Some(handle) = precomposed.as_ref() {
                for placeable in 0..handle.placeables_count() {
                    handle.premeasure(placeable, constraints);
                }
            }
            let took = elapsed(began);
            self.save(content_type, |averages| averages.measure = averaged(averages.measure, took));
            let callback = self.on_item_premeasured.borrow_mut().take();
            if let (Some(callback), Some(handle)) = (callback, precomposed.as_ref()) {
                callback(&PrefetchResultScope { index: self.index, handle });
            }
        }
        false
    }
}
