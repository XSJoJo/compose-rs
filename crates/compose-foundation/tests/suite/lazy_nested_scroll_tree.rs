//! The lazy layouts and `scrollable_2d` in the tree's nested-scroll chain (D1478): a dispatcher
//! inside an item reaches the list, what it leaves goes on to the parents above it, and its own
//! Page keys and semantics scrolls go through them, as Compose's lazy layouts, which are
//! `scrollable`s. Compose's 2D cases are `Scrollable2DTest`'s.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::gestures::LocalBoxFuture;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    IntSize, LOCAL_LAYOUT_DIRECTION, LayoutDirection, NestedScrollConnection,
    NestedScrollDispatcher, NestedScrollSource, Offset, UnitExt,
};

struct Nothing;
impl NestedScrollConnection for Nothing {}

fn none() -> Rc<dyn NestedScrollConnection> {
    Rc::new(Nothing)
}

/// Adds up what it's offered before the scrolls, the post-scrolls' leftovers and the flings' ends
/// it hears, taking all that's left after a scroll when `takes_the_rest`.
#[derive(Default)]
struct Tally {
    pre: Cell<Offset>,
    post: RefCell<Vec<Offset>>,
    post_flings: Cell<u32>,
    takes_the_rest: bool,
}

impl NestedScrollConnection for Tally {
    fn on_pre_scroll(&self, available: Offset, _: NestedScrollSource) -> Offset {
        self.pre.set(self.pre.get() + available);
        Offset::ZERO
    }
    fn on_post_scroll(&self, _: Offset, available: Offset, _: NestedScrollSource) -> Offset {
        self.post.borrow_mut().push(available);
        if self.takes_the_rest { available } else { Offset::ZERO }
    }
    fn on_post_fling(&self, _: Offset, _: Offset) -> LocalBoxFuture<'_, Offset> {
        self.post_flings.set(self.post_flings.get() + 1);
        Box::pin(async { Offset::ZERO })
    }
}

impl Tally {
    fn left(&self) -> Offset {
        self.post.borrow().iter().fold(Offset::ZERO, |sum, it| sum + *it)
    }
}

fn tally(takes_the_rest: bool) -> (Rc<Tally>, Rc<dyn NestedScrollConnection>) {
    let tally = Rc::new(Tally { takes_the_rest, ..Tally::default() });
    let connection: Rc<dyn NestedScrollConnection> = tally.clone();
    (tally, connection)
}

fn window(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), content);
    ui.wait_for_idle();
    ui
}

/// A 50 px row holding `dispatcher`'s node.
fn dispatching_row(dispatcher: &NestedScrollDispatcher) {
    Box!(
        modifier =
            Modifier.size((100.dp(), 50.dp())).nested_scroll_with(none(), Some(dispatcher.clone()))
    );
}

/// In a parent `connection`, a 300 px `LazyColumn` at `state` of `count` 50 px rows, the
/// dispatcher's in the fourth row shown at first, scrolling by the user when `enabled`.
fn column_in(
    connection: Rc<dyn NestedScrollConnection>,
    state: LazyListState,
    count: usize,
    enabled: bool,
    dispatcher: &NestedScrollDispatcher,
) -> ComposeTestRule {
    let given = dispatcher.clone();
    let at = state.first_visible_item_index() + 3;
    window(move || {
        let given = given.clone();
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let given = given.clone();
            LazyColumn!(
                modifier = Modifier.size((100.dp(), 300.dp())),
                state = state,
                user_scroll_enabled = enabled,
                move |scope: LazyListScope| {
                    let given = given.clone();
                    scope.items(count, move |index| {
                        if index == at {
                            dispatching_row(&given);
                        } else {
                            Box!(modifier = Modifier.size((100.dp(), 50.dp())));
                        }
                    });
                }
            );
        });
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollableTest.kt, scrollable_nestedScrollBelow_listensDispatches (a lazy list is a `scrollable`: the same on a `LazyColumn`)
#[test]
fn a_lazy_column_hears_a_dispatcher_in_an_item() {
    let (parent, connection) = tally(false);
    let list = LazyListState::new(0, 0);
    let state = *list;
    let dispatcher = NestedScrollDispatcher::new();
    let mut ui = column_in(connection, state, 100, true, &dispatcher);
    let source = NestedScrollSource::UserInput;
    let pre = dispatcher.dispatch_pre_scroll(Offset::new(20.0, -20.0), source);
    assert_eq!(pre, Offset::ZERO, "a list takes no pre-scroll");
    assert_eq!(parent.pre.get(), Offset::new(20.0, -20.0), "the parent above it was offered it");
    // A finger going up 70 px: the list scrolls 70 px towards its end.
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(30.0, -70.0), source);
    assert_eq!(taken, Offset::new(0.0, -70.0), "along its axis only");
    assert_eq!(parent.left(), Offset::new(30.0, 0.0), "what's across goes on to the parent");
    ui.wait_for_idle();
    assert_eq!(
        (state.first_visible_item_index(), state.first_visible_item_scroll_offset()),
        (1, 20)
    );

    let scope = dispatcher.coroutine_scope();
    let pre_fling = Rc::new(Cell::new(None));
    let (given, heard) = (dispatcher.clone(), pre_fling.clone());
    scope.launch(async move {
        heard.set(Some(given.dispatch_pre_fling(Offset::new(0.0, -50.0)).await));
    });
    ui.wait_for_idle();
    assert_eq!(pre_fling.get(), Some(Offset::ZERO), "a list takes no pre-fling");
    parent.pre.set(Offset::ZERO);
    scope.launch(async move {
        dispatcher.dispatch_post_fling(Offset::ZERO, Offset::new(0.0, -3000.0)).await;
    });
    ui.wait_for_idle();
    assert!(state.first_visible_item_index() > 3, "it flung");
    assert!(parent.pre.get().y < 0.0, "its fling's deltas were offered to the parent above it");
    assert!(parent.post_flings.get() >= 1, "and its end");
}

/// Kills the mutant: the tree connection scrolling the list raw, outside the scope that tells it
/// the parents above (what an estimated end leaves is then lost).
#[test]
fn what_a_lazy_column_cannot_take_goes_on_to_the_parent_above_it() {
    // Ten rows: 200 px to scroll, the end not laid out at first (an estimate).
    let (parent, connection) = tally(false);
    let list = LazyListState::new(0, 0);
    let state = *list;
    let dispatcher = NestedScrollDispatcher::new();
    let mut ui = column_in(connection, state, 10, true, &dispatcher);
    let source = NestedScrollSource::UserInput;
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -300.0), source);
    assert_eq!(taken, Offset::new(0.0, -300.0), "an estimated end takes it all at first");
    ui.wait_for_idle();
    assert_eq!(
        (state.first_visible_item_index(), state.first_visible_item_scroll_offset()),
        (4, 0)
    );
    assert_eq!(parent.left(), Offset::new(0.0, -100.0), "once laid out, the rest went above");

    // At the end, laid out: the list takes nothing and the parent gets it all at once.
    parent.post.borrow_mut().clear();
    let other = NestedScrollDispatcher::new();
    let (parent, connection) = tally(true);
    let list = LazyListState::new(4, 0);
    let state = *list;
    let mut ui = column_in(connection, state, 10, true, &other);
    let taken = other.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -40.0), source);
    assert_eq!(taken, Offset::new(0.0, -40.0));
    assert_eq!(parent.left(), Offset::new(0.0, -40.0));
    ui.wait_for_idle();
    assert_eq!(
        (state.first_visible_item_index(), state.first_visible_item_scroll_offset()),
        (4, 0)
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollableTest.kt, scrollable_nestedScroll_disabledConnectionNoOp (a `LazyColumn` with `userScrollEnabled = false`)
#[test]
fn a_lazy_column_not_scrolled_by_the_user_takes_nothing_from_a_dispatcher_inside() {
    let (parent, connection) = tally(true);
    let list = LazyListState::new(0, 0);
    let state = *list;
    let dispatcher = NestedScrollDispatcher::new();
    let mut ui = column_in(connection, state, 100, false, &dispatcher);
    let source = NestedScrollSource::UserInput;
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -70.0), source);
    assert_eq!(taken, Offset::new(0.0, -70.0), "the parent above took it");
    assert_eq!(parent.left(), Offset::new(0.0, -70.0));
    let scope = dispatcher.coroutine_scope();
    scope.launch(async move {
        dispatcher.dispatch_post_fling(Offset::ZERO, Offset::new(0.0, -3000.0)).await;
    });
    ui.wait_for_idle();
    assert_eq!(
        (state.first_visible_item_index(), state.first_visible_item_scroll_offset()),
        (0, 0)
    );
    assert_eq!(parent.post_flings.get(), 1, "the fling's end went on above");
}

/// Performs the `ScrollBy` semantics action on `tag` by `(x, y)`.
fn semantics_scroll_by(ui: &mut ComposeTestRule, tag: &str, x: f32, y: f32) {
    let node = ui.on_node_with_tag(tag).fetch_semantics_snapshot();
    let handled = ui.host().perform_semantics_action_with(
        node.id,
        &compose_ui::SemanticsActions::SCROLL_BY,
        |scroll| scroll(x, y),
    );
    assert!(handled);
    ui.wait_for_idle();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollableTest.kt, scrollable_nestedScroll_childPartialConsumptionForSemantics_vertical (the child a `vertical_scroll` at its end in a `LazyColumn`'s item, which takes the rest)
#[test]
fn a_semantics_scroll_inside_a_lazy_item_leaves_its_rest_to_the_list() {
    let list = LazyListState::new(0, 0);
    let state = *list;
    let inner = ScrollState::new(0);
    let column = *inner;
    let mut ui = window(move || {
        LazyColumn!(
            modifier = Modifier.size((100.dp(), 300.dp())),
            state = state,
            move |scope: LazyListScope| {
                scope.item(move || {
                    // Its content fits: it can't scroll.
                    Column!(
                        modifier = Modifier
                            .test_tag("inner")
                            .size((100.dp(), 100.dp()))
                            .vertical_scroll(column),
                        {
                            Box!(modifier = Modifier.size((100.dp(), 100.dp())));
                        }
                    );
                });
                scope.items(50, |_| Box!(modifier = Modifier.size((100.dp(), 50.dp()))));
            }
        );
    });
    semantics_scroll_by(&mut ui, "inner", 0.0, 60.0);
    assert_eq!(column.value(), 0);
    assert_eq!(
        (state.first_visible_item_index(), state.first_visible_item_scroll_offset()),
        (0, 60)
    );
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollableTest.kt, scrollable_nestedScroll_childPartialConsumptionForKeyboardPageUpAndDown (a `LazyColumn` in a parent, its Page keys through it)
#[test]
fn a_lazy_column_s_page_keys_and_semantics_scrolls_go_through_the_parents_above() {
    let (parent, connection) = tally(false);
    let list = LazyListState::new(0, 0);
    let state = *list;
    let mut ui = window(move || {
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            LazyColumn!(
                modifier = Modifier.test_tag("list").size((100.dp(), 300.dp())),
                state = state,
                |scope: LazyListScope| {
                    scope.item(|| {
                        Box!(modifier = Modifier.size(50.dp()).test_tag("focus").focusable());
                    });
                    scope.items(50, |_| Box!(modifier = Modifier.size((100.dp(), 50.dp()))));
                }
            );
        });
    });
    ui.on_node_with_tag("focus").request_focus();
    ui.wait_for_idle();
    ui.press_key(compose_ui::Key::PageDown);
    ui.wait_for_idle();
    assert_eq!(parent.pre.get(), Offset::new(0.0, -300.0), "a page down, a drag up");
    assert_eq!(state.first_visible_item_index(), 6);
    parent.pre.set(Offset::ZERO);
    semantics_scroll_by(&mut ui, "list", 0.0, 75.0);
    assert_eq!(parent.pre.get(), Offset::new(0.0, -75.0));
    assert_eq!(
        (state.first_visible_item_index(), state.first_visible_item_scroll_offset()),
        (7, 25)
    );
}

/// Kills the mutants: a lazy row's tree connection or its semantics scroll ignoring the layout's
/// direction or the list's reversal.
#[test]
fn a_dispatcher_s_rest_scrolls_a_row_the_way_a_drag_would() {
    // (right to left, reversed, a finger's way that scrolls it towards its end)
    for (rtl, reverse, towards_end) in
        [(false, false, -1.0), (true, false, 1.0), (false, true, 1.0), (true, true, -1.0)]
    {
        let list = LazyListState::new(0, 0);
        let state = *list;
        let dispatcher = NestedScrollDispatcher::new();
        let given = dispatcher.clone();
        let direction = if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let mut ui = window(move || {
            let given = given.clone();
            CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(direction), move || {
                let given = given.clone();
                LazyRow!(
                    modifier = Modifier.test_tag("row").size((300.dp(), 100.dp())),
                    state = state,
                    reverse_layout = reverse,
                    move |scope: LazyListScope| {
                        let given = given.clone();
                        scope.items(100, move |index| {
                            if index == 1 {
                                Box!(
                                    modifier = Modifier
                                        .size((50.dp(), 100.dp()))
                                        .nested_scroll_with(none(), Some(given.clone()))
                                );
                            } else {
                                Box!(modifier = Modifier.size((50.dp(), 100.dp())));
                            }
                        });
                    }
                );
            });
        });
        let case = format!("rtl {rtl}, reversed {reverse}");
        let taken = dispatcher.dispatch_post_scroll(
            Offset::ZERO,
            Offset::new(30.0 * towards_end, 0.0),
            NestedScrollSource::UserInput,
        );
        assert_eq!(taken, Offset::new(30.0 * towards_end, 0.0), "{case}");
        ui.wait_for_idle();
        assert_eq!(state.first_visible_item_scroll_offset(), 30, "{case}");
        // Its semantics scroll goes towards its end whatever the direction, as before.
        semantics_scroll_by(&mut ui, "row", 10.0, 0.0);
        assert_eq!(state.first_visible_item_scroll_offset(), 40, "{case}");
    }
}

/// A dispatcher in the first item of a grid, a staggered grid and a pager scrolls it.
#[test]
fn a_grid_a_staggered_grid_and_a_pager_hear_a_dispatcher_inside() {
    let source = NestedScrollSource::UserInput;
    let item = |dispatcher: &NestedScrollDispatcher, index: usize| {
        if index == 0 {
            dispatching_row(dispatcher);
        } else {
            Box!(modifier = Modifier.size((100.dp(), 50.dp())));
        }
    };

    let grid = LazyGridState::new(0, 0);
    let state = *grid;
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let given = given.clone();
        LazyVerticalGrid!(
            GridCells::Fixed(2),
            modifier = Modifier.size((200.dp(), 300.dp())),
            state = state,
            move |scope: LazyGridScope| {
                let given = given.clone();
                scope.items(100, move |index| item(&given, index));
            }
        );
    });
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -20.0), source);
    assert_eq!(taken, Offset::new(0.0, -20.0));
    ui.wait_for_idle();
    assert_eq!(state.first_visible_item_scroll_offset(), 20, "the grid");

    let staggered = LazyStaggeredGridState::new(0, 0);
    let state = *staggered;
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let given = given.clone();
        LazyVerticalStaggeredGrid!(
            StaggeredGridCells::Fixed(2),
            modifier = Modifier.size((200.dp(), 300.dp())),
            state = state,
            move |scope: LazyStaggeredGridScope| {
                let given = given.clone();
                scope.items(100, move |index| item(&given, index));
            }
        );
    });
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(0.0, -20.0), source);
    assert_eq!(taken, Offset::new(0.0, -20.0));
    ui.wait_for_idle();
    assert_eq!(state.first_visible_item_scroll_offset(), 20, "the staggered grid");

    // A pager's own connection, inside it, takes what a dispatcher in a page leaves along it
    // outside a gesture, as Compose's (`isAbsorbNonGestureNestedScrollInPagerFixEnabled`); what
    // goes across passes the pager to the parent above. Its semantics scroll goes through that
    // parent.
    let (parent, connection) = tally(false);
    let pager = PagerState::new(0, 0.0, || 5);
    let state = *pager;
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let given = given.clone();
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let given = given.clone();
            HorizontalPager!(
                state,
                modifier = Modifier.test_tag("pager").size((300.dp(), 100.dp())),
                page_content = move |_, page| {
                    if page == 0 {
                        Box!(
                            modifier = Modifier
                                .size(100.dp())
                                .nested_scroll_with(none(), Some(given.clone()))
                        );
                    }
                }
            );
        });
    });
    let taken = dispatcher.dispatch_post_scroll(Offset::ZERO, Offset::new(-60.0, -40.0), source);
    assert_eq!(taken, Offset::new(-60.0, 0.0));
    assert_eq!(parent.left(), Offset::new(0.0, -40.0));
    ui.wait_for_idle();
    assert_eq!(state.current_page_offset_fraction(), 0.0);
    semantics_scroll_by(&mut ui, "pager", 60.0, 0.0);
    assert_eq!(parent.pre.get(), Offset::new(-60.0, 0.0), "the pager's semantics scroll");
    assert!(
        (state.current_page_offset_fraction() - 0.2).abs() < 0.001,
        "the pager: {}",
        state.current_page_offset_fraction()
    );
}

// ---- scrollable_2d ------------------------------------------------------------------------------

/// A 2D state adding up a `fraction` of each delta.
fn consuming_2d(fraction: f32) -> (DefaultScrollable2DState, Rc<Cell<Offset>>) {
    let total = Rc::new(Cell::new(Offset::ZERO));
    let adding = total.clone();
    let state = scrollable_2d_state(move |delta: Offset| {
        let used = Offset::new(delta.x * fraction, delta.y * fraction);
        adding.set(adding.get() + used);
        used
    });
    (state, total)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DTest.kt, scrollable_nestedScroll_childPartialConsumptionForSemantics_horizontal
#[test]
fn a_2d_semantics_scroll_s_rest_scrolls_the_2d_scrollable_around() {
    let (outer, outer_total) = consuming_2d(1.0);
    let (inner, inner_total) = consuming_2d(0.5);
    let mut ui = window(move || {
        let inner = inner.clone();
        Box!(
            modifier = Modifier.size(300.dp()).scrollable_2d(outer.clone(), true, None, None, None),
            content_alignment = Alignment::CENTER,
            {
                Box!(
                    modifier = Modifier.test_tag("inner").size(300.dp()).scrollable_2d(
                        inner.clone(),
                        true,
                        None,
                        None,
                        None
                    )
                );
            }
        );
    });
    semantics_scroll_by(&mut ui, "inner", 200.0, 200.0);
    let (inner, outer) = (inner_total.get(), outer_total.get());
    assert!(inner.x > 0.0 && inner.y > 0.0, "{inner:?}");
    assert!(outer.x > 0.0 && outer.y > 0.0, "{outer:?}");
    assert_eq!(inner, outer);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DTest.kt, scrollable_nestedScrollBelow_listensDispatches
#[test]
fn a_2d_scrollable_hears_a_dispatcher_inside_it() {
    let (state, total) = consuming_2d(0.3);
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let given = given.clone();
        Box!(
            modifier = Modifier.size(300.dp()).scrollable_2d(state.clone(), true, None, None, None),
            {
                Box!(
                    modifier =
                        Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                );
            }
        );
    });
    let source = NestedScrollSource::UserInput;
    let pre = dispatcher.dispatch_pre_scroll(Offset::new(20.0, 20.0), source);
    assert_eq!(pre, Offset::ZERO, "a 2D scrollable takes no pre-scroll");
    let taken =
        dispatcher.dispatch_post_scroll(Offset::new(20.0, 20.0), Offset::new(50.0, 50.0), source);
    assert!((taken.x - 15.0).abs() < 0.001 && (taken.y - 15.0).abs() < 0.001, "{taken:?}");
    let before_fling = total.get();
    let scope = dispatcher.coroutine_scope();
    let pre_fling = Rc::new(Cell::new(None));
    let (fling_dispatcher, heard) = (dispatcher.clone(), pre_fling.clone());
    scope.launch(async move {
        heard.set(Some(fling_dispatcher.dispatch_pre_fling(Offset::new(50.0, 50.0)).await));
    });
    ui.wait_for_idle();
    assert_eq!(pre_fling.get(), Some(Offset::ZERO), "a 2D scrollable takes no pre-fling");
    scope.launch(async move {
        dispatcher
            .dispatch_post_fling(Offset::new(1000.0, 1000.0), Offset::new(2000.0, 2000.0))
            .await;
    });
    ui.wait_for_idle();
    assert!(total.get().x > before_fling.x, "it flung");
    assert!(total.get().y > before_fling.y, "it flung");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/Scrollable2DTest.kt, onNestedFlingCancelled_shouldResetFlingState
#[test]
fn a_2d_scrollable_flings_again_after_a_child_s_fling_was_cancelled() {
    let (outer, outer_total) = consuming_2d(1.0);
    let (inner, _) = consuming_2d(1.0);
    let dispatcher = NestedScrollDispatcher::new();
    let (given, inner_state) = (dispatcher.clone(), inner.clone());
    let mut ui = window(move || {
        let (given, inner) = (given.clone(), inner.clone());
        Box!(
            modifier = Modifier.size(300.dp()).scrollable_2d(outer.clone(), true, None, None, None),
            content_alignment = Alignment::CENTER,
            {
                Box!(
                    modifier = Modifier
                        .size(200.dp())
                        .nested_scroll_with(none(), Some(given.clone()))
                        .scrollable_2d(inner.clone(), true, None, None, None)
                );
            }
        );
    });
    ui.set_auto_advance(false);
    let fling = |dispatcher: NestedScrollDispatcher, inner: DefaultScrollable2DState| {
        CoroutineScope::new().launch(async move {
            let _ = inner
                .scroll(MutatePriority::Default, move |_| async move {
                    dispatcher.dispatch_pre_fling(Offset::new(10000.0, 10000.0)).await;
                    dispatcher
                        .dispatch_post_fling(Offset::ZERO, Offset::new(10000.0, 10000.0))
                        .await;
                })
                .await;
        })
    };
    let job = fling(dispatcher.clone(), inner_state.clone());
    ui.advance_time_by(std::time::Duration::from_millis(200));
    let flung = outer_total.get();
    assert!(flung.x != 0.0 && flung.y != 0.0, "the outer one flings from its post-fling");
    job.cancel();
    outer_total.set(Offset::ZERO);
    fling(dispatcher, inner_state);
    ui.set_auto_advance(true);
    ui.wait_for_idle();
    let flung = outer_total.get();
    assert!(flung.x != 0.0 && flung.y != 0.0, "it flung again: {flung:?}");
}

/// Kills the mutant: a 2D scrollable's tree fling losing the chain above it.
#[test]
fn a_2d_scrollable_s_tree_fling_offers_its_deltas_to_the_parent_above_it() {
    let (parent, connection) = tally(false);
    let (state, total) = consuming_2d(1.0);
    let dispatcher = NestedScrollDispatcher::new();
    let given = dispatcher.clone();
    let mut ui = window(move || {
        let (given, state) = (given.clone(), state.clone());
        Box!(modifier = Modifier.nested_scroll(connection.clone()), {
            let given = given.clone();
            Box!(
                modifier =
                    Modifier.size(300.dp()).scrollable_2d(state.clone(), true, None, None, None),
                {
                    Box!(
                        modifier =
                            Modifier.size(200.dp()).nested_scroll_with(none(), Some(given.clone()))
                    );
                }
            );
        });
    });
    let scope = dispatcher.coroutine_scope();
    scope.launch(async move {
        dispatcher.dispatch_post_fling(Offset::ZERO, Offset::new(2000.0, 2000.0)).await;
    });
    ui.wait_for_idle();
    assert!(total.get().x > 0.0 && total.get().y > 0.0, "it flung");
    let offered = parent.pre.get();
    assert!(
        offered.x > 0.0 && offered.y > 0.0,
        "its fling's deltas reached the parent: {offered:?}"
    );
    assert!(parent.post_flings.get() >= 1, "and its end");
}
