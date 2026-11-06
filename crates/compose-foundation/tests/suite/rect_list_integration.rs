//! The rects the layout-rect callbacks compute for nodes, kept right through moves, paddings,
//! layers, scrolling and lazy lists. Adapted from Compose's `RectListIntegrationTest`, which reads
//! its rect list of every node: compose-rs keeps none (D1372), so these ask the tree for the rect
//! it computes for a registered node (`UiTree::layout_rect_for_test`), at density 1, within a
//! pixel as Compose's `assertRectDp`.

use compose_foundation::layout::*;
use compose_foundation::lazy::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, IntRect, IntSize, Layout, UnitExt, measure_policy};

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(1000, 1000), content)
}

/// The rect of the node tagged `tag`, `None` if it's not placed.
fn rect_of(ui: &mut ComposeTestRule, tag: &str) -> Option<IntRect> {
    ui.wait_for_idle();
    let node = ui.on_node_with_tag(tag).node_id();
    ui.host().with_tree(|tree| tree.layout_rect_for_test(node))
}

/// The rect of `tag` is `(left, top, right, bottom)`, within a pixel.
#[track_caller]
fn assert_rect(ui: &mut ComposeTestRule, tag: &str, expected: (i32, i32, i32, i32)) {
    let rect = rect_of(ui, tag).unwrap_or_else(|| panic!("{tag} has no rect"));
    let (l, t, r, b) = expected;
    let close = |actual: i32, wanted: i32| (actual - wanted).abs() <= 1;
    assert!(
        close(rect.left, l) && close(rect.top, t) && close(rect.right, r) && close(rect.bottom, b),
        "{tag}: expected {expected:?}, was {rect:?}"
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testSingleBox
#[test]
fn a_box_s_rect() {
    let mut ui = rule(|| Box!(modifier = Modifier.test_tag("foo").size(10.dp())));
    assert_rect(&mut ui, "foo", (0, 0, 10, 10));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testNestedBox
#[test]
fn a_nested_box_s_rect() {
    let mut ui = rule(|| {
        Box!(modifier = Modifier.padding(10.dp()), {
            Box!(modifier = Modifier.test_tag("foo").size(10.dp()));
        });
    });
    assert_rect(&mut ui, "foo", (10, 10, 20, 20));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testUpdatePosition
#[test]
fn a_new_padding_moves_the_rect() {
    let toggle = mutable_state_of(false);
    let t = toggle.handle();
    let mut ui = rule(move || {
        Box!(modifier = Modifier.padding(if t.get() { 50.dp() } else { 10.dp() }), {
            Box!(modifier = Modifier.test_tag("foo").size(10.dp()));
        });
    });
    assert_rect(&mut ui, "foo", (10, 10, 20, 20));
    toggle.set(true);
    assert_rect(&mut ui, "foo", (50, 50, 60, 60));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testUpdatePosition_OptimizedMove
#[test]
fn a_moved_parent_moves_its_child_s_rect() {
    let toggle = mutable_state_of(false);
    let t = toggle.handle();
    let mut ui = rule(move || {
        Row!(|_| {
            Box!(
                modifier = Modifier.width(if t.get() { 50.dp() } else { 10.dp() }).height(10.dp())
            );
            Box!(modifier = Modifier.test_tag("parent").width(20.dp()).height(20.dp()), {
                Box!(modifier = Modifier.test_tag("child").size(10.dp()));
            });
        });
    });
    assert_rect(&mut ui, "parent", (10, 0, 30, 20));
    assert_rect(&mut ui, "child", (10, 0, 20, 10));
    toggle.set(true);
    assert_rect(&mut ui, "parent", (50, 0, 70, 20));
    assert_rect(&mut ui, "child", (50, 0, 60, 10));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testUpdateTextPadding
#[test]
fn a_text_s_rect_grows_with_its_padding() {
    let padding = mutable_state_of(0.0f32);
    let p = padding.handle();
    let mut ui = rule(move || {
        Box!(modifier = Modifier.size(200.dp()), {
            BasicText!(
                "Row",
                modifier = Modifier.test_tag("text").padding(p.get().dp()).size(40.dp())
            );
        });
    });
    assert_rect(&mut ui, "text", (0, 0, 40, 40));
    padding.set(10.0);
    assert_rect(&mut ui, "text", (0, 0, 60, 60));
}

/// Three nested paddings around the tagged box, the outermost `outer` (an offset with `offset`).
fn nested_paddings(outer: impl Fn() -> Modifier + 'static) -> impl Fn() + 'static {
    move || {
        Box!(modifier = outer(), {
            Box!(modifier = Modifier.padding(20.dp()), {
                Box!(modifier = Modifier.padding(60.dp()), {
                    Box!(modifier = Modifier.test_tag("test").size(100.dp()));
                });
            });
        });
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testPaddings
#[test]
fn nested_paddings_add_up() {
    let mut ui = rule(nested_paddings(|| Modifier.padding(20.dp())));
    assert_rect(&mut ui, "test", (100, 100, 200, 200));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testPaddingsTwo
#[test]
fn a_padding_in_the_tagged_node_is_in_its_rect() {
    let mut ui = rule(|| {
        Box!(modifier = Modifier.padding(4.dp()), {
            Box!(modifier = Modifier.test_tag("test").padding(4.dp()).size(4.dp()));
        });
    });
    assert_rect(&mut ui, "test", (4, 4, 16, 16));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testPaddingsThree
#[test]
fn an_outer_padding_s_change_moves_the_inner_rect() {
    let padding = mutable_state_of(20.0f32);
    let p = padding.handle();
    let mut ui = rule(nested_paddings(move || Modifier.padding(p.get().dp())));
    assert_rect(&mut ui, "test", (100, 100, 200, 200));
    padding.set(30.0);
    assert_rect(&mut ui, "test", (110, 110, 210, 210));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testPaddingsFour
#[test]
fn an_outer_offset_s_change_moves_the_inner_rect() {
    let padding = mutable_state_of(20);
    let p = padding.handle();
    let mut ui = rule(nested_paddings(move || {
        Modifier.offset_with(move |_| compose_ui::IntOffset::new(p.get(), p.get()))
    }));
    assert_rect(&mut ui, "test", (100, 100, 200, 200));
    padding.set(30);
    assert_rect(&mut ui, "test", (110, 110, 210, 210));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testUpdateSize
#[test]
fn a_new_size_is_the_rect_s() {
    let toggle = mutable_state_of(false);
    let t = toggle.handle();
    let mut ui = rule(move || {
        Box!({
            Box!(modifier = Modifier.test_tag("foo").size(if t.get() { 50.dp() } else { 10.dp() }));
        });
    });
    assert_rect(&mut ui, "foo", (0, 0, 10, 10));
    toggle.set(true);
    assert_rect(&mut ui, "foo", (0, 0, 50, 50));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testUpdateTranslation
#[test]
fn a_parent_s_offset_moves_the_rect() {
    let toggle = mutable_state_of(false);
    let t = toggle.handle();
    let mut ui = rule(move || {
        let at = move |_| {
            if t.get() { compose_ui::IntOffset::new(10, 10) } else { compose_ui::IntOffset::ZERO }
        };
        Box!(modifier = Modifier.offset_with(at), {
            Box!(modifier = Modifier.test_tag("foo").size(10.dp()));
        });
    });
    assert_rect(&mut ui, "foo", (0, 0, 10, 10));
    toggle.set(true);
    assert_rect(&mut ui, "foo", (10, 10, 20, 20));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRemovingNodeRemovesRect
#[test]
fn a_removed_node_has_no_rect() {
    let toggle = mutable_state_of(false);
    let t = toggle.handle();
    let mut ui = rule(move || {
        if !t.get() {
            Box!(modifier = Modifier.test_tag("foo").size(10.dp()));
        }
    });
    assert_rect(&mut ui, "foo", (0, 0, 10, 10));
    let node = ui.on_node_with_tag("foo").node_id();
    toggle.set(true);
    ui.wait_for_idle();
    assert_eq!(ui.host().with_tree(|tree| tree.layout_rect_for_test(node)), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testScrolling
#[test]
fn a_scroll_moves_the_rects() {
    let scroll = ScrollState::new(0);
    let s = scroll.handle();
    let mut ui = rule(move || {
        Column!(modifier = Modifier.test_tag("scroller").vertical_scroll(s), |_| {
            for index in 0..4 {
                Box!(
                    modifier =
                        Modifier.test_tag(format!("foo{index}")).width(200.dp()).height(400.dp()),
                    {
                        for _ in 0..4 {
                            Box!(modifier = Modifier.size(10.dp()));
                        }
                    }
                );
            }
        });
    });
    assert_rect(&mut ui, "foo0", (0, 0, 200, 400));
    assert_rect(&mut ui, "foo3", (0, 1200, 200, 1600));
    let _ = crate::scroll_support::run(&mut ui, async move { s.scroll_by(100.0).await });
    assert_rect(&mut ui, "foo0", (0, -100, 200, 300));
    assert_rect(&mut ui, "foo3", (0, 1100, 200, 1500));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testScrollingWeirdness
#[test]
fn a_scrolled_column_s_children_have_their_rects() {
    let scroll = ScrollState::new(0);
    let s = scroll.handle();
    let mut ui = rule(move || {
        Column!(modifier = Modifier.vertical_scroll(s), |_| {
            Box!(modifier = Modifier.test_tag("foo").size(100.dp()));
            Box!(modifier = Modifier.test_tag("bar").size(100.dp()));
        });
    });
    assert_rect(&mut ui, "foo", (0, 0, 100, 100));
    assert_rect(&mut ui, "bar", (0, 100, 100, 200));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRotatedBox
#[test]
fn a_rotated_layer_bounds_its_child_s_rect_not_its_own() {
    let mut ui = rule(|| {
        Box!(
            modifier = Modifier.test_tag("outer").graphics_layer(|layer| {
                layer.translation_x = 100.0;
                layer.rotation_z = 45.0;
            }),
            {
                Box!(modifier = Modifier.test_tag("inner").size(100.dp()));
            }
        );
    });
    assert_rect(&mut ui, "outer", (0, 0, 100, 100));
    assert_rect(&mut ui, "inner", (79, -21, 220, 121));
}

/// A column of a 100 tall box then the tagged 100 by 100 one, rotated by `rotation()`.
fn rotated_column(rotation: impl Fn() -> f32 + 'static, grandchild: bool) -> impl Fn() + 'static {
    let rotation = std::rc::Rc::new(rotation);
    move || {
        let rotation = rotation.clone();
        Column!(
            modifier = Modifier
                .test_tag("outer")
                .graphics_layer(move |layer| { layer.rotation_z = rotation() }),
            |_| {
                Box!(modifier = Modifier.height(100.dp()));
                if grandchild {
                    Box!({
                        Box!(modifier = Modifier.test_tag("inner").size(100.dp()));
                    });
                } else {
                    Box!(modifier = Modifier.test_tag("inner").size(100.dp()));
                }
            }
        );
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRotatedChildBoxOffset
#[test]
fn a_rotated_column_s_child_has_its_bounding_rect() {
    let mut ui = rule(rotated_column(|| 45.0, false));
    assert_rect(&mut ui, "outer", (0, 0, 100, 200));
    assert_rect(&mut ui, "inner", (-56, 65, 85, 206));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRotatedChildBoxOffsetUpdated
#[test]
fn rotating_a_column_moves_its_child_s_rect() {
    let rotation = mutable_state_of(0.0f32);
    let r = rotation.handle();
    let mut ui = rule(rotated_column(move || r.get(), false));
    assert_rect(&mut ui, "outer", (0, 0, 100, 200));
    assert_rect(&mut ui, "inner", (0, 100, 100, 200));
    rotation.set(45.0);
    assert_rect(&mut ui, "outer", (0, 0, 100, 200));
    assert_rect(&mut ui, "inner", (-56, 65, 85, 206));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRotatedGrandchildBoxOffset
#[test]
fn a_rotated_column_s_grandchild_has_its_bounding_rect() {
    let mut ui = rule(rotated_column(|| 45.0, true));
    assert_rect(&mut ui, "outer", (0, 0, 100, 200));
    assert_rect(&mut ui, "inner", (-56, 65, 85, 206));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRotatedGrandchildBoxOffsetUpdated
#[test]
fn rotating_a_column_moves_its_grandchild_s_rect() {
    let rotation = mutable_state_of(0.0f32);
    let r = rotation.handle();
    let mut ui = rule(rotated_column(move || r.get(), true));
    assert_rect(&mut ui, "inner", (0, 100, 100, 200));
    rotation.set(45.0);
    assert_rect(&mut ui, "outer", (0, 0, 100, 200));
    assert_rect(&mut ui, "inner", (-56, 65, 85, 206));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testTranslatedAndRotatedBox
#[test]
fn translating_and_rotating_a_layer_moves_its_child_s_rect() {
    let toggle = mutable_state_of(true);
    let t = toggle.handle();
    let mut ui = rule(move || {
        Box!(
            modifier = Modifier.test_tag("outer").padding(10.dp()).graphics_layer(move |layer| {
                layer.translation_x = if t.get() { 0.0 } else { 100.0 };
                layer.rotation_z = if t.get() { 0.0 } else { 45.0 };
            }),
            {
                Box!(modifier = Modifier.test_tag("inner").size(10.dp()));
            }
        );
    });
    assert_rect(&mut ui, "outer", (0, 0, 30, 30));
    assert_rect(&mut ui, "inner", (10, 10, 20, 20));
    toggle.set(false);
    assert_rect(&mut ui, "inner", (108, 8, 122, 22));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testScaledBox
#[test]
fn a_scaled_layer_scales_its_child_s_rect() {
    let mut ui = rule(|| {
        Box!(modifier = Modifier.test_tag("outer").padding(10.dp()).scale(2.0), {
            Box!(modifier = Modifier.test_tag("inner").size(10.dp()));
        });
    });
    assert_rect(&mut ui, "outer", (0, 0, 30, 30));
    assert_rect(&mut ui, "inner", (5, 5, 25, 25));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testScrollingNestedLayout
#[test]
fn a_scroll_moves_nested_texts_rects() {
    let scroll = ScrollState::new(0);
    let s = scroll.handle();
    let mut ui = rule(move || {
        Column!(modifier = Modifier.test_tag("scroller").vertical_scroll(s), |_| {
            for index in 0..8 {
                Box!(modifier = Modifier.size(200.dp()), {
                    BasicText!(
                        "Row {index}",
                        modifier = Modifier
                            .test_tag(format!("text{index}"))
                            .width(100.dp())
                            .height(20.dp())
                    );
                });
            }
        });
    });
    assert_rect(&mut ui, "text0", (0, 0, 100, 20));
    assert_rect(&mut ui, "text1", (0, 200, 100, 220));
    assert_rect(&mut ui, "text2", (0, 400, 100, 420));
    let _ = crate::scroll_support::run(&mut ui, async move { s.scroll_by(100.0).await });
    assert_rect(&mut ui, "text2", (0, 300, 100, 320));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testLazyColumn
#[test]
fn a_lazy_column_s_items_have_their_rects_after_a_scroll() {
    let state = LazyListState::new(0, 0);
    let handle = *state;
    let mut ui = rule(move || {
        LazyColumn!(
            modifier = Modifier.test_tag("lazy"),
            state = handle,
            |scope: LazyListScope| {
                scope.items(200, |index| {
                    Box!(
                        modifier = Modifier
                            .test_tag(format!("foo{index}"))
                            .height(100.dp())
                            .width(100.dp())
                    );
                })
            }
        );
    });
    assert_rect(&mut ui, "foo0", (0, 0, 100, 100));
    assert_rect(&mut ui, "foo1", (0, 100, 100, 200));
    let _ = crate::scroll_support::run(&mut ui, async move { handle.scroll_by(2000.0).await });
    ui.wait_for_idle();
    let top = rect_of(&mut ui, "foo20").expect("in view").top;
    assert!((-4..=4).contains(&top), "{top}");
}

/// Compose's `setLazyColumnExample`: a lazy column of `count` rows, each two boxes in a column
/// and a box beside them, a quarter of its 600 pixels tall.
fn lazy_example(state: LazyListState) -> impl Fn() + 'static {
    move || {
        LazyColumn!(modifier = Modifier.size(600.dp()), state = state, |scope: LazyListScope| {
            scope.items(12, |index| {
                Row!(
                    modifier = Modifier
                        .test_tag(format!("Item-{index}"))
                        .fill_max_width()
                        .height(150.dp()),
                    |row| {
                        Column!(
                            modifier = Modifier.fill_max_height().weight_with(row, 0.7, true),
                            |_| {
                                Box!(modifier = Modifier.fill_max_width().height(75.dp()));
                                Box!(modifier = Modifier.fill_max_width().height(75.dp()));
                            }
                        );
                        Box!(modifier = Modifier.fill_max_height().weight_with(row, 0.3, true));
                    }
                );
            })
        });
    }
}

/// The tagged item and the four nodes in it all have rects.
fn assert_rects_in_item(ui: &mut ComposeTestRule, tag: &str) {
    ui.wait_for_idle();
    let item = ui.on_node_with_tag(tag).node_id();
    let count = ui.host().with_tree(|tree| tree.placed_rects_under_for_test(item));
    assert_eq!(count, 5, "{tag}");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testLazyColumn_isConsistentAfterScroll_0
#[test]
fn jumps_forward_leave_every_item_in_view_its_rects() {
    let state = LazyListState::new(0, 0);
    let mut ui = rule(lazy_example(*state));
    for index in [2, 4, 6] {
        state.request_scroll_to_item(index, 0);
        ui.wait_for_idle();
    }
    assert_rect(&mut ui, "Item-6", (0, 0, 600, 150));
    for index in 6..10 {
        assert_rects_in_item(&mut ui, &format!("Item-{index}"));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testLazyColumn_isConsistentAfterScroll_1
#[test]
fn jumps_back_and_forth_leave_every_item_in_view_its_rects() {
    let state = LazyListState::new(0, 0);
    let mut ui = rule(lazy_example(*state));
    for index in [8, 7, 6, 7, 8] {
        state.request_scroll_to_item(index, 0);
        ui.wait_for_idle();
    }
    assert_rect(&mut ui, "Item-8", (0, 0, 600, 150));
    for index in 8..12 {
        assert_rects_in_item(&mut ui, &format!("Item-{index}"));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testUnplacedNodeHasNoRect
#[test]
fn an_unplaced_node_has_no_rect() {
    let place = mutable_state_of(true);
    let p = place.handle();
    let mut ui = rule(move || {
        Layout!(
            measure_policy = measure_policy(move |scope, measurables, constraints: Constraints| {
                let placeable = scope.measure(measurables[0], constraints);
                scope.layout_with(constraints.max_width, constraints.max_height, move |placement| {
                    if p.get() {
                        placement.place(&placeable, 0, 0);
                    }
                })
            }),
            {
                Box!(modifier = Modifier.test_tag("foo").size(10.dp()));
            }
        );
    });
    assert!(rect_of(&mut ui, "foo").is_some());
    // Unplaced, it leaves the semantics tree: asked by its id.
    let node = ui.on_node_with_tag("foo").node_id();
    place.set(false);
    ui.wait_for_idle();
    assert_eq!(ui.host().with_tree(|tree| tree.layout_rect_for_test(node)), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRectListDuringAlignment
#[test]
fn rows_aligned_by_baseline_have_their_rects() {
    let mut ui = rule(|| {
        Row!(|outer| {
            Row!(modifier = Modifier.align_by_baseline(outer), |inner| {
                BasicText!(
                    "text",
                    modifier = Modifier.test_tag("text").size(10.dp()).align_by_baseline(inner)
                );
            });
        });
    });
    assert!(rect_of(&mut ui, "text").is_some());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RectListIntegrationTest.kt, testRectListDuringAlignment_withLayoutModifier
#[test]
fn a_padded_row_aligned_by_baseline_has_its_rects() {
    let mut ui = rule(|| {
        Row!(|outer| {
            Row!(modifier = Modifier.padding(10.dp()).align_by_baseline(outer), |inner| {
                BasicText!(
                    "text",
                    modifier = Modifier.test_tag("text").size(10.dp()).align_by_baseline(inner)
                );
            });
        });
    });
    assert_eq!(rect_of(&mut ui, "text").map(|it| it.top_left().x), Some(10));
}
