//! Compose's `VelocityTest.kt` (ui-unit): `Velocity`'s arithmetic, components and copies. Copies
//! are struct update syntax.

use compose_ui::Velocity;

const FIRST: Velocity = Velocity::new(3.0, -7.0);
const SECOND: Velocity = Velocity::new(5.0, 13.0);

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, operatorUnaryMinus
#[test]
fn negation_flips_both_components() {
    assert_eq!(-FIRST, Velocity::new(-3.0, 7.0));
    assert_eq!(-SECOND, Velocity::new(-5.0, -13.0));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, operatorPlus
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, operatorMinus
#[test]
fn velocities_add_and_subtract() {
    assert_eq!(SECOND + FIRST, Velocity::new(8.0, 6.0));
    assert_eq!(FIRST + SECOND, Velocity::new(8.0, 6.0));
    assert_eq!(FIRST - SECOND, Velocity::new(-2.0, -20.0));
    assert_eq!(SECOND - FIRST, Velocity::new(2.0, 20.0));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, operatorDivide
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, operatorTimes
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, operatorRem
#[test]
fn velocities_scale_divide_and_wrap() {
    assert_eq!(FIRST / 10.0, Velocity::new(0.3, -0.7));
    assert_eq!(FIRST * 10.0, Velocity::new(30.0, -70.0));
    assert_eq!(FIRST % 3.0, Velocity::new(0.0, -1.0));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, components
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, xy
#[test]
fn a_velocity_keeps_and_destructures_its_components() {
    let Velocity { x, y } = FIRST;
    assert_eq!((x, y), (3.0, -7.0));
    assert_eq!((FIRST.x, FIRST.y), (3.0, -7.0));
}

// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, testOffsetCopy
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, testOffsetCopyOverwriteX
// Compose: compose/ui/ui-unit/src/commonTest/kotlin/androidx/compose/ui/unit/VelocityTest.kt, testOffsetCopyOverwriteY
#[test]
fn a_copy_changes_only_what_it_names() {
    let velocity = Velocity::new(100.0, 200.0);
    assert_eq!(Velocity { ..velocity }, velocity);
    let copy = Velocity { x: 50.0, ..velocity };
    assert_eq!((copy.x, copy.y), (50.0, 200.0));
    let copy = Velocity { y: 300.0, ..velocity };
    assert_eq!((copy.x, copy.y), (100.0, 300.0));
}

// Not one of Compose's tests: `toString`.
#[test]
fn a_velocity_prints_its_unit() {
    assert_eq!(FIRST.to_string(), "(3.0, -7.0) px/sec");
}
