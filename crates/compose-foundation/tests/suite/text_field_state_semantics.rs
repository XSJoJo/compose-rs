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

fn selection_told(ui: &mut ComposeTestRule) -> Option<TextRange> {
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
    node.config.get(&P::TEXT_SELECTION_RANGE).copied()
}

/// The set-selection action, as `performTextInputSelection`.
fn select(ui: &mut ComposeTestRule, selection: TextRange) {
    ui.on_node_with_tag(TAG).perform_semantics_action_with(&A::SET_SELECTION, |set| {
        set(selection.start as i32, selection.end as i32, false);
    });
    ui.wait_for_idle();
}

fn has<T: compose_ui::SemanticsValue>(
    ui: &mut ComposeTestRule,
    key: &'static compose_ui::SemanticsPropertyKey<T>,
) -> bool {
    ui.on_node_with_tag(TAG).fetch_semantics_snapshot().config.contains(key)
}

fn set_clipboard(ui: &mut ComposeTestRule, text: &str) {
    ui.host().with_tree(|tree| tree.clipboard().set_text(text));
}

fn clipboard(ui: &mut ComposeTestRule) -> Option<String> {
    ui.host().with_tree(|tree| tree.clipboard().get_text())
}

fn cell<T: PartialEq + 'static>(value: T) -> MutableState<T> {
    Box::leak(Box::new(mutable_state_of(value))).handle()
}

/// A field over `state`, enabled and read-only as the two cells say.
fn switchable(
    state: TextFieldState,
    enabled: MutableState<bool>,
    read_only: MutableState<bool>,
) -> ComposeTestRule {
    compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            enabled = enabled.get(),
            read_only = read_only.get(),
        );
    })
}

/// The text the field laid out, as `fetchTextLayoutResult`.
fn laid_out(ui: &mut ComposeTestRule) -> String {
    let mut results: Vec<TextLayoutResult> = Vec::new();
    ui.on_node_with_tag(TAG).perform_semantics_action_with(&A::GET_TEXT_LAYOUT_RESULT, |get| {
        get(&mut results);
    });
    results.pop().expect("a layout").layout_input.text.text().to_string()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, contentSemanticsAreSet_inTheFirstComposition
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, contentSemanticsAreSet_afterRecomposition
#[test]
fn it_tells_its_text_at_first_and_after_an_edit() {
    let state = state_of("hello", TextRange::at(5));
    let mut ui = field(state, None, None, false);
    ui.on_node_with_tag(TAG).assert_text_equals(&["hello"]);
    state.set_text_and_place_cursor_at_end("hello2");
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_text_equals(&["hello2"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, selectionSemanticsAreSet_inTheFirstComposition
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, selectionSemanticsAreSet_afterRecomposition
#[test]
fn it_tells_its_selection_at_first_and_after_an_edit() {
    let mut ui = field(state_of("hello", TextRange::at(2)), None, None, false);
    ui.on_node_with_tag(TAG).assert_text_equals(&["hello"]);
    assert_eq!(selection_told(&mut ui), Some(TextRange::at(2)));

    let state = state_of("hello", TextRange::ZERO);
    let mut ui = field(state, None, None, false);
    assert_eq!(selection_told(&mut ui), Some(TextRange::ZERO));
    state.edit(|buffer| buffer.set_selection(TextRange::at(2)));
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_text_equals(&["hello"]);
    assert_eq!(selection_told(&mut ui), Some(TextRange::at(2)));
}

/// The selection set through semantics is the state's (Compose's also shows the selection's
/// handles: they wait on touch-text-selection).
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, inputSelection_changesSelectionState
#[test]
fn a_selection_set_through_semantics_is_the_states() {
    let state = state_of("hello", TextRange::at(5));
    let mut ui = field(state, None, None, false);
    select(&mut ui, TextRange::new(2, 3));
    assert_eq!(state.selection(), TextRange::new(2, 3));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, inputSelection_changesSelectionState_appliesFilter
#[test]
fn a_selection_set_through_semantics_goes_through_the_input_transformation() {
    let state = state_of("hello", TextRange::at(5));
    let revert = InputTransformation::from_fn(|buffer| buffer.revert_all_changes());
    let mut ui = field(state, Some(revert), None, false);
    select(&mut ui, TextRange::at(2));
    assert_eq!(state.selection(), TextRange::at(5));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, textLayoutResultSemanticsAreSet_inTheFirstComposition
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, textLayoutResultSemanticsAreUpdated_afterRecomposition
#[test]
fn its_layout_result_is_told_at_first_and_after_typing() {
    let mut ui = field(state_of("hello", TextRange::at(5)), None, None, false);
    ui.on_node_with_tag(TAG).assert_text_equals(&["hello"]);
    assert_eq!(laid_out(&mut ui), "hello");

    let mut ui = field(state_of("", TextRange::ZERO), None, None, false);
    ui.on_node_with_tag(TAG).assert_text_equals(&[""]).perform_text_input("hello");
    assert_eq!(laid_out(&mut ui), "hello");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semanticsAreSet_afterStateObjectChanges
#[test]
fn another_state_is_told() {
    let first = state_of("hello", TextRange::ZERO);
    let second = state_of("world", TextRange::at(2));
    let chosen = cell(true);
    let mut ui = compose_test(move || {
        let state = if chosen.get() { first } else { second };
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag(TAG));
    });
    ui.on_node_with_tag(TAG).assert_text_equals(&["hello"]);
    assert_eq!(selection_told(&mut ui), Some(TextRange::ZERO));
    chosen.set(false);
    ui.wait_for_idle();
    ui.on_node_with_tag(TAG).assert_text_equals(&["world"]);
    assert_eq!(selection_told(&mut ui), Some(TextRange::at(2)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_paste_notAvailable_whenDisabledOrReadOnly
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_cut_notAvailable_whenDisabledOrReadOnly
#[test]
fn paste_and_cut_are_an_editable_fields() {
    let (enabled, read_only) = (cell(false), cell(false));
    let mut ui = switchable(state_of("World!", TextRange::new(0, 1)), enabled, read_only);
    assert!(!has(&mut ui, &A::PASTE_TEXT) && !has(&mut ui, &A::CUT_TEXT));
    enabled.set(true);
    read_only.set(true);
    ui.wait_for_idle();
    assert!(!has(&mut ui, &A::PASTE_TEXT) && !has(&mut ui, &A::CUT_TEXT));
    read_only.set(false);
    ui.wait_for_idle();
    assert!(has(&mut ui, &A::PASTE_TEXT) && has(&mut ui, &A::CUT_TEXT));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_paste
#[test]
fn paste_replaces_the_selection() {
    let state = state_of("Here World!", TextRange::at(11));
    let mut ui = field(state, None, None, false);
    set_clipboard(&mut ui, "Hello");
    select(&mut ui, TextRange::new(0, 4));
    ui.on_node_with_tag(TAG).perform_semantics_action(&A::PASTE_TEXT);
    ui.wait_for_idle();
    assert_eq!((state.text().as_str(), state.selection()), ("Hello World!", TextRange::at(5)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_paste_appliesFilter
#[test]
fn paste_goes_through_the_input_transformation() {
    let state = state_of("Here World!", TextRange::at(11));
    let no_l = InputTransformation::from_fn(|buffer| {
        if buffer.changes().change_count() != 0 {
            let text = buffer.as_str().replace('l', "");
            let length = buffer.length();
            buffer.replace(0, length, &text);
            buffer.place_cursor_at_end();
        }
    });
    let mut ui = field(state, Some(no_l), None, false);
    set_clipboard(&mut ui, "Hello");
    select(&mut ui, TextRange::new(0, 4));
    ui.on_node_with_tag(TAG).perform_semantics_action(&A::PASTE_TEXT);
    ui.wait_for_idle();
    assert_eq!((state.text().as_str(), state.selection()), ("Heo Word!", TextRange::at(9)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_copy
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_copy_disabled_whenSelectionCollapsed
#[test]
fn copy_is_a_selections_and_copies_it() {
    let state = state_of("Hello World!", TextRange::at(12));
    let mut ui = field(state, None, None, false);
    assert!(!has(&mut ui, &A::COPY_TEXT));
    select(&mut ui, TextRange::new(0, 5));
    ui.on_node_with_tag(TAG).perform_semantics_action(&A::COPY_TEXT);
    ui.wait_for_idle();
    assert_eq!(clipboard(&mut ui).as_deref(), Some("Hello"));
    // Copying collapses the selection to its end, as Compose's `copy(cancelSelection = true)`.
    assert_eq!(state.selection(), TextRange::at(5));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_copy_appliesFilter
#[test]
fn copy_collapses_the_selection_through_the_input_transformation() {
    let state = state_of("Hello World!", TextRange::new(0, 5));
    let keep_selection = InputTransformation::from_fn(|buffer| {
        if buffer.selection() != buffer.original_selection() {
            buffer.revert_all_changes();
        }
    });
    let mut ui = field(state, Some(keep_selection), None, false);
    ui.on_node_with_tag(TAG).perform_semantics_action(&A::COPY_TEXT);
    ui.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(0, 5));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_cut
#[test]
fn cut_removes_the_selection_to_the_clipboard() {
    let state = state_of("Hello World!", TextRange::new(0, 5));
    let mut ui = field(state, None, None, false);
    ui.on_node_with_tag(TAG).perform_semantics_action(&A::CUT_TEXT);
    ui.wait_for_idle();
    assert_eq!((state.text().as_str(), state.selection()), (" World!", TextRange::ZERO));
    assert_eq!(clipboard(&mut ui).as_deref(), Some("Hello"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_cut_appliesFilter
#[test]
fn cut_goes_through_the_input_transformation() {
    let state = state_of("Hello World!", TextRange::new(0, 5));
    let revert = InputTransformation::from_fn(|buffer| buffer.revert_all_changes());
    let mut ui = field(state, Some(revert), None, false);
    ui.on_node_with_tag(TAG).perform_semantics_action(&A::CUT_TEXT);
    ui.wait_for_idle();
    assert_eq!((state.text().as_str(), state.selection()), ("Hello World!", TextRange::new(0, 5)));
    assert_eq!(clipboard(&mut ui).as_deref(), Some("Hello"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_isNotEditable_whenDisabledOrReadOnly
#[test]
fn it_is_editable_only_enabled_and_not_read_only() {
    let (enabled, read_only) = (cell(true), cell(false));
    let mut ui = switchable(state_of("", TextRange::ZERO), enabled, read_only);
    let editable = |ui: &mut ComposeTestRule| {
        let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
        node.config.get(&P::IS_EDITABLE).copied()
    };
    assert_eq!(editable(&mut ui), Some(true));
    for (enable, read) in [(true, true), (false, false), (false, true), (true, false)] {
        enabled.set(enable);
        read_only.set(read);
        ui.wait_for_idle();
        assert_eq!(editable(&mut ui), Some(enable && !read), "enabled {enable}, read-only {read}");
    }
}

static TRANSFORMATION_KEY: compose_ui::SemanticsPropertyKey<i32> =
    compose_ui::SemanticsPropertyKey::new("InputTransformation");

/// A transformation telling a number through semantics and changing nothing.
struct Telling(MutableState<i32>);

impl InputTransform for Telling {
    fn transform_input(&self, _buffer: &mut TextFieldBuffer) {}

    fn apply_semantics(&self, config: &mut compose_ui::SemanticsConfiguration) {
        config.set(&TRANSFORMATION_KEY, self.0.get());
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, inputTransformationSemantics_areApplied
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, inputTransformationSemantics_areApplied_stateBacked
#[test]
fn the_input_transformations_semantics_are_told_and_follow_its_state() {
    let number = cell(1);
    let transformation = InputTransformation::new(Telling(number));
    let mut ui = field(state_of("", TextRange::ZERO), Some(transformation), None, false);
    let told = |ui: &mut ComposeTestRule| {
        let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
        node.config.get(&TRANSFORMATION_KEY).copied()
    };
    assert_eq!(told(&mut ui), Some(1));
    number.set(2);
    ui.wait_for_idle();
    assert_eq!(told(&mut ui), Some(2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, maxLengthInputTransformationSemantics
#[test]
fn a_maximum_length_is_told() {
    let trim = InputTransformation::max_length_trim(10);
    let mut ui = field(state_of("", TextRange::ZERO), Some(trim), None, false);
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot();
    assert_eq!(node.config.get(&P::MAX_TEXT_LENGTH).copied(), Some(10));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, passwordSemantics_whenIsPassword_isToggled
#[test]
fn a_password_is_told_and_neither_copied_nor_cut() {
    let password = cell(false);
    let mut ui = compose_test(move || {
        let state = remember_text_field_state("Hello", Some(TextRange::new(0, 2)));
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            password = password.get().then_some(true),
        );
    });
    let told = |ui: &mut ComposeTestRule| {
        (has(ui, &P::PASSWORD), has(ui, &A::COPY_TEXT), has(ui, &A::CUT_TEXT))
    };
    assert_eq!(told(&mut ui), (false, true, true));
    password.set(true);
    ui.wait_for_idle();
    assert_eq!(told(&mut ui), (true, false, false));
    password.set(false);
    ui.wait_for_idle();
    assert_eq!(told(&mut ui), (false, true, true));
}
