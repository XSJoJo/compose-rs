//! A state-based field's gestures, from Compose's `TextFieldSelectionGesturesTest` as
//! `TextField2SelectionGesturesLtrTest` and `TextField2SelectionGesturesRtlTest` run it (each test
//! in both directions): a finger's long press, double and triple taps and their drags, a mouse's
//! and a trackpad's clicks and drags, the input mode switching the handles, a read-only and a
//! disabled field.
//!
//! As Compose's tests: the test font at 15 sp, density 1, a touch slop of 0.1, the field filling
//! the width of a box inset by 32 dp. Each test starts with a finger's tap on the first character
//! (the cursor there, its handle shown). Each check is the asserter's: the text and the selection
//! (by touch, its ends in order: the field never reverses a finger's selection), the handles
//! shown, and at least as many `TextHandleMove` and `LongPress` haptics as expected since the last
//! check (D2027), and the text toolbar shown or not on the test rule's toolbar (D2043); no
//! magnifier shows (Compose's checks are skipped where the platform has none).

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::MutableState;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    Density, HapticFeedbackType, InputMode, IntSize, LayoutDirection, Offset, Rect,
    TextLayoutResult, TextRange, UnitExt,
};

use crate::text_field_touch_support::*;
use crate::text_selection_gestures::{CURSORS, Cursor, CursorScope};

const TAG: &str = "testTag";
const FONT: f32 = 15.0;
/// Compose's `RtlChar`, a Hebrew letter.
const RTL_CHAR: char = '\u{05D1}';

thread_local! {
    /// The `TextHandleMove` and `LongPress` haptics performed since the last check.
    static HAPTICS: std::cell::Cell<(u32, u32)> = const { std::cell::Cell::new((0, 0)) };
}

struct CountingHaptics;

impl compose_ui::HapticFeedback for CountingHaptics {
    fn perform_haptic_feedback(&self, feedback: HapticFeedbackType) {
        HAPTICS.with(|count| {
            let (moves, long) = count.get();
            match feedback {
                HapticFeedbackType::TextHandleMove => count.set((moves + 1, long)),
                HapticFeedbackType::LongPress => count.set((moves, long + 1)),
                _ => {}
            }
        });
    }
}

/// The rule, the field's state and options, and what's expected.
struct FieldGestures {
    ui: ComposeTestRule,
    state: TextFieldState,
    read_only: MutableState<bool>,
    enabled: MutableState<bool>,
    rtl: bool,
    /// The text expected, as the asserter's `textContent`.
    text: String,
    /// The selection expected in characters, as Compose's `TextRange` (crossed when start >
    /// end), and whether a finger set it (`selectionByTouch`: compared in order).
    selection: (usize, usize),
    by_touch: bool,
    handles_shown: bool,
    cursor_handle_shown: bool,
    moves: u32,
    long_presses: u32,
    /// The text toolbar expected shown, as the asserter's `textToolbarShown` (D2041).
    toolbar_shown: bool,
}

/// As Compose's `setTextFieldTestContent` around `Content()`, then its `@Before`: a finger's tap
/// on the first character, then a second so the next gesture isn't a multi-tap.
fn field_gestures(rtl: bool) -> FieldGestures {
    let text = if rtl {
        let word = RTL_CHAR.to_string().repeat(5);
        format!("{word}\n{word} {word} {word}\n{word}")
    } else {
        "line1\nline2 text1 text2\nline3".to_string()
    };
    let state = **Box::leak(Box::new(TextFieldState::new(text.as_str())));
    let read_only = Box::leak(Box::new(compose_runtime::mutable_state_of(false))).handle();
    let enabled = Box::leak(Box::new(compose_runtime::mutable_state_of(true))).handle();
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    HAPTICS.with(|count| count.set((0, 0)));
    let mut ui = compose_test_with(
        IntSize::new(400, 800),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let haptics: Rc<dyn compose_ui::HapticFeedback> = Rc::new(CountingHaptics);
            let direction = if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            compose_runtime::CompositionLocalProvider(
                compose_ui::LOCAL_HAPTIC_FEEDBACK.provides(haptics),
                move || {
                    Box!(
                        modifier = Modifier.padding(32.dp()).fill_max_size().wrap_content_size(),
                        {
                            compose_runtime::CompositionLocalProvider(
                                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                                move || {
                                    BasicTextFieldOfState!(
                                        state,
                                        read_only = read_only.get(),
                                        enabled = enabled.get(),
                                        text_style = TextStyle {
                                            font_size: FONT.sp(),
                                            ..TextStyle::DEFAULT
                                        },
                                        modifier = Modifier.fill_max_width().test_tag(TAG),
                                    );
                                },
                            );
                        }
                    );
                },
            );
        },
    );
    ui.host().set_input_mode(InputMode::Touch);
    ui.host().set_view_configuration(Some(compose_ui::ViewConfiguration {
        touch_slop: 0.1,
        minimum_touch_target_size: compose_ui::DpSize::ZERO,
        ..compose_ui::ViewConfiguration::default()
    }));
    ui.wait_for_idle();
    let mut it = FieldGestures {
        ui,
        state,
        read_only,
        enabled,
        rtl,
        text,
        selection: (0, 0),
        by_touch: false,
        handles_shown: false,
        cursor_handle_shown: false,
        moves: 0,
        long_presses: 0,
        toolbar_shown: false,
    };
    let at = it.character(0);
    it.touch(|touch| touch.click_at(at));
    // So the next event (if any) isn't a multi-tap.
    it.ui.advance_time_by(std::time::Duration::from_millis(1_000));
    it.expect(|it| it.cursor_handle_shown = true);
    it.cursor_handle_shown = false;
    it
}

impl FieldGestures {
    /// The tests' `word`, for one-off contents.
    fn word(&self) -> String {
        if self.rtl { RTL_CHAR.to_string().repeat(5) } else { "hello".to_string() }
    }

    /// Compose's offset `offset` (in UTF-16 units, one per character here) in `text`'s bytes.
    fn byte_in(text: &str, offset: usize) -> usize {
        text.char_indices().nth(offset).map_or(text.len(), |(byte, _)| byte)
    }

    fn byte(&self, offset: usize) -> usize {
        Self::byte_in(&self.state.text(), offset)
    }

    /// The state's text is `text`, its cursor at the end, as the tests' `textContent = …`.
    fn set_content(&mut self, text: &str) {
        self.state.set_text_and_place_cursor_at_end(text);
        self.ui.wait_for_idle();
    }

    fn set_read_only(&mut self, read_only: bool) {
        self.read_only.set(read_only);
        self.ui.wait_for_idle();
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled.set(enabled);
        self.ui.wait_for_idle();
    }

    fn bounds(&mut self) -> Rect {
        let size = self.ui.on_node_with_tag(TAG).fetch_semantics_snapshot().bounds.size();
        Rect::new(0.0, 0.0, size.width, size.height)
    }

    fn start_x(&mut self) -> f32 {
        let bounds = self.bounds();
        if self.rtl { bounds.right - 1.0 } else { bounds.left + 1.0 }
    }

    fn end_x(&mut self) -> f32 {
        let bounds = self.bounds();
        if self.rtl { bounds.left + 1.0 } else { bounds.right - 1.0 }
    }

    fn top_start(&mut self) -> Offset {
        let top = self.bounds().top + 1.0;
        Offset::new(self.start_x(), top)
    }

    fn top_end(&mut self) -> Offset {
        let top = self.bounds().top + 1.0;
        Offset::new(self.end_x(), top)
    }

    fn center(&mut self) -> Offset {
        self.bounds().center()
    }

    fn center_start(&mut self) -> Offset {
        let y = self.bounds().center().y;
        Offset::new(self.start_x(), y)
    }

    fn center_end(&mut self) -> Offset {
        let y = self.bounds().center().y;
        Offset::new(self.end_x(), y)
    }

    fn bottom_start(&mut self) -> Offset {
        let bottom = self.bounds().bottom - 1.0;
        Offset::new(self.start_x(), bottom)
    }

    fn bottom_end(&mut self) -> Offset {
        let bottom = self.bounds().bottom - 1.0;
        Offset::new(self.end_x(), bottom)
    }

    /// Two pixels toward the end (`end`) or the start, and down (`down`) or up, as `nudge`.
    fn nudge(&self, at: Offset, x: Option<bool>, y: Option<bool>) -> Offset {
        let toward = |end: bool| if end != self.rtl { 2.0 } else { -2.0 };
        let dx = x.map_or(0.0, toward);
        let dy = y.map_or(0.0, |down| if down { 2.0 } else { -2.0 });
        Offset::new(at.x + dx, at.y + dy)
    }

    fn layout(&mut self) -> TextLayoutResult {
        let mut results = Vec::new();
        self.ui.on_node_with_tag(TAG).perform_semantics_action_with(
            &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
            |get| {
                get(&mut results);
            },
        );
        results.pop().expect("a text layout")
    }

    /// Character `offset`'s box at its start side's middle, nudged to its end, as the fields'
    /// `characterPosition` (`centerLeft`, or `centerRight` right to left).
    fn character(&mut self, offset: usize) -> Offset {
        let byte = self.byte(offset);
        let rect = self.layout().get_bounding_box(byte);
        let side = Offset::new(if self.rtl { rect.right } else { rect.left }, rect.center().y);
        self.nudge(side, Some(true), None)
    }

    fn touch(&mut self, block: impl FnOnce(&mut TouchInjectionScope<'_>)) {
        self.ui.on_node_with_tag(TAG).perform_touch_input(block);
        self.ui.wait_for_idle();
    }

    /// A finger held at `at` past the long-press timeout, as Compose's `longPress`.
    fn long_press(&mut self, at: Offset) {
        self.touch(|touch| {
            let timeout = touch.view_configuration().long_press_timeout_millis;
            touch.down(at);
            touch.move_pointers_after(timeout + 10);
        });
    }

    fn long_click(&mut self, at: Offset) {
        self.touch(|touch| {
            let duration = touch.view_configuration().long_press_timeout_millis + 100;
            touch.long_click_at(at, duration);
        });
    }

    /// The finger moved to `to` over `millis`, a move each event period, as `touchDragNodeTo`.
    fn touch_drag_to_in(&mut self, to: Offset, millis: u64) {
        self.touch(|touch| {
            let from = touch.current_position(0).expect("a finger down");
            let period = touch.event_period_millis();
            let steps = ((millis as f32 / period as f32).round() as u64).max(1);
            let mut previous = 0;
            for step in 1..=steps {
                let time = millis * step / steps;
                let fraction = time as f32 / millis as f32;
                touch.move_pointer_to(0, from + (to - from) * fraction, time - previous);
                previous = time;
            }
        });
    }

    fn touch_drag_to(&mut self, to: Offset) {
        self.touch_drag_to_in(to, 200);
    }

    /// As `touchDragBy`, over 100 ms.
    fn touch_drag_by(&mut self, by: Offset) {
        let mut at = Offset::ZERO;
        self.touch(|touch| at = touch.current_position(0).expect("a finger down"));
        self.touch_drag_to_in(at + by, 100);
    }

    fn up(&mut self) {
        self.touch(|touch| touch.up());
    }

    /// A mouse's or a trackpad's input on the field, as `performMouseGesture` and
    /// `performTrackpadGesture`.
    fn cursor(&mut self, kind: Cursor, block: impl FnOnce(&mut CursorScope<'_, '_>)) {
        match kind {
            Cursor::Mouse => {
                self.ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| {
                    block(&mut CursorScope::Mouse(mouse));
                });
            }
            Cursor::Trackpad => {
                self.ui.on_node_with_tag(TAG).perform_trackpad_input(|trackpad| {
                    block(&mut CursorScope::Trackpad(trackpad));
                });
            }
        }
        self.ui.wait_for_idle();
    }

    /// The cursor moved to `to` over `millis`, as `mouseDragNodeTo` and `trackpadDragNodeTo`.
    fn cursor_drag_to_in(&mut self, kind: Cursor, to: Offset, millis: u64) {
        self.cursor(kind, |cursor| {
            let from = cursor.current_position();
            let steps = ((millis as f32 / 16.0).round() as u64).max(1);
            let mut previous = 0;
            for step in 1..=steps {
                let time = millis * step / steps;
                let fraction = time as f32 / millis as f32;
                cursor.move_to_after(from + (to - from) * fraction, time - previous);
                previous = time;
            }
        });
    }

    fn cursor_drag_to(&mut self, kind: Cursor, to: Offset) {
        self.cursor_drag_to_in(kind, to, 200);
    }

    /// As `enterTouchMode`: a swipe from the center to below the field.
    fn enter_touch_mode(&mut self) {
        let bounds = self.bounds();
        let (from, to) = (bounds.center(), Offset::new(bounds.center().x, bounds.bottom + 10.0));
        self.touch(|touch| touch.swipe(from, to, 200));
    }

    /// As `enterMouseMode` and `enterTrackpadMode`: the cursor moved, no button pressed, left,
    /// to the bottom right and back, 50 ms each.
    fn enter_cursor_mode(&mut self, kind: Cursor) {
        let bounds = self.bounds();
        self.cursor_drag_to_in(kind, Offset::new(bounds.left, bounds.center().y), 50);
        self.cursor_drag_to_in(kind, Offset::new(bounds.right, bounds.bottom), 50);
        self.cursor_drag_to_in(kind, bounds.center(), 50);
    }

    /// The selection a finger set, as `selectionByTouch = start to end`.
    fn by_touch(&mut self, start: usize, end: usize) {
        self.selection = (start, end);
        self.by_touch = true;
    }

    /// The selection a mouse set, as `selectionByMouse = start to end`.
    fn by_mouse(&mut self, start: usize, end: usize) {
        self.selection = (start, end);
        self.by_touch = false;
    }

    /// Checks what's expected, as the asserter's `assert`; the haptics expected are then reset.
    #[track_caller]
    fn check(&mut self) {
        assert_eq!(self.state.text().as_str(), self.text, "the text");
        let (start, end) = self.selection;
        let (start, end) =
            if self.by_touch { (start.min(end), start.max(end)) } else { (start, end) };
        let expected = TextRange::new(self.byte_in_expected(start), self.byte_in_expected(end));
        assert_eq!(self.state.selection(), expected, "the selection in {:?}", self.text);
        for handle in [Handle::SelectionStart, Handle::SelectionEnd] {
            let nodes =
                self.ui.on_all_nodes(is_selection_handle(Some(handle))).fetch_semantics_nodes();
            let visible = match nodes.as_slice() {
                [] => false,
                [node] => node.config().get(&SELECTION_HANDLE_INFO_KEY).expect("info").visible,
                _ => panic!("found several {handle:?} handles"),
            };
            assert_eq!(visible, self.handles_shown, "{handle:?} shown");
        }
        let mut cursor = self.ui.on_node(is_selection_handle(Some(Handle::Cursor)));
        if self.cursor_handle_shown {
            cursor.assert_exists();
        } else {
            cursor.assert_does_not_exist();
        }
        // The test rule's toolbar, as the spy's `assertShown` (D2041).
        let toolbar = self.ui.text_toolbar().is_shown();
        assert_eq!(toolbar, self.toolbar_shown, "the text toolbar shown");
        let (moves, long) = HAPTICS.with(|count| count.replace((0, 0)));
        assert!(moves >= self.moves, "{moves} TextHandleMove haptics, expected {}", self.moves);
        assert!(
            long >= self.long_presses,
            "{long} LongPress haptics, expected {}",
            self.long_presses
        );
        self.moves = 0;
        self.long_presses = 0;
    }

    /// The expected text's offset `offset` in bytes.
    fn byte_in_expected(&self, offset: usize) -> usize {
        Self::byte_in(&self.text, offset)
    }

    /// Changes what's expected, then checks it, as `applyAndAssert`.
    #[track_caller]
    fn expect(&mut self, change: impl FnOnce(&mut Self)) {
        change(self);
        self.check();
    }
}

const DIRECTIONS: [bool; 2] = [false, true];

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressOutOfBounds_nothingHappens
#[test]
fn whenTouch_withLongPressOutOfBounds_nothingHappens() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.top_start();
        let at = it.nudge(at, None, Some(false));
        it.long_press(at);
        it.expect(|it| it.cursor_handle_shown = true);
        let to = it.top_end();
        let to = it.nudge(to, None, Some(false));
        it.touch_drag_to(to);
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withNoTextThenLongPress_noSelection
#[test]
fn whenTouch_withNoTextThenLongPress_noSelection() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.set_content("");
        it.touch(|touch| touch.click());
        it.expect(|it| it.text = String::new());
        // "Paste will show up if clipboard is not empty": a clip on it, the toolbar's only item
        // where Compose's shows autofill too, which compose-rs hasn't (D2035, D2041).
        it.ui.host().with_tree(|tree| tree.clipboard().set_text("clip"));
        it.touch(|touch| {
            let timeout = touch.view_configuration().double_tap_timeout_millis;
            touch.advance_event_time(timeout * 2);
            let center = touch.center();
            let duration = touch.view_configuration().long_press_timeout_millis + 100;
            touch.long_click_at(center, duration);
        });
        it.toolbar_shown = true;
        it.expect(|it| it.long_presses += 1);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withNoTextThenLongPressAndDrag_noSelection
#[test]
fn whenTouch_withNoTextThenLongPressAndDrag_noSelection() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.set_content("");
        it.touch(|touch| touch.click());
        it.expect(|it| it.text = String::new());
        // "Paste will show up if clipboard is not empty" (D2035, D2041).
        it.ui.host().with_tree(|tree| tree.clipboard().set_text("clip"));
        it.touch(|touch| {
            let timeout = touch.view_configuration().double_tap_timeout_millis;
            touch.advance_event_time(timeout * 2);
        });
        let center = it.center();
        it.long_press(center);
        it.expect(|it| it.long_presses += 1);
        let to = it.center_start();
        it.touch_drag_to(to);
        it.check();
        it.up();
        it.toolbar_shown = true;
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenClear_noSelection
#[test]
fn whenTouch_withLongPressThenClear_noSelection() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.long_click(at);
        it.toolbar_shown = true;
        it.expect(|it| {
            it.by_touch(12, 17);
            it.handles_shown = true;
            it.long_presses += 1;
        });
        let at = it.character(14);
        it.touch(|touch| touch.click_at(at));
        it.toolbar_shown = false;
        it.expect(|it| {
            it.by_touch(14, 14);
            it.handles_shown = false;
            it.cursor_handle_shown = true;
        });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragLeftOutOfBoundsUpAndDown_selectsLines
#[test]
fn whenTouch_withLongPressThenDragLeftOutOfBoundsUpAndDown_selectsLines() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.character(9);
        it.long_press(at);
        // The anchor at the middle line's start.
        it.expect(|it| {
            it.by_touch(6, 11);
            it.long_presses += 1;
        });
        // The middle line's start.
        let to = it.character(6) + Offset::new(-2.0, 0.0);
        it.touch_drag_to(to);
        it.expect(|it| it.by_touch(6, 11));
        // The top line's start.
        let to = it.character(0) + Offset::new(-2.0, 0.0);
        it.touch_drag_to(to);
        it.expect(|it| {
            it.by_touch(11, 0);
            it.moves += 1;
        });
        // Above the top line.
        let to = it.top_start();
        let to = it.nudge(to, None, Some(false));
        it.touch_drag_to(to);
        it.check();
        // Below the bottom line: the end of the text.
        let to = it.bottom_start();
        let to = it.nudge(to, None, Some(true));
        it.touch_drag_to(to);
        it.expect(|it| {
            it.by_touch(6, 29);
            it.moves += 1;
        });
        it.up();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_verifyOneCharStaysSelected_withinLine
#[test]
fn whenTouch_verifyOneCharStaysSelected_withinLine() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.character(14);
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(12, 17);
            it.long_presses += 1;
        });
        let to = it.character(13);
        it.touch_drag_to(to);
        it.expect(|it| it.by_touch(12, 17));
        // No collapsed selection: the one before is kept.
        let to = it.character(12);
        it.touch_drag_to(to);
        it.check();
        let to = it.character(11);
        it.touch_drag_to(to);
        it.expect(|it| {
            it.by_touch(17, 6);
            it.moves += 1;
        });
        it.up();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPress_selectsSingleWord
#[test]
fn whenTouch_withLongPress_selectsSingleWord() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.long_click(at);
        it.toolbar_shown = true;
        it.expect(|it| {
            it.by_touch(12, 17);
            it.handles_shown = true;
            it.long_presses += 1;
        });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressesOnMultipleWords_selectsSingleWords
#[test]
fn whenTouch_withLongPressesOnMultipleWords_selectsSingleWords() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.handles_shown = true;
        it.toolbar_shown = true;
        for (offset, start, end) in
            [(2, 0, 5), (8, 6, 11), (13, 12, 17), (20, 18, 23), (26, 24, 29)]
        {
            let at = it.character(offset);
            it.long_click(at);
            it.expect(|it| {
                it.by_touch(start, end);
                it.long_presses += 1;
            });
        }
    }
}

/// A long press on character 13, dragged to `forward` and back to `backward`: Compose's
/// `touchLongPressThenDragForwardAndBackTest`.
fn long_press_then_drag_forward_and_back(
    rtl: bool,
    forward: usize,
    backward: usize,
    expected: (usize, usize),
) {
    let mut it = field_gestures(rtl);
    let at = it.character(13);
    it.long_press(at);
    it.expect(|it| {
        it.by_touch(12, 17);
        it.long_presses += 1;
    });
    let to = it.character(forward);
    it.touch_drag_to(to);
    it.expect(|it| {
        it.by_touch(expected.0, expected.1);
        it.moves += 1;
    });
    let to = it.character(backward);
    it.touch_drag_to(to);
    it.check();
    it.up();
    it.toolbar_shown = true;
    it.expect(|it| it.handles_shown = true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragLeftAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragLeftAndBack_selectsWordsThenChars() {
    for rtl in DIRECTIONS {
        long_press_then_drag_forward_and_back(rtl, 8, 9, (17, 6));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragUpAndBack_selectsWords
#[test]
fn whenTouch_withLongPressThenDragUpAndBack_selectsWords() {
    for rtl in DIRECTIONS {
        long_press_then_drag_forward_and_back(rtl, 2, 3, (17, 0));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragRightAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragRightAndBack_selectsWordsThenChars() {
    for rtl in DIRECTIONS {
        long_press_then_drag_forward_and_back(rtl, 21, 20, (12, 23));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragDownAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragDownAndBack_selectsWordsThenChars() {
    for rtl in DIRECTIONS {
        long_press_then_drag_forward_and_back(rtl, 27, 26, (12, 29));
    }
}

/// Which end padding a drag ends at: the first, middle or last line's.
#[derive(Clone, Copy)]
enum Place {
    Top,
    Center,
    Bottom,
}

impl FieldGestures {
    fn place(&mut self, place: Place) -> Offset {
        match place {
            Place::Top => self.top_end(),
            Place::Center => self.center_end(),
            Place::Bottom => self.bottom_end(),
        }
    }
}

/// A long press on character 13 dragged to an end padding: Compose's
/// `touchLongPressThenDragToEndPaddingTest`.
fn long_press_then_drag_to_end_padding(rtl: bool, place: Place, expected: (usize, usize)) {
    let mut it = field_gestures(rtl);
    let at = it.character(13);
    it.long_press(at);
    it.expect(|it| {
        it.by_touch(12, 17);
        it.long_presses += 1;
    });
    let to = it.place(place);
    it.touch_drag_to(to);
    it.expect(|it| {
        it.by_touch(expected.0, expected.1);
        it.moves += 1;
    });
    it.up();
    it.toolbar_shown = true;
    it.expect(|it| it.handles_shown = true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragToUpperEndPaddingAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragToUpperEndPaddingAndBack_selectsWordsThenChars() {
    for rtl in DIRECTIONS {
        long_press_then_drag_to_end_padding(rtl, Place::Top, (17, 0));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragToMiddleEndPaddingAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragToMiddleEndPaddingAndBack_selectsWordsThenChars() {
    for rtl in DIRECTIONS {
        long_press_then_drag_to_end_padding(rtl, Place::Center, (12, 23));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThenDragToLowerEndPaddingAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragToLowerEndPaddingAndBack_selectsWordsThenChars() {
    for rtl in DIRECTIONS {
        long_press_then_drag_to_end_padding(rtl, Place::Bottom, (12, 29));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressInEndPadding_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPadding_entersSelectionMode() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.top_end();
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(5, 5);
            it.long_presses += 1;
        });
        // A drag that leaves the selection as it is.
        it.touch_drag_by(Offset::new(-1.0, 0.0));
        it.check();
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.cursor_handle_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingOfEmptyLine_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPaddingOfEmptyLine_entersSelectionMode() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let content = format!("{0}\n\n{0}", it.word());
        it.set_content(&content);
        let at = it.character(0);
        it.touch(|touch| touch.click_at(at));
        it.expect(|it| {
            it.text = content.clone();
            it.cursor_handle_shown = true;
        });
        let at = it.center_end();
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(6, 6);
            it.cursor_handle_shown = false;
            it.long_presses += 1;
        });
        it.touch_drag_by(Offset::new(-1.0, 0.0));
        it.check();
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.cursor_handle_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingThenDragToUpperEndPadding_selectsParagraphAndNewLine
#[test]
fn whenTouch_withLongPressInEndPaddingThenDragToUpperEndPadding_selectsParagraphAndNewLine() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.center_end();
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(23, 23);
            it.long_presses += 1;
        });
        let to = it.top_end();
        it.touch_drag_to(to);
        it.expect(|it| {
            it.by_touch(23, 0);
            it.moves += 1;
        });
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingThenDragToLowerEndPadding_selectsNewLineAndParagraph
#[test]
fn whenTouch_withLongPressInEndPaddingThenDragToLowerEndPadding_selectsNewLineAndParagraph() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.center_end();
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(23, 23);
            it.long_presses += 1;
        });
        let to = it.bottom_end();
        it.touch_drag_to(to);
        it.expect(|it| {
            it.by_touch(18, 29);
            it.moves += 1;
        });
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingOfFinalLine_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPaddingOfFinalLine_entersSelectionMode() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.bottom_end();
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(29, 29);
            it.long_presses += 1;
        });
        it.touch_drag_by(Offset::new(-1.0, 0.0));
        it.check();
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.cursor_handle_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressThanDragAcrossSingleWord_onlySelectsSingleWordAndNoOtherChanges
#[test]
fn whenTouch_withLongPressThanDragAcrossSingleWord_onlySelectsSingleWordAndNoOtherChanges() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.character(15);
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(12, 17);
            it.long_presses += 1;
        });
        let to = it.character(13);
        it.touch_drag_to(to);
        it.expect(|it| it.by_touch(12, 17));
        let to = it.character(15);
        it.touch_drag_to(to);
        it.expect(|it| it.by_touch(12, 17));
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingOfEmptyFinalLine_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPaddingOfEmptyFinalLine_entersSelectionMode() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let content = format!("{}\n\n", it.word());
        it.set_content(&content);
        let at = it.character(0);
        it.touch(|touch| touch.click_at(at));
        it.expect(|it| {
            it.text = content.clone();
            it.cursor_handle_shown = true;
        });
        let at = it.bottom_end();
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(7, 7);
            it.cursor_handle_shown = false;
            it.long_presses += 1;
        });
        it.touch_drag_by(Offset::new(-1.0, 0.0));
        it.check();
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.cursor_handle_shown = true);
    }
}

/// `taps` taps on `at`, each a click, as `repeat(taps) { click(…) }`.
fn taps(it: &mut FieldGestures, taps: u32, at: Offset) {
    it.touch(|touch| {
        for _ in 0..taps {
            touch.click_at(at);
        }
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withDoubleClick_selectsWord
#[test]
fn whenTouch_withDoubleClick_selectsWord() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        taps(&mut it, 2, at);
        it.toolbar_shown = true;
        it.expect(|it| {
            it.handles_shown = true;
            it.by_touch(12, 17);
        });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withTripleClick_selectsParagraph
#[test]
fn whenTouch_withTripleClick_selectsParagraph() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        taps(&mut it, 3, at);
        it.toolbar_shown = true;
        it.expect(|it| {
            it.handles_shown = true;
            it.by_touch(6, 23);
        });
    }
}

/// `taps` presses on character 13, the last held, then a drag to `end`: Compose's
/// `touchTapsThenDragTest`.
fn taps_then_drag(rtl: bool, taps: u32, end: usize, first: (usize, usize), last: (usize, usize)) {
    let mut it = field_gestures(rtl);
    let at = it.character(13);
    it.touch(|touch| {
        let period = touch.event_period_millis();
        touch.down(at);
        for _ in 1..taps {
            touch.advance_event_time(period);
            touch.up();
            touch.advance_event_time(period);
            touch.down(at);
        }
    });
    // A finger waits for a long press or for the tap to end.
    it.ui.advance_time_by(std::time::Duration::from_millis(1_000));
    it.expect(|it| it.by_touch(first.0, first.1));
    let to = it.character(end);
    it.touch_drag_to(to);
    it.expect(|it| it.by_touch(last.0, last.1));
    it.up();
    it.toolbar_shown = true;
    it.expect(|it| it.handles_shown = true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragLeft_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragLeft_selectsWords() {
    for rtl in DIRECTIONS {
        taps_then_drag(rtl, 2, 8, (12, 17), (17, 6));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragUp_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragUp_selectsWords() {
    for rtl in DIRECTIONS {
        taps_then_drag(rtl, 2, 2, (12, 17), (17, 0));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragRight_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragRight_selectsWords() {
    for rtl in DIRECTIONS {
        taps_then_drag(rtl, 2, 19, (12, 17), (12, 23));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragDown_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragDown_selectsWords() {
    for rtl in DIRECTIONS {
        taps_then_drag(rtl, 2, 26, (12, 17), (12, 29));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withTripleClickThenDragUp_selectsParagraphs
#[test]
fn whenTouch_withTripleClickThenDragUp_selectsParagraphs() {
    for rtl in DIRECTIONS {
        taps_then_drag(rtl, 3, 2, (6, 23), (23, 0));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_withTripleClickThenDragDown_selectsParagraphs
#[test]
fn whenTouch_withTripleClickThenDragDown_selectsParagraphs() {
    for rtl in DIRECTIONS {
        taps_then_drag(rtl, 3, 26, (6, 23), (6, 29));
    }
}

fn directions_and_cursors() -> impl Iterator<Item = (bool, Cursor)> {
    DIRECTIONS.into_iter().flat_map(|rtl| CURSORS.map(|kind| (rtl, kind)))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withLongClick_collapsedSelectionAtClick, whenTrackpad_withLongClick_collapsedSelectionAtClick
#[test]
fn whenMouse_withLongClick_collapsedSelectionAtClick() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| cursor.long_click(at));
        it.expect(|it| it.by_mouse(13, 13));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withClick_collapsedSelectionAtClick, whenTrackpad_withClick_collapsedSelectionAtClick
#[test]
fn whenMouse_withClick_collapsedSelectionAtClick() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| cursor.click(at));
        it.expect(|it| it.by_mouse(13, 13));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withSingleClick_collapsedSelection, whenTrackpad_withSingleClick_collapsedSelection
#[test]
fn whenMouse_withSingleClick_collapsedSelection() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| {
            cursor.move_to(at);
            cursor.press();
        });
        it.expect(|it| it.by_mouse(13, 13));
        it.cursor(kind, |cursor| cursor.release());
        it.check();
    }
}

/// `clicks` clicks at `at`, the last held, a drag to `to`, then the release: Compose's
/// `mouseClicksThenDragTest` and `trackpadClicksThenDragTest`.
fn clicks_then_drag_from(
    it: &mut FieldGestures,
    kind: Cursor,
    clicks: u32,
    at: Offset,
    to: Offset,
    first: (usize, usize),
    last: (usize, usize),
) {
    it.cursor(kind, |cursor| {
        cursor.move_to(at);
        cursor.press();
        for _ in 1..clicks {
            cursor.advance_event_time(16);
            cursor.release();
            cursor.advance_event_time(16);
            cursor.press();
        }
    });
    it.expect(|it| it.by_mouse(first.0, first.1));
    it.cursor_drag_to(kind, to);
    it.expect(|it| it.by_mouse(last.0, last.1));
    it.cursor(kind, |cursor| cursor.release());
    it.check();
}

/// From character 13 to character `end`.
fn clicks_then_drag(clicks: u32, end: usize, first: (usize, usize), last: (usize, usize)) {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        let to = it.character(end);
        clicks_then_drag_from(&mut it, kind, clicks, at, to, first, last);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragLeft_selectsCharacters, whenTrackpad_withSingleClickThenDragLeft_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragLeft_selectsCharacters() {
    clicks_then_drag(1, 8, (13, 13), (13, 8));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragUp_selectsCharacters, whenTrackpad_withSingleClickThenDragUp_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragUp_selectsCharacters() {
    clicks_then_drag(1, 2, (13, 13), (13, 2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragRight_selectsCharacters, whenTrackpad_withSingleClickThenDragRight_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragRight_selectsCharacters() {
    clicks_then_drag(1, 19, (13, 13), (13, 19));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragDown_selectsCharacters, whenTrackpad_withSingleClickThenDragDown_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragDown_selectsCharacters() {
    clicks_then_drag(1, 26, (13, 13), (13, 26));
}

/// `clicks` clicks on character 13, as `repeat(clicks) { click(…) }`.
fn clicks_on_13(clicks: u32, expected: (usize, usize), read_only: bool) {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        if read_only {
            it.set_read_only(true);
        }
        let at = it.character(13);
        it.cursor(kind, |cursor| {
            for _ in 0..clicks {
                cursor.click(at);
            }
        });
        it.expect(|it| it.by_mouse(expected.0, expected.1));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withDoubleClick_selectsWord, whenTrackpad_withDoubleClick_selectsWord
#[test]
fn whenMouse_withDoubleClick_selectsWord() {
    clicks_on_13(2, (12, 17), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragLeft_selectsWords, whenTrackpad_withDoubleClickThenDragLeft_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragLeft_selectsWords() {
    clicks_then_drag(2, 8, (12, 17), (17, 6));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragUp_selectsWords, whenTrackpad_withDoubleClickThenDragUp_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragUp_selectsWords() {
    clicks_then_drag(2, 2, (12, 17), (17, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragRight_selectsWords, whenTrackpad_withDoubleClickThenDragRight_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragRight_selectsWords() {
    clicks_then_drag(2, 19, (12, 17), (12, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragDown_selectsWords, whenTrackpad_withDoubleClickThenDragDown_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragDown_selectsWords() {
    clicks_then_drag(2, 26, (12, 17), (12, 29));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withTripleClick_selectsParagraph, whenTrackpad_withTripleClick_selectsParagraph
#[test]
fn whenMouse_withTripleClick_selectsParagraph() {
    clicks_on_13(3, (6, 23), false);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragLeft_selectsParagraphs, whenTrackpad_withTripleClickThenDragLeft_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragLeft_selectsParagraphs() {
    clicks_then_drag(3, 8, (6, 23), (6, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragUp_selectsParagraphs, whenTrackpad_withTripleClickThenDragUp_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragUp_selectsParagraphs() {
    clicks_then_drag(3, 2, (6, 23), (23, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragRight_selectsParagraphs, whenTrackpad_withTripleClickThenDragRight_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragRight_selectsParagraphs() {
    clicks_then_drag(3, 19, (6, 23), (6, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragDown_selectsParagraphs, whenTrackpad_withTripleClickThenDragDown_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragDown_selectsParagraphs() {
    clicks_then_drag(3, 26, (6, 23), (6, 29));
}

/// `clicks` clicks on the middle line's first letter, then a pixel down: Compose's
/// `mouseFirstLetterOfLineClicksTest` and `trackpadFirstLetterOfLineClicksTest`.
fn first_letter_of_line_clicks(clicks: u32, selection: (usize, usize)) {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(6);
        let to = at + Offset::new(0.0, 1.0);
        // The cursor handle the first tap showed is over this line's first letter: Compose's
        // events go to the field's window, past the handle's popup; here the popup would take
        // them (D2017). The cursor hovering the field first hides the handle, as it does there.
        let center = it.center();
        it.cursor(kind, |cursor| cursor.move_to(center));
        clicks_then_drag_from(&mut it, kind, clicks, at, to, selection, selection);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withSingleClickOnFirstLetterOfLine_collapsedSelection, whenTrackpad_withSingleClickOnFirstLetterOfLine_collapsedSelection
#[test]
fn whenMouse_withSingleClickOnFirstLetterOfLine_collapsedSelection() {
    first_letter_of_line_clicks(1, (6, 6));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withDoubleClickOnFirstLetterOfLine_selectsFirstWord, whenTrackpad_withDoubleClickOnFirstLetterOfLine_selectsFirstWord
#[test]
fn whenMouse_withDoubleClickOnFirstLetterOfLine_selectsFirstWord() {
    first_letter_of_line_clicks(2, (6, 11));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withTripleClickOnFirstLetterOfLine_selectsParagraph, whenTrackpad_withTripleClickOnFirstLetterOfLine_selectsParagraph
#[test]
fn whenMouse_withTripleClickOnFirstLetterOfLine_selectsParagraph() {
    first_letter_of_line_clicks(3, (6, 23));
}

/// `clicks` clicks in the middle line's end padding, then two pixels down: Compose's
/// `mouseEndPaddingClicksTest` and `trackpadEndPaddingClicksTest`.
fn end_padding_clicks(clicks: u32, selection: (usize, usize)) {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.center_end();
        let to = it.nudge(at, None, Some(true));
        clicks_then_drag_from(&mut it, kind, clicks, at, to, selection, selection);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withSingleClickInEndPaddingOfLine_collapsedSelection, whenTrackpad_withSingleClickInEndPaddingOfLine_collapsedSelection
#[test]
fn whenMouse_withSingleClickInEndPaddingOfLine_collapsedSelection() {
    end_padding_clicks(1, (23, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withDoubleClickOInEndPaddingOfLine_selectsLastWord, whenTrackpad_withDoubleClickOInEndPaddingOfLine_selectsLastWord
#[test]
fn whenMouse_withDoubleClickOInEndPaddingOfLine_selectsLastWord() {
    end_padding_clicks(2, (18, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_withTripleClickInEndPaddingOfLine_selectsParagraph, whenTrackpad_withTripleClickInEndPaddingOfLine_selectsParagraph
#[test]
fn whenMouse_withTripleClickInEndPaddingOfLine_selectsParagraph() {
    end_padding_clicks(3, (6, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_thenTouch_touchBehaviorsAppear, whenTrackpad_thenTouch_touchBehaviorsAppear
#[test]
fn whenMouse_thenTouch_touchBehaviorsAppear() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| {
            cursor.click(at);
            cursor.click(at);
        });
        it.expect(|it| it.by_mouse(12, 17));
        it.enter_touch_mode();
        it.toolbar_shown = true;
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouch_thenMouse_touchBehaviorsDisappear, whenTouch_thenTrackpad_touchBehaviorsDisappear
#[test]
fn whenTouch_thenMouse_touchBehaviorsDisappear() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.long_click(at);
        it.toolbar_shown = true;
        it.expect(|it| {
            it.by_touch(12, 17);
            it.handles_shown = true;
            it.long_presses += 1;
        });
        it.enter_cursor_mode(kind);
        it.toolbar_shown = false;
        it.expect(|it| it.handles_shown = false);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouseCollapsedSelection_thenTouch_ToolbarAndCursorAppears, whenTrackpadCollapsedSelection_thenTouch_ToolbarAndCursorAppears
#[test]
fn whenMouseCollapsedSelection_thenTouch_ToolbarAndCursorAppears() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| cursor.click(at));
        it.expect(|it| it.by_mouse(13, 13));
        it.enter_touch_mode();
        it.expect(|it| it.cursor_handle_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenTouchCollapsedSelection_thenMouse_noUiElements, whenTouchCollapsedSelection_thenTrackpad_noUiElements
#[test]
fn whenTouchCollapsedSelection_thenMouse_noUiElements() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.character(13);
        it.touch(|touch| touch.click_at(at));
        it.expect(|it| {
            it.by_touch(13, 13);
            it.cursor_handle_shown = true;
        });
        it.enter_cursor_mode(kind);
        it.expect(|it| it.cursor_handle_shown = false);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenMouse_thenTripleClickInEndPadding_selectsCurrentParagraph, whenTrackpad_thenTripleClickInEndPadding_selectsCurrentParagraph
#[test]
fn whenMouse_thenTripleClickInEndPadding_selectsCurrentParagraph() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = field_gestures(rtl);
        let at = it.center_end();
        it.cursor(kind, |cursor| {
            for _ in 0..3 {
                cursor.click(at);
            }
        });
        it.expect(|it| it.by_mouse(6, 23));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenReadOnly_touchTap_setsCollapsedSelection
#[test]
fn whenReadOnly_touchTap_setsCollapsedSelection() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.set_read_only(true);
        let at = it.character(13);
        it.touch(|touch| touch.click_at(at));
        it.expect(|it| it.by_touch(13, 13));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenReadOnly_touchLongPress_startsSelection
#[test]
fn whenReadOnly_touchLongPress_startsSelection() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.set_read_only(true);
        let at = it.character(13);
        it.long_press(at);
        it.expect(|it| {
            it.by_touch(12, 17);
            it.long_presses += 1;
        });
        it.up();
        it.toolbar_shown = true;
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenReadOnly_mouseSingleClick_setsCollapsedSelection
#[test]
fn whenReadOnly_mouseSingleClick_setsCollapsedSelection() {
    clicks_on_13(1, (13, 13), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenReadOnly_mouseDoubleClick_selectsWord
#[test]
fn whenReadOnly_mouseDoubleClick_selectsWord() {
    clicks_on_13(2, (12, 17), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenReadOnly_mouseTripleClick_selectsParagraph
#[test]
fn whenReadOnly_mouseTripleClick_selectsParagraph() {
    clicks_on_13(3, (6, 23), true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenDisabled_touchTap_doesNothing
#[test]
fn whenDisabled_touchTap_doesNothing() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.set_enabled(false);
        let at = it.character(13);
        it.touch(|touch| touch.click_at(at));
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenDisabled_touchLongPress_doesNothing
#[test]
fn whenDisabled_touchLongPress_doesNothing() {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.set_enabled(false);
        let at = it.character(13);
        it.long_press(at);
        it.check();
        it.up();
        it.check();
    }
}

/// `clicks` clicks on character 13 of a disabled field: Compose's `whenDisabled_mouse*`.
fn disabled_clicks(clicks: u32) {
    for rtl in DIRECTIONS {
        let mut it = field_gestures(rtl);
        it.set_enabled(false);
        let at = it.character(13);
        it.cursor(Cursor::Mouse, |cursor| {
            for _ in 0..clicks {
                cursor.click(at);
            }
        });
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenDisabled_mouseSingleClick_doesNothing
#[test]
fn whenDisabled_mouseSingleClick_doesNothing() {
    disabled_clicks(1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenDisabled_mouseDoubleClick_doesNothing
#[test]
fn whenDisabled_mouseDoubleClick_doesNothing() {
    disabled_clicks(2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextFieldSelectionGesturesTest.kt, whenDisabled_mouseTripleClick_doesNothing
#[test]
fn whenDisabled_mouseTripleClick_doesNothing() {
    disabled_clicks(3);
}

/// `handle` dragged right by the touch slop and `distance` more, from its center, over 200 ms.
fn drag_handle(it: &mut FieldGestures, handle: Handle, distance: f32) {
    it.ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let slop = touch.view_configuration().touch_slop;
        let (x, y) = (touch.center_x(), touch.center_y());
        touch.swipe(Offset::new(x, y), Offset::new(x + slop + distance, y), 200);
    });
    it.ui.wait_for_idle();
}

fn performed() -> (u32, u32) {
    HAPTICS.with(|count| count.replace((0, 0)))
}

// Not Compose's (no test of its own checks them): the handles' drags vibrate as Compose's
// `TextFieldSelectionState`'s, at each move that changes the selection (D2027).
#[test]
fn a_handles_drag_vibrates_when_it_moves_the_selection() {
    let mut it = field_gestures(false);
    // The cursor handle a third of a character on: the cursor stays.
    drag_handle(&mut it, Handle::Cursor, FONT / 3.0);
    assert_eq!(it.state.selection(), TextRange::at(0));
    assert_eq!(performed(), (0, 0));
    // Three characters on: the cursor follows, each new place felt.
    drag_handle(&mut it, Handle::Cursor, FONT * 3.0);
    assert_eq!(it.state.selection(), TextRange::at(3));
    assert!(performed().0 >= 1);
    // A selection's end handle dragged into the next word: by words, felt.
    let at = it.character(13);
    it.long_click(at);
    assert_eq!(it.state.selection(), TextRange::new(12, 17));
    performed();
    drag_handle(&mut it, Handle::SelectionEnd, FONT * 2.0);
    assert_eq!(it.state.selection(), TextRange::new(12, 23));
    assert!(performed().0 >= 1);
}

// Not Compose's: a mouse's tap shows the cursor handle once a finger comes back, as Compose's
// `onTapTextField` sets `showCursorHandle` whatever tapped, here after a long press hid it
// (D2027).
#[test]
fn a_mouses_tap_shows_the_cursor_handle_once_a_finger_comes_back() {
    for kind in CURSORS {
        let mut it = field_gestures(false);
        let at = it.character(13);
        it.long_click(at);
        it.toolbar_shown = true;
        it.expect(|it| {
            it.by_touch(12, 17);
            it.handles_shown = true;
            it.long_presses += 1;
        });
        it.cursor(kind, |cursor| cursor.click(at));
        it.toolbar_shown = false;
        it.expect(|it| {
            it.by_mouse(13, 13);
            it.handles_shown = false;
        });
        it.enter_touch_mode();
        it.expect(|it| it.cursor_handle_shown = true);
    }
}

// Not Compose's: a mouse coming into the field ends the touch interaction, whatever its event,
// as Compose's `detectDirectTouchInteraction` hears every event (D2027).
#[test]
fn a_mouse_entering_the_field_hides_the_handles() {
    let mut it = field_gestures(false);
    let at = it.character(13);
    it.long_click(at);
    it.toolbar_shown = true;
    it.expect(|it| {
        it.by_touch(12, 17);
        it.handles_shown = true;
        it.long_presses += 1;
    });
    let center = it.center();
    it.ui.on_node_with_tag(TAG).perform_mouse_input(|mouse| mouse.enter(center));
    it.ui.wait_for_idle();
    it.toolbar_shown = false;
    it.expect(|it| it.handles_shown = false);
}

// Not Compose's: a mouse's event on a handle ends the touch interaction, as Compose's handles
// run `detectDirectTouchInteraction` too (D2027). Its press stays the handle's here, where
// Compose's test would send it to the field's window (D2017).
#[test]
fn a_mouse_on_the_cursor_handle_hides_it() {
    for kind in CURSORS {
        let mut it = field_gestures(false);
        // The cursor handle the first tap showed hangs over the middle line's first letter.
        let at = it.character(6);
        it.cursor(kind, |cursor| cursor.click(at));
        it.check();
    }
}
