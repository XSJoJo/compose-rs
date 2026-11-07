//! What the lazy layouts' lookahead tests share: Compose's `trackPositions`, where each item is
//! placed ahead and now, and its `assertOffsetEquals`. Density 1.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use compose_test::ComposeTestRule;
use compose_ui::{IntOffset, Modifier, Offset};

use crate::lazy_support::Lazy;

/// Where each item was placed, by key: ahead, from the root as the scope sees it
/// (`localLookaheadPositionOf`), and now, in the root; rounded.
#[derive(Clone, Default)]
pub struct Positions {
    pub ahead: Rc<RefCell<HashMap<i32, IntOffset>>>,
    pub now: Rc<RefCell<HashMap<i32, IntOffset>>>,
}

impl Positions {
    pub fn ahead(&self, item: i32) -> Option<IntOffset> {
        self.ahead.borrow().get(&item).copied()
    }

    pub fn now(&self, item: i32) -> Option<IntOffset> {
        self.now.borrow().get(&item).copied()
    }

    pub fn clear(&self) {
        self.ahead.borrow_mut().clear();
        self.now.borrow_mut().clear();
    }
}

fn rounded(offset: Offset) -> IntOffset {
    IntOffset::new(offset.x.round() as i32, offset.y.round() as i32)
}

/// Compose's tests' `trackPositions`: notes where `item` is placed in either pass.
pub fn track(positions: &Positions, scope: compose_ui::LookaheadScope, item: i32) -> Modifier {
    let positions = positions.clone();
    Modifier.layout(move |measure, measurable, constraints| {
        let placeable = measure.measure(measurable, constraints);
        let positions = positions.clone();
        measure.layout_with(placeable.width, placeable.height, move |placement| {
            if let Some(at) = placement.coordinates() {
                if placement.is_looking_ahead() {
                    let root = at.find_root_coordinates();
                    let offset = scope.local_lookahead_position_of(root, at, Offset::ZERO);
                    positions.ahead.borrow_mut().insert(item, rounded(offset));
                } else {
                    positions.now.borrow_mut().insert(item, rounded(at.position_in_root()));
                }
            }
            placement.place(&placeable, 0, 0);
        })
    })
}

pub fn main_axis(lazy: Lazy, offset: Option<IntOffset>) -> Option<i32> {
    offset.map(|it| if lazy.vertical { it.y } else { it.x })
}

/// `(cross, main)` in `lazy`'s orientation.
pub fn at(lazy: Lazy, cross: i32, main: i32) -> IntOffset {
    if lazy.vertical { IntOffset::new(cross, main) } else { IntOffset::new(main, cross) }
}

/// An expected position: `None` for none, a cross axis of `None` for any (Compose's
/// `UnspecifiedOffset`, the cross axis of items before those in view having no contract).
pub type Expected = Option<(Option<i32>, i32)>;

pub fn assert_offset(lazy: Lazy, expected: Expected, actual: Option<IntOffset>, what: &str) {
    match expected {
        None => assert_eq!(actual, None, "{what}"),
        Some((None, main)) => assert_eq!(main_axis(lazy, actual), Some(main), "{what}"),
        Some((Some(cross), main)) => assert_eq!(actual, Some(at(lazy, cross, main)), "{what}"),
    }
}

/// Whether `tag`'s node is there and placed, as `assertIsPlaced`.
pub fn is_placed(ui: &mut ComposeTestRule, tag: &str) -> bool {
    let nodes = ui.on_all_nodes_with_tag(tag).fetch_semantics_nodes();
    nodes.first().is_some_and(|it| it.layout_info().is_placed())
}
