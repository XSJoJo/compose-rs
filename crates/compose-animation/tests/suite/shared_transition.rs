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

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testIsTransitionActiveWhenNoMatch
/// An `AnimatedContent` swapping contents with shared elements of different keys never makes the
/// transition active: nothing matches.
#[test]
fn is_transition_active_when_no_match() {
    let transitioned = mutable_state_of(false);
    let record: Rc<RefCell<Vec<bool>>> = Rc::default();
    let disposed = Rc::new(Cell::new(false));
    let (t, r, d) = (transitioned.handle(), record.clone(), disposed.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (r, d) = (r.clone(), d.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let (s, d) = (scope.clone(), d.clone());
            Box!(modifier = Modifier.fill_max_size(), move |_| {
                let (s, d) = (s.clone(), d.clone());
                AnimatedContent!(t.get(), move |content: AnimatedContentScope, shown: bool| {
                    let (s, d) = (s.clone(), d.clone());
                    Box!(modifier = Modifier.fill_max_size(), move |box_scope| {
                        if shown {
                            let state = s.remember_shared_content_state("key1");
                            Box!(
                                modifier = s
                                    .shared_element(&state, &content)
                                    .aspect_ratio(2.0)
                                    .background(Color::RED)
                                    .align(box_scope, Alignment::TOP_CENTER)
                            );
                        } else {
                            let state = s.remember_shared_content_state("key2");
                            Box!(
                                modifier = s
                                    .shared_element(&state, &content)
                                    .size((200.dp(), 300.dp()))
                                    .background(Color::RED)
                                    .align(box_scope, Alignment::BOTTOM_END)
                            );
                            let d = d.clone();
                            DisposableEffect((), move |effect| {
                                let d = d.clone();
                                effect.on_dispose(move || d.set(true))
                            });
                        }
                    });
                });
            });
            r.borrow_mut().push(scope.is_transition_active());
        });
    });
    ui.wait_for_idle();
    assert!(!record.borrow().is_empty());
    assert!(!record.borrow().iter().any(|it| *it));
    ui.set_auto_advance(false);
    transitioned.set(true);
    ui.wait_for_idle();
    let mut rounds = 0;
    while !disposed.get() {
        ui.advance_time_by(std::time::Duration::from_millis(200));
        ui.wait_for_idle();
        rounds += 1;
        assert!(rounds < 100, "the old content leaves");
    }
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert!(!record.borrow().iter().any(|it| *it));
}

/// A config never enabled.
struct NeverEnabled;

impl SharedContentConfig for NeverEnabled {
    fn is_enabled(&self, _state: &SharedContentState) -> bool {
        false
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testNoLayerWhenConfigDisabled
/// Two contents of a key, one disabled for good: no match, no transition, no layer.
#[test]
fn no_layer_when_config_disabled() {
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let states: Rc<RefCell<Vec<SharedContentState>>> = Rc::default();
    let (sc, st) = (seen.clone(), states.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (sc, st) = (sc.clone(), st.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            *sc.borrow_mut() = Some(scope.clone());
            let (s, st) = (scope.clone(), st.clone());
            Box!(modifier = Modifier.fill_max_size(), move |_| {
                let config: Rc<dyn SharedContentConfig> =
                    remember(|| Rc::new(NeverEnabled) as Rc<dyn SharedContentConfig>);
                let disabled = s.remember_shared_content_state_with("box", config);
                Box!(
                    modifier = s
                        .shared_element_with_caller_managed_visibility(&disabled, false)
                        .size(200.dp())
                );
                let enabled = s.remember_shared_content_state("box");
                Box!(
                    modifier = s
                        .shared_element_with_caller_managed_visibility(&enabled, true)
                        .size(100.dp())
                );
                *st.borrow_mut() = vec![disabled, enabled];
            });
        });
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(scope.has_shared_element_for_key("box"));
    assert!(!scope.is_transition_active());
    assert!(!scope.has_layer_for(&states.borrow()[0]));
    assert!(!states.borrow()[0].is_match_found());
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testLayerOnlyCreatedForOverlayRenderingEntry
/// Only the content drawn in the overlay, the incoming one, gets a layer; the outgoing one,
/// which isn't drawn, gets none; the incoming one's layer goes when it leaves.
#[test]
fn layer_only_created_for_overlay_rendering_entry() {
    let show_match = mutable_state_of(false);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let states: Rc<RefCell<Vec<SharedContentState>>> = Rc::default();
    let (m, sc, st) = (show_match.handle(), seen.clone(), states.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (sc, st) = (sc.clone(), st.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            *sc.borrow_mut() = Some(scope.clone());
            let (s, st0) = (scope.clone(), st.clone());
            let outgoing = remember(|| {
                movable_content_of(move || {
                    let state = s.remember_shared_content_state("box");
                    Box!(
                        modifier = s
                            .shared_element_with_caller_managed_visibility(&state, !m.get())
                            .size(200.dp())
                    );
                    let mut states = st0.borrow_mut();
                    if states.is_empty() {
                        states.push(state);
                    } else {
                        states[0] = state;
                    }
                })
            });
            let (s, st) = (scope.clone(), st.clone());
            Box!(modifier = Modifier.fill_max_size(), move |_| {
                outgoing();
                if m.get() {
                    let state = s.remember_shared_content_state("box");
                    Box!(
                        modifier = s
                            .shared_element_with_caller_managed_visibility(&state, true)
                            .size(100.dp())
                    );
                    let mut states = st.borrow_mut();
                    states.truncate(1);
                    states.push(state);
                }
            });
        });
    });
    ui.wait_for_idle();
    show_match.set(true);
    ui.set_auto_advance(false);
    for _ in 0..5 {
        ui.advance_time_by_frame();
    }
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    let (outgoing, incoming) = (states.borrow()[0].clone(), states.borrow()[1].clone());
    assert!(scope.has_layer_for(&incoming));
    assert!(!scope.has_layer_for(&outgoing));
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    show_match.set(false);
    ui.wait_for_idle();
    assert!(!scope.has_layer_for(&incoming));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testZIndexInOverlay
/// In the overlay, what's drawn on top is what has the highest z-index: a green box rendered in
/// the overlay and the red and blue shared bounds, their z-indices changing each frame.
#[test]
fn z_index_in_overlay() {
    let visible = mutable_state_of(true);
    let green = mutable_state_of(0.0f32);
    let red = mutable_state_of(0.0f32);
    let blue = mutable_state_of(0.0f32);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (v, g, r, b, sc) =
        (visible.handle(), green.handle(), red.handle(), blue.handle(), seen.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let sc = sc.clone();
        SharedTransitionLayout!(
            modifier = Modifier.required_size(100.dp()).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
                *sc.borrow_mut() = Some(scope.clone());
                let in_overlay = {
                    let scope = scope.clone();
                    remember(|| Rc::new(move || scope.is_transition_active()))
                };
                let in_overlay = in_overlay.clone();
                Box!(
                    modifier = scope
                        .render_in_shared_transition_scope_overlay_with(g.get(), move || {
                            in_overlay()
                        })
                        .background(Color::GREEN)
                        .fill_max_size()
                );
                let shared = |z: f32| SharedOptions {
                    enter: EnterTransition::NONE,
                    exit: ExitTransition::NONE,
                    z_index_in_overlay: z,
                    ..Default::default()
                };
                let s1 = scope.clone();
                AnimatedVisibility!(
                    v.get(),
                    enter = EnterTransition::NONE,
                    exit = fade_out_with(tween(100).easing(LINEAR_EASING), 0.0),
                    move |visibility: AnimatedVisibilityScope| {
                        let state = s1.remember_shared_content_state("child");
                        Box!(
                            modifier = s1
                                .shared_bounds_with(&state, &visibility, shared(r.get()))
                                .background(Color::RED)
                                .fill_max_size()
                        );
                    }
                );
                let s2 = scope.clone();
                AnimatedVisibility!(
                    !v.get(),
                    enter = fade_in_with(tween(100).easing(LINEAR_EASING), 0.0),
                    exit = ExitTransition::NONE,
                    move |visibility: AnimatedVisibilityScope| {
                        let state = s2.remember_shared_content_state("child");
                        Box!(
                            modifier = s2
                                .shared_bounds_with(&state, &visibility, shared(b.get()))
                                .background(Color::BLUE)
                                .fill_max_size()
                        );
                    }
                );
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    assert_eq!(top_color_at(&mut ui, Offset::new(50.0, 50.0)), Some(Color::RED));

    ui.set_auto_advance(false);
    visible.set(false);
    green.set(0.0);
    red.set(0.0);
    blue.set(1.0);
    let mut expected = Color::BLUE;
    let mut i = 0;
    let mut frames = 0;
    while !scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "the transition starts");
    }
    while scope.is_transition_active() {
        assert_eq!(top_color_at(&mut ui, Offset::new(50.0, 50.0)), Some(expected), "at {i}");
        let base = i as f32;
        green.set(base);
        red.set(base);
        blue.set(base);
        match i {
            0 => {
                red.set(base + 1.0);
                expected = Color::RED;
            }
            1 => {
                green.set(base + 1.0);
                expected = Color::GREEN;
            }
            _ => {
                blue.set(base + 1.0);
                expected = Color::BLUE;
            }
        }
        i = (i + 1) % 3;
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
}

/// The rects of `color` drawn, at where they're drawn in the root, and the color drawn last over
/// the first's top left.
fn rects_of(ui: &mut ComposeTestRule, color: Color) -> (Vec<Offset>, Option<Color>) {
    let flat = ui.host().display_list();
    let mut origin = Offset::ZERO;
    let mut found = Vec::new();
    for op in &flat.ops {
        match op {
            DrawOp::Translate(by) => origin = origin + *by,
            DrawOp::DrawRect(rect, paint) if paint.color == color => {
                found.push(Offset::new(origin.x + rect.left, origin.y + rect.top));
            }
            _ => {}
        }
    }
    let top = found.first().and_then(|at| top_color_at(ui, *at + Offset::new(0.5, 0.5)));
    (found, top)
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testRenderInSharedTransitionOverlayInvalidationInTheSameFrameAsPlacementChange
/// A box rendered in the overlay only when asked: not asked, it's drawn in place, once, under
/// the content after it; asked, in the overlay, once, over it; either way where its parent's
/// offset, read while placing, puts it in that frame.
#[test]
fn render_in_overlay_follows_placement_in_the_same_frame() {
    let offset = mutable_state_of(0);
    let render_in_overlay = mutable_state_of(false);
    let overlay_background = Color::WHITE.with_alpha(0.5);
    let color = Color::RED.with_alpha(0.5);
    let (o, r) = (offset.handle(), render_in_overlay.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        SharedTransitionLayout!(
            modifier = Modifier.size(20.dp()).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
                let s = scope.clone();
                Box!(
                    modifier =
                        Modifier.offset_with(move |_| compose_ui::IntOffset::new(o.get(), o.get())),
                    move |_| {
                        Box!(
                            modifier = s
                                .render_in_shared_transition_scope_overlay_with(0.0, move || r
                                    .get())
                                .background(color)
                                .size(1.dp())
                        );
                    }
                );
                Box!(modifier = Modifier.fill_max_size().background(overlay_background));
            }
        );
    });
    ui.wait_for_idle();
    for _ in 0..3 {
        offset.update(|it| it + 3);
        ui.wait_for_idle();
        let at = offset.get() as f32;
        let (found, top) = rects_of(&mut ui, color);
        assert_eq!(found, vec![Offset::new(at, at)], "in place, once");
        assert_eq!(top, Some(overlay_background), "under what follows it");
    }
    render_in_overlay.set(true);
    ui.wait_for_idle();
    for _ in 0..3 {
        offset.update(|it| it + 3);
        ui.wait_for_idle();
        let at = offset.get() as f32;
        let (found, top) = rects_of(&mut ui, color);
        assert_eq!(found, vec![Offset::new(at, at)], "in the overlay, once");
        assert_eq!(top, Some(color), "over what follows it");
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, initialVelocityForExitIsUsed
/// A shared element prepared with an initial velocity (right and up) moves off from a control
/// element without one, that way; the next transition, without, keeps them together.
#[test]
fn initial_velocity_for_exit_is_used() {
    let exit = mutable_state_of(false);
    let state: Rc<RefCell<Option<SharedContentState>>> = Rc::default();
    let positions: Rc<RefCell<Vec<Offset>>> = Rc::new(RefCell::new(vec![Offset::ZERO; 4]));
    let (e, st, p) = (exit.handle(), state.clone(), positions.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (st, p) = (st.clone(), p.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let (s, st, p) = (scope.clone(), st.clone(), p.clone());
            AnimatedContent!(e.get(), move |content: AnimatedContentScope, exited: bool| {
                let (s, st, p) = (s.clone(), st.clone(), p.clone());
                let (side, first) = if exited { (200, 1) } else { (100, 0) };
                Box!(
                    modifier = Modifier.fill_max_size(),
                    content_alignment = Alignment::CENTER,
                    move |_| {
                        let test = s.remember_shared_content_state("test");
                        if !exited {
                            *st.borrow_mut() = Some(test.clone());
                        }
                        let noting = |at: usize| {
                            let p = p.clone();
                            Modifier.on_globally_positioned(move |it: LayoutCoordinates| {
                                p.borrow_mut()[at] = it.position_in_root();
                            })
                        };
                        Box!(
                            modifier = s
                                .shared_element(&test, &content)
                                .size(side.dp())
                                .then(noting(first))
                        );
                        let control = s.remember_shared_content_state("control");
                        Box!(
                            modifier = s
                                .shared_element(&control, &content)
                                .size(side.dp())
                                .then(noting(first + 2))
                        );
                    }
                );
            });
        });
    });
    ui.wait_for_idle();
    exit.set(true);
    ui.set_auto_advance(false);
    let last = positions.borrow()[0];
    state
        .borrow()
        .as_ref()
        .expect("composed")
        .prepare_transition_with_initial_velocity(compose_ui::Velocity::new(11000.0, -12000.0));
    let mut frames = 0;
    while positions.borrow()[0] == last {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "it moves");
    }
    let (test, control) = (positions.borrow()[0], positions.borrow()[2]);
    assert!(test.x - control.x > 0.0, "{test:?} {control:?}");
    assert!(test.y - control.y < 0.0, "{test:?} {control:?}");
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    exit.set(false);
    ui.set_auto_advance(false);
    let last = positions.borrow()[1];
    let mut frames = 0;
    while positions.borrow()[1] == last {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "it moves");
    }
    let (test, control) = (positions.borrow()[1], positions.borrow()[3]);
    assert_eq!(test, control);
}
