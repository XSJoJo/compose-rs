//! Compose's `FindersTest`: finding nodes by test tag, text (its texts, input text and editable
//! text; exact or a substring, matching case or not) and content description, merged and not.
//! Compose's `onNodeWithText(text, substring, ignoreCase)` is `on_node(has_text_with(…))` here,
//! its content description finder `on_node(has_content_description_with(…))`.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{Modifier, SemanticsConfiguration};

use super::input::expect_error;

/// A node of `properties` alone, as Compose's `BoundaryNode`.
fn boundary_node(properties: fn(&mut SemanticsConfiguration)) {
    Column!(modifier = Modifier.semantics(properties), {});
}

/// One boundary node of `properties`.
fn one_node(properties: fn(&mut SemanticsConfiguration)) -> ComposeTestRule {
    compose_test(move || boundary_node(properties))
}

/// A box merging two texts, "Hello" and "World".
fn merged_texts() -> ComposeTestRule {
    compose_test(|| {
        Box!(modifier = Modifier.semantics_with(true, |_| {}), {
            BasicText!("Hello");
            BasicText!("World");
        });
    })
}

/// A box merging two content descriptions, "Hello" and "World".
fn merged_descriptions() -> ComposeTestRule {
    compose_test(|| {
        Box!(modifier = Modifier.semantics_with(true, |_| {}), {
            Box!(modifier = Modifier.semantics(|config| config.set_content_description("Hello")));
            Box!(modifier = Modifier.semantics(|config| config.set_content_description("World")));
        });
    })
}

fn hello_world_text(config: &mut SemanticsConfiguration) {
    config.set_text("Hello World");
}

fn hello_world_input_text(config: &mut SemanticsConfiguration) {
    config.set_input_text("Hello World");
}

fn hello_world_editable_text(config: &mut SemanticsConfiguration) {
    config.set_editable_text("Hello World");
}

fn hello_world_description(config: &mut SemanticsConfiguration) {
    config.set_content_description("Hello World");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findAll_zeroOutOfOne_findsNone
#[test]
fn find_all_zero_out_of_one_finds_none() {
    let mut ui = one_node(|config| config.set_test_tag("not_myTestTag"));
    ui.on_all_nodes(has_test_tag("myTestTag")).assert_count_equals(0);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findAll_oneOutOfTwo_findsOne
#[test]
fn find_all_one_out_of_two_finds_one() {
    let mut ui = compose_test(|| {
        boundary_node(|config| config.set_test_tag("myTestTag"));
        boundary_node(|config| config.set_test_tag("myTestTag2"));
    });
    ui.on_all_nodes(has_test_tag("myTestTag")).assert_count_equals(1);
    ui.on_all_nodes(has_test_tag("myTestTag")).on_first().assert(has_test_tag("myTestTag"));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findAll_twoOutOfTwo_findsTwo
#[test]
fn find_all_two_out_of_two_finds_two() {
    let mut ui = compose_test(|| {
        boundary_node(|config| config.set_test_tag("myTestTag"));
        boundary_node(|config| config.set_test_tag("myTestTag"));
    });
    ui.on_all_nodes(has_test_tag("myTestTag")).assert_count_equals(2);
    ui.on_all_nodes(has_test_tag("myTestTag")).get(0).assert(has_test_tag("myTestTag"));
    ui.on_all_nodes(has_test_tag("myTestTag")).get(1).assert(has_test_tag("myTestTag"));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_matches
#[test]
fn find_by_text_matches() {
    let mut ui = one_node(hello_world_text);
    ui.on_node_with_text("Hello World").assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_matches_substring, findBySubstring_matches
#[test]
fn find_by_text_matches_substring() {
    let mut ui = one_node(hello_world_text);
    ui.on_node(has_text_with("World", true, false)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_matches_ignoreCase
#[test]
fn find_by_text_matches_ignore_case() {
    let mut ui = one_node(hello_world_text);
    ui.on_node(has_text_with("hello worlD", false, true)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_matches_substring_ignoreCase, findBySubstring_ignoreCase_matches
#[test]
fn find_by_text_matches_substring_ignore_case() {
    let mut ui = one_node(hello_world_text);
    ui.on_node(has_text_with("world", true, true)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withInputText_matches
#[test]
fn find_by_text_with_input_text_matches() {
    let mut ui = one_node(hello_world_input_text);
    ui.on_node_with_text("Hello World").assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withInputText_matches_substring
#[test]
fn find_by_text_with_input_text_matches_substring() {
    let mut ui = one_node(hello_world_input_text);
    ui.on_node(has_text_with("World", true, false)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withInputText_matches_ignoreCase
#[test]
fn find_by_text_with_input_text_matches_ignore_case() {
    let mut ui = one_node(hello_world_input_text);
    ui.on_node(has_text_with("hello worlD", false, true)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withInputText_matches_substring_ignoreCase
#[test]
fn find_by_text_with_input_text_matches_substring_ignore_case() {
    let mut ui = one_node(hello_world_input_text);
    ui.on_node(has_text_with("world", true, true)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withEditableText_matches
#[test]
fn find_by_text_with_editable_text_matches() {
    let mut ui = one_node(hello_world_editable_text);
    ui.on_node_with_text("Hello World").assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withEditableText_matches_substring
#[test]
fn find_by_text_with_editable_text_matches_substring() {
    let mut ui = one_node(hello_world_editable_text);
    ui.on_node(has_text_with("World", true, false)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withEditableText_matches_ignoreCase
#[test]
fn find_by_text_with_editable_text_matches_ignore_case() {
    let mut ui = one_node(hello_world_editable_text);
    ui.on_node(has_text_with("hello worlD", false, true)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_withEditableText_matches_substring_ignoreCase
#[test]
fn find_by_text_with_editable_text_matches_substring_ignore_case() {
    let mut ui = one_node(hello_world_editable_text);
    ui.on_node(has_text_with("world", true, true)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_merged_matches
#[test]
fn find_by_text_merged_matches() {
    let mut ui = merged_texts();
    ui.on_node_with_text("Hello").assert_exists();
    ui.on_node_with_text("World").assert_exists();
    ui.on_all_nodes_with_text("Hello").assert_count_equals(1);
    ui.on_all_nodes_with_text("World").assert_count_equals(1);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_fails
#[test]
fn find_by_text_fails() {
    let mut ui = one_node(hello_world_text);
    expect_error(|| {
        ui.on_node_with_text("World").assert_exists();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByText_merged_fails
#[test]
fn find_by_text_merged_fails() {
    let mut ui = merged_texts();
    expect_error(|| {
        ui.on_node_with_text("Hello, World").assert_exists();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findBySubstring_merged_matches
#[test]
fn find_by_substring_merged_matches() {
    let mut ui = merged_texts();
    ui.on_node(has_text_with("Wo", true, false)).assert_exists();
    ui.on_node(has_text_with("He", true, false)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findBySubstring_wrongCase_fails
#[test]
fn find_by_substring_wrong_case_fails() {
    let mut ui = one_node(hello_world_text);
    expect_error(|| {
        ui.on_node(has_text_with("world", true, false)).assert_exists();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findAllBySubstring
#[test]
fn find_all_by_substring() {
    let mut ui = compose_test(|| {
        boundary_node(hello_world_text);
        boundary_node(|config| config.set_text("Wello Horld"));
    });
    ui.on_all_nodes(has_text_with("Yellow World", true, false)).assert_count_equals(0);
    ui.on_all_nodes(has_text_with("Hello", true, false)).assert_count_equals(1);
    ui.on_all_nodes(has_text_with("Wello", true, false)).assert_count_equals(1);
    ui.on_all_nodes(has_text_with("ello", true, false)).assert_count_equals(2);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_matches
#[test]
fn find_by_content_description_matches() {
    let mut ui = one_node(hello_world_description);
    ui.on_node_with_content_description("Hello World").assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_merged_matches
#[test]
fn find_by_content_description_merged_matches() {
    let mut ui = merged_descriptions();
    ui.on_node_with_content_description("Hello").assert_exists();
    ui.on_node_with_content_description("World").assert_exists();
    ui.on_all_nodes_with_content_description("Hello").assert_count_equals(1);
    ui.on_all_nodes_with_content_description("World").assert_count_equals(1);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_fails
#[test]
fn find_by_content_description_fails() {
    let mut ui = one_node(hello_world_description);
    expect_error(|| {
        ui.on_node_with_content_description("Hello").assert_exists();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_merged_fails
#[test]
fn find_by_content_description_merged_fails() {
    let mut ui = merged_descriptions();
    expect_error(|| {
        ui.on_node_with_text("Hello, World").assert_exists();
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_substring_matches
#[test]
fn find_by_content_description_substring_matches() {
    let mut ui = one_node(hello_world_description);
    ui.on_node(has_content_description_with("World", true, false)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_merged_substring_matches
#[test]
fn find_by_content_description_merged_substring_matches() {
    let mut ui = merged_descriptions();
    ui.on_node(has_content_description_with("Wo", true, false)).assert_exists();
    ui.on_node(has_content_description_with("He", true, false)).assert_exists();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_substring_noResult
#[test]
fn find_by_content_description_substring_no_result() {
    let mut ui = one_node(hello_world_description);
    ui.on_node(has_content_description_with("world", true, false)).assert_does_not_exist();
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FindersTest.kt, findByContentDescription_substring_ignoreCase_matches
#[test]
fn find_by_content_description_substring_ignore_case_matches() {
    let mut ui = one_node(hello_world_description);
    ui.on_node(has_content_description_with("world", true, true)).assert_exists();
}
