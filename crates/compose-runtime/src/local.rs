//! Composition locals, as Compose's `CompositionLocal`: values provided to a subtree and read
//! anywhere inside it without being passed down.
//!
//! A provider is a replace group (`CompositionLocalProvider`, one key for all, as Compose's
//! `providerKey`) whose first slot holds what it provides: an entry per local, each a holder (a
//! state for a dynamic local, the value for a static one, a function for a computed one).
//! `current()` walks up the slot table from the group composing now through the provider groups
//! only (each group records, when it is allocated, the nearest provider's group above it: a
//! group's ancestors never change) to the nearest one providing the local, so a scope
//! restarting on its own finds the same provider as when its parent composed it, and a read
//! costs the providers above it, not the groups between them.
//!
//! - A **dynamic** local (`CompositionLocal::new`, Compose's `compositionLocalOf`) is held in a
//!   state: reading it subscribes, and a new value recomposes exactly the readers.
//! - A **static** local (`new_static`, `staticCompositionLocalOf`) is held as a value: reading it
//!   records nothing, and a provider whose value changes recomposes its whole content.
//! - A **computed** one (`with_computed_default`, `provides_computed`) is a function of other
//!   locals, run at each read where it's read.

use std::any::Any;
use std::rc::Rc;

use crate::__private::ReplaceGroup;
use crate::compose_arg::ComposeArg;
use crate::hash::key_of;
use crate::remember::remember;
use crate::runtime::{CompositionData, CompositionId, Compositions, Runtime, with_runtime};
use crate::slot_table::NONE;
use crate::state::{MutationPolicy, OwnedState};

/// The key of every provider group, as Compose's `providerKey`.
const PROVIDER_KEY: u64 = key_of("compose_runtime::CompositionLocalProvider");

/// A composition local: declared as a `static` (or a `const`), identified by its name and
/// value type, which must be unique together.
///
/// ```ignore
/// pub static LOCAL_CONTENT_COLOR: CompositionLocal<Color> =
///     CompositionLocal::new("LocalContentColor", || Color::BLACK);
/// ```
pub struct CompositionLocal<T: 'static> {
    name: &'static str,
    is_static: bool,
    default: LocalDefault<T>,
    /// When a newly provided value is the same as the last one (the dynamic state's policy, a
    /// static provider's comparison).
    equivalent: fn(&T, &T) -> bool,
}

/// What a local is outside any provider.
enum LocalDefault<T: 'static> {
    Value(fn() -> T),
    Computed(fn(&CompositionLocalAccessorScope) -> T),
    /// What the host gives for a key, through [`LOCAL_HOST_DEFAULT_PROVIDER`], made a `T`.
    Host {
        key: &'static str,
        resolve: fn(&'static str, Option<Rc<dyn Any>>) -> T,
    },
}

/// What names a value a host gives, as Compose's `HostDefaultKey`: by its name, which is the key's
/// own, and its value's type.
pub struct HostDefaultKey<T: 'static> {
    name: &'static str,
    _value: std::marker::PhantomData<fn() -> T>,
}

impl<T: 'static> Clone for HostDefaultKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for HostDefaultKey<T> {}

impl<T: 'static> HostDefaultKey<T> {
    pub const fn new(name: &'static str) -> HostDefaultKey<T> {
        HostDefaultKey { name, _value: std::marker::PhantomData }
    }

    /// What a [`HostDefaultProvider`] is asked for.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// What a host (a view, a window) gives locals with host defaults, as Compose's
/// `HostDefaultProvider`: provided by [`LOCAL_HOST_DEFAULT_PROVIDER`].
pub trait HostDefaultProvider {
    /// The host's value for the key named `key`, as `getHostDefault`: `None` if it has none. A
    /// value of another type than the key's is taken as none.
    fn get_host_default(&self, key: &'static str) -> Option<Rc<dyn Any>>;
}

/// The host's [`HostDefaultProvider`], as Compose's `LocalHostDefaultProvider`.
///
/// # Panics
///
/// Read where nothing provides it.
pub static LOCAL_HOST_DEFAULT_PROVIDER: CompositionLocal<Rc<dyn HostDefaultProvider>> =
    CompositionLocal::new("LocalHostDefaultProvider", || {
        panic!("compose-runtime: CompositionLocal LocalHostDefaultProvider not present")
    });

/// The host's value for `key`, which a host must have.
fn host_value<T: Clone + 'static>(key: &'static str, found: Option<Rc<dyn Any>>) -> T {
    match found.as_deref().and_then(|value| value.downcast_ref::<T>()) {
        Some(value) => value.clone(),
        None => panic!("compose-runtime: the host has no default for {key}"),
    }
}

/// The host's value for `key`, if it has one.
fn optional_host_value<T: Clone + 'static>(
    _key: &'static str,
    found: Option<Rc<dyn Any>>,
) -> Option<T> {
    found.as_deref().and_then(|value| value.downcast_ref::<T>()).cloned()
}

/// Where a computed local reads other locals, as Compose's `CompositionLocalAccessorScope`:
/// where the computed value is read, or in a [`CompositionLocalMap`].
pub struct CompositionLocalAccessorScope {
    map: Option<CompositionLocalMap>,
    anchor: Option<CompositionLocalAnchor>,
}

impl CompositionLocalAccessorScope {
    /// The value of `local` where the computed value is read, as `currentValue`.
    pub fn current_value<T: Clone + 'static>(&self, local: &CompositionLocal<T>) -> T {
        match (&self.map, &self.anchor) {
            (Some(map), _) => map.get(local),
            (None, Some(anchor)) => anchor.get(local),
            (None, None) => local.current(),
        }
    }
}

/// `probe.resolve(make())` for a `provides_computed` closure, with its scope parameter's type
/// spelled out (a higher-ranked borrow, which `resolve_lazy_1` can't name).
#[doc(hidden)]
#[inline]
pub fn resolve_lazy_computed<K: 'static, T: 'static, C>(
    probe: crate::callback::LambdaProbe<K, dyn Fn(&CompositionLocalAccessorScope) -> T>,
    make: impl FnOnce() -> C,
) -> crate::callback::Callback<dyn Fn(&CompositionLocalAccessorScope) -> T>
where
    C: Fn(&CompositionLocalAccessorScope) -> T + 'static,
{
    match probe {
        crate::callback::LambdaProbe::Hit(callback) => callback,
        crate::callback::LambdaProbe::Miss(pending) => {
            pending.store_callback(crate::callback::Callback::from_rc(Rc::new(make())))
        }
    }
}

/// A value for a local, for [`CompositionLocalProvider`], as Compose's `ProvidedValue`: made by
/// [`provides`](CompositionLocal::provides), [`provides_default`](CompositionLocal::provides_default)
/// or [`provides_computed`](CompositionLocal::provides_computed).
pub struct ProvidedValue<T: 'static> {
    local: u64,
    name: &'static str,
    is_static: bool,
    can_override: bool,
    equivalent: fn(&T, &T) -> bool,
    value: Provided<T>,
}

enum Provided<T: 'static> {
    Value(T),
    Computed(Rc<dyn Fn(&CompositionLocalAccessorScope) -> T>),
}

impl<T: Clone + 'static> Clone for ProvidedValue<T> {
    fn clone(&self) -> Self {
        ProvidedValue {
            value: match &self.value {
                Provided::Value(value) => Provided::Value(value.clone()),
                Provided::Computed(compute) => Provided::Computed(compute.clone()),
            },
            ..*self
        }
    }
}

impl<T: 'static> ProvidedValue<T> {
    /// Whether it replaces a value provided above, as `canOverride`: not for
    /// [`provides_default`](CompositionLocal::provides_default).
    pub fn can_override(&self) -> bool {
        self.can_override
    }

    /// The value provided, if it isn't computed.
    pub fn value(&self) -> Option<&T> {
        match &self.value {
            Provided::Value(value) => Some(value),
            Provided::Computed(_) => None,
        }
    }

    /// The name of the local it's for.
    pub fn local_name(&self) -> &'static str {
        self.name
    }
}

/// What a provider holds for a local.
enum Holder<T: 'static> {
    /// A dynamic local's value: reading it subscribes.
    Dynamic(OwnedState<T>),
    /// A static local's value: reading it records nothing.
    Static(T),
    Computed(Rc<dyn Fn(&CompositionLocalAccessorScope) -> T>),
}

impl<T: Clone + 'static> Holder<T> {
    /// Its value, a computed one where `scope` says: made only for one.
    fn read(&self, scope: impl FnOnce() -> CompositionLocalAccessorScope) -> T {
        match self {
            Holder::Dynamic(state) => state.get(),
            Holder::Static(value) => value.clone(),
            Holder::Computed(compute) => compute(&scope()),
        }
    }
}

/// A local's holder in a provider: `Rc<Holder<T>>` erased.
#[derive(Clone)]
struct Entry {
    local: u64,
    holder: Rc<dyn Any>,
    /// Made by this provider: its dynamic state may be written in place. One from a
    /// [`CompositionLocalContext`] belongs to the provider it was captured from.
    owned: bool,
    /// Given in the provider's pass now: entries not given are dropped after it (checked in
    /// place, so a provider's pass allocates nothing).
    given: bool,
}

/// A provider group's first slot: the locals it provides, and its place's cell.
struct ProviderEntries {
    entries: std::cell::RefCell<Vec<Entry>>,
    /// Where its group is (`PLACES`): what a [`CompositionLocalAnchor`] finds its provider by.
    place: PlaceId,
}

impl ProviderEntries {
    fn new(place: (CompositionId, u32)) -> ProviderEntries {
        ProviderEntries { entries: Default::default(), place: Places::make(place) }
    }
}

impl Drop for ProviderEntries {
    fn drop(&mut self) {
        Places::free(self.place);
    }
}

/// A cell of [`PLACES`]: its index, and the generation that tells it from what was there before.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PlaceId {
    index: u32,
    generation: u32,
}

/// Where each provider group is, by the cell its entries hold, as Compose's `Anchor`s say where
/// their groups are: a composition and a group index, rewritten when the group moves to another
/// composition's table with content (`move_group`), and the cell freed when the entries go with
/// their group. So a cell lives exactly as long as a provider (D1511, third review). Its own
/// thread-local, not the runtime's: entries are dropped wherever their group's values are, the
/// runtime borrowed or not.
#[derive(Default)]
struct Places {
    cells: Vec<(u32, (CompositionId, u32))>,
    free: Vec<u32>,
}

thread_local! {
    static PLACES: std::cell::RefCell<Places> = std::cell::RefCell::new(Places::default());
}

/// The cell no provider has: where cells can't be made, at a thread's exit, its entries' anchors
/// read defaults.
const NO_PLACE: PlaceId = PlaceId { index: NONE - 1, generation: 0 };

impl Places {
    /// A cell saying `place`. Once the thread-local is gone (a value dropped by another
    /// thread-local's destructor), none: what's made then is found nowhere.
    fn make(place: (CompositionId, u32)) -> PlaceId {
        PLACES
            .try_with(|places| {
                let mut places = places.borrow_mut();
                match places.free.pop() {
                    Some(index) => {
                        let cell = &mut places.cells[index as usize];
                        cell.1 = place;
                        PlaceId { index, generation: cell.0 }
                    }
                    None => {
                        let index = u32::try_from(places.cells.len()).expect("too many providers");
                        places.cells.push((0, place));
                        PlaceId { index, generation: 0 }
                    }
                }
            })
            .unwrap_or(NO_PLACE)
    }

    /// `id` is let go: an anchor naming it finds nothing, though another provider takes the cell
    /// (its generation moves on).
    fn free(id: PlaceId) {
        // At a thread's exit, the cells may be gone first: nothing's left to find them.
        let _ = PLACES.try_with(|places| {
            let mut places = places.borrow_mut();
            if let Some(cell) = places.cells.get_mut(id.index as usize) {
                cell.0 = id.generation.wrapping_add(1);
                places.free.push(id.index);
            }
        });
    }

    /// Where the provider of `id` is, if it's still there: not once the thread-local is gone, as
    /// when a tree dropped by another thread-local's destructor has a node read an anchor.
    fn get(id: PlaceId) -> Option<(CompositionId, u32)> {
        PLACES
            .try_with(|places| {
                let (generation, place) = *places.borrow().cells.get(id.index as usize)?;
                (generation == id.generation).then_some(place)
            })
            .ok()
            .flatten()
    }

    fn set(id: PlaceId, place: (CompositionId, u32)) {
        let _ = PLACES.try_with(|places| {
            if let Some(cell) = places.borrow_mut().cells.get_mut(id.index as usize)
                && cell.0 == id.generation
            {
                cell.1 = place;
            }
        });
    }

    /// How many cells are kept, live or free to reuse.
    fn kept() -> usize {
        PLACES.try_with(|places| places.borrow().cells.len()).unwrap_or(0)
    }
}

/// How many provider places are kept: one per provider that ever lived at once, at most (D1511).
#[doc(hidden)]
pub fn provider_place_count() -> usize {
    Places::kept()
}

impl<T: ComposeArg + Clone + 'static> CompositionLocal<Option<T>> {
    /// [`with_host_default`](CompositionLocal::with_host_default) for a key a host may lack, as
    /// `compositionLocalWithHostDefaultOf` of a nullable type: `None` where it does.
    pub const fn with_optional_host_default(
        name: &'static str,
        key: HostDefaultKey<T>,
    ) -> CompositionLocal<Option<T>> {
        CompositionLocal {
            name,
            is_static: false,
            default: LocalDefault::Host { key: key.name, resolve: optional_host_value::<T> },
            equivalent: <Option<T> as ComposeArg>::same,
        }
    }
}

impl<T: 'static> CompositionLocal<T> {
    /// A dynamic local whose value outside any provider is `default()`, as `compositionLocalOf`:
    /// reading it subscribes, and a provider's new value (not [`same`](ComposeArg::same) as the
    /// last) recomposes exactly its readers.
    pub const fn new(name: &'static str, default: fn() -> T) -> CompositionLocal<T>
    where
        T: ComposeArg,
    {
        CompositionLocal {
            name,
            is_static: false,
            default: LocalDefault::Value(default),
            equivalent: <T as ComposeArg>::same,
        }
    }

    /// A dynamic local whose provided values change when `policy` says so, as
    /// `compositionLocalOf(policy) { … }`.
    pub const fn with_policy<P: MutationPolicy<T> + Copy>(
        name: &'static str,
        _policy: P,
        default: fn() -> T,
    ) -> CompositionLocal<T> {
        CompositionLocal {
            name,
            is_static: false,
            default: LocalDefault::Value(default),
            equivalent: P::equivalent,
        }
    }

    /// A static local, as `staticCompositionLocalOf`: reading it records nothing, and a
    /// provider whose value changes recomposes its whole content. For values that rarely
    /// change, read in many places.
    pub const fn new_static(name: &'static str, default: fn() -> T) -> CompositionLocal<T>
    where
        T: ComposeArg,
    {
        CompositionLocal {
            name,
            is_static: true,
            default: LocalDefault::Value(default),
            equivalent: <T as ComposeArg>::same,
        }
    }

    /// A dynamic local whose value outside any provider is computed from other locals where
    /// it's read, as `compositionLocalWithComputedDefaultOf { … }`.
    pub const fn with_computed_default(
        name: &'static str,
        compute: fn(&CompositionLocalAccessorScope) -> T,
    ) -> CompositionLocal<T>
    where
        T: ComposeArg,
    {
        CompositionLocal {
            name,
            is_static: false,
            default: LocalDefault::Computed(compute),
            equivalent: <T as ComposeArg>::same,
        }
    }

    /// Like [`with_computed_default`](CompositionLocal::with_computed_default), static when
    /// provided, as `staticCompositionLocalWithComputedDefaultOf { … }`.
    pub const fn static_with_computed_default(
        name: &'static str,
        compute: fn(&CompositionLocalAccessorScope) -> T,
    ) -> CompositionLocal<T>
    where
        T: ComposeArg,
    {
        CompositionLocal {
            name,
            is_static: true,
            default: LocalDefault::Computed(compute),
            equivalent: <T as ComposeArg>::same,
        }
    }

    /// A dynamic local whose value outside any provider is the host's for `key`, as
    /// `compositionLocalWithHostDefaultOf`: read through [`LOCAL_HOST_DEFAULT_PROVIDER`] where it's
    /// read. Provided, it's an ordinary dynamic local.
    ///
    /// # Panics
    ///
    /// Read outside any provider where the host has no value for `key`, as Compose's throws for a
    /// non-nullable type; [`with_optional_host_default`](Self::with_optional_host_default) reads
    /// `None` there.
    pub const fn with_host_default(
        name: &'static str,
        key: HostDefaultKey<T>,
    ) -> CompositionLocal<T>
    where
        T: ComposeArg + Clone,
    {
        CompositionLocal {
            name,
            is_static: false,
            default: LocalDefault::Host { key: key.name, resolve: host_value::<T> },
            equivalent: <T as ComposeArg>::same,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Its identity: its name and its type (an address isn't one: a `const` has a new one at
    /// each use).
    #[doc(hidden)]
    pub fn id(&self) -> u64 {
        key_of(self.name) ^ key_of(std::any::type_name::<T>()).rotate_left(29)
    }

    fn provided(&self, value: Provided<T>, can_override: bool) -> ProvidedValue<T> {
        ProvidedValue {
            local: self.id(),
            name: self.name,
            is_static: self.is_static,
            can_override,
            equivalent: self.equivalent,
            value,
        }
    }

    /// `value` for this local, as `local provides value`.
    pub fn provides(&self, value: T) -> ProvidedValue<T> {
        self.provided(Provided::Value(value), true)
    }

    /// `value` for this local unless a provider above provides it already, as
    /// `local providesDefault value`.
    pub fn provides_default(&self, value: T) -> ProvidedValue<T> {
        self.provided(Provided::Value(value), false)
    }

    /// A value computed from other locals where this one is read, as
    /// `local providesComputed { … }`. A new function makes the provider recompose its content.
    /// In a `#[composable]` function, a closure written in the call is memoized on its captures,
    /// as lambdas are (D717): the provider recomposing with the same captures keeps the function.
    pub fn provides_computed(
        &self,
        compute: impl Fn(&CompositionLocalAccessorScope) -> T + 'static,
    ) -> ProvidedValue<T> {
        self.provided(Provided::Computed(Rc::new(compute)), true)
    }

    /// [`provides_computed`](Self::provides_computed) of a function memoized on its captures:
    /// what the `#[composable]` macro makes of `provides_computed(closure)` (D717).
    #[doc(hidden)]
    pub fn provides_computed_memoized(
        &self,
        compute: crate::callback::Callback<dyn Fn(&CompositionLocalAccessorScope) -> T>,
    ) -> ProvidedValue<T> {
        self.provided(Provided::Computed(compute.into_rc()), true)
    }

    /// The value of the nearest provider above the composable composing now, or the default,
    /// as `current`. Reading a dynamic local subscribes the composing scope: a new value
    /// recomposes it.
    ///
    /// # Panics
    ///
    /// Outside composition.
    pub fn current(&self) -> T
    where
        T: Clone,
    {
        let here = || CompositionLocalAccessorScope { map: None, anchor: None };
        match find_provided(self.id()) {
            Some(holder) => holder
                .downcast_ref::<Holder<T>>()
                .expect("a local's type is in its identity")
                .read(here),
            None => self.default_value(here),
        }
    }

    /// Its default, a computed one where `scope` says: made only for one.
    fn default_value(&self, scope: impl FnOnce() -> CompositionLocalAccessorScope) -> T {
        match self.default {
            LocalDefault::Value(default) => default(),
            LocalDefault::Computed(compute) => compute(&scope()),
            LocalDefault::Host { key, resolve } => {
                let host = scope().current_value(&LOCAL_HOST_DEFAULT_PROVIDER);
                resolve(key, host.get_host_default(key))
            }
        }
    }
}

/// The holder of the nearest provider of `local` above the composable composing now.
fn find_provided(local: u64) -> Option<Rc<dyn Any>> {
    with_runtime(|runtime| {
        let composition = runtime.check_composing();
        let data = runtime.compositions.data(composition);
        let composing = runtime.frames.last().map_or(NONE, |frame| frame.group);
        let group = data.table.provider_at_or_above(composing);
        find_from(&runtime.compositions, data, group, local)
    })
}

/// The holder of `local` in the provider `group` of `data` or those above it, going on above a
/// child composition's root at its context.
fn find_from(
    compositions: &Compositions,
    data: &CompositionData,
    group: u32,
    local: u64,
) -> Option<Rc<dyn Any>> {
    let (mut data, mut group) = (data, group);
    loop {
        let table = &data.table;
        while group != NONE {
            if let Some(holder) = entries_of(table, group).and_then(|entries| {
                let entries = entries.entries.borrow();
                entries.iter().find(|entry| entry.local == local).map(|entry| entry.holder.clone())
            }) {
                return Some(holder);
            }
            group = table[group].enclosing_provider;
        }
        match above_root(compositions, data) {
            Some(above) => (data, group) = above,
            None => return in_kept_locals(compositions, data, local),
        }
    }
}

/// The composition a child composition's root is under and the nearest provider there: where
/// what's provided goes on from, as Compose's `parentContext.getCompositionLocalScope()`.
fn above_root<'a>(
    compositions: &'a Compositions,
    data: &CompositionData,
) -> Option<(&'a CompositionData, u32)> {
    compositions.context_provider(data.parent?)
}

/// The holder of `local` in what `data`'s context provided when it went, as Compose's keeps it.
fn in_kept_locals(
    compositions: &Compositions,
    data: &CompositionData,
    local: u64,
) -> Option<Rc<dyn Any>> {
    let map = compositions.kept_locals_of(data.parent?)?;
    map.entries.iter().find(|entry| entry.local == local).map(|entry| entry.holder.clone())
}

/// What's provided at `group`, a provider of `data`, and above it, as a map.
pub(crate) fn map_at(
    compositions: &Compositions,
    data: &CompositionData,
    group: u32,
) -> CompositionLocalMap {
    CompositionLocalMap { entries: entries_visible_from(compositions, data, group).into() }
}

/// A provider group's entries.
fn entries_of(table: &crate::slot_table::SlotTable, group: u32) -> Option<Rc<ProviderEntries>> {
    entries_at(table, group).cloned()
}

/// A provider group's entries, where its first slot holds them.
#[inline]
fn entries_at(table: &crate::slot_table::SlotTable, group: u32) -> Option<&Rc<ProviderEntries>> {
    match &table[group].slots.first()?.value {
        crate::slot_table::SlotValue::Plain(value)
        | crate::slot_table::SlotValue::KeptOnReuse(value) => {
            value.downcast_ref::<Rc<ProviderEntries>>()
        }
        _ => None,
    }
}

/// `group` of `table`, a provider, moved there, of `id`, with content: its entries say so, for its
/// anchors.
pub(crate) fn provider_moved(table: &crate::slot_table::SlotTable, group: u32, id: CompositionId) {
    if let Some(entries) = entries_at(table, group) {
        Places::set(entries.place, (id, group));
    }
}

/// What the providers above the composable composing now provide, nearest first, a local
/// once: what [`current_composition_local_map`] and [`current_composition_local_context`]
/// capture.
fn visible_entries() -> Vec<Entry> {
    with_runtime(|runtime| {
        let composition = runtime.check_composing();
        let data = runtime.compositions.data(composition);
        let composing = runtime.frames.last().map_or(NONE, |frame| frame.group);
        let group = data.table.provider_at_or_above(composing);
        entries_visible_from(&runtime.compositions, data, group)
    })
}

/// What `group`, a provider, and those above it provide, nearest first, a local once.
fn entries_visible_from(
    compositions: &Compositions,
    data: &CompositionData,
    group: u32,
) -> Vec<Entry> {
    let (mut data, mut group) = (data, group);
    let mut visible: Vec<Entry> = Vec::new();
    loop {
        let table = &data.table;
        while group != NONE {
            if let Some(entries) = entries_of(table, group) {
                for entry in entries.entries.borrow().iter() {
                    if !visible.iter().any(|seen| seen.local == entry.local) {
                        visible.push(Entry { owned: false, given: false, ..entry.clone() });
                    }
                }
            }
            group = table[group].enclosing_provider;
        }
        let Some(above) = above_root(compositions, data) else {
            if let Some(map) = data.parent.and_then(|place| compositions.kept_locals_of(place)) {
                for entry in map.entries.iter() {
                    if !visible.iter().any(|seen| seen.local == entry.local) {
                        visible.push(Entry { owned: false, given: false, ..entry.clone() });
                    }
                }
            }
            return visible;
        };
        (data, group) = above;
    }
}

/// Whether two maps hold the same holders, in any order: what's provided at two places, maybe of
/// two compositions, is the same.
pub(crate) fn same_entries(first: &CompositionLocalMap, second: &CompositionLocalMap) -> bool {
    first.entries.len() == second.entries.len()
        && first.entries.iter().all(|entry| {
            second
                .entries
                .iter()
                .any(|other| other.local == entry.local && Rc::ptr_eq(&other.holder, &entry.holder))
        })
}

/// Whether what's provided at two provider groups of a composition is the same.
pub(crate) fn same_locals_at(
    compositions: &Compositions,
    composition: u32,
    first: u32,
    second: u32,
) -> bool {
    let data = compositions.data(composition);
    let visible = |group| entries_visible_from(compositions, data, group);
    let (first, second) = (visible(first), visible(second));
    first.len() == second.len()
        && first.iter().all(|entry| {
            second
                .iter()
                .any(|other| other.local == entry.local && Rc::ptr_eq(&other.holder, &entry.holder))
        })
}

/// Whether a provider above the provider group starting now provides `local`.
fn provided_above(local: u64) -> bool {
    with_runtime(|runtime| {
        let composition = runtime.check_composing();
        let data = runtime.compositions.data(composition);
        let provider = runtime.frames.last().map_or(NONE, |frame| frame.group);
        let table = &data.table;
        let group = if provider == NONE { NONE } else { table[provider].enclosing_provider };
        find_from(&runtime.compositions, data, group, local).is_some()
    })
}

/// The locals provided at a point, as Compose's `CompositionLocalMap`
/// (`currentComposer.currentCompositionLocalMap`): [`get`](CompositionLocalMap::get) reads one
/// as `current()` would have there.
#[derive(Clone, Default)]
pub struct CompositionLocalMap {
    entries: Rc<[Entry]>,
}

impl CompositionLocalMap {
    /// A map providing nothing, as `CompositionLocalMap.Empty`: every local reads its default.
    pub fn empty() -> CompositionLocalMap {
        CompositionLocalMap::default()
    }

    /// The value of `local` in the map, or its default. Reading a dynamic local during
    /// composition subscribes the composing scope, as reading it through `current()`.
    pub fn get<T: Clone + 'static>(&self, local: &CompositionLocal<T>) -> T {
        let scope = || CompositionLocalAccessorScope { map: Some(self.clone()), anchor: None };
        let id = local.id();
        match self.entries.iter().find(|entry| entry.local == id) {
            Some(entry) => entry
                .holder
                .downcast_ref::<Holder<T>>()
                .expect("a local's type is in its identity")
                .read(scope),
            None => local.default_value(scope),
        }
    }
}

impl std::fmt::Debug for CompositionLocalMap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "CompositionLocalMap({} locals)", self.entries.len())
    }
}

impl ComposeArg for CompositionLocalMap {
    /// The same holders, as Compose's map `equals` compares its holders.
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.entries, &other.entries)
            || (self.entries.len() == other.entries.len()
                && self
                    .entries
                    .iter()
                    .zip(other.entries.iter())
                    .all(|(a, b)| a.local == b.local && Rc::ptr_eq(&a.holder, &b.holder)))
    }
}

/// The locals provided where [`current_composition_local_map`] is called.
pub fn current_composition_local_map() -> CompositionLocalMap {
    CompositionLocalMap { entries: visible_entries().into() }
}

/// Where the locals are read at a point of a composition: what a layout node keeps, as Compose's
/// `LayoutNode.compositionLocalMap`, for its modifier nodes' `currentValueOf`. Made per layout
/// node, so it's the nearest provider group and nothing else: no map is built, and
/// [`get`](CompositionLocalAnchor::get) walks the providers when asked, outside composition too.
/// Two anchors are equal while what they read can only have changed through a dynamic local's
/// state: a provider's static value or its set of locals changing moves its version
/// (`Group::provider_version`) and those of the providers in its content, not anyone else's.
///
/// It names its provider by the cell its entries hold (`PLACES`), which says where the provider's
/// group is, moved with content to another composition too, as Compose's `Anchor` does (D1511):
/// nothing is kept for it but the provider's own cell. An anchor whose provider has moved on
/// (another version), or is gone, reads defaults: its node is about to be composed again, or gone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CompositionLocalAnchor {
    /// Its provider's place cell; `index` is `NONE` where no provider is above, and then
    /// `generation` and `top` are the composition at the top of those it reads.
    place: PlaceId,
    /// Its provider's version; with no provider, the top composition's index.
    version: u32,
}

impl Default for CompositionLocalAnchor {
    fn default() -> CompositionLocalAnchor {
        CompositionLocalAnchor::NONE
    }
}

impl CompositionLocalAnchor {
    /// Reads no provider: every local reads its default.
    pub const NONE: CompositionLocalAnchor =
        CompositionLocalAnchor { place: PlaceId { index: NONE, generation: 0 }, version: NONE };

    /// No provider above, the composition at the top of those it reads being `id`.
    fn top(id: CompositionId) -> CompositionLocalAnchor {
        CompositionLocalAnchor {
            place: PlaceId { index: NONE, generation: id.generation },
            version: id.index,
        }
    }

    /// The composition at the top of those it reads, with no provider above.
    fn top_id(&self) -> Option<CompositionId> {
        (self.place.index == NONE)
            .then_some(CompositionId { index: self.version, generation: self.place.generation })
    }

    /// Its provider's composition and group, where they are now, if the provider is still at its
    /// version. A composition being disposed is still there for its nodes' `on_detach`.
    fn resolve<'a>(&self, runtime: &'a Runtime) -> Option<(&'a CompositionData, u32)> {
        if self.place.index == NONE {
            return None;
        }
        let (id, group) = Places::get(self.place)?;
        Some((provider_at(runtime, id, group, self.version)?, group))
    }

    /// `action` at the end of a frame of the tree this anchor's composition is in, on its
    /// recomposer, as [`Composition::schedule_frame_end_callback`](crate::Composition): another
    /// tree's frame doesn't run it (once its composition is gone, any frame's end does). `None`
    /// when no frame will end: the composition's no longer known, or its recomposer is shut down.
    /// [`NONE`](Self::NONE) has no composition: any frame of the thread's recomposer.
    pub fn schedule_frame_end_callback(
        &self,
        action: impl FnOnce() + 'static,
    ) -> Option<crate::recomposer::CancellationHandle> {
        // A composition of its tree: its provider's, where that is now, or the top one it reads
        // with no provider above (a gone provider's is no longer known).
        let composition = match self.top_id() {
            Some(id) => id,
            None => Places::get(self.place)?.0,
        };
        let (owner, recomposer) = if composition.index == NONE {
            (None, None)
        } else {
            let recomposer = with_runtime(|runtime| {
                let disposing = runtime.disposing_data.iter().find(|(id, _)| *id == composition);
                match disposing {
                    Some((_, data)) => Some(data.recomposer),
                    None => runtime.compositions.get(composition).map(|data| data.recomposer),
                }
            })?;
            (Some(composition), recomposer)
        };
        let recomposer = recomposer.unwrap_or_else(crate::recomposer::Recomposer::current_thread);
        recomposer
            .is_open_for_frames()
            .then(|| recomposer.schedule_frame_end_callback_of(owner, Box::new(action)))
    }

    /// The value of `local` here, or its default, as `currentValueOf`. A dynamic local is a
    /// state's value: reading it is observed where reads are (measuring, drawing, a composing
    /// scope). An anchor outlives nothing: its composition gone, it reads defaults.
    pub fn get<T: Clone + 'static>(&self, local: &CompositionLocal<T>) -> T {
        let scope = || CompositionLocalAccessorScope { map: None, anchor: Some(*self) };
        let id = local.id();
        match self.find(id) {
            Some(holder) => holder
                .downcast_ref::<Holder<T>>()
                .expect("a local's type is in its identity")
                .read(scope),
            None => local.default_value(scope),
        }
    }

    /// What's provided here, as a map: for a node reading locals often, each read then a look
    /// through what's provided rather than a walk up the providers.
    pub fn map(&self) -> CompositionLocalMap {
        if self.place.index == NONE {
            return CompositionLocalMap::empty();
        }
        let entries = with_runtime(|runtime| {
            self.resolve(runtime).map_or_else(Vec::new, |(data, provider)| {
                entries_visible_from(&runtime.compositions, data, provider)
            })
        });
        CompositionLocalMap { entries: entries.into() }
    }

    /// Whether static `locals` read here what they read at `other`, known with a look at two
    /// provider groups: `other`'s nearest, the one around it, or a sibling of it, none of those
    /// that differ providing them. Saves the walk up to the providers of a layout node's density
    /// when a list's item is a provider of its own (`false` only says it isn't known).
    #[doc(hidden)]
    pub fn reads_same_statics_as(&self, other: &CompositionLocalAnchor, locals: &[u64]) -> bool {
        if self.place == other.place {
            return self.version == other.version;
        }
        if self.place.index == NONE || other.place.index == NONE {
            return false;
        }
        let (Some((id, mine)), Some((their_id, theirs))) =
            (Places::get(self.place), Places::get(other.place))
        else {
            return false;
        };
        if id != their_id {
            return false;
        }
        let version = other.version;
        with_runtime(|runtime| {
            // Gone or moved on, what it gave `other` isn't known.
            let Some(data) = provider_at(runtime, their_id, theirs, version) else { return false };
            let table = &data.table;
            let Some(my_group) = table.groups.get(mine as usize) else { return false };
            let their_group = &table.groups[theirs as usize];
            let provides = |group: u32| {
                entries_at(table, group).is_some_and(|entries| {
                    let entries = entries.entries.borrow();
                    entries.iter().any(|entry| locals.contains(&entry.local))
                })
            };
            // Versions only grow: what's around it changed after `other` read it if its version
            // is the later, the nodes being composed one at a time.
            let unchanged_since = |around: u32| {
                around == NONE || table.groups[around as usize].provider_version() < version
            };
            if my_group.enclosing_provider == theirs {
                !provides(mine)
            } else if their_group.enclosing_provider == mine {
                unchanged_since(mine) && !provides(theirs)
            } else {
                my_group.enclosing_provider == their_group.enclosing_provider
                    && unchanged_since(my_group.enclosing_provider)
                    && !provides(mine)
                    && !provides(theirs)
            }
        })
    }

    fn find(&self, local: u64) -> Option<Rc<dyn Any>> {
        if self.place.index == NONE {
            return None;
        }
        with_runtime(|runtime| {
            let (data, provider) = self.resolve(runtime)?;
            find_from(&runtime.compositions, data, provider, local)
        })
    }
}

/// `id`'s data, being disposed or not, if its group `provider` is a provider at `version`.
fn provider_at(
    runtime: &Runtime,
    id: CompositionId,
    provider: u32,
    version: u32,
) -> Option<&CompositionData> {
    let disposing = runtime.disposing_data.iter().find(|(other, _)| *other == id);
    let data = match disposing {
        Some((_, data)) => data,
        None => runtime.compositions.get(id)?,
    };
    let group = data.table.groups.get(provider as usize)?;
    // A provider's: another group's `node` field is its node, whose bits may be any version.
    (group.kind != crate::slot_table::GroupKind::Free
        && group.is_provider()
        && group.provider_version() == version)
        .then_some(data)
}

/// The anchor of the composable composing now. Costs a look at its group and its provider's
/// entries: what every layout node's emission asks.
pub fn current_composition_local_anchor() -> CompositionLocalAnchor {
    with_runtime(|runtime| anchor_here(runtime))
}

/// [`current_composition_local_anchor`] of a runtime composing.
pub(crate) fn anchor_here(runtime: &Runtime) -> CompositionLocalAnchor {
    let composition = runtime.check_composing();
    let mut id = runtime.active.expect("composing").composition;
    let compositions = &runtime.compositions;
    let mut data = compositions.data(composition);
    let composing = runtime.frames.last().map_or(NONE, |frame| frame.group);
    let mut provider = data.table.provider_at_or_above(composing);
    // In a child composition with none of its own: what's above its root is what it reads, so
    // no provider here says there's none at all.
    while provider == NONE {
        let Some((above, above_data, place)) =
            data.parent.and_then(|place| compositions.context_at(place))
        else {
            break;
        };
        (id, data) = (above, above_data);
        provider = data.table.provider_at_or_above(place);
    }
    if provider == NONE {
        return CompositionLocalAnchor::top(id);
    }
    let version = data.table[provider].provider_version();
    match entries_at(&data.table, provider) {
        Some(entries) => CompositionLocalAnchor { place: entries.place, version },
        // A provider whose entries aren't remembered yet: it provides nothing to find.
        None => CompositionLocalAnchor { place: NO_PLACE, version },
    }
}

impl CompositionLocalAnchor {
    /// Its provider and version if it's of `composition`, as a group's nearest provider there is:
    /// no provider (`NONE`) for a child composition reading above its root, and `None` if its
    /// provider is gone, which nothing's known of.
    pub(crate) fn provider_in(&self, composition: CompositionId) -> Option<(u32, u32)> {
        if self.place.index == NONE {
            return Some((NONE, 0));
        }
        let (id, group) = Places::get(self.place)?;
        Some(if id == composition { (group, self.version) } else { (NONE, 0) })
    }

    /// What was provided here, if it's still known: its provider as it was (where it is, or moved
    /// with content), or no provider in a composition still there (D1511). Its provider's version
    /// standing for what's provided above it too, in other compositions, is the invariant
    /// `Runtime::composed_under` states: a child's pass after a change above it composes from its
    /// root (`providers_changed`).
    pub(crate) fn map_if_known(&self, runtime: &Runtime) -> Option<CompositionLocalMap> {
        let (data, provider) = match self.top_id() {
            Some(id) => (runtime.compositions.get(id)?, NONE),
            None => self.resolve(runtime)?,
        };
        Some(map_at(&runtime.compositions, data, provider))
    }
}

/// Every local provided at a point, to provide elsewhere (another composition, a dialog), as
/// Compose's `CompositionLocalContext`: give it to [`CompositionLocalProvider`]. A dynamic
/// local keeps following its provider's value.
#[derive(Clone, Debug, Default)]
pub struct CompositionLocalContext {
    map: CompositionLocalMap,
}

impl ComposeArg for CompositionLocalContext {
    fn same(&self, other: &Self) -> bool {
        self.map.same(&other.map)
    }
}

/// The locals provided where it's called, as `currentCompositionLocalContext`.
pub fn current_composition_local_context() -> CompositionLocalContext {
    CompositionLocalContext { map: current_composition_local_map() }
}

/// What a provider applies: the entries it had, updated by what it's given now.
#[doc(hidden)]
pub struct ProviderUpdate {
    /// The entries, each marked as it's given.
    entries: Vec<Entry>,
    /// An entry was added, removed or replaced: the content recomposes as a whole (Compose's
    /// `providersInvalid`).
    changed: bool,
}

impl ProviderUpdate {
    fn entry(&self, local: u64) -> Option<usize> {
        self.entries.iter().position(|entry| entry.local == local)
    }

    fn replace(&mut self, entry: Entry) {
        self.changed = true;
        match self.entry(entry.local) {
            Some(index) => self.entries[index] = entry,
            None => self.entries.push(entry),
        }
    }

    fn give<T: Clone + 'static>(&mut self, provided: ProvidedValue<T>) {
        let ProvidedValue { local, is_static, can_override, equivalent, value, .. } = provided;
        // `providesDefault`: nothing if a provider above provides it.
        if !can_override && provided_above(local) {
            return;
        }
        let existing = self.entry(local).map(|index| {
            self.entries[index].given = true;
            self.entries[index].clone()
        });
        let holder = existing.as_ref().and_then(|entry| {
            entry.holder.clone().downcast::<Holder<T>>().ok().map(|holder| (holder, entry.owned))
        });
        match (holder, value) {
            // A dynamic value: written in place, its readers recompose.
            (Some((holder, true)), Provided::Value(value)) if !is_static => {
                if let Holder::Dynamic(state) = &*holder {
                    state.set(value);
                    return;
                }
                self.replace(new_entry(local, is_static, equivalent, Provided::Value(value)));
            }
            (Some((holder, _)), Provided::Value(value)) if is_static => {
                if let Holder::Static(old) = &*holder
                    && equivalent(old, &value)
                {
                    return;
                }
                self.replace(new_entry(local, is_static, equivalent, Provided::Value(value)));
            }
            (Some((holder, _)), Provided::Computed(compute)) => {
                if let Holder::Computed(old) = &*holder
                    && Rc::ptr_eq(old, &compute)
                {
                    return;
                }
                self.replace(new_entry(local, is_static, equivalent, Provided::Computed(compute)));
            }
            (_, value) => self.replace(new_entry(local, is_static, equivalent, value)),
        }
    }

    fn give_context(&mut self, context: CompositionLocalContext) {
        for entry in context.map.entries.iter() {
            let index = self.entry(entry.local);
            if let Some(index) = index {
                self.entries[index].given = true;
            }
            let same =
                index.is_some_and(|index| Rc::ptr_eq(&self.entries[index].holder, &entry.holder));
            if !same {
                self.replace(Entry { owned: false, given: true, ..entry.clone() });
            }
        }
    }
}

/// A new holder for a provided value.
fn new_entry<T: Clone + 'static>(
    local: u64,
    is_static: bool,
    equivalent: fn(&T, &T) -> bool,
    value: Provided<T>,
) -> Entry {
    let holder = match value {
        Provided::Value(value) if is_static => Holder::Static(value),
        Provided::Value(value) => {
            Holder::Dynamic(crate::state::mutable_state_with_equivalence(value, equivalent))
        }
        Provided::Computed(compute) => Holder::Computed(compute),
    };
    Entry { local, holder: Rc::new(holder), owned: true, given: true }
}

/// What [`CompositionLocalProvider`] takes: a [`ProvidedValue`], a tuple of them, a `Vec` or an
/// array of them (Compose's varargs), or a [`CompositionLocalContext`].
pub trait ProvidedValues {
    #[doc(hidden)]
    fn give(self, update: &mut ProviderUpdate);
}

impl<T: Clone + 'static> ProvidedValues for ProvidedValue<T> {
    fn give(self, update: &mut ProviderUpdate) {
        update.give(self);
    }
}

impl<T: Clone + 'static> ProvidedValues for Vec<ProvidedValue<T>> {
    fn give(self, update: &mut ProviderUpdate) {
        for value in self {
            update.give(value);
        }
    }
}

impl<T: Clone + 'static, const N: usize> ProvidedValues for [ProvidedValue<T>; N] {
    fn give(self, update: &mut ProviderUpdate) {
        for value in self {
            update.give(value);
        }
    }
}

impl ProvidedValues for CompositionLocalContext {
    fn give(self, update: &mut ProviderUpdate) {
        update.give_context(self);
    }
}

macro_rules! tuple_values {
    ($($name:ident),+) => {
        impl<$($name: ProvidedValues),+> ProvidedValues for ($($name,)+) {
            #[allow(non_snake_case)]
            fn give(self, update: &mut ProviderUpdate) {
                let ($($name,)+) = self;
                $($name.give(update);)+
            }
        }
    };
}

tuple_values!(A);
tuple_values!(A, B);
tuple_values!(A, B, C);
tuple_values!(A, B, C, D);
tuple_values!(A, B, C, D, E);
tuple_values!(A, B, C, D, E, F);
tuple_values!(A, B, C, D, E, F, G);
tuple_values!(A, B, C, D, E, F, G, H);

/// Provides `values` to `content`, as Compose's `CompositionLocalProvider(values) { … }`:
/// one value, a tuple of them, a `Vec` or an array, or a [`CompositionLocalContext`]. A dynamic
/// local's new value recomposes the scopes that read it, and only those; a static local's new
/// value, or a change in what's provided, recomposes the whole content.
#[allow(non_snake_case)]
pub fn CompositionLocalProvider(values: impl ProvidedValues, content: impl FnOnce()) {
    with_composition_locals(values, content)
}

/// Provides `value` to `content` and returns what it returns, as `withCompositionLocal`.
pub fn with_composition_local<R>(value: impl ProvidedValues, content: impl FnOnce() -> R) -> R {
    with_composition_locals(value, content)
}

/// Provides `values` to `content` and returns what it returns, as `withCompositionLocals`.
pub fn with_composition_locals<R>(values: impl ProvidedValues, content: impl FnOnce() -> R) -> R {
    let (group, place) = start_provider();
    // Where it is from now on, kept by `move_group` if it moves to another composition's table.
    let entries: Rc<ProviderEntries> = remember(|| Rc::new(ProviderEntries::new(place)));
    let inserting = crate::composer::current().inserting();
    let mut update = ProviderUpdate {
        entries: std::mem::take(&mut *entries.entries.borrow_mut()),
        changed: false,
    };
    for entry in &mut update.entries {
        entry.given = false;
    }
    values.give(&mut update);
    let ProviderUpdate { entries: mut updated, mut changed } = update;
    // Locals no longer given.
    let before = updated.len();
    updated.retain(|entry| entry.given);
    changed |= updated.len() != before;
    let dropped = std::mem::replace(&mut *entries.entries.borrow_mut(), updated);
    drop(dropped);
    if changed && !inserting {
        with_runtime(|runtime| runtime.invalidate_providers());
    }
    let result = content();
    drop(group);
    result
}

/// Starts a provider's group: it, and where it is.
fn start_provider() -> (ReplaceGroup, (CompositionId, u32)) {
    let group = ReplaceGroup::start(PROVIDER_KEY, "compose_runtime::CompositionLocalProvider");
    let place = with_runtime(|runtime| runtime.mark_provider());
    (group, place)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A provider's place cell taken by the next provider made: an anchor of the first names
    /// nothing there, though the second's version were its own (versions don't repeat on a thread,
    /// so only the cell's generation tells them apart here).
    #[test]
    fn an_anchor_of_a_provider_gone_names_nothing_in_its_cell_taken_again() {
        let composition = CompositionId { index: 3, generation: 1 };
        let first = Places::make((composition, 7));
        let anchor = CompositionLocalAnchor { place: first, version: 5 };
        assert_eq!(anchor.provider_in(composition), Some((7, 5)));
        Places::set(first, (composition, 8));
        assert_eq!(anchor.provider_in(composition), Some((8, 5)), "moved");
        Places::free(first);
        let second = Places::make((composition, 9));
        assert_eq!(second.index, first.index, "the cell taken again");
        assert_eq!(anchor.provider_in(composition), None);
        Places::set(first, (composition, 10));
        let theirs = CompositionLocalAnchor { place: second, version: 5 };
        assert_eq!(theirs.provider_in(composition), Some((9, 5)), "not moved by the first's");
        assert_ne!(anchor, theirs);
        Places::free(second);
    }
}
