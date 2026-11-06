//! The bounds of what a `SelectionContainer` has selected, where its text toolbar will go, from
//! Compose's `SelectionManagerGetSelectedRegionRectTest`.
//!
//! Compose's test gives `getSelectedRegionRect` fake texts and selections; here they're texts of
//! the test font (each character a 20 px square, Compose's unit) in a container, all selected
//! with Cmd+A (Ctrl+A) after a click, or a click alone for a collapsed selection.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer};
use compose_ui::{
    Density, IntSize, Key, KeyEvent, KeyEventType, KeyModifiers, Offset, PointerEvent, Rect,
    UnitExt,
};

const UNIT: f32 = 20.0;
const CONTAINER: &str = "container";

fn style() -> TextStyle {
    TextStyle { font_size: UNIT.sp(), ..TextStyle::DEFAULT }
}

/// `content` in a container tagged `CONTAINER`, `offset` units from the window's top left.
fn container(offset: (f32, f32), content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let content = Rc::new(content);
    let mut ui = compose_test_with(
        IntSize::new(400, 400),
        Density::new(1.0),
        Box::new(measurer),
        move || {
            let content = content.clone();
            let modifier =
                Modifier.offset((offset.0 * UNIT).dp(), (offset.1 * UNIT).dp()).test_tag(CONTAINER);
            SelectionContainer!(modifier = modifier, { content() });
        },
    );
    ui.wait_for_idle();
    ui
}

/// A text of `count` characters, `width` units wide at most.
fn text(count: usize, width: f32, modifier: Modifier) {
    BasicText!(
        "a".repeat(count),
        modifier = modifier.width_in(0.dp(), (width * UNIT).dp()),
        style = style()
    );
}

/// A mouse's click at `at` units in the container: a collapsed selection, and the focus.
fn click(ui: &mut ComposeTestRule, offset: (f32, f32), at: (f32, f32)) {
    let at = Offset::new((offset.0 + at.0) * UNIT + 1.0, (offset.1 + at.1) * UNIT + 1.0);
    ui.host().send_pointer(PointerEvent::press(at));
    ui.host().send_pointer(PointerEvent::release(at));
    ui.wait_for_idle();
}

/// Cmd+A (Ctrl+A): everything selected.
fn select_all(ui: &mut ComposeTestRule) {
    let modifiers = if cfg!(target_os = "macos") {
        KeyModifiers { meta: true, ..Default::default() }
    } else {
        KeyModifiers { ctrl: true, ..Default::default() }
    };
    for kind in [KeyEventType::KeyDown, KeyEventType::KeyUp] {
        let key = Key::Character('a');
        ui.host().send_key(KeyEvent { key, kind, modifiers, from_soft_keyboard: false });
    }
    ui.wait_for_idle();
}

fn region(ui: &mut ComposeTestRule) -> Option<Rect> {
    let container = ui.on_node_with_tag(CONTAINER).fetch_semantics_snapshot().id;
    ui.host().with_tree(|tree| tree.selected_region_rect(container))
}

fn units(left: f32, top: f32, right: f32, bottom: f32) -> Option<Rect> {
    Some(Rect::new(left * UNIT, top * UNIT, right * UNIT, bottom * UNIT))
}

/// One text of `count` characters, `width` units wide at most, `inset` units in the container,
/// all selected.
fn all_of_one_text(count: usize, width: f32, offset: (f32, f32), inset: f32) -> Option<Rect> {
    let mut ui = container(offset, move || {
        text(count, width, Modifier.offset((inset * UNIT).dp(), (inset * UNIT).dp()));
    });
    click(&mut ui, offset, (inset, inset));
    select_all(&mut ui);
    region(&mut ui)
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/selection/SelectionManagerGetSelectedRegionRectTest.kt, whenNoSelectables_returnsDefaultEmpty
#[test]
fn without_a_selection_there_is_no_region() {
    let mut ui = container((0.0, 0.0), || text(10, 20.0, Modifier));
    assert_eq!(region(&mut ui), None);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/selection/SelectionManagerGetSelectedRegionRectTest.kt, whenCollapsedSelection_returnsDefaultEmpty
#[test]
fn a_collapsed_selection_has_no_region() {
    let mut ui = container((0.0, 0.0), || text(10, 20.0, Modifier));
    click(&mut ui, (0.0, 0.0), (5.0, 0.0));
    assert_eq!(region(&mut ui), None);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/selection/SelectionManagerGetSelectedRegionRectTest.kt, whenNoWrapping_rectIsShortAndWide
#[test]
fn one_selected_line_is_a_short_wide_region() {
    assert_eq!(all_of_one_text(10, 20.0, (0.0, 0.0), 0.0), units(0.0, 0.0, 10.0, 1.0));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/selection/SelectionManagerGetSelectedRegionRectTest.kt, whenWrapping_rectIsSlightlyTallerAndSkinnier
#[test]
fn a_selected_text_wrapped_on_two_lines_is_a_taller_narrower_region() {
    assert_eq!(all_of_one_text(10, 5.0, (0.0, 0.0), 0.0), units(0.0, 0.0, 5.0, 2.0));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/selection/SelectionManagerGetSelectedRegionRectTest.kt, whenRootOffsetsChangeEqually_resultDoesNotChange
#[test]
fn a_container_moved_with_its_text_has_the_same_region() {
    assert_eq!(all_of_one_text(10, 5.0, (1.0, 1.0), 0.0), units(0.0, 0.0, 5.0, 2.0));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/selection/SelectionManagerGetSelectedRegionRectTest.kt, whenDifferentCoordinates_resultChangesByTheDifference
#[test]
fn a_text_moved_in_its_container_moves_the_region() {
    assert_eq!(all_of_one_text(10, 5.0, (1.0, 1.0), 1.0), units(1.0, 1.0, 6.0, 3.0));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/selection/SelectionManagerGetSelectedRegionRectTest.kt, whenTwoSelectables_resultTracksBoth
#[test]
fn two_selected_texts_make_one_region() {
    let mut ui = container((0.0, 0.0), || {
        Column!({
            text(5, 20.0, Modifier);
            text(5, 20.0, Modifier);
        });
    });
    click(&mut ui, (0.0, 0.0), (0.0, 0.0));
    select_all(&mut ui);
    assert_eq!(region(&mut ui), units(0.0, 0.0, 5.0, 2.0));
}

/// `text` alone in a container, all selected: its region.
fn all_of(text: &'static str) -> Option<Rect> {
    let mut ui = container((0.0, 0.0), move || {
        BasicText!(text, style = style());
    });
    click(&mut ui, (0.0, 0.0), (0.0, 0.0));
    select_all(&mut ui);
    region(&mut ui)
}

// Not one of Compose's (offsets there are UTF-16's): a text ending in a character of several
// bytes has its last character's box, never a byte inside it.
#[test]
fn a_region_ending_on_a_character_of_two_bytes_is_its_box() {
    assert_eq!(all_of("a\u{e9}"), units(0.0, 0.0, 2.0, 1.0));
}

// Not one of Compose's: an emoji of four bytes, alone and last.
#[test]
fn a_region_of_an_emoji_is_its_box() {
    assert_eq!(all_of("\u{1F600}"), units(0.0, 0.0, 1.0, 1.0));
    assert_eq!(all_of("ab\u{1F600}"), units(0.0, 0.0, 3.0, 1.0));
}

// Not one of Compose's: a combining mark last takes no space, after the character it marks.
#[test]
fn a_region_ending_on_a_combining_mark_ends_with_its_base() {
    assert_eq!(all_of("ae\u{301}"), units(0.0, 0.0, 2.0, 1.0));
}
