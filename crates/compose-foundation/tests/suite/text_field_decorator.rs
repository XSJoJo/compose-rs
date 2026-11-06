//! A state-based field's decorator, from Compose's `TextFieldDecoratorTest`: the focus, the
//! semantics and the taps are the decorated field's, and the decoration is measured at the
//! field's least size.
//!
//! Compose's two `nonPlacedInnerTextField_*` don't hold: compose-rs's field node is the inner
//! text's, so a decorator not calling it has no field to type in (D1147). Nor do
//! `focusIsAppliedOnDecoratedComposable` and `semanticsAreAppliedOnDecoratedComposable`: the
//! inner field's semantics are its nearest semantics node's above it (`PART_OF_PARENT`), the
//! decoration's tagged box there, where Compose's are the decorated field's (the decorator
//! modifier's), the tagged box a child of it (D1149). The long press's selection waits on
//! touch-text-selection.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Color, Constraints, Layout, Offset, TextRange, UnitExt, measure_policy};

const TAG: &str = "field";
const DECORATION: &str = "decoration";

fn state_of(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

/// A field over `state` whose decoration, tagged, is a bordered box padded 16 dp around the
/// inner field: Compose's tests' decorator.
fn decorated(state: TextFieldState) -> ComposeTestRule {
    compose_test(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag(TAG),
            decorator = |inner: Callback| {
                Box!(
                    modifier =
                        Modifier.border(2.dp(), Color::RED).padding(16.dp()).test_tag(DECORATION),
                    {
                        inner();
                    }
                );
            },
        );
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldDecoratorTest.kt, clickGestureIsAppliedOnDecoratedComposable
#[test]
fn a_tap_on_the_decoration_focuses_the_field() {
    let mut ui = decorated(state_of("hello"));
    // On the box's padding, not on the inner field.
    ui.on_node_with_tag_unmerged(DECORATION)
        .perform_touch_input(|touch| touch.click_at(Offset::new(1.0, 1.0)));
    ui.on_node_with_tag(TAG).assert_is_focused();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldDecoratorTest.kt, minConstraintsArePropagated
#[test]
fn the_decoration_is_measured_at_the_fields_least_size() {
    let constraints: Rc<Cell<Option<Constraints>>> = Rc::default();
    let into = constraints.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        BasicTextFieldOfState!(
            state_of(""),
            modifier = Modifier.fill_max_size().test_tag(TAG),
            decorator = move |_inner: Callback| {
                let into = into.clone();
                Layout!(
                    measure_policy = measure_policy(move |scope, _measurables, constraints| {
                        into.set(Some(constraints));
                        scope.layout(0, 0)
                    }),
                    {}
                );
            },
        );
    });
    ui.wait_for_idle();
    let constraints = constraints.get().expect("measured");
    assert_ne!(constraints.min_width, 0);
    assert_eq!(constraints.min_width, constraints.max_width);
    assert_ne!(constraints.min_height, 0);
    assert_eq!(constraints.min_height, constraints.max_height);
}

/// A long press on a decoration around no inner field selects nothing, and doesn't crash; nor
/// does a drag after it (b/308398612).
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldDecoratorTest.kt, longClickGestureIsIgnored_whenInnerTextFieldNotPlaced
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldDecoratorTest.kt, longClickAndDragGestureIsIgnored_whenInnerTextFieldNotPlaced
#[test]
fn a_long_press_around_no_inner_field_selects_nothing() {
    for drag in [false, true] {
        let state = state_of("hello");
        let mut ui = compose_test(move || {
            BasicTextFieldOfState!(
                state,
                modifier = Modifier.test_tag(TAG),
                decorator = move |_inner: Callback| {
                    Box!(
                        modifier = Modifier
                            .border(2.dp(), Color::RED)
                            .padding(16.dp())
                            .test_tag(DECORATION),
                        {
                            BasicText!(state.text().to_string());
                        }
                    );
                },
            );
        });
        ui.on_node_with_tag_unmerged(DECORATION).perform_touch_input(|touch| {
            let slop = touch.view_configuration().touch_slop;
            if drag {
                // Held past the long-press timeout, then dragged.
                touch.down(Offset::new(1.0, 1.0));
                touch.move_pointer_by(0, Offset::new(0.0, 0.0), 600);
                touch.move_by(Offset::new(slop * 2.0, 0.0));
                touch.up();
            } else {
                touch.long_click_at(Offset::new(1.0, 1.0), 600);
            }
        });
        ui.wait_for_idle();
        assert_eq!(state.selection(), TextRange::at(5), "drag {drag}");
    }
}
