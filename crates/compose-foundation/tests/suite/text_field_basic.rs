//! The state-based field as Compose's `BasicTextFieldTest` has it: its state and semantics after
//! each input path, the input session's life, transformations on every path, the changes they
//! see, keyboard options, focus by taps and swipes, relayouts.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::gestures::ScrollableState;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{FixedTextMeasurer, UiHost};
use compose_ui::{
    CommitTextCommand, DeleteSurroundingTextCommand, Density, EditCommand,
    FinishComposingTextCommand, IntSize, Key, Locale, LocaleList, SemanticsActions as A,
    SemanticsProperties as P, SetComposingRegionCommand, SetComposingTextCommand,
    SetSelectionCommand, TextDirection, TextLayoutResult, TextRange, TextStyle, UnitExt,
};

fn state_of(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

fn state_with(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
}

fn cell<T: PartialEq + 'static>(value: T) -> MutableState<T> {
    Box::leak(Box::new(mutable_state_of(value))).handle()
}

/// A field over `state`, tagged "field", with `transformation`.
fn field(state: TextFieldState, transformation: Option<InputTransformation>) -> ComposeTestRule {
    compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            input_transformation = transformation.clone(),
        );
    })
}

/// An input session is open: a focused field taking text, as Compose's `assertSessionActive`.
fn session(ui: &mut ComposeTestRule) -> bool {
    ui.host().text_input_options().is_some()
}

/// What the field shows, as `assertTextEquals` reads it.
fn shown(ui: &mut ComposeTestRule) -> String {
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    node.config.get(&P::EDITABLE_TEXT).map(|text| text.to_string()).unwrap_or_default()
}

/// The commands as one batch, as Compose's input connection.
fn send(ui: &mut ComposeTestRule, commands: &[&dyn EditCommand]) {
    assert!(ui.host().send_edit_commands(commands));
    ui.wait_for_idle();
}

/// Compose's test `commitText`: a batch finishing what's composed, then committing `text`.
fn commit(ui: &mut ComposeTestRule, text: &str) {
    send(ui, &[&FinishComposingTextCommand, &CommitTextCommand::new(text, 1)]);
}

fn set_text(ui: &mut ComposeTestRule, text: &str) {
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_TEXT, |set| {
        set(text.into());
    });
}

fn insert_text(ui: &mut ComposeTestRule, text: &str) {
    ui.on_node_with_tag("field").perform_semantics_action_with(
        &A::INSERT_TEXT_AT_CURSOR,
        |insert| {
            insert(text.into());
        },
    );
}

/// Typed on a physical keyboard: each character's key down, its text, its key up, as a platform
/// sends them (Compose's `performKeyInput { pressKey(…) }`).
pub(crate) fn type_on_keys(ui: &mut ComposeTestRule, text: &str) {
    for character in text.chars() {
        let key = Key::Character(character);
        ui.host().send_key(compose_ui::KeyEvent::down(key));
        ui.host().send_text(&character.to_string());
        ui.host().send_key(compose_ui::KeyEvent::up(key));
    }
    ui.wait_for_idle();
}

/// Compose's test `RejectAllTextFilter`.
fn reject_all() -> InputTransformation {
    InputTransformation::from_fn(|buffer| buffer.revert_all_changes())
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textFieldState_textChange_updatesState
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textFieldState_textChange_updatesSemantics
#[test]
fn typed_text_goes_into_the_state_and_the_semantics() {
    let state = state_with("Hello ", TextRange::at(6));
    let mut ui = field(state, None);
    insert_text(&mut ui, "World!");
    assert_eq!(state.text().as_str(), "Hello World!");
    assert_eq!(shown(&mut ui), "Hello World!");
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    assert_eq!(node.config.get(&P::TEXT_SELECTION_RANGE).copied(), Some(TextRange::at(12)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_imeUpdatesDontCauseRecomposition
#[test]
fn typing_recomposes_nothing() {
    let state = state_of("");
    let compositions = Rc::new(Cell::new(0));
    let counted = compositions.clone();
    let mut ui = compose_test(move || {
        counted.set(counted.get() + 1);
        BasicTextFieldOfState!(state, modifier = Modifier.width(300.dp()).test_tag("field"));
    });
    ui.on_node_with_tag("field").perform_text_input("hello");
    ui.on_node_with_tag("field").perform_text_input("world");
    assert_eq!(shown(&mut ui), "helloworld");
    assert_eq!(compositions.get(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_textStyleColorChange_doesNotRelayout
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldDrawPhaseToggleTest.kt, colorChange_reflectsOnView
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldDrawPhaseToggleTest.kt, brushChange_reflectsOnView
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldDrawPhaseToggleTest.kt, shadowChange_reflectsOnView
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldDrawPhaseToggleTest.kt, textDecorationChange_reflectsOnView
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldLayoutPhaseToggleTest.kt, fontWeightChange_reflectsOnView
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldLayoutPhaseToggleTest.kt, textAlignChange_reflectsOnView
#[test]
fn a_new_color_draws_the_layout_again_without_laying_it_out() {
    let style = cell(TextStyle { color: compose_ui::Color::RED, ..TextStyle::DEFAULT });
    let heard: Rc<RefCell<Vec<TextLayoutResult>>> = Rc::default();
    let into = heard.clone();
    let state = state_of("Hello");
    let (measurer, counter) = CountingTextMeasurer::new(FixedTextMeasurer::new());
    let host = UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(measurer));
    let mut ui = ComposeTestRule::with_host(host, move || {
        let into = into.clone();
        BasicTextFieldOfState!(
            state,
            text_style = style.get(),
            on_text_layout = Some(move |result: TextLayoutResult| into.borrow_mut().push(result)),
        );
    });
    ui.wait_for_idle();
    counter.clear();
    style.set(TextStyle { color: compose_ui::Color::BLUE, ..TextStyle::DEFAULT });
    ui.wait_for_idle();
    let counts = counter.of("Hello");
    assert_eq!((counts.fresh, counts.relaid, counts.restyled), (0, 0, 1));
    let colors: Vec<_> = heard.borrow().iter().map(|it| it.layout_input.style.color).collect();
    assert_eq!(colors, [compose_ui::Color::RED, compose_ui::Color::BLUE]);

    // What only paints differently is drawn again; what lays out differently is laid out again.
    // By the layouts, where Compose's compares pixels.
    let blue = TextStyle { color: compose_ui::Color::BLUE, ..TextStyle::DEFAULT };
    let brush = compose_ui::Brush::horizontal_gradient(vec![
        compose_ui::Color::RED,
        compose_ui::Color::BLUE,
    ]);
    let shadow = compose_ui::Shadow {
        color: compose_ui::Color::BLACK,
        offset: compose_ui::Offset::new(2.0, 2.0),
        blur_radius: 1.0,
    };
    let painted = [
        TextStyle { brush: Some(brush), ..blue.clone() },
        TextStyle { shadow: Some(shadow), ..blue.clone() },
        TextStyle { text_decoration: Some(compose_ui::TextDecoration::UNDERLINE), ..blue.clone() },
    ];
    for paint in painted {
        counter.clear();
        style.set(paint.clone());
        ui.wait_for_idle();
        let counts = counter.of("Hello");
        assert_eq!((counts.fresh, counts.restyled), (0, 1), "{paint:?}");
    }
    let laid_out = [
        TextStyle { font_weight: Some(compose_ui::FontWeight::BOLD), ..blue.clone() },
        TextStyle { text_align: Some(compose_ui::TextAlign::Center), ..blue },
    ];
    for layout in laid_out {
        counter.clear();
        style.set(layout.clone());
        ui.wait_for_idle();
        let counts = counter.of("Hello");
        assert_eq!((counts.fresh, counts.restyled), (1, 0), "{layout:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_focus_showsSoftwareKeyboard
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_focus_doesNotShowSoftwareKeyboard_ifDisabled
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_focus_doesNotShowSoftwareKeyboard_ifReadOnly
#[test]
fn a_click_opens_a_session_unless_disabled_or_read_only() {
    for (enabled, read_only) in [(true, false), (false, false), (true, true)] {
        let state = state_of("");
        let mut ui = compose_test(move || {
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.width(300.dp()).test_tag("field"),
                enabled = enabled,
                read_only = read_only,
            );
        });
        if !enabled {
            ui.on_node_with_tag("field").assert_is_not_enabled();
        }
        ui.on_node_with_tag("field").perform_click();
        ui.wait_for_idle();
        if enabled {
            ui.on_node_with_tag("field").assert_is_focused();
        }
        assert_eq!(session(&mut ui), enabled && !read_only, "{enabled} {read_only}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, disposeSession_whenTextFieldIsRemoved
#[test]
fn removing_the_field_ends_its_session() {
    let shown_field = cell(true);
    let state = state_of("initial text");
    let mut ui = compose_test(move || {
        if shown_field.get() {
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
        }
    });
    ui.on_node_with_tag("field").request_focus();
    assert!(session(&mut ui));
    shown_field.set(false);
    ui.wait_for_idle();
    assert!(!session(&mut ui));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, disposeSessionWhenFocusCleared
#[test]
fn clearing_the_focus_ends_the_session() {
    let manager: Rc<RefCell<Option<compose_ui::FocusManager>>> = Rc::default();
    let into = manager.clone();
    let state = state_of("initial text");
    let mut ui = compose_test(move || {
        *into.borrow_mut() = Some(compose_ui::LOCAL_FOCUS_MANAGER.current());
        Row!({
            Box!(Modifier.size(10.dp()).focusable());
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
        });
    });
    ui.on_node_with_tag("field").request_focus();
    assert!(session(&mut ui));
    manager.borrow().as_ref().expect("composed").clear_focus(false);
    ui.wait_for_idle();
    assert!(!session(&mut ui));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_whenStateObjectChanges_newTextIsRendered
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_whenStateObjectChanges_restartsInput
#[test]
fn a_new_state_is_shown_and_edited() {
    let (first, second) = (state_of("Hello"), state_of("World"));
    let toggle = cell(true);
    let mut ui = compose_test(move || {
        let state = if toggle.get() { first } else { second };
        BasicTextFieldOfState!(state, modifier = Modifier.width(300.dp()).test_tag("field"));
    });
    assert_eq!(shown(&mut ui), "Hello");
    set_text(&mut ui, "Compose");
    assert_eq!(shown(&mut ui), "Compose");
    ui.on_node_with_tag("field").request_focus();
    ui.host().take_input_method_restart();
    toggle.set(false);
    ui.wait_for_idle();
    assert_eq!(shown(&mut ui), "World");
    assert!(ui.host().take_input_method_restart(), "the input method reads the new state");
    set_text(&mut ui, "Compose2");
    assert_eq!(shown(&mut ui), "Compose2");
    assert_eq!(first.text().as_str(), "Compose");
    assert_eq!(second.text().as_str(), "Compose2");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_passesKeyboardOptionsThrough
#[test]
fn the_keyboard_options_reach_the_input_method() {
    let state = state_of("");
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("field"),
            keyboard_options = KeyboardOptions {
                capitalization: KeyboardCapitalization::Characters,
                keyboard_type: KeyboardType::Email,
                ime_action: ImeAction::Previous,
                ..KeyboardOptions::DEFAULT
            },
        );
    });
    ui.on_node_with_tag("field").request_focus();
    let options = ui.host().text_input_options().expect("a session");
    assert_eq!(options.ime_action, ImeAction::Previous);
    assert_eq!(options.keyboard_type, KeyboardType::Email);
    assert_eq!(options.capitalization, KeyboardCapitalization::Characters);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toInputConnection
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toInputConnection_changingComposition
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toSetTextSemanticsAction
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toInsertTextSemanticsAction
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toKeyEvents
#[test]
fn a_rejecting_transformation_rejects_every_input_path() {
    type Input = fn(&mut ComposeTestRule);
    let inputs: [(&str, Input); 5] = [
        ("commit", |ui| commit(ui, "hello")),
        ("compose", |ui| send(ui, &[&SetComposingTextCommand::new("hello", 1)])),
        ("set text", |ui| set_text(ui, "hello")),
        ("insert text", |ui| insert_text(ui, "hello")),
        ("key", |ui| type_on_keys(ui, "a")),
    ];
    for (name, input) in inputs {
        let state = state_of("");
        let mut ui = field(state, Some(reject_all()));
        ui.on_node_with_tag("field").request_focus();
        input(&mut ui);
        assert_eq!(shown(&mut ui), "", "{name}");
        assert_eq!(state.composition(), None, "{name}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toInputConnection_afterChanging
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toSetTextSemanticsAction_afterChanging
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toInsertTextSemanticsAction_afterChanging
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_appliesFilter_toKeyEvents_afterChanging
#[test]
fn a_new_transformation_applies_from_the_next_edit() {
    type Input = fn(&mut ComposeTestRule, &str);
    let inputs: [(&str, Input, [&str; 3]); 4] = [
        ("commit", commit, ["hello", "hello", "helloworld"]),
        ("set text", set_text, ["hello", "hello", "world"]),
        ("insert text", insert_text, ["hello", "hello", "helloworld"]),
        ("key", type_on_keys, ["hello", "hello", "helloworld"]),
    ];
    for (name, input, expected) in inputs {
        let filter = cell(None::<InputTransformation>);
        let state = state_of("");
        let mut ui = compose_test(move || {
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.width(300.dp()).test_tag("field"),
                input_transformation = filter.get(),
            );
        });
        ui.on_node_with_tag("field").request_focus();
        input(&mut ui, "hello");
        assert_eq!(shown(&mut ui), expected[0], "{name}");
        filter.set(Some(reject_all()));
        ui.wait_for_idle();
        input(&mut ui, "world");
        assert_eq!(shown(&mut ui), expected[1], "{name}");
        filter.set(None);
        ui.wait_for_idle();
        input(&mut ui, "world");
        assert_eq!(shown(&mut ui), expected[2], "{name}");
    }
}

/// The changes the transformation last saw: each one's range and original range.
type Seen = Rc<RefCell<Vec<(TextRange, TextRange)>>>;

/// A field over `text` whose transformation keeps the changes of each edit it sees.
fn tracking(text: &str) -> (ComposeTestRule, TextFieldState, Seen) {
    let seen: Seen = Rc::default();
    let into = seen.clone();
    let transformation = InputTransformation::from_fn(move |buffer| {
        let changes = buffer.changes();
        if changes.change_count() > 0 {
            *into.borrow_mut() = (0..changes.change_count())
                .map(|index| (changes.get_range(index), changes.get_original_range(index)))
                .collect();
        }
    });
    let state = state_of(text);
    let mut ui = field(state, Some(transformation));
    ui.on_node_with_tag("field").request_focus();
    (ui, state, seen)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_changesAreTracked_whenInputConnectionCommits
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_changesAreTracked_whenInputConnectionComposes
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_changesAreTracked_whenKeyEventInserts
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_changesAreTracked_whenSemanticsActionInserts
#[test]
fn an_insertion_is_one_change_on_every_path() {
    type Input = fn(&mut ComposeTestRule);
    let inputs: [(&str, Input, usize); 4] = [
        ("commit", |ui| commit(ui, "hello"), 5),
        ("compose", |ui| send(ui, &[&SetComposingTextCommand::new("hello", 1)]), 5),
        ("key", |ui| type_on_keys(ui, "a"), 1),
        ("semantics", |ui| insert_text(ui, "hello"), 5),
    ];
    for (name, input, length) in inputs {
        let (mut ui, _, seen) = tracking("");
        input(&mut ui);
        assert_eq!(*seen.borrow(), [(TextRange::new(0, length), TextRange::ZERO)], "{name}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_changesAreTracked_whenInputConnectionDeletes
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_changesAreTracked_whenInputConnectionDeletesViaComposition
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_changesAreTracked_whenKeyEventDeletes
#[test]
fn a_deletion_is_one_change_on_every_path() {
    let (mut ui, _, seen) = tracking("hello");
    send(
        &mut ui,
        &[
            &FinishComposingTextCommand,
            &SetSelectionCommand::new(5, 5),
            &DeleteSurroundingTextCommand::new(1, 0),
        ],
    );
    assert_eq!(*seen.borrow(), [(TextRange::at(4), TextRange::new(4, 5))]);

    let (mut ui, _, seen) = tracking("hello");
    send(&mut ui, &[&SetComposingRegionCommand::new(0, 5), &SetComposingTextCommand::new("h", 1)]);
    assert_eq!(*seen.borrow(), [(TextRange::at(1), TextRange::new(1, 5))]);

    let (mut ui, _, seen) = tracking("hello");
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_SELECTION, |set| {
        set(5, 5, false);
    });
    ui.press_key(Key::Backspace);
    assert_eq!(*seen.borrow(), [(TextRange::at(4), TextRange::new(4, 5))]);
}

/// A transformation that says `options` and changes nothing.
fn saying(options: KeyboardOptions) -> InputTransformation {
    InputTransformation::new(Saying(Rc::new(move || options)))
}

struct Saying(Rc<dyn Fn() -> KeyboardOptions>);

impl InputTransform for Saying {
    fn transform_input(&self, _: &mut TextFieldBuffer) {}
    fn keyboard_options(&self) -> Option<KeyboardOptions> {
        Some((self.0)())
    }
}

fn options(keyboard_type: KeyboardType, ime_action: ImeAction) -> KeyboardOptions {
    KeyboardOptions { keyboard_type, ime_action, ..KeyboardOptions::DEFAULT }
}

/// A focused field with `transformation` and `keyboard_options`; the options the input method
/// reads.
fn options_told(
    transformation: State<InputTransformation>,
    keyboard_options: KeyboardOptions,
) -> ComposeTestRule {
    let state = state_of("");
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("field"),
            input_transformation = Some(transformation.get()),
            keyboard_options = keyboard_options,
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui
}

fn told(ui: &mut ComposeTestRule) -> (KeyboardType, ImeAction) {
    let options = ui.host().text_input_options().expect("a session");
    (options.keyboard_type, options.ime_action)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_filterKeyboardOptions_sentToIme
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_filterKeyboardOptions_mergedWithParams
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_filterKeyboardOptions_overriddenByParams
#[test]
fn a_transformations_keyboard_options_fill_the_fields() {
    let email_previous = saying(options(KeyboardType::Email, ImeAction::Previous));
    let mut ui = options_told(cell(email_previous).into(), KeyboardOptions::DEFAULT);
    assert_eq!(told(&mut ui), (KeyboardType::Email, ImeAction::Previous));

    let previous = saying(options(KeyboardType::Unspecified, ImeAction::Previous));
    let email = options(KeyboardType::Email, ImeAction::Unspecified);
    let mut ui = options_told(cell(previous.clone()).into(), email);
    assert_eq!(told(&mut ui), (KeyboardType::Email, ImeAction::Previous));

    let search = options(KeyboardType::Unspecified, ImeAction::Search);
    let mut ui = options_told(cell(previous).into(), search);
    assert_eq!(told(&mut ui).1, ImeAction::Search);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_filterKeyboardOptions_applyWhenFilterChanged
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_filterKeyboardOptions_applyWhenKeyboardOptionsChanged
#[test]
fn new_keyboard_options_restart_the_input_method() {
    let filter = cell(saying(options(KeyboardType::Email, ImeAction::Previous)));
    let mut ui = options_told(filter.into(), KeyboardOptions::DEFAULT);
    assert_eq!(told(&mut ui), (KeyboardType::Email, ImeAction::Previous));
    ui.host().take_input_method_restart();
    filter.set(saying(options(KeyboardType::Decimal, ImeAction::Search)));
    ui.wait_for_idle();
    assert!(ui.host().take_input_method_restart());
    assert_eq!(told(&mut ui), (KeyboardType::Decimal, ImeAction::Search));

    // The options a transformation reads from a state.
    let said = cell(options(KeyboardType::Email, ImeAction::Previous));
    let reading = InputTransformation::new(Saying(Rc::new(move || said.get())));
    let mut ui = options_told(cell(reading).into(), KeyboardOptions::DEFAULT);
    assert_eq!(told(&mut ui), (KeyboardType::Email, ImeAction::Previous));
    ui.host().take_input_method_restart();
    said.set(options(KeyboardType::Decimal, ImeAction::Search));
    ui.wait_for_idle();
    assert!(ui.host().take_input_method_restart());
    assert_eq!(told(&mut ui), (KeyboardType::Decimal, ImeAction::Search));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, changingInputTransformation_doesNotRestartInput
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, changingInputTransformation_restartsInput_ifKeyboardOptionsChange
#[test]
fn a_new_transformation_restarts_the_input_method_only_for_new_options() {
    let filter = cell(Some(InputTransformation::max_length_trim(10)));
    let state = state_of("");
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("field"),
            input_transformation = filter.get(),
        );
    });
    ui.on_node_with_tag("field").request_focus();
    assert!(session(&mut ui));
    ui.host().take_input_method_restart();
    filter.set(Some(InputTransformation::max_length_trim(15)));
    ui.wait_for_idle();
    assert!(session(&mut ui));
    assert!(!ui.host().take_input_method_restart());
    filter.set(Some(InputTransformation::all_caps(Locale::new("en"))));
    ui.wait_for_idle();
    assert!(session(&mut ui));
    assert!(ui.host().take_input_method_restart());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_showsKeyboardAgainWhenTapped_ifFocused
#[test]
fn a_tap_on_a_focused_field_shows_the_keyboard_again() {
    let mut ui = field(state_of(""), None);
    ui.on_node_with_tag("field").request_focus();
    ui.host().take_software_keyboard_request();
    ui.on_node_with_tag("field").perform_click();
    ui.wait_for_idle();
    assert_eq!(ui.host().take_software_keyboard_request(), Some(true));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, swipingThroughTextField_doesNotGainFocus
#[test]
fn a_swipe_through_the_field_does_not_focus_it() {
    let mut ui = field(state_of(""), None);
    ui.on_node_with_tag("field").perform_touch_input(|touch| {
        let (left, right) = (touch.left(), touch.right());
        touch.swipe_right_with(left, right + 200.0, 100);
    });
    ui.wait_for_idle();
    ui.on_node_with_tag("field").assert_is_not_focused();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, swipingTextFieldInScrollableContainer_doesNotGainFocus
#[test]
fn a_swipe_scrolling_the_container_does_not_focus_the_field() {
    let scroll_state = **Box::leak(Box::new(ScrollState::new(0)));
    let state = state_of("");
    let mut ui = compose_test(move || {
        Column!(modifier = Modifier.height(100.dp()).vertical_scroll(scroll_state), {
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
            Box!(Modifier.height(200.dp()));
        });
    });
    ui.on_node_with_tag("field").perform_touch_input(|touch| touch.swipe_up());
    ui.wait_for_idle();
    ui.on_node_with_tag("field").assert_is_not_focused();
    assert_ne!(scroll_state.value(), 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, densityChanges_causesRelayout
#[test]
fn a_new_density_lays_the_text_out_again() {
    let density = cell(1.0f32);
    let state = state_of("Hello");
    let mut ui = compose_test(move || {
        CompositionLocalProvider(
            compose_ui::LOCAL_DENSITY.provides(Density::new(density.get())),
            move || {
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.test_tag("field"),
                    text_style = TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT },
                );
            },
        );
    });
    let first = layout_of(&mut ui).size;
    density.set(2.0);
    ui.wait_for_idle();
    let second = layout_of(&mut ui).size;
    assert_eq!((second.width, second.height), (first.width * 2, first.height * 2));
}

/// The field's text layout, through its semantics action, as `fetchTextLayoutResult`.
fn layout_of(ui: &mut ComposeTestRule) -> TextLayoutResult {
    let mut results = Vec::new();
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::GET_TEXT_LAYOUT_RESULT, |get| {
        get(&mut results);
    });
    results.pop().expect("a layout")
}

/// A long click selects nothing yet (`touch-text-selection`): the text is selected through
/// semantics, then pasted over.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, whenPastingTextThatIncreasesEndOffset_noCrashAndCursorAtEndOfPastedText
#[test]
fn pasting_a_longer_text_over_the_selection_puts_the_cursor_after_it() {
    let (long, short) = ("Text".repeat(4), "Text".repeat(2));
    let state = state_of(&short);
    let mut ui = field(state, None);
    ui.host().with_tree(|tree| tree.clipboard().set_text(&long));
    let length = short.len() as i32;
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_SELECTION, |set| {
        set(0, length, false);
    });
    ui.on_node_with_tag("field").perform_semantics_action(&A::PASTE_TEXT);
    assert_eq!(state.text().as_str(), long);
    assert_eq!(state.selection(), TextRange::at(long.len()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, phoneKeyboardType_RtlLocaleLtrDigits_resolvesToLtrTextDirection
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, phoneKeyboardType_RtlLocaleRtlDigits_resolvesToRtlTextDirection
#[test]
fn a_phone_number_goes_the_way_its_digits_do() {
    for (locale, direction) in [("ar", TextDirection::Ltr), ("ff-Adlm-BF", TextDirection::Rtl)] {
        let heard: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
        let into = heard.clone();
        let state = state_of("");
        let mut ui = compose_test(move || {
            let into = into.clone();
            BasicTextFieldOfState!(
                state,
                text_style = TextStyle {
                    locale_list: Some(LocaleList::from_language_tags(locale)),
                    ..TextStyle::DEFAULT
                },
                keyboard_options = KeyboardOptions {
                    keyboard_type: KeyboardType::Phone,
                    ..KeyboardOptions::DEFAULT
                },
                on_text_layout = Some(move |result: TextLayoutResult| {
                    *into.borrow_mut() = Some(result);
                }),
            );
        });
        ui.wait_for_idle();
        let result = heard.borrow().clone().expect("laid out");
        assert_eq!(result.layout_input.style.text_direction, Some(direction), "{locale}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, longText_doesNotCrash
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, longText_doesNotCrash_singleLine
#[test]
fn a_long_text_is_laid_out_whole() {
    for single_line in [false, true] {
        let heard = Rc::new(Cell::new(0));
        let into = heard.clone();
        let state = state_of(&"A".repeat(100_000));
        let line_limits = if single_line {
            TextFieldLineLimits::SingleLine
        } else {
            TextFieldLineLimits::DEFAULT
        };
        let mut ui = compose_test(move || {
            let into = into.clone();
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.height(56.dp()).fill_max_width().test_tag("field"),
                line_limits = line_limits,
                on_text_layout = Some(move |result: TextLayoutResult| {
                    into.set(result.layout_input.text.len());
                }),
            );
        });
        ui.on_node_with_tag("field").request_focus();
        assert!(ui.host().cursor_anchor(true).is_some());
        assert_eq!(heard.get(), 100_000);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, whenElementFocusLost_compositionIsCleared
#[test]
fn losing_the_focus_ends_the_composition() {
    let manager: Rc<RefCell<Option<compose_ui::FocusManager>>> = Rc::default();
    let into = manager.clone();
    let state = state_of("");
    let mut ui = compose_test(move || {
        *into.borrow_mut() = Some(compose_ui::LOCAL_FOCUS_MANAGER.current());
        Column!({
            Box!(Modifier.size(10.dp()).focusable());
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
        });
    });
    ui.on_node_with_tag("field").request_focus();
    send(&mut ui, &[&SetComposingTextCommand::new("Hello", 1)]);
    assert_eq!(state.composition(), Some(TextRange::new(0, 5)));
    manager.borrow().as_ref().expect("composed").move_focus(compose_ui::FocusDirection::Previous);
    ui.wait_for_idle();
    assert_eq!(state.text().as_str(), "Hello");
    assert_eq!(state.composition(), None);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, whenCursorOutOfView_bringCursorIntoView
#[test]
fn a_new_line_brings_the_cursor_into_view() {
    let scroll_state = **Box::leak(Box::new(ScrollState::new(0)));
    let state = state_of("");
    let mut ui = compose_test(move || {
        Column!(modifier = Modifier.fill_max_size().vertical_scroll(scroll_state), {
            for _ in 0..100 {
                Box!(Modifier.size(100.dp()));
            }
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
            for _ in 0..100 {
                Box!(Modifier.size(100.dp()));
            }
        });
    });
    ui.on_node_with_tag("field").request_focus();
    let initial = scroll_state.value();
    commit(&mut ui, "\n");
    assert_ne!(scroll_state.value(), initial);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_multiLine_minLinesOneAndMaxLinesOne_doesSoftWrap
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldTest.kt, textField_singleLine_doesNotSoftWrap
#[test]
fn only_a_single_line_does_not_wrap() {
    for (line_limits, wraps) in
        [(TextFieldLineLimits::multi_line(1, 1), true), (TextFieldLineLimits::SingleLine, false)]
    {
        let heard: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
        let into = heard.clone();
        let state = state_of("");
        let mut ui = compose_test(move || {
            let into = into.clone();
            BasicTextFieldOfState!(
                state,
                line_limits = line_limits,
                on_text_layout = Some(move |result: TextLayoutResult| {
                    *into.borrow_mut() = Some(result);
                }),
            );
        });
        ui.wait_for_idle();
        let result = heard.borrow().clone().expect("laid out");
        assert_eq!(result.layout_input.soft_wrap, wraps);
    }
}

/// A field given less room by its row is laid out narrower.
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldLayoutPhaseToggleTest.kt, constraintsMinWidthDecrease_textLayoutReflects
#[test]
fn a_narrower_field_lays_its_text_out_narrower() {
    let left = cell(100.0f32);
    let heard: Rc<RefCell<Option<TextLayoutResult>>> = Rc::default();
    let into = heard.clone();
    let state = state_of("abc");
    let mut ui = compose_test(move || {
        let into = into.clone();
        Row!(modifier = Modifier.size(200.dp()), |scope| {
            let into = into.clone();
            Box!(Modifier.size(left.get().dp()));
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.weight(scope, 1.0),
                line_limits = TextFieldLineLimits::SingleLine,
                on_text_layout = Some(move |result: TextLayoutResult| {
                    *into.borrow_mut() = Some(result);
                }),
            );
        });
    });
    ui.wait_for_idle();
    let first = heard.borrow().clone().expect("laid out").size.width;
    left.set(150.0);
    ui.wait_for_idle();
    let second = heard.borrow().clone().expect("laid out").size.width;
    let density = ui.density().density;
    assert_eq!(first - second, (50.0 * density).round() as i32);
}

/// A field taller than the column scrolling it, mostly above it, shows its cursor (its first
/// line) when focused, as Compose's `bringSelectionEndIntoView`, not its bottom: both fields
/// (D1141).
#[test]
fn a_tall_field_focused_shows_its_cursor() {
    let text = "line\n".repeat(40);
    for state_based in [false, true] {
        let scroll_state = **Box::leak(Box::new(ScrollState::new(0)));
        let state = state_with(&text, TextRange::ZERO);
        let value = text.clone();
        let mut ui = compose_test(move || {
            let value = value.clone();
            Column!(modifier = Modifier.height(100.dp()).vertical_scroll(scroll_state), {
                Box!(Modifier.height(300.dp()));
                if state_based {
                    BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
                } else {
                    BasicTextField!(value.clone(), |_| {}, modifier = Modifier.test_tag("field"));
                }
                Box!(Modifier.height(300.dp()));
            });
        });
        let field = ui.on_node_with_tag("field").get_unclipped_bounds_in_root();
        let density = ui.density().density;
        let (top, height) = (field.top.0 * density, field.height().0 * density);
        assert!(height > 2.0 * 100.0 * density, "taller than the column: {height}");
        // Its last 50 dp in view, its first line far above.
        scroll_state.dispatch_raw_delta(top + height - 50.0 * density);
        ui.wait_for_idle();
        let before = scroll_state.value();
        ui.on_node_with_tag("field").request_focus();
        ui.wait_for_idle();
        let scrolled = scroll_state.value() as f32;
        assert!(scrolled < before as f32, "state {state_based}");
        assert!(
            scrolled <= top + 1.0 && scrolled + 100.0 * density > top,
            "the first line in view, state {state_based}: {scrolled} for a field at {top}"
        );
    }
}
