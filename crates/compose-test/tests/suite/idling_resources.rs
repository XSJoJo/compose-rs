//! Idling resources (D908), as Compose's `junit4/IdlingResourceRegistryTest` and
//! `RobolectricIdlingResourceTest`. Compose's registry is internal: its aggregate idleness and
//! its polling job (Espresso polls a busy registry until it idles, then calls back). compose-rs
//! has no registry object: waiting for idle asks each resource on each turn of its loop, so the
//! registry's tests are adapted to what waiting does: asks an idle resource once and goes on
//! (no polling), and asks a busy one until it idles (polling that ends).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_test::*;

/// A resource busy for its first `busy_for` looks, counting the looks.
struct TestIdlingResource {
    busy_for: Cell<u32>,
    looks: Cell<u32>,
}

impl TestIdlingResource {
    fn new(busy_for: u32) -> Rc<TestIdlingResource> {
        Rc::new(TestIdlingResource { busy_for: Cell::new(busy_for), looks: Cell::new(0) })
    }

    /// Busy again for `looks` more looks.
    fn make_busy(&self, looks: u32) {
        self.busy_for.set(self.looks.get() + looks);
    }
}

impl IdlingResource for TestIdlingResource {
    fn is_idle_now(&self) -> bool {
        self.looks.set(self.looks.get() + 1);
        self.looks.get() > self.busy_for.get()
    }
}

fn register(ui: &mut ComposeTestRule, resource: &Rc<TestIdlingResource>) {
    let resource: Rc<dyn IdlingResource> = resource.clone();
    ui.register_idling_resource(resource);
}

/// Waiting asks `resource` once and goes on: no polling.
#[track_caller]
fn assert_not_polling(ui: &mut ComposeTestRule, resource: &TestIdlingResource) {
    let before = resource.looks.get();
    ui.wait_for_idle();
    assert_eq!(resource.looks.get() - before, 1, "asked once");
    assert_eq!(ui.current_time_millis(), 0, "the clock stayed");
}

/// Waiting asks `resource`, busy for `busy` more looks, until it idles: polling that ends.
#[track_caller]
fn assert_that_polling_starts_and_ends(
    ui: &mut ComposeTestRule,
    resource: &TestIdlingResource,
    busy: u32,
) {
    let before = resource.looks.get();
    ui.wait_for_idle();
    assert_eq!(resource.looks.get() - before, busy + 1, "asked until idle");
    let before = resource.looks.get();
    ui.wait_for_idle();
    assert_eq!(resource.looks.get() - before, 1, "idle now: asked once");
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/IdlingResourceRegistryTest.kt, isIdleNow_0_IdlingResources
#[test]
fn is_idle_now_0_idling_resources() {
    let mut ui = compose_test(|| {});
    assert!(!ui.has_pending_work());
    ui.wait_for_idle();
    assert_eq!(ui.current_time_millis(), 0);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/IdlingResourceRegistryTest.kt, isIdleNow_1_IdlingResource
#[test]
fn is_idle_now_1_idling_resource() {
    let mut ui = compose_test(|| {});
    let resource = TestIdlingResource::new(0);
    register(&mut ui, &resource);
    assert_not_polling(&mut ui, &resource);
    resource.make_busy(3);
    assert_that_polling_starts_and_ends(&mut ui, &resource, 3);
    resource.make_busy(2);
    assert_that_polling_starts_and_ends(&mut ui, &resource, 2);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/IdlingResourceRegistryTest.kt, isIdleNow_2_IdlingResources
/// Waiting goes on once both are idle: each busy one is asked until it idles.
#[test]
fn is_idle_now_2_idling_resources() {
    let mut ui = compose_test(|| {});
    let (first, second) = (TestIdlingResource::new(0), TestIdlingResource::new(0));
    register(&mut ui, &first);
    register(&mut ui, &second);
    for (busy_first, busy_second) in [(0, 0), (2, 0), (0, 2), (1, 3)] {
        first.make_busy(busy_first);
        second.make_busy(busy_second);
        let looks = (first.looks.get(), second.looks.get());
        ui.wait_for_idle();
        let rounds = busy_first.max(busy_second) + 1;
        assert_eq!(
            (first.looks.get() - looks.0, second.looks.get() - looks.1),
            (rounds, rounds),
            "both asked each round, until both idle"
        );
    }
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/IdlingResourceRegistryTest.kt, isIdleNow_true_doesNotStartPolling
#[test]
fn is_idle_now_true_does_not_start_polling() {
    let mut ui = compose_test(|| {});
    let resource = TestIdlingResource::new(0);
    register(&mut ui, &resource);
    assert_not_polling(&mut ui, &resource);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/IdlingResourceRegistryTest.kt, isIdleOrStartPolling_emptyRegister_doesNotStartPolling
#[test]
fn is_idle_or_start_polling_empty_register_does_not_start_polling() {
    let mut ui = compose_test(|| {});
    ui.wait_for_idle();
    assert_eq!(ui.current_time_millis(), 0);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/IdlingResourceRegistryTest.kt, isIdleOrStartPolling_idleRegister_doesNotStartPolling
#[test]
fn is_idle_or_start_polling_idle_register_does_not_start_polling() {
    let mut ui = compose_test(|| {});
    let resource = TestIdlingResource::new(0);
    register(&mut ui, &resource);
    assert_not_polling(&mut ui, &resource);
    assert_not_polling(&mut ui, &resource);
}

// Compose: compose/ui/ui-test/src/androidDeviceTest/kotlin/androidx/compose/ui/test/junit4/IdlingResourceRegistryTest.kt, isIdleOrStartPolling_busyRegister_doesStartPolling
#[test]
fn is_idle_or_start_polling_busy_register_does_start_polling() {
    let mut ui = compose_test(|| {});
    let resource = TestIdlingResource::new(5);
    register(&mut ui, &resource);
    assert_that_polling_starts_and_ends(&mut ui, &resource, 5);
}

// ---- RobolectricIdlingResourceTest -------------------------------------------------------------

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricIdlingResourceTest.kt, testIdlingResourcesAreQueried
/// A resource that idles after ten reads: waiting reads it until then.
#[test]
fn test_idling_resources_are_queried() {
    run_compose_ui_test(|ui| {
        let resource = TestIdlingResource::new(10);
        let registered: Rc<dyn IdlingResource> = resource.clone();
        ui.register_idling_resource(registered.clone());
        assert_eq!(resource.looks.get(), 0);
        ui.wait_for_idle();
        assert!(resource.looks.get() >= 10, "read {} times", resource.looks.get());
        assert!(resource.is_idle_now());
        ui.unregister_idling_resource(&registered);
    });
}

/// A resource whose look needs what a task posted to the UI thread sets.
struct StrictlyOrdered(Rc<RefCell<Option<String>>>);

impl IdlingResource for StrictlyOrdered {
    fn is_idle_now(&self) -> bool {
        !self.0.borrow().as_ref().expect("the posted task ran first").is_empty()
    }
}

// Compose: compose/ui/ui-test/src/androidHostTest/kotlin/androidx/compose/ui/test/RobolectricIdlingResourceTest.kt, testIsIdleNow_calledBeforeMainQueueDrained_causesCrash
/// Waiting runs the tasks posted to the UI thread before it asks the resources.
#[test]
fn test_is_idle_now_called_before_main_queue_drained_causes_crash() {
    run_compose_ui_test(|ui| {
        let state: Rc<RefCell<Option<String>>> = Rc::default();
        let set = state.clone();
        // As `Handler(Looper.getMainLooper()).post { … }`.
        compose_ui::platform::run_soon(move || {
            *set.borrow_mut() = Some("Initialized and Ready".to_string());
        });
        let resource: Rc<dyn IdlingResource> = Rc::new(StrictlyOrdered(state));
        ui.register_idling_resource(resource.clone());
        ui.wait_for_idle();
        ui.unregister_idling_resource(&resource);
    });
}
