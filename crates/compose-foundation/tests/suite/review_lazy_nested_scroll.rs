//! Review tests for `lazy-nested-scroll` (D1478, D1479): the lazy layouts in the tree's
//! nested-scroll chain and their `BeyondBoundsLayout`. Tests failing on a bug are ignored with
//! `review: <bug>`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::gestures::LocalBoxFuture;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::BeyondBoundsLayoutDirection::*;
use compose_ui::{
    BeyondBoundsLayout, IntSize, LOCAL_LAYOUT_DIRECTION, LayoutDirection, ModifierNode,
    ModifierNodeElement, NestedScrollConnection, NestedScrollDispatcher, NestedScrollSource,
    NodeContext, Offset, SemanticsProperties, UnitExt,
};

use crate::scroll_support::{Touch, run};

// ---- Helpers ------------------------------------------------------------------------------------

struct Nothing;
impl NestedScrollConnection for Nothing {}

fn none() -> Rc<dyn NestedScrollConnection> {
    Rc::new(Nothing)
}

/// Adds up what it's offered before the scrolls and the post-scrolls' leftovers.
#[derive(Default)]
struct Tally {
    pre: Cell<Offset>,
    post: RefCell<Vec<Offset>>,
}

impl NestedScrollConnection for Tally {
    fn on_pre_scroll(&self, available: Offset, _: NestedScrollSource) -> Offset {
        self.pre.set(self.pre.get() + available);
        Offset::ZERO
    }
    fn on_post_scroll(&self, _: Offset, available: Offset, _: NestedScrollSource) -> Offset {
        self.post.borrow_mut().push(available);
        Offset::ZERO
    }
    fn on_post_fling(&self, _: Offset, _: Offset) -> LocalBoxFuture<'_, Offset> {
        Box::pin(async { Offset::ZERO })
    }
}

impl Tally {
    fn left(&self) -> Offset {
        self.post.borrow().iter().fold(Offset::ZERO, |sum, it| sum + *it)
    }
}

fn tally() -> (Rc<Tally>, Rc<dyn NestedScrollConnection>) {
    let tally = Rc::new(Tally::default());
    let connection: Rc<dyn NestedScrollConnection> = tally.clone();
    (tally, connection)
}

fn window(size: i32, content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(size, size), content);
    ui.wait_for_idle();
    ui
}

fn position(state: LazyListState) -> (usize, i32) {
    (state.first_visible_item_index(), state.first_visible_item_scroll_offset())
}

type Captured = Rc<RefCell<Option<Rc<dyn BeyondBoundsLayout>>>>;

/// Finds the nearest beyond-bounds layout above it when it's attached, as Compose's
/// `ParentCapturingNode`.
#[derive(Clone)]
struct Capturing(Captured);

struct CapturingNode(Captured);

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

/// The items placed and attached, by their `item{index}` tags.
fn placed(ui: &mut ComposeTestRule) -> Vec<usize> {
    let owner = ui.host().semantics_owner();
    let mut placed: Vec<usize> = owner
        .get_all_semantics_nodes(false, true)
        .into_iter()
        .filter(|node| node.layout_info().is_placed() && node.layout_info().is_attached())
        .filter_map(|node| {
            let config = node.unmerged_config();
            let tag = config.get(&SemanticsProperties::TEST_TAG)?;
            tag.strip_prefix("item")?.parse().ok()
        })
        .collect();
    placed.sort_unstable();
    placed
}

/// A 10 px square tagged `item{index}`, capturing the layout when `index == captures`.
fn square(index: usize, captures: usize, captured: &Captured) {
    let mut modifier = Modifier.size(10.dp()).test_tag(format!("item{index}"));
    if index == captures {
        modifier = modifier.then_custom(Capturing(captured.clone()));
    }
    Box!(modifier = modifier);
}

// ---- D1478: programmatic scrolls and pointer drags ----------------------------------------------

/// A `LazyColumn` of ten 50 px rows (an estimated end at first) in 300 px, inside `connection`,
/// with a nested-scroll node in its first row (so the list takes part in the tree's chain).
fn column_in_parent(
    connection: Rc<dyn NestedScrollConnection>,
    state: LazyListState,
    dispatcher: &NestedScrollDispatcher,
) -> ComposeTestRule {
    let given = dispatcher.clone();
    window(300, move || {
        let given = given.clone();
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let given = given.clone();
            LazyColumn!(
                modifier = Modifier.size((100.dp(), 300.dp())),
                state = state,
                move |scope: LazyListScope| {
                    let given = given.clone();
                    scope.items(10, move |index| {
                        let mut modifier = Modifier.size((100.dp(), 50.dp()));
                        if index == 0 {
                            modifier = modifier.nested_scroll_with(none(), Some(given.clone()));
                        }
                        Box!(modifier = modifier);
                    });
                }
            );
        });
    })
}

/// Compose's `ScrollableState.scrollBy` and `dispatchRawDelta` dispatch no nested scroll: a
/// programmatic scroll past the list's (estimated) end reaches no parent, even with the list in
/// the tree's chain and `dispatch_raw_delta` reading the parents its scope names (D1478).
#[test]
fn a_programmatic_scroll_past_an_estimated_end_reaches_no_parent() {
    let (parent, connection) = tally();
    let list = LazyListState::new(0, 0);
    let state = *list;
    let dispatcher = NestedScrollDispatcher::new();
    let mut ui = column_in_parent(connection, state, &dispatcher);
    // The list joins the chain: a dispatcher in its first row asked for its parent.
    let taken = dispatcher.dispatch_post_scroll(
        Offset::ZERO,
        Offset::new(0.0, -10.0),
        NestedScrollSource::UserInput,
    );
    assert_eq!(taken, Offset::new(0.0, -10.0));
    ui.wait_for_idle();
    assert_eq!(position(state), (0, 10));
    parent.post.borrow_mut().clear();
    parent.pre.set(Offset::ZERO);

    let scrolled = run(&mut ui, async move { state.scroll_by(1000.0).await });
    assert_eq!(scrolled, Ok(1000.0), "an estimated end takes it all");
    ui.wait_for_idle();
    assert_eq!(position(state), (4, 0), "at its end");
    assert_eq!(parent.pre.get(), Offset::ZERO, "no pre-scroll for a programmatic scroll");
    assert!(parent.post.borrow().is_empty(), "nor its rest: {:?}", parent.post.borrow());

    // The same straight through `dispatch_raw_delta`, back past the start.
    let used = state.dispatch_raw_delta(-1000.0);
    ui.wait_for_idle();
    assert!(used < 0.0);
    assert_eq!(position(state), (0, 0));
    assert_eq!(parent.pre.get(), Offset::ZERO);
    assert!(parent.post.borrow().is_empty(), "{:?}", parent.post.borrow());
}

/// A swipe on an inner `LazyColumn` in an outer one's item: the outer list's tree participant
/// (made for a nested-scroll node around the inner list) doesn't hear the pointer's deltas a
/// second time. The outcome is the same as without that node.
#[test]
fn a_swipe_reaches_an_outer_list_once_with_a_tree_participant_made() {
    let run_case = |wrapped: bool| {
        let (parent, connection) = tally();
        let (outer_owner, inner_owner) = (LazyListState::new(0, 0), LazyListState::new(0, 0));
        let (outer, inner) = (*outer_owner, *inner_owner);
        let mut ui = window(300, move || {
            let _ = (&outer_owner, &inner_owner);
            let connection = connection.clone();
            Box!(modifier = Modifier.nested_scroll(connection), {
                LazyColumn!(
                    modifier = Modifier.size((100.dp(), 300.dp())),
                    state = outer,
                    move |scope: LazyListScope| {
                        scope.item(move || {
                            let mut modifier = Modifier.size((100.dp(), 150.dp()));
                            if wrapped {
                                modifier = modifier.nested_scroll(none());
                            }
                            LazyColumn!(
                                modifier = modifier,
                                state = inner,
                                |scope: LazyListScope| {
                                    scope.items(4, |_| {
                                        Box!(modifier = Modifier.size((100.dp(), 50.dp())));
                                    });
                                }
                            );
                        });
                        scope.items(40, |_| Box!(modifier = Modifier.size((100.dp(), 50.dp()))));
                    }
                );
            });
        });
        // Up 120 px in steps, still at the end (no fling): the inner list takes 50, the outer
        // the rest.
        let mut touch = Touch::down(&mut ui, Offset::new(50.0, 100.0));
        for _ in 0..12 {
            touch.move_by(&mut ui, Offset::new(0.0, -10.0));
            ui.wait_for_idle();
        }
        touch.advance_event_time(&mut ui, 1000);
        touch.up(&mut ui);
        ui.wait_for_idle();
        (position(inner), position(outer), parent.pre.get(), parent.left())
    };
    let plain = run_case(false);
    let wrapped = run_case(true);
    assert_eq!(wrapped, plain, "the same with the outer list's participant made");
    assert_eq!(plain.0, (1, 0), "the inner list went to its end");
    // Touch slop takes a few pixels of the first move.
    assert!(plain.1.1 > 0 && plain.1.1 <= 70, "the outer list took the rest once: {plain:?}");
}

/// A `LazyColumn` disabled after its participant was made takes nothing from a dispatcher inside,
/// and enabled again takes it: its update tells its connection (`tell_tree`).
#[test]
fn a_lazy_column_disabled_after_its_participant_was_made_takes_nothing() {
    let (parent, connection) = tally();
    let list = LazyListState::new(0, 0);
    let state = *list;
    let enabled_owner = Rc::new(mutable_state_of(true));
    let enabled = enabled_owner.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(300, move || {
        let _ = &enabled_owner;
        let given = given.clone();
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let given = given.clone();
            LazyColumn!(
                modifier = Modifier.size((100.dp(), 300.dp())),
                state = state,
                user_scroll_enabled = enabled.get(),
                move |scope: LazyListScope| {
                    let given = given.clone();
                    scope.items(100, move |index| {
                        let mut modifier = Modifier.size((100.dp(), 50.dp()));
                        if index == 0 {
                            modifier = modifier.nested_scroll_with(none(), Some(given.clone()));
                        }
                        Box!(modifier = modifier);
                    });
                }
            );
        });
    });
    let source = NestedScrollSource::UserInput;
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -20.0), source);
    assert_eq!(taken, Offset::new(0.0, -20.0));
    ui.wait_for_idle();
    assert_eq!(position(state), (0, 20));

    enabled.set(false);
    ui.wait_for_idle();
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -20.0), source);
    assert_eq!(taken, Offset::ZERO, "disabled, it takes nothing");
    assert_eq!(parent.left(), Offset::new(0.0, -20.0), "the parent was offered it all");
    ui.wait_for_idle();
    assert_eq!(position(state), (0, 20));

    enabled.set(true);
    ui.wait_for_idle();
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -20.0), source);
    assert_eq!(taken, Offset::new(0.0, -20.0), "enabled again");
    ui.wait_for_idle();
    assert_eq!(position(state), (0, 40));
}

/// A `LazyRow` whose layout direction changes after its participant was made scrolls a
/// dispatcher's rest the way a drag would in the new direction (`on_layout_direction_change`).
#[test]
fn a_lazy_row_follows_a_layout_direction_change_after_its_participant_was_made() {
    let list = LazyListState::new(0, 0);
    let state = *list;
    let rtl_owner = Rc::new(mutable_state_of(false));
    let rtl = rtl_owner.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(300, move || {
        let _ = &rtl_owner;
        let given = given.clone();
        let direction = if rtl.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(direction), move || {
            let given = given.clone();
            LazyRow!(
                modifier = Modifier.size((300.dp(), 100.dp())),
                state = state,
                move |scope: LazyListScope| {
                    let given = given.clone();
                    scope.items(100, move |index| {
                        let mut modifier = Modifier.size((50.dp(), 100.dp()));
                        if index == 1 {
                            modifier = modifier.nested_scroll_with(none(), Some(given.clone()));
                        }
                        Box!(modifier = modifier);
                    });
                }
            );
        });
    });
    let source = NestedScrollSource::UserInput;
    // Left to right: a finger going left scrolls towards the end.
    dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(-30.0, 0.0), source);
    ui.wait_for_idle();
    assert_eq!(position(state), (0, 30));

    rtl.set(true);
    ui.wait_for_idle();
    // Right to left: a finger going right scrolls towards the end.
    dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(10.0, 0.0), source);
    ui.wait_for_idle();
    assert_eq!(position(state), (0, 40), "right to left now");
}

/// A `scrollable_2d` disabled after its participant was made takes nothing from a dispatcher
/// inside (its update tells its connection).
#[test]
fn a_2d_scrollable_disabled_after_its_participant_was_made_takes_nothing() {
    let total = Rc::new(Cell::new(Offset::ZERO));
    let adding = total.clone();
    let state = scrollable_2d_state(move |delta: Offset| {
        adding.set(adding.get() + delta);
        delta
    });
    let enabled_owner = Rc::new(mutable_state_of(true));
    let enabled = enabled_owner.handle();
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(300, move || {
        let _ = &enabled_owner;
        let given = given.clone();
        Box!(
            modifier = Modifier.size(300.dp()).scrollable_2d(
                state.clone(),
                enabled.get(),
                None,
                None,
                None
            ),
            {
                Box!(
                    modifier =
                        Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                );
            }
        );
    });
    let source = NestedScrollSource::UserInput;
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(10.0, 10.0), source);
    assert_eq!(taken, Offset::new(10.0, 10.0));
    enabled.set(false);
    ui.wait_for_idle();
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(10.0, 10.0), source);
    assert_eq!(taken, Offset::ZERO, "disabled, it takes nothing");
    assert_eq!(total.get(), Offset::new(10.0, 10.0));
}

// ---- D1479: BeyondBoundsLayout ------------------------------------------------------------------

/// A vertical grid (`staggered` or not) of `cells` cells across, 30 px tall, of `count` 10 px
/// squares, at item `first`, the one at `captures` capturing the layout.
fn grid(
    staggered: bool,
    cells: usize,
    count: usize,
    first: usize,
    captures: usize,
) -> (ComposeTestRule, Captured) {
    let captured: Captured = Rc::default();
    let capturing = captured.clone();
    let width = (10 * cells) as f32;
    let ui = if staggered {
        let owner = Rc::new(LazyStaggeredGridState::new(first, 0));
        let state = **owner;
        window(400, move || {
            let _ = &owner;
            let capturing = capturing.clone();
            LazyVerticalStaggeredGrid!(
                StaggeredGridCells::Fixed(cells),
                modifier = Modifier.size((width.dp(), 30.dp())),
                state = state,
                move |scope: LazyStaggeredGridScope| {
                    for index in 0..count {
                        let capturing = capturing.clone();
                        scope.item(move || square(index, captures, &capturing));
                    }
                }
            );
        })
    } else {
        let owner = Rc::new(LazyGridState::new(first, 0));
        let state = **owner;
        window(400, move || {
            let _ = &owner;
            let capturing = capturing.clone();
            LazyVerticalGrid!(
                GridCells::Fixed(cells),
                modifier = Modifier.size((width.dp(), 30.dp())),
                state = state,
                move |scope: LazyGridScope| {
                    let capturing = capturing.clone();
                    scope.items(count, move |index| square(index, captures, &capturing));
                }
            );
        })
    };
    (ui, captured)
}

/// Runs a block never satisfied `After` and returns how many times it ran and the most items it
/// saw placed.
fn unsatisfied_runs(
    ui: &mut ComposeTestRule,
    layout: &Rc<dyn BeyondBoundsLayout>,
) -> (usize, usize) {
    let mut runs = 0;
    let mut most = 0;
    let found: Option<()> = layout.layout(After, |_| {
        runs += 1;
        most = most.max(placed(ui).len());
        None
    });
    assert_eq!(found, None);
    (runs, most)
}

/// Compose's bound is two viewports' worth along the axis (`itemsPerViewport`: a grid's lines
/// per viewport, a staggered grid's items per lane's viewport): three lines in view, six steps.
/// compose-rs's grid steps by lines, but counts its bound from the items per viewport (the
/// per-item extent `layout_info_written` holds): three times as many lines, 18 steps, 54 items
/// beyond its bounds where Compose's lays out 6.
#[test]
#[ignore = "review: a grid's two-viewport bound counts items per viewport, not lines (cells x too many lines)"]
fn a_grid_of_three_columns_lays_out_at_most_two_viewports_of_lines() {
    let (mut ui, captured) = grid(false, 3, 300, 30, 31);
    let layout = captured.borrow().clone().expect("found");
    assert_eq!(placed(&mut ui), (30..39).collect::<Vec<_>>());
    let (runs, most) = unsatisfied_runs(&mut ui, &layout);
    assert_eq!(runs, 6, "two viewports of three lines");
    assert_eq!(most, 9 + 6 * 3, "six whole lines beyond");
}

/// As above for a staggered grid of three lanes: Compose's bound is the viewport over the
/// average item size, three, so six items; compose-rs's divides by the lanes too (`per_item` is
/// the average over the lanes), 18 items.
#[test]
#[ignore = "review: a staggered grid's two-viewport bound is multiplied by its lanes"]
fn a_staggered_grid_of_three_lanes_lays_out_at_most_two_viewports() {
    let (mut ui, captured) = grid(true, 3, 300, 30, 31);
    let layout = captured.borrow().clone().expect("found");
    assert_eq!(placed(&mut ui), (30..39).collect::<Vec<_>>());
    let (runs, most) = unsatisfied_runs(&mut ui, &layout);
    assert_eq!(runs, 6, "two viewports of items");
    assert_eq!(most, 9 + 6);
}

/// A `LazyColumn` of 100 10 px squares in 30 px at item `first`, `shown` and scrolled by the user
/// as the states say, the square at `captures` capturing the layout.
fn column(
    state: LazyListState,
    shown: MutableState<bool>,
    enabled: MutableState<bool>,
    captures: usize,
) -> (ComposeTestRule, Captured) {
    let captured: Captured = Rc::default();
    let capturing = captured.clone();
    let ui = window(400, move || {
        if !shown.get() {
            return;
        }
        let capturing = capturing.clone();
        LazyColumn!(
            modifier = Modifier.size((10.dp(), 30.dp())),
            state = state,
            user_scroll_enabled = enabled.get(),
            move |scope: LazyListScope| {
                let capturing = capturing.clone();
                scope.items(100, move |index| square(index, captures, &capturing));
            }
        );
    });
    (ui, captured)
}

struct Toggles {
    _owners: (Rc<OwnedState<bool>>, Rc<OwnedState<bool>>),
    shown: MutableState<bool>,
    enabled: MutableState<bool>,
}

fn toggles() -> Toggles {
    let shown = Rc::new(mutable_state_of(true));
    let enabled = Rc::new(mutable_state_of(true));
    Toggles { shown: shown.handle(), enabled: enabled.handle(), _owners: (shown, enabled) }
}

/// Compose's `remeasureSync` removes the items laid out beyond the bounds before `layout`
/// returns; kills the mutant dropping the final remeasure (every port waits for idle first).
#[test]
fn the_items_go_before_the_layout_returns() {
    let owner = LazyListState::new(5, 0);
    let toggles = toggles();
    let (mut ui, captured) = column(*owner, toggles.shown, toggles.enabled, 5);
    let layout = captured.borrow().clone().expect("found");
    let mut seen = Vec::new();
    layout.layout(After, |_| {
        seen = placed(&mut ui);
        Some(())
    });
    assert_eq!(seen, vec![5, 6, 7, 8]);
    assert_eq!(placed(&mut ui), vec![5, 6, 7], "gone at once");
}

/// A list whose node left (its state hoisted, so still there): the block is told there's nothing
/// more, once, as Compose's `!isAttached`. Kills the mutant dropping the remeasurement check.
#[test]
fn a_layout_whose_node_left_with_its_state_kept_is_told_nothing_more() {
    let owner = LazyListState::new(5, 0);
    let toggles = toggles();
    let shown = toggles.shown;
    let (mut ui, captured) = column(*owner, shown, toggles.enabled, 5);
    let layout = captured.borrow().clone().expect("found");
    shown.set(false);
    ui.wait_for_idle();
    let mut told = Vec::new();
    layout.layout(After, |scope| {
        told.push(scope.has_more_content());
        None::<()>
    });
    assert_eq!(told, vec![false]);
}

/// Compose's lazy layouts take `lazyLayoutBeyondBoundsModifier` only while the user can scroll
/// them: turning it off detaches the node, and a layout found before is told nothing more. Here
/// the node stays (only `beyond_bounds_layout` answers `None`), so the layout found before still
/// lays out items beyond the bounds.
#[test]
#[ignore = "review: a BeyondBoundsLayout found before user scrolling went off still lays out items"]
fn a_layout_found_before_user_scrolling_went_off_lays_out_nothing() {
    let owner = LazyListState::new(5, 0);
    let toggles = toggles();
    let enabled = toggles.enabled;
    let (mut ui, captured) = column(*owner, toggles.shown, enabled, 5);
    let layout = captured.borrow().clone().expect("found");
    enabled.set(false);
    ui.wait_for_idle();
    let mut told = Vec::new();
    let mut seen = Vec::new();
    layout.layout(After, |scope| {
        told.push(scope.has_more_content());
        seen = placed(&mut ui);
        Some(())
    });
    assert_eq!(told, vec![false]);
    assert_eq!(seen, vec![5, 6, 7]);
}

/// The list leaving while a block runs (a frame passes in it): the layout ends without a panic,
/// and the list shown again lays out nothing beyond its bounds.
#[test]
fn a_list_leaving_during_the_block_keeps_nothing_beyond_when_it_comes_back() {
    let owner = LazyListState::new(5, 0);
    let toggles = toggles();
    let shown = toggles.shown;
    let (mut ui, captured) = column(*owner, shown, toggles.enabled, 5);
    let layout = captured.borrow().clone().expect("found");
    let mut runs = 0;
    let found: Option<()> = layout.layout(After, |_| {
        runs += 1;
        if runs == 1 {
            shown.set(false);
            ui.wait_for_idle();
            assert_eq!(placed(&mut ui), Vec::<usize>::new());
        }
        None
    });
    assert_eq!(found, None);
    assert!(runs <= 6, "bounded: {runs}");
    shown.set(true);
    ui.wait_for_idle();
    assert_eq!(placed(&mut ui), vec![5, 6, 7]);
}

/// The block scrolling the list (a frame passing in it): the items laid out beyond the bounds are
/// counted from those in view, so the next step lays out one beyond the new ones, and they all go
/// at the end.
#[test]
fn a_block_scrolling_the_list_leaves_nothing_beyond_its_bounds() {
    let owner = LazyListState::new(5, 0);
    let state = *owner;
    let toggles = toggles();
    let (mut ui, captured) = column(state, toggles.shown, toggles.enabled, 5);
    let layout = captured.borrow().clone().expect("found");
    let mut seen = Vec::new();
    let mut runs = 0;
    layout.layout(After, |_| {
        runs += 1;
        if runs == 1 {
            state.dispatch_raw_delta(20.0);
            ui.wait_for_idle();
            return None;
        }
        seen = placed(&mut ui);
        Some(())
    });
    assert_eq!(position(state), (7, 0));
    // Compose's interval (absolute indices: 8, then 9) would place 7 to 10 here: 9 is in view.
    // compose-rs counts two steps from the items now in view: 10 and 11.
    assert_eq!(seen, vec![7, 8, 9, 10, 11], "beyond the new items in view");
    ui.wait_for_idle();
    // 10 stays composed (and so attached) as the prefetch's next item after the forward scroll;
    // the one laid out only for the block goes.
    assert!(!placed(&mut ui).contains(&11), "{:?}", placed(&mut ui));
}

/// A layout asked for from a composition (the tree free or not) doesn't panic, and leaves nothing
/// beyond the bounds.
#[test]
fn a_layout_asked_for_in_a_composition_does_not_panic() {
    let owner = LazyListState::new(5, 0);
    let state = *owner;
    let captured: Captured = Rc::default();
    let capturing = captured.clone();
    let trigger_owner = Rc::new(mutable_state_of(0));
    let trigger = trigger_owner.handle();
    let told: Rc<RefCell<Vec<bool>>> = Rc::default();
    let telling = told.clone();
    let mut ui = window(400, move || {
        let _ = &trigger_owner;
        let (capturing, telling) = (capturing.clone(), telling.clone());
        Column!({
            let telling = telling.clone();
            let capturing = capturing.clone();
            let listed = capturing.clone();
            LazyColumn!(
                modifier = Modifier.size((10.dp(), 30.dp())),
                state = state,
                move |scope: LazyListScope| {
                    let listed = listed.clone();
                    scope.items(100, move |index| square(index, 5, &listed));
                }
            );
            if trigger.get() > 0
                && let Some(layout) = capturing.borrow().clone()
            {
                layout.layout(After, |scope| {
                    telling.borrow_mut().push(scope.has_more_content());
                    Some(())
                });
            }
        });
    });
    trigger.set(1);
    ui.wait_for_idle();
    // One step, told there's more; what it laid out went.
    assert_eq!(*told.borrow(), vec![true], "the block ran once");
    assert_eq!(placed(&mut ui), vec![5, 6, 7]);
    let _ = captured;
}

/// From an event handler (a busy tree), Compose's block sees the item laid out beyond the bounds
/// (its focus search runs there). compose-rs measures the steps at the next layout, after the
/// block ran (D1479): the block runs two viewports' worth of times, told each time there's more,
/// never seeing an item beyond.
#[test]
#[ignore = "review: from a busy tree (an event handler) the block never sees an item beyond the bounds, and is told there's more each time"]
fn a_layout_from_a_click_sees_the_item_beyond() {
    let owner = LazyListState::new(5, 0);
    let state = *owner;
    let captured: Captured = Rc::default();
    let capturing = captured.clone();
    // Item 8 placed: it's laid out beyond the bounds.
    let placed_8 = Rc::new(Cell::new(false));
    let placing = placed_8.clone();
    let result: Rc<RefCell<Option<(usize, Vec<bool>, bool)>>> = Rc::default();
    let into = result.clone();
    let mut ui = window(400, move || {
        let (capturing, placing, into) = (capturing.clone(), placing.clone(), into.clone());
        Column!({
            let (captured, into, seen) = (capturing.clone(), into.clone(), placing.clone());
            Box!(
                modifier = Modifier.size(20.dp()).test_tag("button").clickable(move || {
                    let Some(layout) = captured.borrow().clone() else { return };
                    let mut runs = 0;
                    let mut told = Vec::new();
                    let found = layout.layout(After, |scope| {
                        runs += 1;
                        told.push(scope.has_more_content());
                        seen.get().then_some(())
                    });
                    *into.borrow_mut() = Some((runs, told, found.is_some()));
                })
            );
            let (capturing, placing) = (capturing.clone(), placing.clone());
            LazyColumn!(
                modifier = Modifier.size((10.dp(), 30.dp())),
                state = state,
                move |scope: LazyListScope| {
                    let (capturing, placing) = (capturing.clone(), placing.clone());
                    scope.items(100, move |index| {
                        if index == 8 {
                            let placing = placing.clone();
                            Box!(
                                modifier =
                                    Modifier.size(10.dp()).on_placed(move |_| placing.set(true))
                            );
                        } else {
                            square(index, 5, &capturing);
                        }
                    });
                }
            );
        });
    });
    assert!(!placed_8.get());
    ui.on_node_with_tag("button").perform_click();
    ui.wait_for_idle();
    let (runs, told, found) = result.borrow().clone().expect("clicked");
    assert!(found, "the block saw item 8 placed: {runs} runs, told {told:?}");
    assert_eq!(runs, 1);
}
