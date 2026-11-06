//! Compose's `LazyStaggeredGridInLookaheadTest`: a staggered grid in a `LookaheadScope`, its items
//! composed per item as its measurements reach them (D1312), what either pass wants staying
//! composed (D1226). Vertical grids, density 1.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_animation::{AnimatedVisibility, AnimationModifiers, LINEAR_EASING, tween};
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, Dp, IntSize, UnitExt};

use crate::lazy_animate_support::paused;
use crate::lazy_lookahead_support::*;
use crate::lazy_support::Lazy;
use crate::scroll_support::run;

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

// The others of Compose's class, in both orientations. Its `setup` assumes
// `isCacheWindowLookaheadCheckEnabled`, off at tip, so none of them runs there; none but the
// dispose test's last expectations needs it.

/// Compose's tests' `LazyStaggeredGrid(lanes, modifier, state)` in `lazy`'s orientation.
fn grid(
    lazy: Lazy,
    lanes: usize,
    modifier: Modifier,
    state: Option<LazyStaggeredGridState>,
    content: impl Fn(LazyStaggeredGridScope) + 'static,
) {
    let remembered = remember_lazy_staggered_grid_state(0, 0);
    let state = state.unwrap_or(remembered);
    let content = Rc::new(content);
    if lazy.vertical {
        let content = content.clone();
        LazyVerticalStaggeredGrid!(
            StaggeredGridCells::Fixed(lanes),
            modifier = modifier,
            state = state,
            move |scope: LazyStaggeredGridScope| content(scope)
        );
    } else {
        LazyHorizontalStaggeredGrid!(
            StaggeredGridCells::Fixed(lanes),
            modifier = modifier,
            state = state,
            move |scope: LazyStaggeredGridScope| content(scope)
        );
    }
}

/// Compose's `testLookaheadPositionWithPlacementAnimator`: items 100 square, keyed by value,
/// `animate_item` with a placement of 160 ms, in a grid of `lanes` as long as the items from
/// `start` take and `cross` across; where they are ahead and now at first, then ahead in each of
/// 20 frames after the list becomes `target`, and now where they are ahead in the end if in view.
fn positions_with_the_placement_animator(
    lists: (Vec<i32>, Vec<i32>),
    lanes: usize,
    expected_initial: Vec<Expected>,
    expected_target: Vec<Expected>,
    start: usize,
    cross: Option<i32>,
) {
    const ITEM: i32 = 100;
    let (initial, target) = lists;
    for lazy in [Lazy::column(), Lazy::row()] {
        let list = mutable_state_of(initial.clone());
        let positions = Positions::default();
        let (l, p) = (list.handle(), positions.clone());
        let main = ITEM as f32 * (initial.len() - start) as f32 / lanes as f32;
        let mut ui = paused(IntSize::new(1000, 1000), move || {
            let p = p.clone();
            compose_ui::LookaheadScope(callback(move |scope| {
                let sized = match (lazy.vertical, cross) {
                    (true, Some(cross)) => {
                        Modifier.required_height(Dp(main)).required_width(Dp(cross as f32))
                    }
                    (true, None) => Modifier.required_height(Dp(main)),
                    (false, Some(cross)) => {
                        Modifier.required_width(Dp(main)).required_height(Dp(cross as f32))
                    }
                    (false, None) => Modifier.required_width(Dp(main)),
                };
                let state = remember_lazy_staggered_grid_state(start, 0);
                let (p, list) = (p.clone(), l.get());
                grid(lazy, lanes, sized, Some(state), move |grid| {
                    let item_scope = grid.item_scope();
                    let p = p.clone();
                    grid.items_list_keyed(
                        list.clone(),
                        |it| *it,
                        move |&item| {
                            let placement = tween(160).into();
                            let modifier = Modifier
                                .animate_item_with(&item_scope, None, Some(placement), None)
                                .then(track(&p, scope, item))
                                .required_size(Dp(ITEM as f32));
                            Box!(modifier = modifier);
                        },
                    );
                });
            }));
        });
        ui.wait_for_idle();
        for (item, &expected) in expected_initial.iter().enumerate() {
            let what = format!("{lazy:?} item {item} at first");
            let key = item as i32;
            assert_offset(lazy, expected, positions.ahead(key), &what);
            assert_offset(lazy, expected, positions.now(key), &what);
        }
        positions.clear();
        list.set(target.clone());
        ui.wait_for_idle();
        for frame in 0..20 {
            ui.advance_time_by_frame();
            for (item, &expected) in expected_target.iter().enumerate() {
                let what = format!("{lazy:?} item {item} ahead at frame {frame}");
                assert_offset(lazy, expected, positions.ahead(item as i32), &what);
            }
        }
        for item in 0..target.len() as i32 {
            let ahead = positions.ahead(item);
            if main_axis(lazy, ahead).is_none_or(|main| main + ITEM >= 0) {
                assert_eq!(positions.now(item), ahead, "{lazy:?} item {item} now in the end");
            }
        }
    }
}

// Compose: LazyStaggeredGridInLookaheadTest.testLookaheadPositionWithOnlyInBoundChanges
/// Four items in one lane reversed: each is ahead where the other was, at once.
#[test]
fn items_reordered_in_view_are_where_they_go_ahead_at_once() {
    positions_with_the_placement_animator(
        (vec![0, 1, 2, 3], vec![3, 2, 1, 0]),
        1,
        (0..4).map(|it| Some((Some(0), it * 100))).collect(),
        (0..4).map(|it| Some((Some(0), (3 - it) * 100))).collect(),
        0,
        None,
    );
}

// Compose: LazyStaggeredGridInLookaheadTest.testLookaheadPositionWithCustomStartingIndex
/// Ten items in two lanes from the third, reversed: those that come in view are ahead where they
/// go, those that leave it before the first ahead where they would be.
#[test]
fn items_reordered_from_a_starting_index_are_where_they_go_ahead() {
    let initial = vec![
        None,
        None,
        Some((Some(0), 0)),
        Some((Some(100), 0)),
        Some((Some(0), 100)),
        Some((Some(100), 100)),
        Some((Some(0), 200)),
        Some((Some(100), 200)),
        Some((None, 300)),
        Some((None, 300)),
    ];
    let target = vec![
        Some((Some(100), 300)),
        Some((Some(0), 300)),
        Some((Some(100), 200)),
        Some((Some(0), 200)),
        Some((Some(100), 100)),
        Some((Some(0), 100)),
        Some((Some(100), 0)),
        Some((Some(0), 0)),
        Some((Some(0), -100)),
        Some((Some(100), -100)),
    ];
    positions_with_the_placement_animator(
        ((0..10).collect(), (0..10).rev().collect()),
        2,
        initial,
        target,
        2,
        Some(200),
    );
}

// Compose: LazyStaggeredGridInLookaheadTest.testLookaheadPositionWithTwoInBoundTwoOutBound
/// Six items in one lane from the third, two leaving the view and two coming in.
#[test]
fn two_items_in_and_two_out_of_view_are_where_they_go_ahead() {
    let initial = vec![
        None,
        None,
        Some((Some(0), 0)),
        Some((Some(0), 100)),
        Some((Some(0), 200)),
        Some((Some(0), 300)),
    ];
    let target = [300, 100, 0, 200, -100, -200].map(|main| Some((Some(0), main))).to_vec();
    positions_with_the_placement_animator(
        ((0..6).collect(), vec![5, 4, 2, 1, 3, 0]),
        1,
        initial,
        target,
        2,
        None,
    );
}

// Compose: LazyStaggeredGridInLookaheadTest.testLookaheadPositionWithFourInBoundFourOutBound
/// Ten items in two lanes from the fifth, four leaving the view and four coming in.
#[test]
fn four_items_in_and_four_out_of_view_are_where_they_go_ahead() {
    let initial = vec![
        None,
        None,
        None,
        None,
        Some((Some(0), 0)),
        Some((Some(100), 0)),
        Some((Some(0), 100)),
        Some((Some(100), 100)),
        Some((Some(0), 200)),
        Some((Some(100), 200)),
    ];
    let target = vec![
        Some((Some(100), 200)),
        Some((Some(100), 100)),
        Some((Some(0), 100)),
        Some((Some(0), 200)),
        Some((Some(0), 0)),
        Some((Some(100), 0)),
        Some((None, -100)),
        Some((None, -100)),
        Some((None, -200)),
        Some((None, -200)),
    ];
    positions_with_the_placement_animator(
        ((0..10).collect(), vec![8, 9, 7, 6, 4, 5, 2, 1, 3, 0]),
        2,
        initial,
        target,
        4,
        Some(200),
    );
}

// Compose: LazyStaggeredGridInLookaheadTest.animContentSizeWithPlacementAnimator
/// Eight items in two lanes 200 across growing to 200: ahead they're where they go at once, now
/// they move there in 160 ms; shrinking back with `animate_content_size`, ahead they're where
/// they go at once too, its animation not counted.
#[test]
fn items_growing_are_where_they_go_ahead_and_move_there_now() {
    const ITEM: i32 = 100;
    for lazy in [Lazy::column(), Lazy::row()] {
        let large = mutable_state_of(false);
        let animate_size = mutable_state_of(false);
        let positions = Positions::default();
        let (lg, an, p) = (large.handle(), animate_size.handle(), positions.clone());
        let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
            let p = p.clone();
            compose_ui::LookaheadScope(callback(move |scope| {
                let across = if lazy.vertical {
                    Modifier.required_width(Dp(200.0))
                } else {
                    Modifier.required_height(Dp(200.0))
                };
                let p = p.clone();
                grid(lazy, 2, across, None, move |grid| {
                    let item_scope = grid.item_scope();
                    let p = p.clone();
                    grid.items_keyed(
                        8,
                        |it| it,
                        move |item| {
                            let placement = tween(160).easing(LINEAR_EASING).into();
                            let mut modifier = Modifier
                                .animate_item_with(&item_scope, None, Some(placement), None)
                                .then(track(&p, scope, item as i32));
                            if an.get() {
                                modifier = modifier.animate_content_size_with(
                                    tween(160),
                                    compose_ui::Alignment::TOP_START,
                                    None::<fn(IntSize, IntSize)>,
                                );
                            }
                            let side = if lg.get() { ITEM * 2 } else { ITEM };
                            Box!(modifier = modifier.required_size(Dp(side as f32)));
                        },
                    );
                });
            }));
        });
        ui.wait_for_idle();
        for item in 0..8 {
            let (main, cross) = (item / 2 * ITEM, item % 2 * 100);
            let what = format!("{lazy:?} item {item}");
            assert_eq!(positions.ahead(item), Some(at(lazy, cross, main)), "{what}");
            assert_eq!(positions.now(item), Some(at(lazy, cross, main)), "{what}");
        }
        ui.set_auto_advance(false);
        large.set(true);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        ui.advance_time_by_frame();
        for frame in 0..20 {
            let fraction = (frame as f32 * 16.0 / 160.0).min(1.0);
            for item in 0..8 {
                let what = format!("{lazy:?} item {item} at frame {frame}");
                let main = item / 2 * ITEM;
                assert_eq!(main_axis(lazy, positions.ahead(item)), Some(main * 2), "{what}");
                let now = (main as f32 * (1.0 + fraction)).round() as i32;
                assert_eq!(main_axis(lazy, positions.now(item)), Some(now), "{what}");
            }
            ui.advance_time_by_frame();
        }
        animate_size.set(true);
        large.set(false);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        ui.advance_time_by_frame();
        for frame in 0..20 {
            let fraction = (frame as f32 * 16.0 / 160.0).min(1.0);
            for item in 0..4 {
                let what = format!("{lazy:?} item {item} shrinking at frame {frame}");
                let main = item / 2 * ITEM;
                assert_eq!(main_axis(lazy, positions.ahead(item)), Some(main), "{what}");
                let now = (main as f32 * (2.0 - fraction)).round() as i32;
                assert_eq!(main_axis(lazy, positions.now(item)), Some(now), "{what}");
            }
            ui.advance_time_by_frame();
        }
    }
}

// Compose: LazyStaggeredGridInLookaheadTest.animVisibilityWithPlacementAnimator
/// Four items in one lane, each showing a second box with `AnimatedVisibility`: ahead they're
/// where they go at once, now they move there in 160 ms.
#[test]
fn items_growing_with_animated_visibility_move_where_they_are_ahead() {
    const ITEM: i32 = 100;
    for lazy in [Lazy::column(), Lazy::row()] {
        let visible = mutable_state_of(false);
        let positions = Positions::default();
        let (v, p) = (visible.handle(), positions.clone());
        let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
            let p = p.clone();
            compose_ui::LookaheadScope(callback(move |scope| {
                let p = p.clone();
                grid(lazy, 1, Modifier, None, move |grid| {
                    let item_scope = grid.item_scope();
                    let p = p.clone();
                    grid.items_keyed(
                        4,
                        |it| it,
                        move |item| {
                            let placement = tween(160).easing(LINEAR_EASING).into();
                            let modifier = Modifier
                                .animate_item_with(&item_scope, None, Some(placement), None)
                                .then(track(&p, scope, item as i32));
                            let square =
                                || Box!(modifier = Modifier.required_size(Dp(ITEM as f32)));
                            if lazy.vertical {
                                Column!(modifier = modifier, move |column| {
                                    square();
                                    AnimatedVisibility!((column, v.get()), move |_| square());
                                });
                            } else {
                                Row!(modifier = modifier, move |row| {
                                    square();
                                    AnimatedVisibility!((row, v.get()), move |_| square());
                                });
                            }
                        },
                    );
                });
            }));
        });
        ui.wait_for_idle();
        for item in 0..4 {
            let what = format!("{lazy:?} item {item}");
            assert_eq!(main_axis(lazy, positions.ahead(item)), Some(item * ITEM), "{what}");
            assert_eq!(main_axis(lazy, positions.now(item)), Some(item * ITEM), "{what}");
        }
        ui.set_auto_advance(false);
        visible.set(true);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        ui.advance_time_by_frame();
        for frame in 0..20 {
            let fraction = (frame as f32 * 16.0 / 160.0).min(1.0);
            for item in 0..4 {
                let what = format!("{lazy:?} item {item} at frame {frame}");
                let ahead = main_axis(lazy, positions.ahead(item));
                assert_eq!(ahead, Some(item * ITEM * 2), "{what}");
                let now = (item as f32 * ITEM as f32 * (1.0 + fraction)).round() as i32;
                assert_eq!(main_axis(lazy, positions.now(item)), Some(now), "{what}");
            }
            ui.advance_time_by_frame();
        }
    }
}

/// A grid of two lanes measured `ahead` square ahead and `now()` square now, of eight items 100
/// square, noting where they're placed.
fn sized_by_pass_grid(
    lazy: Lazy,
    ahead: i32,
    now: impl Fn() -> i32 + 'static,
    positions: &Positions,
) -> ComposeTestRule {
    let positions = positions.clone();
    let now = Rc::new(now);
    ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (positions, now) = (positions.clone(), now.clone());
        compose_ui::LookaheadScope(callback(move |scope| {
            let now = now.clone();
            let by_pass = Modifier.layout(move |measure, measurable, _| {
                let side = if measure.is_looking_ahead() { ahead } else { now() };
                let placeable = measure.measure(measurable, Constraints::fixed(side, side));
                measure.layout_with(placeable.width, placeable.height, move |placement| {
                    placement.place(&placeable, 0, 0);
                })
            });
            let positions = positions.clone();
            grid(lazy, 2, by_pass, None, move |grid| {
                let positions = positions.clone();
                grid.items(8, move |item| {
                    let modifier = Modifier.required_size(Dp(100.0));
                    Box!(modifier = modifier.then(track(&positions, scope, item as i32)));
                });
            });
        }));
    })
}

// Compose: LazyStaggeredGridInLookaheadTest.resizeLazyStaggeredGridOnlyDuringApproach
/// A grid 400 square ahead and 200, 290, 340, 270 and 120 now: ahead all eight are where they go,
/// now those its size shows, growing and shrinking.
#[test]
fn a_grid_resized_now_only_places_what_it_shows_now() {
    const ITEM: i32 = 100;
    for lazy in [Lazy::column(), Lazy::row()] {
        let now = mutable_state_of(ITEM * 2);
        let positions = Positions::default();
        let n = now.handle();
        let mut ui = sized_by_pass_grid(lazy, 4 * ITEM, move || n.get(), &positions);
        let check = |ui: &mut ComposeTestRule, shown: i32, size: i32| {
            ui.wait_for_idle();
            for item in 0..8 {
                let what = format!("{lazy:?} item {item} at {size}");
                let main = item / 2 * ITEM;
                assert_eq!(main_axis(lazy, positions.ahead(item)), Some(main), "{what}");
                let now = (item < shown).then_some(main);
                assert_eq!(main_axis(lazy, positions.now(item)), now, "{what}");
            }
        };
        check(&mut ui, 4, 200);
        for (size, shown, cleared) in
            [(290, 6, false), (340, 8, false), (270, 6, true), (120, 4, true)]
        {
            now.set(size);
            if cleared {
                positions.now.borrow_mut().clear();
            }
            check(&mut ui, shown, size);
        }
    }
}

// Compose: LazyStaggeredGridInLookaheadTest.lookaheadSizeSmallerThanPostLookahead
/// A grid 200 square ahead and 400, 290, 200, 270 and 120 now: ahead the first four are where
/// they go and the others past them; now those its size shows.
#[test]
fn a_grid_smaller_ahead_places_what_it_shows_now() {
    const ITEM: i32 = 100;
    for lazy in [Lazy::column(), Lazy::row()] {
        let now = mutable_state_of(ITEM * 4);
        let positions = Positions::default();
        let n = now.handle();
        let mut ui = sized_by_pass_grid(lazy, 2 * ITEM, move || n.get(), &positions);
        let check = |ui: &mut ComposeTestRule, shown: i32, size: i32| {
            ui.wait_for_idle();
            for item in 0..8 {
                let what = format!("{lazy:?} item {item} at {size}");
                let main = item / 2 * ITEM;
                let ahead = main_axis(lazy, positions.ahead(item));
                if item < 4 {
                    assert_eq!(ahead, Some(main), "{what}");
                } else {
                    assert!(ahead.is_some_and(|it| it >= main), "{what}: {ahead:?}");
                }
                let now = (item < shown).then_some(main);
                assert_eq!(main_axis(lazy, positions.now(item)), now, "{what}");
            }
        };
        check(&mut ui, 8, 400);
        for (size, shown) in [(290, 6), (200, 4), (270, 6), (120, 4)] {
            now.set(size);
            positions.now.borrow_mut().clear();
            check(&mut ui, shown, size);
        }
    }
}

// Compose: LazyStaggeredGridInLookaheadTest.approachItemsComposedBasedOnScrollDelta
/// Items 30 square in two lanes 300 long, the fifth 300 long ahead after a scroll of 60: the pass
/// ahead shows it and what follows, the pass that's now (it still 30) the items up to the last;
/// it growing now too, those the pass ahead shows. (Compose's test scrolls a `state` it never
/// sets, which its class's `assumeTrue` keeps from running; here the grid's.)
#[test]
fn the_items_the_pass_that_is_now_needs_after_a_scroll_are_placed() {
    for lazy in [Lazy::column(), Lazy::row()] {
        let ahead = mutable_state_of(30);
        let now = mutable_state_of(30);
        let owner = LazyStaggeredGridState::new(0, 0);
        let state = *owner;
        let (a, n) = (ahead.handle(), now.handle());
        let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
            let _ = &owner;
            compose_ui::LookaheadScope(callback(move |_| {
                let modifier = Modifier.required_size(Dp(300.0));
                grid(lazy, 2, modifier, Some(state), move |grid| {
                    grid.items_keyed(
                        24,
                        |it| it,
                        move |item| {
                            let sized = if item == 4 {
                                Modifier.layout(move |measure, measurable, constraints| {
                                    let placeable = measure.measure(measurable, constraints);
                                    let side =
                                        if measure.is_looking_ahead() { a.get() } else { n.get() };
                                    measure.layout_with(side, side, move |placement| {
                                        placement.place(&placeable, 0, 0);
                                    })
                                })
                            } else {
                                Modifier.size(Dp(30.0))
                            };
                            Box!(modifier = Modifier.test_tag(item.to_string()).then(sized));
                        },
                    );
                });
            }));
        });
        ui.wait_for_idle();
        for item in 0..24 {
            let tag = item.to_string();
            if item < 20 {
                assert!(is_placed(&mut ui, &tag), "{lazy:?} item {item} at first");
            } else {
                ui.on_node_with_tag(&tag).assert_does_not_exist();
            }
        }
        ahead.set(300);
        let _ = run(&mut ui, async move { state.scroll_by(60.0).await });
        ui.wait_for_idle();
        for item in 0..24 {
            let placed = is_placed(&mut ui, &item.to_string());
            assert_eq!(placed, item >= 4, "{lazy:?} item {item} after the scroll");
        }
        now.set(300);
        ui.wait_for_idle();
        for item in 0..24 {
            let placed = is_placed(&mut ui, &item.to_string());
            assert_eq!(placed, (4..=14).contains(&item), "{lazy:?} item {item} grown now");
        }
    }
}

// Compose: LazyStaggeredGridInLookaheadTest.testNoOverScrollWhenSpecified
/// A grid of 30 animated items with no overscroll factory, scrolled past both ends 20 times each
/// and swiped: its first item never goes below its start, nor its last above its end.
#[test]
fn a_grid_with_no_overscroll_never_goes_past_its_ends() {
    let owner = LazyStaggeredGridState::new(0, 0);
    let state = *owner;
    let first: Rc<Cell<Option<f32>>> = Rc::default();
    let last: Rc<Cell<Option<f32>>> = Rc::default();
    let (f, l) = (first.clone(), last.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let _ = &owner;
        let (f, l) = (f.clone(), l.clone());
        compose_ui::LookaheadScope(callback(move |_| {
            let (f, l) = (f.clone(), l.clone());
            CompositionLocalProvider(LOCAL_OVERSCROLL_FACTORY.provides(None), move || {
                let (f, l) = (f.clone(), l.clone());
                Box!(modifier = Modifier.test_tag("grid"), move |_| {
                    let (f, l) = (f.clone(), l.clone());
                    LazyVerticalStaggeredGrid!(
                        StaggeredGridCells::Fixed(2),
                        modifier = Modifier.required_height(Dp(500.0)).fill_max_width(),
                        state = state,
                        move |grid: LazyStaggeredGridScope| {
                            let item_scope = grid.item_scope();
                            let (f, l) = (f.clone(), l.clone());
                            grid.items(30, move |item| {
                                let mut modifier = Modifier;
                                if item == 0 || item == 29 {
                                    let (f, l) = (f.clone(), l.clone());
                                    modifier = modifier.on_globally_positioned(
                                        move |it: compose_ui::LayoutCoordinates| {
                                            let y = it.position_in_root().y;
                                            if item == 0 {
                                                f.set(Some(y));
                                                assert!(y <= 0.0, "the first item at {y}");
                                            } else {
                                                l.set(Some(y));
                                                assert!(y >= 400.0, "the last item at {y}");
                                            }
                                        },
                                    );
                                }
                                let modifier = modifier.height(Dp(100.0)).animate_item(&item_scope);
                                BasicText!(item.to_string(), modifier = modifier);
                            });
                        }
                    );
                });
            });
        }));
    });
    let check = || {
        assert!(first.get().is_none_or(|y| y <= 0.0));
        assert!(last.get().is_none_or(|y| y >= 400.0));
    };
    for step in 0..20 {
        ui.wait_for_idle();
        let _ = run(&mut ui, async move { state.scroll_by(200.0).await });
        if step == 19 {
            assert_eq!(state.first_visible_item_index(), 20);
            let _ = run(&mut ui, async move { state.scroll_to_item(14, 0).await });
            ui.on_node_with_tag("grid").perform_touch_input(|it| {
                let (bottom, top) = (it.bottom(), it.top());
                it.swipe_up_with(bottom, top, 50);
            });
        }
        check();
    }
    for step in 0..20 {
        ui.wait_for_idle();
        let _ = run(&mut ui, async move { state.scroll_by(-200.0).await });
        if step == 19 {
            assert_eq!(state.first_visible_item_index(), 0);
            let _ = run(&mut ui, async move { state.scroll_to_item(7, 0).await });
            ui.on_node_with_tag("grid").perform_touch_input(|it| {
                let (top, bottom) = (it.top(), it.bottom());
                it.swipe_down_with(top, bottom, 50);
            });
        }
        check();
    }
}
