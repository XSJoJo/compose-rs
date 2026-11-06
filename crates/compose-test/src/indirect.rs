//! Indirect pointer injection, as Compose's `performIndirectPointerInput` and
//! `sendIndirectPointerInput` with `IndirectPointerInjectionScope`: the touch scope's gestures on a
//! touch-navigation device, positions in the device's coordinates, the events going to what has
//! the focus (D1675). Each event tells every pointer down, as a `MotionEvent` does.

use std::collections::BTreeMap;
use std::time::Duration;

use compose_ui::{
    Density, IndirectPointerEvent, IndirectPointerEventPrimaryDirectionalMotionAxis,
    IndirectPointerEventType, IndirectPointerInputChange, IntSize, Offset, PointerId,
    ViewConfiguration,
};

use crate::injection::{
    DEFAULT_SWIPE_MILLIS, EVENT_PERIOD_MILLIS, VelocityPath, default_double_tap_delay_millis,
    default_velocity_duration, lerp, lerp_time,
};
use crate::interaction::SemanticsNodeInteraction;
use crate::rule::ComposeTestRule;

type Axis = IndirectPointerEventPrimaryDirectionalMotionAxis;

/// An indirect gesture in progress, as Compose's `PartialIndirectGesture`: kept across blocks.
pub(crate) struct IndirectGesture {
    /// The pointers down, by id: where each is, and where it was at the last event.
    pointers: BTreeMap<u64, (Offset, Offset)>,
    /// A pointer moved since the last event.
    updated: bool,
    /// The axis its press said.
    axis: Axis,
    /// The last event's time (ms).
    last_time: u64,
}

/// Indirect pointer gestures, as Compose's `IndirectPointerInjectionScope`: fingers by pointer id
/// (0 the first) on a device of `input_device_size`, positions in its coordinates, each event sent
/// to what has the focus.
pub struct IndirectPointerInjectionScope<'r> {
    rule: &'r mut ComposeTestRule,
    input_device_size: IntSize,
    axis: Axis,
    density: Density,
    view_configuration: ViewConfiguration,
}

impl<'r> IndirectPointerInjectionScope<'r> {
    fn new(
        rule: &'r mut ComposeTestRule,
        axis: Axis,
        input_device_size: IntSize,
    ) -> IndirectPointerInjectionScope<'r> {
        let (density, view_configuration) =
            rule.host.with_tree(|tree| (tree.density(), tree.view_configuration()));
        IndirectPointerInjectionScope { rule, input_device_size, axis, density, view_configuration }
    }

    /// The time between two events by default, as `eventPeriodMillis` (16 ms).
    pub fn event_period_millis(&self) -> u64 {
        EVENT_PERIOD_MILLIS
    }

    /// Moves the event time (and the test clock) on, as `advanceEventTime`.
    pub fn advance_event_time(&mut self, duration_millis: u64) {
        if duration_millis > 0 {
            self.rule
                .advance_time_by_ignoring_frame_duration(Duration::from_millis(duration_millis));
        }
    }

    pub fn density(&self) -> Density {
        self.density
    }

    pub fn view_configuration(&self) -> ViewConfiguration {
        self.view_configuration
    }

    /// The device's size, as `inputDeviceSize`.
    pub fn input_device_size(&self) -> IntSize {
        self.input_device_size
    }

    /// The device's primary axis, as `indirectPointerEventPrimaryDirectionalMotionAxis`.
    pub fn primary_directional_motion_axis(&self) -> Axis {
        self.axis
    }

    pub fn input_device_width(&self) -> i32 {
        self.input_device_size.width
    }

    pub fn input_device_height(&self) -> i32 {
        self.input_device_size.height
    }

    pub fn input_device_left(&self) -> f32 {
        0.0
    }

    pub fn input_device_top(&self) -> f32 {
        0.0
    }

    pub fn input_device_center_x(&self) -> f32 {
        self.input_device_width() as f32 / 2.0
    }

    pub fn input_device_center_y(&self) -> f32 {
        self.input_device_height() as f32 / 2.0
    }

    /// The last column, as `inputDeviceRight` (width − 1).
    pub fn input_device_right(&self) -> f32 {
        match self.input_device_width() {
            0 => 0.0,
            width => width as f32 - 1.0,
        }
    }

    pub fn input_device_bottom(&self) -> f32 {
        match self.input_device_height() {
            0 => 0.0,
            height => height as f32 - 1.0,
        }
    }

    pub fn input_device_top_left(&self) -> Offset {
        Offset::new(self.input_device_left(), self.input_device_top())
    }

    pub fn input_device_top_center(&self) -> Offset {
        Offset::new(self.input_device_center_x(), self.input_device_top())
    }

    pub fn input_device_top_right(&self) -> Offset {
        Offset::new(self.input_device_right(), self.input_device_top())
    }

    pub fn input_device_center_left(&self) -> Offset {
        Offset::new(self.input_device_left(), self.input_device_center_y())
    }

    pub fn input_device_center(&self) -> Offset {
        Offset::new(self.input_device_center_x(), self.input_device_center_y())
    }

    pub fn input_device_center_right(&self) -> Offset {
        Offset::new(self.input_device_right(), self.input_device_center_y())
    }

    pub fn input_device_bottom_left(&self) -> Offset {
        Offset::new(self.input_device_left(), self.input_device_bottom())
    }

    pub fn input_device_bottom_center(&self) -> Offset {
        Offset::new(self.input_device_center_x(), self.input_device_bottom())
    }

    pub fn input_device_bottom_right(&self) -> Offset {
        Offset::new(self.input_device_right(), self.input_device_bottom())
    }

    fn gesture(&mut self) -> &mut Option<IndirectGesture> {
        &mut self.rule.input.indirect
    }

    /// Positions on the device only, a little rounding allowed, as Compose's `validatePosition`.
    #[track_caller]
    fn validate(&self, position: Offset) {
        let (width, height) =
            (self.input_device_size.width as f32, self.input_device_size.height as f32);
        let epsilon = 0.01;
        let within = |value: f32, end: f32| value >= -epsilon && value <= end + epsilon;
        assert!(
            within(position.x, width) && within(position.y, height),
            "compose-test: Position ({}, {}) is outside of the indirect pointer input device \
             bounds {} x {}",
            position.x,
            position.y,
            self.input_device_size.width,
            self.input_device_size.height
        );
    }

    /// Where pointer `id` is, if it's down, as `currentPosition`.
    pub fn current_position(&self, id: u64) -> Option<Offset> {
        let gesture = self.rule.input.indirect.as_ref()?;
        gesture.pointers.get(&id).map(|(position, _)| *position)
    }

    /// Puts the first finger down at `position`, as `down(position)`.
    #[track_caller]
    pub fn down(&mut self, position: Offset) {
        self.down_pointer(0, position);
    }

    /// Puts finger `id` down at `position`, as `down(pointerId, position)`: a touch gesture is
    /// cancelled, a move of the fingers updated since goes first.
    #[track_caller]
    pub fn down_pointer(&mut self, id: u64, position: Offset) {
        self.validate(position);
        assert!(
            self.gesture().as_ref().is_none_or(|gesture| !gesture.pointers.contains_key(&id)),
            "compose-test: Cannot send DOWN event, an indirect gesture is already in progress \
             for pointer {id}"
        );
        crate::injection::cancel_touch_gesture(self.rule);
        if self.gesture().as_ref().is_some_and(|gesture| gesture.updated) {
            self.send(IndirectPointerEventType::Move, None);
        }
        let axis = self.axis;
        let gesture = self.gesture().get_or_insert_with(|| IndirectGesture {
            pointers: BTreeMap::new(),
            updated: false,
            axis,
            last_time: 0,
        });
        gesture.pointers.insert(id, (position, position));
        self.send(IndirectPointerEventType::Press, Some(id));
    }

    /// Moves finger `id` to `position` without sending an event, as `updatePointerTo`.
    #[track_caller]
    pub fn update_pointer_to(&mut self, id: u64, position: Offset) {
        self.validate(position);
        let Some(gesture) = self.gesture() else {
            panic!("compose-test: Cannot move pointers, no gesture is in progress")
        };
        let Some(pointer) = gesture.pointers.get_mut(&id) else {
            panic!(
                "compose-test: Cannot move pointer {id}, it is not active in the current gesture"
            )
        };
        pointer.0 = position;
        gesture.updated = true;
    }

    /// Moves finger `id` by `delta` without sending an event, as `updatePointerBy`.
    #[track_caller]
    pub fn update_pointer_by(&mut self, id: u64, delta: Offset) {
        let current = self.current_position(id).unwrap_or(Offset::ZERO);
        self.update_pointer_to(id, current + delta);
    }

    /// Sends a move of every finger down, an event period on, as `move()`.
    #[track_caller]
    pub fn move_pointers(&mut self) {
        self.move_pointers_after(EVENT_PERIOD_MILLIS);
    }

    /// As `move(delayMillis)`.
    #[track_caller]
    pub fn move_pointers_after(&mut self, delay_millis: u64) {
        self.advance_event_time(delay_millis);
        assert!(
            self.gesture().is_some(),
            "compose-test: Cannot send MOVE event, no gesture is in progress"
        );
        crate::injection::cancel_touch_gesture(self.rule);
        self.send(IndirectPointerEventType::Move, None);
    }

    /// Moves the first finger to `position`, an event period on, as `moveTo(position)`.
    #[track_caller]
    pub fn move_to(&mut self, position: Offset) {
        self.move_pointer_to(0, position, EVENT_PERIOD_MILLIS);
    }

    /// As `moveTo(pointerId, position, delayMillis)`.
    #[track_caller]
    pub fn move_pointer_to(&mut self, id: u64, position: Offset, delay_millis: u64) {
        self.update_pointer_to(id, position);
        self.move_pointers_after(delay_millis);
    }

    /// Moves the first finger by `delta`, an event period on, as `moveBy(delta)`.
    #[track_caller]
    pub fn move_by(&mut self, delta: Offset) {
        self.move_pointer_by(0, delta, EVENT_PERIOD_MILLIS);
    }

    /// As `moveBy(pointerId, delta, delayMillis)`.
    #[track_caller]
    pub fn move_pointer_by(&mut self, id: u64, delta: Offset, delay_millis: u64) {
        self.update_pointer_by(id, delta);
        self.move_pointers_after(delay_millis);
    }

    /// Lifts the first finger, as `up()`.
    #[track_caller]
    pub fn up(&mut self) {
        self.up_pointer(0);
    }

    /// Lifts finger `id`, as `up(pointerId)`: a move of the fingers updated since goes first.
    #[track_caller]
    pub fn up_pointer(&mut self, id: u64) {
        let Some(gesture) = self.gesture() else {
            panic!("compose-test: Cannot send UP event, no gesture is in progress")
        };
        assert!(
            gesture.pointers.contains_key(&id),
            "compose-test: Cannot send UP event for pointer {id}, it is not active in the \
             current gesture"
        );
        crate::injection::cancel_touch_gesture(self.rule);
        if self.gesture().as_ref().is_some_and(|gesture| gesture.updated) {
            self.send(IndirectPointerEventType::Move, None);
        }
        self.send(IndirectPointerEventType::Release, Some(id));
        let gesture = self.gesture().as_mut().expect("in progress");
        gesture.pointers.remove(&id);
        if gesture.pointers.is_empty() {
            *self.gesture() = None;
        }
    }

    /// Cancels the gesture, an event period on, as `cancel()`.
    #[track_caller]
    pub fn cancel(&mut self) {
        self.cancel_after(EVENT_PERIOD_MILLIS);
    }

    /// As `cancel(delayMillis)`: what has the focus hears the stream end.
    #[track_caller]
    pub fn cancel_after(&mut self, delay_millis: u64) {
        self.advance_event_time(delay_millis);
        assert!(
            self.gesture().take().is_some(),
            "compose-test: Cannot send CANCEL event, no gesture is in progress"
        );
        self.rule.host.cancel_indirect_pointer();
        self.rule.run_current();
    }

    /// One event of every pointer down, `changed` the one coming down (`Press`) or up
    /// (`Release`), at the current time.
    fn send(&mut self, kind: IndirectPointerEventType, changed: Option<u64>) {
        let now = compose_runtime::platform::now_nanos() / 1_000_000;
        let gesture = self.rule.input.indirect.as_mut().expect("a gesture in progress");
        let (last_time, axis) = (gesture.last_time, gesture.axis);
        let changes = gesture
            .pointers
            .iter_mut()
            .map(|(id, (position, sent))| {
                let this = changed == Some(*id);
                let new = this && kind == IndirectPointerEventType::Press;
                let pressed = !(this && kind == IndirectPointerEventType::Release);
                let change = IndirectPointerInputChange::new(
                    PointerId(*id),
                    now,
                    *position,
                    pressed,
                    1.0,
                    if new { now } else { last_time },
                    if new { *position } else { *sent },
                    !new,
                );
                *sent = *position;
                change
            })
            .collect();
        gesture.last_time = now;
        gesture.updated = false;
        let event = IndirectPointerEvent::new(changes, kind, axis);
        self.rule.host.send_indirect_pointer(event);
        self.rule.run_current();
    }

    // ---- Gestures -------------------------------------------------------------------------

    /// A tap at the device's center: down, a move an event period later, up, as `click()`.
    pub fn click(&mut self) {
        let center = self.input_device_center();
        self.click_at(center);
    }

    /// As `click(position)`.
    #[track_caller]
    pub fn click_at(&mut self, position: Offset) {
        self.down(position);
        self.move_pointers();
        self.up();
    }

    /// A press 100 ms past the long-press timeout at the center, as `longClick()`.
    pub fn long_click(&mut self) {
        let center = self.input_device_center();
        let duration = self.view_configuration.long_press_timeout_millis + 100;
        self.long_click_at(center, duration);
    }

    /// As `longClick(position, durationMillis)`.
    #[track_caller]
    pub fn long_click_at(&mut self, position: Offset, duration_millis: u64) {
        let timeout = self.view_configuration.long_press_timeout_millis;
        assert!(
            duration_millis >= timeout,
            "compose-test: Long click must have a duration of at least {timeout}ms"
        );
        self.swipe(position, position, duration_millis);
    }

    /// Two taps at the center, as `doubleClick()`.
    pub fn double_click(&mut self) {
        let center = self.input_device_center();
        let delay = default_double_tap_delay_millis(&self.view_configuration);
        self.double_click_at(center, delay);
    }

    /// As `doubleClick(position, delayMillis)`.
    #[track_caller]
    pub fn double_click_at(&mut self, position: Offset, delay_millis: u64) {
        let configuration = self.view_configuration;
        assert!(
            delay_millis >= configuration.double_tap_min_time_millis,
            "compose-test: Time between clicks in double click must be at least {}ms",
            configuration.double_tap_min_time_millis
        );
        assert!(
            delay_millis < configuration.double_tap_timeout_millis,
            "compose-test: Time between clicks in double click must be smaller than {}ms",
            configuration.double_tap_timeout_millis
        );
        self.click_at(position);
        self.advance_event_time(delay_millis);
        self.click_at(position);
    }

    /// A straight swipe, as `swipe(start, end, durationMillis)`.
    pub fn swipe(&mut self, start: Offset, end: Offset, duration_millis: u64) {
        let duration = duration_millis as f32;
        self.swipe_along(&|time| lerp(start, end, time as f32 / duration), duration_millis, &[]);
    }

    /// As `swipe(curve, durationMillis, keyTimes)`.
    pub fn swipe_along(
        &mut self,
        curve: &dyn Fn(u64) -> Offset,
        duration_millis: u64,
        key_times: &[u64],
    ) {
        self.multi_touch_swipe(&[curve], duration_millis, key_times);
    }

    /// A finger per curve, as `multiTouchSwipe`.
    #[track_caller]
    pub fn multi_touch_swipe(
        &mut self,
        curves: &[&dyn Fn(u64) -> Offset],
        duration_millis: u64,
        key_times: &[u64],
    ) {
        assert!(duration_millis >= 1, "compose-test: duration must be at least 1 millisecond");
        assert!(
            key_times.iter().all(|time| *time <= duration_millis),
            "compose-test: keyTimes contains timestamps out of range [0..{duration_millis}]: \
             {key_times:?}"
        );
        assert!(
            key_times.windows(2).all(|pair| pair[0] <= pair[1]),
            "compose-test: keyTimes must be sorted: {key_times:?}"
        );
        for (index, curve) in curves.iter().enumerate() {
            self.down_pointer(index as u64, curve(0));
        }
        let (mut time, mut key) = (0, 0);
        while time < duration_millis {
            while key < key_times.len() && key_times[key] <= time {
                key += 1;
            }
            let next = key_times.get(key).copied().unwrap_or(duration_millis);
            let steps = (((next - time) as f32 / EVENT_PERIOD_MILLIS as f32).round() as u64).max(1);
            let mut previous = time;
            for step in 1..=steps {
                let at = lerp_time(time, next, step as f32 / steps as f32);
                for (index, curve) in curves.iter().enumerate() {
                    self.update_pointer_to(index as u64, curve(at));
                }
                self.move_pointers_after(at - previous);
                previous = at;
            }
            time = next;
        }
        for index in 0..curves.len() {
            self.up_pointer(index as u64);
        }
    }

    /// Two fingers from `start0` and `start1` to `end0` and `end1`, as `pinch`.
    pub fn pinch(
        &mut self,
        start0: Offset,
        end0: Offset,
        start1: Offset,
        end1: Offset,
        duration_millis: u64,
    ) {
        let duration = duration_millis as f32;
        let first = move |time: u64| lerp(start0, end0, time as f32 / duration);
        let second = move |time: u64| lerp(start1, end1, time as f32 / duration);
        self.multi_touch_swipe(&[&first, &second], duration_millis, &[]);
    }

    /// A swipe ending at `end_velocity` px/s, as `swipeWithVelocity`.
    #[track_caller]
    pub fn swipe_with_velocity(
        &mut self,
        start: Offset,
        end: Offset,
        end_velocity: f32,
        duration_millis: Option<u64>,
    ) {
        assert!(
            end_velocity >= 0.0,
            "compose-test: Velocity cannot be {end_velocity}, it must be positive"
        );
        let duration =
            duration_millis.unwrap_or_else(|| default_velocity_duration(start, end, end_velocity));
        let minimum = (2.5 * EVENT_PERIOD_MILLIS as f32).ceil() as u64;
        assert!(
            duration >= minimum,
            "compose-test: Duration must be at least {minimum}ms because velocity requires at \
             least 3 input events"
        );
        let path = VelocityPath::new(start, end, end_velocity, duration);
        self.swipe_along(&|time| path.offset_at(time), duration, &[]);
    }

    /// A swipe up the device's middle, bottom to top, as `swipeUp()`.
    pub fn swipe_up(&mut self) {
        let (bottom, top) = (self.input_device_bottom(), self.input_device_top());
        self.swipe_up_with(bottom, top, DEFAULT_SWIPE_MILLIS);
    }

    /// As `swipeUp(startY, endY, durationMillis)`.
    #[track_caller]
    pub fn swipe_up_with(&mut self, start_y: f32, end_y: f32, duration_millis: u64) {
        assert!(
            start_y >= end_y,
            "compose-test: startY={start_y} needs to be greater than or equal to endY={end_y}"
        );
        let x = self.input_device_center_x();
        self.swipe(Offset::new(x, start_y), Offset::new(x, end_y), duration_millis);
    }

    /// A swipe down the device's middle, as `swipeDown()`.
    pub fn swipe_down(&mut self) {
        let (top, bottom) = (self.input_device_top(), self.input_device_bottom());
        self.swipe_down_with(top, bottom, DEFAULT_SWIPE_MILLIS);
    }

    /// As `swipeDown(startY, endY, durationMillis)`.
    #[track_caller]
    pub fn swipe_down_with(&mut self, start_y: f32, end_y: f32, duration_millis: u64) {
        assert!(
            start_y <= end_y,
            "compose-test: startY={start_y} needs to be less than or equal to endY={end_y}"
        );
        let x = self.input_device_center_x();
        self.swipe(Offset::new(x, start_y), Offset::new(x, end_y), duration_millis);
    }

    /// A swipe left across the device's middle, as `swipeLeft()`.
    pub fn swipe_left(&mut self) {
        let (right, left) = (self.input_device_right(), self.input_device_left());
        self.swipe_left_with(right, left, DEFAULT_SWIPE_MILLIS);
    }

    /// As `swipeLeft(startX, endX, durationMillis)`.
    #[track_caller]
    pub fn swipe_left_with(&mut self, start_x: f32, end_x: f32, duration_millis: u64) {
        assert!(
            start_x >= end_x,
            "compose-test: startX={start_x} needs to be greater than or equal to endX={end_x}"
        );
        let y = self.input_device_center_y();
        self.swipe(Offset::new(start_x, y), Offset::new(end_x, y), duration_millis);
    }

    /// A swipe right across the device's middle, as `swipeRight()`.
    pub fn swipe_right(&mut self) {
        let (left, right) = (self.input_device_left(), self.input_device_right());
        self.swipe_right_with(left, right, DEFAULT_SWIPE_MILLIS);
    }

    /// As `swipeRight(startX, endX, durationMillis)`.
    #[track_caller]
    pub fn swipe_right_with(&mut self, start_x: f32, end_x: f32, duration_millis: u64) {
        assert!(
            start_x <= end_x,
            "compose-test: startX={start_x} needs to be less than or equal to endX={end_x}"
        );
        let y = self.input_device_center_y();
        self.swipe(Offset::new(start_x, y), Offset::new(end_x, y), duration_millis);
    }
}

/// Ends an indirect gesture in progress: a touch's down cancels it, as Compose's dispatcher.
pub(crate) fn cancel_indirect_gesture(rule: &mut ComposeTestRule) {
    if rule.input.indirect.take().is_some() {
        rule.host.cancel_indirect_pointer();
    }
}

impl ComposeTestRule {
    /// Sends the indirect pointer gestures `block` makes to what has the focus, on a device of
    /// `input_device_size` along `axis`, then waits for idle, as `sendIndirectPointerInput`.
    pub fn send_indirect_pointer_input(
        &mut self,
        axis: Axis,
        input_device_size: IntSize,
        block: impl FnOnce(&mut IndirectPointerInjectionScope<'_>),
    ) -> &mut Self {
        assert!(
            self.host.with_tree(|tree| tree.focused().is_some()),
            "compose-test: Failed to send indirect pointer input: no node has the focus"
        );
        let mut scope = IndirectPointerInjectionScope::new(self, axis, input_device_size);
        block(&mut scope);
        self.wait_for_idle();
        self
    }
}

impl SemanticsNodeInteraction<'_> {
    /// As [`ComposeTestRule::send_indirect_pointer_input`], from this node (which must exist), as
    /// `performIndirectPointerInput`: the events still go to what has the focus.
    #[track_caller]
    pub fn perform_indirect_pointer_input(
        &mut self,
        axis: Axis,
        input_device_size: IntSize,
        block: impl FnOnce(&mut IndirectPointerInjectionScope<'_>),
    ) -> &mut Self {
        self.fetch_semantics_snapshot();
        self.rule.send_indirect_pointer_input(axis, input_device_size, block);
        self
    }
}
