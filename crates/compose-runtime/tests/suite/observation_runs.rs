//! What a platform object read by observed code (a graphics layer, D1337, D1339) tells its readers
//! apart by: an observation's runs, which run is current, and a state counted as initialized
//! wherever it's made.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_runtime::platform::*;
use compose_runtime::snapshots::{Snapshot, StateObjectId};
use compose_runtime::*;

/// The run observed now.
fn run_of(observer: &ReadObserver, key: u64) -> ObservationRun {
    observer.observe(key, || current_observation().expect("observing"))
}

/// A run is current until its key runs again; another key's run doesn't change that.
#[test]
fn a_run_is_current_until_its_key_runs_again() {
    let observer = ReadObserver::new(|| {});
    let first = run_of(&observer, 1);
    assert!(first.is_current());
    let other = run_of(&observer, 2);
    assert!(first.is_current() && other.is_current());
    let second = run_of(&observer, 1);
    assert_ne!(first, second);
    assert!(first.same_reader(&second) && !first.same_reader(&other));
    assert!(!first.is_current() && second.is_current());
}

/// A run in progress is current, and one nested in it under its key, or adding to it, continues
/// it.
#[test]
fn a_run_in_progress_is_current_and_continued_by_nested_runs_of_its_key() {
    let observer = ReadObserver::new(|| {});
    observer.observe(1, || {
        let run = current_observation().expect("observing");
        assert!(run.is_current());
        assert_eq!(run_of(&observer, 1), run, "nested under its key");
        assert!(run.is_current());
    });
    let run = run_of(&observer, 1);
    let added = observer.observe_adding(1, || current_observation().expect("observing"));
    assert_eq!(added, run);
    assert!(run.is_current());
}

/// A key forgotten (its node gone) has no current run, and its next run is another: its number
/// isn't the one its first run had (they don't restart when the key's count is dropped).
#[test]
fn a_forgotten_key_s_runs_are_not_current_and_its_next_runs_are_new() {
    let observer = ReadObserver::new(|| {});
    let first = run_of(&observer, 7);
    observer.forget(7);
    assert!(!first.is_current());
    let again = run_of(&observer, 7);
    assert_ne!(again, first);
    assert!(again.is_current() && !first.is_current());
}

/// An observer dropped leaves no run current, even once another takes its place and observes the
/// same key.
#[test]
fn a_dropped_observer_s_runs_are_not_current() {
    let observer = ReadObserver::new(|| {});
    let run = run_of(&observer, 3);
    drop(observer);
    assert!(!run.is_current());
    let next = ReadObserver::new(|| {});
    let other = run_of(&next, 3);
    assert_ne!(other, run);
    assert!(!run.is_current() && other.is_current());
}

/// No observation outside one, nor inside `without_read_observation` in one, where reads aren't
/// recorded under it: a read there is no drawing's (D1339).
#[test]
fn no_observation_where_reads_are_not_observed() {
    assert_eq!(current_observation(), None);
    let observer = ReadObserver::new(|| {});
    let state = mutable_state_of(0);
    let read = state.handle();
    observer.observe(1, || {
        let run = current_observation();
        assert!(run.is_some());
        Snapshot::without_read_observation(|| {
            assert_eq!(current_observation(), None, "reads aren't observed here");
            read.get();
            // An observation started inside observes again.
            let inner = observer.observe(2, current_observation);
            assert!(inner.is_some_and(|inner| !inner.same_reader(&run.unwrap())));
        });
        assert_eq!(current_observation(), run);
    });
    assert_eq!(current_observation(), None);
}

/// A state counted as initialized is changed by its first write in the global snapshot, where a
/// state made there isn't (as Compose's, until the snapshot advances).
#[test]
fn a_preexisting_state_s_first_write_in_the_global_snapshot_is_a_change() {
    let changed: Rc<RefCell<Vec<StateObjectId>>> = Rc::default();
    let heard = changed.clone();
    // Registering sends, advancing the global snapshot: the states are made after.
    let _observer = Snapshot::register_apply_observer(move |states, _| {
        heard.borrow_mut().extend(states.iter());
    });
    let (plain, preexisting) = (mutable_state_of(0), preexisting_state_of(0));
    plain.set(1);
    preexisting.set(1);
    Snapshot::send_apply_notifications();
    let changed = changed.borrow();
    assert!(!changed.contains(&plain.handle().state_object_id()), "its initialization");
    assert_eq!(changed.as_slice(), [preexisting.handle().state_object_id()]);
    assert_eq!(preexisting.get(), 1);
}

/// Making one sends nothing (taking a snapshot would send the global snapshot's changes), and
/// its first write is a change a reader of it hears.
#[test]
fn making_a_preexisting_state_sends_nothing_and_its_reader_hears_its_first_write() {
    let written = mutable_state_of(0);
    Snapshot::notify_objects_initialized();
    let sends = Rc::new(Cell::new(0));
    let counted = sends.clone();
    let _observer = Snapshot::register_apply_observer(move |_, _| counted.set(counted.get() + 1));
    written.set(1);
    let version = preexisting_state_of(0u64);
    assert_eq!(sends.get(), 0, "sent when made");
    let heard = Rc::new(Cell::new(false));
    let told = heard.clone();
    let observer = ReadObserver::new(move || told.set(true));
    let read = version.handle();
    observer.observe(1, || read.get());
    version.set(1);
    Snapshot::send_apply_notifications();
    assert_eq!(sends.get(), 1, "both writes in one send");
    assert!(heard.get(), "its reader told of its first write");
}

/// A state made in a snapshot counted as initialized: its write there is a change of the
/// snapshot, applied as one.
#[test]
fn a_preexisting_state_made_in_a_snapshot_is_changed_by_its_write_there() {
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    let state = snapshot.enter(|| {
        let state = preexisting_state_of(0);
        state.set(1);
        state
    });
    assert_eq!(state.get(), 0, "not applied yet");
    let id = state.handle().state_object_id();
    let changed = Rc::new(Cell::new(false));
    let seen = changed.clone();
    let _observer = Snapshot::register_apply_observer(move |states, _| {
        if states.contains(id) {
            seen.set(true);
        }
    });
    assert!(snapshot.apply().succeeded());
    snapshot.dispose();
    assert_eq!(state.get(), 1);
    assert!(changed.get(), "applied as a change");
}
