//! Review of `shared-elements-2` (D1332): a `GraphicsLayer`'s two versions, the drawings reading
//! the other one after its first change in a draw pass, and what the pass's end tells.

#![allow(non_snake_case)]

use crate::support::*;

use compose_runtime::*;
use compose_ui::*;

/// A boundary of 10 × 10 logging `name` and drawing `layer` with `before` applied to it first.
fn drawing_after(
    name: &'static str,
    layer: &GraphicsLayer,
    before: impl Fn(&GraphicsLayer) + 'static,
) -> Modifier {
    let layer = layer.clone();
    Modifier
        .repaint_boundary()
        .draw_behind(move |scope| {
            log(name);
            before(&layer);
            scope.draw_layer(&layer);
        })
        .size(10.dp())
}

/// A boundary of 10 × 10 logging `name` and running `change` as it draws, drawing nothing.
fn setting(name: &'static str, change: impl Fn() + 'static) -> Modifier {
    Modifier
        .repaint_boundary()
        .draw_behind(move |_| {
            log(name);
            change();
        })
        .size(10.dp())
}

/// Two drawings drawing one layer, each moving it first where it draws it (its `top_left`): each
/// frame's pass moves it, has it drawn, and moves it back where the pass found it. Before D1332
/// the pass's end saw the layer as it started and told no one; now the first drawing, which drew
/// it after a change undone later, is told, draws again in the second pass, which does the same,
/// and so every frame after: the screen never goes idle.
#[test]
fn two_drawings_placing_one_layer_go_idle() {
    let layer = GraphicsLayer::new();
    let given = layer.clone();
    let mut host = host(move || {
        let layer = given.clone();
        Column!({
            Leaf(drawing_after("left", &layer, |layer| layer.set_top_left(IntOffset::new(0, 0))));
            Leaf(drawing_after("right", &layer, |layer| layer.set_top_left(IntOffset::new(5, 0))));
        });
    });
    // Settled: the first frames may draw them again (the layer moved from its default).
    for _ in 0..3 {
        host.frame();
    }
    take_log();
    for _ in 0..3 {
        host.frame();
    }
    let log = take_log();
    assert!(log.is_empty(), "drawn again with nothing changed: {log:?}");
    assert!(!host.needs_frame(), "a frame asked for with nothing changed");
}

/// The same with one drawing drawing a layer twice, its alpha set before each (a ghost under the
/// content): idle once drawn.
#[test]
fn a_drawing_drawing_a_layer_twice_with_two_alphas_goes_idle() {
    let layer = GraphicsLayer::new();
    let given = layer.clone();
    let mut host = host(move || {
        let layer = given.clone();
        Leaf(
            Modifier
                .repaint_boundary()
                .draw_behind(move |scope| {
                    log("ghost");
                    layer.set_alpha(1.0);
                    scope.draw_layer(&layer);
                    layer.set_alpha(0.5);
                    scope.draw_layer(&layer);
                })
                .size(10.dp()),
        );
    });
    for _ in 0..3 {
        host.frame();
    }
    take_log();
    for _ in 0..3 {
        host.frame();
    }
    let log = take_log();
    assert!(log.is_empty(), "drawn again with nothing changed: {log:?}");
}

/// A drawing that recorded a layer and then drew it (current, so reading the other version) moves
/// onto the version later changes bump: a property set between frames draws it again. Kills a
/// mutant not swapping `read` at the pass's end, and one never marking a read `Current`.
#[test]
fn a_drawing_that_recorded_then_drew_a_layer_hears_of_a_change_between_frames() {
    let layer = GraphicsLayer::new();
    let color = mutable_state_of(Color::RED);
    let (own, shade) = (layer.clone(), color.handle());
    let mut host = host(move || {
        let own = own.clone();
        Leaf(
            Modifier
                .repaint_boundary()
                .draw_with_content(move |scope| {
                    log("recorder");
                    scope.record_layer(&own, |scope| {
                        let size = scope.size();
                        let rect = Rect::new(0.0, 0.0, size.width, size.height);
                        scope.draw_rect(rect, Paint::fill(shade.get()));
                    });
                    scope.draw_layer(&own);
                })
                .size(10.dp()),
        );
    });
    assert_eq!(take_log(), ["recorder"]);
    for next in [Color::BLUE, Color::GREEN] {
        color.set(next);
        host.frame();
        assert_eq!(take_log(), ["recorder"], "drawn once");
        // Outside a pass: what drew it draws again.
        layer.set_alpha(0.5);
        host.frame();
        assert_eq!(take_log(), ["recorder"], "told of the alpha");
        layer.set_alpha(1.0);
        host.frame();
        assert_eq!(take_log(), ["recorder"], "told of the alpha back");
        host.frame();
        assert!(take_log().is_empty(), "settled");
    }
}

/// A drawing that drew a layer after its change in a pass whose changes undid each other moves
/// back onto the version drawings read: a later change between frames draws it again, and so
/// does a second one. Kills a mutant reading the swapped version without telling.
#[test]
fn a_drawing_told_back_onto_the_read_version_hears_of_two_later_changes() {
    let layer = GraphicsLayer::new();
    let step = mutable_state_of(0u32);
    let s = step.handle();
    let (first, last, drawn) = (layer.clone(), layer.clone(), layer.clone());
    let mut host = host(move || {
        let (first, last, drawn) = (first.clone(), last.clone(), drawn.clone());
        Column!({
            let (first, last, drawn) = (first.clone(), last.clone(), drawn.clone());
            // Each step: 0.3 then back to 1 before the drawer draws it, drawing nothing.
            Leaf(setting("first", move || {
                s.get();
                first.set_alpha(0.3);
            }));
            Leaf(setting("last", move || {
                s.get();
                last.set_alpha(1.0);
            }));
            Leaf(drawing_after("drawer", &drawn, |_| {}));
        });
    });
    for _ in 0..3 {
        host.frame();
    }
    take_log();
    step.set(1);
    host.frame();
    take_log();
    for scale in [0.7, 0.9] {
        // Bumps the version the drawings read.
        layer.set_scale_x(scale);
        host.frame();
        let log = take_log();
        assert!(log.iter().any(|it| it == "drawer"), "the drawer told: {log:?}");
    }
}
