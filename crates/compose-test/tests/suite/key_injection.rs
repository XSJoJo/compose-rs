//! Compose's `injectionscope/key` tests typing into a field: `KeyDownTest`, through Compose's
//! `TestTextField` (a field, then a text saying what it holds).

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{Key, Modifier, UnitExt};

use super::input::expect_error;

const TAG: &str = "TestTextField";

/// A field of 400 by 300 dp tagged [`TAG`], then a text "This text was typed: " and what the
/// field holds, as Compose's `TestTextField`.
fn test_text_field() {
    Column!(modifier = Modifier.padding(20.dp()), {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.padding(20.dp()).test_tag(TAG).required_size((400.dp(), 300.dp()))
        );
        BasicText!(format!("This text was typed: {}", state.text()));
    });
}

/// The field, focused by a click, as `KeyDownTest`'s `setUp`.
fn focused_field() -> ComposeTestRule {
    let mut ui = compose_test(test_text_field);
    ui.on_node_with_tag(TAG).perform_click();
    ui
}

/// Keys sent to the field, as Compose's `Common.performKeyInput`.
fn perform_key_input(ui: &mut ComposeTestRule, block: impl FnOnce(&mut KeyInjectionScope<'_>)) {
    ui.on_node_with_tag(TAG).perform_key_input(block);
}

/// The field holds `expected`, as Compose's `Common.assertTyped`.
#[track_caller]
fn assert_typed(ui: &mut ComposeTestRule, expected: &str) {
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_text_contains(expected, false, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/KeyDownTest.kt, doubleDown_throwsIllegalStateException
#[test]
fn double_down_throws_illegal_state_exception() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| keys.key_down(Key::Enter));
    let message = expect_error(|| {
        perform_key_input(&mut ui, |keys| keys.key_down(Key::Enter));
    });
    assert!(
        message.contains("Cannot send key down event, Key(Enter) is already pressed down."),
        "{message}"
    );
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/KeyDownTest.kt, unDownedKey_isNotDown
#[test]
fn un_downed_key_is_not_down() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| assert!(!keys.is_key_down(Key::Character('a'))));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/KeyDownTest.kt, downedKey_isDown
#[test]
fn downed_key_is_down() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        keys.key_down(Key::Character('a'));
        assert!(keys.is_key_down(Key::Character('a')));
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/KeyDownTest.kt, enterDown_typesNewLine
#[test]
fn enter_down_types_new_line() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| keys.key_down(Key::Enter));
    assert_typed(&mut ui, "\n");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/KeyDownTest.kt, letterDown_typesLetter
#[test]
#[ignore = "typed keys: a field doesn't type a key event's character (Compose's isTypedEvent, utf16CodePoint)"]
fn letter_down_types_letter() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| keys.key_down(Key::Character('a')));
    assert_typed(&mut ui, "a");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/KeyDownTest.kt, letterDownWithShiftDown_typesCapitalLetter
#[test]
#[ignore = "typed keys: a field doesn't type a key event's character (Compose's isTypedEvent, utf16CodePoint)"]
fn letter_down_with_shift_down_types_capital_letter() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        keys.key_down(Key::ShiftLeft);
        keys.key_down(Key::Character('a'));
    });
    assert_typed(&mut ui, "A");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/KeyDownTest.kt, semicolonWithShiftDown_typesColon
#[test]
#[ignore = "typed keys: a field doesn't type a key event's character (Compose's isTypedEvent, utf16CodePoint)"]
fn semicolon_with_shift_down_types_colon() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        keys.key_down(Key::ShiftLeft);
        keys.key_down(Key::Character(';'));
    });
    assert_typed(&mut ui, ":");
}
