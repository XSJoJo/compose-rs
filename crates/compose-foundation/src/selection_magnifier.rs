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
    Delegate, Delegates, Density, DpSize, IntSize, Modifier, ModifierNode, ModifierNodeElement,
    NodeContext, Offset,
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
/// `snapshotFlow`, while a gesture shows the magnifier), and the magnifier delegated to reads the
/// animated value. The magnifier is delegated to only while the center is specified (Compose's
/// is always): between gestures the node observes one state, and draws nothing, says nothing and
/// is told no position (D2073).
struct SelectionMagnifierNode {
    source: Rc<dyn SelectionMagnifierSource>,
    animatable: Animatable<Offset>,
    /// Its layout node's density: what the magnifier's size is told in pixels at.
    density: Rc<Cell<Density>>,
    magnifier: Option<Delegate<MagnifierNode>>,
    /// The task following the center while a gesture shows the magnifier.
    job: Option<compose_runtime::Job>,
    /// The springs it started that may still run: a snap or the gesture's end ends them, so
    /// they wait for no frame.
    running: Rc<std::cell::RefCell<Vec<compose_runtime::Job>>>,
    delegates: Delegates,
}

impl SelectionMagnifierNode {
    fn new(source: Rc<dyn SelectionMagnifierSource>) -> SelectionMagnifierNode {
        let initial = compose_runtime::snapshots::Snapshot::without_read_observation(|| {
            source.magnifier_center()
        });
        let animatable =
            Animatable::new_with(initial, Some(Offset::new(THRESHOLD, THRESHOLD)), "magnifier");
        let density = Rc::new(Cell::new(Density::new(1.0)));
        SelectionMagnifierNode {
            source,
            animatable,
            density,
            magnifier: None,
            job: None,
            running: Rc::default(),
            delegates: Delegates::new(),
        }
    }

    /// A gesture shows the magnifier: delegated to, at the node's place in the root.
    fn show(&mut self, context: &mut NodeContext<'_>) {
        if self.magnifier.is_some() {
            return;
        }
        let (center, told, at) =
            (self.animatable.clone(), self.source.clone(), self.density.clone());
        let args = MagnifierArgs {
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
        };
        // Laid out already: the layout that placed it told its position to no magnifier.
        let anchor = context.require_layout_coordinates().position_in_root();
        let node = MagnifierNode::new_at(args, anchor);
        self.magnifier = Some(self.delegates.delegate_attached(node, context));
    }

    /// The gesture's over: the magnifier goes (dismissed as it's detached).
    fn hide(&mut self) {
        if let Some(magnifier) = self.magnifier.take() {
            self.delegates.undelegate(magnifier);
        }
    }
}

impl SelectionMagnifierNode {
    /// Follows the source's center (observed): a gesture starting shows the magnifier and starts
    /// the task following it, as Compose's `snapshotFlow` collector; one ending stops both.
    fn follow(&mut self, context: &mut NodeContext<'_>) {
        let source = self.source.clone();
        let center = context.observe_reads(move || source.magnifier_center());
        if center.is_unspecified() {
            self.stop();
            self.animatable.snap_to(Offset::UNSPECIFIED);
            self.hide();
            return;
        }
        if self.job.is_some() {
            return;
        }
        // Compose's `snapTo` of the first specified target.
        self.animatable.snap_to(center);
        self.show(context);
        let scope = context.coroutine_scope();
        let (source, animated) = (self.source.clone(), self.animatable.clone());
        let running = self.running.clone();
        self.job = Some(scope.launch(async move {
            let mut targets =
                compose_runtime::snapshot_flow(move || Center(source.magnifier_center()));
            while let Some(Center(target)) = compose_runtime::flow::next(&mut targets).await {
                let value = animated.value();
                let mut running = running.borrow_mut();
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
        }));
    }
}

impl SelectionMagnifierNode {
    /// The task following the center and the springs it started end.
    fn stop(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
        for job in self.running.borrow_mut().drain(..) {
            job.cancel();
        }
    }
}

impl ModifierNode for SelectionMagnifierNode {
    fn delegates(&mut self) -> Option<&mut Delegates> {
        Some(&mut self.delegates)
    }
    fn as_observer(&mut self) -> Option<&mut dyn compose_ui::ObserverModifierNode> {
        Some(self)
    }
    fn on_density_change(&mut self, context: &mut NodeContext<'_>) {
        self.density.set(context.require_density());
    }
    fn on_detach(&mut self, _context: &mut NodeContext<'_>) {
        self.stop();
        self.hide();
    }
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        self.density.set(context.require_density());
        self.follow(context);
    }
}

impl compose_ui::ObserverModifierNode for SelectionMagnifierNode {
    fn on_observed_reads_changed(&mut self, context: &mut NodeContext<'_>) {
        self.follow(context);
    }
}
