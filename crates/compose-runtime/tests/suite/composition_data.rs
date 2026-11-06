//! Compose's `CompositionDataTests` and `CompositionInstanceTests`, adapted: the tooling view of a
//! composition's slot table (`tooling::CompositionData`, D1509).
//!
//! Compose's data tests write a slot table by hand; here a composition's content makes the groups,
//! so each test composes what the Kotlin one writes. Keys are hashes, not the integers written
//! there; source information is the composable's path, `#[composable]`'s, where Compose's is the
//! compiler's string; compose-rs gives every composable a group, so Compose's "groupless calls"
//! are non-restartable composables' groups here.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use compose_runtime::platform::NodeRef;
use compose_runtime::tooling::*;
use compose_runtime::*;

use crate::mock::*;

/// Every group of `data`, depth first, those `predicate` takes, as the Kotlin file's `findAll`.
fn find_all(
    data: &dyn CompositionData,
    predicate: &dyn Fn(&CompositionGroup) -> bool,
) -> Vec<CompositionGroup> {
    fn find_in(
        group: CompositionGroup,
        predicate: &dyn Fn(&CompositionGroup) -> bool,
        found: &mut Vec<CompositionGroup>,
    ) {
        if predicate(&group) {
            found.push(group);
        }
        for child in group.composition_groups() {
            find_in(child, predicate, found);
        }
    }
    let mut found = Vec::new();
    for group in data.composition_groups() {
        find_in(group, predicate, &mut found);
    }
    found
}

/// What `group` remembered, as `i32`s.
fn remembered_ints(group: &CompositionGroup) -> Vec<i32> {
    group.data().iter().flatten().filter_map(|value| value.downcast_ref::<i32>().copied()).collect()
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, canGetCompositionDataFromSlotTable
#[test]
fn a_composition_not_yet_composed_has_no_groups() {
    let composition = Composition::new(ViewApplier::new());
    let data = composition.composition_data();
    assert!(data.is_empty());
    assert_eq!(data.composition_groups().count(), 0);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, canIterateASlotTable
#[test]
fn groups_and_their_data_iterate_in_composition_order() {
    let mut test = composition_test();
    test.compose(|| {
        for i in 1..=5 {
            key(i * 10, || {
                for j in 1..=i {
                    remember(|| i * 100 + j);
                }
            });
        }
    });
    let mut seen = Vec::new();
    fn walk(data: &dyn CompositionData, seen: &mut Vec<i32>) {
        for group in data.composition_groups() {
            seen.extend(remembered_ints(&group));
            walk(&group, seen);
        }
    }
    walk(&test.composition().composition_data(), &mut seen);
    assert_eq!(seen, [101, 201, 202, 301, 302, 303, 401, 402, 403, 404, 501, 502, 503, 504, 505]);
    // Each value is its own keyed group's.
    let keyed =
        find_all(&test.composition().composition_data(), &|group| group.data_key().is_some());
    let per_group: Vec<Vec<i32>> = keyed.iter().map(remembered_ints).collect();
    assert_eq!(
        per_group,
        [
            vec![101],
            vec![201, 202],
            vec![301, 302, 303],
            vec![401, 402, 403, 404],
            vec![501, 502, 503, 504, 505]
        ]
    );
    // Distinct keys give distinct data keys; the same call site gives the same key.
    let data_keys: HashSet<u64> = keyed.iter().filter_map(|group| group.data_key()).collect();
    assert_eq!(data_keys.len(), 5);
    assert!(keyed.iter().all(|group| group.key() == keyed[0].key()));
}

/// Emits a node for the first letter, then the rest split in two halves, each in a group of its
/// own, as the Kotlin test's `emit`.
fn emit_letters(letters: &'static [&'static str]) {
    if letters.is_empty() {
        return;
    }
    key(letters.len(), || {
        let mid = (letters.len() - 1) / 2 + 1;
        Text(letters[0]);
        if mid > 1 {
            emit_letters(&letters[1..mid]);
        }
        if mid < letters.len() {
            emit_letters(&letters[mid..]);
        }
    });
}

const LETTERS: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, canFindNodes
#[test]
fn node_groups_give_their_nodes_in_order() {
    let mut test = composition_test();
    test.compose(|| emit_letters(&LETTERS));
    let nodes: Vec<NodeRef> =
        find_all(&test.composition().composition_data(), &|group| group.node().is_some())
            .iter()
            .filter_map(CompositionGroup::node)
            .collect();
    let texts: Vec<String> = test.with_views(|views| {
        nodes
            .iter()
            .map(|node| {
                views.tree.get(*node).expect("a live view").attribute("text").unwrap().into()
            })
            .collect()
    });
    assert_eq!(texts, LETTERS);
}

#[composable]
fn Leaf() {}

#[composable]
fn Pair(depth: u32) {
    if depth == 0 {
        Leaf();
    } else {
        Pair(depth - 1);
        Pair(depth - 1);
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, canFindSourceInfo
#[test]
fn composables_groups_give_their_source_information() {
    let mut test = composition_test();
    test.compose(|| Pair(3));
    let infos: Vec<&str> =
        find_all(&test.composition().composition_data(), &|group| group.source_info().is_some())
            .iter()
            .filter_map(CompositionGroup::source_info)
            .filter(|info| info.starts_with("suite::"))
            .collect();
    // Depth first: a pair, then its two halves, down to pairs of depth 0 and their 8 leaves.
    let mut expected = Vec::new();
    fn expect(depth: u32, expected: &mut Vec<&str>) {
        expected.push("suite::composition_data::Pair");
        if depth == 0 {
            expected.push("suite::composition_data::Leaf");
        } else {
            expect(depth - 1, expected);
            expect(depth - 1, expected);
        }
    }
    expect(3, &mut expected);
    assert_eq!(infos, expected);
    // A group no composable made has none: the content's own.
    let root = test.composition().composition_data().composition_groups().next().unwrap();
    assert_eq!(root.source_info(), None);
}

/// Ten groups under one, composed by `count`'s value.
#[composable]
fn Ten(count: MutableState<i32>) {
    for index in 0..count.get() {
        key(100 + index, || {});
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, writeDuringIterationCausesException
#[test]
fn a_pass_during_an_iteration_makes_it_panic() {
    let mut test = composition_test();
    let count = mutable_state_of(10);
    let c = count.handle();
    test.compose(move || Ten(c));
    let ten = find_all(&test.composition().composition_data(), &|group| {
        group.source_info().is_some_and(|info| info.ends_with("::Ten"))
    })[0];
    let mut groups = ten.composition_groups();
    groups.next();
    count.set(11);
    test.advance();
    let panicked = catch_unwind(AssertUnwindSafe(|| groups.next())).expect_err("it panics");
    let message = panicked.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(message.contains("modified"), "{message}");
    // A group read from before the pass panics as well.
    assert!(catch_unwind(AssertUnwindSafe(|| ten.group_size())).is_err());
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, iterationDuringWriteCausesException
#[test]
fn iterating_a_composition_in_its_own_pass_panics() {
    let tried = Rc::new(RefCell::new(None));
    let t = tried.clone();
    let composition = Composition::new(ViewApplier::new());
    composition.set_content(|| key(1, || {}));
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        composition.set_content(move || {
            key(2, || {});
            let data = current_composition_data();
            *t.borrow_mut() = Some(catch_unwind(AssertUnwindSafe(|| {
                data.composition_groups().next();
            })));
        })
    }));
    // The inner catch left the pass whole: the iteration was refused before it read anything.
    assert!(panicked.is_ok());
    let tried = tried.borrow_mut().take().expect("it tried");
    let message = tried.expect_err("it panics");
    let message = message.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(message.contains("modified"), "{message}");
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, canFindAGroupInCompositionData
#[test]
fn a_group_is_found_by_its_identity() {
    let mut test = composition_test();
    let count = mutable_state_of(10);
    let c = count.handle();
    test.compose(move || Ten(c));
    let data = test.composition().composition_data();
    let sixth = find_all(&data, &|group| group.data_key().is_some())[5];
    let identity = sixth.identity();
    assert_eq!(data.find(identity).map(|group| group.identity()), Some(identity));
    // Found after a pass that moved nothing it holds, from a new reading.
    count.set(11);
    test.advance();
    let found = test.composition().composition_data().find(identity).expect("still there");
    assert_eq!(found.identity(), identity);
    // Gone with its group.
    let key = sixth_key(&test);
    count.set(3);
    test.advance();
    assert_eq!(test.composition().composition_data().find(identity), None);
    // Groups made again in the freed places: only one with its key would be found.
    count.set(10);
    test.advance();
    let again = test.composition().composition_data().find(identity);
    assert!(again.is_none_or(|group| group.data_key() == Some(key)), "{again:?}");
}

/// The data key of the sixth keyed group.
fn sixth_key(test: &CompositionTest) -> u64 {
    let data = test.composition().composition_data();
    find_all(&data, &|group| group.data_key().is_some())[5].data_key().unwrap()
}

#[composable(non_restartable)]
fn Plain(content: impl FnOnce()) {
    content();
}

#[composable(non_restartable)]
fn Other(content: impl FnOnce()) {
    content();
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, canFindAGrouplessCallGroupInCompositionData
#[test]
fn a_non_restartable_composables_group_is_found_by_its_identity() {
    let mut test = composition_test();
    test.compose(|| {
        key(300, || {});
        Plain(|| {});
        key(500, || {});
        Other(|| {});
    });
    let data = test.composition().composition_data();
    let plain = find_all(&data, &|group| group.source_info().is_some());
    assert_eq!(plain.len(), 2);
    for group in plain {
        let identity = group.identity();
        assert_eq!(data.find(identity).map(|found| found.identity()), Some(identity));
    }
}

/// The Kotlin test's nesting of calls and groups.
fn nested_calls() {
    Plain(|| {
        Other(|| {
            key(500, || {});
            Plain(|| key(700, || {}));
            key(800, || {});
            Other(|| {});
        });
        Plain(|| {});
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, canFindANestedGrouplessCallGroup
#[test]
fn nested_composables_groups_are_found_by_their_identities() {
    let mut test = composition_test();
    test.compose(nested_calls);
    let data = test.composition().composition_data();
    let found = find_all(&data, &|group| group.source_info().is_some());
    assert_eq!(found.len(), 5);
    for group in found {
        let identity = group.identity();
        let again = data.find(identity).expect("found by its identity");
        assert_eq!(again.identity(), identity);
        assert_eq!(again, group);
    }
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionDataTests.kt, groupsCreatedCanCompareEqual
#[test]
fn groups_read_twice_are_equal_and_hash_the_same() {
    use std::hash::{BuildHasher, RandomState};
    let mut test = composition_test();
    test.compose(nested_calls);
    let data = test.composition().composition_data();
    let first = find_all(&data, &|_| true);
    let second = find_all(&data, &|_| true);
    assert_eq!(first.len(), second.len());
    let hasher = RandomState::new();
    for (first, second) in first.iter().zip(&second) {
        assert_eq!(first, second);
        assert_eq!(hasher.hash_one(first), hasher.hash_one(second));
    }
}

/// A group's size counts it and every group in it; its slots size, its slots and theirs.
#[test]
fn a_groups_sizes_count_what_it_holds() {
    let mut test = composition_test();
    test.compose(|| {
        key(1, || {
            remember(|| 1);
            key(2, || {
                remember(|| 2);
                remember(|| 3);
            });
            key(3, || {});
        });
    });
    let data = test.composition().composition_data();
    let outer = find_all(&data, &|group| remembered_ints(group) == [1])[0];
    assert_eq!(outer.group_size(), 3);
    assert_eq!(outer.slots_size(), 3);
    assert!(!outer.is_empty());
    let empty = outer.composition_groups().nth(1).expect("the third group");
    assert!(empty.is_empty());
    assert_eq!(empty.group_size(), 1);
}

// ---- CompositionInstanceTests -----------------------------------------------------------------

/// A composition under this place, made in an effect and disposed with it, as the Kotlin file's
/// `TestSubcomposition`.
#[composable]
fn TestSubcomposition(content: impl Fn() + 'static) {
    let parent = remember_composition_context();
    let current = remember_updated_state(content);
    DisposableEffect((), move |scope| {
        let subcomposition = Composition::with_parent(ViewApplier::new(), parent);
        subcomposition.set_content(move || (current.get())());
        scope.on_dispose(move || subcomposition.dispose())
    });
}

/// A table to provide, and composes `content` under it.
fn compose_inspected(content: impl Fn() + Clone + 'static) -> (CompositionTest, InspectionTables) {
    let table: InspectionTables = Rc::default();
    let mut test = composition_test();
    let t = table.clone();
    test.compose(move || {
        let content = content.clone();
        CompositionLocalProvider(LOCAL_INSPECTION_TABLES.provides(Some(t.clone())), move || {
            content()
        });
    });
    (test, table)
}

fn one_of(table: &InspectionTables) -> CompositionInstance {
    *table.borrow().iter().next().expect("a composition in the table")
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionInstanceTests.kt, canFindACompositionInstance
#[test]
fn a_subcomposition_under_an_inspection_table_is_in_it() {
    let (_test, table) = compose_inspected(|| TestSubcomposition!(|| Text("Some value")));
    assert_eq!(table.borrow().len(), 1);
    let data = one_of(&table);
    assert_eq!(data.find_composition_instance(), Some(data));
    // A group isn't an instance.
    let group = data.composition_groups().next().expect("its root group");
    assert_eq!(group.find_composition_instance(), None);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionInstanceTests.kt, canFindContextGroup
#[test]
fn an_instance_finds_its_context_group() {
    let (_test, table) = compose_inspected(|| TestSubcomposition!(|| Text("Some value")));
    let instance = one_of(&table).find_composition_instance().expect("an instance");
    assert!(instance.find_context_group().is_some());
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionInstanceTests.kt, canFindParentInstance
#[test]
fn instances_lead_to_the_root_through_their_parents() {
    let (_test, table) = compose_inspected(|| {
        TestSubcomposition!(|| TestSubcomposition!(|| TestSubcomposition!(|| Text("Some value"))))
    });
    assert_eq!(table.borrow().len(), 3);
    fn root_of(instance: CompositionInstance) -> CompositionInstance {
        instance.parent().map_or(instance, root_of)
    }
    let root = root_of(one_of(&table));
    assert!(!table.borrow().contains(&root), "the root is no subcomposition");
    for instance in table.borrow().iter() {
        assert_eq!(root_of(*instance), root);
        // Every one on the way, but the root, is in the table.
        let mut at = *instance;
        while at != root {
            assert!(table.borrow().contains(&at));
            at = at.parent().expect("a parent below the root");
        }
    }
    // The root has no context group.
    assert_eq!(root.find_context_group(), None);
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionInstanceTests.kt, canFindParentNotInFirstPosition
#[test]
fn a_context_group_after_siblings_is_found() {
    let (_test, table) = compose_inspected(|| {
        Text("Some value");
        Text("Some value");
        Text("Some value");
        TestSubcomposition!(|| Text("Some value"));
    });
    let instance = one_of(&table);
    assert!(instance.find_context_group().is_some());
}

// Compose: compose/runtime/runtime/src/nonEmulatorCommonTest/kotlin/androidx/compose/runtime/tooling/CompositionInstanceTests.kt, contextGroupIsInParent
#[test]
fn the_context_group_is_its_parents() {
    let (_test, table) = compose_inspected(|| TestSubcomposition!(|| Text("Some value")));
    let instance = one_of(&table);
    let context_group = instance.find_context_group().expect("a context group");
    let parent = instance.parent().expect("a parent");
    let identity = context_group.identity();
    assert_eq!(parent.find(identity).map(|group| group.identity()), Some(identity));
    // Among the parent's groups, by identity.
    let all = find_all(&parent, &|_| true);
    assert!(all.iter().any(|group| group.identity() == identity));
}

/// A subcomposition disposed leaves the table, as Compose's context removes its composer's data.
#[test]
fn a_disposed_subcomposition_leaves_the_table() {
    let show = mutable_state_of(true);
    let s = show.handle();
    let (mut test, table) = compose_inspected(move || {
        if s.get() {
            TestSubcomposition!(|| Text("Some value"));
        }
    });
    assert_eq!(table.borrow().len(), 1);
    show.set(false);
    test.advance();
    assert!(table.borrow().is_empty());
}
