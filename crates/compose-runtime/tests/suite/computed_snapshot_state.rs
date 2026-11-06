//! Compose's `ComputedSnapshotStateTests`, adapted: `computed_state_of` outside composition runs
//! its calculation at each read, in whatever snapshot reads it (D1504).

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use compose_runtime::snapshots::{Snapshot, SnapshotStateObserver, StateObjectId};
use compose_runtime::*;

/// Validates at its end, as the Kotlin file's `validateOpenSnapshots`.
struct OpenSnapshots(usize);

fn open_snapshots() -> OpenSnapshots {
    OpenSnapshots(Snapshot::open_snapshot_count())
}

impl Drop for OpenSnapshots {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert_eq!(self.0, Snapshot::open_snapshot_count(), "a snapshot wasn't disposed");
        }
    }
}

/// A run counter a calculation bumps.
fn counter() -> Rc<Cell<u32>> {
    Rc::default()
}

fn bump(runs: &Cell<u32>) {
    runs.set(runs.get() + 1);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, aStateFreeCalculationCanBeUsed
#[test]
fn a_calculation_reading_no_state_gives_its_value() {
    let _open = open_snapshots();
    assert_eq!(computed_state_of(|| 10).get(), 10);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, theCalculationIsRecomputedEachInvocation
#[test]
fn the_calculation_runs_at_each_read() {
    let _open = open_snapshots();
    let (runs, i) = (counter(), Rc::new(Cell::new(0)));
    let (r, value) = (runs.clone(), i.clone());
    let a = computed_state_of(move || {
        bump(&r);
        value.get()
    });
    assert_eq!(runs.get(), 0, "the calculation runs only when the value is asked for");
    i.set(1);
    assert_eq!(a.get(), 1, "what it reads now");
    assert_eq!(runs.get(), 1, "once a read");
    i.set(2);
    assert_eq!(a.get(), 2, "what it reads now, though that isn't state");
    assert_eq!(runs.get(), 2, "once a read");
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, statesCanBeUsedInGlobalSnapshot
#[test]
fn it_follows_the_states_it_reads() {
    let _open = open_snapshots();
    let (a, b) = (mutable_state_of(1), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let c = computed_state_of(move || ah.get() + bh.get());
    assert_eq!(c.get(), 11);
    a.update(|a| a + 1);
    assert_eq!(c.get(), 12);
    b.update(|b| b + 10);
    assert_eq!(c.get(), 22);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, statesCanBeUsedInSnapshot
#[test]
fn it_follows_the_states_it_reads_in_a_snapshot() {
    let _open = open_snapshots();
    let (a, b) = (mutable_state_of(1), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let c = computed_state_of(move || ah.get() + bh.get());
    // Kotlin takes it without entering it: what it checks reads the global snapshot.
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    assert_eq!(c.get(), 11);
    a.update(|a| a + 1);
    assert_eq!(c.get(), 12);
    b.update(|b| b + 10);
    assert_eq!(c.get(), 22);
    snapshot.dispose();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, snapshotsAreIsolatedFromGlobalChanges
#[test]
fn a_snapshot_reads_its_own_view() {
    let _open = open_snapshots();
    let state = mutable_state_of(0);
    let read = state.handle();
    let computed = computed_state_of(move || read.get());
    let snapshot = Snapshot::take_snapshot(None);
    state.set(1);
    assert_eq!(state.get(), 1);
    assert_eq!(computed.get(), 1);
    assert_eq!(snapshot.enter(|| state.get()), 0);
    assert_eq!(snapshot.enter(|| computed.get()), 0);
    snapshot.dispose();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, mutableSnapshotsCanBeApplied
#[test]
fn what_a_mutable_snapshot_wrote_is_read_once_applied() {
    let _open = open_snapshots();
    let state = mutable_state_of(0);
    let read = state.handle();
    let computed = computed_state_of(move || read.get());
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    snapshot.enter(|| {
        assert_eq!((state.get(), computed.get()), (0, 0));
        state.set(1);
        assert_eq!((state.get(), computed.get()), (1, 1));
    });
    assert_eq!((state.get(), computed.get()), (0, 0));
    snapshot.apply().check();
    assert_eq!((state.get(), computed.get()), (1, 1));
    snapshot.dispose();
    Snapshot::with_mutable_snapshot(|| {
        assert_eq!((state.get(), computed.get()), (1, 1));
        state.set(2);
        assert_eq!((state.get(), computed.get()), (2, 2));
    });
    assert_eq!((state.get(), computed.get()), (2, 2));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, multipleSnapshotsAreIsolatedAndCanBeApplied
#[test]
fn snapshots_are_isolated_from_each_other_and_apply() {
    let _open = open_snapshots();
    const COUNT: usize = 2;
    let states: Vec<_> = (0..COUNT).map(|_| mutable_state_of(0)).collect();
    let computed: Vec<_> = states
        .iter()
        .map(|state| {
            let read = state.handle();
            computed_state_of(move || read.get())
        })
        .collect();
    let snapshots: Vec<_> =
        (0..COUNT).map(|_| Snapshot::take_mutable_snapshot(None, None)).collect();
    for index in 0..COUNT {
        assert_eq!((states[index].get(), computed[index].get()), (0, 0));
    }
    for (index, snapshot) in snapshots.iter().enumerate() {
        snapshot.enter(|| states[index].set(index));
    }
    for index in 0..COUNT {
        assert_eq!((states[index].get(), computed[index].get()), (0, 0));
    }
    for (index, snapshot) in snapshots.iter().enumerate() {
        snapshot.enter(|| {
            for other in 0..COUNT {
                let expected = if other == index { other } else { 0 };
                assert_eq!((states[other].get(), computed[other].get()), (expected, expected));
            }
        });
    }
    for snapshot in &snapshots {
        snapshot.apply().check();
    }
    for index in 0..COUNT {
        assert_eq!((states[index].get(), computed[index].get()), (index, index));
    }
    for snapshot in snapshots {
        snapshot.dispose();
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, stateReadsCanBeObserved
#[test]
fn its_reads_and_it_are_heard() {
    let _open = open_snapshots();
    let state = mutable_state_of(0);
    let read = state.handle();
    let computed = computed_state_of(move || read.get());
    let heard: Rc<RefCell<Vec<StateObjectId>>> = Rc::default();
    let log = heard.clone();
    let snapshot =
        Snapshot::take_snapshot(Some(Rc::new(move |state| log.borrow_mut().push(state))));
    assert_eq!(snapshot.enter(|| computed.get()), 0);
    let heard = heard.borrow();
    assert_eq!(heard.len(), 2);
    assert!(heard.contains(&state.state_object_id()));
    assert!(heard.contains(&computed.as_state().state_object_id()));
    snapshot.dispose();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, nestedComputedStatesProduceAResult
#[test]
fn nested_computed_states_give_a_result() {
    let _open = open_snapshots();
    const N: i32 = 3;
    let state = mutable_state_of(1);
    let read = state.handle();
    let first: Rc<Vec<State<i32>>> =
        Rc::new((0..N).map(|_| remember_outside(computed_state_of(move || read.get()))).collect());
    let second: Rc<Vec<State<i32>>> = Rc::new(
        (0..N)
            .map(|_| {
                let first = first.clone();
                remember_outside(computed_state_of(move || first.iter().map(State::get).sum()))
            })
            .collect(),
    );
    let third = computed_state_of(move || second.iter().map(State::get).sum::<i32>());
    assert_eq!(third.get(), N * N);
    state.set(2);
    assert_eq!(third.get(), N * N * 2);
}

/// Keeps a computed state alive for the test's thread, giving its handle: as Kotlin's lists of
/// states, which the garbage collector keeps.
fn remember_outside<T: 'static>(state: ComputedState<T>) -> State<T> {
    let handle = state.as_state();
    std::mem::forget(state);
    handle
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, calculationIsExecutedOnEveryReadEvenIfUnchanged
#[test]
fn the_calculation_runs_at_each_read_though_nothing_changed() {
    let _open = open_snapshots();
    let runs = counter();
    let state = mutable_state_of(0);
    let (r, read) = (runs.clone(), state.handle());
    let computed = computed_state_of(move || {
        bump(&r);
        read.get()
    });
    assert_eq!(runs.get(), 0);
    for expected in 1..=3 {
        assert_eq!(computed.get(), 0);
        assert_eq!(runs.get(), expected);
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, nullResultIsEvaluatedEveryTime
#[test]
fn a_none_result_runs_each_time_too() {
    let _open = open_snapshots();
    let runs = counter();
    let r = runs.clone();
    let computed = computed_state_of(move || {
        bump(&r);
        None::<i32>
    });
    assert_eq!(computed.get(), None);
    assert_eq!(runs.get(), 1);
    assert_eq!(computed.get(), None);
    assert_eq!(runs.get(), 2);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, snapshotReadOnlyDoesNotUpdateRecord
#[test]
fn a_read_in_a_read_only_snapshot_works() {
    let _open = open_snapshots();
    let state = mutable_state_of(0);
    let read = state.handle();
    let computed = computed_state_of(move || read.get());
    let snapshot = Snapshot::take_snapshot(None);
    assert_eq!(snapshot.enter(|| computed.get()), 0);
    snapshot.dispose();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, multipleReadsInSameSnapshotWithDifferentValuesSetMultipleUniqueValues
#[test]
fn reads_in_one_snapshot_see_its_writes() {
    let _open = open_snapshots();
    let state = mutable_state_of(1);
    let read = state.handle();
    let computed = computed_state_of(move || read.get());
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    snapshot.enter(|| {
        assert_eq!(computed.get(), 1);
        state.set(2);
        assert_eq!(computed.get(), 2);
    });
    snapshot.dispose();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, customMutationPolicyStructuralEquality
#[test]
fn a_structural_policy() {
    let _open = open_snapshots();
    let state = mutable_state_of(vec!["a"]);
    let read = state.handle();
    let computed = computed_state_of_with(structural_equality_policy(), move || read.get());
    assert_eq!(computed.get(), ["a"]);
    state.set(vec!["a"]);
    assert_eq!(computed.get(), ["a"]);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, customMutationPolicyReferentialEquality
#[test]
fn a_referential_policy() {
    let _open = open_snapshots();
    let (first, second) = (Rc::new(String::from("test")), Rc::new(String::from("test")));
    let state = mutable_state_of_with(first.clone(), referential_equality_policy());
    let read = state.handle();
    let computed = computed_state_of_with(referential_equality_policy(), move || read.get());
    assert!(Rc::ptr_eq(&computed.get(), &first));
    state.set(second.clone());
    assert!(Rc::ptr_eq(&computed.get(), &second));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, customMutationPolicyNeverEqual
#[test]
fn a_never_equal_policy() {
    let _open = open_snapshots();
    let state = mutable_state_of(10);
    let read = state.handle();
    let computed = computed_state_of_with(never_equal_policy(), move || read.get());
    assert_eq!(computed.get(), 10);
    state.set(10);
    assert_eq!(computed.get(), 10);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, derivedStateOfReadingComputedState
#[test]
fn a_derived_state_reading_a_computed_one() {
    let _open = open_snapshots();
    let (computed_runs, derived_runs) = (counter(), counter());
    let state = mutable_state_of(0);
    let (c, read) = (computed_runs.clone(), state.handle());
    let computed = computed_state_of(move || {
        bump(&c);
        read.get()
    })
    .into_state_kept();
    let d = derived_runs.clone();
    let derived = derived_state_of(move || {
        bump(&d);
        computed.get() * 2
    });
    assert_eq!((computed_runs.get(), derived_runs.get()), (0, 0));
    assert_eq!(derived.get(), 0);
    assert_eq!((computed_runs.get(), derived_runs.get()), (1, 1));
    assert_eq!(derived.get(), 0);
    assert_eq!((computed_runs.get(), derived_runs.get()), (1, 1));
    state.set(5);
    assert_eq!(derived.get(), 10);
    assert_eq!((computed_runs.get(), derived_runs.get()), (2, 2));
}

/// `as_state`, the owner kept for the thread: what Kotlin's closures capture, the collector keeps.
trait Kept<T: 'static> {
    fn into_state_kept(self) -> State<T>;
}

impl<T: 'static> Kept<T> for ComputedState<T> {
    fn into_state_kept(self) -> State<T> {
        remember_outside(self)
    }
}

impl<T: 'static> Kept<T> for DerivedState<T> {
    fn into_state_kept(self) -> State<T> {
        let handle = self.as_state();
        std::mem::forget(self);
        handle
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, computedStateOfReadingDerivedState
#[test]
fn a_computed_state_reading_a_derived_one() {
    let _open = open_snapshots();
    let (derived_runs, computed_runs) = (counter(), counter());
    let state = mutable_state_of(0);
    let (d, read) = (derived_runs.clone(), state.handle());
    let derived = derived_state_of(move || {
        bump(&d);
        read.get() + 10
    })
    .into_state_kept();
    let c = computed_runs.clone();
    let computed = computed_state_of(move || {
        bump(&c);
        derived.get()
    });
    assert_eq!(computed.get(), 10);
    assert_eq!((derived_runs.get(), computed_runs.get()), (1, 1));
    assert_eq!(computed.get(), 10);
    assert_eq!((derived_runs.get(), computed_runs.get()), (1, 2));
    state.set(1);
    assert_eq!(computed.get(), 11);
    assert_eq!((derived_runs.get(), computed_runs.get()), (2, 3));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, computedStateOfReadingNestedDerivedState
#[test]
fn a_computed_state_reading_a_derived_and_a_computed_one() {
    let _open = open_snapshots();
    let (derived_runs, inner_runs, outer_runs) = (counter(), counter(), counter());
    let (a, b) = (mutable_state_of(1), mutable_state_of(2));
    let (d, ah) = (derived_runs.clone(), a.handle());
    let derived = derived_state_of(move || {
        bump(&d);
        ah.get() * 10
    })
    .into_state_kept();
    let (i, bh) = (inner_runs.clone(), b.handle());
    let inner = computed_state_of(move || {
        bump(&i);
        bh.get() * 100
    })
    .into_state_kept();
    let o = outer_runs.clone();
    let outer = computed_state_of(move || {
        bump(&o);
        derived.get() + inner.get()
    });
    let runs = || (derived_runs.get(), inner_runs.get(), outer_runs.get());
    assert_eq!(outer.get(), 210);
    assert_eq!(runs(), (1, 1, 1));
    assert_eq!(outer.get(), 210);
    assert_eq!(runs(), (1, 2, 2));
    a.set(2);
    assert_eq!(outer.get(), 220);
    assert_eq!(runs(), (2, 3, 3));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, derivedStateOfReadingNestedComputedState
#[test]
fn a_derived_state_reading_a_computed_and_a_derived_one() {
    let _open = open_snapshots();
    let (computed_runs, derived_runs, outer_runs) = (counter(), counter(), counter());
    let (a, b, c) = (mutable_state_of(1), mutable_state_of(2), mutable_state_of(3));
    let (r, ah) = (computed_runs.clone(), a.handle());
    let computed = computed_state_of(move || {
        bump(&r);
        ah.get() * 10
    })
    .into_state_kept();
    let (d, bh) = (derived_runs.clone(), b.handle());
    let derived = derived_state_of(move || {
        bump(&d);
        bh.get() * 100
    })
    .into_state_kept();
    let (o, ch) = (outer_runs.clone(), c.handle());
    let outer = derived_state_of(move || {
        bump(&o);
        computed.get() + derived.get() + ch.get()
    });
    let runs = || (computed_runs.get(), derived_runs.get(), outer_runs.get());
    assert_eq!(outer.get(), 213);
    assert_eq!(runs(), (1, 1, 1));
    assert_eq!(outer.get(), 213);
    assert_eq!(runs(), (1, 1, 1));
    a.set(2);
    assert_eq!(outer.get(), 223);
    assert_eq!(runs(), (2, 1, 2));
    b.set(3);
    assert_eq!(outer.get(), 323);
    assert_eq!(runs(), (3, 2, 3));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, toStringDoesNotObserveReads
#[test]
fn display_observes_no_read() {
    let _open = open_snapshots();
    let state = mutable_state_of(42);
    let read = state.handle();
    let computed = computed_state_of(move || read.get());
    let heard = Rc::new(Cell::new(0));
    let count = heard.clone();
    let snapshot = Snapshot::take_snapshot(Some(Rc::new(move |_| bump(&count))));
    let shown = snapshot.enter(|| computed.as_state().to_string());
    assert!(shown.contains("42"), "{shown}");
    assert_eq!(heard.get(), 0, "Display records no read");
    snapshot.dispose();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, exceptionInCalculationIsNotStateRecordMutated
#[test]
fn a_panicking_calculation_leaves_nothing_behind() {
    let _open = open_snapshots();
    let should_panic = Rc::new(Cell::new(true));
    let state = mutable_state_of(1);
    let (panics, read) = (should_panic.clone(), state.handle());
    let computed = computed_state_of(move || {
        assert!(!panics.get(), "Calculation failed");
        read.get()
    });
    assert!(catch_unwind(AssertUnwindSafe(|| computed.get())).is_err());
    should_panic.set(false);
    assert_eq!(computed.get(), 1);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, writingStateInComputationBlockThrows
#[test]
fn writing_state_in_the_calculation_panics() {
    let _open = open_snapshots();
    let state = mutable_state_of(0);
    let write = state.handle();
    let computed = computed_state_of(move || write.set(1));
    assert!(catch_unwind(AssertUnwindSafe(|| computed.get())).is_err());
    assert_eq!(state.get(), 0);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/snapshots/ComputedSnapshotStateTests.kt, writingStateInInvalidationCheckThrows
#[test]
fn writing_state_in_the_check_of_an_observation_panics() {
    let _open = open_snapshots();
    let observer = SnapshotStateObserver::new(|run| run());
    observer.start();
    let (trigger, state) = (mutable_state_of(0), mutable_state_of(0));
    let write_in_calculation = Rc::new(Cell::new(false));
    let (writes, read, write) = (write_in_calculation.clone(), trigger.handle(), state.handle());
    let computed = computed_state_of(move || {
        if writes.get() {
            write.set(1);
        }
        read.get()
    });
    Snapshot::notify_objects_initialized();
    let on_change: Rc<dyn Fn(&())> = Rc::new(|_| {});
    observer.observe_reads((), &on_change, || computed.get());
    write_in_calculation.set(true);
    let checked = catch_unwind(AssertUnwindSafe(|| {
        trigger.set(1);
        Snapshot::send_apply_notifications();
    }));
    assert!(checked.is_err());
    observer.stop();
}
