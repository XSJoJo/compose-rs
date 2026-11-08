//! Compose's `ClipPointerInputTest` (ui, device tests): a clip on a node or around it stops
//! pointers outside the clip from reaching what's inside, for bounds, rounded rects, generic
//! shapes and an outline smaller than the node. Touches are sent to the host as Kotlin's
//! `MotionEvent`s are dispatched to the view: a down each, at window positions (density 1, so a
//! pixel is a dp).
//!
//! Not ported here: the three `interactionBarrier()` tests (Compose's internal modifier, which
//! compose-rs doesn't have).

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::*;
use compose_test::*;
use compose_ui::{
    CustomShape, Density, IntSize, Layout, LayoutDirection, Offset, Outline, PointerEvent,
    PointerEventPass, PointerId, PointerType, Rect, Shape, Size, UnitExt, measure_policy,
};

type Log = Rc<RefCell<Vec<Offset>>>;

/// As the Kotlin test's `LoggingPim`: the position of each change it hears in the initial pass.
fn logging(log: &Log) -> Modifier {
    let log = log.clone();
    Modifier.pointer_input((), move |scope| {
        let log = log.clone();
        async move {
            loop {
                let event = scope.await_pointer_event(PointerEventPass::Initial).await;
                for change in &event.changes {
                    log.borrow_mut().push(change.position);
                }
            }
        }
    })
}

/// As the Kotlin test's `Child`: 2 by 2 pixels.
fn Child(modifier: Modifier) {
    Layout!(
        modifier = modifier,
        measure_policy = measure_policy(|scope, _, _| scope.layout(2, 2)),
        {}
    );
}

/// A touch down at each of `positions` in the window, one after the other.
fn touch_down_at(ui: &mut ComposeTestRule, positions: &[Offset]) {
    for &position in positions {
        let down = PointerEvent::press(position).with_pointer(PointerId(0), PointerType::Touch);
        ui.host().send_pointer(down);
    }
    ui.wait_for_idle();
}

fn logs() -> [Log; 4] {
    [Log::default(), Log::default(), Log::default(), Log::default()]
}

const HIT: [Offset; 4] =
    [Offset::new(1.0, 1.0), Offset::new(3.0, 1.0), Offset::new(1.0, 3.0), Offset::new(3.0, 3.0)];
const MISS: [Offset; 8] = [
    Offset::new(1.0, 0.0),
    Offset::new(3.0, 0.0),
    Offset::new(0.0, 1.0),
    Offset::new(4.0, 1.0),
    Offset::new(0.0, 3.0),
    Offset::new(4.0, 3.0),
    Offset::new(1.0, 4.0),
    Offset::new(3.0, 4.0),
];

/// Each child heard one touch, the one just inside the clipped parent, at its own position.
fn assert_only_inside_hit(logs: &[Log; 4]) {
    let expected = [(1.0, 1.0), (0.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    for (log, (x, y)) in logs.iter().zip(expected) {
        assert_eq!(*log.borrow(), [Offset::new(x, y)]);
    }
}

/// Four 2-pixel children of a 3-pixel parent clipped to its bounds, placed one pixel out of
/// each of its corners by the parent's layout: only the touches inside the parent hit.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/pointer/ClipPointerInputTest.kt, clipToBounds_childrenOffsetViaLayout_onlyCorrectPointersHit
#[test]
fn clip_to_bounds_children_offset_via_layout_only_correct_pointers_hit() {
    let logs = logs();
    let given = logs.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        let given = given.clone();
        Layout!(
            measure_policy = measure_policy(|scope, measurables, constraints| {
                let middle = scope.measure(measurables[0], constraints);
                scope.place(&middle, 1, 1);
                scope.layout(constraints.max_width, constraints.max_height)
            }),
            {
                let given = given.clone();
                Layout!(
                    modifier = Modifier.clip_to_bounds(),
                    measure_policy = measure_policy(|scope, measurables, constraints| {
                        let at = [(-1, -1), (2, -1), (-1, 2), (2, 2)];
                        for (&measurable, (x, y)) in measurables.iter().zip(at) {
                            let child = scope.measure(measurable, constraints);
                            scope.place(&child, x, y);
                        }
                        scope.layout(3, 3)
                    }),
                    {
                        for log in &given {
                            Child(logging(log));
                        }
                    }
                );
            }
        );
    });
    touch_down_at(&mut ui, &[&HIT[..], &MISS[..]].concat());
    assert_only_inside_hit(&logs);
}

/// The same children offset by `offset` modifiers, all placed at the parent's origin.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/pointer/ClipPointerInputTest.kt, clipToBounds_childrenOffsetViaModifier_onlyCorrectPointersHit
#[test]
fn clip_to_bounds_children_offset_via_modifier_only_correct_pointers_hit() {
    let logs = logs();
    let given = logs.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        let given = given.clone();
        Layout!(
            measure_policy = measure_policy(|scope, measurables, constraints| {
                let middle = scope.measure(measurables[0], constraints);
                scope.place(&middle, 1, 1);
                scope.layout(constraints.max_width, constraints.max_height)
            }),
            {
                let given = given.clone();
                Layout!(
                    modifier = Modifier.clip_to_bounds(),
                    measure_policy = measure_policy(|scope, measurables, constraints| {
                        for &measurable in measurables {
                            let child = scope.measure(measurable, constraints);
                            scope.place(&child, 0, 0);
                        }
                        scope.layout(3, 3)
                    }),
                    {
                        let at = [(-1.0, -1.0), (2.0, -1.0), (-1.0, 2.0), (2.0, 2.0)];
                        for (log, (x, y)) in given.iter().zip(at) {
                            Child(Modifier.offset(x.dp(), y.dp()).then(logging(log)));
                        }
                    }
                );
            }
        );
    });
    touch_down_at(&mut ui, &[&HIT[..], &MISS[..]].concat());
    assert_only_inside_hit(&logs);
}

/// A 2-pixel child clipped to `shape`, a circle: touches inside it hit, those just outside
/// (its box's corners, and past its edges) don't.
fn poke_around_circle(shape: Shape) {
    let log = Log::default();
    let given = log.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        Child(Modifier.clip(shape.clone()).then(logging(&given)).layout(
            |scope, measurable, constraints| {
                let placeable = scope.measure(measurable, constraints);
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            },
        ));
    });
    let offset = 1.0 / 128.0;
    let (above0, below2, above2, below0) = (offset, 2.0 - offset, 2.0 + offset, -offset);
    let hit = [
        Offset::new(above0, 1.0),
        Offset::new(1.0, above0),
        Offset::new(1.0, below2),
        Offset::new(below2, 1.0),
        Offset::new(1.0, 1.0),
        Offset::new(0.5, 0.5),
        Offset::new(0.5, 1.5),
        Offset::new(1.5, 0.5),
        Offset::new(1.5, 1.5),
    ];
    let miss = [
        Offset::new(above0, above0),
        Offset::new(above0, below2),
        Offset::new(below2, above0),
        Offset::new(below2, below2),
        Offset::new(1.0, below0),
        Offset::new(above2, 1.0),
        Offset::new(below0, 1.0),
        Offset::new(1.0, above2),
    ];
    touch_down_at(&mut ui, &[&hit[..], &miss[..]].concat());
    assert_eq!(*log.borrow(), hit);
}

/// Clipped to a circle as a rounded rect of 50 % corners.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/pointer/ClipPointerInputTest.kt, clip_roundedRect
#[test]
fn clip_rounded_rect() {
    poke_around_circle(RoundedCornerShape(50));
}

/// Clipped to a circle as a rounded rect of corners larger than half its side.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/pointer/ClipPointerInputTest.kt, clip_roundedRectLargeCorner
#[test]
fn clip_rounded_rect_large_corner() {
    poke_around_circle(RoundedCornerShape(1.1f32));
}

/// Clipped to a circle as a generic shape's oval.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/pointer/ClipPointerInputTest.kt, clip_genericShape
#[test]
fn clip_generic_shape() {
    poke_around_circle(GenericShape(|path, size, _| {
        path.add_oval(Rect::new(0.0, 0.0, size.width, size.height));
    }));
}

/// A rectangle outline of the middle half of the box.
struct SmallRect;

impl CustomShape for SmallRect {
    fn create_outline(&self, size: Size, _: LayoutDirection, _: &Density) -> Outline {
        Outline::Rectangle(Rect::new(
            size.width * 0.25,
            size.height * 0.25,
            size.width * 0.75,
            size.height * 0.75,
        ))
    }

    fn same(&self, other: &dyn CustomShape) -> bool {
        other.downcast_ref::<SmallRect>().is_some()
    }
}

/// Clipped to a rectangle smaller than the node: only touches in it hit.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/input/pointer/ClipPointerInputTest.kt, clip_smallRect
#[test]
fn clip_small_rect() {
    let log = Log::default();
    let given = log.clone();
    let mut ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
        Child(Modifier.clip(Shape::custom(SmallRect)).then(logging(&given)).layout(
            |scope, measurable, constraints| {
                let placeable = scope.measure(measurable, constraints);
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            },
        ));
    });
    let offset = 1.0 / 128.0;
    let (just_in, just_out) = (1.5 - offset, 0.5 - offset);
    let hit = [
        Offset::new(0.5, 0.5),
        Offset::new(just_in, 0.5),
        Offset::new(0.5, just_in),
        Offset::new(just_in, just_in),
        Offset::new(1.0, 0.5),
        Offset::new(0.5, 1.0),
        Offset::new(1.0, just_in),
        Offset::new(just_in, 1.0),
        Offset::new(1.0, 1.0),
    ];
    let miss = [
        Offset::new(just_out, 0.5),
        Offset::new(0.5, just_out),
        Offset::new(1.5, 0.5),
        Offset::new(just_in, just_out),
        Offset::new(0.5, 1.5),
        Offset::new(just_out, just_in),
        Offset::new(just_in, 1.5),
        Offset::new(1.5, just_in),
    ];
    touch_down_at(&mut ui, &[&hit[..], &miss[..]].concat());
    assert_eq!(*log.borrow(), hit);
}
