//! The lazy layouts' `BeyondBoundsLayout` (D1479), found from a node in an item: Compose's
//! `LazyListBeyondBoundsTest`, `LazyGridBeyondBoundsTest` and `LazyStaggeredGridBeyondBoundsTest`,
//! rewritten. Each runs over the six directions, both reversals and both layout directions, as
//! Compose's parameters. What's placed is read from the semantics tree (each item's tag), placed
//! and attached, as Compose's `trackPlaced` hears `onPlaced` and `onDetach`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::BeyondBoundsLayoutDirection::{self, *};
use compose_ui::{
    BeyondBoundsLayout, IntSize, LOCAL_LAYOUT_DIRECTION, LayoutDirection, ModifierNode,
    ModifierNodeElement, NodeContext, Rect, SemanticsProperties, UnitExt,
};

/// A test case, as Compose's `Param`.
#[derive(Clone, Copy, Debug)]
struct Param {
    direction: BeyondBoundsLayoutDirection,
    reverse: bool,
    rtl: bool,
}

/// Every direction, reversed or not, left to right or not, as Compose's `ParamsToTest`.
fn params() -> Vec<Param> {
    let mut all = Vec::new();
    for direction in [Left, Right, Above, Below, Before, After] {
        for reverse in [false, true] {
            for rtl in [false, true] {
                all.push(Param { direction, reverse, rtl });
            }
        }
    }
    all
}

impl Param {
    /// Whether its layout runs sideways: a row for the sideways directions and the order's.
    fn horizontal(&self) -> bool {
        !matches!(self.direction, Above | Below)
    }

    /// As Compose's `expectedExtraItemsBeforeVisibleBounds`.
    fn extra_before(&self) -> bool {
        match self.direction {
            Right => !self.rtl && self.reverse || self.rtl && !self.reverse,
            Left => !self.rtl && !self.reverse || self.rtl && self.reverse,
            Above => !self.reverse,
            Below => self.reverse,
            After => false,
            Before => true,
        }
    }

    /// Whether the items go the other way on the screen, as `PlacementComparator`'s.
    fn reversed_on_screen(&self) -> bool {
        match self.direction {
            Above | Below => self.reverse,
            _ => self.reverse != self.rtl,
        }
    }
}

/// Which lazy layout, and how many cells across a grid's lines.
#[derive(Clone, Copy, Debug)]
enum Kind {
    List,
    Grid(usize),
    Staggered(usize),
}

/// Finds the nearest beyond-bounds layout above it when it's attached, as Compose's
/// `ParentCapturingNode`.
#[derive(Clone)]
struct Capturing(Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>>);

struct CapturingNode(Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>>);

impl ModifierNode for CapturingNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        if let Some(found) = context.find_nearest_beyond_bounds_layout_ancestor() {
            *self.0.borrow_mut() = Some(found);
        }
    }
}

impl ModifierNodeElement for Capturing {
    type Node = CapturingNode;
    fn create(&self) -> CapturingNode {
        CapturingNode(self.0.clone())
    }
    fn update(&self, node: &mut CapturingNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// What an item is: its index and size along both axes, and whether it captures.
#[derive(Clone)]
struct Content {
    count: MutableState<usize>,
    _owner: Rc<OwnedState<usize>>,
    captures: Option<usize>,
    size: fn(usize) -> (f32, f32),
    full_line: Option<usize>,
}

/// A lazy layout shown for a test case, with what it gives the test.
struct Shown {
    ui: ComposeTestRule,
    layout: Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>>,
    visible: Rc<dyn Fn() -> Vec<usize>>,
    total: Rc<dyn Fn() -> usize>,
}

impl Shown {
    fn layout(&self) -> Rc<dyn BeyondBoundsLayout> {
        self.layout.borrow().clone().expect("a beyond-bounds layout was found")
    }

    /// The items placed, by index, with their bounds in the root.
    fn placed(&mut self) -> Vec<(usize, Rect)> {
        let owner = self.ui.host().semantics_owner();
        let mut placed: Vec<(usize, Rect)> = owner
            .get_all_semantics_nodes(false, true)
            .into_iter()
            .filter(|node| node.layout_info().is_placed() && node.layout_info().is_attached())
            .filter_map(|node| {
                let config = node.unmerged_config();
                let tag = config.get(&SemanticsProperties::TEST_TAG)?;
                let index = tag.strip_prefix("item")?.parse().ok()?;
                Some((index, node.bounds_in_root()))
            })
            .collect();
        placed.sort_by_key(|(index, _)| *index);
        placed
    }

    fn placed_indices(&mut self) -> Vec<usize> {
        self.placed().into_iter().map(|(index, _)| index).collect()
    }

    /// Asserts the placed items go in the order of `param` on the screen, as
    /// `PlacementComparator`.
    fn assert_in_order(&mut self, param: Param) {
        let placed = self.placed();
        let along = |rect: &Rect| if param.horizontal() { rect.left } else { rect.top };
        for pair in placed.windows(2) {
            let (a, b) = (along(&pair[0].1), along(&pair[1].1));
            let in_order = if param.reversed_on_screen() { a >= b } else { a <= b };
            assert!(in_order, "{param:?}: {placed:?}");
        }
    }
}

fn item_tag(index: usize) -> String {
    format!("item{index}")
}

/// Shows `kind` for `param`, `viewport` px along its axis (and across), at `first`, its axis
/// across the param's when `perpendicular`.
fn show(
    kind: Kind,
    param: Param,
    perpendicular: bool,
    viewport: f32,
    first: usize,
    content: Content,
) -> Shown {
    let layout: Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>> = Rc::default();
    let horizontal = param.horizontal() != perpendicular;
    let direction = if param.rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
    let captured = layout.clone();
    let item: Rc<dyn Fn(usize)> = Rc::new(move |index: usize| {
        let (main, cross) = (content.size)(index);
        let (width, height) = if horizontal { (main, cross) } else { (cross, main) };
        let mut modifier = Modifier.size((width.dp(), height.dp())).test_tag(item_tag(index));
        if content.captures == Some(index) {
            modifier = modifier.then_custom(Capturing(captured.clone()));
        }
        Box!(modifier = modifier);
    });
    let count = content.count;
    let full_line = content.full_line;
    let reverse = param.reverse;
    let size = viewport.dp();
    // In the layout direction asked, the lazy layout `lay_out` shows.
    let window_of = |lay_out: Rc<dyn Fn()>| {
        window(move || {
            let lay_out = lay_out.clone();
            CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(direction), move || lay_out());
        })
    };
    match kind {
        Kind::List => {
            let owner = Rc::new(LazyListState::new(first, 0));
            let state = **owner;
            let ui = window_of(Rc::new(move || {
                let _ = &owner;
                let item = item.clone();
                let items = move |scope: LazyListScope| {
                    let item = item.clone();
                    scope.items(count.get(), move |index| item(index));
                };
                if horizontal {
                    LazyRow!(
                        modifier = Modifier.size(size),
                        state = state,
                        reverse_layout = reverse,
                        content = items
                    );
                } else {
                    LazyColumn!(
                        modifier = Modifier.size(size),
                        state = state,
                        reverse_layout = reverse,
                        content = items
                    );
                }
            }));
            Shown {
                ui,
                layout,
                visible: Rc::new(move || {
                    state.layout_info().visible_items_info.iter().map(|it| it.index).collect()
                }),
                total: Rc::new(move || state.layout_info().total_items_count),
            }
        }
        Kind::Grid(cells) => {
            let owner = Rc::new(LazyGridState::new(first, 0));
            let state = **owner;
            let ui = window_of(Rc::new(move || {
                let _ = &owner;
                let item = item.clone();
                let items = move |scope: LazyGridScope| {
                    let item = item.clone();
                    scope.items(count.get(), move |index| item(index));
                };
                if horizontal {
                    LazyHorizontalGrid!(
                        GridCells::Fixed(cells),
                        modifier = Modifier.size(size),
                        state = state,
                        reverse_layout = reverse,
                        content = items
                    );
                } else {
                    LazyVerticalGrid!(
                        GridCells::Fixed(cells),
                        modifier = Modifier.size(size),
                        state = state,
                        reverse_layout = reverse,
                        content = items
                    );
                }
            }));
            Shown {
                ui,
                layout,
                visible: Rc::new(move || {
                    state.layout_info().visible_items_info.iter().map(|it| it.index).collect()
                }),
                total: Rc::new(move || state.layout_info().total_items_count),
            }
        }
        Kind::Staggered(cells) => {
            let owner = Rc::new(LazyStaggeredGridState::new(first, 0));
            let state = **owner;
            let ui = window_of(Rc::new(move || {
                let _ = &owner;
                let item = item.clone();
                let items = move |scope: LazyStaggeredGridScope| {
                    for index in 0..count.get() {
                        let item = item.clone();
                        if full_line == Some(index) {
                            scope.item_with_span((), StaggeredGridItemSpan::FullLine, move || {
                                item(index)
                            });
                        } else {
                            scope.item(move || item(index));
                        }
                    }
                };
                if horizontal {
                    LazyHorizontalStaggeredGrid!(
                        StaggeredGridCells::Fixed(cells),
                        modifier = Modifier.size(size),
                        state = state,
                        reverse_layout = reverse,
                        content = items
                    );
                } else {
                    LazyVerticalStaggeredGrid!(
                        StaggeredGridCells::Fixed(cells),
                        modifier = Modifier.size(size),
                        state = state,
                        reverse_layout = reverse,
                        content = items
                    );
                }
            }));
            Shown {
                ui,
                layout,
                visible: Rc::new(move || {
                    let mut indices: Vec<usize> =
                        state.layout_info().visible_items_info.iter().map(|it| it.index).collect();
                    indices.sort_unstable();
                    indices
                }),
                total: Rc::new(move || state.layout_info().total_items_count),
            }
        }
    }
}

fn window(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(400, 400), content);
    ui.wait_for_idle();
    ui
}

/// `count` 10 px items, the one at `captures` finding the layout.
fn squares(count: usize, captures: Option<usize>) -> Content {
    let owner = Rc::new(mutable_state_of(count));
    Content {
        count: owner.handle(),
        _owner: owner,
        captures,
        size: |_| (10.0, 10.0),
        full_line: None,
    }
}

const KINDS: [Kind; 3] = [Kind::List, Kind::Grid(1), Kind::Staggered(1)];

/// Compose's `onlyOneVisibleItemIsPlaced`, `onlyTwoVisibleItemsArePlaced` and
/// `onlyThreeVisibleItemsArePlaced` in each file: nothing beyond the bounds is placed.
fn only_visible_items_are_placed(shown: usize) {
    for kind in KINDS {
        for param in params() {
            let mut lazy = show(kind, param, false, 10.0 * shown as f32, 0, squares(100, None));
            let expected: Vec<usize> = (0..shown).collect();
            assert_eq!(lazy.placed_indices(), expected, "{kind:?} {param:?}");
            assert_eq!((lazy.visible)(), expected, "{kind:?} {param:?}");
        }
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, onlyOneVisibleItemIsPlaced (and the grid's and staggered grid's)
#[test]
fn only_one_visible_item_is_placed() {
    only_visible_items_are_placed(1);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, onlyTwoVisibleItemsArePlaced (and the grid's and staggered grid's)
#[test]
fn only_two_visible_items_are_placed() {
    only_visible_items_are_placed(2);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, onlyThreeVisibleItemsArePlaced (and the grid's and staggered grid's)
#[test]
fn only_three_visible_items_are_placed() {
    only_visible_items_are_placed(3);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, emptyLazyList_doesNotCrash (and the grid's and staggered grid's)
#[test]
fn an_emptied_layout_has_no_more_content() {
    for kind in KINDS {
        for param in params() {
            let content = squares(1, Some(0));
            let count = content.count;
            let mut lazy = show(kind, param, false, 30.0, 0, content);
            let layout = lazy.layout();
            count.set(0);
            lazy.ui.wait_for_idle();
            let more = layout.layout(param.direction, |scope| Some(scope.has_more_content()));
            assert_eq!(more, Some(false), "{kind:?} {param:?}");
        }
    }
}

/// Five items, the capturing one, five more, 10 px each in a 30 px layout at the capturing one.
fn five_one_five() -> Content {
    squares(11, Some(5))
}

/// The items `extra` beyond 5, 6 and 7, before or after them as `param` expects.
fn with_extra(param: Param, extra: usize) -> Vec<usize> {
    if param.extra_before() { (5 - extra..=7).collect() } else { (5..=7 + extra).collect() }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, oneExtraItemBeyondVisibleBounds (and the grid's and staggered grid's)
#[test]
fn one_extra_item_beyond_the_visible_bounds() {
    for kind in KINDS {
        for param in params() {
            let mut lazy = show(kind, param, false, 30.0, 5, five_one_five());
            let layout = lazy.layout();
            let mut ran = 0;
            layout.layout(param.direction, |_| {
                ran += 1;
                assert_eq!(lazy.placed_indices(), with_extra(param, 1), "{kind:?} {param:?}");
                assert_eq!((lazy.visible)(), vec![5, 6, 7], "{kind:?} {param:?}");
                lazy.assert_in_order(param);
                Some(true)
            });
            assert_eq!(ran, 1, "{kind:?} {param:?}");
            lazy.ui.wait_for_idle();
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7], "{kind:?} {param:?}: they went");
            assert_eq!((lazy.visible)(), vec![5, 6, 7]);
        }
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, twoExtraItemsBeyondVisibleBounds (and the grid's and staggered grid's)
#[test]
fn two_extra_items_beyond_the_visible_bounds() {
    for kind in KINDS {
        for param in params() {
            let mut lazy = show(kind, param, false, 30.0, 5, five_one_five());
            let layout = lazy.layout();
            let mut left = 2;
            let found = layout.layout(param.direction, |_| {
                left -= 1;
                if left > 0 {
                    return None;
                }
                assert_eq!(lazy.placed_indices(), with_extra(param, 2), "{kind:?} {param:?}");
                assert_eq!((lazy.visible)(), vec![5, 6, 7]);
                lazy.assert_in_order(param);
                Some(true)
            });
            assert_eq!(found, Some(true), "{kind:?} {param:?}");
            lazy.ui.wait_for_idle();
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7], "{kind:?} {param:?}: they went");
        }
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, allBeyondBoundsItemsInSpecifiedDirection (and the grid's and staggered grid's)
#[test]
fn all_the_items_beyond_the_bounds_one_way() {
    for kind in KINDS {
        for param in params() {
            let mut lazy = show(kind, param, false, 30.0, 5, five_one_five());
            let layout = lazy.layout();
            let found = layout.layout(param.direction, |scope| {
                if scope.has_more_content() {
                    return None;
                }
                let expected: Vec<usize> =
                    if param.extra_before() { (0..=7).collect() } else { (5..=10).collect() };
                assert_eq!(lazy.placed_indices(), expected, "{kind:?} {param:?}");
                assert_eq!((lazy.visible)(), vec![5, 6, 7]);
                lazy.assert_in_order(param);
                Some(true)
            });
            assert_eq!(found, Some(true), "{kind:?} {param:?}");
            lazy.ui.wait_for_idle();
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7], "{kind:?} {param:?}: they went");
        }
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, beyondBoundsLayoutRequest_inDirectionPerpendicularToLazyListOrientation (and the grid's and staggered grid's)
#[test]
fn across_its_axis_there_is_nothing_beyond_the_bounds() {
    for kind in KINDS {
        for param in params() {
            let mut lazy = show(kind, param, true, 30.0, 5, five_one_five());
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7], "{kind:?} {param:?}");
            let layout = lazy.layout();
            let mut ran = 0;
            layout.layout(param.direction, |_| {
                ran += 1;
                let expected = match param.direction {
                    Before | After => with_extra(param, 1),
                    _ => vec![5, 6, 7],
                };
                assert_eq!(lazy.placed_indices(), expected, "{kind:?} {param:?}");
                assert_eq!((lazy.visible)(), vec![5, 6, 7]);
                Some(true)
            });
            let expected_runs = if matches!(param.direction, Before | After) { 1 } else { 0 };
            assert_eq!(ran, expected_runs, "{kind:?} {param:?}");
            lazy.ui.wait_for_idle();
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7], "{kind:?} {param:?}: they went");
        }
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/list/LazyListBeyondBoundsTest.kt, returningNullDoesNotCauseInfiniteLoop (and the grid's and staggered grid's)
#[test]
fn a_block_never_satisfied_ends() {
    for kind in KINDS {
        for param in params() {
            let mut lazy = show(kind, param, false, 30.0, 5, five_one_five());
            let layout = lazy.layout();
            let total = (lazy.total)();
            let count = Cell::new(0);
            let found: Option<()> = layout.layout(param.direction, |_| {
                assert!(count.get() < total, "{kind:?} {param:?}");
                count.set(count.get() + 1);
                None
            });
            assert_eq!(found, None);
            lazy.ui.wait_for_idle();
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7], "{kind:?} {param:?}: they went");
            assert_eq!((lazy.visible)(), vec![5, 6, 7]);
        }
    }
}

/// Asserts one step beyond 10 to 15 (three lines of two) places `before` or `after` too.
fn one_extra_item_in_two_cells(kind: Kind, before: &[usize], after: &[usize]) {
    for param in params() {
        let mut lazy = show(kind, param, false, 30.0, 10, squares(22, Some(11)));
        let layout = lazy.layout();
        let mut ran = 0;
        layout.layout(param.direction, |_| {
            ran += 1;
            let extra = if param.extra_before() { before } else { after };
            let mut expected: Vec<usize> = (10..=15).chain(extra.iter().copied()).collect();
            expected.sort_unstable();
            assert_eq!(lazy.placed_indices(), expected, "{kind:?} {param:?}");
            assert_eq!((lazy.visible)(), (10..=15).collect::<Vec<_>>(), "{kind:?} {param:?}");
            lazy.assert_in_order(param);
            Some(true)
        });
        assert_eq!(ran, 1, "{kind:?} {param:?}");
        lazy.ui.wait_for_idle();
        assert_eq!(lazy.placed_indices(), (10..=15).collect::<Vec<_>>(), "{kind:?} {param:?}");
    }
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/grid/LazyGridBeyondBoundsTest.kt, oneExtraItemBeyondVisibleBounds_multipleCells
#[test]
fn a_grid_lays_out_a_line_beyond_its_bounds() {
    // Compose's lays out one item, 9 or 16, alone in its line; compose-rs's grid lays out lines,
    // as it places a pinned item's (D1479).
    one_extra_item_in_two_cells(Kind::Grid(2), &[8, 9], &[16, 17]);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/staggeredgrid/LazyStaggeredGridBeyondBoundsTest.kt, oneExtraItemBeyondVisibleBounds_multipleCells
#[test]
fn a_staggered_grid_of_two_lanes_lays_out_one_more_item() {
    one_extra_item_in_two_cells(Kind::Staggered(2), &[9], &[16]);
}

// Compose: compose/foundation/foundation/integration-tests/lazy-tests/src/androidTest/kotlin/androidx/compose/foundation/lazy/staggeredgrid/LazyStaggeredGridBeyondBoundsTest.kt, oneExtraItemBeyondVisibleBounds_multipleCells_staggered
#[test]
fn a_staggered_grid_of_uneven_items_lays_out_one_more() {
    // Three lanes of 50 px: 0, 2, 5 and 7 twice as long as the others, 4 a full line.
    for param in params() {
        let owner = Rc::new(mutable_state_of(9));
        let content = Content {
            count: owner.handle(),
            _owner: owner,
            captures: Some(4),
            size: |index| {
                let long = matches!(index, 0 | 2 | 5 | 7);
                (if long { 100.0 } else { 50.0 }, 50.0)
            },
            full_line: Some(4),
        };
        let mut lazy = show(Kind::Staggered(3), param, false, 100.0, 4, content);
        let layout = lazy.layout();
        let mut ran = 0;
        layout.layout(param.direction, |_| {
            ran += 1;
            let expected: Vec<usize> =
                if param.extra_before() { vec![3, 4, 5, 6, 7] } else { vec![4, 5, 6, 7, 8] };
            assert_eq!(lazy.placed_indices(), expected, "{param:?}");
            assert_eq!((lazy.visible)(), vec![4, 5, 6, 7], "{param:?}");
            Some(true)
        });
        assert_eq!(ran, 1, "{param:?}");
        lazy.ui.wait_for_idle();
        assert_eq!(lazy.placed_indices(), vec![4, 5, 6, 7], "{param:?}");
    }
}

/// Re-entrant, as Compose's intervals: a layout inside another's block keeps the outer's items,
/// and only the outer's stay once it returns.
#[test]
fn a_layout_inside_another_keeps_its_items() {
    for kind in KINDS {
        let param = Param { direction: Below, reverse: false, rtl: false };
        let mut lazy = show(kind, param, false, 30.0, 5, five_one_five());
        let layout = lazy.layout();
        let inner = layout.clone();
        let mut runs = 0;
        layout.layout(Below, |_| {
            runs += 1;
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7, 8], "{kind:?}");
            let mut left = 2;
            inner.layout(Above, |_| {
                left -= 1;
                if left > 0 {
                    return None;
                }
                assert_eq!(lazy.placed_indices(), vec![3, 4, 5, 6, 7, 8], "{kind:?}");
                Some(())
            });
            assert_eq!(lazy.placed_indices(), vec![5, 6, 7, 8], "{kind:?}: the inner one's went");
            Some(())
        });
        assert_eq!(runs, 1);
        lazy.ui.wait_for_idle();
        assert_eq!(lazy.placed_indices(), vec![5, 6, 7], "{kind:?}: the outer one's went");
    }
}

/// A layout whose list left: its block is told there's nothing more, as Compose's when its node
/// isn't attached.
#[test]
fn a_layout_whose_list_left_has_no_more_content() {
    for kind in KINDS {
        let param = Param { direction: After, reverse: false, rtl: false };
        let lazy = show(kind, param, false, 30.0, 5, five_one_five());
        let layout = lazy.layout();
        // The window goes, its composition disposed.
        drop(lazy);
        let mut told = Vec::new();
        layout.layout(After, |scope| {
            told.push(scope.has_more_content());
            Some(())
        });
        assert_eq!(told, vec![false], "{kind:?}");
    }
}

/// At most two viewports of items, as Compose's `BeyondBoundsViewportFactor`, however much more
/// there is.
#[test]
fn a_layout_lays_out_at_most_two_viewports() {
    for kind in KINDS {
        let param = Param { direction: After, reverse: false, rtl: false };
        let lazy = show(kind, param, false, 30.0, 5, squares(100, Some(5)));
        let layout = lazy.layout();
        let mut runs = 0;
        let found: Option<()> = layout.layout(After, |scope| {
            runs += 1;
            assert!(scope.has_more_content());
            None
        });
        assert_eq!((found, runs), (None, 6), "{kind:?}");
    }
}

/// Compose's lazy layouts take `lazyLayoutBeyondBoundsModifier` only with user scrolling on.
#[test]
fn a_list_the_user_cannot_scroll_provides_none() {
    for enabled in [true, false] {
        let captured: Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>> = Rc::default();
        let capturing = captured.clone();
        let _ui = window(move || {
            let capturing = capturing.clone();
            LazyColumn!(
                modifier = Modifier.size(30.dp()),
                user_scroll_enabled = enabled,
                move |scope: LazyListScope| {
                    let capturing = capturing.clone();
                    scope.items(10, move |_| {
                        Box!(
                            modifier =
                                Modifier.size(10.dp()).then_custom(Capturing(capturing.clone()))
                        );
                    });
                }
            );
        });
        assert_eq!(captured.borrow().is_some(), enabled);
    }
}

// ---- D1480 --------------------------------------------------------------------------------------

type Told = Rc<RefCell<Vec<bool>>>;
type Found = Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>>;

/// A `LazyColumn` of 100 focusable 10 px rows tagged `item{index}` in 30 px at item 5, item 5
/// finding the layout; with `asking`, a block never satisfied asks for it in a composition once
/// the state is set, what it's told kept.
fn focusable_column(asking: Option<(MutableState<bool>, Told)>) -> (ComposeTestRule, Found) {
    let captured: Found = Rc::default();
    let capturing = captured.clone();
    let owner = Rc::new(LazyListState::new(5, 0));
    let state = **owner;
    let ui = window(move || {
        let _ = &owner;
        let (capturing, asking) = (capturing.clone(), asking.clone());
        Column!({
            let listed = capturing.clone();
            LazyColumn!(
                modifier = Modifier.size((10.dp(), 30.dp())),
                state = state,
                move |scope: LazyListScope| {
                    let listed = listed.clone();
                    scope.items(100, move |index| {
                        let mut modifier =
                            Modifier.size(10.dp()).test_tag(item_tag(index)).clickable(|| {});
                        if index == 5 {
                            modifier = modifier.then_custom(Capturing(listed.clone()));
                        }
                        Box!(modifier = modifier);
                    });
                }
            );
            if let Some((asking, told)) = &asking
                && asking.get()
                && let Some(layout) = capturing.borrow().clone()
            {
                let found: Option<()> = layout.layout(After, |scope| {
                    told.borrow_mut().push(scope.has_more_content());
                    None
                });
                assert_eq!(found, None);
            }
        });
    });
    (ui, captured)
}

fn placed_items(ui: &mut ComposeTestRule) -> Vec<usize> {
    let owner = ui.host().semantics_owner();
    let mut placed: Vec<usize> = owner
        .get_all_semantics_nodes(false, true)
        .into_iter()
        .filter(|node| node.layout_info().is_placed() && node.layout_info().is_attached())
        .filter_map(|node| {
            let config = node.unmerged_config();
            config.get(&SemanticsProperties::TEST_TAG)?.strip_prefix("item")?.parse().ok()
        })
        .collect();
    placed.sort_unstable();
    placed
}

/// From a busy tree (a composition) the steps are laid out at the next layout, after the block
/// ran: rather than two viewports' worth of steps told there's more, never showing anything, the
/// block is told what there is once, then that there's nothing more, once (D1480).
#[test]
fn a_layout_from_a_busy_tree_ends_told_there_is_nothing_more() {
    let asking_owner = Rc::new(mutable_state_of(false));
    let asking = asking_owner.handle();
    let told: Told = Rc::default();
    let (mut ui, _captured) = focusable_column(Some((asking, told.clone())));
    asking.set(true);
    ui.wait_for_idle();
    asking.set(false);
    ui.wait_for_idle();
    assert_eq!(*told.borrow(), vec![true, false]);
    assert_eq!(placed_items(&mut ui), vec![5, 6, 7], "nothing kept beyond the bounds");
}

/// A layout ending while a focus search waits for the item it asked for (D477): the search's
/// item stays laid out, the two asking for the most of either each way, as Compose's intervals
/// share `beyondBoundsInfo`; the search then moves the focus to it (D1480).
#[test]
fn a_layout_ending_keeps_what_a_waiting_focus_search_asked_for() {
    let (mut ui, captured) = focusable_column(None);
    let layout = captured.borrow().clone().expect("found");
    ui.on_node_with_tag("item7").request_focus();
    ui.wait_for_idle();
    // The search finds nothing more in view and waits for item 8, at the next layout.
    ui.host().send_key(compose_ui::KeyEvent::down(compose_ui::Key::Tab));
    assert!(ui.host().with_tree(|tree| tree.has_beyond_bounds_moves()));
    let mut seen = Vec::new();
    layout.layout(After, |_| {
        seen = placed_items(&mut ui);
        Some(())
    });
    assert_eq!(seen, vec![5, 6, 7, 8]);
    assert_eq!(placed_items(&mut ui), vec![5, 6, 7, 8], "the search's item stays");
    ui.wait_for_idle();
    ui.on_node_with_tag("item8").assert_is_focused();
}

/// A block panicking lets go of what its layout asked for: the next layout lays out nothing
/// beyond the bounds, and a layout after it starts afresh (D1480).
#[test]
fn a_block_panicking_leaves_nothing_beyond_the_bounds() {
    let (mut ui, captured) = focusable_column(None);
    let layout = captured.borrow().clone().expect("found");
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        layout.layout(After, |_| -> Option<()> { panic!("the block's own") });
    }));
    assert!(panicked.is_err());
    ui.wait_for_idle();
    assert_eq!(placed_items(&mut ui), vec![5, 6, 7]);
    let mut seen = Vec::new();
    layout.layout(After, |_| {
        seen = placed_items(&mut ui);
        Some(())
    });
    assert_eq!(seen, vec![5, 6, 7, 8], "one item beyond, as the first step of any layout");
    assert_eq!(placed_items(&mut ui), vec![5, 6, 7]);
}
