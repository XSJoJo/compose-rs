//! `BringIntoViewRequester`, `bring_into_view_responder` and `BringIntoViewSpec`: Compose's
//! `BringIntoViewScrollableInteractionTest` (each in both orientations) and
//! `BringIntoViewResponderTest`, rewritten, the ones that don't wait on an animated scroll.
//! Density 1.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Alignment, IntSize, Offset, Rect, UnitExt};

use crate::scroll_support::run;

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

/// A spec that scrolls `distance` whatever is asked, counting its calls.
struct Fixed {
    distance: f32,
    asked: Rc<Cell<u32>>,
}

impl BringIntoViewSpec for Fixed {
    fn calculate_scroll_distance(&self, _: f32, _: f32, _: f32) -> f32 {
        self.asked.set(self.asked.get() + 1);
        self.distance
    }
}

// The scroll reads its spec where it's composed: a custom one says how far it goes.
#[test]
fn a_scroll_asks_the_spec_where_it_is_composed_how_far_to_go() {
    for dir in DIRS {
        let requester = BringIntoViewRequester::new();
        let given = requester.clone();
        let asked = Rc::new(Cell::new(0));
        let spec: Rc<dyn BringIntoViewSpec> =
            Rc::new(Fixed { distance: 30.0, asked: asked.clone() });
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
                                modifier =
                                    Modifier.size(10.dp()).bring_into_view_requester(given.clone())
                            );
                        });
                    });
                },
            );
        });
        bring(&mut ui, &requester, None);
        assert_eq!((asked.get(), state.value()), (1, 30));
    }
}

/// As the tests' `fakeScrollable`: a responder offsetting the rect for its parents by `offset`,
/// recording what it's asked to show.
struct Fake {
    offset: Offset,
    asked: Rc<RefCell<Option<Rect>>>,
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

    fn bring_child_into_view(&self, local_rect: Rect) {
        *self.asked.borrow_mut() = Some(local_rect);
    }
}

fn fake(offset: Offset) -> (Modifier, Rc<RefCell<Option<Rect>>>) {
    let asked = Rc::new(RefCell::new(None));
    let responder: Rc<dyn BringIntoViewResponder> = Rc::new(Fake { offset, asked: asked.clone() });
    let modifier = Modifier
        .bring_into_view_responder(responder)
        .wrap_content_size_with(Alignment::TOP_START, true);
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
