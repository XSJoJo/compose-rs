//! What the touch selection tests share, as Compose's `SelectionHandleTestUtils`.

use compose_foundation::{Handle, SELECTION_HANDLE_INFO_KEY, SelectionHandleInfo};
use compose_test::{ComposeTestRule, Matcher};

/// A handle's node, or any handle's, as Compose's `isSelectionHandle`.
pub fn is_selection_handle(handle: Option<Handle>) -> Matcher {
    let description = match handle {
        Some(handle) => format!("is {handle:?} handle"),
        None => "is any handle".to_string(),
    };
    Matcher::new(description, move |config| match handle {
        None => config.contains(&SELECTION_HANDLE_INFO_KEY),
        Some(handle) => {
            config.get(&SELECTION_HANDLE_INFO_KEY).is_some_and(|info| info.handle == handle)
        }
    })
}

/// What `handle`'s node says of it, as Compose's `getSelectionHandleInfo`.
pub fn handle_info(ui: &mut ComposeTestRule, handle: Handle) -> SelectionHandleInfo {
    let node = ui.on_node(is_selection_handle(Some(handle))).fetch_semantics_node();
    *node.config().get(&SELECTION_HANDLE_INFO_KEY).expect("a handle's info")
}

/// `handle` points at (`x`, `y`) dp within 5 dp, as Compose's `assertHandlePositionMatches` (the
/// tests' density is 1).
pub fn assert_handle_position(ui: &mut ComposeTestRule, handle: Handle, x: f32, y: f32) {
    let found = handle_info(ui, handle).position;
    assert!(
        (found.x - x).abs() <= 5.0 && (found.y - y).abs() <= 5.0,
        "expected {handle:?} at ({x}, {y}), found {found:?}"
    );
}
