//! A lazy layout's `BeyondBoundsLayout`, as Compose's `LazyLayoutBeyondBoundsModifierNode`
//! (`lazyLayoutBeyondBoundsModifier`): lists, grids and staggered grids lay out items beyond those
//! in view, one more at a time, measured at once, while a block looks for what it wants (D1479).
//!
//! It shares what a focus search asks for (D477): the items (a grid's lines) to lay out before
//! and after those in view, and whether there are more each way, as last laid out. Compose's
//! intervals of item indices are counts from the items in view here: a block runs with no scroll
//! between its steps, and Compose's start from the items in view too.

use std::cell::{Cell, RefCell};

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

    /// The state lays out the most items any layout going on asked for, each way.
    fn apply(&self, state: LazyListState) {
        let Some(inner) = state.inner() else { return };
        let requests = self.requests.borrow();
        let wanted =
            requests.iter().fold((0, 0), |(before, after), &(b, a)| (before.max(b), after.max(a)));
        if requests.is_empty() {
            // What was laid out for it goes, disposed rather than kept for reuse (D482).
            inner.beyond_ended.set(true);
        }
        inner.beyond_bounds.set(wanted);
    }

    /// Measures the layout now, as `remeasureSync`: where the tree is busy, at its next layout.
    fn remeasure(&self) {
        if let Some(remeasurement) = self.remeasurement.get() {
            remeasurement.force_remeasure();
        }
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
            let (count, average, _, viewport, _) = inner.layout_info_written.get();
            let shown = inner.laid_out.read_untracked(|laid, _| laid.iter().any(|it| !it.sticky));
            // As Compose's `itemsPerViewport` (a grid's lines): the viewport over the average.
            let in_view =
                if shown { ((viewport as f32 / average.max(1.0)) as usize).max(1) } else { 0 };
            (count, in_view)
        });
        let Some(inner) = inner.filter(|_| count > 0 && in_view > 0) else {
            block(&More(false));
            return;
        };
        if self.remeasurement.get().is_none() {
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
        let at = {
            let mut requests = self.requests.borrow_mut();
            requests.push((0, 0));
            requests.len() - 1
        };
        // At most two viewports of items (a grid's lines), as Compose's.
        let most = (VIEWPORT_FACTOR * in_view).min(count);
        let mut laid = 0;
        while more(state) && laid < most {
            {
                let mut requests = self.requests.borrow_mut();
                let request = &mut requests[at];
                if forward { request.1 += 1 } else { request.0 += 1 }
            }
            laid += 1;
            self.apply(state);
            self.remeasure();
            if block(&More(more(state))) {
                break;
            }
        }
        // What it laid out goes.
        self.requests.borrow_mut().remove(at);
        if laid > 0 {
            self.apply(state);
            self.remeasure();
        }
    }
}
