//! A `SelectionContainer`'s drags outside scrolling what's around the dragged text, and the
//! selected items of a lazy list pinned, from Compose's `SelectionContainerTest`'s
//! `*DragOutsideSelectsAndScrolls*` (`isSelectionAutoScrollEnabled`, true at tip, D2023).
//!
//! Compose's tests at density 1, the test font at the default size, touch input mode; each drag
//! goes on until what scrolls reaches its end.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, NodeId};
use compose_ui::{
    Alignment, Density, InputMode, IntSize, Offset, PinnableContainer, PinnedHandle, Rect, UnitExt,
};

use crate::text_field_touch_support::*;

const LINES: usize = 50;

/// What a drag does: begins on text 0, goes on past the container's bottom right by
/// `beyond` (jiggled sideways each time, as Compose's), ends.
#[derive(Clone, Copy, Debug)]
enum Drag {
    LongPress,
    Mouse,
    Handle,
}

const DRAGS: [Drag; 3] = [Drag::LongPress, Drag::Mouse, Drag::Handle];

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer), content);
    ui.host().set_input_mode(InputMode::Touch);
    ui.wait_for_idle();
    ui
}

fn bounds(ui: &mut ComposeTestRule, tag: &str) -> Rect {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().bounds
}

fn start(ui: &mut ComposeTestRule, drag: Drag) {
    match drag {
        Drag::LongPress => {
            ui.on_node_with_tag("text0").perform_touch_input(|touch| {
                let timeout = touch.view_configuration().long_press_timeout_millis;
                touch.down(Offset::ZERO);
                touch.move_pointers_after(timeout + 10);
            });
        }
        Drag::Mouse => {
            ui.on_node_with_tag("text0").perform_mouse_input(|mouse| {
                mouse.move_to(Offset::ZERO);
                mouse.press(MouseButton::Primary);
            });
        }
        Drag::Handle => {
            ui.on_node_with_tag("text0").perform_touch_input(|touch| {
                let duration = touch.view_configuration().long_press_timeout_millis + 100;
                touch.long_click_at(Offset::ZERO, duration);
            });
            ui.wait_for_idle();
            ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).perform_touch_input(
                |touch| {
                    let center = touch.center();
                    touch.down(center);
                },
            );
        }
    }
    ui.wait_for_idle();
}

/// The pointer to the container's bottom right, `beyond` below it, jiggled by `iteration`.
fn proceed(ui: &mut ComposeTestRule, drag: Drag, beyond: f32, iteration: usize) {
    // As Compose's `performTouchInput`: idle before the input, not after it, so the scroll it
    // starts shows at the next move (the loop checks the state from before it ran).
    ui.wait_for_idle();
    let container = bounds(ui, "container");
    let target = Offset::new(container.right + (iteration % 2) as f32, container.bottom + beyond);
    match drag {
        Drag::LongPress => {
            let local = target - container.top_left();
            ui.on_node_with_tag("container").perform_touch_input(|touch| touch.move_to(local));
        }
        Drag::Mouse => {
            let local = target - container.top_left();
            ui.on_node_with_tag("container").perform_mouse_input(|mouse| mouse.move_to(local));
        }
        Drag::Handle => {
            // As Compose's: by the handle's distance to the target, then back by it and 300 px
            // down (its "neutralizing" move): the drag goes on 300 px further each time.
            let target = Offset::new(container.right, container.bottom + beyond);
            let handle = is_selection_handle(Some(Handle::SelectionEnd));
            let at = ui.on_node(handle.clone()).fetch_semantics_snapshot().bounds.top_left();
            let delta = target - at;
            ui.on_node(handle).perform_touch_input(|touch| {
                touch.move_by(delta);
                touch.move_by(Offset::new(-delta.x, -delta.y + 300.0));
            });
        }
    }
}

fn finish(ui: &mut ComposeTestRule, drag: Drag) {
    match drag {
        Drag::LongPress => ui.on_node_with_tag("text0").perform_touch_input(|touch| touch.up()),
        Drag::Mouse => ui
            .on_node_with_tag("text0")
            .perform_mouse_input(|mouse| mouse.release(MouseButton::Primary)),
        Drag::Handle => ui
            .on_node(is_selection_handle(Some(Handle::SelectionEnd)))
            .perform_touch_input(|touch| touch.up()),
    };
    ui.wait_for_idle();
}

fn all_lines() -> String {
    (0..LINES).map(|line| format!("Line{line}")).collect::<Vec<_>>().join("\n")
}

/// The selection from the first text's start to the last's end (`Line49`'s length), all the
/// lines selected. The end's text is checked only out of a lazy list (`lazy`), as Compose's.
#[track_caller]
fn assert_selected_from_first_to_last(ui: &mut ComposeTestRule, lazy: bool) {
    let first = ui.on_node_with_tag("text0").fetch_semantics_snapshot().id;
    let last: NodeId = ui.on_node_with_tag("text49").fetch_semantics_snapshot().id;
    let anchors = ui.host().with_tree(|tree| tree.selection_anchors());
    let ((start, start_offset), (end, end_offset)) = anchors.expect("a selection");
    assert_eq!((start, start_offset, end_offset), (first, 0, 6));
    if !lazy {
        assert_eq!(end, last);
    }
    let selected = ui.host().with_tree(|tree| tree.selected_text());
    assert_eq!(selected, Some(all_lines()));
}

/// As Compose's `dragOutsideSelectsAndScrolls_withNonLazyContainer`.
fn non_lazy(drag: Drag, scrollable_inside: bool) {
    let state = **Box::leak(Box::new(ScrollState::new(0)));
    let mut ui = rule(move || {
        Box!(modifier = Modifier.fill_max_size(), content_alignment = Alignment::CENTER, {
            let outside =
                if scrollable_inside { Modifier } else { Modifier.vertical_scroll(state) };
            Box!(modifier = Modifier.size(200.dp()).then(outside).test_tag("container"), {
                SelectionContainer!({
                    let inside =
                        if scrollable_inside { Modifier.vertical_scroll(state) } else { Modifier };
                    Box!(modifier = inside, {
                        Column!(modifier = Modifier.test_tag("content"), {
                            for line in 0..LINES {
                                BasicText!(
                                    format!("Line{line}"),
                                    modifier =
                                        Modifier.height(20.dp()).test_tag(format!("text{line}")),
                                );
                            }
                        });
                    });
                });
            });
        });
    });
    start(&mut ui, drag);
    for iteration in 0..=100 {
        proceed(&mut ui, drag, 300.0, iteration);
        if state.value() == state.max_value() {
            break;
        }
    }
    finish(&mut ui, drag);
    assert_eq!(state.value(), state.max_value(), "{drag:?} failed to scroll to the end");
    assert!(state.max_value() > 0);
    assert_selected_from_first_to_last(&mut ui, false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, longPressDragOutsideSelectsAndScrolls_withNonLazyContainer_scrollableInside, dragSelectionHandleOutsideSelectsAndScrolls_withNonLazyContainer_scrollableInside, mouseDragOutsideSelectsAndScrolls_withNonLazyContainer_scrollableInside
#[test]
fn dragOutsideSelectsAndScrolls_withNonLazyContainer_scrollableInside() {
    for drag in DRAGS {
        non_lazy(drag, true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, longPressDragOutsideSelectsAndScrolls_withNonLazyContainer_scrollableOutside, dragSelectionHandleOutsideSelectsAndScrolls_withNonLazyContainer_scrollableOutside, mouseDragOutsideSelectsAndScrolls_withNonLazyContainer_scrollableOutside
#[test]
fn dragOutsideSelectsAndScrolls_withNonLazyContainer_scrollableOutside() {
    for drag in DRAGS {
        non_lazy(drag, false);
    }
}

/// A pinnable container telling which items are pinned, as Compose's
/// `PinnableContainerWithCallbacks`.
struct Tracking {
    delegate: Rc<dyn PinnableContainer>,
    index: usize,
    pinned: Rc<RefCell<BTreeSet<usize>>>,
}

impl PinnableContainer for Tracking {
    fn pin(&self) -> PinnedHandle {
        self.pinned.borrow_mut().insert(self.index);
        let handle = self.delegate.pin();
        let (pinned, index) = (self.pinned.clone(), self.index);
        PinnedHandle::new(move || {
            pinned.borrow_mut().remove(&index);
            handle.release();
        })
    }
}

/// As Compose's `dragOutsideSelectsAndScrolls_withLazyContainer`.
fn lazy(drag: Drag, large_offset: bool) {
    let state = LazyListState::new(0, 0);
    let given = *state;
    let pinned: Rc<RefCell<BTreeSet<usize>>> = Rc::default();
    let tracked = pinned.clone();
    let mut ui = rule(move || {
        let tracked = tracked.clone();
        Box!(modifier = Modifier.fill_max_size(), content_alignment = Alignment::CENTER, {
            let tracked = tracked.clone();
            Box!(modifier = Modifier.size(200.dp()).test_tag("container"), {
                let tracked = tracked.clone();
                SelectionContainer!({
                    let tracked = tracked.clone();
                    LazyColumn!(state = given, move |scope: LazyListScope| {
                        let tracked = tracked.clone();
                        scope.items_keyed(
                            LINES,
                            |index| index,
                            move |index| {
                                let delegate = compose_ui::LOCAL_PINNABLE_CONTAINER
                                    .current()
                                    .expect("an item's pinnable container");
                                let pinned = tracked.clone();
                                let container: Rc<dyn PinnableContainer> =
                                    Rc::new(Tracking { delegate, index, pinned });
                                compose_runtime::CompositionLocalProvider(
                                    compose_ui::LOCAL_PINNABLE_CONTAINER.provides(Some(container)),
                                    move || {
                                        BasicText!(
                                            format!("Line{index}"),
                                            modifier = Modifier.test_tag(format!("text{index}")),
                                        );
                                    },
                                );
                            },
                        );
                    });
                });
            });
        });
    });
    start(&mut ui, drag);
    let beyond = if large_offset { 30000.0 } else { 300.0 };
    for iteration in 0..=100 {
        proceed(&mut ui, drag, beyond, iteration);
        if !state.can_scroll_forward() {
            // compose-rs's lazy list jumps to what's brought into view where Compose's animates
            // (D1317): the end shows here a move sooner than Compose's loop sees it, so the move
            // Compose's makes after it is made too.
            proceed(&mut ui, drag, beyond, iteration + 1);
            break;
        }
    }
    finish(&mut ui, drag);
    assert!(!state.can_scroll_forward(), "{drag:?} failed to scroll to the end");
    assert_selected_from_first_to_last(&mut ui, true);
    assert_eq!(*pinned.borrow(), (0..LINES).collect::<BTreeSet<_>>(), "{drag:?}");
    // Cleared, everything is let go.
    ui.on_node_with_tag("text49").perform_touch_input(|touch| touch.click());
    ui.wait_for_idle();
    assert_eq!(ui.host().with_tree(|tree| tree.selection_anchors()), None);
    assert!(pinned.borrow().is_empty(), "{drag:?}: {:?}", pinned.borrow());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, longPressDragOutsideSelectsAndScrolls_withLazyContainer, dragSelectionHandleOutsideSelectsAndScrolls_withLazyContainer, mouseDragOutsideSelectsAndScrolls_withLazyContainer
#[test]
fn dragOutsideSelectsAndScrolls_withLazyContainer() {
    for drag in DRAGS {
        lazy(drag, false);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, longPressDragOutsideSelectsAndScrolls_withLazyContainer_andLargeOffset, dragSelectionHandleOutsideSelectsAndScrolls_withLazyContainer_andLargeOffset, mouseDragOutsideSelectsAndScrolls_withLazyContainer_andLargeOffset
#[test]
fn dragOutsideSelectsAndScrolls_withLazyContainer_andLargeOffset() {
    for drag in DRAGS {
        lazy(drag, true);
    }
}

/// As Compose's `dragOutsideSelectsAndScrolls_withLongTextInNonLazyContainer`: one text of the
/// fifty lines.
fn long_text(drag: Drag, scrollable_inside: bool) {
    let state = **Box::leak(Box::new(ScrollState::new(0)));
    let mut ui = rule(move || {
        Box!(modifier = Modifier.fill_max_size(), content_alignment = Alignment::CENTER, {
            let outside =
                if scrollable_inside { Modifier } else { Modifier.vertical_scroll(state) };
            Box!(modifier = Modifier.size(200.dp()).then(outside).test_tag("container"), {
                SelectionContainer!({
                    let inside =
                        if scrollable_inside { Modifier.vertical_scroll(state) } else { Modifier };
                    Box!(modifier = inside, {
                        Box!(modifier = Modifier.test_tag("content"), {
                            BasicText!(all_lines(), modifier = Modifier.test_tag("text0"));
                        });
                    });
                });
            });
        });
    });
    start(&mut ui, drag);
    for iteration in 0..=100 {
        proceed(&mut ui, drag, 300.0, iteration);
        if state.value() == state.max_value() {
            break;
        }
    }
    finish(&mut ui, drag);
    assert_eq!(state.value(), state.max_value(), "{drag:?} failed to scroll to the end");
    assert!(state.max_value() > 0);
    let text = ui.on_node_with_tag("text0").fetch_semantics_snapshot().id;
    let anchors = ui.host().with_tree(|tree| tree.selection_anchors());
    assert_eq!(anchors, Some(((text, 0), (text, all_lines().len()))), "{drag:?}");
    assert_eq!(ui.host().with_tree(|tree| tree.selected_text()), Some(all_lines()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, longPressDragOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableInside, dragSelectionHandleOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableInside, mouseDragOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableInside
#[test]
fn dragOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableInside() {
    for drag in DRAGS {
        long_text(drag, true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionContainerTest.kt, longPressDragOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableOutside, dragSelectionHandleOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableOutside, mouseDragOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableOutside
#[test]
fn dragOutsideSelectsAndScrolls_withLongTextInNonLazyContainer_scrollableOutside() {
    for drag in DRAGS {
        long_text(drag, false);
    }
}

type Pinned = Rc<RefCell<BTreeSet<usize>>>;

/// A container, unless `shown` says no, over a lazy list of `LINES` items whose pins `pinned`
/// tracks, each item's content `item`.
fn tracked_list(
    shown: compose_runtime::MutableState<bool>,
    pinned: Pinned,
    item: impl Fn(usize) + Clone + 'static,
) -> ComposeTestRule {
    rule(move || {
        if !shown.get() {
            return;
        }
        let (tracked, item) = (pinned.clone(), item.clone());
        SelectionContainer!({
            let (tracked, item) = (tracked.clone(), item.clone());
            LazyColumn!(modifier = Modifier.size(200.dp()), move |scope: LazyListScope| {
                let (tracked, item) = (tracked.clone(), item.clone());
                scope.items(LINES, move |index| {
                    let delegate = compose_ui::LOCAL_PINNABLE_CONTAINER
                        .current()
                        .expect("an item's pinnable container");
                    let pinned = tracked.clone();
                    let container: Rc<dyn PinnableContainer> =
                        Rc::new(Tracking { delegate, index, pinned });
                    let item = item.clone();
                    compose_runtime::CompositionLocalProvider(
                        compose_ui::LOCAL_PINNABLE_CONTAINER.provides(Some(container)),
                        move || item(index),
                    );
                });
            });
        });
    })
}

fn line(index: usize) {
    BasicText!(format!("Line{index}"), modifier = Modifier.test_tag(format!("text{index}")));
}

fn state_of<T: Clone + PartialEq + 'static>(value: T) -> compose_runtime::MutableState<T> {
    Box::leak(Box::new(compose_runtime::mutable_state_of(value))).handle()
}

// Not one of Compose's (its manager lets go when it's forgotten, `onForgotten`): a container
// leaving the composition with a selection lets go of its texts' pins.
#[test]
fn a_container_leaving_lets_go_of_its_pins() {
    let shown = state_of(true);
    let pinned: Pinned = Rc::default();
    let mut ui = tracked_list(shown, pinned.clone(), line);
    start(&mut ui, Drag::LongPress);
    finish(&mut ui, Drag::LongPress);
    assert_eq!(*pinned.borrow(), BTreeSet::from([0]));
    shown.set(false);
    ui.wait_for_idle();
    assert!(pinned.borrow().is_empty(), "{:?}", pinned.borrow());
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/selection/SelectionManager.kt, afterSelectableUnsubscribe (a text leaving lets go of its pin at once)
#[test]
fn a_selected_text_leaving_lets_go_of_its_pin() {
    let text_shown = state_of(true);
    let pinned: Pinned = Rc::default();
    let mut ui = tracked_list(state_of(true), pinned.clone(), move |index| {
        Box!(
            modifier = Modifier
                .height_in(14.dp(), compose_ui::Dp::UNSPECIFIED)
                .test_tag(format!("item{index}")),
            {
                if index != 0 || text_shown.get() {
                    line(index);
                }
            }
        );
    });
    start(&mut ui, Drag::LongPress);
    finish(&mut ui, Drag::LongPress);
    assert_eq!(*pinned.borrow(), BTreeSet::from([0]));
    // The item stays, its text goes: nothing else of the selection changes.
    text_shown.set(false);
    ui.wait_for_idle();
    ui.on_node_with_tag("item0").assert_exists();
    assert!(pinned.borrow().is_empty(), "{:?}", pinned.borrow());
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/selection/SelectionManager.kt, afterSelectableUnsubscribe (a text reused for other content unsubscribes as it's forgotten, its node keeping no pin)
#[test]
fn a_selected_text_reused_lets_go_of_its_pin() {
    let key = state_of(0);
    let pinned: Pinned = Rc::default();
    let mut ui = tracked_list(state_of(true), pinned.clone(), move |index| {
        let key = if index == 0 { key.get() } else { 0 };
        compose_runtime::ReusableContent(key, || line(index));
    });
    start(&mut ui, Drag::LongPress);
    finish(&mut ui, Drag::LongPress);
    assert_eq!(*pinned.borrow(), BTreeSet::from([0]));
    let node = ui.on_node_with_tag("text0").fetch_semantics_snapshot().id;
    key.set(1);
    ui.wait_for_idle();
    // The same node, reused for the new content: its pin went with what it held.
    assert_eq!(ui.on_node_with_tag("text0").fetch_semantics_snapshot().id, node);
    assert!(pinned.borrow().is_empty(), "{:?}", pinned.borrow());
    // Selected again, it pins its item again.
    start(&mut ui, Drag::LongPress);
    finish(&mut ui, Drag::LongPress);
    assert_eq!(*pinned.borrow(), BTreeSet::from([0]));
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/selection/SelectionManager.kt, updatePinnedSelectables (the texts no longer selected let go)
#[test]
fn a_selection_shrinking_lets_go_of_the_texts_left_out() {
    let pinned: Pinned = Rc::default();
    let mut ui = tracked_list(state_of(true), pinned.clone(), line);
    start(&mut ui, Drag::LongPress);
    let center = |ui: &mut ComposeTestRule, tag: &str| {
        let text = bounds(ui, tag);
        let container = bounds(ui, "text0");
        Offset::new(text.center().x - container.left, text.center().y - container.top)
    };
    let third = center(&mut ui, "text2");
    ui.on_node_with_tag("text0").perform_touch_input(|touch| touch.move_to(third));
    ui.wait_for_idle();
    assert_eq!(*pinned.borrow(), BTreeSet::from([0, 1, 2]));
    let first = center(&mut ui, "text0");
    ui.on_node_with_tag("text0").perform_touch_input(|touch| touch.move_to(first));
    ui.wait_for_idle();
    assert_eq!(*pinned.borrow(), BTreeSet::from([0]));
    finish(&mut ui, Drag::LongPress);
    assert_eq!(*pinned.borrow(), BTreeSet::from([0]));
}
