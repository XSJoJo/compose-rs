//! Compose's `ConstraintsTest.kt` (ui, host tests): construction, bounds, zero, constraining,
//! satisfaction, offsets, copies, error messages and validity. compose-rs keeps the four values in
//! `i32` fields where Compose packs them in a `Long`, so the limits of that packing (the tests of
//! `fitPrioritizingWidth` and `fitPrioritizingHeight` cutting values down, and the sizes too large
//! to pack) don't apply: every value Compose can hold, compose-rs holds the same.

use compose_ui::{Constraints, IntSize};

const INFINITY: i32 = Constraints::INFINITY;

fn assert_values(
    constraints: Constraints,
    min_width: i32,
    max_width: i32,
    min_height: i32,
    max_height: i32,
) {
    assert_eq!(
        (
            constraints.min_width,
            constraints.max_width,
            constraints.min_height,
            constraints.max_height
        ),
        (min_width, max_width, min_height, max_height)
    );
}

fn panic_message(block: impl FnOnce()) -> Option<String> {
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(block)).err()?;
    payload
        .downcast_ref::<&str>()
        .map(|message| message.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, constructor
#[test]
fn the_constructors() {
    assert_values(Constraints::default(), 0, INFINITY, 0, INFINITY);
    assert_values(Constraints::new(0, 1, 2, 3), 0, 1, 2, 3);
    assert_values(Constraints::fixed_width(5), 5, 5, 0, INFINITY);
    assert_values(Constraints::fixed_height(5), 0, INFINITY, 5, 5);
    assert_values(Constraints::fixed(5, 7), 5, 5, 7, 7);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, retrieveSimpleValues
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, retrieveValueMinFocusWidth
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, retrieveValueMinFocusHeight
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, retrieveValueMaxFocusWidth
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, retrieveValueMaxFocusHeight
#[test]
fn constraints_keep_every_value_compose_packs() {
    let cases = [
        (0, INFINITY, 0, INFINITY),
        (0, 0, 0, 0),
        (1, 64000, 2, 32000),
        (64000, INFINITY, 32000, INFINITY),
        (0xFFFE, INFINITY, 0x7FFE, INFINITY),
        (0, 0xFFFE, 0, 0x7FFE),
        (1, 32000, 2, 64000),
        (32000, 32001, 64000, 64001),
        (32000, INFINITY, 64000, INFINITY),
        (0x7FFE, INFINITY, 0xFFFE, INFINITY),
        (0, 0x7FFE, 0, 0xFFFE),
        (1, 250000, 2, 8000),
        (250000, 250001, 8000, 8001),
        (250000, INFINITY, 8000, INFINITY),
        (0x3FFFE, INFINITY, 0x1FFE, INFINITY),
        (0, 0x3FFFE, 0, 0x1FFE),
        (1, 8000, 2, 250000),
        (8000, 8001, 250000, 250001),
        (8000, INFINITY, 250000, INFINITY),
        (0x1FFE, INFINITY, 0x3FFFE, INFINITY),
        (0, 0x1FFE, 0, 0x3FFFE),
    ];
    for (min_width, max_width, min_height, max_height) in cases {
        let constraints = Constraints::new(min_width, max_width, min_height, max_height);
        assert_values(constraints, min_width, max_width, min_height, max_height);
    }
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, hasBoundedDimensions
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, hasFixedDimensions
#[test]
fn bounded_and_fixed_dimensions() {
    let unbounded = Constraints::new(3, INFINITY, 3, INFINITY);
    assert!(!unbounded.has_bounded_width());
    assert!(!unbounded.has_bounded_height());
    let bounded = Constraints::new(3, 5, 3, 5);
    assert!(bounded.has_bounded_width());
    assert!(bounded.has_bounded_height());

    let loose = Constraints::new(3, 4, 8, 9);
    assert!(!loose.has_fixed_width());
    assert!(!loose.has_fixed_height());
    let tight = Constraints::new(3, 3, 5, 5);
    assert!(tight.has_fixed_width());
    assert!(tight.has_fixed_height());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, isZero
#[test]
fn a_zero_maximum_makes_constraints_zero() {
    assert!(!Constraints::new(1, 2, 1, 2).is_zero());
    assert!(Constraints::new(0, 0, 0, 0).is_zero());
    assert!(Constraints::new(0, 0, 1, 2).is_zero());
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, constrain_constraints
#[test]
fn constraining_constraints_clamps_each_value() {
    let constraints = Constraints::new(5, 10, 5, 10);
    assert_values(Constraints::new(4, 11, 4, 11).constrain_by(constraints), 5, 10, 5, 10);
    assert_values(Constraints::new(7, 9, 7, 9).constrain_by(constraints), 7, 9, 7, 9);
    assert_values(Constraints::new(2, 3, 2, 3).constrain_by(constraints), 3, 3, 3, 3);
    assert_values(Constraints::new(10, 11, 10, 11).constrain_by(constraints), 10, 10, 10, 10);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, constrain_size
#[test]
fn constraining_a_size_clamps_it() {
    let constraints = Constraints::new(2, 5, 2, 5);
    assert_eq!(constraints.constrain(IntSize::new(1, 1)), IntSize::new(2, 2));
    assert_eq!(constraints.constrain(IntSize::new(3, 3)), IntSize::new(3, 3));
    assert_eq!(constraints.constrain(IntSize::new(7, 7)), IntSize::new(5, 5));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, satisfiedBy
#[test]
fn a_size_in_range_satisfies_constraints() {
    let constraints = Constraints::new(2, 5, 7, 9);
    assert!(constraints.is_satisfied_by(IntSize::new(4, 8)));
    assert!(constraints.is_satisfied_by(IntSize::new(2, 7)));
    assert!(constraints.is_satisfied_by(IntSize::new(5, 9)));
    assert!(!constraints.is_satisfied_by(IntSize::new(1, 8)));
    assert!(!constraints.is_satisfied_by(IntSize::new(7, 8)));
    assert!(!constraints.is_satisfied_by(IntSize::new(4, 5)));
    assert!(!constraints.is_satisfied_by(IntSize::new(4, 11)));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, offset
#[test]
fn an_offset_moves_every_value_and_stops_at_zero() {
    let constraints = Constraints::new(2, 2, 5, 5);
    assert_values(constraints.offset(2, 3), 4, 4, 8, 8);
    assert_values(constraints.offset(-7, -7), 0, 0, 0, 0);
}

// Compose's `copy` with named arguments is `Constraints::new` with the values kept spelled out
// (it checks as `copy` does), or struct update syntax (which doesn't check).
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, copy
#[test]
fn a_copy_changes_only_what_it_names() {
    let constraints = Constraints::default();
    assert_eq!(Constraints { ..constraints }, constraints);
    let maxima_zero = Constraints::new(constraints.min_width, 0, constraints.min_height, 0);
    assert_eq!(maxima_zero, Constraints::new(0, 0, 0, 0));
    assert_eq!(Constraints { max_width: 0, max_height: 0, ..constraints }, maxima_zero);
    assert_eq!(Constraints::new(1, 2, 3, 4), Constraints::new(1, 2, 3, 4));
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, errorMessage
#[test]
fn invalid_constraints_say_why() {
    let invalid = "maxWidth(3) must be >= than minWidth(5),\n\
                   maxHeight(2147483647) must be >= than minHeight(0),\n\
                   minWidth(5) and minHeight(0) must be >= 0";
    assert_eq!(panic_message(|| _ = Constraints::new(5, 3, 0, INFINITY)).as_deref(), Some(invalid));
    let constraints = Constraints::new(5, 10, 0, INFINITY);
    let copy = || {
        _ = Constraints::new(
            constraints.min_width,
            3,
            constraints.min_height,
            constraints.max_height,
        )
    };
    assert_eq!(panic_message(copy).as_deref(), Some(invalid));
    assert_eq!(
        panic_message(|| _ = Constraints::fixed(-1, 5)).as_deref(),
        Some("width(-1) and height(5) must be >= 0")
    );
    assert_eq!(
        panic_message(|| _ = Constraints::fixed_width(-1)).as_deref(),
        Some("width(-1) must be >= 0")
    );
    assert_eq!(
        panic_message(|| _ = Constraints::fixed_height(-1)).as_deref(),
        Some("height(-1) must be >= 0")
    );
}

// The cases that are rules of the type (a maximum under its minimum, a negative value); the rest
// of Compose's are sizes its packing can't hold, which `i32` fields hold.
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, validity
#[test]
fn reversed_or_negative_values_are_invalid() {
    let cases = [
        (3, 2, 0, INFINITY),
        (0, INFINITY, 3, 2),
        (-1, INFINITY, 0, INFINITY),
        (0, -1, 0, INFINITY),
        (0, INFINITY, -1, INFINITY),
        (0, INFINITY, 0, -1),
    ];
    for (min_width, max_width, min_height, max_height) in cases {
        let made = || _ = Constraints::new(min_width, max_width, min_height, max_height);
        assert!(panic_message(made).is_some(), "{min_width} {max_width} {min_height} {max_height}");
    }
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, restrictedConstraints_noRestriction
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, restrictedConstraints_bounds
#[test]
fn fitting_keeps_values_that_fit() {
    let max_focus = (1 << 18) - 2;
    let max_non_focus = (1 << 13) - 2;
    let min_focus = (1 << 16) - 2;
    let min_non_focus = (1 << 15) - 2;
    let cases = [
        (0, INFINITY, 0, INFINITY),
        (0, INFINITY, 250_000, 250_001),
        (250_000, 250_001, 0, INFINITY),
        (30_000, 30_001, 60_000, 60_001),
        (60_000, 60_001, 30_000, 30_001),
        (30_000, INFINITY, 60_000, INFINITY),
        (60_000, INFINITY, 30_000, INFINITY),
        (max_focus, max_focus, max_non_focus, max_non_focus),
        (max_focus, INFINITY, max_non_focus, INFINITY),
        (max_non_focus, max_non_focus, max_focus, max_focus),
        (max_non_focus, INFINITY, max_focus, INFINITY),
        (min_focus, min_focus, min_non_focus, min_non_focus),
        (min_focus, INFINITY, min_non_focus, INFINITY),
        (min_non_focus, min_non_focus, min_focus, min_focus),
        (min_non_focus, INFINITY, min_focus, INFINITY),
    ];
    for (min_width, max_width, min_height, max_height) in cases {
        let by_width =
            Constraints::fit_prioritizing_width(min_width, max_width, min_height, max_height);
        assert_values(by_width, min_width, max_width, min_height, max_height);
        let by_height =
            Constraints::fit_prioritizing_height(min_width, max_width, min_height, max_height);
        assert_values(by_height, min_width, max_width, min_height, max_height);
    }
}

// Compose's test reads the maxima of a packed value; these are the same maxima.
// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/layout/ConstraintsTest.kt, testCopyMaxDimensions
#[test]
fn copying_the_maxima_drops_the_minima() {
    let constraints = Constraints::new(10, 200, 30, 4000);
    assert_eq!(constraints.copy_max_dimensions(), Constraints::new(0, 200, 0, 4000));
}
