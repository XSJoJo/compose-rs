//! A fake of a platform's edge effects, for overscroll's events on the host: a stretch's
//! `onPullDistance` and its states, with curves of plain arithmetic in place of the platform's
//! physics. As the platform's, its distance goes only as it's drawn: a test calls
//! [`FakeEdgeEffects::draw`] where a host would replay a frame.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_ui::{Color, EdgeEffect, EdgeEffectFactory, EdgeEffectSide};

/// How much of a fraction of its length a released effect loses per frame drawn.
const RECEDE_PER_DRAW: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Idle,
    Pulled,
    Receding,
}

/// One edge's fake effect. It says what it was told, for tests to look at.
pub struct FakeEdgeEffect {
    glow: bool,
    phase: Cell<Phase>,
    distance: Cell<f32>,
    size: Cell<(i32, i32)>,
    displacement: Cell<Option<f32>>,
    absorbed: RefCell<Vec<i32>>,
    releases: Cell<u32>,
    glow_color: Color,
    calls: Rc<Cell<u32>>,
}

impl FakeEdgeEffect {
    /// The velocities a fling reaching the edge gave it, in order.
    pub fn absorbed(&self) -> Vec<i32> {
        self.absorbed.borrow().clone()
    }

    /// Its distance, not counted as a call.
    pub fn distance_now(&self) -> f32 {
        if self.glow { 0.0 } else { self.distance.get() }
    }

    /// Whether it's finished, not counted as a call.
    pub fn is_finished_now(&self) -> bool {
        self.phase.get() == Phase::Idle
    }

    /// The last pull's displacement.
    pub fn displacement(&self) -> Option<f32> {
        self.displacement.get()
    }

    /// The length along its edge and across it, as last set.
    pub fn size(&self) -> (i32, i32) {
        self.size.get()
    }

    /// Whether a release (not a finish) is going on.
    pub fn is_receding(&self) -> bool {
        self.phase.get() == Phase::Receding
    }

    /// The releases it was told of.
    pub fn releases(&self) -> u32 {
        self.releases.get()
    }

    pub fn glow_color(&self) -> Color {
        self.glow_color
    }

    /// A frame's `EdgeEffect.draw`: what's receding goes on.
    fn draw(&self) {
        if self.phase.get() != Phase::Receding {
            return;
        }
        let left = (self.distance.get() - RECEDE_PER_DRAW).max(0.0);
        self.distance.set(left);
        if left == 0.0 {
            self.phase.set(Phase::Idle);
        }
    }

    fn called(&self) {
        self.calls.set(self.calls.get() + 1);
    }
}

impl EdgeEffect for FakeEdgeEffect {
    fn on_pull_distance(&self, delta: f32, displacement: f32) -> f32 {
        self.called();
        if self.glow {
            self.phase.set(Phase::Pulled);
            self.displacement.set(Some(displacement));
            return delta;
        }
        let distance = self.distance.get();
        let reached = (distance + delta).max(0.0);
        let taken = reached - distance;
        if taken == 0.0 && distance == 0.0 {
            return 0.0;
        }
        self.phase.set(Phase::Pulled);
        self.distance.set(reached);
        self.displacement.set(Some(displacement));
        taken
    }

    fn on_release(&self) {
        self.called();
        self.releases.set(self.releases.get() + 1);
        if self.phase.get() == Phase::Pulled {
            self.phase.set(Phase::Receding);
        }
    }

    fn on_absorb(&self, velocity: i32) {
        self.called();
        self.absorbed.borrow_mut().push(velocity);
        // A little past where it is, as a stretch's spring takes the velocity.
        self.distance.set(self.distance.get() + velocity.unsigned_abs() as f32 / 10_000.0);
        self.phase.set(Phase::Receding);
    }

    fn distance(&self) -> f32 {
        self.called();
        if self.glow { 0.0 } else { self.distance.get() }
    }

    fn is_finished(&self) -> bool {
        self.called();
        self.phase.get() == Phase::Idle
    }

    fn finish(&self) {
        self.called();
        self.phase.set(Phase::Idle);
        self.distance.set(0.0);
    }

    fn set_size(&self, width: i32, height: i32) {
        self.called();
        self.size.set((width, height));
    }
}

/// A fake factory: stretch, or the glow of a platform before `onPullDistance`.
#[derive(Default)]
pub struct FakeEdgeEffects {
    glow: bool,
    made: RefCell<Vec<(u32, EdgeEffectSide, Rc<FakeEdgeEffect>)>>,
    released: RefCell<Vec<u32>>,
    calls: Rc<Cell<u32>>,
}

impl FakeEdgeEffects {
    /// A stretch's effects, as API 31 and up.
    pub fn stretch() -> Rc<FakeEdgeEffects> {
        Rc::new(FakeEdgeEffects::default())
    }

    /// A glow's effects, as before API 31.
    pub fn glow() -> Rc<FakeEdgeEffects> {
        Rc::new(FakeEdgeEffects { glow: true, ..FakeEdgeEffects::default() })
    }

    /// The effect made for `side`, of the last scroller to make one.
    pub fn at(&self, side: EdgeEffectSide) -> Option<Rc<FakeEdgeEffect>> {
        self.made.borrow().iter().rev().find(|made| made.1 == side).map(|made| made.2.clone())
    }

    /// How many effects were made: none until a scroller hits an edge, or is let go at 1 px/s or
    /// less, whose velocity is left to the effect.
    pub fn made(&self) -> usize {
        self.made.borrow().len()
    }

    /// Calls the effects got, standing for what a platform's cost: none for a normal scroll.
    pub fn calls(&self) -> u32 {
        self.calls.get()
    }

    /// The owners whose effects were let go.
    pub fn released(&self) -> Vec<u32> {
        self.released.borrow().clone()
    }

    /// What a host's frame does to every effect: `EdgeEffect.draw`.
    pub fn draw(&self) {
        for (_, _, effect) in &*self.made.borrow() {
            effect.draw();
        }
    }
}

impl EdgeEffectFactory for FakeEdgeEffects {
    fn create_edge_effect(
        &self,
        owner: u32,
        side: EdgeEffectSide,
        glow_color: Color,
    ) -> Rc<dyn EdgeEffect> {
        let effect = Rc::new(FakeEdgeEffect {
            glow: self.glow,
            phase: Cell::new(Phase::Idle),
            distance: Cell::new(0.0),
            size: Cell::new((0, 0)),
            displacement: Cell::new(None),
            absorbed: RefCell::default(),
            releases: Cell::new(0),
            glow_color,
            calls: self.calls.clone(),
        });
        self.made.borrow_mut().push((owner, side, effect.clone()));
        effect
    }

    fn release_edge_effects(&self, owner: u32) {
        self.released.borrow_mut().push(owner);
    }
}
