//! Compose's `RunWithoutImplicitWaitTest`: finders inside `run_without_implicit_wait` don't wait
//! for idle, and see the tree as it is. Compose also caches the nodes it collects for the
//! block (one collection per block and tree); compose-rs collects them for each query, so the
//! tests that check the cache's identities are n/a and the others check what the cache must not
//! change: changes between blocks seen, merged and unmerged trees, active and deactivated nodes.

use compose_animation::*;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Color, Density, IntSize, LOCAL_DENSITY, Modifier, UnitExt};

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/RunWithoutImplicitWaitTest.kt, triggeredAnimationAndCaptureMotionValues
/// Frame by frame with the clock paused, a content size animation's values captured without
/// waiting: the default spring's sizes, frame after frame, to its end.
#[test]
fn triggered_animation_and_capture_motion_values() {
    run_compose_ui_test(|ui| {
        let size = mutable_state_of(64.0f32);
        let handle = size.handle();
        let done = std::rc::Rc::new(std::cell::Cell::new(false));
        let finished = done.clone();
        ui.set_content(move || {
            let finished = finished.clone();
            CompositionLocalProvider(LOCAL_DENSITY.provides(Density::new(1.0)), move || {
                let finished = finished.clone();
                Box!(
                    modifier = Modifier
                        .test_tag("foo")
                        .animate_content_size_with(
                            spring_with(1.0, Spring::STIFFNESS_MEDIUM_LOW)
                                .visibility_threshold(IntSize::VISIBILITY_THRESHOLD),
                            compose_ui::Alignment::TOP_START,
                            Some(move |_, _| finished.set(true)),
                        )
                        .size(handle.get().dp())
                        .background(Color::RED)
                );
            });
        });
        ui.set_auto_advance(false);
        let mut series = Vec::new();
        size.set(32.0);
        while !done.get() {
            ui.advance_time_by_frame();
            ui.wait_for_idle();
            ui.run_on_ui_thread(|ui| {
                ui.run_without_implicit_wait(|ui| {
                    series.push(ui.on_node_with_tag("foo").fetch_semantics_node().size());
                    // As motion tests capture many values of the frame.
                    for _ in 0..100 {
                        ui.on_node_with_tag("foo").fetch_semantics_node().size();
                    }
                })
            });
        }
        let expected: Vec<IntSize> =
            [64, 64, 63, 60, 56, 52, 49, 46, 43, 41, 39, 37, 36, 35, 35, 34, 34, 33, 32]
                .into_iter()
                .map(|side| IntSize::new(side, side))
                .collect();
        assert_eq!(series, expected);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/RunWithoutImplicitWaitTest.kt, runWithoutImplicitWait_doesNotTriggerWaitForIdle
#[test]
fn run_without_implicit_wait_does_not_trigger_wait_for_idle() {
    run_compose_ui_test(|ui| {
        let visible = mutable_state_of(true);
        let handle = visible.handle();
        ui.set_content(move || {
            if handle.get() {
                Box!(modifier = Modifier.test_tag("dummy_node"));
            }
        });
        visible.set(false);
        ui.run_on_ui_thread(|ui| {
            ui.run_without_implicit_wait(|ui| {
                // A finder that waited would recompose and remove the node first.
                ui.on_node_with_tag("dummy_node").assert_exists();
            })
        });
        // Outside, the finder waits: the node is gone.
        ui.on_node_with_tag("dummy_node").assert_does_not_exist();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/RunWithoutImplicitWaitTest.kt, runWithoutImplicitWait_seesSemanticsChangedBetweenBlocks
/// A description changed between blocks is seen by the next. compose-rs's semantics block runs
/// in composition, so the change recomposes: the clock isn't paused for it (Compose's reads the
/// state as its semantics are computed, needing no frame).
#[test]
fn run_without_implicit_wait_sees_semantics_changed_between_blocks() {
    run_compose_ui_test(|ui| {
        let description = mutable_state_of("first");
        let handle = description.handle();
        ui.set_content(move || {
            let current = handle.get();
            Box!(
                modifier = Modifier
                    .test_tag("node")
                    .size(10.dp())
                    .semantics(move |config| config.set_content_description(current))
            );
        });
        ui.wait_for_idle();
        ui.run_on_ui_thread(|ui| {
            ui.run_without_implicit_wait(|ui| {
                ui.on_node_with_tag("node").assert_content_description_equals(&["first"]);
            })
        });
        description.set("second");
        ui.on_node_with_tag("node").assert_content_description_equals(&["second"]);
        ui.run_on_ui_thread(|ui| {
            ui.run_without_implicit_wait(|ui| {
                ui.on_node_with_tag("node").assert_content_description_equals(&["second"]);
            })
        });
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/RunWithoutImplicitWaitTest.kt, runWithoutImplicitWait_cachesMergedAndUnmergedTreeSeparately
#[test]
fn run_without_implicit_wait_caches_merged_and_unmerged_tree_separately() {
    run_compose_ui_test(|ui| {
        ui.set_content(|| {
            Box!(modifier = Modifier.test_tag("parent").semantics_with(true, |_| {}), {
                Box!(
                    modifier = Modifier
                        .test_tag("child")
                        .size(10.dp())
                        .semantics(|config| config.set_content_description("desc"))
                );
            });
        });
        ui.run_on_ui_thread(|ui| {
            ui.run_without_implicit_wait(|ui| {
                // In the merged tree, the child's description is merged into its parent.
                ui.on_node_with_content_description("desc").assert(has_test_tag("parent"));
                ui.on_node_with_content_description_unmerged("desc").assert(has_test_tag("child"));
                ui.on_node_with_content_description("desc").assert(has_test_tag("parent"));
            })
        });
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/RunWithoutImplicitWaitTest.kt, runWithoutImplicitWait_cachesActiveAndDeactivatedNodesSeparately
#[test]
fn run_without_implicit_wait_caches_active_and_deactivated_nodes_separately() {
    run_compose_ui_test(|ui| {
        let active = mutable_state_of(true);
        let handle = active.handle();
        ui.set_content(move || {
            ReusableContentHost(handle.get(), || {
                Box!(modifier = Modifier.test_tag("node").size(10.dp()));
            });
        });
        // A deactivated node has no semantics properties: find it by its id.
        let id = ui.on_node_with_tag("node").fetch_semantics_snapshot().id;
        let by_id = move || Matcher::with_node(format!("id = {id:?}"), move |node| node.id() == id);
        active.set(false);
        ui.wait_for_idle();
        ui.run_on_ui_thread(|ui| {
            ui.run_without_implicit_wait(|ui| {
                // Queries skip deactivated nodes; `assert_is_deactivated` includes them.
                ui.on_node(by_id()).assert_does_not_exist();
                ui.on_node(by_id()).assert_is_deactivated();
            })
        });
    });
}
