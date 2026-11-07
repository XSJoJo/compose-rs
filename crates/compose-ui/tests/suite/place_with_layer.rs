//! Placing with a layer (`PlacementScope::place_with_layer`, `place_with_graphics_layer`):
//! the layer placed is a group whose properties the placement's block gives, moved by its
//! placement without drawing again (docs/parity/design/place-with-layer.md, D1909).

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use crate::support::*;

use compose_runtime::*;
use compose_ui::platform::*;
use compose_ui::*;

type Count = Rc<Cell<u32>>;

fn bump(count: &Count) {
    count.set(count.get() + 1);
}

/// A leaf filling itself with `color`, logging `name` when it draws.
fn painted(name: &'static str, color: Color) -> Modifier {
    Modifier.draw_behind(move |scope| {
        log(name);
        let bounds = scope.bounds();
        scope.draw_rect(bounds, Paint::fill(color));
    })
}

/// A layout placing its only child where `at` says, with the layer `block` gives (none for
/// `None`), in a placement block: `at` and `block` read there place it again, measuring
/// nothing. It counts its measurements.
fn placing(
    measures: &Count,
    at: impl Fn() -> (i32, i32) + 'static,
    block: impl Fn() -> Option<LayerBlock> + 'static,
) -> Rc<dyn MeasurePolicy> {
    let measures = measures.clone();
    let (at, block) = (Rc::new(at), Rc::new(block));
    measure_policy(move |scope, measurables, constraints| {
        bump(&measures);
        let placeable = scope.measure(measurables[0], constraints);
        let (at, block) = (at.clone(), block.clone());
        scope.layout_with(100, 100, move |placement| {
            let (x, y) = at();
            match block() {
                Some(block) => placement.place_with_layer(&placeable, x, y, block),
                None => placement.place(&placeable, x, y),
            }
        })
    })
}

/// The group of key `key`.
fn group(list: &FlatDisplayList, key: u64) -> FlatGroup {
    list.groups
        .iter()
        .find(|group| group.key == key)
        .unwrap_or_else(|| panic!("no group {key}\n{}", list.dump()))
        .clone()
}

/// The groups' keys, in the order they start.
fn keys(list: &FlatDisplayList) -> Vec<u64> {
    let mut groups = list.groups.clone();
    groups.sort_by_key(|group| (group.start, std::cmp::Reverse(group.end)));
    groups.iter().map(|group| group.key).collect()
}

/// A child placed with a layer is a group whose properties are the block's, around what it
/// draws, as Compose's layer is a render node with them.
#[test]
fn a_child_placed_with_a_layer_is_a_group_with_the_block_s_properties() {
    let measures = Count::default();
    let block = LayerBlock::new(|layer| {
        layer.alpha = 0.5;
        layer.scale_x = 2.0;
    });
    let policy = placing(&measures, || (10, 20), move || Some(block.clone()));
    let host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| Leaf(painted("child", Color::RED).size(10.dp()))),
        )
    });
    let list = host.display_list();
    let [child] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    let layer = group(&list, child).layer.expect("the placement's layer");
    assert_eq!((layer.alpha, layer.scale), (0.5, (2.0, 1.0)));
    // Its content is in its own coordinates, where its parent placed it.
    assert!(list.dump().contains("translate 10 20"), "{}", list.dump());
}

/// A state read in the block changes the group's properties alone: nothing is measured, placed
/// or drawn again, and no version changes, as Compose's block runs in a layer's own observation.
#[test]
fn a_state_read_in_the_block_changes_only_the_layer_s_properties() {
    let alpha = mutable_state_of(1.0f32);
    let handle = alpha.handle();
    let measures = Count::default();
    let block = LayerBlock::new(move |layer| layer.alpha = handle.get());
    let policy = placing(&measures, || (0, 0), move || Some(block.clone()));
    let mut host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| Leaf(painted("child", Color::RED).size(10.dp()))),
        )
    });
    let before = host.display_list();
    let [child] = keys(&before)[..] else { panic!("one group\n{}", before.dump()) };
    take_log();
    alpha.set(0.25);
    host.frame();
    let after = host.display_list();
    assert_eq!(group(&after, child).layer.expect("a layer").alpha, 0.25);
    assert_eq!(group(&after, child).version, group(&before, child).version);
    assert_eq!(measures.get(), 1);
    assert!(take_log().is_empty(), "drew nothing again");
}

/// Moving a child placed with a layer moves its group: what it drew stays (same version, its
/// drawing not run), as Compose's moved layer draws nothing again.
#[test]
fn a_move_keeps_what_the_layer_drew() {
    let x = mutable_state_of(0);
    let handle = x.handle();
    let measures = Count::default();
    let block = LayerBlock::new(|_| {});
    let policy = placing(&measures, move || (handle.get(), 0), move || Some(block.clone()));
    let mut host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| Leaf(painted("child", Color::RED).size(10.dp()))),
        )
    });
    let before = host.display_list();
    let [child] = keys(&before)[..] else { panic!("one group\n{}", before.dump()) };
    take_log();
    x.set(30);
    host.frame();
    let after = host.display_list();
    assert_eq!(group(&after, child).version, group(&before, child).version);
    assert!(take_log().is_empty(), "drew nothing again");
    assert!(after.dump().contains("translate 30 0"), "{}", after.dump());
    assert_eq!(measures.get(), 1);
}

/// Placed without a layer, a child loses the one it had: it's no group any more, and its
/// drawing is in its parent's again.
#[test]
fn placing_without_a_layer_drops_the_layer() {
    let layered = mutable_state_of(true);
    let handle = layered.handle();
    let measures = Count::default();
    let block = LayerBlock::new(|layer| layer.alpha = 0.5);
    let policy = placing(&measures, || (0, 0), move || handle.get().then(|| block.clone()));
    let mut host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| Leaf(painted("child", Color::RED).size(10.dp()))),
        )
    });
    assert_eq!(keys(&host.display_list()).len(), 1);
    layered.set(false);
    host.frame();
    let list = host.display_list();
    assert!(keys(&list).is_empty(), "{}", list.dump());
    assert!(list.dump().contains("#FFFF0000"), "{}", list.dump());
    layered.set(true);
    host.frame();
    let list = host.display_list();
    let [child] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    assert_eq!(group(&list, child).layer.expect("a layer").alpha, 0.5);
}

/// A block given again (kept) doesn't run again for it; another one runs, and its properties are
/// the group's, as Compose compares blocks by identity (`updateLayerBlock`).
#[test]
fn a_new_block_runs_and_the_same_one_given_again_does_not() {
    let runs = Count::default();
    let (which, x) = (mutable_state_of(0), mutable_state_of(0));
    let (w, xh, r1, r2) = (which.handle(), x.handle(), runs.clone(), runs.clone());
    let first = LayerBlock::new(move |layer| {
        bump(&r1);
        layer.alpha = 0.5;
    });
    let second = LayerBlock::new(move |layer| {
        bump(&r2);
        layer.alpha = 0.75;
    });
    let measures = Count::default();
    let policy = placing(
        &measures,
        move || (xh.get(), 0),
        move || Some(if w.get() == 0 { first.clone() } else { second.clone() }),
    );
    let mut host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| Leaf(painted("child", Color::RED).size(10.dp()))),
        )
    });
    let ran = runs.get();
    assert!(ran >= 1);
    x.set(5);
    host.frame();
    assert_eq!(runs.get(), ran, "placed again with the same block");
    which.set(1);
    host.frame();
    assert!(runs.get() > ran, "a new block runs");
    let list = host.display_list();
    let [child] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    assert_eq!(group(&list, child).layer.expect("a layer").alpha, 0.75);
}

/// A layout modifier placing its content with a layer: what follows it in the chain, and the
/// children, are a group of their own with the block's properties, inside the node's drawing.
#[test]
fn a_layout_modifier_placing_its_content_with_a_layer_makes_it_a_group() {
    let layered = Modifier
        .then(painted("outer", Color::BLUE))
        .layout(|scope, measurable, constraints| {
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place_with_layer(&placeable, 0, 0, |layer: &mut GraphicsLayerScope| {
                    layer.alpha = 0.5
                })
            })
        })
        .then(painted("inner", Color::RED))
        .size(10.dp());
    let host = host(move || Leaf(layered.clone()));
    let list = host.display_list();
    let [inner] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    let inner = group(&list, inner);
    assert_eq!(inner.layer.as_ref().expect("a layer").alpha, 0.5);
    let dump = list.dump();
    let (blue, red) = (dump.find("#FF0000FF").expect("blue"), dump.find("#FFFF0000").expect("red"));
    assert!(blue < red, "{dump}");
    // The red is the group's, the blue outside it.
    let in_group = FlatDisplayList {
        ops: list.ops[inner.start..inner.end].to_vec(),
        ..FlatDisplayList::default()
    }
    .dump();
    assert!(in_group.contains("#FFFF0000") && !in_group.contains("#FF0000FF"), "{dump}");
}

/// A node whose first drawing modifier is a graphics layer, placed with a layer: the placement's
/// is its group's layer, outside, and its own a group inside it, as Compose's two layers nest.
#[test]
fn a_graphics_layer_inside_a_placement_s_layer_is_a_group_inside_it() {
    let measures = Count::default();
    let block = LayerBlock::new(|layer| layer.alpha = 0.5);
    let policy = placing(&measures, || (0, 0), move || Some(block.clone()));
    let host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| {
                Leaf(
                    Modifier
                        .graphics_layer(|layer| layer.scale_x = 3.0)
                        .then(painted("child", Color::RED))
                        .size(10.dp()),
                )
            }),
        )
    });
    let list = host.display_list();
    let [outer, inner] = keys(&list)[..] else { panic!("two groups\n{}", list.dump()) };
    assert_eq!(group(&list, outer).layer.expect("a layer").alpha, 0.5);
    assert_eq!(group(&list, inner).layer.expect("a layer").scale, (3.0, 1.0));
    assert!(group(&list, inner).start >= group(&list, outer).start);
    assert!(group(&list, inner).end <= group(&list, outer).end);
}

/// A child already a group (its first drawing modifier a graphics layer) placed with a layer
/// later: its parent draws it again though it was a group before, as the group now holds its
/// graphics layer inside the placement's, and the same once it's placed without it again.
#[test]
fn a_child_already_a_group_placed_with_a_layer_later_nests_its_own_inside() {
    let layered = mutable_state_of(false);
    let handle = layered.handle();
    let measures = Count::default();
    let block = LayerBlock::new(|layer| layer.alpha = 0.5);
    let policy = placing(&measures, || (0, 0), move || handle.get().then(|| block.clone()));
    let mut host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| {
                Leaf(
                    Modifier
                        .graphics_layer(|layer| layer.scale_x = 3.0)
                        .then(painted("child", Color::RED))
                        .size(10.dp()),
                )
            }),
        )
    });
    let list = host.display_list();
    let [own] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    assert_eq!(group(&list, own).layer.expect("a layer").scale, (3.0, 1.0));
    layered.set(true);
    host.frame();
    let list = host.display_list();
    let [outer, inner] = keys(&list)[..] else { panic!("two groups\n{}", list.dump()) };
    assert_eq!(group(&list, outer).layer.expect("a layer").alpha, 0.5);
    assert_eq!(group(&list, inner).layer.expect("a layer").scale, (3.0, 1.0));
    layered.set(false);
    host.frame();
    let list = host.display_list();
    let [own] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    let own = group(&list, own).layer.expect("a layer");
    assert_eq!((own.alpha, own.scale), (1.0, (3.0, 1.0)));
}

/// A `GraphicsLayer` placed with gives its properties; setting one changes the group's, drawing
/// nothing again.
#[test]
fn a_graphics_layer_placed_with_gives_its_properties() {
    let layer = GraphicsLayer::new();
    layer.set_alpha(0.5);
    let kept = layer.clone();
    let policy = measure_policy(move |scope, measurables, constraints| {
        let placeable = scope.measure(measurables[0], constraints);
        let layer = kept.clone();
        scope.layout_with(100, 100, move |placement| {
            placement.place_with_graphics_layer(&placeable, 0, 0, &layer);
        })
    });
    let mut host = host(move || {
        Layout(
            Modifier,
            policy.clone(),
            callback(|| Leaf(painted("child", Color::RED).size(10.dp()))),
        )
    });
    let list = host.display_list();
    let [child] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    assert_eq!(group(&list, child).layer.expect("a layer").alpha, 0.5);
    take_log();
    layer.set_alpha(0.25);
    host.frame();
    let after = host.display_list();
    assert_eq!(group(&after, child).layer.expect("a layer").alpha, 0.25);
    assert_eq!(group(&after, child).version, group(&list, child).version);
    assert!(take_log().is_empty(), "drew nothing again");
}

/// Coordinates go through a placement's layer: a 10 px child placed at (10, 10) scaled twice is
/// at (5, 5) in the root, as `localToRoot` maps through its layer's matrix.
#[test]
fn coordinates_map_through_the_placement_s_layer() {
    let position = Rc::new(Cell::new(Offset::ZERO));
    let kept = position.clone();
    let measures = Count::default();
    let block = LayerBlock::new(|layer| {
        layer.scale_x = 2.0;
        layer.scale_y = 2.0;
    });
    let policy = placing(&measures, || (10, 10), move || Some(block.clone()));
    let mut host = host(move || {
        let kept = kept.clone();
        Layout(
            Modifier,
            policy.clone(),
            callback(move || {
                let kept = kept.clone();
                Leaf(
                    Modifier
                        .on_globally_positioned(move |coordinates: LayoutCoordinates| {
                            kept.set(coordinates.position_in_root())
                        })
                        .size(10.dp()),
                )
            }),
        )
    });
    host.frame();
    assert_eq!(position.get(), Offset::new(5.0, 5.0));
}

/// A layout modifier placing its content without a layer after placing it with one drops the
/// content's group: what follows the modifier is drawn in the node's drawing again.
#[test]
fn a_layout_modifier_placing_without_a_layer_later_drops_the_group() {
    let layered = mutable_state_of(true);
    let handle = layered.handle();
    let modifier = Modifier
        .layout(move |scope, measurable, constraints| {
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                if handle.get() {
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
        .then(painted("inner", Color::RED))
        .size(10.dp());
    let mut host = host(move || Leaf(modifier.clone()));
    assert_eq!(keys(&host.display_list()).len(), 1);
    layered.set(false);
    host.frame();
    let list = host.display_list();
    assert!(keys(&list).is_empty(), "{}", list.dump());
    assert!(list.dump().contains("#FFFF0000"), "{}", list.dump());
}

/// `place_relative_with_layer` mirrors in a right-to-left layout, as `placeRelativeWithLayer`.
#[test]
fn placing_relatively_with_a_layer_mirrors_right_to_left() {
    let policy = measure_policy(|scope, measurables, constraints| {
        let placeable = scope.measure(measurables[0], constraints);
        scope.layout_with(100, 100, move |placement| {
            placement.place_relative_with_layer(&placeable, 10, 0, |_: &mut GraphicsLayerScope| {})
        })
    });
    let host = host(move || {
        let policy = policy.clone();
        CompositionLocalProvider(
            LOCAL_LAYOUT_DIRECTION.provides(LayoutDirection::Rtl),
            move || {
                Layout(
                    Modifier,
                    policy.clone(),
                    callback(|| Leaf(painted("child", Color::RED).size(10.dp()))),
                )
            },
        );
    });
    let list = host.display_list();
    assert_eq!(keys(&list).len(), 1, "{}", list.dump());
    // 100 - 10 - 10.
    assert!(list.dump().contains("translate 80 0"), "{}", list.dump());
}

/// Between two coordinates, the way goes through a placement's layer too, both ways
/// (`localPositionOf`), and its clip bounds what's under it (`boundsInRoot`).
#[test]
fn local_positions_and_bounds_go_through_the_placement_s_layer() {
    let parent: Rc<std::cell::RefCell<Option<LayoutCoordinates>>> = Rc::default();
    let child: Rc<std::cell::RefCell<Option<LayoutCoordinates>>> = Rc::default();
    let (p, c) = (parent.clone(), child.clone());
    let measures = Count::default();
    let block = LayerBlock::new(|layer| {
        layer.scale_x = 2.0;
        layer.scale_y = 2.0;
        layer.clip = true;
    });
    let policy = placing(&measures, || (10, 10), move || Some(block.clone()));
    let mut host = host(move || {
        let (p, c) = (p.clone(), c.clone());
        Layout(
            Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
                *p.borrow_mut() = Some(coordinates)
            }),
            policy.clone(),
            callback(move || {
                let c = c.clone();
                Leaf(Modifier.size(10.dp()).then(Modifier.on_globally_positioned(
                    move |coordinates: LayoutCoordinates| *c.borrow_mut() = Some(coordinates),
                )));
            }),
        )
    });
    host.frame();
    let (parent, child) = (parent.borrow().unwrap(), child.borrow().unwrap());
    // The 10 px child at (10, 10), scaled twice around its center: from 5 to 25.
    assert_eq!(parent.local_position_of(&child, Offset::ZERO), Offset::new(5.0, 5.0));
    assert_eq!(child.local_position_of(&parent, Offset::new(5.0, 5.0)), Offset::ZERO);
    let bounds = child.bounds_in_root();
    assert_eq!((bounds.left, bounds.top, bounds.right, bounds.bottom), (5.0, 5.0, 25.0, 25.0));
}

/// A graphics layer where a placed layer starts stays its own group once the placed one goes, the
/// last of the tree's: the chain's layers are worked out as before placing with a layer.
#[test]
fn a_graphics_layer_where_a_dropped_placed_layer_started_stays_a_group() {
    let layered = mutable_state_of(true);
    let handle = layered.handle();
    let modifier = Modifier
        .then(painted("outer", Color::BLUE))
        .layout(move |scope, measurable, constraints| {
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                if handle.get() {
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
        .then(painted("inner", Color::RED))
        .size(10.dp());
    let mut host = host(move || Leaf(modifier.clone()));
    assert_eq!(keys(&host.display_list()).len(), 2);
    layered.set(false);
    host.frame();
    let list = host.display_list();
    let [inner] = keys(&list)[..] else { panic!("one group\n{}", list.dump()) };
    assert_eq!(group(&list, inner).layer.expect("a layer").alpha, 0.75);
}

/// A node placed with a layer leaving the tree drops its layer's block, what it captured with
/// it: the layers are kept apart from the node (`UiTree::placement_layers`), not with it.
#[test]
fn a_node_placed_with_a_layer_leaving_drops_its_block() {
    let token = Rc::new(());
    let shown = mutable_state_of(true);
    let (handle, captured) = (shown.handle(), token.clone());
    let mut host = host(move || {
        if handle.get() {
            let captured = captured.clone();
            let policy = measure_policy(move |scope, measurables, constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                let captured = captured.clone();
                scope.layout_with(100, 100, move |placement| {
                    let captured = captured.clone();
                    placement.place_with_layer(
                        &placeable,
                        0,
                        0,
                        move |layer: &mut GraphicsLayerScope| {
                            let _ = &captured;
                            layer.alpha = 0.5;
                        },
                    );
                })
            });
            Layout(Modifier, policy, callback(|| Leaf(painted("child", Color::RED).size(10.dp()))));
        }
    });
    // The test's and the content's, and the policy's, its block's and the layer's.
    assert!(Rc::strong_count(&token) > 2);
    shown.set(false);
    host.frame();
    assert_eq!(Rc::strong_count(&token), 2, "what the layer's block captured is dropped");
}
