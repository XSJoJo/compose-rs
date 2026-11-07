//! Foundation's autofill semantics (D1683, D1684): toggles fill from `OnFillData`
//! (`ToggleableTest`'s autofill tests) and say their value; a field's keyboard type gives its
//! content type (`BasicTextFieldSemanticsTest`'s).

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::*;

/// `perform_semantics_action_with(OnFillData)` with `value`; what the action returned.
fn fill(ui: &mut ComposeTestRule, tag: &str, value: bool) -> bool {
    let handled = Rc::new(Cell::new(true));
    let into = handled.clone();
    let data = FillableData::create_from_boolean(value).unwrap();
    ui.on_node_with_tag(tag)
        .perform_semantics_action_with(&SemanticsActions::ON_FILL_DATA, move |action| {
            into.set(action(data))
        });
    ui.wait_for_idle();
    handled.get()
}

/// Compose's `assertAutofill`: a toggle's data type and fill action, its value if any.
fn assert_autofill(ui: &mut ComposeTestRule, tag: &str, expected: Option<bool>) {
    let node = ui.on_node_with_tag(tag).fetch_semantics_snapshot();
    assert_eq!(
        node.config.get(&SemanticsProperties::CONTENT_DATA_TYPE),
        Some(&ContentDataType::TOGGLE)
    );
    assert!(node.config.contains(&SemanticsActions::ON_FILL_DATA));
    let value = node.config.get(&SemanticsProperties::FILLABLE_DATA).map(|it| it.boolean_value());
    assert_eq!(value, expected.map(Some));
}

/// The toggles' autofill semantics, as `ToggleableTest`'s default semantics tests assert them
/// (their other checks are ported in `selection_focus_hover_parity.rs`).
#[test]
fn toggles_say_their_autofill_value() {
    let mut ui = compose_test(|| {
        compose_foundation::layout::Column!({
            Box!(
                modifier = Modifier.tri_state_toggleable(ToggleableState::On, || {}).test_tag("on")
            );
            Box!(modifier = Modifier.toggleable(false, |_| {}).test_tag("off"));
            Box!(
                modifier = Modifier
                    .tri_state_toggleable(ToggleableState::Indeterminate, || {})
                    .test_tag("indeterminate")
            );
        });
    });
    assert_autofill(&mut ui, "on", Some(true));
    assert_autofill(&mut ui, "off", Some(false));
    assert_autofill(&mut ui, "indeterminate", None);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/selection/ToggleableTest.kt, toggleableTest_autofill_updatesState
/// A toggle filled with true is checked.
#[test]
fn a_toggle_is_filled() {
    let checked = Rc::new(Cell::new(false));
    let into = checked.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        Box!(
            modifier =
                Modifier.toggleable(into.get(), move |it| into.set(it)).test_tag("toggleableNode")
        );
    });
    fill(&mut ui, "toggleableNode", true);
    assert!(checked.get());
}

/// A tri-state toggle starting at `from`, clicked through its states; filled with `value`.
fn tri_state_fill(from: ToggleableState, value: bool) -> (ToggleableState, bool) {
    let state = mutable_state_of(from);
    let handle = state.handle();
    let mut ui = compose_test(move || {
        Box!(
            modifier = Modifier
                .tri_state_toggleable(handle.get(), move || {
                    handle.set(match handle.get() {
                        ToggleableState::Off => ToggleableState::On,
                        ToggleableState::On => ToggleableState::Off,
                        ToggleableState::Indeterminate => ToggleableState::On,
                    })
                })
                .test_tag("triStateToggleableNode")
        );
    });
    let handled = fill(&mut ui, "triStateToggleableNode", value);
    (state.get(), handled)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/selection/ToggleableTest.kt, triStateToggleable_autofill_updatesState
#[test]
fn a_tri_state_toggle_is_filled_on() {
    assert_eq!(tri_state_fill(ToggleableState::Off, true).0, ToggleableState::On);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/selection/ToggleableTest.kt, triStateToggleable_autofill_updatesState_toOff
#[test]
fn a_tri_state_toggle_is_filled_off() {
    assert_eq!(tri_state_fill(ToggleableState::On, false).0, ToggleableState::Off);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/selection/ToggleableTest.kt, triStateToggleable_autofill_indeterminate_ignored
/// An indeterminate tri-state toggle refuses a value, unclicked.
#[test]
fn an_indeterminate_toggle_refuses_a_value() {
    let clicked = Rc::new(Cell::new(false));
    let into = clicked.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        Box!(
            modifier = Modifier
                .tri_state_toggleable(ToggleableState::Indeterminate, move || into.set(true))
                .test_tag("triStateToggleableNode")
        );
    });
    let handled = fill(&mut ui, "triStateToggleableNode", true);
    assert!(!handled);
    assert!(!clicked.get());
}

/// A state-based field with `keyboard_type`; its content type.
fn field_content_type(keyboard_type: KeyboardType) -> Option<ContentType> {
    let mut ui = compose_test(move || {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("tag"),
            keyboard_options = KeyboardOptions { keyboard_type, ..KeyboardOptions::DEFAULT }
        );
    });
    let node = ui.on_node_with_tag("tag").fetch_semantics_snapshot();
    node.config.get(&SemanticsProperties::CONTENT_TYPE).cloned()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, textField_setsEmailContentType_whenKeyboardTypeIsEmail
#[test]
fn an_email_keyboard_gives_an_email_s_content_type() {
    assert_eq!(field_content_type(KeyboardType::Email), Some(ContentType::EMAIL_ADDRESS));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, textField_setsPasswordContentType_whenKeyboardTypeIsNumberPassword
#[test]
fn a_number_password_keyboard_gives_a_password_s_content_type() {
    assert_eq!(field_content_type(KeyboardType::NumberPassword), Some(ContentType::PASSWORD));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, textField_setsPasswordContentType_whenKeyboardTypeIsPassword
#[test]
fn a_password_keyboard_gives_a_password_s_content_type() {
    assert_eq!(field_content_type(KeyboardType::Password), Some(ContentType::PASSWORD));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicTextFieldSemanticsTest.kt, textField_setsPhoneContentType_whenKeyboardTypeIsPhone
#[test]
fn a_phone_keyboard_gives_a_phone_number_s_content_type() {
    assert_eq!(field_content_type(KeyboardType::Phone), Some(ContentType::PHONE_NUMBER));
}

/// compose-rs's own: other keyboards give none.
#[test]
fn a_text_keyboard_gives_no_content_type() {
    assert_eq!(field_content_type(KeyboardType::Text), None);
}

/// A field is text to autofill and fills, as Compose's `defaultSemantics` asserts it, its value
/// its text; a secure field's content type is a password's.
#[test]
fn a_field_s_autofill_semantics() {
    let mut ui = compose_test(|| {
        compose_foundation::layout::Column!({
            let state = remember_text_field_state("abc", None);
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field"));
            let secret = remember_text_field_state("pw", None);
            BasicSecureTextField!(secret, modifier = Modifier.test_tag("secure"));
        });
    });
    let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
    assert_eq!(
        node.config.get(&SemanticsProperties::CONTENT_DATA_TYPE),
        Some(&ContentDataType::TEXT)
    );
    assert!(node.config.contains(&SemanticsActions::ON_FILL_DATA));
    assert_eq!(
        node.config.get(&SemanticsProperties::FILLABLE_DATA),
        FillableData::create_from_text("abc").as_ref()
    );
    let secure = ui.on_node_with_tag("secure").fetch_semantics_snapshot();
    assert_eq!(secure.config.get(&SemanticsProperties::CONTENT_TYPE), Some(&ContentType::PASSWORD));
    // A password's own text, not its mask.
    assert_eq!(
        secure.config.get(&SemanticsProperties::FILLABLE_DATA),
        FillableData::create_from_text("pw").as_ref()
    );
}
