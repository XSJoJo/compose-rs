//! Compose's `LookaheadScopeTest` scenes with foundation's layouts in them: rows, columns, spacers.
//! Density 1.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, IntSize, LookaheadScope, UnitExt};

fn dp(pixels: i32) -> compose_ui::Dp {
    pixels.dp()
}

fn in_a_scope(content: impl Fn(LookaheadScope) + 'static) {
    LookaheadScope(callback(content));
}

/// The tests' `animateSize`, its animation over at once: the size ahead, measured as it is.
fn animate_size() -> Modifier {
    Modifier.approach_layout(
        |_| false,
        |_, _| false,
        |scope, content, _| {
            let ahead = scope.lookahead_size();
            let placeable = scope.measure(content, Constraints::fixed(ahead.width, ahead.height));
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        },
    )
}

fn noting_size(size: &Rc<Cell<IntSize>>) -> Modifier {
    let size = size.clone();
    Modifier.on_size_changed(move |it: IntSize| size.set(it))
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, lookaheadLayoutAnimation
#[test]
fn children_by_weight_follow_their_row_s_size_ahead() {
    let large = mutable_state_of(true);
    let (first, second) = (Rc::new(Cell::new(IntSize::ZERO)), Rc::new(Cell::new(IntSize::ZERO)));
    let (l, a, b) = (large.handle(), first.clone(), second.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (a, b) = (a.clone(), b.clone());
        in_a_scope(move |_| {
            let (a, b) = (a.clone(), b.clone());
            let size =
                if l.get() { Modifier.size(dp(200)) } else { Modifier.size((dp(50), dp(100))) };
            Row!(modifier = size, move |scope| {
                let child = |weight: f32, noted: &Rc<Cell<IntSize>>| {
                    Modifier
                        .fill_max_height()
                        .weight(scope, weight)
                        .then(noting_size(noted))
                        .then(animate_size())
                };
                Box!(modifier = child(2.0, &a), {});
                Box!(modifier = child(3.0, &b), {});
            });
        });
    });
    ui.wait_for_idle();
    assert_eq!((first.get(), second.get()), (IntSize::new(80, 200), IntSize::new(120, 200)));
    large.set(false);
    ui.wait_for_idle();
    assert_eq!((first.get(), second.get()), (IntSize::new(20, 100), IntSize::new(30, 100)));
    large.set(true);
    ui.wait_for_idle();
    assert_eq!((first.get(), second.get()), (IntSize::new(80, 200), IntSize::new(120, 200)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, parentObserveActualMeasurementTest
#[test]
fn what_s_around_a_scope_measures_with_what_s_now() {
    let (width, height) = (200, 120);
    let tenths = mutable_state_of(1);
    let (parent, grandparent, great) = (
        Rc::new(Cell::new(IntSize::ZERO)),
        Rc::new(Cell::new(IntSize::ZERO)),
        Rc::new(Cell::new(IntSize::ZERO)),
    );
    let (t, p, g, gg) = (tenths.handle(), parent.clone(), grandparent.clone(), great.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (p, g, gg) = (p.clone(), g.clone(), gg.clone());
        Column!(modifier = noting_size(&gg), |_| {
            let (p, g) = (p.clone(), g.clone());
            Row!(modifier = noting_size(&g), |_| {
                let p = p.clone();
                Box!(modifier = noting_size(&p), {
                    in_a_scope(move |_| {
                        let approaching = Modifier.approach_layout(
                            move |_| t.get() <= 30,
                            |_, _| false,
                            move |scope, content, constraints| {
                                assert_eq!(scope.lookahead_size(), IntSize::new(width, height));
                                let placeable = scope.measure(content, constraints);
                                let scale = t.get() as f32 / 10.0;
                                let (w, h) = (
                                    (scale * width as f32).round() as i32,
                                    (scale * height as f32).round() as i32,
                                );
                                scope.layout_with(w, h, move |placement| {
                                    placement.place(&placeable, 0, 0);
                                })
                            },
                        );
                        Box!(modifier = approaching.size((dp(width), dp(height))), {});
                    });
                });
                Spacer(Modifier.size(dp(20)));
            });
            Spacer(Modifier.size(dp(50)));
        });
    });
    for _ in 0..20 {
        ui.wait_for_idle();
        let scale = tenths.get() as f32 / 10.0;
        let size = IntSize::new(
            (width as f32 * scale).round() as i32,
            (height as f32 * scale).round() as i32,
        );
        assert_eq!(parent.get(), size);
        assert_eq!(grandparent.get(), IntSize::new(size.width + 20, size.height.max(20)));
        assert_eq!(
            great.get(),
            IntSize::new(grandparent.get().width.max(50), grandparent.get().height + 50)
        );
        tenths.set(tenths.get() + 1);
    }
}

/// The tests' `animateSize`: toward the size ahead, over 200 ms.
#[compose_runtime::composable]
fn AnimatingSize(before: Modifier, after: Modifier) {
    let animation: Rc<std::cell::RefCell<Option<compose_animation::Animatable<IntSize>>>> =
        compose_runtime::remember(Rc::default);
    let scope = compose_runtime::remember_coroutine_scope();
    let (a, a2) = (animation.clone(), animation);
    let approaching = Modifier.approach_layout(
        move |ahead| {
            let mut slot = a.borrow_mut();
            let animation = slot.get_or_insert_with(|| compose_animation::Animatable::new(ahead));
            animation.target_value() != ahead || animation.is_running()
        },
        |_, _| false,
        move |measure, content, _| {
            let ahead = measure.lookahead_size();
            let mut slot = a2.borrow_mut();
            let animation =
                slot.get_or_insert_with(|| compose_animation::Animatable::new(ahead)).clone();
            if animation.target_value() != ahead {
                let going = animation.clone();
                scope.launch(async move {
                    let _ = going.animate_to(ahead, compose_animation::tween(200)).await;
                });
            }
            let at = animation.value();
            let placeable = measure.measure(content, Constraints::fixed(at.width, at.height));
            measure.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        },
    );
    Box!(modifier = before.then(approaching).then(after), {});
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, lookaheadLayoutAnimationParentWidth
#[test]
fn right_to_left_a_child_animates_toward_where_it_is_ahead() {
    let child = mutable_state_of(20);
    let (ahead, now) = (
        Rc::new(Cell::new(compose_ui::Offset::ZERO)),
        Rc::new(Cell::new(compose_ui::Offset::ZERO)),
    );
    let (c, a, n) = (child.handle(), ahead.clone(), now.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (a, n) = (a.clone(), n.clone());
        in_a_scope(move |_| {
            let (a, n) = (a.clone(), n.clone());
            CompositionLocalProvider(
                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(compose_ui::LayoutDirection::Rtl),
                move || {
                    let (a, n) = (a.clone(), n.clone());
                    let sized_by_pass = Modifier.layout(|scope, measurable, _| {
                        let side = if scope.is_looking_ahead() { 300 } else { 200 };
                        let placeable = scope.measure(measurable, Constraints::fixed(side, side));
                        scope.layout_with(side, side, move |placement| {
                            placement.place(&placeable, 0, 0)
                        })
                    });
                    let noting = Modifier.layout(move |scope, measurable, constraints| {
                        let placeable = scope.measure(measurable, constraints);
                        let (a, n) = (a.clone(), n.clone());
                        scope.layout_with(placeable.width, placeable.height, move |placement| {
                            let own = placement.coordinates().expect("placing");
                            let parent = own.parent_coordinates().expect("in the node");
                            let at = parent.local_position_of(&own, compose_ui::Offset::ZERO);
                            if placement.is_looking_ahead() {
                                a.set(at)
                            } else {
                                n.set(at)
                            }
                            placement.place(&placeable, 0, 0);
                        })
                    });
                    let before = Modifier
                        .size(dp(200))
                        .then(sized_by_pass)
                        .wrap_content_width_with(compose_ui::Alignment::START, false)
                        .then(noting);
                    AnimatingSize(before, Modifier.width(dp(c.get())));
                },
            );
        });
    });
    ui.set_auto_advance(false);
    ui.wait_for_idle();
    assert_eq!((now.get().x, ahead.get().x), (180.0, 280.0));
    child.set(100);
    ui.advance_time_by(std::time::Duration::from_millis(50));
    let now_x = now.get().x;
    assert!(now_x < 180.0 && now_x > 100.0, "{now_x}");
    assert_eq!(ahead.get().x, 200.0);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, testPlacementChangeInLookahead
#[test]
fn what_s_placed_changes_in_one_pass_and_not_the_other() {
    let (tall, last_now, ahead) =
        (mutable_state_of(false), mutable_state_of(false), mutable_state_of(true));
    let (t, l, a) = (tall.handle(), last_now.handle(), ahead.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        in_a_scope(move |_| {
            let policy = compose_ui::measure_policy(move |scope, measurables, constraints| {
                let placeables: Vec<_> =
                    measurables.iter().map(|it| scope.measure(*it, constraints)).collect();
                scope.layout_with(100, 100, move |placement| {
                    let places = if placement.is_looking_ahead() { a.get() } else { l.get() };
                    if places {
                        for (at, placeable) in placeables.iter().enumerate() {
                            placement.place(placeable, 0, at as i32 * 10);
                        }
                    }
                })
            });
            compose_ui::Layout(
                Modifier.required_height(dp(60)).fill_max_width(),
                policy,
                callback(move || {
                    for _ in 0..3 {
                        Box!(modifier = Modifier.required_size(dp(20)), {
                            Column!(modifier = Modifier.required_size(dp(20)), |_| {
                                Box!(modifier = Modifier.size(dp(10)), {});
                                if t.get() {
                                    Box!(modifier = Modifier.size(dp(10)), {});
                                }
                            });
                        });
                    }
                }),
            );
        });
    });
    ui.wait_for_idle();
    ahead.set(false);
    ui.wait_for_idle();
    tall.set(true);
    ui.wait_for_idle();
    ahead.set(true);
    ui.wait_for_idle();
    last_now.set(true);
    ui.wait_for_idle();
}

type Count = Rc<Cell<u32>>;

/// The tests' `defaultMeasurePolicy`: each child measured and placed at the origin, its size the
/// largest of them.
fn placing_all() -> Rc<dyn compose_ui::MeasurePolicy> {
    compose_ui::measure_policy(|scope, measurables, constraints| {
        let placeables: Vec<_> =
            measurables.iter().map(|it| scope.measure(*it, constraints)).collect();
        let width = placeables.iter().map(|it| it.width).max().unwrap_or(0);
        let height = placeables.iter().map(|it| it.height).max().unwrap_or(0);
        scope.layout_with(width, height, move |placement| {
            for placeable in &placeables {
                placement.place(placeable, 0, 0);
            }
        })
    })
}

fn counting(measures: &Count, places: &Count) -> Modifier {
    let (measures, places) = (measures.clone(), places.clone());
    Modifier.layout(move |scope, measurable, constraints| {
        let placeable = scope.measure(measurable, constraints);
        measures.set(measures.get() + 1);
        let places = places.clone();
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            placement.place(&placeable, 0, 0);
            places.set(places.get() + 1);
        })
    })
}

fn counting_approach(measures: &Count, places: &Count) -> Modifier {
    let (measures, places) = (measures.clone(), places.clone());
    Modifier.approach_layout(
        |_| true,
        |_, _| false,
        move |scope, content, constraints| {
            let placeable = scope.measure(content, constraints);
            measures.set(measures.get() + 1);
            let places = places.clone();
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
                places.set(places.get() + 1);
            })
        },
    )
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, noExtraLookaheadTest
#[test]
fn a_scope_looks_ahead_no_more_than_a_tree_without_one_measures() {
    let (small, control) = (mutable_state_of(true), mutable_state_of(true));
    let counts: Vec<Count> = (0..10).map(|_| Rc::default()).collect();
    let (s, c, k) = (small.handle(), control.handle(), counts.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let k = k.clone();
        let side = if s.get() { 100 } else { 200 };
        if c.get() {
            let k2 = k.clone();
            Box!(modifier = counting(&k[0], &k[1]), {
                let k = k2.clone();
                compose_ui::Layout(
                    Modifier,
                    placing_all(),
                    callback(move || {
                        Box!(modifier = Modifier.size(dp(side)).then(counting(&k[2], &k[3])), {});
                    }),
                );
            });
        } else {
            let k2 = k.clone();
            Box!(modifier = counting(&k[4], &k[5]), {
                let k = k2.clone();
                in_a_scope(move |_| {
                    let after = counting(&k[6], &k[7]).then(counting_approach(&k[8], &k[9]));
                    AnimatingSize(Modifier.size(dp(side)), after);
                });
            });
        }
    });
    ui.wait_for_idle();
    let get = |at: usize| counts[at].get();
    assert_eq!((get(0), get(1), get(2), get(3)), (1, 1, 1, 1));
    small.set(false);
    ui.wait_for_idle();
    assert_eq!((get(4), get(5), get(6), get(7), get(8), get(9)), (0, 0, 0, 0, 0, 0));
    control.set(false);
    ui.wait_for_idle();
    // The scope's: one measurement and one placing ahead, the rest now.
    assert_eq!((get(4), get(5)), (1, 1));
    assert_eq!((get(6) - get(8), get(7) - get(9)), (1, 1));
    ui.set_auto_advance(false);
    small.set(true);
    for _ in 0..10 {
        ui.advance_time_by_frame();
    }
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    let (lookahead_measures, lookahead_places) = (get(6) - get(8), get(7) - get(9));
    assert_eq!((get(4), get(5)), (get(0), get(1)));
    assert_eq!((lookahead_measures, lookahead_places), (get(2), get(3)));
    assert!(lookahead_measures < get(8) && lookahead_places < get(9));
}

/// Each pass's measurements and placings of what `noting` is on.
#[derive(Clone, Default)]
struct Stats(Rc<Cell<[u32; 4]>>);

impl Stats {
    fn noting(&self) -> Modifier {
        let stats = self.0.clone();
        let bump = move |stats: &Rc<Cell<[u32; 4]>>, at: usize| {
            let mut all = stats.get();
            all[at] += 1;
            stats.set(all);
        };
        Modifier.layout(move |scope, measurable, constraints| {
            bump(&stats, if scope.is_looking_ahead() { 0 } else { 1 });
            let placeable = scope.measure(measurable, constraints);
            let stats = stats.clone();
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                bump(&stats, if placement.is_looking_ahead() { 2 } else { 3 });
                placement.place(&placeable, 0, 0);
            })
        })
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, testReorderChildrenInLookaheadScope
#[test]
fn children_reordered_in_a_scope_are_measured_and_placed_again_in_both_passes() {
    let list = mutable_state_of(vec![1, 2]);
    let (boxed, row) = (Stats::default(), Stats::default());
    let (l, b, r) = (list.handle(), boxed.clone(), row.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (b, r) = (b.clone(), r.clone());
        Row!(|_| {
            let (b, r) = (b.clone(), r.clone());
            in_a_scope(move |_| {
                for it in l.get() {
                    let (b, r) = (b.clone(), r.clone());
                    compose_runtime::key(it, move || {
                        if it == 1 {
                            Box!(modifier = Modifier.size(dp(100)).then(b.noting()), {});
                        } else {
                            Row!(modifier = Modifier.size(dp(200)).then(r.noting()), |_| {});
                        }
                    });
                }
            });
        });
    });
    ui.wait_for_idle();
    assert_eq!((boxed.0.get(), row.0.get()), ([1, 1, 1, 1], [1, 1, 1, 1]));
    list.set(vec![2, 1]);
    ui.wait_for_idle();
    assert_eq!((boxed.0.get(), row.0.get()), ([2, 2, 2, 2], [2, 2, 2, 2]));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, testLookaheadAndApproachCoordinatesAreConsistentOnFirstPass_usingOffset
#[test]
fn both_passes_see_the_same_lookahead_position_at_once() {
    type Seen = Rc<Cell<Option<compose_ui::Offset>>>;
    let (ahead, now): (Seen, Seen) = (Rc::default(), Rc::default());
    let (a, n) = (ahead.clone(), now.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (a, n) = (a.clone(), n.clone());
        Box!(modifier = Modifier.size(dp(300)), {
            let (a, n) = (a.clone(), n.clone());
            in_a_scope(move |lookahead| {
                let (a, n) = (a.clone(), n.clone());
                let moved =
                    Modifier.offset_with(|_| compose_ui::IntOffset::new(0, 100)).fill_max_width();
                Box!(modifier = moved, {
                    let (a, n) = (a.clone(), n.clone());
                    let on_lookahead = Modifier.layout(move |scope, measurable, constraints| {
                        let placeable = scope.measure(measurable, constraints);
                        let a = a.clone();
                        scope.layout_with(placeable.width, placeable.height, move |placement| {
                            if placement.is_looking_ahead()
                                && let (Some(frame), Some(own)) = (
                                    lookahead.lookahead_scope_coordinates(placement),
                                    placement.coordinates(),
                                )
                            {
                                a.set(Some(
                                    frame.local_position_of(&own, compose_ui::Offset::ZERO),
                                ));
                            }
                            placement.place(&placeable, 0, 0);
                        })
                    });
                    let on_approach = Modifier.approach_layout(
                        |_| false,
                        move |placement, coordinates| {
                            if let Some(frame) = lookahead.lookahead_scope_coordinates(placement) {
                                n.set(Some(
                                    lookahead
                                        .to_lookahead_coordinates(frame)
                                        .local_position_of(coordinates, compose_ui::Offset::ZERO),
                                ));
                            }
                            false
                        },
                        |scope, content, constraints| {
                            let placeable = scope.measure(content, constraints);
                            scope.layout_with(placeable.width, placeable.height, move |placement| {
                                placement.place(&placeable, 0, 0);
                            })
                        },
                    );
                    Box!(modifier = on_lookahead.then(on_approach), {});
                });
            });
        });
    });
    ui.wait_for_idle();
    let expected = Some(compose_ui::Offset::new(0.0, 100.0));
    assert_eq!((ahead.get(), now.get()), (expected, expected));
}
