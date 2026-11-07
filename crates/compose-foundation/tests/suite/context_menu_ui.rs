//! The dropdown context menu's look: its items, column and popup (D2036), from Compose's
//! `ContextMenuUiTest` (its pixel tests are compose-skia's `context_menu_colors.rs`).
//!
//! The rule's root is 400 by 400 at density 1, the fixed-advance measurer's characters 10 wide.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::context_menu::*;
use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::mutable_state_of;
use compose_test::*;
use compose_ui::{
    Color, Dp, IntOffset, IntSize, IntrinsicSize, LayoutCoordinates, LayoutDirection,
    PopupPositionProvider, Rect, SemanticsActions,
};

const TAG: &str = "testTag";

/// Compose's `TestColors`.
const TEST_COLORS: ContextMenuColors = ContextMenuColors {
    background_color: Color::WHITE,
    text_color: Color::BLACK,
    icon_color: Color::BLUE,
    disabled_text_color: Color::RED,
    disabled_icon_color: Color::GREEN,
};

fn long_text() -> String {
    "M ".repeat(200).trim_end().to_owned()
}

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(400, 400), content)
}

/// Compose's `testItem`.
fn test_item(scope: &mut ContextMenuScope, label: &str, modifier: Modifier, enabled: bool) {
    scope.item(label, modifier, enabled, None, || {});
}

/// Compose's `TestColumn`: the column of what `block` adds, tagged.
fn test_column(colors: ContextMenuColors, block: impl Fn(&mut ContextMenuScope) + 'static) {
    let block: ContextMenuBuilderBlock = Rc::new(block);
    ContextMenuColumnBuilder(Modifier.test_tag(TAG), colors, block);
}

/// Compose's `TestItem`.
fn test_item_ui(
    modifier: Modifier,
    label: &str,
    enabled: bool,
    leading_icon: Option<ContextMenuIcon>,
    on_click: impl Fn() + 'static,
) {
    ContextMenuItem(
        label.to_owned(),
        enabled,
        TEST_COLORS,
        modifier,
        leading_icon,
        Rc::new(on_click),
    );
}

/// As Compose's tests, emulating the column asking for the maximum intrinsic width.
fn in_intrinsic_box(content: impl Fn() + 'static) {
    Box!(modifier = Modifier.width(IntrinsicSize::Max), { content() });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_enabled_isClicked_onClickTriggers
#[test]
fn an_enabled_item_s_click_calls_it() {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let mut ui = rule(move || {
        let counted = counted.clone();
        test_item_ui(Modifier.test_tag(TAG), "Test", true, None, move || {
            counted.set(counted.get() + 1)
        });
    });
    ui.on_node_with_tag(TAG).perform_click();
    assert_eq!(clicks.get(), 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_disabled_isClicked_noActions
#[test]
fn a_disabled_item_s_click_does_nothing() {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let mut ui = rule(move || {
        let counted = counted.clone();
        test_item_ui(Modifier.test_tag(TAG), "Test", false, None, move || {
            counted.set(counted.get() + 1)
        });
    });
    ui.on_node_with_tag(TAG).perform_click();
    assert_eq!(clicks.get(), 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_withMinSize_sizeIsAsExpected
#[test]
fn a_short_item_is_the_minimum_width() {
    let mut ui =
        rule(|| in_intrinsic_box(|| test_item_ui(Modifier.test_tag(TAG), "M", true, None, || {})));
    ui.on_node_with_tag(TAG)
        .assert_height_is_equal_to(ContextMenuSpec::LIST_ITEM_HEIGHT)
        .assert_width_is_equal_to(ContextMenuSpec::CONTAINER_WIDTH_MIN);
}

fn filling_icon() -> ContextMenuIcon {
    Rc::new(|color| Box!(modifier = Modifier.background(color).fill_max_size()))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_withMaxSize_sizeIsAsExpected
#[test]
fn a_long_item_is_the_maximum_width() {
    let mut ui = rule(|| {
        in_intrinsic_box(|| {
            let label = "M".repeat(200);
            test_item_ui(Modifier.test_tag(TAG), &label, true, Some(filling_icon()), || {});
        })
    });
    ui.on_node_with_tag(TAG)
        .assert_height_is_equal_to(ContextMenuSpec::LIST_ITEM_HEIGHT)
        .assert_width_is_equal_to(ContextMenuSpec::CONTAINER_WIDTH_MAX);
}

/// An item whose icon is `icon` tagged and telling its parent's size (the box sizing it).
fn icon_test(icon: fn(Modifier)) -> (ComposeTestRule, Rc<RefCell<Option<IntSize>>>) {
    let parent: Rc<RefCell<Option<IntSize>>> = Rc::default();
    let told = parent.clone();
    let ui = rule(move || {
        let told = told.clone();
        in_intrinsic_box(move || {
            let told = told.clone();
            let leading: ContextMenuIcon = Rc::new(move |_| {
                let told = told.clone();
                icon(Modifier.test_tag(TAG).on_globally_positioned(
                    move |it: LayoutCoordinates| {
                        *told.borrow_mut() =
                            it.parent_layout_coordinates().map(|parent| parent.size());
                    },
                ));
            });
            test_item_ui(Modifier, "Item", true, Some(leading), || {});
        })
    });
    (ui, parent)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_withLeadingIconMaxSpace_iconSizeIsAsExpected
#[test]
fn a_filling_icon_is_the_icon_size() {
    let (mut ui, parent) =
        icon_test(|modifier| Box!(modifier = modifier.background(Color::BLACK).fill_max_size()));
    ui.on_node_with_tag_unmerged(TAG)
        .assert_width_is_equal_to(ContextMenuSpec::ICON_SIZE)
        .assert_height_is_equal_to(ContextMenuSpec::ICON_SIZE);
    let icon = ContextMenuSpec::ICON_SIZE.0 as i32;
    assert_eq!(*parent.borrow(), Some(IntSize::new(icon, icon)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_withLeadingIconMinSize_iconSizeIsAsExpected
#[test]
fn an_empty_icon_keeps_the_icon_width() {
    let (mut ui, parent) = icon_test(|modifier| Spacer!(modifier = modifier));
    ui.on_node_with_tag_unmerged(TAG)
        .assert_width_is_equal_to(Dp::ZERO)
        .assert_height_is_equal_to(Dp::ZERO);
    let icon = ContextMenuSpec::ICON_SIZE.0 as i32;
    assert_eq!(*parent.borrow(), Some(IntSize::new(icon, 0)));
}

fn assert_click_semantics(enabled: bool) {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let mut ui = rule(move || {
        let counted = counted.clone();
        test_item_ui(Modifier.test_tag(TAG), "Test", enabled, None, move || {
            counted.set(counted.get() + 1)
        });
    });
    let node = ui.on_node_with_tag(TAG).assert_has_click_action().fetch_semantics_node();
    let config = node.config();
    let action = config.get(&SemanticsActions::ON_CLICK).expect("a click action");
    assert_eq!(action.label.as_deref(), Some("Test"));
    assert_eq!(clicks.get(), 0);
    (action.action.as_ref().expect("an action"))();
    assert_eq!(clicks.get(), if enabled { 1 } else { 0 });
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_enabled_semanticsIsCorrect
#[test]
fn an_enabled_item_s_semantics_click_with_its_label() {
    assert_click_semantics(true);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_disabled_semanticsIsCorrect
#[test]
fn a_disabled_item_s_semantics_click_does_nothing() {
    assert_click_semantics(false);
}

/// The text layout of the text `text`, as `fetchTextLayoutResult` on the unmerged tree.
fn layout_of(ui: &mut ComposeTestRule, text: &str) -> compose_ui::TextLayoutResult {
    let mut results = Vec::new();
    ui.on_node_unmerged(has_text(text)).assert_exists().perform_semantics_action_with(
        &SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    results.pop().expect("a layout")
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_withLongLabel_doesNotWrap
#[test]
fn a_long_label_doesnt_wrap() {
    let mut ui = rule(|| test_item_ui(Modifier.test_tag(TAG), &long_text(), true, None, || {}));
    let layout = layout_of(&mut ui, &long_text());
    assert_eq!(layout.line_count, 1);
}

fn assert_text_style(enabled: bool, color: Color) {
    let mut ui =
        rule(move || test_item_ui(Modifier.test_tag(TAG), &long_text(), enabled, None, || {}));
    let style = layout_of(&mut ui, &long_text()).layout_input.style;
    assert_eq!(style.color, color);
    assert_eq!(style.text_align, Some(ContextMenuSpec::LABEL_HORIZONTAL_TEXT_ALIGNMENT));
    assert_eq!(style.font_size, ContextMenuSpec::FONT_SIZE);
    assert_eq!(style.font_weight, Some(ContextMenuSpec::FONT_WEIGHT));
    assert_eq!(style.line_height, ContextMenuSpec::LINE_HEIGHT);
    assert_eq!(style.letter_spacing, ContextMenuSpec::LETTER_SPACING);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_enabled_correctTextStyling
#[test]
fn an_enabled_item_s_text_style() {
    assert_text_style(true, TEST_COLORS.text_color);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_disabled_correctTextStyling
#[test]
fn a_disabled_item_s_text_style() {
    assert_text_style(false, TEST_COLORS.disabled_text_color);
}

fn icon_color(enabled: bool) -> Option<Color> {
    let told: Rc<Cell<Option<Color>>> = Rc::default();
    let into = told.clone();
    let _ui = rule(move || {
        let into = into.clone();
        let icon: ContextMenuIcon = Rc::new(move |color| into.set(Some(color)));
        test_item_ui(Modifier.test_tag(TAG), &long_text(), enabled, Some(icon), || {});
    });
    told.get()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_enabled_correctIconColor
#[test]
fn an_enabled_item_s_icon_color() {
    assert_eq!(icon_color(true), Some(TEST_COLORS.icon_color));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuItem_disabled_correctIconColor
#[test]
fn a_disabled_item_s_icon_color() {
    assert_eq!(icon_color(false), Some(TEST_COLORS.disabled_icon_color));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColumn_usedNormally_allInputItemsAreRendered
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColumn_allInputItemsAreRendered
/// Compose's two tests are the same.
#[test]
fn a_column_shows_all_its_items() {
    let mut ui = rule(|| {
        test_column(TEST_COLORS, |scope| {
            for index in 0..5 {
                test_item(scope, &format!("Item {index}"), Modifier, true);
            }
        })
    });
    ui.on_node_with_tag(TAG).assert_is_displayed();
    for index in 0..5 {
        ui.on_node(has_text(&format!("Item {index}"))).assert_exists();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColumn_5ItemsMaxWidth_sizeIsAsExpected
#[test]
fn a_column_of_long_items_is_the_maximum_width() {
    let mut ui = rule(|| {
        test_column(TEST_COLORS, |scope| {
            for index in 0..5 {
                test_item(
                    scope,
                    &format!("Item {}", index.to_string().repeat(100)),
                    Modifier,
                    true,
                );
            }
        })
    });
    let height =
        ContextMenuSpec::LIST_ITEM_HEIGHT.0 * 5.0 + ContextMenuSpec::VERTICAL_PADDING.0 * 2.0;
    ui.on_node_with_tag(TAG)
        .assert_height_is_equal_to(Dp(height))
        .assert_width_is_equal_to(ContextMenuSpec::CONTAINER_WIDTH_MAX);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColumn_1ItemMinWidth_sizeIsAsExpected
#[test]
fn a_column_of_a_short_item_is_the_minimum_width() {
    let mut ui =
        rule(|| test_column(TEST_COLORS, |scope| test_item(scope, "Item", Modifier, true)));
    let height = ContextMenuSpec::LIST_ITEM_HEIGHT.0 + ContextMenuSpec::VERTICAL_PADDING.0 * 2.0;
    ui.on_node_with_tag(TAG)
        .assert_height_is_equal_to(Dp(height))
        .assert_width_is_equal_to(ContextMenuSpec::CONTAINER_WIDTH_MIN);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColumn_with100Items_scrolls
#[test]
fn a_column_of_a_hundred_items_scrolls() {
    let mut ui = rule(|| {
        test_column(TEST_COLORS, |scope| {
            for index in 0..100 {
                test_item(scope, &format!("Item {}", index + 1), Modifier, true);
            }
        })
    });
    ui.on_node(has_text("Item 1")).assert_is_displayed();
    ui.on_node(has_text("Item 100")).assert_is_not_displayed();
    // Compose's `performScrollToNode` on the column: compose-test scrolls to the node itself.
    ui.on_node(has_text("Item 100")).perform_scroll_to();
    ui.on_node(has_text("Item 1")).assert_is_not_displayed();
    ui.on_node(has_text("Item 100")).assert_is_displayed();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuColumn_withItemsOfDifferentWidths_clickBoxCoversEndOfItems
#[test]
fn every_item_is_clickable_to_the_column_s_end() {
    let clicks: Rc<RefCell<[u32; 5]>> = Rc::default();
    let counted = clicks.clone();
    let mut ui = rule(move || {
        let counted = counted.clone();
        test_column(TEST_COLORS, move |scope| {
            for index in 0..5 {
                let suffix = "Item ".repeat(index).trim_end().to_owned();
                let counted = counted.clone();
                scope.item(format!("Item {suffix}"), Modifier, true, None, move || {
                    counted.borrow_mut()[index] += 1;
                });
            }
        })
    });
    let height = ContextMenuSpec::LIST_ITEM_HEIGHT.0;
    ui.on_node_with_tag(TAG).perform_touch_input(|touch| {
        let x = touch.right() - 1.0;
        for index in 0..5 {
            let y = height * index as f32 + height * 0.5;
            touch.click_at(compose_ui::Offset::new(x, y));
        }
    });
    assert_eq!(*clicks.borrow(), [1; 5], "Each item should have been clicked once.");
}

/// Compose's `centeringPopupPositionProvider`.
struct Centering;

impl PopupPositionProvider for Centering {
    fn position(
        &self,
        _anchor: Rect,
        window: IntSize,
        _direction: LayoutDirection,
        popup: IntSize,
    ) -> IntOffset {
        IntOffset::new(window.width / 2 - popup.width / 2, window.height / 2 - popup.height / 2)
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, whenContextMenuPopup_removedAndReAdded_appearsAsExpected
#[test]
fn a_popup_removed_and_added_again_shows_again() {
    let show = &*Box::leak(Box::new(mutable_state_of(false)));
    let show = **show;
    let mut ui = rule(move || {
        if show.get() {
            let block: ContextMenuBuilderBlock = Rc::new(|scope: &mut ContextMenuScope| {
                test_item(scope, "Item", Modifier.test_tag("itemTag"), true);
            });
            ContextMenuPopup(
                Rc::new(Centering),
                compose_runtime::callback(|| {}),
                Modifier.test_tag(TAG),
                compute_context_menu_colors(),
                block,
            );
        }
    });
    for _ in 0..2 {
        show.set(false);
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_does_not_exist();
        ui.on_node_with_tag("itemTag").assert_does_not_exist();
        show.set(true);
        ui.wait_for_idle();
        ui.on_node_with_tag(TAG).assert_exists();
        ui.on_node_with_tag("itemTag").assert_exists();
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/contextmenu/ContextMenuUiTest.kt, computeContextMenuColors_defaultsAsExpected
/// compose-rs reads no platform style: the colors are always the defaults (D2036), the enabled
/// text's color the icons' too, the disabled text's the disabled icons'.
#[test]
fn the_computed_colors_are_the_defaults() {
    let colors = compute_context_menu_colors();
    assert_eq!(colors.background_color, DEFAULT_CONTEXT_MENU_COLORS.background_color);
    assert_eq!(colors.text_color, DEFAULT_CONTEXT_MENU_COLORS.text_color);
    assert_eq!(colors.icon_color, DEFAULT_CONTEXT_MENU_COLORS.text_color);
    assert_eq!(colors.disabled_text_color, DEFAULT_CONTEXT_MENU_COLORS.disabled_text_color);
    assert_eq!(colors.disabled_icon_color, DEFAULT_CONTEXT_MENU_COLORS.disabled_text_color);
}
