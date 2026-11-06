//! `LazyListTest`'s lookahead cases: a lazy column or row in a `LookaheadScope`, its items composed
//! per item as either pass reaches them (D1312, D1316), what it tells the pass ahead's (D1226).
//! Density 1, both orientations.

use compose_animation::tween;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, Dp, IntSize, LookaheadScope as Scope};

use crate::lazy_animate_support::paused;
use crate::lazy_lookahead_support::*;
use crate::lazy_support::*;
use crate::scroll_support::run;

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

/// Compose's `testLookaheadPositionWithPlacementAnimator`, for a list: items 100 square, keyed by
/// value, `animate_item` with a placement of 160 ms, in a list as long as the items from `start`
/// take; where they are along it ahead and now at first, then ahead in each of 20 frames after
/// the list becomes `target`, and now where they are ahead in the end if in view.
fn positions_with_the_placement_animator(
    lists: (Vec<i32>, Vec<i32>),
    expected: (Vec<Option<i32>>, Vec<Option<i32>>),
    start: usize,
) {
    const ITEM: i32 = 100;
    let (initial, target) = lists;
    for lazy in [Lazy::column(), Lazy::row()] {
        let list = mutable_state_of(initial.clone());
        let positions = Positions::default();
        let (l, p) = (list.handle(), positions.clone());
        let main = Dp((ITEM as usize * (initial.len() - start)) as f32);
        let mut ui = paused(IntSize::new(1000, 1000), move || {
            let p = p.clone();
            compose_ui::LookaheadScope(callback(move |scope| {
                let sized = if lazy.vertical {
                    Modifier.required_height(main)
                } else {
                    Modifier.required_width(main)
                };
                let state = remember_lazy_list_state(start, 0);
                let (p, list) = (p.clone(), l.get());
                LazyColumnOrRow(lazy, sized, state, no_padding(), move |scope_: LazyListScope| {
                    let item_scope = scope_.item_scope();
                    let p = p.clone();
                    scope_.items_list_keyed(
                        list.clone(),
                        |it| *it,
                        move |_, &item| {
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
            assert_eq!(main_axis(lazy, positions.ahead(item as i32)), expected, "{what}");
            assert_eq!(main_axis(lazy, positions.now(item as i32)), expected, "{what}");
        }
        positions.clear();
        list.set(target.clone());
        ui.wait_for_idle();
        for frame in 0..20 {
            ui.advance_time_by_frame();
            for (item, &expected) in target_expected.iter().enumerate() {
                let what = format!("{lazy:?} item {item} ahead at frame {frame}");
                assert_eq!(main_axis(lazy, positions.ahead(item as i32)), expected, "{what}");
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

// Compose: LazyListTest.testLookaheadPositionWithOnlyInBoundChanges
/// Four items reversed: each is ahead where the other was, at once.
#[test]
fn items_reordered_in_view_are_where_they_go_ahead_at_once() {
    positions_with_the_placement_animator(
        (vec![0, 1, 2, 3], vec![3, 2, 1, 0]),
        ([0, 100, 200, 300].map(Some).to_vec(), [300, 200, 100, 0].map(Some).to_vec()),
        0,
    );
}

// Compose: LazyListTest.testLookaheadPositionWithCustomStartingIndex
/// Five items from the second, reversed: the first comes in view, the last leaves it ahead.
#[test]
fn items_reordered_from_a_starting_index_are_where_they_go_ahead() {
    positions_with_the_placement_animator(
        ((0..5).collect(), (0..5).rev().collect()),
        (
            vec![None, Some(0), Some(100), Some(200), Some(300)],
            [300, 200, 100, 0, -100].map(Some).to_vec(),
        ),
        1,
    );
}

// Compose: LazyListTest.testLookaheadPositionWithTwoInBoundTwoOutBound
/// Six items from the third, two leaving the view and two coming in.
#[test]
fn two_items_in_and_two_out_of_view_are_where_they_go_ahead() {
    positions_with_the_placement_animator(
        ((0..6).collect(), vec![5, 4, 2, 1, 3, 0]),
        (
            vec![None, None, Some(0), Some(100), Some(200), Some(300)],
            [300, 100, 0, 200, -100, -200].map(Some).to_vec(),
        ),
        2,
    );
}

/// A list measured `ahead` square ahead and `now()` square now, of four items 100 square, noting
/// where they're placed.
fn sized_by_pass_list(
    lazy: Lazy,
    ahead: i32,
    now: impl Fn() -> i32 + 'static,
    positions: &Positions,
) -> ComposeTestRule {
    let positions = positions.clone();
    let now = std::rc::Rc::new(now);
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
            let state = remember_lazy_list_state(0, 0);
            let positions = positions.clone();
            LazyColumnOrRow(lazy, by_pass, state, no_padding(), move |list: LazyListScope| {
                let positions = positions.clone();
                list.items(4, move |item| {
                    let modifier = Modifier.required_size(Dp(100.0));
                    Box!(modifier = modifier.then(track(&positions, scope, item as i32)));
                });
            });
        }));
    })
}

/// Ahead all four are where they go, now those before `shown`. (`lookaheadSizeSmallerThanPostLookahead`
/// isn't here: its items composed by the pass that's now alone are placed ahead where they are now,
/// Compose's `needsToBePlacedInApproach`, which compose-rs's lookahead doesn't do yet.)
fn check_shown(ui: &mut ComposeTestRule, lazy: Lazy, positions: &Positions, shown: i32, at: i32) {
    ui.wait_for_idle();
    for item in 0..4 {
        let what = format!("{lazy:?} item {item} at {at}");
        let main = item * 100;
        assert_eq!(main_axis(lazy, positions.ahead(item)), Some(main), "{what}");
        let now = (item < shown).then_some(main);
        assert_eq!(main_axis(lazy, positions.now(item)), now, "{what}");
    }
}

// Compose: LazyListTest.resizeLazyList
/// A list 400 square ahead and 200, 290, 340, 270 and 120 now: ahead all four are where they go,
/// now those its size shows, growing and shrinking.
#[test]
fn a_list_resized_now_only_places_what_it_shows_now() {
    for lazy in [Lazy::column(), Lazy::row()] {
        let now = mutable_state_of(200);
        let positions = Positions::default();
        let n = now.handle();
        let mut ui = sized_by_pass_list(lazy, 400, move || n.get(), &positions);
        check_shown(&mut ui, lazy, &positions, 2, 200);
        let steps = [(290, 3, false), (340, 4, false), (270, 3, true), (120, 2, true)];
        for (size, shown, cleared) in steps {
            now.set(size);
            if cleared {
                positions.now.borrow_mut().clear();
            }
            check_shown(&mut ui, lazy, &positions, shown, size);
        }
    }
}

// Compose: LazyListTest.testRetainedItemsDuringScrolling
/// A list of 50 items 10 long, 40 long, from the 41st, jumped back two items at a time ten times:
/// as many items stay composed as in the same list out of a scope.
#[test]
fn a_list_in_a_scope_keeps_as_many_items_as_out_of_one_while_jumping() {
    for lazy in [Lazy::column(), Lazy::row()] {
        let composed: std::rc::Rc<std::cell::RefCell<[Vec<usize>; 2]>> = std::rc::Rc::default();
        let owners = [LazyListState::new(40, 0), LazyListState::new(40, 0)];
        let states = [*owners[0], *owners[1]];
        let c = composed.clone();
        let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
            let _ = &owners;
            let c = c.clone();
            let list = move |group: usize| {
                let c = c.clone();
                let modifier = Modifier.size((Dp(40.0), Dp(40.0)));
                LazyColumnOrRow(lazy, modifier, states[group], no_padding(), move |list| {
                    let c = c.clone();
                    list.items(50, move |item| {
                        Box!(modifier = Modifier.size(Dp(10.0)));
                        let c = c.clone();
                        DisposableEffect(item, move |effect| {
                            c.borrow_mut()[group].push(item);
                            let c = c.clone();
                            effect
                                .on_dispose(move || c.borrow_mut()[group].retain(|it| *it != item))
                        });
                    });
                });
            };
            let in_scope = list.clone();
            Box!(move |_| {
                let in_scope = in_scope.clone();
                compose_ui::LookaheadScope(callback(move |_| in_scope(0)));
            });
            list(1);
        });
        ui.wait_for_idle();
        let sizes = |composed: &std::rc::Rc<std::cell::RefCell<[Vec<usize>; 2]>>| {
            let composed = composed.borrow();
            (composed[0].len(), composed[1].len())
        };
        let (scoped, plain) = sizes(&composed);
        assert_eq!(scoped, plain, "{lazy:?} at first");
        for step in 0..10 {
            let index = 40 - step * 2;
            for state in states {
                let _ = run(&mut ui, async move { state.scroll_to_item(index, 0).await });
            }
            ui.wait_for_idle();
            let (scoped, plain) = sizes(&composed);
            assert_eq!(scoped, plain, "{lazy:?} at {index}");
        }
    }
}

// Compose: LazyListTest.reorderingInLookahead
/// 500 animated items in a list two long shuffled 20 times, a frame each: nothing breaks.
#[test]
fn items_shuffled_in_a_scope_frame_after_frame_lay_out() {
    for lazy in [Lazy::column(), Lazy::row()] {
        let items = mutable_state_of((0..500).collect::<Vec<i32>>());
        let i = items.handle();
        let mut ui = paused(IntSize::new(1000, 1000), move || {
            compose_ui::LookaheadScope(callback(move |_| {
                let modifier = lazy.main_axis_size(Dp(100.0));
                let state = remember_lazy_list_state(0, 0);
                let list = i.get();
                LazyColumnOrRow(
                    lazy,
                    modifier,
                    state,
                    no_padding(),
                    move |scope: LazyListScope| {
                        let item_scope = scope.item_scope();
                        scope.items_list_keyed(
                            list.clone(),
                            |it| *it,
                            move |_, &item| {
                                let modifier = Modifier
                                    .animate_item(&item_scope)
                                    .then(lazy.main_axis_size(Dp(50.0)));
                                Box!(modifier = modifier, move |_| {
                                    Box!(move |_| BasicText!(format!("Item {item}")));
                                });
                            },
                        );
                    },
                );
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
