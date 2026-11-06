//! `Modifier.on_layout_rect_changed` and `register_on_layout_rect_changed`: where a node is, in
//! the root, the window and the screen, after the layouts and graphics layers that move it,
//! throttled and debounced (docs/parity/design/layout-rect-callbacks.md). Adapted from Compose's
//! `OnGlobalRectChangedTest`, at density 1 (a dp is a pixel).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Constraints, IntOffset, IntRect, IntSize, Layout, Measurable, MeasureResult, MeasureScope,
    Popup, RelativeLayoutBounds, UnitExt, measure_policy,
};

type Shared<T> = Rc<Cell<T>>;

fn shared<T: Default>() -> Shared<T> {
    Rc::default()
}

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(1000, 1000), content)
}

/// Keeps `it`'s position in the root in `into`.
fn keep_position(into: &Shared<Option<IntOffset>>) -> Modifier {
    let into = into.clone();
    Modifier.on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
        into.set(Some(bounds.position_in_root()))
    })
}

/// Counts the calls in `count`.
fn count_calls(count: &Shared<u32>) -> Modifier {
    let count = count.clone();
    Modifier.on_layout_rect_changed(0, 0, move |_: RelativeLayoutBounds| count.set(count.get() + 1))
}

/// Compose's test `Wrap`: as large as its largest child, at least `min_width` by `min_height`,
/// its children at its top left.
#[allow(non_snake_case)]
fn Wrap(min_width: i32, min_height: i32, modifier: Modifier, content: impl Fn() + 'static) {
    Layout!(
        modifier = modifier,
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let placeables: Vec<_> =
                measurables.iter().map(|it| scope.measure(*it, constraints)).collect();
            let width = placeables.iter().map(|it| it.width).fold(min_width, i32::max);
            let height = placeables.iter().map(|it| it.height).fold(min_height, i32::max);
            for placeable in &placeables {
                scope.place(placeable, 0, 0);
            }
            scope.layout(width, height)
        }),
        { content() }
    );
}

/// Measures its first child with the constraints and places it at `at()`'s offset, in a
/// placement block (so a change of `at` places it again, measuring nothing), filling the
/// constraints' maximum.
fn placing_first_at(
    at: impl Fn() -> Option<(i32, i32)> + 'static,
) -> std::rc::Rc<dyn compose_ui::MeasurePolicy> {
    let at = Rc::new(at);
    measure_policy(
        move |scope: &mut MeasureScope<'_>,
              measurables: &[Measurable],
              constraints: Constraints|
              -> MeasureResult {
            let placeable = scope.measure(measurables[0], constraints);
            let at = at.clone();
            scope.layout_with(constraints.max_width, constraints.max_height, move |placement| {
                if let Some((x, y)) = at() {
                    placement.place(&placeable, x, y);
                }
            })
        },
    )
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionInRootWhenMovingBothGrandParentAndNodeItself
#[test]
fn the_position_in_root_adds_up_a_grandparent_s_and_the_node_s_moves() {
    let position = shared();
    let offset = mutable_state_of(0);
    let (p, o) = (position.clone(), offset.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        Layout!(measure_policy = placing_first_at(move || Some((o.get(), o.get()))), {
            let p = p.clone();
            Box!(modifier = Modifier.size(10.dp()), {
                let p = p.clone();
                Box!({
                    let p = p.clone();
                    Layout!(measure_policy = placing_first_at(move || Some((o.get(), o.get()))), {
                        Box!(modifier = keep_position(&p));
                    });
                });
            });
        });
    });
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    offset.set(10);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(20, 20)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionInRootWhenMovingGrandParentWithLayerAndNode
#[test]
fn the_position_in_root_adds_up_a_grandparent_s_layer_and_the_node_s_move() {
    let position = shared();
    let offset = mutable_state_of(0);
    let (p, o) = (position.clone(), offset.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        Layout!(
            modifier = Modifier.graphics_layer(move |layer| {
                layer.translation_x = o.get() as f32;
                layer.translation_y = o.get() as f32;
            }),
            measure_policy = placing_first_at(|| Some((0, 0))),
            {
                let p = p.clone();
                Box!(modifier = Modifier.size(10.dp()), {
                    let p = p.clone();
                    Box!({
                        let p = p.clone();
                        Layout!(
                            measure_policy = placing_first_at(move || Some((o.get(), o.get()))),
                            {
                                Box!(modifier = keep_position(&p));
                            }
                        );
                    });
                });
            }
        );
    });
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    offset.set(10);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(20, 20)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionInRootWhenMovingBothGrandParentWithLayoutModifierAndNode
#[test]
fn the_position_in_root_adds_up_a_grandparent_s_layout_modifier_and_the_node_s_move() {
    let position = shared();
    let offset = mutable_state_of(0);
    let (p, o) = (position.clone(), offset.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        let moving = Modifier.layout(move |scope, measurable, constraints| {
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(constraints.max_width, constraints.max_height, move |placement| {
                placement.place(&placeable, o.get(), o.get());
            })
        });
        Layout!(modifier = moving, measure_policy = placing_first_at(|| Some((0, 0))), {
            let p = p.clone();
            Box!(modifier = Modifier.size(10.dp()), {
                let p = p.clone();
                Box!({
                    let p = p.clone();
                    Layout!(measure_policy = placing_first_at(move || Some((o.get(), o.get()))), {
                        Box!(modifier = keep_position(&p));
                    });
                });
            });
        });
    });
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    offset.set(10);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(20, 20)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionInRootWhenUnplacingThenPlacingGrandparentWithDifferentOffsetAndNode
#[test]
fn an_unplaced_grandparent_s_move_is_told_once_it_s_placed_again() {
    let position = shared();
    let offset = mutable_state_of(0);
    let should_place = mutable_state_of(true);
    let (p, o, s) = (position.clone(), offset.handle(), should_place.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        Layout!(measure_policy = placing_first_at(move || s.get().then(|| (o.get(), o.get()))), {
            let p = p.clone();
            Box!(modifier = Modifier.size(10.dp()), {
                let p = p.clone();
                Box!({
                    let p = p.clone();
                    Layout!(measure_policy = placing_first_at(move || Some((o.get(), o.get()))), {
                        Box!(modifier = keep_position(&p));
                    });
                });
            });
        });
    });
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    should_place.set(false);
    offset.set(10);
    position.set(None);
    ui.wait_for_idle();
    assert_eq!(position.get(), None);
    should_place.set(true);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(20, 20)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, handlesChildrenNodeMoveCorrectly
#[test]
fn nodes_moved_between_places_are_told_their_new_bounds() {
    let index = mutable_state_of(0);
    let (wrap1, wrap2) = (shared::<i32>(), shared::<i32>());
    let (i, w1, w2) = (index.handle(), wrap1.clone(), wrap2.clone());
    let mut ui = rule(move || {
        let (w1, w2) = (w1.clone(), w2.clone());
        Row!(|_| {
            for position in 0..2 {
                let into = if i.get() == position { w1.clone() } else { w2.clone() };
                Wrap(
                    50,
                    50,
                    Modifier.on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
                        into.set(bounds.bounds_in_window().left)
                    }),
                    || {},
                );
            }
        });
    });
    assert_eq!((wrap1.get(), wrap2.get()), (0, 50));
    index.set(1);
    ui.wait_for_idle();
    assert_eq!((wrap1.get(), wrap2.get()), (50, 0));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbacksAreCalledWhenChildResized
#[test]
fn a_resize_is_told() {
    let size = mutable_state_of(10);
    let told = shared();
    let (s, t) = (size.handle(), told.clone());
    let mut ui = rule(move || {
        let t = t.clone();
        // Compose's `AtLeastSize(20)`.
        Wrap(20, 20, Modifier, move || {
            let t = t.clone();
            Wrap(
                s.get(),
                s.get(),
                Modifier.on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
                    t.set(bounds.bounds_in_root().size().width)
                }),
                || {},
            );
        });
    });
    assert_eq!(told.get(), 10);
    size.set(15);
    ui.wait_for_idle();
    assert_eq!(told.get(), 15);
}

/// A child of a `Wrap` in a layout placing it at `position`, keeping where the child is.
fn moved_child_content(
    position: &OwnedState<i32>,
    throttle: i64,
    debounce: i64,
    told: &Shared<Option<IntOffset>>,
    calls: &Shared<u32>,
) -> impl Fn() + 'static {
    let (p, t, c) = (position.handle(), told.clone(), calls.clone());
    move || {
        let (t, c) = (t.clone(), c.clone());
        Layout!(
            measure_policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                scope.layout_with(10, 10, move |placement| placement.place(&placeable, p.get(), 0))
            }),
            {
                let (t, c) = (t.clone(), c.clone());
                Wrap(10, 10, Modifier, move || {
                    let (t, c) = (t.clone(), c.clone());
                    Wrap(
                        10,
                        10,
                        Modifier.on_layout_rect_changed(
                            throttle,
                            debounce,
                            move |bounds: RelativeLayoutBounds| {
                                t.set(Some(bounds.bounds_in_root().top_left()));
                                c.set(c.get() + 1);
                            },
                        ),
                        || {},
                    );
                });
            }
        );
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackCalledForChildWhenParentMoved
#[test]
fn a_child_is_told_its_parent_moved() {
    let position = mutable_state_of(0);
    let (told, calls) = (shared(), shared());
    let mut ui = rule(moved_child_content(&position, 0, 0, &told, &calls));
    assert_eq!(calls.get(), 1);
    position.set(10);
    ui.wait_until(|_| calls.get() == 2);
    assert_eq!(told.get(), Some(IntOffset::new(10, 0)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackCalledForChildWhenParentMoved_defaultDebounce
#[test]
fn a_child_is_told_its_parent_moved_after_the_default_debounce() {
    let position = mutable_state_of(0);
    let (told, calls) = (shared(), shared());
    let mut ui = rule(moved_child_content(
        &position,
        0,
        compose_ui::DEFAULT_LAYOUT_RECT_DEBOUNCE_MILLIS,
        &told,
        &calls,
    ));
    ui.wait_until(|_| calls.get() == 1);
    position.set(10);
    ui.wait_for_idle();
    // Not before the debounce.
    assert_eq!(calls.get(), 1);
    ui.wait_until(|_| calls.get() == 2);
    assert_eq!(told.get(), Some(IntOffset::new(10, 0)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackCalledForChildWhenParentMoved_largerDebounce
#[test]
fn a_child_is_told_its_parent_moved_after_a_larger_debounce() {
    let position = mutable_state_of(0);
    let (told, calls) = (shared(), shared());
    let mut ui = rule(moved_child_content(&position, 0, 150, &told, &calls));
    ui.wait_until(|_| calls.get() == 1);
    let start = ui.current_time_millis();
    position.set(10);
    ui.wait_until(|_| calls.get() == 2);
    assert!(ui.current_time_millis() - start >= 150);
    assert_eq!(told.get(), Some(IntOffset::new(10, 0)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackCalledForChildWhenParentMoved_1500children
#[test]
fn the_last_of_1500_children_is_told_its_parent_moved() {
    let position = mutable_state_of(0);
    let told = shared();
    let (p, t) = (position.handle(), told.clone());
    let mut ui = rule(move || {
        let t = t.clone();
        Layout!(
            measure_policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                scope.layout_with(10, 10, move |placement| placement.place(&placeable, p.get(), 0))
            }),
            {
                let t = t.clone();
                Wrap(10, 10, Modifier, move || {
                    for index in 0..1500 {
                        let modifier = if index == 1499 { keep_position(&t) } else { Modifier };
                        Wrap(10, 10, modifier, || {});
                    }
                });
            }
        );
    });
    position.set(10);
    ui.wait_for_idle();
    assert_eq!(told.get(), Some(IntOffset::new(10, 0)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbacksAreCalledOnlyForPositionedChildren
#[test]
fn only_placed_children_are_told() {
    let (wrap1, wrap2, inner) = (shared(), shared(), shared());
    let (w1, w2, i) = (wrap1.clone(), wrap2.clone(), inner.clone());
    let _ui = rule(move || {
        let (w1, w2, i) = (w1.clone(), w2.clone(), i.clone());
        Layout!(
            measure_policy = measure_policy(|scope, measurables, constraints| {
                let placeable = scope.measure(measurables[1], constraints);
                scope.place(&placeable, 0, 0);
                scope.layout(10, 10)
            }),
            {
                Wrap(10, 10, count_calls(&w1), || {});
                let i = i.clone();
                Wrap(10, 10, count_calls(&w2), move || Wrap(10, 10, count_calls(&i), || {}));
            }
        );
    });
    assert_eq!((wrap1.get(), wrap2.get(), inner.get()), (0, 1, 1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, globalPositionedModifierUpdateDoesNotInvalidateLayout
#[test]
fn a_new_callback_is_called_without_a_layout() {
    let (lambda1, lambda2) = (shared::<bool>(), shared::<bool>());
    let (layouts, placements) = (shared::<bool>(), shared::<bool>());
    let change = mutable_state_of(true);
    let callback1: Callback<dyn Fn(RelativeLayoutBounds)> = {
        let lambda1 = lambda1.clone();
        callback(move |_: RelativeLayoutBounds| lambda1.set(true))
    };
    let callback2: Callback<dyn Fn(RelativeLayoutBounds)> = {
        let lambda2 = lambda2.clone();
        callback(move |_: RelativeLayoutBounds| lambda2.set(true))
    };
    let (l, p) = (layouts.clone(), placements.clone());
    let layout_modifier = Modifier.layout(move |scope, measurable, constraints| {
        l.set(true);
        let placeable = scope.measure(measurable, constraints);
        let p = p.clone();
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            p.set(true);
            placement.place(&placeable, 0, 0);
        })
    });
    let c = change.handle();
    let mut ui = rule(move || {
        let callback = if c.get() { callback1.clone() } else { callback2.clone() };
        Box!(
            modifier = layout_modifier.clone().size(10.dp()).on_layout_rect_changed(0, 0, callback)
        );
    });
    assert!(lambda1.get() && layouts.get() && placements.get() && !lambda2.get());
    for flag in [&lambda1, &lambda2, &layouts, &placements] {
        flag.set(false);
    }
    change.set(false);
    ui.wait_for_idle();
    assert!(!lambda1.get() && !layouts.get() && !placements.get());
    assert!(lambda2.get(), "the new lambda is called when it's updated");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, columnCenteringHasCorrectPosition
#[test]
fn a_centered_child_is_where_on_globally_positioned_says() {
    let (from_rect, from_positioned) = (shared(), shared());
    let (r, g) = (from_rect.clone(), from_positioned.clone());
    let _ui = rule(move || {
        let (r, g) = (r.clone(), g.clone());
        Column!(
            modifier = Modifier.size(200.dp()),
            horizontal_alignment = compose_ui::Alignment::CENTER_HORIZONTALLY,
            |_| {
                let g = g.clone();
                Box!(
                    modifier = Modifier
                        .padding(0.dp())
                        .then(keep_position(&r))
                        .on_globally_positioned(
                            move |coordinates: compose_ui::LayoutCoordinates| {
                                let at = coordinates.position_in_root();
                                g.set(Some(IntOffset::new(
                                    at.x.round() as i32,
                                    at.y.round() as i32,
                                )));
                            }
                        )
                        .size(100.dp())
                );
            }
        );
    });
    assert!(from_rect.get().is_some());
    assert_eq!(from_rect.get(), from_positioned.get());
    assert_eq!(from_rect.get(), Some(IntOffset::new(50, 0)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackIsReExecutedOnReuse
#[test]
fn a_reused_node_is_told_again() {
    let size = shared::<i32>();
    let reuse_key = mutable_state_of(0);
    let (s, k) = (size.clone(), reuse_key.handle());
    let mut ui = rule(move || {
        let s = s.clone();
        ReusableContent(k.get(), || {
            let s = s.clone();
            Box!(
                modifier = Modifier.required_size(10.dp()).on_layout_rect_changed(
                    0,
                    0,
                    move |bounds: RelativeLayoutBounds| s.set(bounds.width())
                )
            );
        });
    });
    size.set(i32::MAX);
    reuse_key.set(1);
    ui.wait_for_idle();
    assert_eq!(size.get(), 10);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackIsReexecutedOnPlacingAgainWithTheSameOffset
#[test]
fn placing_again_at_the_same_offset_is_told() {
    let calls = shared();
    let should_place = mutable_state_of(true);
    let (c, s) = (calls.clone(), should_place.handle());
    let mut ui = rule(move || {
        let c = c.clone();
        Layout!(
            measure_policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                scope.layout_with(placeable.width, placeable.height, move |placement| {
                    if s.get() {
                        placement.place(&placeable, 0, 0);
                    }
                })
            }),
            {
                Box!(modifier = count_calls(&c).size(100.dp()));
            }
        );
    });
    assert_eq!(calls.get(), 1);
    should_place.set(false);
    ui.wait_for_idle();
    assert_eq!(calls.get(), 1);
    should_place.set(true);
    ui.wait_for_idle();
    assert_eq!(calls.get(), 2);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackIsReexecutedWithCorrectCoordinatesWhenParentMovesWhileNotPlaced
#[test]
fn a_move_while_unplaced_is_told_once_placed() {
    let position = shared();
    let should_place = mutable_state_of(true);
    let offset = mutable_state_of(0);
    let (p, s, o) = (position.clone(), should_place.handle(), offset.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        Layout!(
            measure_policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                scope.layout_with(placeable.width, placeable.height, move |placement| {
                    placement.place(&placeable, o.get(), 0)
                })
            }),
            {
                let p = p.clone();
                Layout!(
                    measure_policy = measure_policy(move |scope, measurables, constraints| {
                        let placeable = scope.measure(measurables[0], constraints);
                        scope.layout_with(placeable.width, placeable.height, move |placement| {
                            if s.get() {
                                placement.place(&placeable, 0, 0);
                            }
                        })
                    }),
                    {
                        Box!(modifier = keep_position(&p).size(100.dp()));
                    }
                );
            }
        );
    });
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    position.set(None);
    should_place.set(false);
    offset.set(5);
    ui.wait_for_idle();
    assert_eq!(position.get(), None);
    should_place.set(true);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(5, 0)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, changingLayerPropertyWhileNotPlacedNotCausingCallback
#[test]
fn a_layer_s_change_while_unplaced_is_told_once_placed() {
    let position = shared();
    let should_place = mutable_state_of(true);
    let offset = mutable_state_of(0.0f32);
    let (p, s, o) = (position.clone(), should_place.handle(), offset.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        Layout!(
            measure_policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                scope.layout_with(placeable.width, placeable.height, move |placement| {
                    if s.get() {
                        placement.place(&placeable, 0, 0);
                    }
                })
            }),
            {
                Box!(
                    modifier = Modifier
                        .graphics_layer(move |layer| layer.translation_x = o.get())
                        .then(keep_position(&p))
                        .size(100.dp())
                );
            }
        );
    });
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    position.set(None);
    should_place.set(false);
    ui.wait_for_idle();
    offset.set(5.0);
    ui.wait_for_idle();
    assert_eq!(position.get(), None);
    should_place.set(true);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(5, 0)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbackIsCalledWhenAddedLater
#[test]
fn a_callback_added_later_is_called() {
    let size = shared::<i32>();
    let dynamic = mutable_state_of(false);
    let (s, d) = (size.clone(), dynamic.handle());
    let mut ui = rule(move || {
        let s = s.clone();
        let added = if d.get() {
            Modifier.on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
                s.set(bounds.width())
            })
        } else {
            Modifier
        };
        Box!(modifier = Modifier.required_size(10.dp()).then(added));
    });
    dynamic.set(true);
    ui.wait_for_idle();
    assert_eq!(size.get(), 10);
}

/// `once` panics if it's called twice before `reset`.
fn once(called: &Shared<bool>) -> Callback<dyn Fn(RelativeLayoutBounds)> {
    let called = called.clone();
    callback(move |_: RelativeLayoutBounds| {
        assert!(!called.get(), "called twice");
        called.set(true);
    })
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbacksAreCalledOnlyOnceWhenLambdaChangesAndLayoutChanges
#[test]
fn a_new_lambda_and_a_new_size_call_the_new_lambda_once() {
    let (called1, called2) = (shared(), shared());
    let (lambda1, lambda2) = (once(&called1), once(&called2));
    let change = mutable_state_of(true);
    let size = mutable_state_of(100.0f32);
    let (c, s) = (change.handle(), size.handle());
    let mut ui = rule(move || {
        let callback = if c.get() { lambda1.clone() } else { lambda2.clone() };
        Box!(modifier = Modifier.size(s.get().dp()).on_layout_rect_changed(0, 0, callback));
    });
    assert!(called1.get() && !called2.get());
    called1.set(false);
    size.set(120.0);
    change.set(false);
    ui.wait_for_idle();
    assert!(called2.get() && !called1.get());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, callbacksAreCalledOnlyOnceWhenLayoutBelowItAndLambdaChanged
#[test]
fn a_new_lambda_and_a_change_below_it_call_the_new_lambda_once() {
    let (called1, called2) = (shared(), shared());
    let (lambda1, lambda2) = (once(&called1), once(&called2));
    let change = mutable_state_of(true);
    let size = mutable_state_of(10.0f32);
    let (c, s) = (change.handle(), size.handle());
    let mut ui = rule(move || {
        let callback = if c.get() { lambda1.clone() } else { lambda2.clone() };
        Box!(
            modifier = Modifier
                .padding(10.dp())
                .on_layout_rect_changed(0, 0, callback)
                .padding(s.get().dp())
                .size(10.dp())
        );
    });
    assert!(called1.get() && !called2.get());
    called1.set(false);
    size.set(20.0);
    change.set(false);
    ui.wait_for_idle();
    assert!(called2.get() && !called1.get());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, coordinatesOfTheModifierAreReported
#[test]
fn each_callback_reports_its_place_in_the_chain() {
    let (c1, c2, c3) = (shared(), shared(), shared());
    let (a, b, c) = (c1.clone(), c2.clone(), c3.clone());
    let keep = |into: &Shared<Option<IntRect>>| {
        let into = into.clone();
        move |bounds: RelativeLayoutBounds| into.set(Some(bounds.bounds_in_window()))
    };
    let _ui = rule(move || {
        Box!(
            modifier = Modifier
                .fill_max_size()
                .on_layout_rect_changed(0, 0, keep(&a))
                .padding(2.dp())
                .on_layout_rect_changed(0, 0, keep(&b))
                .padding(3.dp())
                .on_layout_rect_changed(0, 0, keep(&c))
        );
    });
    assert_eq!(c1.get().map(|it| it.left), Some(0));
    assert_eq!(c2.get().map(|it| it.left), Some(2));
    assert_eq!(c3.get().map(|it| it.left), Some(5));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, lotsOfNotifications
#[test]
fn thirty_callbacks_are_all_told() {
    let offset = mutable_state_of(IntOffset::ZERO);
    let positions = Rc::new(RefCell::new(Vec::new()));
    let (o, p) = (offset.handle(), positions.clone());
    let mut ui = rule(move || {
        let p = p.clone();
        Box!(modifier = Modifier.fill_max_size(), {
            for index in 0..30 {
                let p = p.clone();
                Box!(
                    modifier = Modifier
                        .fill_max_size()
                        .offset_with(move |_| o.get())
                        .on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
                            p.borrow_mut().push((index, bounds.bounds_in_root().top_left()))
                        })
                );
            }
        });
    });
    positions.borrow_mut().clear();
    offset.set(IntOffset::new(1, 1));
    ui.wait_for_idle();
    let positions = positions.borrow();
    assert_eq!(positions.len(), 30);
    assert!(positions.iter().all(|(_, at)| *at == IntOffset::new(1, 1)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, removingOnPositionedCallbackDoesNotTriggerOtherCallbacks
#[test]
fn removing_a_callback_calls_none_of_the_others() {
    let present = mutable_state_of(true);
    let (calls1, calls2) = (shared(), shared());
    let first = {
        let calls1 = calls1.clone();
        callback(move |_: RelativeLayoutBounds| calls1.set(calls1.get() + 1))
    };
    let second = {
        let calls2 = calls2.clone();
        callback(move |_: RelativeLayoutBounds| calls2.set(calls2.get() + 1))
    };
    let p = present.handle();
    let mut ui = rule(move || {
        let modifier =
            if p.get() { Modifier.on_layout_rect_changed(0, 0, first.clone()) } else { Modifier };
        Box!(
            modifier = Modifier
                .on_layout_rect_changed(0, 0, second.clone())
                .then(modifier)
                .fill_max_size()
        );
    });
    assert_eq!((calls1.get(), calls2.get()), (1, 1));
    present.set(false);
    ui.wait_for_idle();
    assert_eq!((calls1.get(), calls2.get()), (1, 1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, rectChangedBoundsInModifierChain
#[test]
fn bounds_follow_the_chain_s_padding_and_offset() {
    let bounds: Vec<Shared<IntRect>> = (0..5).map(|_| shared()).collect();
    let keep = |into: &Shared<IntRect>| {
        let into = into.clone();
        Modifier.on_layout_rect_changed(0, 0, move |it: RelativeLayoutBounds| {
            into.set(it.bounds_in_root())
        })
    };
    let b = bounds.clone();
    let _ui = rule(move || {
        let b = b.clone();
        Column!(|_| {
            let (b, last) = (b.clone(), b[4].clone());
            Box!(
                modifier = Modifier
                    .size(100.dp())
                    .then(keep(&b[0]))
                    .padding(10.dp())
                    .then(keep(&b[1]))
                    .offset(130.dp(), 130.dp())
                    .then(keep(&b[2])),
                {
                    Box!(modifier = Modifier.size(60.dp()).then(keep(&b[3])));
                }
            );
            Box!(modifier = Modifier.size(55.dp()).then(keep(&last)));
        });
    });
    let rect = |at: (i32, i32), size: i32| {
        IntRect::from_offset_size(IntOffset::new(at.0, at.1), IntSize::new(size, size))
    };
    assert_eq!(bounds[0].get(), rect((0, 0), 100));
    assert_eq!(bounds[1].get(), rect((10, 10), 80));
    assert_eq!(bounds[2].get(), rect((140, 140), 80));
    assert_eq!(bounds[3].get(), rect((140, 140), 60));
    assert_eq!(bounds[4].get(), rect((0, 100), 55));
}

/// Registers once, and unregisters as it's first called, as the test's
/// `calledOnceOnLayoutRectChanged`.
fn called_once(count: &Shared<u32>) -> Modifier {
    use compose_ui::{ModifierNode, ModifierNodeElement, NodeContext, RegistrationHandle};
    struct Element(Shared<u32>);
    struct Node {
        count: Shared<u32>,
        handle: Rc<RefCell<Option<RegistrationHandle>>>,
    }
    impl ModifierNodeElement for Element {
        type Node = Node;
        fn create(&self) -> Node {
            Node { count: self.0.clone(), handle: Rc::default() }
        }
        fn update(&self, node: &mut Node) {
            node.count = self.0.clone();
        }
        fn same(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }
    impl ModifierNode for Node {
        fn on_attach(&mut self, context: &mut NodeContext<'_>) {
            let (count, handle) = (self.count.clone(), self.handle.clone());
            let registered =
                context.register_on_layout_rect_changed(0, 0, move |_: RelativeLayoutBounds| {
                    if let Some(handle) = handle.borrow_mut().take() {
                        handle.unregister();
                    }
                    count.set(count.get() + 1);
                });
            *self.handle.borrow_mut() = Some(registered);
        }
        fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
            if let Some(handle) = self.handle.borrow_mut().take() {
                handle.unregister();
            }
        }
    }
    Modifier.then_custom(Element(count.clone()))
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, havingTwoCallbacks_removingFirstDuringDispatching
#[test]
fn a_callback_unregistering_itself_first_leaves_the_next_called() {
    let (regular, once) = (shared(), shared());
    let callbacks = mutable_state_of(false);
    let (r, o, c) = (regular.clone(), once.clone(), callbacks.handle());
    let mut ui = rule(move || {
        let added = if c.get() { called_once(&o).then(count_calls(&r)) } else { Modifier };
        Box!(modifier = Modifier.required_size(10.dp()).then(added));
    });
    callbacks.set(true);
    ui.wait_for_idle();
    assert_eq!((regular.get(), once.get()), (1, 1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, havingTwoCallbacks_removingMiddleOneDuringDispatching
#[test]
fn a_callback_unregistering_itself_in_the_middle_leaves_the_others_called() {
    let (regular, regular2, once) = (shared(), shared(), shared());
    let callbacks = mutable_state_of(false);
    let (r, r2, o, c) = (regular.clone(), regular2.clone(), once.clone(), callbacks.handle());
    let mut ui = rule(move || {
        let added = if c.get() {
            count_calls(&r).then(called_once(&o)).then(count_calls(&r2))
        } else {
            Modifier
        };
        Box!(modifier = Modifier.required_size(10.dp()).then(added));
    });
    callbacks.set(true);
    ui.wait_for_idle();
    assert_eq!((regular.get(), regular2.get(), once.get()), (1, 1, 1));
}

/// A throttled callback is called at once, then not again within its throttle, then with where
/// the node settled once it rested for the throttle's time (debounce 0 takes it).
// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/layout/OnLayoutRectChangedModifier.kt, onLayoutRectChanged
#[test]
fn a_throttled_callback_hears_where_the_node_settled() {
    let position = mutable_state_of(0);
    let (told, calls) = (shared(), shared());
    let mut ui = ComposeTestRule::empty(compose_ui::platform::UiHost::headless(1000, 1000));
    ui.set_content(moved_child_content(&position, 100, 0, &told, &calls));
    assert_eq!(calls.get(), 1);
    position.set(10);
    ui.wait_for_idle();
    position.set(20);
    ui.wait_for_idle();
    // Within the throttle: not yet.
    assert_eq!(calls.get(), 1);
    ui.advance_time_by(Duration::from_millis(200));
    assert_eq!(calls.get(), 2);
    assert_eq!(told.get(), Some(IntOffset::new(20, 0)));
}

/// A content-less layout of 100 by 200 keeping its bounds in the window, as the tests' `Layout({},
/// modifier) { layout(100, 200) {} }`.
fn sized_leaf(modifier: Modifier, into: &Shared<Option<IntRect>>) {
    let into = into.clone();
    Layout!(
        modifier = modifier.on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
            into.set(Some(bounds.bounds_in_window()))
        }),
        measure_policy = measure_policy(|scope, _, _| scope.layout(100, 200)),
        {}
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, onPositionedCalledWhenLayerChanged
#[test]
fn a_layer_s_move_is_told() {
    let bounds = shared();
    let offset = mutable_state_of(0.0f32);
    let (b, o) = (bounds.clone(), offset.handle());
    let mut ui = rule(move || {
        sized_leaf(Modifier.graphics_layer(move |layer| layer.translation_x = o.get()), &b)
    });
    assert!(bounds.get().is_some());
    bounds.set(None);
    offset.set(5.0);
    ui.wait_for_idle();
    assert_eq!(bounds.get().map(|it| it.left), Some(5));
}

/// The host's window moving on the screen is told, the bounds on the screen moved. Compose moves
/// the `ComposeView` in its window (a view above it gone, a scroll view scrolled); compose-rs's
/// root fills its window, which the host moves on the screen.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, onPositionedIsCalledWhenComposeContainerPositionChanged
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, onPositionedIsCalledWhenComposeContainerIsScrolled
#[test]
fn the_window_moving_on_the_screen_is_told() {
    let screen = shared();
    let s = screen.clone();
    let mut ui = rule(move || {
        let s = s.clone();
        Layout!(
            modifier =
                Modifier.on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
                    s.set(Some((bounds.bounds_in_screen(), bounds.bounds_in_window())))
                }),
            measure_policy = measure_policy(|scope, _, _| scope.layout(100, 200)),
            {}
        );
    });
    let (start, _) = screen.get().expect("told");
    screen.set(None);
    ui.host().set_window_origin(compose_ui::Offset::new(0.0, -50.0));
    ui.wait_for_idle();
    let (moved, window) = screen.get().expect("told again");
    assert_eq!(moved.top, start.top - 50);
    // In the window, it's where it was.
    assert_eq!(window, IntRect::new(0, 0, 100, 200));
}

/// A move and the window's move in one frame are one call, as Compose's `dispatchCallbacks`
/// (the window's entries called only strictly past the throttle).
// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/spatial/RectManager.kt, dispatchCallbacks
#[test]
fn a_move_and_a_window_move_in_one_frame_are_one_call() {
    let told = Rc::new(RefCell::new(Vec::new()));
    let offset = mutable_state_of(IntOffset::ZERO);
    let (t, o) = (told.clone(), offset.handle());
    let mut ui = rule(move || {
        let t = t.clone();
        Box!(
            modifier = Modifier.offset_with(move |_| o.get()).size(10.dp()).on_layout_rect_changed(
                0,
                0,
                move |bounds: RelativeLayoutBounds| t.borrow_mut().push(bounds.position_in_root())
            )
        );
    });
    told.borrow_mut().clear();
    offset.set(IntOffset::new(0, 5));
    ui.host().set_window_origin(compose_ui::Offset::new(0.0, 7.0));
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), vec![IntOffset::new(0, 5)]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, globalCoordinatesAreInActivityCoordinates
#[test]
fn window_and_screen_bounds_are_from_the_host_s_window() {
    let told = shared();
    let t = told.clone();
    let mut ui = ComposeTestRule::empty(compose_ui::platform::UiHost::headless(1000, 1000));
    ui.host().set_window_origin(compose_ui::Offset::new(30.0, 40.0));
    ui.set_content(move || {
        let t = t.clone();
        Box!(
            modifier = Modifier.fill_max_size().padding(30.dp()).on_layout_rect_changed(
                0,
                0,
                move |bounds: RelativeLayoutBounds| {
                    t.set(Some((bounds.bounds_in_window().top_left(), bounds.position_in_screen())))
                }
            )
        );
    });
    assert_eq!(told.get(), Some((IntOffset::new(30, 30), IntOffset::new(60, 70))));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, onPositionedCalledInDifferentPartsOfHierarchy
#[test]
fn callbacks_in_different_parts_of_the_tree_are_told() {
    let (first, second) = (shared(), shared());
    let size = mutable_state_of(10.0f32);
    let (f, s2, s) = (first.clone(), second.clone(), size.handle());
    let mut ui = rule(move || {
        let (f, s2) = (f.clone(), s2.clone());
        Column!(|_| {
            for into in [f.clone(), s2.clone()] {
                Box!(modifier = Modifier.required_size(25.dp()), {
                    let into = into.clone();
                    Box!(
                        modifier = Modifier.required_size(s.get().dp()).on_layout_rect_changed(
                            0,
                            0,
                            move |bounds: RelativeLayoutBounds| {
                                into.set(Some(bounds.bounds_in_root()))
                            }
                        )
                    );
                });
            }
        });
    });
    assert!(first.get().is_some() && second.get().is_some());
    first.set(None);
    second.set(None);
    size.set(15.0);
    ui.wait_for_idle();
    assert!(first.get().is_some() && second.get().is_some());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, testPositionInRootFromRelativeLayoutBoundsForItemsInLazyColumn
#[test]
fn lazy_items_are_told_where_on_globally_positioned_says() {
    use compose_foundation::lazy::*;
    let count = 20;
    let from_rect: Rc<Vec<Shared<Option<IntOffset>>>> =
        Rc::new((0..count).map(|_| shared()).collect());
    let expected: Rc<Vec<Shared<Option<IntOffset>>>> =
        Rc::new((0..count).map(|_| shared()).collect());
    let state = LazyListState::new(0, 0);
    let handle = *state;
    let (r, e) = (from_rect.clone(), expected.clone());
    let mut ui = rule(move || {
        let (r, e) = (r.clone(), e.clone());
        LazyColumn!(
            modifier = Modifier.size((200.dp(), 400.dp())),
            state = handle,
            move |scope: LazyListScope| {
                for id in 0..count {
                    let (r, e) = (r.clone(), e.clone());
                    scope.item(move || {
                        let g = e[id].clone();
                        Box!(
                            modifier = Modifier
                                .size(100.dp())
                                .then(keep_position(&r[id]))
                                .on_globally_positioned(
                                    move |coordinates: compose_ui::LayoutCoordinates| {
                                        let at = coordinates.position_in_root();
                                        g.set(Some(IntOffset::new(
                                            at.x.round() as i32,
                                            at.y.round() as i32,
                                        )));
                                    }
                                )
                                .padding(10.dp())
                        );
                    });
                }
            }
        );
    });
    let check = |ids: std::ops::Range<usize>| {
        for id in ids {
            assert!(expected[id].get().is_some(), "item {id} positioned");
            assert_eq!(expected[id].get(), from_rect[id].get(), "item {id}");
        }
    };
    check(0..4);
    // Half an item: the items in view move.
    let _ = crate::scroll_support::run(&mut ui, async move { handle.scroll_by(50.0).await });
    assert_eq!(state.first_visible_item_index(), 0);
    check(0..5);
    let _ = crate::scroll_support::run(&mut ui, async move { handle.scroll_to_item(10, 20).await });
    assert_eq!(state.first_visible_item_index(), 10);
    check(10..14);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, testRepositionTriggersCallback
#[test]
fn a_new_padding_is_told() {
    let left = mutable_state_of(30.0f32);
    let told = shared();
    let (l, t) = (left.handle(), told.clone());
    let mut ui = rule(move || {
        let t = t.clone();
        Box!({
            let t = t.clone();
            Box!(
                modifier = Modifier
                    .fill_max_size()
                    .padding(PaddingValues::new(l.get().dp(), 0.dp(), 0.dp(), 0.dp()))
                    .on_layout_rect_changed(0, 0, move |bounds: RelativeLayoutBounds| {
                        t.set(Some(bounds.bounds_in_root().left))
                    })
            );
        });
    });
    left.set(40.0);
    ui.wait_for_idle();
    assert_eq!(told.get(), Some(40));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, testGrandParentRepositionTriggersChildrenCallback
#[test]
fn a_grandparent_s_move_is_told_to_the_children() {
    let left = mutable_state_of(20);
    let told = shared();
    let (l, t) = (left.handle(), told.clone());
    let mut ui = rule(move || {
        let t = t.clone();
        Box!({
            let t = t.clone();
            // Compose's test `Offset(left)`.
            Layout!(measure_policy = placing_first_at(move || Some((l.get(), 0))), {
                let t = t.clone();
                Box!(modifier = Modifier.required_size(10.dp()), {
                    let t = t.clone();
                    Box!(modifier = Modifier.required_size(10.dp()), {
                        let t = t.clone();
                        Box!(
                            modifier = Modifier
                                .on_layout_rect_changed(
                                    0,
                                    0,
                                    move |bounds: RelativeLayoutBounds| {
                                        t.set(Some(bounds.bounds_in_root().left))
                                    }
                                )
                                .required_size(10.dp())
                        );
                    });
                });
            });
        });
    });
    assert_eq!(told.get(), Some(20));
    left.set(40);
    ui.wait_until(|_| told.get() == Some(40));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, offsetChangeDuringForceRemeasureTriggersCallbacks
#[test]
fn a_forced_remeasure_moving_the_node_is_told() {
    let position = shared();
    let remeasurement: Rc<RefCell<Option<compose_ui::Remeasurement>>> = Rc::default();
    let offset = Rc::new(Cell::new(IntOffset::ZERO));
    let (p, r, o) = (position.clone(), remeasurement.clone(), offset.clone());
    let keeper: Rc<dyn compose_ui::RemeasurementModifier> =
        Rc::new(move |it: compose_ui::Remeasurement| *r.borrow_mut() = Some(it));
    let mut ui = rule(move || {
        let o = o.clone();
        Box!(
            modifier = Modifier
                .remeasurement_modifier(keeper.clone())
                .offset_with(move |_| o.get())
                .then(keep_position(&p))
                .size(100.dp())
        );
    });
    offset.set(IntOffset::new(10, 10));
    remeasurement.borrow().as_ref().expect("given").force_remeasure();
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(10, 10)));
}

/// Compose's test `FixedSize(size, modifier)`: a layout of `size` by `size`.
fn fixed_size(size: i32, modifier: Modifier) {
    Layout!(
        modifier = modifier,
        measure_policy = measure_policy(move |scope, _, _| scope.layout(size, size)),
        {}
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, testDrawOnlyUpdates
#[test]
fn a_throttle_of_1_calls_at_once_whatever_the_debounce() {
    let rect = shared();
    let offset = mutable_state_of(IntOffset::ZERO);
    let (r, o) = (rect.clone(), offset.handle());
    let mut ui = rule(move || {
        let r = r.clone();
        fixed_size(
            30,
            Modifier.offset_with(move |_| o.get()).on_layout_rect_changed(
                1,
                2000,
                move |bounds: RelativeLayoutBounds| r.set(Some(bounds.bounds_in_root())),
            ),
        );
    });
    // Called at once: the throttle isn't 0.
    assert_eq!(rect.get(), Some(IntRect::new(0, 0, 30, 30)));
    // Compose's device time passes between the two; the test clock moves by hand.
    ui.advance_time_by_frame();
    offset.set(IntOffset::new(10, 10));
    ui.wait_for_idle();
    assert_eq!(rect.get(), Some(IntRect::new(10, 10, 40, 40)));
}

/// In a popup, the root is the popup's: its bounds in the root stay as it moves, those in the
/// window follow it. Compose's popup is a window of its own (its bounds in the window stay
/// too); compose-rs's popups are in the host's window.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, testLayerBoundsPositionInMovedWindow
#[test]
fn a_moved_popup_s_content_keeps_its_bounds_in_the_popup() {
    let told = shared();
    let alignment = mutable_state_of(compose_ui::Alignment::CENTER);
    let (t, a) = (told.clone(), alignment.handle());
    let mut ui = rule(move || {
        let t = t.clone();
        Box!(modifier = Modifier.fill_max_size(), {
            let t = t.clone();
            Popup!(alignment = a.get(), {
                let t = t.clone();
                fixed_size(
                    30,
                    Modifier.padding(10.dp()).on_layout_rect_changed(
                        0,
                        0,
                        move |bounds: RelativeLayoutBounds| {
                            t.set(Some((bounds.bounds_in_root(), bounds.bounds_in_window())))
                        },
                    ),
                );
            });
        });
    });
    let (root, window) = told.get().expect("told");
    assert_eq!(root.top_left(), IntOffset::new(10, 10));
    assert_eq!(window.top_left(), IntOffset::new(485, 485));
    alignment.set(compose_ui::Alignment::BOTTOM_END);
    ui.wait_for_idle();
    let (root, window) = told.get().expect("told");
    assert_eq!(root.top_left(), IntOffset::new(10, 10));
    assert_eq!(window.top_left(), IntOffset::new(960, 960));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, occlusionCalculationOnRectChangedCallbacks
#[test]
fn occlusions_are_those_drawn_over_the_node_as_asked() {
    let fraction = mutable_state_of(1.0f32);
    let last0: Rc<Cell<Option<RelativeLayoutBounds>>> = Rc::default();
    let occlusions: Rc<Vec<Shared<usize>>> = Rc::new((0..3).map(|_| shared()).collect());
    let calls: Rc<Vec<Shared<u32>>> = Rc::new((0..3).map(|_| shared()).collect());
    // Made once, as the test's `val`s: the same callbacks at each composition.
    let watchers: Vec<Callback<dyn Fn(RelativeLayoutBounds)>> = (0..3)
        .map(|index| {
            let (l, o, c) = (last0.clone(), occlusions.clone(), calls.clone());
            callback(move |bounds: RelativeLayoutBounds| {
                if index == 0 {
                    l.set(Some(bounds));
                }
                c[index].set(c[index].get() + 1);
                o[index].set(bounds.calculate_occlusions().len());
            })
        })
        .collect();
    let f = fraction.handle();
    let mut ui = rule(move || {
        let watchers = watchers.clone();
        let watch =
            move |index: usize| Modifier.on_layout_rect_changed(0, 0, watchers[index].clone());
        Box!(modifier = Modifier.fill_max_size(), |scope: BoxScope| {
            use compose_ui::Alignment;
            Box!(
                modifier = Modifier
                    .fill_max_width()
                    .fill_max_height_with(0.5)
                    .align(scope, Alignment::TOP_START)
                    .then(watch(0))
            );
            // Over the first.
            Box!(
                modifier = Modifier
                    .fill_max_width()
                    .fill_max_height_with(0.7)
                    .align(scope, Alignment::BOTTOM_START)
                    .then(watch(1))
            );
            // Over both.
            Box!(
                modifier = Modifier
                    .fill_max_size_with(f.get())
                    .align(scope, Alignment::BOTTOM_START)
                    .then(watch(2))
            );
        });
    });
    let counts = || occlusions.iter().map(|it| it.get()).collect::<Vec<_>>();
    let called = || calls.iter().map(|it| it.get()).collect::<Vec<_>>();
    assert_eq!(counts(), vec![2, 1, 0]);
    assert_eq!(called(), vec![1, 1, 1]);
    // The third no longer over the first: only it is told.
    fraction.set(0.3);
    ui.wait_for_idle();
    assert_eq!(counts(), vec![2, 1, 0]);
    assert_eq!(called(), vec![1, 1, 2]);
    // Asked again, the first's occlusions are as they are now.
    assert_eq!(last0.get().expect("told").calculate_occlusions().len(), 1);
}

/// A layout of 50 by 50 with its first baseline at 0, placing `content` at its top left.
fn baseline_layout(modifier: Modifier, content: impl Fn() + 'static) {
    Layout!(
        modifier = modifier,
        measure_policy = measure_policy(|scope, measurables, constraints| {
            let placeables: Vec<_> =
                measurables.iter().map(|it| scope.measure(*it, constraints)).collect();
            scope.provide_alignment_lines([(compose_ui::FIRST_BASELINE, 0)]);
            for placeable in &placeables {
                scope.place(placeable, 0, 0);
            }
            scope.layout(50, 50)
        }),
        { content() }
    );
}

/// Measures its child, asks its first baseline, and places it at `(x, y)`.
fn asking_baseline(x: i32, y: i32, content: impl Fn() + 'static) {
    Layout!(
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let placeable = scope.measure(measurables[0], constraints);
            let _ = scope.alignment_line(&placeable, compose_ui::FIRST_BASELINE);
            scope.place(&placeable, x, y);
            scope.layout(placeable.width, placeable.height)
        }),
        { content() }
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionIsReportedForANodeParticipatingInAlignmentCalculation
#[test]
fn a_node_whose_line_is_asked_is_told_where_it_is() {
    let position = shared();
    let p = position.clone();
    let _ui = rule(move || {
        let p = p.clone();
        Layout!(measure_policy = placing_first_at(|| Some((10, 10))), {
            let p = p.clone();
            asking_baseline(0, 0, move || {
                let p = p.clone();
                Box!({
                    baseline_layout(keep_position(&p), || {});
                });
            });
        });
    });
    assert_eq!(position.get(), Some(IntOffset::new(10, 10)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionIsReportedForANodeParticipatingInAlignmentCalculation_afterMove
#[test]
fn a_node_in_one_whose_line_is_asked_is_told_its_move() {
    let position = shared();
    let extra = mutable_state_of(IntOffset::ZERO);
    let (p, e) = (position.clone(), extra.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        Layout!(measure_policy = placing_first_at(|| Some((10, 10))), {
            let p = p.clone();
            asking_baseline(0, 0, move || {
                let p = p.clone();
                Box!({
                    let p = p.clone();
                    baseline_layout(Modifier, move || {
                        Box!(
                            modifier = Modifier
                                .offset_with(move |_| e.get())
                                .size(10.dp())
                                .then(keep_position(&p))
                        );
                    });
                });
            });
        });
    });
    extra.set(IntOffset::new(20, 20));
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(30, 30)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionIsReportedWhenAddingChildWithBaselineLater
#[test]
fn a_child_with_a_line_added_later_is_told_where_it_is() {
    let position = shared();
    let need = mutable_state_of(false);
    let (p, n) = (position.clone(), need.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        asking_baseline(10, 0, move || {
            let p = p.clone();
            Box!({
                if n.get() {
                    baseline_layout(keep_position(&p), || {});
                }
            });
        });
    });
    need.set(true);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(10, 0)));
}

/// A layout measuring its child at exactly `size()` by `size()`.
fn fixed_container(size: impl Fn() -> i32 + 'static, content: impl Fn() + 'static) {
    Layout!(
        measure_policy = measure_policy(move |scope, measurables, _| {
            let size = size();
            let placeable = scope.measure(measurables[0], Constraints::fixed(size, size));
            scope.place(&placeable, 0, 0);
            scope.layout(size, size)
        }),
        { content() }
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionInRootWhenOffsetIsProvidedByLayoutCooperation
#[test]
fn a_node_centered_in_constraints_it_doesn_t_fit_is_told_where_it_is() {
    let position = shared();
    let p = position.clone();
    let _ui = rule(move || {
        let p = p.clone();
        fixed_container(
            || 100,
            move || {
                Box!(modifier = Modifier.required_size((50.dp(), 40.dp())).then(keep_position(&p)));
            },
        );
    });
    assert_eq!(position.get(), Some(IntOffset::new(25, 30)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, correctPositionInRootWhenOffsetIsProvidedByLayoutCooperation_afterConstraintsChange
#[test]
fn a_centered_node_is_told_its_container_s_new_size() {
    let position = shared();
    let container = mutable_state_of(100);
    let (p, c) = (position.clone(), container.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        fixed_container(
            move || c.get(),
            move || {
                let p = p.clone();
                Box!(
                    modifier = Modifier.offset(0.dp(), 0.dp()).required_size((50.dp(), 40.dp())),
                    {
                        Box!(modifier = keep_position(&p));
                    }
                );
            },
        );
    });
    assert_eq!(position.get(), Some(IntOffset::new(25, 30)));
    container.set(110);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(30, 35)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, updatingOffsetInParentsLayoutModifier
#[test]
fn an_offset_given_while_measuring_a_parent_s_layout_modifier_is_told() {
    let position = shared();
    let offset = mutable_state_of_with(IntOffset::ZERO, NeverEqualPolicy);
    let (p, o) = (position.clone(), offset.handle());
    let mut ui = rule(move || {
        let p = p.clone();
        let moving = Modifier.layout(move |scope, measurable, constraints| {
            let placeable = scope.measure(measurable, constraints);
            let at = o.get();
            scope.layout_with(constraints.max_width, constraints.max_height, move |placement| {
                placement.place(&placeable, at.x, at.y)
            })
        });
        Box!(modifier = moving, {
            Box!(modifier = Modifier.size(10.dp()).then(keep_position(&p)));
        });
    });
    offset.set(IntOffset::new(20, 20));
    ui.wait_for_idle();
    offset.set(IntOffset::new(20, 20));
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(20, 20)));
}

/// The tests' nested layout: a layout with `modifier` placing a 10 by 10 box at its origin, with
/// a layout in it placing the node at `(0, offset)`.
fn nested_with(
    modifier: impl Fn() -> Modifier + 'static,
    offset: MutableState<i32>,
    into: &Shared<Option<IntOffset>>,
) -> impl Fn() + 'static {
    let into = into.clone();
    let modifier = Rc::new(modifier);
    move || {
        let into = into.clone();
        Layout!(modifier = modifier(), measure_policy = placing_first_at(|| Some((0, 0))), {
            let into = into.clone();
            Box!(modifier = Modifier.size(10.dp()), {
                let into = into.clone();
                Box!({
                    let into = into.clone();
                    Layout!(measure_policy = placing_first_at(move || Some((0, offset.get()))), {
                        Box!(modifier = keep_position(&into));
                    });
                });
            });
        });
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, removingLayoutModifierShouldInvalidateOffsetCacheForSubtree
#[test]
fn removing_an_ancestor_s_layout_modifier_is_told() {
    let position = shared();
    let offset = mutable_state_of(0);
    let with_extra = mutable_state_of(true);
    let w = with_extra.handle();
    let moved = |x: i32| Modifier.offset_with(move |_| IntOffset::new(x, 0));
    let mut ui = rule(nested_with(
        move || if w.get() { moved(10).then(moved(10)) } else { moved(10) },
        offset.handle(),
        &position,
    ));
    assert_eq!(position.get(), Some(IntOffset::new(20, 0)));
    offset.set(5);
    with_extra.set(false);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(10, 5)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, removingLayerModifierShouldInvalidateOffsetCacheForSubtree
#[test]
fn removing_an_ancestor_s_layer_is_told() {
    let position = shared();
    let offset = mutable_state_of(0);
    let with_extra = mutable_state_of(true);
    let w = with_extra.handle();
    let moved = || Modifier.graphics_layer(|layer| layer.translation_x = 10.0);
    let mut ui = rule(nested_with(
        move || if w.get() { moved().then(moved()) } else { moved() },
        offset.handle(),
        &position,
    ));
    assert_eq!(position.get(), Some(IntOffset::new(20, 0)));
    offset.set(5);
    with_extra.set(false);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(10, 5)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, updatingLayerBlockShouldInvalidateOffsetCacheForSubtree
#[test]
fn an_ancestor_s_new_layer_and_a_move_are_told() {
    let position = shared();
    let offset = mutable_state_of(0);
    let o = offset.handle();
    let mut ui = rule(nested_with(
        move || {
            // A new block each time `offset` changes, as `graphicsLayer(translationX = …)`.
            let x = o.get() as f32;
            Modifier.graphics_layer(move |layer| layer.translation_x = x)
        },
        offset.handle(),
        &position,
    ));
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    offset.set(5);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(5, 5)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalRectChangedTest.kt, updatingLayerBlockShouldCallCallbackOnTheSameNode
#[test]
fn a_new_layer_on_the_node_itself_is_told() {
    let position = shared();
    let offset = mutable_state_of(0);
    let (p, o) = (position.clone(), offset.handle());
    let mut ui = rule(move || {
        let x = o.get() as f32;
        Layout!(
            modifier = Modifier
                .graphics_layer(move |layer| layer.translation_x = x)
                .then(keep_position(&p)),
            measure_policy = measure_policy(|scope, _, constraints| {
                scope.layout(constraints.max_width, constraints.max_height)
            }),
            {}
        );
    });
    assert_eq!(position.get(), Some(IntOffset::ZERO));
    offset.set(5);
    ui.wait_for_idle();
    assert_eq!(position.get(), Some(IntOffset::new(5, 0)));
}
