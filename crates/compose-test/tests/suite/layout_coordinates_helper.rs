//! Compose's `LayoutCoordinatesHelperTest`: `local_position_of` between a parent and a child,
//! `on_placed` following an alignment, a layout modifier's placement after its `on_placed`, an
//! `on_placed` added without a measure, again on reuse, and on an empty box.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Alignment, Constraints, Density, IntOffset, IntSize, LOCAL_DENSITY, LayoutCoordinates,
    Modifier, Offset, UnitExt,
};

type Kept = Rc<Cell<Option<LayoutCoordinates>>>;

fn keeping(kept: &Kept) -> Modifier {
    let kept = kept.clone();
    Modifier
        .on_globally_positioned(move |coordinates: LayoutCoordinates| kept.set(Some(coordinates)))
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/LayoutCoordinatesHelperTest.kt, positionInParent_noOffset
#[test]
fn position_in_parent_no_offset() {
    let (parent, child): (Kept, Kept) = (Rc::default(), Rc::default());
    let (p, c) = (parent.clone(), child.clone());
    let mut ui = compose_test(move || {
        let c = c.clone();
        Column!(modifier = keeping(&p), horizontal_alignment = Alignment::START, {
            Box!(modifier = Modifier.size(10.dp()).then(keeping(&c)));
        });
    });
    ui.wait_for_idle();
    let (parent, child) = (parent.get().unwrap(), child.get().unwrap());
    assert_eq!(parent.local_position_of(&child, Offset::ZERO), Offset::ZERO);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/LayoutCoordinatesHelperTest.kt, positionInParent_centered
#[test]
fn position_in_parent_centered() {
    let (parent, child): (Kept, Kept) = (Rc::default(), Rc::default());
    let (p, c) = (parent.clone(), child.clone());
    // Density 1: px are dp.
    let mut ui = compose_test(move || {
        let (p, c) = (p.clone(), c.clone());
        Box!(modifier = Modifier.width(40.dp()), content_alignment = Alignment::CENTER, {
            let c = c.clone();
            Column!(
                modifier = Modifier.width(20.dp()).then(keeping(&p)),
                horizontal_alignment = Alignment::CENTER_HORIZONTALLY,
                {
                    Box!(modifier = Modifier.size(10.dp()).then(keeping(&c)));
                }
            );
        });
    });
    ui.wait_for_idle();
    let (parent, child) = (parent.get().unwrap(), child.get().unwrap());
    assert_eq!(parent.local_position_of(&child, Offset::ZERO), Offset::new(5.0, 0.0));
}

/// Where `coordinates` are placed in their parent, as the test's `placementInParent`.
fn placement_in_parent(coordinates: &LayoutCoordinates) -> IntOffset {
    let position =
        coordinates.parent_coordinates().unwrap().local_position_of(coordinates, Offset::ZERO);
    IntOffset::new(position.x.round() as i32, position.y.round() as i32)
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/LayoutCoordinatesHelperTest.kt, onPlaced_alignmentChange
#[test]
fn on_placed_alignment_change() {
    let alignment = mutable_state_of(Alignment::CENTER);
    let handle = alignment.handle();
    let offset = Rc::new(Cell::new(IntOffset::new(0, 0)));
    let position_in_root = Rc::new(Cell::new(Offset::ZERO));
    let (o, r) = (offset.clone(), position_in_root.clone());
    let mut ui = compose_test(move || {
        let (o, r) = (o.clone(), r.clone());
        CompositionLocalProvider(LOCAL_DENSITY.provides(Density::new(1.0)), move || {
            let (o, r) = (o.clone(), r.clone());
            Box!(modifier = Modifier.offset(200.dp(), 100.dp()).size(300.dp()), |scope| {
                let (o, r) = (o.clone(), r.clone());
                Box!(
                    modifier = Modifier
                        .on_placed(move |coordinates: LayoutCoordinates| {
                            o.set(placement_in_parent(&coordinates));
                            r.set(coordinates.position_in_root());
                        })
                        .align(scope, handle.get())
                        .size(100.dp())
                );
            });
        });
    });
    let parent_offset = Offset::new(200.0, 100.0);
    for (next, expected) in [
        (Alignment::TOP_START, IntOffset::new(100, 100)),
        (Alignment::BOTTOM_END, IntOffset::new(0, 0)),
    ] {
        ui.run_on_idle(|_| {
            assert_eq!(offset.get(), expected);
            let moved = Offset::new(expected.x as f32, expected.y as f32);
            assert_eq!(position_in_root.get(), parent_offset + moved);
            alignment.set(next);
        });
    }
    ui.run_on_idle(|_| {
        assert_eq!(offset.get(), IntOffset::new(200, 200));
        assert_eq!(position_in_root.get(), parent_offset + Offset::new(200.0, 200.0));
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/LayoutCoordinatesHelperTest.kt, onPlacedModifierWithLayoutModifier
/// A node both placed-listening and a layout: its coordinates are told before its placement
/// block runs (Compose's one object is the two modifiers here, the `on_placed` first).
#[test]
#[ignore = "on_placed is told after the layout pass (D1186), not before the placement block of the node it's on: the block sees no coordinates yet"]
fn on_placed_modifier_with_layout_modifier() {
    let coordinates: Kept = Rc::default();
    let told = coordinates.clone();
    let checked = Rc::new(Cell::new(false));
    let placed = checked.clone();
    let mut ui = compose_test(move || {
        let (told, seen, placed) = (told.clone(), told.clone(), placed.clone());
        Box!(
            modifier = Modifier
                .fill_max_size()
                .on_placed(move |coordinates: LayoutCoordinates| told.set(Some(coordinates)))
                .layout(move |scope, measurable, _| {
                    let placeable = scope.measure(measurable, Constraints::fixed(50, 50));
                    let (seen, placed) = (seen.clone(), placed.clone());
                    scope.layout_with(50, 50, move |placement| {
                        // The coordinates are told by the time this runs.
                        assert_eq!(seen.get().map(|it| it.size()), Some(IntSize::new(50, 50)));
                        placed.set(true);
                        placement.place(&placeable, 0, 0);
                    })
                })
        );
    });
    ui.run_on_idle(|_| {
        assert_eq!(coordinates.get().map(|it| it.size()), Some(IntSize::new(50, 50)));
        assert!(checked.get());
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/LayoutCoordinatesHelperTest.kt, addingOnPlacedModifierDoesntRequireRemeasure
#[test]
#[ignore = "an on_placed node that comes lays out its layout node again with a measure (D1186), where Compose's places it again without measuring"]
fn adding_on_placed_modifier_doesnt_require_remeasure() {
    let size = 10;
    let actual = Rc::new(Cell::new(i32::MAX));
    let remeasures = Rc::new(Cell::new(0));
    let with_on_placed = mutable_state_of(false);
    let handle = with_on_placed.handle();
    let (counted, told) = (remeasures.clone(), actual.clone());
    // Made once, outside the content, as Compose's `layoutModifier`: the same element each
    // composition.
    let layout = Modifier.layout(move |scope, measurable, _| {
        let placeable = scope.measure(measurable, Constraints::fixed(size, size));
        counted.set(counted.get() + 1);
        scope.place(&placeable, 0, 0);
        scope.layout(placeable.width, placeable.height)
    });
    let mut ui = compose_test(move || {
        let told = told.clone();
        let on_placed = if handle.get() {
            Modifier
                .on_placed(move |coordinates: LayoutCoordinates| told.set(coordinates.size().width))
        } else {
            Modifier
        };
        Box!(modifier = layout.clone().then(on_placed));
    });
    ui.run_on_idle(|_| {
        remeasures.set(0);
        with_on_placed.set(true);
    });
    ui.run_on_idle(|_| {
        assert_eq!(actual.get(), size);
        assert_eq!(remeasures.get(), 0);
    });
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/LayoutCoordinatesHelperTest.kt, onPlacedIsReExecutedOnReuse
#[test]
fn on_placed_is_re_executed_on_reuse() {
    let size = 10;
    let actual = Rc::new(Cell::new(i32::MAX));
    let reuse_key = mutable_state_of(0);
    let handle = reuse_key.handle();
    let told = actual.clone();
    let mut ui = compose_test(move || {
        let told = told.clone();
        ReusableContent(handle.get(), move || {
            let told = told.clone();
            Box!(
                modifier =
                    Modifier
                        .size((size as f32).dp())
                        .on_placed(move |coordinates: LayoutCoordinates| told
                            .set(coordinates.size().width))
            );
        });
    });
    ui.run_on_idle(|_| {
        actual.set(i32::MAX);
        reuse_key.update(|key| key + 1);
    });
    ui.run_on_idle(|_| assert_eq!(actual.get(), size));
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/LayoutCoordinatesHelperTest.kt, onBoxPlaced_failing
#[test]
fn on_box_placed_failing() {
    let coordinates: Kept = Rc::default();
    let told = coordinates.clone();
    let mut ui = compose_test(move || {
        let told = told.clone();
        Box!(modifier = Modifier.on_placed(move |it: LayoutCoordinates| told.set(Some(it))));
    });
    ui.run_on_idle(|_| assert!(coordinates.get().is_some()));
}
