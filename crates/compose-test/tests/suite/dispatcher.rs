//! What Compose's `inputdispatcher` tests read, for their ports through the injection scopes
//! (`touch_events.rs`, `mouse_events.rs`, `key_and_mouse_events.rs`): Compose's tests record the
//! `MotionEvent`s its internal `InputDispatcher` makes, on a 1000×1000 root; these record what a
//! node filling a 1000×1000 root hears of them, each platform event's changes together (as a
//! `pointerInput` block hears them) and the gesture's cancels, and (focused) its key events.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_test::*;
use compose_ui::{
    EventContext, IntSize, Key, KeyEvent, KeyEventType, KeyModifiers, Modifier, ModifierNode,
    ModifierNodeElement, Offset, PointerButtons, PointerEventPass, PointerEventType, PointerEvents,
    PointerId, PointerInputModifierNode, PointerKeyboardModifiers, PointerType, UnitExt,
};

/// One change of an event: which pointer, where, whether down.
pub(crate) type Change = (PointerId, Offset, bool);

/// What the node heard: an event (its kind, time in ms of the test clock, pointer type, buttons,
/// its changes in pointer order, its scroll, the keys it tells), the gesture cancelled (and
/// when), or a key event.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Heard {
    Event {
        kind: PointerEventType,
        millis: u64,
        pointer_type: PointerType,
        buttons: PointerButtons,
        changes: Vec<Change>,
        scroll: Offset,
        modifiers: PointerKeyboardModifiers,
    },
    Cancel {
        millis: u64,
    },
    Key {
        key: Key,
        kind: KeyEventType,
        modifiers: KeyModifiers,
        millis: u64,
    },
}

impl Heard {
    pub(crate) fn millis(&self) -> u64 {
        match self {
            Heard::Event { millis, .. } | Heard::Cancel { millis } | Heard::Key { millis, .. } => {
                *millis
            }
        }
    }

    fn millis_mut(&mut self) -> &mut u64 {
        match self {
            Heard::Event { millis, .. } | Heard::Cancel { millis } | Heard::Key { millis, .. } => {
                millis
            }
        }
    }

    /// The event's kind, `None` for a cancel.
    pub(crate) fn kind(&self) -> Option<PointerEventType> {
        match self {
            Heard::Event { kind, .. } => Some(*kind),
            Heard::Cancel { .. } | Heard::Key { .. } => None,
        }
    }

    /// The keys a pointer event tells.
    pub(crate) fn modifiers(&self) -> Option<PointerKeyboardModifiers> {
        match self {
            Heard::Event { modifiers, .. } => Some(*modifiers),
            _ => None,
        }
    }
}

pub(crate) type Log = Rc<RefCell<Vec<Heard>>>;

fn now_millis() -> u64 {
    compose_runtime::platform::now_nanos() / 1_000_000
}

#[derive(Clone)]
struct RecorderElement(Log);

struct RecorderNode(Log);

impl ModifierNodeElement for RecorderElement {
    type Node = RecorderNode;
    fn create(&self) -> RecorderNode {
        RecorderNode(self.0.clone())
    }
    fn update(&self, node: &mut RecorderNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ModifierNode for RecorderNode {
    fn as_pointer_input(&mut self) -> Option<&mut dyn PointerInputModifierNode> {
        Some(self)
    }
}

impl PointerInputModifierNode for RecorderNode {
    fn takes_whole_events(&self) -> bool {
        true
    }

    fn on_pointer_events(
        &mut self,
        event: &PointerEvents,
        pass: PointerEventPass,
        _bounds: IntSize,
        _context: &mut EventContext<'_>,
    ) {
        if pass != PointerEventPass::Main {
            return;
        }
        let mut changes: Vec<Change> = event
            .changes
            .iter()
            .map(|change| (change.id, change.position, change.pressed))
            .collect();
        changes.sort_by_key(|change| change.0.0);
        let scroll =
            event.changes.iter().fold(Offset::ZERO, |sum, change| sum + change.scroll_delta);
        self.0.borrow_mut().push(Heard::Event {
            kind: event.kind,
            millis: now_millis(),
            pointer_type: event.changes.first().map_or(PointerType::Touch, |c| c.pointer_type),
            buttons: event.buttons,
            changes,
            scroll,
            modifiers: event.keyboard_modifiers,
        });
    }

    fn on_cancel_pointer_input(&mut self) {
        self.0.borrow_mut().push(Heard::Cancel { millis: now_millis() });
    }
}

pub(crate) const ROOT: &str = "root";

/// A 1000×1000 window (density 1) filled by a node recording what it hears, tagged [`ROOT`].
pub(crate) fn recording_root() -> (ComposeTestRule, Log) {
    let log: Log = Rc::default();
    let recorder = log.clone();
    let ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        Box!(
            modifier = Modifier
                .size(1000.dp())
                .then_custom(RecorderElement(recorder.clone()))
                .test_tag(ROOT)
        );
    });
    (ui, log)
}

/// As [`recording_root`], the root also focused and recording its key events.
pub(crate) fn recording_root_with_keys() -> (ComposeTestRule, Log) {
    let log: Log = Rc::default();
    let recorder = log.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let keys = recorder.clone();
        Box!(
            modifier = Modifier
                .size(1000.dp())
                .then_custom(RecorderElement(recorder.clone()))
                .on_key_event(move |event: KeyEvent| {
                    keys.borrow_mut().push(Heard::Key {
                        key: event.key,
                        kind: event.kind,
                        modifiers: event.modifiers,
                        millis: now_millis(),
                    });
                    false
                })
                .focusable()
                .test_tag(ROOT)
        );
    });
    ui.on_node_with_tag(ROOT).request_focus();
    (ui, log)
}

/// What was heard, times from the first event's.
pub(crate) fn heard(log: &Log) -> Vec<Heard> {
    let recorded = log.borrow();
    let t0 = recorded.first().map_or(0, Heard::millis);
    recorded
        .iter()
        .map(|heard| {
            let mut heard = heard.clone();
            *heard.millis_mut() -= t0;
            heard
        })
        .collect()
}

/// The events' times never go back, as `assertHasValidEventTimes`.
#[track_caller]
pub(crate) fn assert_has_valid_event_times(log: &Log) {
    let times: Vec<u64> = log.borrow().iter().map(Heard::millis).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "times go back: {times:?}");
}

/// A touch event heard: `kind` at `millis`, the fingers' changes.
pub(crate) fn touch(kind: PointerEventType, millis: u64, changes: &[Change]) -> Heard {
    Heard::Event {
        kind,
        millis,
        pointer_type: PointerType::Touch,
        buttons: PointerButtons::NONE,
        changes: changes.to_vec(),
        scroll: Offset::ZERO,
        modifiers: PointerKeyboardModifiers::default(),
    }
}

/// A mouse event heard: `kind` at `millis` at `position`, `buttons` held, the pointer `pressed`.
pub(crate) fn mouse(
    kind: PointerEventType,
    millis: u64,
    position: Offset,
    buttons: PointerButtons,
    pressed: bool,
) -> Heard {
    mouse_scroll(kind, millis, position, buttons, pressed, Offset::ZERO)
}

pub(crate) fn mouse_scroll(
    kind: PointerEventType,
    millis: u64,
    position: Offset,
    buttons: PointerButtons,
    pressed: bool,
    scroll: Offset,
) -> Heard {
    Heard::Event {
        kind,
        millis,
        pointer_type: PointerType::Mouse,
        buttons,
        changes: vec![(MOUSE, position, pressed)],
        scroll,
        modifiers: PointerKeyboardModifiers::default(),
    }
}

/// The mouse's (and a trackpad's) pointer in compose-rs's hosts.
pub(crate) const MOUSE: PointerId = PointerId(0);

/// Finger `id`'s pointer: the touch scope's ids are offset by one so they don't share the
/// mouse's pointer 0.
pub(crate) fn finger(id: u64) -> PointerId {
    PointerId(id + 1)
}
