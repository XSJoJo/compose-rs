//! Review of `shared-elements-3` (D1338): an initial composition sends what its changes wrote at
//! its end, as `Recomposer.composeInitial`'s `notifyObjectsInitialized`, unless it's nested in a
//! pass.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_runtime::platform::*;
use compose_runtime::snapshots::Snapshot;
use compose_runtime::*;

/// Counts the sends apply observers hear, and whether one came during a composing pass.
fn heard() -> (Rc<Cell<usize>>, Rc<Cell<bool>>, compose_runtime::snapshots::ObserverHandle) {
    let (sends, while_composing) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(false)));
    let (counted, composing) = (sends.clone(), while_composing.clone());
    let handle = Snapshot::register_apply_observer(move |_, _| {
        counted.set(counted.get() + 1);
        if compose_runtime::is_composing() {
            composing.set(true);
        }
    });
    (sends, while_composing, handle)
}

// Compose: compose/runtime/runtime/src/commonMain/kotlin/androidx/compose/runtime/Recomposer.kt, composeInitial (notifyObjectsInitialized after applyChanges)
/// What an initial composition's effects wrote (a node's attach, in compose-ui) is sent when
/// `set_content` returns: its readers hear of it without another send (D1338).
#[test]
fn an_initial_composition_sends_what_its_effects_wrote() {
    let state = mutable_state_of(0);
    let handle = state.handle();
    Snapshot::notify_objects_initialized();
    let (sends, _, _observer) = heard();
    let composition = Composition::new(NodeTree::<()>::new());
    composition.set_content(move || {
        SideEffect(move || handle.set(1));
    });
    assert_eq!(sends.get(), 1, "sent at its end");
    Snapshot::send_apply_notifications();
    assert_eq!(sends.get(), 1, "nothing left to send");
}

/// A recomposition doesn't send at its end (as Compose's: `composeInitial` alone notifies): what
/// its effects wrote waits for the next send.
#[test]
fn a_recomposition_sends_nothing_at_its_end() {
    let state = mutable_state_of(0);
    let step = mutable_state_of(0);
    let (handle, s) = (state.handle(), step.handle());
    Snapshot::notify_objects_initialized();
    let composition = Composition::new(NodeTree::<()>::new());
    composition.set_content(move || {
        let step = s.get();
        SideEffect(move || handle.set(step));
    });
    let (sends, _, _observer) = heard();
    step.set(1);
    assert!(composition.recompose());
    assert_eq!(sends.get(), 1, "`step` sent before the pass, the effect's write not after it");
    Snapshot::send_apply_notifications();
    assert_eq!(sends.get(), 2);
}

/// A composition whose content is set while its parent composes (a subcomposition composed in
/// its parent's composition) sends nothing then: apply observers aren't called in the middle of
/// the parent's composition (Compose's `notifyObjectsInitialized` there advances the parent's
/// pass snapshot, which sends nothing).
#[test]
fn a_composition_set_while_its_parent_composes_sends_nothing_then() {
    let state = mutable_state_of(0);
    let written = mutable_state_of(0);
    let (handle, w) = (state.handle(), written.handle());
    Snapshot::notify_objects_initialized();
    let (sends, while_composing, _observer) = heard();
    let children: Rc<RefCell<Vec<Composition>>> = Rc::default();
    let kept = children.clone();
    let step = mutable_state_of(0);
    let s = step.handle();
    let parent = Composition::new(NodeTree::<()>::new());
    parent.set_content(move || {
        if s.get() == 0 {
            return;
        }
        // Written while composing: what a send there would carry.
        w.set(1);
        let context = remember_composition_context();
        let child = Composition::with_parent(NodeTree::<()>::new(), context);
        child.set_content(move || {
            SideEffect(move || handle.set(1));
        });
        kept.borrow_mut().push(child);
    });
    // A change waits to be sent when the parent recomposes: the parent's own send before the
    // pass sends it, so another is written while it composes, by the step's reader.
    step.set(1);
    parent.recompose();
    assert!(!while_composing.get(), "an apply observer called while the parent composes");
    Snapshot::send_apply_notifications();
    assert!(sends.get() >= 1);
    for child in children.borrow().iter() {
        child.dispose();
    }
}

/// A state a composition makes when its content is set while its parent composes, written by
/// the parent after it in the same pass: as Compose's `notifyObjectsInitialized` advances the
/// parent's pass snapshot there (`composerWasComposing` is the child's own), the write is a
/// change, and the child's reader recomposes with it.
#[test]
fn a_state_a_child_made_written_after_it_by_its_composing_parent_recomposes_the_child() {
    let seen: Rc<RefCell<Vec<i32>>> = Rc::default();
    let made: Rc<RefCell<Option<MutableState<i32>>>> = Rc::default();
    let children: Rc<RefCell<Vec<Composition>>> = Rc::default();
    let (kept, log, out) = (children.clone(), seen.clone(), made.clone());
    let step = mutable_state_of(0);
    let s = step.handle();
    let parent = Composition::new(NodeTree::<()>::new());
    parent.set_content(move || {
        if s.get() == 0 {
            return;
        }
        let context = remember_composition_context();
        let first = kept.borrow().is_empty();
        if first {
            let child = Composition::with_parent(NodeTree::<()>::new(), context);
            let (log, given) = (log.clone(), out.clone());
            child.set_content(move || {
                let state = remember(|| mutable_state_of(0));
                *given.borrow_mut() = Some(state);
                log.borrow_mut().push(state.get());
            });
            kept.borrow_mut().push(child);
            let state = out.borrow().expect("made");
            state.set(1);
        }
    });
    step.set(1);
    parent.recompose();
    for child in children.borrow().iter() {
        child.recompose();
    }
    Snapshot::send_apply_notifications();
    for child in children.borrow().iter() {
        child.recompose();
    }
    assert_eq!(*seen.borrow(), [0, 1], "the child recomposed with the parent's write");
    for child in children.borrow().iter() {
        child.dispose();
    }
}
