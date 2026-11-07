//! A field's text toolbar: shown by a tap on the cursor handle, a long press or `SetSelection`,
//! hidden by edits, keys, taps, the focus leaving, drags and scrolls, with the field's items
//! (D2032), from Compose's `TextFieldTextContextMenuToolbarTest`, on the test rule's toolbar
//! (Compose's spy on the platform's).
//!
//! As Compose's tests: the test font at 10 sp, density 1, the host in touch input mode, the
//! field under `ProvidePlatformTextContextMenuToolbar`, after a focusable box.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::platform_toolbar::ProvidePlatformTextContextMenuToolbar;
use compose_foundation::*;
use compose_runtime::{MutableState, mutable_state_of};
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, MemoryClipboard, UiHost};
use compose_ui::{
    ClipboardManager, Density, FocusRequester, InputMode, IntSize, Key, Offset, TextRange, UnitExt,
};

use crate::text_field_touch_support::*;

const TAG: &str = "BasicTextField";
const FONT: f32 = 10.0;
const CUT: &str = "Cut";
const COPY: &str = "Copy";
const PASTE: &str = "Paste";
const SELECT_ALL: &str = "Select all";
const AUTOFILL: &str = "Autofill";

fn leak<T: Copy + 'static>(value: compose_runtime::OwnedState<T>) -> MutableState<T>
where
    MutableState<T>: Copy,
{
    **Box::leak(Box::new(value))
}

struct Options {
    text: &'static str,
    single_line: bool,
    read_only: bool,
    clipboard: Option<&'static str>,
    input_transformation: Option<InputTransformation>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            text: "Hello",
            single_line: false,
            read_only: false,
            clipboard: None,
            input_transformation: None,
        }
    }
}

/// Compose's `TestScope`: the field, its state and toggles, and the toolbar.
struct Scope {
    ui: ComposeTestRule,
    state: MutableState<TextFieldState>,
    show_text_field: MutableState<bool>,
    enabled: MutableState<bool>,
    box_focus: FocusRequester,
    toolbar: Rc<TestTextToolbar>,
    clipboard: Rc<MemoryClipboard>,
}

fn run_test(options: Options) -> Scope {
    let state = leak(mutable_state_of(**Box::leak(Box::new(TextFieldState::new(options.text)))));
    let show_text_field = leak(mutable_state_of(true));
    let enabled = leak(mutable_state_of(true));
    let box_focus = FocusRequester::new();
    let clipboard = Rc::new(MemoryClipboard::default());
    if let Some(text) = options.clipboard {
        clipboard.set_text(text);
    }
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let host = UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer));
    let shared: Rc<dyn ClipboardManager> = clipboard.clone();
    host.with_tree(|tree| tree.set_clipboard(shared));
    let mut ui = ComposeTestRule::empty(host);
    ui.host().set_input_mode(InputMode::Touch);
    let (single_line, read_only) = (options.single_line, options.read_only);
    let transformation = options.input_transformation;
    let requester = box_focus;
    ui.set_content(move || {
        let (requester, transformation) = (requester, transformation.clone());
        ProvidePlatformTextContextMenuToolbar!(Modifier, {
            let (requester, transformation) = (requester, transformation.clone());
            Column!({
                Box!(modifier = Modifier.focus_requester(requester).focusable().size(100.dp()));
                if show_text_field.get() {
                    BasicTextFieldOfState!(
                        state.get(),
                        modifier = Modifier.width(100.dp()).test_tag(TAG),
                        text_style = TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT },
                        enabled = enabled.get(),
                        line_limits = if single_line {
                            TextFieldLineLimits::SingleLine
                        } else {
                            TextFieldLineLimits::DEFAULT
                        },
                        input_transformation = transformation.clone(),
                        read_only = read_only,
                    );
                }
            });
        });
    });
    let toolbar = ui.text_toolbar();
    Scope { ui, state, show_text_field, enabled, box_focus, toolbar, clipboard }
}

fn position_for_offset(offset: usize) -> Offset {
    Offset::new(FONT * offset as f32, FONT / 2.0)
}

fn select_all_key() -> Key {
    if cfg!(target_os = "macos") { Key::MetaLeft } else { Key::CtrlLeft }
}

impl Scope {
    fn assert_shown(&mut self) {
        self.ui.wait_for_idle();
        assert!(self.toolbar.is_shown(), "Text toolbar status should be shown.");
    }

    fn assert_not_shown(&mut self) {
        self.ui.wait_for_idle();
        assert!(!self.toolbar.is_shown(), "Text toolbar status should not be shown.");
    }

    fn assert_has_item(&mut self, title: &str) {
        self.assert_shown();
        assert!(self.toolbar.has_item(title), "{title} not in {:?}", self.toolbar.titles());
    }

    fn assert_has_no_item(&mut self, title: &str) {
        self.assert_shown();
        assert!(!self.toolbar.has_item(title), "{title} in {:?}", self.toolbar.titles());
    }

    fn invoke(&mut self, title: &str) {
        self.assert_has_item(title);
        self.toolbar.click(title);
        self.ui.wait_for_idle();
    }

    fn assert_text(&mut self, text: &str) {
        self.ui.wait_for_idle();
        assert_eq!(self.state.get().text().as_str(), text);
    }

    fn assert_selection(&mut self, (start, end): (usize, usize)) {
        self.ui.wait_for_idle();
        assert_eq!(self.state.get().selection(), TextRange::new(start, end));
    }

    fn assert_clipboard(&mut self, text: &str) {
        self.ui.wait_for_idle();
        assert_eq!(self.clipboard.get_text().as_deref(), Some(text));
    }

    fn request_focus(&mut self) {
        self.ui.on_node_with_tag(TAG).request_focus();
        self.ui.wait_for_idle();
    }

    fn set_selection(&mut self, (start, end): (usize, usize), traversal: bool) {
        use compose_ui::SemanticsActions as A;
        self.ui.on_node_with_tag(TAG).perform_semantics_action_with(&A::SET_SELECTION, |set| {
            set(start as i32, end as i32, traversal);
        });
        self.ui.wait_for_idle();
    }

    /// As `performTextInputSelection`: a service's traversal, no toolbar.
    fn set_selection_via_semantics(&mut self, range: (usize, usize)) {
        self.set_selection(range, true);
    }

    /// As `performTextInputSelectionShowingToolbar`.
    fn set_selection_showing_toolbar(&mut self, range: (usize, usize)) {
        self.set_selection(range, false);
    }

    fn click_center(&mut self) {
        self.ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.click());
        self.ui.wait_for_idle();
    }

    fn click_offset(&mut self, offset: usize) {
        self.ui
            .on_node_with_tag(TAG)
            .perform_touch_input(|touch| touch.click_at(position_for_offset(offset)));
        self.ui.wait_for_idle();
    }

    fn long_click_offset(&mut self, offset: usize) {
        self.ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
            let duration = touch.view_configuration().long_press_timeout_millis + 100;
            touch.advance_event_time(1000);
            touch.long_click_at(position_for_offset(offset), duration);
        });
        self.ui.wait_for_idle();
    }

    fn cursor_handle(&mut self) -> SemanticsNodeInteraction<'_> {
        self.ui.on_node(is_selection_handle(Some(Handle::Cursor)))
    }

    fn click_cursor_handle(&mut self) {
        self.ui.wait_for_idle();
        self.cursor_handle().assert_is_displayed().perform_click();
        self.ui.wait_for_idle();
    }

    fn perform_key_input(&mut self, block: impl FnOnce(&mut KeyInjectionScope<'_>)) {
        self.request_focus();
        self.ui.on_node_with_tag(TAG).perform_key_input(block);
        self.ui.wait_for_idle();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarAppears_whenCursorHandleIsClicked
#[test]
fn a_tap_on_the_cursor_handle_shows_the_toolbar() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.assert_not_shown();
    it.click_cursor_handle();
    it.assert_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDisappears_whenCursorHandleIsClickedAgain
#[test]
fn a_second_tap_on_the_cursor_handle_hides_it() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.assert_not_shown();
    it.click_cursor_handle();
    it.assert_shown();
    it.click_cursor_handle();
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, longClickOnEmptyTextField_showsToolbar_butNoHandle
#[test]
fn a_long_press_on_an_empty_field_shows_the_toolbar_but_no_handle() {
    let mut it = run_test(Options { text: "", ..Options::default() });
    it.long_click_offset(1);
    it.cursor_handle().assert_does_not_exist();
    it.assert_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDisappears_whenTextStateIsUpdated
#[test]
fn an_edit_of_the_state_hides_it() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.state.get().edit(|buffer| {
        buffer.append(" World!");
        buffer.place_cursor_at_end();
    });
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotAppear_ifSelectionIsInitiatedViaHardwareKeys
#[test]
fn a_selection_by_keys_shows_none() {
    let mut it = run_test(Options::default());
    it.perform_key_input(|keys| {
        keys.with_key_down(Key::ShiftLeft, |keys| {
            for _ in 0..3 {
                keys.press_key(Key::DirectionLeft);
            }
        });
    });
    it.assert_selection((5, 2));
    it.assert_not_shown();
    it.perform_key_input(|keys| {
        keys.with_key_down(select_all_key(), |keys| keys.press_key(Key::Character('a')))
    });
    it.assert_selection((0, 5));
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotAppear_ifSelectionIsInitiatedViaStateUpdate
#[test]
fn a_selection_by_the_state_shows_none() {
    let mut it = run_test(Options::default());
    it.request_focus();
    it.state.get().edit(|buffer| buffer.select_all());
    it.assert_selection((0, 5));
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotAppear_ifSelectionIsInitiatedViaSemantics
#[test]
fn a_selection_by_a_traversal_shows_none() {
    let mut it = run_test(Options::default());
    it.request_focus();
    it.set_selection_via_semantics((0, 5));
    it.assert_selection((0, 5));
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarAppears_ifSelectionIsInitiatedViaSemantics_inNoneTraversalMode
#[test]
fn a_selection_by_semantics_shows_it() {
    let mut it = run_test(Options::default());
    it.request_focus();
    it.set_selection_showing_toolbar((0, 5));
    it.assert_selection((0, 5));
    it.assert_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDisappears_whenTextIsEntered_throughIME
#[test]
fn text_from_the_input_method_hides_it() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.ui.on_node_with_tag(TAG).perform_text_input(" World!");
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, cursorToolbarDisappears_whenTextField_getsDisabled_doesNotReappear
#[test]
fn disabling_the_field_hides_the_cursor_s_toolbar_for_good() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.enabled.set(false);
    it.assert_not_shown();
    it.enabled.set(true);
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, selectionToolbarDisappears_whenTextField_getsDisabled_doesNotReappear
#[test]
fn disabling_the_field_hides_the_selection_s_toolbar_for_good() {
    let mut it = run_test(Options::default());
    it.request_focus();
    it.set_selection_showing_toolbar((2, 4));
    it.assert_shown();
    it.enabled.set(false);
    it.assert_not_shown();
    it.enabled.set(true);
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDisappears_whenTextIsEntered_throughHardwareKeyboard
#[test]
fn a_hardware_key_s_text_hides_it() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.ui.on_node_with_tag(TAG).perform_key_input(|keys| keys.press_key(Key::Character('w')));
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotShow_ifSelectionInitiatedByHardwareKeyboard
#[test]
fn a_hardware_key_hides_it_until_a_finger_comes_back() {
    let mut it = run_test(Options::default());
    it.long_click_offset(0);
    it.assert_selection((0, 5));
    it.assert_shown();
    it.ui.on_node_with_tag(TAG).perform_key_input(|keys| {
        keys.with_key_down(select_all_key(), |keys| keys.press_key(Key::Character('a')))
    });
    it.assert_selection((0, 5));
    it.assert_not_shown();
    it.long_click_offset(0);
    it.assert_selection((0, 5));
    it.assert_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarTemporarilyHides_whenHandleIsBeingDragged
#[test]
fn dragging_a_handle_hides_it_meanwhile() {
    let mut it = run_test(Options::default());
    it.click_offset(0);
    it.click_cursor_handle();
    it.assert_shown();
    it.cursor_handle().perform_touch_input(|touch| {
        let (center, slop) = (touch.center(), touch.view_configuration().touch_slop);
        touch.down(center);
        touch.move_by(Offset::new(slop, 0.0));
        touch.move_by(Offset::new(FONT, 0.0));
    });
    it.assert_not_shown();
    it.cursor_handle().perform_touch_input(|touch| touch.up());
    it.assert_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarTemporarilyHides_whenCursor_goesOutOfBounds
#[test]
fn the_cursor_scrolled_out_of_view_hides_it_meanwhile() {
    let text: &'static str = Box::leak("Hello ".repeat(20).into_boxed_str());
    let mut it = run_test(Options { text, single_line: true, ..Options::default() });
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let double_tap = touch.view_configuration().double_tap_timeout_millis;
        touch.advance_event_time(double_tap * 2);
        touch.swipe_left_with(FONT * 3.0, 0.0, 200);
    });
    it.assert_not_shown();
    it.ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let double_tap = touch.view_configuration().double_tap_timeout_millis;
        touch.advance_event_time(double_tap * 2);
        touch.swipe_right_with(0.0, FONT * 3.0, 200);
    });
    it.assert_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarFollowsTheCursor_whenTextFieldIsScrolled
#[test]
fn the_toolbar_follows_the_cursor_scrolled() {
    let text: &'static str = Box::leak("Hello ".repeat(20).into_boxed_str());
    let mut it = run_test(Options { text, single_line: true, ..Options::default() });
    it.click_center();
    it.click_cursor_handle();
    it.assert_shown();
    let first = it.toolbar.content_rect().unwrap();
    it.ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let (center, slop) = (touch.center(), touch.view_configuration().touch_slop);
        touch.advance_event_time(1000);
        touch.down(center);
        touch.move_by(Offset::new(-slop - FONT, 0.0));
        touch.advance_event_time(3000);
        touch.up();
    });
    it.assert_shown();
    let second = it.toolbar.content_rect().unwrap();
    let expected = first.translate(Offset::new(-FONT, 0.0));
    assert!(
        (second.left - expected.left).abs() < 1.0 && (second.top - expected.top).abs() < 1.0,
        "{second:?} isn't {expected:?}"
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarShowsSelectAll
#[test]
fn the_toolbar_has_select_all() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.click_center();
    it.click_cursor_handle();
    it.assert_has_item(SELECT_ALL);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarShowsAutofill_ifNotReadOnly
#[test]
fn an_editable_field_s_toolbar_has_autofill() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_has_item(AUTOFILL);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotShowSelectAll_whenAllTextIsAlreadySelected
#[test]
fn select_all_goes_once_everything_is_selected() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.click_center();
    it.click_cursor_handle();
    it.invoke(SELECT_ALL);
    it.assert_selection((0, 5));
    it.assert_has_no_item(SELECT_ALL);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotShowPaste_whenClipboardHasNoContent
#[test]
fn no_paste_with_nothing_on_the_clipboard() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.click_center();
    it.click_cursor_handle();
    it.assert_has_no_item(PASTE);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarShowsPaste_whenClipboardHasText
#[test]
fn paste_with_text_on_the_clipboard() {
    let mut it =
        run_test(Options { single_line: true, clipboard: Some("world"), ..Options::default() });
    it.click_center();
    it.click_cursor_handle();
    it.assert_has_item(PASTE);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, pasteInsertsContentAtCursor_placesCursorAfterInsertedContent
#[test]
fn paste_inserts_at_the_cursor() {
    let mut it =
        run_test(Options { single_line: true, clipboard: Some("world"), ..Options::default() });
    it.click_offset(2);
    it.click_cursor_handle();
    it.invoke(PASTE);
    it.assert_text("Heworldllo");
    it.assert_selection((7, 7));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotShowCopyOrCut_whenSelectionIsCollapsed
#[test]
fn a_cursor_set_by_semantics_shows_none() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.request_focus();
    it.set_selection_via_semantics((2, 2));
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarShowsCopyAndCut_whenSelectionIsExpanded
#[test]
fn a_selection_s_toolbar_has_copy_and_cut() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.request_focus();
    it.set_selection_showing_toolbar((2, 4));
    it.assert_has_item(COPY);
    it.assert_has_item(CUT);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarShowsAutofill_whenSelectionIsCollapsed
#[test]
fn the_cursor_s_toolbar_has_autofill() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.click_center();
    it.click_cursor_handle();
    it.assert_has_item(AUTOFILL);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDoesNotShowAutofill_whenSelectionIsExpanded
#[test]
fn a_selection_s_toolbar_has_no_autofill() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.request_focus();
    it.set_selection_showing_toolbar((2, 4));
    it.assert_has_no_item(AUTOFILL);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, copyUpdatesClipboardManager_placesCursorAtTheEndOfSelectedRegion
#[test]
fn copy_copies_and_puts_the_cursor_at_the_selection_s_end() {
    let mut it = run_test(Options { single_line: true, ..Options::default() });
    it.request_focus();
    it.set_selection_showing_toolbar((0, 5));
    it.assert_selection((0, 5));
    it.invoke(COPY);
    it.assert_clipboard("Hello");
    it.assert_selection((5, 5));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, cutUpdatesClipboardManager_placesCursorAtTheEndOfSelectedRegion_removesTheCutContent
#[test]
fn cut_copies_and_removes_the_selection() {
    let mut it =
        run_test(Options { text: "Hello World!", single_line: true, ..Options::default() });
    it.request_focus();
    it.set_selection_showing_toolbar((1, 5));
    it.invoke(CUT);
    it.assert_clipboard("ello");
    it.assert_text("H World!");
    it.assert_selection((1, 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, cutAppliesFilter
#[test]
fn cut_goes_through_the_input_transformation() {
    // Rejects text changes, takes the selection.
    let transformation = InputTransformation::from_fn(|buffer| {
        let selection = buffer.selection();
        let original = buffer.original_text().to_string();
        let length = buffer.length();
        buffer.replace(0, length, &original);
        buffer.set_selection(selection);
    });
    let mut it = run_test(Options {
        text: "Hello World!",
        single_line: true,
        input_transformation: Some(transformation),
        ..Options::default()
    });
    it.request_focus();
    it.set_selection_showing_toolbar((1, 5));
    it.invoke(CUT);
    it.assert_clipboard("ello");
    it.assert_text("Hello World!");
    it.assert_selection((1, 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, tappingTextField_hidesTheToolbar
#[test]
fn a_tap_on_the_field_hides_it() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        touch.advance_event_time(1000);
        touch.click_at(position_for_offset(2));
    });
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, interactingWithTextFieldByMouse_doesNotShowTheToolbar
#[test]
fn a_mouse_click_on_the_handle_shows_none() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.cursor_handle().assert_is_displayed().perform_mouse_input(|mouse| mouse.click());
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, interactingWithTextFieldByTrackpad_doesNotShowTheToolbar
#[test]
fn a_trackpad_click_on_the_handle_shows_none() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.cursor_handle().assert_is_displayed().perform_trackpad_input(|trackpad| trackpad.click());
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDisappears_whenFocusIsLost
#[test]
fn the_focus_leaving_hides_it() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.box_focus.request_focus();
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDisappears_whenTextFieldIsDisposed
#[test]
fn the_field_leaving_hides_it() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.show_text_field.set(false);
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarDisappears_whenLongPressIsInitiated
#[test]
fn a_long_press_hides_the_cursor_s_toolbar() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        // Compose's `longPress`: held past the timeout, then a move.
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.advance_event_time(1000);
        touch.down(position_for_offset(6));
        touch.advance_event_time(timeout + 10);
        touch.move_to(position_for_offset(6));
    });
    it.assert_not_shown();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldTextContextMenuToolbarTest.kt, toolbarCanReappear_whenTextFieldStateChanges
#[test]
fn the_toolbar_shows_again_over_a_new_state() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    it.state.set(**Box::leak(Box::new(TextFieldState::new("World"))));
    it.assert_not_shown();
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
}

/// The toolbar goes around the cursor, in the root: below the focusable box, at the cursor's
/// place in the field.
#[test]
fn the_cursor_s_toolbar_goes_around_the_cursor() {
    let mut it = run_test(Options::default());
    it.click_offset(2);
    it.click_cursor_handle();
    it.assert_shown();
    let rect = it.toolbar.content_rect().unwrap();
    assert!(
        (rect.left - 20.0).abs() <= 1.0 && rect.top == 100.0 && rect.bottom == 110.0,
        "{rect:?}"
    );
}

/// A long press on a field not focused yet: the toolbar goes around the selected word, in the
/// root, once the field is focused and placed.
#[test]
fn a_long_press_s_toolbar_goes_around_the_word() {
    let mut it = run_test(Options::default());
    it.long_click_offset(1);
    it.assert_selection((0, 5));
    it.assert_shown();
    let rect = it.toolbar.content_rect().unwrap();
    assert!(rect.top == 100.0 && rect.bottom == 110.0 && rect.right >= 49.0, "{rect:?}");
    // There from the start (D2033).
    assert_eq!(it.toolbar.first_content_rect(), Some(rect));
}
