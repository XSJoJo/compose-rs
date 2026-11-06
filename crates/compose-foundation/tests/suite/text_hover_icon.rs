//! The pointer icons of texts and text fields, as Compose's `BasicTextHoverTest`,
//! `CoreTextFieldHoverTest` and `BasicTextFieldHoverTest` (D1676): a selectable text and a field
//! show the I-beam over a box's icon and their own modifier's, unless that one overrides its
//! descendants; a text that can't be selected shows what's around it. Compose's
//! `PointerIconTestScope` wraps the content in a box with the arrow, as here.

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{PointerIcon, UnitExt};

const CONTAINER: &str = "container";
const TEXT: &str = "text";

/// An icon a test's modifier sets: none, `pointer_hover_icon(icon)`, or the same overriding its
/// descendants.
#[derive(Clone)]
enum Set {
    Nothing,
    Icon(PointerIcon),
    Overriding(PointerIcon),
}

impl Set {
    fn on(&self, modifier: Modifier) -> Modifier {
        match self.clone() {
            Set::Nothing => modifier,
            Set::Icon(icon) => modifier.pointer_hover_icon(icon),
            Set::Overriding(icon) => modifier.pointer_hover_icon_with(icon, true),
        }
    }
}

/// Compose's `PointerIconTestScope.setContent` and the `runTest`s: hovering the text (or field)
/// shows `text_icon`, the box past it `box_icon`, and leaving the window the arrow.
fn run(
    content: fn(Modifier, Modifier),
    box_set: Set,
    text_set: Set,
    box_icon: PointerIcon,
    text_icon: PointerIcon,
) {
    let (box_modifier, text_modifier) = (box_set.on(Modifier), text_set.on(Modifier));
    let mut ui = compose_test(move || {
        let (box_modifier, text_modifier) = (box_modifier.clone(), text_modifier.clone());
        Box!(modifier = Modifier.pointer_hover_icon(PointerIcon::DEFAULT), {
            content(box_modifier.clone(), text_modifier.clone());
        });
    });
    ui.on_node_with_tag(TEXT).perform_mouse_input(|mouse| {
        let position = mouse.bottom_right();
        mouse.enter(position);
    });
    assert_eq!(ui.host().pointer_icon(), text_icon, "over the text");
    ui.on_node_with_tag(CONTAINER).perform_mouse_input(|mouse| {
        let position = mouse.bottom_right();
        mouse.move_to(position);
    });
    assert_eq!(ui.host().pointer_icon(), box_icon, "over the box");
    ui.on_node_with_tag(CONTAINER).perform_mouse_input(|mouse| {
        let position = mouse.current_position();
        mouse.exit(position);
    });
    assert_eq!(ui.host().pointer_icon(), PointerIcon::DEFAULT, "out of the window");
}

const ARROW: PointerIcon = PointerIcon::DEFAULT;
const CROSSHAIR: PointerIcon = PointerIcon::CROSSHAIR;
const HAND: PointerIcon = PointerIcon::HAND;
const I_BEAM: PointerIcon = PointerIcon::TEXT;

// ---- BasicText --------------------------------------------------------------------------------

/// A text in a 200 dp box in a `SelectionContainer`.
fn selectable(box_modifier: Modifier, text_modifier: Modifier) {
    SelectionContainer!({
        let container = Modifier.required_size(200.dp()).then(box_modifier.clone());
        let text_modifier = text_modifier.clone();
        Box!(modifier = container.test_tag(CONTAINER), {
            BasicText!("this is selectable text", modifier = text_modifier.clone().test_tag(TEXT));
        });
    });
}

/// A text in a 200 dp box.
fn not_selectable(box_modifier: Modifier, text_modifier: Modifier) {
    Box!(modifier = Modifier.required_size(200.dp()).then(box_modifier).test_tag(CONTAINER), {
        BasicText!("this is selectable text", modifier = text_modifier.clone().test_tag(TEXT));
    });
}

/// A text under `DisableSelection` in a 200 dp box in a `SelectionContainer`.
fn selection_disabled(box_modifier: Modifier, text_modifier: Modifier) {
    SelectionContainer!({
        let container = Modifier.required_size(200.dp()).then(box_modifier.clone());
        let text_modifier = text_modifier.clone();
        Box!(modifier = container.test_tag(CONTAINER), {
            let text_modifier = text_modifier.clone();
            DisableSelection!({
                BasicText!(
                    "this is selectable text",
                    modifier = text_modifier.clone().test_tag(TEXT)
                );
            });
        });
    });
}

// Compose: BasicTextHoverTest.whenSelectableText_andDefaultIcon_inBoxWithDefaultIcon_textIconIsUsed
#[test]
fn when_selectable_text_and_default_icon_in_box_with_default_icon_text_icon_is_used() {
    run(selectable, Set::Nothing, Set::Nothing, ARROW, I_BEAM);
}

// Compose: BasicTextHoverTest.whenSelectableText_andSetIcon_inBoxWithDefaultIcon_textIconIsUsed
#[test]
fn when_selectable_text_and_set_icon_in_box_with_default_icon_text_icon_is_used() {
    run(selectable, Set::Nothing, Set::Icon(CROSSHAIR), ARROW, I_BEAM);
}

// Compose: BasicTextHoverTest.whenSelectableText_andSetIcon_withOverride_inBoxWithDefaultIcon_setIconIsUsed
#[test]
fn when_selectable_text_and_set_icon_with_override_in_box_with_default_icon_set_icon_is_used() {
    run(selectable, Set::Nothing, Set::Overriding(CROSSHAIR), ARROW, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenSelectableText_andDefaultIcon_inBoxWithSetIcon_textIconIsUsed
#[test]
fn when_selectable_text_and_default_icon_in_box_with_set_icon_text_icon_is_used() {
    run(selectable, Set::Icon(HAND), Set::Nothing, HAND, I_BEAM);
}

// Compose: BasicTextHoverTest.whenSelectableText_andSetIcon_inBoxWithSetIcon_textIconIsUsed
#[test]
fn when_selectable_text_and_set_icon_in_box_with_set_icon_text_icon_is_used() {
    run(selectable, Set::Icon(HAND), Set::Icon(CROSSHAIR), HAND, I_BEAM);
}

// Compose: BasicTextHoverTest.whenSelectableText_andSetIcon_withOverride_inBoxWithSetIcon_setIconIsUsed
#[test]
fn when_selectable_text_and_set_icon_with_override_in_box_with_set_icon_set_icon_is_used() {
    run(selectable, Set::Icon(HAND), Set::Overriding(CROSSHAIR), HAND, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenNonSelectableText_andDefaultIcon_inBoxWithDefaultIcon_textIconIsUsed
// (the arrow, despite the name)
#[test]
fn when_non_selectable_text_and_default_icon_in_box_with_default_icon_text_icon_is_used() {
    run(not_selectable, Set::Nothing, Set::Nothing, ARROW, ARROW);
}

// Compose: BasicTextHoverTest.whenNonSelectableText_andSetIcon_inBoxWithDefaultIcon_setIconIsUsed
#[test]
fn when_non_selectable_text_and_set_icon_in_box_with_default_icon_set_icon_is_used() {
    run(not_selectable, Set::Nothing, Set::Icon(CROSSHAIR), ARROW, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenNonSelectableText_andSetIcon_withOverride_inBoxWithDefaultIcon_setIconIsUsed
#[test]
fn when_non_selectable_text_and_set_icon_with_override_in_box_with_default_icon_set_icon_is_used() {
    run(not_selectable, Set::Nothing, Set::Overriding(CROSSHAIR), ARROW, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenNonSelectableText_andDefaultIcon_inBoxWithSetIcon_textIconIsUsed
// (the box's, despite the name)
#[test]
fn when_non_selectable_text_and_default_icon_in_box_with_set_icon_text_icon_is_used() {
    run(not_selectable, Set::Icon(HAND), Set::Nothing, HAND, HAND);
}

// Compose: BasicTextHoverTest.whenNonSelectableText_andSetIcon_inBoxWithSetIcon_setIconIsUsed
#[test]
fn when_non_selectable_text_and_set_icon_in_box_with_set_icon_set_icon_is_used() {
    run(not_selectable, Set::Icon(HAND), Set::Icon(CROSSHAIR), HAND, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenNonSelectableText_andSetIcon_withOverride_inBoxWithSetIcon_setIconIsUsed
#[test]
fn when_non_selectable_text_and_set_icon_with_override_in_box_with_set_icon_set_icon_is_used() {
    run(not_selectable, Set::Icon(HAND), Set::Overriding(CROSSHAIR), HAND, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenDisabledSelectionText_andDefaultIcon_inBoxWithDefaultIcon_textIconIsUsed
// (the arrow, despite the name)
#[test]
fn when_disabled_selection_text_and_default_icon_in_box_with_default_icon_text_icon_is_used() {
    run(selection_disabled, Set::Nothing, Set::Nothing, ARROW, ARROW);
}

// Compose: BasicTextHoverTest.whenDisabledSelectionText_andSetIcon_inBoxWithDefaultIcon_setIconIsUsed
#[test]
fn when_disabled_selection_text_and_set_icon_in_box_with_default_icon_set_icon_is_used() {
    run(selection_disabled, Set::Nothing, Set::Icon(CROSSHAIR), ARROW, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenDisabledSelectionText_andSetIcon_withOverride_inBoxWithDefaultIcon_setIconIsUsed
#[test]
fn when_disabled_selection_text_and_set_icon_with_override_in_box_with_default_icon_set_icon_is_used()
 {
    run(selection_disabled, Set::Nothing, Set::Overriding(CROSSHAIR), ARROW, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenDisabledSelectionText_andDefaultIcon_inBoxWithSetIcon_textIconIsUsed
// (the box's, despite the name)
#[test]
fn when_disabled_selection_text_and_default_icon_in_box_with_set_icon_text_icon_is_used() {
    run(selection_disabled, Set::Icon(HAND), Set::Nothing, HAND, HAND);
}

// Compose: BasicTextHoverTest.whenDisabledSelectionText_andSetIcon_inBoxWithSetIcon_setIconIsUsed
#[test]
fn when_disabled_selection_text_and_set_icon_in_box_with_set_icon_set_icon_is_used() {
    run(selection_disabled, Set::Icon(HAND), Set::Icon(CROSSHAIR), HAND, CROSSHAIR);
}

// Compose: BasicTextHoverTest.whenDisabledSelectionText_andSetIcon_withOverride_inBoxWithSetIcon_setIconIsUsed
#[test]
fn when_disabled_selection_text_and_set_icon_with_override_in_box_with_set_icon_set_icon_is_used() {
    run(selection_disabled, Set::Icon(HAND), Set::Overriding(CROSSHAIR), HAND, CROSSHAIR);
}

// ---- Text fields ------------------------------------------------------------------------------

/// A 50 dp field over a value (Compose's `CoreTextField`, compose-rs's `BasicTextField`) in a
/// 200 dp box.
fn value_field(box_modifier: Modifier, field_modifier: Modifier) {
    // Remembered, the state is its handle.
    let handle = compose_runtime::remember(|| {
        compose_runtime::mutable_state_of(String::from("initial text"))
    });
    Box!(modifier = Modifier.required_size(200.dp()).then(box_modifier).test_tag(CONTAINER), {
        BasicTextField!(
            handle.get(),
            move |text: String| handle.set(text),
            modifier = Modifier.required_size(50.dp()).then(field_modifier.clone()).test_tag(TEXT)
        );
    });
}

/// A 50 dp field over a `TextFieldState` (Compose's `BasicTextField(state)`) in a 200 dp box.
fn state_field(box_modifier: Modifier, field_modifier: Modifier) {
    let state = remember_text_field_state("initial text", None);
    Box!(modifier = Modifier.required_size(200.dp()).then(box_modifier).test_tag(CONTAINER), {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.required_size(50.dp()).then(field_modifier.clone()).test_tag(TEXT)
        );
    });
}

// Compose: CoreTextFieldHoverTest.whenDefaultIcon_inBoxWithDefaultIcon_textIconIsUsed,
// BasicTextFieldHoverTest.whenDefaultIcon_inBoxWithDefaultIcon_textIconIsUsed
#[test]
fn text_field_when_default_icon_in_box_with_default_icon_text_icon_is_used() {
    for field in [value_field as fn(Modifier, Modifier), state_field] {
        run(field, Set::Nothing, Set::Nothing, ARROW, I_BEAM);
    }
}

// Compose: CoreTextFieldHoverTest.whenSetIcon_inBoxWithDefaultIcon_textIconIsUsed,
// BasicTextFieldHoverTest.whenSetIcon_inBoxWithDefaultIcon_textIconIsUsed
#[test]
fn text_field_when_set_icon_in_box_with_default_icon_text_icon_is_used() {
    for field in [value_field as fn(Modifier, Modifier), state_field] {
        run(field, Set::Nothing, Set::Icon(CROSSHAIR), ARROW, I_BEAM);
    }
}

// Compose: CoreTextFieldHoverTest.whenSetIcon_withOverride_inBoxWithDefaultIcon_setIconIsUsed,
// BasicTextFieldHoverTest.whenSetIcon_withOverride_inBoxWithDefaultIcon_setIconIsUsed
#[test]
fn text_field_when_set_icon_with_override_in_box_with_default_icon_set_icon_is_used() {
    for field in [value_field as fn(Modifier, Modifier), state_field] {
        run(field, Set::Nothing, Set::Overriding(CROSSHAIR), ARROW, CROSSHAIR);
    }
}

// Compose: CoreTextFieldHoverTest.whenDefaultIcon_inBoxWithSetIcon_textIconIsUsed,
// BasicTextFieldHoverTest.whenDefaultIcon_inBoxWithSetIcon_textIconIsUsed
#[test]
fn text_field_when_default_icon_in_box_with_set_icon_text_icon_is_used() {
    for field in [value_field as fn(Modifier, Modifier), state_field] {
        run(field, Set::Icon(HAND), Set::Nothing, HAND, I_BEAM);
    }
}

// Compose: CoreTextFieldHoverTest.whenSetIcon_inBoxWithSetIcon_textIconIsUsed,
// BasicTextFieldHoverTest.whenSetIcon_inBoxWithSetIcon_textIconIsUsed
#[test]
fn text_field_when_set_icon_in_box_with_set_icon_text_icon_is_used() {
    for field in [value_field as fn(Modifier, Modifier), state_field] {
        run(field, Set::Icon(HAND), Set::Icon(CROSSHAIR), HAND, I_BEAM);
    }
}

// Compose: CoreTextFieldHoverTest.whenSetIcon_withOverride_inBoxWithSetIcon_setIconIsUsed,
// BasicTextFieldHoverTest.whenSetIcon_withOverride_inBoxWithSetIcon_setIconIsUsed
#[test]
fn text_field_when_set_icon_with_override_in_box_with_set_icon_set_icon_is_used() {
    for field in [value_field as fn(Modifier, Modifier), state_field] {
        run(field, Set::Icon(HAND), Set::Overriding(CROSSHAIR), HAND, CROSSHAIR);
    }
}
