//! Compose's `StyleUxTaskTests` (device): its tasks compose a style and show it, asserting
//! nothing; here each also checks what it draws or measures. A task and its `_n` form (the
//! deprecated `styleable(state, style)` and an explicitly remembered resolver) are both the
//! resolver chain in compose-rs (D1580): the `_n` form builds its resolver by hand.

#![allow(non_snake_case)]

use compose_animation::tween;
use compose_foundation::layout::*;
use compose_foundation::shape::{CIRCLE_SHAPE, RoundedCornerShape};
use compose_foundation::style::properties::*;
use compose_foundation::style::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::DrawOp;
use compose_ui::{
    Brush, ClickableOptions, Indication, IntSize, MutableInteractionSource, Offset, UnitExt,
};

use crate::style_support::{StyleSetters, styleable, styled};

const TAG: &str = "task";

type Block = fn(&mut dyn CommonStyleScope);

/// A task's box: `styleable(null, style)`, or with `explicit` its resolver remembered by hand.
fn task(style: Block, explicit: bool) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let modifier = if explicit {
            let resolver = remember_observer(move || StyleResolver::of(CommonStyle::new(style)));
            styleable(Modifier.test_tag(TAG), &resolver)
        } else {
            styled(Modifier.test_tag(TAG), None, style)
        };
        Box!(modifier = modifier, |_| {
            BasicText!("Task");
        });
    })
}

/// What's drawn: each rectangle, rounded rectangle and path's color, in order.
fn drawn(ui: &mut ComposeTestRule) -> Vec<Color> {
    let list = ui.host().display_list();
    list.ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::DrawRect(_, paint)
            | DrawOp::DrawRoundRect(_, _, paint)
            | DrawOp::DrawPath { paint, .. } => Some(paint.color),
            _ => None,
        })
        .collect()
}

fn task1_style(s: &mut dyn CommonStyleScope) {
    s.border(3.dp(), Color::MAGENTA);
    s.content_padding(10.dp());
}

fn check_task1(explicit: bool) {
    let mut ui = task(task1_style, explicit);
    assert!(drawn(&mut ui).contains(&Color::MAGENTA));
}

#[test]
fn task1() {
    check_task1(false);
}

#[test]
fn task1_n() {
    check_task1(true);
}

fn check_task2(explicit: bool) {
    let mut ui = task(|s| s.background(Color::RED), explicit);
    assert_eq!(drawn(&mut ui).first(), Some(&Color::RED));
}

#[test]
fn task2() {
    check_task2(false);
}

#[test]
fn task2_n() {
    check_task2(true);
}

fn check_task3(explicit: bool) {
    let mut ui = task(
        |s| {
            s.content_padding(20.dp());
            s.size(100.dp());
        },
        explicit,
    );
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(100.dp()).assert_height_is_equal_to(100.dp());
}

#[test]
fn task3() {
    check_task3(false);
}

#[test]
fn task3_n() {
    check_task3(true);
}

fn check_task4(explicit: bool) {
    let mut ui = task(
        |s| {
            s.shape(CIRCLE_SHAPE);
            s.border(2.dp(), Color::BLUE);
            s.background(Color::LIGHT_GRAY);
            s.size(100.dp());
        },
        explicit,
    );
    assert_eq!(drawn(&mut ui), vec![Color::LIGHT_GRAY, Color::BLUE]);
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(100.dp());
}

#[test]
fn task4() {
    check_task4(false);
}

#[test]
fn task4_n() {
    check_task4(true);
}

fn check_task5(explicit: bool) {
    let mut ui = task(
        |s| {
            s.content_padding(12.dp());
            s.shape(RoundedCornerShape(10.dp()));
            s.background(Color::MAGENTA);
            s.border(1.dp(), Color::BLACK);
            s.size(150.dp());
        },
        explicit,
    );
    assert_eq!(drawn(&mut ui), vec![Color::MAGENTA, Color::BLACK]);
    ui.on_node_with_tag(TAG).assert_width_is_equal_to(150.dp());
}

#[test]
fn task5() {
    check_task5(false);
}

#[test]
fn task5_n() {
    check_task5(true);
}

/// `task8`: a clickable circle, blue while pressed (its source sets the state).
fn check_task8(explicit: bool) {
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let source = remember(MutableInteractionSource::new);
        let state = if explicit {
            let source = source.as_source().clone();
            remember_updated_style_state(Some(&source), |_| {})
        } else {
            let source = source.as_source().clone();
            remember(move || MutableStyleState::new(Some(source)).state().clone())
        };
        let resolver = remember_observer(move || {
            let style = CommonStyle::new(|s| {
                s.background(Color::LIGHT_GRAY);
                s.size(100.dp());
                s.shape(CIRCLE_SHAPE);
                s.pressed(|s| s.background(Color::BLUE));
            });
            StyleResolver::new(style, state)
        });
        let options = ClickableOptions {
            indication: Indication::NONE,
            interaction_source: Some(source.clone()),
            ..ClickableOptions::DEFAULT
        };
        Box!(
            modifier = styleable(Modifier.test_tag(TAG).clickable_with(options, || {}), &resolver),
            content_alignment = Alignment::CENTER,
            |_| BasicText!("Click Me")
        );
    });
    assert_eq!(drawn(&mut ui).first(), Some(&Color::LIGHT_GRAY));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.down(Offset::new(50.0, 50.0)));
    ui.wait_for_idle();
    assert_eq!(drawn(&mut ui).first(), Some(&Color::BLUE));
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
    assert_eq!(drawn(&mut ui).first(), Some(&Color::LIGHT_GRAY));
}

#[test]
fn task8() {
    check_task8(false);
}

#[test]
fn task8_n() {
    check_task8(true);
}

/// A clickable box styled by `style`, pressed by a touch, its background read frame by frame
/// while it animates.
fn pressed_backgrounds(style: CommonStyle) -> Vec<Fill> {
    let resolver_slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot = resolver_slot.clone();
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        let source = remember(MutableInteractionSource::new);
        let source_handle = source.as_source().clone();
        let state = remember_updated_style_state(Some(&source_handle), |_| {});
        let style = style.clone();
        let resolver = remember_observer(move || StyleResolver::new(style, state));
        slot.replace(Some((*resolver).clone()));
        let options = ClickableOptions {
            indication: Indication::NONE,
            interaction_source: Some(source.clone()),
            ..ClickableOptions::DEFAULT
        };
        Box!(
            modifier = styleable(Modifier.test_tag(TAG).clickable_with(options, || {}), &resolver)
        );
    });
    ui.advance_time_by_frame();
    let resolver = resolver_slot.borrow().clone().expect("composed");
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.down(Offset::new(10.0, 10.0)));
    let mut values = Vec::new();
    for _ in 0..90 {
        ui.advance_time_by_frame();
        values.push(resolver.resolve(|s| s.value(&BACKGROUND)));
    }
    values
}

/// `animating_background_press`: a pressed button's background animates blue to green.
#[test]
fn animating_background_press() {
    let style = CommonStyle::new(|s| {
        s.content_padding(12.dp());
        s.shape(RoundedCornerShape(50));
        s.border(1.dp(), Color::BLACK);
        s.background(Color::BLUE);
        s.provide(&CONTENT_FILL, Fill::Color(Color::WHITE));
        s.size(80.dp());
        s.pressed(|s| s.animate(|s| s.background(Color::GREEN)));
    });
    let values = pressed_backgrounds(style);
    assert!(
        values
            .iter()
            .any(|fill| *fill != Fill::Color(Color::BLUE) && *fill != Fill::Color(Color::GREEN))
    );
    assert_eq!(*values.last().unwrap(), Fill::Color(Color::GREEN));
}

/// `style_color_brush_animations`: color to color, color to brush and brush to color, each
/// over a 1 s tween (compose-rs's gradients jump half way, solid brushes lerp).
#[test]
fn style_color_brush_animations() {
    let end_brush = Brush::Solid(Color::YELLOW);
    for (start, end) in [
        (Fill::Color(Color::BLUE), Fill::Color(Color::YELLOW)),
        (Fill::Color(Color::BLUE), Fill::Brush(end_brush.clone())),
        (Fill::Brush(Brush::Solid(Color::RED)), Fill::Color(Color::BLUE)),
    ] {
        let (from, to) = (start.clone(), end.clone());
        let style = CommonStyle::new(move |s| {
            s.size(150.dp());
            s.provide(&BACKGROUND, from.clone());
            let to = to.clone();
            s.pressed(move |s| {
                let to = to.clone();
                s.animate_with(tween(1000), move |s| s.provide(&BACKGROUND, to.clone()));
            });
        });
        let values = pressed_backgrounds(style);
        assert!(values.iter().any(|fill| *fill != start && *fill != end), "{start:?} to {end:?}");
        assert_eq!(*values.last().unwrap(), end);
    }
}
