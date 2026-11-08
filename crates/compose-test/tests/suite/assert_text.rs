//! Compose's `assertions/AssertText`: `assert_text_equals` (as sets, the editable text and the
//! input text counting as asked) and `assert_text_contains`; `AssertContentDescription`, and
//! `FocusActionsTest`.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{Modifier, SemanticsConfiguration, UnitExt};

use super::input::expect_error;

/// A box merging two texts, "Hello" and "World", tagged "test", as Compose's `TestContent`.
fn test_content() -> ComposeTestRule {
    compose_test(|| {
        Box!(modifier = Modifier.semantics_with(true, |config| config.set_test_tag("test")), {
            BasicText!("Hello");
            BasicText!("World");
        });
    })
}

/// A node of `properties` alone, as Compose's `BoundaryNode`.
fn boundary_node(properties: fn(&mut SemanticsConfiguration)) {
    Column!(modifier = Modifier.semantics(properties), {});
}

/// The assertion `block` makes fails.
#[track_caller]
fn assert_fails(block: impl FnOnce()) {
    expect_error(block);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, equals
#[test]
fn equals() {
    let mut ui = test_content();
    ui.on_node_with_tag("test").assert_text_equals(&["Hello", "World"]);
    ui.on_node_with_tag("test").assert_text_equals(&["World", "Hello"]);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, equals_empty
#[test]
fn equals_empty() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.semantics(|config| config.set_test_tag("test")));
    });
    ui.on_node_with_tag("test").assert_text_equals(&[]);
    ui.on_node_with_tag("test").assert_text_equals_with(&[], false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, contains
#[test]
fn contains() {
    let mut ui = test_content();
    ui.on_node_with_tag("test").assert_text_contains("Hello", false, false);
    ui.on_node_with_tag("test").assert_text_contains("World", false, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, contains_substring
#[test]
fn contains_substring() {
    let mut ui = test_content();
    ui.on_node_with_tag("test").assert_text_contains("He", true, false);
    ui.on_node_with_tag("test").assert_text_contains("Wo", true, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, equals_fails_notEnoughElements
#[test]
fn equals_fails_not_enough_elements() {
    let mut ui = test_content();
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_text_equals(&["Hello"]);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, equals_fails_tooManyElements
#[test]
fn equals_fails_too_many_elements() {
    let mut ui = test_content();
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_text_equals(&["Hello", "World", "More"]);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, textAndEditText
/// A text field is a merging node of its own: the box merging the texts doesn't get its
/// editable text, so the first assertion fails, as Compose's.
#[test]
fn text_and_edit_text() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.semantics_with(true, |config| config.set_test_tag("test")), {
            BasicText!("Hello");
            BasicText!("World");
            let state = remember_text_field_state("TextField", None);
            BasicTextFieldOfState!(state);
        });
    });
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_text_equals(&["Hello", "World", "TextField"]);
        ui.on_node_with_tag("test").assert_text_equals_with(&["Hello", "World"], false);
        ui.on_node_with_tag("test").assert_text_contains("TextField", false, false);
    });
    // What it has: the texts alone.
    ui.on_node_with_tag("test").assert_text_equals(&["Hello", "World"]);
}

/// A node whose editable text is "Hello World", tagged "test".
fn editable_hello_world() -> ComposeTestRule {
    compose_test(|| {
        boundary_node(|config| {
            config.set_test_tag("test");
            config.set_editable_text("Hello World");
        });
    })
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, assertTextFieldText_isOk
#[test]
fn assert_text_field_text_is_ok() {
    editable_hello_world().on_node_with_tag("test").assert_text_equals(&["Hello World"]);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, assertTextFieldText_fails
#[test]
fn assert_text_field_text_fails() {
    let mut ui = editable_hello_world();
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_text_equals(&["Hello"]);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, assertTextFieldText_fails2
#[test]
fn assert_text_field_text_fails2() {
    let mut ui = compose_test(|| {
        boundary_node(|config| {
            config.set_test_tag("test");
            config.set_input_text("Hello World");
        });
    });
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_text_equals(&["Hello"]);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, assertTextFieldText_substring_isOk
#[test]
fn assert_text_field_text_substring_is_ok() {
    editable_hello_world().on_node_with_tag("test").assert_text_contains("Hello", true, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, assertTextFieldText_substring_fails
#[test]
fn assert_text_field_text_substring_fails() {
    let mut ui = editable_hello_world();
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_text_contains("hello", false, false);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, assertTextFieldText_substring_ignoreCase_isOk
#[test]
fn assert_text_field_text_substring_ignore_case_is_ok() {
    editable_hello_world().on_node_with_tag("test").assert_text_contains("hello", true, true);
}

/// A box merging a text, "Visual Text", and a node whose input text is "Raw Input".
fn text_and_input_text() -> ComposeTestRule {
    compose_test(|| {
        Box!(modifier = Modifier.semantics_with(true, |config| config.set_test_tag("test")), {
            BasicText!("Visual Text");
            boundary_node(|config| config.set_input_text("Raw Input"));
        });
    })
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, textAndInputText_defaultIgnoresInputText
#[test]
fn text_and_input_text_default_ignores_input_text() {
    text_and_input_text().on_node_with_tag("test").assert_text_equals(&["Visual Text"]);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, textAndInputText_includeInputTextTrue
#[test]
fn text_and_input_text_include_input_text_true() {
    text_and_input_text().on_node_with_tag("test").assert_text_equals_including(
        &["Visual Text", "Raw Input"],
        true,
        true,
    );
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertText.kt, textAndInputText_includeInputTextTrue_failsIfMissing
#[test]
fn text_and_input_text_include_input_text_true_fails_if_missing() {
    let mut ui = text_and_input_text();
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_text_equals_including(&["Visual Text"], true, true);
    });
}

// ---- AssertContentDescription ------------------------------------------------------------------

/// A box merging two boxes described "Hello" and "World", tagged "test".
fn described_content() -> ComposeTestRule {
    compose_test(|| {
        Box!(modifier = Modifier.semantics_with(true, |config| config.set_test_tag("test")), {
            Box!(modifier = Modifier.semantics(|config| config.set_content_description("Hello")));
            Box!(modifier = Modifier.semantics(|config| config.set_content_description("World")));
        });
    })
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertContentDescription.kt, equals
#[test]
fn content_description_equals() {
    let mut ui = described_content();
    ui.on_node_with_tag("test").assert_content_description_equals(&["Hello", "World"]);
    ui.on_node_with_tag("test").assert_content_description_equals(&["World", "Hello"]);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertContentDescription.kt, equals_empty
#[test]
fn content_description_equals_empty() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.semantics(|config| config.set_test_tag("test")));
    });
    ui.on_node_with_tag("test").assert_content_description_equals(&[]);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertContentDescription.kt, contains
#[test]
fn content_description_contains() {
    let mut ui = described_content();
    ui.on_node_with_tag("test").assert_content_description_contains("Hello", false, false);
    ui.on_node_with_tag("test").assert_content_description_contains("World", false, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertContentDescription.kt, contains_substring
#[test]
fn content_description_contains_substring() {
    let mut ui = described_content();
    ui.on_node_with_tag("test").assert_content_description_contains("He", true, false);
    ui.on_node_with_tag("test").assert_content_description_contains("Wo", true, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertContentDescription.kt, equals_fails_notEnoughElements
#[test]
fn content_description_equals_fails_not_enough_elements() {
    let mut ui = described_content();
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_content_description_equals(&["Hello"]);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/assertions/AssertContentDescription.kt, equals_fails_tooManyElements
#[test]
fn content_description_equals_fails_too_many_elements() {
    let mut ui = described_content();
    assert_fails(|| {
        ui.on_node_with_tag("test").assert_content_description_equals(&["Hello", "World", "More"]);
    });
}

// ---- FocusActionsTest --------------------------------------------------------------------------

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/FocusActionsTest.kt, requestFocus_focuses
#[test]
fn request_focus_focuses() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.size(1.dp()).test_tag("tag_0").focusable());
        Box!(modifier = Modifier.size(1.dp()).test_tag("tag_1").focusable());
    });
    ui.on_node_with_tag("tag_0").assert_is_not_focused();
    ui.on_node_with_tag("tag_0").request_focus();
    ui.on_node_with_tag("tag_0").assert_is_focused();

    ui.on_node_with_tag("tag_1").request_focus();
    ui.on_node_with_tag("tag_1").assert_is_focused();
    ui.on_node_with_tag("tag_0").assert_is_not_focused();
}
