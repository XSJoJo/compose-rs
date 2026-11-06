//! Compose's `LazyStaggeredGridInLookaheadTest`: a staggered grid in a `LookaheadScope`, its items
//! composed per item as its measurements reach them (D1312), what either pass wants staying
//! composed (D1226). Vertical grids, density 1.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, Dp, IntSize, UnitExt};

/// Which items' `DisposableEffect`s said they went (disposed, or deactivated and kept for reuse).
#[derive(Clone, Default)]
struct Gone(Rc<RefCell<Vec<bool>>>);

impl Gone {
    fn of(count: usize) -> Gone {
        Gone(Rc::new(RefCell::new(vec![false; count])))
    }

    fn items(&self) -> Vec<usize> {
        let gone = self.0.borrow();
        (0..gone.len()).filter(|it| gone[*it]).collect()
    }
}

/// A grid of 20 items 100 tall in two lanes, 400 wide, measured `ahead` tall ahead and `now` tall
/// now, saying `ahead` either way, with `window` as its cache window: which items went after each
/// of 1000 both ways, 400 now, 400 ahead too, then 300 ahead.
fn gone_as_the_passes_shrink(window: Option<Rc<dyn LazyLayoutCacheWindow>>) -> Vec<Vec<usize>> {
    let gone = Gone::of(20);
    let ahead = mutable_state_of(1000);
    let now = mutable_state_of(1000);
    let (a, n, g) = (ahead.handle(), now.handle(), gone.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (g, window) = (g.clone(), window.clone());
        compose_ui::LookaheadScope(callback(move |_| {
            let by_pass = Modifier.layout(move |scope, measurable, _| {
                let tall = if scope.is_looking_ahead() { a.get() } else { n.get() };
                let placeable = scope.measure(measurable, Constraints::fixed(400, tall));
                scope.layout_with(placeable.width, a.get(), move |placement| {
                    placement.place(&placeable, 0, 0);
                })
            });
            let g = g.clone();
            LazyVerticalStaggeredGrid!(
                StaggeredGridCells::Fixed(2),
                modifier = by_pass,
                cache_window = window.clone(),
                move |scope: LazyStaggeredGridScope| {
                    let g = g.clone();
                    scope.items(20, move |index| {
                        Box!(modifier = Modifier.height(100.dp()).fill_max_width());
                        let g = g.clone();
                        DisposableEffect((), move |effect| {
                            let g = g.clone();
                            effect.on_dispose(move || g.0.borrow_mut()[index] = true)
                        });
                    });
                }
            );
        }));
    });
    let mut seen = Vec::new();
    ui.wait_for_idle();
    seen.push(gone.items());
    now.set(400);
    ui.wait_for_idle();
    seen.push(gone.items());
    ahead.set(400);
    ui.wait_for_idle();
    seen.push(gone.items());
    ahead.set(300);
    ui.wait_for_idle();
    seen.push(gone.items());
    seen
}

// Compose: LazyStaggeredGridInLookaheadTest.testDisposeHappensAfterNoLongerNeededByEitherPass
/// Nothing goes while either pass shows it: none while the pass ahead still shows 20, and none of
/// the 8 the pass that's now shows when the pass ahead shows 6. Compose's test also expects items
/// 8 to 19 to go once both passes show 8, but only with `isCacheWindowLookaheadCheckEnabled`, an
/// experimental flag off at tip, which starts the window again after a pass ahead: with it off, as
/// in compose-rs, the window's bounds only ever grow over what was in view until a scroll trims
/// them (`cacheVisibleItemsInfo`), so `keepAroundItems` keeps all 20, with a window of no size
/// too.
#[test]
fn nothing_goes_while_either_pass_wants_it() {
    let nothing = lazy_layout_cache_window_with(Dp(0.0), Dp(0.0), false);
    for window in [None, Some(nothing)] {
        assert_eq!(gone_as_the_passes_shrink(window), vec![Vec::<usize>::new(); 4]);
    }
}

/// An item `ahead` square ahead and `now` square now.
fn sized_by_pass(ahead: i32, now: i32) -> Modifier {
    Modifier.layout(move |scope, measurable, _| {
        let size = if scope.is_looking_ahead() { ahead } else { now };
        let placeable = scope.measure(measurable, Constraints::fixed(size, size));
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            placement.place(&placeable, 0, 0);
        })
    })
}

// Compose: LazyStaggeredGridInLookaheadTest.approachItemsComposed
/// The first item 300 tall ahead and 30 now, in a grid of two lanes 300 tall: the pass ahead
/// wants it and the second, the pass that's now 20 items, and those 20 are composed and placed,
/// the others not composed.
#[test]
fn the_items_the_pass_that_is_now_needs_are_composed() {
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), || {
        compose_ui::LookaheadScope(callback(|_| {
            LazyVerticalStaggeredGrid!(
                StaggeredGridCells::Fixed(2),
                modifier = Modifier.size(300.dp()),
                |scope: LazyStaggeredGridScope| {
                    scope.items(24, |index| {
                        let sized = if index == 0 {
                            sized_by_pass(300, 30)
                        } else {
                            Modifier.size(30.dp())
                        };
                        Box!(modifier = sized.test_tag(index.to_string()));
                    });
                }
            );
        }));
    });
    ui.wait_for_idle();
    for item in 0..20 {
        let bounds = ui.on_node_with_tag(&item.to_string()).get_unclipped_bounds_in_root();
        let row = (item / 2) as f32 * 30.0;
        assert_eq!(bounds.top.0, row, "item {item}");
    }
    for item in 20..24 {
        ui.on_node_with_tag(&item.to_string()).assert_does_not_exist();
    }
}

// Compose: LazyStaggeredGridInLookaheadTest.testSmallScrollWithLookaheadScope
/// A scroll of 2 in a grid of one lane 15 long of items 10 long moves them by 2 in a scope too.
#[test]
fn a_small_scroll_in_a_scope_moves_the_items() {
    let owner = LazyStaggeredGridState::new(0, 0);
    let state = *owner;
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let _ = &owner;
        compose_ui::LookaheadScope(callback(move |_| {
            LazyVerticalStaggeredGrid!(
                StaggeredGridCells::Fixed(1),
                modifier = Modifier.height(15.dp()),
                state = state,
                |scope: LazyStaggeredGridScope| {
                    scope.items(20, |index| {
                        Box!(modifier = Modifier.size(10.dp()).test_tag(index.to_string()));
                    });
                }
            );
        }));
    });
    ui.wait_for_idle();
    ui.run_on_ui_thread(move |_| state.dispatch_raw_delta(2.0));
    ui.wait_for_idle();
    let top = |ui: &mut ComposeTestRule, tag: &str| {
        ui.on_node_with_tag(tag).get_unclipped_bounds_in_root().top.0
    };
    assert_eq!((top(&mut ui, "0"), top(&mut ui, "1")), (-2.0, 8.0));
}
