//! Compose's `junit4/WaitUntilNodeCountTest`: the `wait_until_…` node helpers under
//! `run_compose_ui_test` (`clock.rs` has the rule's, as `ComposeTestRuleWaitUntilTest`).

#![allow(non_snake_case)]

use std::time::Duration;

use compose_foundation::layout::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Modifier, UnitExt};

const TIMEOUT: Duration = Duration::from_millis(500);

#[composable]
fn TaggedBox() {
    Box!(modifier = Modifier.size((10.dp(), 10.dp())).test_tag("TestTag"));
}

/// `action` times out, saying `expected`.
#[track_caller]
fn expect_timeout(action: impl FnOnce(), expected: &str) {
    let message = super::input::expect_error(action);
    assert!(message.contains("ComposeTimeoutException"), "{message}");
    assert!(message.contains(expected), "{message}");
    assert!(message.contains("still not satisfied after 500 ms"), "{message}");
}

fn tagged() -> Matcher {
    has_test_tag("TestTag")
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilNodeCount_succeedsWhen_nodeCountCorrect
#[test]
fn wait_until_node_count_succeeds_when_node_count_correct() {
    run_compose_ui_test(|ui| {
        ui.set_content(|| {
            TaggedBox();
            TaggedBox();
            TaggedBox();
        });
        ui.wait_until_node_count(&tagged(), 3, TIMEOUT, false);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilNodeCount_throwsWhen_nodeCountIncorrect
#[test]
fn wait_until_node_count_throws_when_node_count_incorrect() {
    run_compose_ui_test(|ui| {
        ui.set_content(|| {
            TaggedBox();
            TaggedBox();
            TaggedBox();
        });
        expect_timeout(
            || ui.wait_until_node_count(&tagged(), 2, TIMEOUT, false),
            "Condition (exactly 2 nodes match (with tag \"TestTag\"))",
        );
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilAtLeastOneExists_succeedsWhen_nodesExist
#[test]
fn wait_until_at_least_one_exists_succeeds_when_nodes_exist() {
    run_compose_ui_test(|ui| {
        ui.set_content(|| {
            TaggedBox();
            TaggedBox();
        });
        ui.wait_until_at_least_one_exists(&tagged(), TIMEOUT, false);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilAtLeastOneExists_throwsWhen_nodesDoNotExist
#[test]
fn wait_until_at_least_one_exists_throws_when_nodes_do_not_exist() {
    run_compose_ui_test(|ui| {
        ui.set_content(|| Box!(modifier = Modifier.size(10.dp())));
        expect_timeout(
            || ui.wait_until_at_least_one_exists(&tagged(), TIMEOUT, false),
            "Condition (at least one node matches (with tag \"TestTag\"))",
        );
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilExactlyOneExists_succeedsWhen_oneNodeExists
#[test]
fn wait_until_exactly_one_exists_succeeds_when_one_node_exists() {
    run_compose_ui_test(|ui| {
        ui.set_content(TaggedBox);
        ui.wait_until_exactly_one_exists(&tagged(), TIMEOUT, false);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilExactlyOneExists_throwsWhen_twoNodesExist
#[test]
fn wait_until_exactly_one_exists_throws_when_two_nodes_exist() {
    run_compose_ui_test(|ui| {
        ui.set_content(|| {
            TaggedBox();
            TaggedBox();
        });
        expect_timeout(
            || ui.wait_until_exactly_one_exists(&tagged(), TIMEOUT, false),
            "Condition (exactly 1 nodes match (with tag \"TestTag\"))",
        );
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilDoesNotExists_succeedsWhen_nodeDoesNotExist
#[test]
fn wait_until_does_not_exists_succeeds_when_node_does_not_exist() {
    run_compose_ui_test(|ui| {
        ui.set_content(|| Box!(modifier = Modifier.size(10.dp())));
        ui.wait_until_does_not_exist(&tagged(), TIMEOUT, false);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntilDoesNotExists_throwsWhen_nodeExistsUntilTimeout
#[test]
fn wait_until_does_not_exists_throws_when_node_exists_until_timeout() {
    run_compose_ui_test(|ui| {
        ui.set_content(TaggedBox);
        expect_timeout(
            || ui.wait_until_does_not_exist(&tagged(), TIMEOUT, false),
            "Condition (exactly 0 nodes match (with tag \"TestTag\"))",
        );
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/WaitUntilNodeCountTest.kt, waitUntil_succeedsWhen_noRoots
/// No content set (b/361250553): nothing exists, at once.
#[test]
fn wait_until_succeeds_when_no_roots() {
    run_compose_ui_test(|ui| {
        ui.wait_until_does_not_exist(&tagged(), TIMEOUT, false);
    });
}
