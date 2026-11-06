//! `BasicSecureTextField` (D1142), from Compose's `BasicSecureTextFieldTest`: its semantics, the
//! last typed character revealed for a moment, the obfuscation modes and characters.

#![allow(non_snake_case)]

use std::time::Duration;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    SemanticsActions as A, SemanticsProperties as P, TextLayoutResult, TextRange, UnitExt,
};

use crate::text_field_basic::type_on_keys;

const BULLET: &str = "\u{2022}";
/// A family of four joined by zero-width joiners: one glyph.
const COMPLEX_EMOJI: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
const REGULAR_EMOJI: &str = "\u{1F603}";

fn state_of(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

fn cell<T: PartialEq + 'static>(value: T) -> MutableState<T> {
    Box::leak(Box::new(mutable_state_of(value))).handle()
}

fn bullets(count: usize) -> String {
    BULLET.repeat(count)
}

/// A secure field over `state`, tagged "field", in `mode` and with `mask`, its clock the test's.
fn secure(
    state: TextFieldState,
    mode: State<TextObfuscationMode>,
    mask: State<char>,
) -> ComposeTestRule {
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        Column!({
            BasicSecureTextField!(
                state,
                modifier = Modifier.width(300.dp()).test_tag("field"),
                text_obfuscation_mode = mode.get(),
                text_obfuscation_character = mask.get(),
            );
            Box!(Modifier.size(1.dp()).focusable().test_tag("other"));
        });
    });
    ui.wait_for_idle();
    ui
}

fn in_mode(mode: TextObfuscationMode) -> ComposeTestRule {
    secure(state_of(""), cell(mode).into(), cell('\u{2022}').into())
}

/// What the field lays out, as `fetchTextLayoutResult`.
fn shown(ui: &mut ComposeTestRule) -> String {
    let mut results: Vec<TextLayoutResult> = Vec::new();
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::GET_TEXT_LAYOUT_RESULT, |get| {
        get(&mut results);
    });
    results.pop().expect("a layout").layout_input.text.text().to_string()
}

fn advance(ui: &mut ComposeTestRule, millis: u64) {
    ui.advance_time_by(Duration::from_millis(millis));
}

/// Typed as an input method commits it, as `performTextInput`.
fn type_in(ui: &mut ComposeTestRule, text: &str) {
    ui.on_node_with_tag("field").perform_text_input(text);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, passwordSemanticsAreSet
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, passwordSemanticsAndIsPasswordObfuscated_whenVisible
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, passwordSemanticsIsPasswordObfuscated_whenHidden
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, passwordSemanticsIsPasswordObfuscated_whenRevealLastTyped
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, semantics_copy
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, semantics_cut
#[test]
fn a_password_says_so_and_can_be_pasted_into_but_never_copied() {
    use TextObfuscationMode::*;
    for (mode, obfuscated) in
        [(System, true), (Visible, false), (Hidden, true), (RevealLastTyped, true)]
    {
        let state =
            **Box::leak(Box::new(TextFieldState::with_selection("Hello", TextRange::new(0, 1))));
        let mut ui = secure(state, cell(mode).into(), cell('\u{2022}').into());
        ui.on_node_with_tag("field").request_focus();
        let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
        assert!(node.config.contains(&P::PASSWORD), "{mode:?}");
        assert_eq!(node.config.get(&P::IS_PASSWORD_OBFUSCATED).copied(), Some(obfuscated));
        assert!(node.config.contains(&A::PASTE_TEXT), "{mode:?}");
        assert!(!node.config.contains(&A::COPY_TEXT), "{mode:?}");
        assert!(!node.config.contains(&A::CUT_TEXT), "{mode:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, passwordSemanticsIsPasswordObfuscated_updatesDynamically
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, systemTextObfuscationMode_isRevealLastTypedEnabled
#[test]
fn obfuscated_follows_the_mode() {
    assert_ne!(TextObfuscationMode::System, TextObfuscationMode::RevealLastTyped);
    assert_eq!(TextObfuscationMode::default(), TextObfuscationMode::System);
    let mode = cell(TextObfuscationMode::Hidden);
    let mut ui = secure(state_of("Hello"), mode.into(), cell('\u{2022}').into());
    let obfuscated = |ui: &mut ComposeTestRule| {
        let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
        node.config.get(&P::IS_PASSWORD_OBFUSCATED).copied()
    };
    assert_eq!(obfuscated(&mut ui), Some(true));
    mode.set(TextObfuscationMode::Visible);
    ui.advance_time_by_frame();
    assert_eq!(obfuscated(&mut ui), Some(false));
    mode.set(TextObfuscationMode::Hidden);
    ui.advance_time_by_frame();
    assert_eq!(obfuscated(&mut ui), Some(true));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, lastTypedCharacterIsRevealedTemporarily
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodRevealLastTyped_usesCustomObfuscationCharacter
#[test]
fn the_last_typed_character_shows_for_a_moment() {
    let mut ui = in_mode(TextObfuscationMode::System);
    type_in(&mut ui, "a");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "a");
    advance(&mut ui, 1500);
    assert_eq!(shown(&mut ui), BULLET);

    let reveal = cell(TextObfuscationMode::RevealLastTyped).into();
    let mut ui = secure(state_of(""), reveal, cell('&').into());
    type_in(&mut ui, "a");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "a");
    advance(&mut ui, 1500);
    assert_eq!(shown(&mut ui), "&");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, lastTypedCharacterIsRevealed_hidesAfterAnotherCharacterIsTyped
#[test]
fn another_character_typed_hides_the_last() {
    let mut ui = in_mode(TextObfuscationMode::System);
    type_in(&mut ui, "a");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "a");
    type_in(&mut ui, "b");
    advance(&mut ui, 50);
    assert_eq!(shown(&mut ui), format!("{BULLET}b"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, lastTypedCharacterIsRevealed_whenInsertedInMiddle
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, lastTypedCharacterIsRevealed_whenReplacingSelection
#[test]
fn a_character_typed_in_the_middle_or_over_a_selection_shows() {
    let mut ui = in_mode(TextObfuscationMode::System);
    type_in(&mut ui, "abc");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), bullets(3));
    // Offsets in what's shown: a bullet is 3 bytes.
    select(&mut ui, 3, 3);
    type_in(&mut ui, "d");
    advance(&mut ui, 50);
    assert_eq!(shown(&mut ui), format!("{BULLET}d{}", bullets(2)));

    let mut ui = in_mode(TextObfuscationMode::System);
    type_in(&mut ui, "abc");
    advance(&mut ui, 200);
    select(&mut ui, 3, 6);
    type_in(&mut ui, "#");
    advance(&mut ui, 50);
    assert_eq!(shown(&mut ui), format!("{BULLET}#{BULLET}"));
    advance(&mut ui, 1500);
    assert_eq!(shown(&mut ui), bullets(3));
}

fn select(ui: &mut ComposeTestRule, start: i32, end: i32) {
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_SELECTION, |set| {
        set(start, end, false);
    });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, lastTypedCharacterIsRevealed_hidesAfterFocusIsLost
#[test]
fn losing_the_focus_hides_it() {
    let mut ui = in_mode(TextObfuscationMode::System);
    type_in(&mut ui, "a");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "a");
    ui.on_node_with_tag("other").request_focus();
    advance(&mut ui, 50);
    assert_eq!(shown(&mut ui), BULLET);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, lastTypedCharacterIsRevealed_hidesAfterAnotherCharacterRemoved
#[test]
fn another_edit_hides_it() {
    let mut ui = in_mode(TextObfuscationMode::System);
    type_in(&mut ui, "abc");
    advance(&mut ui, 200);
    type_in(&mut ui, "d");
    advance(&mut ui, 50);
    assert_eq!(shown(&mut ui), format!("{}d", bullets(3)));
    ui.on_node_with_tag("field").perform_semantics_action_with(&A::SET_TEXT, |set| {
        set("bcd".into());
    });
    assert_eq!(shown(&mut ui), bullets(3));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodVisible_doesNotHideAnything
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodVisible_revealsEverythingWhenSwitchedTo
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodHidden_hidesEverything
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodHidden_hidesEverythingWhenSwitchedTo
#[test]
fn visible_shows_everything_and_hidden_nothing() {
    let mut ui = in_mode(TextObfuscationMode::Visible);
    type_in(&mut ui, "abc");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "abc");

    let mut ui = in_mode(TextObfuscationMode::Hidden);
    type_in(&mut ui, "abc");
    ui.advance_time_by_frame();
    assert_eq!(shown(&mut ui), bullets(3));
    type_in(&mut ui, "d");
    ui.advance_time_by_frame();
    assert_eq!(shown(&mut ui), bullets(4));

    let mode = cell(TextObfuscationMode::Hidden);
    let mut ui = secure(state_of(""), mode.into(), cell('\u{2022}').into());
    type_in(&mut ui, "abc");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), bullets(3));
    mode.set(TextObfuscationMode::Visible);
    ui.advance_time_by_frame();
    assert_eq!(shown(&mut ui), "abc");
    mode.set(TextObfuscationMode::Hidden);
    ui.advance_time_by_frame();
    assert_eq!(shown(&mut ui), bullets(3));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodHidden_usesCustomObfuscationCharacter
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodHidden_toggleObfuscationCharacter
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, obfuscationMethodRevealLastTyped_toggleObfuscationCharacter
#[test]
fn the_mask_is_the_character_given_even_a_new_one() {
    let mask = cell('&');
    let mut ui = secure(state_of(""), cell(TextObfuscationMode::Hidden).into(), mask.into());
    type_in(&mut ui, "abc");
    ui.advance_time_by_frame();
    assert_eq!(shown(&mut ui), "&&&");
    type_in(&mut ui, "d");
    ui.advance_time_by_frame();
    assert_eq!(shown(&mut ui), "&&&&");
    mask.set('*');
    ui.advance_time_by_frame();
    assert_eq!(shown(&mut ui), "****");

    let mask = cell('*');
    let reveal = cell(TextObfuscationMode::RevealLastTyped);
    let mut ui = secure(state_of(""), reveal.into(), mask.into());
    type_in(&mut ui, "a");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "a");
    advance(&mut ui, 1500);
    assert_eq!(shown(&mut ui), "*");
    mask.set('&');
    type_in(&mut ui, "b");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "&b");
    advance(&mut ui, 1500);
    assert_eq!(shown(&mut ui), "&&");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, emojisAreObfuscatedAsSingleCharacter
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, emojisAreRevealedSeparately
#[test]
fn an_emoji_is_one_masked_or_revealed_character() {
    let text = format!("{COMPLEX_EMOJI}hello{REGULAR_EMOJI}");
    let mut ui =
        secure(state_of(&text), cell(TextObfuscationMode::System).into(), cell('*').into());
    assert_eq!(shown(&mut ui), "*******");

    let mut ui = secure(state_of(""), cell(TextObfuscationMode::System).into(), cell('*').into());
    type_in(&mut ui, COMPLEX_EMOJI);
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), COMPLEX_EMOJI);
    type_in(&mut ui, COMPLEX_EMOJI);
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), format!("*{COMPLEX_EMOJI}"));
    type_in(&mut ui, REGULAR_EMOJI);
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), format!("**{REGULAR_EMOJI}"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, inputMethod_doesNotRestart_inResponseToKeyEvents
#[test]
fn keys_and_selections_do_not_restart_the_input_method() {
    let state = **Box::leak(Box::new(TextFieldState::with_selection("hello", TextRange::at(5))));
    let mut ui = secure(state, cell(TextObfuscationMode::System).into(), cell('*').into());
    ui.on_node_with_tag("field").request_focus();
    ui.host().take_input_method_restart();
    ui.press_key(compose_ui::Key::Backspace);
    select(&mut ui, 0, 0);
    ui.press_key(compose_ui::Key::Delete);
    assert!(!ui.host().take_input_method_restart());
    assert_eq!(state.text().as_str(), "ell");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, textField_focus_doesNotShowSoftwareKeyboard_ifReadOnly
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, isNotEditable_whenDisabledOrReadOnly
#[test]
fn disabled_or_read_only_it_is_not_editable() {
    let (enabled, read_only) = (cell(true), cell(true));
    let state = state_of("");
    let mut ui = compose_test(move || {
        BasicSecureTextField!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("field"),
            enabled = enabled.get(),
            read_only = read_only.get(),
        );
    });
    ui.on_node_with_tag("field").perform_click();
    ui.wait_for_idle();
    ui.on_node_with_tag("field").assert_is_focused();
    assert!(ui.host().text_input_options().is_none(), "no session read-only");
    let editable = |ui: &mut ComposeTestRule| {
        let node = ui.on_node_with_tag("field").fetch_semantics_snapshot();
        node.config.get(&P::IS_EDITABLE).copied()
    };
    for (on, read) in [(true, false), (true, true), (false, false), (false, true), (true, false)] {
        enabled.set(on);
        read_only.set(read);
        ui.wait_for_idle();
        assert_eq!(editable(&mut ui), Some(on && !read), "{on} {read}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, minConstraints_arePassedDown
#[test]
fn the_least_width_given_is_its_width() {
    let width = std::rc::Rc::new(std::cell::Cell::new(0));
    let into = width.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        BoxWithConstraints!(
            modifier = Modifier.fill_max_width(),
            propagate_min_constraints = true,
            move |scope: BoxWithConstraintsScope| {
                into.set(scope.constraints.max_width);
                BasicSecureTextField!(
                    remember_text_field_state("", None),
                    modifier = Modifier.test_tag("field")
                );
            }
        );
    });
    ui.wait_for_idle();
    let bounds = ui.on_node_with_tag("field").get_unclipped_bounds_in_root();
    let density = ui.density().density;
    assert_eq!((bounds.width().0 * density).round() as i32, width.get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, hoistedScrollState_passedToBasicTextField
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, hoistedScrollState_passedToBasicTextField_afterScroll
#[test]
fn its_scroll_state_is_the_one_given() {
    let scroll_state = **Box::leak(Box::new(ScrollState::new(0)));
    let mut ui = compose_test(move || {
        BasicSecureTextField!(
            remember_text_field_state("abcd ".repeat(100), None),
            modifier = Modifier.test_tag("field"),
            scroll_state = Some(scroll_state),
        );
    });
    ui.wait_for_idle();
    assert_ne!(scroll_state.max_value(), 0);
    assert_eq!(scroll_state.value(), 0);
    ui.on_node_with_tag("field").perform_touch_input(|touch| touch.swipe_left());
    ui.wait_for_idle();
    assert_ne!(scroll_state.value(), 0);
}

/// A secure field in `System` mode under the platform's `visibility`.
fn under_platform(visibility: State<PasswordVisibility>) -> ComposeTestRule {
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        CompositionLocalProvider(LOCAL_PASSWORD_VISIBILITY.provides(visibility.get()), || {
            BasicSecureTextField!(
                remember_text_field_state("", None),
                modifier = Modifier.width(300.dp()).test_tag("field"),
                text_obfuscation_character = '*',
            );
        });
    });
    ui.wait_for_idle();
    ui
}

fn visibility(touch: bool, physical: bool) -> PasswordVisibility {
    PasswordVisibility { touch, physical }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, revealLastTypedEnabled_initializesWithPlatformSettings
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, revealLastTypedEnabled_observesPlatformSettings
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, systemMode_softwareKeyboard_respectsSplitSettings_doesNotReveal
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, systemMode_softwareKeyboard_respectsSplitSettings_doesReveal
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, systemMode_softwareKeyboard_showsAndAutoHides
#[test]
fn system_mode_reveals_as_the_platform_says_for_the_keyboard() {
    let platform = cell(visibility(false, true));
    let mut ui = under_platform(platform.into());
    ui.on_node_with_tag("field").perform_click();
    type_in(&mut ui, "a");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "*", "a screen's keyboard, not shown for it");
    // A physical keyboard's key, shown for it.
    type_on_keys(&mut ui, "b");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "*b");
    platform.set(visibility(true, false));
    ui.advance_time_by_frame();
    type_in(&mut ui, "c");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "**c");
    advance(&mut ui, 1400);
    assert_eq!(shown(&mut ui), "***");
    type_on_keys(&mut ui, "d");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "****", "a physical keyboard's, not shown for it");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, revealLastTyped_alwaysReveals_evenWhenSystemSettingDisabled
#[test]
fn reveal_last_typed_reveals_whatever_the_platform_says() {
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        CompositionLocalProvider(
            LOCAL_PASSWORD_VISIBILITY.provides(visibility(false, false)),
            || {
                BasicSecureTextField!(
                    remember_text_field_state("", None),
                    modifier = Modifier.width(300.dp()).test_tag("field"),
                    text_obfuscation_mode = TextObfuscationMode::RevealLastTyped,
                );
            },
        );
    });
    ui.wait_for_idle();
    type_in(&mut ui, "a");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), "a");
    type_on_keys(&mut ui, "b");
    advance(&mut ui, 200);
    assert_eq!(shown(&mut ui), format!("{BULLET}b"));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, paste_viaCtrlV_revealLastTyped_immediatelyHidesPassword
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/BasicSecureTextFieldTest.kt, paste_viaCtrlV_systemMode_immediatelyHidesPassword
#[test]
fn a_paste_is_never_revealed() {
    for mode in [TextObfuscationMode::RevealLastTyped, TextObfuscationMode::System] {
        let mut ui = secure(state_of(""), cell(mode).into(), cell('*').into());
        ui.host().with_tree(|tree| tree.clipboard().set_text("ab"));
        ui.on_node_with_tag("field").request_focus();
        let shortcut = if cfg!(target_os = "macos") {
            compose_ui::Key::MetaLeft
        } else {
            compose_ui::Key::CtrlLeft
        };
        ui.on_node_with_tag("field").perform_key_input(|keys| {
            keys.with_key_down(shortcut, |keys| keys.press_key(compose_ui::Key::Character('v')));
        });
        ui.advance_time_by_frame();
        assert_eq!(shown(&mut ui), "**", "{mode:?}");
    }
}

/// A secure field's copy and cut keys do nothing, and are its own: what's around doesn't hear
/// them.
// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/BasicSecureTextField.kt, BasicSecureTextField
#[test]
fn copy_and_cut_keys_do_nothing() {
    let state =
        **Box::leak(Box::new(TextFieldState::with_selection("secret", TextRange::new(0, 6))));
    let mut ui = secure(state, cell(TextObfuscationMode::Hidden).into(), cell('*').into());
    ui.host().with_tree(|tree| tree.clipboard().set_text("before"));
    ui.on_node_with_tag("field").request_focus();
    let shortcut = if cfg!(target_os = "macos") {
        compose_ui::Key::MetaLeft
    } else {
        compose_ui::Key::CtrlLeft
    };
    for key in [compose_ui::Key::Character('c'), compose_ui::Key::Character('x')] {
        ui.on_node_with_tag("field").perform_key_input(|keys| {
            keys.with_key_down(shortcut, |keys| keys.press_key(key));
        });
    }
    assert_eq!(ui.host().with_tree(|tree| tree.clipboard().get_text()).as_deref(), Some("before"));
    assert_eq!(state.text().as_str(), "secret");
}
