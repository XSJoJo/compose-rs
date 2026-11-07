//! The virtual structure an autofill service reads, and filling from it, as Compose's
//! `PerformAndroidAutofillManagerTest`: `UiTree::populate_autofill_structure` gives what Compose
//! writes into Android's `ViewStructure` (its tests' `FakeViewStructure`), and
//! `UiHost::perform_autofill` what the platform fills with (D1680). Compose's tests marked
//! `@Ignore` upstream (visibility, b/383198004) aren't ported; its `AndroidView` one is n/a
//! (no views in compose-rs content, D226).

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{
    HOST_VIEW_ID, INPUT_TYPE_TEXT_PASSWORD, TEXT_FIELD_CLASS_NAME, VIEW_VISIBLE, ViewStructure,
};
use compose_ui::*;

const SIDE: f32 = 200.0;
const CONTENT_TAG: &str = "content_tag";
const PACKAGE: &str = "rs.compose.test";

const AUTOFILL_TYPE_TEXT: i32 = 1;
const AUTOFILL_TYPE_TOGGLE: i32 = 2;
const AUTOFILL_TYPE_DATE: i32 = 4;

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let ui = ComposeTestRule::empty(default_host());
    ui.autofill();
    let mut ui = ui;
    ui.set_content(content);
    ui
}

/// The structure the platform would be given now.
fn structure(ui: &mut ComposeTestRule) -> ViewStructure {
    ui.run_on_idle(|ui| ui.host().with_tree(|tree| tree.populate_autofill_structure(Some(PACKAGE))))
}

fn content_id(ui: &mut ComposeTestRule) -> i32 {
    ui.on_node_with_tag(CONTENT_TAG).fetch_semantics_snapshot().semantics_id
}

fn square(ui: &ComposeTestRule) -> IntRect {
    let side = (SIDE * ui.density().density).round() as i32;
    IntRect { left: 0, top: 0, right: side, bottom: side }
}

/// A structure with the defaults Compose's tests' `ViewStructure(view)` sets: the square's
/// bounds, enabled, the package, sensitive. The root's bounds are the window's (the rule's root
/// fills it).
fn expected(ui: &ComposeTestRule, block: impl FnOnce(&mut ViewStructure)) -> ViewStructure {
    let mut structure = ViewStructure {
        bounds: Some(square(ui)),
        is_enabled: true,
        package_name: Some(PACKAGE.to_string()),
        data_is_sensitive: true,
        ..ViewStructure::default()
    };
    block(&mut structure);
    structure
}

/// The root's structure with the content's as its only child.
fn root_with(ui: &mut ComposeTestRule, child: impl FnOnce(&mut ViewStructure)) -> ViewStructure {
    let id = content_id(ui);
    let child = expected(ui, |structure| {
        structure.virtual_id = id;
        child(structure);
    });
    expected(ui, |root| {
        root.virtual_id = HOST_VIEW_ID;
        root.children.push(child);
        // The root fills the rule's window, where Compose's test view wraps its content.
        let size = compose_test::DEFAULT_SIZE;
        root.bounds = Some(IntRect { left: 0, top: 0, right: size.width, bottom: size.height });
    })
}

fn username_hints() -> Vec<String> {
    vec!["username".to_string()]
}

/// A square with `semantics` and a username's content type, tagged, with `more` after it.
fn username_box(
    semantics: impl Fn(&mut SemanticsConfiguration) + 'static,
    more: impl Fn(Modifier) -> Modifier + 'static,
) -> impl Fn() + 'static {
    let semantics = Rc::new(semantics);
    move || {
        let semantics = semantics.clone();
        let modifier = Modifier.semantics(move |config| {
            config.set_content_type(ContentType::USERNAME);
            semantics(config);
        });
        Box!(modifier = more(modifier.size(SIDE.dp())).test_tag(CONTENT_TAG));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, autofillModifier_contentType
/// `Modifier.content_type` sets the semantics' content type.
#[test]
fn the_content_type_modifier_sets_the_semantics() {
    let mut ui = compose_test(|| {
        Box!(modifier = Modifier.test_tag("TestTag").content_type(ContentType::NEW_USERNAME));
    });
    ui.on_node_with_tag("TestTag").assert(compose_test::expect_value(
        &SemanticsProperties::CONTENT_TYPE,
        ContentType::NEW_USERNAME,
    ));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_empty
/// Nothing for autofill: the root has no children.
#[test]
fn no_content_has_no_children() {
    let mut ui = rule(|| {});
    assert!(structure(&mut ui).children.is_empty());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_defaultValues
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_contentType
/// A node with a content type: its hints, its bounds, enabled, visible, sensitive, under the
/// root (the host's id, the root's bounds). (Compose's two tests check the same.)
#[test]
fn a_content_type_s_node() {
    let mut ui = rule(username_box(|_| {}, |modifier| modifier));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.is_focusable = false;
        child.is_focused = false;
        child.is_long_clickable = false;
        child.visibility = VIEW_VISIBLE;
    });
    assert_eq!(structure(&mut ui), expected);
}

/// A tagged square with `semantics` only.
fn plain_box(semantics: impl Fn(&mut SemanticsConfiguration) + 'static) -> impl Fn() + 'static {
    let semantics = Rc::new(semantics);
    move || {
        let semantics = semantics.clone();
        Box!(
            modifier = Modifier
                .semantics(move |config| semantics(config))
                .size(SIDE.dp())
                .test_tag(CONTENT_TAG)
        );
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_contentDataType
#[test]
fn a_text_data_type_s_node() {
    let mut ui = rule(plain_box(|config| config.set_content_data_type(ContentDataType::TEXT)));
    let expected = root_with(&mut ui, |child| child.autofill_type = AUTOFILL_TYPE_TEXT);
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_contentDataType_toggle
#[test]
fn a_toggle_data_type_s_node() {
    let mut ui = rule(plain_box(|config| config.set_content_data_type(ContentDataType::TOGGLE)));
    let expected = root_with(&mut ui, |child| child.autofill_type = AUTOFILL_TYPE_TOGGLE);
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_contentDataType_date
#[test]
fn a_date_data_type_s_node() {
    let mut ui = rule(plain_box(|config| config.set_content_data_type(ContentDataType::DATE)));
    let expected = root_with(&mut ui, |child| child.autofill_type = AUTOFILL_TYPE_DATE);
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_clickable
/// A clickable node: clickable and focusable.
#[test]
fn a_clickable_node() {
    let mut ui = rule(username_box(|_| {}, |modifier| modifier.clickable(|| {})));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.is_clickable = true;
        child.is_focusable = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_contentDescription
#[test]
fn a_described_node() {
    let mut ui = rule(username_box(|config| config.set_content_description(CONTENT_TAG), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.content_description = Some(CONTENT_TAG.to_string());
    });
    assert_eq!(structure(&mut ui), expected);
}

/// A selected selectable of `role`, without an indication.
fn selectable_of(role: Role) -> impl Fn(Modifier) -> Modifier + 'static {
    move |modifier| {
        modifier.selectable_with(
            true,
            ClickableOptions {
                enabled: true,
                role: Some(role),
                indication: Indication::NONE,
                ..ClickableOptions::DEFAULT
            },
            || {},
        )
    }
}

/// A selected selectable of `role`'s structure.
fn selectable_structure(role: Role, class_name: Option<&str>, checkable: bool) {
    let mut ui = rule(username_box(|_| {}, selectable_of(role)));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.class_name = class_name.map(str::to_string);
        child.is_checkable = checkable;
        child.is_checked = checkable;
        child.is_clickable = true;
        child.is_focusable = true;
        child.is_selected = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_role_tab
/// A selected tab: selected, not checkable.
#[test]
fn a_selected_tab() {
    selectable_structure(Role::Tab, None, false);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_role_radioButton
/// A selected radio button: checked, with Android's class.
#[test]
fn a_selected_radio_button() {
    selectable_structure(Role::RadioButton, Some("android.widget.RadioButton"), true);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_role_dropdownList
#[test]
fn a_selected_dropdown_list() {
    selectable_structure(Role::DropdownList, Some("android.widget.Spinner"), true);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_role_valuePicker
#[test]
fn a_selected_value_picker() {
    selectable_structure(Role::ValuePicker, Some("android.widget.NumberPicker"), true);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_hideFromAccessibility
/// Hidden from accessibility, still visible to autofill.
#[test]
fn hidden_from_accessibility_is_visible() {
    let mut ui = rule(username_box(|config| config.hide_from_accessibility(), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.visibility = VIEW_VISIBLE;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_longClickable
#[test]
fn a_long_clickable_node() {
    let mut ui = rule(username_box(|config| config.on_long_click(None, || true), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.is_long_clickable = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_focusable
#[test]
fn a_focusable_node() {
    let mut ui = rule(username_box(|config| config.request_focus(None, || true), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.is_focusable = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_focusable_focused
/// A focused node: focusable and focused.
#[test]
fn a_focused_node() {
    let mut ui = rule(move || {
        Box!(
            modifier = Modifier
                .semantics(|config| config.set_content_type(ContentType::USERNAME))
                .focusable()
                .size(SIDE.dp())
                .test_tag(CONTENT_TAG)
        );
    });
    ui.on_node_with_tag(CONTENT_TAG).request_focus();
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.is_focusable = true;
        child.is_focused = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_enabled
#[test]
fn an_enabled_node() {
    let mut ui = rule(username_box(|_| {}, |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.is_enabled = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_disabled
#[test]
fn a_disabled_node() {
    let mut ui = rule(username_box(|config| config.disabled(), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.is_enabled = false;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_setMaxLength
/// A node setting text: a text field's class, of text, with its maximum length.
#[test]
fn a_text_field_s_maximum_length() {
    let mut ui = rule(username_box(
        |config| {
            config.set_text_action(None, |_| true);
            config.set_max_text_length(5);
        },
        |m| m,
    ));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.autofill_type = AUTOFILL_TYPE_TEXT;
        child.class_name = Some(TEXT_FIELD_CLASS_NAME.to_string());
        child.max_text_length = 5;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_setMaxLength_notSetForNonTextItems
#[test]
fn no_maximum_length_but_for_text_fields() {
    let mut ui = rule(username_box(|config| config.set_max_text_length(5), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.max_text_length = -1;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_checkable_unchecked
#[test]
fn an_unchecked_toggle_state() {
    let mut ui =
        rule(username_box(|config| config.set_toggleable_state(ToggleableState::Off), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.autofill_type = AUTOFILL_TYPE_TOGGLE;
        child.is_checkable = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_checkable_checked
#[test]
fn a_checked_toggle_state() {
    let mut ui =
        rule(username_box(|config| config.set_toggleable_state(ToggleableState::On), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.autofill_type = AUTOFILL_TYPE_TOGGLE;
        child.is_checkable = true;
        child.is_checked = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_checkable
/// A toggleable: its value as autofill's, checkable, checked, clickable, focusable.
#[test]
fn a_toggleable() {
    let mut ui = rule(move || {
        Box!(
            modifier = Modifier
                .semantics(|config| config.set_content_type(ContentType::USERNAME))
                .toggleable(true, |_| {})
                .size(SIDE.dp())
                .test_tag(CONTENT_TAG)
        );
    });
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.autofill_type = AUTOFILL_TYPE_TOGGLE;
        child.autofill_value = FillableData::create_from_boolean(true);
        child.is_checkable = true;
        child.is_checked = true;
        child.is_clickable = true;
        child.is_focusable = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_password_asSemanticProperty
/// A password's node is sensitive.
#[test]
fn a_password_is_sensitive() {
    let mut ui = rule(username_box(|config| config.password(), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.data_is_sensitive = true;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_autofillValueOverwrites_editableText
/// The fillable data is the value, over the editable text.
#[test]
fn fillable_data_wins_over_the_editable_text() {
    let mut ui = rule(username_box(
        |config| {
            config.set_fillable_data(FillableData::create_from_text("fillableData-test").unwrap());
            config.set_editable_text("editableText-test");
        },
        |m| m,
    ));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.autofill_value = FillableData::create_from_text("fillableData-test");
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_sensitiveSemanticProperty
/// Marked not sensitive, and no password: not sensitive.
#[test]
fn a_node_marked_not_sensitive() {
    let mut ui = rule(username_box(|config| config.set_is_sensitive_data(false), |m| m));
    let expected = root_with(&mut ui, |child| {
        child.autofill_hints = username_hints();
        child.data_is_sensitive = false;
    });
    assert_eq!(structure(&mut ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, performAutofill_toggle
/// The platform's toggle value fills a toggleable.
#[test]
fn filling_a_toggleable() {
    let checked = mutable_state_of(true);
    let handle = checked.handle();
    let mut ui = rule(move || {
        Box!(modifier = Modifier.toggleable(handle.get(), move |it| handle.set(it)).test_tag("t"));
    });
    let id = ui.on_node_with_tag("t").fetch_semantics_snapshot().semantics_id;
    let value = FillableData::create_from_boolean(false).unwrap();
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(id, value)]));
    ui.on_node_with_tag("t").assert(compose_test::expect_value(
        &SemanticsProperties::TOGGLEABLE_STATE,
        ToggleableState::Off,
    ));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, performAutofill_customToggle
/// A custom toggle's `OnFillData` takes the platform's value.
#[test]
fn filling_a_custom_toggle() {
    let checked = mutable_state_of(true);
    let handle = checked.handle();
    let mut ui = rule(move || {
        Box!(
            modifier = Modifier
                .test_tag("toggle_id")
                .clickable(move || handle.set(!handle.get()))
                .semantics(move |config| {
                    if let Some(data) = FillableData::create_from_boolean(handle.get()) {
                        config.set_fillable_data(data);
                    }
                    config.on_fill_data(Some("CustomToggle".into()), move |data| {
                        if let Some(value) = data.boolean_value() {
                            handle.set(value);
                        }
                        true
                    });
                })
        );
    });
    ui.run_on_idle(|_| assert!(checked.get()));
    let id = ui.on_node_with_tag("toggle_id").fetch_semantics_snapshot().semantics_id;
    let value = FillableData::create_from_boolean(false).unwrap();
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(id, value)]));
    ui.run_on_idle(|_| assert!(!checked.get()));
}

// ---- Text fields -------------------------------------------------------------------------------

/// A state-based field of `text` in a column, a square tagged, with `content_type` from the
/// application and `keyboard_type`.
fn field_rule(
    text: &'static str,
    content_type: Option<ContentType>,
    keyboard_type: KeyboardType,
) -> ComposeTestRule {
    rule(move || {
        let content_type = content_type.clone();
        Column!({
            let state = remember_text_field_state(text, None);
            let semantics = match content_type.clone() {
                Some(content_type) => {
                    Modifier.semantics(move |config| config.set_content_type(content_type))
                }
                None => Modifier,
            };
            BasicTextFieldOfState!(
                state,
                modifier = semantics.size(SIDE.dp()).test_tag(CONTENT_TAG),
                keyboard_options = KeyboardOptions { keyboard_type, ..KeyboardOptions::DEFAULT }
            );
        });
    })
}

/// A field's structure: text to autofill with `value`, of `hint`, a text field's class, clicks,
/// focus, visible; a password's input type if `password`.
fn field_structure(ui: &mut ComposeTestRule, hint: &str, value: &str, password: bool) {
    let expected = root_with(ui, |child| {
        child.autofill_hints = vec![hint.to_string()];
        child.autofill_type = AUTOFILL_TYPE_TEXT;
        child.autofill_value = FillableData::create_from_text(value.to_string());
        child.class_name = Some(TEXT_FIELD_CLASS_NAME.to_string());
        child.is_clickable = true;
        child.is_focusable = true;
        child.is_long_clickable = true;
        child.text = String::new();
        child.visibility = VIEW_VISIBLE;
        if password {
            child.input_type = INPUT_TYPE_TEXT_PASSWORD;
        }
    });
    assert_eq!(structure(ui), expected);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_username_empty
/// An empty field with a username's content type: text to autofill, empty, a text field.
#[test]
fn an_empty_username_field() {
    let mut ui = field_rule("", Some(ContentType::USERNAME), KeyboardType::Text);
    field_structure(&mut ui, "username", "", false);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_username_specified
#[test]
fn a_username_field_s_value() {
    let mut ui = field_rule("testUsername", Some(ContentType::USERNAME), KeyboardType::Text);
    field_structure(&mut ui, "username", "testUsername", false);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_password_empty
/// A password's content type: a password's input type.
#[test]
fn an_empty_password_field() {
    let mut ui = field_rule("", Some(ContentType::PASSWORD), KeyboardType::Text);
    field_structure(&mut ui, "password", "", true);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_password_asContentType
#[test]
fn a_password_field_by_its_content_type() {
    let mut ui = field_rule("testPassword", Some(ContentType::PASSWORD), KeyboardType::Text);
    field_structure(&mut ui, "password", "testPassword", true);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_email_asKeyboardType
/// An email's keyboard: an email address to autofill.
#[test]
fn an_email_field_by_its_keyboard() {
    let mut ui = field_rule("test@email.com", None, KeyboardType::Email);
    field_structure(&mut ui, "emailAddress", "test@email.com", false);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_password_asKeyboardType
#[test]
fn a_password_field_by_its_keyboard() {
    let mut ui = field_rule("testPassword", None, KeyboardType::Password);
    field_structure(&mut ui, "password", "testPassword", true);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_numberPassword_asKeyboardType
#[test]
fn a_number_password_field_by_its_keyboard() {
    let mut ui = field_rule("123456", None, KeyboardType::NumberPassword);
    field_structure(&mut ui, "password", "123456", true);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, populateViewStructure_phone_asKeyboardType
#[test]
fn a_phone_field_by_its_keyboard() {
    let mut ui = field_rule("1234567890", None, KeyboardType::Phone);
    field_structure(&mut ui, "phoneNumber", "1234567890", false);
}

/// compose-rs's own: the application's content type wins over its keyboard's, as Compose's
/// (the application's modifier is outside the field's).
#[test]
fn the_application_s_content_type_wins_over_the_keyboard_s() {
    let mut ui = field_rule("a@b.c", Some(ContentType::USERNAME), KeyboardType::Email);
    field_structure(&mut ui, "username", "a@b.c", false);
}

/// Two state-based fields of `first` and `second`'s content types, tagged; the platform fills
/// them with `values`, then each shows its value.
fn fill_two_fields(first: (ContentType, &str), second: (ContentType, &str)) {
    let ((first_type, first_value), (second_type, second_value)) = (first, second);
    let types = (first_type.clone(), second_type.clone());
    let mut ui = rule(move || {
        let types = types.clone();
        Column!({
            for (content_type, tag) in [(types.0.clone(), "first"), (types.1.clone(), "second")] {
                let state = remember_text_field_state("", None);
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier
                        .semantics(move |config| config.set_content_type(content_type.clone()))
                        .test_tag(tag)
                );
            }
        });
    });
    let id = |ui: &mut ComposeTestRule, tag: &str| {
        ui.on_node_with_tag(tag).fetch_semantics_snapshot().semantics_id
    };
    let (first, second) = (id(&mut ui, "first"), id(&mut ui, "second"));
    let values = vec![
        (first, FillableData::from_platform_text(first_value.to_string())),
        (second, FillableData::from_platform_text(second_value.to_string())),
    ];
    ui.run_on_idle(|ui| ui.host().perform_autofill(values));
    ui.on_node_with_tag("first").assert_text_equals(&[first_value]);
    ui.on_node_with_tag("second").assert_text_equals(&[second_value]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, performAutofill_credentials_customBTF
/// Credentials fill a username field and a password field.
#[test]
fn filling_credentials() {
    fill_two_fields(
        (ContentType::USERNAME, "testUsername"),
        (ContentType::PASSWORD, "testPassword"),
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/PerformAndroidAutofillManagerTest.kt, performAutofill_payment_customBTF
/// A card's number and security code fill their fields.
#[test]
fn filling_a_payment() {
    fill_two_fields(
        (ContentType::CREDIT_CARD_NUMBER, "0123 4567 8910 1112"),
        (ContentType::CREDIT_CARD_SECURITY_CODE, "123"),
    );
}

// ---- The semantics -----------------------------------------------------------------------------

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/semantics/SemanticsTests.kt, contentTypeProperty
/// A content type shows in both trees.
#[test]
fn the_content_type_property() {
    let mut ui = compose_test(|| {
        Box!(
            modifier = Modifier
                .test_tag("tag")
                .semantics(|config| config.set_content_type(ContentType::USERNAME))
        );
    });
    let matcher =
        || compose_test::expect_value(&SemanticsProperties::CONTENT_TYPE, ContentType::USERNAME);
    ui.on_node_with_tag_unmerged("tag").assert(matcher());
    ui.on_node_with_tag("tag").assert(matcher());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/semantics/SemanticsTests.kt, contentDataTypeProperty
#[test]
fn the_content_data_type_property() {
    let mut ui = compose_test(|| {
        Box!(
            modifier = Modifier
                .test_tag("tag")
                .semantics(|config| config.set_content_data_type(ContentDataType::TEXT))
        );
    });
    let matcher = || {
        compose_test::expect_value(&SemanticsProperties::CONTENT_DATA_TYPE, ContentDataType::TEXT)
    };
    ui.on_node_with_tag_unmerged("tag").assert(matcher());
    ui.on_node_with_tag("tag").assert(matcher());
}

// ---- compose-rs's own --------------------------------------------------------------------------

/// Nodes related to autofill nest under the nearest such ancestor, in Compose's order: a related
/// node's own children first, then those found under unrelated siblings, last first (its
/// traversal's stack).
#[test]
fn related_nodes_nest_in_compose_s_order() {
    let mut ui = rule(|| {
        Column!({
            Box!(modifier = Modifier.size(10.dp()), {
                Box!(
                    modifier =
                        Modifier.size(10.dp()).content_type(ContentType::USERNAME).test_tag("a")
                );
            });
            Box!(
                modifier = Modifier.size(10.dp()).content_type(ContentType::PASSWORD).test_tag("b"),
                {
                    Box!(
                        modifier =
                            Modifier.size(5.dp()).content_type(ContentType::GENDER).test_tag("c")
                    );
                }
            );
        });
    });
    let id = |ui: &mut ComposeTestRule, tag: &str| {
        ui.on_node_with_tag(tag).fetch_semantics_snapshot().semantics_id
    };
    let (a, b, c) = (id(&mut ui, "a"), id(&mut ui, "b"), id(&mut ui, "c"));
    let root = structure(&mut ui);
    let ids: Vec<i32> = root.children.iter().map(|child| child.virtual_id).collect();
    assert_eq!(ids, [b, a]);
    let under_b: Vec<i32> = root.children[0].children.iter().map(|it| it.virtual_id).collect();
    assert_eq!(under_b, [c]);
}
