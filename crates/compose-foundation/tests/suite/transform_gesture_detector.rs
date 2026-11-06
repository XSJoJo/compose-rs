//! `detect_transform_gestures`: Compose's `TransformGestureDetectorTest`, rewritten, each test with
//! `pan_zoom_lock` off and on. Density 1, a touch slop of 18 px and no minimum touch target, as its
//! `TestViewConfiguration`.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{
    Alignment, DpSize, IntSize, Offset, PointerEventPass, PointerId, PointerInputScope, UnitExt,
    ViewConfiguration,
};

const TAG: &str = "TargetLayout";

#[derive(Default)]
struct Seen {
    centroid: Cell<Offset>,
    panned: Cell<bool>,
    pan: Cell<Offset>,
    rotated: Cell<bool>,
    rotation: Cell<f32>,
    zoomed: Cell<bool>,
    zoom: Cell<f32>,
}

impl Seen {
    fn clear(&self) {
        self.panned.set(false);
        self.pan.set(Offset::ZERO);
        self.rotated.set(false);
        self.rotation.set(0.0);
        self.zoomed.set(false);
        self.zoom.set(1.0);
    }
}

/// The detector in a 1600 px box, with a parent that may consume each change on the way in and
/// records whether each was consumed on the way out.
struct Detector {
    ui: ComposeTestRule,
    seen: Rc<Seen>,
    consume_initial: Rc<Cell<bool>>,
    finals: Rc<RefCell<Vec<(PointerId, bool)>>>,
}

fn setup(pan_zoom_lock: bool) -> Detector {
    let seen = Rc::new(Seen::default());
    seen.clear();
    let consume_initial = Rc::new(Cell::new(false));
    let finals: Rc<RefCell<Vec<(PointerId, bool)>>> = Rc::default();
    let (into, consuming, finals_into) = (seen.clone(), consume_initial.clone(), finals.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(2000, 2000), move || {
        let (into, consuming, finals_into) = (into.clone(), consuming.clone(), finals_into.clone());
        let parent = move |scope: PointerInputScope| {
            let (consuming, finals_into) = (consuming.clone(), finals_into.clone());
            async move {
                loop {
                    let event = scope.await_pointer_event(PointerEventPass::Initial).await;
                    if consuming.get() {
                        event.changes.iter().for_each(|change| change.consume());
                    }
                    let event = scope.await_pointer_event(PointerEventPass::Final).await;
                    let mut finals = finals_into.borrow_mut();
                    finals.extend(
                        event.changes.iter().map(|change| (change.id, change.is_consumed())),
                    );
                }
            }
        };
        let gestures = move |scope: PointerInputScope| {
            let into = into.clone();
            async move {
                scope
                    .detect_transform_gestures(
                        pan_zoom_lock,
                        move |centroid, pan, zoom, rotation| {
                            into.centroid.set(centroid);
                            if rotation != 0.0 {
                                into.rotated.set(true);
                                into.rotation.set(into.rotation.get() + rotation);
                            }
                            if zoom != 1.0 {
                                into.zoomed.set(true);
                                into.zoom.set(into.zoom.get() * zoom);
                            }
                            if pan != Offset::ZERO {
                                into.panned.set(true);
                                into.pan.set(into.pan.get() + pan);
                            }
                        },
                    )
                    .await
            }
        };
        Box!(
            modifier = Modifier
                .fill_max_size()
                .pointer_input((), parent)
                .wrap_content_size_with(Alignment::TOP_START, false)
                .size(1600.dp())
                .pointer_input((), gestures)
                .test_tag(TAG)
        );
    });
    ui.host().set_view_configuration(Some(ViewConfiguration {
        touch_slop: 18.0,
        minimum_touch_target_size: DpSize::new(0.dp(), 0.dp()),
        ..ViewConfiguration::default()
    }));
    ui.wait_for_idle();
    Detector { ui, seen, consume_initial, finals }
}

impl Detector {
    /// Sends `block`'s touches, consuming them on the way in if `consume`: whether each pointer's
    /// last change was consumed on the way out.
    fn touch(
        &mut self,
        consume: bool,
        block: impl FnOnce(&mut TouchInjectionScope<'_>),
    ) -> Vec<(PointerId, bool)> {
        self.finals.borrow_mut().clear();
        self.consume_initial.set(consume);
        self.ui.on_node_with_tag(TAG).perform_touch_input(block);
        self.ui.wait_for_idle();
        self.consume_initial.set(false);
        let mut last: Vec<(PointerId, bool)> = Vec::new();
        for &(id, consumed) in self.finals.borrow().iter() {
            last.retain(|(seen, _)| *seen != id);
            last.push((id, consumed));
        }
        last
    }
}

/// Finger `id`'s pointer, as the test rule numbers them.
fn finger(id: u64) -> PointerId {
    PointerId(id + 1)
}

fn consumed(finals: &[(PointerId, bool)], id: u64) -> bool {
    finals.iter().find(|(pointer, _)| *pointer == finger(id)).expect("seen").1
}

fn near(actual: f32, expected: f32, tolerance: f32) -> bool {
    (actual - expected).abs() <= tolerance
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, singleFingerPan
#[test]
fn one_finger_pans_past_the_slop() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        let finals = detector.touch(false, |touch| touch.down_pointer(0, Offset::new(5.0, 5.0)));
        assert!(!consumed(&finals, 0) && !detector.seen.panned.get());
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(0, Offset::new(12.7, 12.7), 16));
        assert!(!consumed(&finals, 0) && !detector.seen.panned.get());
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(0, Offset::new(0.1, 0.1), 16));
        assert!(consumed(&finals, 0));
        let (seen, centroid) = (&detector.seen, detector.seen.centroid.get());
        assert!(near(centroid.x, 17.7, 0.1) && near(centroid.y, 17.7, 0.1), "{centroid:?}");
        assert!(seen.panned.get() && !seen.zoomed.get() && !seen.rotated.get());
        assert!(seen.pan.get().x.hypot(seen.pan.get().y) < 1.0);
        seen.pan.set(Offset::ZERO);
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(0, Offset::new(1.0, 0.0), 16));
        assert!(consumed(&finals, 0));
        assert_eq!(detector.seen.pan.get(), Offset::new(1.0, 0.0));
        let finals = detector.touch(false, |touch| touch.up_pointer(0));
        assert!(!consumed(&finals, 0));
        assert!(!detector.seen.rotated.get() && !detector.seen.zoomed.get());
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, multiFingerPanZoom
#[test]
fn two_fingers_pan_and_zoom_past_the_slop() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        detector.touch(false, |touch| touch.down_pointer(0, Offset::new(5.0, 5.0)));
        let finals = detector.touch(false, |touch| touch.down_pointer(1, Offset::new(25.0, 25.0)));
        assert!(!consumed(&finals, 1) && !detector.seen.panned.get());
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(0, Offset::new(13.0, 13.0), 16));
        assert!(!consumed(&finals, 0));
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(1, Offset::new(13.0, 13.0), 16));
        assert!(consumed(&finals, 1));
        let (seen, centroid) = (&detector.seen, detector.seen.centroid.get());
        let expected = (5.0 + 25.0 + 13.0) / 2.0;
        assert!(near(centroid.x, expected, 0.1) && near(centroid.y, expected, 0.1), "{centroid:?}");
        assert!(seen.panned.get() && seen.zoomed.get() && !seen.rotated.get());
        assert!(
            near(seen.pan.get().x, 6.4, 0.1) && near(seen.pan.get().y, 6.4, 0.1),
            "{:?}",
            seen.pan.get()
        );
        detector.touch(false, |touch| {
            touch.up_pointer(0);
            touch.up_pointer(1);
        });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, zoom2Pointer
#[test]
fn two_fingers_apart_zoom() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        detector.touch(false, |touch| touch.down_pointer(0, Offset::new(5.0, 5.0)));
        let finals = detector.touch(false, |touch| touch.down_pointer(1, Offset::new(25.0, 5.0)));
        assert!(!consumed(&finals, 1));
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(1, Offset::new(35.95, 0.0), 16));
        assert!(!consumed(&finals, 1));
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(1, Offset::new(0.1, 0.0), 16));
        assert!(consumed(&finals, 1));
        let seen = detector.seen.clone();
        assert!(seen.panned.get() && seen.zoomed.get() && !seen.rotated.get());
        assert!(seen.pan.get().x.hypot(seen.pan.get().y) < 1.0);
        assert!((1.0..=1.1).contains(&seen.zoom.get()), "{}", seen.zoom.get());
        seen.zoom.set(1.0);
        seen.pan.set(Offset::ZERO);
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(0, Offset::new(-1.0, 0.0), 16));
        assert!(consumed(&finals, 0));
        let finals =
            detector.touch(false, |touch| touch.move_pointer_by(1, Offset::new(1.0, 0.0), 16));
        assert!(consumed(&finals, 1));
        assert!(near(seen.pan.get().x, 0.0, 0.01) && near(seen.pan.get().y, 0.0, 0.01));
        assert!(near(seen.zoom.get(), 48.0 / 46.0, 0.01), "{}", seen.zoom.get());
        detector.touch(false, |touch| {
            touch.up_pointer(0);
            touch.up_pointer(1);
        });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, zoom4Pointer
#[test]
fn four_fingers_apart_zoom() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        detector.touch(false, |touch| touch.down_pointer(0, Offset::new(0.0, 50.0)));
        detector.touch(false, |touch| {
            touch.move_pointer_by(0, Offset::new(-1000.0, 0.0), 16);
            touch.move_pointer_by(0, Offset::new(1000.0, 0.0), 16);
        });
        let seen = detector.seen.clone();
        seen.panned.set(false);
        seen.pan.set(Offset::ZERO);
        detector.touch(false, |touch| {
            touch.down_pointer(1, Offset::new(100.0, 50.0));
            touch.down_pointer(2, Offset::new(50.0, 0.0));
            touch.down_pointer(3, Offset::new(50.0, 100.0));
        });
        detector.touch(false, |touch| {
            touch.move_pointer_by(0, Offset::new(-50.0, 0.0), 16);
            touch.move_pointer_by(1, Offset::new(50.0, 0.0), 16);
        });
        assert!(seen.zoomed.get() && seen.panned.get());
        assert!(near(seen.pan.get().x, 0.0, 0.1) && near(seen.pan.get().y, 0.0, 0.1));
        assert!(near(seen.zoom.get(), 1.5, 0.1), "{}", seen.zoom.get());
        detector.touch(false, |touch| {
            touch.move_pointer_by(2, Offset::new(0.0, -50.0), 16);
            touch.move_pointer_by(3, Offset::new(0.0, 50.0), 16);
        });
        assert!(near(seen.pan.get().x, 0.0, 0.1) && near(seen.pan.get().y, 0.0, 0.1));
        assert!(near(seen.zoom.get(), 2.0, 0.1), "{}", seen.zoom.get());
        detector.touch(false, |touch| (0..4).for_each(|id| touch.up_pointer(id)));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, rotation2Pointer
#[test]
fn two_fingers_turning_rotate() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        detector.touch(false, |touch| {
            touch.down_pointer(0, Offset::new(0.0, 50.0));
            touch.down_pointer(1, Offset::new(100.0, 50.0));
            touch.move_pointer_by(0, Offset::new(50.0, -50.0), 16);
            touch.move_pointer_by(1, Offset::new(-50.0, 50.0), 16);
        });
        let seen = detector.seen.clone();
        assert!(seen.rotated.get());
        seen.clear();
        detector.touch(false, |touch| {
            touch.move_pointer_by(0, Offset::new(-50.0, 50.0), 16);
            touch.move_pointer_by(1, Offset::new(50.0, -50.0), 16);
        });
        detector.touch(false, |touch| {
            touch.up_pointer(0);
            touch.up_pointer(1);
        });
        assert!(seen.rotated.get());
        assert!(near(seen.rotation.get(), -90.0, 0.01), "{}", seen.rotation.get());
        assert!(near(seen.pan.get().x, 0.0, 0.1) && near(seen.pan.get().y, 0.0, 0.1));
        assert!(near(seen.zoom.get(), 1.0, 0.1));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, rotation2PointerLock
#[test]
fn a_gesture_started_by_a_pan_rotates_only_unlocked() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        detector.touch(false, |touch| touch.down_pointer(0, Offset::new(0.0, 50.0)));
        detector.touch(false, |touch| {
            touch.move_pointer_by(0, Offset::new(-1000.0, 0.0), 16);
            touch.move_pointer_by(0, Offset::new(1000.0, 0.0), 16);
        });
        detector.touch(false, |touch| touch.down_pointer(1, Offset::new(100.0, 50.0)));
        detector.touch(false, |touch| {
            touch.move_pointer_by(0, Offset::new(50.0, -50.0), 16);
            touch.move_pointer_by(1, Offset::new(-50.0, 50.0), 16);
        });
        detector.touch(false, |touch| {
            touch.up_pointer(0);
            touch.up_pointer(1);
        });
        let seen = &detector.seen;
        if lock {
            assert!(!seen.rotated.get());
        } else {
            assert!(seen.rotated.get());
            assert!(near(seen.rotation.get(), 90.0, 0.01), "{}", seen.rotation.get());
        }
        assert!(near(seen.pan.get().x, 0.0, 0.1) && near(seen.pan.get().y, 0.0, 0.1));
        assert!(near(seen.zoom.get(), 1.0, 0.1));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, noChangeOnPointerDownUp
#[test]
fn a_finger_coming_or_going_changes_nothing() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        detector.touch(false, |touch| {
            touch.down_pointer(0, Offset::new(0.0, 50.0));
            touch.down_pointer(1, Offset::new(100.0, 50.0));
            touch.move_pointer_by(0, Offset::new(50.0, -50.0), 16);
            touch.move_pointer_by(1, Offset::new(-50.0, 50.0), 16);
        });
        let seen = detector.seen.clone();
        let quiet = || !seen.rotated.get() && !seen.panned.get() && !seen.zoomed.get();
        seen.clear();
        detector.touch(false, |touch| touch.down_pointer(2, Offset::new(0.0, 50.0)));
        assert!(quiet());
        detector.touch(false, |touch| touch.down_pointer(3, Offset::new(100.0, 50.0)));
        assert!(quiet());
        detector.touch(false, |touch| (0..4).for_each(|id| touch.up_pointer(id)));
        assert!(quiet());
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, touchSlopCancel
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/gestures/TransformGestureDetectorTest.kt, afterTouchSlopCancel
#[test]
fn a_move_consumed_on_the_way_in_cancels_the_gesture() {
    for lock in [false, true] {
        let mut detector = setup(lock);
        detector.touch(false, |touch| touch.down_pointer(0, Offset::new(5.0, 5.0)));
        detector.touch(true, |touch| touch.move_pointer_by(0, Offset::new(50.0, 0.0), 16));
        detector.touch(false, |touch| touch.up_pointer(0));
        let seen = &detector.seen;
        assert!(!seen.panned.get() && !seen.zoomed.get() && !seen.rotated.get());
        let mut detector = setup(lock);
        detector.touch(false, |touch| touch.down_pointer(0, Offset::new(5.0, 5.0)));
        detector.touch(false, |touch| touch.move_pointer_by(0, Offset::new(50.0, 0.0), 16));
        detector.touch(true, |touch| touch.move_pointer_by(0, Offset::new(50.0, 0.0), 16));
        detector.touch(false, |touch| touch.up_pointer(0));
        let seen = &detector.seen;
        assert!(seen.panned.get() && !seen.zoomed.get() && !seen.rotated.get());
        assert!(near(seen.pan.get().x, 50.0, 0.1), "{:?}", seen.pan.get());
    }
}
