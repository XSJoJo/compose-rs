//! A `SelectionContainer`'s gestures over one text, from Compose's `TextSelectionGesturesTest` as
//! `SingleTextSelectionGesturesTest` and `SingleTextSelectionGesturesRtlTest` run it (each test
//! in both directions): a finger's long press and its drags, a mouse's and a trackpad's clicks
//! and drags (each mouse test with the trackpad too), the input mode switching the handles.
//!
//! As Compose's tests: the test font at 15 sp, density 1, a touch slop of 0.1, the container
//! inset by 32 dp and wrapping its text, which fills its width. Each check is the asserter's:
//! the selection, the handles shown, and at least as many `TextHandleMove` haptics as expected
//! since the last check. The toolbar's checks wait on `text-context-menu`; no magnifier shows
//! (Compose's checks are skipped where the platform has none).

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    Density, InputMode, IntSize, LayoutDirection, Offset, Rect, TextLayoutResult, UnitExt,
};

use crate::text_field_touch_support::*;

const TAG: &str = "selectionContainer";
const TEXT_TAG: &str = "testTag";
const FONT: f32 = 15.0;
/// Compose's `RtlChar`, a Hebrew letter.
const RTL_CHAR: char = '\u{05D1}';

thread_local! {
    static HAPTICS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

struct CountingHaptics;

impl compose_ui::HapticFeedback for CountingHaptics {
    fn perform_haptic_feedback(&self, feedback: compose_ui::HapticFeedbackType) {
        if feedback == compose_ui::HapticFeedbackType::TextHandleMove {
            HAPTICS.with(|count| count.set(count.get() + 1));
        }
    }
}

/// What the mouse tests run with: Compose's `whenMouse_*` and `whenTrackpad_*` twins.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Cursor {
    Mouse,
    Trackpad,
}

pub(crate) const CURSORS: [Cursor; 2] = [Cursor::Mouse, Cursor::Trackpad];

/// A mouse's or a trackpad's injection scope, with their primary buttons.
pub(crate) enum CursorScope<'a, 'b> {
    Mouse(&'a mut MouseInjectionScope<'b>),
    Trackpad(&'a mut TrackpadInjectionScope<'b>),
}

impl CursorScope<'_, '_> {
    pub fn current_position(&self) -> Offset {
        match self {
            CursorScope::Mouse(mouse) => mouse.current_position(),
            CursorScope::Trackpad(trackpad) => trackpad.current_position(),
        }
    }

    pub fn move_to_after(&mut self, to: Offset, delay_millis: u64) {
        match self {
            CursorScope::Mouse(mouse) => mouse.move_to_after(to, delay_millis),
            CursorScope::Trackpad(trackpad) => trackpad.move_to_after(to, delay_millis),
        }
    }

    pub fn move_to(&mut self, to: Offset) {
        self.move_to_after(to, 16);
    }

    pub fn press(&mut self) {
        match self {
            CursorScope::Mouse(mouse) => mouse.press(MouseButton::Primary),
            CursorScope::Trackpad(trackpad) => trackpad.press(TrackpadButton::Primary),
        }
    }

    pub fn release(&mut self) {
        match self {
            CursorScope::Mouse(mouse) => mouse.release(MouseButton::Primary),
            CursorScope::Trackpad(trackpad) => trackpad.release(TrackpadButton::Primary),
        }
    }

    pub fn click(&mut self, at: Offset) {
        match self {
            CursorScope::Mouse(mouse) => mouse.click_at(at, MouseButton::Primary),
            CursorScope::Trackpad(trackpad) => trackpad.click_at(at, TrackpadButton::Primary),
        }
    }

    pub fn long_click(&mut self, at: Offset) {
        match self {
            CursorScope::Mouse(mouse) => mouse.long_click(at),
            CursorScope::Trackpad(trackpad) => trackpad.long_click(at, TrackpadButton::Primary),
        }
    }

    pub fn advance_event_time(&mut self, millis: u64) {
        match self {
            CursorScope::Mouse(mouse) => mouse.advance_event_time(millis),
            CursorScope::Trackpad(trackpad) => trackpad.advance_event_time(millis),
        }
    }
}

/// The rule, its text's content (a state, as Compose's `textContent`), and what's expected.
pub(crate) struct Gestures {
    pub ui: ComposeTestRule,
    content: compose_runtime::MutableState<String>,
    rtl: bool,
    /// The selection expected, `(start, end)` as Compose's `TextRange` (crossed when start > end).
    pub selection: Option<(usize, usize)>,
    pub handles_shown: bool,
    pub start_shown: Option<bool>,
    pub end_shown: Option<bool>,
    pub haptics: u32,
}

/// As Compose's `setTextFieldTestContent` around `Content()`: the container inset by 32 dp in a
/// window of 400 by 800 px (a phone's shape), wrapping its text.
pub(crate) fn gestures(rtl: bool) -> Gestures {
    // `SingleTextSelectionGesturesTest`'s, or `SingleTextSelectionGesturesRtlTest`'s: words of
    // five letters, laid out right to left.
    let text = if rtl {
        let word = RTL_CHAR.to_string().repeat(5);
        format!("{word}\n{word} {word} {word}\n{word}")
    } else {
        "line1\nline2 text1 text2\nline3".to_string()
    };
    let content = Box::leak(Box::new(compose_runtime::mutable_state_of(text))).handle();
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    HAPTICS.with(|count| count.set(0));
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
                    compose_runtime::CompositionLocalProvider(
                        compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                        move || {
                            Box!(
                                modifier =
                                    Modifier.padding(32.dp()).fill_max_size().wrap_content_size(),
                                {
                                    SelectionContainer!(modifier = Modifier.test_tag(TAG), {
                                        BasicText!(
                                            content.get(),
                                            modifier = Modifier.fill_max_width().test_tag(TEXT_TAG),
                                            style = TextStyle {
                                                font_size: FONT.sp(),
                                                ..TextStyle::DEFAULT
                                            },
                                        );
                                    });
                                }
                            );
                        },
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
    Gestures {
        ui,
        content,
        rtl,
        selection: None,
        handles_shown: false,
        start_shown: None,
        end_shown: None,
        haptics: 0,
    }
}

impl Gestures {
    /// The tests' `word`, for one-off contents.
    pub fn word(&self) -> String {
        if self.rtl { RTL_CHAR.to_string().repeat(5) } else { "hello".to_string() }
    }

    /// Compose's offset `offset` (in UTF-16 units, one per character here) in bytes.
    fn byte(&self, offset: usize) -> usize {
        let text = self.content.get();
        text.char_indices().nth(offset).map_or(text.len(), |(byte, _)| byte)
    }

    /// The text shown is `text`, as the tests' `textContent.value = …`.
    pub fn set_content(&mut self, text: &str) {
        self.content.set(text.to_string());
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

    pub fn top_start(&mut self) -> Offset {
        let top = self.bounds().top + 1.0;
        Offset::new(self.start_x(), top)
    }

    pub fn top_end(&mut self) -> Offset {
        let top = self.bounds().top + 1.0;
        Offset::new(self.end_x(), top)
    }

    pub fn center(&mut self) -> Offset {
        self.bounds().center()
    }

    pub fn center_start(&mut self) -> Offset {
        let y = self.bounds().center().y;
        Offset::new(self.start_x(), y)
    }

    pub fn center_end(&mut self) -> Offset {
        let y = self.bounds().center().y;
        Offset::new(self.end_x(), y)
    }

    pub fn bottom_start(&mut self) -> Offset {
        let bottom = self.bounds().bottom - 1.0;
        Offset::new(self.start_x(), bottom)
    }

    pub fn bottom_end(&mut self) -> Offset {
        let bottom = self.bounds().bottom - 1.0;
        Offset::new(self.end_x(), bottom)
    }

    /// Two pixels toward the end (`end`) or the start, and down (`down`) or up, as `nudge`.
    pub fn nudge(&self, at: Offset, x: Option<bool>, y: Option<bool>) -> Offset {
        let toward = |end: bool| if end != self.rtl { 2.0 } else { -2.0 };
        let dx = x.map_or(0.0, toward);
        let dy = y.map_or(0.0, |down| if down { 2.0 } else { -2.0 });
        Offset::new(at.x + dx, at.y + dy)
    }

    fn layout(&mut self) -> TextLayoutResult {
        let mut results = Vec::new();
        self.ui.on_node_with_tag(TEXT_TAG).perform_semantics_action_with(
            &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
            |get| {
                get(&mut results);
            },
        );
        results.pop().expect("a text layout")
    }

    /// Character `offset`'s box at its start side's middle, nudged to its end, as the single
    /// texts' `characterPosition` (`centerLeft`, or `centerRight` right to left).
    pub fn character(&mut self, offset: usize) -> Offset {
        let byte = self.byte(offset);
        let rect = self.layout().get_bounding_box(byte);
        let side = Offset::new(if self.rtl { rect.right } else { rect.left }, rect.center().y);
        self.nudge(side, Some(true), None)
    }

    /// Character `offset`'s box in the text (the container's coordinates).
    pub fn character_box(&mut self, offset: usize) -> Rect {
        let byte = self.byte(offset);
        self.layout().get_bounding_box(byte)
    }

    /// Character `offset`'s box at its left side's middle, as the handle tests'
    /// `characterPosition`.
    pub fn character_center_left(&mut self, offset: usize) -> Offset {
        let rect = self.character_box(offset);
        Offset::new(rect.left, rect.center().y)
    }

    pub fn touch(&mut self, block: impl FnOnce(&mut TouchInjectionScope<'_>)) {
        self.ui.on_node_with_tag(TAG).perform_touch_input(block);
        self.ui.wait_for_idle();
    }

    /// A finger held at `at` past the long-press timeout, as Compose's `longPress`.
    pub fn long_press(&mut self, at: Offset) {
        self.touch(|touch| {
            let timeout = touch.view_configuration().long_press_timeout_millis;
            touch.down(at);
            touch.move_pointers_after(timeout + 10);
        });
    }

    pub fn long_click(&mut self, at: Offset) {
        self.touch(|touch| {
            let duration = touch.view_configuration().long_press_timeout_millis + 100;
            touch.long_click_at(at, duration);
        });
    }

    /// The finger moved to `to` over 200 ms, a move each event period, as `touchDragNodeTo`.
    pub fn touch_drag_to(&mut self, to: Offset) {
        self.touch(|touch| {
            let from = touch.current_position(0).expect("a finger down");
            let period = touch.event_period_millis();
            let steps = ((200.0 / period as f32).round() as u64).max(1);
            let mut previous = 0;
            for step in 1..=steps {
                let time = 200 * step / steps;
                let fraction = time as f32 / 200.0;
                touch.move_pointer_to(0, from + (to - from) * fraction, time - previous);
                previous = time;
            }
        });
    }

    pub fn touch_drag_by(&mut self, by: Offset) {
        let mut at = Offset::ZERO;
        self.touch(|touch| at = touch.current_position(0).expect("a finger down"));
        self.touch_drag_to(at + by);
    }

    pub fn up(&mut self) {
        self.touch(|touch| touch.up());
    }

    /// A mouse's or a trackpad's input on the container, as `performMouseGesture` and
    /// `performTrackpadGesture`.
    pub fn cursor(&mut self, kind: Cursor, block: impl FnOnce(&mut CursorScope<'_, '_>)) {
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

    /// The cursor moved to `to` over 200 ms, as `mouseDragNodeTo` and `trackpadDragNodeTo`.
    pub fn cursor_drag_to(&mut self, kind: Cursor, to: Offset) {
        self.cursor(kind, |cursor| {
            let from = cursor.current_position();
            let steps = ((200.0 / 16.0f32).round() as u64).max(1);
            let mut previous = 0;
            for step in 1..=steps {
                let time = 200 * step / steps;
                let fraction = time as f32 / 200.0;
                cursor.move_to_after(from + (to - from) * fraction, time - previous);
                previous = time;
            }
        });
    }

    /// As `enterTouchMode`: a swipe from the center to below the container.
    pub fn enter_touch_mode(&mut self) {
        let bounds = self.bounds();
        let (from, to) = (bounds.center(), Offset::new(bounds.center().x, bounds.bottom + 10.0));
        self.touch(|touch| touch.swipe(from, to, 200));
    }

    /// As `enterMouseMode` and `enterTrackpadMode`: the cursor moved, no button pressed, left,
    /// to the bottom right and back.
    pub fn enter_cursor_mode(&mut self, kind: Cursor) {
        let bounds = self.bounds();
        self.cursor_drag_to(kind, Offset::new(bounds.left, bounds.center().y));
        self.cursor_drag_to(kind, Offset::new(bounds.right, bounds.bottom));
        self.cursor_drag_to(kind, bounds.center());
    }

    /// Checks what's expected, as the asserter's `assert`; the haptics expected are then reset.
    #[track_caller]
    pub fn check(&mut self) {
        let found = self
            .ui
            .host()
            .with_tree(|tree| tree.selection_anchors())
            .map(|((_, start), (_, end))| (start, end));
        let expected = self.selection.map(|(start, end)| (self.byte(start), self.byte(end)));
        assert_eq!(found, expected, "the selection in {:?}", self.content.get());
        let start = self.start_shown.unwrap_or(self.handles_shown);
        let end = self.end_shown.unwrap_or(self.handles_shown);
        for (handle, shown) in [(Handle::SelectionStart, start), (Handle::SelectionEnd, end)] {
            let nodes =
                self.ui.on_all_nodes(is_selection_handle(Some(handle))).fetch_semantics_nodes();
            let visible = match nodes.as_slice() {
                [] => false,
                [node] => node.config().get(&SELECTION_HANDLE_INFO_KEY).expect("info").visible,
                _ => panic!("found several {handle:?} handles"),
            };
            assert_eq!(visible, shown, "{handle:?} shown");
        }
        let performed = HAPTICS.with(|count| count.replace(0));
        assert!(performed >= self.haptics, "{performed} haptics, expected {}", self.haptics);
        self.haptics = 0;
    }

    /// Changes what's expected, then checks it, as `applyAndAssert`.
    #[track_caller]
    pub fn expect(&mut self, change: impl FnOnce(&mut Self)) {
        change(self);
        self.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressOutOfBounds_nothingHappens
#[test]
fn whenTouch_withLongPressOutOfBounds_nothingHappens() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.top_start();
        let at = it.nudge(at, None, Some(false));
        it.long_press(at);
        it.check();
        let to = it.top_end();
        let to = it.nudge(to, None, Some(false));
        it.touch_drag_to(to);
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withNoTextThenLongPress_noSelection
#[test]
fn whenTouch_withNoTextThenLongPress_noSelection() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        it.set_content("");
        let center = it.center();
        it.long_click(center);
        it.expect(|it| it.selection = Some((0, 0)));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withNoTextThenLongPressAndDrag_noSelection
#[test]
fn whenTouch_withNoTextThenLongPressAndDrag_noSelection() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        it.set_content("");
        let center = it.center();
        it.long_press(center);
        it.expect(|it| it.selection = Some((0, 0)));
        let to = it.center_start();
        it.touch_drag_to(to);
        it.check();
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingOfEmptyLine_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPaddingOfEmptyLine_entersSelectionMode() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let word = it.word();
        it.set_content(&format!("{word}\n\n{word}"));
        let at = it.center_end();
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((6, 6));
            it.haptics += 1;
        });
        it.touch_drag_by(Offset::new(-1.0, 0.0));
        it.check();
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingOfEmptyFinalLine_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPaddingOfEmptyFinalLine_entersSelectionMode() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let word = it.word();
        it.set_content(&format!("{word}\n\n"));
        let at = it.bottom_end();
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((7, 7));
            it.handles_shown = false;
            it.haptics += 1;
        });
        let by = it.nudge(Offset::ZERO, Some(false), None);
        it.touch_drag_by(by);
        it.check();
        let to = it.center_end();
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some((7, 6));
            it.handles_shown = true;
            it.haptics += 1;
        });
        let to = it.top_end();
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some((7, 0));
            it.haptics += 1;
        });
        let to = it.top_start();
        it.touch_drag_to(to);
        it.check();
        let to = it.top_end();
        it.touch_drag_to(to);
        it.check();
        let to = it.center_end();
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some((7, 6));
            it.haptics += 1;
        });
        it.up();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenClear_noSelection
#[test]
fn whenTouch_withLongPressThenClear_noSelection() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.long_click(at);
        it.expect(|it| {
            it.selection = Some((12, 17));
            it.handles_shown = true;
            it.haptics += 1;
        });
        let at = it.character(14);
        it.touch(|touch| touch.click_at(at));
        it.expect(|it| {
            it.selection = None;
            it.handles_shown = false;
            it.haptics += 1;
        });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragLeftOutOfBoundsUpAndDown_selectsLines
#[test]
fn whenTouch_withLongPressThenDragLeftOutOfBoundsUpAndDown_selectsLines() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(9);
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((6, 11));
            it.handles_shown = true;
            it.haptics += 1;
        });
        let to = it.character(6) + Offset::new(-2.0, 0.0);
        it.touch_drag_to(to);
        it.check();
        let to = it.character(0) + Offset::new(-2.0, 0.0);
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some((11, 0));
            it.haptics += 1;
        });
        let to = it.top_start();
        let to = it.nudge(to, Some(false), Some(false));
        it.touch_drag_to(to);
        it.check();
        let to = it.bottom_start();
        let to = it.nudge(to, Some(false), Some(true));
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some((6, 29));
            it.haptics += 1;
        });
        it.up();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPress_selectsSingleWord
#[test]
fn whenTouch_withLongPress_selectsSingleWord() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.long_click(at);
        it.expect(|it| {
            it.selection = Some((12, 17));
            it.handles_shown = true;
            it.haptics += 1;
        });
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressesOnMultipleWords_selectsSingleWords
#[test]
fn whenTouch_withLongPressesOnMultipleWords_selectsSingleWords() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        it.handles_shown = true;
        let words = [(2, (0, 5)), (8, (6, 11)), (13, (12, 17)), (20, (18, 23)), (26, (24, 29))];
        for (offset, selection) in words {
            let at = it.character(offset);
            it.long_click(at);
            it.expect(|it| {
                it.selection = Some(selection);
                it.haptics += 1;
            });
        }
    }
}

/// As the tests' `touchLongPressThenDragForwardsAndBackTest`.
fn long_press_then_drag_forwards_and_back(forward: usize, back: usize, expected: (usize, usize)) {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((12, 17));
            it.handles_shown = true;
            it.haptics += 1;
        });
        let to = it.character(forward);
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some(expected);
            it.haptics += 1;
        });
        let to = it.character(back);
        it.touch_drag_to(to);
        it.check();
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragLeftAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragLeftAndBack_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back(8, 9, (17, 6));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragUpAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragUpAndBack_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back(0, 3, (17, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragRightAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragRightAndBack_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back(22, 19, (12, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragDownAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragDownAndBack_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back(27, 26, (12, 29));
}

/// As the tests' `touchLongPressThenDragToEndPaddingTest`.
fn long_press_then_drag_to_end_padding(to: fn(&mut Gestures) -> Offset, expected: (usize, usize)) {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((12, 17));
            it.handles_shown = true;
            it.haptics += 1;
        });
        let to = to(&mut it);
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some(expected);
            it.haptics += 1;
        });
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragToUpperEndPaddingAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragToUpperEndPaddingAndBack_selectsWordsThenChars() {
    long_press_then_drag_to_end_padding(Gestures::top_end, (17, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragToMiddleEndPaddingAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragToMiddleEndPaddingAndBack_selectsWordsThenChars() {
    long_press_then_drag_to_end_padding(Gestures::center_end, (12, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressThenDragToLowerEndPaddingAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragToLowerEndPaddingAndBack_selectsWordsThenChars() {
    long_press_then_drag_to_end_padding(Gestures::bottom_end, (12, 29));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInEndPadding_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPadding_entersSelectionMode() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.top_end();
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((0, 5));
            it.handles_shown = true;
            it.haptics += 1;
        });
        let by = it.nudge(Offset::ZERO, Some(false), None);
        it.touch_drag_by(by);
        it.check();
        it.up();
        it.check();
    }
}

/// Long presses at the middle line's end, then drags to `to` and back, twice, as the two
/// `whenTouch_withLongPressInEndPaddingThenDragTo*EndPadding_*` tests.
fn long_press_in_end_padding_then_drag(to: fn(&mut Gestures) -> Offset, expected: (usize, usize)) {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.center_end();
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((18, 23));
            it.handles_shown = true;
            it.haptics += 1;
        });
        for round in 0..2 {
            if round == 1 {
                let at = it.center_end();
                it.touch_drag_to(at);
                it.expect(|it| {
                    it.selection = Some((18, 23));
                    it.haptics += 1;
                });
            }
            let at = to(&mut it);
            it.touch_drag_to(at);
            it.expect(|it| {
                it.selection = Some(expected);
                it.haptics += 1;
            });
        }
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingThenDragToUpperEndPadding_selectsParagraphAndNewLine
#[test]
fn whenTouch_withLongPressInEndPaddingThenDragToUpperEndPadding_selectsParagraphAndNewLine() {
    long_press_in_end_padding_then_drag(Gestures::top_end, (23, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingThenDragToLowerEndPadding_selectsNewLineAndParagraph
#[test]
fn whenTouch_withLongPressInEndPaddingThenDragToLowerEndPadding_selectsNewLineAndParagraph() {
    long_press_in_end_padding_then_drag(Gestures::bottom_end, (18, 29));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInEndPaddingOfFinalLine_entersSelectionMode
#[test]
fn whenTouch_withLongPressInEndPaddingOfFinalLine_entersSelectionMode() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.bottom_end();
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((24, 29));
            it.handles_shown = true;
            it.haptics += 1;
        });
        it.touch_drag_by(Offset::new(-1.0, 0.0));
        it.check();
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInFinalLineEndPaddingThenDragToMidEndPadding_entersSelectionMode
#[test]
fn whenTouch_withLongPressInFinalLineEndPaddingThenDragToMidEndPadding_entersSelectionMode() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.bottom_end();
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((24, 29));
            it.handles_shown = true;
            it.haptics += 1;
        });
        for round in 0..2 {
            if round == 1 {
                let at = it.bottom_end();
                it.touch_drag_to(at);
                it.expect(|it| {
                    it.selection = Some((24, 29));
                    it.haptics += 1;
                });
            }
            let at = it.center_end();
            it.touch_drag_to(at);
            it.expect(|it| {
                it.selection = Some((29, 18));
                it.haptics += 1;
            });
        }
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withLongPressInEndPadding_selectsFinalWord
#[test]
fn whenTouch_withLongPressInEndPadding_selectsFinalWord() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.center_end();
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((18, 23));
            it.handles_shown = true;
            it.haptics += 1;
        });
        it.up();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withLongClick_collapsedSelectionAtClick, whenTrackpad_withLongClick_collapsedSelectionAtClick
#[test]
fn whenMouse_withLongClick_collapsedSelectionAtClick() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| cursor.long_click(at));
        it.expect(|it| it.selection = Some((13, 13)));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withClick_collapsedSelectionAtClick, whenTrackpad_withClick_collapsedSelectionAtClick
#[test]
fn whenMouse_withClick_collapsedSelectionAtClick() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| cursor.click(at));
        it.expect(|it| it.selection = Some((13, 13)));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withSingleClick_collapsedSelection, whenTrackpad_withSingleClick_collapsedSelection
#[test]
fn whenMouse_withSingleClick_collapsedSelection() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| {
            cursor.move_to(at);
            cursor.press();
        });
        it.expect(|it| it.selection = Some((13, 13)));
        it.cursor(kind, |cursor| cursor.release());
        it.check();
    }
}

/// Both directions with each cursor.
fn directions_and_cursors() -> impl Iterator<Item = (bool, Cursor)> {
    [false, true].into_iter().flat_map(|rtl| CURSORS.map(|kind| (rtl, kind)))
}

/// As the tests' `mouseClicksThenDragTest` and `trackpadClicksThenDragTest`.
fn clicks_then_drag(clicks: u32, end: usize, first: (usize, usize), last: (usize, usize)) {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
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
        it.expect(|it| it.selection = Some(first));
        let to = it.character(end);
        it.cursor_drag_to(kind, to);
        it.expect(|it| it.selection = Some(last));
        it.cursor(kind, |cursor| cursor.release());
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragLeft_selectsCharacters, whenTrackpad_withSingleClickThenDragLeft_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragLeft_selectsCharacters() {
    clicks_then_drag(1, 8, (13, 13), (13, 8));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragUp_selectsCharacters, whenTrackpad_withSingleClickThenDragUp_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragUp_selectsCharacters() {
    clicks_then_drag(1, 2, (13, 13), (13, 2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragRight_selectsCharacters, whenTrackpad_withSingleClickThenDragRight_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragRight_selectsCharacters() {
    clicks_then_drag(1, 20, (13, 13), (13, 20));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withSingleClickThenDragDown_selectsCharacters, whenTrackpad_withSingleClickThenDragDown_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragDown_selectsCharacters() {
    clicks_then_drag(1, 26, (13, 13), (13, 26));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withDoubleClick_selectsWord, whenTrackpad_withDoubleClick_selectsWord
#[test]
fn whenMouse_withDoubleClick_selectsWord() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| {
            cursor.click(at);
            cursor.click(at);
        });
        it.expect(|it| it.selection = Some((12, 17)));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragLeft_selectsWords, whenTrackpad_withDoubleClickThenDragLeft_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragLeft_selectsWords() {
    clicks_then_drag(2, 8, (12, 17), (17, 6));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragUp_selectsWords, whenTrackpad_withDoubleClickThenDragUp_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragUp_selectsWords() {
    clicks_then_drag(2, 2, (12, 17), (17, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragRight_selectsWords, whenTrackpad_withDoubleClickThenDragRight_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragRight_selectsWords() {
    clicks_then_drag(2, 20, (12, 17), (12, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withDoubleClickThenDragDown_selectsWords, whenTrackpad_withDoubleClickThenDragDown_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragDown_selectsWords() {
    clicks_then_drag(2, 26, (12, 17), (12, 29));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_thenTouch_touchBehaviorsAppear, whenTrackpad_thenTouch_touchBehaviorsAppear
#[test]
fn whenMouse_thenTouch_touchBehaviorsAppear() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| {
            cursor.click(at);
            cursor.click(at);
        });
        it.expect(|it| it.selection = Some((12, 17)));
        it.enter_touch_mode();
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_thenMouse_touchBehaviorsDisappear, whenTouch_thenTrackpad_touchBehaviorsDisappear
#[test]
fn whenTouch_thenMouse_touchBehaviorsDisappear() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.long_click(at);
        it.expect(|it| {
            it.selection = Some((12, 17));
            it.handles_shown = true;
            it.haptics += 1;
        });
        it.enter_cursor_mode(kind);
        it.expect(|it| it.handles_shown = false);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouseCollapsedSelection_thenTouch_noUiElements, whenTrackpadCollapsedSelection_thenTouch_noUiElements
#[test]
fn whenMouseCollapsedSelection_thenTouch_noUiElements() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| cursor.click(at));
        it.expect(|it| it.selection = Some((13, 13)));
        it.enter_touch_mode();
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouchCollapsedSelection_thenMouse_noUiElements, whenTouchCollapsedSelection_thenTrackpad_noUiElements
#[test]
fn whenTouchCollapsedSelection_thenMouse_noUiElements() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.touch(|touch| touch.click_at(at));
        it.check();
        it.enter_cursor_mode(kind);
        it.check();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouseCollapsedSelectionAcrossLines_thenTouch_showUi, whenTrackpadCollapsedSelectionAcrossLines_thenTouch_showUi
#[test]
fn whenMouseCollapsedSelectionAcrossLines_thenTouch_showUi() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.center_end();
        it.cursor(kind, |cursor| {
            cursor.move_to(at);
            cursor.press();
        });
        it.expect(|it| it.selection = Some((23, 23)));
        let to = it.character(24);
        it.cursor_drag_to(kind, to);
        it.expect(|it| it.selection = Some((23, 24)));
        it.enter_touch_mode();
        it.expect(|it| it.handles_shown = true);
    }
}

/// As the tests' `mouseClickThenDragUpToPaddingTest` and its trackpad twin.
fn clicks_then_drag_up_to_end_padding(clicks: u32, last: (usize, usize)) {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
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
        let to = it.top_end();
        it.cursor_drag_to(kind, to);
        it.expect(|it| it.selection = Some(last));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_thenSingleClickAndDragUpToEndPadding_selectsCharacters, whenTrackpad_thenSingleClickAndDragUpToEndPadding_selectsCharacters
#[test]
fn whenMouse_thenSingleClickAndDragUpToEndPadding_selectsCharacters() {
    clicks_then_drag_up_to_end_padding(1, (13, 5));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_thenDoubleClickAndDragUpToEndPadding_selectsWords, whenTrackpad_thenDoubleClickAndDragUpToEndPadding_selectsWords
#[test]
fn whenMouse_thenDoubleClickAndDragUpToEndPadding_selectsWords() {
    clicks_then_drag_up_to_end_padding(2, (17, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withDoubleClick_selectsWord
#[test]
fn whenTouch_withDoubleClick_selectsWord() {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.touch(|touch| {
            touch.click_at(at);
            touch.click_at(at);
        });
        it.expect(|it| {
            it.selection = Some((12, 17));
            it.handles_shown = true;
        });
    }
}

/// As the tests' `touchTapsThenDragTest` of two taps: the second held past the long-press
/// timeout, then dragged.
fn double_tap_then_drag(end: usize, last: (usize, usize)) {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.touch(|touch| {
            touch.down(at);
            touch.advance_event_time(touch.event_period_millis());
            touch.up();
            touch.advance_event_time(touch.event_period_millis());
            touch.down(at);
        });
        it.ui.advance_time_by(std::time::Duration::from_millis(1000));
        it.expect(|it| {
            it.selection = Some((12, 17));
            it.handles_shown = true;
        });
        let to = it.character(end);
        it.touch_drag_to(to);
        it.expect(|it| it.selection = Some(last));
        it.up();
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragLeft_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragLeft_selectsWords() {
    double_tap_then_drag(8, (17, 6));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragUp_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragUp_selectsWords() {
    double_tap_then_drag(2, (17, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragRight_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragRight_selectsWords() {
    double_tap_then_drag(20, (12, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withDoubleClickThenDragDown_selectsWords
#[test]
fn whenTouch_withDoubleClickThenDragDown_selectsWords() {
    double_tap_then_drag(26, (12, 29));
}

// Not one of Compose's: a second tap dragged at once, before the long-press timeout, drags its
// word once past the touch slop (Compose's `touchSelectionSubsequentPress`, `DownResolution.Drag`).
#[test]
fn a_second_tap_dragged_at_once_selects_by_words() {
    let mut it = gestures(false);
    let at = it.character(13);
    it.touch(|touch| {
        touch.click_at(at);
        touch.down(at);
    });
    it.expect(|it| {
        it.selection = Some((12, 17));
        it.handles_shown = true;
    });
    let to = it.character(20);
    it.touch_drag_to(to);
    it.expect(|it| it.selection = Some((12, 23)));
    it.up();
    it.check();
}

/// A handle pressed, as Compose's `withHandlePressed`: down at its center; the gesture's place is
/// where the handle points, in the container's coordinates (`HandlePressedScope`).
fn press_handle(it: &mut Gestures, handle: Handle) -> Offset {
    it.ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        let center = touch.center();
        touch.down(center);
    });
    it.ui.wait_for_idle();
    handle_info(&mut it.ui, handle).position
}

/// The pressed handle's finger moved so the gesture goes to `to`, as `moveHandleTo`: the
/// injection keeps the finger's place in the root, so the popup's own move needs no correction.
fn move_handle_to(it: &mut Gestures, handle: Handle, gesture: &mut Offset, to: Offset) {
    let delta = to - *gesture;
    it.ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| {
        touch.move_by(delta);
    });
    it.ui.wait_for_idle();
    *gesture = to;
}

/// As `moveHandleToCharacter`: the end handle to the bottom left of the character nudged to its
/// start, the start handle nudged to its end.
fn move_handle_to_character(
    it: &mut Gestures,
    handle: Handle,
    gesture: &mut Offset,
    offset: usize,
) {
    let corner = it.character_box(offset).bottom_left();
    let to = it.nudge(corner, Some(handle == Handle::SelectionStart), None);
    move_handle_to(it, handle, gesture, to);
}

fn release_handle(it: &mut Gestures, handle: Handle) {
    it.ui.on_node(is_selection_handle(Some(handle))).perform_touch_input(|touch| touch.up());
    it.ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionHandlesGesturesTest.kt, whenTouchHandle_verifyOneCharStaysSelected_withinLine
#[test]
fn whenTouchHandle_verifyOneCharStaysSelected_withinLine() {
    let mut it = gestures(false);
    let at = it.character_center_left(14);
    it.long_click(at);
    it.expect(|it| {
        it.selection = Some((12, 17));
        it.handles_shown = true;
        it.haptics += 1;
    });
    let handle = Handle::SelectionEnd;
    let mut gesture = press_handle(&mut it, handle);
    move_handle_to_character(&mut it, handle, &mut gesture, 13);
    it.expect(|it| {
        it.selection = Some((12, 13));
        it.haptics += 1;
    });
    move_handle_to_character(&mut it, handle, &mut gesture, 12);
    // Never collapsed: the character before stays selected.
    it.check();
    move_handle_to_character(&mut it, handle, &mut gesture, 11);
    it.expect(|it| {
        it.selection = Some((12, 11));
        it.haptics += 1;
    });
    release_handle(&mut it, handle);
    it.check();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionHandlesGesturesTest.kt, whenTouchHandle_thenDragLeftOutOfBounds_keepsFirstCharSelected
#[test]
fn whenTouchHandle_thenDragLeftOutOfBounds_keepsFirstCharSelected() {
    let mut it = gestures(false);
    let at = it.character_center_left(9);
    it.long_click(at);
    it.expect(|it| {
        it.selection = Some((6, 11));
        it.handles_shown = true;
        it.haptics += 1;
    });
    let handle = Handle::SelectionEnd;
    let mut gesture = press_handle(&mut it, handle);
    for offset in [8, 7, 6] {
        move_handle_to_character(&mut it, handle, &mut gesture, offset);
    }
    it.expect(|it| {
        it.selection = Some((6, 7));
        it.haptics += 1;
    });
    // Just outside the left edge: no change, not onto the line above.
    let y = handle_info(&mut it.ui, handle).position.y;
    move_handle_to(&mut it, handle, &mut gesture, Offset::new(-1.0, y));
    it.check();
    release_handle(&mut it, handle);
    it.check();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionHandlesGesturesTest.kt, whenTouchHandle_withWordSpanningMultipleLines_selectionCanShrinkWithinLine
#[test]
fn whenTouchHandle_withWordSpanningMultipleLines_selectionCanShrinkWithinLine() {
    let mut it = gestures(false);
    let content = "hello".repeat(100);
    it.set_content(&content);
    let at = it.character_center_left(content.len() - 1);
    it.long_click(at);
    it.expect(|it| {
        it.selection = Some((0, content.len()));
        it.handles_shown = true;
        it.haptics += 1;
    });
    let handle = Handle::SelectionEnd;
    let mut gesture = press_handle(&mut it, handle);
    move_handle_to_character(&mut it, handle, &mut gesture, 10);
    move_handle_to_character(&mut it, handle, &mut gesture, 5);
    it.expect(|it| {
        it.selection = Some((0, 5));
        it.haptics += 1;
    });
    release_handle(&mut it, handle);
    it.check();
}

// Not one of Compose's: a second tap held past the long-press timeout drags without passing the
// touch slop, as Compose's `DownResolution.Timeout`; before it, a move within the slop does
// nothing.
#[test]
fn a_second_tap_held_past_the_timeout_drags_within_the_slop() {
    let mut it = gestures(false);
    it.ui.host().set_view_configuration(Some(compose_ui::ViewConfiguration {
        touch_slop: 200.0,
        minimum_touch_target_size: compose_ui::DpSize::ZERO,
        ..compose_ui::ViewConfiguration::default()
    }));
    let (at, to) = (it.character(13), it.character(20));
    it.touch(|touch| {
        touch.click_at(at);
        touch.down(at);
        touch.move_to((at + to) * 0.5);
    });
    it.expect(|it| {
        it.selection = Some((12, 17));
        it.handles_shown = true;
    });
    it.ui.advance_time_by(std::time::Duration::from_millis(1000));
    it.touch(|touch| touch.move_to(to));
    it.expect(|it| it.selection = Some((12, 23)));
    it.up();
    it.check();
}

// Not one of Compose's (its container tests grow by words only into other texts): a handle
// dragged on its line from a word's boundary into the next word grows by the word, then shrinks
// by characters, as `CharacterWithWordAccelerate`.
#[test]
fn a_handle_grows_by_words_from_a_boundary_and_shrinks_by_characters() {
    let mut it = gestures(false);
    let at = it.character_center_left(14);
    it.long_click(at);
    it.expect(|it| {
        it.selection = Some((12, 17));
        it.handles_shown = true;
        it.haptics += 1;
    });
    let handle = Handle::SelectionEnd;
    let mut gesture = press_handle(&mut it, handle);
    move_handle_to_character(&mut it, handle, &mut gesture, 20);
    it.expect(|it| {
        it.selection = Some((12, 23));
        it.haptics += 1;
    });
    move_handle_to_character(&mut it, handle, &mut gesture, 19);
    it.expect(|it| {
        it.selection = Some((12, 19));
        it.haptics += 1;
    });
    release_handle(&mut it, handle);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withTripleClick_selectsParagraph, whenTrackpad_withTripleClick_selectsParagraph, whenTouch_withTripleClick_selectsParagraph
#[test]
fn whenMouse_withTripleClick_selectsParagraph() {
    // Compose's `whenTouch_*` twin clicks with the mouse too.
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.cursor(kind, |cursor| {
            for _ in 0..3 {
                cursor.click(at);
            }
        });
        it.expect(|it| it.selection = Some((6, 23)));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragLeft_selectsParagraphs, whenTrackpad_withTripleClickThenDragLeft_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragLeft_selectsParagraphs() {
    clicks_then_drag(3, 8, (6, 23), (6, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragUp_selectsParagraphs, whenTrackpad_withTripleClickThenDragUp_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragUp_selectsParagraphs() {
    clicks_then_drag(3, 2, (6, 23), (23, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragRight_selectsParagraphs, whenTrackpad_withTripleClickThenDragRight_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragRight_selectsParagraphs() {
    clicks_then_drag(3, 20, (6, 23), (6, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_withTripleClickThenDragDown_selectsParagraphs, whenTrackpad_withTripleClickThenDragDown_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragDown_selectsParagraphs() {
    clicks_then_drag(3, 26, (6, 23), (6, 29));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_thenTripleClickAndDragUpToEndPadding_selectsParagraph, whenTrackpad_thenTripleClickAndDragUpToEndPadding_selectsParagraph
#[test]
fn whenMouse_thenTripleClickAndDragUpToEndPadding_selectsParagraph() {
    clicks_then_drag_up_to_end_padding(3, (23, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenMouse_thenTripleClickInEndPadding_selectsOnlyCurrentParagraph, whenTrackpad_thenTripleClickInEndPadding_selectsOnlyCurrentParagraph
#[test]
fn whenMouse_thenTripleClickInEndPadding_selectsOnlyCurrentParagraph() {
    for (rtl, kind) in directions_and_cursors() {
        let mut it = gestures(rtl);
        let at = it.center_end();
        it.cursor(kind, |cursor| {
            cursor.move_to(at);
            cursor.press();
            for _ in 0..2 {
                cursor.advance_event_time(16);
                cursor.release();
                cursor.advance_event_time(16);
                cursor.press();
            }
            cursor.release();
        });
        it.expect(|it| it.selection = Some((6, 23)));
    }
}

/// As the tests' `touchTapsThenDragTest` of three taps: the third held past the long-press
/// timeout, then dragged.
fn triple_tap_then_drag(end: usize, last: (usize, usize)) {
    for rtl in [false, true] {
        let mut it = gestures(rtl);
        let at = it.character(13);
        it.touch(|touch| {
            touch.down(at);
            for _ in 0..2 {
                touch.advance_event_time(touch.event_period_millis());
                touch.up();
                touch.advance_event_time(touch.event_period_millis());
                touch.down(at);
            }
        });
        it.ui.advance_time_by(std::time::Duration::from_millis(1000));
        it.expect(|it| {
            it.selection = Some((6, 23));
            it.handles_shown = true;
        });
        let to = it.character(end);
        it.touch_drag_to(to);
        it.expect(|it| it.selection = Some(last));
        it.up();
        it.expect(|it| it.handles_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withTripleClickThenDragLeft_selectsParagraphs
#[test]
fn whenTouch_withTripleClickThenDragLeft_selectsParagraphs() {
    triple_tap_then_drag(8, (6, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withTripleClickThenDragUp_selectsParagraphs
#[test]
fn whenTouch_withTripleClickThenDragUp_selectsParagraphs() {
    triple_tap_then_drag(2, (23, 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withTripleClickThenDragRight_selectsParagraphs
#[test]
fn whenTouch_withTripleClickThenDragRight_selectsParagraphs() {
    triple_tap_then_drag(20, (6, 23));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesTest.kt, whenTouch_withTripleClickThenDragDown_selectsParagraphs
#[test]
fn whenTouch_withTripleClickThenDragDown_selectsParagraphs() {
    triple_tap_then_drag(26, (6, 29));
}
