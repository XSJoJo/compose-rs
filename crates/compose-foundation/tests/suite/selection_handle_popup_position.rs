//! Where a selection handle's popup goes and what its semantics say, from Compose's
//! `SelectionHandlePopupPositionTest` and `SelectionHandlesTest`'s direction checks.
//!
//! As Compose's tests: density 1, the handle pointing at (120, 120) in the layout around it.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{LayoutDirection, Offset, ResolvedTextDirection, UnitExt};

use crate::text_field_touch_support::*;

const OFFSET: Offset = Offset { x: 120.0, y: 120.0 };
/// A selection handle's icon, as Compose's `HandleWidth`.
const HANDLE_WIDTH: f32 = 25.0;

/// A start or end handle pointing at `OFFSET` in a layout 100 dp square, the layout direction
/// `rtl` or not, in a box of `container`'s, as Compose's `createSelectionHandle`.
fn selection_handle(start: bool, container: Modifier, rtl: bool) -> ComposeTestRule {
    let mut ui = compose_test(move || {
        let direction = if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        Box!(modifier = container.clone(), {
            compose_runtime::CompositionLocalProvider(
                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                move || {
                    Box!({
                        Spacer!(modifier = Modifier.size((100.dp(), 100.dp())));
                        let provider: Rc<dyn Fn() -> Option<Offset>> = Rc::new(|| Some(OFFSET));
                        SelectionHandle!(provider, start, ResolvedTextDirection::Ltr, false);
                    });
                },
            );
        });
    });
    ui.wait_for_idle();
    ui
}

/// The popup of the one handle is at (`x`, `y`), as Compose's `singleSelectionHandleMatches`
/// (whole dp).
#[track_caller]
fn assert_popup_at(ui: &mut ComposeTestRule, x: f32, y: f32) {
    let bounds = ui.on_node(is_selection_handle(None)).fetch_semantics_snapshot().bounds;
    assert_eq!((bounds.left as i32, bounds.top as i32), (x as i32, y as i32), "{bounds:?}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlePopupPositionTest.kt, leftHandle_Ltr_correctPosition
#[test]
fn a_left_handles_popup_has_its_top_right_on_the_point() {
    let mut ui = selection_handle(true, Modifier, false);
    assert_popup_at(&mut ui, OFFSET.x - HANDLE_WIDTH, OFFSET.y);
    assert_handle_position(&mut ui, Handle::SelectionStart, OFFSET.x, OFFSET.y);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlePopupPositionTest.kt, leftHandle_Rtl_correctPosition
#[test]
fn a_left_handles_popup_is_placed_alike_in_a_right_to_left_layout() {
    let mut ui = selection_handle(true, Modifier, true);
    assert_popup_at(&mut ui, OFFSET.x - HANDLE_WIDTH, OFFSET.y);
    assert_handle_position(&mut ui, Handle::SelectionStart, OFFSET.x, OFFSET.y);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlePopupPositionTest.kt, rightHandle_Ltr_correctPosition
#[test]
fn a_right_handles_popup_has_its_top_left_on_the_point() {
    let mut ui = selection_handle(false, Modifier, false);
    assert_popup_at(&mut ui, OFFSET.x, OFFSET.y);
    assert_handle_position(&mut ui, Handle::SelectionEnd, OFFSET.x, OFFSET.y);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlePopupPositionTest.kt, rightHandle_Rtl_correctPosition
#[test]
fn a_right_handles_popup_is_placed_alike_in_a_right_to_left_layout() {
    let mut ui = selection_handle(false, Modifier, true);
    assert_popup_at(&mut ui, OFFSET.x, OFFSET.y);
    assert_handle_position(&mut ui, Handle::SelectionEnd, OFFSET.x, OFFSET.y);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlePopupPositionTest.kt, leftHandle_semanticsPosition_isRelativeToContainer
#[test]
fn a_left_handles_semantics_point_is_in_its_layouts_coordinates() {
    let mut ui = selection_handle(true, Modifier.offset(20.dp(), 30.dp()), false);
    assert_popup_at(&mut ui, 20.0 + OFFSET.x - HANDLE_WIDTH, 30.0 + OFFSET.y);
    assert_handle_position(&mut ui, Handle::SelectionStart, OFFSET.x, OFFSET.y);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlePopupPositionTest.kt, rightHandle_semanticsPosition_isRelativeToContainer
#[test]
fn a_right_handles_semantics_point_is_in_its_layouts_coordinates() {
    let mut ui = selection_handle(false, Modifier.offset(20.dp(), 30.dp()), false);
    assert_popup_at(&mut ui, 20.0 + OFFSET.x, 30.0 + OFFSET.y);
    assert_handle_position(&mut ui, Handle::SelectionEnd, OFFSET.x, OFFSET.y);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, isHandleLtrDirection_ltr_handles_not_cross_return_true
#[test]
fn an_uncrossed_handle_in_left_to_right_text_runs_left_to_right() {
    assert!(is_handle_ltr_direction(ResolvedTextDirection::Ltr, false));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, isHandleLtrDirection_ltr_handles_cross_return_false
#[test]
fn a_crossed_handle_in_left_to_right_text_runs_right_to_left() {
    assert!(!is_handle_ltr_direction(ResolvedTextDirection::Ltr, true));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, isHandleLtrDirection_rtl_handles_not_cross_return_false
#[test]
fn an_uncrossed_handle_in_right_to_left_text_runs_right_to_left() {
    assert!(!is_handle_ltr_direction(ResolvedTextDirection::Rtl, false));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, isHandleLtrDirection_rtl_handles_cross_return_true
#[test]
fn a_crossed_handle_in_right_to_left_text_runs_left_to_right() {
    assert!(is_handle_ltr_direction(ResolvedTextDirection::Rtl, true));
}
