//! Which `append_text_context_menu_components` and `filter_text_context_menu_components` a menu
//! collects, and in what order (D2031), from Compose's `TextContextMenuModifierTraversalTest`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::text_context_menu::modifier::collect_text_context_menu_data;
use compose_foundation::text_context_menu::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{ModifierNode, ModifierNodeElement, NodeContext, NodeHandle};

/// Where a test collects the menu's data from, as Compose's `TestTextContextMenuDataInvoker`.
#[derive(Clone, Default)]
pub(crate) struct DataInvoker(Rc<Cell<Option<NodeHandle>>>);

impl DataInvoker {
    pub(crate) fn invoke_traversal(&self) -> TextContextMenuData {
        let node = self.0.get().expect("the reader isn't attached");
        node.with_node::<ReaderNode, _>(|_, context| collect_text_context_menu_data(context))
            .expect("the tree is busy")
    }

    /// Collects the menu's data at the reader, as Compose's `testTextContextMenuDataReader`.
    pub(crate) fn reader(&self) -> Modifier {
        Modifier.then_custom(ReaderElement(self.clone()))
    }
}

struct ReaderElement(DataInvoker);

pub(crate) struct ReaderNode(DataInvoker);

impl ModifierNodeElement for ReaderElement {
    type Node = ReaderNode;
    fn create(&self) -> ReaderNode {
        ReaderNode(self.0.clone())
    }
    fn update(&self, node: &mut ReaderNode) {
        node.0 = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0.0, &other.0.0)
    }
}

impl ModifierNode for ReaderNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        self.0.0.set(Some(context.handle()));
    }
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.0.0.set(None);
    }
}

type Log = Rc<RefCell<Vec<&'static str>>>;

fn logging(log: &Log, name: &'static str) -> impl Fn(&mut TextContextMenuBuilderScope) + 'static {
    let log = log.clone();
    move |_| log.borrow_mut().push(name)
}

/// Sets the content `outer` makes around a box reading the data, then reads it once.
fn set_content_and_invoke(outer: impl Fn(Rc<dyn Fn()>) + 'static) {
    let reader = DataInvoker::default();
    let modifier = reader.reader();
    let _ui = compose_test(move || {
        let modifier = modifier.clone();
        outer(Rc::new(move || {
            Box!(modifier = modifier.clone());
        }));
    });
    reader.invoke_traversal();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuModifierTraversalTest.kt, whenNoModifiers_noInvocations
#[test]
fn without_modifiers_nothing_runs() {
    set_content_and_invoke(|reading| reading());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuModifierTraversalTest.kt, whenOneModifier_nodeTraversed
#[test]
fn one_modifier_runs() {
    let log: Log = Log::default();
    let l = log.clone();
    set_content_and_invoke(move |reading| {
        Box!(modifier = Modifier.append_text_context_menu_components(logging(&l, "node")), {
            reading()
        });
    });
    assert_eq!(*log.borrow(), ["node"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuModifierTraversalTest.kt, whenChainedModifier_nodesTraversedBottomToTop
#[test]
fn chained_modifiers_run_from_the_bottom_up() {
    let log: Log = Log::default();
    let l = log.clone();
    set_content_and_invoke(move |reading| {
        Box!(
            modifier = Modifier
                .append_text_context_menu_components(logging(&l, "outer"))
                .append_text_context_menu_components(logging(&l, "inner")),
            { reading() }
        );
    });
    assert_eq!(*log.borrow(), ["inner", "outer"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuModifierTraversalTest.kt, whenNestedModifier_nodesTraversedBottomToTop
#[test]
fn nested_modifiers_run_from_the_bottom_up() {
    let log: Log = Log::default();
    let l = log.clone();
    set_content_and_invoke(move |reading| {
        let l2 = l.clone();
        Box!(modifier = Modifier.append_text_context_menu_components(logging(&l, "outer")), {
            let reading = reading.clone();
            Box!(modifier = Modifier.append_text_context_menu_components(logging(&l2, "inner")), {
                reading()
            });
        });
    });
    assert_eq!(*log.borrow(), ["inner", "outer"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuModifierTraversalTest.kt, whenMultipleNodes_doesNotTraverseNodesNotInAncestry
#[test]
fn a_sibling_s_modifier_does_not_run() {
    let log: Log = Log::default();
    let l = log.clone();
    set_content_and_invoke(move |reading| {
        let l2 = l.clone();
        Column!(modifier = Modifier.append_text_context_menu_components(logging(&l, "outer")), {
            let reading = reading.clone();
            Box!(
                modifier = Modifier.append_text_context_menu_components(logging(&l2, "inner1")),
                { reading() }
            );
            Box!(modifier = Modifier.append_text_context_menu_components(logging(&l2, "inner2")));
        });
    });
    assert_eq!(*log.borrow(), ["inner1", "outer"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuModifierTraversalTest.kt, whenDeeperChildNodes_doesNotTraverseNodesNotInAncestry
#[test]
fn a_sibling_s_deeper_modifiers_do_not_run() {
    let log: Log = Log::default();
    let l = log.clone();
    set_content_and_invoke(move |reading| {
        let l2 = l.clone();
        Column!(modifier = Modifier.append_text_context_menu_components(logging(&l, "outer")), {
            reading();
            Box!(modifier = Modifier.append_text_context_menu_components(logging(&l2, "inner1")));
            Box!(modifier = Modifier.append_text_context_menu_components(logging(&l2, "inner2")));
        });
    });
    assert_eq!(*log.borrow(), ["outer"]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/contextmenu/modifier/TextContextMenuModifierTraversalTest.kt, whenFilters_filtersAppliedAfterAllBuilders
#[test]
fn filters_run_after_every_builder() {
    let log: Log = Log::default();
    let l = log.clone();
    set_content_and_invoke(move |reading| {
        let filter = |log: &Log, name: &'static str| {
            let log = log.clone();
            move |_: &TextContextMenuComponent| {
                log.borrow_mut().push(name);
                true
            }
        };
        Box!(
            modifier = Modifier
                .filter_text_context_menu_components(filter(&l, "outer filter"))
                .append_text_context_menu_components(logging(&l, "outer builder"))
                .append_text_context_menu_components(logging(&l, "inner builder"))
                .filter_text_context_menu_components(filter(&l, "inner filter"))
                // An item, so the filters have something to run on.
                .append_text_context_menu_components(|scope| {
                    scope.item("key", "label", None, |_| {})
                }),
            { reading() }
        );
    });
    let log = log.borrow();
    assert_eq!(log.len(), 4, "{log:?}");
    assert_eq!(&log[..2], ["inner builder", "outer builder"]);
    assert!(log[2..].contains(&"inner filter") && log[2..].contains(&"outer filter"), "{log:?}");
}
