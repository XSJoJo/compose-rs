//! Selection handles drawn by Skia, from Compose's `SelectionHandlesTest`: the icon points left
//! or right, and a handle pointing nowhere draws nothing and says it isn't visible.

#![allow(non_snake_case)]

use std::collections::HashSet;
use std::rc::Rc;

use compose::foundation::layout::*;
use compose::foundation::*;
use compose::test::*;
use compose::ui::{Alignment, Color, Modifier, Offset, ResolvedTextDirection};

const HANDLE: Color = Color::BLACK;
const BACKGROUND: Color = Color::WHITE;
const TAG: &str = "contentTag";
/// How far into the icon its corners' colors are read, as Compose's `inwardBias`.
const INWARD: i32 = 3;

/// `content` centered in a white box filling the window, handles drawn black, as Compose's
/// `setContent`.
fn handles_rule(content: impl Fn() + 'static) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    let content = Rc::new(content);
    // A phone's density, as Compose's device's: the corners read are well off the icon's arc.
    let measurer = Box::new(compose::ui::platform::FixedTextMeasurer::new());
    let size = compose::ui::IntSize::new(600, 600);
    let mut ui = compose_test_with(size, compose::ui::Density::new(3.0), measurer, move || {
        let content = content.clone();
        Box!(
            modifier = Modifier.fill_max_size().background(BACKGROUND),
            content_alignment = Alignment::CENTER,
            {
                let content = content.clone();
                let colors =
                    TextSelectionColors { handle_color: HANDLE, background_color: BACKGROUND };
                compose::runtime::CompositionLocalProvider(
                    LOCAL_TEXT_SELECTION_COLORS.provides(colors),
                    move || content(),
                );
            }
        );
    });
    ui.wait_for_idle();
    ui
}

fn icon(left: bool, visible: bool) -> ComposeTestRule {
    handles_rule(move || {
        let visible: Rc<dyn Fn() -> bool> = Rc::new(move || visible);
        SelectionHandleIcon(Modifier.test_tag(TAG), visible, left);
    })
}

fn unique_pixels(image: &CapturedImage) -> HashSet<Color> {
    image.pixels().iter().copied().collect()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, SelectionHandleIcon_left_pointsTopRight
#[test]
fn a_left_handles_icon_points_up_and_right() {
    let mut ui = icon(true, true);
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    assert_eq!(image.pixel(INWARD, INWARD), BACKGROUND);
    assert_eq!(image.pixel(image.width() - INWARD - 1, INWARD), HANDLE);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, SelectionHandleIcon_right_pointsTopLeft
#[test]
fn a_right_handles_icon_points_up_and_left() {
    let mut ui = icon(false, true);
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    assert_eq!(image.pixel(INWARD, INWARD), HANDLE);
    assert_eq!(image.pixel(image.width() - INWARD - 1, INWARD), BACKGROUND);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, SelectionHandleIcon_left_notVisible
#[test]
fn a_left_handles_hidden_icon_draws_nothing() {
    let mut ui = icon(true, false);
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    assert_eq!(unique_pixels(&image), HashSet::from([BACKGROUND]));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, SelectionHandleIcon_right_notVisible
#[test]
fn a_right_handles_hidden_icon_draws_nothing() {
    let mut ui = icon(false, false);
    let image = ui.on_node_with_tag(TAG).capture_to_image();
    assert_eq!(unique_pixels(&image), HashSet::from([BACKGROUND]));
}

fn handle_matcher(handle: Handle) -> Matcher {
    Matcher::new(format!("is {handle:?} handle"), move |config| {
        config.get(&SELECTION_HANDLE_INFO_KEY).is_some_and(|info| info.handle == handle)
    })
}

fn visible(ui: &mut ComposeTestRule, handle: Handle) -> bool {
    let node = ui.on_node(handle_matcher(handle)).fetch_semantics_node();
    node.config().get(&SELECTION_HANDLE_INFO_KEY).expect("a handle's info").visible
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/SelectionHandlesTest.kt, DefaultSelectionHandle_visibilityAndSemantics_changesFromOffsetState
#[test]
fn a_handle_pointing_nowhere_draws_nothing_and_isnt_visible() {
    let offset: &'static compose::runtime::OwnedState<Option<Offset>> =
        Box::leak(Box::new(compose::runtime::mutable_state_of(Some(Offset::ZERO))));
    let mut ui = handles_rule(move || {
        let provider: Rc<dyn Fn() -> Option<Offset>> = Rc::new(move || offset.get());
        SelectionHandle(
            provider,
            false,
            ResolvedTextDirection::Ltr,
            false,
            Modifier.test_tag("testTag"),
        );
    });
    // The window's center, where Compose's box's bounds are.
    let center = ui.on_root().fetch_semantics_snapshot().bounds.center();
    offset.set(Some(center));
    ui.wait_for_idle();
    let shown = |ui: &mut ComposeTestRule| {
        let image = ui.on_node_with_tag("testTag").capture_to_image();
        unique_pixels(&image)
    };
    assert!(shown(&mut ui).contains(&HANDLE));
    ui.on_node(handle_matcher(Handle::SelectionStart)).assert_does_not_exist();
    assert!(visible(&mut ui, Handle::SelectionEnd));

    offset.set(None);
    ui.wait_for_idle();
    assert_eq!(shown(&mut ui), HashSet::from([BACKGROUND]));
    ui.on_node(handle_matcher(Handle::SelectionStart)).assert_does_not_exist();
    assert!(!visible(&mut ui, Handle::SelectionEnd));

    offset.set(Some(center));
    ui.wait_for_idle();
    assert!(shown(&mut ui).contains(&HANDLE));
    assert!(visible(&mut ui, Handle::SelectionEnd));
}
