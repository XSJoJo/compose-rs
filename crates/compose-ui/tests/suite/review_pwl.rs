//! The review's tests of placing with a layer (D1909): a placed layer's content drawn again after
//! the tree's last one went, rotations and scales mapped as Compose's, and hit testing as
//! documented.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::support::*;

use compose_runtime::*;
use compose_ui::*;

fn near(a: Offset, b: Offset) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

/// A layout 100 px square placing its only child at `at`, with the layer `block` gives (none for
/// `None`).
fn wrapper(
    at: (i32, i32),
    block: impl Fn() -> Option<LayerBlock> + 'static,
) -> Rc<dyn MeasurePolicy> {
    let block = Rc::new(block);
    measure_policy(move |scope, measurables, constraints| {
        let placeable = scope.measure(measurables[0], constraints);
        let block = block.clone();
        scope.layout_with(100, 100, move |placement| match block() {
            Some(block) => placement.place_with_layer(&placeable, at.0, at.1, block),
            None => placement.place(&placeable, at.0, at.1),
        })
    })
}

/// The tree's last placed layer goes (a layout modifier's, with a graphics layer at the same
/// first entry), then another node places with a layer: the first node's content still draws
/// again when a state it reads changes.
#[test]
fn content_draws_again_after_the_last_placed_layer_went_and_another_node_placed_one() {
    let layered = mutable_state_of(true);
    let other = mutable_state_of(false);
    let color = mutable_state_of(Color::RED);
    let (layered_handle, other_handle, color_handle) =
        (layered.handle(), other.handle(), color.handle());
    let modifier = Modifier
        .draw_behind(|scope| {
            let bounds = scope.bounds();
            scope.draw_rect(bounds, Paint::fill(Color::BLUE));
        })
        .layout(move |scope, measurable, constraints| {
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                if layered_handle.get() {
                    placement.place_with_layer(
                        &placeable,
                        0,
                        0,
                        |layer: &mut GraphicsLayerScope| layer.alpha = 0.5,
                    );
                } else {
                    placement.place(&placeable, 0, 0);
                }
            })
        })
        .graphics_layer(|layer| layer.alpha = 0.75)
        .draw_behind(move |scope| {
            let bounds = scope.bounds();
            scope.draw_rect(bounds, Paint::fill(color_handle.get()));
        })
        .size(10.dp());
    let policy = wrapper((0, 0), move || {
        other_handle
            .get()
            .then(|| LayerBlock::new(|layer: &mut GraphicsLayerScope| layer.alpha = 0.9))
    });
    let mut host = host(move || {
        let (modifier, policy) = (modifier.clone(), policy.clone());
        Column(
            Modifier,
            callback(move || {
                Leaf(modifier.clone());
                Layout(Modifier, policy.clone(), callback(|| Leaf(Modifier.size(10.dp()))));
            }),
        );
    });
    host.display_list();
    layered.set(false);
    host.frame();
    other.set(true);
    host.frame();
    color.set(Color::GREEN);
    host.frame();
    let dump = host.display_list().dump();
    assert!(dump.contains("#FF00FF00"), "{dump}");
    assert!(!dump.contains("#FFFF0000"), "{dump}");
}

/// After Compose: a 10x20 child placed at (10, 10) with `rotationZ = 90` turns around its center
/// (15, 20): its origin goes to (25, 15), its bounds to (5, 15)-(25, 25).
#[test]
fn a_rotated_placed_layer_maps_positions_and_bounds() {
    let parent: Rc<RefCell<Option<LayoutCoordinates>>> = Rc::default();
    let child: Rc<RefCell<Option<LayoutCoordinates>>> = Rc::default();
    let (parent_slot, child_slot) = (parent.clone(), child.clone());
    let block = LayerBlock::new(|layer: &mut GraphicsLayerScope| layer.rotation_z = 90.0);
    let policy = wrapper((10, 10), move || Some(block.clone()));
    let mut host = host(move || {
        let (parent_slot, child_slot) = (parent_slot.clone(), child_slot.clone());
        Layout(
            Modifier.on_globally_positioned(move |it: LayoutCoordinates| {
                *parent_slot.borrow_mut() = Some(it)
            }),
            policy.clone(),
            callback(move || {
                let child_slot = child_slot.clone();
                Leaf(Modifier.width(10.dp()).height(20.dp()).on_globally_positioned(
                    move |it: LayoutCoordinates| *child_slot.borrow_mut() = Some(it),
                ));
            }),
        )
    });
    host.frame();
    let (parent, child) = (parent.borrow().unwrap(), child.borrow().unwrap());
    let origin = child.position_in_root();
    assert!(near(origin, Offset::new(25.0, 15.0)), "{origin:?}");
    let local = parent.local_position_of(&child, Offset::ZERO);
    assert!(near(local, Offset::new(25.0, 15.0)), "{local:?}");
    let back = child.local_position_of(&parent, Offset::new(25.0, 15.0));
    assert!(near(back, Offset::ZERO), "{back:?}");
    for bounds in [child.bounds_in_root(), child.bounds_in_window()] {
        assert!(
            near(Offset::new(bounds.left, bounds.top), Offset::new(5.0, 15.0))
                && near(Offset::new(bounds.right, bounds.bottom), Offset::new(25.0, 25.0)),
            "{bounds:?}"
        );
    }
}

/// After Compose: a layout modifier placing its content with `translationX = 30` and scale 2
/// (its pivot the center); a child 4 px in (padding) of the content: Compose's `positionInRoot`
/// is ((4 - 10) * 2 + 10 + 30, (4 - 10) * 2 + 10) = (28, -2).
#[test]
fn a_layout_modifier_s_placed_layer_maps_a_child_inside_it() {
    let child: Rc<Cell<Offset>> = Rc::default();
    let child_slot = child.clone();
    let mut host = host(move || {
        let child_slot = child_slot.clone();
        Column(
            Modifier
                .layout(|scope, measurable, constraints| {
                    let placeable = scope.measure(measurable, constraints);
                    scope.layout_with(placeable.width, placeable.height, move |placement| {
                        placement.place_with_layer(
                            &placeable,
                            0,
                            0,
                            |layer: &mut GraphicsLayerScope| {
                                layer.translation_x = 30.0;
                                layer.scale_x = 2.0;
                                layer.scale_y = 2.0;
                            },
                        );
                    })
                })
                .size(20.dp())
                .padding(4.dp()),
            callback(move || {
                let child_slot = child_slot.clone();
                Leaf(Modifier.size(4.dp()).on_globally_positioned(move |it: LayoutCoordinates| {
                    child_slot.set(it.position_in_root())
                }));
            }),
        );
    });
    host.frame();
    assert!(near(child.get(), Offset::new(28.0, -2.0)), "{:?}", child.get());
}

/// Hit testing ignores a placed layer's transform, as it does a graphics layer's (a difference,
/// D1909): a child at (0, 0) drawn 50 px right is hit where it's laid out, not where it's drawn.
/// Compose's hit test goes through the layer (`fromParentPosition` inverts it).
#[test]
fn hit_testing_ignores_a_placed_layer_s_translation() {
    let block = LayerBlock::new(|layer: &mut GraphicsLayerScope| layer.translation_x = 50.0);
    let policy = wrapper((0, 0), move || Some(block.clone()));
    let mut host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| Leaf(Modifier.size(10.dp()).clickable(|| log("child")))),
        )
    });
    take_log();
    click(&mut host, 55.0, 5.0);
    assert_eq!(take_log(), Vec::<String>::new(), "where it's drawn");
    click(&mut host, 5.0, 5.0);
    assert_eq!(take_log(), vec!["child".to_string()], "where it's laid out");
}
