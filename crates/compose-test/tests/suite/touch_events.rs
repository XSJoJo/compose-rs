//! Compose's `inputdispatcher/TouchEventsTest`, through `TouchInjectionScope`: Compose's tests
//! read the `MotionEvent`s its internal `InputDispatcher` makes; these read what a node filling
//! the root hears of them (`dispatcher.rs`): each event's kind, time and every finger down with
//! its position. A cancel is heard as the gesture's cancel, without positions.

use compose_test::*;
use compose_ui::{Offset, PointerButtons, PointerEventType};

use super::dispatcher::*;
use PointerEventType::{Enter, Exit, Move, Press, Release};

const T: u64 = 16;

const POINTER_1: u64 = 11;
const POINTER_2: u64 = 22;
const POINTER_3: u64 = 33;
const POINTER_4: u64 = 44;

const POSITION_1: Offset = Offset::new(1.0, 1.0);
const POSITION_2: Offset = Offset::new(2.0, 2.0);
const POSITION_3: Offset = Offset::new(3.0, 3.0);
const POSITION_4: Offset = Offset::new(4.0, 4.0);

const POSITION_1_1: Offset = Offset::new(11.0, 11.0);
const POSITION_2_1: Offset = Offset::new(21.0, 21.0);
const POSITION_3_1: Offset = Offset::new(31.0, 31.0);

const POSITION_1_2: Offset = Offset::new(12.0, 12.0);
const POSITION_2_2: Offset = Offset::new(22.0, 22.0);

const POSITION_1_3: Offset = Offset::new(13.0, 13.0);

/// Finger `id` down at `position`.
fn down(id: u64, position: Offset) -> Change {
    (finger(id), position, true)
}

/// Finger `id` lifted at `position`.
fn up(id: u64, position: Offset) -> Change {
    (finger(id), position, false)
}

/// Runs `block` in one touch input on the root, returning what the root heard.
fn touches(block: impl FnOnce(&mut TouchInjectionScope<'_>)) -> Vec<Heard> {
    let (mut ui, log) = recording_root();
    ui.on_node_with_tag(ROOT).perform_touch_input(block);
    assert_has_valid_event_times(&log);
    heard(&log)
}

/// Puts finger `id` down and checks where it is, as `generateTouchDownAndCheck`.
#[track_caller]
fn down_and_check(scope: &mut TouchInjectionScope<'_>, id: u64, position: Offset) {
    scope.down_pointer(id, position);
    assert_eq!(scope.current_position(id), Some(position));
}

/// As `updateTouchPointerAndCheck`.
#[track_caller]
fn update_and_check(scope: &mut TouchInjectionScope<'_>, id: u64, position: Offset) {
    scope.update_pointer_to(id, position);
    assert_eq!(scope.current_position(id), Some(position));
}

/// Lifts finger `id` (after `delay` ms) and checks it's gone, as `generateTouchUpAndCheck`.
#[track_caller]
fn up_and_check(scope: &mut TouchInjectionScope<'_>, id: u64, delay: u64) {
    scope.advance_event_time(delay);
    scope.up_pointer(id);
    assert_eq!(scope.current_position(id), None);
}

/// Cancels (an event period on) and checks no finger is left, as
/// `generateCancelAndCheckPointers`.
#[track_caller]
fn cancel_and_check(scope: &mut TouchInjectionScope<'_>) {
    scope.cancel();
    assert_no_touch_gesture_in_progress(scope);
}

#[track_caller]
fn assert_no_touch_gesture_in_progress(scope: &TouchInjectionScope<'_>) {
    for id in [POINTER_1, POINTER_2, POINTER_3, POINTER_4] {
        assert_eq!(scope.current_position(id), None, "pointer {id}");
    }
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, onePointer_down
#[test]
fn one_pointer_down() {
    let heard = touches(|scope| down_and_check(scope, POINTER_1, POSITION_1));
    assert_eq!(heard, [touch(Press, 0, &[down(POINTER_1, POSITION_1)])]);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, onePointer_downUp
#[test]
fn one_pointer_down_up() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1);
        up_and_check(scope, POINTER_1, 0);
        assert_no_touch_gesture_in_progress(scope);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1)]),
            touch(Release, 0, &[up(POINTER_1, POSITION_1)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, onePointer_downDelayUp
#[test]
fn one_pointer_down_delay_up() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        up_and_check(scope, POINTER_1, 2 * T);
        assert_no_touch_gesture_in_progress(scope);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Release, 2 * T, &[up(POINTER_1, POSITION_1_1)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, onePointer_downUpdateMove
#[test]
fn one_pointer_down_update_move() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        update_and_check(scope, POINTER_1, POSITION_1_2);
        scope.move_pointers();
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Move, T, &[down(POINTER_1, POSITION_1_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, onePointer_downCancel
#[test]
fn one_pointer_down_cancel() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        cancel_and_check(scope);
    });
    assert_eq!(
        heard,
        [touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]), Heard::Cancel { millis: T }]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, twoPointers_downDownMoveMove
#[test]
fn two_pointers_down_down_move_move() {
    // 2 fingers, both go down before they move.
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        update_and_check(scope, POINTER_1, POSITION_1_2);
        scope.move_pointers();
        update_and_check(scope, POINTER_2, POSITION_2_2);
        scope.move_pointers();
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Move, T, &[down(POINTER_1, POSITION_1_2), down(POINTER_2, POSITION_2_1)]),
            touch(Move, 2 * T, &[down(POINTER_1, POSITION_1_2), down(POINTER_2, POSITION_2_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, twoPointers_downMoveDownMove
#[test]
fn two_pointers_down_move_down_move() {
    // 2 fingers, the 1st moves before the 2nd goes down and moves.
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        update_and_check(scope, POINTER_1, POSITION_1_2);
        scope.move_pointers();
        down_and_check(scope, POINTER_2, POSITION_2_1);
        update_and_check(scope, POINTER_2, POSITION_2_2);
        scope.move_pointers();
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Move, T, &[down(POINTER_1, POSITION_1_2)]),
            touch(Press, T, &[down(POINTER_1, POSITION_1_2), down(POINTER_2, POSITION_2_1)]),
            touch(Move, 2 * T, &[down(POINTER_1, POSITION_1_2), down(POINTER_2, POSITION_2_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, twoPointers_moveSimultaneously
#[test]
fn two_pointers_move_simultaneously() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        update_and_check(scope, POINTER_1, POSITION_1_2);
        update_and_check(scope, POINTER_2, POSITION_2_2);
        scope.move_pointers();
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Move, T, &[down(POINTER_1, POSITION_1_2), down(POINTER_2, POSITION_2_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, twoPointers_downUp_sameOrder
#[test]
fn two_pointers_down_up_same_order() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        up_and_check(scope, POINTER_2, 0);
        up_and_check(scope, POINTER_1, 0);
        assert_no_touch_gesture_in_progress(scope);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Release, 0, &[down(POINTER_1, POSITION_1_1), up(POINTER_2, POSITION_2_1)]),
            touch(Release, 0, &[up(POINTER_1, POSITION_1_1)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, twoPointers_downUp_inverseOrder
#[test]
fn two_pointers_down_up_inverse_order() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        up_and_check(scope, POINTER_1, 0);
        up_and_check(scope, POINTER_2, 0);
        assert_no_touch_gesture_in_progress(scope);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Release, 0, &[up(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Release, 0, &[up(POINTER_2, POSITION_2_1)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, twoPointers_downDownCancel
#[test]
fn two_pointers_down_down_cancel() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        cancel_and_check(scope);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            Heard::Cancel { millis: T },
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, threePointers_notSimultaneously
#[test]
fn three_pointers_not_simultaneously() {
    // 3 fingers, the 1st goes up before the 3rd goes down (no overlap).
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1);
        down_and_check(scope, POINTER_2, POSITION_2);
        scope.move_pointers();
        up_and_check(scope, POINTER_1, 0);
        scope.move_pointers();
        down_and_check(scope, POINTER_3, POSITION_3);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1), down(POINTER_2, POSITION_2)]),
            touch(Move, T, &[down(POINTER_1, POSITION_1), down(POINTER_2, POSITION_2)]),
            touch(Release, T, &[up(POINTER_1, POSITION_1), down(POINTER_2, POSITION_2)]),
            touch(Move, 2 * T, &[down(POINTER_2, POSITION_2)]),
            touch(Press, 2 * T, &[down(POINTER_2, POSITION_2), down(POINTER_3, POSITION_3)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, threePointers_pointerIdReuse
#[test]
fn three_pointers_pointer_id_reuse() {
    // 3 fingers, the 1st goes up before the 3rd goes down, the 3rd reusing the 1st's id.
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1);
        down_and_check(scope, POINTER_2, POSITION_2);
        scope.move_pointers();
        up_and_check(scope, POINTER_1, 0);
        scope.move_pointers();
        down_and_check(scope, POINTER_1, POSITION_1_2);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1), down(POINTER_2, POSITION_2)]),
            touch(Move, T, &[down(POINTER_1, POSITION_1), down(POINTER_2, POSITION_2)]),
            touch(Release, T, &[up(POINTER_1, POSITION_1), down(POINTER_2, POSITION_2)]),
            touch(Move, 2 * T, &[down(POINTER_2, POSITION_2)]),
            touch(Press, 2 * T, &[down(POINTER_1, POSITION_1_2), down(POINTER_2, POSITION_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, fourPointers_downOnly
#[test]
fn four_pointers_down_only() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_3, POSITION_3);
        down_and_check(scope, POINTER_1, POSITION_1);
        down_and_check(scope, POINTER_4, POSITION_4);
        down_and_check(scope, POINTER_2, POSITION_2);
    });
    let (one, two, three, four) = (
        down(POINTER_1, POSITION_1),
        down(POINTER_2, POSITION_2),
        down(POINTER_3, POSITION_3),
        down(POINTER_4, POSITION_4),
    );
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[three]),
            touch(Press, 0, &[one, three]),
            touch(Press, 0, &[one, three, four]),
            touch(Press, 0, &[one, two, three, four]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, fourPointers_downWithMove
#[test]
fn four_pointers_down_with_move() {
    // 4 fingers, going down at different times.
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_3, POSITION_3);
        scope.move_pointers();
        down_and_check(scope, POINTER_1, POSITION_1);
        down_and_check(scope, POINTER_2, POSITION_2);
        scope.move_pointers();
        scope.move_pointers();
        scope.move_pointers();
        down_and_check(scope, POINTER_4, POSITION_4);
    });
    let (one, two, three, four) = (
        down(POINTER_1, POSITION_1),
        down(POINTER_2, POSITION_2),
        down(POINTER_3, POSITION_3),
        down(POINTER_4, POSITION_4),
    );
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[three]),
            touch(Move, T, &[three]),
            touch(Press, T, &[one, three]),
            touch(Press, T, &[one, two, three]),
            touch(Move, 2 * T, &[one, two, three]),
            touch(Move, 3 * T, &[one, two, three]),
            touch(Move, 4 * T, &[one, two, three]),
            touch(Press, 4 * T, &[one, two, three, four]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_flushesPointerMovement
/// Moves updated and not sent yet are sent before a down.
#[test]
fn enqueue_touch_down_flushes_pointer_movement() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        update_and_check(scope, POINTER_1, POSITION_1_2);
        update_and_check(scope, POINTER_1, POSITION_1_3);
        scope.advance_event_time(T);
        down_and_check(scope, POINTER_3, POSITION_3_1);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Move, T, &[down(POINTER_1, POSITION_1_3), down(POINTER_2, POSITION_2_1)]),
            touch(
                Press,
                T,
                &[
                    down(POINTER_1, POSITION_1_3),
                    down(POINTER_2, POSITION_2_1),
                    down(POINTER_3, POSITION_3_1)
                ]
            ),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchUp_flushesPointerMovement
/// Moves updated and not sent yet go with an up, in its event (D2075).
#[test]
fn enqueue_touch_up_flushes_pointer_movement() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        update_and_check(scope, POINTER_1, POSITION_1_2);
        update_and_check(scope, POINTER_1, POSITION_1_3);
        scope.advance_event_time(T);
        up_and_check(scope, POINTER_1, 0);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Release, T, &[up(POINTER_1, POSITION_1_3), down(POINTER_2, POSITION_2_1)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchCancel_doesNotFlushPointerMovement
/// A cancel sends no move first (Compose's carries the latest positions; compose-rs's cancel
/// has none).
#[test]
fn enqueue_touch_cancel_does_not_flush_pointer_movement() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        update_and_check(scope, POINTER_1, POSITION_1_2);
        update_and_check(scope, POINTER_2, POSITION_2_2);
        cancel_and_check(scope);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            Heard::Cancel { millis: T },
        ]
    );
}

// ---- Errors ------------------------------------------------------------------------------------

/// What `block` panics with, in a touch input on a recording root.
fn touch_error(block: impl FnOnce(&mut TouchInjectionScope<'_>)) -> String {
    let (mut ui, _) = recording_root();
    super::input::expect_error(|| {
        ui.on_node_with_tag(ROOT).perform_touch_input(block);
    })
}

#[track_caller]
fn assert_says(message: &str, expected: &str) {
    assert!(message.contains(expected), "expected {expected:?}, got {message:?}");
}

const NO_GESTURE_TO_MOVE: &str = "Cannot move pointers, no gesture is in progress";
const NO_GESTURE_TO_MOVE_EVENT: &str = "Cannot send MOVE event, no gesture is in progress";
const NO_GESTURE_TO_LIFT: &str = "Cannot send UP event, no gesture is in progress";
const NO_GESTURE_TO_CANCEL: &str = "Cannot send CANCEL event, no gesture is in progress";

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_afterDown
#[test]
fn enqueue_touch_down_after_down() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1);
        scope.down_pointer(POINTER_1, POSITION_2);
    });
    assert_says(
        &message,
        "Cannot send DOWN event, a gesture is already in progress for pointer 11",
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, updateTouchPointer_withoutDown
#[test]
fn update_touch_pointer_without_down() {
    let message = touch_error(|scope| scope.update_pointer_to(POINTER_1, POSITION_1_1));
    assert_says(&message, NO_GESTURE_TO_MOVE);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, updateTouchPointer_wrongPointerId
#[test]
fn update_touch_pointer_wrong_pointer_id() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.update_pointer_to(POINTER_2, POSITION_1_2);
    });
    assert_says(&message, "Cannot move pointer 22, it is not active in the current gesture");
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, updateTouchPointer_afterUp
#[test]
fn update_touch_pointer_after_up() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.up_pointer(POINTER_1);
        scope.update_pointer_to(POINTER_1, POSITION_1_2);
    });
    assert_says(&message, NO_GESTURE_TO_MOVE);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, updateTouchPointer_afterCancel
#[test]
fn update_touch_pointer_after_cancel() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.cancel();
        scope.update_pointer_to(POINTER_1, POSITION_1_2);
    });
    assert_says(&message, NO_GESTURE_TO_MOVE);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchMove_withoutDown
#[test]
fn enqueue_touch_move_without_down() {
    let message = touch_error(|scope| scope.move_pointers());
    assert_says(&message, NO_GESTURE_TO_MOVE_EVENT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchMove_afterUp
#[test]
fn enqueue_touch_move_after_up() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.up_pointer(POINTER_1);
        scope.move_pointers();
    });
    assert_says(&message, NO_GESTURE_TO_MOVE_EVENT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchMove_afterCancel
#[test]
fn enqueue_touch_move_after_cancel() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.cancel();
        scope.move_pointers();
    });
    assert_says(&message, NO_GESTURE_TO_MOVE_EVENT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchUp_withoutDown
#[test]
fn enqueue_touch_up_without_down() {
    let message = touch_error(|scope| scope.up_pointer(POINTER_1));
    assert_says(&message, NO_GESTURE_TO_LIFT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchUp_wrongPointerId
#[test]
fn enqueue_touch_up_wrong_pointer_id() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.up_pointer(POINTER_2);
    });
    assert_says(
        &message,
        "Cannot send UP event for pointer 22, it is not active in the current gesture",
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchUp_afterUp
#[test]
fn enqueue_touch_up_after_up() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.up_pointer(POINTER_1);
        scope.up_pointer(POINTER_1);
    });
    assert_says(&message, NO_GESTURE_TO_LIFT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchUp_afterCancel
#[test]
fn enqueue_touch_up_after_cancel() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.cancel();
        scope.up_pointer(POINTER_1);
    });
    assert_says(&message, NO_GESTURE_TO_LIFT);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchCancel_withoutDown
#[test]
fn enqueue_touch_cancel_without_down() {
    let message = touch_error(|scope| scope.cancel());
    assert_says(&message, NO_GESTURE_TO_CANCEL);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchCancel_afterUp
#[test]
fn enqueue_touch_cancel_after_up() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.up_pointer(POINTER_1);
        scope.cancel();
    });
    assert_says(&message, NO_GESTURE_TO_CANCEL);
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchCancel_afterCancel
#[test]
fn enqueue_touch_cancel_after_cancel() {
    let message = touch_error(|scope| {
        scope.down_pointer(POINTER_1, POSITION_1_1);
        scope.cancel();
        scope.cancel();
    });
    assert_says(&message, NO_GESTURE_TO_CANCEL);
}

// ---- A touch down and the cursor -----------------------------------------------------------

/// The root's events: what `cursor` sends, then a finger down at `POSITION_2` an event period
/// on.
fn cursor_then_touch(cursor: impl FnOnce(&mut ComposeTestRule)) -> Vec<Heard> {
    let (mut ui, log) = recording_root();
    cursor(&mut ui);
    ui.on_node_with_tag(ROOT).perform_touch_input(|scope| {
        scope.advance_event_time(T);
        down_and_check(scope, POINTER_1, POSITION_2);
    });
    assert_has_valid_event_times(&log);
    heard(&log)
}

const NONE: PointerButtons = PointerButtons::NONE;
const PRIMARY: PointerButtons = PointerButtons::PRIMARY;

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_exitsMouse
/// A hovering mouse exits before the finger comes down.
#[test]
fn enqueue_touch_down_exits_mouse() {
    let heard = cursor_then_touch(|ui| {
        ui.on_node_with_tag(ROOT).perform_mouse_input(|mouse| mouse.enter(POSITION_1));
    });
    let exit = heard[1].clone();
    assert_eq!(heard.len(), 3, "{heard:?}");
    assert_eq!(heard[0], mouse(Enter, 0, POSITION_1, NONE, false));
    assert_eq!((exit.kind(), exit.millis()), (Some(Exit), T));
    assert_eq!(heard[2], touch(Press, T, &[down(POINTER_1, POSITION_2)]));
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_cancelsMouse
/// A mouse's press is cancelled before the finger comes down. The press comes over the root
/// with an `Enter` first, where Compose's root hears the press alone (as `trackpad.rs` records).
#[test]
fn enqueue_touch_down_cancels_mouse() {
    let heard = cursor_then_touch(|ui| {
        ui.on_node_with_tag(ROOT).perform_mouse_input(|mouse| mouse.press(MouseButton::Primary));
    });
    assert_eq!(
        heard,
        [
            mouse(Enter, 0, Offset::ZERO, PRIMARY, false),
            mouse(Press, 0, Offset::ZERO, PRIMARY, true),
            Heard::Cancel { millis: T },
            touch(Press, T, &[down(POINTER_1, POSITION_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_cancelsMouseAfterMove
#[test]
fn enqueue_touch_down_cancels_mouse_after_move() {
    let heard = cursor_then_touch(|ui| {
        ui.on_node_with_tag(ROOT).perform_mouse_input(|mouse| {
            mouse.move_to_after(POSITION_3, 0);
            mouse.press(MouseButton::Primary);
        });
    });
    assert_eq!(
        heard,
        [
            mouse(Enter, 0, POSITION_3, NONE, false),
            mouse(Press, 0, POSITION_3, PRIMARY, true),
            Heard::Cancel { millis: T },
            touch(Press, T, &[down(POINTER_1, POSITION_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_exitsTrackpad
/// A hovering trackpad's cursor exits before the finger comes down.
#[test]
fn enqueue_touch_down_exits_trackpad() {
    let heard = cursor_then_touch(|ui| {
        ui.on_node_with_tag(ROOT).perform_trackpad_input(|trackpad| trackpad.enter(POSITION_1));
    });
    let exit = heard[1].clone();
    assert_eq!(heard.len(), 3, "{heard:?}");
    assert_eq!(heard[0], mouse(Enter, 0, POSITION_1, NONE, false));
    assert_eq!((exit.kind(), exit.millis()), (Some(Exit), T));
    assert_eq!(heard[2], touch(Press, T, &[down(POINTER_1, POSITION_2)]));
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_cancelsTrackpad
/// As the mouse's (its `Enter` too).
#[test]
fn enqueue_touch_down_cancels_trackpad() {
    let heard = cursor_then_touch(|ui| {
        ui.on_node_with_tag(ROOT)
            .perform_trackpad_input(|trackpad| trackpad.press(TrackpadButton::Primary));
    });
    assert_eq!(
        heard,
        [
            mouse(Enter, 0, Offset::ZERO, PRIMARY, false),
            mouse(Press, 0, Offset::ZERO, PRIMARY, true),
            Heard::Cancel { millis: T },
            touch(Press, T, &[down(POINTER_1, POSITION_2)]),
        ]
    );
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/TouchEventsTest.kt, enqueueTouchDown_cancelsTrackpadAfterMove
#[test]
fn enqueue_touch_down_cancels_trackpad_after_move() {
    let heard = cursor_then_touch(|ui| {
        ui.on_node_with_tag(ROOT).perform_trackpad_input(|trackpad| {
            trackpad.move_to_after(POSITION_3, 0);
            trackpad.press(TrackpadButton::Primary);
        });
    });
    assert_eq!(
        heard,
        [
            mouse(Enter, 0, POSITION_3, NONE, false),
            mouse(Press, 0, POSITION_3, PRIMARY, true),
            Heard::Cancel { millis: T },
            touch(Press, T, &[down(POINTER_1, POSITION_2)]),
        ]
    );
}

/// The other fingers' updates go in the up's event too, which moves them (D2075): no move
/// event before it, nor after.
#[test]
fn an_up_carries_the_other_fingers_updates() {
    let heard = touches(|scope| {
        down_and_check(scope, POINTER_1, POSITION_1_1);
        down_and_check(scope, POINTER_2, POSITION_2_1);
        update_and_check(scope, POINTER_2, POSITION_2_2);
        scope.advance_event_time(T);
        up_and_check(scope, POINTER_1, 0);
        up_and_check(scope, POINTER_2, 0);
    });
    assert_eq!(
        heard,
        [
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1)]),
            touch(Press, 0, &[down(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_1)]),
            touch(Release, T, &[up(POINTER_1, POSITION_1_1), down(POINTER_2, POSITION_2_2)]),
            touch(Release, T, &[up(POINTER_2, POSITION_2_2)]),
        ]
    );
}
