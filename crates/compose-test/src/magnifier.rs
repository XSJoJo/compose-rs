//! A platform magnifier for tests: what a test rule's tree makes its magnifiers with, as Compose's
//! device tests run where Android's `Magnifier` is (API 28+) and its own tests use
//! `PlatformMagnifierFactory` fakes. It records what each magnifier was made with and told
//! (D2072).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_ui::{
    IntSize, Offset, PlatformMagnifier, PlatformMagnifierConfig, PlatformMagnifierFactory,
};

/// Makes [`TestMagnifier`]s, each `size` big, and keeps them in the order made.
pub struct TestMagnifierFactory {
    can_update_zoom: Cell<bool>,
    size: Cell<IntSize>,
    made: RefCell<Vec<Rc<TestMagnifier>>>,
}

impl Default for TestMagnifierFactory {
    fn default() -> TestMagnifierFactory {
        TestMagnifierFactory::new()
    }
}

impl TestMagnifierFactory {
    /// A factory updating the zoom in place (Android 29's and up), of magnifiers 0 by 0, as
    /// Compose's tests' `defaultMagnifierSize`.
    pub fn new() -> TestMagnifierFactory {
        TestMagnifierFactory {
            can_update_zoom: Cell::new(true),
            size: Cell::new(IntSize::ZERO),
            made: RefCell::new(Vec::new()),
        }
    }

    /// Whether its magnifiers take a new zoom without being made anew.
    pub fn set_can_update_zoom(&self, can: bool) {
        self.can_update_zoom.set(can);
    }

    /// The size of the magnifiers it makes from now on.
    pub fn set_size(&self, size: IntSize) {
        self.size.set(size);
    }

    /// How many magnifiers it made.
    pub fn creation_count(&self) -> usize {
        self.made.borrow().len()
    }

    /// The magnifiers it made, the first first.
    pub fn made(&self) -> Vec<Rc<TestMagnifier>> {
        self.made.borrow().clone()
    }

    /// The last one it made, if any.
    pub fn last(&self) -> Option<Rc<TestMagnifier>> {
        self.made.borrow().last().cloned()
    }

    /// The magnifiers showing now (shown and not dismissed since).
    pub fn showing(&self) -> Vec<Rc<TestMagnifier>> {
        self.made.borrow().iter().filter(|magnifier| magnifier.is_showing()).cloned().collect()
    }
}

impl PlatformMagnifierFactory for TestMagnifierFactory {
    fn can_update_zoom(&self) -> bool {
        self.can_update_zoom.get()
    }

    fn create(&self, config: &PlatformMagnifierConfig) -> Rc<dyn PlatformMagnifier> {
        let magnifier = Rc::new(TestMagnifier::new(*config, self.size.get()));
        self.made.borrow_mut().push(magnifier.clone());
        magnifier
    }
}

/// What a [`TestMagnifier`] was last shown with.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MagnifierShown {
    pub source_center: Offset,
    pub magnifier_center: Offset,
    pub zoom: f32,
}

/// One magnifier: what it was made with, counts of what it was told, and where it shows.
pub struct TestMagnifier {
    config: PlatformMagnifierConfig,
    size: Cell<IntSize>,
    shown: Cell<Option<MagnifierShown>>,
    property_updates: Cell<usize>,
    content_updates: Cell<usize>,
    dismisses: Cell<usize>,
}

impl TestMagnifier {
    /// A magnifier of `size`, made with `config`.
    pub fn new(config: PlatformMagnifierConfig, size: IntSize) -> TestMagnifier {
        TestMagnifier {
            config,
            size: Cell::new(size),
            shown: Cell::new(None),
            property_updates: Cell::new(0),
            content_updates: Cell::new(0),
            dismisses: Cell::new(0),
        }
    }

    pub fn config(&self) -> PlatformMagnifierConfig {
        self.config
    }

    /// The size it reports from now on.
    pub fn set_size(&self, size: IntSize) {
        self.size.set(size);
    }

    /// Where it shows, `None` while it's dismissed or before it's shown.
    pub fn shown(&self) -> Option<MagnifierShown> {
        self.shown.get()
    }

    pub fn is_showing(&self) -> bool {
        self.shown.get().is_some()
    }

    /// How many times it was shown (`update`), as Compose's tests' `propertyUpdateCount`.
    pub fn property_update_count(&self) -> usize {
        self.property_updates.get()
    }

    /// How many times its content was copied again, as `contentUpdateCount`.
    pub fn content_update_count(&self) -> usize {
        self.content_updates.get()
    }

    /// How many times it was dismissed, as `dismissCount`.
    pub fn dismiss_count(&self) -> usize {
        self.dismisses.get()
    }
}

impl PlatformMagnifier for TestMagnifier {
    fn size(&self) -> IntSize {
        self.size.get()
    }

    fn update_content(&self) {
        self.content_updates.set(self.content_updates.get() + 1);
    }

    fn update(&self, source_center: Offset, magnifier_center: Offset, zoom: f32) {
        self.property_updates.set(self.property_updates.get() + 1);
        self.shown.set(Some(MagnifierShown { source_center, magnifier_center, zoom }));
    }

    fn dismiss(&self) {
        self.dismisses.set(self.dismisses.get() + 1);
        self.shown.set(None);
    }
}
