//! Compose's `PopupPositionProviderTest` (ui, host tests): where `AlignmentOffsetPositionProvider`
//! puts a 40 by 20 popup on a 100 by 100 anchor at (50, 50), offset by (10, 10), for each
//! alignment, left to right and right to left (the offset then goes left).

use compose_ui::{
    Alignment, AlignmentOffsetPositionProvider, IntOffset, IntSize, LayoutDirection,
    PopupPositionProvider, Rect,
};

fn calculate_position(alignment: Alignment, direction: LayoutDirection) -> IntOffset {
    let anchor = Rect::new(50.0, 50.0, 150.0, 150.0);
    let window = IntSize::new(1000, 1000);
    let popup = IntSize::new(40, 20);
    AlignmentOffsetPositionProvider { alignment, offset: IntOffset::new(10, 10) }
        .position(anchor, window, direction, popup)
}

const LTR: LayoutDirection = LayoutDirection::Ltr;
const RTL: LayoutDirection = LayoutDirection::Rtl;

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionTopStart
#[test]
fn position_top_start() {
    assert_eq!(calculate_position(Alignment::TOP_START, LTR), IntOffset::new(60, 60));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionTopStart_rtl
#[test]
fn position_top_start_rtl() {
    assert_eq!(calculate_position(Alignment::TOP_START, RTL), IntOffset::new(100, 60));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionTopCenter
#[test]
fn position_top_center() {
    assert_eq!(calculate_position(Alignment::TOP_CENTER, LTR), IntOffset::new(90, 60));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionTopCenter_rtl
#[test]
fn position_top_center_rtl() {
    assert_eq!(calculate_position(Alignment::TOP_CENTER, RTL), IntOffset::new(70, 60));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionTopEnd
#[test]
fn position_top_end() {
    assert_eq!(calculate_position(Alignment::TOP_END, LTR), IntOffset::new(120, 60));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionTopEnd_rtl
#[test]
fn position_top_end_rtl() {
    assert_eq!(calculate_position(Alignment::TOP_END, RTL), IntOffset::new(40, 60));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionCenterEnd
#[test]
fn position_center_end() {
    assert_eq!(calculate_position(Alignment::CENTER_END, LTR), IntOffset::new(120, 100));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionCenterEnd_rtl
#[test]
fn position_center_end_rtl() {
    assert_eq!(calculate_position(Alignment::CENTER_END, RTL), IntOffset::new(40, 100));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionBottomEnd
#[test]
fn position_bottom_end() {
    assert_eq!(calculate_position(Alignment::BOTTOM_END, LTR), IntOffset::new(120, 140));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionBottomEnd_rtl
#[test]
fn position_bottom_end_rtl() {
    assert_eq!(calculate_position(Alignment::BOTTOM_END, RTL), IntOffset::new(40, 140));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionBottomCenter
#[test]
fn position_bottom_center() {
    assert_eq!(calculate_position(Alignment::BOTTOM_CENTER, LTR), IntOffset::new(90, 140));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionBottomCenter_rtl
#[test]
fn position_bottom_center_rtl() {
    assert_eq!(calculate_position(Alignment::BOTTOM_CENTER, RTL), IntOffset::new(70, 140));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionBottomStart
#[test]
fn position_bottom_start() {
    assert_eq!(calculate_position(Alignment::BOTTOM_START, LTR), IntOffset::new(60, 140));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionBottomStart_rtl
#[test]
fn position_bottom_start_rtl() {
    assert_eq!(calculate_position(Alignment::BOTTOM_START, RTL), IntOffset::new(100, 140));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionCenterStart
#[test]
fn position_center_start() {
    assert_eq!(calculate_position(Alignment::CENTER_START, LTR), IntOffset::new(60, 100));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionCenterStart_rtl
#[test]
fn position_center_start_rtl() {
    assert_eq!(calculate_position(Alignment::CENTER_START, RTL), IntOffset::new(100, 100));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionCenter
#[test]
fn position_center() {
    assert_eq!(calculate_position(Alignment::CENTER, LTR), IntOffset::new(90, 100));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/window/PopupPositionProviderTest.kt, positionCenter_rtl
#[test]
fn position_center_rtl() {
    assert_eq!(calculate_position(Alignment::CENTER, RTL), IntOffset::new(70, 100));
}
