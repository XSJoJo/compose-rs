//! `SubcomposeLayout`, as Compose's: a layout that composes parts of its content while it's measured,
//! each slot a composition under this place, its nodes under a virtual node of the layout's
//! (composition-context.md §3.4, D1309). Measuring holds the tree, so a slot's pass borrows it
//! (`with_lent_applier`), with the access window shut: what asks the tree from a slot's effect in that
//! pass finds it busy (D1211).

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use compose_runtime::platform::{NodeRef, with_lent_applier};
use compose_runtime::snapshots::Snapshot;
use compose_runtime::{
    Callback, Composition, CompositionContext, DisposableEffect, ReusableComposition, SideEffect,
    composable, remember_composition_context,
};

use crate::constraints::Constraints;
use crate::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult,
    MeasureScope,
};
use crate::tree::{NodeId, UiTree};
use crate::{IntSize, Modifier};

/// A slot's id: anything comparable, as Compose's `Any?`.
pub trait SlotId: Any {
    fn same_slot(&self, other: &dyn SlotId) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + 'static> SlotId for T {
    fn same_slot(&self, other: &dyn SlotId) -> bool {
        other.as_any().downcast_ref::<T>().is_some_and(|other| self == other)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

type SlotKey = Rc<dyn SlotId>;

/// The slots a reuse policy's asked about, as `SlotIdsSet`, oldest used first.
#[derive(Default)]
pub struct SlotIdsSet {
    ids: Vec<SlotKey>,
}

impl SlotIdsSet {
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// The ids, in order.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Any> {
        self.ids.iter().map(|id| (**id).as_any())
    }

    /// Keeps only those `keep` says, as `retainAll`.
    pub fn retain(&mut self, mut keep: impl FnMut(&dyn Any) -> bool) {
        self.ids.retain(|id| keep((**id).as_any()));
    }

    /// Keeps the first `size`, as `trimToSize`.
    pub fn trim_to_size(&mut self, size: usize) {
        self.ids.truncate(size);
    }

    fn contains(&self, id: &dyn SlotId) -> bool {
        self.ids.iter().any(|other| other.same_slot(id))
    }
}

/// Which slots are kept for reuse, and what they can be reused for, as `SubcomposeSlotReusePolicy`.
pub trait SubcomposeSlotReusePolicy: 'static {
    /// Leaves in `slots` those to keep, the rest being disposed.
    fn get_slots_to_retain(&self, slots: &mut SlotIdsSet);
    /// Whether a composition kept for `reusable` can compose `slot`'s content.
    fn are_compatible(&self, slot: &dyn Any, reusable: &dyn Any) -> bool;
}

struct FixedCount(usize);

impl SubcomposeSlotReusePolicy for FixedCount {
    fn get_slots_to_retain(&self, slots: &mut SlotIdsSet) {
        slots.trim_to_size(self.0);
    }

    fn are_compatible(&self, _slot: &dyn Any, _reusable: &dyn Any) -> bool {
        true
    }
}

struct NoReuse;

impl SubcomposeSlotReusePolicy for NoReuse {
    fn get_slots_to_retain(&self, slots: &mut SlotIdsSet) {
        slots.clear();
    }

    fn are_compatible(&self, _slot: &dyn Any, _reusable: &dyn Any) -> bool {
        false
    }
}

/// A policy keeping up to `max_slots_to_retain_for_reuse`, each usable for any slot, as
/// `SubcomposeSlotReusePolicy(maxSlotsToRetainForReuse)`.
pub fn subcompose_slot_reuse_policy(
    max_slots_to_retain_for_reuse: usize,
) -> impl SubcomposeSlotReusePolicy {
    FixedCount(max_slots_to_retain_for_reuse)
}

/// A slot. In `Subcompositions::slots`, in the order of its virtual nodes: those of this measurement,
/// then those kept for reuse, then those precomposed.
struct Slot {
    node: NodeId,
    key: SlotKey,
    composition: Option<Rc<ReusableComposition>>,
    content: Option<Callback>,
    force_recompose: bool,
    force_reuse: bool,
    active: bool,
}

#[derive(Default)]
struct Subcompositions {
    root: Option<NodeRef>,
    context: Option<CompositionContext>,
    /// A composition of the layout's tree, composing nothing: what the tree's reached through.
    handle: Option<Rc<Composition>>,
    slots: Vec<Slot>,
    current_index: usize,
    reusable_count: usize,
    precomposed_count: usize,
}

/// What a `SubcomposeLayout` keeps, as `SubcomposeLayoutState`: its slots, for it to reuse and for
/// precomposing ahead of a measurement. Shared by `Rc`, so a test or a lazy list holds one.
pub struct SubcomposeLayoutState {
    policy: RefCell<Rc<dyn SubcomposeSlotReusePolicy>>,
    inner: RefCell<Subcompositions>,
}

impl SubcomposeLayoutState {
    /// With no slots kept for reuse, as `SubcomposeLayoutState()`.
    pub fn new() -> Rc<SubcomposeLayoutState> {
        SubcomposeLayoutState::with_policy(NoReuse)
    }

    pub fn with_policy(policy: impl SubcomposeSlotReusePolicy) -> Rc<SubcomposeLayoutState> {
        Rc::new(SubcomposeLayoutState {
            policy: RefCell::new(Rc::new(policy)),
            inner: RefCell::default(),
        })
    }

    /// A new policy, as the state object being replaced does: what's kept is asked of it next.
    pub fn set_policy(&self, policy: impl SubcomposeSlotReusePolicy) {
        *self.policy.borrow_mut() = Rc::new(policy);
    }

    fn set_place(&self, context: CompositionContext, root: NodeRef) {
        let mut inner = self.inner.borrow_mut();
        inner.context = Some(context);
        inner.root = Some(root);
        if inner.handle.is_none() {
            inner.handle = Some(Rc::new(compose_runtime::platform::subcomposition(context, root)));
        }
    }

    /// The body of the layout ran: each slot composes anew at its next measurement, as what it's
    /// under may have changed in ways no state tells (`forceRecomposeChildren`).
    fn force_recompose_children(&self) {
        let (handle, root) = {
            let mut inner = self.inner.borrow_mut();
            for slot in &mut inner.slots {
                slot.force_recompose = true;
            }
            if inner.slots.is_empty() {
                return;
            }
            (inner.handle.clone(), inner.root)
        };
        if let (Some(handle), Some(root)) = (handle, root) {
            handle.with_applier(|tree: &mut UiTree| {
                let root = tree.id_of(root);
                tree.invalidate_measure(root);
            });
        }
    }

    /// Its slots' compositions go with it, as its `onRelease`.
    fn dispose_current_nodes(&self) {
        let (slots, handle) = {
            let mut inner = self.inner.borrow_mut();
            inner.current_index = 0;
            inner.reusable_count = 0;
            inner.precomposed_count = 0;
            inner.root = None;
            (std::mem::take(&mut inner.slots), inner.handle.take())
        };
        for slot in slots {
            if let Some(composition) = slot.composition {
                composition.dispose();
            }
        }
        if let Some(handle) = handle {
            handle.dispose();
        }
    }

    fn slot_index(&self, key: &dyn SlotId, range: std::ops::Range<usize>) -> Option<usize> {
        let inner = self.inner.borrow();
        range.into_iter().find(|at| inner.slots[*at].key.same_slot(key))
    }

    /// The slot of `key` among those kept for reuse, or one compatible, moved to the front of
    /// those, out of them: `takeNodeFromReusables`. Returns where it is now.
    fn take_from_reusables(&self, key: &SlotKey, tree: &mut UiTree) -> Option<usize> {
        let policy = self.policy.borrow().clone();
        let mut inner = self.inner.borrow_mut();
        if inner.reusable_count == 0 {
            return None;
        }
        let end = inner.slots.len() - inner.precomposed_count;
        let start = end - inner.reusable_count;
        let chosen =
            (start..end).rev().find(|at| inner.slots[*at].key.same_slot(&**key)).or_else(|| {
                (start..end).rev().find(|at| {
                    policy.are_compatible((**key).as_any(), (*inner.slots[*at].key).as_any())
                })
            })?;
        let root = tree.id_of(inner.root.expect("it's measured or precomposed under a place"));
        move_slot(&mut inner, tree, root, chosen, start);
        inner.reusable_count -= 1;
        let slot = &mut inner.slots[start];
        slot.key = key.clone();
        slot.active = true;
        slot.force_reuse = true;
        slot.force_recompose = true;
        Some(start)
    }

    /// `at`'s composition composes `content` if it has to. `lend` is the tree a measurement holds.
    fn compose_slot(&self, at: usize, content: Callback, lend: Option<&mut UiTree>) {
        let (compose, handle) = {
            let mut inner = self.inner.borrow_mut();
            let context = inner.context.expect("under a place");
            let handle = inner.handle.clone().expect("under a place");
            let slot = &mut inner.slots[at];
            let pending = slot.composition.as_ref().is_none_or(|it| it.has_invalidations());
            let same = slot.content.as_ref().is_some_and(|last| last.ptr_eq(&content));
            if same && !pending && !slot.force_recompose {
                return;
            }
            slot.content = Some(content.clone());
            slot.force_recompose = false;
            let reuse = std::mem::take(&mut slot.force_reuse);
            let root = slot.node.to_ref();
            let composition = slot
                .composition
                .get_or_insert_with(|| {
                    Rc::new(compose_runtime::platform::reusable_subcomposition(context, root))
                })
                .clone();
            ((composition, reuse), handle)
        };
        let (composition, reuse) = compose;
        // A closure as a `Callback` isn't an `Fn` of its own.
        #[allow(clippy::redundant_closure)]
        let pass = move || {
            if reuse {
                composition.set_content_with_reuse(move || content());
            } else {
                composition.set_content(move || content());
            }
        };
        let root = self.inner.borrow().root.expect("under a place");
        with_the_tree(&handle, root, lend, pass);
    }

    /// From `start` on: those the policy retains are kept for reuse, deactivated, and the others
    /// disposed: `disposeOrReuseStartingFromIndex`.
    fn dispose_or_reuse_from(&self, start: usize, tree: Option<&mut UiTree>) {
        let policy = self.policy.borrow().clone();
        let mut set = SlotIdsSet::default();
        let (last, handle) = {
            let mut inner = self.inner.borrow_mut();
            inner.reusable_count = 0;
            let end = inner.slots.len() - inner.precomposed_count;
            if start >= end {
                return;
            }
            set.ids = (start..end).map(|at| inner.slots[at].key.clone()).collect();
            (end, inner.handle.clone().expect("under a place"))
        };
        policy.get_slots_to_retain(&mut set);
        let root = self.inner.borrow().root.expect("under a place");
        let mut gone = Vec::new();
        let leave = || {
            for at in (start..last).rev() {
                let keep = {
                    let inner = self.inner.borrow();
                    set.contains(&*inner.slots[at].key)
                };
                if keep {
                    let composition = {
                        let mut inner = self.inner.borrow_mut();
                        inner.reusable_count += 1;
                        let slot = &mut inner.slots[at];
                        let was_active = std::mem::replace(&mut slot.active, false);
                        slot.force_reuse = true;
                        slot.composition.clone().filter(|_| was_active)
                    };
                    if let Some(composition) = composition {
                        composition.deactivate();
                    }
                } else {
                    let slot = self.inner.borrow_mut().slots.remove(at);
                    if let Some(composition) = &slot.composition {
                        composition.dispose();
                    }
                    gone.push(slot.node);
                }
            }
        };
        // Their virtual nodes go once their compositions have, and the tree's back in hand.
        match tree {
            Some(tree) => {
                with_the_tree(&handle, root, Some(&mut *tree), leave);
                let host = tree.id_of(root);
                let ignoring = tree.ignoring_remeasure.replace(host);
                for node in gone {
                    tree.remove_virtual_node(host, node);
                }
                tree.ignoring_remeasure = ignoring;
            }
            None => {
                with_the_tree(&handle, root, None, leave);
                with_the_tree(&handle, root, None, || {
                    handle.with_applier(|tree: &mut UiTree| {
                        let host = tree.id_of(root);
                        for node in gone {
                            tree.remove_virtual_node(host, node);
                        }
                    });
                });
            }
        }
    }

    /// `content` under `slot_id`, ahead of the measurement that will ask for it, as `precompose`.
    /// A slot already composed in a measurement stays as it is, and the handle has nothing of it.
    pub fn precompose(
        self: &Rc<Self>,
        slot_id: impl PartialEq + 'static,
        content: Callback,
    ) -> PrecomposedSlotHandle {
        let key: SlotKey = Rc::new(slot_id);
        let (handle, root, end) = {
            let inner = self.inner.borrow();
            let Some(handle) = inner.handle.clone() else {
                return PrecomposedSlotHandle { state: self.clone(), key: None };
            };
            let end = inner.slots.len();
            (handle, inner.root.expect("with its handle"), end)
        };
        let active_end = {
            let inner = self.inner.borrow();
            end - inner.precomposed_count - inner.reusable_count
        };
        if self.slot_index(&*key, 0..active_end).is_some() {
            return PrecomposedSlotHandle { state: self.clone(), key: None };
        }
        let precomposed = end - self.inner.borrow().precomposed_count;
        let at = match self.slot_index(&*key, precomposed..end) {
            Some(at) => at,
            None => handle.with_applier(|tree: &mut UiTree| {
                let host = tree.id_of(root);
                let ignoring = tree.ignoring_remeasure.replace(host);
                let at = match self.take_from_reusables(&key, tree) {
                    Some(taken) => {
                        let mut inner = self.inner.borrow_mut();
                        let last = inner.slots.len() - 1;
                        move_slot(&mut inner, tree, host, taken, last);
                        last
                    }
                    None => {
                        let mut inner = self.inner.borrow_mut();
                        let at = inner.slots.len();
                        add_slot(&mut inner, tree, host, key.clone(), at);
                        at
                    }
                };
                tree.ignoring_remeasure = ignoring;
                self.inner.borrow_mut().precomposed_count += 1;
                at
            }),
        };
        self.compose_slot(at, content, None);
        PrecomposedSlotHandle { state: self.clone(), key: Some(key) }
    }

    fn precomposed_at(&self, key: &SlotKey) -> Option<usize> {
        let end = self.inner.borrow().slots.len();
        let start = end - self.inner.borrow().precomposed_count;
        self.slot_index(&**key, start..end)
    }

    /// Its slots are its layout's virtual nodes, as `makeSureStateIsConsistent`.
    fn check_consistent(&self, tree: &UiTree) {
        if cfg!(debug_assertions) {
            let inner = self.inner.borrow();
            let root = tree.id_of(inner.root.expect("measured under a place"));
            let nodes = &tree.node(root).virtual_children;
            assert!(
                nodes.iter().eq(inner.slots.iter().map(|slot| &slot.node)),
                "compose-ui: a SubcomposeLayout's slots aren't its virtual nodes"
            );
        }
    }

    /// How many are kept for reuse, for tests and tools.
    pub fn reusable_count(&self) -> usize {
        self.inner.borrow().reusable_count
    }
}

/// `f` where a pass reaches the tree: lent by what holds it, or as it is, what it does to the
/// layout's children no reason to measure it again.
fn with_the_tree<R>(
    handle: &Rc<Composition>,
    root: NodeRef,
    tree: Option<&mut UiTree>,
    f: impl FnOnce() -> R,
) -> R {
    match tree {
        Some(tree) => {
            let host = tree.id_of(root);
            let ignoring = tree.ignoring_remeasure.replace(host);
            let result = Snapshot::without_read_observation(|| with_lent_applier(handle, tree, f));
            tree.ignoring_remeasure = ignoring;
            result
        }
        None => {
            let ignoring = handle.with_applier(|tree: &mut UiTree| {
                let host = tree.id_of(root);
                tree.ignoring_remeasure.replace(host)
            });
            let result = Snapshot::without_read_observation(f);
            handle.with_applier(|tree: &mut UiTree| tree.ignoring_remeasure = ignoring);
            result
        }
    }
}

fn add_slot(inner: &mut Subcompositions, tree: &mut UiTree, host: NodeId, key: SlotKey, at: usize) {
    let node = tree.add_virtual_node(host, at);
    inner.slots.insert(
        at,
        Slot {
            node,
            key,
            composition: None,
            content: None,
            force_recompose: false,
            force_reuse: false,
            active: true,
        },
    );
}

fn move_slot(inner: &mut Subcompositions, tree: &mut UiTree, host: NodeId, from: usize, to: usize) {
    if from == to {
        return;
    }
    let slot = inner.slots.remove(from);
    inner.slots.insert(to, slot);
    tree.move_virtual_node(host, from, to);
}

/// What can be done with a slot precomposed, as `PrecomposedSlotHandle`.
pub struct PrecomposedSlotHandle {
    state: Rc<SubcomposeLayoutState>,
    key: Option<SlotKey>,
}

impl PrecomposedSlotHandle {
    /// Done with it: it's kept for reuse or disposed, as the policy says, unless a measurement's
    /// taken it.
    pub fn dispose(&self) {
        let Some(key) = &self.key else { return };
        let Some(at) = self.state.precomposed_at(key) else { return };
        let (handle, root, start) = {
            let mut inner = self.state.inner.borrow_mut();
            inner.reusable_count += 1;
            inner.precomposed_count -= 1;
            let start = inner.slots.len() - inner.precomposed_count - inner.reusable_count;
            (inner.handle.clone().expect("it's precomposed"), inner.root.expect("too"), start)
        };
        with_the_tree(&handle, root, None, || {
            handle.with_applier(|tree: &mut UiTree| {
                let host = tree.id_of(root);
                let mut inner = self.state.inner.borrow_mut();
                move_slot(&mut inner, tree, host, at, start);
            });
        });
        self.state.dispose_or_reuse_from(start, None);
    }

    fn node(&self) -> Option<(Rc<Composition>, NodeId)> {
        let at = self.state.precomposed_at(self.key.as_ref()?)?;
        let inner = self.state.inner.borrow();
        Some((inner.handle.clone()?, inner.slots[at].node))
    }

    /// How many layouts it emitted, as `placeablesCount`.
    pub fn placeables_count(&self) -> usize {
        let Some((handle, node)) = self.node() else { return 0 };
        handle.with_applier(|tree: &mut UiTree| tree.node(node).children.len())
    }

    /// Measures the layout at `index`, as `premeasure`: the measurement that takes the slot finds
    /// it measured with these constraints.
    ///
    /// # Panics
    ///
    /// With an `index` it doesn't have, as Compose's.
    pub fn premeasure(&self, index: usize, constraints: Constraints) {
        let (handle, node) = self.node().expect("compose-ui: premeasure of a slot it doesn't hold");
        handle.with_applier(|tree: &mut UiTree| {
            let count = tree.node(node).children.len();
            assert!(index < count, "Index ({index}) is out of bound of [0, {count})");
            let child = tree.node(node).children[index].node;
            tree.measure_node(child, constraints);
        });
    }

    /// The size of the layout at `index`, as `getSize`.
    pub fn get_size(&self, index: usize) -> IntSize {
        let Some((handle, node)) = self.node() else { return IntSize::new(0, 0) };
        handle.with_applier(|tree: &mut UiTree| {
            let count = tree.node(node).children.len();
            assert!(index < count, "Index ({index}) is out of bound of [0, {count})");
            let child = tree.node(node).children[index].node;
            tree.node(child).layers[0].size
        })
    }
}

/// What a `SubcomposeLayout`'s policy measures in: a [`MeasureScope`] that also composes.
pub struct SubcomposeMeasureScope<'s, 'a> {
    scope: &'s mut MeasureScope<'a>,
    state: &'s Rc<SubcomposeLayoutState>,
}

impl<'a> std::ops::Deref for SubcomposeMeasureScope<'_, 'a> {
    type Target = MeasureScope<'a>;

    fn deref(&self) -> &MeasureScope<'a> {
        self.scope
    }
}

impl<'a> std::ops::DerefMut for SubcomposeMeasureScope<'_, 'a> {
    fn deref_mut(&mut self) -> &mut MeasureScope<'a> {
        self.scope
    }
}

impl SubcomposeMeasureScope<'_, '_> {
    /// `content` under `slot_id` in this measurement, composed if it's new or has changed, as
    /// `subcompose`: its layouts, to measure.
    ///
    /// # Panics
    ///
    /// For a slot already composed in this measurement, as Compose's.
    pub fn subcompose(
        &mut self,
        slot_id: impl PartialEq + 'static,
        content: Callback,
    ) -> Vec<Measurable> {
        let key: SlotKey = Rc::new(slot_id);
        let state = self.state.clone();
        let current = state.inner.borrow().current_index;
        assert!(
            state.slot_index(&*key, 0..current).is_none(),
            "Error: currently it is not supported to subcompose the same slot id twice in a \
             measurement"
        );
        let mut tree = self.scope.base.tree.enter();
        let tree: &mut UiTree = &mut tree;
        let root = {
            let inner = state.inner.borrow();
            tree.id_of(inner.root.expect("measured under its place"))
        };
        let (active_end, end) = {
            let inner = state.inner.borrow();
            let end = inner.slots.len();
            (end - inner.precomposed_count - inner.reusable_count, end)
        };
        let found = state.slot_index(&*key, current..active_end).or_else(|| {
            let precomposed = end - state.inner.borrow().precomposed_count;
            let at = state.slot_index(&*key, precomposed..end)?;
            state.inner.borrow_mut().precomposed_count -= 1;
            Some(at)
        });
        let at = match found.or_else(|| state.take_from_reusables(&key, tree)) {
            Some(at) => at,
            None => {
                let mut inner = state.inner.borrow_mut();
                add_slot(&mut inner, tree, root, key.clone(), current);
                current
            }
        };
        {
            let mut inner = state.inner.borrow_mut();
            move_slot(&mut inner, tree, root, at, current);
            inner.current_index = current + 1;
        }
        state.compose_slot(current, content, Some(tree));
        let inner = state.inner.borrow();
        tree.virtual_measurables(inner.slots[current].node)
    }
}

/// Measures a `SubcomposeLayout`, as a [`MeasurePolicy`] does a `Layout`.
pub trait SubcomposeMeasurePolicy: 'static {
    fn measure(
        &self,
        scope: &mut SubcomposeMeasureScope<'_, '_>,
        constraints: Constraints,
    ) -> MeasureResult;
}

impl<F> SubcomposeMeasurePolicy for F
where
    F: Fn(&mut SubcomposeMeasureScope<'_, '_>, Constraints) -> MeasureResult + 'static,
{
    fn measure(
        &self,
        scope: &mut SubcomposeMeasureScope<'_, '_>,
        constraints: Constraints,
    ) -> MeasureResult {
        self(scope, constraints)
    }
}

/// A closure as a policy, its signature inferred.
pub fn subcompose_policy<F>(policy: F) -> Rc<dyn SubcomposeMeasurePolicy>
where
    F: Fn(&mut SubcomposeMeasureScope<'_, '_>, Constraints) -> MeasureResult + 'static,
{
    Rc::new(policy)
}

struct Adapter {
    state: Rc<SubcomposeLayoutState>,
    policy: Rc<dyn SubcomposeMeasurePolicy>,
}

impl MeasurePolicy for Adapter {
    fn measure(
        &self,
        scope: &mut MeasureScope<'_>,
        _measurables: &[Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        self.state.inner.borrow_mut().current_index = 0;
        let result = {
            let mut scope = SubcomposeMeasureScope { scope: &mut *scope, state: &self.state };
            self.policy.measure(&mut scope, constraints)
        };
        let start = self.state.inner.borrow().current_index;
        let mut tree = scope.base.tree.enter();
        self.state.dispose_or_reuse_from(start, Some(&mut tree));
        self.state.check_consistent(&tree);
        result
    }

    fn same(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<Adapter>().is_some_and(|other| {
            Rc::ptr_eq(&self.state, &other.state) && Rc::ptr_eq(&self.policy, &other.policy)
        })
    }

    fn min_intrinsic_width(
        &self,
        _: &mut IntrinsicMeasureScope<'_>,
        _: &[IntrinsicMeasurable],
        _: i32,
    ) -> i32 {
        panic!("{}", crate::layout::NO_INTRINSICS_MESSAGE)
    }

    fn min_intrinsic_height(
        &self,
        _: &mut IntrinsicMeasureScope<'_>,
        _: &[IntrinsicMeasurable],
        _: i32,
    ) -> i32 {
        panic!("{}", crate::layout::NO_INTRINSICS_MESSAGE)
    }

    fn max_intrinsic_width(
        &self,
        _: &mut IntrinsicMeasureScope<'_>,
        _: &[IntrinsicMeasurable],
        _: i32,
    ) -> i32 {
        panic!("{}", crate::layout::NO_INTRINSICS_MESSAGE)
    }

    fn max_intrinsic_height(
        &self,
        _: &mut IntrinsicMeasureScope<'_>,
        _: &[IntrinsicMeasurable],
        _: i32,
    ) -> i32 {
        panic!("{}", crate::layout::NO_INTRINSICS_MESSAGE)
    }

    fn debug_name(&self) -> &'static str {
        "SubcomposeLayout"
    }
}

/// A layout that composes parts of its content while it's measured, as `SubcomposeLayout`, with
/// state of its own, keeping nothing for reuse.
#[composable]
#[allow(non_snake_case)]
pub fn SubcomposeLayout(
    #[default(Modifier)] modifier: Modifier,
    measure_policy: Rc<dyn SubcomposeMeasurePolicy>,
) {
    let state = compose_runtime::remember(SubcomposeLayoutState::new);
    SubcomposeLayoutWithState!(state, modifier = modifier, measure_policy = measure_policy);
}

/// [`SubcomposeLayout`] with `state`, as `SubcomposeLayout(state, …)`.
#[composable]
#[allow(non_snake_case)]
pub fn SubcomposeLayoutWithState(
    state: Rc<SubcomposeLayoutState>,
    #[default(Modifier)] modifier: Modifier,
    measure_policy: Rc<dyn SubcomposeMeasurePolicy>,
) {
    let context = remember_composition_context();
    let adapter: Rc<dyn MeasurePolicy> =
        Rc::new(Adapter { state: state.clone(), policy: measure_policy });
    let node = crate::layout::emit_layout_node(modifier, adapter, || {});
    compose_runtime::platform::current().with_applier(|tree: &mut UiTree| {
        let id = tree.id_of(node);
        tree.node_mut(id).composes_while_measured = true;
    });
    state.set_place(context, node);
    let told = state.clone();
    SideEffect(move || told.force_recompose_children());
    let released = state.clone();
    DisposableEffect((), move |scope| scope.on_dispose(move || released.dispose_current_nodes()));
}
