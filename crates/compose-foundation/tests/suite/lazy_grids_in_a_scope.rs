//! Lazy grids and staggered grids in a `LookaheadScope`, sized one way ahead and another now
//! (D1226, as lists): what they tell and keep is the pass ahead's, and their animated items are
//! placed where they are ahead. Density 1.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, Dp, IntSize, UnitExt};

use crate::lazy_animate_support::*;

fn dp(pixels: i32) -> Dp {
    pixels.dp()
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Grid,
    Staggered,
}

const KINDS: [Kind; 2] = [Kind::Grid, Kind::Staggered];

/// Either grid's state, for what the tests ask of it.
#[derive(Clone, Copy)]
enum State {
    Grid(LazyGridState),
    Staggered(LazyStaggeredGridState),
}

impl State {
    fn first(self) -> (usize, i32) {
        match self {
            State::Grid(it) => {
                (it.first_visible_item_index(), it.first_visible_item_scroll_offset())
            }
            State::Staggered(it) => {
                (it.first_visible_item_index(), it.first_visible_item_scroll_offset())
            }
        }
    }

    fn can_scroll(self) -> (bool, bool) {
        match self {
            State::Grid(it) => (it.can_scroll_backward(), it.can_scroll_forward()),
            State::Staggered(it) => (it.can_scroll_backward(), it.can_scroll_forward()),
        }
    }

    fn dispatch(self, delta: f32) -> f32 {
        match self {
            State::Grid(it) => it.dispatch_raw_delta(delta),
            State::Staggered(it) => it.dispatch_raw_delta(delta),
        }
    }

    fn scroll_to(self, index: usize) {
        match self {
            State::Grid(it) => it.request_scroll_to_item(index, 0),
            State::Staggered(it) => it.request_scroll_to_item(index, 0),
        }
    }

    fn visible(self) -> usize {
        match self {
            State::Grid(it) => it.layout_info().visible_items_info.len(),
            State::Staggered(it) => it.layout_info().visible_items_info.len(),
        }
    }
}

/// What a scene's grid holds: `keys`, each `item` tall, animated if `animated`.
#[derive(Clone)]
struct Content {
    keys: Rc<dyn Fn() -> Vec<i32>>,
    item: i32,
    animated: bool,
    composed: Rc<Cell<usize>>,
}

/// A grid of `kind`, two cells across 100, `ahead` tall ahead and `now` tall now, in a scope.
fn scene(kind: Kind, ahead: i32, now: i32, content: Content) -> (ComposeTestRule, State) {
    let grid_owner = LazyGridState::new(0, 0);
    let staggered_owner = LazyStaggeredGridState::new(0, 0);
    let state = match kind {
        Kind::Grid => State::Grid(*grid_owner),
        Kind::Staggered => State::Staggered(*staggered_owner),
    };
    let ui = paused(IntSize::new(1000, 1000), move || {
        let _ = (&grid_owner, &staggered_owner);
        let content = content.clone();
        compose_ui::LookaheadScope(callback(move |_| {
            let by_pass = Modifier.layout(move |scope, measurable, _| {
                let tall = if scope.is_looking_ahead() { ahead } else { now };
                let placeable = scope.measure(measurable, Constraints::fixed(100, tall));
                scope.layout_with(placeable.width, placeable.height, move |placement| {
                    placement.place(&placeable, 0, 0);
                })
            });
            let keys = (content.keys)();
            let (item, animated, composed) =
                (content.item, content.animated, content.composed.clone());
            let cell = move |key: i32, animate: Modifier| {
                composed.set(composed.get() + 1);
                let sized = animate.height(dp(item)).fill_max_width();
                Box!(modifier = sized.test_tag(key.to_string()));
            };
            match state {
                State::Grid(state) => {
                    LazyVerticalGrid!(
                        GridCells::Fixed(2),
                        modifier = by_pass,
                        state = state,
                        move |scope: LazyGridScope| {
                            let item_scope = scope.item_scope();
                            let cell = cell.clone();
                            scope.items_list_keyed(
                                keys.clone(),
                                |it: &i32| *it,
                                move |it: &i32| {
                                    let animate = if animated {
                                        Modifier.animate_item_with(
                                            &item_scope,
                                            None,
                                            Some(anim_spec()),
                                            None,
                                        )
                                    } else {
                                        Modifier
                                    };
                                    cell(*it, animate);
                                },
                            );
                        }
                    );
                }
                State::Staggered(state) => {
                    LazyVerticalStaggeredGrid!(
                        StaggeredGridCells::Fixed(2),
                        modifier = by_pass,
                        state = state,
                        move |scope: LazyStaggeredGridScope| {
                            let cell = cell.clone();
                            scope.items_list_keyed(
                                keys.clone(),
                                |it: &i32| *it,
                                move |it: &i32| {
                                    let animate = if animated {
                                        Modifier.animate_item_with(
                                            &LazyStaggeredGridItemScope,
                                            None,
                                            Some(anim_spec()),
                                            None,
                                        )
                                    } else {
                                        Modifier
                                    };
                                    cell(*it, animate);
                                },
                            );
                        }
                    );
                }
            }
        }));
    });
    (ui, state)
}

fn plain(count: i32, item: i32) -> Content {
    Content {
        keys: Rc::new(move || (0..count).collect()),
        item,
        animated: false,
        composed: Rc::default(),
    }
}

/// All in view ahead, smaller now: it can't scroll, and a scroll takes nothing.
#[test]
fn a_grid_all_in_view_ahead_takes_no_scroll_now() {
    for kind in KINDS {
        let (mut ui, state) = scene(kind, 200, 50, plain(10, 20));
        ui.wait_for_idle();
        assert_eq!(state.visible(), 10, "{kind:?}");
        assert_eq!(state.can_scroll(), (false, false), "{kind:?}");
        let consumed = ui.run_on_ui_thread(move |_| state.dispatch(30.0));
        ui.wait_for_idle();
        assert_eq!((consumed, state.first()), (0.0, (0, 0)), "{kind:?}");
    }
}

/// At its end ahead and larger now: what's now steps back to fill its room, but leaves the scroll
/// it couldn't take to no one, so the grid stays where it's ahead.
#[test]
fn a_grid_at_its_end_ahead_stays_there_when_larger_now() {
    for kind in KINDS {
        let (mut ui, state) = scene(kind, 50, 200, plain(40, 20));
        ui.wait_for_idle();
        ui.run_on_ui_thread(move |_| state.scroll_to(39));
        ui.wait_for_idle();
        assert_eq!(state.first(), (34, 10), "{kind:?}");
        for _ in 0..3 {
            ui.host().frame();
            ui.wait_for_idle();
            assert_eq!(state.first(), (34, 10), "{kind:?}");
            assert_eq!(state.can_scroll(), (true, false), "{kind:?}");
        }
    }
}

/// Larger now than ahead, or smaller: the items either pass places stay composed, idle time after
/// idle time (what's composed is what either wants).
#[test]
fn a_grid_keeps_what_either_pass_places_composed() {
    for kind in KINDS {
        for (ahead, now) in [(50, 200), (200, 50)] {
            let content = plain(80, 20);
            let composed = content.composed.clone();
            let (mut ui, state) = scene(kind, ahead, now, content);
            ui.wait_for_idle();
            let settled = composed.get();
            let later = Instant::now() + Duration::from_secs(60);
            for _ in 0..5 {
                while ui.host().prefetch(later) {}
                ui.run_on_ui_thread(move |_| state.dispatch(0.0));
                ui.host().frame();
                ui.wait_for_idle();
            }
            assert_eq!(composed.get(), settled, "{kind:?}, {ahead} ahead, {now} now");
        }
    }
}

/// At its end ahead, larger now, its items animated: the pass that's now places them where they
/// are ahead (the last two at 30, where its own layout, scrolled back, would have them at 180),
/// and swapped, each moves across to the other's cell.
#[test]
fn a_grid_s_animated_items_are_where_they_are_ahead() {
    for kind in KINDS {
        let keys = mutable_state_of((0..40).collect::<Vec<i32>>());
        let handle = keys.handle();
        let content = Content {
            keys: Rc::new(move || handle.get()),
            item: 20,
            animated: true,
            composed: Rc::default(),
        };
        let (mut ui, state) = scene(kind, 50, 200, content);
        ui.wait_for_idle();
        ui.run_on_ui_thread(move |_| state.scroll_to(39));
        ui.wait_for_idle();
        assert_eq!((position_of(&mut ui, "38"), position_of(&mut ui, "39")), ((0, 30), (50, 30)));
        let set = keys.handle();
        ui.run_on_ui_thread(move |_| {
            let mut swapped: Vec<i32> = (0..38).collect();
            swapped.extend([39, 38]);
            set.set(swapped);
        });
        on_animation_frame(&mut ui, |ui, fraction| {
            let across = |from: f32, to: f32| (from + (to - from) * fraction).round() as i32;
            assert_eq!(
                (position_of(ui, "38"), position_of(ui, "39")),
                ((across(0.0, 50.0), 30), (across(50.0, 0.0), 30)),
                "{kind:?} at {fraction}"
            );
        });
    }
}

/// Where a staggered grid's items are, scrolled `by` four times and back three, `ahead` wide ahead and `now` wide now, its
/// lanes at least 50 across: (index, lane, offset along) of those in view, as it says.
fn staggered_lanes(ahead: i32, now: i32, by: f32) -> Vec<(usize, usize, i32)> {
    let owner = LazyStaggeredGridState::new(0, 0);
    let state = *owner;
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let _ = &owner;
        compose_ui::LookaheadScope(callback(move |_| {
            let by_pass = Modifier.layout(move |scope, measurable, _| {
                let wide = if scope.is_looking_ahead() { ahead } else { now };
                let placeable = scope.measure(measurable, Constraints::fixed(wide, 100));
                scope.layout_with(placeable.width, placeable.height, move |placement| {
                    placement.place(&placeable, 0, 0);
                })
            });
            LazyVerticalStaggeredGrid!(
                StaggeredGridCells::Adaptive(dp(50)),
                modifier = by_pass,
                state = state,
                move |scope: LazyStaggeredGridScope| {
                    scope.items(60, |index| {
                        let tall = 20 + (index as i32 * 7) % 30;
                        Box!(modifier = Modifier.height(dp(tall)).fill_max_width());
                    });
                }
            );
        }));
    });
    ui.wait_for_idle();
    // Forward, then back over items whose lanes it keeps.
    for delta in [by, by, by, by, -by, -by, -by] {
        ui.run_on_ui_thread(move |_| state.dispatch_raw_delta(delta));
        ui.wait_for_idle();
    }
    let info = state.layout_info();
    info.visible_items_info.iter().map(|it| (it.index, it.lane, it.offset.y)).collect()
}

/// Two lanes ahead and four now: the lanes the pass ahead put its items in are its own, the pass
/// that's now leaving them as they were, so the grid scrolls ahead as it would with no other pass.
#[test]
fn a_staggered_grid_with_other_lanes_now_keeps_those_ahead() {
    assert_eq!(staggered_lanes(100, 200, 70.0), staggered_lanes(100, 100, 70.0));
}
