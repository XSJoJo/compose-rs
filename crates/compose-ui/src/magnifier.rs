//! The platform's magnifier behind a trait, as Compose's internal `PlatformMagnifierFactory` and
//! `PlatformMagnifier`: Android's `android.widget.Magnifier` (API 28+). A host that has one sets
//! its factory on its tree (`UiTree::set_platform_magnifier_factory`), and
//! `compose_foundation`'s `Modifier.magnifier` and text selection show it; a host without one (the
//! desktop's, as Compose's `commonStubs`) has none, and the modifier is a no-op there (D2072).

use std::rc::Rc;

use crate::geometry::Offset;
use crate::node_context::NodeContext;
use crate::platform_services::of_composing_tree;
use crate::tree::UiTree;
use crate::unit::{Density, Dp, DpSize, IntSize};

/// One magnifier, as Compose's `PlatformMagnifier` over a `Magnifier`.
pub trait PlatformMagnifier {
    /// Its actual size, even if not asked for at creation, as `Magnifier.getWidth`/`getHeight`.
    fn size(&self) -> IntSize;
    /// Copies the magnified pixels again, as `Magnifier.update`.
    fn update_content(&self);
    /// Sets what may change without making it anew (the zoom, where the factory
    /// `can_update_zoom`) and shows it over `source_center` (in the root's coordinates), at
    /// `magnifier_center` if specified (else where the platform puts it), as `Magnifier.show`.
    fn update(&self, source_center: Offset, magnifier_center: Offset, zoom: f32);
    /// Hides it, as `Magnifier.dismiss`.
    fn dismiss(&self);
}

/// What a magnifier is made with, as the arguments of Compose's `PlatformMagnifierFactory.create`
/// (its `view` is the tree's host). Unspecified values are the platform's defaults.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PlatformMagnifierConfig {
    /// The platform's text magnifier's defaults (Android's deprecated `Magnifier(View)`), as a
    /// text selection's magnifier is made; the other values are then ignored.
    pub use_text_default: bool,
    pub size: DpSize,
    pub corner_radius: Dp,
    pub elevation: Dp,
    pub clipping_enabled: bool,
    pub density: Density,
    /// NaN: the platform's.
    pub initial_zoom: f32,
}

/// Makes a host's magnifiers, as Compose's `PlatformMagnifierFactory`.
pub trait PlatformMagnifierFactory {
    /// Whether a magnifier it made takes a new zoom in `update`; if not, a zoom change makes it
    /// anew (Android 28's, where the zoom is set only at creation).
    fn can_update_zoom(&self) -> bool;
    fn create(&self, config: &PlatformMagnifierConfig) -> Rc<dyn PlatformMagnifier>;
}

/// Whether the tree composing now has a platform magnifier, as Compose's
/// `isPlatformMagnifierSupported()` (Android's API 28 and up): `None` outside composition, where
/// no tree is known.
pub fn is_platform_magnifier_supported() -> Option<bool> {
    of_composing_tree(|tree| tree.magnifier_factory.is_some())
}

impl UiTree {
    /// Sets the factory of the platform's magnifiers (the host's, before composing), or removes
    /// it: without one, magnifiers are a no-op.
    pub fn set_platform_magnifier_factory(
        &mut self,
        factory: Option<Rc<dyn PlatformMagnifierFactory>>,
    ) {
        self.magnifier_factory = factory;
    }

    pub fn platform_magnifier_factory(&self) -> Option<Rc<dyn PlatformMagnifierFactory>> {
        self.magnifier_factory.clone()
    }
}

impl NodeContext<'_> {
    /// The tree's magnifier factory, if its host has one.
    pub fn platform_magnifier_factory(&self) -> Option<Rc<dyn PlatformMagnifierFactory>> {
        self.tree.magnifier_factory.clone()
    }
}
