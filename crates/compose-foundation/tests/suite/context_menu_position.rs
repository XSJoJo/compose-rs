//! The dropdown context menu's state and where its popup goes (D2034), from Compose's
//! `ContextMenuStateTest` and `ContextMenuPopupPositionProviderTest`.

use std::rc::Rc;

use compose_foundation::context_menu::*;
use compose_ui::{IntOffset, IntRect, IntSize, LayoutDirection, Offset};

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuStateTest.kt, whenStatusOpen_unspecifiedOffset_throws
#[test]
#[should_panic(expected = "unspecified offset")]
fn an_open_status_needs_an_offset() {
    ContextMenuStatus::open(Offset::new(f32::NAN, f32::NAN));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuStateTest.kt, whenStatusOpen_equalsNull_false
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuStateTest.kt, whenStatusOpen_equalsOtherType_false
/// Rust's equality takes no null nor another type: an open state isn't a closed one.
#[test]
fn an_open_state_is_not_a_closed_one() {
    let open = ContextMenuState::new(ContextMenuStatus::open(Offset::ZERO));
    assert!(open != ContextMenuState::new(ContextMenuStatus::Closed));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuStateTest.kt, whenStatusOpen_equalsSame_true
#[test]
fn a_state_equals_itself() {
    let open = ContextMenuState::new(ContextMenuStatus::open(Offset::ZERO));
    assert!(open == open);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuStateTest.kt, whenStatusOpen_equalsOther_withDifferentValue_false
#[test]
fn states_open_elsewhere_differ() {
    let zero = ContextMenuState::new(ContextMenuStatus::open(Offset::ZERO));
    let one = ContextMenuState::new(ContextMenuStatus::open(Offset::new(1.0, 1.0)));
    assert!(zero != one);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuStateTest.kt, whenStatusOpen_equalsOther_withSameValue_true
#[test]
fn states_open_at_one_place_are_equal() {
    let one = ContextMenuState::new(ContextMenuStatus::open(Offset::new(1.0, 1.0)));
    let other = ContextMenuState::new(ContextMenuStatus::open(Offset::new(1.0, 1.0)));
    assert!(one == other);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuStateTest.kt, whenClose_statusIsClosed
#[test]
fn close_closes() {
    let state = ContextMenuState::new(ContextMenuStatus::open(Offset::ZERO));
    assert!(matches!(state.status(), ContextMenuStatus::Open(_)));
    state.close();
    assert_eq!(state.status(), ContextMenuStatus::Closed);
}

const WINDOW: IntSize = IntSize::new(100, 100);
const ANCHOR: IntRect = IntRect::new(10, 20, 90, 80);
/// 50, 50 in the window.
const POSITION: IntOffset = IntOffset::new(40, 30);

fn calculate(direction: LayoutDirection, popup: IntSize, expected: IntOffset) {
    let told = Rc::new(std::cell::Cell::new(None));
    let t = told.clone();
    let subject = ContextMenuPopupPositionProvider::at(
        POSITION,
        Some(Rc::new(move |position, bounds| t.set(Some((position, bounds))))),
    );
    let actual = subject.calculate_position(ANCHOR, WINDOW, direction, popup);
    assert_eq!(actual, expected);
    let expected_bounds =
        IntRect::new(expected.x, expected.y, expected.x + popup.width, expected.y + popup.height);
    assert_eq!(told.get(), Some((POSITION, expected_bounds)));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, calculatePosition_onlyFitsInXDirection
#[test]
fn fitting_only_across() {
    calculate(LayoutDirection::Ltr, IntSize::new(10, 200), IntOffset::new(50, 0));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, calculatePosition_onlyFitsInYDirection
#[test]
fn fitting_only_down() {
    calculate(LayoutDirection::Ltr, IntSize::new(200, 10), IntOffset::new(0, 50));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, calculatePosition_windowIsAccountedFor_ltr
#[test]
fn in_the_window_left_to_right() {
    calculate(LayoutDirection::Ltr, IntSize::new(10, 10), IntOffset::new(50, 50));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, calculatePosition_windowIsAccountedFor_rtl
#[test]
fn in_the_window_right_to_left() {
    calculate(LayoutDirection::Rtl, IntSize::new(10, 10), IntOffset::new(40, 50));
}

fn align(position: i32, popup: i32, close: bool, expected: i32) {
    assert_eq!(align_popup_axis(position, popup, 100, close), expected);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_closeAffinity_popupLargerThanWindow
#[test]
fn close_affinity_larger_than_the_window() {
    align(50, 200, true, 0);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_closeAffinity_popupFitsNeitherSide
#[test]
fn close_affinity_fitting_neither_side() {
    align(50, 75, true, 25);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_closeAffinity_popupFitsCloseSide
#[test]
fn close_affinity_fitting_the_close_side() {
    align(90, 30, true, 60);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_closeAffinity_popupFitsFarSide
#[test]
fn close_affinity_fitting_the_far_side() {
    align(20, 30, true, 20);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_closeAffinity_popupFitsEitherSide
#[test]
fn close_affinity_fitting_either_side() {
    align(50, 10, true, 50);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_farAffinity_popupLargerThanWindow
#[test]
fn far_affinity_larger_than_the_window() {
    align(50, 200, false, -100);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_farAffinity_popupFitsNeitherSide
#[test]
fn far_affinity_fitting_neither_side() {
    align(50, 75, false, 0);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_farAffinity_popupFitsCloseSide
#[test]
fn far_affinity_fitting_the_close_side() {
    align(90, 30, false, 60);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_farAffinity_popupFitsFarSide
#[test]
fn far_affinity_fitting_the_far_side() {
    align(20, 30, false, 20);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_farAffinity_popupFitsEitherSide
#[test]
fn far_affinity_fitting_either_side() {
    align(50, 10, false, 40);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_popupBarelyFitsInAfterSpace
#[test]
fn barely_fitting_after() {
    align(74, 25, true, 74);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_popupBarelyDoesNotFitInAfterSpace
#[test]
fn barely_not_fitting_after() {
    align(75, 25, true, 50);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_popupBarelyFitsInBeforeSpace
#[test]
fn barely_fitting_before() {
    align(25, 25, false, 0);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuPopupPositionProviderTest.kt, alignPopupAxis_popupBarelyDoesNotFitInBeforeSpace
#[test]
fn barely_not_fitting_before() {
    align(24, 25, false, 24);
}
