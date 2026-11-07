//! A style's animations between states (D1585): `StyleCollectorTest`'s animations,
//! `StyleLayoutTest.testAnimationCleanup` and the animated `StyleUxTaskTests`, driven by the
//! test clock frame by frame (Compose's host tests use a frame clock of their own).

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose_animation::tween;
use compose_foundation::layout::*;
use compose_foundation::style::properties::*;
use compose_foundation::style::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::DrawOp;
use compose_ui::{Brush, Constraints, UnitExt};

use crate::style_support::{StyleSetters, styleable};

static CUSTOM_FLOAT: ProvidableStyleProperty<f32> =
    style_property_of_with("customFloatProperty", |a, b, t| compose_ui::lerp(*a, *b, t), || 0.0);
static CUSTOM_BACKGROUND: ProvidableStyleProperty<Fill> =
    style_property_of("customBackground", || Fill::Color(Color::TRANSPARENT));

/// A box bound to `resolver`, its content's measurements counted, on the manual clock.
fn animated(resolver: StyleResolver, measures: Rc<Cell<u32>>) -> ComposeTestRule {
    ComposeTestRule::with_manual_clock(move || {
        let measures = measures.clone();
        let counting = Modifier.layout(move |scope, measurable, constraints: Constraints| {
            measures.set(measures.get() + 1);
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        });
        Box!(modifier = styleable(counting, &resolver), |_| {
            Box!(modifier = Modifier.size(10.dp()));
        });
    })
}

/// `collectAnimated`: `property` at `initial`, at `target` animated by a 450 ms tween while
/// pressed; pressed for `duration` ms then released until it settles, read each frame.
fn collect_animated<T: Clone + PartialEq + 'static>(
    property: &'static ProvidableStyleProperty<T>,
    initial: T,
    target: T,
    duration: u64,
) -> Vec<T> {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(move |s| {
        s.provide(property, initial.clone());
        let target = target.clone();
        s.pressed(move |s| {
            let target = target.clone();
            s.animate_with(tween(450), move |s| s.provide(property, target.clone()));
        });
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
    let read = || resolver.resolve(|s| s.value(property));
    let mut values = vec![read()];
    state.set_pressed(true);
    let frames = |ui: &mut ComposeTestRule, values: &mut Vec<T>, millis: u64| {
        for _ in 0..millis / 16 {
            ui.advance_time_by_frame();
            values.push(read());
        }
    };
    frames(&mut ui, &mut values, duration);
    state.set_pressed(false);
    frames(&mut ui, &mut values, 1000);
    values
}

fn distinct(values: &[f32]) -> usize {
    let mut seen: Vec<f32> = Vec::new();
    for value in values {
        if !seen.iter().any(|seen| (seen - value).abs() < 0.001) {
            seen.push(*value);
        }
    }
    seen.len()
}

#[test]
fn can_animate_a_float_property() {
    let values = collect_animated(&CUSTOM_FLOAT, 0.0, 100.0, 1000);
    assert_eq!(values[0], 0.0);
    assert!(values.contains(&100.0));
    assert_eq!(*values.last().unwrap(), 0.0);
    assert!(distinct(&values) > 2);
}

/// Released half way, it turns back from where it is: up, then down, never a jump.
#[test]
fn can_animate_interrupted_property() {
    let values = collect_animated(&CUSTOM_FLOAT, 0.0, 100.0, 200);
    assert_eq!(values[0], 0.0);
    let peak = values.iter().cloned().fold(0.0, f32::max);
    assert!(peak > 0.0 && peak < 100.0, "interrupted before its end: {peak}");
    let mut going_up = true;
    for pair in values.windows(2) {
        let step = pair[1] - pair[0];
        if step.abs() < 0.001 {
            continue;
        }
        if step < 0.0 {
            going_up = false;
        } else {
            assert!(going_up, "went up again after going down: {values:?}");
        }
        assert!(step.abs() < 40.0, "jumped: {values:?}");
    }
    assert_eq!(*values.last().unwrap(), 0.0);
}

/// Leaving the state animates back with the spec it came with (the from spec).
#[test]
fn can_animate_an_exit_property() {
    let values = collect_animated(&CUSTOM_FLOAT, 0.0, 100.0, 1000);
    assert!(values.contains(&100.0));
    let exit: Vec<f32> = values
        .iter()
        .copied()
        .skip_while(|value| *value < 100.0)
        .skip_while(|v| *v == 100.0)
        .collect();
    assert_eq!(*exit.last().unwrap(), 0.0);
    assert!(exit.iter().any(|value| *value > 0.0 && *value < 100.0));
}

#[test]
fn can_animate_between_color_and_brush() {
    let color = Fill::Color(Color::BLUE);
    let brush = Fill::Brush(Brush::Solid(Color::RED));
    let values = collect_animated(&CUSTOM_BACKGROUND, color.clone(), brush.clone(), 1000);
    assert_eq!(values[0], color);
    assert_eq!(*values.last().unwrap(), color);
    assert!(values.contains(&brush));
    let mut distinct: Vec<&Fill> = Vec::new();
    for value in &values {
        if !distinct.contains(&value) {
            distinct.push(value);
        }
    }
    assert!(distinct.len() > 2);
}

/// Without `animate`, a change is at once.
#[test]
fn a_change_outside_animate_is_immediate() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.provide(&CUSTOM_FLOAT, 0.0);
        s.pressed(|s| s.provide(&CUSTOM_FLOAT, 100.0));
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
    state.set_pressed(true);
    ui.advance_time_by_frame();
    assert_eq!(resolver.resolve(|s| s.value(&CUSTOM_FLOAT)), 100.0);
}

/// `StyleLayoutTest.testAnimationCleanup`: a state left half way through its animation.
#[test]
fn test_animation_cleanup() {
    let state = MutableStyleState::new(None);
    let animating = Rc::new(Cell::new(false));
    let flag = animating.clone();
    let style = CommonStyle::new(move |s| {
        s.size(100.dp());
        s.background(Color::GREEN);
        let flag = flag.clone();
        s.focused(move |s| {
            let flag = flag.clone();
            s.animate_with(tween(100), move |s| {
                s.background(Color::BLUE);
                flag.set(true);
            });
        });
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver, Rc::new(Cell::new(0)));
    ui.wait_for_idle();
    state.set_focused(true);
    ui.advance_time_until(|_| animating.get());
    ui.advance_time_by(std::time::Duration::from_millis(50));
    state.set_focused(false);
    ui.advance_time_by_frame();
    ui.advance_time_by(std::time::Duration::from_millis(1000));
    assert_eq!(fills(&mut ui), vec![Color::GREEN]);
}

fn fills(ui: &mut ComposeTestRule) -> Vec<Color> {
    let list = ui.host().display_list();
    list.ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::DrawRect(_, paint) => Some(paint.color),
            _ => None,
        })
        .collect()
}

/// An animated background draws each frame and measures nothing (D1585).
#[test]
fn an_animated_background_draws_without_measuring() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.size(40.dp());
        s.background(Color::RED);
        s.pressed(|s| s.animate_with(tween(300), |s| s.background(Color::BLUE)));
    });
    let measures = Rc::new(Cell::new(0));
    let mut ui = animated(StyleResolver::new(style, state.clone()), measures.clone());
    ui.advance_time_by_frame();
    let measured = measures.get();
    state.set_pressed(true);
    let mut drawn = Vec::new();
    for _ in 0..30 {
        ui.advance_time_by_frame();
        drawn.extend(fills(&mut ui));
    }
    assert_eq!(*drawn.last().unwrap(), Color::BLUE);
    assert!(drawn.iter().any(|color| *color != Color::RED && *color != Color::BLUE));
    assert_eq!(measures.get(), measured, "an animated background measured again");
}

/// `animation_state_nestingOrder`: `pressed { animate { … } }` and `animate { pressed { … } }`
/// animate the same.
#[test]
fn animation_state_nesting_order() {
    let run = |style: CommonStyle| {
        let state = MutableStyleState::new(None);
        let resolver = StyleResolver::new(style, state.clone());
        let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
        state.set_pressed(true);
        let mut values = Vec::new();
        for _ in 0..80 {
            ui.advance_time_by_frame();
            values.push(resolver.resolve(|s| s.value(&BACKGROUND)));
        }
        values
    };
    let pressed_animate = run(CommonStyle::new(|s| {
        s.background(Color::RED);
        s.pressed(|s| s.animate_with(tween(1000), |s| s.background(Color::GREEN)));
    }));
    let animate_pressed = run(CommonStyle::new(|s| {
        s.background(Color::RED);
        s.animate_with(tween(1000), |s| s.pressed(|s| s.background(Color::GREEN)));
    }));
    assert_eq!(pressed_animate, animate_pressed);
    assert!(pressed_animate.len() > 10);
    assert_ne!(pressed_animate[10], Fill::Color(Color::GREEN));
    assert_eq!(*pressed_animate.last().unwrap(), Fill::Color(Color::GREEN));
}

/// `translation_in_animation_block`: a translation animated on press, a layer change each
/// frame. The property newly set measures once (the set of properties changed, which every
/// reader observes, as Compose's snapshot map), not at each frame.
#[test]
fn translation_in_animation_block() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.size(100.dp());
        s.background(Color::BLUE);
        s.pressed(|s| s.animate(|s| s.translation(0.0, 100.0)));
    });
    let resolver = StyleResolver::new(style, state.clone());
    let measures = Rc::new(Cell::new(0));
    let mut ui = animated(resolver.clone(), measures.clone());
    ui.advance_time_by_frame();
    let measured = measures.get();
    state.set_pressed(true);
    let mut values = Vec::new();
    for _ in 0..60 {
        ui.advance_time_by_frame();
        values.push(resolver.resolve(|s| s.value(&TRANSLATION_Y)));
    }
    assert!(values.iter().any(|value| *value > 0.0 && *value < 100.0));
    assert!((values.last().unwrap() - 100.0).abs() < 0.5);
    assert_eq!(measures.get(), measured + 1, "a translation measured each frame");
}

/// A spec given with `animate_both` animates into the state with the first and out of it
/// with the second.
#[test]
fn to_and_from_specs_time_the_two_ways() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.provide(&CUSTOM_FLOAT, 0.0);
        s.pressed(|s| s.animate_both(tween(100), tween(500), |s| s.provide(&CUSTOM_FLOAT, 100.0)));
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
    let settle = |ui: &mut ComposeTestRule, target: f32| {
        let mut frames = 0;
        while resolver.resolve(|s| s.value(&CUSTOM_FLOAT)) != target && frames < 200 {
            ui.advance_time_by_frame();
            frames += 1;
        }
        frames
    };
    state.set_pressed(true);
    let into = settle(&mut ui, 100.0);
    state.set_pressed(false);
    let out = settle(&mut ui, 0.0);
    assert!(into < 12, "into the state with the 100 ms spec: {into} frames");
    assert!(out > 25, "out of it with the 500 ms spec: {out} frames");
}

/// Frames until `resolver`'s float is `target` (200 at most).
fn frames_to(ui: &mut ComposeTestRule, resolver: &StyleResolver, target: f32) -> u32 {
    let mut frames = 0;
    while resolver.resolve(|s| s.value(&CUSTOM_FLOAT)) != target && frames < 200 {
        ui.advance_time_by_frame();
        frames += 1;
    }
    frames
}

/// Into a state, its own spec wins over the one the last state gave for leaving it, as
/// `recordAnimations` (the to spec, then the previous from spec).
#[test]
fn the_spec_into_a_state_wins_over_the_last_state_s_from_spec() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.provide(&CUSTOM_FLOAT, 0.0);
        s.hovered(|s| s.animate_both(tween(500), tween(500), |s| s.provide(&CUSTOM_FLOAT, 50.0)));
        s.pressed(|s| s.animate_both(tween(80), tween(80), |s| s.provide(&CUSTOM_FLOAT, 100.0)));
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
    state.set_hovered(true);
    frames_to(&mut ui, &resolver, 50.0);
    state.set_pressed(true);
    let into = frames_to(&mut ui, &resolver, 100.0);
    assert!(into < 10, "pressed with its 80 ms spec: {into} frames");
}

/// A new spec for the same value restarts the animation with it, from where it is.
#[test]
fn a_new_spec_for_the_same_value_restarts_the_animation() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.provide(&CUSTOM_FLOAT, 0.0);
        s.pressed(|s| {
            s.animate_with(tween(2000), |s| s.provide(&CUSTOM_FLOAT, 100.0));
            s.hovered(|s| s.animate_with(tween(50), |s| s.provide(&CUSTOM_FLOAT, 100.0)));
        });
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
    state.set_pressed(true);
    for _ in 0..10 {
        ui.advance_time_by_frame();
    }
    let partway = resolver.resolve(|s| s.value(&CUSTOM_FLOAT));
    assert!(partway > 0.0 && partway < 50.0, "{partway}");
    state.set_hovered(true);
    let left = frames_to(&mut ui, &resolver, 100.0);
    assert!(left < 8, "finished with the 50 ms spec: {left} frames");
}

/// An interrupted animation draws on from where it was in the frame it's interrupted, never a
/// jump.
#[test]
fn an_interrupted_animation_draws_on_from_where_it_was() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.size(20.dp());
        s.background(Color::BLACK);
        s.pressed(|s| s.animate_with(tween(500), |s| s.background(Color::WHITE)));
    });
    let mut ui = animated(StyleResolver::new(style, state.clone()), Rc::new(Cell::new(0)));
    let red = |ui: &mut ComposeTestRule| fills(ui).last().map_or(0.0, |color| color.red());
    state.set_pressed(true);
    let mut drawn = Vec::new();
    for _ in 0..12 {
        ui.advance_time_by_frame();
        drawn.push(red(&mut ui));
    }
    state.set_pressed(false);
    for _ in 0..40 {
        ui.advance_time_by_frame();
        drawn.push(red(&mut ui));
    }
    for pair in drawn.windows(2) {
        assert!((pair[1] - pair[0]).abs() < 0.2, "jumped: {drawn:?}");
    }
}

/// The red drawn each frame of a background animated to white while pressed, released after
/// `frames` frames (never if none).
fn reds_released_after(frames: Option<usize>) -> Vec<f32> {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.size(20.dp());
        s.background(Color::BLACK);
        s.pressed(|s| s.animate_with(tween(500), |s| s.background(Color::WHITE)));
    });
    let mut ui = animated(StyleResolver::new(style, state.clone()), Rc::new(Cell::new(0)));
    state.set_pressed(true);
    let mut drawn = Vec::new();
    for frame in 0..20 {
        if Some(frame) == frames {
            state.set_pressed(false);
        }
        ui.advance_time_by_frame();
        drawn.push(fills(&mut ui).last().map_or(0.0, |color| color.red()));
    }
    drawn
}

/// The frame an animation is interrupted in draws the value it showed (its new start, from
/// which it turns back), D1585.
#[test]
fn the_frame_an_animation_is_interrupted_draws_where_it_was() {
    let pressed = reds_released_after(None);
    let released = reds_released_after(Some(12));
    assert_eq!(released[..12], pressed[..12]);
    assert!(pressed[11] > 0.2 && pressed[11] < 0.9, "partway: {pressed:?}");
    assert_eq!(released[12], pressed[11], "{released:?}");
    assert!(released[19] < released[12], "turned back: {released:?}");
}

/// Between the collection that interrupts an entry and its restart's task, the entry reads the
/// value it showed, as Compose's `Interrupted` state (its animation still at the old fraction
/// would lerp the new start), D1585. The collection here is a test's, in a scope of its own
/// whose task hasn't run.
#[test]
fn an_interrupted_entry_shows_where_it_was_until_its_restart_runs() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.provide(&CUSTOM_FLOAT, 0.0);
        s.pressed(|s| s.animate_with(tween(500), |s| s.provide(&CUSTOM_FLOAT, 100.0)));
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
    state.set_pressed(true);
    for _ in 0..12 {
        ui.advance_time_by_frame();
    }
    let read = || resolver.resolve(|s| s.value(&CUSTOM_FLOAT));
    let shown = read();
    assert!(shown > 10.0 && shown < 90.0, "partway: {shown}");
    state.set_pressed(false);
    let scope = compose_runtime::CoroutineScope::new();
    resolver.collect_for_test_in(ui.density(), *scope);
    assert_eq!(read(), shown);
}

static NESTED: NestedStyleKey = NestedStyleKey::new("nested");

/// A parent whose child attached (and reached it) first: the parent's own collection is still
/// its first (a resolver not yet collected isn't read by a traversal reaching it), and shows its
/// animated values at once (D1587).
#[test]
fn a_parent_reached_by_its_child_first_shows_its_animated_values_at_once() {
    let parent = StyleResolver::of(CommonStyle::new(|s| {
        s.animate_with(tween(1000), |s| s.provide(&CUSTOM_FLOAT, 100.0));
        s.provide_nested_style(&NESTED, &CommonStyle::new(|s| s.provide(&CUSTOM_FLOAT, 1.0)));
    }));
    let child = StyleResolver::of(CommonStyle::new(|s| s.apply_nested_style(&NESTED)));
    let (bound_parent, bound_child) = (parent.clone(), child.clone());
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        let child = bound_child.clone();
        Box!(modifier = styleable(Modifier.size(40.dp()), &bound_parent), move |_| {
            Box!(modifier = styleable(Modifier.size(20.dp()), &child));
        });
    });
    let read = || parent.resolve(|s| s.value(&CUSTOM_FLOAT));
    let mut values = vec![read()];
    for _ in 0..5 {
        ui.advance_time_by_frame();
        values.push(read());
    }
    assert!(values.iter().all(|value| *value == 100.0), "animated in: {values:?}");
    assert_eq!(child.resolve(|s| s.value(&CUSTOM_FLOAT)), 1.0);
}

static PARENT_LOCAL: ProvidableStyleProperty<f32> =
    style_local_of_with("parentLocal", |a, b, t| compose_ui::lerp(*a, *b, t), || 0.0);

/// A parent resolver nothing applies, read only through a child (its local), animates: a read
/// through a descendant is a read (Compose's traversal resolves each ancestor it reaches), so
/// its later collections aren't first ones (D1587).
#[test]
fn a_parent_read_only_through_its_child_animates_its_locals() {
    let state = MutableStyleState::new(None);
    let parent = StyleResolver::new(
        CommonStyle::new(|s| {
            s.provide(&PARENT_LOCAL, 0.0);
            s.pressed(|s| s.animate_with(tween(500), |s| s.provide(&PARENT_LOCAL, 100.0)));
        }),
        state.clone(),
    );
    let child = StyleResolver::of(CommonStyle::EMPTY);
    let (bound_parent, bound_child) = (parent.clone(), child.clone());
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        let child = bound_child.clone();
        Box!(modifier = Modifier.style_resolver(bound_parent.clone()), move |_| {
            Box!(modifier = styleable(Modifier.size(20.dp()), &child));
        });
    });
    let read = || child.resolve(|s| s.value(&PARENT_LOCAL));
    assert_eq!(read(), 0.0);
    state.set_pressed(true);
    let mut values = Vec::new();
    for _ in 0..40 {
        ui.advance_time_by_frame();
        values.push(read());
    }
    assert!(values.iter().any(|value| *value > 0.0 && *value < 100.0), "jumped: {values:?}");
    assert_eq!(*values.last().unwrap(), 100.0);
}

/// `animating_font_weight`: the font weight and content color locals animate on press, as
/// Compose's font weight `lerp` and colors; the text motion is animated while they differ.
#[test]
fn animating_font_weight() {
    use compose_ui::{FontWeight, TextMotion};
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.content_padding(12.dp());
        s.provide(&CONTENT_FILL, Fill::Color(Color::new(0.68, 0.78, 1.0, 1.0)));
        s.provide(&FONT_WEIGHT, FontWeight::MEDIUM);
        s.provide(&FONT_SIZE, 14.sp());
        s.pressed(|s| {
            s.animate_with(tween(300), |s| {
                s.provide(&CONTENT_FILL, Fill::Color(Color::new(0.13, 0.48, 1.0, 1.0)));
                s.background(Color::new(0.78, 0.89, 1.0, 1.0));
                s.provide(&FONT_WEIGHT, FontWeight::BOLD);
            });
        });
    });
    let resolver = StyleResolver::new(style, state.clone());
    let mut ui = animated(resolver.clone(), Rc::new(Cell::new(0)));
    state.set_pressed(true);
    let mut weights = Vec::new();
    for _ in 0..30 {
        ui.advance_time_by_frame();
        weights.push(resolver.resolve(|s| s.value(&FONT_WEIGHT)));
    }
    assert!(
        weights.iter().any(|weight| *weight > FontWeight::MEDIUM && *weight < FontWeight::BOLD)
    );
    assert_eq!(*weights.last().unwrap(), FontWeight::BOLD);
    assert_eq!(resolver.resolve(|s| s.content_color()), Color::new(0.13, 0.48, 1.0, 1.0));
    assert_eq!(TEXT_MOTION.lerp(&TextMotion::Static, &TextMotion::Static, 0.5), TextMotion::Static);
    assert_eq!(
        TEXT_MOTION.lerp(&TextMotion::Static, &TextMotion::Animated, 0.1),
        TextMotion::Animated
    );
}
