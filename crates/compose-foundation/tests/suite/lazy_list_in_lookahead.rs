//! `LazyListTest`'s lookahead cases: a lazy column or row in a `LookaheadScope`, its items composed
//! per item as either pass reaches them (D1312, D1316), what it tells the pass ahead's (D1226).
//! Density 1, both orientations.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, IntSize, LookaheadScope as Scope};

use crate::lazy_support::*;

/// `content` in a scope, in a window 1000 square.
fn in_a_scope(content: impl Fn() + 'static) -> ComposeTestRule {
    let content = std::rc::Rc::new(content);
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let content = content.clone();
        compose_ui::LookaheadScope(callback(move |_: Scope| content()));
    });
    ui.wait_for_idle();
    ui
}

/// An item `ahead` square ahead and `now` square now, tagged `tag`.
fn sized_by_pass(ahead: i32, now: i32) -> Modifier {
    Modifier.layout(move |scope, measurable, _| {
        let size = if scope.is_looking_ahead() { ahead } else { now };
        let placeable = scope.measure(measurable, Constraints::fixed(size, size));
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            placement.place(&placeable, 0, 0);
        })
    })
}

// Compose: LazyListTest.testSmallScrollWithLookaheadScope
/// A scroll of 2 in a list 15 long of items 10 long moves them by 2 in a scope too.
#[test]
fn a_small_scroll_in_a_scope_moves_the_items() {
    for lazy in [Lazy::column(), Lazy::row()] {
        let owner = LazyListState::new(0, 0);
        let state = *owner;
        let mut ui = in_a_scope(move || {
            let _ = &owner;
            LazyColumnOrRow(lazy, lazy.sized(px(15), px(100)), state, no_padding(), |scope| {
                scope.items(20, |index| {
                    Box!(modifier = Modifier.size(px(10)).test_tag(index.to_string()));
                });
            });
        });
        ui.run_on_ui_thread(move |_| state.dispatch_raw_delta(2.0));
        ui.wait_for_idle();
        assert_eq!((lazy.main_start(&mut ui, "0"), lazy.main_start(&mut ui, "1")), (-2.0, 8.0));
    }
}

// Compose: LazyListTest.testSmallScrollWithLookaheadScope_lookaheadSizeIsDifferent
/// Items 10 long ahead and 5 now, in a list 12 long: what's seen is the sizes now, and a scroll of
/// 7 leaves the first out of view now, though the pass ahead still shows part of it.
#[test]
fn a_scroll_in_a_scope_moves_the_items_by_their_sizes_now() {
    for lazy in [Lazy::column(), Lazy::row()] {
        let owner = LazyListState::new(0, 0);
        let state = *owner;
        let mut ui = in_a_scope(move || {
            let _ = &owner;
            LazyColumnOrRow(lazy, lazy.sized(px(12), px(100)), state, no_padding(), |scope| {
                scope.items(20, |index| {
                    Box!(modifier = sized_by_pass(10, 5).test_tag(index.to_string()));
                });
            });
        });
        assert_eq!((lazy.main_start(&mut ui, "0"), lazy.main_start(&mut ui, "1")), (0.0, 5.0));
        ui.run_on_ui_thread(move |_| state.dispatch_raw_delta(7.0));
        ui.wait_for_idle();
        ui.on_node_with_tag("0").assert_is_not_displayed();
        assert_eq!((lazy.main_start(&mut ui, "1"), lazy.main_start(&mut ui, "2")), (-2.0, 3.0));
    }
}

// Compose: LazyListTest.postLookaheadItemsComposed
/// The first item 300 long ahead and 30 now, in a list 300 long: the pass ahead wants it alone,
/// the pass that's now 10 items, and those 10 are composed and placed, the others not composed.
#[test]
fn the_items_the_pass_that_is_now_needs_are_composed() {
    for lazy in [Lazy::column(), Lazy::row()] {
        let owner = LazyListState::new(0, 0);
        let state = *owner;
        let mut ui = in_a_scope(move || {
            let _ = &owner;
            LazyColumnOrRow(lazy, lazy.sized(px(300), px(300)), state, no_padding(), |scope| {
                scope.items(12, |index| {
                    let sized =
                        if index == 0 { sized_by_pass(300, 30) } else { Modifier.size(px(30)) };
                    Box!(modifier = sized.test_tag(index.to_string()));
                });
            });
        });
        for item in 0..10 {
            let at = lazy.main_start(&mut ui, &item.to_string());
            assert_eq!(at, item as f32 * 30.0, "{lazy:?}, item {item}");
        }
        for item in 10..12 {
            ui.on_node_with_tag(&item.to_string()).assert_does_not_exist();
        }
    }
}
