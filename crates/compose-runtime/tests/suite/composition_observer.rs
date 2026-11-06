//! Compose's `CompositionObserverTests`, adapted: `tooling::CompositionObserver` hears a
//! composition's passes, its scopes and their reads, invalidations and disposals (D1505).
//!
//! Compose runs some tests with each of its two composers (`_Gap`, `_Link`); compose-rs has one,
//! and one test cites both. A handle removes its observer when dropped, so the tests keep theirs.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use compose_runtime::snapshots::StateObjectId;
use compose_runtime::tooling::{
    CompositionObserver, CompositionObserverHandle, ObservableComposition,
};
use compose_runtime::*;

use crate::mock::*;

/// Counts the enters, exits and disposals of one scope, as the Kotlin file's
/// `SingleScopeObserver`.
#[derive(Clone, Default)]
struct SingleScope {
    target: Rc<Cell<Option<RecomposeScope>>>,
    starts: Rc<Cell<u32>>,
    ends: Rc<Cell<u32>>,
    disposals: Rc<Cell<u32>>,
}

impl SingleScope {
    fn counts(&self) -> (u32, u32, u32) {
        (self.starts.get(), self.ends.get(), self.disposals.get())
    }

    fn bump_if_target(&self, scope: &RecomposeScope, counter: &Cell<u32>) {
        if self.target.get() == Some(*scope) {
            counter.set(counter.get() + 1);
        }
    }
}

impl CompositionObserver for SingleScope {
    fn on_scope_enter(&self, scope: &RecomposeScope) {
        self.bump_if_target(scope, &self.starts);
    }

    fn on_scope_exit(&self, scope: &RecomposeScope) {
        self.bump_if_target(scope, &self.ends);
    }

    fn on_scope_disposed(&self, scope: &RecomposeScope) {
        self.bump_if_target(scope, &self.disposals);
    }
}

/// Keeps the reads it hears and counts scope events, as the Kotlin file's `ReadObserver`.
#[derive(Clone, Default)]
struct Reads {
    reads: Rc<RefCell<Vec<StateObjectId>>>,
    scope_events: Rc<Cell<u32>>,
}

impl CompositionObserver for Reads {
    fn on_scope_enter(&self, _: &RecomposeScope) {
        self.scope_events.set(self.scope_events.get() + 1);
    }

    fn on_scope_exit(&self, _: &RecomposeScope) {
        self.scope_events.set(self.scope_events.get() + 1);
    }

    fn on_read_in_scope(&self, _: &RecomposeScope, value: StateObjectId) {
        self.reads.borrow_mut().push(value);
    }
}

/// Counts begins and ends, and keeps the compositions begun.
#[derive(Clone, Default)]
struct Passes {
    begins: Rc<Cell<u32>>,
    ends: Rc<Cell<u32>>,
    seen: Rc<RefCell<Vec<ObservableComposition>>>,
}

impl Passes {
    fn counts(&self) -> (u32, u32) {
        (self.begins.get(), self.ends.get())
    }

    fn compositions_seen(&self) -> usize {
        self.seen.borrow().iter().collect::<HashSet<_>>().len()
    }
}

impl CompositionObserver for Passes {
    fn on_begin_composition(&self, composition: &ObservableComposition) {
        self.begins.set(self.begins.get() + 1);
        self.seen.borrow_mut().push(*composition);
    }

    fn on_end_composition(&self, _: &ObservableComposition) {
        self.ends.set(self.ends.get() + 1);
    }
}

/// A composition of its own composing `content` where it's called, at each pass of its caller,
/// as the Kotlin tests' `TestSubcomposition`.
#[composable]
fn TestSubcomposition(content: Rc<dyn Fn()>) {
    let context = remember_composition_context();
    let child = remember(move || Rc::new(Composition::with_parent(ViewApplier::new(), context)));
    child.set_content(move || content());
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeScope_Gap
// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeScope_Link
#[test]
fn a_scope_is_heard_entered_exited_and_disposed() {
    let observer = SingleScope::default();
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let d = data.handle();
    let scope: Rc<Cell<Option<RecomposeScope>>> = Rc::default();
    let s = scope.clone();
    test.compose(move || {
        s.set(Some(current_recompose_scope()));
        Text(&d.get().to_string());
    });
    test.validate(move |v| v.text(&d.get().to_string()));
    observer.target.set(scope.get());
    let _handle = test.composition().set_observer(observer.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    test.dispose();
    assert_eq!(observer.counts(), (1, 1, 1));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeScope_dispose_Gap
// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeScope_dispose_Link
#[test]
fn a_disposed_handle_hears_no_more() {
    let observer = SingleScope::default();
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let d = data.handle();
    let scope: Rc<Cell<Option<RecomposeScope>>> = Rc::default();
    let s = scope.clone();
    test.compose(move || {
        s.set(Some(current_recompose_scope()));
        Text(&d.get().to_string());
    });
    test.validate(move |v| v.text(&d.get().to_string()));
    observer.target.set(scope.get());
    let handle = test.composition().set_observer(observer.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    test.dispose();
    assert_eq!(observer.counts(), (1, 1, 0));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeScope_scopeRemoved_linkComposer
// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeScope_scopeRemoved_gapComposer
#[test]
fn a_scope_removed_is_heard_disposed() {
    let observer = SingleScope::default();
    let mut test = composition_test();
    let (data, visible) = (mutable_state_of(0), mutable_state_of(true));
    let (d, v) = (data.handle(), visible.handle());
    let scope: Rc<Cell<Option<RecomposeScope>>> = Rc::default();
    let s = scope.clone();
    test.compose(move || {
        if v.get() {
            let s = s.clone();
            Wrap!(move || {
                s.set(Some(current_recompose_scope()));
                Text(&d.get().to_string());
            });
        }
    });
    test.validate(move |e| {
        if v.get() {
            e.text(&d.get().to_string());
        }
    });
    observer.target.set(scope.get());
    let _handle = test.composition().set_observer(observer.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(observer.disposals.get(), 0);
    visible.set(false);
    test.expect_changes();
    test.revalidate();
    assert_eq!(observer.disposals.get(), 1);
    test.dispose();
    assert_eq!(observer.counts(), (1, 1, 1));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeComposition
#[test]
fn passes_are_heard_begun_and_ended() {
    let observer = Passes::default();
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let d = data.handle();
    let handle = test.compose_observed(observer.clone(), move || {
        Text(&format!("Some composition: {}", d.get()))
    });
    test.validate(move |v| v.text(&format!("Some composition: {}", d.get())));
    assert_eq!(observer.counts(), (1, 1));
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(observer.counts(), (2, 2));
    handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(observer.counts(), (2, 2));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeComposition_delayedStart
#[test]
fn an_observer_set_later_hears_the_passes_after() {
    let observer = Passes::default();
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let d = data.handle();
    test.compose(move || Text(&format!("Some composition: {}", d.get())));
    test.validate(move |v| v.text(&format!("Some composition: {}", d.get())));
    let handle = test.composition().set_observer(observer.clone());
    assert_eq!(observer.counts(), (0, 0));
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(observer.counts(), (1, 1));
    handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(observer.counts(), (1, 1));
}

/// A root reading `data` and a subcomposition reading it too, as the subcomposition tests compose.
fn root_and_subcomposition(data: MutableState<i32>, seen: Rc<Cell<i32>>) -> impl Fn() + 'static {
    move || {
        Text(&format!("Root: {}", data.get()));
        let seen = seen.clone();
        TestSubcomposition(Rc::new(move || seen.set(data.get())));
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeComposition_observeSubcompose
#[test]
fn an_observer_hears_the_compositions_under_its_own() {
    let observer = Passes::default();
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let seen = Rc::new(Cell::new(-1));
    let handle = test
        .compose_observed(observer.clone(), root_and_subcomposition(data.handle(), seen.clone()));
    assert_eq!(seen.get(), data.get());
    assert_eq!(observer.counts(), (2, 2));
    assert_eq!(observer.compositions_seen(), 2);
    data.update(|d| d + 1);
    test.expect_changes();
    let (begins, ends) = observer.counts();
    assert!(begins > 2);
    assert_eq!(begins, ends);
    handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!(observer.counts(), (begins, ends));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeComposition_observeSubcompose_deferred
#[test]
fn an_observer_set_later_hears_the_compositions_under_its_own() {
    let observer = Passes::default();
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let seen = Rc::new(Cell::new(-1));
    test.compose(root_and_subcomposition(data.handle(), seen.clone()));
    assert_eq!(seen.get(), data.get());
    assert_eq!(observer.counts(), (0, 0));
    let handle = test.composition().set_observer(observer.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    let (begins, ends) = observer.counts();
    assert!(begins > 0);
    assert_eq!(begins, ends);
    assert_eq!(observer.compositions_seen(), 2);
    handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!(observer.counts(), (begins, ends));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeComposition_observeSubcompose_shadowing
#[test]
fn a_subcompositions_own_observer_shadows_its_parents() {
    let (one, two) = (Passes::default(), Passes::default());
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let d = data.handle();
    let seen = Rc::new(Cell::new(-1));
    let s = seen.clone();
    test.compose(move || {
        Text(&format!("Root: {}", d.get()));
        let s = s.clone();
        TestSubcomposition(Rc::new(move || {
            s.set(d.get());
            let s = s.clone();
            TestSubcomposition(Rc::new(move || s.set(d.get())));
        }));
    });
    assert_eq!(seen.get(), data.get());
    assert_eq!(one.counts(), (0, 0));
    let root = test.composition().observable();
    let handle = root.set_observer(one.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    assert!(one.begins.get() > 0);
    assert_eq!(one.begins.get(), one.ends.get());
    assert_eq!(one.compositions_seen(), 3);
    // The first composition under the root.
    let sub = *one.seen.borrow().iter().find(|seen| **seen != root).expect("a subcomposition");
    let sub_handle = sub.set_observer(two.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    assert!(two.begins.get() > 0);
    assert_eq!(two.begins.get(), two.ends.get());
    let first_two = two.begins.get();
    let middle_one = one.begins.get();
    handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!(one.counts(), (middle_one, middle_one));
    assert!(two.begins.get() > first_two);
    assert_eq!(two.begins.get(), two.ends.get());
    let middle_two = two.begins.get();
    let handle = root.set_observer(one.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    assert!(one.begins.get() > middle_one);
    assert!(two.begins.get() > middle_two);
    let (penultimate_one, last_two) = (one.begins.get(), two.begins.get());
    sub_handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!(two.begins.get(), last_two);
    assert!(one.begins.get() >= penultimate_one + 3);
    let last_one = one.begins.get();
    handle.dispose();
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!((one.begins.get(), two.begins.get()), (last_one, last_two));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeComposition_observeSubcompose_replaced
#[test]
fn a_new_observer_replaces_the_old_one() {
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let seen = Rc::new(Cell::new(-1));
    test.compose(root_and_subcomposition(data.handle(), seen.clone()));
    let (first, second) = (Passes::default(), Passes::default());
    let _first = test.composition().set_observer(first.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!(seen.get(), data.get());
    assert_eq!(first.compositions_seen(), 2);
    let last_first = first.begins.get();
    let _second = test.composition().set_observer(second.clone());
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!(seen.get(), data.get());
    assert_eq!(first.begins.get(), last_first);
    assert_eq!(second.compositions_seen(), 2);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeComposition_insertMovableContent
#[test]
fn moving_content_into_a_subcomposition_is_heard_in_passes() {
    #[derive(Clone, Default)]
    struct Depth {
        depth: Rc<Cell<i32>>,
        outside: Rc<Cell<u32>>,
    }
    impl Depth {
        fn scope_event(&self) {
            if self.depth.get() == 0 {
                self.outside.set(self.outside.get() + 1);
            }
        }
    }
    impl CompositionObserver for Depth {
        fn on_begin_composition(&self, _: &ObservableComposition) {
            self.depth.set(self.depth.get() + 1);
        }
        fn on_end_composition(&self, _: &ObservableComposition) {
            self.depth.set(self.depth.get() - 1);
        }
        fn on_scope_enter(&self, _: &RecomposeScope) {
            self.scope_event();
        }
        fn on_scope_exit(&self, _: &RecomposeScope) {
            self.scope_event();
        }
    }
    let observer = Depth::default();
    let mut test = composition_test();
    let content = movable_content_of(|| Text("Movable"));
    let in_subcomposition = mutable_state_of(false);
    let inside = in_subcomposition.handle();
    let _handle = test.compose_observed(observer.clone(), move || {
        if !inside.get() {
            content();
        }
        let content = content.clone();
        TestSubcomposition(Rc::new(move || {
            if inside.get() {
                content();
            }
        }));
    });
    in_subcomposition.set(true);
    test.expect_changes();
    assert_eq!(observer.depth.get(), 0);
    assert_eq!(observer.outside.get(), 0);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, observeDataChanges
#[test]
fn what_changed_is_heard_entered_read_and_invalidated() {
    let mut test = composition_test();
    let data: Rc<[OwnedState<i32>]> = (0..4).map(|_| mutable_state_of(0)).collect();
    let handles: Vec<MutableState<i32>> = data.iter().map(|state| state.handle()).collect();
    let scopes: Rc<RefCell<[Option<RecomposeScope>; 4]>> = Rc::default();
    let (h, s) = (handles.clone(), scopes.clone());
    test.compose(move || {
        for (index, state) in h.iter().enumerate() {
            let (state, s) = (*state, s.clone());
            Wrap!(move || {
                Text(&format!("Data {}", state.get()));
                s.borrow_mut()[index] = Some(current_recompose_scope());
            });
        }
    });
    let h = handles.clone();
    test.validate(move |v| {
        for state in &h {
            v.text(&format!("Data {}", state.get()));
        }
    });
    let expected: Vec<RecomposeScope> = scopes.borrow().iter().map(|s| s.expect("scope")).collect();
    assert_eq!(expected.iter().collect::<HashSet<_>>().len(), 4);

    #[derive(Clone, Default)]
    struct Heard {
        entered: Rc<RefCell<Vec<RecomposeScope>>>,
        reads: Rc<RefCell<Vec<(RecomposeScope, StateObjectId)>>>,
        invalidations: Rc<RefCell<Vec<Invalidation>>>,
    }
    impl CompositionObserver for Heard {
        fn on_scope_enter(&self, scope: &RecomposeScope) {
            self.entered.borrow_mut().push(*scope);
        }
        fn on_read_in_scope(&self, scope: &RecomposeScope, value: StateObjectId) {
            self.reads.borrow_mut().push((*scope, value));
        }
        fn on_scope_invalidated(&self, scope: &RecomposeScope, value: Option<StateObjectId>) {
            self.invalidations.borrow_mut().push((*scope, value));
        }
    }
    let index_of = |scope: &RecomposeScope| {
        expected.iter().position(|e| e == scope).expect("one of the four scopes")
    };
    let mut changes = |indexes: &[usize]| {
        let heard = Heard::default();
        let handle = test.composition().set_observer(heard.clone());
        for &index in indexes {
            handles[index].update(|v| v + 1);
        }
        test.expect_changes();
        let entered: Vec<usize> = heard.entered.borrow().iter().map(index_of).collect();
        assert_eq!(entered, indexes);
        for (scope, value) in heard.reads.borrow().iter() {
            let index = index_of(scope);
            assert_eq!(*value, handles[index].state_object_id());
            assert!(indexes.contains(&index));
        }
        for (scope, value) in heard.invalidations.borrow().iter() {
            let index = index_of(scope);
            assert_eq!(*value, Some(handles[index].state_object_id()));
            assert!(indexes.contains(&index));
        }
        assert_eq!(heard.invalidations.borrow().len(), indexes.len());
        handle.dispose();
    };
    for indexes in [
        &[0][..],
        &[1],
        &[2],
        &[3],
        &[0, 1],
        &[0, 2],
        &[0, 3],
        &[1, 2],
        &[1, 3],
        &[2, 3],
        &[0, 1, 2],
        &[0, 1, 3],
        &[0, 2, 3],
        &[1, 2, 3],
        &[0, 1, 2, 3],
    ] {
        changes(indexes);
    }
    drop(data);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, nestedScopeObservations
#[test]
fn nested_scopes_are_heard_in_order() {
    #[derive(Clone, Default)]
    struct Log {
        lines: Rc<RefCell<Vec<String>>>,
        invalidations: Rc<RefCell<std::collections::HashMap<RecomposeScope, u32>>>,
    }
    impl Log {
        fn line(&self, line: String) {
            self.lines.borrow_mut().push(line);
        }
    }
    impl CompositionObserver for Log {
        fn on_begin_composition(&self, _: &ObservableComposition) {
            self.line("begin".into());
        }
        fn on_end_composition(&self, _: &ObservableComposition) {
            self.line("end".into());
        }
        fn on_scope_enter(&self, scope: &RecomposeScope) {
            match self.invalidations.borrow().get(scope) {
                Some(count) => self.line(format!("enter {count}")),
                None => self.line("enter".into()),
            }
        }
        fn on_scope_exit(&self, _: &RecomposeScope) {
            self.line("exit".into());
        }
        fn on_read_in_scope(&self, _: &RecomposeScope, _: StateObjectId) {
            self.line("read".into());
        }
        fn on_scope_disposed(&self, _: &RecomposeScope) {
            self.line("dispose".into());
        }
        fn on_scope_invalidated(&self, scope: &RecomposeScope, value: Option<StateObjectId>) {
            *self.invalidations.borrow_mut().entry(*scope).or_default() += 1;
            self.line(format!("invalidate {}", value.is_some()));
        }
    }
    /// Kotlin's `Wrapper`: a composable calling its content.
    #[composable]
    fn Wrapper(content: Rc<dyn Fn()>) {
        content();
    }
    let observer = Log::default();
    let mut test = composition_test();
    let state = mutable_state_of(String::from("text"));
    let s = state.handle();
    let _handle = test.compose_observed(observer.clone(), move || {
        Wrapper(Rc::new(move || Text(&s.get())));
    });
    test.validate(move |v| v.text(&s.get()));
    // Compose's three scopes are the content lambda's, `Wrapper`'s and its content lambda's: here
    // the root's and `Wrapper`'s (a closure argument isn't a restartable composable).
    assert_eq!(
        *observer.lines.borrow(),
        ["begin", "enter", "enter", "read", "exit", "exit", "end"]
    );
    observer.lines.borrow_mut().clear();
    state.set(String::from("text2"));
    test.advance();
    // Compose's value prints as `MutableState(value=text2)`: here, that a state was named.
    assert_eq!(
        *observer.lines.borrow(),
        ["invalidate true", "begin", "enter 1", "read", "exit", "end"]
    );
    test.revalidate();
}

/// A scope invalidated, by a change to a state or not.
type Invalidation = (RecomposeScope, Option<StateObjectId>);

/// A composable reading `first` and `second`, setting `observer` on the composition between them
/// while `set` is on (a plain flag, as Kotlin's `var`), as the Kotlin tests' content.
fn setting_in_composition(
    first: MutableState<i32>,
    second: MutableState<i32>,
    set: Rc<Cell<bool>>,
    composition: Rc<Cell<Option<ObservableComposition>>>,
    observer: Reads,
    handles: Rc<RefCell<Vec<CompositionObserverHandle>>>,
) -> impl Fn() + 'static {
    move || {
        Text(&first.get().to_string());
        if set.get()
            && let Some(composition) = composition.get()
        {
            handles.borrow_mut().push(composition.set_observer(observer.clone()));
        }
        Text(&second.get().to_string());
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, replaceObserverDuringComposition
#[test]
fn an_observer_replaced_during_a_pass_hears_that_pass_out() {
    let mut test = composition_test();
    let (first, second, replace) =
        (mutable_state_of(0), mutable_state_of(0), Rc::new(Cell::new(false)));
    let (first_observer, second_observer) = (Reads::default(), Reads::default());
    let composition: Rc<Cell<Option<ObservableComposition>>> = Rc::default();
    let handles: Rc<RefCell<Vec<CompositionObserverHandle>>> = Rc::default();
    test.compose(setting_in_composition(
        first.handle(),
        second.handle(),
        replace.clone(),
        composition.clone(),
        second_observer.clone(),
        handles.clone(),
    ));
    composition.set(Some(test.composition().observable()));
    let _first = test.composition().set_observer(first_observer.clone());
    let expected = [first.state_object_id(), second.state_object_id()];
    replace.set(true);
    first.update(|v| v + 1);
    second.update(|v| v + 1);
    test.expect_changes();
    assert_eq!(*first_observer.reads.borrow(), expected);
    assert!(second_observer.reads.borrow().is_empty());
    assert_eq!(second_observer.scope_events.get(), 0);
    replace.set(false);
    first.update(|v| v + 1);
    second.update(|v| v + 1);
    test.expect_changes();
    assert_eq!(*first_observer.reads.borrow(), expected);
    assert_eq!(*second_observer.reads.borrow(), expected);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionObserverTests.kt, setObserverDuringComposition
#[test]
fn an_observer_set_during_a_pass_hears_the_next() {
    let mut test = composition_test();
    let (first, second, set) =
        (mutable_state_of(0), mutable_state_of(0), Rc::new(Cell::new(false)));
    let observer = Reads::default();
    let composition: Rc<Cell<Option<ObservableComposition>>> = Rc::default();
    let handles: Rc<RefCell<Vec<CompositionObserverHandle>>> = Rc::default();
    test.compose(setting_in_composition(
        first.handle(),
        second.handle(),
        set.clone(),
        composition.clone(),
        observer.clone(),
        handles.clone(),
    ));
    composition.set(Some(test.composition().observable()));
    set.set(true);
    first.update(|v| v + 1);
    second.update(|v| v + 1);
    test.expect_changes();
    assert!(observer.reads.borrow().is_empty());
    assert_eq!(observer.scope_events.get(), 0);
    set.set(false);
    first.update(|v| v + 1);
    second.update(|v| v + 1);
    test.expect_changes();
    assert_eq!(*observer.reads.borrow(), [first.state_object_id(), second.state_object_id()]);
}

// ---- Beyond the ports ---------------------------------------------------------------------------

/// A composition dropped with its observer set, its handle dropped after: nothing's logged any
/// more, however much another composition recomposes.
#[test]
fn a_composition_dropped_with_its_observer_leaves_no_log() {
    use compose_runtime::tooling::logged_scope_events;
    for dispose_first in [true, false] {
        let observed = Composition::new(ViewApplier::new());
        observed.set_content(|| Text("observed"));
        let handle = observed.set_observer(Passes::default());
        if dispose_first {
            observed.dispose();
        }
        drop(observed);
        drop(handle);
        let mut test = composition_test();
        let data = mutable_state_of(0);
        let d = data.handle();
        test.compose(move || Text(&d.get().to_string()));
        for _ in 0..100 {
            data.update(|d| d + 1);
            test.expect_changes();
        }
        assert_eq!(logged_scope_events(), None, "dispose first: {dispose_first}");
    }
}

/// An observed composition and a busy unobserved one: what the busy one logs is let go at each
/// pass, so the log stays small.
#[test]
fn an_unobserved_composition_doesnt_grow_the_log() {
    use compose_runtime::tooling::logged_scope_events;
    let observed = Composition::new(ViewApplier::new());
    observed.set_content(|| Text("observed"));
    let _handle = observed.set_observer(Passes::default());
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let d = data.handle();
    test.compose(move || {
        Wrap!(move || Text(&d.get().to_string()));
    });
    for _ in 0..100 {
        data.update(|d| d + 1);
        test.expect_changes();
    }
    assert!(logged_scope_events().is_some_and(|logged| logged <= 2), "{:?}", logged_scope_events());
}

/// What happens in a pass, and in its changes, goes to the observer as the pass began, as
/// Kotlin's `pin()`: a scope removed in a pass that set another one's disposal is the first's.
/// An invalidation logged before an observer is replaced goes to the one there was.
#[test]
fn a_passs_observer_is_pinned_through_it() {
    #[derive(Clone, Default)]
    struct Heard {
        disposals: Rc<Cell<u32>>,
        invalidations: Rc<Cell<u32>>,
    }
    impl CompositionObserver for Heard {
        fn on_scope_disposed(&self, _: &RecomposeScope) {
            self.disposals.set(self.disposals.get() + 1);
        }
        fn on_scope_invalidated(&self, _: &RecomposeScope, _: Option<StateObjectId>) {
            self.invalidations.set(self.invalidations.get() + 1);
        }
    }
    let (first, second) = (Heard::default(), Heard::default());
    let mut test = composition_test();
    let (show, replace) = (mutable_state_of(true), Rc::new(Cell::new(false)));
    let composition: Rc<Cell<Option<ObservableComposition>>> = Rc::default();
    let handles: Rc<RefCell<Vec<CompositionObserverHandle>>> = Rc::default();
    let (s, r, c, h, o) =
        (show.handle(), replace.clone(), composition.clone(), handles.clone(), second.clone());
    test.compose(move || {
        if r.get()
            && let Some(composition) = c.get()
        {
            h.borrow_mut().push(composition.set_observer(o.clone()));
            r.set(false);
        }
        if s.get() {
            Wrap!(|| Text("shown"));
        }
    });
    composition.set(Some(test.composition().observable()));
    let _first = test.composition().set_observer(first.clone());
    replace.set(true);
    show.set(false);
    test.expect_changes();
    assert!(first.disposals.get() >= 1, "the removed scope's disposal is the pinned observer's");
    assert_eq!(second.disposals.get(), 0);
    // Logged, not told yet; replaced: the old one hears it.
    let third = Heard::default();
    show.set(true);
    compose_runtime::snapshots::Snapshot::send_apply_notifications();
    let invalidated_before = second.invalidations.get();
    let _third = test.composition().set_observer(third.clone());
    assert!(second.invalidations.get() > invalidated_before, "told before the replacement");
    assert_eq!(third.invalidations.get(), 0);
    test.expect_changes();
}

/// A subcomposition's reads in a pass inside its observed parent's are heard once, as the
/// subcomposition's: the parent's frame tells only its own.
#[test]
fn a_subcompositions_reads_are_heard_once() {
    let observer = Reads::default();
    let mut test = composition_test();
    let state = mutable_state_of(0);
    let s = state.handle();
    let _handle = test.compose_observed(observer.clone(), move || {
        TestSubcomposition(Rc::new(move || {
            s.get();
        }));
    });
    let heard = observer.reads.borrow();
    assert_eq!(
        heard.iter().filter(|read| **read == state.state_object_id()).count(),
        1,
        "{heard:?}"
    );
}

/// The first handle dropped after a replacement leaves the second observer.
#[test]
fn an_old_handle_leaves_the_new_observer() {
    let (first, second) = (Passes::default(), Passes::default());
    let mut test = composition_test();
    let data = mutable_state_of(0);
    let d = data.handle();
    test.compose(move || Text(&d.get().to_string()));
    let first_handle = test.composition().set_observer(first.clone());
    let _second = test.composition().set_observer(second.clone());
    drop(first_handle);
    data.update(|d| d + 1);
    test.expect_changes();
    assert_eq!(second.counts(), (1, 1));
    assert_eq!(first.counts(), (0, 0));
}
