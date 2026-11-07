//! Shared elements drawn by Skia, as Compose's `SharedTransitionTest` checks their pixels on a
//! device: what each content shows while the bounds animate, the overlay and its clip.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose::animation::*;
use compose::foundation::layout::*;
use compose::foundation::shape::CIRCLE_SHAPE;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Alignment, ClickableOptions, Color, ContentScale, Indication, IntSize, LayoutCoordinates,
    Modifier, MutableInteractionSource, UnitExt,
};

const TAG: &str = "scope";

/// A test of a 100 × 100 white `SharedTransitionLayout` tagged "scope", with `content`.
fn scope_rule(
    seen: &Rc<RefCell<Option<SharedTransitionScope>>>,
    content: impl Fn(&SharedTransitionScope) + 'static,
) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    let (seen, content) = (seen.clone(), Rc::new(content));
    ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let (seen, content) = (seen.clone(), content.clone());
        SharedTransitionLayout!(
            modifier = Modifier.test_tag(TAG).required_size(100.dp()).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
                *seen.borrow_mut() = Some(scope.clone());
                content(&scope);
            }
        );
    })
}

/// Turns auto-advance off, runs `change` and advances frames until the transition starts.
fn start(ui: &mut ComposeTestRule, scope: &SharedTransitionScope, change: impl FnOnce()) {
    ui.set_auto_advance(false);
    change();
    let mut frames = 0;
    while !scope.is_transition_active() {
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 100, "the transition starts");
    }
}

/// Runs `check` on the scope's pixels each frame until the transition ends.
fn each_frame(
    ui: &mut ComposeTestRule,
    scope: &SharedTransitionScope,
    mut check: impl FnMut(&mut ComposeTestRule, &CapturedImage),
) {
    let mut frames = 0;
    while scope.is_transition_active() {
        let image = ui.on_node_with_tag(TAG).capture_to_image();
        check(ui, &image);
        ui.wait_for_idle();
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
    assert!(frames > 1, "{frames} frames");
}

/// The fraction of 100 ms `transition` played.
fn fraction(transition: &Option<Transition<EnterExitState>>) -> f32 {
    let millis = transition.as_ref().expect("composed").play_time_nanos() / 1_000_000;
    (millis as f32 / 100.0).clamp(0.0, 1.0)
}

/// Two `AnimatedVisibility`s with no enter or exit, the first shown while `visible`, the second
/// while not, each a full-size `shared_bounds` of key "test" fading in and out over 100 ms,
/// holding a half-wide box: red at the start, blue at the end. Their transitions go in
/// `transitions`.
fn half_boxes(
    scope: &SharedTransitionScope,
    visible: bool,
    transitions: &Rc<RefCell<Vec<Option<Transition<EnterExitState>>>>>,
) {
    for (at, shown, color, align) in [
        (0, visible, Color::RED, Alignment::CENTER_START),
        (1, !visible, Color::BLUE, Alignment::CENTER_END),
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

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testBothContentShowing
/// Shared bounds show both contents through the transition, the entering one fading in as the
/// leaving one fades out, each at its own transition's play time.
#[test]
fn both_content_showing() {
    let visible = mutable_state_of(false);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let transitions = Rc::new(RefCell::new(vec![None, None]));
    let (v, given) = (visible.handle(), transitions.clone());
    let mut ui = scope_rule(&seen, move |scope| half_boxes(scope, v.get(), &given));
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    start(&mut ui, &scope, || visible.set(true));
    each_frame(&mut ui, &scope, |_, image| {
        let (enter, exit) = {
            let transitions = transitions.borrow();
            (fraction(&transitions[0]), fraction(&transitions[1]))
        };
        let width = image.width();
        image.assert_pixels(None, |at| {
            if at.x < width / 2 {
                Some(Color::RED.with_alpha(enter).composite_over(Color::WHITE))
            } else if at.x > width / 2 {
                Some(Color::BLUE.with_alpha(1.0 - exit).composite_over(Color::WHITE))
            } else {
                None
            }
        });
    });
}

/// A full-size shared element of key "test" in each of two `AnimatedVisibility`s, holding a
/// half-wide box: red at the start in the first, shown while `visible` and fading over 100 ms,
/// blue at the end in the second, shown while not, with no enter or exit.
fn half_elements(scope: &SharedTransitionScope, visible: bool) {
    let fade = tween(100).easing(LINEAR_EASING);
    for (shown, color, align, enter, exit) in [
        (
            visible,
            Color::RED,
            Alignment::CENTER_START,
            fade_in_with(fade.clone(), 0.0),
            fade_out_with(fade.clone(), 0.0),
        ),
        (!visible, Color::BLUE, Alignment::CENTER_END, EnterTransition::NONE, ExitTransition::NONE),
    ] {
        let scope = scope.clone();
        AnimatedVisibility!(
            shown,
            enter = enter,
            exit = exit,
            move |v: AnimatedVisibilityScope| {
                let state = scope.remember_shared_content_state("test");
                Box!(modifier = scope.shared_element(&state, &v).fill_max_size(), move |inner| {
                    Box!(
                        modifier = Modifier
                            .fill_max_height()
                            .fill_max_width_with(0.5)
                            .background(color)
                            .align(inner, align)
                    );
                });
            }
        );
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testOnlyVisibleContentShowingInSharedElement
/// A shared element shows only its visible content: the leaving one at first, then, through the
/// transition, the entering one, opaque in the overlay though its visibility fades it in.
#[test]
fn only_visible_content_showing_in_shared_element() {
    let visible = mutable_state_of(false);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let v = visible.handle();
    let mut ui = scope_rule(&seen, move |scope| half_elements(scope, v.get()));
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    let halves = |image: &CapturedImage, start: Color, end: Color| {
        let width = image.width();
        image.assert_pixels(None, |at| {
            if at.x < width / 2 - 2 {
                Some(start)
            } else if at.x > width / 2 + 2 {
                Some(end)
            } else {
                None
            }
        });
    };
    halves(&ui.on_node_with_tag(TAG).capture_to_image(), Color::WHITE, Color::BLUE);
    start(&mut ui, &scope, || visible.set(true));
    each_frame(&mut ui, &scope, |_, image| halves(image, Color::RED, Color::WHITE));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testOverlayClip
/// A shared element clipped by a circle in the overlay draws within the circle of its size each
/// frame, as its bounds animate.
#[test]
fn overlay_clip() {
    let visible = mutable_state_of(true);
    let size: Rc<RefCell<Option<IntSize>>> = Rc::default();
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let (v, noted) = (visible.handle(), size.clone());
    let mut ui = scope_rule(&seen, move |scope| {
        let transform = BoundsTransform::new(|_, _| tween(100).easing(LINEAR_EASING).into());
        let s1 = scope.clone();
        let t1 = transform.clone();
        AnimatedVisibility!(
            v.get(),
            enter = EnterTransition::NONE,
            exit = ExitTransition::NONE,
            move |visibility: AnimatedVisibilityScope| {
                let state = s1.remember_shared_content_state("child");
                let options = SharedOptions { bounds_transform: t1.clone(), ..Default::default() };
                Box!(
                    modifier = s1.shared_element_with(&state, &visibility, options).fill_max_size()
                );
            }
        );
        let (s2, t2, noted) = (scope.clone(), transform.clone(), noted.clone());
        AnimatedVisibility!(
            !v.get(),
            modifier = Modifier.fill_max_size(),
            enter = fade_in_with(tween(100), 0.0),
            exit = ExitTransition::NONE,
            move |visibility: AnimatedVisibilityScope| {
                let state = s2.remember_shared_content_state("child");
                let options = SharedOptions {
                    clip_in_overlay_during_transition: OverlayClip::of_shape(CIRCLE_SHAPE),
                    bounds_transform: t2.clone(),
                    ..Default::default()
                };
                let noted = noted.clone();
                Box!(
                    modifier = Modifier
                        .fill_max_size_with(0.5)
                        .then(s2.shared_element_with(&state, &visibility, options))
                        .on_globally_positioned(move |it: LayoutCoordinates| {
                            *noted.borrow_mut() = Some(it.size())
                        })
                        .background(Color::BLUE)
                );
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(None, |_| Some(Color::WHITE));
    start(&mut ui, &scope, || visible.set(false));
    // Five pixels aside for anti-aliasing.
    let tolerance = 4.0;
    each_frame(&mut ui, &scope, |_, image| {
        let radius = size.borrow().expect("placed").width as f32 / 2.0;
        let (width, height) = (image.width(), image.height());
        image.assert_pixels(None, |at| {
            let (dx, dy) = ((at.x - width / 2) as f32, (at.y - height / 2) as f32);
            let distance = (dx * dx + dy * dy).sqrt();
            if at.x < width / 2 && distance < radius - tolerance {
                Some(Color::BLUE)
            } else if distance > radius + tolerance {
                Some(Color::WHITE)
            } else {
                None
            }
        });
    });
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testOverlayClipInheritedByChildren
/// A shared element inside shared bounds clipped by a circle in the overlay is clipped by its
/// parent's circle too (`OverlayClip`'s default, the parent's path).
#[test]
fn overlay_clip_inherited_by_children() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let v = visible.handle();
    let mut ui = scope_rule(&seen, move |scope| {
        for (shown, enter, clip, color) in [
            (v.get(), EnterTransition::NONE, None, None),
            (!v.get(), fade_in_with(tween(100), 0.0), Some(CIRCLE_SHAPE), Some(Color::BLUE)),
        ] {
            let scope = scope.clone();
            AnimatedVisibility!(
                shown,
                enter = enter,
                exit = ExitTransition::NONE,
                move |visibility: AnimatedVisibilityScope| {
                    let state = scope.remember_shared_content_state("test");
                    let mut options = SharedOptions::default();
                    if let Some(shape) = clip.clone() {
                        options.clip_in_overlay_during_transition = OverlayClip::of_shape(shape);
                    }
                    let (inner_scope, visibility) = (scope.clone(), visibility.clone());
                    Box!(
                        modifier =
                            scope.shared_bounds_with(&state, &visibility, options).fill_max_size(),
                        move |inner| {
                            let child = inner_scope.remember_shared_content_state("child");
                            let mut modifier = inner_scope
                                .shared_element(&child, &visibility)
                                .fill_max_height()
                                .fill_max_width_with(0.5)
                                .align(inner, Alignment::CENTER_START);
                            if let Some(color) = color {
                                modifier = modifier.background(color);
                            }
                            Box!(modifier = modifier);
                        }
                    );
                }
            );
        }
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(None, |_| Some(Color::WHITE));
    start(&mut ui, &scope, || visible.set(false));
    let tolerance = 5.0;
    each_frame(&mut ui, &scope, |_, image| {
        let (width, height) = (image.width(), image.height());
        let radius = width as f32 / 2.0;
        image.assert_pixels(None, |at| {
            let (dx, dy) = ((at.x - width / 2) as f32, (at.y - height / 2) as f32);
            let distance = (dx * dx + dy * dy).sqrt();
            if at.x < width / 2 && distance < radius - tolerance {
                Some(Color::BLUE)
            } else if at.x > width / 2 + 5 || distance > radius + tolerance {
                Some(Color::WHITE)
            } else {
                None
            }
        });
    });
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testRenderInOverlayEqualsFalse
/// A shared element not rendering in the overlay draws in place, so its visibility's fade shows.
#[test]
fn render_in_overlay_equals_false() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let exit: Rc<RefCell<Option<Transition<EnterExitState>>>> = Rc::default();
    let (v, entering) = (visible.handle(), exit.clone());
    let mut ui = scope_rule(&seen, move |scope| {
        let s1 = scope.clone();
        AnimatedVisibility!(
            v.get(),
            enter = EnterTransition::NONE,
            exit = ExitTransition::NONE,
            move |visibility: AnimatedVisibilityScope| {
                let state = s1.remember_shared_content_state("child");
                Box!(
                    modifier = s1
                        .shared_element(&state, &visibility)
                        .background(Color::RED)
                        .fill_max_size()
                );
            }
        );
        let (s2, entering) = (scope.clone(), entering.clone());
        AnimatedVisibility!(
            !v.get(),
            enter = fade_in_with(tween(100).easing(LINEAR_EASING), 0.0),
            exit = ExitTransition::NONE,
            move |visibility: AnimatedVisibilityScope| {
                *entering.borrow_mut() = Some(visibility.transition().clone());
                let state = s2.remember_shared_content_state("child");
                let options = SharedOptions {
                    render_in_overlay_during_transition: false,
                    ..Default::default()
                };
                Box!(
                    modifier = s2
                        .shared_element_with(&state, &visibility, options)
                        .background(Color::BLUE)
                        .fill_max_size()
                );
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(None, |_| Some(Color::RED));
    start(&mut ui, &scope, || visible.set(false));
    each_frame(&mut ui, &scope, |_, image| {
        let fraction = fraction(&exit.borrow());
        let expected = Color::BLUE.with_alpha(fraction).composite_over(Color::WHITE);
        image.assert_pixels(None, |_| Some(expected));
    });
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testRenderInOverlay
/// A box rendering in the overlay at z-index 1 draws over the shared bounds' content (in the
/// overlay at 0) during the transition; out of it, in place, under the content after it.
#[test]
fn render_in_overlay() {
    let visible = mutable_state_of(true);
    let seen: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::default();
    let v = visible.handle();
    let mut ui = scope_rule(&seen, move |scope| {
        let overlay = scope.clone();
        Box!(
            modifier = scope
                .render_in_shared_transition_scope_overlay_with(1.0, move || {
                    overlay.is_transition_active()
                })
                .background(Color::GREEN)
                .fill_max_size()
        );
        let s1 = scope.clone();
        AnimatedVisibility!(
            v.get(),
            enter = EnterTransition::NONE,
            exit = ExitTransition::NONE,
            move |visibility: AnimatedVisibilityScope| {
                let s1 = s1.clone();
                Box!(
                    modifier = Modifier
                        .fill_max_size()
                        .wrap_content_size_with(Alignment::BOTTOM_END, false),
                    move |_| {
                        let state = s1.remember_shared_content_state("child");
                        Box!(
                            modifier = s1
                                .shared_bounds(&state, &visibility)
                                .fill_max_size_with(0.5)
                                .background(Color::RED)
                        );
                    }
                );
            }
        );
        let s2 = scope.clone();
        AnimatedVisibility!(
            !v.get(),
            enter = fade_in_with(tween(100), 0.0),
            exit = ExitTransition::NONE,
            move |visibility: AnimatedVisibilityScope| {
                let state = s2.remember_shared_content_state("child");
                Box!(
                    modifier = s2
                        .shared_bounds(&state, &visibility)
                        .fill_max_size_with(0.5)
                        .background(Color::BLUE)
                );
            }
        );
    });
    ui.wait_for_idle();
    let scope = seen.borrow().clone().expect("the scope");
    assert!(!scope.is_transition_active());
    // Anti-aliasing.
    let tolerance = 2;
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (width, height) = (image.width(), image.height());
    image.assert_pixels(None, |at| {
        if at.x > width / 2 + tolerance && at.y > height / 2 + tolerance {
            Some(Color::RED)
        } else if at.x < width / 2 - tolerance || at.y < height / 2 - tolerance {
            Some(Color::GREEN)
        } else {
            None
        }
    });
    start(&mut ui, &scope, || visible.set(false));
    each_frame(&mut ui, &scope, |_, image| {
        image.assert_pixels(None, |_| Some(Color::GREEN));
    });
}

/// The visible content's transition, which `scale_rule` notes.
type Noted = Rc<RefCell<Option<Transition<EnterExitState>>>>;

/// An `AnimatedContent` of `is_square` with no enter or exit and no size transform, in a white
/// `SharedTransitionLayout` tagged "scope" with `padding`: shared bounds of key "test" scaled to
/// their bounds over a 100 ms linear tween, animating their placeholder size, a red 80 × 80 box
/// with `square` while square, a gray 40 × 160 one with `tall` while not.
fn scale_rule(
    is_square: MutableState<bool>,
    square: ResizeMode,
    tall: ResizeMode,
    padding: i32,
) -> (ComposeTestRule, Noted) {
    set_capture_renderer(compose_skia::raster_colors);
    let noted: Noted = Rc::default();
    let given = noted.clone();
    let ui = ComposeTestRule::sized(IntSize::new(300, 300), move || {
        let (noted, square, tall) = (given.clone(), square, tall);
        SharedTransitionLayout!(
            modifier = Modifier.test_tag(TAG).background(Color::WHITE).padding(padding.dp()),
            move |scope: SharedTransitionScope| {
                let (noted, square, tall) = (noted.clone(), square, tall);
                AnimatedContent!(
                    is_square.get(),
                    transition_spec = |_| {
                        EnterTransition::NONE.together_with(ExitTransition::NONE).using(None)
                    },
                    move |content: AnimatedContentScope, is_square: bool| {
                        if content.transition().target_state() == EnterExitState::Visible {
                            *noted.borrow_mut() = Some(content.transition().clone());
                        }
                        let state = scope.remember_shared_content_state("test");
                        let options = |resize_mode: &ResizeMode| SharedOptions {
                            enter: EnterTransition::NONE,
                            exit: ExitTransition::NONE,
                            resize_mode: *resize_mode,
                            bounds_transform: BoundsTransform::new(|_, _| {
                                tween(100).easing(LINEAR_EASING).into()
                            }),
                            placeholder_size: PlaceholderSize::animated_size(),
                            ..Default::default()
                        };
                        if is_square {
                            Box!(
                                modifier = scope
                                    .shared_bounds_with(&state, &content, options(&square))
                                    .size(80.dp())
                                    .background(Color::RED)
                            );
                        } else {
                            Box!(
                                modifier = scope
                                    .shared_bounds_with(&state, &content, options(&tall))
                                    .size((40.dp(), 160.dp()))
                                    .background(Color::GRAY)
                            );
                        }
                    }
                );
            }
        );
    });
    (ui, noted)
}

/// Sets `is_square` to `to` with the clock paused, waits for the visible content's transition to
/// start, then, each frame of it after its first, checks the capture is `from` × … → `to`'s size
/// plus `padding` on each side at its fraction, and runs `check` with the image.
fn each_scale_frame(
    ui: &mut ComposeTestRule,
    noted: &Noted,
    is_square: &OwnedState<bool>,
    to: bool,
    padding: i32,
    mut check: impl FnMut(&CapturedImage),
) {
    ui.set_auto_advance(false);
    is_square.set(to);
    let running = |noted: &Noted| {
        let transition = noted.borrow().clone().expect("composed");
        transition.target_state() != transition.current_state()
    };
    let mut frames = 0;
    while !running(noted) {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        frames += 1;
        assert!(frames < 100, "the transition starts");
    }
    let ((from_w, from_h), (to_w, to_h)) =
        if to { ((40.0, 160.0), (80.0, 80.0)) } else { ((80.0, 80.0), (40.0, 160.0)) };
    let mut checked = 0;
    while running(noted) {
        let millis = noted.borrow().as_ref().unwrap().play_time_nanos() / 1_000_000;
        let fraction = millis as f32 / 100.0;
        if fraction > 0.0 {
            let width = (1.0 - fraction) * from_w + fraction * to_w + 2.0 * padding as f32;
            let height = (1.0 - fraction) * from_h + fraction * to_h + 2.0 * padding as f32;
            let image = ui.on_node_with_tag(TAG).capture_to_image();
            assert_eq!(image.width(), width.round() as i32, "at {fraction}");
            assert_eq!(image.height(), height.round() as i32, "at {fraction}");
            check(&image);
            checked += 1;
        }
        ui.advance_time_by_frame();
        frames += 1;
        assert!(frames < 300, "the transition ends");
    }
    assert!(checked > 2, "{checked} frames checked");
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testScaleTransition
/// Shared bounds scaled to fit their bounds: the gray tall content in the middle at the bounds'
/// height, the red square over the bounds' width, centered.
#[test]
fn scale_transition() {
    let is_square = mutable_state_of(true);
    let fit = ResizeMode::scale_to_bounds(ContentScale::Fit, Alignment::CENTER);
    let (mut ui, noted) = scale_rule(is_square.handle(), fit, fit, 0);
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(None, |_| Some(Color::RED));
    // Anti-aliasing.
    let tolerance = 2;
    each_scale_frame(&mut ui, &noted, &is_square, false, 0, |image| {
        let (width, height) = (image.width(), image.height());
        image.assert_pixels(None, |at| {
            let gray_min = width / 2 - height / 8;
            let gray_max = width / 2 + height / 8;
            if at.x > gray_min + tolerance && at.x < gray_max - tolerance {
                Some(Color::GRAY)
            } else if at.x < gray_min - tolerance || at.x > gray_max + tolerance {
                // Red or white, by the height.
                let (y, w, h) = (at.y as f32, width as f32, height as f32);
                let t = tolerance as f32;
                (y > h / 2.0 - w / 2.0 + t && y < h / 2.0 + w / 2.0 - t).then_some(Color::RED)
            } else {
                None
            }
        });
    });
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testScaleTransitionFillHeightAndFillWidth
/// The square filling the bounds' height and the tall content their width: the tall one covers
/// the bounds both ways, the square too on the way back.
#[test]
fn scale_transition_fill_height_and_fill_width() {
    let is_square = mutable_state_of(true);
    let (mut ui, noted) = scale_rule(
        is_square.handle(),
        ResizeMode::scale_to_bounds(ContentScale::FillHeight, Alignment::CENTER),
        ResizeMode::scale_to_bounds(ContentScale::FillWidth, Alignment::CENTER),
        0,
    );
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(None, |_| Some(Color::RED));
    each_scale_frame(&mut ui, &noted, &is_square, false, 0, |image| {
        image.assert_pixels(None, |_| Some(Color::GRAY));
    });
    ui.wait_for_idle();
    each_scale_frame(&mut ui, &noted, &is_square, true, 0, |image| {
        image.assert_pixels(None, |_| Some(Color::RED));
    });
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testScaleTransitionAlignment
/// Scaled to fit, the square aligned to the top start and the tall content to the bottom start.
#[test]
fn scale_transition_alignment() {
    let is_square = mutable_state_of(true);
    let (mut ui, noted) = scale_rule(
        is_square.handle(),
        ResizeMode::scale_to_bounds(ContentScale::Fit, Alignment::TOP_START),
        ResizeMode::scale_to_bounds(ContentScale::Fit, Alignment::BOTTOM_START),
        0,
    );
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(None, |_| Some(Color::RED));
    let tolerance = 2;
    each_scale_frame(&mut ui, &noted, &is_square, false, 0, |image| {
        let (width, height) = (image.width(), image.height());
        image.assert_pixels(None, |at| {
            let gray_max = height / 4;
            if at.x >= 0 && at.x < gray_max - tolerance {
                Some(Color::GRAY)
            } else if at.x > gray_max + tolerance {
                // Red or white, by the height.
                if at.y > tolerance && at.y < width - tolerance {
                    Some(Color::RED)
                } else if at.y > width + tolerance {
                    Some(Color::WHITE)
                } else {
                    None
                }
            } else {
                None
            }
        });
    });
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testScaleTransitionCrop
/// The square cropped to the bounds, inside the layout's padding; the tall content filling the
/// width, not clipped, drawing in the padding above and below.
#[test]
fn scale_transition_crop() {
    let is_square = mutable_state_of(true);
    let (mut ui, noted) = scale_rule(
        is_square.handle(),
        ResizeMode::scale_to_bounds(ContentScale::Crop, Alignment::CENTER),
        ResizeMode::scale_to_bounds(ContentScale::FillWidth, Alignment::CENTER),
        20,
    );
    ui.wait_for_idle();
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (width, height) = (image.width(), image.height());
    image.assert_pixels(None, |at| {
        if at.x > 20 && at.x < width - 20 && at.y > 20 && at.y < height - 20 {
            Some(Color::RED)
        } else if at.x < 20 || at.x > width - 20 || (at.y < 20 && at.y > height - 20) {
            Some(Color::WHITE)
        } else {
            None
        }
    });
    // Gray, filling the width, draws in the padding: it doesn't crop.
    let gray_or_white = |at: compose::ui::IntOffset, width: i32, height: i32| {
        let vertical_twice = (2 * at.y - height).abs();
        if vertical_twice > (width - 40) * 4 {
            Some(Color::WHITE)
        } else if vertical_twice < (width - 40) * 4 {
            Some(Color::GRAY)
        } else {
            None
        }
    };
    each_scale_frame(&mut ui, &noted, &is_square, false, 20, |image| {
        let (width, height) = (image.width(), image.height());
        image.assert_pixels(None, |at| {
            if at.x > 20 && at.x < width - 20 {
                gray_or_white(at, width, height)
            } else if at.x < 20 || at.x > width + 20 {
                Some(Color::WHITE)
            } else {
                None
            }
        });
    });
    ui.wait_for_idle();
    each_scale_frame(&mut ui, &noted, &is_square, true, 20, |image| {
        let (width, height) = (image.width(), image.height());
        image.assert_pixels(None, |at| {
            if at.x > 20 && at.x < width - 20 {
                if at.y < 20 || at.y > height - 20 {
                    gray_or_white(at, width, height)
                } else if at.y > 20 && at.y < height - 20 {
                    Some(Color::RED)
                } else {
                    None
                }
            } else if at.x < 20 || at.x > width - 20 {
                Some(Color::WHITE)
            } else {
                None
            }
        });
    });
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testSharedElementsDroppedFromOverlayAfterTransition
/// Shared elements leave the overlay after their transition though their match stays: the black
/// box rendered in the overlay at z-index 1 shows through each frame, both ways.
#[test]
fn shared_elements_dropped_from_overlay_after_transition() {
    set_capture_renderer(compose_skia::raster_colors);
    let show_overlay = mutable_state_of(false);
    let s = show_overlay.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        SharedTransitionLayout!(
            modifier = Modifier.required_size(120.dp()).test_tag("root"),
            move |scope: SharedTransitionScope| {
                let options = || SharedOptions {
                    bounds_transform: BoundsTransform::new(|_, _| tween(500).into()),
                    ..Default::default()
                };
                let first = scope.remember_shared_content_state("box");
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility_with(
                            &first,
                            !s.get(),
                            options(),
                        )
                        .background(Color::LIGHT_GRAY)
                        .fill_max_size()
                );
                let second = scope.remember_shared_content_state("box");
                Box!(
                    modifier = scope
                        .shared_element_with_caller_managed_visibility_with(
                            &second,
                            s.get(),
                            options(),
                        )
                        .background(Color::LIGHT_GRAY)
                        .size(110.dp())
                );
                let overlay = scope.clone();
                Box!(
                    modifier = scope
                        .render_in_shared_transition_scope_overlay_with(1.0, move || {
                            overlay.is_transition_active()
                        })
                        .size(100.dp())
                        .background(Color::BLACK)
                );
            }
        );
    });
    for _ in 0..2 {
        ui.wait_for_idle();
        ui.set_auto_advance(false);
        show_overlay.set(!show_overlay.get());
        ui.wait_for_idle();
        for _ in 0..6 {
            ui.advance_time_by(std::time::Duration::from_millis(100));
            ui.on_node_with_tag("root").capture_to_image().assert_contains_color(Color::BLACK);
        }
        ui.set_auto_advance(true);
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/SharedTransitionTest.kt, testSharedTransitionScopeIsInvalidated
/// The layout's overlay draws again as contents come and go under nested `AnimatedContent`s:
/// each new color shows half-way through its transition (b/347520198).
#[test]
fn shared_transition_scope_is_invalidated() {
    set_capture_renderer(compose_skia::raster_colors);
    let state = mutable_state_of(0);
    let anim_duration_millis = 500;
    let s = state.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        SharedTransitionLayout!(
            modifier = Modifier.size(100.dp()).test_tag("STL"),
            move |scope: SharedTransitionScope| {
                // An outer AnimatedContent doing nothing, which the issue needed.
                let scope = scope.clone();
                AnimatedContent!(true, move |_: AnimatedContentScope, _outer: bool| {
                    let scope = scope.clone();
                    AnimatedContent!(
                        s.get(),
                        transition_spec = move |_| {
                            fade_in_with(snap(0), 0.0)
                                .together_with(fade_out_with(snap(anim_duration_millis), 0.0))
                        },
                        move |inner: AnimatedContentScope, current: i32| {
                            let source = remember(MutableInteractionSource::new);
                            let options = ClickableOptions {
                                interaction_source: Some(source),
                                indication: Indication::NONE,
                                ..ClickableOptions::DEFAULT
                            };
                            let scope = scope.clone();
                            Box!(
                                modifier = Modifier
                                    .test_tag("click-target")
                                    .clickable_with(options, move || {
                                        s.set(match current {
                                            0 | 1 => 2,
                                            _ => 1,
                                        });
                                    })
                                    .fill_max_size(),
                                move |_| {
                                    let color = match current {
                                        0 => Color::GREEN,
                                        1 => Color::RED,
                                        _ => Color::BLUE,
                                    };
                                    let key =
                                        if current == 0 { "no match" } else { "matching key" };
                                    let shared = scope.remember_shared_content_state(key);
                                    let options = SharedOptions {
                                        enter: fade_in_with(snap(0), 0.0),
                                        exit: fade_out_with(snap(0), 0.0),
                                        ..Default::default()
                                    };
                                    Box!(
                                        modifier = scope
                                            .shared_bounds_with(&shared, &inner, options)
                                            .background(color)
                                            .fill_max_size()
                                    );
                                }
                            );
                        }
                    );
                });
            }
        );
    });
    ui.wait_for_idle();
    ui.on_node_with_tag("STL").capture_to_image().assert_contains_color(Color::GREEN);
    for color in [Color::BLUE, Color::RED] {
        ui.set_auto_advance(false);
        ui.on_node_with_tag("click-target").perform_click();
        ui.advance_time_by_frame();
        ui.advance_time_by(std::time::Duration::from_millis(anim_duration_millis as u64 / 2));
        ui.on_node_with_tag("STL").capture_to_image().assert_contains_color(color);
        ui.set_auto_advance(true);
        ui.wait_for_idle();
    }
}
