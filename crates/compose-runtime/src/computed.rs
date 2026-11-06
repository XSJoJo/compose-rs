//! `computed_state_of`: a state whose value is its calculation's, as Compose's `computedStateOf`
//! (D1504).
//!
//! Read with nothing observing it, it's calculated at each read, as it is: no record, no cache,
//! what the calculation reads heard by the snapshot read observers around it, and in a derived
//! state's calculation (or another computed state's) what it reads is that calculation's
//! dependencies. Read where the read registers (a composable, a `SnapshotStateObserver`'s
//! observation), it's a derived state, the cell being one: cached, validated, its readers
//! invalidated only if its value changed by its policy. Its calculation can't write state: it runs
//! read-only, as Compose's in `Snapshot.readOnly`.

use std::panic::Location;
use std::rc::Rc;

use crate::remember::Remember;
use crate::snapshots::Snapshot;
use crate::state::{MutationPolicy, OwnedState, State, StateId};

/// Reads computed state `id`: as a derived state's where the read registers, else calculated.
#[track_caller]
pub(crate) fn read<T: 'static, R>(id: StateId, f: impl FnOnce(&T) -> R) -> R {
    if crate::state::read_is_tracked() {
        return crate::derived::read(id, f);
    }
    let cell = crate::state::object_cell(id, Location::caller());
    // As Compose's value getter: its read observers hear it, then what the calculation reads.
    crate::snapshots::notify_read_if_observed(id);
    f(&crate::derived::calculate::<T>(&cell))
}

/// Owns a computed state: dropping it retires its cell, as a [`DerivedState`]'s.
/// `remember(|| computed_state_of(…))` keeps it in the slot and returns its `Copy` [`State`]
/// handle.
///
/// [`DerivedState`]: crate::DerivedState
pub struct ComputedState<T: 'static> {
    owner: OwnedState<T>,
}

#[track_caller]
fn new_computed<T: 'static>(
    equivalent: fn(&T, &T) -> bool,
    calculation: impl Fn() -> T + 'static,
) -> ComputedState<T> {
    let calculation = Rc::new(calculation);
    let read_only = move || Snapshot::with_read_only(|| calculation());
    ComputedState {
        owner: crate::derived::new_cell(Some(equivalent), equivalent, Box::new(read_only), true),
    }
}

/// A state whose value is `calculation`'s, as Compose's `computedStateOf`: calculated at each read
/// unless something observes it, when its readers are invalidated only if the value changes
/// (`==`, as `structuralEqualityPolicy`). Writing state in `calculation` panics.
#[track_caller]
pub fn computed_state_of<T: PartialEq + 'static>(
    calculation: impl Fn() -> T + 'static,
) -> ComputedState<T> {
    new_computed(<T as PartialEq>::eq, calculation)
}

/// Like [`computed_state_of`], with a mutation policy deciding when the value changed, as
/// `computedStateOf(policy, calculation)`.
#[track_caller]
pub fn computed_state_of_with<T: 'static, P: MutationPolicy<T>>(
    policy: P,
    calculation: impl Fn() -> T + 'static,
) -> ComputedState<T> {
    new_computed(crate::derived::equivalence_of(&policy), calculation)
}

impl<T: 'static> ComputedState<T> {
    /// The value: calculated now, or the cached one where reads are observed and nothing it read
    /// changed. Reading it registers the reader, as reading any state does.
    #[track_caller]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.as_state().get()
    }

    #[track_caller]
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.as_state().with(f)
    }

    /// The `Copy` read-only handle of the value.
    pub fn as_state(&self) -> State<T> {
        State::of_computed(self.owner.handle())
    }
}

impl<T: 'static> Remember for ComputedState<T> {
    type Stored = ComputedState<T>;
    type Output = State<T>;

    fn into_stored(self) -> ComputedState<T> {
        self
    }

    fn output(stored: &ComputedState<T>) -> State<T> {
        stored.as_state()
    }
}

impl<T: 'static> std::fmt::Debug for ComputedState<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "ComputedState({:?})", self.as_state())
    }
}
