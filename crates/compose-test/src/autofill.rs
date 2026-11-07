//! The platform's autofill for tests: what a test rule's tree tells it, recorded, as Compose's
//! device tests read it through a mock `PlatformAutofillManager` (D1680).

use std::cell::RefCell;

use compose_ui::platform::{PlatformAutofillCall, PlatformAutofillManager};

/// Records what the tree tells the platform's autofill. A test rule's tree has one (Compose's
/// device tests run on Android 8 and up, where every view has autofill): `rule.autofill()`.
#[derive(Default)]
pub struct TestAutofillManager {
    calls: RefCell<Vec<PlatformAutofillCall>>,
}

impl PlatformAutofillManager for TestAutofillManager {
    fn call(&self, call: PlatformAutofillCall) {
        self.calls.borrow_mut().push(call);
    }
}

impl TestAutofillManager {
    pub fn new() -> TestAutofillManager {
        TestAutofillManager::default()
    }

    /// What it was told, in order, since it was last cleared.
    pub fn calls(&self) -> Vec<PlatformAutofillCall> {
        self.calls.borrow().clone()
    }

    /// Forgets what it was told, as Mockito's `clearInvocations`.
    pub fn clear(&self) {
        self.calls.borrow_mut().clear();
    }

    /// How many times it was told `call`.
    pub fn count(&self, call: &PlatformAutofillCall) -> usize {
        self.calls.borrow().iter().filter(|each| *each == call).count()
    }

    /// Checks it was told `call` exactly once and forgets that call, as Mockito's `verify`
    /// (verified calls don't count for `assert_no_more_calls`).
    #[track_caller]
    pub fn verify(&self, call: &PlatformAutofillCall) {
        self.verify_times(call, 1);
    }

    /// Checks it was told `call` `times` times and forgets those calls, as
    /// `verify(mock, times(n))`.
    #[track_caller]
    pub fn verify_times(&self, call: &PlatformAutofillCall, times: usize) {
        let mut calls = self.calls.borrow_mut();
        let found = calls.iter().filter(|each| *each == call).count();
        assert_eq!(found, times, "{call:?} told {found} times, not {times}; all: {calls:?}");
        calls.retain(|each| each != call);
    }

    /// Checks it was never told `call`, as `verify(mock, never())`.
    #[track_caller]
    pub fn verify_never(&self, call: &PlatformAutofillCall) {
        self.verify_times(call, 0);
    }

    /// Checks it was told nothing it wasn't verified to have been told, as
    /// `verifyNoMoreInteractions`.
    #[track_caller]
    pub fn assert_no_more_calls(&self) {
        let calls = self.calls.borrow();
        assert!(calls.is_empty(), "unverified autofill calls: {calls:?}");
    }
}
