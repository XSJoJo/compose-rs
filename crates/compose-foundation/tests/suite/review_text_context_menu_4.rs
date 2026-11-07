//! Review tests for `text-context-menu-4` (a `SelectionContainer`'s toolbar and menu, D2041):
//! Compose's `SelectionManager` behaviours the branch's ports don't reach. Those that fail on the
//! branch are `#[ignore = "review: ..."]`.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::MemoryClipboard;
use compose_ui::{ClipboardManager, IntSize, Offset, Rect};

use crate::text_field_touch_support::*;

const TEXT_TAG: &str = "text";

fn rule_with(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), || {});
    let clipboard: Rc<dyn ClipboardManager> = Rc::new(MemoryClipboard::default());
    ui.host().with_tree(|tree| tree.set_clipboard(clipboard));
    ui.set_content(content);
    ui
}

fn long_click_center(ui: &mut ComposeTestRule, tag: &str) {
    ui.on_node_with_tag(tag).perform_touch_input(|touch| {
        let center = touch.center();
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.long_click_at(center, duration);
    });
    ui.wait_for_idle();
}

/// The selection's offsets in the one text.
fn selection(ui: &mut ComposeTestRule) -> Option<(usize, usize)> {
    ui.host().with_tree(|tree| tree.selection_anchors()).map(|((_, start), (_, end))| (start, end))
}

/// The bounding box of `offset` in the text tagged `tag`, in the root.
fn char_box(ui: &mut ComposeTestRule, tag: &str, offset: usize) -> Rect {
    let bounds = ui.on_node_with_tag(tag).fetch_semantics_snapshot().bounds;
    let mut results = Vec::new();
    ui.on_node_with_tag(tag).perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    let layout = results.pop().expect("a text layout");
    layout.get_bounding_box(offset).translate(Offset::new(bounds.left, bounds.top))
}

// Compose's `handleDragObserver(isStartHandle).onCancel()` is `done()`: `showSelectionToolbar()`.
// A handle pressed (the toolbar hidden at `onDown`) whose press is then cancelled before the touch
// slop shows the toolbar again. On the branch, the handle's node only tells the container on a
// tap (a release) or the end of a drag that passed the slop: a cancel before the slop leaves the
// toolbar hidden for good (`HandleNode::on_cancel_pointer_input` -> `end_drag`, `origin` None).
#[test]
#[ignore = "review: a handle's press cancelled before the slop leaves the container's toolbar hidden"]
fn a_handles_cancelled_press_shows_the_toolbar_again() {
    let mut ui = rule_with(|| {
        SelectionContainer!({
            BasicText!("Text Text Text", modifier = Modifier.test_tag(TEXT_TAG));
        });
    });
    long_click_center(&mut ui, TEXT_TAG);
    assert_eq!(selection(&mut ui), Some((5, 9)));
    assert!(ui.text_toolbar().is_shown(), "the long press's toolbar");

    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.wait_for_idle();
    assert!(!ui.text_toolbar().is_shown(), "hidden while the handle is held (`onDown`)");

    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd)))
        .perform_touch_input(|touch| touch.cancel());
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), Some((5, 9)));
    assert!(ui.text_toolbar().is_shown(), "shown again once the press is cancelled (`onCancel`)");
}

// The same press lifted before the slop (a tap, Compose's `onUp`) shows it again: the branch's
// path that works, as a control for the test above.
#[test]
fn a_handles_tap_shows_the_toolbar_again() {
    let mut ui = rule_with(|| {
        SelectionContainer!({
            BasicText!("Text Text Text", modifier = Modifier.test_tag(TEXT_TAG));
        });
    });
    long_click_center(&mut ui, TEXT_TAG);
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    ui.wait_for_idle();
    assert!(!ui.text_toolbar().is_shown());
    ui.on_node(is_selection_handle(Some(Handle::SelectionEnd)))
        .perform_touch_input(|touch| touch.up());
    ui.wait_for_idle();
    assert!(ui.text_toolbar().is_shown());
}

// Compose's `onSelectableChangeCallback`: a selected text's content changing lets the selection
// go (`onRelease`), which hides the toolbar. On the branch the tree's selection only notices the
// text's new version when something asks it (a request, or a layout moving or resizing the
// selection's texts): a text changed to one of the same size keeps a stale toolbar shown, its
// "Copy" copying nothing.
#[test]
#[ignore = "review: a selected text changed in place (same size) leaves the container's toolbar shown"]
fn the_toolbar_hides_when_the_selected_text_changes() {
    let text = Box::leak(Box::new(compose_runtime::mutable_state_of(
        "Text Text Text".to_string(),
    )))
    .handle();
    let mut ui = rule_with(move || {
        SelectionContainer!({
            BasicText!(text.get(), modifier = Modifier.test_tag(TEXT_TAG));
        });
    });
    long_click_center(&mut ui, TEXT_TAG);
    assert_eq!(selection(&mut ui), Some((5, 9)));
    assert!(ui.text_toolbar().is_shown());
    let before = ui.on_node_with_tag(TEXT_TAG).fetch_semantics_snapshot().bounds;

    // As many characters, so it lays out the same.
    text.set("Abcd Abcd Abcd".to_string());
    ui.wait_for_idle();
    let after = ui.on_node_with_tag(TEXT_TAG).fetch_semantics_snapshot().bounds;
    assert_eq!(before, after, "the text's bounds are the same");

    assert!(!ui.text_toolbar().is_shown(), "Compose's `onRelease` hides the toolbar");
}

// Compose's `getContentRect` takes the region of the first and the last selected texts alone
// (`firstAndLast()`), not those between: a wide middle text doesn't widen the toolbar's rect.
// A mutant taking every covered text (`orders` of all `covered`) passes the branch's tests.
#[test]
fn the_toolbars_rect_leaves_out_the_texts_between_the_first_and_the_last() {
    let mut ui = rule_with(|| {
        SelectionContainer!({
            Column!({
                BasicText!("ab", modifier = Modifier.test_tag("first"));
                BasicText!("a much longer middle text", modifier = Modifier.test_tag("middle"));
                BasicText!("cd", modifier = Modifier.test_tag("last"));
            });
        });
    });
    // A long press on the first text, dragged to the last's end.
    let first = ui.on_node_with_tag("first").fetch_semantics_snapshot().bounds;
    let last = ui.on_node_with_tag("last").fetch_semantics_snapshot().bounds;
    ui.on_node_with_tag("first").perform_touch_input(|touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(Offset::new(1.0, first.height() / 2.0));
        touch.move_pointers_after(timeout + 10);
        let to = Offset::new(last.right - 1.0 - first.left, last.center().y - first.top);
        touch.move_to(to);
        touch.up();
    });
    ui.wait_for_idle();
    let anchors = ui.host().with_tree(|tree| tree.selection_anchors());
    assert!(anchors.is_some(), "a selection across the texts");
    assert!(ui.text_toolbar().is_shown());

    let boxes = [
        char_box(&mut ui, "first", 0),
        char_box(&mut ui, "first", 1),
        char_box(&mut ui, "last", 0),
        char_box(&mut ui, "last", 1),
    ];
    let left = boxes.iter().map(|it| it.left).fold(f32::INFINITY, f32::min);
    let top = boxes.iter().map(|it| it.top).fold(f32::INFINITY, f32::min);
    let right = boxes.iter().map(|it| it.right).fold(f32::NEG_INFINITY, f32::max);
    let bottom = boxes.iter().map(|it| it.bottom).fold(f32::NEG_INFINITY, f32::max);
    let expected = Rect::new(left, top, right, bottom + 100.0);
    let middle = ui.on_node_with_tag("middle").fetch_semantics_snapshot().bounds;
    assert!(expected.right < middle.right, "the middle text is wider than the others");
    assert_eq!(ui.text_toolbar().content_rect(), Some(expected));
}

// Compose's toolbar re-reads its items when what they read changes (the platform toolbar's
// snapshot observer): "Select all" clicked on the toolbar in touch mode leaves it open
// (`closePredicate`: the toolbar's asked for in touch mode) and takes "Select all" off it.
// A mutant dropping `menu_items`' read of the selection's changes passes the branch's tests.
#[test]
fn the_toolbars_items_follow_its_select_all() {
    let mut ui = rule_with(|| {
        SelectionContainer!({
            BasicText!("Text Text Text", modifier = Modifier.test_tag(TEXT_TAG));
        });
    });
    long_click_center(&mut ui, TEXT_TAG);
    let toolbar = ui.text_toolbar();
    assert!(toolbar.is_shown());
    assert_eq!(toolbar.titles(), vec!["Copy".to_string(), "Select all".to_string()]);

    toolbar.click("Select all");
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), Some((0, 14)));
    assert!(toolbar.is_shown(), "select all in touch mode keeps the toolbar");
    assert_eq!(toolbar.titles(), vec!["Copy".to_string()]);
}

// Compose's `selectAll()` asks for the focus and `showSelectionToolbar()`: "Select all" from the
// right click's dropdown (no focus, no toolbar asked for before) leaves the toolbar asked for,
// so a finger coming back (Compose's `updateSelectionTouchMode`) shows it. Mutants dropping
// either the focus request in `run_menu_action` or `SelectAll`'s `set_show_toolbar(true)` pass
// the branch's tests.
#[test]
fn select_all_from_the_dropdown_asks_for_the_toolbar_a_finger_shows() {
    let mut ui = rule_with(|| {
        SelectionContainer!({
            BasicText!("Text Text Text", modifier = Modifier.test_tag(TEXT_TAG));
        });
    });
    ui.on_node_with_tag(TEXT_TAG).perform_mouse_input(|mouse| {
        let center = mouse.center();
        mouse.right_click(center);
    });
    ui.wait_for_idle();
    ui.on_node(has_any_ancestor(is_popup()).and(has_text("Select all"))).perform_click();
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), Some((0, 14)));
    assert!(!ui.text_toolbar().is_shown(), "no toolbar out of touch mode");

    // A finger's swipe: touch mode, neither a tap (which clears) nor a long press.
    ui.on_node_with_tag(TEXT_TAG).perform_touch_input(|touch| {
        let (from, to) = (touch.center_left(), touch.center_right());
        touch.swipe(from + Offset::new(1.0, 0.0), to - Offset::new(1.0, 0.0), 200);
    });
    ui.wait_for_idle();
    assert_eq!(selection(&mut ui), Some((0, 14)));
    assert!(ui.text_toolbar().is_shown(), "Compose's `showToolbar` from `selectAll`");
}
