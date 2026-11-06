//! Indirect pointer input, as Compose's `androidx.compose.ui.input.indirect`: a pointer on a device
//! that isn't over the content (a touch-navigation pad), whose events go to the focused node and its
//! ancestors, not to what's under them (D1675; design:
//! `docs/parity/design/trackpad-and-indirect-input.md`). A fling along the device's primary axis
//! that no node consumed moves the focus, as Android's touch navigation.

use std::cell::Cell;

use crate::focus_manager::FocusDirection;
use crate::input::{EventContext, PointerEventPass, PointerId};
use crate::pointer_event::HistoricalChange;
use crate::unit::{Offset, Velocity};
use crate::velocity_tracker::VelocityTracker;
use crate::view_configuration::ViewConfiguration;

/// Why an indirect pointer event was sent, as `IndirectPointerEventType`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IndirectPointerEventType {
    Unknown,
    /// A pointer came down.
    Press,
    /// A pointer went up.
    Release,
    Move,
}

impl std::fmt::Display for IndirectPointerEventType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            IndirectPointerEventType::Press => "Press",
            IndirectPointerEventType::Release => "Release",
            IndirectPointerEventType::Move => "Move",
            IndirectPointerEventType::Unknown => "Unknown",
        })
    }
}

/// The axis the device's movements mostly go along, which a fling along moves the focus on, as
/// `IndirectPointerEventPrimaryDirectionalMotionAxis`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum IndirectPointerEventPrimaryDirectionalMotionAxis {
    #[default]
    None,
    X,
    Y,
}

/// A change of one pointer of the device, as `IndirectPointerInputChange`: positions in the
/// device's coordinates. A copy (`clone`, as `copy`) is consumed if this one is, then on its own.
#[derive(Clone)]
pub struct IndirectPointerInputChange {
    pub id: PointerId,
    pub uptime_millis: u64,
    pub position: Offset,
    pub pressed: bool,
    pub pressure: f32,
    pub previous_uptime_millis: u64,
    pub previous_position: Offset,
    pub previous_pressed: bool,
    /// The moves since the last event, as `historical`.
    pub historical: Vec<HistoricalChange>,
    consumed: Cell<bool>,
}

impl IndirectPointerInputChange {
    /// As `IndirectPointerInputChange(id, uptimeMillis, position, pressed, pressure,
    /// previousUptimeMillis, previousPosition, previousPressed)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PointerId,
        uptime_millis: u64,
        position: Offset,
        pressed: bool,
        pressure: f32,
        previous_uptime_millis: u64,
        previous_position: Offset,
        previous_pressed: bool,
    ) -> IndirectPointerInputChange {
        IndirectPointerInputChange {
            id,
            uptime_millis,
            position,
            pressed,
            pressure,
            previous_uptime_millis,
            previous_position,
            previous_pressed,
            historical: Vec::new(),
            consumed: Cell::new(false),
        }
    }

    /// The same change with `historical` moves.
    pub fn with_historical(self, historical: Vec<HistoricalChange>) -> IndirectPointerInputChange {
        IndirectPointerInputChange { historical, ..self }
    }

    pub fn is_consumed(&self) -> bool {
        self.consumed.get()
    }

    /// Marks it used: nodes after this one should leave it, as `consume()`.
    pub fn consume(&self) {
        self.consumed.set(true);
    }
}

impl std::fmt::Debug for IndirectPointerInputChange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "IndirectPointerInputChange(id={:?}, uptimeMillis={}, position={:?}, pressed={}, \
             pressure={}, previousUptimeMillis={}, previousPosition={:?}, previousPressed={}, \
             historical={:?}, isConsumed={})",
            self.id,
            self.uptime_millis,
            self.position,
            self.pressed,
            self.pressure,
            self.previous_uptime_millis,
            self.previous_position,
            self.previous_pressed,
            self.historical,
            self.is_consumed()
        )
    }
}

/// One event of the device, as Compose's `IndirectPointerEvent`: its pointers' changes, why it was
/// sent (`kind`, as `type`) and the device's primary axis.
#[derive(Clone, Debug)]
pub struct IndirectPointerEvent {
    pub changes: Vec<IndirectPointerInputChange>,
    pub kind: IndirectPointerEventType,
    pub primary_directional_motion_axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
}

impl IndirectPointerEvent {
    /// An event of `changes` (at least one), as the test-only `IndirectPointerEvent(...)`.
    #[track_caller]
    pub fn new(
        changes: Vec<IndirectPointerInputChange>,
        kind: IndirectPointerEventType,
        primary_directional_motion_axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
    ) -> IndirectPointerEvent {
        assert!(
            !changes.is_empty(),
            "compose-ui: an IndirectPointerEvent needs at least one change"
        );
        IndirectPointerEvent { changes, kind, primary_directional_motion_axis }
    }

    /// Whether a node consumed one of its changes.
    pub fn is_consumed(&self) -> bool {
        self.changes.iter().any(IndirectPointerInputChange::is_consumed)
    }
}

/// A node hearing the indirect pointer events of the focus it holds or is around, as
/// `IndirectPointerInputModifierNode`: each in three passes, `Initial` from the outermost in,
/// `Main` back out, `Final` in again.
pub trait IndirectPointerInputModifierNode {
    fn on_indirect_pointer_event(
        &mut self,
        event: &IndirectPointerEvent,
        pass: PointerEventPass,
        context: &mut EventContext<'_>,
    );
    /// The stream ended without its events: cancelled, or the focus left what it's around.
    fn on_cancel_indirect_pointer_input(&mut self);
}

/// What turns a stream no node consumed into a focus move, as Compose's
/// `IndirectPointerNavigationGestureDetector` over Android's `GestureDetector`: a fling (a release
/// out of the touch slop, faster than the minimum fling velocity, not after a long press) mostly
/// along the primary axis moves the focus to the next (forward) or previous target.
pub struct IndirectPointerNavigationGestureDetector {
    /// The axis of the stream, from its press.
    pub primary_directional_motion_axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
    ignore_current_stream: bool,
    /// Where and when the stream's pointer came down.
    down: Option<(Offset, u64)>,
    in_tap_region: bool,
    in_long_press: bool,
    tracker: VelocityTracker,
}

impl Default for IndirectPointerNavigationGestureDetector {
    fn default() -> Self {
        IndirectPointerNavigationGestureDetector {
            primary_directional_motion_axis: IndirectPointerEventPrimaryDirectionalMotionAxis::None,
            ignore_current_stream: false,
            down: None,
            in_tap_region: false,
            in_long_press: false,
            tracker: VelocityTracker::new(),
        }
    }
}

impl IndirectPointerNavigationGestureDetector {
    pub fn new() -> IndirectPointerNavigationGestureDetector {
        IndirectPointerNavigationGestureDetector::default()
    }

    /// Takes `event`, which a node `consumed` or not: the focus direction a fling asks for. A
    /// consumed move or release ignores the rest of the stream.
    pub fn on_indirect_pointer_event(
        &mut self,
        event: &IndirectPointerEvent,
        consumed: bool,
        configuration: &ViewConfiguration,
    ) -> Option<FocusDirection> {
        let change = &event.changes[0];
        match event.kind {
            IndirectPointerEventType::Press => {
                self.primary_directional_motion_axis = event.primary_directional_motion_axis;
                self.ignore_current_stream = false;
                // A new stream: Android's tracker starts over at a down.
                self.tracker.reset_tracking();
                self.down = Some((change.position, change.uptime_millis));
                self.in_tap_region = true;
                self.in_long_press = false;
            }
            IndirectPointerEventType::Move | IndirectPointerEventType::Release if consumed => {
                self.cancel_current_event_stream();
            }
            _ => {}
        }
        let (down_position, down_time) = self.down?;
        self.tracker.add_position(change.uptime_millis as i64, change.position);
        match event.kind {
            IndirectPointerEventType::Move if self.in_tap_region => {
                let delta = change.position - down_position;
                let slop = configuration.touch_slop;
                if delta.x * delta.x + delta.y * delta.y > slop * slop {
                    self.in_tap_region = false;
                    // Still in the tap region at the timeout, it was a long press.
                    let timeout = configuration.long_press_timeout_millis;
                    self.in_long_press = change.uptime_millis >= down_time + timeout;
                }
                None
            }
            IndirectPointerEventType::Release => {
                self.down = None;
                if self.in_tap_region || self.in_long_press {
                    return None;
                }
                let maximum = configuration.maximum_fling_velocity;
                let velocity =
                    self.tracker.calculate_velocity_with(Velocity::new(maximum, maximum));
                let minimum = configuration.minimum_fling_velocity;
                if velocity.x.abs() <= minimum && velocity.y.abs() <= minimum {
                    return None;
                }
                self.fling(velocity)
            }
            _ => None,
        }
    }

    fn fling(&self, velocity: Velocity) -> Option<FocusDirection> {
        if self.ignore_current_stream {
            return None;
        }
        let along = match self.primary_directional_motion_axis {
            IndirectPointerEventPrimaryDirectionalMotionAxis::X
                if velocity.x.abs() > velocity.y.abs() =>
            {
                velocity.x
            }
            IndirectPointerEventPrimaryDirectionalMotionAxis::Y
                if velocity.y.abs() > velocity.x.abs() =>
            {
                velocity.y
            }
            // No primary axis: the system turns the stream into arrow keys.
            _ => return None,
        };
        Some(if along > 0.0 { FocusDirection::Next } else { FocusDirection::Previous })
    }

    /// Forgets the stream's axis and ignores the rest of it, as `cancelCurrentEventStream`.
    pub fn cancel_current_event_stream(&mut self) {
        self.primary_directional_motion_axis =
            IndirectPointerEventPrimaryDirectionalMotionAxis::None;
        self.ignore_current_stream = true;
    }

    /// Ends what it tracks, as `dispose` (Android's sends its detector a cancel).
    pub fn dispose(&mut self) {
        self.cancel_current_event_stream();
        self.down = None;
        self.tracker.reset_tracking();
    }
}
