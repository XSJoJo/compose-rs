//! Shared elements under a deferred transition, against Compose's `DeferredSharedElementTest`'s
//! cases that check bounds: while the content is moved by hand the incoming and outgoing shared
//! elements move together, and when the phase ends (committed or cancelled) they start from
//! there. The cases checking pixels are compose-skia's.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_animation::*;
use compose_foundation::layout::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Alignment, Color, IntSize, Modifier, Rect, TransformOrigin, UnitExt};

type Scope = AnimatedContentTransitionScope<&'static str>;

/// A `SharedTransitionLayout` of `side` holding a deferred content over `state`.
fn deferred_rule(
    side: i32,
    state: &DeferredTransitionState<&'static str>,
    spec: impl Fn(Scope) -> ContentTransform + 'static,
    mutable_spec: impl Fn(Scope) -> Option<MutableContentTransform> + 'static,
    content: impl Fn(&SharedTransitionScope, &AnimatedContentScope, &'static str) + 'static,
) -> ComposeTestRule {
    let held = state.clone();
    let (spec, mutable_spec, content) = (Rc::new(spec), Rc::new(mutable_spec), Rc::new(content));
    ComposeTestRule::sized(IntSize::new(side + 100, side + 100), move || {
        let (held, spec, mutable_spec, content) =
            (held.clone(), spec.clone(), mutable_spec.clone(), content.clone());
        let transition = remember_deferred_transition(&held, None);
        SharedTransitionLayout!(
            modifier = Modifier.size(side.dp()).background(Color::WHITE),
            move |scope: SharedTransitionScope| {
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

fn fades(millis: u32) -> ContentTransform {
    fade_in_with(tween(millis), 0.0).together_with(fade_out_with(tween(millis), 0.0))
}

fn origin(pivot_fraction_x: f32, pivot_fraction_y: f32) -> TransformOrigin {
    TransformOrigin { pivot_fraction_x, pivot_fraction_y }
}

fn bounds_in_root(ui: &mut ComposeTestRule, tag: &str) -> Rect {
    ui.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root()
}

#[track_caller]
fn close(actual: f32, expected: f32, what: &str) {
    assert!((actual - expected).abs() <= 1.0, "{what}: {actual}, Compose's {expected}");
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testCancelDeferredPhase_doesNotJumpToIncomingState
/// A gesture cancelled back: during the deferred phase the transition is held at its old state,
/// its targets inverted, and the cancelled shared element mustn't take the incoming content's
/// slide in.
#[test]
fn a_cancelled_deferred_phase_does_not_jump_to_the_incoming_state() {
    let state = DeferredTransitionState::new("A");
    let scale = mutable_state_of(1.0f32);
    let (held, s) = (state.clone(), scale.handle());
    let transform = MutableContentTransform::new().initial_content_transform(move |it, _| {
        if held.pending_target_state().is_some() {
            it.set_transform_origin(origin(0.5, 0.5));
            it.set_scale(s.get());
        }
    });
    let slide = || spring_with(Spring::DAMPING_RATIO_NO_BOUNCY, Spring::STIFFNESS_MEDIUM_LOW);
    let mut ui = deferred_rule(
        300,
        &state,
        move |it: Scope| {
            if *it.target_state() == "B" {
                slide_in_horizontally_with(slide(), |width| width)
                    .together_with(fade_out_with(tween(100), 0.0))
            } else {
                fade_in_with(tween(100), 0.0)
                    .together_with(slide_out_horizontally_with(slide(), |width| width))
            }
        },
        move |_| Some(transform.clone()),
        |scope, content, target| {
            let (scope, content) = (scope.clone(), content.clone());
            Box!(modifier = Modifier.fill_max_size(), move |_| {
                let state = scope.remember_shared_content_state("shared");
                let shared = scope.shared_bounds(&state, &content);
                let tagged = if target == "A" { shared.test_tag("shared") } else { shared };
                Box!(modifier = tagged.size(200.dp()).background(Color::RED));
            });
        },
    );
    ui.wait_for_idle();
    state.defer("B");
    scale.set(0.5);
    ui.wait_for_idle();

    ui.set_auto_advance(false);
    state.animate_to("A");
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let bounds = ui.on_node_with_tag("shared").fetch_semantics_node().bounds_in_window();
    assert!(bounds.left < 150.0, "jumped to the incoming content's bounds: left {}", bounds.left);

    ui.set_auto_advance(true);
    ui.wait_for_idle();
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, incomingElement_alignsWithOutgoing_whenRenderingInPlace_atDeferredHandoff
/// At the hand-off, an incoming shared element drawn in place starts where the outgoing one,
/// drawn in the overlay and scaled with its content, was left.
#[test]
fn an_incoming_element_drawn_in_place_aligns_with_the_outgoing_one_at_the_hand_off() {
    let state = DeferredTransitionState::new("B");
    let scale = mutable_state_of(1.0f32);
    let s = scale.handle();
    let mut ui = deferred_rule(
        300,
        &state,
        |_| fades(100),
        move |_| {
            Some(MutableContentTransform::new().initial_content_transform(move |it, _| {
                it.set_transform_origin(origin(0.0, 0.0));
                it.set_scale(s.get());
            }))
        },
        |scope, content, target| {
            let (scope, content) = (scope.clone(), content.clone());
            Box!(modifier = Modifier.fill_max_size(), move |_| {
                let state = scope.remember_shared_content_state("shared");
                if target == "A" {
                    let options = SharedOptions {
                        render_in_overlay_during_transition: false,
                        ..Default::default()
                    };
                    Box!(
                        modifier = scope
                            .shared_bounds_with(&state, &content, options)
                            .test_tag("incoming")
                            .size(100.dp())
                            .background(Color::RED)
                    );
                } else {
                    Box!(
                        modifier = scope
                            .shared_bounds(&state, &content)
                            .test_tag("outgoing")
                            .size(200.dp())
                            .background(Color::BLUE)
                    );
                }
            });
        },
    );
    ui.wait_for_idle();
    ui.set_auto_advance(false);

    ui.wait_for_idle();
    state.defer("A");
    ui.advance_time_by_frame();

    scale.set(0.5);
    ui.advance_time_by_frame();
    ui.wait_for_idle();

    state.animate_to("A");
    ui.advance_time_by_frame();
    ui.wait_for_idle();

    let incoming = bounds_in_root(&mut ui, "incoming");
    assert_eq!(incoming.width(), 100.0, "half of the outgoing size at the hand-off: {incoming:?}");
    assert_eq!((incoming.left, incoming.top), (0.0, 0.0), "{incoming:?}");
}

/// Compose's two nested cases: a deferred content scaled by hand around its centre, holding a
/// shared element of key "shared" under `wrap` (40 × 40, at (20, 60) in "A" and (60, 100) in
/// "B", tagged `shared-<state>`).
fn nested_rule(
    state: &DeferredTransitionState<&'static str>,
    scale: MutableState<f32>,
    wrap: impl Fn(&SharedTransitionScope, &AnimatedContentScope) -> Modifier + 'static,
) -> ComposeTestRule {
    let wrap = Rc::new(wrap);
    deferred_rule(
        200,
        state,
        |_| EnterTransition::NONE.together_with(ExitTransition::NONE),
        move |_| {
            Some(
                MutableContentTransform::new()
                    .initial_content_transform(move |it, _| it.set_scale(scale.get())),
            )
        },
        move |scope, content, target| {
            let (scope, content) = (scope.clone(), content.clone());
            let around = wrap(&scope, &content);
            Box!(modifier = Modifier.fill_max_size().then(around), move |_| {
                let state = scope.remember_shared_content_state("shared");
                let options = SharedOptions {
                    bounds_transform: BoundsTransform::new(|_, _| tween(300).into()),
                    ..Default::default()
                };
                let (x, y) = if target == "A" { (20, 60) } else { (60, 100) };
                Box!(
                    modifier = Modifier
                        .offset(x.dp(), y.dp())
                        .then(scope.shared_element_with(&state, &content, options))
                        .size(40.dp())
                        .test_tag(format!("shared-{target}"))
                );
            });
        },
    )
}

/// Defers "B" scaled by half, checks "A" at its scaled bounds, commits and checks both start
/// there at the hand-off.
fn check_nested_hand_off(
    ui: &mut ComposeTestRule,
    state: &DeferredTransitionState<&'static str>,
    scale: &OwnedState<f32>,
) {
    ui.wait_for_idle();
    ui.set_auto_advance(false);
    // (20, 60, 40 × 40) scaled by half around (100, 100): (60, 80), 20 wide.
    ui.wait_for_idle();
    state.defer("B");
    scale.set(0.5);
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    let deferred = bounds_in_root(ui, "shared-A");
    close(deferred.left, 60.0, "A's left, deferred");
    close(deferred.top, 80.0, "A's top, deferred");
    close(deferred.width(), 20.0, "A's width, deferred");

    ui.wait_for_idle();
    state.animate_to("B");
    ui.advance_time_by_frame();
    ui.wait_for_idle();
    for tag in ["shared-A", "shared-B"] {
        let bounds = bounds_in_root(ui, tag);
        close(bounds.left, 60.0, &format!("{tag}'s left at the hand-off"));
        close(bounds.top, 80.0, &format!("{tag}'s top at the hand-off"));
        close(bounds.width(), 20.0, &format!("{tag}'s width at the hand-off"));
    }
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testSharedElementNestedUnderAnimateEnterExit_preservesDeferredTransformAtHandoff
/// Under an `animate_enter_exit`, a shared element's hand-off starts from its transformed bounds.
#[test]
fn a_shared_element_under_animate_enter_exit_keeps_the_deferred_transform_at_the_hand_off() {
    let state = DeferredTransitionState::new("A");
    let scale = mutable_state_of(1.0f32);
    let mut ui = nested_rule(&state, scale.handle(), |_, content| {
        content.animate_enter_exit(
            fade_in_with(tween(300), 0.0),
            fade_out_with(tween(300), 0.0),
            "animateEnterExit",
        )
    });
    check_nested_hand_off(&mut ui, &state, &scale);
}

// Compose: compose/animation/animation/src/androidDeviceTest/kotlin/androidx/compose/animation/DeferredSharedElementTest.kt, testSharedElementNestedUnderSharedBounds_preservesDeferredTransformAtHandoff
/// Under shared bounds, a shared element's hand-off starts from its transformed bounds.
#[test]
fn a_shared_element_under_shared_bounds_keeps_the_deferred_transform_at_the_hand_off() {
    let state = DeferredTransitionState::new("A");
    let scale = mutable_state_of(1.0f32);
    let mut ui = nested_rule(&state, scale.handle(), |scope, content| {
        let bounds = scope.remember_shared_content_state("bounds");
        let options = SharedOptions {
            enter: fade_in_with(tween(300), 0.0),
            exit: fade_out_with(tween(300), 0.0),
            resize_mode: ResizeMode::RemeasureToBounds,
            ..Default::default()
        };
        scope.shared_bounds_with(&bounds, content, options)
    });
    check_nested_hand_off(&mut ui, &state, &scale);
}
