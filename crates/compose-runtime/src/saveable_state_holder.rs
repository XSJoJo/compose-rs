//! `SaveableStateHolder`, as Compose's `androidx.compose.runtime.saveable.SaveableStateHolder`:
//! the saveable state of content that leaves the composition, kept by key and given back when
//! content with that key comes back (a navigation back stack's screens, a lazy list's items).
//!
//! Each provider's content registers its values with a [`SaveableStateRegistry`] of its own;
//! when the content leaves, the holder keeps what that registry saves, and gives it to the
//! registry of the content that next comes with the key. Values in a provider are found again
//! by their place within it (its group stops [`current_composite_key_hash_code`]'s walk), so a
//! provider moved elsewhere (from one animated container's slot to another's) finds them too.
//!
//! [`current_composite_key_hash_code`]: crate::current_composite_key_hash_code

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use crate::hash::{hash_key, key_of};
use crate::remember::{RememberObserver, remember, remember_observer};
use crate::saveable::{
    LOCAL_SAVEABLE_STATE_REGISTRY, SaveableStateRegistry, SavedState, SavedValue, Saver,
    remember_saveable_in, saver,
};
use crate::snapshots::{OwnedStateSet, SnapshotStateSet};
use crate::state::{ObjectHandle, Owned};

/// The group key of [`SaveableStateHolder::SaveableStateProvider`]'s reusable groups, whose data
/// key is the hash of the provider's key. `current_composite_key_hash_code` stops there (D290).
pub(crate) const PROVIDER_GROUP: u64 = key_of("compose_runtime::SaveableStateProvider");

/// The group of a provider's [`ProviderEntry`], after its content's.
const ENTRY_GROUP: u64 = key_of("compose_runtime::SaveableStateProvider::entry");

/// Content composed in a provider: its registry, and whether to keep what it saves when it
/// leaves.
struct ActiveProvider {
    registry: SaveableStateRegistry,
    should_save: Cell<bool>,
}

#[derive(Default)]
struct HolderInner {
    /// What content that left the composition saved, by key, if anything (D1951).
    saved_states: RefCell<HashMap<u64, SavedState>>,
    /// The content in the composition, by key.
    active: RefCell<HashMap<u64, Rc<ActiveProvider>>>,
    /// The keys kept, observed, once asked for (as Compose's, made when first read: a lazy
    /// list's holder has no use for it).
    keys: RefCell<Option<OwnedStateSet<u64>>>,
    /// The registry where `remember_saveable_state_holder` was called, at its last pass.
    parent: RefCell<Option<SaveableStateRegistry>>,
}

impl HolderInner {
    /// The saved state of every key with something saved: of content that left, and of content
    /// in the composition.
    fn save_all(&self) -> HashMap<u64, SavedState> {
        let mut states = self.saved_states.borrow().clone();
        // The registries save without the borrow: their providers run user code.
        let active: Vec<(u64, Rc<ActiveProvider>)> =
            self.active.borrow().iter().map(|(key, active)| (*key, active.clone())).collect();
        for (key, active) in active {
            if !active.should_save.get() {
                continue;
            }
            states.insert(key, active.registry.perform_save());
        }
        // Keys with nothing saved aren't kept across recreation, as Compose's.
        states.retain(|_, state| !state.is_empty());
        states
    }

    /// The keys set, if it was asked for.
    fn keys(&self) -> Option<SnapshotStateSet<u64>> {
        self.keys.borrow().as_ref().map(|keys| keys.handle())
    }

    /// Forgets `key`, as Compose's `removeState`.
    fn remove_state(&self, key: u64) {
        if let Some(keys) = self.keys() {
            keys.remove(&key);
        }
        match self.active.borrow().get(&key) {
            Some(active) => active.should_save.set(false),
            None => {
                self.saved_states.borrow_mut().remove(&key);
            }
        }
    }
}

/// Keeps the saveable state of content that leaves the composition, by key, and gives it back
/// when content with that key comes back, as Compose's `SaveableStateHolder`: each screen of a
/// navigation back stack, each item of a lazy list. What's kept is what `remember_saveable`
/// remembered there (a scroll position, a counter); `remember`ed values go with the content.
/// [`remember_saveable_state_holder`] saves the holder, with what it keeps, across the host's
/// recreation.
///
/// A `Copy` handle (D240): [`new`](Self::new) returns its owner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SaveableStateHolder {
    object: ObjectHandle<HolderInner>,
}

impl crate::ComposeArg for SaveableStateHolder {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl SaveableStateHolder {
    #[track_caller]
    pub fn new() -> Owned<SaveableStateHolder> {
        SaveableStateHolder::with_states(HashMap::new())
    }

    #[track_caller]
    fn with_states(states: HashMap<u64, SavedState>) -> Owned<SaveableStateHolder> {
        let inner = HolderInner { saved_states: RefCell::new(states), ..HolderInner::default() };
        Owned::new(inner, |object| SaveableStateHolder { object })
    }

    /// Composes `content` with the saveable state kept under `key`, as Compose's
    /// `SaveableStateProvider`: what its `remember_saveable` calls remember is kept by the
    /// holder when it leaves the composition, and given back when content with `key` comes
    /// back. Content with one key is composed at one place at a time (debug builds panic
    /// otherwise, as Compose throws). Content that leaves one place and comes in at another in
    /// one pass starts without its state (the new place composes before the old one saves), as
    /// in Compose. As Compose's, it's a `ReusableContent(key)`: content with another key
    /// composed in its place reuses its groups and nodes, its remembered values computed anew
    /// (D430); matched by position, so providers in a loop need a `key(…)` each to move with
    /// their keys.
    #[allow(non_snake_case)]
    #[track_caller]
    pub fn SaveableStateProvider(&self, key: impl Hash, content: impl FnOnce()) {
        let key = hash_key(&key);
        let composer = crate::composer::current();
        composer.start_reusable_group(PROVIDER_GROUP, key);
        let holder = self.object.get();
        // The registry the content's values register with, given back what was kept under
        // this key.
        let registry = remember(|| {
            let restored =
                holder.as_ref().and_then(|holder| holder.saved_states.borrow().get(&key).cloned());
            // What can be saved is the parent registry's to say, as Compose's.
            let parent = holder.as_ref().and_then(|holder| holder.parent.borrow().clone());
            match parent {
                Some(parent) if parent.checks_values() => {
                    SaveableStateRegistry::with_can_be_saved(restored, move |value| {
                        parent.can_be_saved(value)
                    })
                }
                _ => SaveableStateRegistry::new(restored),
            }
        });
        crate::local::CompositionLocalProvider(
            LOCAL_SAVEABLE_STATE_REGISTRY.provides(Some(registry.clone())),
            content,
        );
        // In a group after the content's: a removed subtree's observers are forgotten in the
        // reverse of its pre-order (a group's slots before its children's), so this one saves
        // the content's values before they unregister themselves.
        composer.start_replace_group(ENTRY_GROUP);
        remember_observer(|| ProviderEntry {
            holder,
            key,
            active: Rc::new(ActiveProvider { registry, should_save: Cell::new(true) }),
        });
        composer.end_group();
        composer.end_group();
    }

    /// Forgets what's kept under `key`, as Compose's `removeState`: content with that key in
    /// the composition keeps its state while it stays, and leaves nothing behind.
    #[track_caller]
    pub fn remove_state(&self, key: impl Hash) {
        self.remove_state_of_key(hash_key(&key));
    }

    /// The number `key` is kept under: what [`keys`](Self::keys) holds, and what
    /// [`remove_state_of_key`](Self::remove_state_of_key) takes.
    pub fn state_key(&self, key: impl Hash) -> u64 {
        hash_key(&key)
    }

    /// [`remove_state`](Self::remove_state), by its number.
    #[doc(hidden)]
    pub fn remove_state_of_key(&self, key: u64) {
        if let Some(holder) = self.object.get() {
            holder.remove_state(key);
        }
    }

    /// The keys the holder keeps state for, as Compose's `keys`: those composed in a
    /// [`SaveableStateProvider`](Self::SaveableStateProvider) and not removed since, in the
    /// composition or not, and those it was restored with. Keys are kept by their number
    /// ([`state_key`](Self::state_key)): they're hashed. Observed: a composition reading them
    /// recomposes when they change. Empty once the holder is gone.
    pub fn keys(&self) -> HashSet<u64> {
        let Some(holder) = self.object.get() else { return HashSet::new() };
        let keys = holder.keys().unwrap_or_else(|| {
            // Made when first asked for, from the keys composed and those saved.
            let mut keys: Vec<u64> = holder
                .active
                .borrow()
                .iter()
                .filter(|(_, active)| active.should_save.get())
                .map(|(key, _)| *key)
                .collect();
            keys.extend(holder.saved_states.borrow().keys().copied());
            let owner = crate::snapshots::mutable_state_set_from(keys);
            let handle = owner.handle();
            *holder.keys.borrow_mut() = Some(owner);
            handle
        });
        keys.iter_cloned().collect()
    }

    /// Saves what the holder keeps, with the state of its content in the composition, as
    /// Compose's `SaveableStateHolderImpl.Saver`: a list of each key and its state.
    pub fn saver() -> impl Saver<Owned<SaveableStateHolder>> {
        saver(
            |holder: &Owned<SaveableStateHolder>| {
                let states = holder.object.try_get()?.save_all();
                // Nothing to keep: nothing saved, as Compose's.
                if states.is_empty() {
                    return None;
                }
                let mut keys: Vec<&u64> = states.keys().collect();
                keys.sort();
                Some(SavedValue::List(
                    keys.into_iter()
                        .map(|key| {
                            SavedValue::List(vec![
                                SavedValue::Int(*key as i64),
                                saved_state_to_value(&states[key]),
                            ])
                        })
                        .collect(),
                ))
            },
            |saved| {
                let SavedValue::List(entries) = saved else { return None };
                let mut states = HashMap::new();
                for entry in entries {
                    let SavedValue::List(pair) = entry else { return None };
                    let [SavedValue::Int(key), state] = pair.as_slice() else { return None };
                    states.insert(*key as u64, saved_state_from_value(state)?);
                }
                Some(SaveableStateHolder::with_states(states))
            },
        )
    }
}

/// A [`SaveableStateHolder`] kept in the composition and saved across the host's recreation
/// with what it keeps, as Compose's `rememberSaveableStateHolder()`.
#[track_caller]
pub fn remember_saveable_state_holder() -> SaveableStateHolder {
    let parent = LOCAL_SAVEABLE_STATE_REGISTRY.current();
    let holder = remember_saveable_in(
        parent.clone(),
        false,
        SaveableStateHolder::saver(),
        SaveableStateHolder::new,
    );
    // The registry above, whose `can_be_saved` its providers' registries take, as Compose's
    // `parentSaveableStateRegistry`.
    if let Some(inner) = holder.object.get() {
        *inner.parent.borrow_mut() = parent;
    }
    holder
}

/// A provider's content in the composition: known to its holder while there, and saved into
/// it when it leaves.
struct ProviderEntry {
    holder: Option<Rc<HolderInner>>,
    key: u64,
    active: Rc<ActiveProvider>,
}

impl RememberObserver for ProviderEntry {
    fn on_remembered(&self) {
        let Some(holder) = &self.holder else { return };
        // What was kept is now the registry's: saved again when the content leaves.
        holder.saved_states.borrow_mut().remove(&self.key);
        if let Some(keys) = holder.keys() {
            keys.insert(self.key);
        }
        let previous = holder.active.borrow_mut().insert(self.key, self.active.clone());
        if previous.is_some() {
            // A programming error, as Compose's `require`: debug builds panic (once the pass
            // is applied), release builds keep the state of the last one composed.
            let message = format!(
                "SaveableStateProvider: the key {:#x} is composed at two places at once (each \
                 key's content must be composed at one place at a time)",
                self.key
            );
            if cfg!(debug_assertions) {
                panic!("{message}");
            }
            log::warn!("{message}; the state of the last one composed is kept");
        }
    }

    fn on_forgotten(&self) {
        let Some(holder) = &self.holder else { return };
        {
            let mut active = holder.active.borrow_mut();
            if active.get(&self.key).is_some_and(|other| Rc::ptr_eq(other, &self.active)) {
                active.remove(&self.key);
            }
        }
        if self.active.should_save.get() {
            // Nothing saved, nothing kept: a lazy list's items scrolled by leave no entry. The
            // key stays among `keys` if they were asked for, as it's in their set; Compose keeps
            // an empty state for it, so keys first asked for later have it too (D1951).
            let state = self.active.registry.perform_save();
            if !state.is_empty() {
                holder.saved_states.borrow_mut().insert(self.key, state);
            }
        }
    }
}

/// A saved state as a saved value: a list of each key and its values, sorted by key.
fn saved_state_to_value(state: &SavedState) -> SavedValue {
    let mut keys: Vec<&u64> = state.values.keys().collect();
    keys.sort();
    SavedValue::List(
        keys.into_iter()
            .map(|key| {
                SavedValue::List(vec![
                    SavedValue::Int(*key as i64),
                    SavedValue::List(state.values[key].clone()),
                ])
            })
            .collect(),
    )
}

fn saved_state_from_value(value: &SavedValue) -> Option<SavedState> {
    let SavedValue::List(entries) = value else { return None };
    let mut values = HashMap::new();
    for entry in entries {
        let SavedValue::List(pair) = entry else { return None };
        let [SavedValue::Int(key), SavedValue::List(list)] = pair.as_slice() else { return None };
        values.insert(*key as u64, list.clone());
    }
    Some(SavedState { values })
}
