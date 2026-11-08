//! A finger's click placing a state-based field's cursor, from Compose's
//! `TextFieldClickToMoveCursorTest`: an empty field, left-to-right and right-to-left texts in
//! either layout direction, the empty region after the text, the edges of fields scrolled
//! sideways and down, a read-only field, and a field in a dialog.
//!
//! As Compose's tests: the test font at 10 sp, the touch input mode; at density 2, a device's
//! (`clickOnTextEdge_horizontalScrollable` clicks the middle of a letter at density 1). Offsets
//! are UTF-8 bytes: a Hebrew letter is 2.

#![allow(non_snake_case)]

use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    Density, Dialog, FocusRequester, IntSize, LOCAL_LAYOUT_DIRECTION, LOCAL_WINDOW_INFO,
    LayoutDirection, Offset, TextRange, UnitExt,
};

const TAG: &str = "BasicTextField";
const DENSITY: f32 = 2.0;
/// 10 sp in pixels.
const FONT: f32 = 10.0 * DENSITY;
/// Three Hebrew letters.
const RTL_TEXT: &str = "\u{05D0}\u{05D1}\u{05D2}";
/// A Hebrew letter's length in UTF-8.
const HEBREW: usize = 2;

fn state_of(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

fn scroll() -> ScrollState {
    **Box::leak(Box::new(ScrollState::new(0)))
}

fn style() -> TextStyle {
    TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT }
}

/// The test font, in touch mode, as `setTextFieldTestContent`.
fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui = compose_test_with(
        IntSize::new(800, 800),
        Density::new(DENSITY),
        Box::new(measurer),
        content,
    );
    ui.host().set_input_mode(compose_ui::InputMode::Touch);
    ui.wait_for_idle();
    ui
}

/// A 50 by 15 dp field over `state`, in `direction`.
fn field(state: TextFieldState, direction: LayoutDirection) -> ComposeTestRule {
    rule(move || {
        compose_runtime::CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(direction), {
            move || {
                BasicTextFieldOfState!(
                    state,
                    text_style = style(),
                    modifier = Modifier.test_tag(TAG).width(50.dp()).height(15.dp()),
                );
            }
        });
    })
}

/// A finger's click at `at(scope)` on the field.
fn click(ui: &mut ComposeTestRule, at: impl Fn(&TouchInjectionScope<'_>) -> Offset) {
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let at = at(touch);
        touch.click_at(at);
    });
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, emptyTextField_clinkOnCornersAndCenter
#[test]
fn emptyTextField_clinkOnCornersAndCenter() {
    // More about a possible crash.
    let state = state_of("");
    let mut ui = field(state, LayoutDirection::Ltr);
    let points: [fn(&TouchInjectionScope<'_>) -> Offset; 5] = [
        |touch| touch.center(),
        |touch| Offset::new(touch.left() + 1.0, touch.top() + 1.0),
        |touch| Offset::new(touch.right() - 1.0, touch.top() + 1.0),
        |touch| Offset::new(touch.left() + 1.0, touch.bottom() - 1.0),
        |touch| Offset::new(touch.right() - 1.0, touch.bottom() - 1.0),
    ];
    for (index, at) in points.into_iter().enumerate() {
        click(&mut ui, at);
        assert_eq!(state.selection(), TextRange::at(0), "click {index}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnText_ltr
#[test]
fn clickOnText_ltr() {
    let state = state_of("abc");
    let mut ui = field(state, LayoutDirection::Ltr);
    click(&mut ui, |touch| Offset::new(FONT * 2.0, touch.height() as f32 / 2.0));
    assert_eq!(state.selection(), TextRange::at(2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnText_rtl
#[test]
fn clickOnText_rtl() {
    let state = state_of(RTL_TEXT);
    let mut ui = field(state, LayoutDirection::Rtl);
    click(&mut ui, |touch| Offset::new(touch.right() - FONT * 2.0, touch.height() as f32 / 2.0));
    assert_eq!(state.selection(), TextRange::at(2 * HEBREW));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnText_ltr_in_rtlLayout
#[test]
fn clickOnText_ltr_in_rtlLayout() {
    let state = state_of("abc");
    let mut ui = field(state, LayoutDirection::Rtl);
    click(&mut ui, |touch| Offset::new(touch.right() - FONT * 2.0, touch.height() as f32 / 2.0));
    assert_eq!(state.selection(), TextRange::at(1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnText_rtl_in_ltrLayout
#[test]
fn clickOnText_rtl_in_ltrLayout() {
    let state = state_of(RTL_TEXT);
    let mut ui = field(state, LayoutDirection::Ltr);
    click(&mut ui, |touch| Offset::new(FONT * 2.0, touch.height() as f32 / 2.0));
    assert_eq!(state.selection(), TextRange::at(HEBREW));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnEmptyRegion_ltr
#[test]
fn clickOnEmptyRegion_ltr() {
    let state = state_of("abc");
    let mut ui = field(state, LayoutDirection::Ltr);
    click(&mut ui, |touch| Offset::new(FONT * 4.0, touch.height() as f32 / 2.0));
    assert_eq!(state.selection(), TextRange::at(3));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnEmptyRegion_rtl
#[test]
fn clickOnEmptyRegion_rtl() {
    let state = state_of(RTL_TEXT);
    let mut ui = field(state, LayoutDirection::Rtl);
    click(&mut ui, |touch| Offset::new(FONT, touch.height() as f32 / 2.0));
    assert_eq!(state.selection(), TextRange::at(3 * HEBREW));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnTextEdge_horizontalScrollable
#[test]
fn clickOnTextEdge_horizontalScrollable() {
    let state = state_of("abcabcabcabc");
    let scroll_state = scroll();
    let mut ui = rule(move || {
        BasicTextFieldOfState!(
            state,
            text_style = style(),
            line_limits = TextFieldLineLimits::SingleLine,
            scroll_state = Some(scroll_state),
            modifier = Modifier.test_tag(TAG).width(57.dp()).height(12.dp()),
        );
    });
    click(&mut ui, |touch| Offset::new(touch.right() - 1.0, touch.height() as f32 / 2.0));
    ui.run_on_idle(|_| {
        assert_eq!(state.selection(), TextRange::at(6));
        assert!(scroll_state.value() > 0, "scrolled: {}", scroll_state.value());
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnTextEdge_verticalScrollable
#[test]
fn clickOnTextEdge_verticalScrollable() {
    let state = state_of("abc abc abc abc");
    let scroll_state = scroll();
    let mut ui = rule(move || {
        BasicTextFieldOfState!(
            state,
            text_style = style(),
            line_limits = TextFieldLineLimits::multi_line(1, 2),
            scroll_state = Some(scroll_state),
            modifier = Modifier.test_tag(TAG).width(50.dp()).height(17.dp()),
        );
    });
    click(&mut ui, |touch| Offset::new(FONT, touch.bottom() - 1.0));
    assert_eq!(state.selection(), TextRange::at(5));
    assert!(scroll_state.value() > 0, "scrolled: {}", scroll_state.value());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, clickOnText_readOnly
#[test]
fn clickOnText_readOnly() {
    let state = state_of("abc");
    let mut ui = rule(move || {
        BasicTextFieldOfState!(
            state,
            text_style = style(),
            read_only = true,
            modifier = Modifier.test_tag(TAG).width(50.dp()).height(15.dp()),
        );
    });
    click(&mut ui, |touch| Offset::new(FONT * 2.0, touch.height() as f32 / 2.0));
    assert_eq!(state.selection(), TextRange::at(2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/selection/TextFieldClickToMoveCursorTest.kt, textFieldInsideDialog_tapsChangeCursorPosition
#[test]
fn textFieldInsideDialog_tapsChangeCursorPosition() {
    let state = state_of("abc abc abc abc");
    let mut ui = rule(move || {
        Dialog!(on_dismiss_request = || {}, {
            let focus_requester = compose_runtime::remember(FocusRequester::new);
            let is_window_focused = LOCAL_WINDOW_INFO.current().is_window_focused();
            BasicTextFieldOfState!(
                state,
                text_style = style(),
                modifier = Modifier.fill_max_width().test_tag(TAG).focus_requester(focus_requester),
            );
            compose_runtime::LaunchedEffect(is_window_focused, async move {
                if is_window_focused {
                    focus_requester.request_focus();
                }
            });
        });
    });
    click(&mut ui, |touch| Offset::new(2.0 * FONT, touch.center_y()));
    assert_eq!(state.selection(), TextRange::at(2));
}
