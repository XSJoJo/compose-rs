//! Compose's `RootNodeLayoutTest` (ui, device tests): the root measures its content with no
//! minimum size and places it at the top left (docs/06-ui.md). The two tests of a `ComposeView`
//! in a `FrameLayout` (wrap content, match parent) are n/a: Android views.

use std::cell::Cell;
use std::rc::Rc;

use crate::support::*;

use compose_runtime::*;
use compose_ui::*;

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RootNodeLayoutTest.kt, rootMeasuresWithZeroMinConstraints
#[test]
fn root_measures_with_zero_min_constraints() {
    let seen = Rc::new(Cell::new(None));
    let s = seen.clone();
    let _host = host(move || {
        let s = s.clone();
        Layout(
            Modifier,
            measure_policy(move |scope, _, constraints| {
                s.set(Some(constraints));
                scope.layout(10, 10)
            }),
            callback(|| {}),
        );
    });
    let constraints = seen.get().expect("measured");
    assert_eq!((constraints.min_width, constraints.min_height), (0, 0));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/RootNodeLayoutTest.kt, rootPositionsInTheTopLeftCorner
#[test]
fn root_positions_in_the_top_left_corner() {
    let bounds = Rc::new(Cell::new(None));
    let b = bounds.clone();
    let _host = host(move || {
        let b = b.clone();
        Layout(
            Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
                b.set(Some(coordinates.bounds_in_root()))
            }),
            measure_policy(|scope, _, _| scope.layout(10, 10)),
            callback(|| {}),
        );
    });
    assert_eq!(bounds.get(), Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
}
