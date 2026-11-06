//! `register_on_global_layout_listener` and `RelativeLayoutBounds::calculate_occlusions`: a node
//! told of any change of the layout, and the layouts that may draw over it (siblings placed after
//! it or above it, never its ancestors or descendants). Adapted from Compose's
//! `OnGlobalLayoutListenerTest`, at density 1. Compose's test reads its rect list; these ask the
//! target's `calculate_occlusions`, which reads it as it is, and check the occluding nodes' bounds.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Constraints, IntOffset, IntRect, IntSize, Layout, ModifierNode, ModifierNodeElement,
    NodeContext, RegistrationHandle, RelativeLayoutBounds, UnitExt, measure_policy,
};

const TARGET: &str = "target";
const OCCLUDING: &str = "occluding";

type Bounds = Rc<Cell<Option<RelativeLayoutBounds>>>;

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(1000, 1000), content)
}

/// The test's target: tagged, its bounds kept to ask its occlusions.
fn target(bounds: &Bounds) -> Modifier {
    let bounds = bounds.clone();
    Modifier
        .test_tag(TARGET)
        .on_layout_rect_changed(0, 0, move |it: RelativeLayoutBounds| bounds.set(Some(it)))
}

/// Compose's `PlaceTwoLayoutsApartVert`: the first child at the top, the second at the bottom.
fn place_two_apart(modifier: Modifier, content: impl Fn() + 'static) {
    Layout!(
        modifier = modifier,
        measure_policy = measure_policy(|scope, measurables, constraints| {
            assert!(constraints.min_height == constraints.max_height, "a fixed height");
            assert_eq!(measurables.len(), 2);
            let first = scope.measure(measurables[0], Constraints::unbounded());
            let second = scope.measure(measurables[1], Constraints::unbounded());
            let wrap = first.width.max(second.width);
            let width = if constraints.min_width == constraints.max_width {
                constraints.max_width
            } else {
                wrap.min(constraints.max_width)
            };
            scope.place(&first, 0, 0);
            scope.place(&second, 0, constraints.max_height - second.height);
            scope.layout(width, constraints.max_height)
        }),
        { content() }
    );
}

/// The occlusions of the target, as they are now.
fn occlusions(bounds: &Bounds) -> Vec<IntRect> {
    bounds.get().expect("the target is laid out").calculate_occlusions()
}

fn assert_no_occlusions(bounds: &Bounds) {
    assert_eq!(occlusions(bounds), Vec::<IntRect>::new());
}

/// `expected` occlusions, each the bounds of a node tagged as occluding, and every such node one.
fn assert_occlusions(ui: &mut ComposeTestRule, bounds: &Bounds, expected: usize) {
    let found = occlusions(bounds);
    assert_eq!(found.len(), expected, "{found:?}");
    let tagged = ui.on_all_nodes_with_tag(OCCLUDING).fetch_semantics_nodes();
    assert_eq!(tagged.len(), expected);
    for node in tagged {
        let rect = node.unclipped_bounds_in_root();
        let rect =
            IntRect::new(rect.left as i32, rect.top as i32, rect.right as i32, rect.bottom as i32);
        assert!(found.contains(&rect), "{rect:?} isn't among {found:?}");
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, notOccludingSiblings_whenNotOverlapping
#[test]
fn siblings_apart_don_t_occlude() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let _ui = rule(move || {
        place_two_apart(Modifier.size(300.dp()), {
            let b = b.clone();
            move || {
                Box!(modifier = Modifier.size(100.dp()).then(target(&b)));
                Box!(modifier = Modifier.size(100.dp()).test_tag(OCCLUDING));
            }
        });
    });
    assert_no_occlusions(&bounds);
}

/// Rects whose edges touch intersect, as the rect list's `rectIntersectsRect`.
// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/spatial/RectList.kt, rectIntersectsRect
#[test]
fn a_sibling_touching_the_target_s_edge_occludes() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let mut ui = rule(move || {
        let b = b.clone();
        Column!(|_| {
            Box!(modifier = Modifier.size(100.dp()).then(target(&b)));
            Box!(modifier = Modifier.size(100.dp()).test_tag(OCCLUDING));
        });
    });
    assert_occlusions(&mut ui, &bounds, 1);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, notOccludingSiblings_whenPlacedBeforeWithHigherZIndex
#[test]
fn a_sibling_placed_after_but_below_doesn_t_occlude() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let _ui = rule(move || {
        place_two_apart(Modifier.size(150.dp()), {
            let b = b.clone();
            move || {
                Box!(modifier = Modifier.size(100.dp()).z_index(1.0).then(target(&b)));
                Box!(modifier = Modifier.size(100.dp()).z_index(0.0).test_tag(OCCLUDING));
            }
        });
    });
    assert_no_occlusions(&bounds);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, notOccludingSiblings_whenPlacedAfter
#[test]
fn a_sibling_placed_before_doesn_t_occlude() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let _ui = rule(move || {
        let b = b.clone();
        Box!(modifier = Modifier.size(150.dp()), {
            Box!(modifier = Modifier.size(100.dp()).test_tag(OCCLUDING));
            Box!(modifier = Modifier.size(100.dp()).then(target(&b)));
        });
    });
    assert_no_occlusions(&bounds);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, notOccludingByChildrenAndGrandChildren
#[test]
fn children_and_grandchildren_don_t_occlude() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let _ui = rule(move || {
        Box!(modifier = target(&b), {
            Row!(|_| {
                for _ in 0..3 {
                    Box!(modifier = Modifier.size(100.dp()));
                }
            });
            Column!(|_| {
                for _ in 0..3 {
                    Box!(modifier = Modifier.size(100.dp()));
                }
            });
        });
    });
    assert_no_occlusions(&bounds);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, notOccluding_whenBranchingParentIsPlacedBeforeWithHigherZIndex
#[test]
fn a_branch_above_its_sibling_isn_t_occluded_by_it() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let _ui = rule(move || {
        place_two_apart(Modifier.size(150.dp()), {
            let b = b.clone();
            move || {
                let b = b.clone();
                Column!(modifier = Modifier.z_index(1.0), |_| {
                    Box!(modifier = Modifier.size(100.dp()).then(target(&b)));
                });
                Column!(modifier = Modifier.z_index(0.0).test_tag(OCCLUDING), |_| {
                    Box!(modifier = Modifier.size(100.dp()).test_tag(OCCLUDING));
                });
            }
        });
    });
    assert_no_occlusions(&bounds);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, occludingSiblings_whenPlacedBefore
#[test]
fn a_sibling_placed_after_occludes() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let mut ui = rule(move || {
        place_two_apart(Modifier.size(150.dp()), {
            let b = b.clone();
            move || {
                Box!(modifier = Modifier.size(100.dp()).then(target(&b)));
                Box!(modifier = Modifier.size(100.dp()).test_tag(OCCLUDING));
            }
        });
    });
    assert_occlusions(&mut ui, &bounds, 1);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, occludingSiblings_whenPlacedAfterWithLowerZIndex
#[test]
fn a_sibling_placed_before_but_above_occludes() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let mut ui = rule(move || {
        place_two_apart(Modifier.size(150.dp()), {
            let b = b.clone();
            move || {
                Box!(modifier = Modifier.size(100.dp()).z_index(1.0).test_tag(OCCLUDING));
                Box!(modifier = Modifier.size(100.dp()).z_index(0.0).then(target(&b)));
            }
        });
    });
    assert_occlusions(&mut ui, &bounds, 1);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, occludingSiblings_afterRotatingTargetOnZAxis
#[test]
fn a_rotated_branch_is_occluded_by_what_its_box_meets() {
    let bounds = Bounds::default();
    let degrees = mutable_state_of(0.0f32);
    let (b, d) = (bounds.clone(), degrees.handle());
    let mut ui = rule(move || {
        let b = b.clone();
        Column!(
            modifier = Modifier.size(200.dp()),
            vertical_arrangement = Arrangement::SPACE_BETWEEN,
            |_| {
                let b = b.clone();
                Box!(
                    modifier = Modifier.graphics_layer(move |layer| {
                        layer.transform_origin = compose_ui::TransformOrigin {
                            pivot_fraction_x: 0.0,
                            pivot_fraction_y: 1.0,
                        };
                        layer.rotation_z = d.get();
                    }),
                    {
                        Box!(modifier = Modifier.fill_max_width().height(90.dp()).then(target(&b)));
                    }
                );
                Box!(modifier = Modifier.fill_max_width().height(90.dp()).test_tag(OCCLUDING));
            }
        );
    });
    assert_no_occlusions(&bounds);
    degrees.set(90.0);
    ui.wait_for_idle();
    assert_occlusions(&mut ui, &bounds, 1);
}

/// Compose's test has Material's `Scaffold` with a top bar; here, the top bar placed after the
/// content, over it, as `Scaffold` places it.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, occludingTopBar_whenTargetIsScrolled
#[test]
fn a_target_scrolled_under_a_top_bar_is_occluded_by_it() {
    let bounds = Bounds::default();
    let scroll = ScrollState::new(0);
    let (b, s) = (bounds.clone(), scroll.handle());
    let mut ui = rule(move || {
        let b = b.clone();
        Box!({
            let b = b.clone();
            Column!(
                modifier = Modifier
                    .padding(PaddingValues::new(0.dp(), 51.dp(), 0.dp(), 0.dp()))
                    .size(50.dp())
                    .vertical_scroll(s),
                vertical_arrangement = Arrangement::spaced_by(2.dp()),
                |_| {
                    Box!(modifier = Modifier.size(50.dp()).then(target(&b)));
                    Box!(modifier = Modifier.size(50.dp()));
                }
            );
            Row!(
                modifier = Modifier.height(49.dp()).test_tag(OCCLUDING),
                horizontal_arrangement = Arrangement::SPACE_BETWEEN,
                |_| {
                    Box!(modifier = Modifier.fill_max_height().width(52.dp()).test_tag(OCCLUDING));
                    // Not over the target: it's 50 wide.
                    Box!(modifier = Modifier.fill_max_height().width(52.dp()));
                }
            );
        });
    });
    assert_no_occlusions(&bounds);
    let _ = crate::scroll_support::run(&mut ui, async move { s.scroll_by(50.0).await });
    ui.wait_for_idle();
    assert_occlusions(&mut ui, &bounds, 2);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, occluding_whenBranchingParentIsPlacedAfterWithLowerZIndex
#[test]
fn a_branch_above_the_target_s_occludes_it_and_its_children_too() {
    let bounds = Bounds::default();
    let b = bounds.clone();
    let mut ui = rule(move || {
        place_two_apart(Modifier.size(150.dp()), {
            let b = b.clone();
            move || {
                Column!(modifier = Modifier.z_index(1.0).test_tag(OCCLUDING), |_| {
                    Box!(modifier = Modifier.size(100.dp()).test_tag(OCCLUDING));
                });
                let b = b.clone();
                Column!(modifier = Modifier.z_index(0.0), |_| {
                    Box!(modifier = Modifier.size(100.dp()).then(target(&b)));
                });
            }
        });
    });
    assert_occlusions(&mut ui, &bounds, 2);
}

/// The test's `testGlobalLayoutListener`: a node registering a global layout listener (0, 0).
fn global_listener(callback: impl Fn(RelativeLayoutBounds) + 'static) -> Modifier {
    struct Element(Callback<dyn Fn(RelativeLayoutBounds)>);
    struct Node {
        callback: Callback<dyn Fn(RelativeLayoutBounds)>,
        handle: Option<RegistrationHandle>,
    }
    impl ModifierNodeElement for Element {
        type Node = Node;
        fn create(&self) -> Node {
            Node { callback: self.0.clone(), handle: None }
        }
        fn update(&self, node: &mut Node) {
            node.callback = self.0.clone();
        }
        fn same(&self, other: &Self) -> bool {
            self.0.ptr_eq(&other.0)
        }
    }
    impl ModifierNode for Node {
        fn on_attach(&mut self, context: &mut NodeContext<'_>) {
            if let Some(handle) = self.handle.take() {
                handle.unregister();
            }
            self.handle =
                Some(context.register_on_global_layout_listener(0, 0, self.callback.clone()));
        }
        fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
            if let Some(handle) = self.handle.take() {
                handle.unregister();
            }
        }
    }
    Modifier.then_custom(Element(compose_runtime::callback(callback)))
}

/// What a global listener heard last: its width and its occlusions' count.
type Heard = Rc<RefCell<Vec<(i32, usize)>>>;

fn hearing(heard: &Heard) -> Modifier {
    let heard = heard.clone();
    global_listener(move |bounds| {
        heard.borrow_mut().push((bounds.width(), bounds.calculate_occlusions().len()))
    })
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, globalLayoutListenerCallback_whenTargetChangesIntoOcclusion
#[test]
fn a_global_listener_hears_its_node_grow_into_an_occlusion() {
    let heard = Heard::default();
    let multiplier = mutable_state_of(1.0f32);
    let (h, m) = (heard.clone(), multiplier.handle());
    let mut ui = rule(move || {
        place_two_apart(Modifier.size(300.dp()), {
            let h = h.clone();
            move || {
                Box!(modifier = hearing(&h).size((100.0 * m.get()).dp()));
                Box!(modifier = Modifier.size(100.dp()));
            }
        });
    });
    assert_eq!(heard.borrow().last(), Some(&(100, 0)));
    multiplier.set(3.0);
    ui.wait_for_idle();
    assert_eq!(heard.borrow().last(), Some(&(300, 1)));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, globalLayoutListener_whenSiblingChangesIntoOcclusion
#[test]
fn a_global_listener_hears_a_sibling_grow_over_it() {
    let heard = Heard::default();
    let multiplier = mutable_state_of(1.0f32);
    let (h, m) = (heard.clone(), multiplier.handle());
    let mut ui = rule(move || {
        place_two_apart(Modifier.size(300.dp()), {
            let h = h.clone();
            move || {
                Box!(modifier = Modifier.size(100.dp()).then(hearing(&h)));
                Box!(modifier = Modifier.size((100.0 * m.get()).dp()));
            }
        });
    });
    assert_eq!(heard.borrow().last().map(|it| it.1), Some(0));
    let calls = heard.borrow().len();
    // Its own rect stays: a global listener hears any change.
    multiplier.set(3.0);
    ui.wait_for_idle();
    assert!(heard.borrow().len() > calls);
    assert_eq!(heard.borrow().last().map(|it| it.1), Some(1));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, globalLayoutListener_whenParentChangesIntoOcclusion
#[test]
fn a_global_listener_hears_its_parent_shrink_into_an_occlusion() {
    let heard = Heard::default();
    let multiplier = mutable_state_of(1.0f32);
    let (h, m) = (heard.clone(), multiplier.handle());
    let mut ui = rule(move || {
        place_two_apart(Modifier.size((300.0 * m.get()).dp()), {
            let h = h.clone();
            move || {
                Box!(modifier = Modifier.size(100.dp()).then(hearing(&h)));
                Box!(modifier = Modifier.size(100.dp()));
            }
        });
    });
    assert_eq!(heard.borrow().last().map(|it| it.1), Some(0));
    multiplier.set(1.0 / 3.0);
    ui.wait_for_idle();
    assert_eq!(heard.borrow().last().map(|it| it.1), Some(1));
}

/// A layout that moves and resizes nothing doesn't call a global listener: Compose calls them
/// when a rect changed (`isDirty`) or the window moved.
// Compose: compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/spatial/RectManager.kt, dispatchCallbacks
#[test]
fn a_layout_moving_nothing_doesn_t_call_a_global_listener() {
    let heard = Heard::default();
    let tick = mutable_state_of(0);
    let size = mutable_state_of(100.0f32);
    let (h, t, z) = (heard.clone(), tick.handle(), size.handle());
    let mut ui = rule(move || {
        let h = h.clone();
        Column!(|_| {
            Box!(modifier = Modifier.size(100.dp()).then(hearing(&h)));
            // Measured again when `tick` changes, at the same size.
            Layout!(
                measure_policy = measure_policy(move |scope, _, _| {
                    let _ = t.get();
                    scope.layout(10, 10)
                }),
                {}
            );
            Box!(modifier = Modifier.size(z.get().dp()));
        });
    });
    let calls = heard.borrow().len();
    assert_eq!(calls, 1);
    tick.set(1);
    ui.wait_for_idle();
    assert_eq!(heard.borrow().len(), calls);
    // A node elsewhere resized: heard.
    size.set(50.0);
    ui.wait_for_idle();
    assert_eq!(heard.borrow().len(), calls + 1);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/layout/OnGlobalLayoutListenerTest.kt, globalLayoutListener_updatedOffset
#[test]
fn a_global_listener_hears_its_new_place_and_placed_size() {
    let told: Rc<Cell<Option<(IntOffset, IntSize)>>> = Rc::default();
    let (first, second) = (mutable_state_of(1.0f32), mutable_state_of(1.0f32));
    let (t, f, s) = (told.clone(), first.handle(), second.handle());
    let mut ui = rule(move || {
        let t = t.clone();
        Column!(modifier = Modifier.size(400.dp()), |_| {
            Box!(modifier = Modifier.size((100.0 * f.get()).dp()));
            let t = t.clone();
            Box!(
                modifier =
                    Modifier.size((100.0 * s.get()).dp()).then(global_listener(move |bounds| {
                        t.set(Some((
                            bounds.position_in_root(),
                            IntSize::new(bounds.width(), bounds.height()),
                        )))
                    }))
            );
        });
    });
    assert_eq!(told.get(), Some((IntOffset::new(0, 100), IntSize::new(100, 100))));
    first.set(2.0);
    second.set(3.0);
    ui.wait_for_idle();
    // Its height as placed: what's left of the column.
    assert_eq!(told.get(), Some((IntOffset::new(0, 200), IntSize::new(300, 200))));
}
