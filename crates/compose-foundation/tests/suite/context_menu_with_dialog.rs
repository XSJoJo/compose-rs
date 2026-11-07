//! A field's toolbar in a dialog, shown by a provider around the dialog, is placed on the field
//! (D2035), from Compose's `ContextMenuWithTextFieldInDialogTest`.
//!
//! Compose's provider asserts as it's asked; here it records what it found and the test asserts
//! it. The two `textField1_*` tests are of the deprecated field (`n/a`);
//! `textField2_showContextMenu_doesNotCrash` waits on a field's right click.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{Dialog, InputMode, IntSize, LayoutCoordinates, LocalBoxFuture, Offset, Rect};

const TAG: &str = "textField";

/// Compose's `CustomizedContextMenu`'s provider: where the menu's position and bounds are on the
/// screen, against the field's bounds there.
struct Checking {
    coordinates: Rc<Cell<Option<LayoutCoordinates>>>,
    field: Rc<Cell<Option<LayoutCoordinates>>>,
    /// Whether each check held, at each show.
    checks: Rc<RefCell<Vec<[bool; 3]>>>,
}

impl TextContextMenuProvider for Checking {
    fn show_text_context_menu(
        &self,
        data_provider: Rc<dyn TextContextMenuDataProvider>,
    ) -> LocalBoxFuture<'static, ()> {
        let (coordinates, field, checks) =
            (self.coordinates.get(), self.field.get(), self.checks.clone());
        Box::pin(async move {
            let Some(coordinates) = coordinates else { return };
            let position = data_provider.position(&coordinates);
            let bounds = data_provider.content_bounds(&coordinates);
            let position_on_screen = coordinates.local_to_screen(position);
            let size = coordinates.size();
            let bounds_on_screen = Rect::from_offset_size(
                coordinates.local_to_screen(bounds.top_left()),
                compose_ui::Size::new(size.width as f32, size.height as f32),
            );
            let Some(field) = field else { return };
            let field_size = field.size();
            let field_on_screen = Rect::from_offset_size(
                field.local_to_screen(Offset::ZERO),
                compose_ui::Size::new(field_size.width as f32, field_size.height as f32),
            );
            let inclusive = |point: Offset| {
                point.x >= field_on_screen.left
                    && point.x <= field_on_screen.right
                    && point.y >= field_on_screen.top
                    && point.y <= field_on_screen.bottom
            };
            checks.borrow_mut().push([
                field_on_screen.contains(position_on_screen),
                inclusive(bounds_on_screen.top_left()),
                inclusive(bounds_on_screen.bottom_right()),
            ]);
        })
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuWithDialogTest.kt, textField2_showToolbar_doesNotCrash
#[test]
fn textField2_showToolbar_doesNotCrash() {
    let coordinates: Rc<Cell<Option<LayoutCoordinates>>> = Rc::default();
    let field: Rc<Cell<Option<LayoutCoordinates>>> = Rc::default();
    let checks: Rc<RefCell<Vec<[bool; 3]>>> = Rc::default();
    let provider: Rc<dyn TextContextMenuProvider> = Rc::new(Checking {
        coordinates: coordinates.clone(),
        field: field.clone(),
        checks: checks.clone(),
    });
    let state: TextFieldState = **Box::leak(Box::new(TextFieldState::new("Dialog TextField")));
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (provider, coordinates, field) = (provider.clone(), coordinates.clone(), field.clone());
        compose_runtime::CompositionLocalProvider(
            (
                LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER.provides(Some(provider.clone())),
                LOCAL_TEXT_CONTEXT_MENU_TOOLBAR_PROVIDER.provides(Some(provider)),
            ),
            move || {
                let field = field.clone();
                Box!(
                    modifier = Modifier.on_globally_positioned(move |it| coordinates.set(Some(it))),
                    {
                        let field = field.clone();
                        Dialog!(on_dismiss_request = || {}, {
                            let field = field.clone();
                            BasicTextFieldOfState!(
                                state,
                                modifier = Modifier
                                    .test_tag(TAG)
                                    .on_globally_positioned(move |it| field.set(Some(it))),
                            );
                        });
                    }
                );
            },
        );
    });
    ui.host().set_input_mode(InputMode::Touch);
    // Shows the selection's toolbar inside the dialog.
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| touch.double_click());
    ui.wait_for_idle();
    let checks = checks.borrow();
    assert!(!checks.is_empty(), "the toolbar wasn't shown");
    for check in checks.iter() {
        assert_eq!(*check, [true; 3], "the menu's position and bounds aren't on the field");
    }
}
