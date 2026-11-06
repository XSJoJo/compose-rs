//! A state-based field's focus, from Compose's `TextFieldFocusTest`: requesting it, the focus
//! interactions, the field's bounds in a focus search, the input session started by an effect's
//! request, the D-pad's keys and Tab moving it on.
//!
//! Compose's D-pad tests press a physical device's keys (a D-pad's, a keyboard's, or both); a
//! compose-rs key has no source, so each direction is one test here. The `focusBounds_*` tests
//! wait on the field's focus rect: compose-ui has `FocusProperties::focus_rect` and
//! `FocusRequester::focused_rect`, but the field doesn't set its cursor's or selection's yet, and
//! unfocused, a search from above its end finds the box below it, where Compose's finds it.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Alignment, Color, Dialog, FocusDirection, FocusManager, FocusRequester, FocusState, ImeAction,
    Key, KeyEvent, KeyModifiers, LOCAL_FOCUS_MANAGER, LOCAL_SOFTWARE_KEYBOARD_CONTROLLER,
    MutableInteractionSource, TextRange, UnitExt,
};

use crate::interaction_helpers::{ends, kinds, record};
use crate::text_field_basic::type_on_keys;

fn state_of(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

fn cell<T: PartialEq + 'static>(value: T) -> MutableState<T> {
    Box::leak(Box::new(mutable_state_of(value))).handle()
}

type Kept<T> = Rc<RefCell<Option<T>>>;

fn kept<T: Clone>(kept: &Kept<T>) -> T {
    kept.borrow().clone().expect("composed")
}

/// An input session is open, as Compose's `assertSessionActive`.
fn session(ui: &mut ComposeTestRule) -> bool {
    ui.host().text_input_options().is_some()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, requestFocus
#[test]
fn a_request_focuses_its_field_alone() {
    let requesters: Kept<Vec<FocusRequester>> = Rc::default();
    let focused: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(vec![false; 3]));
    let (into, told) = (requesters.clone(), focused.clone());
    let mut ui = compose_test(move || {
        let mut all = Vec::new();
        for index in 0..3 {
            let requester = remember(FocusRequester::new);
            all.push(requester);
            let state = remember(|| TextFieldState::new(""));
            let told = told.clone();
            BasicTextFieldOfState!(
                state,
                modifier = Modifier
                    .focus_requester(requester)
                    .on_focus_changed(move |focus: FocusState| {
                        told.borrow_mut()[index] = focus.is_focused();
                    })
                    .required_width(10.dp()),
            );
        }
        *into.borrow_mut() = Some(all);
    });
    for chosen in 0..3 {
        kept(&requesters)[chosen].request_focus();
        ui.wait_for_idle();
        let expected: Vec<bool> = (0..3).map(|index| index == chosen).collect();
        assert_eq!(*focused.borrow(), expected);
    }
}

/// A field with `source`, tagged, and a focusable box after it; the focus manager kept.
fn field_and_box(
    source: MutableState<MutableInteractionSource>,
    enabled: MutableState<bool>,
) -> (ComposeTestRule, Kept<FocusManager>) {
    let manager: Kept<FocusManager> = Rc::default();
    let into = manager.clone();
    let ui = compose_test(move || {
        *into.borrow_mut() = Some(LOCAL_FOCUS_MANAGER.current());
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("field"),
            enabled = enabled.get(),
            interaction_source = Some(source.get()),
        );
        Box!(Modifier.size(10.dp()).focusable());
    });
    (ui, manager)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, interactionSource_emitsFocusEvents
#[test]
fn the_interaction_source_hears_the_focus_come_and_go() {
    let source = MutableInteractionSource::new();
    let (mut ui, manager) = field_and_box(cell(source.clone()), cell(true));
    let recorded = record(&source);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!(kinds(&recorded), ["Focus"]);
    kept(&manager).move_focus(FocusDirection::Next);
    ui.wait_for_idle();
    assert_eq!(kinds(&recorded), ["Focus", "Unfocus"]);
    assert!(ends(&recorded, 0, 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, interactionSource_emitsUnfocusEvent_whenDisabled
#[test]
fn the_interaction_source_hears_the_focus_go_when_disabled() {
    let source = MutableInteractionSource::new();
    let enabled = cell(true);
    let (mut ui, _) = field_and_box(cell(source.clone()), enabled);
    let recorded = record(&source);
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!(kinds(&recorded), ["Focus"]);
    enabled.set(false);
    ui.wait_for_idle();
    assert_eq!(kinds(&recorded), ["Focus", "Unfocus"]);
    assert!(ends(&recorded, 0, 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, interactionSource_focusEventsAreSentTo_newInteractionSource
#[test]
fn a_new_interaction_source_hears_the_focus_from_then_on() {
    let (first, second) = (MutableInteractionSource::new(), MutableInteractionSource::new());
    let source = cell(first.clone());
    let (mut ui, manager) = field_and_box(source, cell(true));
    let (heard_first, heard_second) = (record(&first), record(&second));
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!((kinds(&heard_first), kinds(&heard_second)), (vec!["Focus"], vec![]));
    source.set(second.clone());
    ui.wait_for_idle();
    kept(&manager).clear_focus(false);
    ui.wait_for_idle();
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!(kinds(&heard_first), ["Focus", "Unfocus"]);
    assert!(ends(&heard_first, 0, 1));
    assert_eq!(kinds(&heard_second), ["Focus"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, interactionSourceChanges_whenFieldDisabled_eventsAreSentToNewInteractionSource
#[test]
fn a_source_given_while_disabled_hears_the_focus_once_enabled() {
    let (first, second) = (MutableInteractionSource::new(), MutableInteractionSource::new());
    let (source, enabled) = (cell(first.clone()), cell(false));
    let (mut ui, _) = field_and_box(source, enabled);
    let (heard_first, heard_second) = (record(&first), record(&second));
    source.set(second.clone());
    ui.wait_for_idle();
    enabled.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    assert_eq!(kinds(&heard_first), Vec::<&str>::new());
    assert_eq!(kinds(&heard_second), ["Focus"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, noCrashWhenSwitchingBetweenEnabledFocusedAndDisabledTextField
#[test]
fn disabled_it_loses_the_focus_and_enabled_again_doesnt_take_it() {
    let enabled = cell(true);
    let focused = Rc::new(Cell::new(false));
    let told = focused.clone();
    let mut ui = compose_test(move || {
        let state = remember(|| TextFieldState::new(""));
        let told = told.clone();
        BasicTextFieldOfState!(
            state,
            enabled = enabled.get(),
            modifier = Modifier
                .test_tag("field")
                .on_focus_changed(move |focus: FocusState| told.set(focus.is_focused()))
                .required_width(10.dp()),
        );
    });
    ui.on_node_with_tag("field").perform_click();
    ui.wait_for_idle();
    assert!(focused.get());
    enabled.set(false);
    ui.wait_for_idle();
    assert!(!focused.get());
    enabled.set(true);
    ui.wait_for_idle();
    assert!(!focused.get());
}

/// A field focused by an effect at its first composition starts its input session, in the
/// window or in a dialog.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, textInputStarted_forFieldInActivity_whenFocusRequestedImmediately_fromLaunchedEffect
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, textInputStarted_forFieldInActivity_whenFocusRequestedImmediately_fromDisposableEffect
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, textInputStarted_forFieldInDialog_whenFocusRequestedImmediately_fromLaunchedEffect
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, textInputStarted_forFieldInDialog_whenFocusRequestedImmediately_fromDisposableEffect
#[test]
fn a_field_focused_by_an_effect_at_once_starts_its_session() {
    #[composable]
    fn Focused(launched: bool) {
        let requester = remember(FocusRequester::new);
        let state = remember(|| TextFieldState::new(""));
        if launched {
            LaunchedEffect((), async move {
                requester.request_focus();
            });
        } else {
            DisposableEffect((), move |scope| {
                requester.request_focus();
                scope.on_dispose(|| {})
            });
        }
        BasicTextFieldOfState!(state, modifier = Modifier.focus_requester(requester));
    }
    for (launched, in_dialog) in [(true, false), (false, false), (true, true), (false, true)] {
        let mut ui = compose_test(move || {
            if in_dialog {
                Dialog!(on_dismiss_request = || {}, {
                    Focused(launched);
                });
            } else {
                Focused(launched);
            }
        });
        ui.wait_for_idle();
        assert!(session(&mut ui), "launched {launched}, in a dialog {in_dialog}");
    }
}

/// Compose's D-pad tests' content: focusable texts above, left of, right of and below a
/// field (its action `Next`) that asks for the focus.
fn around(single_line: bool) -> ComposeTestRule {
    #[composable]
    fn Element(id: &'static str) {
        BasicText!(
            format!("test-button-{id}"),
            modifier = Modifier
                .test_tag(format!("test-button-{id}"))
                .padding(10.dp())
                .focusable()
                .border(2.dp(), Color::CYAN),
        );
    }
    let mut ui = compose_test(move || {
        Column!(horizontal_alignment = Alignment::CENTER_HORIZONTALLY, {
            Element("top");
            Row!({
                Element("left");
                let state = remember_text_field_state("", None);
                let requester = remember(FocusRequester::new);
                let line_limits = if single_line {
                    TextFieldLineLimits::SingleLine
                } else {
                    TextFieldLineLimits::DEFAULT
                };
                BasicTextFieldOfState!(
                    state,
                    line_limits = line_limits,
                    keyboard_options =
                        KeyboardOptions { ime_action: ImeAction::Next, ..KeyboardOptions::DEFAULT },
                    modifier = Modifier
                        .focus_requester(requester)
                        .test_tag("test-text-field-1")
                        .padding(10.dp()),
                );
                LaunchedEffect((), async move {
                    requester.request_focus();
                });
                Element("right");
            });
            Element("bottom");
        });
    });
    // Android's D-pad and arrow keys move the focus when nothing takes them (D230).
    ui.host().set_keys_navigate(true);
    ui.wait_for_idle();
    ui.on_node_with_tag("test-text-field-1").assert_is_focused();
    ui
}

fn press(ui: &mut ComposeTestRule, key: Key, modifiers: KeyModifiers) {
    ui.host().send_key(KeyEvent { modifiers, ..KeyEvent::down(key) });
    ui.host().send_key(KeyEvent { modifiers, ..KeyEvent::up(key) });
    ui.wait_for_idle();
}

fn selection_of(ui: &mut ComposeTestRule) -> Option<TextRange> {
    let node = ui.on_node_with_tag("test-text-field-1").fetch_semantics_snapshot();
    node.config.get(&compose_ui::SemanticsProperties::TEXT_SELECTION_RANGE).copied()
}

fn select(ui: &mut ComposeTestRule, at: usize) {
    ui.on_node_with_tag("test-text-field-1").perform_semantics_action_with(
        &compose_ui::SemanticsActions::SET_SELECTION,
        |set| {
            set(at as i32, at as i32, false);
        },
    );
    ui.wait_for_idle();
}

/// The on-screen keyboard hidden, as the virtual keyboard's Back does.
fn hide_keyboard(
    ui: &mut ComposeTestRule,
    controller: &Kept<compose_ui::SoftwareKeyboardController>,
) {
    kept(controller).hide();
    ui.wait_for_idle();
    ui.host().take_software_keyboard_request();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkKeyboardShown_onDPadCenter_DPadDevice
#[test]
fn the_dpads_center_shows_the_keyboard() {
    let controller: Kept<compose_ui::SoftwareKeyboardController> = Rc::default();
    let into = controller.clone();
    let state = state_of("");
    let mut ui = compose_test(move || {
        *into.borrow_mut() = LOCAL_SOFTWARE_KEYBOARD_CONTROLLER.current();
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
    });
    ui.on_node_with_tag("field").request_focus();
    type_on_keys(&mut ui, "abc");
    hide_keyboard(&mut ui, &controller);
    assert_eq!(ui.host().take_software_keyboard_request(), None);
    press(&mut ui, Key::DirectionCenter, KeyModifiers::default());
    assert_eq!(ui.host().take_software_keyboard_request(), Some(true));
}

/// Pressing `key` moves the cursor until it can't (`first` the selection after the first
/// press), then the focus to `to`.
fn walks_out(text: &str, start: Option<usize>, key: Key, first: usize, to: &str) {
    let mut ui = around(false);
    type_on_keys(&mut ui, text);
    if let Some(start) = start {
        select(&mut ui, start);
    }
    press(&mut ui, key, KeyModifiers::default());
    ui.on_node_with_tag("test-text-field-1").assert_is_focused();
    assert_eq!(selection_of(&mut ui), Some(TextRange::at(first)));
    for _ in 0..3 {
        ui.on_node_with_tag(to).assert_is_not_focused();
        press(&mut ui, key, KeyModifiers::default());
    }
    ui.on_node_with_tag(to).assert_is_focused();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadLeft_DPadDevice
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadLeft_hardwareKeyboard
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadLeft_DpadHardwareKeyboard
#[test]
fn left_at_the_start_moves_the_focus_left() {
    walks_out("abc", None, Key::DirectionLeft, 2, "test-button-left");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadRight_DPadDevice
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadRight_hardwareKeyboard
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadRight_DpadHardwareKeyboard
#[test]
fn right_at_the_end_moves_the_focus_right() {
    walks_out("abc", Some(0), Key::DirectionRight, 1, "test-button-right");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadUp_DPadDevice
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadUp_hardwareKeyboard
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadUp_DpadHardwareKeyboard
#[test]
fn up_on_the_first_line_moves_the_focus_up() {
    walks_out("a\nb\nc", None, Key::DirectionUp, 3, "test-button-top");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadDown_DPadDevice
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadDown_hardwareKeyboard
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onDPadDown_DpadHardwareKeyboard
#[test]
fn down_on_the_last_line_moves_the_focus_down() {
    walks_out("a\nb\nc", Some(0), Key::DirectionDown, 2, "test-button-bottom");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onTab
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_withImeActionNext_checkFocusNavigation_onEnter
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldFocusTest.kt, basicTextField_checkFocusNavigation_onShiftTab
#[test]
fn tab_enter_and_shift_tab_move_a_single_lines_focus() {
    let shift = KeyModifiers { shift: true, ..KeyModifiers::default() };
    for (key, modifiers, to) in [
        (Key::Tab, KeyModifiers::default(), "test-button-right"),
        (Key::Enter, KeyModifiers::default(), "test-button-right"),
        (Key::Tab, shift, "test-button-left"),
    ] {
        let mut ui = around(true);
        type_on_keys(&mut ui, "abc");
        press(&mut ui, key, modifiers);
        ui.on_node_with_tag(to).assert_is_focused();
    }
}
