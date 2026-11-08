//! Compose's `inputdispatcher/KeyEventsTest`, through `KeyInjectionScope`: Compose's tests
//! drive its internal `InputDispatcher` and read Android's `KeyEvent`s; these send the keys to
//! a focused box and read what it hears. compose-rs's `KeyEvent` is Compose's common one: the
//! key, down or up, and the four modifiers held (no native key code, no meta mask per side or
//! for Fn and the locks, no repeat count or down time), so the meta states are checked through
//! the modifiers and the scope's lock states, the repeats through the downs and their times.

use compose_test::*;
use compose_ui::{Key, KeyEventType, KeyModifiers};

use super::input::{Keys, expect_error, recording_keys};

const INITIAL_REPEAT_DELAY: u64 = 500;
const SUBSEQUENT_REPEAT_DELAY: u64 = 50;

const ONE: Key = Key::Character('1');
const A: Key = Key::Character('a');
/// Android's `KEYCODE_FUNCTION`: compose-rs has no `Key.Function`, nor a modifier for it.
const FUNCTION: Key = Key::Other(119);

use KeyEventType::{KeyDown as Down, KeyUp as Up};

/// What the box heard: the key, down or up, its modifiers and its time in ms since the first.
fn heard(keys: &Keys) -> Vec<(Key, KeyEventType, KeyModifiers, u64)> {
    let recorded = keys.borrow();
    let t0 = recorded.first().map_or(0, |(_, millis)| *millis);
    recorded
        .iter()
        .map(|(event, millis)| (event.key, event.kind, event.modifiers, millis - t0))
        .collect()
}

/// The events' times never go back, as `assertHasValidEventTimes`.
#[track_caller]
fn assert_has_valid_event_times(keys: &Keys) {
    let times: Vec<u64> = keys.borrow().iter().map(|(_, millis)| *millis).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "times go back: {times:?}");
}

/// Runs `block` in one key input on the focused box, returning what it heard.
fn keys(block: impl FnOnce(&mut KeyInjectionScope<'_>, &Keys)) -> Keys {
    let (mut ui, keys) = recording_keys();
    let seen = keys.clone();
    ui.on_node_with_tag("keys").perform_key_input(|scope| block(scope, &seen));
    keys
}

fn press(scope: &mut KeyInjectionScope<'_>, key: Key) {
    scope.key_down(key);
    scope.key_up(key);
}

const NONE: KeyModifiers = KeyModifiers { shift: false, ctrl: false, alt: false, meta: false };

fn event(
    key: Key,
    kind: KeyEventType,
    modifiers: KeyModifiers,
    millis: u64,
) -> (Key, KeyEventType, KeyModifiers, u64) {
    (key, kind, modifiers, millis)
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, keyStartsUp
#[test]
fn key_starts_up() {
    keys(|scope, _| assert!(!scope.is_key_down(ONE)));
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, keyIsDownAfterEnqueueDown
#[test]
fn key_is_down_after_enqueue_down() {
    keys(|scope, _| {
        scope.key_down(A);
        assert!(scope.is_key_down(A));
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, keyIsUpAfterEnqueueUp
#[test]
fn key_is_up_after_enqueue_up() {
    keys(|scope, _| {
        scope.key_down(Key::Backspace);
        scope.key_up(Key::Backspace);
        assert!(!scope.is_key_down(Key::Backspace));
    });
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, enqueueKeyDown_enqueuesKeyEvent
#[test]
fn enqueue_key_down_enqueues_key_event() {
    let keys = keys(|scope, _| scope.key_down(Key::Enter));
    assert_eq!(heard(&keys), [event(Key::Enter, Down, NONE, 0)]);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, enqueueKeyUp_enqueuesKeyEvent
#[test]
fn enqueue_key_up_enqueues_key_event() {
    let keys = keys(|scope, _| press(scope, ONE));
    let heard = heard(&keys);
    assert_eq!(heard.len(), 2);
    assert_eq!(heard[1], event(ONE, Up, NONE, 0));
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, capsLock_andShift_produceCorrectMetaState
/// Caps Lock on and Shift held: the A's down has Shift (the lock is the scope's, not the
/// event's).
#[test]
fn caps_lock_and_shift_produce_correct_meta_state() {
    let keys = keys(|scope, _| {
        press(scope, Key::CapsLock);
        scope.key_down(Key::ShiftLeft);
        scope.key_down(A);
        assert!(scope.is_caps_lock_on());
    });
    let last = *heard(&keys).last().unwrap();
    assert_eq!(last, event(A, Down, KeyModifiers { shift: true, ..NONE }, 0));
}

// ---- Lock key states ---------------------------------------------------------------------------

/// The lock state as `is_on` reads it: off at first, on after a down and after a press, off
/// after two presses.
fn check_lock(key: Key, is_on: fn(&KeyInjectionScope<'_>) -> bool, case: &str) {
    keys(|scope, _| match case {
        "startsOff" => assert!(!is_on(scope)),
        "afterDown" => {
            scope.key_down(key);
            assert!(is_on(scope));
        }
        "afterPress" => {
            press(scope, key);
            assert!(is_on(scope));
        }
        "afterDoublePress" => {
            press(scope, key);
            press(scope, key);
            assert!(!is_on(scope));
        }
        _ => unreachable!(),
    });
}

fn caps(scope: &KeyInjectionScope<'_>) -> bool {
    scope.is_caps_lock_on()
}

fn num(scope: &KeyInjectionScope<'_>) -> bool {
    scope.is_num_lock_on()
}

fn scroll(scope: &KeyInjectionScope<'_>) -> bool {
    scope.is_scroll_lock_on()
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, capsLock_startsOff
#[test]
fn caps_lock_starts_off() {
    check_lock(Key::CapsLock, caps, "startsOff");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, capsLockOn_afterDown
#[test]
fn caps_lock_on_after_down() {
    check_lock(Key::CapsLock, caps, "afterDown");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, capsLockOn_afterPress
#[test]
fn caps_lock_on_after_press() {
    check_lock(Key::CapsLock, caps, "afterPress");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, capsLockOff_afterDoublePress
#[test]
fn caps_lock_off_after_double_press() {
    check_lock(Key::CapsLock, caps, "afterDoublePress");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, numLock_startsOff
#[test]
fn num_lock_starts_off() {
    check_lock(Key::NumLock, num, "startsOff");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, numLockOn_afterDown
#[test]
fn num_lock_on_after_down() {
    check_lock(Key::NumLock, num, "afterDown");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, numLockOn_afterPress
#[test]
fn num_lock_on_after_press() {
    check_lock(Key::NumLock, num, "afterPress");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, numLockOff_afterDoublePress
#[test]
fn num_lock_off_after_double_press() {
    check_lock(Key::NumLock, num, "afterDoublePress");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, scrollLock_startsOff
#[test]
fn scroll_lock_starts_off() {
    check_lock(Key::ScrollLock, scroll, "startsOff");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, scrollLockOn_afterDown
#[test]
fn scroll_lock_on_after_down() {
    check_lock(Key::ScrollLock, scroll, "afterDown");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, scrollLockOn_afterPress
#[test]
fn scroll_lock_on_after_press() {
    check_lock(Key::ScrollLock, scroll, "afterPress");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, scrollLockOff_afterDoublePress
#[test]
fn scroll_lock_off_after_double_press() {
    check_lock(Key::ScrollLock, scroll, "afterDoublePress");
}

// ---- Meta states -------------------------------------------------------------------------------

/// A modifier key's press: its down has its modifier, its up none, as `verifyMetaKeyMetaState`.
#[track_caller]
fn verify_meta_key_meta_state(key: Key, expected: KeyModifiers) {
    let keys = keys(|scope, _| press(scope, key));
    assert_has_valid_event_times(&keys);
    assert_eq!(heard(&keys), [event(key, Down, expected, 0), event(key, Up, NONE, 0)]);
}

/// A lock key pressed twice: on through its first press and its second down, off at its second
/// up, as `verifyLockKeyMetaState` (the lock states are the scope's; the events carry none).
#[track_caller]
fn verify_lock_key_meta_state(key: Key, is_on: fn(&KeyInjectionScope<'_>) -> bool) {
    let keys = keys(|scope, _| {
        let mut states = Vec::new();
        for step in 0..4 {
            if step % 2 == 0 {
                scope.key_down(key);
            } else {
                scope.key_up(key);
            }
            states.push(is_on(scope));
        }
        assert_eq!(states, [true, true, true, false]);
    });
    assert_has_valid_event_times(&keys);
    let kinds: Vec<KeyEventType> = heard(&keys).iter().map(|event| event.1).collect();
    assert_eq!(kinds, [Down, Up, Down, Up]);
}

const SHIFT: KeyModifiers = KeyModifiers { shift: true, ..NONE };
const CTRL: KeyModifiers = KeyModifiers { ctrl: true, ..NONE };
const ALT: KeyModifiers = KeyModifiers { alt: true, ..NONE };
const META: KeyModifiers = KeyModifiers { meta: true, ..NONE };

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, functionKey_metaState_generatedCorrectly
/// compose-rs's modifiers have no Fn: its press holds none of the four.
#[test]
fn function_key_meta_state_generated_correctly() {
    verify_meta_key_meta_state(FUNCTION, NONE);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, leftCtrl_metaState_generatedCorrectly
#[test]
fn left_ctrl_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::CtrlLeft, CTRL);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, rightCtrl_metaState_generatedCorrectly
#[test]
fn right_ctrl_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::CtrlRight, CTRL);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, leftAlt_metaState_generatedCorrectly
#[test]
fn left_alt_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::AltLeft, ALT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, rightAlt_metaState_generatedCorrectly
#[test]
fn right_alt_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::AltRight, ALT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, leftMeta_metaState_generatedCorrectly
#[test]
fn left_meta_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::MetaLeft, META);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, rightMeta_metaState_generatedCorrectly
#[test]
fn right_meta_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::MetaRight, META);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, leftShift_metaState_generatedCorrectly
#[test]
fn left_shift_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::ShiftLeft, SHIFT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, rightShift_metaState_generatedCorrectly
#[test]
fn right_shift_meta_state_generated_correctly() {
    verify_meta_key_meta_state(Key::ShiftRight, SHIFT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, capsLock_metaState_generatedCorrectly
#[test]
fn caps_lock_meta_state_generated_correctly() {
    verify_lock_key_meta_state(Key::CapsLock, caps);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, numLock_metaState_generatedCorrectly
#[test]
fn num_lock_meta_state_generated_correctly() {
    verify_lock_key_meta_state(Key::NumLock, num);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, scrollLock_metaState_generatedCorrectly
#[test]
fn scroll_lock_meta_state_generated_correctly() {
    verify_lock_key_meta_state(Key::ScrollLock, scroll);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, lockKeys_metaState_combinedCorrectly
#[test]
fn lock_keys_meta_state_combined_correctly() {
    let keys = keys(|scope, _| {
        press(scope, Key::CapsLock);
        press(scope, Key::NumLock);
        press(scope, Key::ScrollLock);
        assert!(scope.is_caps_lock_on() && scope.is_num_lock_on() && scope.is_scroll_lock_on());
    });
    assert_has_valid_event_times(&keys);
    let heard = heard(&keys);
    assert_eq!(heard.len(), 6);
    assert_eq!(heard[5], event(Key::ScrollLock, Up, NONE, 0));
}

const ALL_META: KeyModifiers = KeyModifiers { shift: true, ctrl: true, alt: true, meta: true };

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

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, metaKeys_metaState_combinedCorrectly
#[test]
fn meta_keys_meta_state_combined_correctly() {
    let keys = keys(|scope, _| {
        for key in META_KEYS {
            scope.key_down(key);
        }
        assert!(scope.is_shift_down() && scope.is_ctrl_down());
        assert!(scope.is_alt_down() && scope.is_meta_down());
    });
    assert_has_valid_event_times(&keys);
    let heard = heard(&keys);
    assert_eq!(heard.len(), 9);
    assert_eq!(heard[8], event(Key::ShiftRight, Down, ALL_META, 0));
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, metaAndLockKeys_metaState_combinedCorrectly
#[test]
fn meta_and_lock_keys_meta_state_combined_correctly() {
    let keys = keys(|scope, _| {
        press(scope, Key::CapsLock);
        press(scope, Key::NumLock);
        press(scope, Key::ScrollLock);
        for key in META_KEYS {
            scope.key_down(key);
        }
        assert!(scope.is_caps_lock_on() && scope.is_num_lock_on() && scope.is_scroll_lock_on());
    });
    assert_has_valid_event_times(&keys);
    let heard = heard(&keys);
    assert_eq!(heard.len(), 15);
    assert_eq!(heard[14], event(Key::ShiftRight, Down, ALL_META, 0));
}

// ---- Repeats -----------------------------------------------------------------------------------

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, noRepeat_before_repeatDelay
#[test]
fn no_repeat_before_repeat_delay() {
    let keys = keys(|scope, _| {
        scope.key_down(A);
        scope.advance_event_time(INITIAL_REPEAT_DELAY - 1);
        scope.key_up(A);
    });
    assert_has_valid_event_times(&keys);
    assert_eq!(
        heard(&keys),
        [event(A, Down, NONE, 0), event(A, Up, NONE, INITIAL_REPEAT_DELAY - 1)]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, keyDownRepeats_exactlyAt_repeatDelay
#[test]
fn key_down_repeats_exactly_at_repeat_delay() {
    let keys = keys(|scope, _| {
        scope.key_down(A);
        scope.advance_event_time(INITIAL_REPEAT_DELAY);
        scope.key_up(A);
    });
    assert_has_valid_event_times(&keys);
    assert_eq!(
        heard(&keys),
        [
            event(A, Down, NONE, 0),
            event(A, Down, NONE, INITIAL_REPEAT_DELAY),
            event(A, Up, NONE, INITIAL_REPEAT_DELAY),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, repeat_cancelledBy_newKeyDown
#[test]
fn repeat_cancelled_by_new_key_down() {
    let keys = keys(|scope, _| {
        scope.key_down(A);
        scope.advance_event_time(INITIAL_REPEAT_DELAY - 1);
        scope.key_down(ONE);
        scope.advance_event_time(INITIAL_REPEAT_DELAY - 1);
    });
    assert_has_valid_event_times(&keys);
    assert_eq!(
        heard(&keys),
        [event(A, Down, NONE, 0), event(ONE, Down, NONE, INITIAL_REPEAT_DELAY - 1)]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, secondKeyDown_isRepeated
#[test]
fn second_key_down_is_repeated() {
    let keys = keys(|scope, _| {
        scope.key_down(A);
        scope.advance_event_time(INITIAL_REPEAT_DELAY - 1);
        scope.key_down(ONE);
        scope.advance_event_time(INITIAL_REPEAT_DELAY);
    });
    assert_has_valid_event_times(&keys);
    assert_eq!(
        heard(&keys),
        [
            event(A, Down, NONE, 0),
            event(ONE, Down, NONE, INITIAL_REPEAT_DELAY - 1),
            event(ONE, Down, NONE, 2 * INITIAL_REPEAT_DELAY - 1),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, firstKeyDown_notRepeated_afterLatestKeyUp
#[test]
fn first_key_down_not_repeated_after_latest_key_up() {
    let keys = keys(|scope, _| {
        scope.key_down(A);
        scope.advance_event_time(INITIAL_REPEAT_DELAY - 1);
        scope.key_down(ONE);
        scope.advance_event_time(INITIAL_REPEAT_DELAY);
        scope.key_up(ONE);
        // The A key doesn't repeat.
        scope.advance_event_time(INITIAL_REPEAT_DELAY * 3);
    });
    assert_has_valid_event_times(&keys);
    assert_eq!(
        heard(&keys),
        [
            event(A, Down, NONE, 0),
            event(ONE, Down, NONE, INITIAL_REPEAT_DELAY - 1),
            event(ONE, Down, NONE, 2 * INITIAL_REPEAT_DELAY - 1),
            event(ONE, Up, NONE, 2 * INITIAL_REPEAT_DELAY - 1),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, multipleRepeatKeyTest
/// The repeats come as the event time passes them: Compose's flushes are the reads here.
#[test]
fn multiple_repeat_key_test() {
    keys(|scope, keys| {
        scope.key_down(A); // t = 0
        scope.advance_event_time(INITIAL_REPEAT_DELAY - 1); // t = 499
        assert_eq!(heard(keys), [event(A, Down, NONE, 0)]);

        scope.advance_event_time(1); // t = 500
        assert_eq!(heard(keys).len(), 2);
        assert_eq!(heard(keys)[1], event(A, Down, NONE, INITIAL_REPEAT_DELAY));

        scope.advance_event_time(SUBSEQUENT_REPEAT_DELAY - 1); // t = 549
        assert_eq!(heard(keys).len(), 2);
        scope.advance_event_time(1); // t = 550
        assert_eq!(heard(keys).len(), 3);
        assert_eq!(
            heard(keys)[2],
            event(A, Down, NONE, INITIAL_REPEAT_DELAY + SUBSEQUENT_REPEAT_DELAY)
        );

        scope.advance_event_time(SUBSEQUENT_REPEAT_DELAY * 10); // t = 1050
        assert_has_valid_event_times(keys);
        let heard = heard(keys);
        assert_eq!(heard.len(), 13);
        for (i, event) in heard[3..].iter().enumerate() {
            let at = INITIAL_REPEAT_DELAY + SUBSEQUENT_REPEAT_DELAY * (i as u64 + 2);
            assert_eq!(*event, (A, Down, NONE, at), "repeat {}", i + 3);
        }
    });
}

// ---- Errors ------------------------------------------------------------------------------------

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, enqueueKeyDown_alreadyDown
#[test]
fn enqueue_key_down_already_down() {
    let (mut ui, _) = recording_keys();
    let message = expect_error(|| {
        ui.on_node_with_tag("keys").perform_key_input(|scope| {
            scope.key_down(ONE);
            scope.key_down(ONE);
        });
    });
    assert!(
        message
            .contains("Cannot send key down event, Key(Character('1')) is already pressed down."),
        "{message}"
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/KeyEventsTest.kt, enqueueKeyUp_withoutDown
#[test]
fn enqueue_key_up_without_down() {
    let (mut ui, _) = recording_keys();
    let message = expect_error(|| {
        ui.on_node_with_tag("keys").perform_key_input(|scope| scope.key_up(A));
    });
    assert!(
        message.contains("Cannot send key up event, Key(Character('a')) is not pressed down."),
        "{message}"
    );
}
