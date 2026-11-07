//! Review of `shared-elements-3` (D1337): a `GraphicsLayer`'s readers, each with a version of its
//! own, what each set itself and what it saw.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::support::*;

use compose_runtime::snapshots::Snapshot;
use compose_runtime::*;
use compose_ui::platform::*;
use compose_ui::*;

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

/// A layer recorded once and never changed, drawn by a new node each step (a lazy list's items
/// coming and going, each drawing one layer the list holds): the drawings gone stay among the
/// layer's readers, each with a state of its own, and each drawing of the layer scans them all.
/// Told readers make room for new ones (`told_readers_make_room_for_new_ones`), but a layer that
/// doesn't change tells no one. In Compose a drawing keeps nothing in the layer.
#[test]
#[ignore = "review: a GraphicsLayer that doesn't change keeps a reader (and a state) for every \
            drawing that ever drew it, gone or not: unbounded in a long-running app"]
fn a_layer_drawn_by_drawings_coming_and_going_keeps_few_readers() {
    let layer = GraphicsLayer::new();
    let step = mutable_state_of(0u32);
    let s = step.handle();
    let given = layer.clone();
    let mut host = host(move || {
        let layer = given.clone();
        key(s.get(), || {
            let layer = layer.clone();
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_behind(move |scope| scope.draw_layer(&layer))
                    .size(10.dp()),
            );
        });
    });
    for next in 1..50u32 {
        step.set(next);
        host.frame();
    }
    assert!(layer.reader_count() <= 2, "{} readers", layer.reader_count());
}

/// The same with drawings that each set the layer's alpha before drawing it while another
/// drawing animates the alpha: a change to what a reader set itself tells it nothing, so it's
/// never told and its place is never taken.
#[test]
#[ignore = "review: a reader that set what changes is never told, so a drawing gone that set a \
            layer's alpha keeps its place however often the alpha changes"]
fn drawings_gone_that_set_what_changes_leave_the_readers() {
    let layer = GraphicsLayer::new();
    let step = mutable_state_of(0u32);
    let s = step.handle();
    let given = layer.clone();
    let mut host = host(move || {
        let layer = given.clone();
        key(s.get(), || {
            let layer = layer.clone();
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_behind(move |scope| {
                        layer.set_alpha(0.5);
                        scope.draw_layer(&layer);
                    })
                    .size(10.dp()),
            );
        });
    });
    for next in 1..50u32 {
        step.set(next);
        host.frame();
        layer.set_alpha(if next % 2 == 0 { 1.0 } else { 0.25 });
        host.frame();
    }
    assert!(layer.reader_count() <= 2, "{} readers", layer.reader_count());
}

/// A drawing setting two properties, then drawing the layer, and another setting both to other
/// values and drawing it: each set both itself, so neither is told and the screen goes idle.
/// (Kills a mutant noting only the last property a run set.)
#[test]
fn drawings_each_setting_two_properties_go_idle() {
    let layer = GraphicsLayer::new();
    let step = mutable_state_of(0u32);
    let s = step.handle();
    let (left, right) = (layer.clone(), layer.clone());
    let mut host = host(move || {
        let (left, right) = (left.clone(), right.clone());
        Column!({
            let (left, right) = (left.clone(), right.clone());
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_behind(move |scope| {
                        s.get();
                        log("left");
                        left.set_alpha(0.5);
                        left.set_top_left(IntOffset::new(0, 0));
                        scope.draw_layer(&left);
                    })
                    .size(10.dp()),
            );
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_behind(move |scope| {
                        log("right");
                        right.set_alpha(1.0);
                        right.set_top_left(IntOffset::new(5, 0));
                        scope.draw_layer(&right);
                    })
                    .size(10.dp()),
            );
        });
    });
    for _ in 0..3 {
        host.frame();
    }
    take_log();
    for next in 1..4 {
        step.set(next);
        host.frame();
        assert_eq!(take_log(), ["left"], "the right one sets both itself");
        host.frame();
        assert!(take_log().is_empty(), "idle");
    }
}

/// A drawing that set a layer's alpha in one run and not in the next (a state it reads) is told
/// of a later change of the alpha: what an earlier run of it set isn't what it draws now.
/// (Kills a mutant taking any run of the drawing as the one that set the alpha.)
#[test]
fn a_drawing_that_stopped_setting_a_property_hears_of_its_change() {
    let layer = GraphicsLayer::new();
    let sets = mutable_state_of(true);
    let s = sets.handle();
    let given = layer.clone();
    let mut host = host(move || {
        let layer = given.clone();
        Leaf(
            Modifier
                .repaint_boundary()
                .draw_behind(move |scope| {
                    log("drawer");
                    if s.get() {
                        layer.set_alpha(0.5);
                    }
                    scope.draw_layer(&layer);
                })
                .size(10.dp()),
        );
    });
    host.frame();
    take_log();
    sets.set(false);
    host.frame();
    assert_eq!(take_log(), ["drawer"]);
    // Nothing set anything since: the last writer is still the drawing's first run.
    layer.set_alpha(0.25);
    host.frame();
    assert_eq!(take_log(), ["drawer"], "told: its run now didn't set the alpha");
    assert!(host.display_list().dump().contains("0.25"), "{}", host.display_list().dump());
}

/// A drawing that draws a layer after its first change in the pass (another drawing moving it)
/// hears of a recording of it later in the pass, in that frame: it saw the content before it.
/// (Kills a mutant comparing only the properties with what a reader saw.)
#[test]
fn a_drawing_after_a_change_hears_of_a_recording_after_it() {
    let layer = GraphicsLayer::new();
    let step = mutable_state_of(0u32);
    let color = mutable_state_of(Color::RED);
    let (s, shade) = (step.handle(), color.handle());
    let (moved, drawn, recorded) = (layer.clone(), layer.clone(), layer.clone());
    let mut host = host(move || {
        let (moved, drawn, recorded) = (moved.clone(), drawn.clone(), recorded.clone());
        Column!({
            let (moved, drawn, recorded) = (moved.clone(), drawn.clone(), recorded.clone());
            Leaf(setting("mover", move || moved.set_translation_x(s.get() as f32)));
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_behind(move |scope| {
                        s.get();
                        log("drawer");
                        scope.draw_layer(&drawn);
                    })
                    .size(10.dp()),
            );
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_with_content(move |scope| {
                        s.get();
                        log("recorder");
                        scope.record_layer(&recorded, |scope| {
                            let size = scope.size();
                            let rect = Rect::new(0.0, 0.0, size.width, size.height);
                            scope.draw_rect(rect, Paint::fill(shade.get()));
                        });
                    })
                    .size(10.dp()),
            );
        });
    });
    for _ in 0..3 {
        host.frame();
    }
    take_log();
    step.set(1);
    color.set(Color::BLUE);
    host.frame();
    assert_eq!(take_log(), ["mover", "drawer", "recorder", "drawer"], "told in the frame");
    let dump = host.display_list().dump();
    assert!(!dump.contains("#FFFF0000"), "the old recording drawn: {dump}");
    host.frame();
    assert!(take_log().is_empty(), "idle");
}

/// A layer recorded by a drawing in one window and drawn in another: the other window draws it
/// again showing the new recording (its reader told at the first window's pass's end, the send
/// in that window's second pass reaching the other's observer).
#[test]
fn a_layer_recorded_in_one_window_is_drawn_again_in_another() {
    let layer = GraphicsLayer::new();
    let color = mutable_state_of(Color::RED);
    let shade = color.handle();
    let recorded = layer.clone();
    let mut recorder = host(move || {
        let recorded = recorded.clone();
        Leaf(
            Modifier
                .repaint_boundary()
                .draw_with_content(move |scope| {
                    log("recorder");
                    scope.record_layer(&recorded, |scope| {
                        let size = scope.size();
                        let rect = Rect::new(0.0, 0.0, size.width, size.height);
                        scope.draw_rect(rect, Paint::fill(shade.get()));
                    });
                })
                .size(10.dp()),
        );
    });
    let drawn = layer.clone();
    let mut drawer = UiHost::headless(200, 100);
    drawer.set_content(move || {
        let drawn = drawn.clone();
        Leaf(
            Modifier
                .repaint_boundary()
                .draw_behind(move |scope| {
                    log("drawer");
                    scope.draw_layer(&drawn);
                })
                .size(10.dp()),
        );
    });
    drawer.frame();
    assert!(drawer.display_list().dump().contains("#FFFF0000"));
    take_log();
    color.set(Color::BLUE);
    recorder.frame();
    assert_eq!(take_log(), ["recorder"]);
    assert!(drawer.needs_frame(), "the other window asked for a frame");
    drawer.frame();
    assert_eq!(take_log(), ["drawer"]);
    let dump = drawer.display_list().dump();
    assert!(dump.contains("#FF0000FF") && !dump.contains("#FFFF0000"), "{dump}");
    recorder.frame();
    drawer.frame();
    assert!(take_log().is_empty(), "idle");
}

/// A drawing drawing a layer for the first time makes the layer's reader a version in a snapshot
/// of its own (`new_version`), and taking a snapshot sends the global snapshot's changes
/// (`take_root`'s `send(true)`, as `advanceGlobalSnapshot`): what drawing code wrote before is
/// sent in the middle of the draw, its apply observers called there. Compose's `drawLayer`
/// takes no snapshot.
#[test]
#[ignore = "review: a GraphicsLayer's first drawing by a drawing sends the global snapshot's \
            changes (apply observers run) in the middle of the draw pass"]
fn a_first_drawing_of_a_layer_sends_nothing_mid_draw() {
    let layer = GraphicsLayer::new();
    let written = mutable_state_of(0u32);
    let step = mutable_state_of(0u32);
    let (w, s) = (written.handle(), step.handle());
    let heard: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let (in_draw, given) = (heard.clone(), layer.clone());
    let mut host = host(move || {
        let (layer, in_draw) = (given.clone(), in_draw.clone());
        Leaf(
            Modifier
                .repaint_boundary()
                .draw_behind(move |scope| {
                    let step = s.get();
                    in_draw.borrow_mut().push("draw");
                    if step > 0 {
                        w.set(step);
                        scope.draw_layer(&layer);
                    }
                    in_draw.borrow_mut().push("drawn");
                })
                .size(10.dp()),
        );
    });
    let observed = heard.clone();
    let _handle = Snapshot::register_apply_observer(move |_, _| observed.borrow_mut().push("sent"));
    heard.borrow_mut().clear();
    step.set(1);
    host.frame();
    let heard = heard.borrow().clone();
    let at = heard.iter().position(|it| *it == "draw").expect("drawn");
    assert_eq!(heard[at + 1], "drawn", "a send mid-draw: {heard:?}");
}

/// A drawing that releases a layer and then draws it (nothing) set its content itself: a
/// recording of the layer later in the pass doesn't tell it, as running again it releases the
/// layer again first. (Kills a mutant not noting a release as the run's.)
#[test]
fn a_drawing_releasing_a_layer_then_drawing_it_is_not_told_of_a_later_recording() {
    let layer = GraphicsLayer::new();
    let step = mutable_state_of(0u32);
    let s = step.handle();
    let (released, recorded) = (layer.clone(), layer.clone());
    let mut host = host(move || {
        let (released, recorded) = (released.clone(), recorded.clone());
        Column!({
            let (released, recorded) = (released.clone(), recorded.clone());
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_behind(move |scope| {
                        s.get();
                        log("releaser");
                        released.release();
                        scope.draw_layer(&released);
                    })
                    .size(10.dp()),
            );
            Leaf(
                Modifier
                    .repaint_boundary()
                    .draw_with_content(move |scope| {
                        s.get();
                        log("recorder");
                        scope.record_layer(&recorded, |scope| {
                            let size = scope.size();
                            let rect = Rect::new(0.0, 0.0, size.width, size.height);
                            scope.draw_rect(rect, Paint::fill(Color::RED));
                        });
                    })
                    .size(10.dp()),
            );
        });
    });
    for _ in 0..3 {
        host.frame();
    }
    take_log();
    for next in 1..4 {
        step.set(next);
        host.frame();
        assert_eq!(take_log(), ["releaser", "recorder"], "the releaser not told");
        host.frame();
        assert!(take_log().is_empty(), "idle");
    }
}
