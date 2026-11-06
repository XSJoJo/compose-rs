//! Compose's `ScrollFocusableInteractionTest`, rewritten: a scroll keeps its focused child in view
//! when its viewport shrinks (`ContentInViewNode.onRemeasured`), and focusing brings a child into
//! view. Each runs vertically, horizontally left to right and right to left, each with and without
//! `reverse_scrolling`. Density 1.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::gestures::{LocalBoxFuture, ScrollBlock};
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    FocusDirection, FocusManager, IntSize, LOCAL_FOCUS_MANAGER, LOCAL_LAYOUT_DIRECTION,
    LayoutDirection, Offset, Rect, UnitExt,
};

use crate::scroll_support::{Touch, run};

const AREA: &str = "scrollableArea";
const FOCUSABLE: &str = "focusable";

#[derive(Clone, Copy, Debug)]
struct Case {
    horizontal: bool,
    rtl: bool,
    reverse: bool,
}

/// Compose's parameters: vertical, horizontal left to right, horizontal right to left, each
/// without and with `reverse_scrolling`.
const CASES: [Case; 6] = [
    Case { horizontal: false, rtl: false, reverse: false },
    Case { horizontal: true, rtl: false, reverse: false },
    Case { horizontal: true, rtl: true, reverse: false },
    Case { horizontal: false, rtl: false, reverse: true },
    Case { horizontal: true, rtl: false, reverse: true },
    Case { horizontal: true, rtl: true, reverse: true },
];

impl Case {
    /// Whether the content is laid out from the viewport's end: for position assertions only.
    fn reverse_layout(self) -> bool {
        let rtl = self.horizontal && self.rtl;
        if self.reverse { !rtl } else { rtl }
    }

    /// `x` along the axis, as the test's `if (reverseLayout) … else …`.
    fn pick(self, reversed: f32, normal: f32) -> f32 {
        if self.reverse_layout() { reversed } else { normal }
    }
}

/// A test's window: the scroll's state, and the focus manager once composed.
struct Scene {
    ui: ComposeTestRule,
    case: Case,
    state: ScrollState,
    manager: Rc<RefCell<Option<FocusManager>>>,
}

/// `ScrollableRowOrColumn(size) { content() }`, `size` read from `viewport`.
fn scene(
    case: Case,
    viewport: Option<MutableState<f32>>,
    size: f32,
    content: impl Fn(Case) + Clone + 'static,
) -> Scene {
    scene_with(case, viewport, size, |modifier| modifier, content)
}

/// [`scene`], with `area` of the scroll's area modifier, before the scroll.
fn scene_with(
    case: Case,
    viewport: Option<MutableState<f32>>,
    size: f32,
    area: impl Fn(Modifier) -> Modifier + 'static,
    content: impl Fn(Case) + Clone + 'static,
) -> Scene {
    let scroll = ScrollState::new(0);
    let state = *scroll;
    let manager: Rc<RefCell<Option<FocusManager>>> = Rc::default();
    let into = manager.clone();
    let area = Rc::new(area);
    let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), move || {
        let _owner = &scroll;
        *into.borrow_mut() = Some(LOCAL_FOCUS_MANAGER.current());
        let size = viewport.map_or(size, |viewport| viewport.get());
        let direction = if case.rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let (content, area) = (content.clone(), area.clone());
        CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(direction), move || {
            let modifier = area(Modifier.test_tag(AREA).size(size.dp()));
            let content = content.clone();
            if case.horizontal {
                let modifier = modifier.horizontal_scroll_with(state, true, None, case.reverse);
                Row!(modifier = modifier, |_| content(case));
            } else {
                let modifier = modifier.vertical_scroll_with(state, true, None, case.reverse);
                Column!(modifier = modifier, |_| content(case));
            }
        });
    });
    ui.wait_for_idle();
    Scene { ui, case, state, manager }
}

/// A focusable of `size`, tagged `tag`.
fn TestFocusable(size: f32, tag: &'static str) {
    Box!(modifier = Modifier.test_tag(tag).size(size.dp()).focusable());
}

/// A focusable of `size` whose focus rectangle is `rect`.
fn TestFocusableWithFocusRect(size: f32, rect: Rect) {
    Box!(
        modifier = Modifier
            .test_tag(FOCUSABLE)
            .size(size.dp())
            .focus_properties(move |properties| properties.focus_rect = rect)
            .focusable()
    );
}

/// `content` after a spacer of `size`, or before it when scrolling in reverse.
fn WithSpacerBefore(case: Case, size: f32, content: impl Fn()) {
    if !case.reverse {
        Spacer!(modifier = Modifier.size(size.dp()));
    }
    content();
    if case.reverse {
        Spacer!(modifier = Modifier.size(size.dp()));
    }
}

impl Scene {
    fn request_focus(&mut self, tag: &str) {
        self.ui.on_node_with_tag(tag).request_focus();
        self.ui.wait_for_idle();
    }

    fn scroll_to_top(&mut self) {
        self.ui.wait_for_idle();
        let state = self.state;
        let _ = run(&mut self.ui, async move { state.scroll_to(0).await });
    }

    fn request_focus_and_scroll_to_top(&mut self) {
        self.request_focus(FOCUSABLE);
        self.scroll_to_top();
    }

    /// Where `tag` starts along the axis, in the root.
    fn position(&mut self, tag: &str) -> f32 {
        let bounds = self.ui.on_node_with_tag(tag).get_unclipped_bounds_in_root();
        if self.case.horizontal { bounds.left.value() } else { bounds.top.value() }
    }

    fn assert_position(&mut self, tag: &str, expected: f32) {
        let case = self.case;
        assert_eq!(self.position(tag), expected, "{case:?}");
    }

    fn displayed(&mut self, tag: &str) -> bool {
        self.ui.on_node_with_tag(tag).is_displayed()
    }

    fn assert_focused(&mut self, tag: &str) {
        self.ui.on_node_with_tag(tag).assert_is_focused();
    }
}

/// A focusable of 10 after a spacer of `before`, in a viewport `viewport` read from a state:
/// focused, then scrolled back to the start.
fn shrinking(case: Case, viewport: f32, before: f32) -> (Scene, OwnedState<f32>) {
    let size = mutable_state_of(viewport);
    let mut scene = scene(case, Some(size.handle()), viewport, move |case| {
        WithSpacerBefore(case, before, || TestFocusable(10.0, FOCUSABLE));
    });
    scene.request_focus_and_scroll_to_top();
    (scene, size)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsFocusedFocusableIntoView_whenFullyInViewAndBecomesFullyHidden
#[test]
fn a_focused_child_hidden_by_a_shrink_is_scrolled_back_into_view() {
    for case in CASES {
        let (mut scene, size) = shrinking(case, 100.0, 90.0);
        scene.assert_position(FOCUSABLE, case.pick(0.0, 90.0));
        assert!(scene.displayed(FOCUSABLE));
        scene.assert_focused(FOCUSABLE);
        size.set(50.0);
        scene.ui.wait_for_idle();
        scene.assert_position(FOCUSABLE, case.pick(0.0, 40.0));
        assert!(scene.displayed(FOCUSABLE), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsFocusedFocusableIntoView_whenFullyInViewAndBecomesPartiallyHidden
#[test]
fn a_focused_child_partly_hidden_by_a_shrink_is_scrolled_back_into_view() {
    for case in CASES {
        let (mut scene, size) = shrinking(case, 100.0, 90.0);
        scene.assert_position(FOCUSABLE, case.pick(0.0, 90.0));
        size.set(95.0);
        scene.ui.wait_for_idle();
        scene.assert_position(FOCUSABLE, case.pick(0.0, 85.0));
        assert!(scene.displayed(FOCUSABLE), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsFocusedFocusableIntoView_whenFullyInViewAndBecomesFullyHidden_usingFocusRect
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsFocusedFocusableIntoView_whenFullyInViewAndBecomesPartiallyHidden_usingFocusRect
#[test]
fn what_is_kept_in_view_is_the_focus_rect() {
    // The focus rect's start and end across the child, the viewport after, and where the child
    // ends up (reversed, not).
    for (start, end, shrunk, expected) in
        [(20.0, 40.0, 50.0, (-20.0, 10.0)), (10.0, 30.0, 70.0, (-10.0, 40.0))]
    {
        for case in CASES {
            let size = mutable_state_of(100.0);
            let rect = Rect::new(start, start, end, end);
            let mut scene = scene(case, Some(size.handle()), 100.0, move |case| {
                WithSpacerBefore(case, 50.0, || TestFocusableWithFocusRect(50.0, rect));
            });
            scene.request_focus_and_scroll_to_top();
            scene.assert_position(FOCUSABLE, case.pick(0.0, 50.0));
            size.set(shrunk);
            scene.ui.wait_for_idle();
            scene.assert_position(FOCUSABLE, case.pick(expected.0, expected.1));
            assert!(scene.displayed(FOCUSABLE), "{case:?}");
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, doesNotScrollFocusedFocusableIntoView_whenPartiallyInViewAndBecomesMoreHidden
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, doesNotScrollFocusedFocusableIntoView_whenPartiallyInViewAndBecomesFullyHidden
#[test]
fn a_focused_child_partly_hidden_before_isnt_followed() {
    for (shrunk, displayed) in [(91.0, true), (90.0, false)] {
        for case in CASES {
            let (mut scene, size) = shrinking(case, 95.0, 90.0);
            scene.assert_position(FOCUSABLE, case.pick(-5.0, 90.0));
            scene.assert_focused(FOCUSABLE);
            size.set(shrunk);
            scene.ui.wait_for_idle();
            // Laid out from the end, it moves with the viewport's end.
            scene.assert_position(FOCUSABLE, case.pick(-5.0 - (95.0 - shrunk), 90.0));
            assert_eq!(scene.displayed(FOCUSABLE), displayed, "{case:?}");
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsFocusedFocusableIntoView_whenViewportAnimatedQuickly
#[test]
fn a_viewport_shrinking_frame_after_frame_keeps_the_focused_child_in_view() {
    for case in CASES {
        let size = mutable_state_of(100.0);
        let animate = mutable_state_of(false);
        let (sizes, animates) = (size.handle(), animate.handle());
        let mut scene = scene(case, Some(size.handle()), 100.0, move |case| {
            WithSpacerBefore(case, 90.0, || TestFocusable(10.0, FOCUSABLE));
            if animates.get() {
                LaunchedEffect((), async move {
                    for to in [80.0, 60.0, 40.0] {
                        with_frame_nanos(|_| sizes.set(to)).await;
                    }
                });
            }
        });
        scene.request_focus_and_scroll_to_top();
        scene.assert_position(FOCUSABLE, case.pick(0.0, 90.0));
        animate.set(true);
        scene.ui.wait_for_idle();
        scene.assert_position(FOCUSABLE, case.pick(0.0, 30.0));
        assert!(scene.displayed(FOCUSABLE), "{case:?}");
    }
}

/// A shrink to 80 starting its animation, a frame of it, then a press and release on the scroll.
fn shrink_then_press(case: Case) -> (Scene, OwnedState<f32>) {
    let (mut scene, size) = shrinking(case, 100.0, 90.0);
    scene.ui.set_auto_advance(false);
    size.set(80.0);
    scene.ui.advance_time_by_frame();
    scene.ui.wait_for_idle();
    let finger = Touch::down(&mut scene.ui, Offset::new(40.0, 40.0));
    finger.up(&mut scene.ui);
    scene.ui.advance_time_by_frame();
    scene.ui.set_auto_advance(true);
    scene.ui.wait_for_idle();
    (scene, size)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollFromViewportShrink_isInterrupted_byGesture
#[test]
fn a_press_interrupts_keeping_the_focused_child_in_view() {
    for case in CASES {
        let (mut scene, _) = shrink_then_press(case);
        assert!(!scene.displayed(FOCUSABLE), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsFocusedFocusableIntoView_whenViewportExpandedThenReshrunk_afterInterruption
#[test]
fn after_an_interruption_a_new_shrink_keeps_it_in_view_again() {
    for case in CASES {
        let (mut scene, size) = shrink_then_press(case);
        scene.assert_focused(FOCUSABLE);
        assert!(!scene.displayed(FOCUSABLE), "{case:?}");
        size.set(100.0);
        scene.ui.wait_for_idle();
        size.set(50.0);
        scene.ui.wait_for_idle();
        assert!(scene.displayed(FOCUSABLE), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, doesNotScrollFocusedFocusableIntoView_whenNotInViewAndViewportShrunk
#[test]
fn a_focused_child_out_of_view_isnt_followed() {
    for case in CASES {
        let (mut scene, size) = shrinking(case, 100.0, 100.0);
        scene.assert_position(FOCUSABLE, case.pick(-10.0, 100.0));
        assert!(!scene.displayed(FOCUSABLE));
        scene.assert_focused(FOCUSABLE);
        size.set(50.0);
        scene.ui.wait_for_idle();
        scene.assert_position(FOCUSABLE, case.pick(-60.0, 100.0));
        assert!(!scene.displayed(FOCUSABLE), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, doesNotScrollUnfocusedFocusableIntoView_whenViewportShrunk
#[test]
fn an_unfocused_child_isnt_followed() {
    for case in CASES {
        let size = mutable_state_of(100.0);
        let mut scene = scene(case, Some(size.handle()), 100.0, move |case| {
            WithSpacerBefore(case, 90.0, || TestFocusable(10.0, FOCUSABLE));
        });
        scene.assert_position(FOCUSABLE, case.pick(0.0, 90.0));
        size.set(50.0);
        scene.ui.wait_for_idle();
        scene.assert_position(FOCUSABLE, case.pick(-50.0, 90.0));
        assert!(!scene.displayed(FOCUSABLE), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, doesNotScrollFocusedFocusableIntoView_whenPartiallyInViewAndViewportGrown
#[test]
fn a_viewport_growing_doesnt_scroll() {
    for case in CASES {
        let size = mutable_state_of(50.0);
        let mut scene = scene(case, Some(size.handle()), 50.0, |_| {
            Spacer!(modifier = Modifier.size(100.dp()));
            TestFocusable(30.0, FOCUSABLE);
            Spacer!(modifier = Modifier.size(100.dp()));
        });
        scene.scroll_to_top();
        scene.request_focus(FOCUSABLE);
        // Brought into view, then half of it out.
        let state = scene.state;
        let _ = run(&mut scene.ui, async move { state.scroll_by(-15.0).await });
        scene.assert_position(FOCUSABLE, case.pick(-15.0, 35.0));
        size.set(100.0);
        scene.ui.wait_for_idle();
        scene.assert_position(FOCUSABLE, 35.0);
        assert!(scene.displayed(FOCUSABLE), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsToNewFocusable_whenFocusedChildChangesDuringAnimation
#[test]
fn the_focus_moving_mid_animation_brings_the_new_one_into_view() {
    for case in CASES {
        let size = mutable_state_of(100.0);
        let mut scene = scene(case, Some(size.handle()), 100.0, move |case| {
            let (first, second) = if case.reverse { ("2", "1") } else { ("1", "2") };
            TestFocusable(10.0, first);
            Spacer!(modifier = Modifier.size(100.dp()));
            TestFocusable(10.0, second);
        });
        scene.request_focus("2");
        assert!(!scene.displayed("1"));
        assert!(scene.displayed("2"));
        scene.ui.set_auto_advance(false);
        size.set(20.0);
        for _ in 0..3 {
            scene.ui.advance_time_by_frame();
        }
        assert!(!scene.displayed("1"), "{case:?}");
        assert!(!scene.displayed("2"), "{case:?}");
        scene.ui.on_node_with_tag("1").request_focus();
        scene.ui.set_auto_advance(true);
        scene.ui.wait_for_idle();
        assert!(scene.displayed("1"), "{case:?}");
        assert!(!scene.displayed("2"), "{case:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, scrollsFocusedFocusableIntoView_whenViewportShrinksMultipleTimes
#[test]
fn shrink_after_shrink_keeps_the_focused_child_in_view() {
    for case in CASES {
        let (mut scene, size) = shrinking(case, 100.0, 90.0);
        scene.assert_position(FOCUSABLE, case.pick(0.0, 90.0));
        for step in 1..=5 {
            size.set(100.0 - 10.0 * step as f32);
            scene.ui.wait_for_idle();
            scene.assert_position(FOCUSABLE, case.pick(0.0, 90.0 - 10.0 * step as f32));
            assert!(scene.displayed(FOCUSABLE), "{case:?}");
        }
    }
}

// compose-rs's own (D1376, on review): two shrinks in one frame, the first leaving the focused
// child in view and the second hiding it, are checked as one, from the size the frame started
// with to the last: the child is kept in view.
#[test]
fn two_shrinks_in_a_frame_are_checked_from_first_to_last() {
    for case in CASES {
        let size = mutable_state_of(100.0);
        let sizes = size.handle();
        // The first shrink, to 95, is seen in a layout whose callback shrinks it to 50: laid
        // out again in the same frame.
        let area = move |modifier: Modifier| {
            modifier.on_size_changed(move |seen: IntSize| {
                if seen.width.min(seen.height) == 95 {
                    sizes.set(50.0);
                }
            })
        };
        // A child of 10 after 85, and 20 more after it: the content outgrows the viewport.
        let mut scene = scene_with(case, Some(size.handle()), 100.0, area, move |case| {
            if case.reverse {
                Spacer!(modifier = Modifier.size(20.dp()));
            }
            WithSpacerBefore(case, 85.0, || TestFocusable(10.0, FOCUSABLE));
            if !case.reverse {
                Spacer!(modifier = Modifier.size(20.dp()));
            }
        });
        scene.request_focus_and_scroll_to_top();
        scene.assert_position(FOCUSABLE, case.pick(5.0, 85.0));
        size.set(95.0);
        scene.ui.wait_for_idle();
        assert_eq!(size.get(), 50.0);
        assert!(scene.displayed(FOCUSABLE), "{case:?}");
        scene.assert_position(FOCUSABLE, case.pick(0.0, 40.0));
    }
}

/// Five focusables of 100 in a viewport of `viewport`, the third tagged.
fn five(case: Case, viewport: f32) -> Scene {
    scene(case, None, viewport, |_| {
        for tag in ["", "", FOCUSABLE, "", ""] {
            TestFocusable(100.0, tag);
        }
    })
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, focusingOnVisibleItemDoesNotScroll_whenMultipleFocusables
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, focusingOutOfBoundsItem_bringsItIntoView_whenMultipleFocusables
#[test]
fn focusing_scrolls_only_what_is_out_of_view() {
    for (viewport, expected) in [(300.0, 0), (200.0, 100)] {
        for case in CASES {
            let mut scene = five(case, viewport);
            scene.request_focus(FOCUSABLE);
            assert_eq!(scene.state.value(), expected, "{case:?}");
        }
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/ScrollFocusableInteractionTest.kt, moveOutFromBoundaryItem_bringsNextItemIntoView
#[test]
fn moving_the_focus_on_brings_the_next_into_view() {
    for case in CASES {
        let mut scene = scene(case, None, 200.0, |_| {
            for tag in ["", FOCUSABLE, ""] {
                TestFocusable(100.0, tag);
            }
        });
        scene.request_focus(FOCUSABLE);
        let manager = scene.manager.borrow().clone().expect("composed");
        manager.move_focus(if case.reverse {
            FocusDirection::Previous
        } else {
            FocusDirection::Next
        });
        scene.ui.wait_for_idle();
        assert_eq!(scene.state.value(), 100, "{case:?}");
    }
}

// compose-rs's own: a focused node outside the scroll, even over it, isn't the scroll's to keep in
// view, as Compose's `getFocusedRect()` of its own target only sees what's inside.
#[test]
fn a_focused_node_outside_isnt_followed() {
    for horizontal in [false, true] {
        let size = mutable_state_of(100.0);
        let viewport = size.handle();
        let scroll = ScrollState::new(0);
        let state = *scroll;
        let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), move || {
            let _owner = &scroll;
            let size = viewport.get();
            Box!({
                let area = Modifier.test_tag(AREA).size(size.dp());
                if horizontal {
                    Row!(modifier = area.horizontal_scroll(state), |_| Box!(
                        modifier = Modifier.size(300.dp())
                    ));
                } else {
                    Column!(modifier = area.vertical_scroll(state), |_| Box!(
                        modifier = Modifier.size(300.dp())
                    ));
                }
                // Over the scroll's end, not inside it.
                let at = if horizontal { (90.dp(), 0.dp()) } else { (0.dp(), 90.dp()) };
                Box!(
                    modifier =
                        Modifier.offset(at.0, at.1).test_tag(FOCUSABLE).size(10.dp()).focusable()
                );
            });
        });
        ui.wait_for_idle();
        ui.on_node_with_tag(FOCUSABLE).request_focus();
        ui.wait_for_idle();
        ui.on_node_with_tag(FOCUSABLE).assert_is_focused();
        size.set(50.0);
        ui.wait_for_idle();
        assert_eq!(state.value(), 0, "horizontal: {horizontal}");
    }
}

/// A scrollable state whose clones are counted through `alive`: what keeps one keeps it.
#[derive(Clone)]
struct Tracked {
    inner: DefaultScrollableState,
    alive: Rc<()>,
}

impl ScrollableState for Tracked {
    fn scroll_boxed<'a>(
        &'a self,
        priority: MutatePriority,
        block: ScrollBlock<'a>,
    ) -> LocalBoxFuture<'a, Result<(), MutationInterrupted>> {
        self.inner.scroll_boxed(priority, block)
    }

    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        self.inner.dispatch_raw_delta(delta)
    }

    fn is_scroll_in_progress(&self) -> bool {
        self.inner.is_scroll_in_progress()
    }
}

impl ComposeArg for Tracked {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.alive, &other.alive)
    }
}

// compose-rs's own (D1376, on review): a container detached in the frame its viewport shrank
// cancels the check waiting for the frame's end, which would otherwise keep it (and its state)
// queued until a frame ends.
#[test]
fn a_container_leaving_in_the_frame_it_shrank_cancels_its_check() {
    let size = mutable_state_of(100.0);
    let shown = mutable_state_of(true);
    let state = Tracked { inner: scrollable_state(|delta| delta), alive: Rc::new(()) };
    let alive = Rc::downgrade(&state.alive);
    let (sizes, shows) = (size.handle(), shown.handle());
    let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), move || {
        if shows.get() {
            Box!(
                modifier = Modifier
                    .size(sizes.get().dp())
                    .scrollable(state.clone(), Orientation::Vertical)
                    // The shrink seen, the container leaves in the same frame.
                    .on_size_changed(move |size: IntSize| {
                        if size.height < 100 {
                            shows.set(false);
                        }
                    })
            );
        }
    });
    ui.wait_for_idle();
    // At the end of the frame, before the check: what keeps the state then.
    let at_frame_end = Rc::new(Cell::new(None));
    let (probe, seen) = (alive.clone(), at_frame_end.clone());
    let _probe = Recomposer::current_thread()
        .schedule_frame_end_callback(move || seen.set(Some(probe.strong_count())));
    size.set(50.0);
    ui.wait_for_idle();
    assert!(!shown.get(), "it left in the frame it shrank");
    assert_eq!(at_frame_end.get(), Some(alive.strong_count()), "nothing queued keeps it");
}
