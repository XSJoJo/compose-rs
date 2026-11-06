//! Adversarial review of C8 on main (D1312-D1317).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, Dp, IntSize, UnitExt};

fn zero() -> Option<Rc<dyn LazyLayoutCacheWindow>> {
    Some(lazy_layout_cache_window_with(Dp(0.0), Dp(0.0), false))
}

fn by_pass(ahead: i32, now: i32) -> Modifier {
    Modifier.layout(move |scope, measurable, constraints| {
        let tall = if scope.is_looking_ahead() { ahead } else { now };
        let placeable =
            scope.measure(measurable, Constraints::new(0, constraints.max_width, tall, tall));
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            placement.place(&placeable, 0, 0);
        })
    })
}

#[derive(Clone, Copy)]
enum Kind {
    List,
    Grid,
    Staggered,
}

/// Body runs, settled, then after 5 remeasures with idle times between: what either pass places
/// stays composed, with a cache window of no size (D1316's `wanted_now`).
fn composed_runs(
    kind: Kind,
    ahead: i32,
    now: i32,
    window: Option<Rc<dyn LazyLayoutCacheWindow>>,
) -> (usize, usize) {
    let composed = Rc::new(Cell::new(0usize));
    let c = composed.clone();
    let list = LazyListState::new(0, 0);
    let grid = LazyGridState::new(0, 0);
    let staggered = LazyStaggeredGridState::new(0, 0);
    let (l, g, s) = (*list, *grid, *staggered);
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 1000), move || {
        let _ = (&list, &grid, &staggered);
        let c = c.clone();
        let window = window.clone();
        compose_ui::LookaheadScope(callback(move |_| {
            let c = c.clone();
            let body = move |_: usize| {
                c.set(c.get() + 1);
                Box!(modifier = Modifier.size(20.dp()), {});
            };
            match kind {
                Kind::List => {
                    LazyColumn!(
                        state = l,
                        modifier = by_pass(ahead, now),
                        cache_window = window.clone(),
                        move |items: LazyListScope| {
                            items.items(80, body.clone());
                        }
                    );
                }
                Kind::Grid => {
                    LazyVerticalGrid!(
                        GridCells::Fixed(2),
                        state = g,
                        modifier = by_pass(ahead, now),
                        cache_window = window.clone(),
                        move |items: LazyGridScope| {
                            items.items(80, body.clone());
                        }
                    );
                }
                Kind::Staggered => {
                    LazyVerticalStaggeredGrid!(
                        StaggeredGridCells::Fixed(2),
                        state = s,
                        modifier = by_pass(ahead, now),
                        cache_window = window.clone(),
                        move |items: LazyStaggeredGridScope| {
                            items.items(80, body.clone());
                        }
                    );
                }
            }
        }));
    });
    ui.wait_for_idle();
    let settled = composed.get();
    let later = std::time::Instant::now() + std::time::Duration::from_secs(60);
    for _ in 0..5 {
        while ui.host().prefetch(later) {}
        match kind {
            Kind::List => l.force_remeasure(),
            Kind::Grid => g.force_remeasure(),
            Kind::Staggered => s.force_remeasure(),
        }
        ui.wait_for_idle();
    }
    (settled, composed.get())
}

#[test]
fn review_c8_either_pass_keeps_composed_with_no_window() {
    let mut failures = Vec::new();
    for kind in [Kind::List, Kind::Grid, Kind::Staggered] {
        for (ahead, now) in [(50, 200), (200, 50)] {
            for (name, window) in [("zero", zero()), ("default", None)] {
                let (settled, after) = composed_runs(kind, ahead, now, window);
                let label = match kind {
                    Kind::List => "list",
                    Kind::Grid => "grid",
                    Kind::Staggered => "staggered",
                };
                if settled != after {
                    failures.push(format!("{label} {ahead}/{now} {name}: {settled} -> {after}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

thread_local! {
    static TIMES: RefCell<HashMap<usize, usize>> = RefCell::default();
}

/// A list scrolling one way with prefetch between frames: no item composed more than once.
#[test]
fn review_c8_list_scrolling_one_way_composes_each_item_once() {
    for nested_window in [None, zero()] {
        TIMES.with(|t| t.borrow_mut().clear());
        let state = LazyListState::new(0, 0);
        let given = *state;
        let window = nested_window.clone();
        let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), move || {
            let window = window.clone();
            LazyColumn!(
                modifier = Modifier.size((300.dp(), 200.dp())),
                state = given,
                cache_window = window,
                move |scope: LazyListScope| {
                    scope.items_with(
                        5000,
                        |index| index % 4 == 0,
                        |index| {
                            TIMES.with(|t| *t.borrow_mut().entry(index).or_default() += 1);
                            Box!(modifier = Modifier.size((300.dp(), 30.dp())), {});
                        },
                    );
                }
            );
        });
        for _ in 0..300 {
            state.dispatch_raw_delta(9.0);
            ui.wait_for_idle();
            ui.host().prefetch(std::time::Instant::now() + std::time::Duration::from_secs(60));
        }
        let times = TIMES.with(|t| t.borrow().clone());
        let mut again: Vec<(usize, usize)> = times.into_iter().filter(|(_, n)| *n > 1).collect();
        again.sort_unstable();
        assert!(again.is_empty(), "composed again {:?}", &again[..again.len().min(20)]);
    }
}

/// Slot reuse: an item reusing another key's slot gets its own `remember`, and scrolled back the
/// first key gets its saved state (`remember_saveable`), never the other's.
#[test]
fn review_c8_reused_slot_drops_old_state_and_saveable_comes_back() {
    let seen: Rc<RefCell<Vec<(usize, usize, usize)>>> = Rc::default();
    let s = seen.clone();
    let state = LazyListState::new(0, 0);
    let given = *state;
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        let s = s.clone();
        LazyColumn!(
            modifier = Modifier.size((100.dp(), 100.dp())),
            state = given,
            move |scope: LazyListScope| {
                let s = s.clone();
                scope.items(200, move |index| {
                    let plain = remember(move || index);
                    let saved = remember_saveable(move || mutable_state_of(index * 1000));
                    if saved.get() == index * 1000 {
                        saved.set(index * 1000 + 1);
                    }
                    s.borrow_mut().push((index, plain, saved.get()));
                    Box!(modifier = Modifier.size((100.dp(), 50.dp())), {});
                });
            }
        );
    });
    ui.wait_for_idle();
    for target in [100, 0, 150, 1, 0] {
        state.request_scroll_to_item(target, 0);
        ui.wait_for_idle();
        ui.host().prefetch(std::time::Instant::now() + std::time::Duration::from_secs(60));
        ui.wait_for_idle();
    }
    for (index, plain, saved) in seen.borrow().iter() {
        assert_eq!(*plain, *index, "remember leaked across keys: {:?}", (index, plain, saved));
        assert!(*saved / 1000 == *index, "saveable leaked: {:?}", (index, plain, saved));
    }
}
