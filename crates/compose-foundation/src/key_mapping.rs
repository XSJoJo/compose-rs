//! What a text field's hardware keys do, as Compose's `KeyMapping`: a key with its modifiers maps
//! to a [`KeyCommand`], by one of three mappings (D1148). Android's is Compose's
//! `platformDefaultKeyMapping` on Android (`KeyMapping.android.kt`: Alt to the line's and the
//! text's ends, then `defaultKeyMapping`); other desktops' is Compose Desktop's
//! `defaultKeyMapping` (`KeyMapping.kt`: Ctrl's words and paragraphs, Alt+Delete alone to the
//! line's end); macOS's is Compose Desktop's macOS one (Cmd for lines, Option for words, Ctrl's
//! Emacs bindings). Android's and the default match the modifiers exactly, as Compose's `when`
//! over `KeyModifiers`: Ctrl+Alt (AltGr) or Ctrl+Shift+H map to nothing.

use std::cell::Cell;

use compose_ui::{Key, KeyModifiers};

/// The three mappings: the one a build uses is its platform's, which tests can change on their
/// thread (`with_key_mapping`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyMapping {
    /// Compose's `platformDefaultKeyMapping` on Android.
    Android,
    /// Compose Desktop's `defaultKeyMapping`: Linux and Windows.
    Desktop,
    /// Compose Desktop's macOS mapping.
    MacOs,
}

/// A key's command, as Compose's `KeyCommand`; `select` says whether it extends the selection
/// (Compose's `SELECT_*` commands).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyCommand {
    LeftChar,
    RightChar,
    LeftWord,
    RightWord,
    PrevParagraph,
    NextParagraph,
    Up,
    Down,
    PageUp,
    PageDown,
    /// The visual line's start (Home), as `LINE_START`.
    LineStart,
    LineEnd,
    /// The line's left side (Alt+Left on Android, Cmd+Left on macOS), as `LINE_LEFT`.
    LineLeft,
    LineRight,
    /// The text's start, as `HOME`.
    Home,
    End,
    DeletePrevChar,
    DeleteNextChar,
    DeletePrevWord,
    DeleteNextWord,
    DeleteFromLineStart,
    DeleteToLineEnd,
    /// macOS's Ctrl+K: to the line's end, or the new line at it (Cocoa's `deleteToEndOfParagraph`).
    KillToLineEnd,
    NewLine,
    Tab,
    Copy,
    Paste,
    Cut,
    SelectAll,
    Undo,
    Redo,
    Deselect,
    /// The D-pad's center, as `CENTER`.
    Center,
}

impl KeyCommand {
    /// Whether it edits the text, as Compose's `editsText`: a read-only field doesn't take it.
    pub(crate) fn edits_text(self) -> bool {
        matches!(
            self,
            KeyCommand::DeletePrevChar
                | KeyCommand::DeleteNextChar
                | KeyCommand::DeletePrevWord
                | KeyCommand::DeleteNextWord
                | KeyCommand::DeleteFromLineStart
                | KeyCommand::DeleteToLineEnd
                | KeyCommand::KillToLineEnd
                | KeyCommand::NewLine
                | KeyCommand::Tab
                | KeyCommand::Paste
                | KeyCommand::Cut
                | KeyCommand::Undo
                | KeyCommand::Redo
        )
    }
}

/// A command and whether it selects.
pub(crate) type Mapped = (KeyCommand, bool);

thread_local! {
    /// The mapping tests set on their thread, else the platform's.
    static OVERRIDE: Cell<Option<KeyMapping>> = const { Cell::new(None) };
}

impl KeyMapping {
    /// The platform's: Android's on Android, macOS's on macOS, the default elsewhere.
    const PLATFORM: KeyMapping = if cfg!(target_os = "android") {
        KeyMapping::Android
    } else if cfg!(target_os = "macos") {
        KeyMapping::MacOs
    } else {
        KeyMapping::Desktop
    };

    /// The mapping in use on this thread.
    pub(crate) fn current() -> KeyMapping {
        OVERRIDE.with(Cell::get).unwrap_or(KeyMapping::PLATFORM)
    }

    /// Whether `modifiers` hold the shortcuts' modifier: Cmd on macOS, Ctrl elsewhere.
    pub(crate) fn shortcut(self, modifiers: KeyModifiers) -> bool {
        if self == KeyMapping::MacOs { modifiers.meta } else { modifiers.ctrl }
    }

    /// Whether `modifiers` are macOS's Emacs bindings' (Ctrl without Cmd).
    pub(crate) fn emacs(self, modifiers: KeyModifiers) -> bool {
        self == KeyMapping::MacOs && modifiers.ctrl && !modifiers.meta
    }

    /// The command of `key` with `modifiers`.
    pub(crate) fn map(self, key: Key, modifiers: KeyModifiers) -> Option<Mapped> {
        match self {
            KeyMapping::Android => android(key, modifiers),
            KeyMapping::Desktop => default(key, modifiers),
            KeyMapping::MacOs => macos(key, modifiers),
        }
    }
}

/// Runs `block` with `mapping` on this thread, as the tests of another platform's keys do.
#[cfg(test)]
pub(crate) fn with_key_mapping<R>(mapping: KeyMapping, block: impl FnOnce() -> R) -> R {
    struct Restore(Option<KeyMapping>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDE.with(|it| it.set(self.0));
        }
    }
    let _restore = Restore(OVERRIDE.with(|it| it.replace(Some(mapping))));
    block()
}

/// Modifiers as Compose's `KeyModifiers` constants, compared whole.
const NONE: KeyModifiers = KeyModifiers { shift: false, ctrl: false, alt: false, meta: false };
const SHIFT: KeyModifiers = KeyModifiers { shift: true, ..NONE };
const CTRL: KeyModifiers = KeyModifiers { ctrl: true, ..NONE };
const ALT: KeyModifiers = KeyModifiers { alt: true, ..NONE };
const CTRL_SHIFT: KeyModifiers = KeyModifiers { ctrl: true, shift: true, ..NONE };
const ALT_SHIFT: KeyModifiers = KeyModifiers { alt: true, shift: true, ..NONE };
const SHIFT_META: KeyModifiers = KeyModifiers { shift: true, meta: true, ..NONE };

/// Whether `key` is the letter `letter` (either case), as Compose's `Key.A` to `Key.Z`.
fn is_letter(key: Key, letter: char) -> bool {
    matches!(key, Key::Character(c) if c.eq_ignore_ascii_case(&letter))
}

/// Compose's `platformDefaultKeyMapping` on Android: Alt (with Shift, selecting) to the line's
/// sides and the text's ends, Alt+Backspace from the line's start, then `defaultKeyMapping`.
fn android(key: Key, modifiers: KeyModifiers) -> Option<Mapped> {
    let select = match modifiers {
        ALT_SHIFT => true,
        ALT => false,
        _ => return default(key, modifiers),
    };
    let command = match key {
        Key::DirectionLeft => KeyCommand::LineLeft,
        Key::DirectionRight => KeyCommand::LineRight,
        Key::DirectionUp => KeyCommand::Home,
        Key::DirectionDown => KeyCommand::End,
        Key::Backspace if !select => KeyCommand::DeleteFromLineStart,
        _ => return default(key, modifiers),
    };
    Some((command, select))
}

/// Compose's `defaultKeyMapping` ("non macOS"): Ctrl's words, paragraphs and Backspace by words,
/// Ctrl+H back by a character, Ctrl+Delete a word, Ctrl+\ deselecting, Alt+Delete to the line's
/// end, then `commonKeyMapping` with Ctrl.
fn default(key: Key, modifiers: KeyModifiers) -> Option<Mapped> {
    let command = match (key, modifiers) {
        (Key::Backspace, NONE | SHIFT | SHIFT_META) => Some(KeyCommand::DeletePrevChar),
        (Key::Backspace, CTRL | CTRL_SHIFT) => Some(KeyCommand::DeletePrevWord),
        (Key::Enter | Key::NumPadEnter, NONE | SHIFT | CTRL | CTRL_SHIFT) => {
            Some(KeyCommand::NewLine)
        }
        _ => None,
    };
    if let Some(command) = command {
        return Some((command, false));
    }
    let select = modifiers.shift;
    let command = match modifiers {
        CTRL_SHIFT | CTRL => match key {
            Key::DirectionLeft | Key::NumPadDirectionLeft => Some(KeyCommand::LeftWord),
            Key::DirectionRight | Key::NumPadDirectionRight => Some(KeyCommand::RightWord),
            Key::DirectionUp | Key::NumPadDirectionUp => Some(KeyCommand::PrevParagraph),
            Key::DirectionDown | Key::NumPadDirectionDown => Some(KeyCommand::NextParagraph),
            _ if select => None,
            Key::Delete => Some(KeyCommand::DeleteNextWord),
            Key::Character('\\') => Some(KeyCommand::Deselect),
            _ if is_letter(key, 'h') => Some(KeyCommand::DeletePrevChar),
            _ => None,
        },
        SHIFT => match key {
            Key::MoveHome | Key::NumPadMoveHome => Some(KeyCommand::LineStart),
            Key::MoveEnd | Key::NumPadMoveEnd => Some(KeyCommand::LineEnd),
            _ => None,
        },
        ALT => match key {
            Key::Delete => Some(KeyCommand::DeleteToLineEnd),
            _ => None,
        },
        _ => None,
    };
    match command {
        Some(command) => Some((command, select)),
        None => common(key, modifiers, CTRL),
    }
}

/// Compose's `commonKeyMapping(systemShortcutModifiers)`: the clipboard, select all, undo and
/// redo with the shortcut's modifier, and the plain keys' (with Shift, selecting).
fn common(key: Key, modifiers: KeyModifiers, shortcut: KeyModifiers) -> Option<Mapped> {
    let shortcut_shift = KeyModifiers { shift: true, ..shortcut };
    if modifiers == shortcut_shift {
        return is_letter(key, 'z').then_some((KeyCommand::Redo, false));
    }
    if modifiers == shortcut {
        let command = match key {
            Key::Insert | Key::NumPadInsert => KeyCommand::Copy,
            _ if is_letter(key, 'c') => KeyCommand::Copy,
            _ if is_letter(key, 'v') => KeyCommand::Paste,
            _ if is_letter(key, 'x') => KeyCommand::Cut,
            _ if is_letter(key, 'a') => KeyCommand::SelectAll,
            _ if is_letter(key, 'y') => KeyCommand::Redo,
            _ if is_letter(key, 'z') => KeyCommand::Undo,
            _ => return None,
        };
        return Some((command, false));
    }
    let select = match modifiers {
        SHIFT => true,
        NONE => false,
        _ => return None,
    };
    let command = match key {
        Key::DirectionLeft | Key::NumPadDirectionLeft => KeyCommand::LeftChar,
        Key::DirectionRight | Key::NumPadDirectionRight => KeyCommand::RightChar,
        Key::DirectionUp | Key::NumPadDirectionUp => KeyCommand::Up,
        Key::DirectionDown | Key::NumPadDirectionDown => KeyCommand::Down,
        Key::PageUp | Key::NumPadPageUp => KeyCommand::PageUp,
        Key::PageDown | Key::NumPadPageDown => KeyCommand::PageDown,
        Key::MoveHome | Key::NumPadMoveHome => KeyCommand::LineStart,
        Key::MoveEnd | Key::NumPadMoveEnd => KeyCommand::LineEnd,
        Key::Insert | Key::NumPadInsert if select => return Some((KeyCommand::Paste, false)),
        _ if select => return None,
        Key::DirectionCenter => KeyCommand::Center,
        Key::Enter | Key::NumPadEnter => KeyCommand::NewLine,
        Key::Backspace => KeyCommand::DeletePrevChar,
        Key::Delete => KeyCommand::DeleteNextChar,
        Key::Paste => KeyCommand::Paste,
        Key::Cut => KeyCommand::Cut,
        Key::Copy => KeyCommand::Copy,
        Key::Tab => KeyCommand::Tab,
        _ => return None,
    };
    Some((command, select))
}

/// Compose Desktop's macOS mapping, as compose-rs had it (D1147): Cmd with the arrows to the
/// line's sides and the text's ends, Cmd+Backspace and Cmd+Delete to the line's ends; Option by
/// words and paragraphs; Ctrl's Emacs bindings (A, E, B, F, P, N, D, H as Home, End, the arrows,
/// Delete and Backspace; K killing to the line's end); Ctrl+Home and Ctrl+End to the text's ends;
/// the clipboard, select all, undo and redo (Cmd+Shift+Z) with Cmd. Shift selects with any of
/// them, and the other modifiers held are ignored.
fn macos(key: Key, modifiers: KeyModifiers) -> Option<Mapped> {
    if modifiers.ctrl
        && !modifiers.meta
        && let Key::Character(c) = key
    {
        let without_ctrl = KeyModifiers { ctrl: false, ..modifiers };
        let mapped = match c.to_ascii_lowercase() {
            'a' => Key::MoveHome,
            'e' => Key::MoveEnd,
            'b' => Key::DirectionLeft,
            'f' => Key::DirectionRight,
            'p' => Key::DirectionUp,
            'n' => Key::DirectionDown,
            'd' => Key::Delete,
            'h' => Key::Backspace,
            'k' => return Some((KeyCommand::KillToLineEnd, false)),
            _ => return None,
        };
        return macos(mapped, without_ctrl);
    }
    let select = modifiers.shift;
    let (by_line, by_word) = (modifiers.meta, modifiers.alt);
    let plain = !modifiers.shift && !modifiers.alt && !modifiers.meta;
    let command = match key {
        Key::Character(c) if modifiers.meta => {
            let command = match c.to_ascii_lowercase() {
                'a' => KeyCommand::SelectAll,
                'c' => KeyCommand::Copy,
                'x' => KeyCommand::Cut,
                'v' => KeyCommand::Paste,
                'z' if modifiers.shift => KeyCommand::Redo,
                'z' => KeyCommand::Undo,
                '\\' if !modifiers.shift && !modifiers.alt => KeyCommand::Deselect,
                _ => return None,
            };
            return Some((command, false));
        }
        Key::Copy if plain => return Some((KeyCommand::Copy, false)),
        Key::Cut if plain => return Some((KeyCommand::Cut, false)),
        Key::Paste if plain => return Some((KeyCommand::Paste, false)),
        Key::Insert | Key::NumPadInsert if modifiers.meta && !modifiers.shift && !modifiers.alt => {
            return Some((KeyCommand::Copy, false));
        }
        Key::Insert | Key::NumPadInsert if modifiers.shift && !modifiers.meta && !modifiers.alt => {
            return Some((KeyCommand::Paste, false));
        }
        Key::Tab if !modifiers.shift && !modifiers.meta && !modifiers.alt => {
            return Some((KeyCommand::Tab, false));
        }
        Key::Backspace if by_line => return Some((KeyCommand::DeleteFromLineStart, false)),
        Key::Backspace if by_word => return Some((KeyCommand::DeletePrevWord, false)),
        Key::Backspace => return Some((KeyCommand::DeletePrevChar, false)),
        Key::Delete if by_line => return Some((KeyCommand::DeleteToLineEnd, false)),
        Key::Delete if by_word => return Some((KeyCommand::DeleteNextWord, false)),
        Key::Delete => return Some((KeyCommand::DeleteNextChar, false)),
        Key::Enter | Key::NumPadEnter => return Some((KeyCommand::NewLine, false)),
        // As `commonKeyMapping`'s, with no modifier.
        Key::DirectionCenter if modifiers == NONE => return Some((KeyCommand::Center, false)),
        Key::DirectionLeft | Key::NumPadDirectionLeft if by_line => KeyCommand::LineLeft,
        Key::DirectionLeft | Key::NumPadDirectionLeft if by_word => KeyCommand::LeftWord,
        Key::DirectionLeft | Key::NumPadDirectionLeft => KeyCommand::LeftChar,
        Key::DirectionRight | Key::NumPadDirectionRight if by_line => KeyCommand::LineRight,
        Key::DirectionRight | Key::NumPadDirectionRight if by_word => KeyCommand::RightWord,
        Key::DirectionRight | Key::NumPadDirectionRight => KeyCommand::RightChar,
        Key::DirectionUp | Key::NumPadDirectionUp if by_line => KeyCommand::Home,
        Key::DirectionUp | Key::NumPadDirectionUp if by_word => KeyCommand::PrevParagraph,
        Key::DirectionUp | Key::NumPadDirectionUp => KeyCommand::Up,
        Key::DirectionDown | Key::NumPadDirectionDown if by_line => KeyCommand::End,
        Key::DirectionDown | Key::NumPadDirectionDown if by_word => KeyCommand::NextParagraph,
        Key::DirectionDown | Key::NumPadDirectionDown => KeyCommand::Down,
        Key::PageUp | Key::NumPadPageUp => KeyCommand::PageUp,
        Key::PageDown | Key::NumPadPageDown => KeyCommand::PageDown,
        Key::MoveHome | Key::NumPadMoveHome if modifiers.ctrl => KeyCommand::Home,
        Key::MoveHome | Key::NumPadMoveHome => KeyCommand::LineStart,
        Key::MoveEnd | Key::NumPadMoveEnd if modifiers.ctrl => KeyCommand::End,
        Key::MoveEnd | Key::NumPadMoveEnd => KeyCommand::LineEnd,
        _ => return None,
    };
    Some((command, select))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(shift: bool, ctrl: bool, alt: bool, meta: bool) -> KeyModifiers {
        KeyModifiers { shift, ctrl, alt, meta }
    }

    // Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/KeyMapping.kt, defaultKeyMapping
    #[test]
    fn the_default_matches_its_modifiers_exactly() {
        let map = |key, modifiers| KeyMapping::Desktop.map(key, modifiers);
        assert_eq!(map(Key::Character('h'), CTRL), Some((KeyCommand::DeletePrevChar, false)));
        assert_eq!(map(Key::Character('h'), CTRL_SHIFT), None);
        assert_eq!(map(Key::Character('h'), mods(false, true, true, false)), None);
        assert_eq!(map(Key::DirectionLeft, mods(false, true, true, false)), None);
        assert_eq!(map(Key::Backspace, ALT), None);
        assert_eq!(map(Key::DirectionLeft, ALT), None);
        assert_eq!(map(Key::Delete, ALT), Some((KeyCommand::DeleteToLineEnd, false)));
        assert_eq!(map(Key::Backspace, SHIFT_META), Some((KeyCommand::DeletePrevChar, false)));
        assert_eq!(map(Key::Character('a'), CTRL_SHIFT), None);
        assert_eq!(map(Key::Character('z'), CTRL_SHIFT), Some((KeyCommand::Redo, false)));
        assert_eq!(map(Key::Character('y'), CTRL), Some((KeyCommand::Redo, false)));
        assert_eq!(
            map(Key::NumPadDirectionUp, CTRL_SHIFT),
            Some((KeyCommand::PrevParagraph, true))
        );
        assert_eq!(map(Key::Delete, CTRL_SHIFT), None);
        assert_eq!(map(Key::MoveHome, CTRL), None);
        assert_eq!(map(Key::Enter, ALT), None);
    }

    // Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/text/KeyMapping.android.kt, platformDefaultKeyMapping
    #[test]
    fn android_adds_alt_alone_or_with_shift() {
        let map = |key, modifiers| KeyMapping::Android.map(key, modifiers);
        assert_eq!(map(Key::DirectionLeft, ALT), Some((KeyCommand::LineLeft, false)));
        assert_eq!(map(Key::DirectionDown, ALT_SHIFT), Some((KeyCommand::End, true)));
        assert_eq!(map(Key::Backspace, ALT), Some((KeyCommand::DeleteFromLineStart, false)));
        assert_eq!(map(Key::Backspace, ALT_SHIFT), None);
        assert_eq!(map(Key::NumPadDirectionLeft, ALT), None);
        assert_eq!(map(Key::DirectionLeft, mods(false, true, true, false)), None);
        assert_eq!(map(Key::Delete, ALT), Some((KeyCommand::DeleteToLineEnd, false)));
    }
}
