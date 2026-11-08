//! Compose's single-test swipe files of `injectionscope/touch`: `SwipeStartEndTest`,
//! `SwipeCurveTest`, `SwipeCurveWithKeyTimesTest` (each over its parameters), `SwipeMultiTouchTest`
//! `SwipeWithTouchSlopTest` and `MultiTouchSwipeWithVelocityTest`, read through `dispatcher.rs`'s recorder (a node filling the
//! window, as Compose's box at the window's top left).

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{
    IntSize, Modifier, Offset, PointerEventType, PointerId, UnitExt, ViewConfiguration,
};

use super::dispatcher::*;

/// The recorded events of one pointer: (time from the first, position, pressed, kind).
fn single_pointer(heard: &[Heard]) -> Vec<(u64, Offset, bool, PointerEventType)> {
    let id = match &heard[0] {
        Heard::Event { changes, .. } => changes[0].0,
        other => panic!("not an event: {other:?}"),
    };
    let t0 = heard[0].millis();
    heard
        .iter()
        .map(|heard| match heard {
            Heard::Event { kind, millis, changes, .. } => {
                assert_eq!(changes.len(), 1, "a single pointer: {heard:?}");
                assert_eq!(changes[0].0, id, "the same pointer");
                (millis - t0, changes[0].1, changes[0].2, *kind)
            }
            other => panic!("not an event: {other:?}"),
        })
        .collect()
}

type Events = Vec<(u64, Offset, bool, PointerEventType)>;

/// As Compose's `assertOnlyLastEventIsUp` and `assertUpSameAsLastMove`; returns the downs.
#[track_caller]
fn downs_and_up(events: &Events) -> Events {
    assert!(events.len() >= 3, "{events:?}");
    let (up, downs) = events.split_last().unwrap();
    assert!(downs.iter().all(|event| event.2), "only the last is up: {events:?}");
    assert!(!up.2 && up.3 == PointerEventType::Release, "{up:?}");
    let last_move = downs.last().unwrap();
    assert_eq!((up.0, up.1), (last_move.0, last_move.1), "the up is where the last move is");
    downs.to_vec()
}

/// As `hasSameTimeBetweenEvents`: the times between them differ by a millisecond at most.
#[track_caller]
fn has_same_time_between_events(events: &[(u64, Offset, bool, PointerEventType)]) {
    let mut gaps: Vec<u64> = events.windows(2).map(|pair| pair[1].0 - pair[0].0).collect();
    gaps.sort_unstable();
    if let (Some(first), Some(last)) = (gaps.first(), gaps.last()) {
        assert!(last - first <= 1, "gaps {gaps:?}");
    }
}

/// As `areSampledFromCurve`.
#[track_caller]
fn are_sampled_from_curve(
    events: &[(u64, Offset, bool, PointerEventType)],
    curve: fn(u64) -> Offset,
) {
    for (time, position, _, _) in events {
        assert_eq!(*position, curve(*time), "at {time}");
    }
}

fn swiped(block: impl FnOnce(&mut TouchInjectionScope<'_>)) -> Events {
    let (mut ui, log) = recording_root();
    ui.on_node_with_tag(ROOT).perform_touch_input(block);
    single_pointer(&heard(&log))
}

const DURATIONS: [u64; 10] = [1, 12, 23, 34, 45, 56, 67, 78, 89, 100];

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/touch/SwipeStartEndTest.kt, swipe
/// Over Compose's durations (1 to 100 ms, by 11): from start to end in the duration, evenly in
/// time and distance.
#[test]
fn swipe_start_end() {
    let (start, end) = (Offset::new(5.0, 7.0), Offset::new(23.0, 29.0));
    for duration in DURATIONS {
        let events = swiped(|scope| scope.swipe(start, end, duration));
        let downs = downs_and_up(&events);
        assert_eq!((events[0].1, events[0].3), (start, PointerEventType::Press));
        let last = events.last().unwrap();
        assert_eq!((last.0, last.1), (duration, end), "duration {duration}");
        has_same_time_between_events(&downs);
        let mut speeds: Vec<f32> = downs
            .windows(2)
            .map(|pair| (pair[1].1 - pair[0].1).get_distance() / (pair[1].0 - pair[0].0) as f32)
            .collect();
        speeds.sort_by(f32::total_cmp);
        assert!(speeds.last().unwrap() - speeds.first().unwrap() <= 1e-3, "{speeds:?}");
    }
}

fn curve(time: u64) -> Offset {
    Offset::new(time as f32 + 10.0, time as f32 + 10.0)
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/touch/SwipeCurveTest.kt, swipe
#[test]
fn swipe_curve() {
    for duration in DURATIONS {
        let events = swiped(|scope| scope.swipe_along(&curve, duration, &[]));
        let downs = downs_and_up(&events);
        assert_eq!(events.last().unwrap().0, duration, "the gesture's duration");
        has_same_time_between_events(&downs);
        are_sampled_from_curve(&downs, curve);
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/touch/SwipeCurveWithKeyTimesTest.kt, swipe
#[test]
fn swipe_curve_with_key_times() {
    const DURATION: u64 = 100;
    let cases: [&[u64]; 10] = [
        &[],
        &[0],
        &[1],
        &[50],
        &[51],
        &[DURATION - 1],
        &[DURATION],
        &[33, 66],
        &[45, 46, 47],
        &[45, 55, 65],
    ];
    for key_times in cases {
        let events = swiped(|scope| scope.swipe_along(&curve, DURATION, key_times));
        let downs = downs_and_up(&events);
        let times: Vec<u64> = events.iter().map(|event| event.0).collect();
        assert!(key_times.iter().all(|time| times.contains(time)), "{key_times:?}: {times:?}");
        assert_eq!(events.last().unwrap().0, DURATION);
        are_sampled_from_curve(&downs, curve);
        let mut bounds = vec![0];
        bounds.extend_from_slice(key_times);
        bounds.push(DURATION);
        bounds.dedup();
        for pair in bounds.windows(2) {
            let between: Vec<_> = downs
                .iter()
                .copied()
                .filter(|event| (pair[0]..=pair[1]).contains(&event.0))
                .collect();
            has_same_time_between_events(&between);
        }
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/touch/SwipeMultiTouchTest.kt, test
/// Three fingers left to right together over 64 ms, sampled every 16 ms: three downs, four
/// moves, three ups.
#[test]
fn swipe_multi_touch() {
    const DURATION: u64 = 64;
    let line =
        |y: f32| move |time: u64| Offset::new(10.0 + 80.0 * time as f32 / DURATION as f32, y);
    let (curve1, curve2, curve3) = (line(20.0), line(50.0), line(80.0));
    let (mut ui, log) = recording_root();
    ui.on_node_with_tag(ROOT).perform_touch_input(|scope| {
        scope.multi_touch_swipe(&[&curve1, &curve2, &curve3], DURATION, &[]);
    });
    assert_has_valid_event_times(&log);
    let heard = heard(&log);
    let finger_at = |id: u64, x: f32, y: f32, down: bool| -> (PointerId, Offset, bool) {
        (finger(id), Offset::new(x, y), down)
    };
    use PointerEventType::{Move, Press, Release};
    let mut expected = vec![
        touch(Press, 0, &[finger_at(0, 10.0, 20.0, true)]),
        touch(Press, 0, &[finger_at(0, 10.0, 20.0, true), finger_at(1, 10.0, 50.0, true)]),
        touch(
            Press,
            0,
            &[
                finger_at(0, 10.0, 20.0, true),
                finger_at(1, 10.0, 50.0, true),
                finger_at(2, 10.0, 80.0, true),
            ],
        ),
    ];
    for (step, x) in [30.0, 50.0, 70.0, 90.0].into_iter().enumerate() {
        expected.push(touch(
            Move,
            16 * (step as u64 + 1),
            &[
                finger_at(0, x, 20.0, true),
                finger_at(1, x, 50.0, true),
                finger_at(2, x, 80.0, true),
            ],
        ));
    }
    expected.push(touch(
        Release,
        64,
        &[
            finger_at(0, 90.0, 20.0, false),
            finger_at(1, 90.0, 50.0, true),
            finger_at(2, 90.0, 80.0, true),
        ],
    ));
    expected.push(touch(
        Release,
        64,
        &[finger_at(1, 90.0, 50.0, false), finger_at(2, 90.0, 80.0, true)],
    ));
    expected.push(touch(Release, 64, &[finger_at(2, 90.0, 80.0, false)]));
    assert_eq!(heard, expected);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/touch/SwipeWithTouchSlopTest.kt, swipeScrollable_accountForTouchSlop
/// A drag scrolls by its distance less the touch slop (18 here).
#[test]
fn swipe_scrollable_account_for_touch_slop() {
    let touch_slop = 18.0;
    let state = ScrollState::new(0);
    let given = *state;
    // A window the viewport fits in.
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        // A viewport the size of 10 boxes (density 1).
        Column!(
            modifier = Modifier
                .test_tag("scrollable")
                .required_size((100.dp(), 1000.dp()))
                .vertical_scroll(given),
            {
                for _ in 0..100 {
                    Box!(modifier = Modifier.size(100.dp()));
                }
            }
        );
    });
    ui.host().set_view_configuration(Some(ViewConfiguration { touch_slop, ..Default::default() }));
    ui.wait_for_idle();
    assert_eq!(state.value(), 0);
    // 100 boxes of 100 less the 1000 of the viewport.
    assert_eq!(state.max_value(), 9000);
    // Compose's `performTouchInput` doesn't wait for idle after the gesture, so its test reads
    // the value before the fling's frames; compose-rs's waits, so the clock is paused instead.
    ui.set_auto_advance(false);
    ui.on_node_with_tag("scrollable").perform_touch_input(|scope| {
        let from = scope.bottom_center() - Offset::new(0.0, 99.0);
        let past_slop = from - Offset::new(0.0, touch_slop);
        let to = scope.top_center() + Offset::new(0.0, 100.0);
        scope.down(from);
        scope.move_to(past_slop);
        scope.move_to(to);
        scope.up();
    });
    assert_eq!(state.value(), (800.0f32 - touch_slop).round() as i32);
    assert_eq!(state.max_value(), 9000);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/touch/MultiTouchSwipeWithVelocityTest.kt, testMultiTouchSwipeWithVelocity
/// Two fingers swiping together, each ends at its velocity (as a velocity tracker of its
/// events finds it, within 0.1).
#[test]
fn test_multi_touch_swipe_with_velocity() {
    let (mut ui, log) = recording_root();
    let (velocity_a, velocity_b) = (150.0, 300.0);
    ui.on_node_with_tag(ROOT).perform_touch_input(|scope| {
        scope.multi_touch_swipe_with_velocity(
            &[
                SwipeInfo {
                    start: Offset::new(10.0, 10.0),
                    end: Offset::new(100.0, 10.0),
                    end_velocity: velocity_a,
                },
                SwipeInfo {
                    start: Offset::new(10.0, 100.0),
                    end: Offset::new(200.0, 100.0),
                    end_velocity: velocity_b,
                },
            ],
            Some(200),
            &[],
        );
    });
    let (mut tracker_a, mut tracker_b) =
        (compose_ui::VelocityTracker::new(), compose_ui::VelocityTracker::new());
    for heard in heard(&log) {
        let Heard::Event { millis, changes, .. } = heard else { continue };
        for (id, position, _) in changes {
            if id == finger(0) {
                tracker_a.add_position(millis as i64, position);
            } else if id == finger(1) {
                tracker_b.add_position(millis as i64, position);
            }
        }
    }
    let (a, b) = (tracker_a.calculate_velocity(), tracker_b.calculate_velocity());
    assert!((a.x - velocity_a).abs() <= 0.1 && a.y.abs() <= 0.1, "{a:?}");
    assert!((b.x - velocity_b).abs() <= 0.1 && b.y.abs() <= 0.1, "{b:?}");
}
