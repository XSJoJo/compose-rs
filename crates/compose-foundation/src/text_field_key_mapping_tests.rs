//! The three key mappings run on any host (D1148): Compose's tests press Android's keys
//! (`platformDefaultKeyMapping`), Compose Desktop's run `defaultKeyMapping` off macOS. Each test
//! sets its mapping on its thread (`with_key_mapping`), so a macOS host checks Android's and
//! Linux's too.

use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{Density, IntSize, Key, Modifier, TextRange, TextStyle, UnitExt};

use crate::key_mapping::{KeyMapping, with_key_mapping};
use crate::{BasicTextFieldOfState, TextFieldLineLimits, TextFieldState};

const TAG: &str = "field";

/// A focused multi-line field over `text`, the cursor at `selection`, in Compose's test font at
/// 30 sp: `keysSequenceTest`'s.
struct Keys {
    ui: ComposeTestRule,
    state: TextFieldState,
}

impl Keys {
    fn new(text: &str, selection: TextRange) -> Keys {
        let state = **Box::leak(Box::new(TextFieldState::with_selection(text, selection)));
        let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
        let mut ui = compose_test_with(
            IntSize::new(600, 600),
            Density::new(1.0),
            Box::new(measurer),
            move || {
                let style = TextStyle { font_size: 30.sp(), ..TextStyle::DEFAULT };
                crate::BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.fill_max_size().test_tag(TAG),
                    text_style = style,
                    line_limits = TextFieldLineLimits::DEFAULT,
                );
            },
        );
        ui.on_node_with_tag(TAG).request_focus();
        ui.wait_for_idle();
        Keys { ui, state }
    }

    /// The keys held, then the last pressed.
    fn press(&mut self, keys: &[Key]) -> &mut Self {
        let (last, held) = keys.split_last().expect("a key");
        let last = *last;
        self.ui.on_node_with_tag(TAG).perform_key_input(|scope| {
            scope.with_keys_down(held, |scope| scope.press_key(last));
        });
        self.ui.wait_for_idle();
        self
    }

    #[track_caller]
    fn expect(&mut self, text: &str, selection: TextRange) -> &mut Self {
        self.ui.wait_for_idle();
        assert_eq!((self.state.text().as_str(), self.state.selection()), (text, selection));
        self
    }
}

const ALT: Key = Key::AltLeft;
const CTRL: Key = Key::CtrlLeft;
const SHIFT: Key = Key::ShiftLeft;
const META: Key = Key::MetaLeft;

fn h() -> Key {
    Key::Character('h')
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_lineEndStart
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_altLineLeftRight
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_altTop
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_altBottom
#[test]
fn android_moves_to_the_line_sides_and_the_text_ends_with_alt() {
    with_key_mapping(KeyMapping::Android, || {
        let mut keys = Keys::new("hello\nworld", TextRange::at(8));
        keys.press(&[ALT, Key::DirectionRight]).expect("hello\nworld", TextRange::at(11));
        keys.press(&[ALT, Key::DirectionLeft]).expect("hello\nworld", TextRange::at(6));
        keys.press(&[SHIFT, ALT, Key::DirectionRight])
            .expect("hello\nworld", TextRange::new(6, 11));
        keys.press(&[ALT, Key::DirectionUp]).expect("hello\nworld", TextRange::at(0));
        keys.press(&[SHIFT, ALT, Key::DirectionDown]).expect("hello\nworld", TextRange::new(0, 11));
        keys.press(&[ALT, Key::DirectionDown]).expect("hello\nworld", TextRange::at(11));
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_deleteToBeginningOfLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_deleteToEndOfLine
#[test]
fn android_deletes_to_the_line_ends_with_alt() {
    with_key_mapping(KeyMapping::Android, || {
        let mut keys = Keys::new("hello\nworld", TextRange::at(8));
        keys.press(&[ALT, Key::Backspace]).expect("hello\nrld", TextRange::at(6));
        keys.press(&[Key::DirectionRight]).press(&[ALT, Key::Delete]);
        keys.expect("hello\nr", TextRange::at(7));
    });
}

/// Ctrl+Alt (AltGr on a keyboard that has one) isn't Alt: Compose matches the modifiers whole.
#[test]
fn android_takes_no_ctrl_alt_motion() {
    with_key_mapping(KeyMapping::Android, || {
        let mut keys = Keys::new("hello world", TextRange::at(8));
        keys.press(&[CTRL, ALT, Key::DirectionLeft]).expect("hello world", TextRange::at(8));
        keys.press(&[CTRL, ALT, Key::DirectionUp]).expect("hello world", TextRange::at(8));
        keys.press(&[CTRL, ALT, Key::Backspace]).expect("hello world", TextRange::at(8));
        keys.press(&[ALT, Key::NumPadDirectionLeft]).expect("hello world", TextRange::at(8));
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_backspace
#[test]
fn ctrl_h_alone_deletes_back() {
    for mapping in [KeyMapping::Android, KeyMapping::Desktop] {
        with_key_mapping(mapping, || {
            let mut keys = Keys::new("hello", TextRange::at(5));
            keys.press(&[CTRL, h()]).expect("hell", TextRange::at(4));
            keys.press(&[CTRL, SHIFT, h()]).expect("hell", TextRange::at(4));
            keys.press(&[CTRL, ALT, h()]).expect("hell", TextRange::at(4));
        });
    }
}

/// macOS's Ctrl+H is the Emacs binding's, Backspace too, and Cmd+H isn't the field's.
#[test]
fn macos_ctrl_h_is_emacs_backspace() {
    with_key_mapping(KeyMapping::MacOs, || {
        let mut keys = Keys::new("hello", TextRange::at(5));
        keys.press(&[CTRL, h()]).expect("hell", TextRange::at(4));
        keys.press(&[META, h()]).expect("hell", TextRange::at(4));
    });
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/KeyMapping.kt, commonKeyMapping (Key.Y -> KeyCommand.REDO)
#[test]
fn ctrl_y_redoes() {
    for mapping in [KeyMapping::Android, KeyMapping::Desktop] {
        with_key_mapping(mapping, || {
            let mut keys = Keys::new("hello", TextRange::at(5));
            keys.press(&[Key::Backspace]).expect("hell", TextRange::at(4));
            keys.press(&[CTRL, Key::Character('z')]).expect("hello", TextRange::at(5));
            keys.press(&[CTRL, Key::Character('y')]).expect("hell", TextRange::at(4));
            keys.press(&[CTRL, Key::Character('z')]).expect("hello", TextRange::at(5));
            keys.press(&[CTRL, SHIFT, Key::Character('z')]).expect("hell", TextRange::at(4));
            // Ctrl+Shift+Y isn't redo.
            keys.press(&[CTRL, Key::Character('z')]).expect("hello", TextRange::at(5));
            keys.press(&[CTRL, SHIFT, Key::Character('y')]).expect("hello", TextRange::at(5));
        });
    }
}

/// Compose Desktop's default mapping (Linux, Windows) has no Alt motion: Alt+arrows and
/// Alt+Backspace are left to the field's ancestors; Alt+Delete deletes to the line's end; Ctrl
/// goes by words and paragraphs.
// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/KeyMapping.kt, defaultKeyMapping
#[test]
fn the_desktop_default_has_no_alt_motion() {
    with_key_mapping(KeyMapping::Desktop, || {
        let mut keys = Keys::new("hello\nworld wide", TextRange::at(8));
        keys.press(&[ALT, Key::DirectionLeft]).expect("hello\nworld wide", TextRange::at(8));
        keys.press(&[ALT, Key::DirectionRight]).expect("hello\nworld wide", TextRange::at(8));
        keys.press(&[ALT, Key::DirectionUp]).expect("hello\nworld wide", TextRange::at(8));
        keys.press(&[SHIFT, ALT, Key::DirectionDown]).expect("hello\nworld wide", TextRange::at(8));
        keys.press(&[ALT, Key::Backspace]).expect("hello\nworld wide", TextRange::at(8));
        keys.press(&[CTRL, Key::DirectionRight]).expect("hello\nworld wide", TextRange::at(11));
        keys.press(&[CTRL, SHIFT, Key::DirectionUp])
            .expect("hello\nworld wide", TextRange::new(11, 6));
        keys.press(&[Key::DirectionRight]).press(&[CTRL, Key::Backspace]);
        keys.expect("hello\n wide", TextRange::at(6));
        keys.press(&[ALT, Key::Delete]).expect("hello\n", TextRange::at(6));
    });
}

/// macOS's: Cmd to the line's sides and the text's ends, Option by words, Ctrl's Emacs bindings;
/// Cmd+Y isn't redo there.
#[test]
fn macos_moves_by_cmd_and_option() {
    with_key_mapping(KeyMapping::MacOs, || {
        let mut keys = Keys::new("hello\nworld wide", TextRange::at(8));
        keys.press(&[META, Key::DirectionLeft]).expect("hello\nworld wide", TextRange::at(6));
        keys.press(&[ALT, Key::DirectionRight]).expect("hello\nworld wide", TextRange::at(11));
        keys.press(&[CTRL, Key::Character('e')]).expect("hello\nworld wide", TextRange::at(16));
        keys.press(&[META, Key::DirectionUp]).expect("hello\nworld wide", TextRange::at(0));
        keys.press(&[META, Key::Backspace]).expect("hello\nworld wide", TextRange::at(0));
        keys.press(&[META, Key::Delete]).expect("\nworld wide", TextRange::at(0));
        keys.press(&[META, Key::Character('z')]).expect("hello\nworld wide", TextRange::at(0));
        keys.press(&[META, Key::Character('y')]).expect("hello\nworld wide", TextRange::at(0));
    });
}
