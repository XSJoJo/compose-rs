//! Observing compositions, as Compose's `tooling.CompositionObserver` (experimental there, D1505).
//!
//! An observer set on a composition hears its passes, and those of the compositions under it that
//! have none of their own: each pass's begin and end, the scopes it runs and the state they read,
//! and its scopes invalidated and disposed. A pass's observer is the one there as it begins, so one
//! set during a pass hears the next. Nothing is recorded while no composition has an observer.
//!
//! Scope events happen in the runtime's borrow, where an observer can't run: they're logged there
//! and told outside it, in their order, before the next read a pass reports, at a pass's begin and
//! end, and at a disposal. An invalidation is told by then: at the latest, the pass it causes.
//!
//! Behind the `composition-observer` feature, off by default: without it a pass takes no step for
//! observers at all (D1505).

use std::marker::PhantomData;
use std::rc::Rc;

use crate::runtime::{CompositionId, Compositions, Runtime, try_with_runtime, with_runtime};
use crate::scope::{RecomposeScope, ScopeId};
use crate::snapshots::StateObjectId;
use crate::state::StateId;

/// Hears a composition's passes and scopes, as Compose's `CompositionObserver`. Every method does
/// nothing unless overridden.
pub trait CompositionObserver: 'static {
    /// A pass of `composition` begins, as `onBeginComposition`.
    fn on_begin_composition(&self, _composition: &ObservableComposition) {}

    /// A pass runs `scope`'s composable, as `onScopeEnter`: it may still skip its body.
    fn on_scope_enter(&self, _scope: &RecomposeScope) {}

    /// `scope` read `value`, as `onReadInScope`: each read, though the scope had read it.
    fn on_read_in_scope(&self, _scope: &RecomposeScope, _value: StateObjectId) {}

    /// The pass is done with `scope`'s composable, as `onScopeExit`.
    fn on_scope_exit(&self, _scope: &RecomposeScope) {}

    /// The pass of `composition` ends, as `onEndComposition`, before its changes apply.
    fn on_end_composition(&self, _composition: &ObservableComposition) {}

    /// `scope` was invalidated by a change to `value`, or by `RecomposeScope::invalidate` (`None`),
    /// as `onScopeInvalidated`.
    fn on_scope_invalidated(&self, _scope: &RecomposeScope, _value: Option<StateObjectId>) {}

    /// `scope` left its composition, as `onScopeDisposed`.
    fn on_scope_disposed(&self, _scope: &RecomposeScope) {}
}

/// A composition an observer can be set on, as Compose's `ObservableComposition`: what
/// [`CompositionObserver::on_begin_composition`] names, a child composition among them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ObservableComposition {
    pub(crate) id: CompositionId,
}

impl ObservableComposition {
    /// Sets the composition's observer, replacing any, as `setObserver`: it hears this composition
    /// and those under it that have none of their own. The handle removes it, disposed or dropped,
    /// if it's still the one set.
    pub fn set_observer(&self, observer: impl CompositionObserver) -> CompositionObserverHandle {
        let observer: Rc<dyn CompositionObserver> = Rc::new(observer);
        let (token, replaced) = with_runtime(|runtime| {
            let token = runtime.scopes.observer_set();
            let replaced = match runtime.compositions.get_mut(self.id) {
                Some(data) => data.observer.replace((token, observer)),
                None => Some((token, observer)),
            };
            if replaced.is_some() {
                runtime.scopes.observer_removed();
            }
            (token, replaced)
        });
        // Outside the borrow: an observer's `Drop` is user code.
        drop(replaced);
        CompositionObserverHandle { composition: self.id, token, _not_send: PhantomData }
    }
}

/// Removes an observer [`ObservableComposition::set_observer`] set, as Compose's
/// `CompositionObserverHandle`: disposed, or dropped.
pub struct CompositionObserverHandle {
    composition: CompositionId,
    token: u64,
    _not_send: PhantomData<*const ()>,
}

impl CompositionObserverHandle {
    /// Removes the observer, if it's still the one set, as `dispose`.
    pub fn dispose(self) {}
}

impl Drop for CompositionObserverHandle {
    fn drop(&mut self) {
        // What was logged for it is told first.
        flush();
        let removed = try_with_runtime(|runtime| {
            let data = runtime.compositions.get_mut(self.composition)?;
            if data.observer.as_ref().is_none_or(|(token, _)| *token != self.token) {
                return None;
            }
            let removed = data.observer.take();
            runtime.scopes.observer_removed();
            removed
        });
        drop(removed);
    }
}

/// What a scope event is.
#[derive(Clone, Copy)]
pub(crate) enum ScopeEventKind {
    Enter,
    Exit,
    Invalidated(Option<StateId>),
    Disposed,
}

/// A scope event, logged in the runtime's borrow, told outside it.
pub(crate) struct ScopeEvent {
    pub composition: u32,
    pub scope: ScopeId,
    pub kind: ScopeEventKind,
    /// The observer it goes to, when its composition can't say any more (it's being disposed).
    pub observer: Option<Rc<dyn CompositionObserver>>,
}

/// The observer of the composition at `index`: its own, or the nearest above it.
pub(crate) fn observer_of(
    compositions: &Compositions,
    mut index: u32,
) -> Option<Rc<dyn CompositionObserver>> {
    loop {
        let data = compositions.data_at(index)?;
        if let Some((_, observer)) = &data.observer {
            return Some(observer.clone());
        }
        index = compositions.contexts.parent_of(data.parent?)?.index;
    }
}

/// The events logged, each with its observer: a pass's scopes go to its observer, the rest to
/// their composition's.
fn settle(runtime: &mut Runtime) -> Vec<(Rc<dyn CompositionObserver>, ScopeEvent)> {
    let Some(log) = runtime.scopes.log.as_mut() else { return Vec::new() };
    if log.is_empty() {
        return Vec::new();
    }
    let events = std::mem::take(log);
    let compositions = &runtime.compositions;
    let settled = events
        .into_iter()
        .filter_map(|event| {
            let observer = match (&event.observer, event.kind) {
                (Some(observer), _) => Some(observer.clone()),
                (None, ScopeEventKind::Enter | ScopeEventKind::Exit) => compositions
                    .data_at(event.composition)
                    .and_then(|data| data.pass_observer.clone()),
                (None, _) => observer_of(compositions, event.composition),
            };
            observer.map(|observer| (observer, event))
        })
        .collect();
    runtime.scopes.stop_logging_if_unobserved();
    settled
}

/// Tells what's logged, outside the runtime's borrow.
pub(crate) fn flush() {
    let events = try_with_runtime(settle).unwrap_or_default();
    for (observer, event) in events {
        let scope = RecomposeScope::of(event.scope);
        match event.kind {
            ScopeEventKind::Enter => observer.on_scope_enter(&scope),
            ScopeEventKind::Exit => observer.on_scope_exit(&scope),
            ScopeEventKind::Invalidated(value) => {
                observer.on_scope_invalidated(&scope, value.map(StateObjectId))
            }
            ScopeEventKind::Disposed => observer.on_scope_disposed(&scope),
        }
    }
}

/// Gives what's logged of the composition at `index`, being disposed, its observer now.
pub(crate) fn tag_disposed(
    runtime: &mut Runtime,
    index: u32,
    observer: Rc<dyn CompositionObserver>,
) {
    if let Some(log) = runtime.scopes.log.as_mut() {
        for event in log.iter_mut().filter(|event| event.composition == index) {
            event.observer.get_or_insert_with(|| observer.clone());
        }
    }
}

/// The read observer a pass's observer hears its scopes' reads through: what registers with a
/// scope of its composition, not what a calculation reads.
pub(crate) fn read_observer(
    composition: CompositionId,
    observer: Rc<dyn CompositionObserver>,
) -> crate::snapshots::StateObserverFn {
    Rc::new(move |value| {
        flush();
        if let Some(scope) = crate::state::scope_reading_in(composition) {
            observer.on_read_in_scope(&RecomposeScope::of(scope), value);
        }
    })
}

/// A pass of `id` begins: its observer, the one there now, if any, hears it, and what its scopes
/// read. With none on the thread, one test.
pub(crate) fn begin_pass(
    id: CompositionId,
    snapshot: &mut Option<crate::snapshots::PassSnapshot>,
) -> Option<Rc<dyn CompositionObserver>> {
    let observer = with_runtime(|runtime| {
        if !runtime.scopes.observing() {
            return None;
        }
        let observer = observer_of(&runtime.compositions, id.index)?;
        if let Some(data) = runtime.compositions.get_mut(id) {
            data.pass_observer = Some(observer.clone());
        }
        if let Some(active) = &mut runtime.active {
            active.observed = true;
        }
        Some(observer)
    })?;
    flush();
    observer.on_begin_composition(&ObservableComposition { id });
    if let Some(snapshot) = snapshot {
        snapshot.push_read_frame(read_observer(id, observer.clone()));
    }
    Some(observer)
}

/// A pass of `id` ends its composing: its observer hears it. Gives the observer back, to drop
/// once the runtime's borrow is over.
pub(crate) fn end_pass(
    id: CompositionId,
    observer: &mut Option<Rc<dyn CompositionObserver>>,
) -> Option<Rc<dyn CompositionObserver>> {
    let observer = observer.take()?;
    flush();
    observer.on_end_composition(&ObservableComposition { id });
    Some(observer)
}

/// The observer the scopes of `id`, about to be disposed, go to.
pub(crate) fn disposing(
    runtime: &Runtime,
    id: CompositionId,
) -> Option<Rc<dyn CompositionObserver>> {
    runtime.scopes.observing().then(|| observer_of(&runtime.compositions, id.index)).flatten()
}
