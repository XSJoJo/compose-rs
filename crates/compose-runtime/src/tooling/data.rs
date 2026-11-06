//! A composition's slot table as tools read it, as Compose's `tooling.CompositionData` (D1509).
//!
//! [`Composition::composition_data`](crate::Composition::composition_data) gives a
//! [`CompositionInstance`]: the composition's groups as a tree of [`CompositionGroup`]s, each
//! with its key, the composable that made it, its node and its slots. The handles are copies that
//! read the runtime when asked, valid while the table is as it was when they were read: a pass of
//! the composition since, or one under way, makes them panic, as Compose's iterators throw a
//! `ConcurrentModificationException`. A group's [`identity`](CompositionGroup::identity) stays
//! valid across passes while the group is there, and [`CompositionData::find`] finds it again.
//!
//! [`LOCAL_INSPECTION_TABLES`], provided, collects the compositions made under it, as Compose's
//! `LocalInspectionTables`: each pass of one adds it, and its disposal takes it out.
//!
//! Behind the `composition-data` feature, off by default: without it a pass takes no step for it.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::CompositionLocal;
use crate::applier::NodeRef;
use crate::composer::Identity;
use crate::error::{ComposeError, misuse};
use crate::runtime::{CompositionData as Data, CompositionId, with_runtime};
use crate::slot_table::{GroupKind, NONE, SENTINEL, SlotTable, SlotValue};

/// What composition tracked, read as a tree of groups, as Compose's `CompositionData`: a
/// composition's ([`CompositionInstance`]) or a group's ([`CompositionGroup`]).
pub trait CompositionData {
    /// Its groups, in composition order, as `compositionGroups`.
    ///
    /// # Panics
    ///
    /// If the table changed since this was read, or a pass of it is under way.
    fn composition_groups(&self) -> CompositionGroups;

    /// Whether it has no groups, as `isEmpty`: a composition not composed yet, or a group with no
    /// group in it.
    fn is_empty(&self) -> bool;

    /// The group with `identity` in it, as `find`: `None` if it's not there.
    fn find(&self, _identity: GroupIdentity) -> Option<CompositionGroup> {
        None
    }

    /// The composition, if this is a composition's data, as `findCompositionInstance`: a group's
    /// is `None`.
    fn find_composition_instance(&self) -> Option<CompositionInstance> {
        None
    }
}

/// A composition's data, as Compose's `CompositionInstance`, and Compose's `Composer`'s
/// `compositionData`. Equal for the same composition.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CompositionInstance {
    id: CompositionId,
}

/// A group of a composition's slot table, as Compose's `CompositionGroup`: a composable's, a
/// branch's, a key's or a node's. Equal when read from the same group of the same table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CompositionGroup {
    id: CompositionId,
    group: u32,
    /// The table's writes when it was read.
    writes: u64,
}

/// What names a group across passes, as `CompositionGroup.identity`: Compose's anchor. It stays
/// the group's while the group is there, moved among its siblings or not.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GroupIdentity {
    id: CompositionId,
    group: u32,
    /// Its keys: an index given to another group since doesn't name that one.
    keys: (u64, u64),
}

/// The groups of a composition or of a group, as an iterator.
pub struct CompositionGroups {
    id: CompositionId,
    next: u32,
    writes: u64,
}

/// The compositions made under a place, as Compose's `LocalInspectionTables` holds.
pub type InspectionTables = Rc<RefCell<HashSet<CompositionInstance>>>;

/// Provided, collects the compositions made under it, as Compose's `LocalInspectionTables`: a pass
/// of one adds it, its disposal removes it. A static local, as Compose's.
pub static LOCAL_INSPECTION_TABLES: CompositionLocal<Option<InspectionTables>> =
    CompositionLocal::new_static("LocalInspectionTables", || None);

/// The data of the composition composing now, as `currentComposer.compositionData`.
///
/// # Panics
///
/// Outside composition.
pub fn current_composition_data() -> CompositionInstance {
    with_runtime(|runtime| {
        let index = runtime.check_composing();
        CompositionInstance { id: runtime.compositions.id_at(index) }
    })
}

impl CompositionInstance {
    pub(crate) fn of(id: CompositionId) -> CompositionInstance {
        CompositionInstance { id }
    }

    /// The composition whose place it's under, as `parent`: `None` for a root.
    pub fn parent(&self) -> Option<CompositionInstance> {
        with_runtime(|runtime| {
            let compositions = &runtime.compositions;
            let place = compositions.get(self.id)?.parent?;
            compositions.contexts.parent_of(place).map(|id| CompositionInstance { id })
        })
    }

    /// The group of its parent's that holds its context, as `findContextGroup`: `None` for a root,
    /// or once that group is gone.
    pub fn find_context_group(&self) -> Option<CompositionGroup> {
        with_runtime(|runtime| {
            let compositions = &runtime.compositions;
            let place = compositions.get(self.id)?.parent?;
            let (id, data, group) = compositions.context_at(place)?;
            Some(CompositionGroup { id, group, writes: readable(data) })
        })
    }
}

impl CompositionData for CompositionInstance {
    fn composition_groups(&self) -> CompositionGroups {
        with_runtime(|runtime| match runtime.compositions.get(self.id) {
            Some(data) => CompositionGroups {
                id: self.id,
                next: data.table[SENTINEL].first_child,
                writes: readable(data),
            },
            None => CompositionGroups { id: self.id, next: NONE, writes: 0 },
        })
    }

    fn is_empty(&self) -> bool {
        with_runtime(|runtime| {
            runtime.compositions.get(self.id).is_none_or(|data| has_no_children(data, SENTINEL))
        })
    }

    fn find(&self, identity: GroupIdentity) -> Option<CompositionGroup> {
        find_below(self.id, SENTINEL, identity)
    }

    fn find_composition_instance(&self) -> Option<CompositionInstance> {
        Some(*self)
    }
}

impl CompositionGroup {
    /// Its key, as `key`: the composable's, the call's or `key(…)`'s site's.
    pub fn key(&self) -> u64 {
        self.read(|_, table, group| table[group].key)
    }

    /// For a group of `key(…)` and reusable content, the key's value, hashed: what Compose's `key`
    /// gives as an object.
    pub fn data_key(&self) -> Option<u64> {
        self.read(|_, table, group| {
            let group = &table[group];
            matches!(group.kind, GroupKind::Movable | GroupKind::Reusable).then_some(group.data_key)
        })
    }

    /// The composable that made it, by its path (`app::screen::Linear`), as `sourceInfo`: `None`
    /// for a group no `#[composable]` made. Compose's is the compiler's source information.
    pub fn source_info(&self) -> Option<&'static str> {
        self.read(|_, table, group| match table[group].identity {
            Some(Identity::Type(_, name) | Identity::Name(name)) => Some(path_of(name)),
            None => None,
        })
    }

    /// The node it emitted, for a node's group, as `node`.
    pub fn node(&self) -> Option<NodeRef> {
        self.read(|_, table, group| {
            let group = &table[group];
            (group.kind == GroupKind::Node && !group.node.is_none()).then_some(group.node)
        })
    }

    /// Its slots' values, as `data`: what `remember` keeps and what composables store; `None` for
    /// a slot forgotten. A restartable composable's arguments aren't in its slots here.
    pub fn data(&self) -> Vec<Option<Rc<dyn Any>>> {
        self.read(|_, table, group| table[group].slots.iter().map(slot_value).collect())
    }

    /// What names it across passes, as `identity`.
    pub fn identity(&self) -> GroupIdentity {
        self.read(|_, table, group| GroupIdentity {
            id: self.id,
            group,
            keys: (table[group].key, table[group].data_key),
        })
    }

    /// The groups it holds and itself, as `groupSize`.
    pub fn group_size(&self) -> usize {
        self.read(|_, table, group| {
            let mut size = 0;
            walk(table, group, &mut |_| size += 1);
            size
        })
    }

    /// The slots of it and of the groups it holds, as `slotsSize`.
    pub fn slots_size(&self) -> usize {
        self.read(|_, table, group| {
            let mut size = 0;
            walk(table, group, &mut |group| size += table[group].slots.len());
            size
        })
    }

    /// `f` of its table, if that's as it was read.
    fn read<R>(&self, f: impl FnOnce(&Data, &SlotTable, u32) -> R) -> R {
        with_runtime(|runtime| {
            let data = runtime.compositions.get(self.id);
            match data {
                Some(data) if readable(data) == self.writes => f(data, &data.table, self.group),
                _ => misuse(ComposeError::ConcurrentModification),
            }
        })
    }
}

impl CompositionData for CompositionGroup {
    fn composition_groups(&self) -> CompositionGroups {
        let next = self.read(|_, table, group| table[group].first_child);
        CompositionGroups { id: self.id, next, writes: self.writes }
    }

    fn is_empty(&self) -> bool {
        self.read(|data, _, group| has_no_children(data, group))
    }

    fn find(&self, identity: GroupIdentity) -> Option<CompositionGroup> {
        self.read(|_, _, _| ());
        find_below(self.id, self.group, identity)
    }
}

impl Iterator for CompositionGroups {
    type Item = CompositionGroup;

    fn next(&mut self) -> Option<CompositionGroup> {
        // Checked at the end too, as Compose's `hasNext`; a composition gone had none to give.
        with_runtime(|runtime| match runtime.compositions.get(self.id) {
            Some(data) if readable(data) == self.writes => {}
            None if self.next == NONE => {}
            _ => misuse(ComposeError::ConcurrentModification),
        });
        if self.next == NONE {
            return None;
        }
        let group = CompositionGroup { id: self.id, group: self.next, writes: self.writes };
        self.next = group.read(|_, table, group| table[group].next_sibling);
        Some(group)
    }
}

/// The count of `data`'s table's writes, refusing a table being written.
fn readable(data: &Data) -> u64 {
    if data.composing {
        misuse(ComposeError::ConcurrentModification)
    }
    data.writes
}

fn has_no_children(data: &Data, group: u32) -> bool {
    data.table[group].first_child == NONE
}

/// `f` of `group` and every group in it, depth first.
fn walk(table: &SlotTable, group: u32, f: &mut impl FnMut(u32)) {
    f(group);
    let mut child = table[group].first_child;
    while child != NONE {
        walk(table, child, f);
        child = table[child].next_sibling;
    }
}

/// The group `identity` names, if it's still there at or below `top` of composition `id`.
fn find_below(id: CompositionId, top: u32, identity: GroupIdentity) -> Option<CompositionGroup> {
    if identity.id != id {
        return None;
    }
    with_runtime(|runtime| {
        let data = runtime.compositions.get(id)?;
        let writes = readable(data);
        let table = &data.table;
        // Its index given to another group since, with other keys: not it.
        let found = table.groups.get(identity.group as usize)?;
        if (found.key, found.data_key) != identity.keys {
            return None;
        }
        // Linked into the table, under `top`: a group freed, or parked, isn't.
        let mut at = identity.group;
        while at != top {
            if at == SENTINEL || at == NONE {
                return None;
            }
            let parent = table[at].parent;
            if parent == NONE || !is_child(table, parent, at) {
                return None;
            }
            at = parent;
        }
        Some(CompositionGroup { id, group: identity.group, writes })
    })
}

/// Whether `child` is among `parent`'s children.
fn is_child(table: &SlotTable, parent: u32, child: u32) -> bool {
    let mut at = table[parent].first_child;
    while at != NONE {
        if at == child {
            return true;
        }
        at = table[at].next_sibling;
    }
    false
}

fn slot_value(slot: &crate::slot_table::Slot) -> Option<Rc<dyn Any>> {
    match &slot.value {
        SlotValue::Empty => None,
        value => value.to_any(),
    }
}

/// `app::screen::Linear` from `app::screen::Linear::{{closure}}`, generics dropped.
fn path_of(full: &'static str) -> &'static str {
    let mut path = full.split('<').next().unwrap_or(full);
    while let Some(shorter) = path.strip_suffix("::{{closure}}") {
        path = shorter;
    }
    path
}

/// A pass of a composition begins: what was read of its table is stale.
pub(crate) fn writing(data: &mut Data) {
    data.writes = data.writes.wrapping_add(1);
}

/// A pass of composition `id` has begun: if it's a child, an inspection table provided above it
/// collects it, as Compose's `startRoot`.
pub(crate) fn inspect(id: CompositionId) {
    let child = with_runtime(|runtime| {
        runtime.compositions.get(id).is_some_and(|data| data.parent.is_some())
    });
    if !child {
        return;
    }
    let Some(tables) = LOCAL_INSPECTION_TABLES.current() else { return };
    if let Ok(mut set) = tables.try_borrow_mut() {
        set.insert(CompositionInstance { id });
    }
    with_runtime(|runtime| {
        if let Some(data) = runtime.compositions.get_mut(id)
            && !data.inspection_tables.iter().any(|table| Rc::ptr_eq(table, &tables))
        {
            data.inspection_tables.push(tables);
        }
    });
}

/// Composition `id` is disposed: the tables that collected it let it go.
pub(crate) fn disposed(id: CompositionId, tables: Vec<InspectionTables>) {
    for table in tables {
        if let Ok(mut set) = table.try_borrow_mut() {
            set.remove(&CompositionInstance { id });
        }
    }
}
