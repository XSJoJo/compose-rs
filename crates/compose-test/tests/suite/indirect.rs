//! Indirect pointer input (D1675): `IndirectPointerEvent` and its changes, the dispatch to what
//! has the focus, the cancels as the focus leaves, the navigation gesture detector, and
//! `send_indirect_pointer_input`, as Compose's `input/indirect` and `injectionscope/indirecttouch`
//! tests check them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_test::*;
use compose_ui::{
    Density, EventContext, FocusDirection, FocusRequester, HistoricalChange, IndirectPointerEvent,
    IndirectPointerEventPrimaryDirectionalMotionAxis as Axis, IndirectPointerEventType,
    IndirectPointerInputChange, IndirectPointerInputModifierNode,
    IndirectPointerNavigationGestureDetector, IntSize, Modifier, ModifierNode, ModifierNodeElement,
    Offset, PointerEventPass, PointerId, UnitExt, ViewConfiguration,
};

use IndirectPointerEventType::{Move, Press, Release};

// ---- A test modifier, as Compose's `Modifier.onIndirectPointerInput` ----------------------------

type OnEvent = Rc<dyn Fn(&IndirectPointerEvent, PointerEventPass)>;

#[derive(Clone)]
struct IndirectElement(OnEvent, Rc<dyn Fn()>);

struct IndirectNode(OnEvent, Rc<dyn Fn()>);

impl ModifierNodeElement for IndirectElement {
    type Node = IndirectNode;
    fn create(&self) -> IndirectNode {
        IndirectNode(self.0.clone(), self.1.clone())
    }
    fn update(&self, node: &mut IndirectNode) {
        node.0 = self.0.clone();
        node.1 = self.1.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ModifierNode for IndirectNode {
    fn as_indirect_pointer_input(&mut self) -> Option<&mut dyn IndirectPointerInputModifierNode> {
        Some(self)
    }
}

impl IndirectPointerInputModifierNode for IndirectNode {
    fn on_indirect_pointer_event(
        &mut self,
        event: &IndirectPointerEvent,
        pass: PointerEventPass,
        _context: &mut EventContext<'_>,
    ) {
        (self.0)(event, pass)
    }
    fn on_cancel_indirect_pointer_input(&mut self) {
        (self.1)()
    }
}

type Log = Rc<RefCell<Vec<String>>>;

/// Logs `name:<pass>` and `name:cancel`.
fn logging(log: &Log, name: &'static str) -> IndirectElement {
    let (events, cancels) = (log.clone(), log.clone());
    IndirectElement(
        Rc::new(move |_, pass| events.borrow_mut().push(format!("{name}:{pass:?}"))),
        Rc::new(move || cancels.borrow_mut().push(format!("{name}:cancel"))),
    )
}

/// Keeps the last event's main pass in `last`, counts cancels in `cancels`.
fn keeping(
    last: &Rc<RefCell<Option<IndirectPointerEvent>>>,
    cancels: &Rc<Cell<u32>>,
) -> IndirectElement {
    let (last, cancels) = (last.clone(), cancels.clone());
    IndirectElement(
        Rc::new(move |event, pass| {
            if pass == PointerEventPass::Main {
                *last.borrow_mut() = Some(event.clone());
            }
        }),
        Rc::new(move || cancels.set(cancels.get() + 1)),
    )
}

fn take(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

/// One change at `position` at `millis`, as a touch-navigation `MotionEvent` of `kind`.
fn event_at(kind: IndirectPointerEventType, position: Offset, millis: u64) -> IndirectPointerEvent {
    let pressed = kind != Release;
    let change = IndirectPointerInputChange::new(
        PointerId(0),
        millis,
        position,
        pressed,
        1.0,
        millis,
        position,
        kind != Press,
    );
    IndirectPointerEvent::new(vec![change], kind, Axis::X)
}

fn send(ui: &mut ComposeTestRule, event: IndirectPointerEvent) -> bool {
    let consumed = ui.host().send_indirect_pointer(event);
    ui.wait_for_idle();
    consumed
}

fn cancel(ui: &mut ComposeTestRule) {
    ui.host().cancel_indirect_pointer();
    ui.wait_for_idle();
}

// ---- IndirectPointerInputChange --------------------------------------------------------------

fn change(id: u64, millis: u64, position: Offset, pressed: bool) -> IndirectPointerInputChange {
    IndirectPointerInputChange::new(
        PointerId(id),
        millis,
        position,
        pressed,
        0.5,
        1000,
        Offset::ZERO,
        !pressed,
    )
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerInputChangeTest.kt, downChange_propertiesAreCorrect, upChange_propertiesAreCorrect, moveChange_propertiesAreCorrect
#[test]
fn a_change_has_its_properties() {
    for pressed in [true, false] {
        let change = change(1, 5000, Offset::new(10.0, 20.0), pressed);
        assert_eq!(change.id, PointerId(1));
        assert_eq!(change.uptime_millis, 5000);
        assert_eq!(change.position, Offset::new(10.0, 20.0));
        assert_eq!(change.pressed, pressed);
        assert_eq!(change.pressure, 0.5);
        assert!(!change.is_consumed());
        assert_eq!(change.previous_uptime_millis, 1000);
        assert_eq!(change.previous_position, Offset::ZERO);
        assert_eq!(change.previous_pressed, !pressed);
    }
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerInputChangeTest.kt, consume_setsIsConsumedToTrue, consume_multipleCalls_isIdempotent
#[test]
fn consuming_a_change_marks_it_once_and_for_all() {
    let change = change(0, 0, Offset::ZERO, true);
    assert!(!change.is_consumed());
    change.consume();
    assert!(change.is_consumed());
    change.consume();
    assert!(change.is_consumed());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerInputChangeTest.kt, historical_propertiesAreCorrect, copy_copiesHistoricalAndOtherProperties, copy_overrideHistorical, toString_containsHistorical
/// A change keeps its historical moves; a copy keeps them and the rest, each field then
/// changeable; its description tells them.
#[test]
fn a_change_keeps_its_history_and_copies_with_it() {
    let historical = vec![
        HistoricalChange::new(2000, Offset::new(5.0, 5.0)),
        HistoricalChange::new(3000, Offset::new(8.0, 8.0)),
    ];
    let change = change(1, 5000, Offset::new(10.0, 20.0), true).with_historical(historical.clone());
    assert_eq!(change.historical, historical);
    assert!(format!("{change:?}").contains(&format!("historical={historical:?}")));
    let mut copy = change.clone();
    copy.uptime_millis = 6000;
    copy.position = Offset::new(15.0, 25.0);
    assert_eq!(
        (copy.id, copy.uptime_millis, copy.position),
        (PointerId(1), 6000, Offset::new(15.0, 25.0))
    );
    assert_eq!((copy.pressed, copy.pressure, copy.previous_uptime_millis), (true, 0.5, 1000));
    assert_eq!((copy.previous_position, copy.previous_pressed), (Offset::ZERO, false));
    assert_eq!(copy.historical, historical);
    let newer = vec![HistoricalChange::new(4000, Offset::new(9.0, 9.0))];
    assert_eq!(change.clone().with_historical(newer.clone()).historical, newer);
    // A copy is consumed if the change is, then on its own.
    change.consume();
    let consumed = change.clone();
    assert!(consumed.is_consumed());
}

// ---- IndirectPointerEvent ----------------------------------------------------------------------

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventTest.kt, constructor_singleChange_propertiesAreCorrect, constructor_multipleChanges_propertiesAreCorrect, constructor_emptyChanges_throwsIllegalArgumentException
#[test]
fn an_event_has_its_changes_and_needs_one() {
    let first = change(0, 200, Offset::new(1.0, 2.0), true);
    let second = change(1, 300, Offset::new(3.0, 4.0), true);
    let event = IndirectPointerEvent::new(vec![first.clone()], Press, Axis::X);
    assert_eq!(event.changes.len(), 1);
    assert_eq!(event.changes[0].position, first.position);
    let event = IndirectPointerEvent::new(vec![first, second], Press, Axis::X);
    let ids: Vec<PointerId> = event.changes.iter().map(|it| it.id).collect();
    assert_eq!(ids, [PointerId(0), PointerId(1)]);
    assert_eq!((event.kind, event.primary_directional_motion_axis), (Press, Axis::X));
    let empty = std::panic::catch_unwind(|| IndirectPointerEvent::new(Vec::new(), Press, Axis::X));
    assert!(empty.is_err());
}

/// A box taking the focus at once, with what `modifier` adds before its focus target.
fn focused_box(before: impl Fn() -> Modifier + 'static) -> ComposeTestRule {
    let requester = FocusRequester::new();
    let before = Rc::new(before);
    let mut ui = compose_test(move || {
        let before = before.clone();
        Box!(modifier = Modifier.size(10.dp()), {
            Box!(modifier = before().focus_requester(requester).focus_target().size(10.dp()));
        });
    });
    requester.request_focus();
    ui.wait_for_idle();
    ui
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventTest.kt, androidTouchNavigationEvent_triggersIndirectPointerEvent
#[test]
fn the_focused_node_hears_an_event_in_three_passes() {
    let log = Log::default();
    let element = logging(&log, "node");
    let mut ui = focused_box(move || Modifier.then_custom(element.clone()));
    send(&mut ui, event_at(Press, Offset::ZERO, 0));
    assert_eq!(take(&log), ["node:Initial", "node:Main", "node:Final"]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventTest.kt, androidTouchNavigationEventWithParent_triggersIndirectPointerEventsInOrderAndInParentAndChild
/// The focused node's ancestors hear it too: in, out, then in again.
#[test]
fn a_parent_hears_it_around_its_child() {
    let log = Log::default();
    let (parent, child) = (logging(&log, "parent"), logging(&log, "child"));
    let requester = FocusRequester::new();
    let mut ui = compose_test(move || {
        let child = child.clone();
        Box!(modifier = Modifier.then_custom(parent.clone()).focus_target(), {
            Box!(
                modifier = Modifier
                    .then_custom(child.clone())
                    .focus_requester(requester)
                    .focus_target()
                    .size(10.dp())
            );
        });
    });
    requester.request_focus();
    ui.wait_for_idle();
    send(&mut ui, event_at(Press, Offset::ZERO, 0));
    assert_eq!(
        take(&log),
        [
            "parent:Initial",
            "child:Initial",
            "child:Main",
            "parent:Main",
            "parent:Final",
            "child:Final"
        ]
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventTest.kt, afterFocusable_androidTouchNavigationEvent_triggersIndirectPointerEventAndCancel
/// A node after the focus target in its chain is the innermost; a cancel goes from it out.
#[test]
fn a_node_after_the_focus_target_is_innermost() {
    let log = Log::default();
    let (before, after) = (logging(&log, "before"), logging(&log, "after"));
    let requester = FocusRequester::new();
    let mut ui = compose_test(move || {
        Box!(
            modifier = Modifier
                .then_custom(before.clone())
                .focus_requester(requester)
                .focus_target()
                .then_custom(after.clone())
                .size(10.dp())
        );
    });
    requester.request_focus();
    ui.wait_for_idle();
    send(&mut ui, event_at(Press, Offset::ZERO, 0));
    assert_eq!(
        take(&log),
        [
            "before:Initial",
            "after:Initial",
            "after:Main",
            "before:Main",
            "before:Final",
            "after:Final"
        ]
    );
    cancel(&mut ui);
    assert_eq!(take(&log), ["after:cancel", "before:cancel"]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventTest.kt, afterFocusable_focusChange_triggersIndirectPointerCancel
#[test]
fn the_focus_leaving_cancels() {
    let log = Log::default();
    let after = logging(&log, "after");
    let (first, second) = (FocusRequester::new(), FocusRequester::new());
    let mut ui = compose_test(move || {
        let after = after.clone();
        Column!({
            Box!(
                modifier = Modifier
                    .focus_requester(first)
                    .focus_target()
                    .then_custom(after.clone())
                    .size(10.dp())
            );
            Box!(modifier = Modifier.focus_requester(second).focus_target().size(10.dp()));
        });
    });
    first.request_focus();
    ui.wait_for_idle();
    assert!(take(&log).is_empty());
    second.request_focus();
    ui.wait_for_idle();
    assert_eq!(take(&log), ["after:cancel"]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventTest.kt, afterFocusable_stopsAtNextFocusTarget
/// What's after the next focus target in the chain is that target's: it hears nothing until
/// that one has the focus, and it's cancelled when the focus goes back out.
#[test]
fn the_path_stops_at_the_next_focus_target() {
    let log = Log::default();
    let (between, after_second) = (logging(&log, "between"), logging(&log, "afterSecond"));
    let (first, second) = (FocusRequester::new(), FocusRequester::new());
    let mut ui = compose_test(move || {
        Box!(
            modifier = Modifier
                .focus_requester(first)
                .focus_target()
                .then_custom(between.clone())
                .focus_requester(second)
                .focus_target()
                .then_custom(after_second.clone())
                .size(10.dp())
        );
    });
    first.request_focus();
    ui.wait_for_idle();
    send(&mut ui, event_at(Press, Offset::ZERO, 0));
    assert_eq!(take(&log), ["between:Initial", "between:Main", "between:Final"]);
    second.request_focus();
    ui.wait_for_idle();
    assert!(take(&log).is_empty(), "the one around it is still on the focus's path");
    send(&mut ui, event_at(Press, Offset::ZERO, 0));
    assert_eq!(
        take(&log),
        [
            "between:Initial",
            "afterSecond:Initial",
            "afterSecond:Main",
            "between:Main",
            "between:Final",
            "afterSecond:Final"
        ]
    );
    first.request_focus();
    ui.wait_for_idle();
    assert_eq!(take(&log), ["afterSecond:cancel"]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventTest.kt, indirectPointerEventContainsPosition, indirectPointerEventContainsEventTime, indirectPointerEvent_actionDown_hasIndirectPointerEventTypePress, indirectPointerEvent_actionUp_hasIndirectPointerEventTypeRelease, indirectPointerEvent_actionMove_hasIndirectPointerEventTypeMove, indirectPointerEvent_actionUnknown_hasIndirectPointerEventTypeUnknown
/// The node hears the event as sent: its type, position and time.
#[test]
fn the_event_comes_as_sent() {
    let last = Rc::new(RefCell::new(None));
    let cancels = Rc::new(Cell::new(0));
    let element = keeping(&last, &cancels);
    let mut ui = focused_box(move || Modifier.then_custom(element.clone()));
    for (kind, millis) in
        [(Press, 1000), (Move, 1100), (Release, 1200), (IndirectPointerEventType::Unknown, 1300)]
    {
        send(&mut ui, event_at(kind, Offset::new(12.0, 34.0), millis));
        let heard = last.borrow().clone().expect("an event");
        assert_eq!(heard.kind, kind);
        assert_eq!(heard.changes[0].position, Offset::new(12.0, 34.0));
        assert_eq!(heard.changes[0].uptime_millis, millis);
    }
    assert_eq!(cancels.get(), 0);
}

/// With nothing focused, nothing hears it.
#[test]
fn nothing_hears_it_without_a_focus() {
    let log = Log::default();
    let element = logging(&log, "node");
    let mut ui = compose_test(move || {
        Box!(modifier = Modifier.then_custom(element.clone()).focus_target().size(10.dp()));
    });
    assert!(!send(&mut ui, event_at(Press, Offset::ZERO, 0)));
    assert!(take(&log).is_empty());
}

// ---- The navigation gesture detector ------------------------------------------------------------

/// The detector's view configuration: Android's at density 1 (8 px of slop, flings from 50 px/s).
fn configuration() -> ViewConfiguration {
    ViewConfiguration::at(Density::new(1.0))
}

/// A swipe of `steps` (each a move `delta` 20 ms after the last, the release one more), then
/// what the detector asked for. `extra_down` presses again before the release; `consumed` says
/// which of press, moves and release a node consumed.
fn detect(
    axis: Axis,
    delta: Offset,
    extra_down: bool,
    consumed: [bool; 3],
) -> Option<FocusDirection> {
    let mut detector = IndirectPointerNavigationGestureDetector::new();
    let configuration = configuration();
    let mut position = Offset::new(100.0, 100.0);
    let mut time = 1000;
    let mut feed = |kind, position, time, consumed| {
        let event = IndirectPointerEvent::new(
            vec![IndirectPointerInputChange::new(
                PointerId(0),
                time,
                position,
                kind != Release,
                1.0,
                time,
                position,
                kind != Press,
            )],
            kind,
            axis,
        );
        detector.on_indirect_pointer_event(&event, consumed, &configuration)
    };
    // Nothing before the release, as Kotlin's asserts after each event.
    assert_eq!(feed(Press, position, time, consumed[0]), None, "the press");
    for at in 0..2 {
        position = position + delta;
        time += 20;
        assert_eq!(feed(Move, position, time, consumed[1]), None, "move {at}");
    }
    if extra_down {
        time += 20;
        assert_eq!(feed(Press, position, time, false), None, "the second press");
    }
    position = position + delta;
    time += 20;
    feed(Release, position, time, consumed[2])
}

const FREE: [bool; 3] = [false; 3];

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerNavigationGestureDetectorTest.kt, indirectPointerNavigationGesture_swipeForwardHorizontally_triggersNext, indirectPointerNavigationGesture_swipeBackwardHorizontally_triggersPrevious, indirectPointerNavigationGesture_swipeForwardVertically_doesNotTrigger, indirectPointerNavigationGesture_swipeBackwardVertically_doesNotTrigger, indirectPointerNavigationGesture_swipeForwardDiagonalSameLargeXAndYDeltas_doesNotTrigger, indirectPointerNavigationGesture_swipeBackwardDiagonalSameLargeXAndYDeltas_doesNotTrigger, indirectPointerNavigationGesture_swipeForwardDiagonalLargeXDeltaSmallYDelta_triggersNext, indirectPointerNavigationGesture_swipeBackwardDiagonalLargeXDeltaSmallYDelta_triggersPrevious, indirectPointerNavigationGesture_swipeForwardDiagonalSmallXDeltaLargeYDelta_doesNotTrigger, indirectPointerNavigationGesture_swipeBackwardDiagonalSmallXDeltaLargeYDelta_doesNotTrigger
/// A fling mostly along the primary axis (X) moves the focus: forward to the next, back to the
/// previous; across it, or as much across as along, nothing.
#[test]
fn a_fling_along_the_axis_moves_the_focus() {
    let cases = [
        (Offset::new(50.0, 0.0), Some(FocusDirection::Next)),
        (Offset::new(-50.0, 0.0), Some(FocusDirection::Previous)),
        (Offset::new(0.0, 50.0), None),
        (Offset::new(0.0, -50.0), None),
        (Offset::new(50.0, 50.0), None),
        (Offset::new(-50.0, -50.0), None),
        (Offset::new(50.0, 10.0), Some(FocusDirection::Next)),
        (Offset::new(-50.0, -10.0), Some(FocusDirection::Previous)),
        (Offset::new(10.0, 50.0), None),
        (Offset::new(-10.0, -50.0), None),
    ];
    for (delta, expected) in cases {
        assert_eq!(detect(Axis::X, delta, false, FREE), expected, "{delta:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerNavigationGestureDetectorTest.kt, indirectPointerNavigationGesture_swipeForwardHorizontallyWithExtraDown_doesNotTrigger
/// A press in the middle starts the stream over: the release right after it is a tap.
#[test]
fn a_press_in_the_middle_starts_over() {
    assert_eq!(detect(Axis::X, Offset::new(50.0, 0.0), true, FREE), None);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerNavigationGestureDetectorTest.kt, indirectPointerNavigationGesture_swipeForwardHorizontally_whenDownIsConsumed_doesTrigger, indirectPointerNavigationGesture_swipeForwardHorizontally_whenMoveIsConsumed_doesNotTrigger, indirectPointerNavigationGesture_swipeForwardHorizontally_whenUpIsConsumed_doesNotTrigger
/// A node consuming the moves or the release takes the stream; its press doesn't.
#[test]
fn a_consumed_move_or_release_takes_the_stream() {
    let forward = Offset::new(50.0, 0.0);
    assert_eq!(detect(Axis::X, forward, false, [true, false, false]), Some(FocusDirection::Next));
    assert_eq!(detect(Axis::X, forward, false, [false, true, false]), None);
    assert_eq!(detect(Axis::X, forward, false, [false, false, true]), None);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerNavigationGestureDetectorHostTest.kt
/// Disposed in the middle of a stream, the detector asks for nothing at its release.
#[test]
fn a_disposed_detector_asks_for_nothing() {
    let mut detector = IndirectPointerNavigationGestureDetector::new();
    let configuration = configuration();
    let at = |kind, x: f32, time| {
        let mut event = event_at(kind, Offset::new(x, 0.0), time);
        event.primary_directional_motion_axis = Axis::X;
        event
    };
    detector.on_indirect_pointer_event(&at(Press, 0.0, 0), false, &configuration);
    detector.on_indirect_pointer_event(&at(Move, 50.0, 20), false, &configuration);
    detector.dispose();
    assert_eq!(
        detector.on_indirect_pointer_event(&at(Release, 100.0, 40), false, &configuration),
        None
    );
}

// ---- Through the system: swipes on a device move the focus --------------------------------------

const DEVICE: IntSize = IntSize::new(3082, 616);

struct Row3 {
    ui: ComposeTestRule,
    /// The container's and each box's cancels.
    cancels: [Rc<Cell<u32>>; 4],
    last: Rc<RefCell<Option<IndirectPointerEvent>>>,
}

/// A column of three focusable boxes, each with an indirect node before its focus target, in a
/// container with one, as Compose's navigation tests' simple UI; the middle box focused.
fn three_boxes(vertical: bool) -> Row3 {
    let cancels: [Rc<Cell<u32>>; 4] = Default::default();
    let last = Rc::new(RefCell::new(None));
    let container = keeping(&last, &cancels[0]);
    let boxes: Vec<IndirectElement> =
        (1..4).map(|at| keeping(&Rc::new(RefCell::new(None)), &cancels[at])).collect();
    let content = Rc::new(move || {
        for (at, element) in boxes.iter().enumerate() {
            Box!(
                modifier = Modifier
                    .test_tag(["box1", "box2", "box3"][at])
                    .size(100.dp())
                    .padding(10.dp())
                    .then_custom(element.clone())
                    .focusable()
            );
        }
    });
    let ui = compose_test(move || {
        let content = content.clone();
        let modifier = Modifier.fill_max_size().then_custom(container.clone());
        if vertical {
            Column!(modifier = modifier, { content() });
        } else {
            Row!(modifier = modifier, { content() });
        }
    });
    let mut row = Row3 { ui, cancels, last };
    row.ui.on_node_with_tag("box2").request_focus();
    row
}

impl Row3 {
    fn cancels(&self) -> [u32; 4] {
        [0, 1, 2, 3].map(|at| self.cancels[at].get())
    }
}

/// A swipe on the device from its center by `delta` in `steps` moves 20 ms apart.
fn swipe_by(scope: &mut IndirectPointerInjectionScope<'_>, delta: Offset, steps: u32) {
    let start = scope.input_device_center();
    scope.down(start);
    for step in 1..=steps {
        scope.move_pointer_to(0, start + delta * (step as f32 / steps as f32), 20);
    }
    scope.up();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, swipeViaNavigation_swipeRightWithPrimaryAxisX_movesFocusableBoxToNext, swipeViaNavigation_swipeLeftWithPrimaryAxisX_movesFocusableBoxToPrevious, swipeViaNavigation_swipeDownWithPrimaryAxisY_movesFocusableBoxToNext, swipeViaNavigation_swipeUpWithPrimaryAxisY_movesFocusableBoxToPrevious
/// A swipe along the primary axis moves the focus to the next or the previous box: the box
/// that had it is told its stream ended, the container around them all isn't; the release
/// reaches the container unconsumed.
#[test]
fn a_swipe_along_the_axis_moves_the_focus() {
    let cases = [
        (Axis::X, false, Offset::new(600.0, 0.0), "box3"),
        (Axis::X, false, Offset::new(-600.0, 0.0), "box1"),
        (Axis::Y, true, Offset::new(0.0, 250.0), "box3"),
        (Axis::Y, true, Offset::new(0.0, -250.0), "box1"),
    ];
    for (axis, vertical, delta, focused) in cases {
        let mut row = three_boxes(vertical);
        row.ui.send_indirect_pointer_input(axis, DEVICE, |scope| swipe_by(scope, delta, 3));
        row.ui.on_node_with_tag(focused).assert_is_focused();
        assert_eq!(row.cancels(), [0, 0, 1, 0], "{delta:?}");
        let last = row.last.borrow().clone().expect("the release");
        assert_eq!(last.kind, Release);
        assert!(!last.changes[0].is_consumed() && !last.changes[0].pressed);
        assert!(last.changes[0].previous_pressed);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, swipeViaNavigation_swipeRightAndSmallDownWithPrimaryAxisX_movesFocusableBoxToNext, swipeViaNavigation_swipeLeftAndSmallUpWithPrimaryAxisX_movesFocusableBoxToPrevious, swipeViaNavigation_swipeDownAndSmallRightWithPrimaryAxisY_movesFocusableBoxToNext, swipeViaNavigation_swipeUpAndSmallLeftWithPrimaryAxisY_movesFocusableBoxToPrevious
/// A little across the axis still moves it.
#[test]
fn a_swipe_a_little_across_still_moves_the_focus() {
    let cases = [
        (Axis::X, Offset::new(600.0, 20.0), "box3"),
        (Axis::X, Offset::new(-600.0, -20.0), "box1"),
        (Axis::Y, Offset::new(20.0, 250.0), "box3"),
        (Axis::Y, Offset::new(-20.0, -250.0), "box1"),
    ];
    for (axis, delta, focused) in cases {
        let mut row = three_boxes(axis == Axis::Y);
        row.ui.send_indirect_pointer_input(axis, DEVICE, |scope| swipe_by(scope, delta, 3));
        row.ui.on_node_with_tag(focused).assert_is_focused();
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, swipeViaNavigation_swipeRightAndDoubleSwipeDownWithPrimaryAxisX_noBehavior, swipeViaNavigation_swipeLeftAndDoubleSwipeUpWithPrimaryAxisX_noBehavior, swipeViaNavigation_swipeDownWithPrimaryAxisX_noBehavior, swipeViaNavigation_swipeUpWithPrimaryAxisX_noBehavior, swipeViaNavigation_swipeDownAndDoubleSwipeRightWithPrimaryAxisY_noBehavior, swipeViaNavigation_swipeUpAndDoubleSwipeLeftWithPrimaryAxisY_noBehavior, swipeViaNavigation_swipeRightWithPrimaryAxisY_noBehavior, swipeViaNavigation_swipeLeftWithPrimaryAxisY_noBehavior, nonSwipeViaNavigation_minimalMoveRightWithPrimaryAxisX_noBehavior, nonSwipeViaNavigation_minimalMoveLeftWithPrimaryAxisX_noBehavior, nonSwipeViaNavigation_minimalMoveDownWithPrimaryAxisY_noBehavior, nonSwipeViaNavigation_minimalMoveUpWithPrimaryAxisY_noBehavior, swipeViaNavigation_swipeDownAndRightWithPrimaryAxisNone_noBehavior, swipeViaNavigation_swipeUpAndLeftWithPrimaryAxisNone_noBehavior, nonSwipeViaNavigation_minimalMoveDownAndRightWithPrimaryAxisNone_noBehavior, nonSwipeViaNavigation_minimalMoveUpAndLeftWithPrimaryAxisNone_noBehavior, swipeViaNavigation_swipeRightWithPrimaryAxisNone_noBehavior, swipeViaNavigation_swipeLeftWithPrimaryAxisNone_noBehavior, swipeViaNavigation_swipeDownWithPrimaryAxisNone_noBehavior, swipeViaNavigation_swipeUpWithPrimaryAxisNone_noBehavior
/// Across the axis, mostly across it, within the touch slop, or with no primary axis: the focus
/// stays and nothing is cancelled.
#[test]
fn other_swipes_leave_the_focus() {
    let cases = [
        (Axis::X, Offset::new(100.0, 200.0), 3),
        (Axis::X, Offset::new(-100.0, -200.0), 3),
        (Axis::X, Offset::new(0.0, 250.0), 3),
        (Axis::X, Offset::new(0.0, -250.0), 3),
        (Axis::Y, Offset::new(200.0, 100.0), 3),
        (Axis::Y, Offset::new(-200.0, -100.0), 3),
        (Axis::Y, Offset::new(600.0, 0.0), 3),
        (Axis::Y, Offset::new(-600.0, 0.0), 3),
        (Axis::X, Offset::new(5.0, 0.0), 1),
        (Axis::X, Offset::new(-5.0, 0.0), 1),
        (Axis::Y, Offset::new(0.0, 5.0), 1),
        (Axis::Y, Offset::new(0.0, -5.0), 1),
        (Axis::None, Offset::new(200.0, 200.0), 3),
        (Axis::None, Offset::new(-200.0, -200.0), 3),
        (Axis::None, Offset::new(5.0, 5.0), 1),
        (Axis::None, Offset::new(-5.0, -5.0), 1),
        (Axis::None, Offset::new(600.0, 0.0), 3),
        (Axis::None, Offset::new(-600.0, 0.0), 3),
        (Axis::None, Offset::new(0.0, 250.0), 3),
        (Axis::None, Offset::new(0.0, -250.0), 3),
    ];
    for (axis, delta, steps) in cases {
        let mut row = three_boxes(axis == Axis::Y);
        row.ui.send_indirect_pointer_input(axis, DEVICE, |scope| swipe_by(scope, delta, steps));
        row.ui.on_node_with_tag("box2").assert_is_focused();
        assert_eq!(row.cancels(), [0; 4], "{axis:?} {delta:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, swipeViaNavigation_noFocus_throwsAssertionError
#[test]
fn sending_without_a_focus_fails() {
    let mut row = three_boxes(false);
    row.ui.host().clear_focus();
    row.ui.wait_for_idle();
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        row.ui.send_indirect_pointer_input(Axis::X, DEVICE, |scope| scope.swipe_right());
    }));
    assert!(failed.is_err());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, swipeViaNavigation_cancelEvent_callsOnCancel
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/DelegatedIndirectPointerAndFocusEventTests.kt, downViaNavigationMotionEvent_moveFocusProgrammatically_triggersIndirectCancel, downAndMovesViaNavigationMotionEvent_moveFocusProgrammatically_triggersIndirectCancel, swipeViaNavigationMotionEvent_moveFocusProgrammatically_triggersIndirectCancel
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, downViaNavigation_moveFocusProgrammatically_triggersIndirectCancel, downAndMovesViaNavigation_moveFocusProgrammatically_triggersIndirectCancel, swipeViaNavigation_moveFocusProgrammatically_triggersIndirectCancel
/// A cancel tells the focus's path; so does the focus moving away during a stream (the box
/// it left, not the container).
#[test]
fn a_cancel_or_a_move_of_the_focus_ends_the_stream() {
    let mut row = three_boxes(false);
    row.ui.send_indirect_pointer_input(Axis::X, DEVICE, |scope| {
        let center = scope.input_device_center();
        scope.down(center);
        scope.cancel();
    });
    assert_eq!(row.cancels(), [1, 0, 1, 0]);
    for moves in [0, 2] {
        let mut row = three_boxes(false);
        row.ui.send_indirect_pointer_input(Axis::X, DEVICE, |scope| {
            let center = scope.input_device_center();
            scope.down(center);
            for _ in 0..moves {
                scope.move_by(Offset::new(1.0, 0.0));
            }
        });
        row.ui.on_node_with_tag("box3").request_focus();
        assert_eq!(row.cancels(), [0, 0, 1, 0], "{moves} moves");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/DelegatedIndirectPointerAndFocusEventTests.kt, noNavigationMotionEvent_clearsFocus_triggersIndirectCancel, noNavigationMotionEvent_moveFocusNextProgrammatically_triggersIndirectCancel, noNavigationMotionEvent_moveFocusProgrammaticallyWrapAround_triggersIndirectCancel
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, noNavigation_clearsFocus_triggersIndirectCancel, noNavigation_moveFocusNextProgrammatically_triggersIndirectCancel, noNavigation_moveFocusProgrammaticallyWrapAround_triggersIndirectCancel
/// Without a stream, the focus leaving a box still tells it: cleared, moved next, or around
/// from the last to the first. What stays around the focus isn't told, but for a move around.
#[test]
fn the_focus_leaving_tells_what_it_left() {
    let mut row = three_boxes(false);
    row.ui.host().clear_focus();
    row.ui.wait_for_idle();
    assert_eq!(row.cancels(), [1, 0, 1, 0], "cleared");
    let mut row = three_boxes(false);
    row.ui.host().with_tree(|tree| tree.move_focus_in(FocusDirection::Next));
    row.ui.wait_for_idle();
    row.ui.on_node_with_tag("box3").assert_is_focused();
    assert_eq!(row.cancels(), [0, 0, 1, 0], "next");
    row.ui.host().with_tree(|tree| tree.move_focus_in(FocusDirection::Next));
    row.ui.wait_for_idle();
    row.ui.on_node_with_tag("box1").assert_is_focused();
    // Around, the focus is cleared first, as Compose's: the container is told too.
    assert_eq!(row.cancels(), [1, 0, 1, 1], "around");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, simpleUI_removeIndirectModifierFocusedUiNode_triggersIndirectCancelsInFocusPath
/// An indirect node removed from the focused box is told its stream ended as it goes; the
/// focus stays, and what's around isn't told.
#[test]
fn a_node_removed_from_the_focus_s_path_is_told() {
    let (container, removed) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let show = compose_runtime::mutable_state_of(true);
    let handle = show.handle();
    let outer = keeping(&Rc::new(RefCell::new(None)), &container);
    let inner = keeping(&Rc::new(RefCell::new(None)), &removed);
    let mut ui = compose_test(move || {
        let inner = inner.clone();
        Column!(modifier = Modifier.fill_max_size().then_custom(outer.clone()), {
            let modifier = Modifier.test_tag("box1").size(100.dp());
            let modifier =
                if handle.get() { modifier.then_custom(inner.clone()) } else { modifier };
            Box!(modifier = modifier.focusable());
        });
    });
    ui.on_node_with_tag("box1").request_focus();
    assert_eq!((container.get(), removed.get()), (0, 0));
    ui.run_on_idle(|_| show.set(false));
    ui.wait_for_idle();
    ui.on_node_with_tag("box1").assert_is_focused();
    assert_eq!((container.get(), removed.get()), (0, 1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/indirect/IndirectPointerEventNavigationSystemTests.kt, simpleUI_detachAllUIWithNoFocus_noIndirectCancels, simpleUI_detachAllUINodesWithSomeFocused_triggersIndirectCancelsInFocusPath, simpleUI_requestFocusAndDetachAllUI_triggersIndirectCancelsInFocusPath
/// Removing the content tells only what was on the focus's path.
#[test]
fn removing_the_content_tells_the_focus_s_path() {
    for focused in [false, true] {
        let cancels: [Rc<Cell<u32>>; 3] = Default::default();
        let show = compose_runtime::mutable_state_of(true);
        let handle = show.handle();
        let elements: Vec<IndirectElement> =
            (0..3).map(|at| keeping(&Rc::new(RefCell::new(None)), &cancels[at])).collect();
        let mut ui = compose_test(move || {
            if handle.get() {
                let elements = elements.clone();
                Column!(modifier = Modifier.fill_max_size().then_custom(elements[0].clone()), {
                    for (at, element) in elements[1..].iter().enumerate() {
                        Box!(
                            modifier = Modifier
                                .test_tag(["box1", "box2"][at])
                                .size(100.dp())
                                .then_custom(element.clone())
                                .focusable()
                        );
                    }
                });
            }
        });
        if focused {
            ui.on_node_with_tag("box2").request_focus();
        }
        ui.run_on_idle(|_| show.set(false));
        ui.wait_for_idle();
        let told: Vec<u32> = cancels.iter().map(|it| it.get()).collect();
        let expected = if focused { [1, 0, 1] } else { [0, 0, 0] };
        assert_eq!(told, expected, "focused: {focused}");
    }
}

// ---- Injection: IndirectPointerInjectionScope ---------------------------------------------------

/// One change as the focused box heard it (the main pass): its event's type, the pointer, where,
/// whether down, when (ms since the block began).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Heard {
    kind: IndirectPointerEventType,
    id: u64,
    position: Offset,
    pressed: bool,
    millis: u64,
}

type Recorded = Rc<RefCell<Vec<Vec<Heard>>>>;

/// A focused box recording each event's changes, as Compose's `ClickableTestBox` with its
/// recorder, times from its first event's.
fn recording() -> (ComposeTestRule, Recorded, Rc<Cell<Option<u64>>>) {
    let recorded = Recorded::default();
    let start = Rc::new(Cell::new(None));
    let (events, since) = (recorded.clone(), start.clone());
    let element = IndirectElement(
        Rc::new(move |event, pass| {
            if pass != PointerEventPass::Main {
                return;
            }
            let changes = event
                .changes
                .iter()
                .map(|change| Heard {
                    kind: event.kind,
                    id: change.id.0,
                    position: change.position,
                    pressed: change.pressed,
                    millis: change.uptime_millis - first_time(&since, change.uptime_millis),
                })
                .collect();
            events.borrow_mut().push(changes);
        }),
        Rc::new(|| {}),
    );
    let mut ui = compose_test(move || {
        Box!(
            modifier =
                Modifier.then_custom(element.clone()).focusable().size(10.dp()).test_tag("box")
        );
    });
    ui.on_node_with_tag("box").request_focus();
    (ui, recorded, start)
}

/// The first event's time, `millis` if it's the first.
fn first_time(since: &Cell<Option<u64>>, millis: u64) -> u64 {
    *since.get().get_or_insert_with(|| {
        since.set(Some(millis));
        millis
    })
}

/// Runs `block` on the focused box, as Compose's `performIndirectPointerInput`, and returns what
/// it heard.
fn inject(block: impl FnOnce(&mut IndirectPointerInjectionScope<'_>)) -> Vec<Vec<Heard>> {
    let (mut ui, recorded, _) = recording();
    ui.on_node_with_tag("box").perform_indirect_pointer_input(Axis::X, DEVICE, block);
    recorded.take()
}

#[track_caller]
fn expect_error(action: impl FnOnce()) -> String {
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
    let panic = failed.expect_err("an error");
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()))
        .unwrap_or_default()
}

/// An injection that must fail saying `expected`.
#[track_caller]
fn inject_error(expected: &str, block: impl FnOnce(&mut IndirectPointerInjectionScope<'_>)) {
    let message = expect_error(|| {
        inject(block);
    });
    assert!(message.contains(expected), "{expected}: {message}");
}

const T: u64 = 16;

fn heard(
    kind: IndirectPointerEventType,
    id: u64,
    position: Offset,
    pressed: bool,
    millis: u64,
) -> Heard {
    Heard { kind, id, position, pressed, millis }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/ClickTest.kt, click
/// A click is a press, a move an event period later and a release, at the device's center by
/// default.
#[test]
fn a_click_is_a_press_a_move_and_a_release() {
    for position in [Some(Offset::new(10.0, 10.0)), Some(Offset::new(3072.0, 606.0)), None] {
        let events = inject(|scope| match position {
            Some(position) => scope.click_at(position),
            None => scope.click(),
        });
        let at = position.unwrap_or(Offset::new(1541.0, 308.0));
        assert_eq!(
            events,
            [
                vec![heard(Press, 0, at, true, 0)],
                vec![heard(Move, 0, at, true, T)],
                vec![heard(Release, 0, at, false, T)],
            ]
        );
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/DownTest.kt, onePointer, twoPointers, duplicatePointers
// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/IndirectPointerEventsTest.kt, onePointer_down, twoPointers_downDownMoveMove, enqueueIndirectPointerDown_afterDown
/// Each event tells every pointer down; a second press of a pointer down fails.
#[test]
fn pointers_go_down_by_id() {
    let (a, b) = (Offset::new(10.0, 20.0), Offset::new(30.0, 40.0));
    let events = inject(|scope| {
        scope.down_pointer(1, a);
        scope.down_pointer(2, b);
        assert_eq!((scope.current_position(1), scope.current_position(2)), (Some(a), Some(b)));
    });
    assert_eq!(
        events,
        [
            vec![heard(Press, 1, a, true, 0)],
            vec![heard(Press, 1, a, true, 0), heard(Press, 2, b, true, 0)],
        ]
    );
    inject_error("an indirect gesture is already in progress for pointer 1", |scope| {
        scope.down_pointer(1, a);
        scope.down_pointer(1, b);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/MoveToTest.kt, onePointer, twoPointers_separateMoveEvents, twoPointers_oneMoveEvent
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/MoveByTest.kt, onePointerSameInputBlock, onePointerDifferentInputBlocks, twoPointers, onePointer_oneMoveEvent, twoPointers_oneMoveEvent
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/CurrentPositionTest.kt, currentPosition_pointer0Down, currentPosition_pointer1Down, currentPosition_pointer0And1Down, currentPosition_pointerMoved
// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/IndirectPointerEventsTest.kt, onePointer_downUpdateMove, twoPointers_downMoveDownMove, twoPointers_moveSimultaneously, enqueueIndirectPointerDown_flushesPointerMovement, enqueueIndirectPointerUp_flushesPointerMovement
/// Moves update pointers and send them all in one event; a press or a release sends the moves
/// updated since first.
#[test]
fn pointers_move_together() {
    let (a, b) = (Offset::new(10.0, 10.0), Offset::new(50.0, 10.0));
    let step = Offset::new(5.0, 2.0);
    let events = inject(|scope| {
        scope.down_pointer(0, a);
        scope.down_pointer(1, b);
        scope.move_pointer_to(0, a + step, 2 * T);
        scope.move_pointer_by(1, step, T);
        assert_eq!(scope.current_position(0), Some(a + step));
        assert_eq!(scope.current_position(1), Some(b + step));
        scope.update_pointer_by(0, step);
        scope.up_pointer(1);
    });
    assert_eq!(events.len(), 6);
    assert_eq!(events[2], [heard(Move, 0, a + step, true, 2 * T), heard(Move, 1, b, true, 2 * T)]);
    assert_eq!(
        events[3],
        [heard(Move, 0, a + step, true, 3 * T), heard(Move, 1, b + step, true, 3 * T)]
    );
    // The update goes before the release.
    assert_eq!(events[4][0], heard(Move, 0, a + step * 2.0, true, 3 * T));
    assert_eq!(
        events[5],
        [heard(Release, 0, a + step * 2.0, true, 3 * T), heard(Release, 1, b + step, false, 3 * T)]
    );
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/UpTest.kt, up_withoutDown, up_wrongPointerId, up_afterUp, up_afterCancel
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/MoveByTest.kt, moveBy_withoutDown, moveBy_wrongPointerId, moveBy_afterUp, moveBy_afterCancel, updatePointerBy_withoutDown, updatePointerBy_wrongPointerId, updatePointerBy_afterUp, updatePointerBy_afterCancel
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/MoveToTest.kt, moveTo_withoutDown, moveTo_wrongPointerId, moveTo_afterUp, moveTo_afterCancel, updatePointerTo_withoutDown, updatePointerTo_wrongPointerId, updatePointerTo_afterUp, updatePointerTo_afterCancel
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/MoveTest.kt, move_withoutDown, move_afterUp, move_afterCancel
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/CancelTest.kt, cancel_withoutDown, cancel_afterUp, cancel_afterCancel
// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/IndirectPointerEventsTest.kt, updateIndirectPointerPointer_withoutDown, updateIndirectPointerPointer_wrongPointerId, updateIndirectPointerPointer_afterUp, updateIndirectPointerPointer_afterCancel, enqueueIndirectPointerMove_withoutDown, enqueueIndirectPointerMove_afterUp, enqueueIndirectPointerMove_afterCancel, enqueueIndirectPointerUp_withoutDown, enqueueIndirectPointerUp_wrongPointerId, enqueueIndirectPointerUp_afterUp, enqueueIndirectPointerUp_afterCancel, enqueueIndirectPointerCancel_withoutDown, enqueueIndirectPointerCancel_afterUp, enqueueIndirectPointerCancel_afterCancel
/// What can't be sent says why.
#[test]
fn what_cannot_be_sent_says_why() {
    const AT: Offset = Offset::new(10.0, 10.0);
    type Block = dyn Fn(&mut IndirectPointerInjectionScope<'_>);
    let cases: [(&str, &Block); 12] = [
        ("Cannot send UP event, no gesture is in progress", &|scope| scope.up()),
        ("Cannot send UP event for pointer 1, it is not active", &|scope| {
            scope.down(AT);
            scope.up_pointer(1);
        }),
        ("Cannot send UP event, no gesture is in progress", &|scope| {
            scope.down(AT);
            scope.up();
            scope.up();
        }),
        ("Cannot send UP event, no gesture is in progress", &|scope| {
            scope.down(AT);
            scope.cancel();
            scope.up();
        }),
        ("Cannot send MOVE event, no gesture is in progress", &|scope| scope.move_pointers()),
        ("Cannot move pointers, no gesture is in progress", &|scope| {
            scope.down(AT);
            scope.up();
            scope.move_to(AT);
        }),
        ("Cannot move pointer 1, it is not active in the current gesture", &|scope| {
            scope.down(AT);
            scope.update_pointer_to(1, AT);
        }),
        ("Cannot move pointers, no gesture is in progress", &|scope| {
            scope.down(AT);
            scope.cancel();
            scope.update_pointer_to(0, AT);
        }),
        ("Cannot send CANCEL event, no gesture is in progress", &|scope| scope.cancel()),
        ("Cannot send CANCEL event, no gesture is in progress", &|scope| {
            scope.down(AT);
            scope.up();
            scope.cancel();
        }),
        ("Cannot send CANCEL event, no gesture is in progress", &|scope| {
            scope.down(AT);
            scope.cancel();
            scope.cancel();
        }),
        ("Cannot send MOVE event, no gesture is in progress", &|scope| {
            scope.down(AT);
            scope.up();
            scope.move_pointers();
        }),
    ];
    for (expected, block) in cases {
        inject_error(expected, block);
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/CancelTest.kt, onePointer, twoPointers
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/CurrentPositionTest.kt, currentPosition_noPointersDown, currentPosition_pointerUp, currentPosition_pointerCancel
// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/IndirectPointerEventsTest.kt, onePointer_downCancel, twoPointers_downDownCancel
// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/IsIndirectPointerGestureInProgressTest.kt, downUp, downCancel
/// A cancel ends the gesture, an event period on, without an event: the focus's path is told,
/// and no pointer is down after it, nor after the last release.
#[test]
fn a_cancel_ends_the_gesture() {
    let cancels = Rc::new(Cell::new(0));
    let element = keeping(&Rc::new(RefCell::new(None)), &cancels);
    let mut ui = focused_box(move || Modifier.then_custom(element.clone()));
    let start = ui.current_time_millis();
    ui.send_indirect_pointer_input(Axis::X, DEVICE, |scope| {
        scope.down_pointer(0, Offset::new(1.0, 1.0));
        scope.down_pointer(1, Offset::new(2.0, 2.0));
        scope.cancel();
        assert_eq!((scope.current_position(0), scope.current_position(1)), (None, None));
        scope.down(Offset::new(1.0, 1.0));
        scope.up();
        assert_eq!(scope.current_position(0), None);
    });
    assert_eq!(cancels.get(), 1);
    assert!(ui.current_time_millis() >= start + T);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/DoubleClickTest.kt, doubleClick
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/LongClickTest.kt, longClick
/// A double click is two clicks halfway through the double-tap window apart; a long click a
/// press held 100 ms past the long-press timeout.
#[test]
fn double_and_long_clicks() {
    let configuration = ViewConfiguration::at(Density::new(1.0));
    let delay =
        (configuration.double_tap_min_time_millis + configuration.double_tap_timeout_millis) / 2;
    let presses: Vec<u64> = inject(|scope| scope.double_click())
        .iter()
        .filter(|event| event[0].kind == Press)
        .map(|event| event[0].millis)
        .collect();
    assert_eq!(presses, [0, T + delay]);
    let events = inject(|scope| scope.long_click());
    let release = events.last().unwrap()[0];
    assert_eq!(
        (release.kind, release.millis),
        (Release, configuration.long_press_timeout_millis + 100)
    );
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/SwipeStartEndTest.kt, swipe
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/SwipeCurveTest.kt, swipe
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/SwipeCurveWithKeyTimesTest.kt, swipe
/// A swipe moves about every event period to its end at its duration, through the key times.
#[test]
fn swipes_follow_their_curve() {
    let (start, end) = (Offset::new(100.0, 100.0), Offset::new(300.0, 200.0));
    let events = inject(|scope| scope.swipe(start, end, 400));
    assert_eq!(events[0][0], heard(Press, 0, start, true, 0));
    let last = events.last().unwrap()[0];
    assert_eq!((last.kind, last.position, last.millis), (Release, end, 400));
    let moves: Vec<u64> =
        events.iter().filter(|it| it[0].kind == Move).map(|it| it[0].millis).collect();
    assert_eq!(moves.len(), 25);
    assert!(moves.windows(2).all(|pair| pair[1] - pair[0] <= 17));
    let curve = |t: u64| Offset::new(100.0 + t as f32, 100.0);
    let events = inject(|scope| scope.swipe_along(&curve, 100, &[25, 50]));
    let times: Vec<u64> =
        events.iter().filter(|it| it[0].kind == Move).map(|it| it[0].millis).collect();
    assert!(times.contains(&25) && times.contains(&50), "{times:?}");
    assert!(events.iter().all(|it| it[0].position == curve(it[0].millis)));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/SwipeMultiTouchTest.kt, test
// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/PinchTest.kt, pinch
/// Several fingers swipe together; a pinch moves two from their starts to their ends.
#[test]
fn several_fingers_swipe_together() {
    let (s0, e0, s1, e1) = (
        Offset::new(100.0, 100.0),
        Offset::new(50.0, 100.0),
        Offset::new(200.0, 100.0),
        Offset::new(250.0, 100.0),
    );
    let events = inject(|scope| scope.pinch(s0, e0, s1, e1, 400));
    assert_eq!(events[1], [heard(Press, 0, s0, true, 0), heard(Press, 1, s1, true, 0)]);
    let moves: Vec<&Vec<Heard>> = events.iter().filter(|it| it[0].kind == Move).collect();
    assert!(moves.iter().all(|it| it.len() == 2));
    let last_move = moves.last().unwrap();
    assert_eq!((last_move[0].position, last_move[1].position), (e0, e1));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/SwipeDirectionTest.kt, swipeUp, swipeDown, swipeLeft, swipeRight, swipeUp_withParameters, swipeDown_withParameters, swipeLeft_withParameters, swipeRight_withParameters, swipeUp_wrongParameters, swipeDown_wrongParameters, swipeLeft_wrongParameters, swipeRight_wrongParameters
/// The directional swipes cross the device's middle edge to edge (or between the positions
/// given), and refuse backward positions.
#[test]
fn directional_swipes_cross_the_device() {
    let ends = |block: &dyn Fn(&mut IndirectPointerInjectionScope<'_>)| {
        let events = inject(block);
        (events[0][0].position, events.last().unwrap()[0].position)
    };
    let (cx, cy, right, bottom) = (1541.0, 308.0, 3081.0, 615.0);
    assert_eq!(ends(&|s| s.swipe_up()), (Offset::new(cx, bottom), Offset::new(cx, 0.0)));
    assert_eq!(ends(&|s| s.swipe_down()), (Offset::new(cx, 0.0), Offset::new(cx, bottom)));
    assert_eq!(ends(&|s| s.swipe_left()), (Offset::new(right, cy), Offset::new(0.0, cy)));
    assert_eq!(ends(&|s| s.swipe_right()), (Offset::new(0.0, cy), Offset::new(right, cy)));
    assert_eq!(
        ends(&|s| s.swipe_up_with(500.0, 100.0, 300)),
        (Offset::new(cx, 500.0), Offset::new(cx, 100.0))
    );
    assert_eq!(
        ends(&|s| s.swipe_down_with(100.0, 500.0, 300)),
        (Offset::new(cx, 100.0), Offset::new(cx, 500.0))
    );
    assert_eq!(
        ends(&|s| s.swipe_left_with(900.0, 100.0, 300)),
        (Offset::new(900.0, cy), Offset::new(100.0, cy))
    );
    assert_eq!(
        ends(&|s| s.swipe_right_with(100.0, 900.0, 300)),
        (Offset::new(100.0, cy), Offset::new(900.0, cy))
    );
    inject_error("needs to be greater than or equal to", |s| s.swipe_up_with(100.0, 500.0, 300));
    inject_error("needs to be less than or equal to", |s| s.swipe_down_with(500.0, 100.0, 300));
    inject_error("needs to be greater than or equal to", |s| s.swipe_left_with(100.0, 900.0, 300));
    inject_error("needs to be less than or equal to", |s| s.swipe_right_with(900.0, 100.0, 300));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/SwipeWithVelocityTest.kt, swipeWithVelocity
/// A swipe with velocity ends where and when asked, at about that velocity.
#[test]
fn a_swipe_with_velocity_ends_at_its_velocity() {
    let (start, end) = (Offset::new(100.0, 300.0), Offset::new(1100.0, 300.0));
    let events = inject(|scope| scope.swipe_with_velocity(start, end, 2000.0, Some(500)));
    let last = events.last().unwrap()[0];
    assert_eq!((last.position, last.millis), (end, 500));
    let mut tracker = compose_ui::VelocityTracker::new();
    for event in &events {
        tracker.add_position(event[0].millis as i64, event[0].position);
    }
    let velocity = tracker.calculate_velocity();
    assert!((velocity.x - 2000.0).abs() < 1.0 && velocity.y.abs() < 0.1, "{velocity:?}");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/injectionscope/indirecttouch/SynchronizedWithMainClockTest.kt, zeroTimeBetween_performTouchInput, someTimeBetween_performTouchInput
/// Events follow the main clock across blocks.
#[test]
fn events_follow_the_main_clock() {
    let (mut ui, recorded, _) = recording();
    let at = Offset::new(10.0, 10.0);
    ui.send_indirect_pointer_input(Axis::X, DEVICE, |scope| scope.down(at));
    ui.advance_time_by(std::time::Duration::from_millis(100));
    ui.send_indirect_pointer_input(Axis::X, DEVICE, |scope| scope.up());
    let events = recorded.take();
    assert!(events[1][0].millis >= events[0][0].millis + 100, "{events:?}");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/IndirectPointerValidationRoundingTest.kt, validatePosition_allowsTinyNegativeXCoordinate, validatePosition_allowsTinyPositiveXCoordinate, validatePosition_allowsTinyNegativeYCoordinate, validatePosition_allowsTinyPositiveYCoordinate, validatePosition_throwsWhenXCoordinateTooNegative, validatePosition_throwsWhenXCoordinateTooPositive, validatePosition_throwsWhenYCoordinateTooNegative, validatePosition_throwsWhenYCoordinateTooPositive
/// Positions must be on the device, a little rounding allowed.
#[test]
fn positions_must_be_on_the_device() {
    let size = IntSize::new(100, 100);
    for position in [
        Offset::new(-0.0000001, 0.0),
        Offset::new(100.001, 0.0),
        Offset::new(0.0, -0.0000001),
        Offset::new(0.0, 100.001),
    ] {
        let (mut ui, _, _) = recording();
        ui.send_indirect_pointer_input(Axis::X, size, |scope| scope.down_pointer(0, position));
    }
    for position in [
        Offset::new(-1.0, 0.0),
        Offset::new(101.0, 0.0),
        Offset::new(0.0, -1.0),
        Offset::new(0.0, 101.0),
    ] {
        let (mut ui, _, _) = recording();
        let message = expect_error(|| {
            ui.send_indirect_pointer_input(Axis::X, size, |scope| scope.down_pointer(0, position));
        });
        assert!(
            message.contains("is outside of the indirect pointer input device bounds"),
            "{message}"
        );
    }
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/inputdispatcher/IndirectPointerEventsTest.kt, twoPointers_downUp_sameOrder, twoPointers_downUp_inverseOrder, threePointers_notSimultaneously, threePointers_pointerIdReuse, fourPointers_downOnly
/// Pointers lift in any order, each release telling those still down; an id lifted can come
/// down again.
#[test]
fn pointers_lift_in_any_order() {
    let at = |x: f32| Offset::new(x, 10.0);
    let events = inject(|scope| {
        scope.down_pointer(0, at(1.0));
        scope.down_pointer(1, at(2.0));
        scope.down_pointer(2, at(3.0));
        scope.up_pointer(1);
        scope.down_pointer(1, at(4.0));
        scope.up_pointer(0);
        scope.up_pointer(2);
        scope.up_pointer(1);
    });
    let summary: Vec<Vec<(u64, bool)>> =
        events.iter().map(|event| event.iter().map(|it| (it.id, it.pressed)).collect()).collect();
    assert_eq!(
        summary,
        [
            vec![(0, true)],
            vec![(0, true), (1, true)],
            vec![(0, true), (1, true), (2, true)],
            vec![(0, true), (1, false), (2, true)],
            vec![(0, true), (1, true), (2, true)],
            vec![(0, false), (1, true), (2, true)],
            vec![(1, true), (2, false)],
            vec![(1, false)],
        ]
    );
}

/// Moves the focus to the next target when it hears a press (its main pass).
#[derive(Clone)]
struct MoverElement;

struct MoverNode;

impl ModifierNodeElement for MoverElement {
    type Node = MoverNode;
    fn create(&self) -> MoverNode {
        MoverNode
    }
    fn update(&self, _node: &mut MoverNode) {}
    fn same(&self, _other: &Self) -> bool {
        true
    }
}

impl ModifierNode for MoverNode {
    fn as_indirect_pointer_input(&mut self) -> Option<&mut dyn IndirectPointerInputModifierNode> {
        Some(self)
    }
}

impl IndirectPointerInputModifierNode for MoverNode {
    fn on_indirect_pointer_event(
        &mut self,
        event: &IndirectPointerEvent,
        pass: PointerEventPass,
        context: &mut EventContext<'_>,
    ) {
        if pass == PointerEventPass::Main && event.kind == Press {
            context.move_focus(true);
        }
    }
    fn on_cancel_indirect_pointer_input(&mut self) {}
}

// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/focus/FocusOwnerImpl.kt, dispatchIndirectPointerEvent (a node moving the focus as it hears an event; no test of its own)
/// A node moving the focus as it hears the event: the focus moves and the focus callbacks run,
/// with no fling.
#[test]
fn a_node_can_move_the_focus_as_it_hears_an_event() {
    let told: Rc<RefCell<Vec<(&'static str, bool)>>> = Rc::default();
    let seen = told.clone();
    let mut ui = compose_test(move || {
        let seen = seen.clone();
        Column!({
            for (at, tag) in ["first", "second"].into_iter().enumerate() {
                let seen = seen.clone();
                let modifier = Modifier.test_tag(tag).size(10.dp()).on_focus_changed(
                    move |state: compose_ui::FocusState| {
                        seen.borrow_mut().push((tag, state.is_focused()))
                    },
                );
                let modifier = if at == 0 { modifier.then_custom(MoverElement) } else { modifier };
                Box!(modifier = modifier.focusable());
            }
        });
    });
    ui.on_node_with_tag("first").request_focus();
    told.borrow_mut().clear();
    send(&mut ui, event_at(Press, Offset::ZERO, 0));
    ui.on_node_with_tag("second").assert_is_focused();
    assert_eq!(*told.borrow(), [("first", false), ("second", true)]);
}
