//! Compose's `inputdispatcher/KeyAndMouseEventsTest`, through the key and mouse injection
//! scopes: the keys held and the locks on are told by the mouse's events. What the focused root
//! hears is recorded in one log (`dispatcher.rs`): its key events and its pointer events in
//! compose-rs's model (`mouse_events.rs` says how Android's events map to it; a press with no
//! hover first comes with an `Enter`). compose-rs has no Fn key (`Key::Other(119)` here), so
//! its meta bit isn't told; Android's left and right masks are the four modifiers.

use compose_test::*;
use compose_ui::{
    Key, KeyEventType, KeyModifiers, Offset, PointerButtons, PointerEventType,
    PointerKeyboardModifiers,
};

use super::dispatcher::*;
use KeyEventType::{KeyDown as Down, KeyUp as Up};
use PointerEventType::{Enter, Move, Press, Release, Scroll};

const T: u64 = 16;
const POSITION_1: Offset = Offset::new(1.0, 1.0);
const FUNCTION: Key = Key::Other(119);

const NONE: KeyModifiers = KeyModifiers { shift: false, ctrl: false, alt: false, meta: false };
const ALL: KeyModifiers = KeyModifiers { shift: true, ctrl: true, alt: true, meta: true };

/// What a pointer event's keys say: shift, ctrl, alt, meta, Fn, caps, num and scroll lock.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Told {
    keys: KeyModifiers,
    function: bool,
    caps: bool,
    num: bool,
    scroll: bool,
}

fn told(modifiers: PointerKeyboardModifiers) -> Told {
    Told {
        keys: KeyModifiers {
            shift: modifiers.is_shift_pressed(),
            ctrl: modifiers.is_ctrl_pressed(),
            alt: modifiers.is_alt_pressed(),
            meta: modifiers.is_meta_pressed(),
        },
        function: modifiers.is_function_pressed(),
        caps: modifiers.is_caps_lock_on(),
        num: modifiers.is_num_lock_on(),
        scroll: modifiers.is_scroll_lock_on(),
    }
}

const ALL_LOCKS: Told = Told { keys: NONE, function: false, caps: true, num: true, scroll: true };

/// A pointer event's kind and what its keys say, a key event's key, kind and modifiers.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Summary {
    Pointer(PointerEventType, Told),
    Key(Key, KeyEventType, KeyModifiers),
}

fn summaries(heard: &[Heard]) -> Vec<Summary> {
    heard
        .iter()
        .map(|heard| match heard {
            Heard::Event { kind, modifiers, .. } => Summary::Pointer(*kind, told(*modifiers)),
            Heard::Key { key, kind, modifiers, .. } => Summary::Key(*key, *kind, *modifiers),
            Heard::Cancel { .. } => panic!("a cancel: {heard:?}"),
        })
        .collect()
}

fn pointer(kind: PointerEventType, told: Told) -> Summary {
    Summary::Pointer(kind, told)
}

fn key(key: Key, kind: KeyEventType, modifiers: KeyModifiers) -> Summary {
    Summary::Key(key, kind, modifiers)
}

struct Recording {
    ui: ComposeTestRule,
    log: Log,
}

impl Recording {
    fn new() -> Recording {
        let (ui, log) = recording_root_with_keys();
        Recording { ui, log }
    }

    fn keys(&mut self, block: impl FnOnce(&mut KeyInjectionScope<'_>)) {
        self.ui.on_node_with_tag(ROOT).perform_key_input(block);
    }

    fn mouse(&mut self, block: impl FnOnce(&mut MouseInjectionScope<'_>)) {
        self.ui.on_node_with_tag(ROOT).perform_mouse_input(block);
    }

    fn heard(&self) -> Vec<Heard> {
        assert_has_valid_event_times(&self.log);
        heard(&self.log)
    }
}

fn press(scope: &mut KeyInjectionScope<'_>, key: Key) {
    scope.key_down(key);
    scope.key_up(key);
}

/// A modifier key held through a mouse click: its events tell it; released, the next press
/// doesn't, as `verifyMetaKeyClickState`.
#[track_caller]
fn verify_meta_key_click_state(held: Key, expected: KeyModifiers) {
    let mut it = Recording::new();
    it.keys(|scope| scope.key_down(held));
    it.mouse(|scope| {
        scope.press(MouseButton::Primary);
        scope.release(MouseButton::Primary);
    });
    it.keys(|scope| scope.key_up(held));
    it.mouse(|scope| scope.press(MouseButton::Primary));
    let with = Told { keys: expected, ..Told::default() };
    assert_eq!(
        summaries(&it.heard()),
        [
            key(held, Down, expected),
            pointer(Enter, with),
            pointer(Press, with),
            pointer(Release, with),
            pointer(Move, with),
            key(held, Up, NONE),
            pointer(Press, Told::default()),
        ]
    );
}

/// A lock key toggled on through a mouse click: its events tell it; toggled off, the next press
/// doesn't, as `verifyLockKeyClickState`.
#[track_caller]
fn verify_lock_key_click_state(lock: Key, on: Told) {
    let mut it = Recording::new();
    it.keys(|scope| press(scope, lock));
    it.mouse(|scope| {
        assert_eq!(scope.current_position(), Offset::ZERO);
        scope.press(MouseButton::Primary);
        scope.release(MouseButton::Primary);
    });
    it.keys(|scope| press(scope, lock));
    it.mouse(|scope| scope.press(MouseButton::Primary));
    assert_eq!(
        summaries(&it.heard()),
        [
            key(lock, Down, NONE),
            key(lock, Up, NONE),
            pointer(Enter, on),
            pointer(Press, on),
            pointer(Release, on),
            pointer(Move, on),
            key(lock, Down, NONE),
            key(lock, Up, NONE),
            pointer(Press, Told::default()),
        ]
    );
}

const SHIFT: KeyModifiers = KeyModifiers { shift: true, ..NONE };
const CTRL: KeyModifiers = KeyModifiers { ctrl: true, ..NONE };
const ALT: KeyModifiers = KeyModifiers { alt: true, ..NONE };
const META: KeyModifiers = KeyModifiers { meta: true, ..NONE };

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, functionKey_metaState_generatedCorrectly
#[test]
fn function_key_meta_state_generated_correctly() {
    verify_meta_key_click_state(FUNCTION, NONE);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, leftCtrl_metaState_generatedCorrectly
#[test]
fn left_ctrl_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::CtrlLeft, CTRL);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, rightCtrl_metaState_generatedCorrectly
#[test]
fn right_ctrl_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::CtrlRight, CTRL);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, leftAlt_metaState_generatedCorrectly
#[test]
fn left_alt_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::AltLeft, ALT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, rightAlt_metaState_generatedCorrectly
#[test]
fn right_alt_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::AltRight, ALT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, leftMeta_metaState_generatedCorrectly
#[test]
fn left_meta_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::MetaLeft, META);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, rightMeta_metaState_generatedCorrectly
#[test]
fn right_meta_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::MetaRight, META);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, leftShift_metaState_generatedCorrectly
#[test]
fn left_shift_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::ShiftLeft, SHIFT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, rightShift_metaState_generatedCorrectly
#[test]
fn right_shift_meta_state_generated_correctly() {
    verify_meta_key_click_state(Key::ShiftRight, SHIFT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, capsLock_metaState_generatedCorrectly
#[test]
fn caps_lock_meta_state_generated_correctly() {
    verify_lock_key_click_state(Key::CapsLock, Told { caps: true, ..Told::default() });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, numLock_metaState_generatedCorrectly
#[test]
fn num_lock_meta_state_generated_correctly() {
    verify_lock_key_click_state(Key::NumLock, Told { num: true, ..Told::default() });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, scrollLock_metaState_generatedCorrectly
#[test]
fn scroll_lock_meta_state_generated_correctly() {
    verify_lock_key_click_state(Key::ScrollLock, Told { scroll: true, ..Told::default() });
}

const META_KEYS: [Key; 9] = [
    FUNCTION,
    Key::CtrlLeft,
    Key::CtrlRight,
    Key::AltLeft,
    Key::AltRight,
    Key::MetaLeft,
    Key::MetaRight,
    Key::ShiftLeft,
    Key::ShiftRight,
];

fn press_locks(it: &mut Recording) {
    it.keys(|scope| {
        press(scope, Key::CapsLock);
        press(scope, Key::NumLock);
        press(scope, Key::ScrollLock);
    });
}

fn hold_meta_keys(it: &mut Recording) {
    it.keys(|scope| {
        for held in META_KEYS {
            scope.key_down(held);
        }
    });
}

fn press_primary(it: &mut Recording) {
    it.mouse(|scope| {
        assert_eq!(scope.current_position(), Offset::ZERO);
        scope.press(MouseButton::Primary);
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, lockKeys_metaState_combinedCorrectly_inMousePress
#[test]
fn lock_keys_meta_state_combined_correctly_in_mouse_press() {
    let mut it = Recording::new();
    press_locks(&mut it);
    press_primary(&mut it);
    let heard = summaries(&it.heard());
    assert_eq!(heard.len(), 8);
    assert_eq!(heard[5], key(Key::ScrollLock, Up, NONE));
    assert_eq!(heard[6..], [pointer(Enter, ALL_LOCKS), pointer(Press, ALL_LOCKS)]);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, metaKeys_metaState_combinedCorrectly_inMousePress
#[test]
fn meta_keys_meta_state_combined_correctly_in_mouse_press() {
    let mut it = Recording::new();
    hold_meta_keys(&mut it);
    press_primary(&mut it);
    let heard = summaries(&it.heard());
    let all = Told { keys: ALL, ..Told::default() };
    assert_eq!(heard.len(), 11);
    assert_eq!(heard[8], key(Key::ShiftRight, Down, ALL));
    assert_eq!(heard[9..], [pointer(Enter, all), pointer(Press, all)]);
}

const ALL_MASKS: Told = Told { keys: ALL, ..ALL_LOCKS };

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, metaAndLockKeys_metaState_combinedCorrectly_inMousePress
#[test]
fn meta_and_lock_keys_meta_state_combined_correctly_in_mouse_press() {
    let mut it = Recording::new();
    press_locks(&mut it);
    hold_meta_keys(&mut it);
    press_primary(&mut it);
    let heard = summaries(&it.heard());
    assert_eq!(heard.len(), 17);
    assert_eq!(heard[14], key(Key::ShiftRight, Down, ALL));
    assert_eq!(heard[15..], [pointer(Enter, ALL_MASKS), pointer(Press, ALL_MASKS)]);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, scroll_vertically
#[test]
fn scroll_vertically() {
    scroll_test(ScrollWheel::Vertical, |units| Offset::new(0.0, 64.0 * units));
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyAndMouseEventsTest.kt, scroll_horizontally
#[test]
fn scroll_horizontally() {
    scroll_test(ScrollWheel::Horizontal, |units| Offset::new(64.0 * units, 0.0));
}

/// Every lock on and modifier held, `mouse_events.rs`'s scroll sequence: each event tells them.
fn scroll_test(wheel: ScrollWheel, scrolled: fn(f32) -> Offset) {
    let mut it = Recording::new();
    press_locks(&mut it);
    hold_meta_keys(&mut it);
    it.mouse(|scope| {
        assert_eq!(scope.current_position(), Offset::ZERO);
        scope.move_to_after(POSITION_1, 0);
        assert_eq!(scope.current_position(), POSITION_1);
        scope.advance_event_time(T);
        scope.scroll(1.0, wheel);
        scope.advance_event_time(T);
        scope.press(MouseButton::Primary);
        scope.advance_event_time(T);
        scope.scroll(2.0, wheel);
        scope.advance_event_time(T);
        scope.release(MouseButton::Primary);
        scope.advance_event_time(T);
        scope.scroll(3.0, wheel);
    });
    let heard = it.heard();
    assert_eq!(heard.len(), 15 + 10, "{heard:?}");
    // The keys' events aren't what's tested.
    let mouse_events = &heard[15..];
    assert!(mouse_events.iter().all(|event| event.modifiers().map(told) == Some(ALL_MASKS)));
    let t0 = mouse_events[0].millis();
    let without_keys = |heard: &Heard| match heard.clone() {
        Heard::Event { kind, millis, pointer_type, buttons, changes, scroll, .. } => Heard::Event {
            kind,
            millis: millis - t0,
            pointer_type,
            buttons,
            changes,
            scroll,
            modifiers: PointerKeyboardModifiers::default(),
        },
        other => panic!("not a pointer event: {other:?}"),
    };
    let mouse_events: Vec<Heard> = mouse_events.iter().map(without_keys).collect();
    let (p, none, primary) = (POSITION_1, PointerButtons::NONE, PointerButtons::PRIMARY);
    assert_eq!(
        mouse_events,
        [
            mouse(Enter, 0, p, none, false),
            mouse(Move, T, p, none, false),
            mouse_scroll(Scroll, T, p, none, false, scrolled(1.0)),
            mouse(Press, 2 * T, p, primary, true),
            mouse(Move, 3 * T, p, primary, true),
            mouse_scroll(Scroll, 3 * T, p, primary, true, scrolled(2.0)),
            mouse(Release, 4 * T, p, none, false),
            mouse(Move, 4 * T, p, none, false),
            mouse(Move, 5 * T, p, none, false),
            mouse_scroll(Scroll, 5 * T, p, none, false, scrolled(3.0)),
        ]
    );
}
