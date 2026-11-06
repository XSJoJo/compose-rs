//! A recomposer's compositions registered and unregistered, as Compose's
//! `CompositionRegistrationObserver` and `Recomposer.observe` (D1510).
//!
//! Compose's recomposer keeps a list of the compositions it knows; here a composition is the
//! recomposer's while it's under it and has begun its first composition (its root group is
//! there), isn't poisoned or closed, and the recomposer isn't shutting down. So nothing is kept
//! for registration: a composition's first pass, a pass that panics, a disposal and a shutdown
//! tell the observers, and only while the thread has one (`OBSERVERS`), from outside the
//! runtime's borrow.

use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use super::{CompositionObserverHandle, ObservableComposition, Observed};
use crate::Recomposer;
use crate::runtime::{CompositionData, CompositionId, with_runtime};
use crate::slot_table::{NONE, SENTINEL};

/// Hears the compositions a recomposer registers and unregisters, as Compose's
/// `CompositionRegistrationObserver`, through [`Recomposer::observe`].
pub trait CompositionRegistrationObserver: 'static {
    /// `composition` is the recomposer's: its first composition begins, before its content runs,
    /// as `onCompositionRegistered`. An observer is told of those there already when it's set.
    fn on_composition_registered(&self, composition: &ObservableComposition);

    /// `composition` is the recomposer's no more, as `onCompositionUnregistered`: it's disposed, a
    /// pass of it panicked, or the recomposer shut down.
    fn on_composition_unregistered(&self, composition: &ObservableComposition);
}

thread_local! {
    /// Registration observers set on the thread's recomposers: with none, nothing looks.
    static OBSERVERS: Cell<usize> = const { Cell::new(0) };
    static NEXT_TOKEN: Cell<u64> = const { Cell::new(0) };
}

fn observing() -> bool {
    OBSERVERS.with(Cell::get) > 0
}

/// Whether `data` is registered, its recomposer taking registrations.
fn known(data: &CompositionData) -> bool {
    !data.closed && !data.poisoned && !data.disposed && data.table[SENTINEL].first_child != NONE
}

/// The recomposer `id` is under.
fn recomposer_of(id: CompositionId) -> Option<Recomposer> {
    let given = with_runtime(|runtime| runtime.compositions.get(id).map(|data| data.recomposer))?;
    Some(given.unwrap_or_else(Recomposer::current_thread))
}

/// The compositions registered with `recomposer`, in the order they were made.
fn registered_with(recomposer: Recomposer) -> Vec<CompositionId> {
    let is_default = recomposer.is_thread_default();
    with_runtime(|runtime| {
        let compositions = &runtime.compositions;
        let mut ids: Vec<(u64, CompositionId)> = (compositions.live_ids().into_iter())
            .filter_map(|id| {
                let data = compositions.get(id)?;
                (known(data) && recomposer.holds_composition(data, is_default))
                    .then_some((data.created, id))
            })
            .collect();
        ids.sort_unstable_by_key(|(created, _)| *created);
        ids.into_iter().map(|(_, id)| id).collect()
    })
}

/// The observers of a recomposer, each with its handle's token.
type Observers = Vec<(u64, Rc<dyn CompositionRegistrationObserver>)>;

/// Tells `observers` of `recomposer`, each only while its handle is there: one an earlier one's
/// call, or what ran since the list was taken, disposed isn't told.
fn tell(recomposer: Recomposer, observers: &Observers, id: CompositionId, registered: bool) {
    let composition = ObservableComposition::of(id);
    for (token, observer) in observers {
        if !recomposer.has_registration_observer(*token) {
            continue;
        }
        if registered {
            observer.on_composition_registered(&composition);
        } else {
            observer.on_composition_unregistered(&composition);
        }
    }
}

/// `Recomposer::observe`.
pub(crate) fn observe_registrations(
    recomposer: Recomposer,
    observer: Rc<dyn CompositionRegistrationObserver>,
) -> CompositionObserverHandle {
    let token = NEXT_TOKEN.with(|next| {
        next.set(next.get() + 1);
        next.get()
    });
    recomposer.add_registration_observer(token, observer.clone());
    OBSERVERS.with(|count| count.set(count.get() + 1));
    let handle = CompositionObserverHandle {
        observed: Observed::Recomposer(recomposer, token),
        _not_send: PhantomData,
    };
    if recomposer.takes_registrations() {
        for id in registered_with(recomposer) {
            tell(recomposer, &vec![(token, observer.clone())], id, true);
        }
    }
    handle
}

/// The handle of `token` on `recomposer` is gone.
pub(crate) fn stop(recomposer: Recomposer, token: u64) {
    let removed = recomposer.remove_registration_observer(token);
    if removed.is_some() {
        OBSERVERS.with(|count| count.set(count.get() - 1));
    }
    drop(removed);
}

/// A pass of `id` in `Mode::Normal` has begun, as Compose's `composeInitial`: its first, before
/// its content runs, registers it.
pub(crate) fn registering(id: CompositionId) {
    if !observing() {
        return;
    }
    let first = with_runtime(|runtime| {
        runtime
            .compositions
            .get(id)
            .is_some_and(|data| !data.poisoned && data.table[SENTINEL].first_child == NONE)
    });
    if !first {
        return;
    }
    let Some(recomposer) = recomposer_of(id) else { return };
    if recomposer.takes_registrations() {
        tell(recomposer, &recomposer.registration_observers(), id, true);
    }
}

/// `id` is about to be disposed, or poisoned by a pass unwinding: the observers to tell it's
/// unregistered, if it's registered, for [`unregistered`].
pub(crate) fn unregistering(id: CompositionId) -> Option<Unregistering> {
    if !observing() {
        return None;
    }
    let registered = with_runtime(|runtime| runtime.compositions.get(id).is_some_and(known));
    if !registered {
        return None;
    }
    let recomposer = recomposer_of(id)?;
    if !recomposer.takes_registrations() {
        return None;
    }
    Some(Unregistering { id, recomposer, observers: recomposer.registration_observers() })
}

/// What [`unregistering`] found, told once the composition is disposed or poisoned: those of its
/// observers whose handles are still there then (a disposal runs `on_dispose`s, which may drop
/// one).
pub(crate) struct Unregistering {
    id: CompositionId,
    recomposer: Recomposer,
    observers: Observers,
}

impl Unregistering {
    pub(crate) fn tell(self) {
        tell(self.recomposer, &self.observers, self.id, false);
    }
}

/// `recomposer` is shutting down: its compositions are its no more.
pub(crate) fn recomposer_shut_down(recomposer: Recomposer) {
    if !observing() {
        return;
    }
    let observers = recomposer.registration_observers();
    if observers.is_empty() {
        return;
    }
    for id in registered_with(recomposer) {
        tell(recomposer, &observers, id, false);
    }
}
