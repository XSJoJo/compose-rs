//! Material's password fields, `SecureTextField` and `OutlinedSecureTextField` (D1143), from
//! Compose's `SecureTextFieldTest`: the decorated `BasicSecureTextField`.

#![allow(non_snake_case)]

use compose_foundation::{TextFieldState, TextObfuscationMode};
use compose_material3::*;
use compose_test::*;
use compose_ui::{Modifier, SemanticsActions as A, SemanticsProperties as P, TextLayoutResult};

fn state(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

/// A password field over `held`, filled or outlined, tagged "field", its text hidden.
fn password_field(held: TextFieldState, outlined: bool) -> ComposeTestRule {
    let mut ui = compose_test(move || {
        MaterialTheme!({
            if outlined {
                OutlinedSecureTextField!(
                    held,
                    modifier = Modifier.test_tag("field"),
                    label = Some(|| Text!("Password")),
                    text_obfuscation_mode = TextObfuscationMode::Hidden,
                );
            } else {
                SecureTextField!(
                    held,
                    modifier = Modifier.test_tag("field"),
                    label = Some(|| Text!("Password")),
                    text_obfuscation_mode = TextObfuscationMode::Hidden,
                );
            }
        });
    });
    ui.wait_for_idle();
    ui
}

// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SecureTextFieldTest.kt, testFilledSecureTextField_textContentIsPreObfuscation
// Compose: compose/material3/material3/src/androidDeviceTest/kotlin/androidx/compose/material3/SecureTextFieldTest.kt, testOutlinedSecureTextField_textContentIsPreObfuscation
#[test]
fn the_text_told_is_the_password_and_the_text_drawn_is_masked() {
    for outlined in [false, true] {
        let mut ui = password_field(state("password"), outlined);
        let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
        let told = node.config.get(&P::EDITABLE_TEXT).map(|text| text.to_string());
        assert_eq!(told.as_deref(), Some("password"), "outlined {outlined}");
        assert!(node.config.contains(&P::PASSWORD), "outlined {outlined}");
        let mut results: Vec<TextLayoutResult> = Vec::new();
        ui.on_node_with_tag("field").perform_semantics_action_with(
            &A::GET_TEXT_LAYOUT_RESULT,
            |get| {
                get(&mut results);
            },
        );
        let drawn = results.pop().expect("a layout").layout_input.text.text().to_string();
        assert_eq!(drawn, "\u{2022}".repeat(8), "outlined {outlined}");
    }
}

/// What's typed goes into the state; a single line asking for a password keyboard that corrects
/// nothing, at least Material's least width.
#[test]
fn it_is_a_single_line_password_field_with_its_decoration() {
    for outlined in [false, true] {
        let held = state("");
        let mut ui = password_field(held, outlined);
        ui.on_node_with_tag("field").perform_text_input("ab");
        assert_eq!(held.text().as_str(), "ab", "outlined {outlined}");
        let options = ui.host().text_input_options().expect("a session");
        assert_eq!(options.keyboard_type, compose_foundation::KeyboardType::Password);
        assert!(!options.auto_correct && options.single_line, "outlined {outlined}");
        let width = ui.on_node_with_tag("field").get_unclipped_bounds_in_root().width();
        assert!(width.0 >= TextFieldDefaults::MIN_WIDTH.0 - 0.5, "outlined {outlined}");
    }
}
