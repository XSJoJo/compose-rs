//! Hover: a mouse over a clickable shows Material's 8 % state layer, gone when it leaves the
//! clickable or the window.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::ClickableOptions;
use compose_ui::{IntSize, Offset, PointerEvent, UnitExt, platform::DrawOp};

fn layer_alpha(ui: &mut ComposeTestRule) -> Option<u8> {
    ui.host().display_list().ops.iter().find_map(|op| match op {
        DrawOp::DrawRect(_, paint) if paint.color.red() == 255 && paint.color.alpha() < 255 => {
            Some(paint.color.alpha())
        }
        _ => None,
    })
}

#[test]
fn hovering_a_clickable_shows_its_state_layer() {
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), || {
        Column!({
            Box!(
                modifier = Modifier.size(50.dp()).clickable_with(
                    ClickableOptions { indication: Color::RED.into(), ..ClickableOptions::DEFAULT },
                    || {}
                )
            );
            Box!(modifier = Modifier.size(50.dp()));
        });
    });
    assert_eq!(layer_alpha(&mut ui), None);
    ui.host().send_pointer(PointerEvent::moved(Offset::new(10.0, 10.0), false));
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), Some(20), "8 %");
    // Pressed: 10 %.
    ui.host().send_pointer(PointerEvent::press(Offset::new(10.0, 10.0)));
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), Some(26));
    ui.host().send_pointer(PointerEvent::release(Offset::new(10.0, 10.0)));
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), Some(20), "still hovered");
    // Moving off it, then leaving the window.
    ui.host().send_pointer(PointerEvent::moved(Offset::new(10.0, 70.0), false));
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), None);
    ui.host().send_pointer(PointerEvent::moved(Offset::new(10.0, 10.0), false));
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), Some(20));
    ui.host().send_pointer_left();
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), None);
}

#[test]
fn a_text_field_shows_an_i_beam() {
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), || {
        Column!({
            BasicTextField!(String::new(), |_| {}, modifier = Modifier.size((100.dp(), 30.dp())));
            Box!(
                modifier = Modifier.size(50.dp()).pointer_hover_icon(compose_ui::PointerIcon::HAND)
            );
        });
    });
    ui.host().send_pointer(PointerEvent::moved(Offset::new(10.0, 10.0), false));
    assert_eq!(ui.host().pointer_icon(), compose_ui::PointerIcon::TEXT);
    ui.host().send_pointer(PointerEvent::moved(Offset::new(10.0, 50.0), false));
    assert_eq!(ui.host().pointer_icon(), compose_ui::PointerIcon::HAND);
    ui.host().send_pointer(PointerEvent::moved(Offset::new(150.0, 150.0), false));
    assert_eq!(ui.host().pointer_icon(), compose_ui::PointerIcon::DEFAULT);
}

#[test]
fn a_tooltip_shows_after_the_pointer_rests_and_hides_when_it_leaves() {
    use std::time::Duration;
    let mut ui = ComposeTestRule::with_manual_clock(|| {
        Column!({
            TooltipArea!(
                || BasicText!("Saves the file", modifier = Modifier.test_tag("tooltip")),
                {
                    Box!(modifier = Modifier.size(50.dp()).test_tag("area"));
                }
            );
            Box!(modifier = Modifier.size(50.dp()));
        });
    });
    // A move, then the frame that recomposes for it: Compose recomposes only on frames (D900).
    let rest = |ui: &mut ComposeTestRule, x: f32, y: f32| {
        ui.host().send_pointer(PointerEvent::moved(Offset::new(x, y), false));
        ui.advance_time_by_frame();
    };
    rest(&mut ui, 10.0, 10.0);
    ui.advance_time_by(Duration::from_millis(300));
    // Moving starts the wait again.
    rest(&mut ui, 20.0, 20.0);
    ui.advance_time_by(Duration::from_millis(300));
    ui.on_node_with_tag("tooltip").assert_does_not_exist();
    ui.advance_time_by(Duration::from_millis(300));
    let tooltip = ui.on_node_with_tag("tooltip").fetch_semantics_snapshot().bounds;
    // At the pointer, 16 dp below it.
    assert_eq!((tooltip.left, tooltip.top), (20.0, 36.0));
    // Leaving hides it.
    rest(&mut ui, 10.0, 80.0);
    ui.on_node_with_tag("tooltip").assert_does_not_exist();
    // Back, shown again, then a press hides it.
    rest(&mut ui, 10.0, 10.0);
    ui.advance_time_by(Duration::from_millis(600));
    ui.on_node_with_tag("tooltip").assert_exists();
    ui.host().send_pointer(PointerEvent::press(Offset::new(10.0, 10.0)));
    ui.advance_time_by_frame();
    ui.on_node_with_tag("tooltip").assert_does_not_exist();
}

// Compose: no test of its own (`HoverableNode` hears the `Enter` and `Exit` of any pointer, a
// hovering stylus's too, D1677). A stylus hovering a clickable shows its state layer; its
// modifiers changing while hovered (nothing laid out again, so nothing hit-tested again), the
// stylus going out of range still ends the hover, as the
// mouse's (`review_round6.rs` `a_hovered_clickable_whose_modifiers_change_still_hears_exit`).
#[test]
fn a_hovering_stylus_hovers_a_clickable_whose_modifiers_change() {
    use compose_runtime::mutable_state_of;
    use compose_ui::{PointerId, PointerType};
    let bordered = mutable_state_of(false);
    let handle = bordered.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        Column!({
            let modifier = Modifier.size(50.dp());
            // A background: the chain changes, nothing is measured again.
            let modifier = if handle.get() { modifier.background(Color::BLUE) } else { modifier };
            Box!(
                modifier = modifier
                    .clickable_with(
                        ClickableOptions {
                            indication: Color::RED.into(),
                            ..ClickableOptions::DEFAULT
                        },
                        || {}
                    )
                    .size(48.dp())
            );
        });
    });
    let stylus = |x: f32, y: f32| {
        PointerEvent::moved(Offset::new(x, y), false)
            .with_pointer(PointerId(1), PointerType::Stylus)
    };
    ui.host().send_pointer(stylus(10.0, 10.0));
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), Some(20), "hovered");
    bordered.set(true);
    ui.wait_for_idle();
    ui.host().send_stylus_left();
    ui.wait_for_idle();
    assert_eq!(layer_alpha(&mut ui), None, "no longer hovered");
}
