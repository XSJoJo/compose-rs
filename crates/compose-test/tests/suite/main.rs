//! compose-test's own tests in one binary, each file a module, so they build and link once
//! (docs/08-fast-builds.md). The harness's APIs are tested as Compose's ui-test tests them, each
//! test citing the one it adapts.

extern crate compose_runtime as compose;

#[path = "../../../compose-ui/tests/support/mod.rs"]
mod support;

mod actions;
mod assert_text;
mod bounds;
mod clock;
mod configuration_override;
mod dispatcher;
mod displayed;
mod idling_resources;
mod indirect;
mod injection_scopes;
mod input;
mod key_and_mouse_events;
mod key_events;
mod layout_coordinates_helper;
mod mouse_events;
mod restoration;
mod review_d;
mod robolectric;
mod rule;
mod run;
mod sample_font;
mod selectors;
mod swipes;
mod touch_events;
mod trackpad;
mod unhandled_exceptions;
mod wait_until_node_count;
mod waiting;
