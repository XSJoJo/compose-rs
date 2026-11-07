//! Review of `shared-elements-2`: the compose-ui gap D1336 ignores
//! `testDetachingSharedElementAndReattachingInNewPositionBeforeAnimating` for, shown without
//! shared elements.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{IntSize, ModifierNode, ModifierNodeElement, NodeContext, UnitExt};

type Log = Rc<RefCell<Vec<&'static str>>>;

/// A node logging its attaches and detaches.
fn lifecycle(log: &Log) -> Modifier {
    struct Element(Log);
    struct Node(Log);
    impl ModifierNodeElement for Element {
        type Node = Node;
        fn create(&self) -> Node {
            Node(self.0.clone())
        }
        fn update(&self, node: &mut Node) {
            node.0 = self.0.clone();
        }
        fn same(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }
    impl ModifierNode for Node {
        fn on_attach(&mut self, _context: &mut NodeContext<'_>) {
            self.0.borrow_mut().push("attach");
        }
        fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
            self.0.borrow_mut().push("detach");
        }
    }
    Modifier.then_custom(Element(log.clone()))
}

// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/node/LayoutNode.kt, detach and attach (a node movable content moves leaves its owner and joins it again)
/// A layout node movable content moves to another parent detaches its modifier nodes and attaches
/// them again, as Compose's `LayoutNode` removed from one parent and inserted in another: the
/// shared node's `boundsBeforeDetached` relies on it.
#[test]
#[ignore = "review: confirms D1336's compose-ui gap: a node movable content moves keeps its \
            modifier nodes attached (no detach, no attach)"]
fn moved_content_detaches_and_attaches_its_modifier_nodes() {
    let log: Log = Rc::default();
    let in_second = mutable_state_of(false);
    let given = log.clone();
    let content = movable_content_of(move || {
        Box!(modifier = lifecycle(&given).size(20.dp()));
    });
    let second = in_second.handle();
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let moved_there = second.get();
        let (first, then) = (content.clone(), content.clone());
        Box!(modifier = Modifier.offset(0.dp(), 0.dp()), move |_| {
            if !moved_there {
                first();
            }
        });
        Box!(modifier = Modifier.offset(50.dp(), 30.dp()), move |_| {
            if moved_there {
                then();
            }
        });
    });
    ui.wait_for_idle();
    assert_eq!(*log.borrow(), ["attach"]);
    log.borrow_mut().clear();
    in_second.set(true);
    ui.wait_for_idle();
    assert_eq!(*log.borrow(), ["detach", "attach"], "moved");
}
