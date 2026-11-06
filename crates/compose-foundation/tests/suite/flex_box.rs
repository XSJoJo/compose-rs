//! Compose's `FlexBoxTest.kt`: alignment by baselines, lines and blocks, right to left, refused
//! values, intrinsics, the focus order and text in a column. Density 1: a dp is a pixel.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{
    AlignmentLine, FIRST_BASELINE, FocusDirection, FocusRequester, HorizontalAlignmentLine,
    IntSize, IntrinsicSize, LOCAL_FOCUS_MANAGER, Layout, LayoutCoordinates, LayoutDirection,
    Offset, TextLayoutResult, UnitExt, VerticalAlignmentLine, measure_policy,
};

fn min(a: i32, b: i32) -> i32 {
    a.min(b)
}

thread_local! {
    static TEST_HORIZONTAL_LINE: HorizontalAlignmentLine = HorizontalAlignmentLine::new(min);
    static TEST_VERTICAL_LINE: VerticalAlignmentLine = VerticalAlignmentLine::new(min);
}

fn horizontal_line() -> HorizontalAlignmentLine {
    TEST_HORIZONTAL_LINE.with(|line| *line)
}

fn vertical_line() -> VerticalAlignmentLine {
    TEST_VERTICAL_LINE.with(|line| *line)
}

/// As the test's `BaselineTestLayout`: a leaf `width` by `height` giving `line` and the test's
/// vertical line at `baseline`.
fn baseline_layout(
    width: i32,
    height: i32,
    baseline: i32,
    line: HorizontalAlignmentLine,
    modifier: Modifier,
) {
    let vertical = vertical_line();
    let policy = measure_policy(move |scope, _, constraints| {
        let lines: [(AlignmentLine, i32); 2] =
            [(line.into(), baseline), (vertical.into(), baseline)];
        scope.provide_alignment_lines(lines);
        scope.layout(width.max(constraints.min_width), height.max(constraints.min_height))
    });
    Layout(modifier, policy, compose_runtime::callback(|| {}));
}

type Places = Rc<RefCell<Vec<Offset>>>;

/// Records where each of `count` children is in its parent.
fn places(count: usize) -> Places {
    Rc::new(RefCell::new(vec![Offset::new(-1.0, -1.0); count]))
}

fn place_into(places: &Places, index: usize) -> Modifier {
    let places = places.clone();
    Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
        places.borrow_mut()[index] = coordinates.position_in_parent();
    })
}

/// Two 50 by 40 children with `line` at 10 and 30, in a row aligned by `config`, each with its
/// own `flex`: their tops.
fn tops(line: HorizontalAlignmentLine, config: FlexBoxConfig, own: Option<FlexConfig>) -> Vec<f32> {
    let at = places(2);
    let into = at.clone();
    let _ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (into, own) = (into.clone(), own.clone());
        FlexBox!(config = config.clone(), |scope: FlexBoxScope| {
            for (index, baseline) in [10, 30].into_iter().enumerate() {
                let modifier = match &own {
                    Some(own) => Modifier.flex(scope, own.clone()),
                    None => Modifier,
                };
                baseline_layout(50, 40, baseline, line, modifier.then(place_into(&into, index)));
            }
        });
    });
    at.borrow().iter().map(|at| at.y).collect()
}

fn row_config(block: impl Fn(&mut FlexBoxConfigScope) + 'static) -> FlexBoxConfig {
    FlexBoxConfig::new(move |c| {
        c.direction(FlexDirection::Row);
        block(c);
    })
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, alignItemsBaseline_FirstBaseline
#[test]
fn children_align_by_their_first_baselines() {
    let config = row_config(|c| c.align_items(FlexAlignItems::Baseline));
    assert_eq!(tops(FIRST_BASELINE, config, None), [20.0, 0.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, alignItemsToBaseline_FirstBaseline
#[test]
fn children_align_by_a_line_of_their_own() {
    let config = row_config(|c| c.align_items_by(horizontal_line()));
    assert_eq!(tops(horizontal_line(), config, None), [20.0, 0.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, alignSelfToBaseline
#[test]
fn a_child_aligns_by_its_own_line() {
    let config = row_config(|c| c.align_items(FlexAlignItems::Start));
    let own = FlexConfig::new(|c| c.align_self_by(horizontal_line()));
    assert_eq!(tops(horizontal_line(), config, Some(own)), [20.0, 0.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, alignItemsToBaseline_lambda
#[test]
fn children_align_where_a_block_says() {
    let at = places(2);
    let into = at.clone();
    let _ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let into = into.clone();
        let config =
            row_config(|c| c.align_items_by_block(|measured| measured.measured_height() / 2));
        FlexBox!(config = config, |_: FlexBoxScope| {
            for (index, size) in [20.0, 40.0].into_iter().enumerate() {
                Box!(modifier = Modifier.size(size.dp()).then(place_into(&into, index)));
            }
        });
    });
    let tops: Vec<f32> = at.borrow().iter().map(|at| at.y).collect();
    assert_eq!(tops, [10.0, 0.0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, alignItemsToBaseline_Column_VerticalLine
#[test]
fn a_columns_children_align_by_a_vertical_line() {
    let at = places(2);
    let into = at.clone();
    let _ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let into = into.clone();
        let config = FlexBoxConfig::new(|c| {
            c.direction(FlexDirection::Column);
            c.align_items_by(vertical_line());
        });
        FlexBox!(config = config, |_: FlexBoxScope| {
            for (index, baseline) in [10, 30].into_iter().enumerate() {
                baseline_layout(40, 50, baseline, horizontal_line(), place_into(&into, index));
            }
        });
    });
    let lefts: Vec<f32> = at.borrow().iter().map(|at| at.x).collect();
    assert_eq!(lefts, [20.0, 0.0]);
}

/// Three 20 px squares right to left in a 200 px box, `direction` and `justify`: where each is
/// (left, top).
fn right_to_left(
    direction: FlexDirection,
    justify: FlexJustifyContent,
    fill: Modifier,
) -> Vec<Offset> {
    let at = places(3);
    let into = at.clone();
    let _ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let (into, fill) = (into.clone(), fill.clone());
        compose_runtime::CompositionLocalProvider(
            compose_ui::LOCAL_LAYOUT_DIRECTION.provides(LayoutDirection::Rtl),
            || {
                let (into, fill) = (into.clone(), fill.clone());
                Box!(modifier = Modifier.size(200.dp()), {
                    let into = into.clone();
                    let config = FlexBoxConfig::new(move |c| {
                        c.direction(direction);
                        c.justify_content(justify);
                        c.align_items(FlexAlignItems::Start);
                    });
                    FlexBox!(modifier = fill.clone(), config = config, |_: FlexBoxScope| {
                        for index in 0..3 {
                            Box!(modifier = Modifier.size(20.dp()).then(place_into(&into, index)));
                        }
                    });
                });
            },
        );
    });
    at.borrow().clone()
}

fn xs(at: &[Offset]) -> Vec<f32> {
    at.iter().map(|at| at.x).collect()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, row_rtl_start_mirrorsToRight
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, rowReverse_rtl_start_doubleReversalFlowsLeftToRight
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, row_rtl_spaceBetween
#[test]
fn a_row_right_to_left_starts_at_the_right() {
    let fill = Modifier.fill_max_width();
    assert_eq!(
        xs(&right_to_left(FlexDirection::Row, FlexJustifyContent::Start, fill.clone())),
        [180.0, 160.0, 140.0]
    );
    assert_eq!(
        xs(&right_to_left(FlexDirection::RowReverse, FlexJustifyContent::Start, fill.clone())),
        [0.0, 20.0, 40.0]
    );
    assert_eq!(
        xs(&right_to_left(FlexDirection::Row, FlexJustifyContent::SpaceBetween, fill)),
        [180.0, 90.0, 0.0]
    );
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, column_rtl_alignItemsStart_crossAxisFlips
#[test]
fn a_column_right_to_left_aligns_at_the_right() {
    let at =
        right_to_left(FlexDirection::Column, FlexJustifyContent::Start, Modifier.fill_max_size());
    assert_eq!(xs(&at), [180.0; 3]);
}

/// Whether composing a FlexBox of one child with `flex` panics.
fn refused(config: FlexBoxConfig, flex: Option<FlexConfig>) -> bool {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ui = ComposeTestRule::sized(IntSize::new(100, 100), move || {
            let flex = flex.clone();
            FlexBox!(config = config.clone(), |scope: FlexBoxScope| {
                let modifier = flex.clone().map_or(Modifier, |flex| Modifier.flex(scope, flex));
                Box!(modifier = modifier);
            });
        });
    }))
    .is_err();
    std::panic::set_hook(hook);
    panicked
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, invalidFlexGrow_negative
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, invalidFlexGrow_nan
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, invalidFlexShrink_negative
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, invalidFlexShrink_nan
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, invalidMaxItemsInEachLine_zero
#[test]
fn negative_or_nan_factors_and_no_items_in_a_line_are_refused() {
    let flex = |block: fn(&mut FlexConfigScope)| Some(FlexConfig::new(block));
    assert!(refused(FlexBoxConfig::default(), flex(|c| c.grow(-1.0))));
    assert!(refused(FlexBoxConfig::default(), flex(|c| c.grow(f32::NAN))));
    assert!(refused(FlexBoxConfig::default(), flex(|c| c.shrink(-1.0))));
    assert!(refused(FlexBoxConfig::default(), flex(|c| c.shrink(f32::NAN))));
    assert!(refused(FlexBoxConfig::new(|c| c.max_items_in_each_line(0)), None));
    assert!(!refused(FlexBoxConfig::default(), flex(|c| c.grow(1.0))));
}

/// The width of a wrapping row of 20, 30 and 40 px squares in a box of `width` (an intrinsic),
/// at most `per_line` in a line.
fn intrinsic_width(width: IntrinsicSize, per_line: Option<usize>) -> i32 {
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let into = size.clone();
    let _ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        let into = into.clone();
        Box!(modifier = Modifier.width(width), {
            let into = into.clone();
            let config = FlexBoxConfig::new(move |c| {
                c.direction(FlexDirection::Row);
                c.wrap(FlexWrap::Wrap);
                if let Some(per_line) = per_line {
                    c.max_items_in_each_line(per_line);
                }
            });
            FlexBox!(
                modifier = Modifier.on_size_changed(move |size: IntSize| into.set(size)),
                config = config,
                |_: FlexBoxScope| {
                    for size in [20.0, 30.0, 40.0] {
                        Box!(modifier = Modifier.size(size.dp()));
                    }
                }
            );
        });
    });
    size.get().width
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, maxItemsInEachLine_maxIntrinsicWidth_reportsLargestCappedLine
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, testFlexBox_wrap_maxIntrinsicWidth_reportsSumOfChildren
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, testFlexBox_wrap_minIntrinsicWidth_reportsMaxChildWidth
#[test]
fn a_wrapping_rows_intrinsic_widths_are_its_lines_and_its_widest() {
    assert_eq!(intrinsic_width(IntrinsicSize::Max, Some(2)), 50);
    assert_eq!(intrinsic_width(IntrinsicSize::Max, None), 90);
    assert_eq!(intrinsic_width(IntrinsicSize::Min, None), 40);
}

/// `count` focusable squares of `size` in a FlexBox configured by `config`, the one at `first`
/// focused, the focus moved forward `moves` times: the order focused.
fn focus_order(
    config: FlexBoxConfig,
    count: usize,
    size: f32,
    ordered: Option<usize>,
    first: usize,
    moves: usize,
) -> Vec<usize> {
    let log = Rc::new(RefCell::new(Vec::new()));
    let manager = Rc::new(RefCell::new(None));
    let requester = FocusRequester::new();
    let (into, taken, request) = (log.clone(), manager.clone(), requester);
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), move || {
        *taken.borrow_mut() = Some(LOCAL_FOCUS_MANAGER.current());
        let (into, request) = (into.clone(), request);
        let config = config.clone();
        Box!(modifier = Modifier.size(200.dp()), {
            let (into, request) = (into.clone(), request);
            FlexBox!(config = config.clone(), |scope: FlexBoxScope| {
                for index in 0..count {
                    let into = into.clone();
                    let modifier = if ordered == Some(index) {
                        Modifier.flex(scope, FlexConfig::new(|c| c.order(1)))
                    } else {
                        Modifier
                    };
                    let modifier = modifier.size(size.dp());
                    let modifier =
                        if index == first { modifier.focus_requester(request) } else { modifier };
                    let modifier = modifier
                        .on_focus_changed(move |state: compose_ui::FocusState| {
                            if state.is_focused() {
                                into.borrow_mut().push(index);
                            }
                        })
                        .focusable();
                    Box!(modifier = modifier);
                }
            });
        });
    });
    requester.request_focus();
    ui.wait_for_idle();
    let manager = manager.borrow().clone().expect("composed");
    for _ in 0..moves {
        manager.move_focus(FocusDirection::Next);
        ui.wait_for_idle();
    }
    log.borrow().clone()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, focusTraversal_rowReverse_followsVisualOrder
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, focusTraversal_wrapReverse_followsVisualOrder
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, focusTraversal_customOrder_followsVisualOrder
#[test]
fn the_focus_goes_in_the_order_shown() {
    let reversed = FlexBoxConfig::new(|c| c.direction(FlexDirection::RowReverse));
    assert_eq!(focus_order(reversed, 3, 50.0, None, 2, 2), [2, 1, 0]);
    let wrap_reversed = FlexBoxConfig::new(|c| {
        c.direction(FlexDirection::Row);
        c.wrap(FlexWrap::WrapReverse);
    });
    assert_eq!(focus_order(wrap_reversed, 4, 100.0, None, 2, 3), [2, 3, 0, 1]);
    assert_eq!(focus_order(FlexBoxConfig::default(), 3, 50.0, Some(0), 1, 2), [1, 2, 0]);
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxTest.kt, column_multiLineText_wrapsLikeStandardColumn
#[test]
fn text_in_a_column_wraps_as_in_a_column() {
    const TEXT: &str = "This is a long paragraph of text that needs to wrap into multiple lines when \
                        placed in a narrow 120dp container.";
    let flex = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0u32)), Rc::new(Cell::new(-1.0f32)));
    let column = (Rc::new(Cell::new(0)), Rc::new(Cell::new(-1.0f32)));
    let (f, c) = (flex.clone(), column.clone());
    let _ui = ComposeTestRule::sized(IntSize::new(400, 800), move || {
        let (f, c) = (f.clone(), c.clone());
        Row!({
            let f = f.clone();
            let config = FlexBoxConfig::new(|c| c.direction(FlexDirection::Column));
            FlexBox!(modifier = Modifier.width(120.dp()), config = config, |_: FlexBoxScope| {
                let (height, lines, sibling) = f.clone();
                BasicText!(
                    TEXT,
                    modifier =
                        Modifier.on_size_changed(move |size: IntSize| height.set(size.height)),
                    on_text_layout = move |layout: TextLayoutResult| lines.set(layout.line_count)
                );
                Box!(
                    modifier = Modifier.fill_max_width().height(40.dp()).on_globally_positioned(
                        move |at: LayoutCoordinates| sibling.set(at.position_in_parent().y)
                    )
                );
            });
            let (height, sibling) = c.clone();
            Column!(modifier = Modifier.width(120.dp()), {
                let (height, sibling) = (height.clone(), sibling.clone());
                BasicText!(
                    TEXT,
                    modifier =
                        Modifier.on_size_changed(move |size: IntSize| height.set(size.height))
                );
                Box!(
                    modifier = Modifier.fill_max_width().height(40.dp()).on_globally_positioned(
                        move |at: LayoutCoordinates| sibling.set(at.position_in_parent().y)
                    )
                );
            });
        });
    });
    assert!(flex.1.get() > 1, "{}", flex.1.get());
    assert_eq!(flex.0.get(), column.0.get());
    assert_eq!(flex.2.get(), column.1.get());
    assert_eq!(flex.2.get(), flex.0.get() as f32);
}
