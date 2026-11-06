//! The state-based field's hardware keys, from Compose's `TextFieldKeyEventTest`: typing, the
//! clipboard's shortcuts, moving by characters, words, lines, paragraphs and pages, deleting, Tab
//! and Enter, undo, and which key events reach the field's ancestors.
//!
//! Compose's tests press Android's keys (`platformDefaultKeyMapping`): Ctrl moves by words and
//! paragraphs, Alt by lines and to the text's ends. compose-rs maps them so on Android, as Compose
//! Desktop's `defaultKeyMapping` on Linux and Windows (no Alt motion) and as Compose Desktop does
//! on macOS (Option by words and paragraphs, Cmd by lines), so these tests hold the host's
//! modifier (`word`, `line`, `shortcut`), D1147, D1148; every mapping's own keys are checked on
//! any host by the crate's `text_field_key_mapping_tests.rs`.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{Density, ImeAction, IntSize, Key, KeyEvent, KeyEventType, TextRange, UnitExt};

use crate::text_field_basic::type_on_keys;

const TAG: &str = "field";

/// Words (and paragraphs with Up and Down): Option on macOS, Ctrl elsewhere.
fn word() -> Key {
    if cfg!(target_os = "macos") { Key::AltLeft } else { Key::CtrlLeft }
}

/// Lines (and the text's ends with Up and Down): Cmd on macOS, Alt on Android. Compose
/// Desktop's default mapping (Linux, Windows) has none: there, Android's is the unit tests'
/// (`text_field_key_mapping_tests.rs`, D1148).
fn line() -> Option<Key> {
    if cfg!(target_os = "macos") {
        Some(Key::MetaLeft)
    } else if cfg!(target_os = "android") {
        Some(Key::AltLeft)
    } else {
        None
    }
}

/// The clipboard's and undo's shortcuts: Cmd on macOS, Ctrl elsewhere.
fn shortcut() -> Key {
    if cfg!(target_os = "macos") { Key::MetaLeft } else { Key::CtrlLeft }
}

fn character(c: char) -> Key {
    Key::Character(c)
}

/// What `keysSequenceTest` sets up.
#[derive(Clone, Copy)]
struct Setup {
    selection: TextRange,
    /// A size in dp (`requiredSize`); else the field fills the root.
    size: Option<f32>,
    single_line: bool,
    secure: bool,
}

const DEFAULT: Setup =
    Setup { selection: TextRange::ZERO, size: None, single_line: false, secure: false };

/// A focused field over `text` in Compose's test font at 30 sp, density 1, as `keysSequenceTest`.
struct Sequence {
    ui: ComposeTestRule,
    state: TextFieldState,
}

impl Sequence {
    fn new(text: &str) -> Sequence {
        Sequence::with(text, DEFAULT)
    }

    fn with(text: &str, setup: Setup) -> Sequence {
        let state = **Box::leak(Box::new(TextFieldState::with_selection(text, setup.selection)));
        let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
        let mut ui = compose_test_with(
            IntSize::new(600, 600),
            Density::new(1.0),
            Box::new(measurer),
            move || {
                let style = TextStyle { font_size: 30.sp(), ..TextStyle::DEFAULT };
                let modifier = match setup.size {
                    Some(size) => Modifier.required_size(size.dp()),
                    None => Modifier.fill_max_size(),
                }
                .test_tag(TAG);
                if setup.secure {
                    BasicSecureTextField!(state, modifier = modifier, text_style = style);
                } else {
                    let line_limits = if setup.single_line {
                        TextFieldLineLimits::SingleLine
                    } else {
                        TextFieldLineLimits::DEFAULT
                    };
                    BasicTextFieldOfState!(
                        state,
                        modifier = modifier,
                        text_style = style,
                        line_limits = line_limits,
                    );
                }
            },
        );
        ui.on_node_with_tag(TAG).request_focus();
        ui.wait_for_idle();
        Sequence { ui, state }
    }

    /// The keys held, then the last pressed, as the test's `press(keys)`.
    fn press(&mut self, keys: &[Key]) -> &mut Self {
        let (last, held) = keys.split_last().expect("a key");
        let last = *last;
        self.ui.on_node_with_tag(TAG).perform_key_input(|scope| {
            scope.with_keys_down(held, |scope| scope.press_key(last));
        });
        self
    }

    fn press_key(&mut self, key: Key) -> &mut Self {
        self.press(&[key])
    }

    /// Characters typed on keys: compose-rs's keys carry no text, the platform sends it.
    fn typed(&mut self, text: &str) -> &mut Self {
        type_on_keys(&mut self.ui, text);
        self
    }

    #[track_caller]
    fn expect_text(&mut self, text: &str) -> &mut Self {
        self.ui.wait_for_idle();
        assert_eq!(self.state.text().as_str(), text);
        self
    }

    #[track_caller]
    fn expect_selection(&mut self, start: usize, end: usize) -> &mut Self {
        self.ui.wait_for_idle();
        assert_eq!(self.state.selection(), TextRange::new(start, end));
        self
    }

    fn clipboard(&mut self) -> Option<String> {
        self.ui.host().with_tree(|tree| tree.clipboard().get_text())
    }

    fn set_clipboard(&mut self, text: &str) {
        self.ui.host().with_tree(|tree| tree.clipboard().set_text(text));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_typedEvents
#[test]
fn typed_keys_type() {
    Sequence::new("").typed("h").typed("I").expect_text("hI");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_copyPaste
#[test]
fn copy_then_paste() {
    let mut keys = Sequence::new("hello");
    keys.press(&[shortcut(), character('a')]).press(&[shortcut(), character('c')]);
    keys.press_key(Key::DirectionRight).typed(" ");
    keys.press(&[shortcut(), character('v')]).expect_text("hello hello");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_directCopyPaste
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_directCutPaste
#[test]
fn the_clipboard_keys_copy_cut_and_paste() {
    let mut keys = Sequence::new("hello");
    keys.press(&[shortcut(), character('a')]).press_key(Key::Copy).expect_text("hello");
    keys.press_key(Key::DirectionRight).typed(" ");
    keys.press_key(Key::Paste).expect_text("hello hello");
    let mut keys = Sequence::new("hello");
    keys.press(&[shortcut(), character('a')]).press_key(Key::Cut).expect_text("");
    keys.press_key(Key::Paste).expect_text("hello");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, secureTextField_doesNotAllowCopy
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, secureTextField_doesNotAllowCut
#[test]
fn a_secure_field_is_neither_copied_nor_cut() {
    let mut keys = Sequence::with("hello", Setup { secure: true, ..DEFAULT });
    keys.set_clipboard("world");
    keys.press(&[shortcut(), character('a')]).press(&[shortcut(), character('c')]);
    keys.press_key(Key::Copy);
    assert_eq!(keys.clipboard().as_deref(), Some("world"));
    keys.press(&[shortcut(), character('x')]).press_key(Key::Cut).expect_text("hello");
    assert_eq!(keys.clipboard().as_deref(), Some("world"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_linesNavigation
#[test]
fn up_and_down_move_through_lines() {
    let mut keys = Sequence::new("hello\nworld");
    keys.press_key(Key::DirectionDown).typed("a").press_key(Key::DirectionUp).typed("a");
    keys.expect_text("haello\naworld");
    keys.press_key(Key::DirectionUp).typed("a").expect_text("ahaello\naworld");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_linesNavigation_cache
#[test]
fn up_and_down_keep_where_they_started_across() {
    let mut keys = Sequence::new("hello\n\nworld");
    keys.press_key(Key::DirectionRight).press_key(Key::DirectionDown).press_key(Key::DirectionDown);
    keys.typed("0").expect_text("hello\n\nw0orld");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_linesNavigation_cache_resets_at_start
#[test]
fn reaching_the_start_forgets_where_up_and_down_started() {
    let mut keys = Sequence::new("hello\n\nworld");
    keys.press_key(Key::DirectionRight).press_key(Key::DirectionRight);
    keys.press_key(Key::DirectionDown).press_key(Key::DirectionUp).press_key(Key::DirectionUp);
    keys.press_key(Key::DirectionDown).press_key(Key::DirectionDown).typed("0");
    keys.expect_text("hello\n\n0world");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_linesNavigation_cache_resets_at_end
#[test]
fn reaching_the_end_forgets_where_up_and_down_started() {
    let mut keys = Sequence::new("hello\n\nworld");
    keys.press_key(Key::DirectionRight).press_key(Key::DirectionRight);
    keys.press_key(Key::DirectionDown).press_key(Key::DirectionDown).press_key(Key::DirectionDown);
    keys.press_key(Key::DirectionUp).press_key(Key::DirectionUp).typed("0");
    keys.expect_text("hello0\n\nworld");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_newLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_enterMultiLine
#[test]
fn enter_types_a_new_line() {
    Sequence::new("hello").press_key(Key::Enter).expect_text("\nhello");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_backspace
#[test]
fn backspace_deletes_back() {
    let mut keys = Sequence::new("hello");
    keys.press_key(Key::DirectionRight).press_key(Key::DirectionRight);
    keys.press_key(Key::Backspace).expect_text("hllo");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_backspace_withDiacritic
#[test]
fn backspace_deletes_a_combining_mark_alone() {
    let mut keys = Sequence::new("e\u{301}f");
    keys.press(&[word(), Key::DirectionRight]);
    keys.press_key(Key::Backspace).expect_text("e\u{301}");
    keys.press_key(Key::Backspace).expect_text("e");
    keys.press_key(Key::Backspace).expect_text("");
    keys.press_key(Key::Backspace).expect_text("");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_backspace_withEmoji
#[test]
fn backspace_deletes_a_joined_emoji_whole() {
    let emoji = "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F469}";
    let mut keys = Sequence::new(emoji);
    keys.press(&[word(), Key::DirectionRight]);
    keys.press_key(Key::Backspace).expect_text("");
}

/// Backspace deletes an emoji whole, as EmojiCompat's `getEmojiStart` finds it: a keycap (its
/// digit isn't pictographic, its VS16 and keycap mark are), a flag, a lone regional indicator after
/// one, a skin tone, a family joined by ZWJs, a heart with VS16; else a code point: the second of
/// two combining marks, a Hangul syllable's last jamo (D1147).
// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/text/StringHelpers.android.kt, findCodePointOrEmojiStartBefore
#[test]
fn backspace_deletes_an_emoji_whole_or_a_code_point() {
    let cases = [
        ("a1\u{FE0F}\u{20E3}", "a"),
        ("a\u{1F1EB}\u{1F1F7}", "a"),
        ("\u{1F1EB}\u{1F1F7}\u{1F1E9}", "\u{1F1EB}\u{1F1F7}"),
        ("a\u{1F44D}\u{1F3FD}", "a"),
        ("a\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}", "a"),
        ("a\u{2764}\u{FE0F}", "a"),
        ("b\u{301}\u{302}", "b\u{301}"),
        ("\u{1100}\u{1161}\u{11A8}", "\u{1100}\u{1161}"),
    ];
    for (text, left) in cases {
        let at_end = Setup { selection: TextRange::at(text.len()), ..DEFAULT };
        Sequence::with(text, at_end).press_key(Key::Backspace).expect_text(left);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_delete
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_delete_atEnd
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_delete_whenEmpty
#[test]
fn delete_deletes_forward_and_nothing_at_the_end() {
    Sequence::new("hello").press_key(Key::Delete).expect_text("ello");
    let at_end = Setup { selection: TextRange::at(5), ..DEFAULT };
    Sequence::with("hello", at_end).press_key(Key::Delete).expect_text("hello");
    Sequence::new("").press_key(Key::Delete).expect_text("");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_nextWord
#[test]
fn the_word_modifier_moves_to_the_next_words_end() {
    let mut keys = Sequence::new("hello world");
    keys.press(&[word(), Key::DirectionRight]).typed("0").expect_text("hello0 world");
    keys.press(&[word(), Key::DirectionRight]).typed("0").expect_text("hello0 world0");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_nextWord_doubleSpace
#[test]
fn the_next_word_is_past_two_spaces() {
    let mut keys = Sequence::new("hello  world");
    keys.press(&[word(), Key::DirectionRight]).press_key(Key::DirectionRight);
    keys.press(&[word(), Key::DirectionRight]).typed("0").expect_text("hello  world0");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_prevWord
#[test]
fn the_word_modifier_moves_back_to_a_words_start() {
    let mut keys = Sequence::new("hello world");
    keys.ui.on_node_with_tag(TAG).perform_key_input(|scope| {
        scope.with_key_down(word(), |scope| {
            scope.press_key(Key::DirectionRight);
            scope.press_key(Key::DirectionRight);
            scope.press_key(Key::DirectionLeft);
        });
    });
    keys.typed("0").expect_text("hello 0world");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_HomeAndEnd
#[test]
fn home_and_end() {
    let mut keys = Sequence::new("hello world");
    keys.press_key(Key::MoveEnd).typed("0").press_key(Key::MoveHome).typed("0");
    keys.expect_text("0hello world0");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_byWordSelection
#[test]
fn shift_and_the_word_modifier_select_by_words() {
    let mut keys = Sequence::new("hello  world\nhi");
    let held = [Key::ShiftLeft, word()];
    keys.press(&[held[0], held[1], Key::DirectionRight]).expect_selection(0, 5);
    keys.press(&[held[0], held[1], Key::DirectionRight]).expect_selection(0, 12);
    keys.press(&[held[0], held[1], Key::DirectionRight]).expect_selection(0, 15);
    keys.press(&[held[0], held[1], Key::DirectionLeft]).expect_selection(0, 13);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_lineEndStart
#[test]
fn home_and_end_reach_the_lines_ends_and_shift_selects() {
    let mut keys = Sequence::new("hi\nhello world\nhi");
    keys.press_key(Key::MoveEnd).press_key(Key::DirectionRight).typed("0");
    keys.expect_text("hi\n0hello world\nhi");
    keys.press_key(Key::MoveEnd).typed("0").expect_text("hi\n0hello world0\nhi");
    keys.press(&[Key::ShiftLeft, Key::MoveHome]).expect_selection(16, 3);
    keys.press_key(Key::MoveHome).press_key(Key::DirectionRight);
    keys.press(&[Key::ShiftLeft, Key::MoveEnd]).expect_selection(4, 16);
    keys.expect_text("hi\n0hello world0\nhi");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_altLineLeftRight
#[test]
fn the_line_modifier_moves_to_the_lines_ends() {
    let Some(line) = line() else { return };
    let mut keys = Sequence::new("hi\nhello world\nhi");
    keys.press(&[line, Key::DirectionRight]).press_key(Key::DirectionRight).typed("0");
    keys.expect_text("hi\n0hello world\nhi");
    keys.press(&[line, Key::DirectionRight]).typed("0").expect_text("hi\n0hello world0\nhi");
    keys.press(&[Key::ShiftLeft, line, Key::DirectionLeft]).expect_selection(16, 3);
    keys.press(&[line, Key::DirectionLeft]).press_key(Key::DirectionRight);
    keys.press(&[Key::ShiftLeft, line, Key::DirectionRight]).expect_selection(4, 16);
    keys.expect_text("hi\n0hello world0\nhi");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_altTop
#[test]
fn the_line_modifier_with_up_goes_to_the_start() {
    let Some(line) = line() else { return };
    let mut keys = Sequence::new("hi\nhello world\nhi");
    keys.press_key(Key::MoveEnd);
    for _ in 0..3 {
        keys.press_key(Key::DirectionRight);
    }
    keys.typed("0").expect_text("hi\nhe0llo world\nhi");
    keys.press(&[line, Key::DirectionUp]).typed("0").expect_text("0hi\nhe0llo world\nhi");
    keys.press_key(Key::MoveEnd);
    for _ in 0..3 {
        keys.press_key(Key::DirectionRight);
    }
    keys.press(&[Key::ShiftLeft, line, Key::DirectionUp]).expect_selection(6, 0);
    keys.expect_text("0hi\nhe0llo world\nhi");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_altBottom
#[test]
fn the_line_modifier_with_down_goes_to_the_end() {
    let Some(line) = line() else { return };
    let mut keys = Sequence::new("hi\nhello world\nhi");
    keys.press_key(Key::MoveEnd);
    for _ in 0..3 {
        keys.press_key(Key::DirectionRight);
    }
    keys.typed("0").expect_text("hi\nhe0llo world\nhi");
    keys.press(&[Key::ShiftLeft, line, Key::DirectionDown]).expect_selection(6, 18);
    keys.press_key(Key::DirectionLeft).typed("0").expect_text("hi\nhe00llo world\nhi");
    keys.press(&[line, Key::DirectionDown]).typed("0").expect_text("hi\nhe00llo world\nhi0");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_deleteToBeginningOfLine
#[test]
fn the_line_modifier_with_backspace_deletes_to_the_lines_start() {
    let Some(line) = line() else { return };
    let mut keys = Sequence::new("hello world\nhi world");
    keys.press(&[word(), Key::DirectionRight]);
    keys.press(&[line, Key::Backspace]).expect_text(" world\nhi world");
    keys.press(&[line, Key::Backspace]).expect_text(" world\nhi world");
    for _ in 0..3 {
        keys.press_key(Key::DirectionRight);
    }
    keys.press(&[line, Key::Backspace]).expect_text("rld\nhi world");
    keys.press_key(Key::DirectionDown).press_key(Key::MoveEnd);
    keys.press(&[line, Key::Backspace]).expect_text("rld\n");
    keys.press(&[line, Key::Backspace]).expect_text("rld\n");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_deleteToEndOfLine
#[test]
fn the_line_modifier_with_delete_deletes_to_the_lines_end() {
    let Some(line) = line() else { return };
    let mut keys = Sequence::new("hello world\nhi world");
    keys.press(&[word(), Key::DirectionRight]);
    keys.press(&[line, Key::Delete]).expect_text("hello\nhi world");
    keys.press(&[line, Key::Delete]).expect_text("hello\nhi world");
    for _ in 0..3 {
        keys.press_key(Key::DirectionRight);
    }
    keys.press(&[line, Key::Delete]).expect_text("hello\nhi");
    keys.press_key(Key::MoveHome);
    keys.press(&[line, Key::Delete]).expect_text("hello\n");
    keys.press(&[line, Key::Delete]).expect_text("hello\n");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_paragraphNavigation
#[test]
fn the_word_modifier_with_up_and_down_moves_by_paragraphs() {
    let mut keys = Sequence::new("hello world\nhi");
    keys.press(&[word(), Key::DirectionDown]).typed("0").expect_text("hello world0\nhi");
    keys.press(&[word(), Key::DirectionDown]).press(&[word(), Key::DirectionUp]);
    keys.typed("0").expect_text("hello world0\n0hi");
    keys.press(&[word(), Key::DirectionUp]).press(&[word(), Key::DirectionUp]);
    keys.typed("0").expect_text("0hello world0\n0hi");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_selectionCaret
#[test]
fn the_shortcut_with_a_backslash_collapses_the_selection_to_its_end() {
    let mut keys = Sequence::new("hello world");
    keys.press(&[word(), Key::ShiftLeft, Key::DirectionRight]).expect_selection(0, 5);
    keys.press(&[Key::ShiftLeft, Key::DirectionRight]).expect_selection(0, 6);
    keys.press(&[shortcut(), character('\\')]).expect_selection(6, 6);
    keys.press(&[word(), Key::ShiftLeft, Key::DirectionLeft]).expect_selection(6, 0);
    keys.press(&[Key::ShiftLeft, Key::DirectionRight]).expect_selection(6, 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_pageNavigationDown
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_pageNavigationDown_exactFit
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_pageNavigationUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_pageNavigationUp_exactFit
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_pageNavigationUp_cantGoUp
#[test]
fn page_up_and_down_move_by_the_fields_height() {
    let sized = |selection: usize, size: f32| Setup {
        selection: TextRange::at(selection),
        size: Some(size),
        ..DEFAULT
    };
    // 73 dp shows two lines of 30; 90, exactly three.
    Sequence::with("A\nB\nC\nD\nE", sized(0, 73.0)).press_key(Key::PageDown).expect_selection(4, 4);
    Sequence::with("A\nB\nC\nD\nE", sized(0, 90.0)).press_key(Key::PageDown).expect_selection(6, 6);
    Sequence::with("A\nB\nC\nD\nE", sized(8, 73.0)).press_key(Key::PageUp).expect_selection(4, 4);
    Sequence::with("A\nB\nC\nD\nE", sized(8, 90.0)).press_key(Key::PageUp).expect_selection(2, 2);
    Sequence::with("1\n2\n3\n4\n5", sized(0, 90.0)).press_key(Key::PageUp).expect_selection(0, 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_tabSingleLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_shiftTabSingleLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_enterSingleLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_withActiveSelection_tabSingleLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_withActiveSelection_shiftTabSingleLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_withActiveSelection_enterSingleLine
#[test]
fn a_single_lines_tab_and_enter_type_nothing() {
    let single = Setup { single_line: true, ..DEFAULT };
    for keys in [&[Key::Tab][..], &[Key::ShiftLeft, Key::Tab], &[Key::Enter]] {
        Sequence::with("text", single).press(keys).expect_text("text");
        let mut selected = Sequence::with("text", single);
        selected.press_key(Key::DirectionRight);
        selected.press(&[Key::ShiftLeft, Key::DirectionRight]);
        selected.press(&[Key::ShiftLeft, Key::DirectionRight]);
        selected.press(keys).expect_text("text");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_tabMultiLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_withActiveSelection_tabMultiLine
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_withActiveSelection_enterMultiLine
#[test]
fn a_multi_lines_tab_and_enter_type_over_the_selection() {
    Sequence::new("text").press_key(Key::Tab).expect_text("\ttext");
    for (key, typed) in [(Key::Tab, "t\tt"), (Key::Enter, "t\nt")] {
        let mut keys = Sequence::new("text");
        keys.press_key(Key::DirectionRight);
        keys.press(&[Key::ShiftLeft, Key::DirectionRight]);
        keys.press(&[Key::ShiftLeft, Key::DirectionRight]);
        keys.press_key(key).expect_text(typed);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_selectToLeft
#[test]
fn shift_with_left_selects_back() {
    let mut keys = Sequence::new("hello world hello");
    keys.press_key(Key::MoveEnd).expect_selection(17, 17);
    for _ in 0..3 {
        keys.press(&[Key::ShiftLeft, Key::DirectionLeft]);
    }
    keys.expect_selection(17, 14);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_simpleUndo
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_simpleRedo
#[test]
fn undo_and_redo_on_keys() {
    let mut keys = Sequence::new("hello");
    keys.press(&[word(), Key::DirectionRight]).typed(" abc").expect_text("hello abc");
    keys.press(&[shortcut(), character('z')]).expect_text("hello");
    keys.press(&[shortcut(), Key::ShiftLeft, character('z')]).expect_text("hello abc");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_ctrlBackspace
#[test]
fn the_word_modifier_with_backspace_deletes_the_word_before() {
    let at_end = Setup { selection: TextRange::at(11), ..DEFAULT };
    let mut keys = Sequence::with("hello world", at_end);
    keys.press(&[word(), Key::Backspace]).expect_text("hello ");
    keys.press(&[word(), Key::ShiftLeft, Key::Backspace]).expect_text("");
}

/// A field in a box hearing the keys its field leaves, and a clickable box around both.
fn in_parents(
    line_limits: TextFieldLineLimits,
    heard: Rc<RefCell<Vec<KeyEvent>>>,
    clicks: MutableState<i32>,
    actions: MutableState<i32>,
    consume: bool,
) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::new("")));
    let ui = compose_test(move || {
        let heard = heard.clone();
        Box!(Modifier.clickable(move || clicks.set(clicks.get() + 1)), {
            let heard = heard.clone();
            Box!(
                Modifier.on_key_event(move |event: KeyEvent| {
                    heard.borrow_mut().push(event);
                    consume
                }),
                {
                    BasicTextFieldOfState!(
                        state,
                        modifier = Modifier.test_tag(TAG),
                        line_limits = line_limits,
                        keyboard_options = KeyboardOptions {
                            ime_action: ImeAction::Next,
                            ..KeyboardOptions::DEFAULT
                        },
                        on_keyboard_action = Some(KeyboardActionHandler::new(move |_| {
                            actions.set(actions.get() + 1)
                        })),
                    );
                }
            );
        });
    });
    (ui, state)
}

fn counter() -> MutableState<i32> {
    Box::leak(Box::new(mutable_state_of(0))).handle()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_singleLine_pressEnter_parentClickable
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_multiLine_pressEnter_parentClickable
#[test]
fn enter_is_the_fields_not_a_clickable_parents() {
    for (line_limits, action, text) in
        [(TextFieldLineLimits::SingleLine, 1, ""), (TextFieldLineLimits::DEFAULT, 0, "\n")]
    {
        let (clicks, actions) = (counter(), counter());
        let (mut ui, state) = in_parents(line_limits, Rc::default(), clicks, actions, false);
        ui.on_node_with_tag(TAG).request_focus();
        ui.on_node_with_tag(TAG).perform_key_input(|scope| scope.press_key(Key::Enter));
        assert_eq!(clicks.get(), 0);
        assert_eq!(actions.get(), action);
        assert_eq!(state.text().as_str(), text);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_consumedKeyDownEvent_keyUpDoesNotPropagate
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_simultaneousConsumedKeyDownEvents_keyUpDoesNotPropagate
#[test]
fn typed_keys_do_not_reach_the_parent_up_or_down() {
    let heard: Rc<RefCell<Vec<KeyEvent>>> = Rc::default();
    let (mut ui, state) =
        in_parents(TextFieldLineLimits::DEFAULT, heard.clone(), counter(), counter(), true);
    ui.on_node_with_tag(TAG).request_focus();
    type_on_keys(&mut ui, "a");
    assert!(heard.borrow().is_empty());
    assert_eq!(state.text().as_str(), "a");
    // Three held at once, let go in reverse.
    for c in ['b', 'c', 'd'] {
        ui.host().send_key(KeyEvent::down(character(c)));
        ui.host().send_text(&c.to_string());
    }
    for c in ['d', 'c', 'b'] {
        ui.host().send_key(KeyEvent::up(character(c)));
    }
    ui.wait_for_idle();
    assert!(heard.borrow().is_empty());
    assert_eq!(state.text().as_str(), "abcd");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, textField_simultaneousMetaConsumedKeyDownEvents_keyUpDoesNotPropagate
#[test]
fn shift_alone_reaches_the_parent() {
    let heard: Rc<RefCell<Vec<KeyEvent>>> = Rc::default();
    let (mut ui, state) =
        in_parents(TextFieldLineLimits::DEFAULT, heard.clone(), counter(), counter(), true);
    ui.on_node_with_tag(TAG).request_focus();
    ui.on_node_with_tag(TAG).perform_key_input(|scope| {
        scope.key_down(Key::ShiftLeft);
        scope.key_down(character('A'));
    });
    ui.host().send_text("A");
    ui.on_node_with_tag(TAG).perform_key_input(|scope| {
        scope.key_up(Key::ShiftLeft);
        scope.key_up(character('A'));
    });
    let heard: Vec<(Key, KeyEventType)> =
        heard.borrow().iter().map(|event| (event.key, event.kind)).collect();
    assert_eq!(
        heard,
        [(Key::ShiftLeft, KeyEventType::KeyDown), (Key::ShiftLeft, KeyEventType::KeyUp)]
    );
    assert_eq!(state.text().as_str(), "A");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyEventTest.kt, singleLineTextField_enterIsNotConsumed_withDefaultKeyboardAction
#[test]
fn a_single_lines_enter_with_the_default_action_reaches_the_parent() {
    let heard: Rc<RefCell<Vec<KeyEvent>>> = Rc::default();
    let into = heard.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        Box!(
            Modifier.on_key_event(move |event: KeyEvent| {
                into.borrow_mut().push(event);
                false
            }),
            {
                BasicTextFieldOfState!(
                    remember_text_field_state("", None),
                    modifier = Modifier.test_tag(TAG),
                    line_limits = TextFieldLineLimits::SingleLine,
                );
            }
        );
    });
    ui.on_node_with_tag(TAG).request_focus();
    ui.on_node_with_tag(TAG).perform_key_input(|scope| scope.press_key(Key::Enter));
    let enters: Vec<KeyEventType> =
        heard.borrow().iter().filter(|event| event.key == Key::Enter).map(|e| e.kind).collect();
    assert_eq!(enters, [KeyEventType::KeyDown, KeyEventType::KeyUp]);
}
