//! One key stroke on a state-based field over "Hello", from Compose's `CommonTextFieldKeyEventTest`:
//! the arrows, Home, End, Page Up and Down and Insert, the numeric keypad's too, alone and with
//! Shift; Backspace; Shift and Ctrl with Enter. The keypad's keys are Compose's desktop ones
//! (Android has none, D1147). Ctrl+Insert is the platform's shortcut with Insert (Cmd on macOS).

use compose_foundation::*;
use compose_test::*;
use compose_ui::{Key, TextRange};

const TAG: &str = "field";
const HELLO: &str = "Hello";

fn shortcut() -> Key {
    if cfg!(target_os = "macos") { Key::MetaLeft } else { Key::CtrlLeft }
}

/// After `keys` (held, then the last pressed) on a focused field over `text` with `selection`
/// and `clipboard`: its text, selection and clipboard.
fn stroke(
    text: &str,
    selection: TextRange,
    clipboard: Option<&str>,
    keys: &[Key],
) -> (String, TextRange, Option<String>) {
    let state = **Box::leak(Box::new(TextFieldState::with_selection(text, selection)));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG));
    });
    if let Some(clipboard) = clipboard {
        ui.host().with_tree(|tree| tree.clipboard().set_text(clipboard));
    }
    ui.on_node_with_tag(TAG).request_focus();
    let (last, held) = keys.split_last().expect("a key");
    let last = *last;
    ui.on_node_with_tag(TAG).perform_key_input(|scope| {
        scope.with_keys_down(held, |scope| scope.press_key(last));
    });
    ui.wait_for_idle();
    let clipboard = ui.host().with_tree(|tree| tree.clipboard().get_text());
    (state.text().as_str().to_string(), state.selection(), clipboard)
}

#[track_caller]
fn selects(text: &str, from: usize, keys: &[Key], start: usize, end: usize) {
    let (_, selection, _) = stroke(text, TextRange::at(from), None, keys);
    assert_eq!(selection, TextRange::new(start, end), "{keys:?} from {from} in {text:?}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_left
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numPadLeft
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_right
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numPadRight
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_up
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numPadUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_down
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numpadDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_pageUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numPadPageUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_pageDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numPadPageDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_home
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numPadHome
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_end
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_numPadEnd
#[test]
fn the_navigation_keys_move_the_cursor() {
    let two_lines = "Hello\nWorld";
    for (plain, pad) in [
        (Key::DirectionLeft, Key::NumPadDirectionLeft),
        (Key::DirectionRight, Key::NumPadDirectionRight),
        (Key::DirectionUp, Key::NumPadDirectionUp),
        (Key::DirectionDown, Key::NumPadDirectionDown),
        (Key::PageUp, Key::NumPadPageUp),
        (Key::PageDown, Key::NumPadPageDown),
        (Key::MoveHome, Key::NumPadMoveHome),
        (Key::MoveEnd, Key::NumPadMoveEnd),
    ] {
        for key in [plain, pad] {
            let (text, from, to) = match plain {
                Key::DirectionLeft => (HELLO, 1, 0),
                Key::DirectionRight => (HELLO, 0, 1),
                Key::DirectionUp => (HELLO, 2, 0),
                Key::DirectionDown => (HELLO, 2, 5),
                Key::PageUp => (two_lines, 6, 0),
                Key::PageDown => (two_lines, 0, 6),
                Key::MoveHome => (HELLO, 5, 0),
                _ => (HELLO, 0, 5),
            };
            selects(text, from, &[key], to, to);
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftLeft
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadLeft
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftRight
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadRight
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumpadDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftPageUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadPageUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftPageDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadPageDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftHome
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadHome
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftEnd
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadEnd
#[test]
fn shift_with_the_navigation_keys_selects() {
    let two_lines = "Hello\nWorld";
    for (plain, pad) in [
        (Key::DirectionLeft, Key::NumPadDirectionLeft),
        (Key::DirectionRight, Key::NumPadDirectionRight),
        (Key::DirectionUp, Key::NumPadDirectionUp),
        (Key::DirectionDown, Key::NumPadDirectionDown),
        (Key::PageUp, Key::NumPadPageUp),
        // Compose's `textField_shiftNumPadPageDown` presses Page Down itself.
        (Key::PageDown, Key::NumPadPageDown),
        (Key::MoveHome, Key::NumPadMoveHome),
        (Key::MoveEnd, Key::NumPadMoveEnd),
    ] {
        for key in [plain, pad] {
            let (text, from, to) = match plain {
                Key::DirectionLeft => (HELLO, 1, 0),
                Key::DirectionRight => (HELLO, 0, 1),
                Key::DirectionUp => (HELLO, 2, 0),
                Key::DirectionDown => (HELLO, 2, 5),
                Key::PageUp => (two_lines, 6, 0),
                Key::PageDown => (two_lines, 0, 6),
                Key::MoveHome => (HELLO, 5, 0),
                _ => (HELLO, 0, 5),
            };
            selects(text, from, &[Key::ShiftLeft, key], from, to);
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftInsert
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftNumPadInsert
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shortcutInsert
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shortcutNumPadInsert
#[test]
fn insert_pastes_with_shift_and_copies_with_the_shortcut() {
    for insert in [Key::Insert, Key::NumPadInsert] {
        let (text, ..) = stroke(HELLO, TextRange::ZERO, Some("X"), &[Key::ShiftLeft, insert]);
        assert_eq!(text, "XHello", "{insert:?}");
        let all = TextRange::new(0, HELLO.len());
        let (text, _, clipboard) = stroke(HELLO, all, Some("X"), &[shortcut(), insert]);
        assert_eq!((text.as_str(), clipboard.as_deref()), (HELLO, Some(HELLO)), "{insert:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_backspace
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftBackspace
#[test]
fn backspace_with_or_without_shift_deletes_back() {
    for keys in [&[Key::Backspace][..], &[Key::ShiftLeft, Key::Backspace]] {
        let (text, selection, _) = stroke(HELLO, TextRange::at(5), None, keys);
        assert_eq!((text.as_str(), selection), ("Hell", TextRange::at(4)), "{keys:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_shiftEnter_insertsNewline
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/CommonTextFieldKeyEventTest.kt, textField_ctrlEnter_insertsNewline
#[test]
fn shift_or_ctrl_with_enter_types_a_new_line() {
    for modifier in [Key::ShiftLeft, Key::CtrlLeft] {
        let (text, ..) = stroke("helloworld", TextRange::at(5), None, &[modifier, Key::Enter]);
        assert_eq!(text, "hello\nworld", "{modifier:?}");
    }
}
