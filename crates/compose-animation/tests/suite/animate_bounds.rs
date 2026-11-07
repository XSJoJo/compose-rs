//! `Modifier.animate_bounds` against Compose's `AnimateBoundsTest`: a layout's position, size
//! and bounds animating in a `LookaheadScope` toward where the pass ahead puts them, with a
//! modifier inside the animation or chained after it.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use compose_animation::*;
use compose_foundation::layout::*;
use compose_foundation::{
    GridCells, LazyGridScope, LazyVerticalGrid, ScrollModifiers, ScrollableStateExt,
};
use compose_runtime::*;
use compose_test::*;
use compose_ui::{IntOffset, IntSize, LookaheadScope, Offset, PaddingValues, Rect, Size, UnitExt};

/// A tween of `millis`, linear, as each test's `boundsTransform`.
fn linear(millis: u32) -> BoundsTransform {
    BoundsTransform::new(move |_, _| tween(millis).easing(LINEAR_EASING).into())
}

fn mid(a: Size, b: Size) -> Size {
    Size::new((a.width + b.width) * 0.5, (a.height + b.height) * 0.5)
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, animatePosition
/// A box going from the top start to the bottom end of its 100 × 100 parent is half-way there
/// half-way through the tween, and at the bottom end once it's over.
#[test]
fn animate_position() {
    let frames = 14;
    let duration = frames * 16;
    let (root, side) = (100, 20);
    let position = Rc::new(Cell::new(IntOffset::new(-1, -1)));
    let at_start = mutable_state_of(true);
    let (p, start) = (position.clone(), at_start.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let p = p.clone();
        Box!(modifier = Modifier.size(root.dp()), move |box_scope| {
            let p = p.clone();
            LookaheadScope(callback(move |scope| {
                let p = p.clone();
                let alignment =
                    if start.get() { Alignment::TOP_START } else { Alignment::BOTTOM_END };
                Box!(
                    modifier = Modifier
                        .align(box_scope, alignment)
                        .size(side.dp())
                        .animate_bounds_with(scope, Modifier, linear(duration), false)
                        .on_globally_positioned(move |it: compose_ui::LayoutCoordinates| p
                            .set(it.position_in_parent().round()))
                );
            }));
        });
    });
    ui.wait_for_idle();
    assert_eq!(position.get(), IntOffset::ZERO);

    ui.set_auto_advance(false);
    at_start.set(false);
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    ui.advance_time_by(Duration::from_millis(duration as u64 / 2));
    let half = (root - side) as f32 * 0.5;
    assert_eq!(position.get(), Offset::new(half, half).round());

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(position.get(), IntOffset::new(root - side, root - side));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, animateSize
/// A box growing from a quarter to half of its 400 × 400 parent is half-way between them
/// half-way through, from its first animated frame, and at the large size at the end.
#[test]
fn animate_size() {
    let frame = 16;
    let duration = 14 * frame;
    let root = 400.0;
    let (small, large) = (Size::new(root * 0.25, root * 0.25), Size::new(root * 0.5, root * 0.5));
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let expanded = mutable_state_of(false);
    let (s, e) = (size.clone(), expanded.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let s = s.clone();
        Box!(modifier = Modifier.size(root.dp()), move |_| {
            let s = s.clone();
            LookaheadScope(callback(move |scope| {
                let s = s.clone();
                let side = if e.get() { large.width } else { small.width };
                Box!(
                    modifier = Modifier
                        .size(side.dp())
                        .animate_bounds_with(scope, Modifier, linear(duration), false)
                        .on_globally_positioned(
                            move |it: compose_ui::LayoutCoordinates| s.set(it.size())
                        )
                );
            }));
        });
    });
    ui.wait_for_idle();
    assert_eq!(size.get(), small.round_to_int_size());

    ui.set_auto_advance(false);
    expanded.set(true);
    ui.wait_for_idle();
    // Up to the first animated frame.
    loop {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        if size.get() != small.round_to_int_size() {
            break;
        }
    }
    ui.advance_time_by(Duration::from_millis((duration / 2 - frame) as u64));
    ui.wait_for_idle();
    assert_eq!(size.get(), mid(large, small).round_to_int_size());

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(size.get(), large.round_to_int_size());
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, animateBounds
/// A box going from a small one at the top start to a large one at the bottom end is half-way
/// in both size and position half-way through, and there at the end.
#[test]
fn animate_bounds() {
    let duration = 14 * 16;
    let root = 400.0;
    let (small, large) = (Size::new(root * 0.25, root * 0.25), Size::new(root * 0.5, root * 0.5));
    let bounds = Rc::new(Cell::new(Rect::ZERO));
    let toggle = mutable_state_of(false);
    let (b, t) = (bounds.clone(), toggle.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let b = b.clone();
        Box!(modifier = Modifier.size(root.dp()), move |box_scope| {
            let b = b.clone();
            LookaheadScope(callback(move |scope| {
                let b = b.clone();
                let placed = if t.get() {
                    Modifier.align(box_scope, Alignment::BOTTOM_END).size(large.width.dp())
                } else {
                    Modifier.align(box_scope, Alignment::TOP_START).size(small.width.dp())
                };
                Box!(
                    modifier = Modifier
                        .then(placed)
                        .animate_bounds_with(scope, Modifier, linear(duration), false)
                        .on_globally_positioned(
                            move |it: compose_ui::LayoutCoordinates| b.set(it.bounds_in_parent())
                        )
                );
            }));
        });
    });
    ui.wait_for_idle();
    assert_eq!(bounds.get(), Rect::from_offset_size(Offset::ZERO, small));

    ui.set_auto_advance(false);
    toggle.set(true);
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    ui.advance_time_by(Duration::from_millis(duration as u64 / 2));
    ui.wait_for_idle();
    let half = (root - large.width) * 0.5;
    assert_eq!(bounds.get(), Rect::from_offset_size(Offset::new(half, half), mid(large, small)));

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    let end = root - large.width;
    assert_eq!(bounds.get(), Rect::from_offset_size(Offset::new(end, end), large));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, animateBounds_withIntermediateModifier
/// A padding given as the animation's modifier animates (a sixteenth of the way at the first
/// animated frame); one chained after it changes at once.
#[test]
fn animate_bounds_with_intermediate_modifier() {
    let duration = 10 * 16;
    let (root, padding1, padding2) = (100, 10, 20);
    let a = Rc::new(Cell::new(IntOffset::new(-1, -1)));
    let b = Rc::new(Cell::new(IntOffset::new(-1, 1)));
    let toggle = mutable_state_of(true);
    let (pa, pb, t) = (a.clone(), b.clone(), toggle.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (pa, pb) = (pa.clone(), pb.clone());
        LookaheadScope(callback(move |scope| {
            let (pa, pb) = (pa.clone(), pb.clone());
            let padding = move || {
                let side = if t.get() { padding1 } else { padding2 };
                Modifier.padding(PaddingValues::symmetric(side.dp(), 0.dp()))
            };
            Column!(modifier = Modifier.width(root.dp()), move |_| {
                let (pa, pb) = (pa.clone(), pb.clone());
                Row!(modifier = Modifier.fill_max_width(), move |_| {
                    let pa = pa.clone();
                    Box!(
                        modifier =
                            Modifier.animate_bounds_with(scope, padding(), linear(duration), false),
                        move |_| {
                            let pa = pa.clone();
                            Box!(
                                modifier = Modifier.on_globally_positioned(
                                    move |it: compose_ui::LayoutCoordinates| {
                                        pa.set(it.position_in_root().round())
                                    }
                                )
                            );
                        }
                    );
                });
                Row!(modifier = Modifier.fill_max_width(), move |_| {
                    let pb = pb.clone();
                    Box!(
                        modifier = Modifier
                            .animate_bounds_with(scope, Modifier, linear(duration), false)
                            .then(padding()),
                        move |_| {
                            let pb = pb.clone();
                            Box!(
                                modifier = Modifier.on_globally_positioned(
                                    move |it: compose_ui::LayoutCoordinates| {
                                        pb.set(it.position_in_root().round())
                                    }
                                )
                            );
                        }
                    );
                });
            });
        }));
    });
    ui.wait_for_idle();
    assert_eq!(a.get(), IntOffset::new(padding1, 0));
    assert_eq!(b.get(), IntOffset::new(padding1, 0));

    ui.set_auto_advance(false);
    toggle.set(false);
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    // The first animated frame.
    ui.advance_time_by_frame();
    let expected = padding1 as f32 + (padding2 - padding1) as f32 * (16.0 / duration as f32);
    assert_eq!(a.get(), IntOffset::new(expected.round() as i32, 0));
    assert_eq!(b.get(), IntOffset::new(padding2, 0));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, animateBounds_usingMovableContent
/// A box moved by movable content from a 30 × 30 slot at (70, 70) to a 50 × 50 one at (110, 110)
/// keeps its animation: half-way through it's half-way in position and size.
#[test]
fn animate_bounds_using_movable_content() {
    let duration = 14 * 16;
    let (a_side, a_at) = (30, IntOffset::new(70, 70));
    let (b_side, b_at) = (50, IntOffset::new(110, 110));
    let position = Rc::new(Cell::new(IntOffset::ZERO));
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let at_a = mutable_state_of(true);
    let (p, s, a) = (position.clone(), size.clone(), at_a.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (p, s) = (p.clone(), s.clone());
        let movable = remember(|| {
            movable_content_with_receiver_of(move |scope: LookaheadScope, (): ()| {
                let (p, s) = (p.clone(), s.clone());
                Box!(
                    modifier = Modifier
                        .fill_max_size()
                        .animate_bounds_with(scope, Modifier, linear(duration), false)
                        .on_globally_positioned(move |it: compose_ui::LayoutCoordinates| {
                            p.set(it.position_in_root().round());
                            s.set(it.size());
                        })
                );
            })
        });
        LookaheadScope(callback(move |scope| {
            let movable = movable.clone();
            Box!(move |_| {
                let (m1, m2) = (movable.clone(), movable.clone());
                Box!(
                    modifier = Modifier.offset(a_at.x.dp(), a_at.y.dp()).size(a_side.dp()),
                    move |_| {
                        if a.get() {
                            m1(scope, ());
                        }
                    }
                );
                Box!(
                    modifier = Modifier.offset(b_at.x.dp(), b_at.y.dp()).size(b_side.dp()),
                    move |_| {
                        if !a.get() {
                            m2(scope, ());
                        }
                    }
                );
            });
        }));
    });
    ui.wait_for_idle();
    assert_eq!(position.get(), a_at);
    assert_eq!(size.get(), IntSize::new(a_side, a_side));

    ui.set_auto_advance(false);
    at_a.set(false);
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    ui.advance_time_by(Duration::from_millis(duration as u64 / 2));
    ui.wait_for_idle();
    let half = (a_side + b_side) / 2;
    assert_eq!(position.get(), IntOffset::new((a_at.x + b_at.x) / 2, (a_at.y + b_at.y) / 2));
    assert_eq!(size.get(), IntSize::new(half, half));

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(position.get(), b_at);
    assert_eq!(size.get(), IntSize::new(b_side, b_side));
}

/// A two-column grid of square items, each with `animate_bounds`, `noted` hearing item `id`'s
/// coordinates; the grid's width is `width`.
fn grid_of_animated_items(
    width: MutableState<i32>,
    id: usize,
    noted: impl Fn(compose_ui::LayoutCoordinates) + 'static,
) -> ComposeTestRule {
    let noted: Rc<dyn Fn(compose_ui::LayoutCoordinates)> = Rc::new(noted);
    ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let noted = noted.clone();
        LookaheadScope(callback(move |scope| {
            let noted = noted.clone();
            LazyVerticalGrid!(
                GridCells::Fixed(2),
                modifier = Modifier.fill_max_height().width(width.get().dp()),
                move |grid: LazyGridScope| {
                    let noted = noted.clone();
                    grid.items_keyed(
                        5,
                        |it| it,
                        move |item| {
                            let noted = noted.clone();
                            let noting = if item == id {
                                Modifier.on_globally_positioned(
                                    move |it: compose_ui::LayoutCoordinates| noted(it),
                                )
                            } else {
                                Modifier
                            };
                            Box!(
                                modifier = Modifier
                                    .animate_bounds(scope)
                                    .then(noting)
                                    .fill_max_width()
                                    .aspect_ratio(1.0)
                            );
                        },
                    );
                }
            );
        }));
    })
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, animateBounds_inLazyLayout
/// In a grid widening from 200 to 400, the first item grows from 100 to 200 without moving: its
/// size is kept within its constraints, so the grid doesn't center it.
#[test]
fn animate_bounds_in_lazy_layout() {
    let width = mutable_state_of(200);
    let positions: Rc<RefCell<Vec<Offset>>> = Rc::default();
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let (p, s) = (positions.clone(), size.clone());
    let mut ui = grid_of_animated_items(width.handle(), 0, move |it| {
        p.borrow_mut().push(it.position_in_root());
        s.set(it.size());
    });
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    width.set(400);
    assert_eq!(size.get(), IntSize::new(100, 100));
    let mut frames = 0;
    while size.get() != IntSize::new(200, 200) {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        assert_eq!(positions.borrow().last().copied(), Some(Offset::ZERO));
        frames += 1;
        assert!(frames < 200, "the animation ends");
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, motionFrameOfReferenceOfItemsInLazyGrid
/// In a grid widening from 200 to 400, the second item's motion frame of reference (its offset
/// in the root with the grid's placement of it, less without) stays (200, 0) while it animates.
#[test]
fn motion_frame_of_reference_of_items_in_lazy_grid() {
    let width = mutable_state_of(200);
    let frames_of_reference: Rc<RefCell<Vec<Offset>>> = Rc::default();
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let (m, s) = (frames_of_reference.clone(), size.clone());
    let mut ui = grid_of_animated_items(width.handle(), 1, move |it| {
        let root = it.find_root_coordinates();
        let with = root.local_position_of_with(&it, Offset::ZERO, true);
        let without = root.local_position_of_with(&it, Offset::ZERO, false);
        m.borrow_mut().push(with - without);
        s.set(it.size());
    });
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    width.set(400);
    assert_eq!(size.get(), IntSize::new(100, 100));
    frames_of_reference.borrow_mut().clear();
    let mut frames = 0;
    while size.get() != IntSize::new(200, 200) {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        let last = frames_of_reference.borrow().last().copied();
        assert_eq!(last, Some(Offset::new(200.0, 0.0)));
        frames += 1;
        assert!(frames < 200, "the animation ends");
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/AnimateBoundsTest.kt, animateBounds_scrollBehavior
/// A scroll moves the items at once unless `animate_motion_frame_of_reference`: then it
/// animates, along a keyframe putting the first item at y = 150 at its start.
#[test]
fn animate_bounds_scroll_behavior() {
    let item = 30.0;
    let key_frame_offset = item * 5.0;
    let animate_scroll = mutable_state_of(false);
    let scroll = compose_foundation::ScrollState::new(0);
    let state = *scroll;
    let position = Rc::new(Cell::new(IntOffset::new(-1, -1)));
    let (p, animated) = (position.clone(), animate_scroll.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let p = p.clone();
        LookaheadScope(callback(move |scope| {
            let p = p.clone();
            Column!(modifier = Modifier.size(item.dp()).vertical_scroll(state), move |_| {
                for index in 0..2 {
                    let p = p.clone();
                    // Starts at y = 150: never seen unless the scroll animates.
                    let transform = BoundsTransform::new(move |initial: Rect, _| {
                        let start = Rect::from_offset_size(
                            Offset::new(0.0, key_frame_offset),
                            initial.size(),
                        );
                        keyframes::<Rect>(|frames| {
                            frames.at(0, start).using(LINEAR_EASING);
                        })
                        .into()
                    });
                    Box!(
                        modifier = Modifier
                            .size(item.dp())
                            .animate_bounds_with(scope, Modifier, transform, animated.get())
                            .on_globally_positioned(move |it: compose_ui::LayoutCoordinates| {
                                if index == 0 {
                                    p.set(it.position_in_root().round());
                                }
                            })
                    );
                }
            });
        }));
    });
    let scroll_by = |ui: &mut ComposeTestRule, by: f32| {
        compose_runtime::platform::launch(async move {
            let _ = state.scroll_by(by).await;
        });
        ui.wait_for_idle();
    };
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    scroll_by(&mut ui, item);
    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    assert_eq!(position.get(), IntOffset::new(0, -(item as i32)));

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    animate_scroll.set(true);
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    scroll_by(&mut ui, -item);
    ui.advance_time_by_frame();
    assert_eq!(position.get(), Offset::new(0.0, key_frame_offset).round());
}
