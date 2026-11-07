//! Review of `autofill` (D1680 to D1687): what the tree tells the platform's autofill against
//! what Compose's `AndroidAutofillManager` tells, as it hears of each change at once.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::PlatformAutofillCall as Call;
use compose_ui::*;

const SIDE: f32 = 200.0;

fn set_test_content(content: impl Fn() + 'static) -> (ComposeTestRule, Rc<TestAutofillManager>) {
    let mut ui = ComposeTestRule::empty(default_host());
    let am = ui.autofill();
    ui.set_content(content);
    ui.run_on_idle(|_| am.clear());
    (ui, am)
}

fn id(ui: &mut ComposeTestRule, tag: &str) -> i32 {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().semantics_id
}

fn square(ui: &ComposeTestRule) -> IntRect {
    let side = (SIDE * ui.density().density) as i32;
    IntRect { left: 0, top: 0, right: side, bottom: side }
}

// Compose: `FocusTargetNode.onDetach` clears the focus while the node is still attached, and
// `AndroidAutofillManager.onFocusChanged` reads its configuration then: the platform hears the
// node it's entered is gone (`notifyViewExited`). compose-rs tells the focus's moves at the next
// sync, where the node is gone and `autofillable_owner` finds nothing: nothing's told, and the
// platform's session stays on a field that isn't there (its fill UI anchored to it).
#[test]
#[ignore = "review: a focused filling node removed is never told exited (the sync looks it up after it's gone)"]
fn a_focused_filling_node_removed_exits() {
    let shown = mutable_state_of(true);
    let is_shown = shown.handle();
    let (mut ui, am) = set_test_content(move || {
        if is_shown.get() {
            Box!(
                modifier = Modifier
                    .semantics(|config| {
                        config.set_test_tag("username");
                        config.on_fill_data(None, |_| true);
                    })
                    .size(SIDE.dp())
                    .focusable()
            );
        }
    });
    ui.on_node_with_tag("username").request_focus();
    ui.wait_for_idle();
    let username = id(&mut ui, "username");
    am.verify(&Call::NotifyViewEntered { semantics_id: username, bounds: square(&ui) });
    ui.run_on_idle(|_| shown.set(false));
    ui.wait_for_idle();
    am.verify(&Call::NotifyViewExited { semantics_id: username });
    am.assert_no_more_calls();
}

// The same for a text field: Compose tells the field it exited as it goes (then its input text
// gone hides it).
#[test]
#[ignore = "review: a focused text field removed is never told exited"]
fn a_focused_field_removed_exits() {
    let shown = mutable_state_of(true);
    let is_shown = shown.handle();
    let (mut ui, am) = set_test_content(move || {
        if is_shown.get() {
            let state = remember_text_field_state("", None);
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
        }
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    let field = id(&mut ui, "field");
    am.clear();
    ui.run_on_idle(|_| shown.set(false));
    ui.wait_for_idle();
    am.verify(&Call::NotifyViewExited { semantics_id: field });
}

// Compose tells the focus's move at once (`activeFocusTargetNode`'s setter calls its listeners).
// compose-rs queues it for the next sync, which runs only in a frame that recomposed, measured or
// changed semantics: a bare focus target's focus, no semantics of its own changing, still syncs.
#[test]
fn a_filling_focus_target_focused_alone_enters() {
    let requester = Rc::new(Cell::new(None));
    let into = requester.clone();
    let (mut ui, am) = set_test_content(move || {
        let focus = remember(FocusRequester::new);
        into.set(Some(focus));
        Box!(
            modifier = Modifier
                .semantics(|config| {
                    config.set_test_tag("username");
                    config.on_fill_data(None, |_| true);
                })
                .size(SIDE.dp())
                .focus_requester(focus)
                .focus_target()
        );
    });
    let focus = requester.get().expect("composed");
    ui.run_on_idle(|_| {
        focus.request_focus();
    });
    ui.wait_for_idle();
    for _ in 0..3 {
        ui.advance_time_by_frame();
        ui.wait_for_idle();
    }
    let username = id(&mut ui, "username");
    am.verify(&Call::NotifyViewEntered { semantics_id: username, bounds: square(&ui) });
    am.assert_no_more_calls();
}

// Compose: a node attached is shown (`onPostAttach`) before anything in its composition's effects
// can focus it; compose-rs's sync tells the focus's moves first, then the nodes come: the platform
// hears a node entered before it hears it's visible.
#[test]
#[ignore = "review (nit): a node focused as it appears enters before it's shown"]
fn a_node_focused_as_it_appears_shows_before_it_enters() {
    let shown = mutable_state_of(false);
    let is_shown = shown.handle();
    let (mut ui, am) = set_test_content(move || {
        if is_shown.get() {
            let focus = remember(FocusRequester::new);
            let request = focus;
            LaunchedEffect((), async move {
                request.request_focus();
            });
            Box!(
                modifier = Modifier
                    .semantics(|config| {
                        config.set_test_tag("username");
                        config.set_content_type(ContentType::USERNAME);
                        config.on_fill_data(None, |_| true);
                    })
                    .size(SIDE.dp())
                    .focus_requester(focus)
                    .focusable()
            );
        }
    });
    ui.run_on_idle(|_| shown.set(true));
    ui.wait_for_idle();
    let username = id(&mut ui, "username");
    let calls = am.calls();
    let visible = calls.iter().position(|call| {
        *call == Call::NotifyViewVisibilityChanged { semantics_id: username, is_visible: true }
    });
    let entered = calls.iter().position(|call| matches!(call, Call::NotifyViewEntered { .. }));
    assert!(visible.is_some() && entered.is_some(), "{calls:?}");
    assert!(visible < entered, "entered before shown: {calls:?}");
}

// A value-based field filled while read-only: Compose's `CoreTextField` highlights it (its text
// unchanged) and the fill succeeds; checks compose-rs keeps the text.
#[test]
fn a_read_only_value_field_filled_keeps_its_text() {
    let value = Rc::new(RefCell::new(String::from("kept")));
    let read = value.clone();
    let (mut ui, _am) = set_test_content(move || {
        let read = read.clone();
        let text = read.borrow().clone();
        BasicTextField!(
            text,
            move |new: String| *read.borrow_mut() = new,
            modifier = Modifier.test_tag("field"),
            read_only = true
        );
    });
    let field = id(&mut ui, "field");
    let fill = FillableData::create_from_text("filled").unwrap();
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(field, fill)]));
    ui.wait_for_idle();
    assert_eq!(*value.borrow(), "kept");
}

// Compose's `notifyAutofillManager` tells an input text's change as a value only for a text's
// data type: a node with an input text and no data type tells nothing as it changes (kills a
// mutant dropping that check; Compose's tests all set `ContentDataType.Text`).
#[test]
fn an_input_text_without_a_data_type_tells_no_value() {
    let typed = mutable_state_of("a");
    let text = typed.handle();
    let (mut ui, am) = set_test_content(move || {
        let value = text.get();
        Box!(
            modifier = Modifier
                .semantics(move |config| {
                    config.set_test_tag("node");
                    config.set_content_type(ContentType::USERNAME);
                    config.set_input_text(value);
                })
                .size(SIDE.dp())
        );
    });
    ui.run_on_idle(|_| typed.set("ab"));
    ui.wait_for_idle();
    let _ = id(&mut ui, "node");
    assert!(
        !am.calls().iter().any(|call| matches!(call, Call::NotifyValueChanged { .. })),
        "{:?}",
        am.calls()
    );
    am.assert_no_more_calls();
}

// Compose's `TriStateToggleableNode.onFillData` clicks only when the value differs from its
// state: filled with the state it has, a tri-state toggle isn't clicked (kills a mutant clicking
// always; the ported tests fill only a differing value).
#[test]
fn a_tri_state_toggle_filled_with_its_own_state_is_not_clicked() {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let (mut ui, _am) = set_test_content(move || {
        let counted = counted.clone();
        Box!(
            modifier = Modifier
                .tri_state_toggleable(ToggleableState::On, move || counted.set(counted.get() + 1))
                .size(SIDE.dp())
                .test_tag("toggle")
        );
    });
    let toggle = id(&mut ui, "toggle");
    let on = FillableData::create_from_boolean(true).unwrap();
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(toggle, on)]));
    ui.wait_for_idle();
    assert_eq!(clicks.get(), 0);
    let off = FillableData::create_from_boolean(false).unwrap();
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(toggle, off)]));
    ui.wait_for_idle();
    assert_eq!(clicks.get(), 1);
}

// ---- The stale `SelectionHandleInfo` noted in missing.md (D1682) ------------------------------

const LINES: usize = 20;
const LINE: f32 = 15.0;

/// A `SelectionContainer` over a column of 20 texts scrolling in a 100 px box, as
/// `selection_container_touch`'s `scrolling_texts`, with or without a platform autofill.
fn scrolling_texts(state: ScrollState, autofill: bool) -> ComposeTestRule {
    use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, UiHost};
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let host = UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer));
    let mut ui = ComposeTestRule::empty(host);
    if autofill {
        ui.autofill();
    }
    ui.set_content(move || {
        Box!(modifier = Modifier.test_tag("container"), {
            SelectionContainer!({
                Box!(modifier = Modifier.height(100.dp()), {
                    Column!(modifier = Modifier.vertical_scroll(state), {
                        for index in 0..LINES {
                            BasicText!(
                                index.to_string(),
                                modifier = Modifier.fill_max_width().test_tag(index.to_string()),
                                style = TextStyle { font_size: LINE.sp(), ..TextStyle::DEFAULT }
                            );
                        }
                    });
                });
            });
        });
    });
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_auto_advance(false);
    ui.wait_for_idle();
    ui
}

fn line_center(ui: &mut ComposeTestRule, line: usize) -> Offset {
    let bounds = ui.on_node_with_tag(&line.to_string()).fetch_semantics_snapshot().bounds;
    Offset::new(bounds.left + LINE / 2.0, bounds.top + LINE / 2.0)
}

/// The handles' nodes, read through the semantics tree (the cache check runs there).
fn handles(ui: &mut ComposeTestRule) -> usize {
    let matcher = crate::text_field_touch_support::is_selection_handle(None);
    ui.on_all_nodes(matcher).fetch_semantics_nodes().len()
}

// The author's case: with a platform autofill, every frame computes semantics, and the semantics
// cache check (debug builds) finds a handle's kept info stale during a long press's drag.
#[test]
#[ignore = "review: a container's end handle keeps stale SelectionHandleInfo after a drag crosses the selection (also on main, next test)"]
fn with_autofill_a_long_press_dragged_in_a_scrolling_column_keeps_fresh_handle_semantics() {
    let state = *ScrollState::new(0);
    let mut ui = scrolling_texts(state, true);
    let (from, to) = (line_center(&mut ui, 3), line_center(&mut ui, 1));
    ui.on_node_with_tag("container").perform_touch_input(|touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(from);
        touch.advance_event_time(timeout + 10);
        touch.move_to(from);
        touch.move_to(to);
        touch.up();
    });
    ui.wait_for_idle();
    assert!(handles(&mut ui) > 0);
}

// The same on main's terms, no autofill: a semantics read between the long press and its drag
// keeps the handles' info, then the drag.
#[test]
#[ignore = "review: a container's end handle keeps stale SelectionHandleInfo after a drag crosses the selection (positions written in the tree's pass don't invalidate the handle's semantics)"]
fn a_semantics_read_mid_drag_then_the_drag_keeps_fresh_handle_semantics() {
    let state = *ScrollState::new(0);
    let mut ui = scrolling_texts(state, false);
    let (from, to) = (line_center(&mut ui, 3), line_center(&mut ui, 1));
    ui.on_node_with_tag("container").perform_touch_input(|touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(from);
        touch.advance_event_time(timeout + 10);
        touch.move_to(from);
    });
    ui.wait_for_idle();
    assert!(handles(&mut ui) > 0);
    ui.on_node_with_tag("container").perform_touch_input(|touch| touch.move_to(to));
    ui.wait_for_idle();
    assert!(handles(&mut ui) > 0);
    ui.on_node_with_tag("container").perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
    assert!(handles(&mut ui) > 0);
}

// After a selection, its handles' info kept by a read, a scroll of the column moves them.
#[test]
fn a_scroll_after_a_selection_keeps_fresh_handle_semantics() {
    let state = *ScrollState::new(0);
    let mut ui = scrolling_texts(state, false);
    let (from, to) = (line_center(&mut ui, 3), line_center(&mut ui, 1));
    ui.on_node_with_tag("container").perform_touch_input(|touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(from);
        touch.advance_event_time(timeout + 10);
        touch.move_to(from);
        touch.move_to(to);
        touch.up();
    });
    ui.wait_for_idle();
    assert!(handles(&mut ui) > 0);
    ui.run_on_idle(|_| {
        state.dispatch_raw_delta(10.0);
    });
    ui.wait_for_idle();
    assert!(handles(&mut ui) > 0);
}
