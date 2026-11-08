//! Text selection's magnifier, as Compose's `SelectionMagnifier.kt` (`animatedSelectionMagnifier`)
//! and the field's `TextFieldMagnifierNodeImpl28`: a node delegating to a `MagnifierNode` with the
//! platform's text defaults (`use_text_default`), over the center a selection drag gives,
//! following the drag across a line at once and moving between lines with a spring (D2073).
//!
//! What computes the center (a `SelectionContainer`'s tree, a field's node) writes it to a state
//! its [`SelectionMagnifierSource`] reads, unspecified while no drag shows it. The node is on a
//! container or a field only where the platform has a magnifier; its platform magnifier is made
//! at the first drag.

use std::cell::Cell;
use std::rc::Rc;

use compose_animation::Animatable;
use compose_ui::{
    Delegates, Density, DpSize, IntSize, Modifier, ModifierNode, ModifierNodeElement, NodeContext,
    Offset,
};

use crate::magnifier::{MagnifierArgs, MagnifierNode};

/// What a selection's magnifier follows: where it's centered, in the coordinates of the node it's
/// on (a read of a state; unspecified hides it), and where its size is told (what hides it when
/// dragged far off the line).
pub(crate) trait SelectionMagnifierSource {
    fn magnifier_center(&self) -> Offset;
    fn set_magnifier_size(&self, size: IntSize);
}

impl SelectionMagnifierSource for compose_ui::ContainerHandles {
    fn magnifier_center(&self) -> Offset {
        compose_ui::ContainerHandles::magnifier_center(self)
    }
    fn set_magnifier_size(&self, size: IntSize) {
        compose_ui::ContainerHandles::set_magnifier_size(self, size);
    }
}

/// An offset compared as Compose's: unspecified equal to unspecified, so a hidden magnifier's
/// center isn't followed again.
#[derive(Clone, Copy)]
struct Center(Offset);

impl PartialEq for Center {
    fn eq(&self, other: &Self) -> bool {
        (self.0.is_unspecified() && other.0.is_unspecified()) || self.0 == other.0
    }
}

/// Compose's `OffsetDisplacementThreshold`: the spring's `DefaultDisplacementThreshold` on each
/// axis.
const THRESHOLD: f32 = 0.01;

/// The magnifier of `source`, as Compose's `animatedSelectionMagnifier` over a `MagnifierNode`
/// with the text defaults: its center follows `source`'s, snapping to it within a line and
/// animated with Compose's `MagnifierSpringSpec` when it moves to another line (a change of `y`).
pub(crate) fn selection_magnifier(source: Rc<dyn SelectionMagnifierSource>) -> Modifier {
    Modifier.then_custom(SelectionMagnifierElement(source))
}

struct SelectionMagnifierElement(Rc<dyn SelectionMagnifierSource>);

impl ModifierNodeElement for SelectionMagnifierElement {
    type Node = SelectionMagnifierNode;
    fn create(&self) -> SelectionMagnifierNode {
        SelectionMagnifierNode::new(self.0.clone())
    }
    fn update(&self, node: &mut SelectionMagnifierNode) {
        // The same source while its container or field is composed.
        node.source = self.0.clone();
    }
    fn same(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.0), Rc::as_ptr(&other.0))
    }
    fn name(&self) -> &'static str {
        "selectionMagnifier"
    }
}

/// As Compose's `AnimatedSelectionMagnifierNode`: a task follows the source's center (its
/// `snapshotFlow`), the magnifier delegated to reads the animated value.
struct SelectionMagnifierNode {
    source: Rc<dyn SelectionMagnifierSource>,
    animatable: Animatable<Offset>,
    /// Its layout node's density: what the magnifier's size is told in pixels at.
    density: Rc<Cell<Density>>,
    delegates: Delegates,
}

impl SelectionMagnifierNode {
    fn new(source: Rc<dyn SelectionMagnifierSource>) -> SelectionMagnifierNode {
        let initial = compose_runtime::snapshots::Snapshot::without_read_observation(|| {
            source.magnifier_center()
        });
        let animatable =
            Animatable::new_with(initial, Some(Offset::new(THRESHOLD, THRESHOLD)), "magnifier");
        let mut delegates = Delegates::new();
        let density = Rc::new(Cell::new(Density::new(1.0)));
        let (center, told, at) = (animatable.clone(), source.clone(), density.clone());
        delegates.delegate(MagnifierNode::new(MagnifierArgs {
            // Its pixels, as Compose's `roundToPx` of the size told.
            on_size_changed: Some(compose_runtime::IntoCallback::into_callback(
                move |size: DpSize| {
                    let density = at.get();
                    told.set_magnifier_size(IntSize::new(
                        density.round_to_px(size.width),
                        density.round_to_px(size.height),
                    ));
                },
            )),
            use_text_default: true,
            ..MagnifierArgs::new(move |_: Density| center.value())
        }));
        SelectionMagnifierNode { source, animatable, density, delegates }
    }
}

impl ModifierNode for SelectionMagnifierNode {
    fn delegates(&mut self) -> Option<&mut Delegates> {
        Some(&mut self.delegates)
    }
    fn on_density_change(&mut self, context: &mut NodeContext<'_>) {
        self.density.set(context.require_density());
    }
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        self.density.set(context.require_density());
        let scope = context.coroutine_scope();
        let (source, animated) = (self.source.clone(), self.animatable.clone());
        scope.launch(async move {
            let mut targets =
                compose_runtime::snapshot_flow(move || Center(source.magnifier_center()));
            // The animations under way: a snap ends them, waiting for no frame.
            let mut running: Vec<compose_runtime::Job> = Vec::new();
            while let Some(Center(target)) = compose_runtime::flow::next(&mut targets).await {
                let value = animated.value();
                running.retain(compose_runtime::Job::is_active);
                // Animated only between lines: across a line it stays with the finger.
                if value.is_specified() && target.is_specified() && value.y != target.y {
                    let animated = animated.clone();
                    // Launched, not awaited: the next target interrupts it, keeping its velocity.
                    running.push(scope.launch(async move {
                        let spec = compose_animation::spring()
                            .visibility_threshold(Offset::new(THRESHOLD, THRESHOLD));
                        animated.animate_to(target, spec).await;
                    }));
                } else {
                    animated.snap_to(target);
                    for job in running.drain(..) {
                        job.cancel();
                    }
                }
            }
        });
    }
}
