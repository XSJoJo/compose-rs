//! `LazyGridTest`'s lookahead cases: a grid in a `LookaheadScope`, its items composed per item as
//! either pass's measurement reaches them (D1312, D1316), placed ahead where they go and now where
//! the pass that's now puts them (D1226). Both orientations, density 1.

use std::cell::Cell;
use std::rc::Rc;

use compose_animation::{AnimatedVisibility, AnimationModifiers, LINEAR_EASING, tween};
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, Dp, IntSize, UnitExt};

use crate::lazy_animate_support::paused;
use crate::lazy_lookahead_support::*;
use crate::lazy_support::{Lazy, LazyGrid, no_padding};
use crate::scroll_support::run;

/// Compose's `testLookaheadPositionWithPlacementAnimator`, for a grid: items 100 square, keyed by
/// value, `animate_item` with a placement of 160 ms, in a grid of `cells` as long as the items
/// from `start` take and `cross` across; where they are ahead and now at first, then ahead in each
/// of 20 frames after the list becomes `target`, and now where they are ahead in the end if in
/// view. Expected positions are `(main, cross)`, as Compose's `AxisAwareIntOffset`.
fn positions_with_the_placement_animator(
    lists: (Vec<i32>, Vec<i32>),
    cells: usize,
    expected: (Vec<Expected>, Vec<Expected>),
    start: usize,
    cross: Option<i32>,
) {
    const ITEM: i32 = 100;
    let (initial, target) = lists;
    for lazy in Lazy::grids() {
        let list = mutable_state_of(initial.clone());
        let positions = Positions::default();
        let (l, p) = (list.handle(), positions.clone());
        let main = ITEM as f32 * (initial.len() - start) as f32 / cells as f32;
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
                let state = remember_lazy_grid_state(start, 0);
                let (p, list) = (p.clone(), l.get());
                LazyGrid(lazy, cells, sized, state, no_padding(), move |grid| {
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
        let (initial_expected, target_expected) = &expected;
        ui.wait_for_idle();
        for (item, &expected) in initial_expected.iter().enumerate() {
            let what = format!("{lazy:?} item {item} at first");
            assert_offset(lazy, expected, positions.ahead(item as i32), &what);
            assert_offset(lazy, expected, positions.now(item as i32), &what);
        }
        positions.clear();
        list.set(target.clone());
        ui.wait_for_idle();
        for frame in 0..20 {
            ui.advance_time_by_frame();
            for (item, &expected) in target_expected.iter().enumerate() {
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

/// `(main, cross)`, as Compose's `AxisAwareIntOffset`; a cross axis of `None` unspecified.
fn axis(main: i32, cross: Option<i32>) -> Expected {
    Some((cross, main))
}

fn in_line(main: i32) -> Expected {
    axis(main, Some(0))
}

// Compose: LazyGridTest.testLookaheadPositionWithOnlyInBoundChanges
/// Four items in one cell reversed: each is ahead where the other was, at once.
#[test]
fn items_reordered_in_view_are_where_they_go_ahead_at_once() {
    positions_with_the_placement_animator(
        (vec![0, 1, 2, 3], vec![3, 2, 1, 0]),
        1,
        ([0, 100, 200, 300].map(in_line).to_vec(), [300, 200, 100, 0].map(in_line).to_vec()),
        0,
        None,
    );
}

// Compose: LazyGridTest.testLookaheadPositionWithCustomStartingIndex
/// Ten items in two cells from the third, reversed: those that come in view are ahead where they
/// go, those that leave it before the first ahead where they would be.
#[test]
fn items_reordered_from_a_starting_index_are_where_they_go_ahead() {
    let (a, b) = (Some(0), Some(100));
    let initial = vec![
        None,
        None,
        axis(0, a),
        axis(0, b),
        axis(100, a),
        axis(100, b),
        axis(200, a),
        axis(200, b),
        axis(300, None),
        axis(300, None),
    ];
    let target = vec![
        axis(300, b),
        axis(300, a),
        axis(200, b),
        axis(200, a),
        axis(100, b),
        axis(100, a),
        axis(0, b),
        axis(0, a),
        axis(-100, a),
        axis(-100, b),
    ];
    positions_with_the_placement_animator(
        ((0..10).collect(), (0..10).rev().collect()),
        2,
        (initial, target),
        2,
        Some(200),
    );
}

// Compose: LazyGridTest.testLookaheadPositionWithTwoInBoundTwoOutBound
/// Six items in one cell from the third, two leaving the view and two coming in.
#[test]
fn two_items_in_and_two_out_of_view_are_where_they_go_ahead() {
    let mut initial = vec![None, None];
    initial.extend([0, 100, 200, 300].map(in_line));
    positions_with_the_placement_animator(
        ((0..6).collect(), vec![5, 4, 2, 1, 3, 0]),
        1,
        (initial, [300, 100, 0, 200, -100, -200].map(in_line).to_vec()),
        2,
        None,
    );
}

// Compose: LazyGridTest.testLookaheadPositionWithFourInBoundFourOutBound
/// Ten items in two cells from the fifth, four leaving the view and four coming in.
#[test]
fn four_items_in_and_four_out_of_view_are_where_they_go_ahead() {
    let (a, b) = (Some(0), Some(100));
    let initial = vec![
        None,
        None,
        None,
        None,
        axis(0, a),
        axis(0, b),
        axis(100, a),
        axis(100, b),
        axis(200, a),
        axis(200, b),
    ];
    let target = vec![
        axis(200, b),
        axis(100, b),
        axis(100, a),
        axis(200, a),
        axis(0, a),
        axis(0, b),
        axis(-100, None),
        axis(-100, None),
        axis(-200, None),
        axis(-200, None),
    ];
    positions_with_the_placement_animator(
        ((0..10).collect(), vec![8, 9, 7, 6, 4, 5, 2, 1, 3, 0]),
        2,
        (initial, target),
        4,
        Some(200),
    );
}

// Compose: LazyGridTest.animContentSizeWithPlacementAnimator
/// Eight items in two cells 200 across growing to 200: ahead they're where they go at once, now
/// they move there in 160 ms; shrinking back with `animate_content_size`, ahead they're where they
/// go at once too, its animation not counted.
#[test]
fn items_growing_are_where_they_go_ahead_and_move_there_now() {
    const ITEM: i32 = 100;
    for lazy in Lazy::grids() {
        let large = mutable_state_of(false);
        let animate_size = mutable_state_of(false);
        let positions = Positions::default();
        let (lg, an, p) = (large.handle(), animate_size.handle(), positions.clone());
        let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
            let p = p.clone();
            compose_ui::LookaheadScope(callback(move |scope| {
                let across = lazy.cross_axis_size(Dp(200.0));
                let state = remember_lazy_grid_state(0, 0);
                let p = p.clone();
                LazyGrid(lazy, 2, across, state, no_padding(), move |grid| {
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

// Compose: LazyGridTest.animVisibilityWithPlacementAnimator
/// Four items in one cell, each showing a second box with `AnimatedVisibility`: ahead they're
/// where they go at once, now they move there in 160 ms.
#[test]
fn items_growing_with_animated_visibility_move_where_they_are_ahead() {
    const ITEM: i32 = 100;
    for lazy in Lazy::grids() {
        let visible = mutable_state_of(false);
        let positions = Positions::default();
        let (v, p) = (visible.handle(), positions.clone());
        let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
            let p = p.clone();
            compose_ui::LookaheadScope(callback(move |scope| {
                let state = remember_lazy_grid_state(0, 0);
                let p = p.clone();
                LazyGrid(lazy, 1, Modifier, state, no_padding(), move |grid| {
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

/// A grid of two cells measured `ahead` square ahead and `now()` square now, of eight items 100
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
            let state = remember_lazy_grid_state(0, 0);
            let positions = positions.clone();
            LazyGrid(lazy, 2, by_pass, state, no_padding(), move |grid| {
                let positions = positions.clone();
                grid.items(8, move |item| {
                    let modifier = Modifier.required_size(Dp(100.0));
                    Box!(modifier = modifier.then(track(&positions, scope, item as i32)));
                });
            });
        }));
    })
}

/// Checks that ahead all eight are where they go, and now those before `shown`.
fn check_shown(ui: &mut ComposeTestRule, lazy: Lazy, positions: &Positions, shown: i32, at: i32) {
    ui.wait_for_idle();
    for item in 0..8 {
        let what = format!("{lazy:?} item {item} at {at}");
        let main = item / 2 * 100;
        assert_eq!(main_axis(lazy, positions.ahead(item)), Some(main), "{what}");
        let now = (item < shown).then_some(main);
        assert_eq!(main_axis(lazy, positions.now(item)), now, "{what}");
    }
}

// Compose: LazyGridTest.resizeLazyGridOnlyDuringApproach
/// A grid 400 square ahead and 200, 290, 340, 270 and 120 now: ahead all eight are where they go,
/// now those its size shows, growing and shrinking.
#[test]
fn a_grid_resized_now_only_places_what_it_shows_now() {
    for lazy in Lazy::grids() {
        let now = mutable_state_of(200);
        let positions = Positions::default();
        let n = now.handle();
        let mut ui = sized_by_pass_grid(lazy, 400, move || n.get(), &positions);
        check_shown(&mut ui, lazy, &positions, 4, 200);
        let steps = [(290, 6, false), (340, 8, false), (270, 6, true), (120, 4, true)];
        for (size, shown, cleared) in steps {
            now.set(size);
            if cleared {
                positions.now.borrow_mut().clear();
            }
            check_shown(&mut ui, lazy, &positions, shown, size);
        }
    }
}

// Compose: LazyGridTest.lookaheadSizeSmallerThanPostLookahead
/// A grid 200 square ahead and 400, 290, 200, 270 and 120 now: ahead all eight are where they go
/// (the lines the pass that's now shows past its own), now those its size shows.
#[test]
fn a_grid_smaller_ahead_places_what_it_shows_now() {
    for lazy in Lazy::grids() {
        let now = mutable_state_of(400);
        let positions = Positions::default();
        let n = now.handle();
        let mut ui = sized_by_pass_grid(lazy, 200, move || n.get(), &positions);
        check_shown(&mut ui, lazy, &positions, 8, 400);
        for (size, shown) in [(290, 6), (200, 4), (270, 6), (120, 4)] {
            now.set(size);
            positions.now.borrow_mut().clear();
            check_shown(&mut ui, lazy, &positions, shown, size);
        }
    }
}

/// A grid of two cells 300 square of 24 items 30 square, item `sized` measured to what `side`
/// says in each pass, tagged by index.
fn grid_of_small_items(
    lazy: Lazy,
    state: LazyGridState,
    sized: usize,
    side: impl Fn(bool) -> i32 + Clone + 'static,
) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let side = side.clone();
        compose_ui::LookaheadScope(callback(move |_| {
            let side = side.clone();
            let modifier = Modifier.required_size(Dp(300.0));
            LazyGrid(lazy, 2, modifier, state, no_padding(), move |grid| {
                let side = side.clone();
                grid.items_keyed(
                    24,
                    |it| it,
                    move |item| {
                        let modifier = if item == sized {
                            let side = side.clone();
                            Modifier.layout(move |measure, measurable, constraints| {
                                let placeable = measure.measure(measurable, constraints);
                                let side = side(measure.is_looking_ahead());
                                measure.layout_with(side, side, move |placement| {
                                    placement.place(&placeable, 0, 0);
                                })
                            })
                        } else {
                            Modifier.size(Dp(30.0))
                        };
                        Box!(modifier = Modifier.test_tag(item.to_string()).then(modifier));
                    },
                );
            });
        }));
    })
}

// Compose: LazyGridTest.approachItemsComposed
/// The first item 300 long ahead and 30 now: the pass ahead wants its line, the pass that's now
/// ten lines, and those 20 items are placed, the others not composed.
#[test]
fn the_items_the_pass_that_is_now_needs_are_composed() {
    for lazy in Lazy::grids() {
        let owner = LazyGridState::new(0, 0);
        let mut ui = grid_of_small_items(lazy, *owner, 0, |ahead| if ahead { 300 } else { 30 });
        ui.wait_for_idle();
        for item in 0..24 {
            let tag = item.to_string();
            if item < 20 {
                assert!(is_placed(&mut ui, &tag), "{lazy:?} item {item}");
            } else {
                ui.on_node_with_tag(&tag).assert_does_not_exist();
            }
        }
        drop(owner);
    }
}

// Compose: LazyGridTest.approachItemsComposedBasedOnScrollDelta
/// Items 30 square in two cells 300 long, the fifth 300 long ahead after a scroll of 60: the pass
/// ahead shows its line and what follows, the pass that's now (it still 30) the items up to the
/// last; it growing now too, its line alone.
#[test]
fn the_items_the_pass_that_is_now_needs_after_a_scroll_are_placed() {
    for lazy in Lazy::grids() {
        let ahead = mutable_state_of(30);
        let now = mutable_state_of(30);
        let owner = LazyGridState::new(0, 0);
        let state = *owner;
        let (a, n) = (ahead.handle(), now.handle());
        let side = move |looking_ahead: bool| if looking_ahead { a.get() } else { n.get() };
        let mut ui = grid_of_small_items(lazy, state, 4, side);
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
            assert_eq!(placed, item == 4 || item == 5, "{lazy:?} item {item} grown now");
        }
        drop(owner);
    }
}

// Compose: LazyGridTest.testNoOverScroll
/// A grid of 30 animated items with no overscroll factory, scrolled past both ends 20 times each
/// and swiped: its first item never goes below its start, nor its last above its end.
#[test]
fn a_grid_with_no_overscroll_never_goes_past_its_ends() {
    let owner = LazyGridState::new(0, 0);
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
                    LazyVerticalGrid!(
                        GridCells::Fixed(2),
                        modifier = Modifier.required_height(Dp(500.0)).fill_max_width(),
                        state = state,
                        move |grid: LazyGridScope| {
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

// Compose: LazyGridTest.testSmallScrollWithLookaheadScope
/// A scroll of 2 in a grid of one cell 15 long of items 10 long moves them by 2 in a scope too.
#[test]
fn a_small_scroll_in_a_scope_moves_the_items() {
    for lazy in Lazy::grids() {
        let owner = LazyGridState::new(0, 0);
        let state = *owner;
        let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
            let _ = &owner;
            compose_ui::LookaheadScope(callback(move |_| {
                let modifier = lazy.main_axis_size(Dp(15.0));
                LazyGrid(lazy, 1, modifier, state, no_padding(), |grid| {
                    for item in 0..20 {
                        grid.item(move || {
                            Box!(modifier = Modifier.size(10.dp()).test_tag(item.to_string()));
                        });
                    }
                });
            }));
        });
        ui.wait_for_idle();
        ui.run_on_ui_thread(move |_| state.dispatch_raw_delta(2.0));
        ui.wait_for_idle();
        assert_eq!(lazy.main_start(&mut ui, "0"), -2.0, "{lazy:?}");
        assert_eq!(lazy.main_start(&mut ui, "1"), 8.0, "{lazy:?}");
    }
}

// Compose: LazyGridTest.reorderingInLookahead
/// 500 animated items in one cell two long shuffled 20 times, a frame each: nothing breaks.
#[test]
fn items_shuffled_in_a_scope_frame_after_frame_lay_out() {
    for lazy in Lazy::grids() {
        let items = mutable_state_of((0..500).collect::<Vec<i32>>());
        let i = items.handle();
        let mut ui = paused(IntSize::new(1000, 1000), move || {
            compose_ui::LookaheadScope(callback(move |_| {
                let modifier = lazy.main_axis_size(Dp(100.0));
                let state = remember_lazy_grid_state(0, 0);
                let list = i.get();
                LazyGrid(lazy, 1, modifier, state, no_padding(), move |grid| {
                    let item_scope = grid.item_scope();
                    grid.items_list_keyed(
                        list.clone(),
                        |it| *it,
                        move |&item| {
                            let modifier = Modifier
                                .animate_item(&item_scope)
                                .then(lazy.main_axis_size(Dp(50.0)));
                            Box!(modifier = modifier, move |_| {
                                Box!(move |_| BasicText!(format!("Item {item}")));
                            });
                        },
                    );
                });
            }));
        });
        ui.wait_for_idle();
        // Shuffled as Compose's `Random(42)` does, in kind: any order does.
        let mut seed = 42u64;
        for _ in 0..20 {
            let mut next = items.get();
            for at in (1..next.len()).rev() {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                next.swap(at, (seed >> 33) as usize % (at + 1));
            }
            items.set(next);
            ui.advance_time_by_frame();
        }
    }
}
