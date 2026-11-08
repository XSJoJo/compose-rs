//! Compose's `RowColumnTest`: `Row` and `Column` sizes and positions, weights, roundings,
//! alignments, arrangements, constraints and right-to-left layouts, through the `LayoutTest`
//! helpers of [`layout_test`](crate::layout_test) (a phone's density, a root of 1000 by 1600).
//! Its intrinsics tests are in `intrinsics.rs`, its `alignBy` ones in `alignment_line.rs`.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_ui::{Alignment, Dp, LayoutCoordinates};

use crate::layout_test::*;

/// `content` in a container aligning it top-start, as most of these tests.
fn top_start(content: impl Fn() + 'static) {
    container(Modifier, Spec::aligned(Alignment::TOP_START), content);
}

fn width_into(width: &Rc<Cell<f32>>) -> Modifier {
    let width = width.clone();
    Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
        width.set(coordinates.size().width as f32)
    })
}

fn height_into(height: &Rc<Cell<f32>>) -> Modifier {
    let height = height.clone();
    Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
        height.set(coordinates.size().height as f32)
    })
}

// ---- Size and position --------------------------------------------------------------------

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow
#[test]
fn row() {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        top_start(move || {
            let p = p.clone();
            Row!(|_| {
                empty(p.at(0), Spec::sized(size_dp, size_dp));
                empty(p.at(1), Spec::sized(size_dp * 2.0, size_dp * 2.0));
            });
        });
    });
    let double = (to_px(size_dp) * 2.0).round() as i32;
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.size(1), self::size(double, double));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(size, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn
#[test]
fn column() {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        top_start(move || {
            let p = p.clone();
            Column!(|_| {
                empty(p.at(0), Spec::sized(size_dp, size_dp));
                empty(p.at(1), Spec::sized(size_dp * 2.0, size_dp * 2.0));
            });
        });
    });
    let double = (to_px(size_dp) * 2.0).round() as i32;
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.size(1), self::size(double, double));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(0, size));
}

/// Two children of `width` by `height` (the second `second_height`) in a row or column, with
/// `weight`s and `fill`.
fn weighted(
    vertical: bool,
    weights: [f32; 2],
    fill: bool,
    width: f32,
    height: f32,
    second_height: f32,
) -> Probe {
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        top_start(move || {
            let p = p.clone();
            let child = move |index: usize, modifier: Modifier| {
                let height = if index == 0 { height } else { second_height };
                empty(modifier.then(p.at(index)), Spec::sized(px(width), px(height)));
            };
            if vertical {
                Column!(|scope| {
                    child(0, Modifier.weight_with(scope, weights[0], fill));
                    child(1, Modifier.weight_with(scope, weights[1], fill));
                });
            } else {
                Row!(|scope| {
                    child(0, Modifier.weight_with(scope, weights[0], fill));
                    child(1, Modifier.weight_with(scope, weights[1], fill));
                });
            }
        });
    });
    probe
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withChildrenWithWeight
#[test]
fn row_with_children_with_weight() {
    let probe = weighted(false, [1.0, 2.0], true, 50.0, 80.0, 80.0);
    let (root, height) = (ROOT.width as f32, round(px(80.0)));
    assert_eq!(probe.size(0), size((root / 3.0).round() as i32, height));
    assert_eq!(probe.size(1), size((root * 2.0 / 3.0).round() as i32, height));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset((root / 3.0).round() as i32, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withChildrenWithWeightNonFilling
#[test]
fn row_with_children_with_weight_non_filling() {
    let probe = weighted(false, [1.0, 2.0], false, 50.0, 80.0, 160.0);
    let (width, height) = (round(px(50.0)), round(px(80.0)));
    assert_eq!(probe.size(0), size(width, height));
    assert_eq!(probe.size(1), size(width, round(px(80.0) * 2.0)));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(width, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withChildrenWithMaxValueWeight
#[test]
fn row_with_children_with_max_value_weight() {
    let probe = weighted(false, [f32::MAX, 1.0], true, 50.0, 80.0, 80.0);
    let height = round(px(80.0));
    assert_eq!(probe.size(0), size(ROOT.width, height));
    assert_eq!(probe.size(1), size(0, height));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(ROOT.width, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withChildrenWithPositiveInfinityWeight
#[test]
fn row_with_children_with_positive_infinity_weight() {
    let probe = weighted(false, [f32::INFINITY, 1.0], true, 50.0, 80.0, 80.0);
    let height = round(px(80.0));
    assert_eq!(probe.size(0), size(ROOT.width, height));
    assert_eq!(probe.size(1), size(0, height));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(ROOT.width, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withChildrenWithWeight
#[test]
fn column_with_children_with_weight() {
    let probe = weighted(true, [1.0, 2.0], true, 80.0, 50.0, 50.0);
    let (root, width) = (ROOT.height as f32, round(px(80.0)));
    assert_eq!(probe.size(0), size(width, (root / 3.0).round() as i32));
    assert_eq!(probe.size(1), size(width, (root * 2.0 / 3.0).round() as i32));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(0, (root / 3.0).round() as i32));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withChildrenWithWeightNonFilling
#[test]
fn column_with_children_with_weight_non_filling() {
    let probe = weighted(true, [1.0, 2.0], false, 80.0, 50.0, 50.0);
    let (width, height) = (round(px(80.0)), round(px(50.0)));
    assert_eq!(probe.size(0), size(width, height));
    assert_eq!(probe.size(1), size(width, height));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(0, height));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withChildrenWithMaxValueWeight
#[test]
fn column_with_children_with_max_value_weight() {
    let probe = weighted(true, [f32::MAX, 1.0], true, 80.0, 50.0, 50.0);
    let width = round(px(80.0));
    assert_eq!(probe.size(0), size(width, ROOT.height));
    assert_eq!(probe.size(1), size(width, 0));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(0, ROOT.height));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withChildrenWithPositiveInfinityWeight
#[test]
fn column_with_children_with_positive_infinity_weight() {
    let probe = weighted(true, [f32::INFINITY, 1.0], true, 80.0, 50.0, 50.0);
    let width = round(px(80.0));
    assert_eq!(probe.size(0), size(width, ROOT.height));
    assert_eq!(probe.size(1), size(width, 0));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(0, ROOT.height));
}

fn panics(f: impl FnOnce() -> Modifier) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_invalidWeight
#[test]
fn row_invalid_weight() {
    let scope = RowScope::__new();
    assert!(panics(|| Modifier.weight(scope, -1.0)));
    assert!(panics(|| Modifier.weight(scope, f32::NAN)));
    assert!(panics(|| Modifier.weight(scope, f32::NEG_INFINITY)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_invalidWeight
#[test]
fn column_invalid_weight() {
    let scope = ColumnScope::__new();
    assert!(panics(|| Modifier.weight(scope, -1.0)));
    assert!(panics(|| Modifier.weight(scope, f32::NAN)));
    assert!(panics(|| Modifier.weight(scope, f32::NEG_INFINITY)));
}

// ---- Roundings ----------------------------------------------------------------------------

/// A row (or column) wrapping its content, after `padding` pixels at its start (top), at most
/// `max` pixels long, of children with `weights`: its length, and their lengths and positions.
fn rounded(
    vertical: bool,
    max: f32,
    padding: f32,
    weights: &'static [f32],
) -> (f32, Vec<(f32, f32)>) {
    let length = Rc::new(Cell::new(0.0));
    let children: Vec<_> = weights.iter().map(|_| Rc::new(Cell::new((0.0, 0.0)))).collect();
    let (l, c) = (length.clone(), children.clone());
    let _ui = show(move || {
        let c = c.clone();
        let child = move |index: usize, modifier: Modifier| {
            let seen = c[index].clone();
            let modifier =
                modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
                    let (size, at) = (coordinates.size(), coordinates.position_in_root());
                    seen.set(if vertical {
                        (size.height as f32, at.y)
                    } else {
                        (size.width as f32, at.x)
                    });
                });
            empty(modifier, Spec::default());
        };
        let wrap = Modifier.wrap_content_size_with(Alignment::TOP_START, false);
        if vertical {
            let modifier = wrap
                .padding(PaddingValues { top: px(padding), ..PaddingValues::ZERO })
                .height_in(Dp(0.0), px(max))
                .then(height_into(&l));
            Column!(modifier = modifier, |scope| {
                for (index, &weight) in weights.iter().enumerate() {
                    child(index, Modifier.weight(scope, weight));
                }
            });
        } else {
            let modifier = wrap
                .padding(PaddingValues { start: px(padding), ..PaddingValues::ZERO })
                .width_in(Dp(0.0), px(max))
                .then(width_into(&l));
            Row!(modifier = modifier, |scope| {
                for (index, &weight) in weights.iter().enumerate() {
                    child(index, Modifier.weight(scope, weight));
                }
            });
        }
    });
    (length.get(), children.iter().map(|child| child.get()).collect())
}

fn assert_rounded(vertical: bool, max: f32, weights: &'static [f32]) {
    let (length, children) = rounded(vertical, max, 1.0, weights);
    assert_eq!(length, max);
    let mut at = 1.0;
    for &(child, position) in &children {
        assert_eq!(position, at);
        at += child;
    }
    assert_eq!(children.iter().map(|&(child, _)| child).sum::<f32>(), length);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_doesNotPlaceChildrenOutOfBounds_becauseOfRoundings
#[test]
fn row_does_not_place_children_out_of_bounds_because_of_roundings() {
    assert_rounded(false, 11.0, &[1.0, 1.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_isNotLargerThanItsChildren_becauseOfRoundings
#[test]
fn row_is_not_larger_than_its_children_because_of_roundings() {
    assert_rounded(false, 8.0, &[2.0, 2.0, 3.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_isNotLargetThanItsChildren_becauseOfRoundings
#[test]
fn column_is_not_larger_than_its_children_because_of_roundings() {
    assert_rounded(true, 8.0, &[1.0, 1.0, 1.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_doesNotPlaceChildrenOutOfBounds_becauseOfRoundings
#[test]
fn column_does_not_place_children_out_of_bounds_because_of_roundings() {
    assert_rounded(true, 11.0, &[1.0, 1.0]);
}
