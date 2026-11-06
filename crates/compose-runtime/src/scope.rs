//! Recompose scopes: one per restart group, invalidated by state writes.

use std::marker::PhantomData;

use crate::runtime::{invalidate_scope, request_frames, with_runtime};

/// A generational handle to a recompose scope in the thread's scope arena.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ScopeId {
    pub index: u32,
    pub generation: u32,
}

impl ScopeId {
    pub const NONE: ScopeId = ScopeId { index: u32::MAX, generation: 0 };

    pub fn is_none(self) -> bool {
        self.index == u32::MAX
    }
}

pub(crate) struct ScopeEntry {
    /// Odd while the scope is live; bumped on allocation and on free.
    pub generation: u32,
    /// The index of the composition the scope belongs to.
    pub composition: u32,
    /// Its restart group in that composition's slot table.
    pub group: u32,
    /// Bumped each time the scope's body runs; a state read records it, so reads from an
    /// earlier run that weren't repeated are recognised as stale.
    pub epoch: u32,
    /// Invalidated: its body runs in the next pass. With `conditions`, only if one of them
    /// changed.
    pub invalid: bool,
    /// Invalid only on conditions (`Scopes::conditions`): derived states whose dependencies
    /// changed since the scope read them (Compose's conditional invalidation). The pass reaching
    /// it recomputes them, and runs its body only if one changed.
    pub conditional: bool,
    /// Its body was paused by a paused composition (Compose's `RecomposeScopeImpl.paused`): it's
    /// invalid, and the paused composition's next resume runs it, from its start.
    pub paused: bool,
    /// Paused while content was being reused: resumed, it reuses its content still (Compose's
    /// `RecomposeScopeImpl.reusing`).
    pub reusing: bool,
    /// Paused: the paused composition's segment its remember observers and effects go to, so
    /// they're dispatched in the order they would have been without the pause (Compose's
    /// `rememberPausingScope`).
    pub segment: u32,
    /// The last compaction of a cell's readers that kept a reader of this scope: a compaction
    /// keeps one reader per scope (`Scopes::keep_reader`).
    compaction: u32,
}

#[derive(Default)]
pub(crate) struct Scopes {
    entries: Vec<ScopeEntry>,
    free: Vec<u32>,
    /// The generation of new entries (odd): after a reset, above every earlier one.
    base: u32,
    /// The latest compaction of a cell's readers (`begin_compaction`); 0 is never one.
    compaction: u32,
    /// How many compactions of readers ran on this thread (for tests: their cost is bounded).
    pub compactions: u64,
    /// The conditions of the scopes invalid on conditions (`ScopeEntry::conditional`), by index:
    /// rare, so kept out of the entries, which a pass scans.
    conditions: crate::hash::KeyMap<u32, Vec<crate::state::StateId>>,
    /// While a composition has an observer (`tooling`): scope events, to tell outside the borrow.
    pub log: Option<Vec<crate::tooling::ScopeEvent>>,
    /// How many compositions have an observer.
    observed: u32,
    /// The last observer set's token.
    observer_tokens: u64,
}

impl Scopes {
    pub const fn new() -> Scopes {
        Scopes {
            entries: Vec::new(),
            free: Vec::new(),
            base: 1,
            compaction: 0,
            compactions: 0,
            conditions: crate::hash::KeyMap::with_hasher(std::hash::BuildHasherDefault::new()),
            log: None,
            observed: 0,
            observer_tokens: 0,
        }
    }

    /// A composition has an observer now: scope events are logged. Gives its token.
    pub fn observer_set(&mut self) -> u64 {
        self.observed += 1;
        self.log.get_or_insert_with(Vec::new);
        self.observer_tokens += 1;
        self.observer_tokens
    }

    /// A composition's observer was removed or replaced.
    pub fn observer_removed(&mut self) {
        self.observed -= 1;
        self.stop_logging_if_unobserved();
    }

    /// Nothing's logged once no composition has an observer and what was logged is told.
    pub fn stop_logging_if_unobserved(&mut self) {
        if self.observed == 0 && self.log.as_ref().is_some_and(Vec::is_empty) {
            self.log = None;
        }
    }

    /// Logs a scope event, if compositions are observed: a pass with none pays one test.
    #[inline(always)]
    pub fn log(&mut self, composition: u32, scope: ScopeId, kind: crate::tooling::ScopeEventKind) {
        if self.log.is_some() {
            self.log_observed(composition, scope, kind);
        }
    }

    #[cold]
    #[inline(never)]
    fn log_observed(
        &mut self,
        composition: u32,
        scope: ScopeId,
        kind: crate::tooling::ScopeEventKind,
    ) {
        if let Some(log) = &mut self.log {
            log.push(crate::tooling::ScopeEvent { composition, scope, kind, observer: None });
        }
    }

    /// Makes a scope valid or invalid outright: no conditions any more.
    pub fn clear_conditions(&mut self, id: ScopeId) {
        if let Some(entry) = self.get_mut(id)
            && std::mem::take(&mut entry.conditional)
        {
            self.conditions.remove(&id.index);
        }
    }

    /// Adds a condition to a scope invalid on conditions.
    pub fn add_condition(&mut self, id: ScopeId, cell: crate::state::StateId) {
        if let Some(entry) = self.get_mut(id) {
            entry.conditional = true;
            let conditions = self.conditions.entry(id.index).or_default();
            if !conditions.contains(&cell) {
                conditions.push(cell);
            }
        }
    }

    /// The conditions of a scope invalid on conditions.
    pub fn conditions(&self, id: ScopeId) -> Vec<crate::state::StateId> {
        self.conditions.get(&id.index).cloned().unwrap_or_default()
    }

    pub fn alloc(&mut self, composition: u32, group: u32) -> ScopeId {
        if let Some(index) = self.free.pop() {
            let entry = &mut self.entries[index as usize];
            entry.generation = entry.generation.wrapping_add(1);
            entry.composition = composition;
            entry.group = group;
            entry.epoch = 0;
            entry.invalid = false;
            entry.conditional = false;
            entry.paused = false;
            entry.reusing = false;
            entry.segment = 0;
            entry.compaction = 0;
            ScopeId { index, generation: entry.generation }
        } else {
            let index = u32::try_from(self.entries.len()).expect("too many recompose scopes");
            self.entries.push(ScopeEntry {
                generation: self.base,
                composition,
                group,
                epoch: 0,
                invalid: false,
                conditional: false,
                paused: false,
                reusing: false,
                segment: 0,
                compaction: 0,
            });
            ScopeId { index, generation: self.base }
        }
    }

    pub fn free(&mut self, id: ScopeId) {
        if self.log.is_some() {
            let composition = self.entries[id.index as usize].composition;
            self.log_observed(composition, id, crate::tooling::ScopeEventKind::Disposed);
        }
        let entry = &mut self.entries[id.index as usize];
        debug_assert_eq!(entry.generation, id.generation, "freeing a stale scope");
        entry.generation = entry.generation.wrapping_add(1);
        entry.invalid = false;
        if std::mem::take(&mut entry.conditional) {
            self.conditions.remove(&id.index);
        }
        entry.paused = false;
        self.free.push(id.index);
    }

    pub fn get(&self, id: ScopeId) -> Option<&ScopeEntry> {
        self.entries.get(id.index as usize).filter(|entry| entry.generation == id.generation)
    }

    pub fn get_mut(&mut self, id: ScopeId) -> Option<&mut ScopeEntry> {
        self.entries.get_mut(id.index as usize).filter(|entry| entry.generation == id.generation)
    }

    /// Whether a read recorded at `epoch` still belongs to the scope's latest run.
    pub fn is_current(&self, id: ScopeId, epoch: u32) -> bool {
        self.get(id).is_some_and(|entry| entry.epoch == epoch)
    }

    /// Starts compacting a cell's readers: [`keep_reader`](Scopes::keep_reader) then keeps one
    /// current reader per scope.
    pub fn begin_compaction(&mut self) {
        self.compactions += 1;
        self.compaction = self.compaction.wrapping_add(1);
        if self.compaction == 0 {
            // Wrapped: a scope's mark from 2^32 compactions ago must not read as this one's.
            for entry in &mut self.entries {
                entry.compaction = 0;
            }
            self.compaction = 1;
        }
    }

    /// Whether a compaction keeps a reader recorded by `id` at `epoch`: it belongs to the
    /// scope's latest run, and no other reader of the scope was kept in this compaction.
    pub fn keep_reader(&mut self, id: ScopeId, epoch: u32) -> bool {
        let compaction = self.compaction;
        match self.get_mut(id) {
            Some(entry) if entry.epoch == epoch && entry.compaction != compaction => {
                entry.compaction = compaction;
                true
            }
            _ => false,
        }
    }

    pub fn set_base(&mut self, base: u32) {
        self.base = base;
    }

    pub fn next_base(&self) -> u32 {
        crate::runtime::next_base(self.base, self.entries.iter().map(|entry| entry.generation))
    }

    pub fn live_count(&self) -> usize {
        self.entries.len() - self.free.len()
    }
}

/// The recompose scope of a restartable composable, as Compose's `RecomposeScope`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RecomposeScope {
    pub(crate) id: ScopeId,
    pub(crate) _not_send: PhantomData<*const ()>,
}

impl RecomposeScope {
    pub(crate) fn of(id: ScopeId) -> RecomposeScope {
        RecomposeScope { id, _not_send: PhantomData }
    }

    /// Schedules the scope's composable to run again in the next pass. Does nothing if the scope
    /// has left the composition.
    pub fn invalidate(&self) {
        let requested = with_runtime(|runtime| {
            invalidate_scope(runtime, self.id) || crate::runtime::frames_requested(runtime)
        });
        if requested {
            request_frames();
        }
    }

    /// Whether the scope is still in a composition.
    pub fn is_valid(&self) -> bool {
        with_runtime(|runtime| runtime.scopes.get(self.id).is_some())
    }
}
