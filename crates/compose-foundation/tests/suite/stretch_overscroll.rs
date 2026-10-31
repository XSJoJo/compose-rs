//! The events of a platform's edge effects, over `compose-test`'s fake: scenarios of Compose's
//! `StretchOverscrollIntegrationTest` and the rest of `OverscrollTest`, rewritten, then what a lazy
//! layout's estimated end changes (D1473). The fake's curves are arithmetic: what depends on the
//! platform's release curve waits for a device.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::gestures::LocalBoxFuture;
use compose_foundation::layout::*;
use compose_foundation::lazy::{LazyGridScope, LazyListScope, LazyStaggeredGridScope};
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    EdgeEffectSide, IntSize, NestedScrollConnection, NestedScrollSource, Offset, UnitExt, Velocity,
};

/// A way to pull: the pointer's unit along it, and the edge that shows.
#[derive(Clone, Copy, Debug)]
struct Pull {
    pointer: Offset,
    side: EdgeEffectSide,
    orientation: Orientation,
    /// Pulling this way needs the scroll at its end.
    from_the_end: bool,
}

const LEFT: Pull = Pull {
    pointer: Offset::new(-1.0, 0.0),
    side: EdgeEffectSide::Right,
    orientation: Orientation::Horizontal,
    from_the_end: true,
};
const UP: Pull = Pull {
    pointer: Offset::new(0.0, -1.0),
    side: EdgeEffectSide::Bottom,
    orientation: Orientation::Vertical,
    from_the_end: true,
};
const RIGHT: Pull = Pull {
    pointer: Offset::new(1.0, 0.0),
    side: EdgeEffectSide::Left,
    orientation: Orientation::Horizontal,
    from_the_end: false,
};
const DOWN: Pull = Pull {
    pointer: Offset::new(0.0, 1.0),
    side: EdgeEffectSide::Top,
    orientation: Orientation::Vertical,
    from_the_end: false,
};
const WAYS: [Pull; 4] = [LEFT, UP, RIGHT, DOWN];

impl Pull {
    fn by(&self, pixels: f32) -> Offset {
        Offset::new(self.pointer.x * pixels, self.pointer.y * pixels)
    }

    /// `offset` along the pull, positive the way it pulls.
    fn along(&self, offset: Offset) -> f32 {
        offset.x * self.pointer.x + offset.y * self.pointer.y
    }

    fn speed(&self, velocity: Velocity) -> f32 {
        velocity.x * self.pointer.x + velocity.y * self.pointer.y
    }
}

/// What the connection around sees, as Compose's `TestState`.
#[derive(Default)]
struct Seen {
    pre_scroll: Cell<Offset>,
    post_scroll: Cell<Offset>,
    pre_fling: Cell<Velocity>,
    post_fling: Cell<Velocity>,
}

struct Watching(Rc<Seen>);

impl NestedScrollConnection for Watching {
    fn on_pre_scroll(&self, available: Offset, _: NestedScrollSource) -> Offset {
        self.0.pre_scroll.set(self.0.pre_scroll.get() + available);
        Offset::ZERO
    }

    fn on_post_scroll(&self, _: Offset, available: Offset, _: NestedScrollSource) -> Offset {
        self.0.post_scroll.set(self.0.post_scroll.get() + available);
        Offset::ZERO
    }

    fn on_pre_fling(&self, available: Offset) -> LocalBoxFuture<'_, Offset> {
        let seen = self.0.pre_fling.get() + Velocity::new(available.x, available.y);
        self.0.pre_fling.set(seen);
        Box::pin(async { Offset::ZERO })
    }

    fn on_post_fling(&self, _: Offset, available: Offset) -> LocalBoxFuture<'_, Offset> {
        let seen = self.0.post_fling.get() + Velocity::new(available.x, available.y);
        self.0.post_fling.set(seen);
        Box::pin(async { Offset::ZERO })
    }
}

/// Its effects don't ask for frames (the fake doesn't move by itself): a test can wait for idle.
fn factory_of(effects: &Rc<FakeEdgeEffects>) -> Rc<EdgeEffectOverscrollFactory> {
    let factory = Rc::new(EdgeEffectOverscrollFactory::new(effects.clone()));
    factory.set_invalidation_enabled(false);
    factory
}

struct Stretching {
    ui: ComposeTestRule,
    pull: Pull,
    position: Rc<Cell<f32>>,
    max: Rc<Cell<f32>>,
    min: Rc<Cell<f32>>,
    seen: Rc<Seen>,
    effects: Rc<FakeEdgeEffects>,
    /// What the box draws reads it: a frame recording the effect.
    redraw: OwnedState<i32>,
    effect: Rc<dyn OverscrollEffect>,
}

/// A box scrolling a position kept in floats between 0 and 1000, with its effect, its way of
/// scrolling the content (as `ScrollableDefaults.reverseDirection`), at the end the pull needs.
fn stretching(pull: Pull) -> Stretching {
    let effects = FakeEdgeEffects::stretch();
    let position = Rc::new(Cell::new(0.0f32));
    let (min, max) = (Rc::new(Cell::new(0.0f32)), Rc::new(Cell::new(1000.0f32)));
    let seen = Rc::new(Seen::default());
    let scrolled = {
        let (position, min, max) = (position.clone(), min.clone(), max.clone());
        scrollable_state(move |delta| {
            let next = (position.get() + delta).clamp(min.get(), max.get());
            let consumed = next - position.get();
            position.set(next);
            consumed
        })
    };
    let factory = factory_of(&effects);
    let kept = Rc::new(std::cell::RefCell::new(None::<Rc<dyn OverscrollEffect>>));
    let connection: Rc<dyn NestedScrollConnection> = Rc::new(Watching(seen.clone()));
    let redraw = mutable_state_of(0);
    let read = *redraw;
    let mut ui = {
        let kept = kept.clone();
        ComposeTestRule::sized(IntSize::new(600, 600), move || {
            let (kept, connection, state) = (kept.clone(), connection.clone(), scrolled.clone());
            let factory: Rc<dyn OverscrollFactory> = factory.clone();
            CompositionLocalProvider(LOCAL_OVERSCROLL_FACTORY.provides(Some(factory)), move || {
                let effect = remember_overscroll_effect();
                *kept.borrow_mut() = effect.clone();
                Box!(
                    modifier = Modifier
                        .size(250.dp())
                        .nested_scroll(connection.clone())
                        .scrollable_overscroll(
                            state.clone(),
                            pull.orientation,
                            effect.clone(),
                            true,
                            true,
                            None,
                            None,
                        )
                        .overscroll(effect)
                        .draw_behind(move |_| {
                            let _ = read.get();
                        })
                );
            });
        })
    };
    ui.wait_for_idle();
    ui.host().set_view_configuration(Some(compose_ui::ViewConfiguration {
        touch_slop: 0.0,
        ..compose_ui::ViewConfiguration::default()
    }));
    ui.advance_time_by_frame();
    if pull.from_the_end {
        position.set(1000.0);
    }
    let effect = kept.borrow().clone().expect("the effect");
    Stretching { ui, pull, position, max, min, seen, effects, redraw, effect }
}

const CENTER: Offset = Offset::new(125.0, 125.0);

impl Stretching {
    fn start(&mut self, pixels: f32) {
        let by = self.pull.by(pixels);
        self.ui.on_root().perform_touch_input(|touch| {
            touch.down(CENTER);
            touch.move_by(by);
        });
        self.ui.wait_for_idle();
    }

    fn move_by(&mut self, pixels: f32) {
        let by = self.pull.by(pixels);
        self.ui.on_root().perform_touch_input(|touch| touch.move_by(by));
        self.ui.wait_for_idle();
    }

    fn scrolled_from_edge(&self) -> f32 {
        if self.pull.from_the_end { 1000.0 - self.position.get() } else { self.position.get() }
    }

    /// Where the scroll is measured from its edge, and the connection's pre and post scroll seen
    /// along the pull.
    fn assert_scroll(&self, pre: f32, scrolled: f32, post: f32) {
        assert_eq!(self.pull.along(self.seen.pre_scroll.get()), pre, "{:?} pre", self.pull);
        assert_eq!(self.scrolled_from_edge(), scrolled, "{:?} position", self.pull);
        assert_eq!(self.pull.along(self.seen.post_scroll.get()), post, "{:?} post", self.pull);
    }

    fn fake(&self) -> Rc<FakeEdgeEffect> {
        self.effects.at(self.pull.side).expect("the edge's effect")
    }

    /// As `assertOverscrollIsFinishing`: released, it goes as frames are drawn.
    fn assert_finishing(&mut self) {
        for _ in 0..200 {
            if !self.effect.is_in_progress() {
                return;
            }
            self.effects.draw();
            self.ui.advance_time_by_frame();
        }
        panic!("{:?}: the stretch stays", self.pull);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/StretchOverscrollIntegrationTest.kt, stretchOverscroll_consumesDelta_pullLeft, stretchOverscroll_consumesDelta_pullTop, stretchOverscroll_consumesDelta_pullRight, stretchOverscroll_consumesDelta_pullBottom
#[test]
fn a_pull_stretches_after_the_scroll_and_relaxes_before_it() {
    for way in WAYS {
        let mut it = stretching(way);
        it.start(200.0);
        it.assert_scroll(200.0, 0.0, 200.0);
        it.move_by(200.0);
        it.assert_scroll(400.0, 0.0, 400.0);
        // All of it relaxes what's there: nothing goes on.
        it.move_by(-200.0);
        it.assert_scroll(400.0, 0.0, 400.0);
        // Past what's left: the rest scrolls, the connection seeing it before.
        it.move_by(-400.0);
        it.assert_scroll(200.0, 200.0, 400.0);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/StretchOverscrollIntegrationTest.kt, stretchOverscroll_consumesDelta_flingLeft_stretch, stretchOverscroll_consumesDelta_flingTop_stretch, stretchOverscroll_consumesDelta_flingRight_stretch, stretchOverscroll_consumesDelta_flingBottom_stretch
#[test]
fn a_fling_at_the_edge_leaves_its_velocity_to_the_effect_after_the_connections() {
    for way in WAYS {
        let mut it = stretching(way);
        let by = way.by(100.0);
        it.ui.on_root().perform_touch_input(|touch| {
            touch.swipe_with_velocity(CENTER, CENTER + by, 1000.0, None);
        });
        it.ui.wait_for_idle();
        let pre = way.speed(it.seen.pre_fling.get());
        assert!((pre - 1000.0).abs() <= 1.0, "{way:?}: {pre}");
        assert_eq!(it.scrolled_from_edge(), 0.0, "{way:?}");
        // A little goes in the frame of decay before the scroll comes back with nothing used.
        let post = way.speed(it.seen.post_fling.get());
        assert!((post - 1000.0).abs() <= 100.0, "{way:?}: {post}");
        let absorbed = it.fake().absorbed();
        assert_eq!(absorbed.len(), 1, "{way:?}");
        assert!((absorbed[0] as f32 - post).abs() <= 1.0, "{way:?}: {absorbed:?}, {post}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/StretchOverscrollIntegrationTest.kt, stretchOverscroll_consumesDelta_flingLeft_relax_lowVelocity, stretchOverscroll_consumesDelta_flingTop_relax_lowVelocity, stretchOverscroll_consumesDelta_flingRight_relax_lowVelocity, stretchOverscroll_consumesDelta_flingBottom_relax_lowVelocity
#[test]
fn a_slow_fling_back_is_all_taken_by_the_stretch() {
    for way in WAYS {
        let mut it = stretching(way);
        it.start(200.0);
        it.assert_scroll(200.0, 0.0, 200.0);
        assert_eq!(way.speed(it.seen.pre_fling.get()), 0.0);
        let back = way.by(-1.0);
        it.ui.on_root().perform_touch_input(|touch| {
            for _ in 0..100 {
                touch.move_by(back);
            }
            touch.up();
        });
        it.ui.wait_for_idle();
        it.assert_scroll(200.0, 0.0, 200.0);
        // The stretch absorbed it: nothing is told.
        assert_eq!(way.speed(it.seen.pre_fling.get()), 0.0, "{way:?}");
        assert_eq!(way.speed(it.seen.post_fling.get()), 0.0, "{way:?}");
        assert_eq!(it.fake().absorbed().len(), 1, "{way:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/StretchOverscrollIntegrationTest.kt, stretchOverscroll_consumesDelta_flingLeft_relax_highVelocity, stretchOverscroll_consumesDelta_flingTop_relax_highVelocity, stretchOverscroll_consumesDelta_flingRight_relax_highVelocity, stretchOverscroll_doesNotConsumeDelta_flingBottom_relax_highVelocity
#[test]
fn a_fast_fling_back_relaxes_in_its_scroll_and_goes_on() {
    for way in WAYS {
        let mut it = stretching(way);
        it.start(200.0);
        let back = way.by(-50.0);
        it.ui.on_root().perform_touch_input(|touch| {
            touch.move_by(back);
            touch.move_by(back);
            touch.up();
        });
        it.ui.wait_for_idle();
        // Against the pull: it's told before the fling, and what the relaxing leaves scrolls.
        assert!(way.speed(it.seen.pre_fling.get()) < 0.0, "{way:?}");
        assert!(way.along(it.seen.pre_scroll.get()) < 200.0, "{way:?}");
        assert!(it.scrolled_from_edge() > 0.0, "{way:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/StretchOverscrollIntegrationTest.kt, stretchOverscroll_whenPulledWithSmallDelta_doesNotConsumesOppositePreScroll_pullLeft, stretchOverscroll_whenPulledWithSmallDelta_doesNotConsumesOppositePreScroll_pullTop, stretchOverscroll_whenPulledWithSmallDelta_doesNotConsumesOppositePreScroll_pullRight, stretchOverscroll_whenPulledWithSmallDelta_doesNotConsumesOppositePreScroll_pullBottom
#[test]
fn less_than_half_a_pixel_stretches_nothing() {
    for way in WAYS {
        let mut it = stretching(way);
        it.start(0.4);
        it.move_by(-200.0);
        let pre = way.along(it.seen.pre_scroll.get());
        assert!((pre - -199.6).abs() < 0.001, "{way:?}: {pre}");
        assert_eq!(it.scrolled_from_edge(), 200.0, "{way:?}");
        let post = way.along(it.seen.post_scroll.get());
        assert!((post - 0.4).abs() < 0.001, "{way:?}: {post}");
        assert_eq!(it.effects.made(), 0, "{way:?}: no effect was asked for");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/StretchOverscrollIntegrationTest.kt, stretchOverscroll_scrollStartsToConsumeAfterBeingStretched_releaseOverscroll_pullLeft, stretchOverscroll_scrollStartsToConsumeAfterBeingStretched_releaseOverscroll_pullTop, stretchOverscroll_scrollStartsToConsumeAfterBeingStretched_releaseOverscroll_pullRight, stretchOverscroll_scrollStartsToConsumeAfterBeingStretched_releaseOverscroll_pullBottom
#[test]
fn a_stretch_goes_back_when_the_scroll_consumes_again() {
    for way in WAYS {
        let mut it = stretching(way);
        it.start(200.0);
        it.assert_scroll(200.0, 0.0, 200.0);
        // More to scroll: as an item coming at the end.
        it.max.set(2000.0);
        it.min.set(-1000.0);
        it.move_by(200.0);
        assert_eq!(way.along(it.seen.pre_scroll.get()), 400.0, "{way:?}");
        assert_eq!(way.along(it.seen.post_scroll.get()), 200.0, "{way:?}");
        assert!(it.effect.is_in_progress(), "{way:?}: still showing, going back");
        assert!(it.fake().is_receding(), "{way:?}");
        it.assert_finishing();
    }
}

/// Its effect from `effects`, under a factory, with what `Layout` of `content` needs.
fn with_effect(
    effects: &Rc<FakeEdgeEffects>,
    content: impl Fn(Option<Rc<dyn OverscrollEffect>>) + 'static,
) -> (ComposeTestRule, Rc<dyn OverscrollEffect>) {
    let factory = factory_of(effects);
    let kept = Rc::new(std::cell::RefCell::new(None::<Rc<dyn OverscrollEffect>>));
    let into = kept.clone();
    let content = Rc::new(content);
    let mut ui = ComposeTestRule::sized(IntSize::new(600, 600), move || {
        let (into, content) = (into.clone(), content.clone());
        let factory: Rc<dyn OverscrollFactory> = factory.clone();
        CompositionLocalProvider(LOCAL_OVERSCROLL_FACTORY.provides(Some(factory)), move || {
            let effect = remember_overscroll_effect();
            *into.borrow_mut() = effect.clone();
            content(effect);
        });
    });
    ui.wait_for_idle();
    let effect = kept.borrow().clone().expect("the effect");
    (ui, effect)
}

/// What an effect passes on of a scroll and a fling it's given, at once.
fn assert_passes_on_everything(
    effect: &Rc<dyn OverscrollEffect>,
    delta: Offset,
    velocity: Velocity,
) {
    for _ in 0..2 {
        let mut given = None;
        effect.apply_to_scroll(delta, NestedScrollSource::UserInput, &mut |offset| {
            given = Some(offset);
            Offset::ZERO
        });
        assert_eq!(given, Some(delta));
    }
    let given = Rc::new(Cell::new(None));
    let into = given.clone();
    let perform: PerformFling = Rc::new(move |velocity| {
        into.set(Some(velocity));
        Box::pin(async { Velocity::ZERO })
    });
    let mut fling = effect.apply_to_fling(velocity, perform);
    let waker = std::task::Waker::noop();
    let polled = fling.as_mut().poll(&mut std::task::Context::from_waker(waker));
    assert!(polled.is_ready());
    assert_eq!(given.get(), Some(velocity));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/OverscrollTest.kt, rememberOverscrollEffect_defaultValue
#[test]
fn the_default_effect_is_over_the_host_s_edge_effects() {
    let effects = FakeEdgeEffects::stretch();
    let host = compose_test::default_host();
    let factory: Rc<dyn compose_ui::EdgeEffectFactory> = effects.clone();
    host.with_tree(|tree| tree.set_edge_effect_factory(factory));
    let tick = mutable_state_of(0);
    let read = *tick;
    let kept = Rc::new(std::cell::RefCell::new(Vec::new()));
    let into = kept.clone();
    let mut ui = ComposeTestRule::with_host(host, move || {
        let _ = read.get();
        into.borrow_mut().push(remember_overscroll_effect().expect("the host's"));
    });
    tick.set(1);
    ui.wait_for_idle();
    let effects_made = kept.borrow();
    assert_eq!(effects_made.len(), 2);
    // The same effect after a recomposition: one factory for the tree.
    assert!(Rc::ptr_eq(&effects_made[0], &effects_made[1]));
    assert!(effects_made[0].node().is_some());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/OverscrollTest.kt, zeroSizedEffectIsNotConsumingOffsetsAndVelocity
#[test]
fn an_effect_of_no_size_passes_everything_on() {
    let effects = FakeEdgeEffects::stretch();
    let (_ui, effect) = with_effect(&effects, |effect| {
        Box!({
            Box!(modifier = Modifier.overscroll(effect.clone()).size(0.dp()));
        });
    });
    assert_passes_on_everything(&effect, Offset::new(-10.0, -10.0), Velocity::new(-5.0, -5.0));
    assert_eq!(effects.made(), 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/OverscrollTest.kt, notAttachedEffectIsNotConsumingOffsetsAndVelocity
#[test]
fn an_effect_not_attached_passes_everything_on() {
    let effects = FakeEdgeEffects::stretch();
    let (_ui, effect) = with_effect(&effects, |_| {});
    assert_passes_on_everything(&effect, Offset::new(0.0, 10.0), Velocity::new(0.0, 5.0));
    assert_eq!(effects.made(), 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/OverscrollTest.kt, testOverscrollModifierDrawsOnce
#[test]
fn the_effect_s_modifier_draws_what_s_under_it_once() {
    let effects = FakeEdgeEffects::stretch();
    let drawn = Rc::new(Cell::new(0));
    let counted = drawn.clone();
    let (mut ui, _effect) = with_effect(&effects, move |effect| {
        let counted = counted.clone();
        Box!(
            modifier = Modifier.size(100.dp()).overscroll(effect).draw_behind(move |_| {
                counted.set(counted.get() + 1);
            })
        );
    });
    ui.wait_for_idle();
    assert_eq!(drawn.get(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/OverscrollTest.kt, modifierDoesNotAddAlreadyAttachedNode
#[test]
fn an_effect_applied_at_two_places_is_drawn_by_the_one_attached_first() {
    let effects = FakeEdgeEffects::stretch();
    let (ui, effect) = with_effect(&effects, move |effect| {
        Box!(modifier = Modifier.size(100.dp()).overscroll(effect.clone()), {
            Box!(modifier = Modifier.size(50.dp()).overscroll(effect.clone()));
        });
    });
    drop(ui);
    // Children are attached first: the inner one's size reached it, the outer's never. It takes a
    // scroll at 50 px.
    effect.apply_to_scroll(Offset::new(0.0, 25.0), NestedScrollSource::UserInput, &mut |_| {
        Offset::ZERO
    });
    let top = effects.at(EdgeEffectSide::Top).expect("pulled");
    assert_eq!(top.size(), (50, 50));
    assert_eq!(top.distance_now(), 0.5);
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, displacement
#[test]
fn a_pull_is_where_the_pointer_is_or_in_the_middle() {
    let mut it = stretching(DOWN);
    it.start(100.0);
    // Across its edge the pointer stayed at the center.
    assert_eq!(it.fake().displacement(), Some(0.5));
    let mut it = stretching(DOWN);
    it.ui.on_root().perform_touch_input(|touch| {
        touch.down(Offset::new(50.0, 125.0));
        touch.move_by(Offset::new(0.0, 100.0));
    });
    it.ui.wait_for_idle();
    assert_eq!(it.fake().displacement(), Some(0.2));
    // No pointer: the middle, as an effect driven by code.
    let effects = FakeEdgeEffects::stretch();
    let (_ui, effect) = with_effect(&effects, |effect| {
        Box!(modifier = Modifier.size(100.dp()).overscroll(effect));
    });
    effect.apply_to_scroll(Offset::new(0.0, -25.0), NestedScrollSource::UserInput, &mut |_| {
        Offset::ZERO
    });
    assert_eq!(effects.at(EdgeEffectSide::Bottom).expect("pulled").displacement(), Some(0.5));
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, EdgeEffectWrapper's lazy creation
#[test]
fn a_scroller_that_never_hits_an_edge_never_calls_the_platform() {
    let effects = FakeEdgeEffects::stretch();
    let kept = Rc::new(Cell::new(None));
    let into = kept.clone();
    let (mut ui, _effect) = with_effect(&effects, move |effect| {
        let list = remember_lazy_list_state(0, 0);
        into.set(Some(list));
        LazyColumn!(
            modifier = Modifier.size(300.dp()),
            state = list,
            overscroll_effect = effect,
            content = |scope: LazyListScope| {
                scope.items(100, |_| Box!(modifier = Modifier.size(100.dp())));
            }
        );
    });
    no_slop(&mut ui);
    ui.on_root().perform_touch_input(|touch| {
        touch.down(Offset::new(100.0, 250.0));
        for _ in 0..30 {
            touch.move_by(Offset::new(0.0, -7.0));
        }
    });
    ui.wait_for_idle();
    let state: compose_foundation::lazy::LazyListState = kept.get().expect("composed");
    assert!(state.first_visible_item_index() > 0, "it scrolled");
    assert_eq!(effects.made(), 0);
    assert_eq!(effects.calls(), 0);
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, EdgeEffectWrapper's lazy creation
#[test]
fn once_it_s_over_a_normal_scroll_calls_the_platform_no_more() {
    let mut it = stretching(DOWN);
    let by = DOWN.by(50.0);
    it.ui.on_root().perform_touch_input(|touch| {
        touch.down(CENTER);
        touch.move_by(by);
        touch.up();
    });
    it.ui.wait_for_idle();
    it.assert_finishing();
    assert!(it.fake().is_finished_now());
    // A frame sees it's over.
    it.redraw.set(1);
    it.ui.wait_for_idle();
    it.position.set(500.0);
    let by = DOWN.by(1.0);
    it.ui.on_root().perform_touch_input(|touch| {
        touch.down(CENTER);
        touch.move_by(by);
    });
    it.ui.wait_for_idle();
    let calls = it.effects.calls();
    it.ui.on_root().perform_touch_input(|touch| {
        for _ in 0..10 {
            touch.move_by(by);
        }
    });
    it.ui.wait_for_idle();
    assert_eq!(it.position.get(), 489.0);
    assert_eq!(it.effects.calls(), calls);
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, applyToScroll
#[test]
fn a_glow_takes_a_pull_and_releases_on_the_way_back() {
    let effects = FakeEdgeEffects::glow();
    let position = Rc::new(Cell::new(0.0f32));
    let scrolled = {
        let position = position.clone();
        scrollable_state(move |delta| {
            let next = (position.get() + delta).clamp(0.0, 1000.0);
            let consumed = next - position.get();
            position.set(next);
            consumed
        })
    };
    let (mut ui, _effect) = with_effect(&effects, move |effect| {
        Box!(
            modifier = Modifier
                .size(250.dp())
                .scrollable_overscroll(
                    scrolled.clone(),
                    Orientation::Vertical,
                    effect.clone(),
                    true,
                    true,
                    None,
                    None,
                )
                .overscroll(effect)
        );
    });
    no_slop(&mut ui);
    ui.on_root().perform_touch_input(|touch| {
        touch.down(CENTER);
        touch.move_by(Offset::new(0.0, 100.0));
    });
    ui.wait_for_idle();
    let top = effects.at(EdgeEffectSide::Top).expect("the top's glow");
    assert!(!top.is_finished_now());
    assert_eq!(top.releases(), 0);
    // Against it: the delta scrolls, and the glow is told to go. It takes nothing of it.
    ui.on_root().perform_touch_input(|touch| touch.move_by(Offset::new(0.0, -40.0)));
    ui.wait_for_idle();
    assert_eq!(position.get(), 40.0);
    assert_eq!(top.releases(), 1);
    assert!(top.is_receding());
}

// Compose: compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/Scroll.kt, verticalScroll without an effect: useLocalOverscrollFactory
#[test]
fn vertical_scroll_with_the_local_factory_pulls_at_a_press() {
    let effects = FakeEdgeEffects::stretch();
    let (mut ui, _effect) = with_effect(&effects, |_| {
        let state = remember_scroll_state(0);
        Box!(modifier = Modifier.size(200.dp()).vertical_scroll(state), {
            Box!(modifier = Modifier.size(300.dp()));
        });
    });
    no_slop(&mut ui);
    assert_eq!(effects.made(), 0);
    ui.on_root().perform_touch_input(|touch| {
        touch.down(Offset::new(50.0, 50.0));
        touch.move_by(Offset::new(0.0, 100.0));
    });
    ui.wait_for_idle();
    let top = effects.at(EdgeEffectSide::Top).expect("pulled");
    assert_eq!(top.distance_now(), 0.5);
    assert_eq!(top.size(), (200, 200));
    // Its displacement: it heard the pointer.
    assert_eq!(top.displacement(), Some(0.25));
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, defaultOverscrollFactory
#[test]
fn vertical_scroll_with_the_default_factory_reads_the_host_s_at_a_press() {
    let effects = FakeEdgeEffects::stretch();
    let host = compose_test::default_host();
    let factory: Rc<dyn compose_ui::EdgeEffectFactory> = effects.clone();
    host.with_tree(|tree| tree.set_edge_effect_factory(factory));
    let mut ui = ComposeTestRule::with_host(host, || {
        let state = remember_scroll_state(0);
        Box!(modifier = Modifier.size(200.dp()).vertical_scroll(state), {
            Box!(modifier = Modifier.size(300.dp()));
        });
    });
    no_slop(&mut ui);
    press_and_move(&mut ui, Offset::new(50.0, 50.0), Offset::new(0.0, 100.0));
    assert_eq!(effects.at(EdgeEffectSide::Top).expect("pulled").distance_now(), 0.5);
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, defaultOverscrollFactory
#[test]
fn a_list_inside_an_item_has_the_host_s_effect_too() {
    let effects = FakeEdgeEffects::stretch();
    let host = compose_test::default_host();
    let factory: Rc<dyn compose_ui::EdgeEffectFactory> = effects.clone();
    host.with_tree(|tree| tree.set_edge_effect_factory(factory));
    let mut ui = ComposeTestRule::with_host(host, || {
        LazyColumn!(
            modifier = Modifier.size(300.dp()),
            content = |scope: LazyListScope| {
                scope.items(1, |_| {
                    LazyRow!(
                        modifier = Modifier.size(300.dp()),
                        content = |scope: LazyListScope| {
                            scope.items(10, |_| Box!(modifier = Modifier.size(100.dp())));
                        }
                    );
                });
            }
        );
    });
    no_slop(&mut ui);
    press_and_move(&mut ui, Offset::new(50.0, 50.0), Offset::new(30.0, 0.0));
    let left = effects.at(EdgeEffectSide::Left).expect("pulled");
    assert!((left.distance_now() - 0.1).abs() < 1e-5);
}

/// A touch's press and move, sent as a host sends them: with the host's factory its effect asks for
/// frames while it's pulled, so the rule's waiting for idle wouldn't end.
fn press_and_move(ui: &mut ComposeTestRule, down: Offset, by: Offset) {
    let touch = |event: compose_ui::PointerEvent| compose_ui::PointerEvent {
        pointer_type: compose_ui::PointerType::Touch,
        ..event
    };
    ui.host().send_pointer(touch(compose_ui::PointerEvent::press(down)));
    ui.host().frame();
    ui.host().send_pointer(touch(compose_ui::PointerEvent::moved(down + by, true)));
}

fn no_slop(ui: &mut ComposeTestRule) {
    ui.host().set_view_configuration(Some(compose_ui::ViewConfiguration {
        touch_slop: 0.0,
        ..compose_ui::ViewConfiguration::default()
    }));
    ui.advance_time_by_frame();
}

// --- What a lazy layout's estimated end changes (D1473). ---

const ITEM: f32 = 100.0;
const COUNT: usize = 20;
/// Its scrolling room: 20 items of 100 in a viewport of 300.
const ROOM: f32 = COUNT as f32 * ITEM - 300.0;

/// A list under an effect, with a connection seeing what's around, in a way of the three lazy
/// layouts, from `first` item.
fn lazy_layout(
    kind: usize,
    effects: &Rc<FakeEdgeEffects>,
    seen: Option<Rc<dyn NestedScrollConnection>>,
    first: usize,
) -> ComposeTestRule {
    lazy_layout_of(kind, effects, seen, first, COUNT, ITEM)
}

/// [`lazy_layout`] of `count` items of `item` pixels.
fn lazy_layout_of(
    kind: usize,
    effects: &Rc<FakeEdgeEffects>,
    seen: Option<Rc<dyn NestedScrollConnection>>,
    first: usize,
    count: usize,
    item: f32,
) -> ComposeTestRule {
    let (mut ui, _effect) = with_effect(effects, move |effect| {
        let mut modifier = Modifier;
        if let Some(seen) = &seen {
            modifier = modifier.nested_scroll(seen.clone());
        }
        Box!(modifier = modifier, {
            let modifier = Modifier.size(300.dp());
            match kind {
                0 => {
                    let state = remember_lazy_list_state(first, 0);
                    LazyColumn!(
                        modifier = modifier,
                        state = state,
                        overscroll_effect = effect.clone(),
                        content = |scope: LazyListScope| {
                            scope.items(count, move |_| {
                                Box!(modifier = Modifier.size(item.dp()));
                            });
                        }
                    );
                }
                1 => {
                    let state = remember_lazy_grid_state(first, 0);
                    LazyVerticalGrid!(
                        GridCells::Fixed(1),
                        modifier = modifier,
                        state = state,
                        overscroll_effect = effect.clone(),
                        content = |scope: LazyGridScope| {
                            scope.items(count, move |_| {
                                Box!(modifier = Modifier.size(item.dp()));
                            });
                        }
                    );
                }
                _ => {
                    let state = remember_lazy_staggered_grid_state(first, 0);
                    LazyVerticalStaggeredGrid!(
                        StaggeredGridCells::Fixed(1),
                        modifier = modifier,
                        state = state,
                        overscroll_effect = effect.clone(),
                        content = |scope: LazyStaggeredGridScope| {
                            scope.items(count, move |_| {
                                Box!(modifier = Modifier.size(item.dp()));
                            });
                        }
                    );
                }
            }
        });
    });
    no_slop(&mut ui);
    ui
}

/// A column scrolling the same content, whose end is known: a scroll consuming at once.
fn column_of_items(
    effects: &Rc<FakeEdgeEffects>,
    first: usize,
    count: usize,
    item: f32,
) -> ComposeTestRule {
    let (mut ui, _effect) = with_effect(effects, move |effect| {
        let state = remember_scroll_state((first as f32 * item) as i32);
        Column!(
            modifier = Modifier
                .size(300.dp())
                .vertical_scroll_overscroll(state, effect, true, None, false),
            {
                for _ in 0..count {
                    Box!(modifier = Modifier.size(item.dp()));
                }
            }
        );
    });
    no_slop(&mut ui);
    ui
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, applyToFling
#[test]
fn a_fling_into_a_list_s_end_gives_the_effect_its_velocity() {
    // Items smaller than a frame's delta: the frame reaching the end doesn't know it's there.
    let fling = |ui: &mut ComposeTestRule| {
        ui.on_root().perform_touch_input(|touch| {
            touch.swipe_with_velocity(
                Offset::new(100.0, 280.0),
                Offset::new(100.0, 20.0),
                12_000.0,
                Some(40),
            );
        });
        ui.wait_for_idle();
    };
    let measured = FakeEdgeEffects::stretch();
    let mut column = column_of_items(&measured, 40, 90, 30.0);
    fling(&mut column);
    let known = measured.at(EdgeEffectSide::Bottom).expect("the column's end").absorbed();
    assert_eq!(known.len(), 1);
    for kind in 0..3 {
        let effects = FakeEdgeEffects::stretch();
        let mut ui = lazy_layout_of(kind, &effects, None, 40, 90, 30.0);
        fling(&mut ui);
        let absorbed = effects.at(EdgeEffectSide::Bottom).expect("the end").absorbed();
        assert_eq!(absorbed.len(), 1, "{kind}");
        // A frame later at most: the list hears it's over when the next delta finds no room.
        let (reached, expected) = (absorbed[0] as f32, known[0] as f32);
        assert!(reached <= expected && reached >= expected * 0.9, "{kind}: {reached}, {expected}");
    }
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, applyToScroll
#[test]
fn a_drag_past_a_list_s_estimated_end_pulls_with_what_s_left() {
    for kind in 0..3 {
        let effects = FakeEdgeEffects::stretch();
        let mut ui = lazy_layout(kind, &effects, None, 0);
        // One delta past all of it: the end isn't known, and the list takes it all.
        ui.on_root().perform_touch_input(|touch| {
            touch.down(Offset::new(100.0, 250.0));
            touch.move_by(Offset::new(0.0, -(ROOM + 150.0)));
        });
        ui.wait_for_idle();
        let bottom = effects.at(EdgeEffectSide::Bottom).unwrap_or_else(|| panic!("{kind}"));
        assert!((bottom.distance_now() - 0.5).abs() < 1e-3, "{kind}: {}", bottom.distance_now());
        // The next delta finds the end known: it goes on to the stretch at once.
        ui.on_root().perform_touch_input(|touch| touch.move_by(Offset::new(0.0, -30.0)));
        ui.wait_for_idle();
        assert!((bottom.distance_now() - 0.6).abs() < 1e-3, "{kind}: {}", bottom.distance_now());
        // Back: the stretch relaxes before the list moves.
        ui.on_root().perform_touch_input(|touch| touch.move_by(Offset::new(0.0, 180.0)));
        ui.wait_for_idle();
        assert_eq!(bottom.distance_now(), 0.0, "{kind}");
    }
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, applyToFling
#[test]
fn a_drag_ending_as_it_passes_a_list_s_estimated_end_goes_back() {
    for kind in 0..3 {
        let effects = FakeEdgeEffects::stretch();
        let mut ui = lazy_layout(kind, &effects, None, 0);
        // The last move and the lift of one batch: its fling, at no velocity, is over before the
        // layout finds what's left.
        let touch = |event: compose_ui::PointerEvent, millis: u64| compose_ui::PointerEvent {
            pointer_type: compose_ui::PointerType::Touch,
            ..event.at_time(compose_runtime::platform::now_nanos() + millis * 1_000_000)
        };
        let down = Offset::new(100.0, 250.0);
        let end = down - Offset::new(0.0, ROOM + 150.0);
        ui.host().send_pointer(touch(compose_ui::PointerEvent::press(down), 0));
        ui.advance_time_by_frame();
        ui.host().send_pointer(touch(compose_ui::PointerEvent::moved(end, true), 0));
        ui.host().send_pointer(touch(compose_ui::PointerEvent::release(end), 3000));
        ui.wait_for_idle();
        let bottom = effects.at(EdgeEffectSide::Bottom).unwrap_or_else(|| panic!("{kind}"));
        assert!(bottom.distance_now() > 0.0, "{kind}");
        assert!(bottom.is_receding(), "{kind}: it would stay stretched");
    }
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, StretchOverscrollNode.draw
#[test]
fn drawing_wraps_the_content_only_while_an_effect_is_active_a_new_stamp_each_frame() {
    use compose_ui::EdgeEffect;
    use compose_ui::platform::DrawOp;
    let effects = FakeEdgeEffects::stretch();
    let factory: Rc<dyn OverscrollFactory> =
        Rc::new(EdgeEffectOverscrollFactory::new(effects.clone()));
    let kept = Rc::new(std::cell::RefCell::new(None));
    let into = kept.clone();
    let mut host = compose_test::default_host();
    host.set_content(move || {
        let into = into.clone();
        CompositionLocalProvider(
            LOCAL_OVERSCROLL_FACTORY.provides(Some(factory.clone())),
            move || {
                let effect = remember_overscroll_effect();
                *into.borrow_mut() = effect.clone();
                Box!(
                    modifier = Modifier.offset(10.dp(), 20.dp()).size(100.dp()).overscroll(effect)
                );
            },
        );
    });
    host.frame();
    let stamps = |host: &compose_ui::platform::UiHost| {
        let list = host.display_list();
        let begun: Vec<(u32, compose_ui::Rect)> = list
            .ops
            .iter()
            .filter_map(|op| match op {
                DrawOp::BeginEdgeEffects { stamp, bounds, .. } => Some((*stamp, *bounds)),
                _ => None,
            })
            .collect();
        let ended = list.ops.iter().filter(|op| matches!(op, DrawOp::EndEdgeEffects)).count();
        assert_eq!(begun.len(), ended, "balanced");
        begun
    };
    assert!(stamps(&host).is_empty(), "at rest: no op");
    assert!(!host.needs_frame());
    let effect: Rc<dyn OverscrollEffect> = kept.borrow().clone().expect("the effect");
    effect.apply_to_scroll(Offset::new(0.0, 25.0), NestedScrollSource::UserInput, &mut |_| {
        Offset::ZERO
    });
    assert!(host.needs_frame(), "a pull asks for a frame");
    host.frame();
    let first = stamps(&host);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].1, compose_ui::Rect::new(10.0, 20.0, 110.0, 120.0), "the node's bounds");
    // Pulled and the same bytes: each frame says it's another, and asks for the next.
    assert!(host.needs_frame());
    host.frame();
    let second = stamps(&host);
    assert_ne!(first[0].0, second[0].0);
    // Over, as the platform says at a frame's replay: the next frame draws plainly, and stops.
    let top = effects.at(EdgeEffectSide::Top).expect("pulled");
    top.finish();
    assert!(host.needs_frame());
    host.frame();
    assert!(stamps(&host).is_empty());
    assert!(!host.needs_frame());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/OverscrollTest.kt, modifierDoesNotAddAlreadyAttachedNode
#[test]
fn an_effect_at_two_places_is_written_once() {
    use compose_ui::platform::DrawOp;
    let effects = FakeEdgeEffects::stretch();
    let factory: Rc<dyn OverscrollFactory> =
        Rc::new(EdgeEffectOverscrollFactory::new(effects.clone()));
    let kept = Rc::new(std::cell::RefCell::new(None));
    let into = kept.clone();
    let mut host = compose_test::default_host();
    host.set_content(move || {
        let into = into.clone();
        CompositionLocalProvider(
            LOCAL_OVERSCROLL_FACTORY.provides(Some(factory.clone())),
            move || {
                let effect = remember_overscroll_effect();
                *into.borrow_mut() = effect.clone();
                Box!(modifier = Modifier.size(100.dp()).overscroll(effect.clone()), {
                    Box!(modifier = Modifier.size(50.dp()).overscroll(effect.clone()));
                });
            },
        );
    });
    host.frame();
    let effect: Rc<dyn OverscrollEffect> = kept.borrow().clone().expect("the effect");
    effect.apply_to_scroll(Offset::new(0.0, 25.0), NestedScrollSource::UserInput, &mut |_| {
        Offset::ZERO
    });
    host.frame();
    let list = host.display_list();
    let begun = list.ops.iter().filter(|op| matches!(op, DrawOp::BeginEdgeEffects { .. })).count();
    assert_eq!(begun, 1);
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, FlingDestretchFactor
#[test]
fn a_fling_relaxes_a_stretch_four_times_as_fast_as_a_drag() {
    for (source, left) in
        [(NestedScrollSource::UserInput, 0.36), (NestedScrollSource::SideEffect, 0.24)]
    {
        let effects = FakeEdgeEffects::stretch();
        let (_ui, effect) = with_effect(&effects, |effect| {
            Box!(modifier = Modifier.size(250.dp()).overscroll(effect));
        });
        let mut nothing = |_| Offset::ZERO;
        effect.apply_to_scroll(
            Offset::new(0.0, 100.0),
            NestedScrollSource::UserInput,
            &mut nothing,
        );
        let consumed = effect.apply_to_scroll(Offset::new(0.0, -10.0), source, &mut nothing);
        assert_eq!(consumed, Offset::new(0.0, -10.0));
        let top = effects.at(EdgeEffectSide::Top).expect("pulled");
        assert!((top.distance_now() - left).abs() < 1e-5, "{}", top.distance_now());
    }
}

/// Takes half of what a post-scroll offers.
struct TakesHalf(Rc<Cell<f32>>);

impl NestedScrollConnection for TakesHalf {
    fn on_post_scroll(&self, _: Offset, available: Offset, _: NestedScrollSource) -> Offset {
        self.0.set(self.0.get() + available.y);
        Offset::new(0.0, available.y / 2.0)
    }
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, applyToScroll
#[test]
fn past_a_list_s_estimated_end_the_parents_are_offered_first() {
    let effects = FakeEdgeEffects::stretch();
    let offered = Rc::new(Cell::new(0.0));
    let parent: Rc<dyn NestedScrollConnection> = Rc::new(TakesHalf(offered.clone()));
    let mut ui = lazy_layout(0, &effects, Some(parent), 0);
    ui.on_root().perform_touch_input(|touch| {
        touch.down(Offset::new(100.0, 250.0));
        touch.move_by(Offset::new(0.0, -(ROOM + 300.0)));
    });
    ui.wait_for_idle();
    assert_eq!(offered.get(), -300.0);
    let bottom = effects.at(EdgeEffectSide::Bottom).expect("the end");
    assert!((bottom.distance_now() - 0.5).abs() < 1e-3, "{}", bottom.distance_now());
}

// Compose: compose/foundation/foundation/src/androidMain/kotlin/androidx/compose/foundation/AndroidOverscroll.android.kt, applyToScroll
#[test]
fn a_wheel_past_a_list_s_estimated_end_doesn_t_pull() {
    let effects = FakeEdgeEffects::stretch();
    let mut ui = lazy_layout(0, &effects, None, 0);
    ui.on_root().perform_mouse_input(|mouse| {
        mouse.move_to(Offset::new(100.0, 150.0));
        mouse.scroll(5000.0, compose_test::ScrollWheel::Vertical);
    });
    ui.wait_for_idle();
    assert_eq!(effects.made(), 0);
}
