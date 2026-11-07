//! Shared elements against Compose's `SharedTransitionTest`: contents matched by key across an
//! `AnimatedVisibility`'s (or the caller's) visibility, their bounds animating from one to the
//! other, in sync, interrupted, and the match found and lost as contents come and go.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_animation::*;
use compose_foundation::BasicText;
use compose_foundation::layout::*;
use compose_foundation::{LazyColumn, LazyListScope, LazyListState, ScrollableStateExt};
use compose_runtime::*;
use compose_test::*;
use compose_ui::ContentScale;
use compose_ui::platform::DrawOp;
use compose_ui::subcompose::SubcomposeLayout;
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

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testSharedContentStateClipPathInOverlay
/// Through a transition, shared bounds with a clip of their own and the shared element inside
/// them, clipped by its parent's (the default), both report that path in the overlay; those of
/// the content leaving, not drawn there, report none.
#[test]
fn shared_content_state_clip_path_in_overlay() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let states: Rc<RefCell<Vec<Option<SharedContentState>>>> =
        Rc::new(RefCell::new(vec![None, None, None, None]));
    let predefined = compose_ui::Path::new();
    predefined.add_rect(compose_ui::Rect::new(5.0, 5.0, 6.0, 6.0));
    let (st1, st2, clip_path) = (states.clone(), states.clone(), predefined.clone());
    let mut ui = two_visibilities(
        visible.handle(),
        seen.clone(),
        Modifier.fill_max_size_with(0.5).offset(25.dp(), 25.dp()),
        move |scope, visibility| {
            let parent = scope.remember_shared_content_state("parent");
            st1.borrow_mut()[0] = Some(parent.clone());
            let (s, v, st) = (scope.clone(), visibility.clone(), st1.clone());
            Box!(
                modifier = scope.shared_bounds(&parent, visibility).fill_max_size(),
                content_alignment = Alignment::CENTER,
                move |_| {
                    let child = s.remember_shared_content_state("child");
                    st.borrow_mut()[1] = Some(child.clone());
                    Box!(
                        modifier =
                            Modifier.fill_max_size_with(0.5).then(s.shared_element(&child, &v))
                    );
                }
            );
        },
        move |scope, visibility| {
            let parent = scope.remember_shared_content_state("parent");
            st2.borrow_mut()[2] = Some(parent.clone());
            let path = clip_path.clone();
            let options = SharedOptions {
                clip_in_overlay_during_transition: OverlayClip::new(move |_, _, _, _| {
                    Some(path.clone())
                }),
                ..Default::default()
            };
            let (s, v, st) = (scope.clone(), visibility.clone(), st2.clone());
            Box!(
                modifier = Modifier
                    .offset(20.dp(), 0.dp())
                    .then(scope.shared_bounds_with(&parent, visibility, options)),
                move |_| {
                    let child = s.remember_shared_content_state("child");
                    st.borrow_mut()[3] = Some(child.clone());
                    Box!(
                        modifier = Modifier
                            .offset((-20).dp(), 0.dp())
                            .then(s.shared_element(&child, &v))
                            .fill_max_size()
                    );
                }
            );
        },
    );
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    start_transition(&mut ui, &visible, &scope);
    ui.wait_for_idle();
    let clip_of = |at: usize| {
        let state = states.borrow()[at].clone().expect("composed");
        state.clip_path_in_overlay().map(|path| path.id())
    };
    let mut frames = 0;
    while scope.is_transition_active() {
        assert_eq!(clip_of(2), Some(predefined.id()));
        assert_eq!(clip_of(3), Some(predefined.id()));
        assert_eq!(clip_of(0), None);
        assert_eq!(clip_of(1), None);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
}

/// The alphas under 1 the display list composites with: its layers' and its groups'.
fn faded_alphas(ui: &mut ComposeTestRule) -> Vec<f32> {
    let flat = ui.host().display_list();
    let mut alphas: Vec<f32> = flat
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::SaveLayerAlpha { alpha, .. } | DrawOp::SaveLayer { alpha, .. } => Some(*alpha),
            _ => None,
        })
        .collect();
    alphas.extend(flat.groups.iter().filter_map(|group| group.layer.as_ref()).map(|l| l.alpha));
    alphas.retain(|alpha| *alpha < 1.0);
    alphas
}

// Compose: compose/animation/animation/src/commonMain/kotlin/androidx/compose/animation/SharedTransitionScope.kt, sharedBounds (its enter and exit enabled while isMatchFound)
/// A `shared_bounds` entering with no match doesn't fade: its enter waits for a match, so the
/// content comes in opaque from its first frame.
#[test]
fn shared_bounds_without_a_match_does_not_fade() {
    let visible = mutable_state_of(false);
    let v = visible.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        SharedTransitionLayout!(
            modifier = Modifier.required_size(100.dp()),
            move |scope: SharedTransitionScope| {
                let s = scope.clone();
                AnimatedVisibility!(
                    v.get(),
                    enter = EnterTransition::NONE,
                    exit = ExitTransition::NONE,
                    move |visibility: AnimatedVisibilityScope| {
                        let state = s.remember_shared_content_state("alone");
                        let options = SharedOptions {
                            enter: fade_in_with(tween(160).easing(LINEAR_EASING), 0.0),
                            ..Default::default()
                        };
                        Box!(
                            modifier = s
                                .shared_bounds_with(&state, &visibility, options)
                                .background(Color::RED)
                                .fill_max_size()
                        );
                    }
                );
            }
        );
    });
    ui.wait_for_idle();
    assert!(rects_of(&mut ui, Color::RED).0.is_empty());
    ui.set_auto_advance(false);
    visible.set(true);
    for frame in 0..10 {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        assert!(!rects_of(&mut ui, Color::RED).0.is_empty(), "shown at {frame}");
        assert_eq!(faded_alphas(&mut ui), Vec::<f32>::new(), "at {frame}");
    }
}

// Compose: compose/animation/animation/src/commonMain/kotlin/androidx/compose/animation/SharedTransitionScope.kt, LayerRendererComparator (a top-level shared entry of z-index 0 sorts as -1)
/// In the overlay, a top-level shared content at z-index 0 draws under what renders in the
/// overlay at z-index 0, though it joins the overlay after it.
#[test]
fn a_top_level_shared_content_at_z_zero_draws_under_the_overlay() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (v, sc) = (visible.handle(), seen.clone());
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
                        .render_in_shared_transition_scope_overlay_with(0.0, move || in_overlay())
                        .background(Color::GREEN)
                        .fill_max_size()
                );
                let options = SharedOptions {
                    enter: EnterTransition::NONE,
                    exit: ExitTransition::NONE,
                    bounds_transform: linear(200),
                    ..Default::default()
                };
                let (s1, o1) = (scope.clone(), options.clone());
                AnimatedVisibility!(
                    v.get(),
                    enter = EnterTransition::NONE,
                    exit = fade_out_with(tween(200).easing(LINEAR_EASING), 0.0),
                    move |visibility: AnimatedVisibilityScope| {
                        let state = s1.remember_shared_content_state("child");
                        Box!(
                            modifier = s1
                                .shared_bounds_with(&state, &visibility, o1.clone())
                                .background(Color::RED)
                                .fill_max_size()
                        );
                    }
                );
                let (s2, o2) = (scope.clone(), options.clone());
                AnimatedVisibility!(
                    !v.get(),
                    enter = fade_in_with(tween(200).easing(LINEAR_EASING), 0.0),
                    exit = ExitTransition::NONE,
                    move |visibility: AnimatedVisibilityScope| {
                        let state = s2.remember_shared_content_state("child");
                        Box!(
                            modifier = s2
                                .shared_bounds_with(&state, &visibility, o2.clone())
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
    assert_eq!(top_color_at(&mut ui, Offset::new(50.0, 50.0)), Some(Color::RED));
    start_transition(&mut ui, &visible, &scope);
    let mut frames = 0;
    while scope.is_transition_active() {
        assert_eq!(top_color_at(&mut ui, Offset::new(50.0, 50.0)), Some(Color::GREEN), "{frames}");
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
    assert!(frames > 2, "{frames}");
}

// Compose: compose/runtime/runtime/src/commonMain/kotlin/androidx/compose/runtime/Recomposer.kt, runRecomposeAndApplyChanges (a frame's animations run before the recomposition reversing the target)
/// Reversed mid-way, a shared element takes the frame's step toward the old target (the frame's
/// animations run before the recomposition reversing it, as Compose's), then turns back at once:
/// the deferred match update applies in that frame's lookahead placement, not a frame later.
#[test]
fn a_reversal_turns_back_after_the_frames_step() {
    let visible = mutable_state_of(true);
    let positions: Rc<RefCell<Vec<Offset>>> = Rc::new(RefCell::new(vec![Offset::ZERO; 2]));
    let sizes: Rc<RefCell<Vec<IntSize>>> = Rc::new(RefCell::new(vec![IntSize::ZERO; 2]));
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (p, s, sc, v) = (positions.clone(), sizes.clone(), seen.clone(), visible.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (p, s, sc) = (p.clone(), s.clone(), sc.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            *sc.borrow_mut() = Some(scope.clone());
            let options = SharedOptions { bounds_transform: linear(500), ..Default::default() };
            for (at, shown) in [(0usize, v.get()), (1, !v.get())] {
                let (p, s, scope, options) = (p.clone(), s.clone(), scope.clone(), options.clone());
                AnimatedVisibility!(
                    shown,
                    enter = EnterTransition::NONE,
                    exit = ExitTransition::NONE,
                    move |visibility: AnimatedVisibilityScope| {
                        let state = scope.remember_shared_content_state("box");
                        Box!(
                            modifier = Modifier
                                .offset((at as i32 * 100).dp(), 0.dp())
                                .then(scope.shared_element_with(
                                    &state,
                                    &visibility,
                                    options.clone(),
                                ))
                                .then(noting(&scope, &p, &s, at))
                                .size(20.dp())
                        );
                    }
                );
            }
        });
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    start_transition(&mut ui, &visible, &scope);
    for _ in 0..10 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    let before = positions.borrow()[1].x;
    assert!(before > 10.0 && before < 90.0, "{before}");
    visible.set(true);
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    let reversed = positions.borrow()[0].x;
    assert!(reversed > before, "the frame's step toward the old target: {reversed} {before}");
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    let back = positions.borrow()[0].x;
    assert!(back < reversed, "turned back in the next frame: {back} {reversed}");
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(positions.borrow()[0], Offset::ZERO);
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, transitionInterruptionSelfManagedVisibility
/// The same swap with the caller managing visibility: in sync with their match through the
/// transition and when it's interrupted, changing continuously.
#[test]
fn transition_interruption_self_managed_visibility() {
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
            let options = SharedOptions { bounds_transform: linear(500), ..Default::default() };
            let (p1, s1, sc1, o1) = (p.clone(), s.clone(), scope.clone(), options.clone());
            Column!(move |_| {
                let (p, s, scope, options) = (&p1, &s1, &sc1, &o1);
                let cat = scope.remember_shared_content_state("cat");
                let dog = scope.remember_shared_content_state("dog");
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility_with(
                            &cat,
                            v.get(),
                            options.clone(),
                        )
                        .then(noting(scope, p, s, 0))
                        .size(200.dp())
                );
                Box!(
                    modifier = scope
                        .shared_bounds_with_caller_managed_visibility(
                            &dog,
                            v.get(),
                            options.clone()
                        )
                        .then(noting(scope, p, s, 1))
                        .size(50.dp())
                );
            });
            let (p2, s2, sc2, o2) = (p.clone(), s.clone(), scope.clone(), options.clone());
            Row!(move |_| {
                let (p, s, scope, options) = (&p2, &s2, &sc2, &o2);
                let dog = scope.remember_shared_content_state("dog");
                let cat = scope.remember_shared_content_state("cat");
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility_with(
                            &dog,
                            !v.get(),
                            options.clone(),
                        )
                        .then(noting(scope, p, s, 2))
                        .size(50.dp())
                );
                Box!(
                    modifier = scope
                        .shared_bounds_with_caller_managed_visibility(
                            &cat,
                            !v.get(),
                            options.clone(),
                        )
                        .then(noting(scope, p, s, 3))
                        .size(200.dp())
                );
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
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 500, "the transition ends");
    }
    assert_eq!(sizes.borrow()[0], IntSize::new(200, 200));
    assert_eq!(sizes.borrow()[1], IntSize::new(50, 50));
    assert_eq!(positions.borrow()[0], Offset::ZERO);
    assert_eq!(positions.borrow()[1], Offset::new(0.0, 200.0));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testMatchFoundUpdatedPromptly
/// A seekable transition's `AnimatedContent` going to a state whose content has the other
/// content's key finds the match within a few frames, and loses it in the frame it goes to a
/// state without the key.
#[test]
fn match_found_updated_promptly() {
    let set: Rc<RefCell<Vec<SharedContentState>>> = Rc::default();
    let seekable = SeekableTransitionState::new(1);
    let target = mutable_state_of(1);
    let first_frame = Rc::new(Cell::new(true));
    let (st, sk, t, ff) = (set.clone(), seekable.clone(), target.handle(), first_frame.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let seeking = sk.clone();
        let goal = t.get();
        LaunchedEffect(goal, async move {
            if seeking.current_state() != goal {
                seeking.animate_to(goal).await;
            }
        });
        let transition = remember_transition(&sk, None);
        let (st, ff) = (st.clone(), ff.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let state1 = scope.remember_shared_content_state("key1");
            let state2 = scope.remember_shared_content_state("key1");
            let (st, ff, scope) = (st.clone(), ff.clone(), scope.clone());
            transition.AnimatedContent(
                Modifier,
                |_| default_content_transform(),
                Alignment::TOP_START,
                ContentKey::state(),
                move |content: AnimatedContentScope, it: i32| {
                    let tracked = |state: &SharedContentState, size: i32| {
                        let (st, state) = (st.clone(), state.clone());
                        Box!(
                            modifier = scope.shared_element(&state, &content).size(size.dp()),
                            move |_| {
                                let (st, state) = (st.clone(), state.clone());
                                DisposableEffect((), move |effect| {
                                    st.borrow_mut().push(state.clone());
                                    let (st, state) = (st.clone(), state.clone());
                                    effect.on_dispose(move || {
                                        st.borrow_mut().retain(|it| *it != state);
                                    })
                                });
                            }
                        );
                    };
                    match it {
                        1 => tracked(&state1, 200),
                        2 => tracked(&state2, 600),
                        _ => {
                            let ff = ff.clone();
                            Box!(modifier = Modifier.size(200.dp()), move |_| {
                                if ff.get() {
                                    ff.set(false);
                                }
                            });
                        }
                    }
                },
            );
        });
    });
    ui.wait_for_idle();
    assert_eq!(set.borrow().len(), 1);
    assert!(set.borrow().iter().all(|it| !it.is_match_found()));

    // The second content comes in, matching the first.
    target.set(2);
    let tracked = set.clone();
    ui.wait_until(move |_| tracked.borrow().len() == 2);
    for _ in 0..5 {
        ui.advance_time_by_frame();
    }
    assert_eq!(set.borrow().len(), 2);
    assert!(set.borrow().iter().all(|it| it.is_match_found()));
    target.set(3);
    let first = first_frame.clone();
    ui.wait_until(move |_| !first.get());
    for it in set.borrow().iter() {
        assert_eq!(set.borrow().len(), 2);
        assert!(!it.is_match_found());
    }
    ui.advance_time_by(std::time::Duration::from_millis(50000));
}

/// Two `AnimatedVisibility`s with no enter or exit, the first shown while `visible`, the second
/// while not, each a full-size `shared_bounds` of key "test" fading in and out over 100 ms,
/// holding a half-wide box: `colors.0` at the start, `colors.1` at the end. Their transitions go
/// in `transitions`, the first's first.
fn half_boxes(
    scope: &SharedTransitionScope,
    visible: bool,
    colors: (Color, Color),
    transitions: &Rc<RefCell<Vec<Option<Transition<EnterExitState>>>>>,
) {
    for (at, shown, color, align) in [
        (0, visible, colors.0, Alignment::CENTER_START),
        (1, !visible, colors.1, Alignment::CENTER_END),
    ] {
        let (scope, transitions) = (scope.clone(), transitions.clone());
        AnimatedVisibility!(
            shown,
            enter = EnterTransition::NONE,
            exit = ExitTransition::NONE,
            move |visibility: AnimatedVisibilityScope| {
                transitions.borrow_mut()[at] = Some(visibility.transition().clone());
                let state = scope.remember_shared_content_state("test");
                let options = SharedOptions {
                    enter: fade_in_with(tween(100).easing(LINEAR_EASING), 0.0),
                    exit: fade_out_with(tween(100).easing(LINEAR_EASING), 0.0),
                    ..Default::default()
                };
                Box!(
                    modifier =
                        scope.shared_bounds_with(&state, &visibility, options).fill_max_size(),
                    move |inner| {
                        Box!(
                            modifier = Modifier
                                .fill_max_height()
                                .fill_max_width_with(0.5)
                                .background(color)
                                .align(inner, align)
                        );
                    }
                );
            }
        );
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testObserverScopeClearedAfterDisposing
/// A `SharedTransitionLayout` removed right after its contents swapped leaves without failing.
#[test]
fn observer_scope_cleared_after_disposing() {
    let visible = mutable_state_of(false);
    let should_dispose = mutable_state_of(false);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let transitions = Rc::new(RefCell::new(vec![None, None]));
    let (v, d, sc) = (visible.handle(), should_dispose.handle(), seen.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        if d.get() {
            return;
        }
        let (sc, transitions) = (sc.clone(), transitions.clone());
        SharedTransitionLayout!(
            modifier = Modifier.required_size(100.dp()).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
                *sc.borrow_mut() = Some(scope.clone());
                half_boxes(&scope, v.get(), (Color::RED, Color::BLUE), &transitions);
            }
        );
    });
    ui.wait_for_idle();
    assert!(!seen.borrow().clone().expect("the scope").is_transition_active());
    visible.set(true);
    ui.wait_for_idle();
    should_dispose.set(true);
    ui.wait_for_idle();
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testParentSharedContentState
/// A shared element inside shared bounds has their state as its parent, in both contents, and
/// all four are matched through the transition.
#[test]
fn parent_shared_content_state() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let states: Rc<RefCell<Vec<Option<SharedContentState>>>> =
        Rc::new(RefCell::new(vec![None, None, None, None]));
    let (first, second) = (states.clone(), states.clone());
    let mut ui = two_visibilities(
        visible.handle(),
        seen.clone(),
        Modifier.fill_max_size_with(0.5).offset(25.dp(), 25.dp()),
        move |scope, visibility| {
            let parent = scope.remember_shared_content_state("parent");
            first.borrow_mut()[0] = Some(parent.clone());
            let (s, v, states) = (scope.clone(), visibility.clone(), first.clone());
            Box!(
                modifier = scope.shared_bounds(&parent, visibility).fill_max_size(),
                content_alignment = Alignment::CENTER,
                move |_| {
                    let child = s.remember_shared_content_state("child");
                    states.borrow_mut()[1] = Some(child.clone());
                    Box!(
                        modifier =
                            Modifier.fill_max_size_with(0.5).then(s.shared_element(&child, &v))
                    );
                }
            );
        },
        move |scope, visibility| {
            let parent = scope.remember_shared_content_state("parent");
            second.borrow_mut()[2] = Some(parent.clone());
            let (s, v, states) = (scope.clone(), visibility.clone(), second.clone());
            Box!(
                modifier =
                    Modifier.offset(20.dp(), 0.dp()).then(scope.shared_bounds(&parent, visibility)),
                move |_| {
                    let child = s.remember_shared_content_state("child");
                    states.borrow_mut()[3] = Some(child.clone());
                    Box!(
                        modifier = Modifier
                            .offset((-20).dp(), 0.dp())
                            .then(s.shared_element(&child, &v))
                            .fill_max_size()
                    );
                }
            );
        },
    );
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    start_transition(&mut ui, &visible, &scope);
    // Another frame, for the drawing.
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    let mut frames = 0;
    while scope.is_transition_active() {
        let states = states.borrow();
        let state = |at: usize| states[at].clone().expect("composed");
        assert_eq!(state(1).parent_shared_content_state(), Some(state(0)));
        assert_eq!(state(3).parent_shared_content_state(), Some(state(2)));
        for at in 0..4 {
            assert!(state(at).is_match_found(), "{at}");
        }
        drop(states);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
}

/// The x and y scales from `it`'s parent to it.
fn scales_from_parent(it: &LayoutCoordinates) -> (f32, f32) {
    let mut matrix = compose_ui::Matrix::default();
    it.transform_from(&it.parent_layout_coordinates().expect("a parent"), &mut matrix);
    (matrix.values[compose_ui::Matrix::SCALE_X], matrix.values[compose_ui::Matrix::SCALE_Y])
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testScaleTransitionInterruption
/// Shared bounds scaled to fit between a detail and a lazy list's item scale gradually both ways,
/// and, interrupted for another item, from where they were.
#[test]
fn scale_transition_interruption() {
    let show_list = mutable_state_of(false);
    let selected = mutable_state_of(1usize);
    let scales: Rc<RefCell<Vec<(f32, f32)>>> = Rc::new(RefCell::new(vec![(0.0, 0.0); 5]));
    let detail = Rc::new(Cell::new((0.0f32, 0.0f32)));
    let (sl, sel, sc, dt) = (show_list.handle(), selected.handle(), scales.clone(), detail.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (sc, dt) = (sc.clone(), dt.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let (sc, dt) = (sc.clone(), dt.clone());
            AnimatedContent!(
                sl.get(),
                transition_spec = |_| fade_in().together_with(fade_out()).using(None),
                move |content: AnimatedContentScope, it: bool| {
                    let options = SharedOptions {
                        bounds_transform: linear(160),
                        resize_mode: ResizeMode::scale_to_bounds(
                            ContentScale::Fit,
                            Alignment::CENTER,
                        ),
                        ..Default::default()
                    };
                    if it {
                        let (scope, content, sc) = (scope.clone(), content.clone(), sc.clone());
                        LazyColumn!(move |list: LazyListScope| {
                            for id in 0..5usize {
                                let (scope, content, sc, options) =
                                    (scope.clone(), content.clone(), sc.clone(), options.clone());
                                list.item(move || {
                                    let (scope, content, sc, options) = (
                                        scope.clone(),
                                        content.clone(),
                                        sc.clone(),
                                        options.clone(),
                                    );
                                    Box!(modifier = Modifier.fill_max_width(), move |_| {
                                        let state = scope.remember_shared_content_state(id);
                                        let sc = sc.clone();
                                        let color =
                                            if sel.get() == id { Color::RED } else { Color::WHITE };
                                        Box!(
                                            modifier = scope
                                                .shared_bounds_with(
                                                    &state,
                                                    &content,
                                                    options.clone()
                                                )
                                                .size((200.dp(), 50.dp()))
                                                .background(color)
                                                .on_placed(move |it: LayoutCoordinates| {
                                                    sc.borrow_mut()[id] = scales_from_parent(&it);
                                                })
                                        );
                                    });
                                });
                            }
                        });
                    } else {
                        let (scope, dt) = (scope.clone(), dt.clone());
                        Box!(modifier = Modifier.fill_max_size(), move |_| {
                            let state = scope.remember_shared_content_state(sel.get());
                            let dt = dt.clone();
                            Box!(
                                modifier = scope
                                    .shared_bounds_with(&state, &content, options.clone())
                                    .size(100.dp())
                                    .on_placed(move |it: LayoutCoordinates| {
                                        dt.set(scales_from_parent(&it));
                                    })
                            );
                        });
                    }
                }
            );
        });
    });
    ui.wait_for_idle();
    assert_eq!(detail.get(), (1.0, 1.0));
    ui.set_auto_advance(false);
    // Detail to list to detail, interrupted.
    show_list.set(true);
    let mut frames = 0;
    while detail.get().0 == 1.0 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "the detail scales");
    }
    let list_x = |at: usize| scales.borrow()[at].0;
    let mut last_detail = detail.get().0;
    let mut last_list = list_x(selected.get());
    for frame in 0..6 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        let at = selected.get();
        let (x, y) = scales.borrow()[at];
        assert_eq!(x, y, "at {frame}");
        assert_eq!(detail.get().0, detail.get().1, "at {frame}");
        // The list's item shrinks gradually, the detail grows.
        let shrunk = last_list - x;
        assert!(shrunk > 0.05 && shrunk < 0.2, "{shrunk} at {frame}");
        last_list = x;
        let grown = detail.get().0 - last_detail;
        assert!(grown > 0.05 && grown < 0.2, "{grown} at {frame}");
        last_detail = detail.get().0;
    }
    // Interrupted.
    selected.set(3);
    show_list.set(false);
    let mut frames = 0;
    while list_x(3) == 1.0 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "the item scales");
    }
    let mut last_list = list_x(3);
    let mut last_detail = detail.get().0;
    for frame in 0..6 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        let (x, y) = scales.borrow()[3];
        assert_eq!(x, y, "at {frame}");
        assert_eq!(detail.get().0, detail.get().1, "at {frame}");
        let grown = x - last_list;
        assert!(grown > 0.05 && grown < 0.2, "{grown} at {frame}");
        last_list = x;
        let shrunk = last_detail - detail.get().0;
        assert!(shrunk > 0.05 && shrunk < 0.2, "{shrunk} at {frame}");
        last_detail = detail.get().0;
    }
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(detail.get(), (1.0, 1.0));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testFoundMatchBeforeSharedContentComposed
/// While an `AnimatedContent` swaps two shared bounds of a key, each finds the match once the
/// other is composed, not before: the entering one reads no match in its first composition.
#[test]
fn found_match_before_shared_content_composed() {
    let is_square = mutable_state_of(true);
    let box2_composed = Rc::new(Cell::new(false));
    let checked = Rc::new(Cell::new(0));
    let (sq, b2, ch) = (is_square.handle(), box2_composed.clone(), checked.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (b2, ch) = (b2.clone(), ch.clone());
        SharedTransitionLayout!(
            modifier = Modifier.background(Color::WHITE).padding(20.dp()),
            move |scope: SharedTransitionScope| {
                let (b2, ch) = (b2.clone(), ch.clone());
                AnimatedContent!(
                    sq.get(),
                    transition_spec = |_| {
                        EnterTransition::NONE.together_with(ExitTransition::NONE).using(None)
                    },
                    move |content: AnimatedContentScope, square: bool| {
                        let transition = content.transition();
                        let settled = transition.current_state() == transition.target_state();
                        let state = scope.remember_shared_content_state("test");
                        ch.set(ch.get() + 1);
                        if square {
                            if settled {
                                assert!(!state.is_match_found());
                            } else {
                                assert_eq!(b2.get(), state.is_match_found());
                            }
                            Box!(
                                modifier = scope
                                    .shared_bounds(&state, &content)
                                    .size(80.dp())
                                    .background(Color::RED)
                            );
                        } else {
                            if !settled {
                                // No match when made; one the next frame.
                                assert_eq!(b2.get(), state.is_match_found());
                            }
                            b2.set(true);
                            Box!(
                                modifier = scope
                                    .shared_bounds(&state, &content)
                                    .size((40.dp(), 160.dp()))
                                    .background(Color::GRAY)
                            );
                        }
                    }
                );
            }
        );
    });
    is_square.set(false);
    ui.wait_for_idle();
    assert!(checked.get() > 2, "{}", checked.get());
}

/// Four colors, as the scrolling tests'.
const COLORS: [Color; 4] = [
    Color::from_argb(0xffff6f69),
    Color::from_argb(0xffffcc5c),
    Color::from_argb(0xff2a9d84),
    Color::from_argb(0xff264653),
];

/// The list screen or an item's details, as the scrolling tests' `Screen`.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Screen {
    List,
    Details(usize),
}

impl Screen {
    fn key(self) -> u32 {
        match self {
            Screen::List => 1,
            Screen::Details(_) => 2,
        }
    }
}

/// Scrolls `state` to `index` at once, as `runBlocking { scrollToItem(index) }`, and lets the
/// layout follow.
fn scroll_to_item(ui: &mut ComposeTestRule, state: LazyListState, index: usize) {
    let scope = CoroutineScope::new();
    scope.launch(async move {
        let _ = state.scroll_to_item(index, 0).await;
    });
    ui.wait_for_idle();
}

/// A slide by `by` of the width, with the default spec.
fn slide_in(by: i32) -> EnterTransition {
    let spec = AnimationSpec::Spring(
        spring_with(Spring::DAMPING_RATIO_NO_BOUNCY, Spring::STIFFNESS_MEDIUM_LOW)
            .visibility_threshold(compose_ui::IntOffset::VISIBILITY_THRESHOLD),
    );
    slide_in_horizontally_with(spec, move |it| by * it)
}

/// A slide out by `by` of the width, with the default spec.
fn slide_out(by: i32) -> ExitTransition {
    let spec = AnimationSpec::Spring(
        spring_with(Spring::DAMPING_RATIO_NO_BOUNCY, Spring::STIFFNESS_MEDIUM_LOW)
            .visibility_threshold(compose_ui::IntOffset::VISIBILITY_THRESHOLD),
    );
    slide_out_horizontally_with(spec, move |it| by * it)
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testRandomScrollingWithInterruption
/// A list's item image shared with its details, the transition interrupted and the list
/// scrolled about while it runs: nothing fails.
#[test]
fn random_scrolling_with_interruption() {
    let screen = mutable_state_of(Screen::List);
    let state = LazyListState::new(0, 0);
    let (s, list_state) = (screen.handle(), *state);
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        SharedTransitionLayout!(
            modifier = Modifier.fill_max_width().height(800.dp()),
            move |scope: SharedTransitionScope| {
                AnimatedContent!(
                    s.get(),
                    content_key = ContentKey::of(|it: &Screen| it.key()),
                    transition_spec = |change: AnimatedContentTransitionScope<Screen>| {
                        if *change.initial_state() == Screen::List {
                            (slide_in(-1) + fade_in()).together_with(slide_out(1) + fade_out())
                        } else {
                            (slide_in(1) + fade_in()).together_with(slide_out(-1) + fade_out())
                        }
                    },
                    move |content: AnimatedContentScope, it: Screen| match it {
                        Screen::List => {
                            let scope = scope.clone();
                            LazyColumn!(state = list_state, move |list: LazyListScope| {
                                let (scope, content) = (scope.clone(), content.clone());
                                list.items(50, move |item| {
                                    let state = scope
                                        .remember_shared_content_state(format!("item-image{item}"));
                                    let (scope, content) = (scope.clone(), content.clone());
                                    Row!(modifier = Modifier.fill_max_width(), move |_| {
                                        Box!(
                                            modifier = Modifier
                                                .size(100.dp())
                                                .then(scope.shared_element(&state, &content))
                                                .background(COLORS[item % 4])
                                        );
                                        Spacer(Modifier.size(15.dp()));
                                    });
                                });
                            });
                        }
                        Screen::Details(item) => {
                            let scope = scope.clone();
                            Column!(modifier = Modifier.fill_max_size(), move |_| {
                                let state = scope
                                    .remember_shared_content_state(format!("item-image{item}"));
                                Box!(
                                    modifier = scope
                                        .shared_element(&state, &content)
                                        .background(COLORS[item % 4])
                                        .fill_max_width()
                                );
                            });
                        }
                    }
                );
            }
        );
    });
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    screen.set(Screen::Details(5));
    for _ in 0..3 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    screen.set(Screen::List);
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    for _ in 0..3 {
        for it in 0..5 {
            scroll_to_item(&mut ui, *state, 40 - it * 10);
        }
        ui.advance_time_by_frame();
        for it in 0..10 {
            scroll_to_item(&mut ui, *state, it * 5);
        }
        ui.advance_time_by_frame();
        scroll_to_item(&mut ui, *state, 0);
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, vigorouslyScrollingSharedElementsInLazyList
/// Items of a list, each in its own `AnimatedVisibility`, shared with a details screen, the list
/// scrolled to random items while the transition back runs: nothing fails.
#[test]
fn vigorously_scrolling_shared_elements_in_lazy_list() {
    let screen = mutable_state_of(Screen::List);
    let state = LazyListState::new(0, 0);
    let (s, list_state) = (screen.handle(), *state);
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        SharedTransitionLayout!(
            modifier = Modifier.fill_max_size(),
            move |scope: SharedTransitionScope| {
                let in_list = scope.clone();
                LazyColumn!(state = list_state, move |list: LazyListScope| {
                    let scope = in_list.clone();
                    list.items(50, move |item| {
                        let scope = scope.clone();
                        AnimatedVisibility!(
                            s.get() != Screen::Details(item),
                            move |visibility: AnimatedVisibilityScope| {
                                let state = scope
                                    .remember_shared_content_state(format!("item-image{item}"));
                                let scope = scope.clone();
                                Row!(modifier = Modifier.fill_max_width(), move |_| {
                                    Box!(
                                        modifier = Modifier
                                            .size(100.dp())
                                            .then(scope.shared_element(&state, &visibility))
                                            .background(COLORS[item % 4])
                                    );
                                    Spacer(Modifier.size(15.dp()));
                                });
                            }
                        );
                    });
                });
                let scope = scope.clone();
                AnimatedVisibility!(
                    matches!(s.get(), Screen::Details(_)),
                    move |visibility: AnimatedVisibilityScope| {
                        let item = remember(|| mutable_state_of(None::<usize>));
                        if let Screen::Details(shown) = s.get() {
                            item.set(Some(shown));
                        }
                        let shown = item.get();
                        let scope = scope.clone();
                        Column!(modifier = Modifier.fill_max_size(), move |_| {
                            let state = scope.remember_shared_content_state(format!(
                                "item-image{}",
                                shown.map_or("null".to_string(), |it| it.to_string())
                            ));
                            Box!(
                                modifier = scope
                                    .shared_element(&state, &visibility)
                                    .fill_max_width()
                                    .background(COLORS[shown.expect("an item") % 4])
                            );
                        });
                    }
                );
            }
        );
    });
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    screen.set(Screen::Details(5));
    for _ in 0..5 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    screen.set(Screen::List);
    for _ in 0..3 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    for it in 0..5 {
        scroll_to_item(&mut ui, *state, it + 1);
        ui.advance_time_by_frame();
    }
    // A fixed sequence for Compose's random items below 20.
    let mut seed = 17u32;
    for _ in 0..20 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let id = ((seed >> 16) % 20) as usize;
        scroll_to_item(&mut ui, *state, id);
        ui.advance_time_by_frame();
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testPlaceholderLogicSkippedWhenNoMatch
/// Shared bounds with no match are the size of their content, whatever an approach inside them
/// does: no placeholder logic.
#[test]
fn placeholder_logic_skipped_when_no_match() {
    let parent_size: Rc<Cell<Option<IntSize>>> = Rc::default();
    let test_size = mutable_state_of(IntSize::ZERO);
    let (ps, ts) = (parent_size.clone(), test_size.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let ps = ps.clone();
        AnimatedVisibility!(true, move |visibility: AnimatedVisibilityScope| {
            let ps = ps.clone();
            SharedTransitionLayout!(move |scope: SharedTransitionScope| {
                let state = scope.remember_shared_content_state("test");
                let ps = ps.clone();
                Box!(
                    modifier = Modifier
                        .on_size_changed(move |it: IntSize| ps.set(Some(it)))
                        .then(scope.shared_bounds(&state, &visibility)),
                    move |_| {
                        Box!(
                            modifier = Modifier
                                .approach_layout(
                                    |_| true,
                                    |_, _| false,
                                    move |scope, measurable, constraints| {
                                        let placeable = scope.measure(measurable, constraints);
                                        let size = ts.get();
                                        scope.layout_with(size.width, size.height, move |at| {
                                            at.place(&placeable, 0, 0)
                                        })
                                    },
                                )
                                .required_size((40.dp(), 40.dp()))
                        );
                    }
                );
            });
        });
    });
    ui.wait_for_idle();
    assert_eq!(parent_size.get(), Some(test_size.get()));
    for size in [IntSize::new(20, 25), IntSize::new(35, 10)] {
        test_size.set(size);
        ui.wait_for_idle();
        assert_eq!(parent_size.get(), Some(size));
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testUserModifierInSharedTransitionLayout
/// The layout's own modifier moves it: its coordinates are where the offset puts it.
#[test]
fn user_modifier_in_shared_transition_layout() {
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let sc = seen.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let sc = sc.clone();
        SharedTransitionLayout!(
            modifier = Modifier.offset_with(|_| compose_ui::IntOffset::new(65, 536)),
            move |scope: SharedTransitionScope| {
                *sc.borrow_mut() = Some(scope.clone());
                Box!(modifier = Modifier.fill_max_size());
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    let root = scope.root_for_test().expect("placed");
    assert_eq!(root.position_in_window(), Offset::new(65.0, 536.0));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testNoAdditionalPlacementWhenNoMatch
/// A lazy list of shared elements with no match places its items as often as the same list
/// without them, scrolled alike.
#[test]
#[ignore = "compose-ui doesn't tell a lazy item moved by a scroll `on_placed`, where Compose \
            tells each placement; the shared items are told, through their approach node \
            reading where it is (missing.md)"]
fn no_additional_placement_when_no_match() {
    let state = LazyListState::new(0, 0);
    let control = LazyListState::new(0, 0);
    let placements = Rc::new(RefCell::new(vec![0u32; 20]));
    let control_placements = Rc::new(RefCell::new(vec![0u32; 20]));
    let (s, c, p, cp) = (*state, *control, placements.clone(), control_placements.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (p, cp) = (p.clone(), cp.clone());
        Row!(move |_| {
            let p = p.clone();
            AnimatedVisibility!(true, move |visibility: AnimatedVisibilityScope| {
                let p = p.clone();
                SharedTransitionLayout!(move |scope: SharedTransitionScope| {
                    let (p, scope, visibility) = (p.clone(), scope.clone(), visibility.clone());
                    LazyColumn!(
                        modifier = Modifier.size(100.dp()),
                        state = s,
                        move |list: LazyListScope| {
                            for id in 0..20usize {
                                let (p, scope, visibility) =
                                    (p.clone(), scope.clone(), visibility.clone());
                                list.item_keyed(id, move || {
                                    let state = scope.remember_shared_content_state(id.to_string());
                                    let p = p.clone();
                                    Box!(
                                        modifier = scope
                                            .shared_element(&state, &visibility)
                                            .on_placed(move |_: LayoutCoordinates| {
                                                p.borrow_mut()[id] += 1;
                                            })
                                            .size(10.dp())
                                            .background(Color::BLACK)
                                    );
                                });
                            }
                        }
                    );
                });
            });
            let cp = cp.clone();
            LazyColumn!(
                modifier = Modifier.size(100.dp()),
                state = c,
                move |list: LazyListScope| {
                    for id in 0..20usize {
                        let cp = cp.clone();
                        list.item_keyed(id, move || {
                            let cp = cp.clone();
                            Box!(
                                modifier = Modifier
                                    .on_placed(move |_: LayoutCoordinates| cp.borrow_mut()[id] += 1)
                                    .size(10.dp())
                                    .background(Color::BLACK)
                            );
                        });
                    }
                }
            );
        });
    });
    ui.wait_for_idle();
    for _ in 0..20 {
        let scope = CoroutineScope::new();
        let (s, c) = (*state, *control);
        scope.launch(async move {
            let _ = s.scroll_by(5.0).await;
            let _ = c.scroll_by(5.0).await;
        });
        // One after the other, as Compose's `runBlocking`.
        ui.wait_for_idle();
    }
    assert_eq!(*control_placements.borrow(), *placements.borrow());
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testLookaheadPositionInSkipToLookaheadSize
/// A layout inside shared bounds sizing itself otherwise in the approach than ahead doesn't move
/// the lookahead placement: ahead, it's at its parent's origin.
#[test]
fn lookahead_position_in_skip_to_lookahead_size() {
    let target = mutable_state_of(true);
    let target_positions: Rc<RefCell<Vec<Option<Offset>>>> = Rc::default();
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (t, tp, sc) = (target.handle(), target_positions.clone(), seen.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (tp, sc) = (tp.clone(), sc.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            *sc.borrow_mut() = Some(scope.clone());
            let tp = tp.clone();
            AnimatedContent!(
                t.get(),
                transition_spec = |_| EnterTransition::NONE.together_with(ExitTransition::NONE),
                move |content: AnimatedContentScope, state: bool| {
                    let state_now = scope.remember_shared_content_state("test");
                    let tp = tp.clone();
                    let noting = Modifier.layout(move |measure, measurable, constraints| {
                        let placeable = measure.measure(measurable, constraints);
                        let ahead = measure.is_looking_ahead();
                        let (w, h) =
                            if ahead { (placeable.width, placeable.height) } else { (200, 300) };
                        let tp = tp.clone();
                        measure.layout_with(w, h, move |placement| {
                            if ahead && state == t.get() && !t.get() {
                                // Sized otherwise in the approach, it's still placed ahead at
                                // its parent's origin.
                                let at = placement.coordinates().map(|it| it.position_in_parent());
                                tp.borrow_mut().insert(0, at);
                            }
                            placement.place(&placeable, 0, 0);
                        })
                    });
                    Box!(
                        modifier = Modifier
                            .size_in(compose_ui::SizeSpec {
                                min_width: compose_ui::Dp::UNSPECIFIED,
                                min_height: compose_ui::Dp::UNSPECIFIED,
                                max_width: 400.dp(),
                                max_height: 400.dp(),
                                enforce_incoming: false,
                            })
                            .then(scope.shared_bounds(&state_now, &content))
                            .then(noting)
                            .background(Color::RED),
                        |_| {
                            Box!(modifier = Modifier.fill_max_size().background(Color::BLACK));
                        }
                    );
                }
            );
        });
    });
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    target.set(false);
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(scope.is_transition_active());
    for at in target_positions.borrow().iter().flatten() {
        assert_eq!(*at, Offset::ZERO);
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testScaleToBoundsCaching
/// `scale_to_bounds` of the same content scale and alignment is the same resize mode, and of
/// another one another. (Compose caches the objects for identity; here they're values. Its
/// custom scales and alignments, never cached, have no counterpart: both are enums.)
#[test]
fn scale_to_bounds_caching() {
    let alignments = [
        Alignment::TOP_START,
        Alignment::TOP_CENTER,
        Alignment::TOP_END,
        Alignment::CENTER_START,
        Alignment::CENTER,
        Alignment::CENTER_END,
        Alignment::BOTTOM_START,
        Alignment::BOTTOM_CENTER,
        Alignment::BOTTOM_END,
    ];
    let content_scales = [
        ContentScale::FillWidth,
        ContentScale::FillHeight,
        ContentScale::FillBounds,
        ContentScale::Fit,
        ContentScale::Crop,
        ContentScale::None,
        ContentScale::Inside,
    ];
    let mut previous: Option<ResizeMode> = None;
    for alignment in alignments {
        for content_scale in content_scales {
            let mode = ResizeMode::scale_to_bounds(content_scale, alignment);
            assert!(mode == ResizeMode::scale_to_bounds(content_scale, alignment));
            assert!(previous.as_ref() != Some(&mode));
            previous = Some(mode);
        }
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, foundMatchedElementButNeverMeasured
/// Shared bounds composed in a slot its layout never measures, matched across an
/// `AnimatedContent`'s change: nothing fails.
#[test]
fn found_matched_element_but_never_measured() {
    let target = mutable_state_of(true);
    let t = target.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let scope = scope.clone();
            AnimatedContent!(t.get(), move |content: AnimatedContentScope, _: bool| {
                let (scope, content) = (scope.clone(), content.clone());
                SubcomposeLayout!(
                    measure_policy =
                        compose_ui::subcompose::subcompose_policy(move |measure, _| {
                            let (scope, content) = (scope.clone(), content.clone());
                            measure.subcompose(
                                0,
                                Callback::new(move || {
                                    let state = scope.remember_shared_content_state("test");
                                    Box!(
                                        modifier = scope
                                            .shared_bounds(&state, &content)
                                            .size(200.dp())
                                            .background(Color::RED)
                                    );
                                }),
                            );
                            // Measures nothing.
                            measure.layout(200, 200)
                        })
                );
                Box!(modifier = Modifier.size(200.dp()).background(Color::BLACK));
            });
        });
    });
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    target.set(!target.get());
    ui.advance_time_by_frame();
    ui.wait_for_idle();
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, intrinsicsQueryComingFromAboveLookaheadRoot
/// A layout above a `SharedTransitionLayout` asking its intrinsic width gets its content's,
/// through `skip_to_lookahead_size`.
#[test]
fn intrinsics_query_coming_from_above_lookahead_root() {
    let intrinsic_width = Rc::new(Cell::new(0));
    let iw = intrinsic_width.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let iw = iw.clone();
        compose_ui::Layout(
            Modifier,
            compose_ui::measure_policy(move |scope, measurables, constraints| {
                let measurable = measurables[0];
                iw.set(scope.max_intrinsic_width(measurable, constraints.max_height));
                let placeable = scope.measure(measurable, constraints);
                scope.place(&placeable, 0, 0);
                scope.layout(constraints.max_width, constraints.max_height)
            }),
            Callback::new(|| {
                SharedTransitionLayout!(move |scope: SharedTransitionScope| {
                    Box!(modifier = scope.skip_to_lookahead_size().size(100.dp()));
                });
            }),
        );
    });
    ui.wait_for_idle();
    assert_eq!(intrinsic_width.get(), 100);
}

/// Places the content at `offset` under a motion frame of reference, as the test's
/// `offsetWithMFR`.
fn offset_with_mfr(offset: compose_ui::IntOffset) -> Modifier {
    Modifier.layout(move |scope, measurable, constraints| {
        let placeable = scope.measure(measurable, constraints);
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            placement.with_motion_frame_of_reference_placement(|placement| {
                placement.place(&placeable, offset.x, offset.y);
            });
        })
    })
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, SharedElementWithStructuralChangesAmidAnimation
/// Shared bounds whose target's alignment changes amid their animation turn towards the new
/// target and end there, both ways.
#[test]
fn shared_element_with_structural_changes_amid_animation() {
    let select_first = mutable_state_of(true);
    let alignment = mutable_state_of(Alignment::TOP_START);
    let position: Rc<Cell<Option<Offset>>> = Rc::default();
    let first_position: Rc<Cell<Option<Offset>>> = Rc::default();
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (sf, al, p, fp, sc) = (
        select_first.handle(),
        alignment.handle(),
        position.clone(),
        first_position.clone(),
        seen.clone(),
    );
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let key = remember(|| "key");
        let (p, fp, sc) = (p.clone(), fp.clone(), sc.clone());
        SharedTransitionLayout!(
            modifier = Modifier.size(400.dp()),
            move |scope: SharedTransitionScope| {
                *sc.borrow_mut() = Some(scope.clone());
                let (p, fp) = (p.clone(), fp.clone());
                AnimatedContent!(sf.get(), move |content: AnimatedContentScope, first: bool| {
                    let state = scope.remember_shared_content_state(key);
                    if !first {
                        let (scope, content, p) = (scope.clone(), content.clone(), p.clone());
                        Box!(modifier = Modifier.fill_max_size(), move |inner| {
                            let p = p.clone();
                            Box!(
                                modifier = Modifier
                                    .align(inner, al.get())
                                    .then(scope.shared_bounds(&state, &content))
                                    .background(Color::RED)
                                    .on_globally_positioned(move |it: LayoutCoordinates| {
                                        p.set(Some(it.position_in_root()))
                                    })
                                    .size(100.dp()),
                                |_| {
                                    BasicText!("false");
                                }
                            );
                        });
                    } else {
                        let fp = fp.clone();
                        Box!(
                            modifier = offset_with_mfr(compose_ui::IntOffset::new(10, 180))
                                .then(scope.shared_bounds(&state, &content))
                                .on_globally_positioned(move |it: LayoutCoordinates| {
                                    fp.set(Some(it.position_in_root()))
                                })
                                .alpha(0.5)
                                .background(Color::BLUE)
                                .size(180.dp()),
                            |_| {
                                BasicText!("true");
                            }
                        );
                    }
                });
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    ui.set_auto_advance(false);
    select_first.set(false);
    for _ in 0..3 {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
    }
    assert!(position.get().is_some());
    assert!(scope.is_transition_active());
    let last = position.get().unwrap();
    alignment.set(Alignment::BOTTOM_CENTER);
    // Five frames, for the spring's overshoot.
    for _ in 0..5 {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
    }
    // Turned towards the bottom center.
    let now = position.get().unwrap();
    assert!(now.y > last.y && now.x > last.x, "{now:?} after {last:?}");
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(position.get().unwrap().round(), compose_ui::IntOffset::new(150, 300));
    // The other way, the alignment changed during the transition.
    ui.set_auto_advance(false);
    first_position.set(None);
    select_first.set(true);
    for _ in 0..3 {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
    }
    assert!(first_position.get().is_some());
    assert!(scope.is_transition_active());
    alignment.set(Alignment::TOP_START);
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(first_position.get().unwrap().round(), compose_ui::IntOffset::new(10, 180));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, TestSharedElementWithFixedMfrOffset
/// Two shared bounds whose motion frame of reference and structural offsets differ but add up
/// to the same place stay there through the transition and its interruption.
#[test]
fn shared_element_with_fixed_mfr_offset() {
    let select_first = mutable_state_of(true);
    let position1: Rc<Cell<Option<Offset>>> = Rc::default();
    let position2: Rc<Cell<Option<Offset>>> = Rc::default();
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (sf, p1, p2, sc) =
        (select_first.handle(), position1.clone(), position2.clone(), seen.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let key = remember(|| "key");
        let (p1, p2, sc) = (p1.clone(), p2.clone(), sc.clone());
        SharedTransitionLayout!(
            modifier = Modifier.fill_max_size(),
            move |scope: SharedTransitionScope| {
                *sc.borrow_mut() = Some(scope.clone());
                let (p1, p2) = (p1.clone(), p2.clone());
                AnimatedContent!(
                    sf.get(),
                    transition_spec = |_| {
                        EnterTransition::NONE.together_with(ExitTransition::NONE).using(None)
                    },
                    move |content: AnimatedContentScope, first: bool| {
                        let state = scope.remember_shared_content_state(key);
                        if !first {
                            let p1 = p1.clone();
                            Box!(
                                modifier = offset_with_mfr(compose_ui::IntOffset::new(80, 80))
                                    .then(scope.shared_bounds(&state, &content))
                                    .on_globally_positioned(move |it: LayoutCoordinates| {
                                        p1.set(Some(it.position_in_root()))
                                    })
                                    .background(Color::RED)
                                    .size(100.dp()),
                                |_| {
                                    BasicText!("false");
                                }
                            );
                        } else {
                            let p2 = p2.clone();
                            Box!(
                                modifier = Modifier
                                    .offset_with(|_| compose_ui::IntOffset::new(180, 180))
                                    .then(offset_with_mfr(compose_ui::IntOffset::new(-100, -100)))
                                    .then(scope.shared_bounds(&state, &content))
                                    .on_globally_positioned(move |it: LayoutCoordinates| {
                                        p2.set(Some(it.position_in_root()))
                                    })
                                    .alpha(0.5)
                                    .background(Color::BLUE)
                                    .size(180.dp()),
                                |_| {
                                    BasicText!("true");
                                }
                            );
                        }
                    }
                );
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    ui.set_auto_advance(false);
    select_first.set(false);
    let mut frames = 0;
    while !scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "the transition starts");
    }
    let at = Some(Offset::new(80.0, 80.0));
    assert_eq!(position1.get(), at);
    assert_eq!(position2.get(), at);
    // Interrupted.
    select_first.set(true);
    while !scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    while scope.is_transition_active() {
        assert_eq!(position1.get(), at);
        assert_eq!(position2.get(), at);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, SharedElementWithChangingMfrOffset
/// A shared element in a lazy list scrolled during its animation follows each scroll at once;
/// going back with the list scrolled nearly to the target, it stays within a pixel of it.
#[test]
fn shared_element_with_changing_mfr_offset() {
    let target = mutable_state_of(true);
    let position1: Rc<Cell<Option<Offset>>> = Rc::default();
    let position2: Rc<Cell<Option<Offset>>> = Rc::default();
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let state = LazyListState::new(0, 0);
    let (t, p1, p2, sc, list_state) =
        (target.handle(), position1.clone(), position2.clone(), seen.clone(), *state);
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let key = remember(|| "key");
        let (p1, p2, sc) = (p1.clone(), p2.clone(), sc.clone());
        SharedTransitionLayout!(
            modifier = Modifier.fill_max_size(),
            move |scope: SharedTransitionScope| {
                *sc.borrow_mut() = Some(scope.clone());
                let (p1, p2) = (p1.clone(), p2.clone());
                AnimatedContent!(
                    t.get(),
                    modifier = Modifier.fill_max_size(),
                    transition_spec = |_| {
                        EnterTransition::NONE.together_with(ExitTransition::NONE).using(None)
                    },
                    move |content: AnimatedContentScope, target: bool| {
                        if target {
                            let (scope, content, p1) = (scope.clone(), content.clone(), p1.clone());
                            Box!(move |_| {
                                let state = scope.remember_shared_content_state(key);
                                let p1 = p1.clone();
                                // After the offsets, the target is at (40, 80).
                                Box!(
                                    modifier = offset_with_mfr(compose_ui::IntOffset::new(80, 80))
                                        .offset((-40).dp(), 0.dp())
                                        .then(scope.shared_element(&state, &content))
                                        .on_globally_positioned(move |it: LayoutCoordinates| {
                                            p1.set(Some(it.position_in_root()))
                                        })
                                        .background(Color::RED)
                                        .size(300.dp()),
                                    |_| {
                                        BasicText!("false");
                                    }
                                );
                            });
                        } else {
                            let (scope, content, p2) = (scope.clone(), content.clone(), p2.clone());
                            let gray = || {
                                Box!(
                                    modifier = Modifier
                                        .size(100.dp())
                                        .padding(10.dp())
                                        .background(Color::GRAY)
                                );
                            };
                            LazyColumn!(
                                modifier =
                                    Modifier.padding(PaddingValues::symmetric(40.dp(), 80.dp())),
                                state = list_state,
                                move |list: LazyListScope| {
                                    for _ in 0..5 {
                                        list.item(gray);
                                    }
                                    // Item 5.
                                    let (scope, content, p2) =
                                        (scope.clone(), content.clone(), p2.clone());
                                    list.item(move || {
                                        let state = scope.remember_shared_content_state(key);
                                        let p2 = p2.clone();
                                        Box!(
                                            modifier = scope
                                                .shared_element(&state, &content)
                                                .on_globally_positioned(
                                                    move |it: LayoutCoordinates| {
                                                        p2.set(Some(it.position_in_root()))
                                                    },
                                                )
                                                .background(Color::BLUE)
                                                .size(100.dp()),
                                            |_| {
                                                BasicText!("true");
                                            }
                                        );
                                    });
                                    for _ in 0..30 {
                                        list.item(gray);
                                    }
                                }
                            );
                        }
                    }
                );
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    ui.set_auto_advance(false);
    target.set(false);
    ui.wait_for_idle();
    let mut frames = 0;
    while !scope.is_transition_active() {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        frames += 1;
        assert!(frames < 100, "the transition starts");
    }
    let before = position2.get().expect("placed");
    let scroll = |ui: &mut ComposeTestRule, by: f32| {
        let coroutines = CoroutineScope::new();
        coroutines.launch(async move {
            let _ = list_state.scroll_by(by).await;
        });
        ui.wait_for_idle();
        // Compose measures a scroll at once (`forceRemeasure`), its clock paused; here the
        // layout waits for a frame: one at the same time, which moves no animation.
        ui.host().frame();
    };
    for it in 0..4 {
        scroll(&mut ui, 100.0);
        let expected = Offset::new(before.x, before.y - 100.0 * (it + 1) as f32);
        assert_eq!(position2.get(), Some(expected), "after {} scrolls", it + 1);
    }
    scroll(&mut ui, 99.0);
    assert_eq!(position2.get(), Some(Offset::new(before.x, before.y - 499.0)));
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    // Back, with the list scrolled nearly to the target.
    ui.set_auto_advance(false);
    target.set(true);
    while !scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    let near = |position: Option<Offset>| {
        let position = position.expect("placed");
        assert_eq!(position.x, 40.0);
        assert!(position.y >= 80.0 && position.y <= 81.0, "{position:?}");
    };
    near(position1.get());
    // Never out of that pixel: no jump.
    while scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        near(position1.get());
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
}

/// Enabled only while its transition goes to `false`, keeping an animation running on or not,
/// as the one-direction tests' configs.
struct TowardsFalse {
    transition: Transition<bool>,
    keep_for_ongoing_animation: bool,
}

impl SharedContentConfig for TowardsFalse {
    fn should_keep_enabled_for_ongoing_animation(&self) -> bool {
        self.keep_for_ongoing_animation
    }

    fn is_enabled(&self, _state: &SharedContentState) -> bool {
        !self.transition.target_state()
    }
}

/// What the one-direction tests record of each content: sizes and offsets in the layout.
#[derive(Clone, Default)]
struct Recorded {
    sizes: [Rc<RefCell<Vec<IntSize>>>; 2],
    offsets: [Rc<RefCell<Vec<Offset>>>; 2],
}

impl Recorded {
    fn clear(&self) {
        for at in 0..2 {
            self.sizes[at].borrow_mut().clear();
            self.offsets[at].borrow_mut().clear();
        }
    }

    fn noting(&self, scope: &SharedTransitionScope, at: usize) -> Modifier {
        let (scope, recorded) = (scope.clone(), self.clone());
        Modifier.on_placed(move |it: LayoutCoordinates| {
            recorded.sizes[at].borrow_mut().push(it.size());
            if let Some(root) = scope.lookahead_root_for_test() {
                recorded.offsets[at].borrow_mut().push(root.local_position_of(&it, Offset::ZERO));
            }
        })
    }
}

/// An `AnimatedContent` of `target`'s transition with no enter or exit: a shared element of
/// 20 × 20 while `true`, one of 180 × 180 at (20, 20) while `false`, both over a 160 ms tween.
/// With `both`, both contents are enabled only towards `false`, an animation running not kept;
/// otherwise the second only, kept.
fn one_direction(
    target: MutableState<bool>,
    both: bool,
) -> (ComposeTestRule, Rc<RefCell<Option<Transition<bool>>>>, Recorded) {
    let recorded = Recorded::default();
    let created: Rc<RefCell<Option<Transition<bool>>>> = Rc::default();
    let (given, made) = (recorded.clone(), created.clone());
    let ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (recorded, made) = (given.clone(), made.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let transition = update_transition(target.get(), None);
            *made.borrow_mut() = Some(transition.clone());
            let config: Rc<dyn SharedContentConfig> = {
                let transition = transition.clone();
                remember(move || {
                    Rc::new(TowardsFalse {
                        transition: transition.clone(),
                        keep_for_ongoing_animation: !both,
                    }) as Rc<dyn SharedContentConfig>
                })
            };
            let (recorded, scope) = (recorded.clone(), scope.clone());
            transition.AnimatedContent(
                Modifier,
                |_| EnterTransition::NONE.together_with(ExitTransition::NONE).using(None),
                Alignment::TOP_START,
                ContentKey::state(),
                move |content: AnimatedContentScope, target_state: bool| {
                    let (recorded, scope, config) =
                        (recorded.clone(), scope.clone(), config.clone());
                    Box!(modifier = Modifier.size(200.dp()), move |_| {
                        let options =
                            SharedOptions { bounds_transform: tween_of(160), ..Default::default() };
                        if target_state {
                            let state = if both {
                                scope.remember_shared_content_state_with("test", config.clone())
                            } else {
                                scope.remember_shared_content_state("test")
                            };
                            Box!(
                                modifier = scope
                                    .shared_element_with(&state, &content, options)
                                    .then(recorded.noting(&scope, 0))
                                    .size(20.dp())
                            );
                        } else {
                            let state =
                                scope.remember_shared_content_state_with("test", config.clone());
                            Box!(
                                modifier = Modifier
                                    .offset(20.dp(), 20.dp())
                                    .then(scope.shared_element_with(&state, &content, options))
                                    .then(recorded.noting(&scope, 1))
                                    .size(180.dp())
                            );
                        }
                    });
                },
            );
        });
    });
    (ui, created, recorded)
}

/// A tween of `millis` with the default easing, for bounds.
fn tween_of(millis: u32) -> BoundsTransform {
    BoundsTransform::new(move |_, _| tween(millis).into())
}

/// Advances frames until `done` says so.
fn frames_until(ui: &mut ComposeTestRule, mut done: impl FnMut() -> bool) {
    let mut frames = 0;
    while !done() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "it happens");
    }
}

/// Checks the first content took the animation mid-way, in size and offset.
fn mid_way(recorded: &Recorded) {
    let size = *recorded.sizes[0].borrow().last().expect("placed");
    assert!(size.width > 20 && size.width < 180, "width {}", size.width);
    assert!(size.height > 20 && size.height < 180, "height {}", size.height);
    let offset = *recorded.offsets[0].borrow().last().expect("placed");
    assert!(offset.x > 0.0 && offset.x < 20.0, "x {}", offset.x);
    assert!(offset.y > 0.0 && offset.y < 20.0, "y {}", offset.y);
}

/// Checks nothing animated: each content kept its size and place.
fn unanimated(recorded: &Recorded) {
    assert!(recorded.sizes[0].borrow().iter().all(|it| *it == IntSize::new(20, 20)));
    assert!(recorded.sizes[1].borrow().iter().all(|it| *it == IntSize::new(180, 180)));
    assert!(recorded.offsets[0].borrow().iter().all(|it| *it == Offset::ZERO));
    assert!(recorded.offsets[1].borrow().iter().all(|it| *it == Offset::new(20.0, 20.0)));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testIsEnabledOnlyInOneDirection
/// A shared element enabled only going to `false` animates that way, growing and moving
/// steadily, and not back.
#[test]
fn is_enabled_only_in_one_direction() {
    let target = mutable_state_of(true);
    let (mut ui, created, recorded) = one_direction(target.handle(), false);
    let current = |created: &Rc<RefCell<Option<Transition<bool>>>>| {
        created.borrow().as_ref().expect("composed").current_state()
    };
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    target.set(false);
    frames_until(&mut ui, || !current(&created));
    let sizes = recorded.sizes[1].borrow().clone();
    for pair in sizes.windows(2) {
        assert!(pair[1].width >= pair[0].width && pair[1].height >= pair[0].height, "{sizes:?}");
    }
    assert_eq!(sizes.first(), Some(&IntSize::new(20, 20)));
    assert_eq!(sizes.last(), Some(&IntSize::new(180, 180)));
    let offsets = recorded.offsets[1].borrow().clone();
    for pair in offsets.windows(2) {
        assert!(pair[1].x >= pair[0].x && pair[1].y >= pair[0].y, "{offsets:?}");
    }
    assert_eq!(offsets.first(), Some(&Offset::ZERO));
    assert_eq!(offsets.last(), Some(&Offset::new(20.0, 20.0)));
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    // Back: no animation.
    ui.set_auto_advance(false);
    recorded.clear();
    target.set(true);
    frames_until(&mut ui, || current(&created));
    unanimated(&recorded);
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testIsEnabledOnlyInOneDirectionWithInterruption
/// Enabled only going to `false` and interrupted mid-way, the way back keeps the animation
/// running (`should_keep_enabled_for_ongoing_animation`): the second content shrinks steadily,
/// moving.
#[test]
fn is_enabled_only_in_one_direction_with_interruption() {
    let target = mutable_state_of(true);
    let (mut ui, created, recorded) = one_direction(target.handle(), false);
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    target.set(false);
    let sizes = recorded.sizes[0].clone();
    frames_until(&mut ui, || sizes.borrow().last().expect("placed").width != 20);
    for _ in 0..3 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    mid_way(&recorded);
    ui.wait_for_idle();
    recorded.clear();
    target.set(true);
    frames_until(&mut ui, || {
        let transition = created.borrow().clone().expect("composed");
        transition.current_state() && transition.target_state()
    });
    let sizes = recorded.sizes[1].borrow().clone();
    for pair in sizes.windows(2) {
        assert!(pair[0].width >= pair[1].width && pair[0].height >= pair[1].height, "{sizes:?}");
    }
    assert!(distinct(&recorded.offsets[1].borrow()) > 2);
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testIsEnabledOnlyInOneDirectionExplicitlyNoInterruptionHandling
/// Enabled only going to `false`, an animation running not kept: interrupted mid-way, the way
/// back doesn't animate.
#[test]
fn is_enabled_only_in_one_direction_explicitly_no_interruption_handling() {
    let target = mutable_state_of(true);
    let (mut ui, created, recorded) = one_direction(target.handle(), true);
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    target.set(false);
    let sizes = recorded.sizes[0].clone();
    frames_until(&mut ui, || sizes.borrow().last().expect("placed").width != 20);
    for _ in 0..3 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
    mid_way(&recorded);
    ui.wait_for_idle();
    recorded.clear();
    target.set(true);
    frames_until(&mut ui, || {
        let transition = created.borrow().clone().expect("composed");
        transition.current_state() && transition.target_state()
    });
    unanimated(&recorded);
}

/// The alternative-target tests' config of the `false` content: enabled while `enabled` (always
/// without), keeping an animation running on as `keep` says, going to `alternative` once its
/// visible content is removed.
struct Alternative {
    enabled: Option<MutableState<bool>>,
    keep: bool,
    alternative: Option<compose_ui::Rect>,
}

impl SharedContentConfig for Alternative {
    fn should_keep_enabled_for_ongoing_animation(&self) -> bool {
        self.keep
    }

    fn is_enabled(&self, _state: &SharedContentState) -> bool {
        self.enabled.is_none_or(|it| it.get())
    }

    fn alternative_target_bounds_in_transition_scope_after_removal(
        &self,
        _state: &SharedContentState,
        _target_bounds_before_removal: compose_ui::Rect,
        _shared_transition_layout_size: compose_ui::Size,
    ) -> Option<compose_ui::Rect> {
        self.alternative
    }
}

/// `target`'s transition's `AnimatedContent` fading over 2 s: a shared element of 20 × 20 while
/// `true`; while `false`, a 200 × 200 box, then, unless `removed`, a shared element of 180 × 180
/// at (20, 20) with `config`, both over a 160 ms tween.
fn alternative_target(
    target: MutableState<bool>,
    removed: MutableState<bool>,
    config: impl Fn() -> Alternative + 'static,
) -> (ComposeTestRule, Rc<RefCell<Option<Transition<bool>>>>, Recorded) {
    let recorded = Recorded::default();
    let created: Rc<RefCell<Option<Transition<bool>>>> = Rc::default();
    let (given, made, config) = (recorded.clone(), created.clone(), Rc::new(config));
    let ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (recorded, made, config) = (given.clone(), made.clone(), config.clone());
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let transition = update_transition(target.get(), None);
            *made.borrow_mut() = Some(transition.clone());
            let config = config.clone();
            let config: Rc<dyn SharedContentConfig> =
                remember(move || Rc::new(config()) as Rc<dyn SharedContentConfig>);
            let (recorded, scope) = (recorded.clone(), scope.clone());
            transition.AnimatedContent(
                Modifier,
                |_| {
                    fade_in_with(tween(2000), 0.0)
                        .together_with(fade_out_with(tween(2000), 0.0))
                        .using(None)
                },
                Alignment::TOP_START,
                ContentKey::state(),
                move |content: AnimatedContentScope, target_state: bool| {
                    let (recorded, scope, config) =
                        (recorded.clone(), scope.clone(), config.clone());
                    Box!(modifier = Modifier.size(200.dp()), move |_| {
                        let options =
                            SharedOptions { bounds_transform: tween_of(160), ..Default::default() };
                        if target_state {
                            let state = scope.remember_shared_content_state("test");
                            Box!(
                                modifier = scope
                                    .shared_element_with(&state, &content, options)
                                    .then(recorded.noting(&scope, 0))
                                    .size(20.dp())
                            );
                        } else {
                            Box!(modifier = Modifier.size(200.dp()));
                            if !removed.get() {
                                let state = scope
                                    .remember_shared_content_state_with("test", config.clone());
                                Box!(
                                    modifier = Modifier
                                        .offset(20.dp(), 20.dp())
                                        .then(scope.shared_element_with(&state, &content, options))
                                        .then(recorded.noting(&scope, 1))
                                        .size(180.dp())
                                );
                            }
                        }
                    });
                },
            );
        });
    });
    (ui, created, recorded)
}

/// Goes `target` to `to` and advances until the first content's size moves off `from_width`,
/// then three frames more.
fn into_the_animation(
    ui: &mut ComposeTestRule,
    recorded: &Recorded,
    target: &OwnedState<bool>,
    to: bool,
    at: usize,
    from_width: i32,
) {
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    target.set(to);
    let sizes = recorded.sizes[at].clone();
    frames_until(ui, || sizes.borrow().last().expect("placed").width != from_width);
    for _ in 0..3 {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, removeTargetWithoutExplicitAlternativeTarget
/// The visible content removed mid-way with no alternative target: the animation ends, the
/// other content back at its size and place.
#[test]
fn remove_target_without_explicit_alternative_target() {
    let target = mutable_state_of(true);
    let removed = mutable_state_of(false);
    let (mut ui, created, recorded) = alternative_target(target.handle(), removed.handle(), || {
        Alternative { enabled: None, keep: true, alternative: None }
    });
    into_the_animation(&mut ui, &recorded, &target, false, 0, 20);
    mid_way(&recorded);
    ui.wait_for_idle();
    recorded.clear();
    removed.set(true);
    frames_until(&mut ui, || !created.borrow().as_ref().expect("composed").current_state());
    assert!(recorded.sizes[0].borrow().iter().all(|it| *it == IntSize::new(20, 20)));
    assert!(recorded.offsets[0].borrow().iter().all(|it| *it == Offset::ZERO));
    assert!(recorded.sizes[1].borrow().is_empty());
    assert!(recorded.offsets[1].borrow().is_empty());
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, disableIgnoreAnimationWithAlternativeTargetOnDisabledContent
/// The content with an alternative target disabled (an animation running not kept) as the
/// transition turns back: no animation, though the target has an alternative.
#[test]
fn disable_ignore_animation_with_alternative_target_on_disabled_content() {
    let target = mutable_state_of(false);
    let enable_target = mutable_state_of(true);
    let removed = mutable_state_of(false);
    let enabled = enable_target.handle();
    let (mut ui, created, recorded) =
        alternative_target(target.handle(), removed.handle(), move || Alternative {
            enabled: Some(enabled),
            keep: false,
            alternative: Some(compose_ui::Rect::new(30.0, 40.0, 80.0, 100.0)),
        });
    into_the_animation(&mut ui, &recorded, &target, true, 1, 180);
    mid_way(&recorded);
    ui.wait_for_idle();
    recorded.clear();
    enable_target.set(false);
    target.set(false);
    frames_until(&mut ui, || {
        let transition = created.borrow().clone().expect("composed");
        !transition.target_state() && !transition.current_state()
    });
    unanimated(&recorded);
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testRemoveTargetWithAlternativeTargetSpecified
/// The visible content removed mid-way with an alternative target: the other content animates
/// on to it, at (30, 40), 50 × 60.
#[test]
fn remove_target_with_alternative_target_specified() {
    let target = mutable_state_of(true);
    let removed = mutable_state_of(false);
    let (mut ui, created, recorded) =
        alternative_target(target.handle(), removed.handle(), || Alternative {
            enabled: None,
            keep: true,
            alternative: Some(compose_ui::Rect::new(30.0, 40.0, 80.0, 100.0)),
        });
    into_the_animation(&mut ui, &recorded, &target, false, 0, 20);
    mid_way(&recorded);
    ui.wait_for_idle();
    recorded.clear();
    removed.set(true);
    frames_until(&mut ui, || !created.borrow().as_ref().expect("composed").current_state());
    assert!(recorded.sizes[1].borrow().is_empty());
    assert!(recorded.offsets[1].borrow().is_empty());
    assert_eq!(recorded.sizes[0].borrow().last(), Some(&IntSize::new(50, 60)));
    assert_eq!(recorded.offsets[0].borrow().last(), Some(&Offset::new(30.0, 40.0)));
    assert!(distinct(&recorded.offsets[0].borrow()) > 2);
    assert!(distinct(&recorded.sizes[0].borrow()) > 2);
}

/// How many distinct values `values` holds, as Kotlin's `distinct().size`.
fn distinct<T: PartialEq>(values: &[T]) -> usize {
    let mut seen: Vec<&T> = Vec::new();
    for value in values {
        if !seen.contains(&value) {
            seen.push(value);
        }
    }
    seen.len()
}
