//! Compose's `LookaheadScopeTest` scenes with foundation's layouts in them: rows, columns, spacers.
//! Density 1.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Constraints, IntSize, LookaheadScope, UnitExt};

fn dp(pixels: i32) -> compose_ui::Dp {
    pixels.dp()
}

fn in_a_scope(content: impl Fn(LookaheadScope) + 'static) {
    LookaheadScope(callback(content));
}

/// The tests' `animateSize`, its animation over at once: the size ahead, measured as it is.
fn animate_size() -> Modifier {
    Modifier.approach_layout(
        |_| false,
        |_, _| false,
        |scope, content, _| {
            let ahead = scope.lookahead_size();
            let placeable = scope.measure(content, Constraints::fixed(ahead.width, ahead.height));
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        },
    )
}

fn noting_size(size: &Rc<Cell<IntSize>>) -> Modifier {
    let size = size.clone();
    Modifier.on_size_changed(move |it: IntSize| size.set(it))
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, lookaheadLayoutAnimation
#[test]
fn children_by_weight_follow_their_row_s_size_ahead() {
    let large = mutable_state_of(true);
    let (first, second) = (Rc::new(Cell::new(IntSize::ZERO)), Rc::new(Cell::new(IntSize::ZERO)));
    let (l, a, b) = (large.handle(), first.clone(), second.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (a, b) = (a.clone(), b.clone());
        in_a_scope(move |_| {
            let (a, b) = (a.clone(), b.clone());
            let size =
                if l.get() { Modifier.size(dp(200)) } else { Modifier.size((dp(50), dp(100))) };
            Row!(modifier = size, move |scope| {
                let child = |weight: f32, noted: &Rc<Cell<IntSize>>| {
                    Modifier
                        .fill_max_height()
                        .weight(scope, weight)
                        .then(noting_size(noted))
                        .then(animate_size())
                };
                Box!(modifier = child(2.0, &a), {});
                Box!(modifier = child(3.0, &b), {});
            });
        });
    });
    ui.wait_for_idle();
    assert_eq!((first.get(), second.get()), (IntSize::new(80, 200), IntSize::new(120, 200)));
    large.set(false);
    ui.wait_for_idle();
    assert_eq!((first.get(), second.get()), (IntSize::new(20, 100), IntSize::new(30, 100)));
    large.set(true);
    ui.wait_for_idle();
    assert_eq!((first.get(), second.get()), (IntSize::new(80, 200), IntSize::new(120, 200)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/LookaheadScopeTest.kt, parentObserveActualMeasurementTest
#[test]
fn what_s_around_a_scope_measures_with_what_s_now() {
    let (width, height) = (200, 120);
    let tenths = mutable_state_of(1);
    let (parent, grandparent, great) = (
        Rc::new(Cell::new(IntSize::ZERO)),
        Rc::new(Cell::new(IntSize::ZERO)),
        Rc::new(Cell::new(IntSize::ZERO)),
    );
    let (t, p, g, gg) = (tenths.handle(), parent.clone(), grandparent.clone(), great.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (p, g, gg) = (p.clone(), g.clone(), gg.clone());
        Column!(modifier = noting_size(&gg), |_| {
            let (p, g) = (p.clone(), g.clone());
            Row!(modifier = noting_size(&g), |_| {
                let p = p.clone();
                Box!(modifier = noting_size(&p), {
                    in_a_scope(move |_| {
                        let approaching = Modifier.approach_layout(
                            move |_| t.get() <= 30,
                            |_, _| false,
                            move |scope, content, constraints| {
                                assert_eq!(scope.lookahead_size(), IntSize::new(width, height));
                                let placeable = scope.measure(content, constraints);
                                let scale = t.get() as f32 / 10.0;
                                let (w, h) = (
                                    (scale * width as f32).round() as i32,
                                    (scale * height as f32).round() as i32,
                                );
                                scope.layout_with(w, h, move |placement| {
                                    placement.place(&placeable, 0, 0);
                                })
                            },
                        );
                        Box!(modifier = approaching.size((dp(width), dp(height))), {});
                    });
                });
                Spacer(Modifier.size(dp(20)));
            });
            Spacer(Modifier.size(dp(50)));
        });
    });
    for _ in 0..20 {
        ui.wait_for_idle();
        let scale = tenths.get() as f32 / 10.0;
        let size = IntSize::new(
            (width as f32 * scale).round() as i32,
            (height as f32 * scale).round() as i32,
        );
        assert_eq!(parent.get(), size);
        assert_eq!(grandparent.get(), IntSize::new(size.width + 20, size.height.max(20)));
        assert_eq!(
            great.get(),
            IntSize::new(grandparent.get().width.max(50), grandparent.get().height + 50)
        );
        tenths.set(tenths.get() + 1);
    }
}
