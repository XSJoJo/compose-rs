//! A lazy layout's `BeyondBoundsLayout`, as Compose's `LazyLayoutBeyondBoundsModifierNode`
//! (`lazyLayoutBeyondBoundsModifier`): lists, grids and staggered grids lay out items beyond those
//! in view, one more at a time, measured at once, while a block looks for what it wants (D1479).
//!
//! What it asks for is laid out with what a focus search asks for (D477), the most of the two
//! each way, as Compose's intervals share its `beyondBoundsInfo` (D1480): the items (a grid's
//! lines) to lay out before and after those in view, and whether there are more each way, as
//! last laid out. Compose's intervals of item indices are counts from the items in view here: a
//! block runs with no scroll between its steps, and Compose's start from the items in view too.

use std::cell::{Cell, RefCell};

use compose_runtime::snapshots::Snapshot;
use compose_ui::{
    BeyondBoundsLayout, BeyondBoundsLayoutDirection, BeyondBoundsScope, Remeasurement,
};

use super::LazyListState;

/// How many viewports of items a layout lays out at most, as Compose's
/// `BeyondBoundsViewportFactor`.
const VIEWPORT_FACTOR: usize = 2;

/// A lazy layout's beyond-bounds layout: made when a node inside asks for it, kept by its node.
pub(crate) struct LazyBeyondBounds {
    state: Cell<LazyListState>,
    vertical: Cell<bool>,
    reverse: Cell<bool>,
    /// What measures the layout at once, while its node is attached.
    remeasurement: Cell<Option<Remeasurement>>,
    /// Whether its node still offers it: not once the user can't scroll the layout, as Compose's
    /// lazy layouts drop the modifier then (D1480). A new one is made if they can again.
    offered: Cell<bool>,
    /// The items each layout going on (re-entrant) asked for, before and after those in view:
    /// the state lays out the most of them.
    requests: RefCell<Vec<(usize, usize)>>,
}

/// What a block is told: whether there's more that way.
struct More(bool);

impl BeyondBoundsScope for More {
    fn has_more_content(&self) -> bool {
        self.0
    }
}

/// A layout's request, let go of when the layout ends, even by a panic in its block.
struct Request<'a> {
    layout: &'a LazyBeyondBounds,
    state: LazyListState,
    at: usize,
}

impl Drop for Request<'_> {
    fn drop(&mut self) {
        // The layouts inside it ended first: it's the last.
        self.layout.requests.borrow_mut().truncate(self.at);
        self.layout.apply(self.state);
    }
}

impl LazyBeyondBounds {
    pub(crate) fn new(
        state: LazyListState,
        vertical: bool,
        reverse: bool,
        remeasurement: Option<Remeasurement>,
    ) -> LazyBeyondBounds {
        LazyBeyondBounds {
            state: Cell::new(state),
            vertical: Cell::new(vertical),
            reverse: Cell::new(reverse),
            remeasurement: Cell::new(remeasurement),
            offered: Cell::new(true),
            requests: RefCell::new(Vec::new()),
        }
    }

    /// Its layout's state, axis and reversal, as its node's update says.
    pub(crate) fn update(&self, state: LazyListState, vertical: bool, reverse: bool) {
        self.state.set(state);
        self.vertical.set(vertical);
        self.reverse.set(reverse);
    }

    /// Its node is attached (`Some`) or detached.
    pub(crate) fn set_remeasurement(&self, remeasurement: Option<Remeasurement>) {
        self.remeasurement.set(remeasurement);
    }

    /// Its node no longer offers it (the user can't scroll the layout): it lays out nothing
    /// more, as Compose's node gone.
    pub(crate) fn withdraw(&self) {
        self.offered.set(false);
    }

    /// Whether it can lay out items now: offered by an attached node.
    fn live(&self) -> bool {
        self.offered.get() && self.remeasurement.get().is_some()
    }

    /// Whether `direction` goes towards the layout's end, as `isForward`, and whether it goes
    /// across its axis (then there's nothing that way).
    fn way(&self, direction: BeyondBoundsLayoutDirection, rtl: bool) -> (bool, bool) {
        use BeyondBoundsLayoutDirection::*;
        let (vertical, reverse) = (self.vertical.get(), self.reverse.get());
        match direction {
            Before => (false, false),
            After => (true, false),
            Above => (reverse, !vertical),
            Below => (!reverse, !vertical),
            Left => (reverse != rtl, vertical),
            Right => (reverse == rtl, vertical),
        }
    }

    /// The state lays out the most items any layout going on asked for, each way, besides what
    /// a focus search asks for (D477): its own counts.
    fn apply(&self, state: LazyListState) {
        let Some(inner) = state.inner() else { return };
        let requests = self.requests.borrow();
        let wanted =
            requests.iter().fold((0, 0), |(before, after), &(b, a)| (before.max(b), after.max(a)));
        // Not a read to observe: a layout asked for in a composition would recompose it.
        let laid = Snapshot::without_read_observation(|| inner.beyond_layouts.get());
        if requests.is_empty() && laid != (0, 0) {
            // What was laid out for it goes, disposed rather than kept for reuse (D482).
            inner.beyond_ended.set(true);
        }
        inner.beyond_layouts.set(wanted);
    }

    /// Measures the layout now, as `remeasureSync`, and returns whether it was: where the tree
    /// is busy (a composition, a measurement), it's measured at its next layout instead.
    fn remeasure(&self, state: LazyListState) -> bool {
        let Some(remeasurement) = self.remeasurement.get() else { return false };
        let measures = || state.inner().map(|inner| inner.measures.get());
        let before = measures();
        remeasurement.force_remeasure();
        measures() != before
    }
}

impl BeyondBoundsLayout for LazyBeyondBounds {
    fn layout_until(
        &self,
        direction: BeyondBoundsLayoutDirection,
        block: &mut dyn FnMut(&dyn BeyondBoundsScope) -> bool,
    ) {
        let state = self.state.get();
        let inner = state.inner();
        // Nothing laid out, no room for an item, or the node not attached: the block is told
        // there's nothing more, as Compose's.
        let (count, in_view) = inner.as_ref().map_or((0, 0), |inner| {
            let (count, _, _, viewport, _) = inner.layout_info_written.get();
            let shown = inner.laid_out.read_untracked(|laid, _| laid.iter().any(|it| !it.sticky));
            // As Compose's `itemsPerViewport`: the viewport over the average size of an item
            // in view (a grid's line, a staggered grid's item whatever its lane), with spacing.
            let step = inner.beyond_step.get();
            let in_view = match () {
                _ if !shown => 0,
                _ if step <= 0 => 1,
                _ => (viewport / step).max(1) as usize,
            };
            (count, in_view)
        });
        let Some(inner) = inner.filter(|_| count > 0 && in_view > 0) else {
            block(&More(false));
            return;
        };
        if !self.live() {
            block(&More(false));
            return;
        }
        let (forward, across) = self.way(direction, inner.rtl.get());
        drop(inner);
        let more = |state: LazyListState| {
            !across
                && state.inner().is_some_and(|inner| {
                    let (before, after) = inner.beyond_more.get();
                    if forward { after } else { before }
                })
        };
        let request = {
            let mut requests = self.requests.borrow_mut();
            requests.push((0, 0));
            Request { layout: self, state, at: requests.len() - 1 }
        };
        // At most two viewports of items (a grid's lines), as Compose's.
        let most = (VIEWPORT_FACTOR * in_view).min(count);
        let mut laid = 0;
        while self.live() && more(state) && laid < most {
            {
                let mut requests = self.requests.borrow_mut();
                let asked = &mut requests[request.at];
                if forward { asked.1 += 1 } else { asked.0 += 1 }
            }
            laid += 1;
            self.apply(state);
            let measured = self.remeasure(state);
            if block(&More(more(state))) {
                break;
            }
            if !measured {
                // The tree is busy: what it asked for is laid out at the next layout, after the
                // block ran, and no step shows it more. The block is told there's no more, once,
                // to end with what it has (D1480).
                block(&More(false));
                break;
            }
        }
        // What it laid out goes.
        drop(request);
        if laid > 0 {
            self.remeasure(state);
        }
    }
}
