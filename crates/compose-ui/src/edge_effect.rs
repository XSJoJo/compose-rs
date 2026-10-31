//! The platform's edge effects behind a trait: Android's `android.widget.EdgeEffect`, whose physics
//! and drawing stay the platform's (docs/parity/design/stretch-overscroll.md). A host that has them
//! sets their factory on its tree (`UiTree::set_edge_effect_factory`), and
//! `compose_foundation`'s overscroll runs its events over them; other hosts have none, as Compose's
//! non-Android stub.

use std::any::Any;
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use crate::graphics::Color;
use crate::node_context::NodeContext;
use crate::platform_services::of_composing_tree;
use crate::tree::UiTree;

/// One edge's effect, as `android.widget.EdgeEffect` with `EdgeEffectCompat`'s fallbacks: a
/// platform without `onPullDistance` pulls by the whole delta, and its `distance` stays 0.
///
/// Its distance goes as the platform draws it: what a release or an absorb starts moves it only
/// while frames are drawn.
pub trait EdgeEffect {
    /// Pulls it by `delta`, a fraction of its length, at `displacement` along it: returns what it
    /// took of `delta`, as `onPullDistance`.
    fn on_pull_distance(&self, delta: f32, displacement: f32) -> f32;
    /// The pull is over: it goes back by itself, as `onRelease`.
    fn on_release(&self);
    /// A scroll of `delta` went the other way, as `onReleaseWithOppositeDelta`: Android's glow
    /// releases only past a small distance, so a shaking finger doesn't end it (b/242864658).
    fn on_release_with_opposite_delta(&self, _delta: f32) {
        self.on_release();
    }
    /// A fling reached the edge at `velocity` pixels per second, as `onAbsorb`.
    fn on_absorb(&self, velocity: i32);
    /// How far it's pulled, a fraction of its length, as `getDistance`.
    fn distance(&self) -> f32;
    /// Whether it has nothing left to draw, as `isFinished`, what clears it included.
    fn is_finished(&self) -> bool;
    /// Stops it at once, as `finish`.
    fn finish(&self);
    /// Its length along the edge and across it, as `setSize`.
    fn set_size(&self, width: i32, height: i32);
}

/// The edge an [`EdgeEffect`] is at. Its number is what the Android replayer reads.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum EdgeEffectSide {
    Top = 0,
    Bottom = 1,
    Left = 2,
    Right = 3,
}

/// Makes a host's edge effects.
pub trait EdgeEffectFactory {
    /// The effect at `side` of the scroller numbered `owner` ([`next_edge_effects_owner`]), which
    /// names them in the display list. Android's glow is `glow_color` where the platform glows.
    fn create_edge_effect(
        &self,
        owner: u32,
        side: EdgeEffectSide,
        glow_color: Color,
    ) -> Rc<dyn EdgeEffect>;

    /// `owner`'s effects are gone: what the host keeps for them can go.
    fn release_edge_effects(&self, owner: u32);
}

/// A number of its own for a scroller's edge effects.
pub fn next_edge_effects_owner() -> u32 {
    thread_local! {
        static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
    }
    NEXT.with(|next| next.replace(next.get().wrapping_add(1)))
}

/// A host's edge effects, as a tree holds them, with what a library made of them once: the same
/// `Rc` for as long as the tree has this factory (an overscroll factory compares by identity).
pub struct PlatformEdgeEffects {
    pub factory: Rc<dyn EdgeEffectFactory>,
    made: OnceCell<Rc<dyn Any>>,
}

impl PlatformEdgeEffects {
    /// What `make` made, the first time this is asked.
    pub fn made_from(&self, make: impl FnOnce() -> Rc<dyn Any>) -> Rc<dyn Any> {
        self.made.get_or_init(make).clone()
    }
}

thread_local! {
    /// The edge effects of the tree whose node reads a local, outside composition.
    static READING: RefCell<Option<Rc<PlatformEdgeEffects>>> = const { RefCell::new(None) };
}

/// The edge effects of the tree composing now, or whose node is in
/// [`with_platform_edge_effects`](NodeContext::with_platform_edge_effects): `None` elsewhere, or
/// without any.
pub fn platform_edge_effects() -> Option<Rc<PlatformEdgeEffects>> {
    READING
        .with(|reading| reading.borrow().clone())
        .or_else(|| of_composing_tree(|tree| tree.edge_effects.clone()).flatten())
}

impl NodeContext<'_> {
    /// Runs `read` with [`platform_edge_effects`] saying this tree's: a node reads a local whose
    /// default is the composing tree's outside composition (`vertical_scroll` at a press, D1474).
    pub fn with_platform_edge_effects<R>(&self, read: impl FnOnce() -> R) -> R {
        READING.with(|reading| *reading.borrow_mut() = self.tree.edge_effects.clone());
        let result = read();
        READING.with(|reading| *reading.borrow_mut() = None);
        result
    }
}

impl UiTree {
    /// Sets the factory the local overscroll factory's default makes its effects with: the host's,
    /// before composing.
    pub fn set_edge_effect_factory(&mut self, factory: Rc<dyn EdgeEffectFactory>) {
        self.edge_effects = Some(Rc::new(PlatformEdgeEffects { factory, made: OnceCell::new() }));
    }
}
