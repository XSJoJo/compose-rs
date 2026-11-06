//! Compose's `CompositionRegistrationObserverTest`, its unconfined-dispatcher regression test and
//! `AndroidCompositionObserverTests`, adapted: a recomposer's registration observers hear its
//! compositions registered and unregistered (`tooling::CompositionRegistrationObserver`, D1510).
//!
//! Compose's tests compose UI: a `SubcomposeLayout`'s subcompositions, a `ComposeView` inside an
//! `AndroidView`, a `Dialog`. Here each is the composition the UI makes of it: a subcomposition
//! sharing its parent's applier and made after its parent's pass (as a layout measures), a root
//! composition of its own tree under the same recomposer, and a child composition of its own tree
//! under a place of its parent's.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use compose_runtime::platform::*;
use compose_runtime::tooling::*;
use compose_runtime::*;

use crate::mock::*;

type Compositions = Rc<RefCell<HashSet<ObservableComposition>>>;

/// What a test's observer was told, as each Kotlin test's object: a composition registered twice,
/// or unregistered unknown, or a call once the handle is disposed fails the test.
#[derive(Clone, Default)]
struct Received {
    compositions: Compositions,
    disposed: Rc<Cell<bool>>,
}

impl CompositionRegistrationObserver for Received {
    fn on_composition_registered(&self, composition: &ObservableComposition) {
        assert!(!self.disposed.get(), "told after being disposed");
        assert!(self.compositions.borrow_mut().insert(*composition), "registered twice");
    }

    fn on_composition_unregistered(&self, composition: &ObservableComposition) {
        assert!(!self.disposed.get(), "told after being disposed");
        assert!(self.compositions.borrow_mut().remove(composition), "unregistered unknown");
    }
}

impl Received {
    fn set(&self) -> HashSet<ObservableComposition> {
        self.compositions.borrow().clone()
    }
}

/// How a child composition is made at a place.
type Make = Rc<dyn Fn(CompositionContext) -> Composition>;

/// A `SubcomposeLayout`'s: its parent's applier.
fn subcomposed() -> Make {
    Rc::new(|place| subcomposition(place, ViewApplier::new().root()))
}

/// A `Dialog`'s: a tree of its own under the place.
fn dialog() -> Make {
    Rc::new(|place| Composition::with_parent(ViewApplier::new(), place))
}

/// A `ComposeView`'s in an `AndroidView`: a root of its own under the same recomposer.
fn compose_view(recomposer: Recomposer) -> Make {
    Rc::new(move |_| Composition::with_parent(ViewApplier::new(), recomposer.as_context()))
}

/// What a child's content runs first, with the child.
type Check = Option<Rc<dyn Fn(ObservableComposition)>>;

/// A child composition made after this pass, in `expected` while it's there, as the Kotlin
/// tests' compositions add themselves in a `DisposableEffect`.
#[composable]
fn Child(make: Make, expected: Compositions, check: Check) {
    let place = remember_composition_context();
    DisposableEffect((), move |scope| {
        let child = make(place);
        let me = child.observable();
        child.set_content(move || {
            // No node: those sharing the parent's applier would all emit below its root.
            if let Some(check) = &check {
                check(me);
            }
        });
        expected.borrow_mut().insert(me);
        scope.on_dispose(move || {
            expected.borrow_mut().remove(&me);
            child.dispose();
        })
    });
}

/// A root composition under a recomposer of its own, composing `count` children made by `make`
/// unless `hidden`, in `expected` with them.
struct Scene {
    recomposer: Owned<Recomposer>,
    root: Option<Composition>,
    expected: Compositions,
    count: OwnedState<usize>,
    hidden: OwnedState<bool>,
}

impl Scene {
    fn new(count: usize, hidden: bool, make: impl Fn(Recomposer) -> Make, check: Check) -> Scene {
        let recomposer = Recomposer::new();
        let expected = Compositions::default();
        let (count, hidden) = (mutable_state_of(count), mutable_state_of(hidden));
        let root = Composition::with_parent(ViewApplier::new(), recomposer.as_context());
        let (n, h, e, m) = (count.handle(), hidden.handle(), expected.clone(), make(*recomposer));
        root.set_content(move || {
            if h.get() {
                return;
            }
            let (m, e, check) = (m.clone(), e.clone(), check.clone());
            Linear!(move || {
                for index in 0..n.get() {
                    key(index, || Child(m.clone(), e.clone(), check.clone()));
                }
            });
        });
        expected.borrow_mut().insert(root.observable());
        let mut scene = Scene { recomposer, root: Some(root), expected, count, hidden };
        scene.frame();
        scene
    }

    /// A frame of its recomposer, as `awaitIdle`.
    fn frame(&mut self) {
        while perform_frame(*self.recomposer, 0) {}
    }

    fn expected(&self) -> HashSet<ObservableComposition> {
        self.expected.borrow().clone()
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            root.dispose();
        }
    }
}

/// Observes `scene`'s recomposer.
fn observe(scene: &Scene) -> (Received, CompositionObserverHandle) {
    let received = Received::default();
    let handle = scene.recomposer.observe(received.clone());
    (received, handle)
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesWhenRegistered
#[test]
fn an_observer_is_told_of_the_compositions_already_there() {
    let mut scene = Scene::new(4, false, |_| subcomposed(), None);
    let (received, handle) = observe(&scene);
    scene.frame();
    assert_eq!(received.set(), scene.expected());
    assert_eq!(scene.expected().len(), 5);
    received.disposed.set(true);
    handle.dispose();
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesForInitialCompositions
#[test]
fn a_composition_is_registered_before_its_content_composes() {
    let received = Received::default();
    let seen = received.compositions.clone();
    let check: Check = Some(Rc::new(move |me| {
        assert!(seen.borrow().contains(&me), "registered before its content");
    }));
    let mut scene = Scene::new(4, true, |_| subcomposed(), check);
    let handle = scene.recomposer.observe(received.clone());
    scene.hidden.set(false);
    scene.frame();
    assert_eq!(received.set(), scene.expected());
    assert_eq!(scene.expected().len(), 5);
    received.disposed.set(true);
    handle.dispose();
}

/// Children added, then the count of them taken to `to`: the observer follows.
fn children_change(make: impl Fn(Recomposer) -> Make, from: usize, to: usize) {
    let mut scene = Scene::new(from, false, make, None);
    let (received, handle) = observe(&scene);
    scene.frame();
    assert_eq!(received.set(), scene.expected());
    assert_eq!(scene.expected().len(), from + 1);
    scene.count.set(to);
    scene.frame();
    assert_eq!(received.set(), scene.expected());
    assert_eq!(scene.expected().len(), to + 1);
    received.disposed.set(true);
    handle.dispose();
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesForAddedSubcompositions
#[test]
fn subcompositions_added_are_registered() {
    children_change(|_| subcomposed(), 0, 12);
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesForRemovedSubcompositions
#[test]
fn subcompositions_removed_are_unregistered() {
    children_change(|_| subcomposed(), 12, 3);
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesForAddedComposeView
#[test]
fn root_compositions_added_under_the_recomposer_are_registered() {
    children_change(compose_view, 0, 6);
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesForRemovedComposeView
#[test]
fn root_compositions_removed_from_the_recomposer_are_unregistered() {
    children_change(compose_view, 8, 2);
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesForAddedDialog
#[test]
fn a_composition_of_its_own_tree_shown_is_registered() {
    children_change(|_| dialog(), 0, 1);
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, testRecomposerNotifiesForRemovedDialog
#[test]
fn a_composition_of_its_own_tree_dismissed_is_unregistered() {
    children_change(|_| dialog(), 1, 0);
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverTest.kt, cancelRecomposerDuringComposition
#[test]
fn a_recomposer_cancelled_during_a_composition_unregisters_them_all() {
    let received = Received::default();
    let seen = received.compositions.clone();
    let cancel: Rc<Cell<Option<Recomposer>>> = Rc::default();
    let c = cancel.clone();
    let check: Check = Some(Rc::new(move |me| {
        assert!(seen.borrow().contains(&me), "registered before its content");
        // The Kotlin test cancels from another thread while this content waits.
        c.get().expect("the recomposer").cancel();
    }));
    let mut scene = Scene::new(1, true, |_| subcomposed(), check);
    cancel.set(Some(*scene.recomposer));
    let handle = scene.recomposer.observe(received.clone());
    assert_eq!(received.set().len(), 1, "the root");
    scene.hidden.set(false);
    scene.frame();
    assert_eq!(scene.recomposer.current_state().get(), RecomposerState::ShutDown);
    assert!(received.set().is_empty(), "{:?}", received.set());
    received.disposed.set(true);
    handle.dispose();
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/CompositionRegistrationObserverWithUnconfinedDispatcherTest.kt, forceRecompositionDuringInitialComposition
#[test]
fn a_frame_forced_during_the_first_composition_keeps_registration_whole() {
    let recomposer = Recomposer::new();
    let received = Received::default();
    let handle = recomposer.observe(received.clone());
    let state = mutable_state_of(true);
    let s = state.handle();
    let r = *recomposer;
    let root = Composition::with_parent(ViewApplier::new(), recomposer.as_context());
    root.set_content(move || {
        let value = s.get();
        if value {
            s.set(false);
            // A frame now, as the test's clock advanced inside composition.
            perform_frame(r, 0);
        }
        Text(&value.to_string());
    });
    assert_eq!(received.set(), HashSet::from([root.observable()]));
    while perform_frame(r, 0) {}
    assert_eq!(received.set(), HashSet::from([root.observable()]), "registered once");
    root.dispose();
    assert!(received.set().is_empty());
    received.disposed.set(true);
    handle.dispose();
}

/// Counts the passes it hears.
#[derive(Clone, Default)]
struct Passes(Rc<Cell<u32>>);

impl CompositionObserver for Passes {
    fn on_begin_composition(&self, _: &ObservableComposition) {
        self.0.set(self.0.get() + 1);
    }
}

// Compose: compose/runtime/runtime/integration-tests/src/androidTest/kotlin/androidx/compose/runtime/tooling/AndroidCompositionObserverTests.kt, testObservingUiComposition
#[test]
fn an_observer_set_on_a_composition_from_its_content_hears_it() {
    let composition = Composition::new(ViewApplier::new());
    let me = composition.observable();
    let passes = Passes::default();
    let handle: Rc<RefCell<Option<CompositionObserverHandle>>> = Rc::default();
    let state = mutable_state_of(0);
    let (h, p, s) = (handle.clone(), passes.clone(), state.handle());
    composition.set_content(move || {
        Text(&s.get().to_string());
        if h.borrow().is_none() {
            *h.borrow_mut() = Some(me.set_observer(p.clone()));
        }
    });
    state.set(1);
    composition.recompose();
    assert_eq!(passes.0.get(), 1, "the pass after it was set");
    handle.borrow_mut().take().expect("a handle").dispose();
    state.set(2);
    composition.recompose();
    assert_eq!(passes.0.get(), 1, "none once disposed");
}

/// A composition whose pass panics is unregistered, as Compose's failed compositions are; one
/// disposed after isn't told again.
#[test]
fn a_composition_poisoned_is_unregistered() {
    let recomposer = Recomposer::new();
    let received = Received::default();
    let handle = recomposer.observe(received.clone());
    let state = mutable_state_of(false);
    let s = state.handle();
    let root = Composition::with_parent(ViewApplier::new(), recomposer.as_context());
    root.set_content(move || {
        if s.get() {
            panic!("on purpose");
        }
    });
    assert_eq!(received.set().len(), 1);
    state.set(true);
    let panicked =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| perform_frame(*recomposer, 0)));
    assert!(panicked.is_err());
    assert!(received.set().is_empty());
    root.dispose();
    received.disposed.set(true);
    handle.dispose();
}

/// The handle dropped stops the observer, as disposing it does; `RecomposerInfo` observes too.
#[test]
fn a_dropped_handle_stops_the_observer() {
    let recomposer = Recomposer::new();
    let received = Received::default();
    let handle = recomposer.as_recomposer_info().observe(received.clone());
    let first = Composition::with_parent(ViewApplier::new(), recomposer.as_context());
    first.set_content(|| {});
    assert_eq!(received.set().len(), 1);
    drop(handle);
    received.disposed.set(true);
    let second = Composition::with_parent(ViewApplier::new(), recomposer.as_context());
    second.set_content(|| {});
    first.dispose();
    second.dispose();
}
