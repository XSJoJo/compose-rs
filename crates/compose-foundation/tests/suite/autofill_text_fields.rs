//! Text fields and autofill (D1684, D1685): filled by the platform's values (Compose's
//! `TextFieldStateSemanticAutofillTest` and `TextFieldsSemanticAutofillTest`'s foundation field),
//! highlighted once filled, their toolbar's "Autofill" asking for the service, their values told
//! to the platform as they change. Material's fields are in compose-material3's suite.

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::PlatformAutofillCall as Call;
use compose_ui::*;

fn text(value: &str) -> FillableData {
    FillableData::from_platform_text(value.to_string())
}

fn id(ui: &mut ComposeTestRule, tag: &str) -> i32 {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().semantics_id
}

/// A rule with the recording platform manager.
fn rule(content: impl Fn() + 'static) -> (ComposeTestRule, Rc<TestAutofillManager>) {
    let mut ui = ComposeTestRule::empty(default_host());
    let am = ui.autofill();
    ui.set_content(content);
    (ui, am)
}

fn username(config: &mut SemanticsConfiguration) {
    config.set_content_type(ContentType::USERNAME);
}

fn password(config: &mut SemanticsConfiguration) {
    config.set_content_type(ContentType::PASSWORD);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/TextFieldStateSemanticAutofillTest.kt, performAutofill_credentials_BTF
/// A username and a password filled into state-based fields.
#[test]
fn credentials_fill_state_based_fields() {
    let (mut ui, _am) = rule(|| {
        Column!({
            let first = remember_text_field_state("", None);
            BasicTextFieldOfState!(
                first,
                modifier = Modifier.test_tag("username_tag").semantics(username)
            );
            let second = remember_text_field_state("", None);
            BasicTextFieldOfState!(
                second,
                modifier = Modifier.test_tag("password_tag").semantics(password)
            );
        });
    });
    let (user, pass) = (id(&mut ui, "username_tag"), id(&mut ui, "password_tag"));
    ui.run_on_idle(|ui| {
        ui.host().perform_autofill(vec![(user, text("testUsername")), (pass, text("testPassword"))])
    });
    ui.on_node_with_tag("username_tag").assert_text_equals(&["testUsername"]);
    ui.on_node_with_tag("password_tag").assert_text_equals(&["testPassword"]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/TextFieldStateSemanticAutofillTest.kt, performAutofill_credentials_BSTF
/// A secure field is filled too.
#[test]
fn credentials_fill_a_secure_field() {
    let (mut ui, _am) = rule(|| {
        Column!({
            let state = remember_text_field_state("", None);
            BasicSecureTextField!(
                state,
                modifier = Modifier.test_tag("username_tag").semantics(username)
            );
        });
    });
    let user = id(&mut ui, "username_tag");
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(user, text("testUsername"))]));
    ui.on_node_with_tag("username_tag").assert_text_equals(&["testUsername"]);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/TextFieldsSemanticAutofillTest.kt, performAutofill_credentials_BTF
/// Value-based fields are filled through their callbacks.
#[test]
fn credentials_fill_value_based_fields() {
    let user_input = mutable_state_of(String::new());
    let pass_input = mutable_state_of(String::new());
    let (user_state, pass_state) = (user_input.handle(), pass_input.handle());
    let (mut ui, _am) = rule(move || {
        Column!({
            BasicTextField!(
                user_state.get(),
                move |it: String| user_state.set(it),
                modifier = Modifier.test_tag("username_tag").semantics(username)
            );
            BasicTextField!(
                pass_state.get(),
                move |it: String| pass_state.set(it),
                modifier = Modifier.test_tag("password_tag").semantics(password)
            );
        });
    });
    let (user, pass) = (id(&mut ui, "username_tag"), id(&mut ui, "password_tag"));
    ui.run_on_idle(|ui| {
        ui.host().perform_autofill(vec![(user, text("testUsername")), (pass, text("testPassword"))])
    });
    ui.on_node_with_tag("username_tag").assert_text_equals(&["testUsername"]);
    ui.on_node_with_tag("password_tag").assert_text_equals(&["testPassword"]);
}

// ---- compose-rs's own --------------------------------------------------------------------------

/// The fills the display list draws in `color`.
fn rects_of(ui: &mut ComposeTestRule, color: Color) -> usize {
    ui.host()
        .display_list()
        .ops
        .iter()
        .filter(|op| matches!(op, compose_ui::platform::DrawOp::DrawRect(_, paint) if paint.color == color))
        .count()
}

/// A filled field is highlighted (`LOCAL_AUTOFILL_HIGHLIGHT_BRUSH`'s default) until its text
/// changes, as Compose's decorator.
#[test]
fn a_filled_field_is_highlighted_until_its_text_changes() {
    let (mut ui, _am) = rule(|| {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field").semantics(username));
    });
    ui.wait_for_idle();
    assert_eq!(rects_of(&mut ui, DEFAULT_AUTOFILL_HIGHLIGHT_COLOR), 0);
    let field = id(&mut ui, "field");
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(field, text("name"))]));
    ui.wait_for_idle();
    assert_eq!(rects_of(&mut ui, DEFAULT_AUTOFILL_HIGHLIGHT_COLOR), 1);
    ui.on_node_with_tag("field").perform_text_input("s");
    ui.wait_for_idle();
    assert_eq!(rects_of(&mut ui, DEFAULT_AUTOFILL_HIGHLIGHT_COLOR), 0);
}

/// The highlight's brush is the local's where the field is.
#[test]
fn the_highlight_is_the_local_s() {
    let (mut ui, _am) = rule(|| {
        CompositionLocalProvider(
            LOCAL_AUTOFILL_HIGHLIGHT_BRUSH.provides(Brush::solid(Color::RED)),
            || {
                let state = remember_text_field_state("", None);
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.test_tag("field").semantics(username)
                );
            },
        );
    });
    let field = id(&mut ui, "field");
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(field, text("name"))]));
    ui.wait_for_idle();
    assert_eq!(rects_of(&mut ui, Color::RED), 1);
    assert_eq!(rects_of(&mut ui, DEFAULT_AUTOFILL_HIGHLIGHT_COLOR), 0);
}

/// A read-only state-based field refuses the value, as Compose's decorator.
#[test]
fn a_read_only_field_isn_t_filled() {
    let (mut ui, _am) = rule(|| {
        let state = remember_text_field_state("kept", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("field").semantics(username),
            read_only = true
        );
    });
    let field = id(&mut ui, "field");
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(field, text("name"))]));
    ui.on_node_with_tag("field").assert_text_equals(&["kept"]);
    assert_eq!(rects_of(&mut ui, DEFAULT_AUTOFILL_HIGHLIGHT_COLOR), 0);
}

/// Typing in a field with a content type tells the platform its value, as Compose's manager hears
/// the field's input text change.
#[test]
fn typing_tells_the_value() {
    let (mut ui, am) = rule(|| {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("field").semantics(username));
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    am.clear();
    ui.on_node_with_tag("field").perform_text_input("a");
    ui.wait_for_idle();
    let field = id(&mut ui, "field");
    // Twice, as Compose's: its input text changed, and its fillable data.
    am.verify_times(&Call::NotifyValueChanged { semantics_id: field, value: text("a") }, 2);
    am.assert_no_more_calls();
}

/// Focusing a field enters it for the platform, with the field's bounds, as Compose's manager on
/// the focus's move (a field fills: it's autofillable).
#[test]
fn focusing_a_field_enters_it() {
    let (mut ui, am) = rule(|| {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.size(100.dp()).test_tag("field").semantics(username)
        );
    });
    am.clear();
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    let field = id(&mut ui, "field");
    let side = (100.0 * ui.density().density) as i32;
    let bounds = IntRect { left: 0, top: 0, right: side, bottom: side };
    am.verify(&Call::NotifyViewEntered { semantics_id: field, bounds });
}

/// The toolbar's "Autofill" asks the platform for the field, with its id and bounds, and closes
/// the toolbar.
#[test]
fn the_toolbar_s_autofill_asks_for_the_service() {
    let mut ui = ComposeTestRule::empty(default_host());
    let am = ui.autofill();
    // A finger's toolbar, as the toolbar tests'.
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_content(|| {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.size(100.dp()).test_tag("field").semantics(username)
        );
    });
    long_press(&mut ui);
    ui.wait_for_idle();
    let toolbar = ui.text_toolbar();
    assert!(toolbar.titles().iter().any(|title| title == "Autofill"));
    am.clear();
    toolbar.click("Autofill");
    ui.wait_for_idle();
    let field = id(&mut ui, "field");
    let side = (100.0 * ui.density().density) as i32;
    let bounds = IntRect { left: 0, top: 0, right: side, bottom: side };
    am.verify(&Call::RequestAutofill { semantics_id: field, bounds });
    assert!(toolbar.titles().is_empty());
}

/// A long press on the field's start, as the toolbar tests'.
fn long_press(ui: &mut ComposeTestRule) {
    ui.on_node_with_tag("field").perform_touch_input(|touch| {
        let duration = touch.view_configuration().long_press_timeout_millis + 100;
        touch.advance_event_time(1000);
        touch.long_click_at(Offset::new(1.0, 1.0), duration);
    });
    ui.wait_for_idle();
}

/// Without autofill (desktop), no field offers it.
#[test]
fn without_autofill_no_item() {
    let mut ui = ComposeTestRule::empty(default_host());
    ui.host().with_tree(|tree| tree.set_autofill_available(false));
    ui.host().set_input_mode(InputMode::Touch);
    ui.set_content(|| {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(state, modifier = Modifier.size(100.dp()).test_tag("field"));
    });
    ui.host().with_tree(|tree| tree.clipboard().set_text("clip"));
    long_press(&mut ui);
    ui.wait_for_idle();
    let titles = ui.text_toolbar().titles();
    assert!(titles.iter().any(|title| title == "Paste"));
    assert!(!titles.iter().any(|title| title == "Autofill"));
}
