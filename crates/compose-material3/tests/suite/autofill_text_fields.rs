//! Material's fields and autofill, from Compose's `TextFieldsSemanticAutofillTest` (D1684,
//! D1685): filled by the platform's values, and highlighted with `LOCAL_AUTOFILL_HIGHLIGHT_BRUSH`
//! once filled, read in the display list (compose-test has no capture to image). Its two tests of
//! the deprecated `LocalAutofillHighlightColor` are n/a (deprecated API).

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_material3::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Brush, Color, ContentType, FillableData, Modifier};

const USERNAME_TAG: &str = "username_tag";

fn username_id(ui: &mut ComposeTestRule) -> i32 {
    ui.on_node_with_tag(USERNAME_TAG).fetch_semantics_snapshot().semantics_id
}

fn fill_username(ui: &mut ComposeTestRule) {
    let id = username_id(ui);
    let value = FillableData::from_platform_text("testUsername");
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(id, value)]));
    ui.wait_for_idle();
}

fn username_modifier() -> Modifier {
    Modifier
        .test_tag(USERNAME_TAG)
        .semantics(|config| config.set_content_type(ContentType::USERNAME))
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/TextFieldsSemanticAutofillTest.kt, performAutofill_credentials_legacyTF
/// A filled `TextField` hands its value to its callback.
#[test]
fn a_text_field_is_filled() {
    let input = mutable_state_of(String::new());
    let state = input.handle();
    let mut ui = compose_test(move || {
        Column!({
            TextField!(
                state.get(),
                move |it: String| state.set(it),
                modifier = username_modifier(),
                label = Some(|| Text!("Enter username here"))
            );
        });
    });
    fill_username(&mut ui);
    assert_eq!(input.get(), "testUsername");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/TextFieldsSemanticAutofillTest.kt, performAutofill_credentials_outlinedTF
#[test]
fn an_outlined_text_field_is_filled() {
    let input = mutable_state_of(String::new());
    let state = input.handle();
    let mut ui = compose_test(move || {
        Column!({
            OutlinedTextField!(
                state.get(),
                move |it: String| state.set(it),
                modifier = username_modifier(),
                label = Some(|| Text!("Enter username here"))
            );
        });
    });
    fill_username(&mut ui);
    assert_eq!(input.get(), "testUsername");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/TextFieldsSemanticAutofillTest.kt, performAutofill_customBrush_gradientHighlight
/// A gradient as the highlight's brush: neither color is drawn before the fill, both after.
#[test]
fn a_gradient_highlights_a_filled_field() {
    let input = mutable_state_of(String::new());
    let state = input.handle();
    let (start, end) = (Color::BLUE, Color::GREEN);
    let mut ui = compose_test(move || {
        CompositionLocalProvider(
            compose_foundation::LOCAL_AUTOFILL_HIGHLIGHT_BRUSH
                .provides(Brush::horizontal_gradient(vec![start, end])),
            move || {
                TextField!(
                    state.get(),
                    move |it: String| state.set(it),
                    modifier = username_modifier()
                );
            },
        );
    });
    let dump = |ui: &mut ComposeTestRule| ui.host().display_list().dump();
    let (start_text, end_text) = (format!("{start:?}"), format!("{end:?}"));
    let before = dump(&mut ui);
    assert!(!before.contains(&start_text) && !before.contains(&end_text), "{before}");
    fill_username(&mut ui);
    let after = dump(&mut ui);
    assert!(after.contains(&start_text) && after.contains(&end_text), "{after}");
}
