//! `BringIntoViewRequester`, `bring_into_view_responder` and `BringIntoViewSpec`: Compose's
//! `BringIntoViewScrollableInteractionTest` (each in both orientations) and
//! `BringIntoViewResponderTest`, rewritten, and the animation of the scrolling containers
//! (`ContentInViewNode`, D1376). Density 1.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Alignment, Constraints, IntSize, Offset, Rect, UnitExt};

use crate::scroll_support::{Touch, launch, run};

const PARENT: &str = "parent box";
const CHILD: &str = "child box";

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    Horizontal,
    Vertical,
}

const DIRS: [Dir; 2] = [Dir::Horizontal, Dir::Vertical];

impl Dir {
    /// `main` along the axis, `cross` across.
    fn size(self, main: f32, cross: f32) -> Modifier {
        match self {
            Dir::Horizontal => Modifier.size((main.dp(), cross.dp())),
            Dir::Vertical => Modifier.size((cross.dp(), main.dp())),
        }
    }

    fn scroll(self, state: ScrollState) -> Modifier {
        match self {
            Dir::Horizontal => Modifier.horizontal_scroll(state),
            Dir::Vertical => Modifier.vertical_scroll(state),
        }
    }

    /// The other axis's scroll.
    fn cross_scroll(self, state: ScrollState) -> Modifier {
        match self {
            Dir::Horizontal => Modifier.vertical_scroll(state),
            Dir::Vertical => Modifier.horizontal_scroll(state),
        }
    }

    fn offset(self, along: f32) -> Modifier {
        match self {
            Dir::Horizontal => Modifier.offset(along.dp(), 0.dp()),
            Dir::Vertical => Modifier.offset(0.dp(), along.dp()),
        }
    }

    /// A position `along` the axis.
    fn at(self, along: f32) -> (f32, f32) {
        match self {
            Dir::Horizontal => (along, 0.0),
            Dir::Vertical => (0.0, along),
        }
    }
}

fn window(content: impl Fn() + 'static) -> ComposeTestRule {
    let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), content);
    ui.wait_for_idle();
    ui
}

fn bounds(ui: &mut ComposeTestRule, tag: &str) -> (f32, f32, f32, f32) {
    let rect = ui.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    (rect.left.0, rect.top.0, rect.right.0 - rect.left.0, rect.bottom.0 - rect.top.0)
}

fn position(ui: &mut ComposeTestRule, tag: &str) -> (f32, f32) {
    let (x, y, _, _) = bounds(ui, tag);
    (x, y)
}

fn bring(ui: &mut ComposeTestRule, requester: &BringIntoViewRequester, rect: Option<Rect>) {
    let requester = requester.clone();
    run(ui, async move { requester.bring_into_view(rect).await });
}

fn scroll_to(
    ui: &mut ComposeTestRule,
    state: ScrollState,
    to: impl Fn(ScrollState) -> i32 + 'static,
) {
    run(ui, async move {
        let _ = state.scroll_to(to(state)).await;
    });
}

/// As `assertChildMaxInView`: as much of the child shows as fits in the parent.
fn assert_child_max_in_view(ui: &mut ComposeTestRule) {
    let parent = ui.on_node_with_tag(PARENT).get_unclipped_bounds_in_root();
    let child = ui.on_node_with_tag(CHILD).get_unclipped_bounds_in_root();
    let shown = ui.on_node_with_tag(CHILD).get_bounds_in_root();
    let size = |rect: compose_ui::DpRect| (rect.right.0 - rect.left.0, rect.bottom.0 - rect.top.0);
    let (parent, child) = (size(parent), size(child));
    assert_eq!(size(shown), (parent.0.min(child.0), parent.1.min(child.1)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, noScrollableParent_noChange
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, noScrollableParent_itemNotVisible_noChange
#[test]
fn without_a_scrolling_parent_nothing_moves() {
    for dir in DIRS {
        for at in [0.0, 150.0] {
            let requester = BringIntoViewRequester::new();
            let given = requester.clone();
            let mut ui = window(move || {
                let given = given.clone();
                Box!(modifier = dir.size(100.0, 50.0).test_tag(PARENT), {
                    Box!(
                        modifier = dir
                            .offset(at)
                            .size(50.dp())
                            .bring_into_view_requester(given.clone())
                            .test_tag(CHILD)
                    );
                });
            });
            let before = bounds(&mut ui, CHILD);
            bring(&mut ui, &requester, None);
            assert_eq!(bounds(&mut ui, CHILD), before);
            if at == 0.0 {
                assert_child_max_in_view(&mut ui);
            }
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, itemAtLeadingEdge_alreadyVisible_noChange
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, itemAtTrailingEdge_alreadyVisible_noChange
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, itemAtCenter_alreadyVisible_noChange
#[test]
fn a_visible_child_stays_put() {
    for dir in DIRS {
        for at in [0.0, 50.0, 25.0] {
            let requester = BringIntoViewRequester::new();
            let given = requester.clone();
            let scroll = ScrollState::new(0);
            let state = *scroll;
            let mut ui = window(move || {
                let given = given.clone();
                Box!(
                    modifier = Modifier
                        .test_tag(PARENT)
                        .then(dir.size(100.0, 50.0))
                        .then(dir.scroll(state)),
                    {
                        Box!(
                            modifier = dir
                                .offset(at)
                                .size(50.dp())
                                .bring_into_view_requester(given.clone())
                                .test_tag(CHILD)
                        );
                    }
                );
            });
            let before = bounds(&mut ui, CHILD);
            bring(&mut ui, &requester, None);
            assert_eq!(bounds(&mut ui, CHILD), before);
            assert_child_max_in_view(&mut ui);
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, itemBiggerThanParentAtLeadingEdge_alreadyVisible_noChange
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, itemBiggerThanParentAtTrailingEdge_alreadyVisible_noChange
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, itemBiggerThanParentAtCenter_alreadyVisible_noChange
#[test]
fn a_child_bigger_than_its_parent_spanning_it_stays_put() {
    for dir in DIRS {
        for scrolled in [0, 2, 1] {
            let requester = BringIntoViewRequester::new();
            let given = requester.clone();
            let scroll = ScrollState::new(0);
            let state = *scroll;
            let mut ui = window(move || {
                let given = given.clone();
                Box!(modifier = Modifier.size(50.dp()).test_tag(PARENT).then(dir.scroll(state)), {
                    // A row or a column of three 50 px boxes.
                    Box!(
                        modifier = dir
                            .size(150.0, 50.0)
                            .bring_into_view_requester(given.clone())
                            .test_tag(CHILD)
                    );
                });
            });
            // At the start, the end, the middle.
            scroll_to(&mut ui, state, move |state| state.max_value() * scrolled / 2);
            let before = bounds(&mut ui, CHILD);
            bring(&mut ui, &requester, None);
            assert_eq!(bounds(&mut ui, CHILD), before);
            assert_child_max_in_view(&mut ui);
        }
    }
}

/// A child 50 px long `at` along a 200 px box, in a 100 px scroll scrolled by `to`.
fn scrolled_child(
    dir: Dir,
    at: f32,
    to: impl Fn(ScrollState) -> i32 + 'static,
) -> (ComposeTestRule, BringIntoViewRequester) {
    let requester = BringIntoViewRequester::new();
    let given = requester.clone();
    let scroll = ScrollState::new(0);
    let state = *scroll;
    let mut ui = window(move || {
        let given = given.clone();
        Box!(
            modifier =
                Modifier.test_tag(PARENT).then(dir.size(100.0, 50.0)).then(dir.scroll(state)),
            {
                let given = given.clone();
                Box!(modifier = dir.size(200.0, 50.0), {
                    Box!(
                        modifier = dir
                            .offset(at)
                            .size(50.dp())
                            .bring_into_view_requester(given.clone())
                            .test_tag(CHILD)
                    );
                });
            }
        );
    });
    scroll_to(&mut ui, state, to);
    (ui, requester)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, childBeforeVisibleBounds_parentIsScrolledSoThatLeadingEdgeOfChildIsVisible
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, childAfterVisibleBounds_parentIsScrolledSoThatTrailingEdgeOfChildIsVisible
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, childPartiallyVisible_parentIsScrolledSoThatLeadingEdgeOfChildIsVisible
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, childPartiallyVisible_parentIsScrolledSoThatTrailingEdgeOfChildIsVisible
#[test]
fn a_hidden_child_is_scrolled_to_by_its_nearer_edge() {
    for dir in DIRS {
        // Before the viewport: its leading edge at the start.
        let (mut ui, requester) = scrolled_child(dir, 50.0, |state| state.max_value());
        bring(&mut ui, &requester, None);
        assert_eq!(position(&mut ui, CHILD), (0.0, 0.0));
        assert_child_max_in_view(&mut ui);
        // After it, then partly after it: its trailing edge at the end.
        for to in [0, 1] {
            let (mut ui, requester) =
                scrolled_child(dir, 150.0, move |state| state.max_value() * to);
            bring(&mut ui, &requester, None);
            assert_eq!(position(&mut ui, CHILD), dir.at(50.0));
            assert_child_max_in_view(&mut ui);
        }
        // Partly before it.
        let (mut ui, requester) = scrolled_child(dir, 25.0, |state| state.max_value() / 2);
        bring(&mut ui, &requester, None);
        assert_eq!(position(&mut ui, CHILD), (0.0, 0.0));
        assert_child_max_in_view(&mut ui);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, multipleParentsAreScrolledSoThatChildIsVisible
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, multipleParentsAreScrolledInDifferentDirectionsSoThatChildIsVisible
#[test]
fn every_scrolling_parent_scrolls_to_show_it() {
    for dir in DIRS {
        for crossing in [false, true] {
            let requester = BringIntoViewRequester::new();
            let given = requester.clone();
            let (outer, inner) = (ScrollState::new(0), ScrollState::new(0));
            let (outer_state, inner_state) = (*outer, *inner);
            let mut ui = window(move || {
                let given = given.clone();
                let outer_scroll =
                    if crossing { dir.cross_scroll(outer_state) } else { dir.scroll(outer_state) };
                Box!(
                    modifier =
                        Modifier.test_tag(PARENT).then(dir.size(100.0, 50.0)).then(outer_scroll),
                    {
                        let given = given.clone();
                        let inner_size =
                            if crossing { Modifier.size(100.dp()) } else { dir.size(200.0, 50.0) };
                        Box!(modifier = inner_size.then(dir.scroll(inner_state)), {
                            let given = given.clone();
                            let content = if crossing {
                                Modifier.size(200.dp())
                            } else {
                                dir.size(400.0, 50.0)
                            };
                            Box!(modifier = content, {
                                let offset = if crossing {
                                    Modifier.offset(25.dp(), 25.dp())
                                } else {
                                    dir.offset(25.0)
                                };
                                Box!(
                                    modifier = offset
                                        .size(50.dp())
                                        .bring_into_view_requester(given.clone())
                                        .test_tag(CHILD)
                                );
                            });
                        });
                    }
                );
            });
            scroll_to(&mut ui, inner_state, |state| state.max_value());
            scroll_to(&mut ui, outer_state, |state| state.max_value());
            bring(&mut ui, &requester, None);
            assert_eq!(position(&mut ui, CHILD), (0.0, 0.0));
            assert_child_max_in_view(&mut ui);
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, specifiedPartOfComponentBroughtOnScreen
#[test]
fn the_part_asked_for_is_shown() {
    for dir in DIRS {
        let requester = BringIntoViewRequester::new();
        let given = requester.clone();
        let scroll = ScrollState::new(0);
        let state = *scroll;
        let mut ui = window(move || {
            let given = given.clone();
            Box!(modifier = Modifier.test_tag(PARENT).size(50.dp()).then(dir.scroll(state)), {
                Box!(modifier = dir.size(150.0, 50.0).bring_into_view_requester(given.clone()), {
                    Box!(modifier = Modifier.size(50.dp()).then(dir.offset(50.0)).test_tag(CHILD));
                });
            });
        });
        let (x, y) = dir.at(50.0);
        bring(&mut ui, &requester, Some(Rect::new(x, y, x + 50.0, y + 50.0)));
        assert_eq!(position(&mut ui, CHILD), (0.0, 0.0));
        assert_child_max_in_view(&mut ui);
    }
}

/// A spec that puts what's asked `at` from the container's start, counting its calls.
struct At {
    at: f32,
    asked: Rc<Cell<u32>>,
}

impl BringIntoViewSpec for At {
    fn calculate_scroll_distance(&self, offset: f32, _: f32, _: f32) -> f32 {
        self.asked.set(self.asked.get() + 1);
        offset - self.at
    }
}

// The scroll reads its spec where it's composed: a custom one says how far it goes, asked again
// as it goes.
#[test]
fn a_scroll_asks_the_spec_where_it_is_composed_how_far_to_go() {
    for dir in DIRS {
        let requester = BringIntoViewRequester::new();
        let given = requester.clone();
        let asked = Rc::new(Cell::new(0));
        let spec: Rc<dyn BringIntoViewSpec> = Rc::new(At { at: 30.0, asked: asked.clone() });
        let scroll = ScrollState::new(0);
        let state = *scroll;
        let mut ui = window(move || {
            let given = given.clone();
            CompositionLocalProvider(
                LOCAL_BRING_INTO_VIEW_SPEC.provides(spec.clone()),
                move || {
                    let given = given.clone();
                    Box!(modifier = Modifier.size(200.dp()).then(dir.scroll(state)), {
                        let given = given.clone();
                        Box!(modifier = dir.size(400.0, 50.0), {
                            Box!(
                                modifier = dir
                                    .offset(100.0)
                                    .size(10.dp())
                                    .bring_into_view_requester(given.clone())
                            );
                        });
                    });
                },
            );
        });
        bring(&mut ui, &requester, None);
        assert_eq!(state.value(), 70);
        assert!(asked.get() > 2);
    }
}

/// What a responder is given to read what it shows.
type LocalRect = Rc<dyn Fn() -> Option<Rect>>;

/// What a fake responder does when asked: given what to show, a future.
type OnBringIntoView = Rc<dyn Fn(Rc<dyn Fn() -> Option<Rect>>) -> LocalBoxFuture<'static, ()>>;

/// As the tests' `fakeScrollable`: a responder offsetting the rect for its parents by `offset`,
/// handling a request with `on_bring_into_view`.
struct Fake {
    offset: Offset,
    on_bring_into_view: OnBringIntoView,
}

impl BringIntoViewResponder for Fake {
    fn calculate_rect_for_parent(&self, local_rect: Rect) -> Rect {
        Rect::new(
            local_rect.left + self.offset.x,
            local_rect.top + self.offset.y,
            local_rect.right + self.offset.x,
            local_rect.bottom + self.offset.y,
        )
    }

    fn bring_child_into_view(
        &self,
        local_rect: Rc<dyn Fn() -> Option<Rect>>,
    ) -> LocalBoxFuture<'_, ()> {
        (self.on_bring_into_view)(local_rect)
    }
}

/// `Modifier.fakeScrollable(offset, onBringIntoView)`.
fn fake_scrollable(
    offset: Offset,
    on_bring_into_view: impl Fn(Rc<dyn Fn() -> Option<Rect>>) -> LocalBoxFuture<'static, ()> + 'static,
) -> Modifier {
    let responder: Rc<dyn BringIntoViewResponder> =
        Rc::new(Fake { offset, on_bring_into_view: Rc::new(on_bring_into_view) });
    Modifier.bring_into_view_responder(responder).wrap_content_size_with(Alignment::TOP_START, true)
}

/// A fake scrollable recording the rect it's asked to show.
fn fake(offset: Offset) -> (Modifier, Rc<RefCell<Option<Rect>>>) {
    let asked = Rc::new(RefCell::new(None));
    let into = asked.clone();
    let modifier = fake_scrollable(offset, move |local_rect| {
        *into.borrow_mut() = local_rect();
        Box::pin(async {})
    });
    (modifier, asked)
}

/// A box with `modifier(requester)`: what the requester brings into view.
fn responding(modifier: impl Fn(Modifier) -> Modifier + 'static, rect: Option<Rect>) {
    let requester = BringIntoViewRequester::new();
    let given = requester.clone();
    let mut ui = window(move || {
        Box!(modifier = modifier(Modifier.bring_into_view_requester(given.clone())))
    });
    bring(&mut ui, &requester, rect);
}

const ZERO: Rect = Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, zeroSizedItem_zeroSizedParent_bringIntoView
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_rectInChild
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringChildIntoView_isCalled_whenRectForParentDoesNotReturnInput
#[test]
fn a_responder_is_asked_for_the_rect_in_its_own_coordinates() {
    let (scrolling, asked) = fake(Offset::ZERO);
    responding(move |requester| scrolling.clone().then(requester), None);
    assert_eq!(*asked.borrow(), Some(ZERO));
    let (scrolling, asked) = fake(Offset::ZERO);
    let rect = Rect::new(1.0, 2.0, 3.0, 4.0);
    responding(move |requester| scrolling.clone().then(requester), Some(rect));
    assert_eq!(*asked.borrow(), Some(rect));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_childWithSize
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_childBiggerThanParent
#[test]
fn a_responder_is_asked_for_the_requesters_bounds() {
    let (scrolling, asked) = fake(Offset::ZERO);
    responding(
        move |requester| {
            scrolling.clone().size((20.dp(), 10.dp())).offset(40.dp(), 30.dp()).then(requester)
        },
        None,
    );
    assert_eq!(*asked.borrow(), Some(Rect::new(40.0, 30.0, 60.0, 40.0)));
    let (scrolling, asked) = fake(Offset::ZERO);
    responding(
        move |requester| {
            Modifier.size(1.dp()).then(scrolling.clone()).then(requester).size((20.dp(), 10.dp()))
        },
        None,
    );
    assert_eq!(*asked.borrow(), Some(Rect::new(0.0, 0.0, 20.0, 10.0)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_onlyPropagatesUp
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_propagatesUp_whenRectForParentReturnsInput
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_translatesByCalculateRectForParent
#[test]
fn a_request_goes_up_the_responders_outside_it_each_saying_where() {
    let (parent, parent_asked) = fake(Offset::ZERO);
    let (child, child_asked) = fake(Offset::ZERO);
    responding(move |requester| parent.clone().then(requester).then(child.clone()), None);
    assert_eq!((*parent_asked.borrow(), *child_asked.borrow()), (Some(ZERO), None));
    let (parent, parent_asked) = fake(Offset::ZERO);
    let (child, child_asked) = fake(Offset::ZERO);
    responding(move |requester| parent.clone().then(child.clone()).then(requester), None);
    assert_eq!((*parent_asked.borrow(), *child_asked.borrow()), (Some(ZERO), Some(ZERO)));
    let (parent, parent_asked) = fake(Offset::ZERO);
    let (child, _) = fake(Offset::new(2.0, 3.0));
    responding(move |requester| parent.clone().then(child.clone()).then(requester), None);
    assert_eq!(*parent_asked.borrow(), Some(Rect::new(2.0, 3.0, 2.0, 3.0)));
}

// ---- Requests held by responders --------------------------------------------------------------

/// A request a fake responder holds, as the tests' `CancellableContinuation`: active until it's
/// cancelled, or its future is dropped (the request's task went).
#[derive(Default)]
struct Held {
    ended: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl Held {
    fn is_active(&self) -> bool {
        !self.ended.get()
    }

    fn cancel(&self) {
        self.ended.set(true);
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

struct Holding(Rc<Held>);

impl Future for Holding {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.0.ended.get() {
            return Poll::Ready(());
        }
        *self.0.waker.borrow_mut() = Some(context.waker().clone());
        Poll::Pending
    }
}

impl Drop for Holding {
    fn drop(&mut self) {
        self.0.ended.set(true);
    }
}

/// A fake scrollable holding each request it's asked until it's cancelled, as
/// `fakeScrollable { suspendCancellableCoroutine { requests += it } }`.
fn holding() -> (Modifier, Rc<RefCell<Vec<Rc<Held>>>>) {
    let requests: Rc<RefCell<Vec<Rc<Held>>>> = Rc::default();
    let into = requests.clone();
    let modifier = fake_scrollable(Offset::ZERO, move |_| {
        let held = Rc::new(Held::default());
        into.borrow_mut().push(held.clone());
        Box::pin(Holding(held))
    });
    (modifier, requests)
}

/// Launches `requester.bring_into_view(rect)`: its task, and whether it returned.
fn request(requester: &BringIntoViewRequester, rect: Option<Rect>) -> (Job, Rc<Cell<bool>>) {
    let done = Rc::new(Cell::new(false));
    let (requester, into) = (requester.clone(), done.clone());
    let job = launch(async move {
        requester.bring_into_view(rect).await;
        into.set(true);
    });
    (job, done)
}

/// A box with `modifier.bring_into_view_requester(requester)`.
fn requesting(modifier: Modifier) -> (ComposeTestRule, BringIntoViewRequester) {
    let requester = BringIntoViewRequester::new();
    let given = requester.clone();
    let ui =
        window(move || Box!(modifier = modifier.clone().bring_into_view_requester(given.clone())));
    (ui, requester)
}

fn active(requests: &Rc<RefCell<Vec<Rc<Held>>>>) -> Vec<bool> {
    requests.borrow().iter().map(|held| held.is_active()).collect()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_interruptsCurrentRequest_whenNewRequestOverlapsButNotContainedByCurrent
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_interruptsCurrentRequest_whenNewRequestOutsideCurrent
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_doesNotCancelOngoingRequest_whenInterrupted
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_invokesResponder_whenPreviousRequestStillSuspended
#[test]
fn a_newer_request_reaches_the_responder_while_the_older_goes_on() {
    for newer in [
        Rect::new(5.0, 5.0, 15.0, 15.0),
        Rect::new(15.0, 15.0, 20.0, 20.0),
        Rect::new(20.0, 20.0, 30.0, 30.0),
    ] {
        let (scrollable, requests) = holding();
        let (mut ui, requester) = requesting(scrollable);
        request(&requester, Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
        ui.wait_for_idle();
        assert_eq!(active(&requests), [true]);
        request(&requester, Some(newer));
        ui.wait_for_idle();
        // The responder is asked again; what it does with the older one is its own business.
        assert_eq!(active(&requests), [true, true]);
        assert!(!Rc::ptr_eq(&requests.borrow()[0], &requests.borrow()[1]));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_childResponderNotCancelled_whenParentCancelled
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_parentResponderNotCancelled_whenChildCancelled
#[test]
fn one_responder_cancelled_leaves_the_other_going() {
    for parent_cancelled in [true, false] {
        let (parent, parent_requests) = holding();
        let (child, child_requests) = holding();
        let (mut ui, requester) = requesting(parent.then(child));
        request(&requester, Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
        ui.wait_for_idle();
        assert_eq!((active(&parent_requests), active(&child_requests)), (vec![true], vec![true]));
        let (cancelled, other) = if parent_cancelled {
            (&parent_requests, &child_requests)
        } else {
            (&child_requests, &parent_requests)
        };
        cancelled.borrow()[0].cancel();
        ui.wait_for_idle();
        assert_eq!(active(other), [true]);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringIntoView_invokesParent_whenPreviousRequestStillSuspended
#[test]
fn a_newer_request_reaches_the_parent_while_the_child_holds_the_older() {
    let (parent, requests) = holding();
    // A child that never ends a request.
    let child = fake_scrollable(Offset::ZERO, |_| Box::pin(std::future::pending()));
    let (mut ui, requester) = requesting(parent.then(child));
    request(&requester, Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
    ui.wait_for_idle();
    assert_eq!(active(&requests), [true]);
    request(&requester, Some(Rect::new(20.0, 20.0, 30.0, 30.0)));
    ui.wait_for_idle();
    assert_eq!(active(&requests), [true, true]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewResponderTest.kt, bringChildIntoView_calledConcurrentlyOnAllResponders
#[test]
fn the_responders_run_alongside_and_end_with_the_request() {
    let (parent, parent_requests) = holding();
    let (child, child_requests) = holding();
    let (mut ui, requester) = requesting(parent.then(child));
    let (job, done) = request(&requester, None);
    ui.wait_for_idle();
    // Both started, neither waits for the other.
    assert_eq!((active(&parent_requests), active(&child_requests)), (vec![true], vec![true]));
    job.cancel();
    ui.wait_for_idle();
    assert_eq!((active(&parent_requests), active(&child_requests)), (vec![false], vec![false]));
    assert!(!done.get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, doesNotCrashWhenCoordinatesDetachedDuringOperation
#[test]
fn a_rect_read_after_the_requester_left_is_none() {
    let asked: Rc<RefCell<Vec<LocalRect>>> = Rc::default();
    let into = asked.clone();
    let responder = fake_scrollable(Offset::ZERO, move |local_rect| {
        into.borrow_mut().push(local_rect);
        Box::pin(async {})
    });
    let requester = BringIntoViewRequester::new();
    let given = requester.clone();
    let attach = mutable_state_of(true);
    let read = attach.handle();
    let mut ui = window(move || {
        if read.get() {
            Box!(
                modifier = responder.clone().bring_into_view_requester(given.clone()).size(10.dp())
            );
            let given = given.clone();
            LaunchedEffect((), async move {
                // A frame for the nodes to be placed.
                with_frame_millis(|_| ()).await;
                given.bring_into_view(None).await;
            });
        }
    });
    ui.wait_for_idle();
    assert_eq!(asked.borrow().len(), 1);
    assert_eq!(asked.borrow()[0](), Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
    attach.set(false);
    ui.wait_for_idle();
    assert_eq!(asked.borrow()[0](), None);
}

// ---- The animation -----------------------------------------------------------------------------

/// Three boxes inside each other, 100, 50 and 25 px from their start, the largest past a 100 px
/// scroll, each with its requester.
fn nested_requesters(dir: Dir) -> (ComposeTestRule, [BringIntoViewRequester; 3]) {
    let requesters = [(); 3].map(|_| BringIntoViewRequester::new());
    let given = requesters.clone();
    let ui = window(move || {
        let given = given.clone();
        let scroll = remember_scroll_state(0);
        Box!(modifier = Modifier.test_tag(PARENT).size(100.dp()).then(dir.scroll(scroll)), {
            let given = given.clone();
            let before = match dir {
                Dir::Horizontal => Modifier.padding((100.dp(), 0.dp(), 0.dp(), 0.dp())),
                Dir::Vertical => Modifier.padding((0.dp(), 100.dp(), 0.dp(), 0.dp())),
            };
            Box!(modifier = before.size(100.dp()).bring_into_view_requester(given[0].clone()), {
                let given = given.clone();
                Box!(
                    modifier = Modifier.size(50.dp()).bring_into_view_requester(given[1].clone()),
                    {
                        Box!(
                            modifier =
                                Modifier.size(25.dp()).bring_into_view_requester(given[2].clone())
                        );
                    }
                );
            });
        });
    });
    (ui, requesters)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoView_concurrentOverlappingRequests_completeInGeometricOrder
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoView_concurrentOverlappingRequests_completeInGeometricOrder_whenReversed
#[test]
fn requests_inside_each_other_complete_innermost_first() {
    for dir in DIRS {
        // Asked largest first, then smallest first.
        for order in [[0, 1, 2], [2, 1, 0]] {
            let (mut ui, requesters) = nested_requesters(dir);
            let completed: Rc<RefCell<Vec<usize>>> = Rc::default();
            for index in order {
                let (requester, into) = (requesters[index].clone(), completed.clone());
                launch(async move {
                    requester.bring_into_view(None).await;
                    into.borrow_mut().push(index);
                });
            }
            ui.wait_for_idle();
            assert_eq!(*completed.borrow(), [2, 1, 0]);
        }
    }
}

/// A child `long` px long `at` along 400 px of content in a 100 px scroll, which is shown while
/// `scrolls` says so (the scroll leaves, the child stays).
fn animated_child(
    dir: Dir,
    at: f32,
    scrolls: MutableState<bool>,
) -> (ComposeTestRule, BringIntoViewRequester, ScrollState) {
    let requester = BringIntoViewRequester::new();
    let given = requester.clone();
    let scroll = ScrollState::new(0);
    let state = *scroll;
    let ui = window(move || {
        let _owner = &scroll;
        let given = given.clone();
        let scrolling = if scrolls.get() { dir.scroll(state) } else { Modifier };
        Box!(modifier = Modifier.test_tag(PARENT).then(dir.size(100.0, 100.0)).then(scrolling), {
            let given = given.clone();
            Box!(modifier = dir.size(400.0, 100.0), {
                Box!(
                    modifier = dir
                        .offset(at)
                        .size(50.dp())
                        .bring_into_view_requester(given.clone())
                        .test_tag(CHILD)
                );
            });
        });
    });
    (ui, requester, state)
}

// compose-rs's own: the scroll goes there over frames, and the request returns once it's there.
#[test]
fn a_scroll_animates_to_what_is_asked() {
    for dir in DIRS {
        let scrolls = mutable_state_of(true);
        let (mut ui, requester, state) = animated_child(dir, 300.0, scrolls.handle());
        ui.set_auto_advance(false);
        let (_, done) = request(&requester, None);
        for _ in 0..4 {
            ui.advance_time_by_frame();
        }
        let midway = state.value();
        assert!(0 < midway && midway < 250, "{midway}");
        assert!(!done.get());
        ui.set_auto_advance(true);
        ui.wait_for_idle();
        assert_eq!((state.value(), done.get()), (250, true));
        assert_child_max_in_view(&mut ui);
    }
}

// compose-rs's own: a drag takes the scroll over, and the request is cancelled.
#[test]
fn a_drag_interrupts_the_animation() {
    for dir in DIRS {
        let scrolls = mutable_state_of(true);
        let (mut ui, requester, state) = animated_child(dir, 300.0, scrolls.handle());
        ui.set_auto_advance(false);
        let (_, done) = request(&requester, None);
        for _ in 0..4 {
            ui.advance_time_by_frame();
        }
        let mut finger = Touch::down(&mut ui, Offset::new(50.0, 50.0));
        let along = |by: f32| match dir {
            Dir::Horizontal => Offset::new(by, 0.0),
            Dir::Vertical => Offset::new(0.0, by),
        };
        finger.move_by(&mut ui, along(-20.0));
        finger.move_by(&mut ui, along(-5.0));
        ui.advance_time_by_frame();
        assert!(done.get());
        let held = state.value();
        for _ in 0..10 {
            ui.advance_time_by_frame();
        }
        assert_eq!(state.value(), held);
        assert!(held < 250);
        finger.up(&mut ui);
    }
}

// compose-rs's own: a container leaving cancels what it was showing.
#[test]
fn a_scroll_leaving_cancels_its_requests() {
    for dir in DIRS {
        let scrolls = mutable_state_of(true);
        let (mut ui, requester, state) = animated_child(dir, 300.0, scrolls.handle());
        ui.set_auto_advance(false);
        let (_, done) = request(&requester, None);
        for _ in 0..4 {
            ui.advance_time_by_frame();
        }
        assert!(!done.get());
        scrolls.set(false);
        ui.advance_time_by_frame();
        assert!(done.get());
        let held = state.value();
        for _ in 0..10 {
            ui.advance_time_by_frame();
        }
        assert_eq!(state.value(), held);
    }
}

// compose-rs's own: a newer request beside the one being shown interrupts it, and is shown.
#[test]
fn a_request_beside_the_one_shown_interrupts_it() {
    for dir in DIRS {
        let requester = BringIntoViewRequester::new();
        let other = BringIntoViewRequester::new();
        let (given, given_other) = (requester.clone(), other.clone());
        let scroll = ScrollState::new(250);
        let state = *scroll;
        let mut ui = window(move || {
            let _owner = &scroll;
            let (given, given_other) = (given.clone(), given_other.clone());
            Box!(modifier = dir.size(100.0, 100.0).then(dir.scroll(state)), {
                let (given, given_other) = (given.clone(), given_other.clone());
                Box!(modifier = dir.size(400.0, 100.0), {
                    Box!(
                        modifier =
                            dir.offset(0.0).size(50.dp()).bring_into_view_requester(given.clone())
                    );
                    Box!(
                        modifier = dir
                            .offset(120.0)
                            .size(50.dp())
                            .bring_into_view_requester(given_other.clone())
                    );
                });
            });
        });
        ui.set_auto_advance(false);
        let (_, done) = request(&requester, None);
        for _ in 0..3 {
            ui.advance_time_by_frame();
        }
        let (_, other_done) = request(&other, None);
        // Asked between frames: the older is cancelled at once, the newer goes on.
        ui.wait_for_idle();
        assert_eq!((done.get(), other_done.get()), (true, false));
        ui.set_auto_advance(true);
        ui.wait_for_idle();
        assert!(other_done.get());
        // The animation carries on from where it was going: the other, at 120 to 170, shows.
        assert!((70..=120).contains(&state.value()), "{}", state.value());
    }
}

// compose-rs's own: a request reads where the scroll asked for and not laid out yet will put the
// child, as D1371's did: shown by that scroll, it returns at once.
#[test]
fn a_request_counts_the_scroll_not_laid_out_yet() {
    for dir in DIRS {
        let scrolls = mutable_state_of(true);
        let (mut ui, requester, state) = animated_child(dir, 120.0, scrolls.handle());
        ui.set_auto_advance(false);
        let done = Rc::new(Cell::new(false));
        let into = done.clone();
        launch(async move {
            state.dispatch_raw_delta(100.0);
            requester.bring_into_view(None).await;
            into.set(true);
        });
        ui.wait_for_idle();
        assert!(done.get());
        ui.set_auto_advance(true);
        ui.wait_for_idle();
        assert_eq!(state.value(), 100);
    }
}

// compose-rs's own: a request whose task went is forgotten, and the scroll stops there.
#[test]
fn a_request_withdrawn_stops_the_scroll() {
    for dir in DIRS {
        let scrolls = mutable_state_of(true);
        let (mut ui, requester, state) = animated_child(dir, 300.0, scrolls.handle());
        ui.set_auto_advance(false);
        let (job, _) = request(&requester, None);
        for _ in 0..3 {
            ui.advance_time_by_frame();
        }
        job.cancel();
        // The frame under way, then nothing.
        ui.advance_time_by_frame();
        let held = state.value();
        for _ in 0..10 {
            ui.advance_time_by_frame();
        }
        assert_eq!(state.value(), held);
        assert!(held < 250, "{held}");
    }
}

/// A spec never satisfied: always that many px further.
struct Further(f32);

impl BringIntoViewSpec for Further {
    fn calculate_scroll_distance(&self, _: f32, _: f32, _: f32) -> f32 {
        self.0
    }
}

// compose-rs's own, as Compose's comment says: a scroll that can't go as far as asked ends at its
// edge, and the request returns, instead of asking forever.
#[test]
fn a_scroll_stopped_by_its_edge_ends() {
    for dir in DIRS {
        let requester = BringIntoViewRequester::new();
        let given = requester.clone();
        // Always 50 px further.
        let spec: Rc<dyn BringIntoViewSpec> = Rc::new(Further(50.0));
        let scroll = ScrollState::new(0);
        let state = *scroll;
        let mut ui = window(move || {
            let _owner = &scroll;
            let given = given.clone();
            CompositionLocalProvider(
                LOCAL_BRING_INTO_VIEW_SPEC.provides(spec.clone()),
                move || {
                    let given = given.clone();
                    Box!(modifier = dir.size(100.0, 100.0).then(dir.scroll(state)), {
                        let given = given.clone();
                        Box!(modifier = dir.size(400.0, 100.0), {
                            Box!(
                                modifier =
                                    Modifier.size(10.dp()).bring_into_view_requester(given.clone())
                            );
                        });
                    });
                },
            );
        });
        bring(&mut ui, &requester, None);
        assert_eq!(state.value(), 300);
    }
}

// ---- Through `scrollable(bring_into_view_spec)` -------------------------------------------------

/// The layout of a scroll without its gestures, as the tests' `ScrollingLayoutElement`: the
/// content unbounded along the axis, moved back by the state's value.
fn scrolling_layout(state: ScrollState, dir: Dir) -> Modifier {
    Modifier.layout(move |scope, measurable, constraints: Constraints| {
        let vertical = dir == Dir::Vertical;
        let child = if vertical {
            Constraints::new(constraints.min_width, constraints.max_width, 0, Constraints::INFINITY)
        } else {
            Constraints::new(
                0,
                Constraints::INFINITY,
                constraints.min_height,
                constraints.max_height,
            )
        };
        let placeable = scope.measure(measurable, child);
        let (width, height) = (
            constraints.constrain_width(placeable.width),
            constraints.constrain_height(placeable.height),
        );
        let offset = state.value();
        let (x, y) = if vertical { (0, -offset) } else { (-offset, 0) };
        scope.place(&placeable, x, y);
        scope.layout(width, height)
    })
}

fn orientation(dir: Dir) -> Orientation {
    match dir {
        Dir::Horizontal => Orientation::Horizontal,
        Dir::Vertical => Orientation::Vertical,
    }
}

/// `scrollable(state, orientation, overscrollEffect = null, bringIntoViewSpec = spec)`.
fn scrollable_with_spec(state: ScrollState, dir: Dir, spec: Rc<dyn BringIntoViewSpec>) -> Modifier {
    Modifier.scrollable_overscroll(
        state,
        orientation(dir),
        None,
        true,
        false,
        None,
        None,
        Some(spec),
    )
}

/// A spec checking what it's asked against what's expected, saying no scroll is needed.
struct Expecting {
    expected: (f32, f32, f32),
    asked: Rc<Cell<u32>>,
}

impl BringIntoViewSpec for Expecting {
    fn calculate_scroll_distance(&self, offset: f32, size: f32, container_size: f32) -> f32 {
        assert_eq!((offset, size, container_size), self.expected);
        self.asked.set(self.asked.get() + 1);
        0.0
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoViewScroller_childIsAtTopOfParent_shouldReturnCorrectValues
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoViewScroller_childIsInTheMiddleOfParent_shouldReturnCorrectValues
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoViewScroller_childIsPartOutOfBoundsOfParent_shouldReturnCorrectValues
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoViewScroller_childIsOutOfBoundsOfParent_shouldReturnCorrectValues
#[test]
fn a_scrollable_asks_its_spec_where_the_child_is() {
    for dir in DIRS {
        // What comes before the child, and whether a box comes after it.
        for (before, after) in [(0.0, true), (100.0, true), (195.0, false), (205.0, false)] {
            let requester = BringIntoViewRequester::new();
            let given = requester.clone();
            let asked = Rc::new(Cell::new(0));
            let spec: Rc<dyn BringIntoViewSpec> =
                Rc::new(Expecting { expected: (before, 10.0, 200.0), asked: asked.clone() });
            let scroll = ScrollState::new(0);
            let state = *scroll;
            let mut ui = window(move || {
                let _owner = &scroll;
                let given = given.clone();
                let modifier = Modifier
                    .size(200.dp())
                    .then(scrollable_with_spec(state, dir, spec.clone()))
                    .then(scrolling_layout(state, dir));
                let content = move || {
                    if before > 0.0 {
                        Box!(modifier = Modifier.size(before.dp()));
                    }
                    Box!(
                        modifier = Modifier.size(10.dp()).bring_into_view_requester(given.clone())
                    );
                    if after {
                        Box!(modifier = Modifier.size(100.dp()));
                    }
                };
                match dir {
                    Dir::Horizontal => Row!(modifier = modifier, |_| content()),
                    Dir::Vertical => Column!(modifier = modifier, |_| content()),
                }
            });
            bring(&mut ui, &requester, None);
            assert!(asked.get() > 0);
            assert_eq!(state.value(), 0);
        }
    }
}

/// A spec giving 300, then 150, then 0 from then on, noting the scroll when it first gives 0
/// and counting the animation specs it's asked for.
struct Steps {
    index: Cell<usize>,
    state: ScrollState,
    at_zero: Rc<Cell<Option<i32>>>,
    animations: Rc<Cell<u32>>,
}

impl BringIntoViewSpec for Steps {
    fn scroll_animation_spec(&self) -> compose_animation::AnimationSpec {
        self.animations.set(self.animations.get() + 1);
        compose_animation::spring().into()
    }

    fn calculate_scroll_distance(&self, _: f32, _: f32, _: f32) -> f32 {
        let index = self.index.get();
        self.index.set((index + 1).min(2));
        if index == 2 && self.at_zero.get().is_none() {
            self.at_zero.set(Some(self.state.value()));
        }
        [300.0, 150.0, 0.0][index]
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoViewScroller_shouldStopScrollingWhenReceivingZero
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/relocation/BringIntoViewScrollableInteractionTest.kt, bringIntoViewScroller_shouldUseCustomSpec
#[test]
fn a_scrollable_stops_once_its_spec_says_zero_and_animates_along_its_spec() {
    for dir in DIRS {
        let requester = BringIntoViewRequester::new();
        let given = requester.clone();
        let scroll = ScrollState::new(0);
        let state = *scroll;
        let (at_zero, animations) = (Rc::new(Cell::new(None)), Rc::new(Cell::new(0)));
        let spec: Rc<dyn BringIntoViewSpec> = Rc::new(Steps {
            index: Cell::new(0),
            state,
            at_zero: at_zero.clone(),
            animations: animations.clone(),
        });
        let mut ui = window(move || {
            let _owner = &scroll;
            let given = given.clone();
            Box!(
                modifier =
                    Modifier.size(200.dp()).then(scrollable_with_spec(state, dir, spec.clone())),
                {
                    Box!(
                        modifier = Modifier.size(10.dp()).bring_into_view_requester(given.clone())
                    );
                }
            );
        });
        bring(&mut ui, &requester, None);
        assert_eq!(Some(state.value()), at_zero.get());
        assert_eq!(animations.get(), 1);
    }
}
