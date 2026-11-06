//! A state-based field's output transformation: what it shows of its state, and the cursor over
//! what it inserted, replaced or deleted (D1134).

#![allow(non_snake_case)]

use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Key, KeyEvent, KeyEventType, KeyModifiers, Offset, SemanticsActions as A,
    SemanticsProperties as P, TextRange, TextStyle, UnitExt,
};

/// A focused field over `text`, its selection `selection`, showing it through `transform`.
fn field_showing(
    text: &str,
    selection: TextRange,
    transform: impl Fn(&mut TextFieldBuffer) + 'static,
) -> (ComposeTestRule, TextFieldState) {
    field_with(text, selection, OutputTransformation::from_fn(transform), None)
}

fn field_with(
    text: &str,
    selection: TextRange,
    output: OutputTransformation,
    input: Option<InputTransformation>,
) -> (ComposeTestRule, TextFieldState) {
    let state = **Box::leak(Box::new(TextFieldState::with_selection(text, selection)));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            input_transformation = input.clone(),
            text_style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
            output_transformation = Some(output.clone()),
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    (ui, state)
}

/// What the field shows and the selection it tells, from its semantics.
fn told(ui: &mut ComposeTestRule) -> (String, TextRange) {
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    let text = node.config.get(&P::EDITABLE_TEXT).map(|text| text.to_string()).unwrap_or_default();
    (text, *node.config.get(&P::TEXT_SELECTION_RANGE).expect("a field's selection"))
}

fn cursor_told(ui: &mut ComposeTestRule) -> usize {
    let (_, selection) = told(ui);
    assert!(selection.collapsed(), "a cursor, not {selection:?}");
    selection.start
}

fn press(ui: &mut ComposeTestRule, key: Key, modifiers: KeyModifiers) {
    for kind in [KeyEventType::KeyDown, KeyEventType::KeyUp] {
        ui.host().send_key(KeyEvent { key, kind, modifiers, from_soft_keyboard: false });
    }
    ui.wait_for_idle();
}

/// The word-motion modifier: Option on macOS, Ctrl elsewhere.
fn word() -> KeyModifiers {
    if cfg!(target_os = "macos") {
        KeyModifiers { alt: true, ..Default::default() }
    } else {
        KeyModifiers { ctrl: true, ..Default::default() }
    }
}

/// The state's selection and the one told after each press of `key`.
fn walk(
    ui: &mut ComposeTestRule,
    state: TextFieldState,
    key: Key,
    modifiers: KeyModifiers,
    presses: usize,
) -> (Vec<TextRange>, Vec<usize>) {
    let (mut selections, mut cursors) = (vec![state.selection()], vec![cursor_told(ui)]);
    for _ in 0..presses {
        press(ui, key, modifiers);
        selections.push(state.selection());
        cursors.push(cursor_told(ui));
    }
    (selections, cursors)
}

fn at(offsets: &[usize]) -> Vec<TextRange> {
    offsets.iter().map(|&offset| TextRange::at(offset)).collect()
}

const PLAIN: KeyModifiers = KeyModifiers { shift: false, ctrl: false, alt: false, meta: false };

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, replacement_visualText
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_visualText
#[test]
fn the_field_shows_the_transformed_text_and_the_state_keeps_its_own() {
    let (mut ui, state) = field_showing("abcd", TextRange::at(0), |buffer| {
        buffer.replace(1, 3, "efg");
    });
    assert_eq!(told(&mut ui).0, "aefgd");
    assert_eq!(state.text().as_str(), "abcd");
    let (mut ui, state) = field_showing("ab", TextRange::at(0), |buffer| buffer.insert(1, "efg"));
    assert_eq!(told(&mut ui).0, "aefgb");
    assert_eq!(state.text().as_str(), "ab");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_setTextAction_outputTransformation
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, semantics_setTextAction_inputAndOutputTransformation
#[test]
fn text_set_or_typed_goes_to_the_state_and_is_shown_transformed() {
    let suffix = || OutputTransformation::from_fn(|buffer| buffer.append(" xyz"));
    let (mut ui, state) = field_with("", TextRange::ZERO, suffix(), None);
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_TEXT, |set| {
        set("abc".into());
    });
    assert_eq!(state.text().as_str(), "abc");
    assert_eq!(told(&mut ui), ("abc xyz".to_string(), TextRange::at(3)));
    ui.type_text("d");
    assert_eq!(state.text().as_str(), "abcd");
    assert_eq!(told(&mut ui), ("abcd xyz".to_string(), TextRange::at(4)));
    let caps = InputTransformation::all_caps(compose_ui::Locale::new("en"));
    let (mut ui, state) = field_with("", TextRange::ZERO, suffix(), Some(caps));
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_TEXT, |set| {
        set("abc".into());
    });
    assert_eq!(state.text().as_str(), "ABC");
    assert_eq!(told(&mut ui).0, "ABC xyz");
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    assert_eq!(node.config.get(&P::INPUT_TEXT).map(|text| text.as_str()), Some("ABC"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, replacement_cursorMovement_leftToRight_byCharacter_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, replacement_cursorMovement_rightToLeft_byCharacter_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, replacement_cursorMovement_leftToRight_byCharacter_semanticsOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, replacement_cursorMovement_rightToLeft_byCharacter_semanticsOffsets
#[test]
fn arrows_step_over_a_replacement() {
    let replace = |buffer: &mut TextFieldBuffer| buffer.replace(1, 3, "efg");
    let (mut ui, state) = field_showing("abcd", TextRange::at(0), replace);
    let walked = walk(&mut ui, state, Key::DirectionRight, PLAIN, 3);
    assert_eq!(walked, (at(&[0, 1, 3, 4]), vec![0, 1, 4, 5]));
    let (mut ui, state) = field_showing("abcd", TextRange::at(4), replace);
    let walked = walk(&mut ui, state, Key::DirectionLeft, PLAIN, 3);
    assert_eq!(walked, (at(&[4, 3, 1, 0]), vec![5, 4, 1, 0]));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_leftToRight_byCharacter_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_rightToLeft_byCharacter_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_leftToRight_byCharacter_semanticsOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_rightToLeft_byCharacter_semanticsOffsets
#[test]
fn arrows_stop_on_both_sides_of_an_insertion() {
    let insert = |buffer: &mut TextFieldBuffer| buffer.insert(1, "efg");
    let (mut ui, state) = field_showing("ab", TextRange::at(0), insert);
    let walked = walk(&mut ui, state, Key::DirectionRight, PLAIN, 3);
    assert_eq!(walked, (at(&[0, 1, 1, 2]), vec![0, 1, 4, 5]));
    let (mut ui, state) = field_showing("ab", TextRange::at(2), insert);
    let walked = walk(&mut ui, state, Key::DirectionLeft, PLAIN, 3);
    assert_eq!(walked, (at(&[2, 1, 1, 0]), vec![5, 4, 1, 0]));
}

/// A step across an insertion that leaves the state's cursor where it was still moves the cursor
/// shown, and the frame tells the platform the semantics changed.
#[test]
fn a_step_moving_only_the_cursor_shown_is_told() {
    let (mut ui, state) = field_showing("ab", TextRange::at(1), |buffer| buffer.insert(1, "efg"));
    ui.wait_for_idle();
    assert!(!ui.host().semantics_changed_since_frame());
    for kind in [KeyEventType::KeyDown, KeyEventType::KeyUp] {
        let event = KeyEvent {
            key: Key::DirectionRight,
            kind,
            modifiers: PLAIN,
            from_soft_keyboard: false,
        };
        ui.host().send_key(event);
    }
    assert_eq!(state.selection(), TextRange::at(1));
    assert!(ui.host().semantics_changed_since_frame());
    ui.wait_for_idle();
    assert_eq!(cursor_told(&mut ui), 4);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_leftToRight_byWord_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_leftToRight_byWord_semanticsOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_rightToLeft_byWord_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_rightToLeft_byWord_semanticsOffsets
#[test]
fn words_are_the_shown_texts() {
    let (mut ui, state) =
        field_showing("ab", TextRange::at(0), |buffer| buffer.insert(2, "efg hij"));
    let mut walked = walk(&mut ui, state, Key::DirectionRight, word(), 2);
    press(&mut ui, Key::DirectionLeft, word());
    walked.0.push(state.selection());
    walked.1.push(cursor_told(&mut ui));
    assert_eq!(walked, (at(&[0, 2, 2, 2]), vec![0, 9, 9, 2]));
    let (mut ui, state) =
        field_showing("ab", TextRange::at(2), |buffer| buffer.insert(0, "hij efg"));
    let mut walked = walk(&mut ui, state, Key::DirectionLeft, word(), 2);
    press(&mut ui, Key::DirectionRight, word());
    walked.0.push(state.selection());
    walked.1.push(cursor_told(&mut ui));
    assert_eq!(walked.0, at(&[2, 0, 0, 0]));
    // Compose's test of these offsets reads the state's; shown, the last step goes past all
    // that was inserted, the next word's end being inside it.
    assert_eq!(walked.1, [9, 0, 0, 7]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_leftToRight_byLine_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_leftToRight_byLine_semanticsOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_rightToLeft_byLine_stateOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, insert_cursorMovement_rightToLeft_byLine_semanticsOffsets
#[test]
fn home_and_end_go_to_the_shown_lines_edges() {
    let insert = |buffer: &mut TextFieldBuffer| buffer.insert(4, "xyz\nxyz ");
    let (mut ui, state) = field_showing("abc def", TextRange::at(0), insert);
    let walked = walk(&mut ui, state, Key::MoveEnd, PLAIN, 2);
    assert_eq!(walked, (at(&[0, 4, 7]), vec![0, 12, 15]));
    let (mut ui, state) = field_showing("abc def", TextRange::at(7), insert);
    let walked = walk(&mut ui, state, Key::MoveHome, PLAIN, 2);
    assert_eq!(walked, (at(&[7, 4, 0]), vec![15, 4, 0]));
}

/// Compose's: `>abc def ghi` shown as `>wedge  insertionghi<`, an insertion at each end, a
/// replacement, a deletion and an insertion in the middle.
fn mixed(buffer: &mut TextFieldBuffer) {
    buffer.insert(0, ">");
    buffer.replace(1, 4, "wedge");
    buffer.delete(7, 10);
    buffer.insert(8, "insertion");
    buffer.insert(buffer.length(), "<");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, mixed_cursorMovement_leftToRight_allOffsets
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationHardwareKeysIntegrationTest.kt, mixed_cursorMovement_rightToLeft_allOffsets
#[test]
fn arrows_over_every_kind_of_change_select_what_was_deleted() {
    let (mut ui, state) = field_showing("abc def ghi", TextRange::at(0), mixed);
    assert_eq!(told(&mut ui).0, ">wedge  insertionghi<");
    let (mut selections, mut shown) = (vec![], vec![]);
    for _ in 0..10 {
        selections.push(state.selection());
        shown.push(told(&mut ui).1);
        press(&mut ui, Key::DirectionRight, PLAIN);
    }
    let mut expected = at(&[0, 0, 3]);
    expected.push(TextRange::new(4, 7));
    expected.extend(at(&[8, 8, 9, 10, 11, 11]));
    assert_eq!(selections, expected);
    assert_eq!(shown, at(&[0, 1, 6, 7, 8, 17, 18, 19, 20, 21]));

    let (mut ui, state) = field_showing("abc def ghi", TextRange::at(11), mixed);
    // At the end of what's shown, as Compose's test's tap at the end makes it.
    press(&mut ui, Key::MoveEnd, PLAIN);
    let (mut selections, mut shown) = (vec![], vec![]);
    for _ in 0..10 {
        selections.push(state.selection());
        shown.push(told(&mut ui).1);
        press(&mut ui, Key::DirectionLeft, PLAIN);
    }
    let mut expected = at(&[11, 11, 10, 9, 8, 8]);
    expected.push(TextRange::new(4, 7));
    expected.extend(at(&[3, 0, 0]));
    assert_eq!(selections, expected);
    assert_eq!(shown, at(&[21, 20, 19, 18, 17, 8, 7, 6, 1, 0]));
}

/// Ctrl with Home or End goes to the shown text's ends: past the insertions there.
#[test]
fn the_texts_ends_are_the_shown_texts() {
    let (mut ui, state) = field_showing("abc def ghi", TextRange::at(0), mixed);
    let ctrl = KeyModifiers { ctrl: true, ..PLAIN };
    press(&mut ui, Key::MoveEnd, ctrl);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(11), 21));
    press(&mut ui, Key::MoveHome, ctrl);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(0), 0));
}

/// A selection set through semantics is in what's shown: inside a replacement, the cursor goes
/// to its nearer edge.
#[test]
fn a_selection_set_in_what_is_shown_goes_to_a_replacements_nearer_edge() {
    let (mut ui, state) =
        field_showing("zaz", TextRange::at(0), |buffer| buffer.replace(1, 2, "bbbb"));
    for (shown, expected) in [(4, 2), (2, 1), (5, 2), (6, 3)] {
        ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_SELECTION, |set| {
            set(shown, shown, false);
        });
        assert_eq!(state.selection(), TextRange::at(expected), "shown {shown}");
    }
}

/// A field's character advance with its 10 sp text (half an em in the test measurer).
fn advance(ui: &ComposeTestRule) -> f32 {
    let density = ui.density();
    5.0 * density.density * density.font_scale
}

fn tap(ui: &mut ComposeTestRule, x: f32) {
    let y = advance(ui);
    ui.on_node_with_tag("field").perform_touch_input(|touch| {
        // Apart from any tap before, which would make it a double tap.
        touch.advance_event_time(1000);
        touch.click_at(Offset::new(x, y));
    });
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationGesturesIntegrationTest.kt, clickingAroundReplacement_movesCursorToEdgesOfReplacement
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationGesturesIntegrationTest.kt, clickingAroundReplacement_movesCursorToEdgesOfInsertion
#[test]
fn a_tap_inside_a_replacement_or_an_insertion_goes_to_its_nearer_edge() {
    let (mut ui, state) =
        field_showing("zaz", TextRange::at(0), |buffer| buffer.replace(1, 2, "bbbb"));
    assert_eq!(told(&mut ui).0, "zbbbbz");
    let w = advance(&ui);
    tap(&mut ui, 3.0 * w + 1.0);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(2), 5));
    tap(&mut ui, 3.0 * w - 1.0);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(1), 1));
    let (mut ui, state) = field_showing("zz", TextRange::at(0), |buffer| buffer.insert(1, "bbbb"));
    tap(&mut ui, 3.0 * w + 1.0);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(1), 5));
    tap(&mut ui, 3.0 * w - 1.0);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(1), 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldOutputTransformationGesturesIntegrationTest.kt, clickingAroundReplacement_movesCursorToEdgesOfReplacement_withLineBreak
#[test]
fn the_nearer_edge_is_on_the_screen_not_in_the_text() {
    // "zzzzzzzz" then "bbbb\nbb" for the "a", then "z": the first line's end is nearer the
    // replacement's start on the screen, the second line's start nearer its end.
    let (mut ui, state) = field_showing("zzzzzzzzaz", TextRange::at(0), |buffer| {
        buffer.replace(8, 9, "bbbb\nbb");
    });
    let w = advance(&ui);
    tap(&mut ui, 11.5 * w);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(8), 8));
    let (y, x) = (3.0 * w, 0.5 * w);
    ui.on_node_with_tag("field").perform_touch_input(|touch| {
        touch.advance_event_time(1000);
        touch.click_at(Offset::new(x, y));
    });
    ui.wait_for_idle();
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(9), 15));
}

/// Typing after a step past an insertion puts the cursor before what's inserted at the new
/// cursor, as Compose's wedge affinity goes back to the start after an edit.
#[test]
fn an_edit_puts_the_cursor_shown_back_before_an_insertion() {
    let (mut ui, state) = field_showing("ab", TextRange::at(0), |buffer| {
        if buffer.length() >= 2 {
            buffer.insert(buffer.length() - 1, "-");
        }
    });
    assert_eq!(told(&mut ui), ("a-b".to_string(), TextRange::at(0)));
    press(&mut ui, Key::DirectionRight, PLAIN);
    press(&mut ui, Key::DirectionRight, PLAIN);
    assert_eq!((state.selection(), cursor_told(&mut ui)), (TextRange::at(1), 2));
    ui.type_text("x");
    // "axb" shown "ax-b": the cursor after "x" is before the inserted "-".
    assert_eq!(told(&mut ui), ("ax-b".to_string(), TextRange::at(2)));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, wedgeAffinity_resetsBackToStartAffinity_afterEditUntransformedTextAsUser
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, wedgeAffinity_resetsBackToStartAffinity_afterReplaceAll
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, wedgeAffinity_resetsBackToStartAffinity_afterDeleteSelectedText
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, wedgeAffinity_resetsBackToStartAffinity_afterReplaceText
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, wedgeAffinity_resetsBackToStartAffinity_afterReplaceSelectedText
#[test]
fn every_kind_of_edit_puts_the_cursor_shown_back_before_an_insertion() {
    let after_first = |buffer: &mut TextFieldBuffer| {
        if buffer.length() >= 1 {
            buffer.insert(1, "--");
        }
    };
    // Past the insertion, the cursor shown after it.
    let past = |ui: &mut ComposeTestRule, state: TextFieldState| {
        press(ui, Key::MoveHome, PLAIN);
        press(ui, Key::DirectionRight, PLAIN);
        press(ui, Key::DirectionRight, PLAIN);
        assert_eq!(cursor_told(ui), 3);
        assert_eq!(state.selection(), TextRange::at(1));
    };
    let (mut ui, state) = field_showing("abc", TextRange::at(0), after_first);
    past(&mut ui, state);
    press(&mut ui, Key::Delete, PLAIN);
    assert_eq!(told(&mut ui), ("a--c".to_string(), TextRange::at(1)), "a deletion");
    past(&mut ui, state);
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_TEXT, |set| {
        set("a".into());
    });
    assert_eq!(told(&mut ui), ("a--".to_string(), TextRange::at(1)), "all replaced");
    state.edit(|buffer| buffer.append("bc"));
    ui.wait_for_idle();
    past(&mut ui, state);
    let shift = KeyModifiers { shift: true, ..PLAIN };
    press(&mut ui, Key::DirectionRight, shift);
    press(&mut ui, Key::Backspace, PLAIN);
    assert_eq!(told(&mut ui), ("a--c".to_string(), TextRange::at(1)), "a selection deleted");
    past(&mut ui, state);
    press(&mut ui, Key::DirectionRight, shift);
    ui.type_text("x");
    assert_eq!(told(&mut ui), ("a--x".to_string(), TextRange::at(4)), "a selection typed over");
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, textFieldStateSelection_isTransformed_toPresentedText
#[test]
fn a_selection_is_told_over_what_is_shown() {
    let (mut ui, state) = field_showing("hello", TextRange::at(0), |buffer| buffer.insert(0, "aa"));
    state.edit(|buffer| buffer.set_selection(TextRange::new(0, 2)));
    ui.wait_for_idle();
    assert_eq!(told(&mut ui), ("aahello".to_string(), TextRange::new(0, 4)));
}

/// A single line shows a new line as a space, a carriage return as nothing; the state keeps
/// them.
#[test]
fn a_single_line_shows_new_lines_as_spaces() {
    let state = **Box::leak(Box::new(TextFieldState::new("ab\ncd")));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    });
    assert_eq!(told(&mut ui).0, "ab cd");
    state.edit(|buffer| buffer.replace(0, buffer.length(), "a\rb"));
    ui.wait_for_idle();
    assert_eq!(told(&mut ui).0, "a\u{FEFF}b");
    assert_eq!(state.text().as_str(), "a\rb");
}

/// A new output transformation is shown at once.
#[test]
fn a_new_output_transformation_is_shown() {
    let which = Box::leak(Box::new(mutable_state_of(0usize))).handle();
    let transformations = [
        OutputTransformation::from_fn(|buffer| buffer.append("!")),
        OutputTransformation::from_fn(|buffer| buffer.append("?")),
    ];
    let state = **Box::leak(Box::new(TextFieldState::new("hi")));
    let mut ui = compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            output_transformation = Some(transformations[which.get()].clone()),
        );
    });
    assert_eq!(told(&mut ui).0, "hi!");
    which.set(1);
    ui.wait_for_idle();
    assert_eq!(told(&mut ui).0, "hi?");
}
