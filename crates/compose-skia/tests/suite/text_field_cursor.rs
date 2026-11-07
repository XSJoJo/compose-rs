//! A field's cursor drawn by Skia, from Compose's `TextFieldCursorTest`: no cursor over a
//! selection that isn't collapsed (D2027), as Compose's `TextFieldCoreModifierNode.draw`, which
//! draws the cursor only for a collapsed selection.
//!
//! As Compose's test: the text and its background in the field's color, so only the cursor and
//! the selection's highlight could show.

#![allow(non_snake_case)]

use compose::foundation::layout::*;
use compose::foundation::*;
use compose::test::*;
use compose::ui::{Brush, Color, Density, IntSize, TextRange, TextStyle, UnitExt};
use compose_skia::SkiaTextMeasurer;

const CONTENT: Color = Color::WHITE;
const CURSOR: Color = Color::RED;

fn state_of(text: &str, selection: TextRange) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::with_selection(text, selection)))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldCursorTest.kt, selectionNotCollapsed_cursorNotDrawn
#[test]
fn a_selection_not_collapsed_draws_no_cursor() {
    set_capture_renderer(compose_skia::raster_colors);
    let state = state_of("test", TextRange::new(2, 3));
    let mut ui = compose_test_with(
        IntSize::new(300, 300),
        Density::new(1.0),
        Box::new(SkiaTextMeasurer::new()),
        move || {
            // The selection's highlight in a known color.
            let colors =
                TextSelectionColors { handle_color: Color::BLUE, background_color: Color::BLUE };
            compose::runtime::CompositionLocalProvider(
                LOCAL_TEXT_SELECTION_COLORS.provides(colors),
                move || {
                    Box!(modifier = Modifier.padding(8.dp()), {
                        BasicTextFieldOfState!(
                            state,
                            modifier = Modifier.background(CONTENT),
                            text_style = TextStyle {
                                color: CONTENT,
                                font_size: 10.sp(),
                                ..TextStyle::DEFAULT
                            },
                            cursor_brush = Brush::solid(CURSOR),
                        );
                    });
                },
            );
        },
    );
    ui.on_node(has_perform_ime_action()).request_focus();
    ui.wait_for_idle();

    ui.on_node(has_set_text_action()).capture_to_image().assert_does_not_contain_color(CURSOR);

    // Not Compose's: the same field collapsed draws its cursor, so the check above can fail.
    state.edit(|buffer| buffer.set_selection(TextRange::at(2)));
    ui.wait_for_idle();
    ui.on_node(has_set_text_action()).capture_to_image().assert_contains_color(CURSOR);
}
