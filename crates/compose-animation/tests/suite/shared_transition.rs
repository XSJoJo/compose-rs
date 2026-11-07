//! Shared elements against Compose's `SharedTransitionTest`: contents matched by key across an
//! `AnimatedVisibility`'s (or the caller's) visibility, their bounds animating from one to the
//! other, in sync, interrupted, and the match found and lost as contents come and go.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_animation::*;
use compose_foundation::layout::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::DrawOp;
use compose_ui::{Color, IntSize, LayoutCoordinates, Offset, UnitExt};

/// A linear tween of `millis` for bounds.
fn linear(millis: u32) -> BoundsTransform {
    BoundsTransform::new(move |_, _| tween(millis).easing(LINEAR_EASING).into())
}

fn near_offset(expected: Offset, actual: Offset, tolerance: f32) -> bool {
    (expected.x - actual.x).abs() <= tolerance && (expected.y - actual.y).abs() <= tolerance
}

fn near_size(expected: IntSize, actual: IntSize, tolerance: i32) -> bool {
    (expected.width - actual.width).abs() <= tolerance
        && (expected.height - actual.height).abs() <= tolerance
}

/// Notes `at`'s position in the layout and size into `positions` and `sizes`.
fn noting(
    scope: &SharedTransitionScope,
    positions: &Rc<RefCell<Vec<Offset>>>,
    sizes: &Rc<RefCell<Vec<IntSize>>>,
    at: usize,
) -> Modifier {
    let (scope, positions, sizes) = (scope.clone(), positions.clone(), sizes.clone());
    Modifier.on_globally_positioned(move |it: LayoutCoordinates| {
        if let Some(root) = scope.lookahead_root_for_test() {
            positions.borrow_mut()[at] = root.local_position_of(&it, Offset::ZERO);
        }
        sizes.borrow_mut()[at] = it.size();
    })
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, transitionInterruption
/// A shared element and shared bounds swapping places between a column and a row stay in sync
/// with their match through the transition and when it's interrupted, and change continuously.
#[test]
fn transition_interruption() {
    let visible = mutable_state_of(true);
    let positions: Rc<RefCell<Vec<Offset>>> = Rc::new(RefCell::new(vec![Offset::ZERO; 4]));
    let sizes: Rc<RefCell<Vec<IntSize>>> = Rc::new(RefCell::new(vec![
        IntSize::new(-1, -1),
        IntSize::new(-1, -1),
        IntSize::ZERO,
        IntSize::ZERO,
    ]));
    let scope_seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (p, s, seen, v) = (positions.clone(), sizes.clone(), scope_seen.clone(), visible.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (p, s, seen) = (p.clone(), s.clone(), seen.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            *seen.borrow_mut() = Some(scope.clone());
            let transform = linear(500);
            let options = SharedOptions { bounds_transform: transform, ..Default::default() };
            let (p1, s1, sc1, o1) = (p.clone(), s.clone(), scope.clone(), options.clone());
            AnimatedVisibility!(v.get(), move |visibility: AnimatedVisibilityScope| {
                let (p, s, scope, options) = (p1.clone(), s1.clone(), sc1.clone(), o1.clone());
                Column!(move |_| {
                    let visibility = &visibility;
                    let cat = scope.remember_shared_content_state("cat");
                    let dog = scope.remember_shared_content_state("dog");
                    Box!(
                        modifier = scope
                            .shared_element_with(&cat, visibility, options.clone())
                            .then(noting(&scope, &p, &s, 0))
                            .size(200.dp())
                    );
                    Box!(
                        modifier = scope
                            .shared_bounds_with(&dog, visibility, options.clone())
                            .then(noting(&scope, &p, &s, 1))
                            .size(50.dp())
                    );
                });
            });
            let (p2, s2, sc2, o2) = (p.clone(), s.clone(), scope.clone(), options.clone());
            AnimatedVisibility!(!v.get(), move |visibility: AnimatedVisibilityScope| {
                let (p, s, scope, options) = (p2.clone(), s2.clone(), sc2.clone(), o2.clone());
                Row!(move |_| {
                    let visibility = &visibility;
                    let dog = scope.remember_shared_content_state("dog");
                    let cat = scope.remember_shared_content_state("cat");
                    Box!(
                        modifier = scope
                            .shared_element_with(&dog, visibility, options.clone())
                            .then(noting(&scope, &p, &s, 2))
                            .size(50.dp())
                    );
                    Box!(
                        modifier = scope
                            .shared_bounds_with(&cat, visibility, options.clone())
                            .then(noting(&scope, &p, &s, 3))
                            .size(200.dp())
                    );
                });
            });
        });
    });
    ui.wait_for_idle();
    let scope = scope_seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    assert_eq!(sizes.borrow()[0], IntSize::new(200, 200));
    assert_eq!(sizes.borrow()[1], IntSize::new(50, 50));
    assert_eq!(positions.borrow()[0], Offset::ZERO);
    assert_eq!(positions.borrow()[1], Offset::new(0.0, 200.0));

    ui.set_auto_advance(false);
    visible.set(false);
    for _ in 0..20 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    let in_sync = |positions: &[Offset], sizes: &[IntSize]| {
        assert!(near_offset(positions[0], positions[3], 5.0), "{positions:?}");
        assert!(near_offset(positions[1], positions[2], 5.0), "{positions:?}");
        assert!(near_size(sizes[0], sizes[3], 5), "{sizes:?}");
        assert!(near_size(sizes[1], sizes[2], 5), "{sizes:?}");
    };
    in_sync(&positions.borrow(), &sizes.borrow());
    assert!(scope.is_transition_active());

    // Interrupted.
    visible.set(true);
    let mut last_sizes = sizes.borrow().clone();
    let mut last_positions = positions.borrow().clone();
    let mut frames = 0;
    while scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        let (p, s) = (positions.borrow().clone(), sizes.borrow().clone());
        in_sync(&p, &s);
        // The first grows and the second shrinks from where they were, continuously.
        assert!(s[0].width >= last_sizes[0].width && s[0].height >= last_sizes[0].height);
        assert!(near_size(s[0], last_sizes[0], 10), "{s:?} after {last_sizes:?}");
        assert!(s[1].width <= last_sizes[1].width && s[1].height <= last_sizes[1].height);
        assert!(near_size(s[1], last_sizes[1], 10), "{s:?} after {last_sizes:?}");
        assert!(near_offset(p[0], last_positions[0], 20.0), "{p:?} after {last_positions:?}");
        assert!(near_offset(p[1], last_positions[1], 40.0), "{p:?} after {last_positions:?}");
        assert_eq!(p[0].y, 0.0);
        assert_eq!(p[1].x, 0.0);
        last_sizes = s;
        last_positions = p;
        frames += 1;
        assert!(frames < 500, "the transition ends");
    }
    assert_eq!(sizes.borrow()[0], IntSize::new(200, 200));
    assert_eq!(sizes.borrow()[1], IntSize::new(50, 50));
    assert_eq!(positions.borrow()[0], Offset::ZERO);
    assert_eq!(positions.borrow()[1], Offset::new(0.0, 200.0));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testKeyMatch
/// Contents of four keys (an object, a number, a string, unit) match only once a second content
/// of each key is composed, through a change of visibility, and no more once it leaves.
#[test]
fn key_match() {
    #[derive(Hash, PartialEq, Eq, Debug, Clone)]
    struct Key1;
    let states: Rc<RefCell<Vec<SharedContentState>>> = Rc::default();
    let show_row = mutable_state_of(false);
    let visible = mutable_state_of(true);
    let (set, row, v) = (states.clone(), show_row.handle(), visible.handle());
    let add = |set: &Rc<RefCell<Vec<SharedContentState>>>, state: SharedContentState| {
        if !set.borrow().contains(&state) {
            set.borrow_mut().push(state.clone());
        }
        state
    };
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let set = set.clone();
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let (set1, scope1) = (set.clone(), scope.clone());
            Column!(move |_| {
                let (set, scope) = (&set1, &scope1);
                let shown = v.get();
                let s = add(set, scope.remember_shared_content_state(Key1));
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility(&s, shown)
                        .size(200.dp())
                );
                let s = add(set, scope.remember_shared_content_state(2));
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility(&s, shown)
                        .size(200.dp())
                );
                let s = add(set, scope.remember_shared_content_state("cat"));
                Box!(
                    modifier = scope
                        .shared_bounds_with_caller_managed_visibility(&s, shown, Default::default())
                        .size(200.dp())
                );
                let s = add(set, scope.remember_shared_content_state(()));
                Box!(
                    modifier = scope
                        .shared_bounds_with_caller_managed_visibility(&s, shown, Default::default())
                        .size(200.dp())
                );
            });
            if row.get() {
                let (set2, scope2) = (set.clone(), scope.clone());
                Row!(move |_| {
                    let (set, scope) = (&set2, &scope2);
                    let shown = !v.get();
                    let s = add(set, scope.remember_shared_content_state(Key1));
                    Box!(
                        modifier = scope
                            .shared_element_with_caller_managed_visibility(&s, shown)
                            .size(50.dp())
                    );
                    let s = add(set, scope.remember_shared_content_state(2));
                    Box!(
                        modifier = scope
                            .shared_element_with_caller_managed_visibility(&s, shown)
                            .size(50.dp())
                    );
                    let s = add(set, scope.remember_shared_content_state("cat"));
                    Box!(
                        modifier = scope
                            .shared_bounds_with_caller_managed_visibility(
                                &s,
                                shown,
                                Default::default()
                            )
                            .size(50.dp())
                    );
                    let s = add(set, scope.remember_shared_content_state(()));
                    Box!(
                        modifier = scope
                            .shared_bounds_with_caller_managed_visibility(
                                &s,
                                shown,
                                Default::default()
                            )
                            .size(50.dp())
                    );
                });
            }
        });
    });
    ui.wait_for_idle();
    assert_eq!(states.borrow().len(), 4);
    assert!(states.borrow().iter().all(|it| !it.is_match_found()));

    show_row.set(true);
    ui.wait_for_idle();
    assert_eq!(states.borrow().len(), 8);
    assert!(states.borrow().iter().all(SharedContentState::is_match_found));
    visible.set(false);
    ui.wait_for_idle();
    assert!(states.borrow().iter().all(SharedContentState::is_match_found));
    states.borrow_mut().clear();
    show_row.set(false);
    ui.wait_for_idle();
    assert_eq!(states.borrow().len(), 4);
    assert!(states.borrow().iter().all(|it| !it.is_match_found()));
}

/// A `SharedTransitionLayout` of 100 × 100 with two `AnimatedVisibility`s, the first shown while
/// `visible` with no enter or exit, the second while not, fading in over 100 ms, with
/// `second_modifier`: `first` and `second` give each one's content.
fn two_visibilities(
    visible: MutableState<bool>,
    seen: Rc<RefCell<Option<SharedTransitionScope>>>,
    second_modifier: Modifier,
    first: impl Fn(&SharedTransitionScope, &AnimatedVisibilityScope) + 'static,
    second: impl Fn(&SharedTransitionScope, &AnimatedVisibilityScope) + 'static,
) -> ComposeTestRule {
    let (first, second) = (Rc::new(first), Rc::new(second));
    ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (seen, first, second) = (seen.clone(), first.clone(), second.clone());
        let second_modifier = second_modifier.clone();
        SharedTransitionLayout!(
            modifier = Modifier.required_size(100.dp()).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
                *seen.borrow_mut() = Some(scope.clone());
                let (s1, f) = (scope.clone(), first.clone());
                AnimatedVisibility!(
                    visible.get(),
                    enter = EnterTransition::NONE,
                    exit = ExitTransition::NONE,
                    move |visibility: AnimatedVisibilityScope| f(&s1, &visibility)
                );
                let (s2, g) = (scope.clone(), second.clone());
                AnimatedVisibility!(
                    !visible.get(),
                    modifier = second_modifier.clone(),
                    enter = fade_in_with(tween(100), 0.0),
                    exit = ExitTransition::NONE,
                    move |visibility: AnimatedVisibilityScope| g(&s2, &visibility)
                );
            }
        );
    })
}

/// Hides the first content, then advances frames until the transition starts.
fn start_transition(
    ui: &mut ComposeTestRule,
    visible: &OwnedState<bool>,
    scope: &SharedTransitionScope,
) {
    ui.set_auto_advance(false);
    visible.set(false);
    let mut frames = 0;
    while !scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "the transition starts");
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testBoundsTransform
/// Shared bounds going from the whole 100 × 100 layout to a 50 × 50 box at (45, 25) follow their
/// linear tween; the shared element inside them, the same size and place in both, stays put.
#[test]
fn bounds_transform() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let parent: Rc<Cell<Option<(Offset, IntSize)>>> = Rc::default();
    let child: Rc<Cell<Option<(Offset, IntSize)>>> = Rc::default();
    let exit_transition: Rc<RefCell<Option<Transition<EnterExitState>>>> = Rc::default();
    let (p, c, e) = (parent.clone(), child.clone(), exit_transition.clone());
    let mut ui = two_visibilities(
        visible.handle(),
        seen.clone(),
        Modifier.fill_max_size_with(0.5).offset(25.dp(), 25.dp()),
        |scope, visibility| {
            let test = scope.remember_shared_content_state("test");
            let (s, v) = (scope.clone(), visibility.clone());
            Box!(
                modifier = scope.shared_bounds(&test, visibility).fill_max_size(),
                content_alignment = Alignment::CENTER,
                move |_| {
                    let child = s.remember_shared_content_state("child");
                    Box!(
                        modifier =
                            Modifier.fill_max_size_with(0.5).then(s.shared_element(&child, &v))
                    );
                }
            );
        },
        move |scope, visibility| {
            *e.borrow_mut() = Some(visibility.transition().clone());
            let test = scope.remember_shared_content_state("test");
            let options = SharedOptions {
                bounds_transform: linear(100),
                resize_mode: ResizeMode::RemeasureToBounds,
                ..Default::default()
            };
            let (s, v, p, c) = (scope.clone(), visibility.clone(), p.clone(), c.clone());
            let noted = {
                let s = s.clone();
                Modifier.on_globally_positioned(move |it: LayoutCoordinates| {
                    let root = s.lookahead_root_for_test().expect("placed");
                    p.set(Some((root.local_position_of(&it, Offset::ZERO), it.size())));
                })
            };
            Box!(
                modifier = Modifier
                    .offset(20.dp(), 0.dp())
                    .then(scope.shared_bounds_with(&test, visibility, options))
                    .then(noted),
                move |_| {
                    let child = s.remember_shared_content_state("child");
                    let options = SharedOptions {
                        bounds_transform: BoundsTransform::new(|initial, target| {
                            assert_eq!(initial, target);
                            spring().into()
                        }),
                        ..Default::default()
                    };
                    let (s2, c) = (s.clone(), c.clone());
                    Box!(
                        modifier = Modifier
                            .offset((-20).dp(), 0.dp())
                            .then(s.shared_element_with(&child, &v, options))
                            .on_globally_positioned(move |it: LayoutCoordinates| {
                                let root = s2.lookahead_root_for_test().expect("placed");
                                c.set(Some((root.local_position_of(&it, Offset::ZERO), it.size())));
                            })
                            .fill_max_size()
                            .background(Color::BLUE)
                    );
                }
            );
        },
    );
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    start_transition(&mut ui, &visible, &scope);
    let mut frames = 0;
    while scope.is_transition_active() {
        let play = exit_transition.borrow().as_ref().expect("shown").play_time_nanos();
        let fraction = ((play / 1_000_000) as f32 / 100.0).clamp(0.0, 1.0);
        let side = (50.0 * fraction + 100.0 * (1.0 - fraction)).round() as i32;
        let (at, size) = parent.get().expect("placed");
        assert!(near_size(IntSize::new(side, side), size, 3), "{size:?} at {fraction}");
        assert!(near_offset(Offset::new(45.0 * fraction, 25.0 * fraction), at, 3.0), "{at:?}");
        assert_eq!(child.get(), Some((Offset::new(25.0, 25.0), IntSize::new(50, 50))));
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 200, "the transition ends");
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testPlaceholderSize
/// Through a transition, the parent of a shared element with the default placeholder keeps its
/// content's size; the other's parent is whatever its placeholder size says, each frame.
#[test]
fn placeholder_size() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let parent1 = Rc::new(Cell::new(IntSize::ZERO));
    let parent2 = Rc::new(Cell::new(IntSize::ZERO));
    let expected = mutable_state_of(IntSize::ZERO);
    let (p1, p2, ex) = (parent1.clone(), parent2.clone(), expected.handle());
    let mut ui = two_visibilities(
        visible.handle(),
        seen.clone(),
        Modifier,
        move |scope, visibility| {
            let p1 = p1.clone();
            let (s, v) = (scope.clone(), visibility.clone());
            Box!(
                modifier = Modifier
                    .fill_max_size()
                    .wrap_content_size()
                    .on_globally_positioned(move |it: LayoutCoordinates| p1.set(it.size())),
                content_alignment = Alignment::CENTER,
                move |_| {
                    let child = s.remember_shared_content_state("child");
                    Box!(modifier = s.shared_element(&child, &v).fill_max_size_with(0.5));
                }
            );
        },
        move |scope, visibility| {
            let p2 = p2.clone();
            let child = scope.remember_shared_content_state("child");
            let options = SharedOptions {
                placeholder_size: PlaceholderSize::new(move |_, _| ex.get()),
                ..Default::default()
            };
            Box!(
                modifier = Modifier
                    .on_globally_positioned(move |it: LayoutCoordinates| p2.set(it.size()))
                    .offset((-20).dp(), 0.dp())
                    .then(scope.shared_element_with(&child, visibility, options))
                    .fill_max_size()
                    .background(Color::BLUE)
            );
        },
    );
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    start_transition(&mut ui, &visible, &scope);
    let sizes = [IntSize::new(13, 77), IntSize::new(90, 4), IntSize::new(50, 61), IntSize::ZERO];
    let mut frame = 0;
    while scope.is_transition_active() {
        assert_eq!(parent1.get(), IntSize::new(50, 50));
        assert_eq!(parent2.get(), expected.get());
        expected.set(sizes[frame % sizes.len()]);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frame += 1;
        assert!(frame < 200, "the transition ends");
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testSkipToLookahead
/// Under `skip_to_lookahead_size`, a shared element's content keeps its size ahead, 50 × 50,
/// throughout the transition, while the shared element's bounds animate.
#[test]
fn skip_to_lookahead() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let size: Rc<Cell<Option<IntSize>>> = Rc::default();
    let s = size.clone();
    let mut ui = two_visibilities(
        visible.handle(),
        seen.clone(),
        Modifier.fill_max_size(),
        |scope, visibility| {
            let child = scope.remember_shared_content_state("child");
            let options = SharedOptions { bounds_transform: linear(100), ..Default::default() };
            Box!(modifier = scope.shared_element_with(&child, visibility, options).fill_max_size());
        },
        move |scope, visibility| {
            let s = s.clone();
            let child = scope.remember_shared_content_state("child");
            let options = SharedOptions {
                bounds_transform: linear(100),
                clip_in_overlay_during_transition: scope
                    .overlay_clip(compose_foundation::CIRCLE_SHAPE),
                ..Default::default()
            };
            Box!(
                modifier = Modifier
                    .padding(25.dp())
                    .then(scope.shared_element_with(&child, visibility, options))
                    .then(scope.skip_to_lookahead_size())
                    .on_globally_positioned(move |it: LayoutCoordinates| s.set(Some(it.size())))
                    .background(Color::BLUE)
            );
        },
    );
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    assert_eq!(size.get(), None);
    start_transition(&mut ui, &visible, &scope);
    let mut frames = 0;
    while scope.is_transition_active() {
        assert_eq!(size.get(), Some(IntSize::new(50, 50)));
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 200, "the transition ends");
    }
}

/// A config enabled while `enabled` is.
struct EnabledWhile(MutableState<bool>);

impl SharedContentConfig for EnabledWhile {
    fn is_enabled(&self, _state: &SharedContentState) -> bool {
        self.0.get()
    }
}

/// The color of the last rect drawn over `point` in the root.
fn top_color_at(ui: &mut ComposeTestRule, point: Offset) -> Option<Color> {
    let flat = ui.host().display_list();
    let mut origin = Offset::ZERO;
    let mut top = None;
    for op in &flat.ops {
        match op {
            DrawOp::Translate(by) => origin = origin + *by,
            DrawOp::DrawRect(rect, paint) => {
                let rect = rect.translate(origin);
                if rect.contains(point) {
                    top = Some(paint.color);
                }
            }
            _ => {}
        }
    }
    top
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testSharedElementRenderedInOverlayAboveOtherContent
/// Disabled, two shared elements of one key don't match and hold no layer, and the red box
/// composed after them draws over them; enabled and transitioning, the visible one draws in the
/// overlay, over the red box.
#[test]
fn shared_element_rendered_in_overlay_above_other_content() {
    let config_enabled = mutable_state_of(false);
    let visible = mutable_state_of(false);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let states: Rc<RefCell<Vec<SharedContentState>>> = Rc::default();
    let (enabled, v, sc, st) =
        (config_enabled.handle(), visible.handle(), seen.clone(), states.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (sc, st) = (sc.clone(), st.clone());
        SharedTransitionLayout!(
            modifier = Modifier.required_size(100.dp()).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
                *sc.borrow_mut() = Some(scope.clone());
                let config: Rc<dyn SharedContentConfig> =
                    remember(|| Rc::new(EnabledWhile(enabled)) as Rc<dyn SharedContentConfig>);
                let outgoing = scope.remember_shared_content_state_with("box", config.clone());
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility(&outgoing, !v.get())
                        .required_size(100.dp())
                        .background(Color::BLUE)
                );
                let incoming = scope.remember_shared_content_state_with("box", config.clone());
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility(&incoming, v.get())
                        .required_size(100.dp())
                        .background(Color::BLUE)
                );
                *st.borrow_mut() = vec![outgoing, incoming];
                Box!(modifier = Modifier.required_size(50.dp()).background(Color::RED));
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    for state in states.borrow().iter() {
        assert!(!scope.has_layer_for(state));
    }
    assert_eq!(top_color_at(&mut ui, Offset::new(10.0, 10.0)), Some(Color::RED));

    ui.set_auto_advance(false);
    config_enabled.set(true);
    ui.advance_time_by_frame();
    visible.set(true);
    let mut frames = 0;
    while !scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "the transition starts");
    }
    let incoming = states.borrow()[1].clone();
    assert!(scope.has_layer_for(&incoming));
    assert_eq!(top_color_at(&mut ui, Offset::new(10.0, 10.0)), Some(Color::BLUE));
    ui.set_auto_advance(true);
    ui.wait_for_idle();
}
