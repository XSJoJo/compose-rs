//! Shared elements under a deferred transition, drawn by Skia, as Compose's
//! `DeferredSharedElementTest` checks their pixels on a device: while the content is moved by hand
//! (a predictive back gesture), a shared element moves with it or stays where it was, and when the
//! phase ends its bounds animate on from where it was left.

use std::rc::Rc;

use compose::animation::*;
use compose::foundation::layout::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Alignment, Color, IntOffset, IntRect, IntSize, Modifier, TransformOrigin, UnitExt,
};

const TAG: &str = "scope";

type Scope = AnimatedContentTransitionScope<&'static str>;

/// A shared box in a content: `size` square, at `offset` in a full-size box, of key "shared".
#[derive(Clone, Copy)]
struct Shared {
    size: i32,
    offset: Option<i32>,
    /// `SharedContentConfig(permitTransformDuringDeferredTransition)`.
    permit: bool,
    in_overlay: bool,
    color: Color,
}

impl Shared {
    fn red(size: i32) -> Shared {
        Shared { size, offset: None, permit: true, in_overlay: true, color: Color::RED }
    }
}

/// `Box(Modifier.fillMaxSize()) { Box(Modifier[.offset(o, o)].sharedBounds(…).size(s)
/// .background(color)) }`, the shared bounds' other parameters Compose's defaults.
fn shared_box(scope: &SharedTransitionScope, content: &AnimatedContentScope, shared: Shared) {
    let (scope, content) = (scope.clone(), content.clone());
    Box!(modifier = Modifier.fill_max_size(), move |_| {
        let config = scope.shared_content_config_with(shared.permit);
        let state = scope.remember_shared_content_state_with("shared", config);
        let options = SharedOptions {
            render_in_overlay_during_transition: shared.in_overlay,
            ..Default::default()
        };
        let offset = match shared.offset {
            Some(offset) => Modifier.offset(offset.dp(), offset.dp()),
            None => Modifier,
        };
        Box!(
            modifier = offset
                .then(scope.shared_bounds_with(&state, &content, options))
                .size(shared.size.dp())
                .background(shared.color)
        );
    });
}

/// A white `SharedTransitionLayout` of `side` tagged "scope", holding a deferred content over
/// `state` with `spec` and `mutable_spec`, each state's content `content`.
fn deferred_rule(
    side: i32,
    state: &DeferredTransitionState<&'static str>,
    spec: impl Fn(Scope) -> ContentTransform + 'static,
    mutable_spec: impl Fn(Scope) -> Option<MutableContentTransform> + 'static,
    content: impl Fn(&SharedTransitionScope, &AnimatedContentScope, &'static str) + 'static,
) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    let held = state.clone();
    let (spec, mutable_spec, content) = (Rc::new(spec), Rc::new(mutable_spec), Rc::new(content));
    ComposeTestRule::sized(IntSize::new(side + 100, side + 100), move || {
        let (held, spec, mutable_spec, content) =
            (held.clone(), spec.clone(), mutable_spec.clone(), content.clone());
        SharedTransitionLayout!(
            modifier = Modifier.size(side.dp()).test_tag(TAG).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
                let transition = remember_deferred_transition(&held, None);
                let (spec, mutable_spec, content) =
                    (spec.clone(), mutable_spec.clone(), content.clone());
                transition.DeferredAnimatedContent(
                    Modifier,
                    move |it| spec(it),
                    Alignment::TOP_START,
                    ContentKey::state(),
                    move |it| mutable_spec(it),
                    move |it, target| content(&scope, &it, target),
                );
            }
        );
    })
}

fn origin(pivot_fraction_x: f32, pivot_fraction_y: f32) -> TransformOrigin {
    TransformOrigin { pivot_fraction_x, pivot_fraction_y }
}

fn fades(millis: u32) -> ContentTransform {
    fade_in_with(tween(millis), 0.0).together_with(fade_out_with(tween(millis), 0.0))
}

/// A held transform scaling the leaving content by `scale` around `origin` (Compose's default,
/// the centre, if none) while a target is pending, as `remember { MutableContentTransform {
/// initialContentTransform { if (state.pendingTargetState != null) … } } }`.
fn scaling_while_pending(
    state: &DeferredTransitionState<&'static str>,
    scale: MutableState<f32>,
    origin: Option<TransformOrigin>,
) -> MutableContentTransform {
    let held = state.clone();
    MutableContentTransform::new().initial_content_transform(move |it, _| {
        if held.pending_target_state().is_some() {
            if let Some(origin) = origin {
                it.set_transform_origin(origin);
            }
            it.set_scale(scale.get());
        }
    })
}

/// The bounds of the image's red pixels, as the tests' `getBounds`.
fn red_bounds(ui: &mut ComposeTestRule) -> IntRect {
    bounds_of(ui, |color| color.red() > 0.9 && color.green() < 0.1 && color.blue() < 0.1)
}

fn bounds_of(ui: &mut ComposeTestRule, matches: impl Fn(Color) -> bool) -> IntRect {
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    let (mut left, mut top, mut right, mut bottom) = (image.width(), image.height(), -1, -1);
    for y in 0..image.height() {
        for x in 0..image.width() {
            if matches(image.pixel(x, y)) {
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
    }
    if right == -1 { IntRect::ZERO } else { IntRect::new(left, top, right + 1, bottom + 1) }
}

/// The scope's pixels are red in `red` and white elsewhere in `white`.
#[track_caller]
fn assert_red_in(
    ui: &mut ComposeTestRule,
    red: impl Fn(IntOffset) -> bool,
    white: impl Fn(IntOffset) -> bool,
) {
    ui.on_node_with_tag(TAG).capture_to_image().assert_pixels(None, |at| {
        if red(at) {
            Some(Color::RED)
        } else if white(at) {
            Some(Color::WHITE)
        } else {
            None
        }
    });
}

fn within(at: IntOffset, left: i32, top: i32, right: i32, bottom: i32) -> bool {
    (left..right).contains(&at.x) && (top..bottom).contains(&at.y)
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testDetachedPreview
/// Not permitted to take part in the transform, a shared element stays where it was, unscaled,
/// drawn in the overlay while its container is scaled by hand.
#[test]
fn a_shared_element_not_permitted_to_transform_stays_detached() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let transform = scaling_while_pending(&state, scale.handle(), None);
    let mut ui = deferred_rule(
        300,
        &state,
        |_| fades(100),
        move |_| Some(transform.clone()),
        |scope, content, target| {
            let size = if target == "A" { 100 } else { 200 };
            shared_box(scope, content, Shared { permit: false, ..Shared::red(size) });
        },
    );
    ui.wait_for_idle();
    state.defer("A");
    scale.set(0.5);
    ui.wait_for_idle();
    assert_red_in(
        &mut ui,
        |at| within(at, 0, 0, 200, 200),
        |at| (200..300).contains(&at.x) || (200..300).contains(&at.y),
    );
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testAttachedPreview
/// A shared element scales and moves with its container in the overlay: around the container's
/// pivot, then by its offset.
#[test]
fn a_shared_element_scales_and_moves_with_its_container() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let offset = mutable_state_of(IntOffset::ZERO);
    let (held, s, o) = (state.clone(), scale.handle(), offset.handle());
    let transform = MutableContentTransform::new().initial_content_transform(move |it, _| {
        if held.pending_target_state().is_some() {
            it.set_transform_origin(origin(0.5, 0.5));
            it.set_scale(s.get());
            it.set_offset(o.get());
        }
    });
    let mut ui = deferred_rule(
        300,
        &state,
        |_| fades(100),
        move |_| Some(transform.clone()),
        |scope, content, target| {
            shared_box(scope, content, Shared::red(if target == "A" { 100 } else { 200 }));
        },
    );
    ui.wait_for_idle();
    state.defer("A");
    scale.set(0.5);
    offset.set(IntOffset::new(10, 20));
    ui.wait_for_idle();
    // (0, 0, 200, 200) scaled by half around (150, 150) is (75, 75, 175, 175), moved by (10, 20).
    assert_red_in(&mut ui, |at| within(at, 85, 95, 185, 195), |at| within(at, 0, 0, 300, 300));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testAttachedPreviewWithOffCenterOrigin
/// The pivot is the container's, even outside the shared element's bounds.
#[test]
fn a_shared_element_scales_around_its_container_s_pivot() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let transform = scaling_while_pending(&state, scale.handle(), Some(origin(1.0, 1.0)));
    let mut ui = deferred_rule(
        300,
        &state,
        |_| fades(100),
        move |_| Some(transform.clone()),
        |scope, content, target| {
            shared_box(scope, content, Shared::red(if target == "A" { 100 } else { 200 }));
        },
    );
    ui.wait_for_idle();
    state.defer("A");
    scale.set(0.5);
    ui.wait_for_idle();
    // Around (300, 300): (150, 150, 250, 250).
    assert_red_in(&mut ui, |at| within(at, 150, 150, 250, 250), |at| within(at, 0, 0, 300, 300));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testHandoffPreview
/// When the deferred phase ends, the bounds animate from where the preview left them, not from
/// the unscaled ones, to the target's.
#[test]
fn the_hand_off_animates_from_the_previewed_bounds() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let s = scale.handle();
    let mut ui = deferred_rule(
        300,
        &state,
        // Long, to catch the first frame after the hand-off.
        |_| fades(1000),
        move |_| {
            Some(MutableContentTransform::new().initial_content_transform(move |it, _| {
                it.set_transform_origin(origin(0.5, 0.5));
                it.set_scale(s.get());
            }))
        },
        |scope, content, target| {
            shared_box(scope, content, Shared::red(if target == "A" { 100 } else { 200 }));
        },
    );
    ui.wait_for_idle();
    state.defer("A");
    scale.set(0.5);
    ui.wait_for_idle();
    assert_red_in(&mut ui, |at| within(at, 75, 75, 175, 175), |at| within(at, 0, 0, 300, 300));

    ui.set_auto_advance(false);
    ui.wait_for_idle();
    let target = state.pending_target_state().unwrap_or_else(|| state.target_state());
    state.animate_to(target);
    ui.wait_for_idle();
    // One frame: the hand-off's, the animation not moved yet.
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let hand_off = red_bounds(&mut ui);
    assert_eq!(hand_off, IntRect::new(75, 75, 175, 175), "the hand-off's frame");

    ui.advance_time_by_frame();
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let after_one = red_bounds(&mut ui);
    assert!(
        after_one.left < hand_off.left
            && after_one.top < hand_off.top
            && after_one.left >= 0
            && after_one.top >= 0,
        "between {hand_off:?} and (0, 0, 100, 100): {after_one:?}"
    );

    ui.advance_time_by_frame();
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let after_three = red_bounds(&mut ui);
    assert!(
        after_three.left < after_one.left
            && after_three.top < after_one.top
            && after_three.left >= 0
            && after_three.top >= 0,
        "between {after_one:?} and (0, 0, 100, 100): {after_three:?}"
    );

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(red_bounds(&mut ui), IntRect::new(0, 0, 100, 100), "the end");
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testSimpleAttachedPreview
/// With no transform, a deferred target leaves the shared element where it is.
#[test]
fn a_pending_target_alone_leaves_the_shared_element_in_place() {
    let state = DeferredTransitionState::new("B");
    let mut ui = deferred_rule(
        300,
        &state,
        |_| default_content_transform(),
        |_| None,
        |scope, content, _| shared_box(scope, content, Shared::red(200)),
    );
    ui.wait_for_idle();
    state.defer("A");
    ui.wait_for_idle();
    assert_red_in(
        &mut ui,
        |at| within(at, 0, 0, 200, 200),
        |at| (200..300).contains(&at.x) || (200..300).contains(&at.y),
    );
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testAttachedPreviewWithOffset
/// A shared element away from its container's origin scales around the container's pivot.
#[test]
fn a_shared_element_away_from_its_container_s_origin_scales_around_its_pivot() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let s = scale.handle();
    let mut ui = deferred_rule(
        400,
        &state,
        |_| fades(100),
        move |_| {
            // Around the centre of the 400 × 400 container, (200, 200).
            Some(MutableContentTransform::new().initial_content_transform(move |it, _| {
                it.set_transform_origin(TransformOrigin::CENTER);
                it.set_scale(s.get());
            }))
        },
        |scope, content, _| {
            shared_box(scope, content, Shared { offset: Some(50), ..Shared::red(100) })
        },
    );
    ui.wait_for_idle();
    state.defer("A");
    scale.set(0.5);
    ui.wait_for_idle();
    // (50, 50, 150, 150) scaled by half around (200, 200): (125, 125, 175, 175).
    assert_eq!(red_bounds(&mut ui), IntRect::new(125, 125, 175, 175));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testHandoffVelocity
/// The hand-off takes the gesture's velocity: a faster gesture to the same scale ends smaller
/// three frames on.
#[test]
fn the_hand_off_takes_the_gesture_s_velocity() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let s = scale.handle();
    let mut ui = deferred_rule(
        300,
        &state,
        |_| {
            fade_in_with(tween(1000), 0.0).together_with(fade_out_with(
                spring_with(Spring::DAMPING_RATIO_NO_BOUNCY, Spring::STIFFNESS_VERY_LOW),
                0.0,
            ))
        },
        move |_| {
            Some(MutableContentTransform::new().initial_content_transform(move |it, _| {
                it.set_scale(s.get());
            }))
        },
        |scope, content, target| {
            shared_box(scope, content, Shared::red(if target == "A" { 100 } else { 200 }));
        },
    );
    ui.wait_for_idle();
    ui.set_auto_advance(false);

    let gesture = |ui: &mut ComposeTestRule, steps: &[f32]| {
        ui.wait_for_idle();
        state.defer("A");
        ui.advance_time_by_frame();
        ui.wait_for_idle();
        for &step in steps {
            ui.advance_time_by_frame();
            ui.wait_for_idle();
            scale.set(step);
            ui.wait_for_idle();
        }
        ui.wait_for_idle();
        state.animate_to("A");
        for _ in 0..3 {
            ui.advance_time_by_frame();
        }
        ui.wait_for_idle();
        red_bounds(ui)
    };

    let slow = gesture(&mut ui, &[0.9, 0.8, 0.7, 0.6, 0.5]);

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    scale.set(1.0);
    state.animate_to("B");
    ui.wait_for_idle();
    ui.set_auto_advance(false);

    let fast = gesture(&mut ui, &[0.9, 0.7, 0.5]);
    assert!(
        fast.width() < slow.width() && fast.height() < slow.height(),
        "{fast:?} smaller than {slow:?}"
    );
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testInterruptionHandoff_noJump
/// A deferred phase interrupting a running transition, then committed: the shared element
/// doesn't jump at the hand-off.
#[test]
fn a_deferred_phase_interrupting_a_transition_hands_off_without_a_jump() {
    let (deferred, committed) = interrupted_gesture(true);
    assert_ne!(deferred, IntRect::ZERO, "shown");
    assert_eq!(deferred, committed, "no jump after the commit");
}

/// Not Compose's: drawn in place, the same shared element shows where the overlay draws it,
/// moved with its content while its bounds animate (its placement goes through the content's
/// transform, which the content's layer maps back).
#[test]
fn a_shared_element_drawn_in_place_follows_the_transform_while_its_bounds_animate() {
    let (in_overlay, _) = interrupted_gesture(true);
    let (in_place, _) = interrupted_gesture(false);
    assert_ne!(in_place, IntRect::ZERO, "shown");
    assert!(near(in_place, in_overlay), "in place {in_place:?}, in the overlay {in_overlay:?}");
}

/// Within a pixel each side: one drawn at a fraction, the other at its rounded position.
fn near(a: IntRect, b: IntRect) -> bool {
    let close = |a: i32, b: i32| (a - b).abs() <= 1;
    close(a.left, b.left)
        && close(a.top, b.top)
        && close(a.right, b.right)
        && close(a.bottom, b.bottom)
}

/// Compose's `testInterruptionHandoff_noJump`: in a 400 × 400 layout, a 100 × 100 shared element
/// at (0, 0) in "A" and (200, 200) in "B", the contents sliding over 2 s. "A" goes to "B", then
/// half a second in, a gesture back to "A" moves "B" by (100, 100); a frame later it's committed.
/// The red pixels' bounds a frame into the gesture and a frame after the commit.
fn interrupted_gesture(in_overlay: bool) -> (IntRect, IntRect) {
    let state = DeferredTransitionState::new("A");
    let offset = mutable_state_of(IntOffset::ZERO);
    let (held, o) = (state.clone(), offset.handle());
    let transform = MutableContentTransform::new().initial_content_transform(move |it, _| {
        if held.pending_target_state() == Some("A") {
            it.set_offset(o.get());
        }
    });
    let mut ui = deferred_rule(
        400,
        &state,
        |_| {
            slide_in_horizontally_with(tween(2000), |width| -width / 2)
                .together_with(slide_out_horizontally_with(tween(2000), |width| width / 2))
        },
        move |_| Some(transform.clone()),
        move |scope, content, target| {
            let offset = if target == "A" { None } else { Some(200) };
            shared_box(scope, content, Shared { offset, in_overlay, ..Shared::red(100) });
        },
    );
    ui.wait_for_idle();
    let red = |ui: &mut ComposeTestRule| bounds_of(ui, |c| c.red() > 0.5 && c.green() < 0.1);

    ui.set_auto_advance(false);
    state.animate_to("B");
    ui.advance_time_by(std::time::Duration::from_millis(500));
    ui.wait_for_idle();

    state.defer("A");
    offset.set(IntOffset::new(100, 100));
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let deferred = red(&mut ui);

    state.animate_to("A");
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let committed = red(&mut ui);

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    (deferred, committed)
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testDeferredTransition_withRenderInOverlayFalse_scalesWithParent
/// Drawn in place, a shared element is inside its container and scales with it, permitted or
/// not.
#[test]
fn a_shared_element_drawn_in_place_scales_with_its_container() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let transform = scaling_while_pending(&state, scale.handle(), Some(origin(0.0, 0.0)));
    let mut ui = deferred_rule(
        300,
        &state,
        |_| fades(100),
        move |_| Some(transform.clone()),
        |scope, content, target| {
            let shared = Shared {
                offset: Some(50),
                permit: false,
                in_overlay: false,
                ..Shared::red(if target == "A" { 100 } else { 200 })
            };
            shared_box(scope, content, shared);
        },
    );
    ui.wait_for_idle();
    state.defer("A");
    scale.set(0.5);
    ui.wait_for_idle();
    // (50, 50, 250, 250) scaled by half around (0, 0).
    assert_red_in(&mut ui, |at| within(at, 25, 25, 125, 125), |at| within(at, 0, 0, 300, 300));
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testDeferredHandoff_withRenderInOverlayFalse_doesNotJump
/// Drawn in place, a shared element's hand-off starts where the gesture left it.
#[test]
fn a_shared_element_drawn_in_place_hands_off_without_a_jump() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let transform = scaling_while_pending(&state, scale.handle(), Some(origin(0.0, 0.0)));
    let mut ui = deferred_rule(
        300,
        &state,
        |_| fades(1000),
        move |_| Some(transform.clone()),
        |scope, content, target| {
            let shared = Shared {
                offset: Some(50),
                in_overlay: false,
                ..Shared::red(if target == "A" { 100 } else { 200 })
            };
            shared_box(scope, content, shared);
        },
    );
    ui.wait_for_idle();
    state.defer("A");
    scale.set(0.5);
    ui.wait_for_idle();
    let hand_off = red_bounds(&mut ui);

    ui.set_auto_advance(false);
    ui.wait_for_idle();
    state.animate_to("A");
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    assert_eq!(red_bounds(&mut ui), hand_off, "the hand-off's frame");

    ui.set_auto_advance(true);
    ui.wait_for_idle();
    assert_eq!(red_bounds(&mut ui), IntRect::new(50, 50, 150, 150), "the end, unscaled");
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testDeferredTransition_cancellationDoesNotJump
/// A gesture cancelled back to the content it started on: the shared element stays where it is,
/// not jumping to the other content's offset.
#[test]
fn a_cancelled_gesture_does_not_jump() {
    let state = DeferredTransitionState::new("Detail");
    let transform = MutableContentTransform::new()
        // The detail's.
        .initial_content_transform(|it, _| it.set_offset(IntOffset::ZERO))
        // The main screen's: a parallax to the right.
        .target_content_transform(|it, _| it.set_offset(IntOffset::new(50, 0)));
    let mut ui = deferred_rule(
        300,
        &state,
        |_| fades(100),
        move |_| Some(transform.clone()),
        |scope, content, _| shared_box(scope, content, Shared::red(100)),
    );
    ui.wait_for_idle();
    state.defer("Main");
    ui.wait_for_idle();
    let before = red_bounds(&mut ui);
    assert_eq!(before.width(), 100, "{before:?}");
    assert_eq!(before.left, 0, "{before:?}");

    ui.set_auto_advance(false);
    ui.wait_for_idle();
    state.animate_to("Detail");
    ui.wait_for_idle();
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let after = red_bounds(&mut ui);
    assert_eq!(after.left, 0, "{after:?}");
}

/// The bounds of what isn't white `millis` into a change from "A" to "B" with no enter or exit,
/// a 100 × 100 shared element at (0, 0) in "A" and (200, 200) in "B".
fn moving_element(in_overlay: bool, millis: u64) -> IntRect {
    let state = DeferredTransitionState::new("A");
    let mut ui = deferred_rule(
        400,
        &state,
        |_| EnterTransition::NONE.together_with(ExitTransition::NONE),
        |_| None,
        move |scope, content, target| {
            let offset = if target == "A" { None } else { Some(200) };
            shared_box(scope, content, Shared { offset, in_overlay, ..Shared::red(100) });
        },
    );
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    state.animate_to("B");
    ui.advance_time_by(std::time::Duration::from_millis(millis));
    ui.wait_for_idle();
    bounds_of(&mut ui, |color| color.green() < 0.9)
}

/// Not Compose's: a shared element moving between its contents is drawn in the overlay where it
/// is placed (and drawn in place), not moved again by where its node puts its content (D1341).
#[test]
fn a_moving_shared_element_is_drawn_in_the_overlay_where_it_is_placed() {
    for millis in [50, 150, 300] {
        let (in_overlay, in_place) = (moving_element(true, millis), moving_element(false, millis));
        assert_ne!(in_place, IntRect::ZERO, "shown at {millis} ms");
        assert!(near(in_overlay, in_place), "at {millis} ms: {in_overlay:?}, {in_place:?}");
    }
}
