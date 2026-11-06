//! A state-based field's input transformations, keyboard options and keyboard action handler
//! (D1133).

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    CommitTextCommand, ImeOptions, Key, Locale, SemanticsActions as A, SemanticsConfiguration,
    SemanticsProperties as P, SetComposingTextCommand, TextRange, UnitExt,
};

// ---- KeyboardOptions ------------------------------------------------------------------------

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/KeyboardOptionsTest.kt, toImeOptions_copiesRelevantProperties
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/KeyboardOptionsTest.kt, toImeOptions_replacesUnspecifiedValues
#[test]
fn options_for_the_input_method_keep_what_is_said_and_default_the_rest() {
    let options = KeyboardOptions {
        keyboard_type: KeyboardType::Number,
        ime_action: ImeAction::Go,
        capitalization: KeyboardCapitalization::Sentences,
        auto_correct_enabled: Some(false),
    };
    assert_eq!(
        options.to_ime_options(true),
        ImeOptions {
            single_line: true,
            capitalization: KeyboardCapitalization::Sentences,
            auto_correct: false,
            keyboard_type: KeyboardType::Number,
            ime_action: ImeAction::Go,
        }
    );
    assert_eq!(
        KeyboardOptions::DEFAULT.to_ime_options(false),
        ImeOptions {
            single_line: false,
            capitalization: KeyboardCapitalization::None,
            auto_correct: true,
            keyboard_type: KeyboardType::Text,
            ime_action: ImeAction::Default,
        }
    );
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/KeyboardOptionsTest.kt, fillUnspecifiedValuesWith_takesReceiverWhenOtherNull
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/KeyboardOptionsTest.kt, fillUnspecifiedValuesWith_takesReceiverWhenOtherEqual
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/KeyboardOptionsTest.kt, fillUnspecifiedValuesWith_takesReceiverWhenOtherAllUnspecified
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/KeyboardOptionsTest.kt, fillUnspecifiedValuesWith_takesOtherWhenReceiverAllUnspecified
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/KeyboardOptionsTest.kt, fillUnspecifiedValuesWith_prefersReceiv3er
#[test]
fn filling_unspecified_values_prefers_what_is_said() {
    let password =
        KeyboardOptions { keyboard_type: KeyboardType::Password, ..KeyboardOptions::DEFAULT };
    assert_eq!(password.fill_unspecified_values_with(None), password);
    assert_eq!(password.fill_unspecified_values_with(Some(&password)), password);
    assert_eq!(password.fill_unspecified_values_with(Some(&KeyboardOptions::DEFAULT)), password);
    assert_eq!(KeyboardOptions::DEFAULT.fill_unspecified_values_with(Some(&password)), password);
    let receiver = KeyboardOptions {
        capitalization: KeyboardCapitalization::Sentences,
        auto_correct_enabled: Some(false),
        keyboard_type: KeyboardType::Password,
        ime_action: ImeAction::Search,
    };
    let other = KeyboardOptions {
        capitalization: KeyboardCapitalization::Words,
        auto_correct_enabled: Some(true),
        keyboard_type: KeyboardType::Phone,
        ime_action: ImeAction::Search,
    };
    assert_eq!(receiver.fill_unspecified_values_with(Some(&other)), receiver);
}

// ---- InputTransformation --------------------------------------------------------------------

/// A transformation of nothing, saying `options` and setting the longest text to `length`.
struct Says {
    options: Option<KeyboardOptions>,
    length: Option<i32>,
}

impl InputTransform for Says {
    fn transform_input(&self, _buffer: &mut TextFieldBuffer) {}
    fn keyboard_options(&self) -> Option<KeyboardOptions> {
        self.options
    }
    fn apply_semantics(&self, config: &mut SemanticsConfiguration) {
        if let Some(length) = self.length {
            config.set_max_text_length(length);
        }
    }
}

fn saying(options: Option<KeyboardOptions>) -> InputTransformation {
    InputTransformation::new(Says { options, length: None })
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_areEqual
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_areNotEqual_whenFiltersAreDifferent
#[test]
fn chains_are_equal_by_their_parts() {
    let first = InputTransformation::from_fn(|_| {});
    let second = InputTransformation::from_fn(|_| {});
    let third = InputTransformation::from_fn(|_| {});
    assert_eq!(first.clone().then(second.clone()), first.clone().then(second.clone()));
    assert_ne!(first.clone().then(second), first.then(third));
    assert_eq!(InputTransformation::max_length_trim(5), InputTransformation::max_length_trim(5));
    assert_ne!(InputTransformation::max_length_trim(5), InputTransformation::max_length_reject(5));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_haveNullKeyboardOptions_whenBothOptionsAreNull
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_takeFirstKeyboardOptions_whenSecondOptionsAreNull
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_takeSecondKeyboardOptions_whenFirstOptionsAreNull
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_takeSecondKeyboardOptions_whenFirstOptionsAreNotNull
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_mergeKeyboardOptions_withPrecedenceToNext
#[test]
fn a_chains_keyboard_options_are_its_seconds_filled_with_its_firsts() {
    let some = Some(KeyboardOptions::DEFAULT);
    assert_eq!(saying(None).then(saying(None)).keyboard_options(), None);
    assert_eq!(saying(some).then(saying(None)).keyboard_options(), some);
    assert_eq!(saying(None).then(saying(some)).keyboard_options(), some);
    let first = KeyboardOptions {
        keyboard_type: KeyboardType::Password,
        capitalization: KeyboardCapitalization::Sentences,
        ..KeyboardOptions::DEFAULT
    };
    let second = KeyboardOptions {
        keyboard_type: KeyboardType::Email,
        ime_action: ImeAction::Search,
        ..KeyboardOptions::DEFAULT
    };
    assert_eq!(
        saying(Some(first)).then(saying(Some(second))).keyboard_options(),
        Some(KeyboardOptions {
            keyboard_type: KeyboardType::Email,
            capitalization: KeyboardCapitalization::Sentences,
            ime_action: ImeAction::Search,
            ..KeyboardOptions::DEFAULT
        })
    );
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, chainedFilters_applySecondSemantics_afterFirstSemantics
#[test]
fn a_chains_semantics_are_its_seconds_last() {
    let first = InputTransformation::new(Says { options: None, length: Some(10) });
    let second = InputTransformation::new(Says { options: None, length: Some(20) });
    let mut config = SemanticsConfiguration::default();
    first.then(second).apply_semantics(&mut config);
    assert_eq!(config.get(&P::MAX_TEXT_LENGTH).copied(), Some(20));
}

// ---- The field ------------------------------------------------------------------------------

fn field_with(
    text: &str,
    transformation: InputTransformation,
    keyboard_options: KeyboardOptions,
) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::new(text)));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            input_transformation = Some(transformation.clone()),
            keyboard_options = keyboard_options,
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    (ui, state)
}

fn shown(ui: &mut ComposeTestRule) -> String {
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    node.config.get(&P::EDITABLE_TEXT).map(|text| text.to_string()).unwrap_or_default()
}

/// Every way of typing goes through the transformation: keys' text, the input method's commands,
/// semantics' text and a paste; the field shows what it left.
#[test]
fn every_input_path_goes_through_the_transformation() {
    let limit = || InputTransformation::max_length_trim(5);
    let (mut ui, state) = field_with("", limit(), KeyboardOptions::DEFAULT);
    ui.type_text("abcdefg");
    assert_eq!(state.text().as_str(), "abcde");
    assert_eq!(shown(&mut ui), "abcde");
    let (mut ui, state) = field_with("", limit(), KeyboardOptions::DEFAULT);
    assert!(ui.host().send_edit_commands(&[&CommitTextCommand::new("1234567", 1)]));
    ui.wait_for_idle();
    assert_eq!(state.text().as_str(), "12345");
    let (mut ui, state) = field_with("", limit(), KeyboardOptions::DEFAULT);
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_TEXT, |set| {
        set("hello world".into());
    });
    assert_eq!(state.text().as_str(), "hello");
    let (mut ui, state) = field_with("ab", limit(), KeyboardOptions::DEFAULT);
    ui.host().with_tree(|tree| tree.clipboard().set_text("cdefgh"));
    ui.on_node_with_tag("field").perform_semantics_action(&A::PASTE_TEXT);
    assert_eq!(state.text().as_str(), "abcde");
    assert_eq!(state.selection(), TextRange::at(5));
}

/// A transformation changing what the input method composed ends the composition and restarts
/// it, as Compose's.
#[test]
fn a_transformed_composition_restarts_the_input_method() {
    let caps = InputTransformation::all_caps(Locale::new("en"));
    let (mut ui, state) = field_with("", caps, KeyboardOptions::DEFAULT);
    ui.host().take_input_method_restart();
    assert!(ui.host().send_edit_commands(&[&SetComposingTextCommand::new("hi", 1)]));
    ui.wait_for_idle();
    assert_eq!(state.text().as_str(), "HI");
    assert_eq!(state.composition(), None);
    assert!(ui.host().take_input_method_restart());
    let value = ui.host().input_method_value().expect("a value");
    assert_eq!(value.text(), "HI", "what the keyboard reads");
}

/// The transformation's keyboard options fill what the field's don't say, and its semantics are
/// the field's.
#[test]
fn the_transformation_says_the_keyboard_and_the_longest_text() {
    let caps = InputTransformation::all_caps(Locale::new("en"))
        .then(InputTransformation::max_length_reject(8));
    let options = KeyboardOptions { ime_action: ImeAction::Search, ..KeyboardOptions::DEFAULT };
    let (mut ui, _) = field_with("", caps, options);
    let ime = ui.host().text_input_options().expect("options");
    assert_eq!(ime.capitalization, KeyboardCapitalization::Characters);
    assert_eq!(ime.ime_action, ImeAction::Search);
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    assert_eq!(node.config.get(&P::MAX_TEXT_LENGTH).copied(), Some(8));
}

/// A rejected edit leaves the text and the cursor as they were.
#[test]
fn a_rejected_edit_leaves_the_field_as_it_was() {
    let (mut ui, state) =
        field_with("abc", InputTransformation::max_length_reject(3), KeyboardOptions::DEFAULT);
    ui.type_text("d");
    assert_eq!((state.text().as_str(), state.selection()), ("abc", TextRange::at(3)));
    ui.press_key(Key::Backspace);
    assert_eq!(state.text().as_str(), "ab");
}

// ---- KeyboardActionHandler ------------------------------------------------------------------

/// A field between two focusable boxes, its action `action`, its handler `handler`.
fn field_between_boxes(
    action: ImeAction,
    handler: Option<KeyboardActionHandler>,
    line_limits: TextFieldLineLimits,
) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::new("")));
    let mut ui = compose_test(move || {
        let handler = handler.clone();
        Column!({
            Box!(Modifier.size(1.dp()).focusable().test_tag("box1"));
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.width(200.dp()).test_tag("field"),
                keyboard_options =
                    KeyboardOptions { ime_action: action, ..KeyboardOptions::DEFAULT },
                on_keyboard_action = handler.clone(),
                line_limits = line_limits,
            );
            Box!(Modifier.size(1.dp()).focusable().test_tag("box2"));
        });
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    (ui, state)
}

fn counting(calls: &Rc<Cell<u32>>, default: bool) -> Option<KeyboardActionHandler> {
    let calls = calls.clone();
    Some(KeyboardActionHandler::new(move |perform_default| {
        calls.set(calls.get() + 1);
        if default {
            perform_default();
        }
    }))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_performsImeAction_viaSemantics
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_performsImeAction_viaInputConnection
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_performsUnexpectedImeAction_fromInputConnection
#[test]
fn the_handler_hears_the_action_from_semantics_and_the_input_method() {
    let calls = Rc::new(Cell::new(0));
    let multi = TextFieldLineLimits::DEFAULT;
    let (mut ui, _) = field_between_boxes(ImeAction::Send, counting(&calls, false), multi);
    ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
    assert_eq!(calls.get(), 1, "semantics");
    assert!(ui.host().perform_ime_action(ImeAction::Send));
    ui.wait_for_idle();
    assert_eq!(calls.get(), 2, "the input method's action");
    assert!(ui.host().perform_ime_action(ImeAction::Search));
    ui.wait_for_idle();
    assert_eq!(calls.get(), 3, "an action the keyboard wasn't asked for");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_performsDefaultBehavior_forFocusNext
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_performsDefaultBehavior_forFocusPrevious
#[test]
fn without_a_handler_next_and_previous_move_the_focus() {
    for (action, tag) in [(ImeAction::Next, "box2"), (ImeAction::Previous, "box1")] {
        let (mut ui, _) = field_between_boxes(action, None, TextFieldLineLimits::DEFAULT);
        ui.on_node_with_tag(tag).assert_is_not_focused();
        ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
        ui.on_node_with_tag(tag).assert_is_focused();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_performsDefaultBehavior_forDone
#[test]
fn without_a_handler_done_hides_the_keyboard() {
    let (mut ui, _) = field_between_boxes(ImeAction::Done, None, TextFieldLineLimits::DEFAULT);
    ui.host().take_software_keyboard_request();
    ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
    assert_eq!(ui.host().take_software_keyboard_request(), Some(false));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_canOverrideDefaultBehavior
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_canRequestDefaultBehavior
#[test]
fn a_handler_replaces_the_default_unless_it_asks_for_it() {
    let calls = Rc::new(Cell::new(0));
    let multi = TextFieldLineLimits::DEFAULT;
    let (mut ui, _) = field_between_boxes(ImeAction::Next, counting(&calls, false), multi);
    ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
    ui.on_node_with_tag("field").assert_is_focused();
    ui.on_node_with_tag("box2").assert_is_not_focused();
    let (mut ui, _) = field_between_boxes(ImeAction::Next, counting(&calls, true), multi);
    ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
    ui.on_node_with_tag("box2").assert_is_focused();
    assert_eq!(calls.get(), 2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_changingKeyboardActions_usesNewKeyboardActions
#[test]
fn a_new_handler_is_the_one_called() {
    let last = Rc::new(Cell::new(0));
    let (first, second) = (last.clone(), last.clone());
    let handlers = [
        KeyboardActionHandler::new(move |_| first.set(1)),
        KeyboardActionHandler::new(move |_| second.set(2)),
    ];
    let which = Box::leak(Box::new(mutable_state_of(0usize))).handle();
    let state = **Box::leak(Box::new(TextFieldState::new("")));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(200.dp()).test_tag("field"),
            keyboard_options =
                KeyboardOptions { ime_action: ImeAction::Done, ..KeyboardOptions::DEFAULT },
            on_keyboard_action = Some(handlers[which.get()].clone()),
        );
    });
    ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
    ui.wait_for_idle();
    assert_eq!(last.get(), 1);
    which.set(1);
    ui.wait_for_idle();
    ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
    ui.wait_for_idle();
    assert_eq!(last.get(), 2);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_singleLinePressEnter_triggersKeyboardAction
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_multiLinePressEnter_doesNotTriggerKeyboardAction
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_singleLinePressEnter_triggersDefaultBehavior
#[test]
fn enter_performs_a_single_lines_action_only() {
    let calls = Rc::new(Cell::new(0));
    let single = TextFieldLineLimits::SingleLine;
    let (mut ui, _) = field_between_boxes(ImeAction::Go, counting(&calls, false), single);
    ui.press_key(Key::Enter);
    ui.wait_for_idle();
    assert_eq!(calls.get(), 1, "a single line");
    let one_high = TextFieldLineLimits::multi_line(1, 1);
    let (mut ui, state) = field_between_boxes(ImeAction::Go, counting(&calls, false), one_high);
    ui.press_key(Key::Enter);
    ui.wait_for_idle();
    assert_eq!(calls.get(), 1, "a multi-line field types a new line");
    assert_eq!(state.text().as_str(), "\n");
    let (mut ui, _) = field_between_boxes(ImeAction::Next, None, single);
    ui.press_key(Key::Enter);
    ui.on_node_with_tag("box2").assert_is_focused();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_ImeActionNone_isNotPassedToKeyboardActionHandler
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldKeyboardActionsTest.kt, textField_ImeActionDefault_isNotPassedToKeyboardActionHandler
#[test]
fn none_and_default_are_never_the_handlers() {
    let calls = Rc::new(Cell::new(0));
    let multi = TextFieldLineLimits::DEFAULT;
    let (mut ui, _) = field_between_boxes(ImeAction::None, counting(&calls, false), multi);
    ui.on_node_with_tag("field").perform_semantics_action(&A::ON_IME_ACTION);
    let (mut ui, _) = field_between_boxes(ImeAction::Default, counting(&calls, false), multi);
    ui.host().perform_ime_action(ImeAction::None);
    ui.host().perform_ime_action(ImeAction::Default);
    ui.wait_for_idle();
    assert_eq!(calls.get(), 0);
}
