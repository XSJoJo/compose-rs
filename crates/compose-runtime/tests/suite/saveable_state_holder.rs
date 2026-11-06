//! Compose's `SaveableStateHolderTest`: screens' saveable state kept while they're away and across
//! recreation, removed on request, nothing saved for content without saveable state, each
//! provider's own registry, and the holder's `keys`. Compose's screens are an enum; numbers here.
//! `keys` holds the numbers the keys are kept under (`state_key`): keys are hashed (D1950).

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use compose_runtime::platform::NodeTree;
use compose_runtime::*;

use crate::restoration::RestorationTester;

const SCREEN_1: u8 = 1;
const SCREEN_2: u8 = 2;

/// What a screen remembered, -1 until it composes.
#[derive(Clone)]
struct Numbers {
    increment: Rc<Cell<i32>>,
    remembered: Rc<Cell<i32>>,
    saved: Rc<Cell<i32>>,
}

impl ComposeArg for Numbers {
    fn same(&self, other: &Numbers) -> bool {
        Rc::ptr_eq(&self.increment, &other.increment)
    }
}

impl Numbers {
    fn new() -> Numbers {
        Numbers {
            increment: Rc::default(),
            remembered: Rc::new(Cell::new(-1)),
            saved: Rc::new(Cell::new(-1)),
        }
    }

    fn next(&self) -> i32 {
        let value = self.increment.get();
        self.increment.set(value + 1);
        value
    }

    fn get(&self) -> (i32, i32) {
        (self.remembered.get(), self.saved.get())
    }

    fn forget(&self) {
        self.remembered.set(-1);
        self.saved.set(-1);
    }
}

/// The screen with a remembered number and a saved one, if it's `shown_on`.
#[composable]
fn Screen(screen: u8, shown_on: u8, numbers: Numbers) {
    if screen == shown_on {
        numbers.remembered.set(remember(|| numbers.next()));
        numbers.saved.set(remember_saveable(|| numbers.next()));
    }
}

/// The screen `screen` says, in a provider of its key, the screen with numbers being `shown_on`.
fn screens(
    screen: MutableState<u8>,
    shown_on: u8,
    numbers: &Numbers,
    holder_out: Rc<Cell<Option<SaveableStateHolder>>>,
) -> impl Fn() + 'static {
    let numbers = numbers.clone();
    move || {
        let holder = remember_saveable_state_holder();
        holder_out.set(Some(holder));
        let screen = screen.get();
        let numbers = numbers.clone();
        holder.SaveableStateProvider(screen, || Screen(screen, shown_on, numbers));
    }
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, stateIsRestoredWhenGoBackToScreen1
#[test]
fn state_is_restored_when_going_back_to_screen_1() {
    let numbers = Numbers::new();
    let screen = mutable_state_of(SCREEN_1);
    let tester =
        RestorationTester::set_content(screens(screen.handle(), SCREEN_1, &numbers, Rc::default()));
    assert_eq!(numbers.get(), (0, 1));
    screen.set(SCREEN_2);
    tester.run_on_idle();
    numbers.forget();
    screen.set(SCREEN_1);
    tester.run_on_idle();
    assert_eq!(numbers.get(), (2, 1));
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, simpleRestoreOnlyOneScreen
#[test]
fn one_screen_is_restored() {
    let numbers = Numbers::new();
    let screen = mutable_state_of(SCREEN_1);
    let mut tester =
        RestorationTester::set_content(screens(screen.handle(), SCREEN_1, &numbers, Rc::default()));
    assert_eq!(numbers.get(), (0, 1));
    numbers.forget();
    tester.emulate_saved_instance_state_restore();
    assert_eq!(numbers.get(), (2, 1));
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, switchToScreen2AndRestore
#[test]
fn screen_2_is_restored_after_switching_to_it() {
    let numbers = Numbers::new();
    let screen = mutable_state_of(SCREEN_1);
    let mut tester =
        RestorationTester::set_content(screens(screen.handle(), SCREEN_2, &numbers, Rc::default()));
    screen.set(SCREEN_2);
    tester.run_on_idle();
    assert_eq!(numbers.get(), (0, 1));
    numbers.forget();
    tester.emulate_saved_instance_state_restore();
    assert_eq!(numbers.get(), (2, 1));
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, stateOfScreen1IsSavedAndRestoredWhileWeAreOnScreen2
#[test]
fn screen_1_is_saved_and_restored_while_on_screen_2() {
    let numbers = Numbers::new();
    let screen = mutable_state_of(SCREEN_1);
    let mut tester =
        RestorationTester::set_content(screens(screen.handle(), SCREEN_1, &numbers, Rc::default()));
    assert_eq!(numbers.get(), (0, 1));
    screen.set(SCREEN_2);
    tester.run_on_idle();
    numbers.forget();
    tester.emulate_saved_instance_state_restore();
    screen.set(SCREEN_1);
    tester.run_on_idle();
    assert_eq!(numbers.get(), (2, 1));
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, weCanSkipSavingForCurrentScreen
#[test]
fn saving_the_current_screen_can_be_skipped() {
    let numbers = Numbers::new();
    let screen = mutable_state_of(SCREEN_1);
    let holder: Rc<Cell<Option<SaveableStateHolder>>> = Rc::default();
    let tester = RestorationTester::set_content(screens(
        screen.handle(),
        SCREEN_1,
        &numbers,
        holder.clone(),
    ));
    assert_eq!(numbers.saved.get(), 1);
    numbers.forget();
    holder.get().expect("composed").remove_state(SCREEN_1);
    screen.set(SCREEN_2);
    tester.run_on_idle();
    screen.set(SCREEN_1);
    tester.run_on_idle();
    assert_eq!(numbers.saved.get(), 3);
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, weCanRemoveAlreadySavedState
#[test]
fn state_already_saved_can_be_removed() {
    let numbers = Numbers::new();
    let screen = mutable_state_of(SCREEN_1);
    let holder: Rc<Cell<Option<SaveableStateHolder>>> = Rc::default();
    let tester = RestorationTester::set_content(screens(
        screen.handle(),
        SCREEN_1,
        &numbers,
        holder.clone(),
    ));
    assert_eq!(numbers.saved.get(), 1);
    numbers.forget();
    screen.set(SCREEN_2);
    tester.run_on_idle();
    holder.get().expect("composed").remove_state(SCREEN_1);
    screen.set(SCREEN_1);
    tester.run_on_idle();
    assert_eq!(numbers.saved.get(), 3);
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, restoringStateOfThePreviousPageAfterCreatingBundle
#[test]
fn the_previous_page_is_restored_after_the_host_saved() {
    #[composable]
    fn Page(show_first: bool, out: Rc<Cell<Option<MutableState<i32>>>>) {
        if show_first {
            out.set(Some(remember_saveable(|| mutable_state_of(0))));
        }
    }
    let show_first = mutable_state_of(true);
    let first_page: Rc<Cell<Option<MutableState<i32>>>> = Rc::default();
    let (show, out) = (show_first.handle(), first_page.clone());
    let tester = RestorationTester::set_content(move || {
        let holder = remember_saveable_state_holder();
        let show = show.get();
        let out = out.clone();
        holder.SaveableStateProvider(show, || Page(show, out));
    });
    let state = first_page.take().expect("composed");
    assert_eq!(state.get(), 0);
    state.set(1);
    show_first.set(false);
    tester.run_on_idle();
    // The activity saves its state (`doFakeSave`), and stays.
    tester.registry().perform_save();
    show_first.set(true);
    tester.run_on_idle();
    assert_eq!(first_page.take().map(|state| state.get()), Some(1));
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, saveNothingWhenNoRememberSaveableIsUsedInternally
#[test]
fn nothing_is_saved_without_saveable_state_inside() {
    let show_first = mutable_state_of(true);
    // Compose's `SaveableStateRegistry(null) { true }`: a registry that checks, so the holder's
    // saver runs when it registers.
    let registry = SaveableStateRegistry::with_can_be_saved(None, |_| true);
    let (show, r) = (show_first.handle(), registry.clone());
    let composition = Composition::new(NodeTree::<()>::new());
    composition.set_content(move || {
        CompositionLocalProvider(LOCAL_SAVEABLE_STATE_REGISTRY.provides(Some(r.clone())), || {
            let holder = remember_saveable_state_holder();
            holder.SaveableStateProvider(show.get(), || {});
        });
    });
    show_first.set(false);
    composition.recompose();
    assert!(registry.perform_save().is_empty());
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, childLocalCompositionsAreSet
#[test]
fn each_provider_gets_a_registry_of_its_own() {
    // Compose's `LocalSavedStateRegistryOwner`, the same object as the registry there, waits on
    // saved-state (docs/parity/lifecycle.md).
    let key = mutable_state_of(1);
    let root = SaveableStateRegistry::new(None);
    let registries: Rc<RefCell<Vec<SaveableStateRegistry>>> = Rc::default();
    let (k, r, seen) = (key.handle(), root.clone(), registries.clone());
    let composition = Composition::new(NodeTree::<()>::new());
    composition.set_content(move || {
        let seen = seen.clone();
        CompositionLocalProvider(
            LOCAL_SAVEABLE_STATE_REGISTRY.provides(Some(r.clone())),
            move || {
                let holder = remember_saveable_state_holder();
                holder.SaveableStateProvider(k.get(), || {
                    seen.borrow_mut()
                        .push(LOCAL_SAVEABLE_STATE_REGISTRY.current().expect("provided"));
                });
            },
        );
    });
    for next in 2..=5 {
        key.set(next);
        composition.recompose();
    }
    let registries = registries.borrow();
    assert_eq!(registries.len(), 5);
    for (index, registry) in registries.iter().enumerate() {
        assert!(!registry.same(&root));
        assert!(registries[..index].iter().all(|other| !other.same(registry)));
    }
}

#[test]
#[should_panic(expected = "cannot be saved using the current SaveableStateRegistry")]
fn a_providers_registry_refuses_what_its_parent_refuses() {
    // Compose's provider registries ask the holder's parent registry's `canBeSaved`.
    let root = SaveableStateRegistry::with_can_be_saved(None, |value| {
        !matches!(value, SavedValue::Text(_))
    });
    let composition = Composition::new(NodeTree::<()>::new());
    composition.set_content(move || {
        CompositionLocalProvider(
            LOCAL_SAVEABLE_STATE_REGISTRY.provides(Some(root.clone())),
            || {
                let holder = remember_saveable_state_holder();
                holder.SaveableStateProvider(1, || {
                    remember_saveable(|| String::from("refused"));
                });
            },
        );
    });
}

/// The numbers `keys` are kept under.
fn kept(holder: SaveableStateHolder, keys: &[&str]) -> HashSet<u64> {
    keys.iter().map(|key| holder.state_key(key)).collect()
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, keysPropertyIsUpdated
#[test]
fn keys_are_updated() {
    let holder: Rc<Cell<Option<SaveableStateHolder>>> = Rc::default();
    let key_set_size = Rc::new(Cell::new(usize::MAX));
    let active_key: OwnedState<Option<&'static str>> = mutable_state_of(None);
    let (h, size, active) = (holder.clone(), key_set_size.clone(), active_key.handle());
    let tester = RestorationTester::set_content(move || {
        let local = remember_saveable_state_holder();
        h.set(Some(local));
        // Read here, so the content recomposes when the keys change: they're observed.
        size.set(local.keys().len());
        if let Some(key) = active.get() {
            local.SaveableStateProvider(key, || {});
        }
    });
    let holder = holder.get().expect("composed");
    tester.run_on_idle();
    assert_eq!(key_set_size.get(), 0);
    assert!(holder.keys().is_empty());
    // Composing a key adds it.
    active_key.set(Some("A"));
    tester.run_on_idle();
    assert_eq!(key_set_size.get(), 1);
    assert_eq!(holder.keys(), kept(holder, &["A"]));
    // A new key is added, and "A" stays: it's saved.
    active_key.set(Some("B"));
    tester.run_on_idle();
    assert_eq!(key_set_size.get(), 2);
    assert_eq!(holder.keys(), kept(holder, &["A", "B"]));
    // "B" leaving is saved, not removed: keys go with `remove_state` only.
    active_key.set(None);
    tester.run_on_idle();
    assert_eq!(key_set_size.get(), 2);
    assert_eq!(holder.keys(), kept(holder, &["A", "B"]));
    // A saved key removed.
    holder.remove_state("A");
    tester.run_on_idle();
    assert_eq!(key_set_size.get(), 1);
    assert_eq!(holder.keys(), kept(holder, &["B"]));
    // "B" composed again is active again, the set the same.
    active_key.set(Some("B"));
    tester.run_on_idle();
    assert_eq!(key_set_size.get(), 1);
    assert_eq!(holder.keys(), kept(holder, &["B"]));
    // An active key removed.
    holder.remove_state("B");
    tester.run_on_idle();
    assert_eq!(key_set_size.get(), 0);
    assert!(holder.keys().is_empty());
}

#[test]
fn keys_first_read_late_leave_out_content_that_left_with_nothing_saved() {
    // The set is made when first read, from what the holder kept: content that left with
    // nothing saved left nothing, so its key isn't there. Compose keeps an empty state for it
    // (`savedStates[key] = registry.performSave()`); here a lazy list's items scrolled by don't
    // fill the holder (D1951). Read before, the set has it (`keys_are_updated`).
    let holder: Rc<Cell<Option<SaveableStateHolder>>> = Rc::default();
    let active_key: OwnedState<Option<&'static str>> = mutable_state_of(Some("A"));
    let (h, active) = (holder.clone(), active_key.handle());
    let tester = RestorationTester::set_content(move || {
        let local = remember_saveable_state_holder();
        h.set(Some(local));
        if let Some(key) = active.get() {
            local.SaveableStateProvider(key, || {});
        }
    });
    active_key.set(Some("B"));
    tester.run_on_idle();
    let holder = holder.get().expect("composed");
    assert_eq!(holder.keys(), kept(holder, &["B"]));
}

// Compose: compose/runtime/runtime-saveable/src/androidDeviceTest/kotlin/androidx/compose/runtime/saveable/SaveableStateHolderTest.kt, keysPropertyIsRestored
#[test]
fn keys_are_restored() {
    #[composable]
    fn Content(key: &'static str) {
        // State, so the key is saved across recreation (keys with nothing saved aren't).
        remember_saveable(|| format!("state_for_{key}"));
    }
    let holder: Rc<Cell<Option<SaveableStateHolder>>> = Rc::default();
    let active_key: OwnedState<Option<&'static str>> = mutable_state_of(None);
    let (h, active) = (holder.clone(), active_key.handle());
    let mut tester = RestorationTester::set_content(move || {
        let local = remember_saveable_state_holder();
        h.set(Some(local));
        if let Some(key) = active.get() {
            local.SaveableStateProvider(key, || Content(key));
        }
    });
    active_key.set(Some("A"));
    tester.run_on_idle();
    active_key.set(Some("B"));
    tester.run_on_idle();
    let first = holder.get().expect("composed");
    assert_eq!(first.keys(), kept(first, &["A", "B"]));
    tester.emulate_saved_instance_state_restore();
    // The new holder has its keys from what it was restored with: "B" composed, "A" saved.
    let holder = holder.get().expect("composed again");
    assert_eq!(holder.keys(), kept(holder, &["A", "B"]));
    holder.remove_state("A");
    tester.run_on_idle();
    assert_eq!(holder.keys(), kept(holder, &["B"]));
    holder.remove_state("B");
    tester.run_on_idle();
    assert!(holder.keys().is_empty());
}
