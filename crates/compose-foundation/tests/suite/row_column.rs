//! Compose's `RowColumnTest`: `Row` and `Column` sizes and positions, weights, roundings,
//! alignments, arrangements, constraints and right-to-left layouts, through the `LayoutTest`
//! helpers of [`layout_test`](crate::layout_test) (a phone's density, a root of 1000 by 1600).
//! Its intrinsics tests are in `intrinsics.rs`, its `alignBy` ones in `alignment_line.rs`.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::callback;
use compose_ui::{
    Alignment, Constraints, Dp, DpSize, IntSize, IntrinsicSize, Layout, LayoutCoordinates,
    measure_policy,
};

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

// ---- Cross-axis alignment -----------------------------------------------------------------

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withColumnAlignModifier_usesCorrectCrossAxisSize
#[test]
fn row_with_column_align_modifier_uses_correct_cross_axis_size() {
    let (child_width, child_height, row_height) = (px(20.0), px(50.0), px(100.0));
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        Column!(|column| {
            let column_modifier = Modifier.align(column, Alignment::END);
            let p = p.clone();
            Row!(modifier = Modifier.height(row_height), |_| {
                Box!(
                    modifier = column_modifier
                        .clone()
                        .size(DpSize::new(child_width, child_height))
                        .then(p.at(0))
                );
            });
        });
    });
    let expected_y = (to_px(row_height) - to_px(child_height)).round() as i32;
    assert_eq!(probe.position(0), offset(0, expected_y));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withRowAlignModifier_usesCorrectCrossAxisSize
#[test]
fn column_with_row_align_modifier_uses_correct_cross_axis_size() {
    let (child_width, child_height, column_width) = (px(50.0), px(20.0), px(100.0));
    let probe = Probe::new(1);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        Row!(|row| {
            let row_modifier = Modifier.align(row, Alignment::BOTTOM);
            let p = p.clone();
            Column!(modifier = Modifier.width(column_width), |_| {
                Box!(
                    modifier = row_modifier
                        .clone()
                        .size(DpSize::new(child_width, child_height))
                        .then(p.at(0))
                );
            });
        });
    });
    let expected_x = (to_px(column_width) - to_px(child_width)).round() as i32;
    assert_eq!(probe.position(0), offset(expected_x, 0));
}

/// D1927: in a right-to-left column, a row's alignment isn't mirrored (Compose's
/// `Alignment.Vertical` takes no direction: `TOP` is on the left), the column's own horizontal
/// one is (its start on the right).
#[test]
fn a_row_alignment_in_a_right_to_left_column_is_not_mirrored() {
    let (child, column_width) = (px(20.0), px(100.0));
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        let rtl = compose_ui::LOCAL_LAYOUT_DIRECTION.provides(compose_ui::LayoutDirection::Rtl);
        compose_runtime::CompositionLocalProvider(rtl, move || {
            let p = p.clone();
            Row!(|row| {
                let top = Modifier.align(row, Alignment::TOP);
                let p = p.clone();
                Column!(modifier = Modifier.width(column_width), |_| {
                    Box!(modifier = top.clone().size(child).then(p.at(0)));
                    Box!(modifier = Modifier.size(child).then(p.at(1)));
                });
            });
        });
    });
    let end = round(column_width) - round(child);
    // The row wraps the column, at the root's origin.
    let column_x = 0;
    assert_eq!(probe.position(0).x as i32, column_x);
    assert_eq!(probe.position(1).x as i32, column_x + end);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withStretchCrossAxisAlignment
#[test]
fn row_with_stretch_cross_axis_alignment() {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        Row!(|_| {
            empty(Modifier.fill_max_height().then(p.at(0)), Spec::sized(size_dp, size_dp));
            empty(
                Modifier.fill_max_height().then(p.at(1)),
                Spec::sized(size_dp * 2.0, size_dp * 2.0),
            );
        });
    });
    assert_eq!(probe.size(0), self::size(size, ROOT.height));
    assert_eq!(probe.size(1), self::size((to_px(size_dp) * 2.0).round() as i32, ROOT.height));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(size, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withStretchCrossAxisAlignment
#[test]
fn column_with_stretch_cross_axis_alignment() {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        Column!(|_| {
            empty(Modifier.fill_max_width().then(p.at(0)), Spec::sized(size_dp, size_dp));
            empty(
                Modifier.fill_max_width().then(p.at(1)),
                Spec::sized(size_dp * 2.0, size_dp * 2.0),
            );
        });
    });
    assert_eq!(probe.size(0), self::size(ROOT.width, size));
    assert_eq!(probe.size(1), self::size(ROOT.width, round(size_dp * 2.0)));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(0, size));
}

/// Three children of 50 px in a row filling the root's height (`center` its alignment, else
/// top), aligned top, by the row (or centered), bottom.
fn row_gravities(centered_by_row: bool) {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let probe = Probe::new(3);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        let alignment = if centered_by_row { Alignment::CENTER_VERTICALLY } else { Alignment::TOP };
        Row!(modifier = Modifier.fill_max_height(), vertical_alignment = alignment, |row| {
            let spec = Spec::sized(size_dp, size_dp);
            empty(Modifier.align(row, Alignment::TOP).then(p.at(0)), spec);
            let middle = if centered_by_row {
                Modifier
            } else {
                Modifier.align(row, Alignment::CENTER_VERTICALLY)
            };
            empty(middle.then(p.at(1)), spec);
            empty(Modifier.align(row, Alignment::BOTTOM).then(p.at(2)), spec);
        });
    });
    let root = ROOT.height as f32;
    for index in 0..3 {
        assert_eq!(probe.size(index), self::size(size, size));
    }
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(size, ((root - size as f32) / 2.0).round() as i32));
    assert_eq!(probe.position(2), offset(size * 2, ROOT.height - size));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withGravityModifier_andGravityParameter
#[test]
fn row_with_gravity_modifier_and_gravity_parameter() {
    row_gravities(true);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withGravityModifier
#[test]
fn row_with_gravity_modifier() {
    row_gravities(false);
}

/// As [`row_gravities`], in a column filling the root's width.
fn column_gravities(centered_by_column: bool) {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let probe = Probe::new(3);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        let alignment =
            if centered_by_column { Alignment::CENTER_HORIZONTALLY } else { Alignment::START };
        Column!(modifier = Modifier.fill_max_width(), horizontal_alignment = alignment, |column| {
            let spec = Spec::sized(size_dp, size_dp);
            empty(Modifier.align(column, Alignment::START).then(p.at(0)), spec);
            let middle = if centered_by_column {
                Modifier
            } else {
                Modifier.align(column, Alignment::CENTER_HORIZONTALLY)
            };
            empty(middle.then(p.at(1)), spec);
            empty(Modifier.align(column, Alignment::END).then(p.at(2)), spec);
        });
    });
    let root = ROOT.width as f32;
    for index in 0..3 {
        assert_eq!(probe.size(index), self::size(size, size));
    }
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.position(1), offset(((root - size as f32) / 2.0).round() as i32, size));
    assert_eq!(probe.position(2), offset(ROOT.width - size, size * 2));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withGravityModifier
#[test]
fn column_with_gravity_modifier() {
    column_gravities(false);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withGravityModifier_andGravityParameter
#[test]
fn column_with_gravity_modifier_and_gravity_parameter() {
    column_gravities(true);
}

// ---- Sizes --------------------------------------------------------------------------------

/// The size of the layout `content` puts its modifier on.
fn measured(content: impl Fn(Modifier) + 'static) -> IntSize {
    let seen = Rc::new(Cell::new(IntSize::new(-1, -1)));
    let s = seen.clone();
    let _ui = show(move || {
        let s = s.clone();
        content(Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
            s.set(coordinates.size())
        }));
    });
    seen.get()
}

/// The two spacers of most size tests: 50 px and 100 px squares.
fn two_spacers() {
    let size_dp = px(50.0);
    Spacer(Modifier.size(DpSize::new(size_dp, size_dp)));
    Spacer(Modifier.size(DpSize::new(size_dp * 2.0, size_dp * 2.0)));
}

/// A row (or column) of `modifier` with [`two_spacers`], centered in the root.
fn centered(vertical: bool, modifier: Modifier) {
    center(move || {
        if vertical {
            Column!(modifier = modifier.clone(), |_| two_spacers());
        } else {
            Row!(modifier = modifier.clone(), |_| two_spacers());
        }
    });
}

/// As [`centered`], with a weighted 50 px container and a 100 px one.
fn centered_weighted(vertical: bool, modifier: Modifier) {
    center(move || {
        let size_dp = px(50.0);
        let children = move |weight: Modifier| {
            empty(weight, Spec::sized(size_dp, size_dp));
            empty(Modifier, Spec::sized(size_dp * 2.0, size_dp * 2.0));
        };
        if vertical {
            Column!(modifier = modifier.clone(), |scope| children(Modifier.weight(scope, 1.0)));
        } else {
            Row!(modifier = modifier.clone(), |scope| children(Modifier.weight(scope, 1.0)));
        }
    });
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_expandedWidth_withExpandedModifier
#[test]
fn row_expanded_width_with_expanded_modifier() {
    let size = measured(|seen| centered(false, Modifier.fill_max_width().then(seen)));
    assert_eq!(size.width, ROOT.width);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_wrappedWidth_withNoWeightChildren
#[test]
fn row_wrapped_width_with_no_weight_children() {
    let size = measured(|seen| centered(false, seen));
    assert_eq!(size.width, round(px(50.0) * 3.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_expandedWidth_withWeightChildren
#[test]
fn row_expanded_width_with_weight_children() {
    let size = measured(|seen| centered_weighted(false, seen));
    assert_eq!(size.width, ROOT.width);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withMaxCrossAxisSize
#[test]
fn row_with_max_cross_axis_size() {
    let size = measured(|seen| centered(false, Modifier.fill_max_height().then(seen)));
    assert_eq!(size.height, ROOT.height);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withMinCrossAxisSize
#[test]
fn row_with_min_cross_axis_size() {
    let size = measured(|seen| centered(false, seen));
    assert_eq!(size.height, round(px(50.0) * 2.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_expandedHeight_withExpandedModifier
#[test]
fn column_expanded_height_with_expanded_modifier() {
    let size = measured(|seen| centered(true, Modifier.fill_max_height().then(seen)));
    assert_eq!(size.height, ROOT.height);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_wrappedHeight_withNoChildrenWithWeight
#[test]
fn column_wrapped_height_with_no_children_with_weight() {
    let size = measured(|seen| centered(true, seen));
    assert_eq!(size.height, round(px(50.0) * 3.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_expandedHeight_withWeightChildren
#[test]
fn column_expanded_height_with_weight_children() {
    let size = measured(|seen| centered_weighted(true, seen));
    assert_eq!(size.height, ROOT.height);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withMaxCrossAxisSize
#[test]
fn column_with_max_cross_axis_size() {
    let size = measured(|seen| centered(true, Modifier.fill_max_width().then(seen)));
    assert_eq!(size.width, ROOT.width);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withMinCrossAxisSize
#[test]
fn column_with_min_cross_axis_size() {
    let size = measured(|seen| centered(true, seen));
    assert_eq!(size.width, round(px(50.0) * 2.0));
}

/// A row (or column) of `modifier` with [`two_spacers`] (or a weighted container and a
/// fixed one), in a `ConstrainedBox` of `constraints`, centered in the root.
fn constrained(vertical: bool, weighted: bool, constraints: DpConstraints, modifier: Modifier) {
    center(move || {
        let modifier = modifier.clone();
        constrained_box(constraints, Modifier, move || {
            if weighted {
                let size_dp = px(50.0);
                let children = move |weight: Modifier| {
                    empty(weight, Spec::sized(size_dp, size_dp));
                    empty(Modifier, Spec::sized(size_dp * 2.0, size_dp * 2.0));
                };
                if vertical {
                    Column!(modifier = modifier.clone(), |scope| children(
                        Modifier.weight(scope, 1.0)
                    ));
                } else {
                    Row!(modifier = modifier.clone(), |scope| children(
                        Modifier.weight(scope, 1.0)
                    ));
                }
            } else if vertical {
                Column!(modifier = modifier.clone(), |_| two_spacers());
            } else {
                Row!(modifier = modifier.clone(), |_| two_spacers());
            }
        });
    });
}

fn max_width(width: Dp) -> DpConstraints {
    DpConstraints { max_width: width, ..DpConstraints::default() }
}

fn min_width(width: Dp) -> DpConstraints {
    DpConstraints { min_width: width, ..DpConstraints::default() }
}

fn max_height(height: Dp) -> DpConstraints {
    DpConstraints { max_height: height, ..DpConstraints::default() }
}

fn min_height(height: Dp) -> DpConstraints {
    DpConstraints { min_height: height, ..DpConstraints::default() }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withExpandedModifier_respectsMaxWidthConstraint
#[test]
fn row_with_expanded_modifier_respects_max_width_constraint() {
    let width = px(250.0);
    let size = measured(move |seen| {
        constrained(false, false, max_width(width), Modifier.fill_max_width().then(seen))
    });
    assert_eq!(size.width, ROOT.width.min(round(width)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withChildrenWithWeight_respectsMaxWidthConstraint
#[test]
fn row_with_children_with_weight_respects_max_width_constraint() {
    let width = px(250.0);
    let size = measured(move |seen| constrained(false, true, max_width(width), seen));
    assert_eq!(size.width, ROOT.width.min(round(width)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withNoWeightChildren_respectsMinWidthConstraint
#[test]
fn row_with_no_weight_children_respects_min_width_constraint() {
    let width = px(250.0);
    let size = measured(move |seen| constrained(false, false, min_width(width), seen));
    assert_eq!(size.width, round(width));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withMaxCrossAxisSize_respectsMaxHeightConstraint
#[test]
fn row_with_max_cross_axis_size_respects_max_height_constraint() {
    let height = px(250.0);
    let size = measured(move |seen| {
        constrained(false, false, max_height(height), Modifier.fill_max_height().then(seen))
    });
    assert_eq!(size.height, ROOT.height.min(round(height)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withMinCrossAxisSize_respectsMinHeightConstraint
#[test]
fn row_with_min_cross_axis_size_respects_min_height_constraint() {
    let height = px(150.0);
    let size = measured(move |seen| constrained(false, false, min_height(height), seen));
    assert_eq!(size.height, round(height));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withExpandedModifier_respectsMaxHeightConstraint
#[test]
fn column_with_expanded_modifier_respects_max_height_constraint() {
    let height = px(250.0);
    let size = measured(move |seen| {
        constrained(true, false, max_height(height), Modifier.fill_max_height().then(seen))
    });
    assert_eq!(size.height, ROOT.height.min(round(height)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withWeightChildren_respectsMaxHeightConstraint
#[test]
fn column_with_weight_children_respects_max_height_constraint() {
    let height = px(250.0);
    let size = measured(move |seen| constrained(true, true, max_height(height), seen));
    assert_eq!(size.height, ROOT.height.min(round(height)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withChildren_respectsMinHeightConstraint
#[test]
fn column_with_children_respects_min_height_constraint() {
    let height = px(250.0);
    let size = measured(move |seen| constrained(true, false, min_height(height), seen));
    assert_eq!(size.height, round(height));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withMaxCrossAxisSize_respectsMaxWidthConstraint
#[test]
fn column_with_max_cross_axis_size_respects_max_width_constraint() {
    let width = px(250.0);
    let size = measured(move |seen| {
        constrained(true, false, max_width(width), Modifier.fill_max_width().then(seen))
    });
    assert_eq!(size.width, ROOT.width.min(round(width)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withMinCrossAxisSize_respectsMinWidthConstraint
#[test]
fn column_with_min_cross_axis_size_respects_min_width_constraint() {
    let width = px(150.0);
    let size = measured(move |seen| constrained(true, false, min_width(width), seen));
    assert_eq!(size.width, round(width));
}

/// A row (column) of `IntrinsicSize::Max` width (height) in a `ConstrainedBox` at least 250 px
/// long, of one weighted 50 px container: the row's and the child's sizes.
fn min_main_axis(vertical: bool) -> (IntSize, IntSize) {
    let length = px(250.0);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        center(move || {
            let p = p.clone();
            let constraints = if vertical { min_height(length) } else { min_width(length) };
            constrained_box(constraints, Modifier, move || {
                let p = p.clone();
                let size_dp = px(50.0);
                if vertical {
                    let modifier = Modifier.height(IntrinsicSize::Max).then(p.at(0));
                    Column!(modifier = modifier, |scope| {
                        empty(
                            Modifier.weight(scope, 1.0).then(p.at(1)),
                            Spec::sized(size_dp, size_dp),
                        );
                    });
                } else {
                    let modifier = Modifier.width(IntrinsicSize::Max).then(p.at(0));
                    Row!(modifier = modifier, |scope| {
                        empty(
                            Modifier.weight(scope, 1.0).then(p.at(1)),
                            Spec::sized(size_dp, size_dp),
                        );
                    });
                }
            });
        });
    });
    (probe.size(0), probe.size(1))
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_withMinMainAxisSize
#[test]
fn row_with_min_main_axis_size() {
    let (row, child) = min_main_axis(false);
    let expected = size(round(px(250.0)), round(px(50.0)));
    assert_eq!(row, expected);
    assert_eq!(child, expected);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_withMinMainAxisSize
#[test]
fn column_with_min_main_axis_size() {
    let (column, child) = min_main_axis(true);
    let expected = size(round(px(50.0)), round(px(250.0)));
    assert_eq!(column, expected);
    assert_eq!(child, expected);
}

/// Saves the constraints of what it's measured with into `into`, as a leaf of no size.
fn constraints_into(modifier: Modifier, into: &Rc<Cell<Option<Constraints>>>) {
    let into = into.clone();
    let policy = measure_policy(move |scope, _, constraints| {
        into.set(Some(constraints));
        scope.layout(0, 0)
    });
    Layout(modifier, policy, callback(|| {}));
}

/// A `BoxWithConstraints` saving its constraints into `into`, of a `FixedSizeLayout` of `width`
/// by `height`.
fn box_with_constraints_into(into: &Rc<Cell<Option<Constraints>>>, width: i32, height: i32) {
    let into = into.clone();
    BoxWithConstraints!(|scope: BoxWithConstraintsScope| {
        into.set(Some(scope.constraints));
        fixed_size_layout(width, height, Vec::new());
    });
}

fn seen() -> Rc<Cell<Option<Constraints>>> {
    Rc::new(Cell::new(None))
}

/// In infinite constraints, a `ConstrainedBox` at least 100 px long holding a row (column) of
/// two 30 px children and a weighted one: their constraints.
fn infinite(vertical: bool) -> [Option<Constraints>; 3] {
    let seen = [seen(), seen(), seen()];
    let s = seen.clone();
    let _ui = show(move || {
        let s = s.clone();
        with_infinite_constraints(move || {
            let s = s.clone();
            let (minimum, child) = (px(100.0), round(px(30.0)));
            let constraints = if vertical { min_height(minimum) } else { min_width(minimum) };
            constrained_box(constraints, Modifier, move || {
                let s = s.clone();
                if vertical {
                    Column!(|scope| {
                        box_with_constraints_into(&s[0], 0, child);
                        box_with_constraints_into(&s[1], 0, child);
                        constraints_into(Modifier.weight(scope, 1.0), &s[2]);
                    });
                } else {
                    Row!(|scope| {
                        box_with_constraints_into(&s[0], child, 0);
                        box_with_constraints_into(&s[1], child, 0);
                        constraints_into(Modifier.weight(scope, 1.0), &s[2]);
                    });
                }
            });
        });
    });
    [seen[0].get(), seen[1].get(), seen[2].get()]
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_measuresChildrenCorrectly_whenMeasuredWithInfiniteWidth
#[test]
fn row_measures_children_correctly_when_measured_with_infinite_width() {
    let [first, second, weighted] = infinite(false);
    assert_eq!(first, Some(Constraints::unbounded()));
    assert_eq!(second, Some(Constraints::unbounded()));
    let left = round(px(100.0)) - round(px(30.0)) * 2;
    let weighted = weighted.unwrap();
    assert_eq!((weighted.min_width, weighted.max_width), (left, left));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_measuresChildrenCorrectly_whenMeasuredWithInfiniteHeight
#[test]
fn column_measures_children_correctly_when_measured_with_infinite_height() {
    let [first, second, weighted] = infinite(true);
    assert_eq!(first, Some(Constraints::unbounded()));
    assert_eq!(second, Some(Constraints::unbounded()));
    let left = round(px(100.0)) - round(px(30.0)) * 2;
    let weighted = weighted.unwrap();
    assert_eq!((weighted.min_height, weighted.max_height), (left, left));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_protectsAgainstOverflow
#[test]
fn row_protects_against_overflow() {
    let placed = Rc::new(Cell::new(0));
    let first = seen();
    let (p, f) = (placed.clone(), first.clone());
    let _ui = show(move || {
        let (p, f) = (p.clone(), f.clone());
        with_infinite_constraints(move || {
            let (p, f) = (p.clone(), f.clone());
            constrained_box(min_width(px(0.0)), Modifier, move || {
                let (p, f) = (p.clone(), f.clone());
                Row!(horizontal_arrangement = Arrangement::spaced_by(Dp(0.5)), |scope| {
                    let (counted, f) = (p.clone(), f.clone());
                    let policy = measure_policy(move |scope, _, constraints| {
                        f.set(Some(constraints));
                        counted.set(counted.get() + 1);
                        scope.layout(1 << 23, 100)
                    });
                    Layout(Modifier, policy, callback(|| {}));
                    let counted = p.clone();
                    Box!(modifier = Modifier.weight_with(scope, 1.0, true), |_| {
                        counted.set(counted.get() + 1)
                    });
                    let counted = p.clone();
                    Box!(modifier = Modifier.weight_with(scope, 0.000_000_01, true), |_| {
                        counted.set(counted.get() + 1)
                    });
                });
            });
        });
    });
    assert_eq!(first.get(), Some(Constraints::unbounded()));
    assert!(placed.get() >= 3, "each child measured or composed");
}

/// In a box, a `ConstrainedBox` of exactly 100 by 200 px holding a row (column) of two
/// `BoxWithConstraints` of 50 by 100 px: their constraints.
fn no_weight_children(vertical: bool) -> [Option<Constraints>; 2] {
    let seen = [seen(), seen()];
    let s = seen.clone();
    let _ui = show(move || {
        let s = s.clone();
        Box!(|_| {
            let s = s.clone();
            let (width, height) = (px(100.0), px(200.0));
            constrained_box(DpConstraints::fixed(width, height), Modifier, move || {
                let s = s.clone();
                let (child_width, child_height) = (round(px(50.0)), round(px(100.0)));
                if vertical {
                    Column!(|_| {
                        box_with_constraints_into(&s[0], child_width, child_height);
                        box_with_constraints_into(&s[1], child_width, child_height);
                    });
                } else {
                    Row!(|_| {
                        box_with_constraints_into(&s[0], child_width, child_height);
                        box_with_constraints_into(&s[1], child_width, child_height);
                    });
                }
            });
        });
    });
    [seen[0].get(), seen[1].get()]
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_measuresNoWeightChildrenCorrectly
#[test]
fn row_measures_no_weight_children_correctly() {
    let [first, second] = no_weight_children(false);
    let (width, height, child) = (round(px(100.0)), round(px(200.0)), round(px(50.0)));
    assert_eq!(first, Some(Constraints::new(0, width, 0, height)));
    assert_eq!(second, Some(Constraints::new(0, width - child, 0, height)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_measuresNoWeightChildrenCorrectly
#[test]
fn column_measures_no_weight_children_correctly() {
    let [first, second] = no_weight_children(true);
    let (width, height, child) = (round(px(100.0)), round(px(200.0)), round(px(100.0)));
    assert_eq!(first, Some(Constraints::new(0, width, 0, height)));
    assert_eq!(second, Some(Constraints::new(0, width, 0, height - child)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_doesNotExpand_whenWeightChildrenDoNotFill
#[test]
fn row_does_not_expand_when_weight_children_do_not_fill() {
    let size = measured(|seen| {
        Row!(modifier = seen, |scope| {
            Box!(modifier = Modifier.weight_with(scope, 1.0, false).size(px(10.0)));
        });
    });
    assert_eq!(size.width, 10);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_doesNotExpand_whenWeightChildrenDoNotFill
#[test]
fn column_does_not_expand_when_weight_children_do_not_fill() {
    let size = measured(|seen| {
        Column!(modifier = seen, |scope| {
            Box!(modifier = Modifier.weight_with(scope, 1.0, false).size(px(10.0)));
        });
    });
    assert_eq!(size.height, 10);
}

/// A row 40 px at most (a column of 40 px) spaced by 8 px, of two weighted boxes: their
/// lengths and positions.
fn spaced_weights(vertical: bool) {
    let (length, space) = (40, 8);
    let probe = Probe::new(2);
    let p = probe.clone();
    let _ui = show(move || {
        let p = p.clone();
        let spacing = Arrangement::spaced_by(px(space as f32));
        if vertical {
            Column!(
                modifier = Modifier.height(px(length as f32)),
                vertical_arrangement = spacing,
                |scope| {
                    Box!(modifier = Modifier.weight(scope, 1.0).then(p.at(0)));
                    Box!(modifier = Modifier.weight(scope, 1.0).then(p.at(1)));
                }
            );
        } else {
            Row!(
                modifier = Modifier.width_in(Dp(0.0), px(length as f32)),
                horizontal_arrangement = spacing,
                |scope| {
                    Box!(modifier = Modifier.weight(scope, 1.0).then(p.at(0)));
                    Box!(modifier = Modifier.weight(scope, 1.0).then(p.at(1)));
                }
            );
        }
    });
    let each = (length - space) / 2;
    let main = |index: usize| {
        let (size, at) = (probe.size(index), probe.position(index));
        if vertical { (size.height, at.y as i32) } else { (size.width, at.x as i32) }
    };
    assert_eq!(main(0), (each, 0));
    assert_eq!(main(1), (each, each + space));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testRow_includesSpacing_withWeightChildren
#[test]
fn row_includes_spacing_with_weight_children() {
    spaced_weights(false);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/RowColumnTest.kt, testColumn_includesSpacing_withWeightChildren
#[test]
fn column_includes_spacing_with_weight_children() {
    spaced_weights(true);
}
