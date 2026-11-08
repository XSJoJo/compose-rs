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

// ---- MetaKeysTest (Compose's `Key.Function` and `isFnDown` have no compose-rs counterpart: the
// tests holding every meta key hold the other four) ----------------------------------------------

const META_KEYS: [Key; 4] = [Key::CtrlLeft, Key::AltLeft, Key::MetaLeft, Key::ShiftLeft];

fn meta_keys_down(keys: &KeyInjectionScope<'_>) -> [bool; 4] {
    [keys.is_ctrl_down(), keys.is_alt_down(), keys.is_meta_down(), keys.is_shift_down()]
}

fn press_all_meta_keys_down(ui: &mut ComposeTestRule) {
    perform_key_input(ui, |keys| META_KEYS.iter().for_each(|key| keys.key_down(*key)));
}

#[track_caller]
fn assert_all_meta_keys(ui: &mut ComposeTestRule, down: bool) {
    perform_key_input(ui, |keys| assert_eq!(meta_keys_down(keys), [down; 4]));
}

#[track_caller]
fn assert_key_down_is_true_with_key_down(key: Key) {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        assert!(!keys.is_key_down(key));
        keys.with_key_down(key, |keys| assert!(keys.is_key_down(key)));
        assert!(!keys.is_key_down(key));
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, metaKeys_startUp
#[test]
fn meta_keys_start_up() {
    let mut ui = focused_field();
    assert_all_meta_keys(&mut ui, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, metaKeys_areDown_afterDown
#[test]
fn meta_keys_are_down_after_down() {
    let mut ui = focused_field();
    press_all_meta_keys_down(&mut ui);
    assert_all_meta_keys(&mut ui, true);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, metaKeys_areUp_afterUp
#[test]
fn meta_keys_are_up_after_up() {
    let mut ui = focused_field();
    press_all_meta_keys_down(&mut ui);
    perform_key_input(&mut ui, |keys| META_KEYS.iter().for_each(|key| keys.key_up(*key)));
    assert_all_meta_keys(&mut ui, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, ctrl_isDown_withCtrlDown
#[test]
fn ctrl_is_down_with_ctrl_down() {
    assert_key_down_is_true_with_key_down(Key::CtrlLeft);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, alt_isDown_withAltDown
#[test]
fn alt_is_down_with_alt_down() {
    assert_key_down_is_true_with_key_down(Key::AltLeft);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, meta_isDown_withMetaDown
#[test]
fn meta_is_down_with_meta_down() {
    assert_key_down_is_true_with_key_down(Key::MetaLeft);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, shift_isDown_withShiftDown
#[test]
fn shift_is_down_with_shift_down() {
    assert_key_down_is_true_with_key_down(Key::ShiftLeft);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, allMetaKeysDown_withKeysDown
#[test]
fn all_meta_keys_down_with_keys_down() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        let held = [Key::CtrlRight, Key::AltRight, Key::MetaRight, Key::ShiftRight];
        keys.with_keys_down(&held, |keys| assert_eq!(meta_keys_down(keys), [true; 4]));
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, lettersTyped_withShiftDown_areUppercase
#[test]
#[ignore = "typed keys: a field doesn't type a key event's character (Compose's isTypedEvent, utf16CodePoint)"]
fn letters_typed_with_shift_down_are_uppercase() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        keys.press_key(Key::Character('a'));
        keys.with_key_down(Key::ShiftLeft, |keys| {
            keys.press_key(Key::Character('a'));
            keys.press_key(Key::Character('b'));
        });
        keys.press_key(Key::Character('b'));
    });
    assert_typed(&mut ui, "aABb");
}

/// `block` panics; the panic is caught, as Kotlin's `try { … } catch (e: Exception) {}`.
fn caught(block: impl FnOnce()) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(block));
    assert!(result.is_err());
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, withKeyDown_keyIsUp_afterThrow
#[test]
fn with_key_down_key_is_up_after_throw() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        caught(|| keys.with_key_down(Key::ShiftLeft, |_| panic!("thrown")));
        assert!(!keys.is_shift_down());
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/MetaKeysTest.kt, withKeysDown_keysAreUp_afterThrow
#[test]
fn with_keys_down_keys_are_up_after_throw() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        caught(|| keys.with_keys_down(&[Key::ShiftLeft, Key::AltRight], |_| panic!("thrown")));
        assert!(!keys.is_shift_down());
        assert!(!keys.is_alt_down());
    });
}

// ---- LockKeysTest (compose-rs has no numeric keypad digit keys: `numPadKeysPressed_*` and
// `withKeyToggled_turnsNumLockOff_*` aren't written) -------------------------------------------

const LOCK_KEYS: [Key; 3] = [Key::CapsLock, Key::NumLock, Key::ScrollLock];

fn lock_keys_on(keys: &KeyInjectionScope<'_>) -> [bool; 3] {
    [keys.is_caps_lock_on(), keys.is_num_lock_on(), keys.is_scroll_lock_on()]
}

fn toggle_all_lock_keys(ui: &mut ComposeTestRule) {
    perform_key_input(ui, |keys| LOCK_KEYS.iter().for_each(|key| keys.press_key(*key)));
}

#[track_caller]
fn assert_all_lock_keys(ui: &mut ComposeTestRule, on: bool) {
    perform_key_input(ui, |keys| assert_eq!(lock_keys_on(keys), [on; 3]));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, lockKeys_startOff
#[test]
fn lock_keys_start_off() {
    let mut ui = focused_field();
    assert_all_lock_keys(&mut ui, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, lockKeys_areOn_afterToggle
#[test]
fn lock_keys_are_on_after_toggle() {
    let mut ui = focused_field();
    toggle_all_lock_keys(&mut ui);
    assert_all_lock_keys(&mut ui, true);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, lockKeys_areOff_afterDoubleToggle
#[test]
fn lock_keys_are_off_after_double_toggle() {
    let mut ui = focused_field();
    toggle_all_lock_keys(&mut ui);
    toggle_all_lock_keys(&mut ui);
    assert_all_lock_keys(&mut ui, false);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, allLockKeysOn_withKeysToggled
#[test]
fn all_lock_keys_on_with_keys_toggled() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        keys.with_keys_toggled(&LOCK_KEYS, |keys| assert_eq!(lock_keys_on(keys), [true; 3]));
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, lettersTyped_withCapsLockOn_areUppercase
#[test]
#[ignore = "typed keys: a field doesn't type a key event's character (Compose's isTypedEvent, utf16CodePoint)"]
fn letters_typed_with_caps_lock_on_are_uppercase() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        keys.press_key(Key::Character('a'));
        keys.with_key_toggled(Key::CapsLock, |keys| {
            keys.press_key(Key::Character('a'));
            keys.press_key(Key::Character('b'));
        });
        keys.press_key(Key::Character('b'));
    });
    assert_typed(&mut ui, "aABb");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, withKeyToggled_turnsCapsLockOff_ifCapsLockAlreadyOn
#[test]
#[ignore = "typed keys: a field doesn't type a key event's character (Compose's isTypedEvent, utf16CodePoint)"]
fn with_key_toggled_turns_caps_lock_off_if_caps_lock_already_on() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        keys.press_key(Key::CapsLock);
        keys.press_key(Key::Character('a'));
        keys.press_key(Key::Character('b'));
        keys.with_key_toggled(Key::CapsLock, |keys| {
            keys.press_key(Key::Character('a'));
            keys.press_key(Key::Character('b'));
        });
        keys.press_key(Key::Character('a'));
        keys.press_key(Key::Character('b'));
    });
    assert_typed(&mut ui, "ABabAB");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, scrollLockOn_isTrue_withKeyToggled
#[test]
fn scroll_lock_on_is_true_with_key_toggled() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        assert!(!keys.is_scroll_lock_on());
        keys.with_key_toggled(Key::ScrollLock, |keys| assert!(keys.is_scroll_lock_on()));
        assert!(!keys.is_scroll_lock_on());
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, withKeyToggled_keyIsOff_afterThrow
#[test]
fn with_key_toggled_key_is_off_after_throw() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        caught(|| keys.with_key_toggled(Key::CapsLock, |_| panic!("thrown")));
        assert!(!keys.is_caps_lock_on());
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/key/LockKeysTest.kt, withKeysToggled_keysAreOff_afterThrow
#[test]
fn with_keys_toggled_keys_are_off_after_throw() {
    let mut ui = focused_field();
    perform_key_input(&mut ui, |keys| {
        caught(|| keys.with_keys_toggled(&[Key::CapsLock, Key::NumLock], |_| panic!("thrown")));
        assert!(!keys.is_caps_lock_on());
        assert!(!keys.is_num_lock_on());
    });
}
