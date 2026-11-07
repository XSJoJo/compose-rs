//! A text context menu's builder: separators collapsed and trimmed, filters applied to items
//! only (D2031), from Compose's `TextContextMenuBuilderScopeTest`.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::text_context_menu::*;

const BUILDER_TEST: &str = "compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt";

#[derive(Debug, PartialEq)]
enum Component {
    Separator,
    Item(i32),
}

use Component::{Item, Separator};

fn assert_has_components(data: &TextContextMenuData, expected: &[Component]) {
    let actual: Vec<Component> = data
        .components
        .iter()
        .map(|component| match component {
            TextContextMenuComponent::Separator => Separator,
            TextContextMenuComponent::Item(item) => {
                Item(*item.key.downcast_ref::<i32>().expect("an int key"))
            }
        })
        .collect();
    assert_eq!(actual, expected, "{BUILDER_TEST}");
}

fn create_and_run_scope(
    filter: Option<TextContextMenuFilter>,
    scope: impl FnOnce(&mut TextContextMenuBuilderScope),
) -> TextContextMenuData {
    let mut builder = TextContextMenuBuilderScope::new();
    scope(&mut builder);
    if let Some(filter) = filter {
        builder.add_filter(filter);
    }
    builder.build()
}

fn int_item(scope: &mut TextContextMenuBuilderScope, value: i32) {
    scope.item(value, value.to_string(), None, |_| {});
}

struct FakeSession;

impl TextContextMenuSession for FakeSession {
    fn close(&self) {}
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenNoFunctionsCalled_emptyDataReturned
#[test]
fn nothing_added_builds_nothing() {
    let data = create_and_run_scope(None, |_| {});
    assert_has_components(&data, &[]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenOnlySeparators_emptyDataReturned
#[test]
fn only_separators_build_nothing() {
    let data = create_and_run_scope(None, |scope| {
        for _ in 0..3 {
            scope.separator();
        }
    });
    assert_has_components(&data, &[]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenItem_itemReturnedAndDrawableAndOnClickMatches
#[test]
fn an_item_keeps_its_icon_and_click() {
    let clicked = Rc::new(Cell::new(false));
    let icon = compose_ui::Painter::new(compose_ui::ColorPainter::new(compose_ui::Color::RED));
    let data = create_and_run_scope(None, |scope| {
        let clicked = clicked.clone();
        scope.item(0, "0", Some(icon.clone()), move |_| clicked.set(true));
    });
    assert_has_components(&data, &[Item(0)]);
    let item = data.components[0].as_item().unwrap();
    assert!(!clicked.get());
    (item.on_click)(&FakeSession);
    assert!(clicked.get());
    assert!(item.leading_icon.as_ref().is_some_and(|it| *it == icon));
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenItemsWithSeparatorsBetween_separatorsCombined
#[test]
fn separators_between_items_are_one() {
    let data = create_and_run_scope(None, |scope| {
        int_item(scope, 0);
        scope.separator();
        scope.separator();
        int_item(scope, 1);
    });
    assert_has_components(&data, &[Item(0), Separator, Item(1)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenHeadingSeparator_separatorRemoved
#[test]
fn a_leading_separator_goes() {
    let data = create_and_run_scope(None, |scope| {
        scope.separator();
        int_item(scope, 0);
    });
    assert_has_components(&data, &[Item(0)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenTrailingSeparator_separatorRemoved
#[test]
fn a_trailing_separator_goes() {
    let data = create_and_run_scope(None, |scope| {
        int_item(scope, 0);
        scope.separator();
    });
    assert_has_components(&data, &[Item(0)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenOnlyItems_itemsReturned
#[test]
fn items_alone_stay_in_order() {
    let data = create_and_run_scope(None, |scope| {
        for value in 0..3 {
            int_item(scope, value);
        }
    });
    assert_has_components(&data, &[Item(0), Item(1), Item(2)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenItemSurroundedBySeparators_separatorsTrimmed
#[test]
fn separators_around_an_item_go() {
    let data = create_and_run_scope(None, |scope| {
        scope.separator();
        int_item(scope, 0);
        scope.separator();
    });
    assert_has_components(&data, &[Item(0)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenThreeGroupsWithTwoItemsEach_separatorsCombined
#[test]
fn three_groups_of_two_items() {
    let data = create_and_run_scope(None, |scope| {
        for group in 0..3 {
            if group > 0 {
                scope.separator();
                scope.separator();
            }
            int_item(scope, group * 2);
            int_item(scope, group * 2 + 1);
        }
    });
    assert_has_components(
        &data,
        &[Item(0), Item(1), Separator, Item(2), Item(3), Separator, Item(4), Item(5)],
    );
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenExcessiveSeparators_separatorsCombined
#[test]
fn many_separators_are_one() {
    let data = create_and_run_scope(None, |scope| {
        for value in 0..3 {
            for _ in 0..10 {
                scope.separator();
            }
            int_item(scope, value);
            for _ in 0..10 {
                scope.separator();
            }
        }
    });
    assert_has_components(&data, &[Item(0), Separator, Item(1), Separator, Item(2)]);
}

fn even(component: &TextContextMenuComponent) -> bool {
    component.key().downcast_ref::<i32>().is_some_and(|value| value % 2 == 0)
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenFiltering_filtersWork
#[test]
fn a_filter_keeps_what_it_passes() {
    let data = create_and_run_scope(Some(Rc::new(even)), |scope| {
        for value in 0..4 {
            int_item(scope, value);
        }
    });
    assert_has_components(&data, &[Item(0), Item(2)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenFiltering_separatorsStillCombinedAfterFiltersApplied
#[test]
fn separators_are_combined_after_filtering() {
    let data = create_and_run_scope(Some(Rc::new(even)), |scope| {
        for value in 0..4 {
            scope.separator();
            int_item(scope, value);
            scope.separator();
        }
    });
    assert_has_components(&data, &[Item(0), Separator, Item(2)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenFiltering_cannotFilterSeparators
#[test]
fn a_filter_cannot_take_separators_out() {
    let data = create_and_run_scope(Some(Rc::new(|it| !it.is_separator())), |scope| {
        int_item(scope, 0);
        scope.separator();
        int_item(scope, 1);
    });
    assert_has_components(&data, &[Item(0), Separator, Item(1)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenFiltering_filterCannotReceiveSeparator
#[test]
fn a_filter_is_never_given_a_separator() {
    let filter: TextContextMenuFilter = Rc::new(|it| {
        assert!(!it.is_separator(), "a filter was given a separator");
        true
    });
    let data = create_and_run_scope(Some(filter), |scope| {
        for value in 0..2 {
            scope.separator();
            int_item(scope, value);
            scope.separator();
        }
    });
    assert_has_components(&data, &[Item(0), Separator, Item(1)]);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/contextmenu/builder/TextContextMenuBuilderScopeTest.kt, whenFilteringAll_emptyResult
#[test]
fn filtering_every_item_leaves_nothing() {
    let data = create_and_run_scope(Some(Rc::new(|_| false)), |scope| {
        for value in 0..4 {
            scope.separator();
            int_item(scope, value);
            scope.separator();
        }
    });
    assert_has_components(&data, &[]);
}

/// Keys compare by value and type, as Compose's `equals`; Compose's own keys are their own.
#[test]
fn keys_compare_by_value_and_type() {
    assert_eq!(TextContextMenuKey::new(1), TextContextMenuKey::new(1));
    assert_ne!(TextContextMenuKey::new(1), TextContextMenuKey::new(2));
    assert_ne!(TextContextMenuKey::new(1), TextContextMenuKey::new(1u8));
    assert!(TextContextMenuKey::new("key").is(&"key"));
    assert_eq!(TextContextMenuKeys::Cut.key(), TextContextMenuKey::new(TextContextMenuKeys::Cut));
    assert_ne!(TextContextMenuKeys::Cut.key(), TextContextMenuKeys::Copy.key());
    // A key given as a key is that key.
    let key = TextContextMenuKey::new(5);
    assert_eq!(TextContextMenuKey::new(key.clone()), key);
    assert!(key.is(&TextContextMenuKey::new(5)));
}
