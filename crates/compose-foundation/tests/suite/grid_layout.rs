//! Compose's `GridTest.kt` (density 1: a dp is a pixel): track sizes fixed, by percentage,
//! flexible, by content and between a minimum and a weight; gaps; alignment in a cell; right to
//! left; spans; automatic, explicit and named placement; constraints.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::mutable_state_of;
use compose_test::*;
use compose_ui::{
    Alignment, IntSize, Layout, LayoutCoordinates, LayoutDirection, MeasurePolicy, Rect, UnitExt,
};

/// A child: what it emits given the grid's scope and the modifier recording its bounds.
type Child = Rc<dyn Fn(GridScope, Modifier)>;

/// A box with `modifier`.
fn boxed(modifier: impl Fn(GridScope) -> Modifier + 'static) -> Child {
    Rc::new(move |scope, record| Box!(modifier = modifier(scope).then(record)))
}

/// As the test's `IntrinsicItem`: at least `min` wide, reaching for `max_intrinsic`, its height
/// the minimum given, its intrinsic widths those given.
fn intrinsic(
    min: i32,
    min_intrinsic: i32,
    max_intrinsic: i32,
    modifier: impl Fn(GridScope) -> Modifier + 'static,
) -> Child {
    struct Policy(i32, i32, i32);
    impl MeasurePolicy for Policy {
        fn measure(
            &self,
            scope: &mut compose_ui::MeasureScope<'_>,
            _: &[compose_ui::Measurable],
            constraints: compose_ui::Constraints,
        ) -> compose_ui::MeasureResult {
            let width = constraints.constrain_width(self.2).max(self.0);
            scope.layout(width, constraints.min_height)
        }
        fn min_intrinsic_width(
            &self,
            _: &mut compose_ui::IntrinsicMeasureScope<'_>,
            _: &[compose_ui::IntrinsicMeasurable],
            _: i32,
        ) -> i32 {
            self.1
        }
        fn max_intrinsic_width(
            &self,
            _: &mut compose_ui::IntrinsicMeasureScope<'_>,
            _: &[compose_ui::IntrinsicMeasurable],
            _: i32,
        ) -> i32 {
            self.2
        }
    }
    Rc::new(move |scope, record| {
        let policy = compose_runtime::remember(|| {
            Rc::new(Policy(min, min_intrinsic, max_intrinsic)) as Rc<dyn MeasurePolicy>
        });
        Layout(modifier(scope).then(record), policy, compose_runtime::callback(|| {}));
    })
}

struct Laid {
    grid: Rect,
    items: Vec<Rect>,
}

impl Laid {
    /// Where `index` is in the grid.
    fn at(&self, index: usize) -> (f32, f32) {
        (self.items[index].left - self.grid.left, self.items[index].top - self.grid.top)
    }

    fn size(&self, index: usize) -> (f32, f32) {
        (self.items[index].width(), self.items[index].height())
    }

    fn width(&self, index: usize) -> f32 {
        self.items[index].width()
    }

    fn grid_size(&self) -> (f32, f32) {
        (self.grid.width(), self.grid.height())
    }
}

/// A grid configured by `config` with `modifier`, in a box with `outer`, right to left if `rtl`,
/// in a 1000 px root.
fn lay(
    config: impl Fn(&mut GridConfigurationScope) + 'static,
    modifier: Modifier,
    outer: Modifier,
    rtl: bool,
    children: Vec<Child>,
) -> Laid {
    let count = children.len();
    let recorded: Rc<RefCell<Vec<Option<Rect>>>> = Rc::new(RefCell::new(vec![None; count + 1]));
    let into = |recorded: &Rc<RefCell<Vec<Option<Rect>>>>, index: usize| {
        let recorded = recorded.clone();
        Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
            let (at, size) = (coordinates.position_in_root(), coordinates.size());
            recorded.borrow_mut()[index] =
                Some(Rect::new(at.x, at.y, at.x + size.width as f32, at.y + size.height as f32));
        })
    };
    let config = GridConfig::new(config);
    let _ui = ComposeTestRule::sized(IntSize::new(1000, 1000), {
        let recorded = recorded.clone();
        move || {
            let (config, children, recorded) = (config.clone(), children.clone(), recorded.clone());
            let modifier = modifier.clone().then(into(&recorded, count));
            let direction = if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            let outer = outer.clone();
            compose_runtime::CompositionLocalProvider(
                compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction),
                || {
                    let (config, children, recorded, modifier) =
                        (config.clone(), children.clone(), recorded.clone(), modifier.clone());
                    Box!(modifier = outer.clone(), {
                        let (children, recorded) = (children.clone(), recorded.clone());
                        Grid!(config.clone(), modifier = modifier.clone(), |scope: GridScope| {
                            for (index, child) in children.iter().enumerate() {
                                child(scope, into(&recorded, index));
                            }
                        });
                    });
                },
            );
        }
    });
    let recorded = recorded.borrow();
    let rect = |index: usize| recorded[index].unwrap_or_else(|| panic!("{index} wasn't placed"));
    Laid { grid: rect(count), items: (0..count).map(rect).collect() }
}

/// A child filling cell `row`, `column`.
fn cell(row: i32, column: i32) -> Child {
    boxed(move |scope| Modifier.grid_item(scope, row, column).fill_max_size())
}

/// A child of `size` px in cell `row`, `column`.
fn sized(row: i32, column: i32, size: (f32, f32)) -> Child {
    boxed(move |scope| Modifier.grid_item(scope, row, column).size((size.0.dp(), size.1.dp())))
}

/// A `size` px child flowing into the next free cell.
fn flowing(size: f32) -> Child {
    boxed(move |_| Modifier.size(size.dp()))
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_fixedTracks_sizeCorrectly
#[test]
fn fixed_tracks_are_their_sizes() {
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.column(100.dp());
            c.row(50.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![cell(1, 1), cell(1, 2)],
    );
    assert_eq!((laid.size(0), laid.at(0)), ((50.0, 50.0), (0.0, 0.0)));
    assert_eq!((laid.size(1), laid.at(1)), ((100.0, 50.0), (50.0, 0.0)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_percentageTracks
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_percentageRows_resolvesAgainstHeight
#[test]
fn percentage_tracks_take_their_share_of_the_grid() {
    let laid = lay(
        |c| {
            c.column(GridTrackSize::percentage(0.25));
            c.column(0.75);
            c.row(50.dp());
        },
        Modifier.size((200.dp(), 50.dp())),
        Modifier,
        false,
        vec![cell(1, 1), cell(1, 2)],
    );
    assert_eq!((laid.size(0), laid.at(0)), ((50.0, 50.0), (0.0, 0.0)));
    assert_eq!((laid.size(1), laid.at(1)), ((150.0, 50.0), (50.0, 0.0)));
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.row(0.25);
            c.row(0.75);
        },
        Modifier.height(200.dp()),
        Modifier,
        false,
        vec![cell(1, 1), cell(2, 1)],
    );
    assert_eq!((laid.size(0).1, laid.size(1).1), (50.0, 150.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_flexTracks
#[test]
fn flexible_tracks_share_what_the_fixed_ones_leave() {
    let laid = lay(
        |c| {
            c.column(100.dp());
            c.column(1.fr());
            c.column(GridTrackSize::flex(3.fr()));
            c.row(50.dp());
        },
        Modifier.width(300.dp()),
        Modifier,
        false,
        vec![cell(1, 1), cell(1, 2), cell(1, 3)],
    );
    assert_eq!((laid.size(1), laid.at(1)), ((50.0, 50.0), (100.0, 0.0)));
    assert_eq!((laid.size(2), laid.at(2)), ((150.0, 50.0), (150.0, 0.0)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_flexRespectsMinContent
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_flexTrack_minContent_lowerBound
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_flexTracks_distributeSpace_afterMinContent
#[test]
fn a_flexible_track_is_at_least_its_contents_minimum() {
    let laid = lay(
        |c| {
            c.column(GridTrackSize::MIN_CONTENT);
            c.column(1.fr());
            c.row(50.dp());
        },
        Modifier.width(150.dp()),
        Modifier,
        false,
        vec![intrinsic(100, 100, 100, |s| Modifier.grid_item(s, 1, 1)), cell(1, 2)],
    );
    assert_eq!((laid.width(0), laid.width(1)), (100.0, 50.0));
    let laid = lay(
        |c| {
            c.column(1.fr());
            c.row(50.dp());
        },
        Modifier,
        Modifier.width(50.dp()),
        false,
        vec![intrinsic(100, 100, 100, |s| Modifier.grid_item(s, 1, 1).fill_max_height())],
    );
    assert_eq!(laid.width(0), 100.0);
    let laid = lay(
        |c| {
            c.column(1.fr());
            c.column(1.fr());
            c.row(50.dp());
        },
        Modifier.width(200.dp()),
        Modifier,
        false,
        vec![
            intrinsic(150, 150, 150, |s| Modifier.grid_item(s, 1, 1).fill_max_width()),
            intrinsic(10, 10, 10, |s| Modifier.grid_item(s, 1, 2).fill_max_width()),
        ],
    );
    // Each its minimum and an even share of the 40 left.
    assert_eq!((laid.width(0), laid.width(1)), (170.0, 30.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_mixedTrackTypes_resolutionOrder
#[test]
fn fixed_then_content_then_flexible_tracks_are_resolved() {
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.column(GridTrackSize::AUTO);
            c.column(1.fr());
            c.row(50.dp());
        },
        Modifier.width(200.dp()),
        Modifier,
        false,
        vec![
            cell(1, 1),
            boxed(|s| Modifier.grid_item(s, 1, 2).width(30.dp()).fill_max_height()),
            cell(1, 3),
        ],
    );
    assert_eq!([laid.width(0), laid.width(1), laid.width(2)], [50.0, 30.0, 120.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_gaps
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_flexWithGaps
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_spanningWithGaps
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_implicitTracks_respectGaps
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_gapPrecedence_specificOverridesGeneric
#[test]
fn gaps_space_the_tracks_and_come_out_of_the_flexible_ones() {
    let laid = lay(
        |c| {
            c.columns(&[GridTrackSize::fixed(50.dp()); 2]);
            c.row(50.dp());
            c.gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![cell(1, 1), cell(1, 2)],
    );
    assert_eq!(laid.at(1), (60.0, 0.0));
    let laid = lay(
        |c| {
            c.columns(&[GridTrackSize::flex(1.fr()); 2]);
            c.row(50.dp());
            c.gap(10.dp());
        },
        Modifier.required_size(50.dp()),
        Modifier,
        false,
        vec![cell(1, 1), cell(1, 2)],
    );
    assert_eq!((laid.size(0), laid.size(1), laid.at(1)), ((20.0, 50.0), (20.0, 50.0), (30.0, 0.0)));
    let laid = lay(
        |c| {
            c.columns(&[GridTrackSize::fixed(50.dp()); 3]);
            c.row(50.dp());
            c.gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![boxed(|s| {
            Modifier.grid_item_with(s, 1, 1, 1, 2, Alignment::TOP_START).fill_max_size()
        })],
    );
    assert_eq!(laid.size(0), (110.0, 50.0));
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.row(50.dp());
            c.gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![sized(1, 1, (50.0, 50.0)), sized(1, 2, (50.0, 50.0))],
    );
    assert_eq!(laid.at(1), (60.0, 0.0));
    let laid = lay(
        |c| {
            c.columns(&[GridTrackSize::fixed(50.dp()); 2]);
            c.rows(&[GridTrackSize::fixed(50.dp()); 2]);
            c.gap(10.dp());
            c.row_gap(20.dp());
            c.column_gap(5.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![cell(1, 1), cell(1, 2), cell(2, 1)],
    );
    assert_eq!((laid.at(1), laid.at(2)), ((55.0, 0.0), (0.0, 70.0)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_alignment
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_alignment_spanning
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_alignment_rtl
#[test]
fn a_child_aligns_in_its_cell() {
    let laid = lay(
        |c| {
            c.column(100.dp());
            c.row(100.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![boxed(|s| Modifier.grid_item_with(s, 1, 1, 1, 1, Alignment::CENTER).size(40.dp()))],
    );
    assert_eq!(laid.at(0), (30.0, 30.0));
    let laid = lay(
        |c| {
            c.columns(&[GridTrackSize::fixed(50.dp()); 2]);
            c.rows(&[GridTrackSize::fixed(60.dp()); 2]);
            c.gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![boxed(|s| {
            Modifier.grid_item_with(s, 1, 1, 2, 2, Alignment::BOTTOM_END).size(40.dp())
        })],
    );
    assert_eq!(laid.at(0), (70.0, 90.0));
    let laid = lay(
        |c| {
            c.column(100.dp());
            c.row(100.dp());
        },
        Modifier,
        Modifier,
        true,
        vec![boxed(|s| Modifier.grid_item_with(s, 1, 1, 1, 1, Alignment::TOP_START).size(40.dp()))],
    );
    // The start is on the right. compose-ui has no `AbsoluteAlignment`: its half isn't ported.
    assert_eq!(laid.at(0), (60.0, 0.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_rtl
#[test]
fn right_to_left_the_first_column_is_on_the_right() {
    let laid = lay(
        |c| {
            c.columns(&[GridTrackSize::fixed(100.dp()); 2]);
            c.row(100.dp());
        },
        Modifier,
        Modifier,
        true,
        vec![cell(1, 1), cell(1, 2)],
    );
    assert_eq!((laid.at(0), laid.at(1)), ((100.0, 0.0), (0.0, 0.0)));
}

/// Three children of 50 to 100 px intrinsic width in min-content, max-content and auto columns.
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_contentBasedSizing
#[test]
fn content_tracks_are_their_contents_minimum_or_maximum() {
    let laid = lay(
        |c| {
            c.column(GridTrackSize::MIN_CONTENT);
            c.column(GridTrackSize::MAX_CONTENT);
            c.column(GridTrackSize::AUTO);
        },
        Modifier.width(500.dp()),
        Modifier,
        false,
        (1..=3)
            .map(|column| intrinsic(50, 50, 100, move |s| Modifier.grid_item(s, 1, column)))
            .collect(),
    );
    assert_eq!([laid.width(0), laid.width(1), laid.width(2)], [50.0, 100.0, 100.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_autoTrack_resolvesToMinMax
#[test]
fn an_auto_track_grows_from_its_minimum_to_its_maximum() {
    let auto = |outer: Modifier| {
        lay(
            |c| c.column(GridTrackSize::AUTO),
            Modifier,
            outer,
            false,
            vec![intrinsic(50, 50, 100, |s| Modifier.grid_item(s, 1, 1).fill_max_height())],
        )
        .width(0)
    };
    // Compose's in a horizontal scroll: what matters is an unbounded width.
    assert_eq!(auto(Modifier.wrap_content_width_with(Alignment::START, true)), 100.0);
    assert_eq!(auto(Modifier.width(60.dp())), 60.0);
    assert_eq!(auto(Modifier.width(200.dp())), 100.0);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_autoTracks_distributeSpaceProportionally
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_auto_vs_flex_prioritization
#[test]
fn auto_tracks_grow_by_their_room_and_before_flexible_ones() {
    let laid = lay(
        |c| {
            c.column(GridTrackSize::AUTO);
            c.column(GridTrackSize::AUTO);
        },
        Modifier.width(100.dp()),
        Modifier,
        false,
        vec![
            intrinsic(20, 20, 40, |s| Modifier.grid_item(s, 1, 1)),
            intrinsic(20, 20, 100, |s| Modifier.grid_item(s, 1, 2)),
        ],
    );
    assert_eq!((laid.width(0), laid.width(1)), (32.0, 68.0));
    let laid = lay(
        |c| {
            c.column(GridTrackSize::AUTO);
            c.column(1.fr());
        },
        Modifier.width(200.dp()),
        Modifier,
        false,
        vec![intrinsic(50, 50, 100, |s| Modifier.grid_item(s, 1, 1)), cell(1, 2)],
    );
    assert_eq!((laid.width(0), laid.width(1)), (100.0, 100.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_implicitTracks_shrinkToFitContent
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_rowSpan_expandsRowsToFitContent
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_rowSpan_expandsFlexRows
#[test]
fn rows_grow_to_fit_a_child_spanning_them() {
    let laid = lay(|c| c.row(50.dp()), Modifier, Modifier, false, vec![flowing(50.0)]);
    assert_eq!(laid.at(0), (0.0, 0.0));
    let laid = lay(
        |c| {
            c.columns(&[GridTrackSize::fixed(50.dp()); 2]);
            c.rows(&[GridTrackSize::AUTO; 2]);
        },
        Modifier,
        Modifier,
        false,
        vec![
            sized(1, 1, (50.0, 10.0)),
            sized(2, 1, (50.0, 10.0)),
            boxed(|s| {
                Modifier
                    .grid_item_with(s, 1, 2, 2, 1, Alignment::TOP_START)
                    .size((50.dp(), 100.dp()))
            }),
        ],
    );
    assert_eq!(laid.grid_size().1, 100.0);
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.rows(&[GridTrackSize::flex(1.fr()); 2]);
        },
        Modifier,
        Modifier.height(100.dp()).wrap_content_height_with(Alignment::TOP, true),
        false,
        vec![boxed(|s| {
            Modifier.grid_item_with(s, 1, 1, 2, 1, Alignment::TOP_START).size((50.dp(), 200.dp()))
        })],
    );
    assert_eq!(laid.grid_size().1, 200.0);
}

fn fixed(size: f32, count: usize) -> Vec<GridTrackSize> {
    vec![GridTrackSize::fixed(size.dp()); count]
}

/// A child of 50 px at `row` and `column` (0 for unspecified).
fn at(row: i32, column: i32) -> Child {
    boxed(move |s| Modifier.grid_item(s, row, column).size(50.dp()))
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_autoPlacement_rowFlow
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_autoPlacement_wrapping_respectsGaps
#[test]
fn children_flow_along_the_rows_wrapping_at_the_last_column() {
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.flow(GridFlow::Row);
        },
        Modifier,
        Modifier,
        false,
        vec![flowing(50.0), flowing(50.0), flowing(50.0)],
    );
    assert_eq!([laid.at(0), laid.at(1), laid.at(2)], [(0.0, 0.0), (50.0, 0.0), (0.0, 50.0)]);
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.row(50.dp());
            c.gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![flowing(50.0), flowing(50.0)],
    );
    assert_eq!(laid.at(1), (0.0, 60.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_autoPlacement_columnFlow
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_columnFlow_wrapsCorrectly
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_columnFlow_createsImplicitColumns
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_columnFlow_implicitRows
#[test]
fn children_flow_down_the_columns_in_a_column_flow() {
    let laid = lay(
        |c| {
            c.flow(GridFlow::Column);
            c.rows(&fixed(50.0, 2));
        },
        Modifier,
        Modifier,
        false,
        vec![flowing(50.0), flowing(50.0), flowing(50.0)],
    );
    assert_eq!([laid.at(0), laid.at(1), laid.at(2)], [(0.0, 0.0), (0.0, 50.0), (50.0, 0.0)]);
    let laid = lay(
        |c| {
            c.rows(&fixed(50.0, 2));
            c.flow(GridFlow::Column);
        },
        Modifier,
        Modifier,
        false,
        (0..4).map(|_| flowing(50.0)).collect(),
    );
    assert_eq!(laid.grid_size(), (100.0, 100.0));
    let laid = lay(
        |c| {
            c.flow(GridFlow::Column);
            c.column(50.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![flowing(50.0), flowing(50.0), flowing(50.0)],
    );
    assert_eq!([laid.at(0), laid.at(1), laid.at(2)], [(0.0, 0.0), (0.0, 50.0), (0.0, 100.0)]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_mixedPlacement_skipsOccupiedCells
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_explicitPlacement_allowsOverlaps
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_explicitPlacement_doesNotMoveAutoCursor
#[test]
fn flowing_children_skip_placed_ones_and_placed_ones_may_overlap() {
    let three = |c: &mut GridConfigurationScope| {
        c.columns(&fixed(50.0, 3));
        c.row(50.dp());
    };
    let laid = lay(three, Modifier, Modifier, false, vec![at(1, 2), flowing(50.0), flowing(50.0)]);
    assert_eq!([laid.at(0), laid.at(1), laid.at(2)], [(50.0, 0.0), (0.0, 0.0), (100.0, 0.0)]);
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.row(50.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![at(1, 1), at(1, 1)],
    );
    assert_eq!((laid.at(0), laid.at(1)), ((0.0, 0.0), (0.0, 0.0)));
    let laid = lay(three, Modifier, Modifier, false, vec![flowing(50.0), at(1, 3), flowing(50.0)]);
    assert_eq!([laid.at(0), laid.at(1), laid.at(2)], [(0.0, 0.0), (100.0, 0.0), (50.0, 0.0)]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_fixedRow_autoCol_skipsOccupied
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_fixedCol_autoRow_skipsOccupied
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_mixedFlow_fixedRowInColumnFlow
#[test]
fn a_child_given_one_index_takes_the_next_free_cell_along_the_other() {
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 3));
            c.row(50.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![at(1, 1), at(1, 0), at(1, 0)],
    );
    assert_eq!([laid.at(0), laid.at(1), laid.at(2)], [(0.0, 0.0), (50.0, 0.0), (100.0, 0.0)]);
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.rows(&fixed(50.0, 3));
        },
        Modifier,
        Modifier,
        false,
        vec![at(1, 1), at(0, 1)],
    );
    assert_eq!(laid.at(1), (0.0, 50.0));
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.rows(&fixed(50.0, 2));
            c.flow(GridFlow::Column);
        },
        Modifier,
        Modifier,
        false,
        vec![at(2, 1), at(2, 0)],
    );
    assert_eq!(laid.at(1), (50.0, 50.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_negativeIndices_placeCorrectly
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_negativeIndex_refersToExplicitBounds
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_invalidNegativeIndices_fallbackToAuto
#[test]
fn negative_indices_count_from_the_explicit_end() {
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 3));
            c.rows(&fixed(50.0, 3));
        },
        Modifier,
        Modifier,
        false,
        vec![at(1, 1), at(1, -1), at(-1, 1), at(-1, -1)],
    );
    assert_eq!(
        [laid.at(0), laid.at(1), laid.at(2), laid.at(3)],
        [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)]
    );
    // An implicit third column doesn't move the explicit end.
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.rows(&fixed(50.0, 2));
        },
        Modifier,
        Modifier,
        false,
        vec![at(1, 3), at(0, -1)],
    );
    assert_eq!(laid.at(1), (50.0, 0.0));
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.rows(&fixed(50.0, 2));
        },
        Modifier,
        Modifier,
        false,
        vec![at(-1, -1), at(-5, -5)],
    );
    assert_eq!((laid.at(0), laid.at(1)), ((50.0, 50.0), (0.0, 0.0)));
}

/// A child at `row`, `column` spanning `rows` by `columns`, filling them.
fn spanning(row: i32, column: i32, rows: i32, columns: i32) -> Child {
    boxed(move |s| {
        Modifier.grid_item_with(s, row, column, rows, columns, Alignment::TOP_START).fill_max_size()
    })
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_spanning
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_spanEntireGrid
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_spanning_intoImplicitTracks
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_itemSpanLargerThanExplicitGrid_doesNotLoop
#[test]
fn a_child_spans_its_tracks_explicit_or_not() {
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 3));
            c.rows(&fixed(50.0, 3));
        },
        Modifier,
        Modifier,
        false,
        vec![spanning(2, 2, 2, 2)],
    );
    assert_eq!((laid.size(0), laid.at(0)), ((100.0, 100.0), (50.0, 50.0)));
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 4));
            c.row(50.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![spanning(1, 1, 1, 4)],
    );
    assert_eq!(laid.width(0), 200.0);
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.row(50.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![spanning(1, 1, 1, 2), at(1, 2)],
    );
    assert_eq!(laid.width(0), 100.0);
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.flow(GridFlow::Row);
        },
        Modifier,
        Modifier,
        false,
        vec![boxed(|s| Modifier.grid_item_with(s, 0, 0, 1, 3, Alignment::TOP_START).size(50.dp()))],
    );
    assert_eq!(laid.at(0), (0.0, 0.0));
}

/// An empty child in a grid of `track` px tracks with `modifier` in a box with `outer`: the grid's
/// size.
fn grid_of(track: f32, modifier: Modifier, outer: Modifier, children: Vec<Child>) -> (f32, f32) {
    lay(
        move |c| {
            c.column(track.dp());
            c.row(track.dp());
        },
        modifier,
        outer,
        false,
        children,
    )
    .grid_size()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_respectsMinConstraints_expandsToFill
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_respectsMaxConstraints_coercesSize
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_respectsConstraints_whenContentOverflows
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_respectsConstraints_whenContentUnderflows
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_zeroSizeChildren
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_itemFillsCell_whenRequested
#[test]
fn a_grid_is_its_tracks_within_its_constraints() {
    assert_eq!(grid_of(50.0, Modifier.size(100.dp()), Modifier, vec![]), (100.0, 100.0));
    assert_eq!(grid_of(200.0, Modifier, Modifier.size(100.dp()), vec![]), (100.0, 100.0));
    assert_eq!(grid_of(200.0, Modifier, Modifier.size(100.dp()), vec![cell(1, 1)]), (100.0, 100.0));
    assert_eq!(
        grid_of(50.0, Modifier.fill_max_size(), Modifier.required_size(200.dp()), vec![cell(1, 1)]),
        (200.0, 200.0)
    );
    assert_eq!(grid_of(50.0, Modifier, Modifier, vec![sized(1, 1, (0.0, 0.0))]), (50.0, 50.0));
    let laid = lay(
        |c| {
            c.column(100.dp());
            c.row(100.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![cell(1, 1)],
    );
    assert_eq!(laid.size(0), (100.0, 100.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_percentageTrack_inIndefiniteContainer_fallbacksToAuto
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_flexColumn_inInfiniteConstraints_withGap_doesNotExpand
#[test]
fn unbounded_percentage_and_flexible_tracks_fit_their_content() {
    let unbounded = || Modifier.wrap_content_width_with(Alignment::START, true);
    let laid = lay(
        |c| {
            c.column(0.5);
            c.row(50.dp());
        },
        Modifier,
        unbounded(),
        false,
        vec![sized(1, 1, (10.0, 10.0))],
    );
    assert_eq!(laid.grid_size(), (10.0, 50.0));
    let laid = lay(
        |c| {
            c.column(1.fr());
            c.column(0.dp());
            c.row(50.dp());
            c.column_gap(10.dp());
        },
        Modifier,
        unbounded(),
        false,
        vec![sized(1, 1, (50.0, 50.0))],
    );
    assert_eq!(laid.grid_size().0, 60.0);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_zeroSizeTrack_layoutCorrectly
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_stressTest_manyItems
#[test]
fn an_empty_track_takes_no_room_and_a_hundred_children_make_ten_rows() {
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.column(0.dp());
            c.column(50.dp());
            c.row(50.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![at(1, 1), at(1, 3)],
    );
    assert_eq!(laid.at(1), (50.0, 0.0));
    let laid = lay(
        |c| {
            c.columns(&fixed(10.0, 10));
            c.flow(GridFlow::Row);
        },
        Modifier,
        Modifier,
        false,
        (0..100).map(|_| flowing(10.0)).collect(),
    );
    assert_eq!(laid.grid_size(), (100.0, 100.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_nestedGrid
#[test]
fn a_grid_nests_in_a_cell() {
    let inner: Child = Rc::new(|scope, record| {
        let config = GridConfig::new(|c| {
            c.columns(&[GridTrackSize::flex(1.fr()); 2]);
            c.row(1.fr());
        });
        Grid!(
            config,
            modifier = Modifier.grid_item(scope, 1, 1).fill_max_size(),
            |inner: GridScope| {
                Box!(
                    modifier = Modifier.grid_item(inner, 1, 1).fill_max_size().then(record.clone())
                );
                Box!(modifier = Modifier.grid_item(inner, 1, 2).fill_max_size());
            }
        );
    });
    let laid = lay(
        |c| {
            c.column(100.dp());
            c.row(100.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![inner],
    );
    assert_eq!(laid.size(0), (50.0, 100.0));
}

/// Whether `run` panics.
fn panics(run: impl FnOnce()) -> bool {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).is_err();
    std::panic::set_hook(hook);
    panicked
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_invalidIndices_throws
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_infiniteFlexWeight_throws
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_infiniteMinMaxWeight_throws
#[test]
fn out_of_range_indices_and_infinite_weights_are_refused() {
    assert!(panics(|| drop(lay(
        |c| {
            c.column(10.dp());
            c.row(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![at(100_000, 1)]
    ))));
    assert!(panics(|| {
        let _ = GridTrackSize::flex(Fr(f32::INFINITY));
    }));
    assert!(panics(|| {
        let _ = GridTrackSize::min_max(0.dp(), Fr(f32::INFINITY));
    }));
    assert!(!panics(|| {
        let _ = GridTrackSize::min_max(0.dp(), Fr(1.0));
    }));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_config_accessConstraints
#[test]
fn the_config_sees_the_grids_constraints_and_density() {
    let seen = Rc::new(RefCell::new(None));
    let into = seen.clone();
    lay(
        move |c| {
            *into.borrow_mut() = Some((c.constraints(), c.density().density));
            c.column(10.dp());
            c.row(10.dp());
        },
        Modifier,
        Modifier.size(100.dp()),
        false,
        vec![sized(1, 1, (10.0, 10.0))],
    );
    let (constraints, density) = seen.borrow().expect("configured");
    assert_eq!((constraints.max_width, constraints.max_height, density), (100, 100, 1.0));
}

/// The rule's content, a 1000 px root.
fn show(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), content);
    ui.wait_for_idle();
    ui
}

/// Records the bounds in the root of what it modifies into `into`.
fn bounds_into(into: &Rc<RefCell<Rect>>) -> Modifier {
    let into = into.clone();
    Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
        let (at, size) = (coordinates.position_in_root(), coordinates.size());
        *into.borrow_mut() =
            Rect::new(at.x, at.y, at.x + size.width as f32, at.y + size.height as f32);
    })
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_responsive_columnsChangeWithWidth
#[test]
fn a_config_reading_the_constraints_follows_them() {
    let width = mutable_state_of(100.0f32);
    let (read, item) = (width.handle(), Rc::new(RefCell::new(Rect::new(0.0, 0.0, 0.0, 0.0))));
    let into = item.clone();
    let mut ui = show(move || {
        let into = into.clone();
        Box!(modifier = Modifier.width(read.get().dp()), {
            let into = into.clone();
            let config = GridConfig::new(|c| {
                let columns = if c.constraints().max_width < 200 { 1 } else { 2 };
                for _ in 0..columns {
                    c.column(1.fr());
                }
                c.row(GridTrackSize::AUTO);
            });
            Grid!(config, |scope: GridScope| {
                let modifier = Modifier.grid_item(scope, 1, 1).fill_max_width().height(50.dp());
                Box!(modifier = modifier.then(bounds_into(&into)));
            });
        });
    });
    assert_eq!(item.borrow().width(), 100.0);
    width.set(300.0);
    ui.wait_for_idle();
    assert_eq!(item.borrow().width(), 150.0);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_aspectRatio_inFlexTrack_doesNotExplode
#[test]
fn an_aspect_ratio_fits_its_flexible_cell() {
    let laid = lay(
        |c| {
            c.column(1.fr());
            c.row(1.fr());
        },
        Modifier.fill_max_size(),
        Modifier.size(500.dp()),
        false,
        vec![boxed(|s| Modifier.grid_item(s, 1, 1).aspect_ratio(16.0 / 9.0).fill_max_size())],
    );
    assert_eq!(laid.size(0), (500.0, 281.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_implicitRows_includeGapsInTotalSize
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_implicitColumns_includeGapsInTotalSize
#[test]
fn implicit_tracks_are_gapped_as_explicit_ones() {
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        (1..=3).map(|row| sized(row, 1, (50.0, 50.0))).collect(),
    );
    assert_eq!(laid.grid_size(), (50.0, 170.0));
    let laid = lay(
        |c| {
            c.row(50.dp());
            c.gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        (1..=3).map(|column| sized(1, column, (50.0, 50.0))).collect(),
    );
    assert_eq!(laid.grid_size(), (170.0, 50.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_lazyColumn_inMinMaxTrack_doesNotCrash
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_lazyRow_inMinMaxTrack_doesNotCrash
#[test]
fn a_lazy_list_takes_what_a_minmax_track_leaves() {
    let list: Child = Rc::new(|scope, record| {
        LazyColumn!(
            modifier = Modifier.grid_item(scope, 2, 1).fill_max_size().then(record),
            |list: LazyListScope| {
                list.items(10, |_| Box!(modifier = Modifier.height(20.dp())));
            }
        );
    });
    let laid = lay(
        |c| {
            c.column(GridTrackSize::min_max(0.dp(), 1.fr()));
            c.row(50.dp());
            c.row(GridTrackSize::min_max(0.dp(), 1.fr()));
        },
        Modifier.fill_max_size(),
        Modifier.height(200.dp()),
        false,
        vec![boxed(|s| Modifier.grid_item(s, 1, 1).fill_max_size()), list],
    );
    assert_eq!(laid.size(1).1, 150.0);
    let list: Child = Rc::new(|scope, record| {
        LazyRow!(
            modifier = Modifier.grid_item(scope, 1, 1).fill_max_size().then(record),
            |list: LazyListScope| {
                list.items(10, |_| Box!(modifier = Modifier.width(20.dp())));
            }
        );
    });
    let laid = lay(
        |c| {
            c.column(GridTrackSize::min_max(0.dp(), 1.fr()));
            c.column(100.dp());
            c.row(GridTrackSize::min_max(0.dp(), 1.fr()));
        },
        Modifier.fill_max_size(),
        Modifier.width(300.dp()),
        false,
        vec![list, boxed(|s| Modifier.grid_item(s, 1, 2).fill_max_size())],
    );
    assert_eq!(laid.width(0), 200.0);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_autoRowHeight_respectsColumnSpanAndGap
#[test]
fn an_auto_rows_height_is_its_child_at_the_spanned_width_with_its_gap() {
    const SPANNED: i32 = 50 * 2 + 10;
    struct Policy;
    impl MeasurePolicy for Policy {
        fn measure(
            &self,
            scope: &mut compose_ui::MeasureScope<'_>,
            _: &[compose_ui::Measurable],
            constraints: compose_ui::Constraints,
        ) -> compose_ui::MeasureResult {
            let width =
                if constraints.has_bounded_width() { constraints.max_width } else { SPANNED };
            scope.layout(width, if width < SPANNED { 200 } else { 100 })
        }
        fn min_intrinsic_width(
            &self,
            _: &mut compose_ui::IntrinsicMeasureScope<'_>,
            _: &[compose_ui::IntrinsicMeasurable],
            _: i32,
        ) -> i32 {
            SPANNED
        }
        fn max_intrinsic_width(
            &self,
            _: &mut compose_ui::IntrinsicMeasureScope<'_>,
            _: &[compose_ui::IntrinsicMeasurable],
            _: i32,
        ) -> i32 {
            SPANNED
        }
        fn min_intrinsic_height(
            &self,
            _: &mut compose_ui::IntrinsicMeasureScope<'_>,
            _: &[compose_ui::IntrinsicMeasurable],
            width: i32,
        ) -> i32 {
            if width < SPANNED { 200 } else { 100 }
        }
        fn max_intrinsic_height(
            &self,
            _: &mut compose_ui::IntrinsicMeasureScope<'_>,
            _: &[compose_ui::IntrinsicMeasurable],
            width: i32,
        ) -> i32 {
            if width < SPANNED { 200 } else { 100 }
        }
    }
    let child: Child = Rc::new(|scope, record| {
        let policy = compose_runtime::remember(|| Rc::new(Policy) as Rc<dyn MeasurePolicy>);
        let modifier = Modifier.grid_item_with(scope, 1, 1, 1, 2, Alignment::TOP_START);
        Layout(modifier.then(record), policy, compose_runtime::callback(|| {}));
    });
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.row(GridTrackSize::AUTO);
            c.column_gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![child],
    );
    assert_eq!(laid.size(0), (SPANNED as f32, 100.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_autoColumnWidth_respectsColumnSpanAndGap
#[test]
fn auto_columns_share_a_spanning_childs_width_less_its_gap() {
    let laid = lay(
        |c| {
            c.column(GridTrackSize::AUTO);
            c.column(GridTrackSize::AUTO);
            c.rows(&fixed(50.0, 2));
            c.column_gap(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![
            intrinsic(110, 110, 110, |s| {
                Modifier.grid_item_with(s, 1, 1, 1, 2, Alignment::TOP_START)
            }),
            cell(2, 1),
            cell(2, 2),
        ],
    );
    assert_eq!((laid.width(1), laid.width(2)), (50.0, 50.0));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_focusTraversal_followsPlacementOrder
#[test]
fn the_focus_goes_in_the_cells_order_not_the_childrens() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let manager = Rc::new(RefCell::new(None));
    let requester = compose_ui::FocusRequester::new();
    let (into, taken) = (log.clone(), manager.clone());
    let mut ui = show(move || {
        *taken.borrow_mut() = Some(compose_ui::LOCAL_FOCUS_MANAGER.current());
        let into = into.clone();
        let config = GridConfig::new(|c| {
            c.columns(&[GridTrackSize::flex(1.fr()); 2]);
            c.rows(&[GridTrackSize::flex(1.fr()); 2]);
        });
        Grid!(config, modifier = Modifier.size(100.dp()), |scope: GridScope| {
            for (row, column) in [(2, 2), (2, 1), (1, 2), (1, 1)] {
                let into = into.clone();
                let modifier = Modifier.grid_item(scope, row, column).size(50.dp());
                let modifier = if (row, column) == (1, 1) {
                    modifier.focus_requester(requester)
                } else {
                    modifier
                };
                let modifier = modifier
                    .on_focus_changed(move |state: compose_ui::FocusState| {
                        if state.is_focused() {
                            into.borrow_mut().push((row, column));
                        }
                    })
                    .focusable();
                Box!(modifier = modifier);
            }
        });
    });
    requester.request_focus();
    ui.wait_for_idle();
    let manager = manager.borrow().clone().expect("composed");
    for _ in 0..3 {
        manager.move_focus(compose_ui::FocusDirection::Next);
        ui.wait_for_idle();
    }
    assert_eq!(*log.borrow(), [(1, 1), (1, 2), (2, 1), (2, 2)]);
}

#[derive(PartialEq, Debug)]
enum TestArea {
    Header,
    Sidebar,
    Content,
    Footer,
    Center,
}

/// A child in `area`, filling it.
fn in_area(area: fn() -> TestArea) -> Child {
    boxed(move |s| Modifier.grid_item_area(s, area(), Alignment::TOP_START).fill_max_size())
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_namedArea_basic2DPlacement
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_namedArea_rangeBasedPlacement
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_namedArea_overlapsWithExplicitPlacement
#[test]
fn a_child_in_a_named_area_takes_its_cells() {
    let laid = lay(
        |c| {
            c.column(10.dp());
            c.column(20.dp());
            c.row(10.dp());
            c.row(30.dp());
            c.area(TestArea::Content, 2, 2, 1, 1);
        },
        Modifier,
        Modifier,
        false,
        vec![in_area(|| TestArea::Content)],
    );
    assert_eq!((laid.at(0), laid.size(0)), ((10.0, 10.0), (20.0, 30.0)));
    let laid = lay(
        |c| {
            c.columns(&fixed(10.0, 3));
            c.rows(&fixed(10.0, 3));
            c.area_in(TestArea::Header, 1..=2, 1..=3);
        },
        Modifier,
        Modifier,
        false,
        vec![in_area(|| TestArea::Header)],
    );
    assert_eq!((laid.at(0), laid.size(0)), ((0.0, 0.0), (30.0, 20.0)));
    let laid = lay(
        |c| {
            c.column(50.dp());
            c.row(50.dp());
            c.area(TestArea::Center, 1, 1, 1, 1);
        },
        Modifier,
        Modifier,
        false,
        vec![sized(1, 1, (50.0, 50.0)), in_area(|| TestArea::Center)],
    );
    assert_eq!((laid.at(0), laid.at(1)), ((0.0, 0.0), (0.0, 0.0)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_namedArea_1D_autoFlow
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_namedArea_1DColumn_autoFlowsVertically
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_namedArea_1DAreas_doNotCorruptGlobalCursor
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_namedArea_unknownArea_fallsBackToAutoPlacement
#[test]
fn children_flow_along_a_one_track_area_and_an_unknown_one_flows_as_any() {
    let unspecified = GridScope::GRID_INDEX_UNSPECIFIED;
    let laid = lay(
        move |c| {
            c.column(10.dp());
            c.column(20.dp());
            c.column(30.dp());
            c.row(15.dp());
            c.area(TestArea::Header, 1, unspecified, 1, 1);
        },
        Modifier,
        Modifier,
        false,
        (0..3).map(|_| in_area(|| TestArea::Header)).collect(),
    );
    let placed: Vec<_> = (0..3).map(|index| (laid.at(index), laid.size(index))).collect();
    assert_eq!(
        placed,
        [((0.0, 0.0), (10.0, 15.0)), ((10.0, 0.0), (20.0, 15.0)), ((30.0, 0.0), (30.0, 15.0))]
    );
    let laid = lay(
        move |c| {
            c.column(50.dp());
            c.rows(&fixed(50.0, 2));
            c.area(TestArea::Sidebar, unspecified, 1, 1, 1);
        },
        Modifier,
        Modifier,
        false,
        (0..2).map(|_| in_area(|| TestArea::Sidebar)).collect(),
    );
    assert_eq!((laid.at(0), laid.at(1)), ((0.0, 0.0), (0.0, 50.0)));
    let laid = lay(
        move |c| {
            c.columns(&fixed(50.0, 3));
            c.rows(&fixed(50.0, 3));
            c.area(TestArea::Footer, 3, unspecified, 1, 1);
        },
        Modifier,
        Modifier,
        false,
        vec![flowing(50.0), in_area(|| TestArea::Footer), flowing(50.0)],
    );
    assert_eq!((laid.at(0), laid.at(1), laid.at(2)), ((0.0, 0.0), (0.0, 100.0), (50.0, 0.0)));
    let laid = lay(
        |c| {
            c.column(10.dp());
            c.column(20.dp());
            c.row(10.dp());
        },
        Modifier,
        Modifier,
        false,
        vec![cell(1, 1), in_area(|| TestArea::Sidebar)],
    );
    assert_eq!((laid.at(0), laid.at(1), laid.size(1)), ((0.0, 0.0), (10.0, 0.0), (20.0, 10.0)));
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_withRowFlow_itemSpanExceedsExplicitColumns_isClamped
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_withColumnFlow_itemSpanExceedsExplicitRows_isClamped
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_withRowFlow_itemSpanExceedsExplicitColumns_isClampedToExplicitCount_evenWithImplicitTracks
#[test]
fn a_flowing_childs_span_is_cut_to_the_explicit_tracks() {
    let unspecified = GridScope::GRID_INDEX_UNSPECIFIED;
    let wide = move |rows, columns| -> Child {
        boxed(move |s| {
            Modifier
                .grid_item_with(s, unspecified, unspecified, rows, columns, Alignment::TOP_START)
                .fill_max_size()
        })
    };
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.row(50.dp());
            c.column_gap(10.dp());
        },
        Modifier,
        Modifier.wrap_content_width_with(Alignment::START, true),
        false,
        vec![wide(1, 4)],
    );
    assert_eq!((laid.width(0), laid.grid_size().0), (110.0, 110.0));
    let laid = lay(
        |c| {
            c.flow(GridFlow::Column);
            c.column(50.dp());
            c.rows(&fixed(50.0, 2));
            c.row_gap(10.dp());
        },
        Modifier,
        Modifier.wrap_content_height_with(Alignment::TOP, true),
        false,
        vec![wide(5, 1)],
    );
    assert_eq!((laid.size(0).1, laid.grid_size().1), (110.0, 110.0));
    let laid = lay(
        |c| {
            c.columns(&fixed(50.0, 2));
            c.rows(&fixed(50.0, 2));
            c.column_gap(10.dp());
        },
        Modifier,
        Modifier.wrap_content_width_with(Alignment::START, true),
        false,
        vec![sized(2, 4, (50.0, 50.0)), wide(1, 3)],
    );
    assert_eq!(laid.width(1), 110.0);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_itemModifierChange_triggersRelayout
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/GridTest.kt, testGrid_configStateChange_updatesLayout
#[test]
fn a_new_cell_or_a_state_the_config_reads_lays_the_grid_out_again() {
    let row = mutable_state_of(1);
    let (read, item) = (row.handle(), Rc::new(RefCell::new(Rect::new(0.0, 0.0, 0.0, 0.0))));
    let into = item.clone();
    let mut ui = show(move || {
        let into = into.clone();
        let config = GridConfig::new(|c| {
            c.column(50.dp());
            c.rows(&fixed(50.0, 2));
        });
        Grid!(config, |scope: GridScope| {
            let modifier = Modifier.grid_item(scope, read.get(), 1).size(50.dp());
            Box!(modifier = modifier.then(bounds_into(&into)));
        });
    });
    assert_eq!(item.borrow().top, 0.0);
    row.set(2);
    ui.wait_for_idle();
    assert_eq!(item.borrow().top, 50.0);

    let track = mutable_state_of(50.0f32);
    let (read, item) = (track.handle(), Rc::new(RefCell::new(Rect::new(0.0, 0.0, 0.0, 0.0))));
    let into = item.clone();
    let config = GridConfig::new(move |c| {
        c.column(read.get().dp());
        c.row(read.get().dp());
    });
    let mut ui = show(move || {
        let into = into.clone();
        Grid!(config.clone(), |scope: GridScope| {
            Box!(
                modifier = Modifier.grid_item(scope, 1, 1).fill_max_size().then(bounds_into(&into))
            );
        });
    });
    assert_eq!((item.borrow().width(), item.borrow().height()), (50.0, 50.0));
    track.set(100.0);
    ui.wait_for_idle();
    assert_eq!((item.borrow().width(), item.borrow().height()), (100.0, 100.0));
}
