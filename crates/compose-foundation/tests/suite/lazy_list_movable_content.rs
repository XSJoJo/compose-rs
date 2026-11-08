//! Compose's `LazyListMovableContentTest`: a lazy list inside movable content keeps its scroll
//! position when moved, and movable content moves from an item of one list to another's.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Color, IntSize, SemanticsActions, UnitExt};

fn scroll_to_index(ui: &mut ComposeTestRule, tag: &str, index: i32) {
    ui.on_node_with_tag(tag).perform_semantics_action_with(
        &SemanticsActions::SCROLL_TO_INDEX,
        |scroll| {
            scroll(index);
        },
    );
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListMovableContentTest.kt, lazyListInsideMovableContent_movesItem_andPreservesScrollPosition
#[test]
fn lazy_list_inside_movable_content_moves_item_and_preserves_scroll_position() {
    let show_lazy_list = mutable_state_of(true);
    let show = show_lazy_list.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let state = compose_foundation::lazy::remember_lazy_list_state(0, 0);
        let movable_content = remember(move || {
            movable_content_of(move || {
                LazyColumn!(
                    modifier = Modifier.size(100.dp()).test_tag("LazyList"),
                    state = state,
                    |scope: LazyListScope| {
                        scope.items_keyed(
                            21,
                            |index| index.to_string(),
                            |index| {
                                Box!(
                                    modifier =
                                        Modifier.size(50.dp()).test_tag(format!("Item{index}"))
                                );
                            },
                        );
                    }
                );
            })
        });
        Box!({
            if show.get() {
                movable_content();
            } else {
                let movable_content = movable_content.clone();
                Box!(modifier = Modifier.size(100.dp()).test_tag("ParentBox"), {
                    movable_content();
                });
            }
        });
    });

    scroll_to_index(&mut ui, "LazyList", 20);
    ui.on_node_with_tag("Item20").assert_is_displayed();

    // Move the list into the nested box.
    show_lazy_list.set(false);
    ui.wait_for_idle();

    ui.on_node_with_tag("ParentBox").assert_is_displayed();
    ui.on_node_with_tag("LazyList").assert_is_displayed();
    ui.on_node_with_tag("Item20").assert_is_displayed();
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListMovableContentTest.kt, lazyListInsideMovableContent_movesItemAcrossLists
#[test]
fn lazy_list_inside_movable_content_moves_item_across_lists() {
    let switch_content = mutable_state_of(true);
    let switched = switch_content.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let movable_content = remember(|| {
            movable_content_of(|| {
                Box!(modifier = Modifier.size(100.dp()), {
                    BasicText!("Movable Box");
                });
            })
        });
        let (first, second) = (movable_content.clone(), movable_content.clone());
        Column!({
            let first = first.clone();
            LazyColumn!(
                modifier = Modifier.size(100.dp()).test_tag("LazyList1").background(Color::RED),
                move |scope: LazyListScope| {
                    scope.items_keyed(
                        21,
                        |index| index.to_string(),
                        |index| {
                            Box!(modifier = Modifier.size(100.dp()), {
                                BasicText!("First List={index}");
                            });
                        },
                    );
                    if switched.get() {
                        let first = first.clone();
                        scope.item(move || {
                            first();
                        });
                    }
                }
            );
            let second = second.clone();
            LazyColumn!(
                modifier = Modifier.size(100.dp()).test_tag("LazyList2").background(Color::BLUE),
                move |scope: LazyListScope| {
                    if !switched.get() {
                        let second = second.clone();
                        scope.item(move || {
                            second();
                        });
                    }
                    scope.items_keyed(
                        21,
                        |index| index.to_string(),
                        |index| {
                            Box!(modifier = Modifier.size(100.dp()), {
                                BasicText!("Second List={index}");
                            });
                        },
                    );
                }
            );
        });
    });

    scroll_to_index(&mut ui, "LazyList1", 21);
    ui.on_node_with_text("Movable Box").assert_is_displayed();
    let position = ui.on_node_with_text("Movable Box").fetch_semantics_node().position_on_screen();

    // Move the content into the second list.
    switch_content.set(false);
    ui.wait_for_idle();

    scroll_to_index(&mut ui, "LazyList2", 0);
    ui.on_node_with_text("Movable Box").assert_is_displayed();
    let new_position =
        ui.on_node_with_text("Movable Box").fetch_semantics_node().position_on_screen();

    assert_ne!(position, new_position);
}
