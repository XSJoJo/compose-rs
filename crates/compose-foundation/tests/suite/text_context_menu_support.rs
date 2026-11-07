//! What the text context menu's tests share, as Compose's `TextContextMenuTestUtils`.

use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;

use compose_foundation::text_context_menu::*;
use compose_runtime::CoroutineScope;
use compose_ui::{LayoutCoordinates, Offset, Rect};

/// How a launched job went, as Compose's `Job` states: running, completed, or cancelled (its
/// future dropped before it ended).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Outcome {
    Active,
    Completed,
    Cancelled,
}

/// A launched job and how it went.
#[derive(Clone)]
pub(crate) struct JobProbe {
    pub job: compose_runtime::Job,
    outcome: Rc<Cell<Outcome>>,
}

impl JobProbe {
    pub(crate) fn outcome(&self) -> Outcome {
        self.outcome.get()
    }

    pub(crate) fn assert_active(&self) {
        assert_eq!(self.outcome(), Outcome::Active, "the job isn't active");
    }

    /// Ended, cancelled or not, as Compose's `isCompleted`.
    pub(crate) fn assert_completed(&self) {
        assert_ne!(self.outcome(), Outcome::Active, "the job didn't end");
    }

    pub(crate) fn assert_cancelled(&self) {
        assert_eq!(self.outcome(), Outcome::Cancelled, "the job wasn't cancelled");
    }

    pub(crate) fn assert_completed_successfully(&self) {
        assert_eq!(self.outcome(), Outcome::Completed, "the job didn't complete");
    }
}

/// Sets the outcome to cancelled if dropped while still active.
struct Watch(Rc<Cell<Outcome>>);

impl Drop for Watch {
    fn drop(&mut self) {
        if self.0.get() == Outcome::Active {
            self.0.set(Outcome::Cancelled);
        }
    }
}

/// Launches `future` in `scope`, watching how it goes.
pub(crate) fn launch(
    scope: &CoroutineScope,
    future: impl Future<Output = ()> + 'static,
) -> JobProbe {
    let outcome = Rc::new(Cell::new(Outcome::Active));
    let watched = outcome.clone();
    let job = scope.launch(async move {
        let watch = Watch(watched);
        future.await;
        watch.0.set(Outcome::Completed);
    });
    JobProbe { job, outcome }
}

pub(crate) fn label_for_number(number: i32) -> String {
    format!("Item {number}")
}

pub(crate) fn number_for_label(label: &str) -> i32 {
    label.rsplit(' ').next().and_then(|it| it.parse().ok()).expect("a numbered label")
}

/// An item keyed and labeled by `number`, as Compose's `testItem`.
pub(crate) fn test_item(
    number: i32,
    on_click: impl Fn(&dyn TextContextMenuSession) + 'static,
) -> TextContextMenuComponent {
    TextContextMenuComponent::Item(TextContextMenuItem::new(
        TextContextMenuKey::new(number),
        label_for_number(number),
        None,
        on_click,
    ))
}

pub(crate) fn numbers_to_data(numbers: &[i32]) -> TextContextMenuData {
    TextContextMenuData::new(numbers.iter().map(|&number| test_item(number, |_| {})).collect())
}

/// The destination's center, as Compose's `defaultPositioner`.
pub(crate) fn default_positioner(destination: &LayoutCoordinates) -> Offset {
    let size = destination.size();
    Offset::new((size.width / 2) as f32, (size.height / 2) as f32)
}

pub(crate) struct TestDataProvider {
    pub positioner: Box<dyn Fn(&LayoutCoordinates) -> Offset>,
    pub data: Box<dyn Fn() -> TextContextMenuData>,
}

impl TextContextMenuDataProvider for TestDataProvider {
    fn position(&self, destination: &LayoutCoordinates) -> Offset {
        (self.positioner)(destination)
    }

    fn content_bounds(&self, destination: &LayoutCoordinates) -> Rect {
        let position = self.position(destination);
        Rect::from_corners(position, position)
    }

    fn data(&self) -> TextContextMenuData {
        (self.data)()
    }
}

/// A data provider of numbered items at the destination's center, as Compose's
/// `testDataProvider(vararg itemNumbers)`.
pub(crate) fn test_data_provider(numbers: &[i32]) -> Rc<dyn TextContextMenuDataProvider> {
    let numbers = numbers.to_vec();
    Rc::new(TestDataProvider {
        positioner: Box::new(default_positioner),
        data: Box::new(move || numbers_to_data(&numbers)),
    })
}

/// A data provider of what `data` and `positioner` say, as Compose's `testDataProvider(positioner,
/// data)`.
pub(crate) fn data_provider_with(
    positioner: impl Fn(&LayoutCoordinates) -> Offset + 'static,
    data: impl Fn() -> TextContextMenuData + 'static,
) -> Rc<dyn TextContextMenuDataProvider> {
    Rc::new(TestDataProvider { positioner: Box::new(positioner), data: Box::new(data) })
}
