//! Compose's `CompositionAndComputedStateTests`, adapted: `computed_state_of` read in composition
//! invalidates only on a changed result, by its policy, as a derived state does (D1504).
//!
//! As in `composition_and_derived_state.rs`, two writes in a row are seen together by the next
//! pass, as Compose's `withMutableSnapshot` makes them. Where a composable reads it, a computed
//! state is cached as a derived state is: counts of its calculations are a derived state's, and
//! say so where they differ from Compose's.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_runtime::snapshots::Snapshot;
use compose_runtime::*;

use crate::mock::*;

#[composable]
fn DisplayItem(name: &'static str, state: State<i32>) {
    Text(&format!("{name} = {}", state.get()));
}

#[composable]
fn DisplayIndirect(name: &'static str, state: State<i32>) {
    DisplayItem(name, state);
}

/// `Display(vararg names)`: a new list each call, as Kotlin's varargs, so it always runs.
#[composable]
fn Display(names: Rc<[(&'static str, State<i32>)]>) {
    for (name, state) in names.iter() {
        DisplayIndirect(name, *state);
    }
}

/// Keeps a computed (or derived) state for the test's thread, as Kotlin's closures keep theirs.
fn kept<S: Into<Kept<T>>, T: 'static>(state: S) -> State<T> {
    state.into().0
}

struct Kept<T: 'static>(State<T>);

impl<T: 'static> From<ComputedState<T>> for Kept<T> {
    fn from(state: ComputedState<T>) -> Kept<T> {
        let handle = state.as_state();
        std::mem::forget(state);
        Kept(handle)
    }
}

impl<T: 'static> From<DerivedState<T>> for Kept<T> {
    fn from(state: DerivedState<T>) -> Kept<T> {
        let handle = state.as_state();
        std::mem::forget(state);
        Kept(handle)
    }
}

/// The sum of `a` and `b`, computed.
fn sum(a: MutableState<i32>, b: MutableState<i32>) -> State<i32> {
    kept(computed_state_of(move || a.get() + b.get()))
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateOfChangedOutputInvalidates
#[test]
fn a_changed_result_invalidates_its_readers() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(31), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let answer = sum(ah, bh);
    test.compose(move || Text(&format!("Sum is {}", answer.get())));
    test.validate(move |v| v.text(&format!("Sum is {}", ah.get() + bh.get())));
    a.update(|a| a + 1);
    test.expect_changes();
    b.update(|b| b + 1);
    test.expect_changes();
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateOfChangedInputsSameOutputsDoesNotInvalidate
#[test]
fn changed_inputs_with_the_same_result_invalidate_nothing() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(31), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let answer = sum(ah, bh);
    let invocations = Counter::new();
    let i = invocations.clone();
    test.compose(move || {
        i.increment();
        Text(&format!("Sum is {}", answer.get()));
    });
    test.validate(move |v| v.text(&format!("Sum is {}", ah.get() + bh.get())));
    assert_eq!(invocations.get(), 1, "Text should compose exactly once");
    a.update(|a| a + 1);
    b.update(|b| b - 1);
    // Compose asks `hasPendingWork` here: the condition is checked by the pass here (D711).
    test.expect_no_changes();
    assert!(!test.composition().has_invalidations(), "nothing left to do");
    assert_eq!(invocations.get(), 1, "Text should compose exactly once");
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateOfChangedInCompositionInvalidatesAllUsages
#[test]
fn a_change_reaches_every_reader() {
    let mut test = composition_test();
    let count = mutable_state_of(0);
    let ch = count.handle();
    let computed = kept(computed_state_of(move || ch.get() * 10));
    test.compose(move || {
        Linear!(move || {
            Text(&format!("First: {}", computed.get()));
            Text(&format!("Second: {}", computed.get()));
        })
    });
    test.validate(|v| {
        v.linear(|v| {
            v.text("First: 0");
            v.text("Second: 0");
        })
    });
    count.update(|c| c + 1);
    test.expect_changes();
    test.validate(|v| {
        v.linear(|v| {
            v.text("First: 10");
            v.text("Second: 10");
        })
    });
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, observingComputedStateInMultipleScopes
#[test]
fn a_computed_state_read_in_two_scopes_follows_the_one_left() {
    #[composable]
    fn Content(observe_in_first: MutableState<bool>, count: MutableState<i32>) {
        let items =
            remember(move || computed_state_of(move || (0..count.get()).collect::<Vec<_>>()));
        Linear!({
            if observe_in_first.get() {
                Text(&format!("List of size {}", items.with(Vec::len)));
            }
        });
        Linear!({
            Text(&format!("List of size {}", items.with(Vec::len)));
        });
    }
    let mut test = composition_test();
    let (observe_in_first, count) = (mutable_state_of(true), mutable_state_of(0));
    let (o, c) = (observe_in_first.handle(), count.handle());
    test.compose(move || Content(o, c));
    test.validate(|v| {
        v.linear(|v| v.text("List of size 0"));
        v.linear(|v| v.text("List of size 0"));
    });
    observe_in_first.set(false);
    test.advance();
    count.update(|c| c + 1);
    test.advance();
    test.validate(|v| {
        v.empty_linear();
        v.linear(|v| v.text("List of size 1"));
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, writingToAComputedStateDependencyTriggersAForwardInvalidate
#[test]
fn writing_a_dependency_in_composition_invalidates_forward() {
    #[composable]
    fn Content(a: MutableState<i32>, b: MutableState<i32>, c: State<i32>) {
        DisplayIndirect("c", c);
        let d = remember(|| mutable_state_of(0));
        d.set(a.get() + b.get());
        let e = remember(move || computed_state_of(move || a.get() + b.get() + d.get()));
        DisplayIndirect("e", e);
    }
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(12), mutable_state_of(30));
    let (ah, bh) = (a.handle(), b.handle());
    let c = sum(ah, bh);
    test.compose(move || Content(ah, bh, c));
    test.validate(move |v| {
        let (a, b) = (ah.get(), bh.get());
        v.text(&format!("c = {}", a + b));
        v.text(&format!("e = {}", a + b + a + b));
    });
    a.update(|a| a + 1);
    test.expect_changes();
    test.revalidate();
    b.update(|b| b - 1);
    test.expect_changes();
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, observingBothNormalAndComputedInSameScope
#[test]
fn a_scope_reading_state_and_computed_state_follows_both() {
    #[composable]
    fn Content(b: State<bool>, c: MutableState<bool>) {
        Linear!({
            if b.get() {
                Text("B is true");
            }
            if c.get() {
                Text("C is true");
            }
        });
    }
    let mut test = composition_test();
    let (a, c) = (mutable_state_of(0), mutable_state_of(false));
    let ah = a.handle();
    let b = kept(computed_state_of(move || ah.get() > 0));
    let ch = c.handle();
    test.compose(move || Content(b, ch));
    let expected = move |v: &mut Expected| {
        v.linear(|v| {
            if b.get() {
                v.text("B is true");
            }
            if ch.get() {
                v.text("C is true");
            }
        })
    };
    test.validate(expected);
    a.update(|a| a + 1);
    test.expect_changes();
    test.revalidate();
    c.set(true);
    test.advance();
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, changingTheComputedStateInstanceShouldRelease
#[test]
fn a_new_instance_releases_the_old_one() {
    let mut test = composition_test();
    let reload = mutable_state_of(0);
    let r = reload.handle();
    let instances: Rc<std::cell::RefCell<Vec<State<Vec<i32>>>>> = Rc::default();
    let seen = instances.clone();
    test.compose(move || {
        let items = remember_keyed(r.get(), || computed_state_of(|| (0..10).collect::<Vec<_>>()));
        seen.borrow_mut().push(items);
        Text(&format!("List of size {}", items.with(Vec::len)));
    });
    test.validate(|v| v.text("List of size 10"));
    for _ in 0..10 {
        reload.update(|r| r + 1);
        test.advance();
    }
    test.revalidate();
    // Compose counts the composition's observed objects; here, the instances left are freed.
    assert_old_instances_freed(&instances.borrow());
    let last = instances.borrow()[instances.borrow().len() - 1];
    assert_eq!(last.try_get().map(|items| items.len()), Some(10));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, onlyEvaluateComputedStatesThatAreLive
#[test]
fn a_computed_state_no_longer_read_isnt_calculated() {
    let mut test = composition_test();
    let (a, use_d) = (mutable_state_of(11), mutable_state_of(true));
    let ah = a.handle();
    let calculated = Counter::new();
    let counter = calculated.clone();
    let d = kept(computed_state_of(move || {
        counter.increment();
        ah.get()
    }));
    let u = use_d.handle();
    test.compose(move || {
        if u.get() {
            Text(&format!("d = {}", d.get()));
        } else {
            Text("idle");
        }
    });
    test.validate(move |v| v.text(&format!("d = {}", ah.get())));
    use_d.set(false);
    test.expect_changes();
    test.validate(|v| v.text("idle"));
    let before = calculated.get();
    a.update(|a| a + 1);
    test.expect_no_changes();
    test.revalidate();
    assert_eq!(calculated.get(), before, "nobody reads it");
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateOfNestedChangesInvalidate
#[test]
fn nested_computed_states_invalidate_their_readers() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(31), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let transient = sum(ah, bh);
    let answer = kept(computed_state_of(move || transient.get() - 1));
    test.compose(move || Text(&format!("The answer is {}", answer.get())));
    test.validate(move |v| v.text(&format!("The answer is {}", ah.get() + bh.get() - 1)));
    a.update(|a| a + 1);
    test.expect_changes();
    b.update(|b| b + 1);
    test.expect_changes();
    test.revalidate();
}

/// Composes what `answer` gives through `remember_updated_state` in a `Linear`, counting the
/// compositions of the `Linear`'s content.
#[composable]
fn Remembered(answer: State<Vec<bool>>, compositions: Counter) {
    let remembered = remember_updated_state(Rc::new(answer.get()));
    Linear!({
        compositions.increment();
        Text(&format!("The answer is {:?}", remembered.get()));
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateOfReferentialMutationPolicyRecomposes
#[test]
fn a_new_result_by_the_referential_policy_recomposes() {
    #[composable]
    fn Content(answer: State<Rc<Vec<bool>>>, compositions: Counter) {
        let remembered = remember_updated_state(answer.get());
        Linear!({
            compositions.increment();
            Text(&format!("The answer is {:?}", remembered.get()));
        });
    }
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(30), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    // Kotlin's `listOf` is a new list each time: a new `Rc` here.
    let answer = kept(computed_state_of_with(referential_equality_policy(), move || {
        Rc::new(vec![ah.get() >= 30, bh.get() >= 10])
    }));
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || Content(answer, c.clone()));
    test.validate(|v| v.linear(|v| v.text("The answer is [true, true]")));
    assert_eq!(compositions.get(), 1);
    a.update(|a| a + 1);
    // An equal list, but a new one: its readers recompose, and change no view.
    test.expect_no_changes();
    assert_eq!(compositions.get(), 2);
    b.update(|b| b + 1);
    test.expect_no_changes();
    assert_eq!(compositions.get(), 3);
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateOfStructuralMutationPolicyDoesNotRecompose
#[test]
fn an_equal_result_by_the_structural_policy_recomposes_nothing() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(30), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let answer = kept(computed_state_of_with(structural_equality_policy(), move || {
        vec![ah.get() >= 30, bh.get() >= 10]
    }));
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || Remembered(answer, c.clone()));
    test.validate(|v| v.linear(|v| v.text("The answer is [true, true]")));
    assert_eq!(compositions.get(), 1);
    a.update(|a| a + 1);
    test.expect_no_changes();
    assert_eq!(compositions.get(), 1);
    b.update(|b| b + 1);
    test.expect_no_changes();
    assert_eq!(compositions.get(), 1);
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateInvalidatesAfterUnchanged
#[test]
fn an_unchanged_result_then_a_changed_one() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(10), mutable_state_of(20));
    let (ah, bh) = (a.handle(), b.handle());
    let total = sum(ah, bh);
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || {
        c.increment();
        Text(&format!("Sum: {}", total.get()));
    });
    test.validate(move |v| v.text(&format!("Sum: {}", ah.get() + bh.get())));
    assert_eq!(compositions.get(), 1, "composed at the start");
    a.update(|a| a + 1);
    b.update(|b| b - 1);
    test.expect_no_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 1);
    a.update(|a| a + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 2);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateInvalidatesTwice
#[test]
fn two_changes_in_a_row_recompose_twice() {
    let mut test = composition_test();
    let a = mutable_state_of(1);
    let ah = a.handle();
    let doubled = kept(computed_state_of(move || ah.get() * 2));
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || {
        c.increment();
        Text(&format!("Value is {}", doubled.get()));
    });
    test.validate(move |v| v.text(&format!("Value is {}", 2 * ah.get())));
    for expected in [2, 3] {
        a.update(|a| a + 1);
        test.expect_changes();
        test.revalidate();
        assert_eq!(compositions.get(), expected);
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, derivedStateReadingComputedStateInComposition
#[test]
fn a_remembered_derived_state_reading_a_computed_one() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(10), mutable_state_of(20));
    let (ah, bh) = (a.handle(), b.handle());
    let computed = sum(ah, bh);
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || {
        let derived = remember(|| derived_state_of(move || computed.get() * 2));
        c.increment();
        Text(&format!("Value is {}", derived.get()));
    });
    test.validate(move |v| v.text(&format!("Value is {}", (ah.get() + bh.get()) * 2)));
    assert_eq!(compositions.get(), 1);
    for (state, expected) in [(&a, 2), (&b, 3)] {
        state.update(|value| value + 1);
        test.expect_changes();
        test.revalidate();
        assert_eq!(compositions.get(), expected);
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateReadingDerivedStateInComposition
#[test]
fn a_remembered_computed_state_reading_a_derived_one() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(10), mutable_state_of(20));
    let (ah, bh) = (a.handle(), b.handle());
    let derived = kept(derived_state_of(move || ah.get() + bh.get()));
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || {
        let computed = remember(|| computed_state_of(move || derived.get() * 2));
        c.increment();
        Text(&format!("Value is {}", computed.get()));
    });
    test.validate(move |v| v.text(&format!("Value is {}", (ah.get() + bh.get()) * 2)));
    assert_eq!(compositions.get(), 1);
    for (state, expected) in [(&a, 2), (&b, 3)] {
        state.update(|value| value + 1);
        test.expect_changes();
        test.revalidate();
        assert_eq!(compositions.get(), expected);
    }
}

/// `a`, `b` and a positivity test of their sum through `positive`, composed, counting compositions.
fn positivity_test(positive: impl Fn(State<i32>) -> State<bool> + 'static, computed_sum: bool) {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(10), mutable_state_of(20));
    let (ah, bh) = (a.handle(), b.handle());
    let total: State<i32> = if computed_sum {
        sum(ah, bh)
    } else {
        kept(derived_state_of(move || ah.get() + bh.get()))
    };
    let compositions = Counter::new();
    let c = compositions.clone();
    let positive = Rc::new(positive);
    test.compose(move || {
        let positive = positive.clone();
        let is = remember(move || HandleOnly(positive(total)));
        c.increment();
        Text(&format!("Is positive: {}", is.get()));
    });
    test.validate(move |v| v.text(&format!("Is positive: {}", ah.get() + bh.get() > 0)));
    assert_eq!(compositions.get(), 1);
    a.update(|a| a + 1);
    test.expect_no_changes();
    assert_eq!(compositions.get(), 1);
    a.set(-100);
    b.set(0);
    test.expect_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 2);
}

/// A `State` to remember as itself: what `positivity_test`'s factory made is kept already.
struct HandleOnly<T: 'static>(State<T>);

impl<T: 'static> Remember for HandleOnly<T> {
    type Stored = State<T>;
    type Output = State<T>;

    fn into_stored(self) -> State<T> {
        self.0
    }

    fn output(stored: &State<T>) -> State<T> {
        *stored
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateReadingDerivedStateSkips
#[test]
fn a_computed_state_reading_a_derived_one_skips_an_equal_result() {
    positivity_test(|total| kept(computed_state_of(move || total.get() > 0)), false);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, derivedStateReadingComputedStateSkips
#[test]
fn a_derived_state_reading_a_computed_one_skips_an_equal_result() {
    positivity_test(|total| kept(derived_state_of(move || total.get() > 0)), true);
}

/// `if cond then a + b else c` through `pick`, composed: changing what it reads without changing
/// its value recomposes nothing.
fn changing_dependencies_test(
    pick: impl Fn(MutableState<bool>, State<i32>, MutableState<i32>) -> State<i32>,
    sum_of: impl Fn(MutableState<i32>, MutableState<i32>) -> State<i32>,
) {
    let mut test = composition_test();
    let (a, b, c) = (mutable_state_of(10), mutable_state_of(20), mutable_state_of(30));
    let condition = mutable_state_of(true);
    let value = pick(condition.handle(), sum_of(a.handle(), b.handle()), c.handle());
    let compositions = Counter::new();
    let counted = compositions.clone();
    test.compose(move || {
        counted.increment();
        Text(&format!("Value is {}", value.get()));
    });
    test.validate(|v| v.text("Value is 30"));
    assert_eq!(compositions.get(), 1);
    condition.set(false);
    test.expect_no_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 1);
    a.set(30);
    test.expect_no_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 1);
    c.set(0);
    test.expect_changes();
    test.validate(|v| v.text("Value is 0"));
    assert_eq!(compositions.get(), 2);
    Snapshot::with_mutable_snapshot(|| {
        condition.set(true);
        a.set(-20);
    });
    test.expect_no_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 2);
    a.set(0);
    test.expect_changes();
    test.validate(|v| v.text("Value is 20"));
    assert_eq!(compositions.get(), 3);
}

fn picking_computed(
    condition: MutableState<bool>,
    sum: State<i32>,
    c: MutableState<i32>,
) -> State<i32> {
    kept(computed_state_of(move || if condition.get() { sum.get() } else { c.get() }))
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateChangesDependencies
#[test]
fn a_computed_state_changing_what_it_reads() {
    // Kotlin's `a + b` is in the calculation itself: a computed sum is transparent, the same.
    changing_dependencies_test(picking_computed, sum);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateChangesDependenciesBetweenDerivedAndDirectStates
#[test]
fn a_computed_state_changing_between_a_derived_state_and_a_state() {
    changing_dependencies_test(picking_computed, |a, b| {
        kept(derived_state_of(move || a.get() + b.get()))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, derivedStateChangesDependenciesBetweenComputedAndDirectStates
#[test]
fn a_derived_state_changing_between_a_computed_state_and_a_state() {
    changing_dependencies_test(
        |condition, sum, c| {
            kept(derived_state_of(move || if condition.get() { sum.get() } else { c.get() }))
        },
        sum,
    );
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, changingTheComputedStateInstanceShouldClearDependencies
#[test]
fn a_new_instance_leaves_no_reader_behind() {
    // Compose counts the composition's observed objects (the reload state and the last
    // instance); here, every old instance is freed, and a reload recomposes once.
    let mut test = composition_test();
    let reload = mutable_state_of(0);
    let r = reload.handle();
    let compositions = Counter::new();
    let c = compositions.clone();
    let instances: Rc<std::cell::RefCell<Vec<State<Vec<i32>>>>> = Rc::default();
    let seen = instances.clone();
    test.compose(move || {
        c.increment();
        let items = remember_keyed(r.get(), || computed_state_of(|| (0..10).collect::<Vec<_>>()));
        seen.borrow_mut().push(items);
        Text(&format!("List of size {}", items.with(Vec::len)));
    });
    test.validate(|v| v.text("List of size 10"));
    for expected in 2..12 {
        reload.update(|r| r + 1);
        test.advance();
        assert_eq!(compositions.get(), expected, "a reload recomposes once");
    }
    assert_old_instances_freed(&instances.borrow());
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, changingComputedStateDependenciesShouldClearThem
#[test]
fn a_computed_state_reading_a_new_instance_leaves_the_old_one() {
    // Compose counts the composition's derived dependencies; here, the old instances are freed
    // and a change to one recomposes nothing.
    let mut test = composition_test();
    let reload = mutable_state_of(0);
    let r = reload.handle();
    let instances: Rc<std::cell::RefCell<Vec<State<i32>>>> = Rc::default();
    let seen = instances.clone();
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || {
        c.increment();
        let item_value = remember_keyed(r.get(), || computed_state_of(|| 1));
        seen.borrow_mut().push(item_value);
        let intermediate = remember_updated_state(item_value);
        let list = remember(move || {
            computed_state_of(move || (0..10).map(|_| intermediate.get().get()).collect::<Vec<_>>())
        });
        Text(&format!("List of size {}", list.with(Vec::len)));
    });
    test.validate(|v| v.text("List of size 10"));
    for _ in 0..10 {
        reload.update(|r| r + 1);
        test.advance();
    }
    test.revalidate();
    assert_old_instances_freed(&instances.borrow());
    assert_eq!(compositions.get(), 11);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, computedStateOfMutationPolicyDoesNotInvalidateNestedStates
#[test]
fn a_nested_computed_state_unchanged_by_its_policy_stops_the_change() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(30), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let transient = kept(computed_state_of_with(structural_equality_policy(), move || {
        (ah.get() + bh.get()) / 10
    }));
    let invocations = Counter::new();
    let counter = invocations.clone();
    let answer = kept(computed_state_of(move || {
        counter.increment();
        transient.get()
    }));
    test.compose(move || Text(&format!("The answer is {}", answer.get())));
    test.validate(move |v| v.text(&format!("The answer is {}", (ah.get() + bh.get()) / 10)));
    assert_eq!(invocations.get(), 1);
    // Compose calculates at the check and again at the read (3, then 5): cached where it's read,
    // the check's calculation is the read's here.
    a.update(|a| a + 10);
    test.expect_changes();
    assert_eq!(invocations.get(), 2);
    b.update(|b| b + 1);
    test.expect_no_changes();
    assert_eq!(invocations.get(), 3);
    test.revalidate();
}

/// An item whose `active` is state, as Kotlin's `ComputedNestedItem`.
struct NestedItem {
    number: i32,
    active: MutableState<bool>,
}

impl NestedItem {
    fn new(number: i32, active: bool) -> Rc<NestedItem> {
        let active = kept_state(mutable_state_of(active));
        Rc::new(NestedItem { number, active })
    }
}

/// Keeps a state for the test's thread.
fn kept_state<T: 'static>(state: OwnedState<T>) -> MutableState<T> {
    let handle = state.handle();
    std::mem::forget(state);
    handle
}

/// Compose's structural policy on a `ComputedNestedItem?`: `equals` of a class without it is
/// identity.
struct PtrOptionPolicy;

impl MutationPolicy<Option<Rc<NestedItem>>> for PtrOptionPolicy {
    fn equivalent(a: &Option<Rc<NestedItem>>, b: &Option<Rc<NestedItem>>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, nestedComputedStateWithChangingState
#[test]
fn nested_computed_states_follow_state_inside_their_values() {
    let mut test = composition_test();
    let item = mutable_state_of_with(NestedItem::new(10, true), referential_equality_policy());
    let ih = item.handle();
    let even_active_item = kept(computed_state_of_with(PtrOptionPolicy, move || {
        let item = ih.get();
        (item.active.get() && item.number % 2 == 0).then_some(item)
    }));
    let number = kept(computed_state_of(move || {
        even_active_item.with(|item| item.as_ref().map(|i| i.number))
    }));
    test.compose(move || Text(&format!("evenActiveNumber = {:?}", number.get())));
    test.validate(|v| v.text("evenActiveNumber = Some(10)"));
    item.with(|item| item.active.set(false));
    test.expect_changes();
    test.validate(|v| v.text("evenActiveNumber = None"));
    let copy = |number: i32| NestedItem::new(number, ih.with(|item| item.active.get()));
    item.set(copy(9));
    test.expect_no_changes();
    test.revalidate();
    item.set(copy(8));
    test.expect_no_changes();
    test.revalidate();
    item.with(|item| item.active.set(true));
    test.expect_changes();
    test.validate(|v| v.text("evenActiveNumber = Some(8)"));
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, ensureCalculateIsNotCalledTooSoon
#[test]
fn a_computed_state_is_calculated_where_the_pass_reads_it() {
    #[composable]
    fn Content(a: MutableState<i32>, d: State<i32>, calculated: Counter, changed: Counter) {
        Text(&format!("a = {}", a.get()));
        let before = calculated.get();
        Display(Rc::new([("d", d)]));
        changed.set(i64::from(before != calculated.get()));
    }
    let mut test = composition_test();
    let a = mutable_state_of(11);
    let ah = a.handle();
    let calculated = Counter::new();
    let counter = calculated.clone();
    let d = kept(computed_state_of(move || {
        counter.increment();
        ah.get() + 10
    }));
    let changed = Counter::new();
    let (c, ch) = (calculated.clone(), changed.clone());
    test.compose(move || Content(ah, d, c.clone(), ch.clone()));
    test.validate(move |v| {
        v.text(&format!("a = {}", ah.get()));
        v.text(&format!("d = {}", ah.get() + 10));
    });
    assert_eq!(changed.get(), 1, "expected d to be calculated");
    a.update(|a| a + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(changed.get(), 1, "expected d to be calculated where it's read");
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, readInterwovenComputedAndDerivedStates
#[test]
fn interwoven_computed_and_derived_states() {
    let mut test = composition_test();
    let state = mutable_state_of(1);
    let sh = state.handle();
    let c1 = kept(computed_state_of(move || sh.get() * 2));
    let d1 = kept(derived_state_of(move || c1.get() * 3));
    let c2 = kept(computed_state_of(move || d1.get() * 5));
    let compositions = Counter::new();
    let c = compositions.clone();
    test.compose(move || {
        c.increment();
        Text(&format!("Value is {}", c2.get()));
    });
    test.validate(|v| v.text("Value is 30"));
    assert_eq!(compositions.get(), 1);
    state.update(|s| s + 1);
    test.expect_changes();
    test.validate(|v| v.text("Value is 60"));
    assert_eq!(compositions.get(), 2);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, onlyInvalidatesIfResultIsDifferentInMovableContent
#[test]
fn in_movable_content_only_a_different_result_invalidates() {
    let mut test = composition_test();
    let (a, b) = (mutable_state_of(32), mutable_state_of(10));
    let (ah, bh) = (a.handle(), b.handle());
    let answer = sum(ah, bh);
    let compositions = Counter::new();
    let c = compositions.clone();
    let content = movable_content_of(move || {
        c.increment();
        Text(&format!("The answer is {}", answer.get()));
    });
    test.compose(move || {
        content();
    });
    test.validate(move |v| v.text(&format!("The answer is {}", ah.get() + bh.get())));
    assert_eq!(compositions.get(), 1);
    Snapshot::with_mutable_snapshot(|| {
        a.update(|a| a + 1);
        b.update(|b| b - 1);
    });
    test.expect_no_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 1);
    a.update(|a| a + 1);
    test.expect_changes();
    test.revalidate();
    assert_eq!(compositions.get(), 2);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, nestedComputedStateObservedDirectlyInvalidatesOuterComputedState
#[test]
fn a_nested_computed_state_read_directly_too() {
    let mut test = composition_test();
    let a = mutable_state_of(1);
    let ah = a.handle();
    let inner = kept(computed_state_of(move || ah.get()));
    let outer = kept(computed_state_of(move || inner.get() * 10));
    test.compose(move || {
        DisplayItem("inner", inner);
        DisplayItem("outer", outer);
    });
    test.validate(move |v| {
        v.text(&format!("inner = {}", ah.get()));
        v.text(&format!("outer = {}", ah.get() * 10));
    });
    a.update(|a| a + 1);
    test.expect_changes();
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, nestedComputedStateChangesDependenciesWithoutChangingValue
#[test]
fn a_nested_computed_state_changing_what_it_reads_but_not_its_value() {
    let mut test = composition_test();
    let (condition, a, b) = (mutable_state_of(true), mutable_state_of(1), mutable_state_of(1));
    let (ch, ah, bh) = (condition.handle(), a.handle(), b.handle());
    let inner = kept(computed_state_of(move || if ch.get() { ah.get() } else { bh.get() }));
    let outer = kept(computed_state_of(move || inner.get() * 10));
    test.compose(move || DisplayItem("outer", outer));
    let expected = move |v: &mut Expected| {
        v.text(&format!("outer = {}", (if ch.get() { ah.get() } else { bh.get() }) * 10))
    };
    test.validate(expected);
    condition.set(false);
    test.expect_no_changes();
    test.revalidate();
    b.set(2);
    test.expect_changes();
    test.revalidate();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/CompositionAndComputedStateTests.kt, readingComputedStateWithoutObservationDoesNotClearDependencies
#[test]
fn a_read_without_observation_leaves_what_it_depends_on() {
    let mut test = composition_test();
    let a = mutable_state_of(0);
    let ah = a.handle();
    let computed = kept(computed_state_of(move || ah.get()));
    test.compose(move || {
        DisplayItem("computed", computed);
        Snapshot::without_read_observation(|| computed.get());
    });
    test.validate(move |v| v.text(&format!("computed = {}", ah.get())));
    a.update(|a| a + 1);
    test.expect_changes();
    test.revalidate();
}

/// The instances composed before the last two are freed, and the last is there: a dropped owner's
/// cell is freed at the end of the pass after the one dropping it (rule R2), so the one replaced
/// last may still be there.
fn assert_old_instances_freed<T: Clone + 'static>(instances: &[State<T>]) {
    let alive: Vec<bool> = instances.iter().map(|state| state.try_get().is_some()).collect();
    let (old, _) = alive.split_at(alive.len() - 2);
    assert!(old.iter().all(|alive| !alive), "an old instance stays: {alive:?}");
    assert_eq!(alive.last(), Some(&true), "{alive:?}");
}
