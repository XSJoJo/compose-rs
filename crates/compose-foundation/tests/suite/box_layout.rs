//! Compose's `BoxTest`: `Box` alignments, `match_parent_size` (D1929), right to left,
//! `propagate_min_constraints`, through the `LayoutTest` helpers of
//! [`layout_test`](crate::layout_test). Its intrinsics tests are in `intrinsics.rs`; its two
//! inspector tests are n/a (compose-rs has no inspector values).

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_ui::{
    Alignment, Dp, IntSize, LOCAL_LAYOUT_DIRECTION, Layout, LayoutCoordinates, LayoutDirection,
    MeasurePolicy, measure_policy,
};

use crate::layout_test::*;

fn top_start(content: impl Fn() + 'static) {
    container(Modifier, Spec::aligned(Alignment::TOP_START), content);
}

fn size_into(size: &Rc<Cell<IntSize>>) -> Modifier {
    let size = size.clone();
    Modifier
        .on_globally_positioned(move |coordinates: LayoutCoordinates| size.set(coordinates.size()))
}

fn cell() -> Rc<Cell<IntSize>> {
    Rc::new(Cell::new(IntSize::new(-1, -1)))
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox
#[test]
fn box_() {
    let size_dp = px(50.0);
    let size = round(size_dp);
    let (probe, stack) = (Probe::new(2), cell());
    let (p, s) = (probe.clone(), stack.clone());
    let _ui = show(move || {
        let (p, s) = (p.clone(), s.clone());
        top_start(move || {
            let (p, s) = (p.clone(), s.clone());
            Box!(|scope| {
                empty(
                    Modifier.align(scope, Alignment::BOTTOM_END).then(p.at(0)).then(size_into(&s)),
                    Spec::sized(size_dp, size_dp),
                );
                empty(
                    Modifier.match_parent_size(scope).padding(px(10.0)).then(p.at(1)),
                    Spec::default(),
                );
            });
        });
    });
    assert_eq!(stack.get(), self::size(size, size));
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.size(1), self::size(30, 30));
    assert_eq!(probe.position(1), offset(10, 10));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_withMultipleAlignedChildren
#[test]
fn box_with_multiple_aligned_children() {
    let size = 200;
    let size_dp = px(size as f32);
    let double = round(size_dp * 2.0);
    let (probe, stack) = (Probe::new(2), cell());
    let (p, s) = (probe.clone(), stack.clone());
    let _ui = show(move || {
        let (p, s) = (p.clone(), s.clone());
        top_start(move || {
            let p = p.clone();
            Box!(modifier = size_into(&s), |scope| {
                let end = Modifier.align(scope, Alignment::BOTTOM_END);
                empty(end.clone().then(p.at(0)), Spec::sized(size_dp, size_dp));
                empty(end.then(p.at(1)), Spec::sized(size_dp * 2.0, size_dp * 2.0));
            });
        });
    });
    assert_eq!(stack.get(), self::size(double, double));
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(size, size));
    assert_eq!(probe.size(1), self::size(double, double));
    assert_eq!(probe.position(1), offset(0, 0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_withStretchChildren
#[test]
fn box_with_stretch_children() {
    let (size, inset) = (250, 50);
    let (size_dp, inset_dp) = (px(size as f32), px(inset as f32));
    let half = Spec::sized(size_dp / 2.0, size_dp / 2.0);
    let (probe, stack) = (Probe::new(5), cell());
    let (p, s) = (probe.clone(), stack.clone());
    let _ui = show(move || {
        let (p, s) = (p.clone(), s.clone());
        top_start(move || {
            let p = p.clone();
            Box!(modifier = size_into(&s), |scope| {
                empty(
                    Modifier.align(scope, Alignment::CENTER).then(p.at(0)),
                    Spec::sized(size_dp, size_dp),
                );
                let zero = PaddingValues::ZERO;
                let paddings = [
                    PaddingValues { start: inset_dp, top: inset_dp, ..zero },
                    PaddingValues { end: inset_dp, bottom: inset_dp, ..zero },
                    PaddingValues { start: inset_dp, end: inset_dp, ..zero },
                    PaddingValues { top: inset_dp, bottom: inset_dp, ..zero },
                ];
                for (index, padding) in paddings.into_iter().enumerate() {
                    let modifier = Modifier.match_parent_size(scope).padding(padding);
                    empty(modifier.then(p.at(index + 1)), half);
                }
            });
        });
    });
    assert_eq!(stack.get(), self::size(size, size));
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.size(1), self::size(size - inset, size - inset));
    assert_eq!(probe.position(1), offset(inset, inset));
    assert_eq!(probe.size(2), self::size(size - inset, size - inset));
    assert_eq!(probe.position(2), offset(0, 0));
    assert_eq!(probe.size(3), self::size(size - inset * 2, size));
    assert_eq!(probe.position(3), offset(inset, 0));
    assert_eq!(probe.size(4), self::size(size, size - inset * 2));
    assert_eq!(probe.position(4), offset(0, inset));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_Rtl
#[test]
fn box_rtl() {
    let size_dp = px(48.0);
    let size = round(size_dp);
    let triple = round(size_dp * 3.0);
    let (probe, stack) = (Probe::new(9), cell());
    let (p, s) = (probe.clone(), stack.clone());
    let _ui = show(move || {
        let (p, s) = (p.clone(), s.clone());
        Box!(modifier = Modifier.wrap_content_size_with(Alignment::TOP_START, false), |_| {
            let (p, s) = (p.clone(), s.clone());
            let rtl = LOCAL_LAYOUT_DIRECTION.provides(LayoutDirection::Rtl);
            CompositionLocalProvider(rtl, move || {
                let p = p.clone();
                Box!(modifier = Modifier.size(size_dp * 3.0).then(size_into(&s)), |scope| {
                    let alignments = [
                        Alignment::TOP_START,
                        Alignment::TOP_CENTER,
                        Alignment::TOP_END,
                        Alignment::CENTER_START,
                        Alignment::CENTER,
                        Alignment::CENTER_END,
                        Alignment::BOTTOM_START,
                        Alignment::BOTTOM_CENTER,
                        Alignment::BOTTOM_END,
                    ];
                    for (index, alignment) in alignments.into_iter().enumerate() {
                        let modifier = Modifier.align(scope, alignment).size(size_dp);
                        Box!(modifier = modifier.then(p.at(index)));
                    }
                });
            });
        });
    });
    assert_eq!(stack.get(), self::size(triple, triple));
    let expected = [
        (size * 2, 0),
        (size, 0),
        (0, 0),
        (size * 2, size),
        (size, size),
        (0, size),
        (size * 2, size * 2),
        (size, size * 2),
        (0, size * 2),
    ];
    for (index, (x, y)) in expected.into_iter().enumerate() {
        assert_eq!(probe.position(index), offset(x, y), "child {index}");
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_expanded
#[test]
fn box_expanded() {
    let (size, half) = (250, 125);
    let (size_dp, half_dp) = (px(size as f32), px(half as f32));
    let (probe, stack) = (Probe::new(2), cell());
    let (p, s) = (probe.clone(), stack.clone());
    let _ui = show(move || {
        let (p, s) = (p.clone(), s.clone());
        top_start(move || {
            let p = p.clone();
            container(Modifier.size(size_dp).then(size_into(&s)), Spec::default(), move || {
                let p = p.clone();
                Box!(|scope| {
                    empty(Modifier.fill_max_size().then(p.at(0)), Spec::default());
                    empty(
                        Modifier.align(scope, Alignment::BOTTOM_END).then(p.at(1)),
                        Spec::sized(half_dp, half_dp),
                    );
                });
            });
        });
    });
    assert_eq!(stack.get(), self::size(size, size));
    assert_eq!(probe.size(0), self::size(size, size));
    assert_eq!(probe.position(0), offset(0, 0));
    assert_eq!(probe.size(1), self::size(half, half));
    assert_eq!(probe.position(1), offset(size - half, size - half));
}

/// The position in its parent of a 10 px box in a 50 px box.
fn inner_position(content: impl Fn(Modifier) + 'static) -> (f32, f32) {
    let at = Rc::new(Cell::new((-1.0, -1.0)));
    let a = at.clone();
    let _ui = show(move || {
        let a = a.clone();
        content(Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
            let position = coordinates.position_in_parent();
            a.set((position.x, position.y))
        }));
    });
    at.get()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_alignmentParameter
#[test]
fn box_alignment_parameter() {
    let at = inner_position(|seen| {
        Box!(
            content_alignment = Alignment::BOTTOM_END,
            modifier = Modifier.required_size(px(50.0)),
            |_| Box!(modifier = Modifier.required_size(px(10.0)).then(seen.clone()))
        );
    });
    assert_eq!(at, (40.0, 40.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_outermostGravityWins
#[test]
fn box_outermost_gravity_wins() {
    let at = inner_position(|seen| {
        Box!(modifier = Modifier.required_size(px(10.0)), |scope| {
            Box!(
                modifier = Modifier
                    .align(scope, Alignment::BOTTOM_END)
                    .align(scope, Alignment::TOP_START)
                    .then(seen.clone())
            );
        });
    });
    // Compose's inner box has no size: aligned bottom-end in 10 px, it's at (10, 10).
    assert_eq!(at, (10.0, 10.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_childAffectsBoxSize
#[test]
fn box_child_affects_box_size() {
    let size = mutable_state_of(Dp(10.0));
    let (measured, placed) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let state = *size;
    let (m, pl) = (measured.clone(), placed.clone());
    let mut ui = show(move || {
        let (m, pl) = (m.clone(), pl.clone());
        Box!(|_| {
            let (m, pl) = (m.clone(), pl.clone());
            let policy = remember(move || {
                let m = m.clone();
                measure_policy(move |scope, measurables, constraints| {
                    let placeable = scope.measure(measurables[0], constraints);
                    m.set(m.get() + 1);
                    scope.place(&placeable, 0, 0);
                    scope.layout(placeable.width, placeable.height)
                }) as Rc<dyn MeasurePolicy>
            });
            let placed = Modifier.on_placed(move |_| pl.set(pl.get() + 1));
            Layout(
                placed,
                policy,
                callback(move || {
                    Box!(|_| {
                        Box!(modifier = Modifier.required_size((state.get(), Dp(10.0))));
                    });
                }),
            );
        });
    });
    assert_eq!((measured.get(), placed.get()), (1, 1));
    size.set(Dp(20.0));
    ui.wait_for_idle();
    assert_eq!((measured.get(), placed.get()), (2, 2));
}

/// D1929: a lone `match_parent_size` child is measured at the box's minimums, as Compose's.
#[test]
fn a_lone_match_parent_size_child_takes_the_minimums() {
    let child = cell();
    let c = child.clone();
    let _ui = show(move || {
        let c = c.clone();
        let bounds = compose_ui::SizeSpec::bounds(px(50.0), px(30.0), px(100.0), px(100.0));
        Box!(modifier = Modifier.size_in(bounds), |scope| {
            Box!(modifier = Modifier.match_parent_size(scope).then(size_into(&c)));
        });
    });
    assert_eq!(child.get(), size(50, 30));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_canPropagateMinConstraints
#[test]
fn box_can_propagate_min_constraints() {
    let width = Rc::new(Cell::new(-1));
    let w = width.clone();
    let _ui = show(move || {
        let w = w.clone();
        Box!(
            modifier = Modifier.required_width_in(Dp(20.0), Dp(40.0)),
            propagate_min_constraints = true,
            |_| {
                let w = w.clone();
                Box!(
                    modifier = Modifier
                        .width(Dp(10.0))
                        .on_size_changed(move |size: IntSize| w.set(size.width))
                );
            }
        );
    });
    assert_eq!(width.get(), round(Dp(20.0)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/BoxTest.kt, testBox_tracksPropagateMinConstraintsChanges
#[test]
fn box_tracks_propagate_min_constraints_changes() {
    let propagate = mutable_state_of(true);
    let state = *propagate;
    let widths = Rc::new(std::cell::RefCell::new(Vec::new()));
    let w = widths.clone();
    let mut ui = show(move || {
        let w = w.clone();
        Box!(
            modifier = Modifier.required_width_in(Dp(20.0), Dp(40.0)),
            propagate_min_constraints = state.get(),
            content_alignment = Alignment::CENTER,
            |_| {
                let w = w.clone();
                Box!(
                    modifier = Modifier.width(Dp(10.0)).on_size_changed(move |size: IntSize| {
                        w.borrow_mut().push(size.width);
                    })
                );
            }
        );
    });
    assert_eq!(*widths.borrow(), [round(Dp(20.0))]);
    // Compose's test turns it off from the first size's callback.
    propagate.set(false);
    ui.wait_for_idle();
    assert_eq!(*widths.borrow(), [round(Dp(20.0)), round(Dp(10.0))]);
}
