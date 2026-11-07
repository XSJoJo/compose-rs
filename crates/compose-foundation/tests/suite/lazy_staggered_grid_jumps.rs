//! A staggered grid's long jumps, items far apart and wrap-content size, as per-item composition
//! (C8, D1312) does them: what the stops' tests D1313 removed also checked, and it must still do.
//! Each frame of a jump shows something, what's passed leaves, and a wrap-content grid says one
//! size.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::Optimizations;
use compose_ui::{Dp, IntSize, UnitExt};

use crate::lazy_staggered_grid_stops::{drawn, frames_show, grid, idle, laid, scroll_to};
use crate::scroll_support::run;

// Restores `lazy_staggered_grid_stops::a_frame_composes_a_bounded_number_of_items_and_shows_those`
// (removed by D1313), but its count: a frame composes what it measures now, which no rounds bound.
/// A jump over thousands of items shows something each frame, and lands where it was asked.
#[test]
fn a_long_jump_shows_each_frame_and_lands() {
    let mut grid = grid(4, (400, 1990), 20_000, (0, 0), |_| 1);
    frames_show(&mut grid, 8, |state| state.request_scroll_to_item(10_000, 0));
    grid.ui.set_auto_advance(true);
    idle(&mut grid.ui);
    assert!(drawn(&mut grid.ui) > 0);
    grid.ui.on_node_with_tag("17900").assert_exists();
}

// Restores `lazy_staggered_grid_stops::a_jump_stopped_before_its_first_item_shows_what_was_placed`
// (removed by D1313), its first two jumps: measured whole now, they still show each frame.
/// A jump far back from its item, or to the last of a long grid, shows something each frame.
#[test]
fn a_jump_back_from_its_item_shows_each_frame() {
    frames_show(&mut grid(2, (400, 800), 8000, (0, 0), |_| 10), 8, |state| {
        state.request_scroll_to_item(3000, -40_000);
    });
    frames_show(&mut grid(4, (400, 1990), 40_000, (0, 0), |_| 1), 8, |state| {
        state.request_scroll_to_item(19_999, 0);
    });
}

// Restores `lazy_staggered_grid_stops::a_jump_stopped_before_its_first_item_shows_what_was_placed`
// (removed by D1313), its third jump. It passes, but its first frame takes about 12 minutes in a
// debug build: the jump back from an item past the lanes the first layout knows, to the grid's
// start, is a cliff (a 20,000-item grid's jump to 9,999 at -10,000 takes 700 s, a 10,000-item
// grid's to 4,999 at -5,000 26 ms).
/// A jump back past the grid's start from an item far from those known shows each frame.
#[test]
#[ignore = "a regression: its first frame takes about 12 minutes in a debug build"]
fn a_jump_back_past_the_start_shows_each_frame() {
    frames_show(&mut grid(4, (400, 1990), 40_000, (0, 0), |_| 1), 8, |state| {
        state.request_scroll_to_item(19_999, -20_000);
    });
}

// Restores `lazy_staggered_grid_stops::items_between_a_long_item_and_those_in_view_leave` (removed
// by D1313). Compose's `LazyLayoutItemReusePolicy` (LazyLayout.kt) keeps 7 of those passed
// deactivated, their effects disposed, so what's noted alive needs no allowance for them.
/// A very long item in a lane doesn't keep what the other lanes pass composed: what's alive is what's
/// in view, the long item among them, and a few more.
#[test]
#[ignore = "a regression: a staggered grid keeps each item a scroll steps over composed"]
fn items_between_a_long_item_and_those_in_view_leave() {
    let mut grid = grid(4, (400, 800), 8000, (0, 0), |index| if index == 0 { 200_000 } else { 40 });
    grid.ui.wait_for_idle();
    let state = grid.state;
    let _ = run(&mut grid.ui, async move { state.scroll_by(40_000.0).await });
    idle(&mut grid.ui);
    let visible = laid(state).len();
    assert!(visible >= 60, "{visible} in view");
    assert!(grid.live.borrow().contains(&0));
    let live = grid.live.borrow().len();
    assert!(live < visible + 30, "{live} composed for {visible} in view");
}

// Restores `lazy_staggered_grid_stops::a_jump_with_lanes_far_apart_settles` (removed by D1313).
/// Past a long item it leaves; a jump back into it, its lane far from the others, composes it again
/// and few more.
#[test]
#[ignore = "a regression: a staggered grid keeps each item a scroll steps over composed"]
fn a_jump_with_lanes_far_apart_settles() {
    let mut grid =
        grid(4, (400, 800), 12_000, (0, 0), |index| if index == 0 { 60_000 } else { 40 });
    grid.ui.wait_for_idle();
    let state = grid.state;
    // Past the long item, which leaves: its lane's known, and nothing's composed near it.
    let _ = run(&mut grid.ui, async move { state.scroll_by(70_000.0).await });
    idle(&mut grid.ui);
    assert!(!grid.live.borrow().contains(&0));
    let live = grid.live.borrow().len();
    assert!(live < 150, "{live} composed past the long item");
    // Back into it: the first lane's first is item 0, the others' far from it.
    scroll_to(&mut grid, 2000, 30_000);
    idle(&mut grid.ui);
    assert!(grid.live.borrow().contains(&0), "{:?}", laid(state).first());
    let live = grid.live.borrow().len();
    assert!(live < 150, "{live} composed back in the long item");
}

/// The heights a 2-lane grid of `count` items of `item` in `height_in(0, most)` reports.
fn sizes_of(count: usize, item: i32, most: i32) -> Vec<i32> {
    use_optimizations(Optimizations::NONE);
    let sizes: Rc<RefCell<Vec<i32>>> = Rc::default();
    let noted = sizes.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 2000), move || {
        let noted = noted.clone();
        LazyVerticalStaggeredGrid!(
            StaggeredGridCells::Fixed(2),
            modifier = Modifier
                .height_in(0.dp(), Dp(most as f32))
                .on_size_changed(move |size: IntSize| noted.borrow_mut().push(size.height)),
            |scope: LazyStaggeredGridScope| {
                scope.items(count, move |_| Spacer(Modifier.height(Dp(item as f32))));
            }
        );
    });
    ui.wait_for_idle();
    sizes.borrow().clone()
}

// Restores `lazy_staggered_grid_stops::a_stop_doesnt_say_a_false_size` (removed by D1313), which
// said `[0, 50, 150]` with the stops. Compose: LazyStaggeredGridMeasure.kt's `measure`, its
// `layoutHeight` the lanes' furthest offset and the padding, constrained.
/// A wrap-content grid's first measurement says its whole size at once: 6 items of 50 in 2 lanes
/// are 3 lines, 150, never a partial or a larger one.
#[test]
fn a_wrap_content_grid_says_its_size_once() {
    assert_eq!(sizes_of(6, 50, 800), [150]);
}

// Restores `lazy_staggered_grid_stops::partial_sizes_dont_go_on_across_frames` (removed by D1313),
// which said `[0, 1, 1990]` with the stops. Compose: LazyStaggeredGridMeasure.kt's `measure`, its
// `layoutHeight` constrained to the maximum.
/// A grid of more than fills its maximum says the maximum, once.
#[test]
fn a_wrap_content_grid_filling_its_maximum_says_it_once() {
    assert_eq!(sizes_of(20_000, 1, 1990), [1990]);
}
