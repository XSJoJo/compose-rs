//! The state-based field's semantics, from Compose's `BasicTextFieldSemanticsTest`: what it tells
//! (its text as shown and as typed, editable, its action key, its selection), and its actions
//! (set text, text input, click), through its transformations.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    ImeAction, SemanticsActions as A, SemanticsProperties as P, TextLayoutResult, TextRange,
};

const TAG: &str = "field";

fn state_of(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
}

/// A field over `state` with these transformations, tagged.
fn field(
    state: TextFieldState,
    input: Option<InputTransformation>,
    output: Option<OutputTransformation>,
    read_only: bool,
) -> ComposeTestRule {
    compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            read_only = read_only,
            input_transformation = input.clone(),
            output_transformation = output.clone(),
        );
    })
}

fn text_of(
    ui: &mut ComposeTestRule,
    key: &'static compose_ui::SemanticsPropertyKey<compose_ui::SharedString>,
) -> Option<String> {
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
    node.config.get(key).map(|text| text.to_string())
}

/// Its text as shown (`EditableText`) and as typed (`InputText`).
fn told(ui: &mut ComposeTestRule) -> (Option<String>, Option<String>) {
    (text_of(ui, &P::EDITABLE_TEXT), text_of(ui, &P::INPUT_TEXT))
}

/// The set-text action on the focused node, as `performTextReplacement`: whether it took it.
fn replace_text(ui: &mut ComposeTestRule, text: &str) -> bool {
    ui.on_node_with_tag(TAG).request_focus();
    let mut done = false;
    ui.on_node_with_tag(TAG).perform_semantics_action_with(&A::SET_TEXT, |set| {
        done = set(text.into());
    });
    ui.wait_for_idle();
    done
}

fn joined_by_dashes() -> InputTransformation {
    InputTransformation::from_fn(|buffer| {
        if buffer.length() > 1 {
            let text = buffer.as_str().chars().map(String::from).collect::<Vec<_>>().join("-");
            let length = buffer.length();
            buffer.replace(0, length, &text);
        }
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, defaultSemantics
#[test]
fn a_field_tells_what_it_is() {
    let state = state_of("", TextRange::ZERO);
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            decorator = |inner: Callback| {
                Column!({
                    BasicText!("label");
                    inner();
                });
            },
        );
    });
    assert_eq!(told(&mut ui), (Some(String::new()), Some(String::new())));
    ui.on_node_with_tag(TAG)
        .assert_text_equals_with(&["label"], false)
        .assert_has_click_action()
        .assert_is_not_focused();
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
    assert_eq!(node.config.get(&P::IS_EDITABLE).copied(), Some(true));
    assert_eq!(node.config.get(&P::IME_ACTION).copied(), Some(ImeAction::Default));
    assert_eq!(node.config.get(&P::TEXT_SELECTION_RANGE).copied(), Some(TextRange::ZERO));
    assert!(node.config.contains(&A::SET_TEXT));
    assert!(node.config.contains(&A::PASTE_TEXT));
    assert!(node.config.contains(&A::SET_SELECTION));
    assert!(!node.config.contains(&P::PASSWORD));
    let mut results: Vec<TextLayoutResult> = Vec::new();
    ui.on_node_with_tag(TAG).perform_semantics_action_with(&A::GET_TEXT_LAYOUT_RESULT, |get| {
        get(&mut results);
    });
    assert_eq!(results.len(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_enabledStatus
#[test]
fn a_disabled_field_says_so() {
    let enabled = Box::leak(Box::new(mutable_state_of(true))).handle();
    let state = state_of("", TextRange::ZERO);
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG), enabled = enabled.get());
    });
    ui.on_node_with_tag(TAG).assert_is_enabled();
    enabled.set(false);
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_is_not_enabled();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_setTextAction
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_setTextAction_inputTransformation
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_setTextAction_outputTransformation
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_setTextAction_inputAndOutputTransformation
#[test]
fn set_text_goes_through_both_transformations() {
    let caps = || Some(InputTransformation::all_caps(compose_ui::Locale::new("en")));
    let xyz = |suffix: &'static str| Some(OutputTransformation::from_fn(move |b| b.append(suffix)));
    for (input, output, typed, shown) in [
        (None, None, "Hello", "Hello"),
        (caps(), None, "ABC", "ABC"),
        (None, xyz(" xyz"), "abc", "abc xyz"),
        (caps(), xyz(" XYZ"), "ABC", "ABC XYZ"),
    ] {
        let state = state_of("", TextRange::ZERO);
        let mut ui = field(state, input, output, false);
        ui.on_node_with_tag(TAG).assert_is_not_focused();
        let text = if typed == "Hello" { "Hello" } else { "abc" };
        assert!(replace_text(&mut ui, text));
        ui.on_node_with_tag(TAG).assert_is_focused().assert_text_equals(&[shown]);
        assert_eq!(told(&mut ui), (Some(shown.to_string()), Some(typed.to_string())));
        assert_eq!(state.text().as_str(), typed);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_performSetTextAction_whenReadOnly
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_performTextInputAction_whenReadOnly
#[test]
fn a_read_only_field_takes_no_text() {
    let state = state_of("", TextRange::ZERO);
    let mut ui = field(state, None, None, true);
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
    assert!(!node.config.contains(&A::SET_TEXT) || !replace_text(&mut ui, "hello"));
    assert!(!ui.host().send_text("hello"));
    ui.wait_for_idle();
    assert_eq!(state.text().as_str(), "");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_setTextAction_appliesFilter
#[test]
fn set_text_goes_through_the_input_transformation() {
    let state = state_of("", TextRange::ZERO);
    let mut ui = field(state, Some(joined_by_dashes()), None, false);
    assert!(replace_text(&mut ui, "Hello"));
    ui.on_node_with_tag(TAG).assert_is_focused().assert_text_equals(&["H-e-l-l-o"]);
    assert_eq!(state.text().as_str(), "H-e-l-l-o");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_performTextInputAction
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_performTextInputAction_appliesFilter
#[test]
fn text_input_goes_in_at_the_cursor_through_the_input_transformation() {
    let state = state_of("Hello", TextRange::at(1));
    let mut ui = field(state, None, None, false);
    ui.on_node_with_tag(TAG).assert_is_not_focused().perform_text_input("a");
    ui.on_node_with_tag(TAG).assert_is_focused().assert_text_equals(&["Haello"]);
    assert_eq!(state.text().as_str(), "Haello");

    let no_a = InputTransformation::from_fn(|buffer| {
        let text = buffer.as_str().replace('a', "");
        let length = buffer.length();
        buffer.replace(0, length, &text);
    });
    let state = state_of("Hello", TextRange::at(1));
    let mut ui = field(state, Some(no_a), None, false);
    ui.on_node_with_tag(TAG).perform_text_input("abc");
    ui.on_node_with_tag(TAG).assert_is_focused().assert_text_equals(&["Hbcello"]);
    assert_eq!(state.text().as_str(), "Hbcello");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_clickAction
#[test]
fn a_click_focuses_it() {
    let mut ui = field(state_of("", TextRange::ZERO), None, None, false);
    ui.on_node_with_tag(TAG).assert_is_not_focused().perform_semantics_action(&A::ON_CLICK);
    ui.on_node_with_tag(TAG).assert_is_focused();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_imeOption
#[test]
fn it_tells_its_action_key() {
    let state = state_of("", TextRange::ZERO);
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            keyboard_options =
                KeyboardOptions { ime_action: ImeAction::Search, ..KeyboardOptions::DEFAULT },
        );
    });
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
    assert_eq!(node.config.get(&P::IME_ACTION).copied(), Some(ImeAction::Search));
}
