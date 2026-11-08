//! Compose's `PopupAlignmentTest` (ui, device tests): a 40 by 20 popup in a 100 by 100 parent
//! at the window's top left, offset by (10, 10), for each alignment, left to right and right to
//! left (the offset then goes left).
//!
//! On Android the popup's window is clipped to the screen (`PopupProperties.clippingEnabled`,
//! true by default): a popup at x -10 is moved to 0. compose-rs's popups are overlays of the
//! window with no such clipping yet; the three tests that rely on it are ignored.

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{IntOffset, IntSize, LOCAL_LAYOUT_DIRECTION, LayoutDirection, Popup, UnitExt};

/// Where the popup's content is, in the window, with `alignment` in `direction`. The parent is
/// at the window's top left whatever the direction, as Compose's `TestAlign`.
fn popup_position(alignment: Alignment, direction: LayoutDirection) -> IntOffset {
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        Box!({
            CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(direction), move || {
                Box!(modifier = Modifier.size(100.dp()), {
                    Popup!(alignment = alignment, offset = IntOffset::new(10, 10), {
                        Box!(modifier = Modifier.size((40.dp(), 20.dp())).test_tag("popup"));
                    });
                });
            });
        });
    });
    let bounds = ui.on_node_with_tag("popup").fetch_semantics_snapshot().bounds;
    assert_eq!((bounds.width(), bounds.height()), (40.0, 20.0));
    IntOffset::new(bounds.left as i32, bounds.top as i32)
}

const LTR: LayoutDirection = LayoutDirection::Ltr;
const RTL: LayoutDirection = LayoutDirection::Rtl;
const NOT_CLIPPED: &str = "popups aren't clipped to the window (PopupProperties.clippingEnabled)";

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentTopStart
#[test]
fn popup_correct_position_alignment_top_start() {
    assert_eq!(popup_position(Alignment::TOP_START, LTR), IntOffset::new(10, 10));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentTopStart_rtl
#[test]
fn popup_correct_position_alignment_top_start_rtl() {
    assert_eq!(popup_position(Alignment::TOP_START, RTL), IntOffset::new(50, 10));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentTopCenter
#[test]
fn popup_correct_position_alignment_top_center() {
    assert_eq!(popup_position(Alignment::TOP_CENTER, LTR), IntOffset::new(40, 10));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentTopEnd
#[test]
fn popup_correct_position_alignment_top_end() {
    assert_eq!(popup_position(Alignment::TOP_END, LTR), IntOffset::new(70, 10));
}

/// At x -10, clipped to the window's left on Android.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentTopEnd_rtl
#[test]
#[ignore = "popups aren't clipped to the window (PopupProperties.clippingEnabled)"]
fn popup_correct_position_alignment_top_end_rtl() {
    assert_eq!(popup_position(Alignment::TOP_END, RTL), IntOffset::new(0, 10), "{NOT_CLIPPED}");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentCenterEnd
#[test]
fn popup_correct_position_alignment_center_end() {
    assert_eq!(popup_position(Alignment::CENTER_END, LTR), IntOffset::new(70, 50));
}

/// At x -10, clipped to the window's left on Android.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentCenterEnd_rtl
#[test]
#[ignore = "popups aren't clipped to the window (PopupProperties.clippingEnabled)"]
fn popup_correct_position_alignment_center_end_rtl() {
    assert_eq!(popup_position(Alignment::CENTER_END, RTL), IntOffset::new(0, 50), "{NOT_CLIPPED}");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentBottomEnd
#[test]
fn popup_correct_position_alignment_bottom_end() {
    assert_eq!(popup_position(Alignment::BOTTOM_END, LTR), IntOffset::new(70, 90));
}

/// At x -10, clipped to the window's left on Android.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentBottomEnd_rtl
#[test]
#[ignore = "popups aren't clipped to the window (PopupProperties.clippingEnabled)"]
fn popup_correct_position_alignment_bottom_end_rtl() {
    assert_eq!(popup_position(Alignment::BOTTOM_END, RTL), IntOffset::new(0, 90), "{NOT_CLIPPED}");
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentBottomCenter
#[test]
fn popup_correct_position_alignment_bottom_center() {
    assert_eq!(popup_position(Alignment::BOTTOM_CENTER, LTR), IntOffset::new(40, 90));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentBottomCenter_rtl
#[test]
fn popup_correct_position_alignment_bottom_center_rtl() {
    assert_eq!(popup_position(Alignment::BOTTOM_CENTER, RTL), IntOffset::new(20, 90));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentBottomStart
#[test]
fn popup_correct_position_alignment_bottom_start() {
    assert_eq!(popup_position(Alignment::BOTTOM_START, LTR), IntOffset::new(10, 90));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentBottomStart_rtl
#[test]
fn popup_correct_position_alignment_bottom_start_rtl() {
    assert_eq!(popup_position(Alignment::BOTTOM_START, RTL), IntOffset::new(50, 90));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentCenterStart
#[test]
fn popup_correct_position_alignment_center_start() {
    assert_eq!(popup_position(Alignment::CENTER_START, LTR), IntOffset::new(10, 50));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentCenterStart_rtl
#[test]
fn popup_correct_position_alignment_center_start_rtl() {
    assert_eq!(popup_position(Alignment::CENTER_START, RTL), IntOffset::new(50, 50));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentCenter
#[test]
fn popup_correct_position_alignment_center() {
    assert_eq!(popup_position(Alignment::CENTER, LTR), IntOffset::new(40, 50));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/window/PopupAlignmentTest.kt, popup_correctPosition_alignmentCenter_rtl
#[test]
fn popup_correct_position_alignment_center_rtl() {
    assert_eq!(popup_position(Alignment::CENTER, RTL), IntOffset::new(20, 50));
}
