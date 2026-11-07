//! Review tests for D1923 (branch `moved-node-attach`): what a node movable content moves holds
//! (a drag, a pressed pointer, the focus of a group, a semantics id, a subcomposition) goes as in
//! Compose, where `LayoutNode.removeAt` detaches it and `insertAt` attaches it again.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    DragGestures, FocusRequester, FocusState, Offset, PointerEventPass, Popup, PopupProperties,
    UnitExt,
};

use crate::interaction_helpers::*;

const TAG: &str = "moved";

/// `content` in a box of each branch of `moved`: the move takes it out of one layout and into
/// another, at the same place.
#[composable]
fn Moving(moved: bool, content: impl Fn() + 'static) {
    if moved {
        let content = content.clone();
        Box!({ content() });
    } else {
        let content = content.clone();
        Box!({ content() });
    }
}

#[derive(Default)]
struct Drags {
    starts: Cell<u32>,
    drags: Cell<u32>,
    ends: Cell<u32>,
    cancels: Cell<u32>,
}

/// A drag under way when its node moves is cancelled, as Compose's `detectDragGestures` catching
/// the block's cancellation (`onDragCancel`); the pointer's later moves and its release reach
/// nothing of it (the hit path dropped the detached node): no drag, no end.
#[test]
fn a_drag_under_way_is_cancelled_when_its_node_moves() {
    let drags = Rc::new(Drags::default());
    let moved = mutable_state_of(false);
    let (there, counted) = (moved.handle(), drags.clone());
    let content = movable_content_of(move || {
        let counted = counted.clone();
        Box!(
            modifier = Modifier.size(100.dp()).test_tag(TAG).pointer_input((), move |scope| {
                let counted = counted.clone();
                async move {
                    let (a, b, c, d) =
                        (counted.clone(), counted.clone(), counted.clone(), counted.clone());
                    scope
                        .detect_drag_gestures(
                            DragGestures::default()
                                .on_drag_start(move |_| a.starts.set(a.starts.get() + 1))
                                .on_drag(move |_, _| b.drags.set(b.drags.get() + 1))
                                .on_drag_end(move || c.ends.set(c.ends.get() + 1))
                                .on_drag_cancel(move || d.cancels.set(d.cancels.get() + 1)),
                        )
                        .await;
                }
            })
        );
    });
    let mut ui = compose_test(move || Moving(there.get(), content.clone()));
    ui.wait_for_idle();
    let at = center(&mut ui, TAG);
    touch_down(&mut ui, at);
    touch_move(&mut ui, at + Offset::new(60.0, 0.0));
    assert_eq!((drags.starts.get(), drags.cancels.get()), (1, 0));
    let dragged = drags.drags.get();
    assert!(dragged > 0);
    moved.set(true);
    ui.wait_for_idle();
    assert_eq!(drags.cancels.get(), 1, "the drag is cancelled with the block");
    touch_move(&mut ui, at + Offset::new(80.0, 0.0));
    touch_up(&mut ui, at + Offset::new(80.0, 0.0));
    assert_eq!(drags.drags.get(), dragged, "no drag after the move");
    assert_eq!(drags.ends.get(), 0, "no end after the move");
    assert_eq!(drags.starts.get(), 1);
    // A new drag works where it is.
    touch_down(&mut ui, at);
    touch_move(&mut ui, at + Offset::new(60.0, 0.0));
    touch_up(&mut ui, at + Offset::new(60.0, 0.0));
    assert_eq!((drags.starts.get(), drags.ends.get()), (2, 1));
}

/// A pointer pressed on a node that moves isn't delivered to it again until it's lifted: its
/// hit path dropped the node, and Compose hit-tests a pressed pointer only at its down. The
/// block started again after the move hears nothing of that pointer.
#[test]
fn a_pressed_pointer_reaches_a_moved_node_no_more() {
    let heard = Rc::new(RefCell::new(Vec::<String>::new()));
    let moved = mutable_state_of(false);
    let (there, log) = (moved.handle(), heard.clone());
    let content = movable_content_of(move || {
        let log = log.clone();
        Box!(
            modifier = Modifier.size(100.dp()).test_tag(TAG).pointer_input((), move |scope| {
                let log = log.clone();
                async move {
                    loop {
                        let event = scope.await_pointer_event(PointerEventPass::Main).await;
                        log.borrow_mut().push(format!("{:?}", event.kind));
                    }
                }
            })
        );
    });
    let mut ui = compose_test(move || Moving(there.get(), content.clone()));
    ui.wait_for_idle();
    let at = center(&mut ui, TAG);
    touch_down(&mut ui, at);
    assert_eq!(heard.borrow().len(), 1, "{:?}", heard.borrow());
    heard.borrow_mut().clear();
    moved.set(true);
    ui.wait_for_idle();
    touch_move(&mut ui, at + Offset::new(5.0, 0.0));
    touch_up(&mut ui, at + Offset::new(5.0, 0.0));
    assert_eq!(*heard.borrow(), Vec::<String>::new(), "nothing of the pointer pressed before");
    // A new press is heard.
    touch_down(&mut ui, at);
    assert_eq!(heard.borrow().len(), 1);
}

/// A focus group moving with the target focused in it: both lose the focus, the group's
/// listener hears it, nothing is left focused, and the target takes it again where it went.
#[test]
fn a_focus_group_moving_with_its_focused_child() {
    let (group_state, child_state) =
        (Rc::new(Cell::new(FocusState::Inactive)), Rc::new(Cell::new(FocusState::Inactive)));
    let requester = FocusRequester::new();
    let moved = mutable_state_of(false);
    let (group_seen, child_seen, there) =
        (group_state.clone(), child_state.clone(), moved.handle());
    let content = movable_content_of(move || {
        let (group_seen, child_seen) = (group_seen.clone(), child_seen.clone());
        Box!(
            modifier = Modifier
                .on_focus_changed(move |state: FocusState| group_seen.set(state))
                .focus_group(),
            {
                let child_seen = child_seen.clone();
                Box!(
                    modifier = Modifier
                        .size(10.dp())
                        .focus_requester(requester)
                        .on_focus_changed(move |state: FocusState| child_seen.set(state))
                        .focusable()
                        .test_tag(TAG)
                );
            }
        );
    });
    let mut ui = compose_test(move || Moving(there.get(), content.clone()));
    ui.wait_for_idle();
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert!(child_state.get().is_focused());
    assert!(group_state.get().has_focus());
    moved.set(true);
    ui.wait_for_idle();
    assert!(!child_state.get().has_focus(), "{:?}", child_state.get());
    assert!(!group_state.get().has_focus(), "{:?}", group_state.get());
    ui.on_node_with_tag(TAG).assert_is_not_focused();
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert!(child_state.get().is_focused());
    assert!(group_state.get().has_focus());
}

/// A moved node keeps its semantics id, as Compose's `LayoutNode.semanticsId` (the node is the
/// same).
#[test]
fn a_moved_node_keeps_its_semantics_id() {
    let moved = mutable_state_of(false);
    let there = moved.handle();
    let content = movable_content_of(move || {
        Box!(modifier = Modifier.size(10.dp()).test_tag(TAG));
    });
    let mut ui = compose_test(move || Moving(there.get(), content.clone()));
    ui.wait_for_idle();
    let before = ui.on_node_with_tag(TAG).fetch_semantics_node().id();
    moved.set(true);
    ui.wait_for_idle();
    assert_eq!(ui.on_node_with_tag(TAG).fetch_semantics_node().id(), before);
}

/// Content moved into a subcomposition (a `BoxWithConstraints`) and out again: its focused
/// target loses the focus each time, and takes it again where it is.
#[test]
fn a_focused_target_moved_into_a_subcomposition_and_out() {
    let state = Rc::new(Cell::new(FocusState::Inactive));
    let requester = FocusRequester::new();
    let moved = mutable_state_of(false);
    let (seen, there) = (state.clone(), moved.handle());
    let content = movable_content_of(move || {
        let seen = seen.clone();
        Box!(
            modifier = Modifier
                .size(10.dp())
                .focus_requester(requester)
                .on_focus_changed(move |state: FocusState| seen.set(state))
                .focusable()
                .test_tag(TAG)
        );
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        Column!({
            let inside = content.clone();
            BoxWithConstraints!(move |_| {
                if moved {
                    inside();
                }
            });
            if !moved {
                content();
            }
        });
    });
    ui.wait_for_idle();
    for to in [true, false, true] {
        assert!(requester.request_focus());
        ui.wait_for_idle();
        assert!(state.get().is_focused(), "before moving to {to}");
        moved.set(to);
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_exists();
        assert!(!state.get().is_focused(), "moved: {to}: {:?}", state.get());
    }
}

// ---- Moves across compositions, logged ----------------------------------------------------------

type Log = Rc<RefCell<Vec<String>>>;

#[derive(Clone)]
struct Logger(Log, &'static str);

struct LoggerNode(Log, &'static str);

impl compose_ui::ModifierNodeElement for Logger {
    type Node = LoggerNode;
    fn create(&self) -> LoggerNode {
        self.0.borrow_mut().push(format!("create({})", self.1));
        LoggerNode(self.0.clone(), self.1)
    }
    fn update(&self, _: &mut LoggerNode) {}
    fn same(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl compose_ui::ModifierNode for LoggerNode {
    fn on_attach(&mut self, _: &mut compose_ui::NodeContext<'_>) {
        self.0.borrow_mut().push(format!("attach({})", self.1));
    }
    fn on_detach(&mut self, _: &mut compose_ui::NodeContext<'_>) {
        self.0.borrow_mut().push(format!("detach({})", self.1));
    }
    fn on_reset(&mut self, _: &mut compose_ui::NodeContext<'_>) {
        self.0.borrow_mut().push(format!("reset({})", self.1));
    }
}

fn take(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

/// Content moved from the main composition into a `BoxWithConstraints`'s subcomposition:
/// detached once and attached once, never reset (back out of it: the ignored test below).
#[test]
fn content_moved_into_a_subcomposition_is_detached_and_attached_once() {
    let log = Log::default();
    let moved = mutable_state_of(false);
    let (there, content_log) = (moved.handle(), log.clone());
    let content = movable_content_of(move || {
        Box!(modifier = Modifier.size(10.dp()).then_custom(Logger(content_log.clone(), "c")));
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        Column!({
            let inside = content.clone();
            BoxWithConstraints!(move |_| {
                if moved {
                    inside();
                }
            });
            if !moved {
                content();
            }
        });
    });
    ui.wait_for_idle();
    assert_eq!(take(&log), ["create(c)", "attach(c)"]);
    moved.set(true);
    ui.wait_for_idle();
    assert_eq!(take(&log), ["detach(c)", "attach(c)"], "into the subcomposition");
}

/// Content moved from a `BoxWithConstraints`'s subcomposition back into its parent composition,
/// the subcomposition reading the state itself (recomposed in the same frame, as Compose's
/// Recomposer pairs content released and inserted across the compositions it recomposes): its
/// nodes kept, detached once, attached once. (With the state read only in the parent, captured
/// by value in the lambda, the subcomposition recomposes at measure, after the parent took
/// the content: it's composed anew there; not checked against Compose.)
#[test]
fn content_moved_out_of_a_subcomposition_into_its_parent_is_moved() {
    let log = Log::default();
    let moved = mutable_state_of(true);
    let (there, content_log) = (moved.handle(), log.clone());
    let content = movable_content_of(move || {
        Box!(modifier = Modifier.size(10.dp()).then_custom(Logger(content_log.clone(), "c")));
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        Column!({
            let inside = content.clone();
            // The subcomposition reads the state itself, as Compose's `if (moveContent)` in
            // `BoxWithConstraints`'s lambda: the Recomposer recomposes it in the same frame.
            BoxWithConstraints!(move |_| {
                if there.get() {
                    inside();
                }
            });
            if !moved {
                content();
            }
        });
    });
    ui.wait_for_idle();
    assert_eq!(take(&log), ["create(c)", "attach(c)"]);
    moved.set(false);
    ui.wait_for_idle();
    assert_eq!(take(&log), ["detach(c)", "attach(c)"]);
}

/// Content moved from the main composition into a lazy list's item and back: detached once and
/// attached once each time.
#[test]
fn content_moved_into_a_lazy_item_and_back_is_detached_and_attached_once() {
    let log = Log::default();
    let moved = mutable_state_of(false);
    let (there, content_log) = (moved.handle(), log.clone());
    let content = movable_content_of(move || {
        Box!(modifier = Modifier.size(10.dp()).then_custom(Logger(content_log.clone(), "c")));
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        Column!({
            let inside = content.clone();
            LazyColumn!(modifier = Modifier.size((100.dp(), 100.dp())), |scope: LazyListScope| {
                let inside = inside.clone();
                scope.item(move || {
                    if moved {
                        inside();
                    }
                });
            });
            if !moved {
                content();
            }
        });
    });
    ui.wait_for_idle();
    assert_eq!(take(&log), ["create(c)", "attach(c)"]);
    for to in [true, false, true] {
        moved.set(to);
        ui.wait_for_idle();
        assert_eq!(take(&log), ["detach(c)", "attach(c)"], "moved: {to}");
    }
}

// ---- Into another window ------------------------------------------------------------------------

/// A focused target moved from the screen into a popup (a window of its own) loses the focus:
/// the screen's focus is cleared (nothing focused there), its group's listener hears it. It takes
/// the focus again in the popup.
#[test]
fn a_focused_target_moved_into_a_popup() {
    let (group_state, child_state) =
        (Rc::new(Cell::new(FocusState::Inactive)), Rc::new(Cell::new(FocusState::Inactive)));
    let requester = FocusRequester::new();
    let moved = mutable_state_of(false);
    let (group_seen, child_seen, there) =
        (group_state.clone(), child_state.clone(), moved.handle());
    let content = movable_content_of(move || {
        let child_seen = child_seen.clone();
        Box!(
            modifier = Modifier
                .size(10.dp())
                .focus_requester(requester)
                .on_focus_changed(move |state: FocusState| child_seen.set(state))
                .focusable()
                .test_tag(TAG)
        );
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        let group_seen = group_seen.clone();
        let screen = content.clone();
        Box!(
            modifier = Modifier
                .on_focus_changed(move |state: FocusState| group_seen.set(state))
                .focus_group(),
            {
                if !moved {
                    screen();
                }
            }
        );
        if moved {
            let content = content.clone();
            Popup!(on_dismiss_request = || {}, {
                content();
            });
        }
    });
    ui.wait_for_idle();
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert!(child_state.get().is_focused());
    assert!(group_state.get().has_focus());
    moved.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_exists();
    assert!(!child_state.get().has_focus(), "{:?}", child_state.get());
    assert!(!group_state.get().has_focus(), "the screen's group: {:?}", group_state.get());
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert!(child_state.get().is_focused(), "focused in the popup");
    assert!(!group_state.get().has_focus());
}

/// A focused target moved out of a focusable popup (a focus window of its own, which stays open)
/// to the screen: its focus is cleared in the popup's window, which it left (`focus_window_of`
/// walks up through the parent it left while it's detached), and it isn't focused anywhere.
#[test]
fn a_focused_target_moved_out_of_a_focusable_popup() {
    let state = Rc::new(Cell::new(FocusState::Inactive));
    let requester = FocusRequester::new();
    let moved = mutable_state_of(false);
    let (seen, there) = (state.clone(), moved.handle());
    let content = movable_content_of(move || {
        let seen = seen.clone();
        Box!(
            modifier = Modifier
                .size(10.dp())
                .focus_requester(requester)
                .on_focus_changed(move |state: FocusState| seen.set(state))
                .focusable()
                .test_tag(TAG)
        );
    });
    let mut ui = compose_test(move || {
        let content = content.clone();
        let moved = there.get();
        let screen = content.clone();
        Box!({
            if moved {
                screen();
            }
        });
        let popup = content.clone();
        Popup!(
            on_dismiss_request = || {},
            properties = PopupProperties { focusable: true, ..PopupProperties::default() },
            {
                let popup = popup.clone();
                Box!(modifier = Modifier.size(20.dp()), {
                    if !moved {
                        popup();
                    }
                });
            }
        );
    });
    ui.wait_for_idle();
    assert!(requester.request_focus());
    ui.wait_for_idle();
    assert!(state.get().is_focused(), "focused in the popup");
    moved.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_exists();
    assert!(!state.get().has_focus(), "{:?}", state.get());
    ui.on_node_with_tag(TAG).assert_is_not_focused();
    assert_eq!(ui.host().focused_node(), None, "the popup's window holds no stale focus");
}
