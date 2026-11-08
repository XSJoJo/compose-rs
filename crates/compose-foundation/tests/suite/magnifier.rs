//! `Modifier.magnifier`, from Compose's `MagnifierTest`: when its magnifier is made, made anew,
//! shown, hidden and its content copied, through fake platform magnifiers (the test rule's
//! recording factory, or one handing out a magnifier of the test's), as Compose's tests' fake
//! `PlatformMagnifierFactory`s (D2072).

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::{IntoCallback, mutable_state_of};
use compose_test::*;
use compose_ui::{
    Color, Density, Dp, DpSize, IntOffset, IntSize, Offset, Paint, PlatformMagnifier,
    PlatformMagnifierConfig, PlatformMagnifierFactory, UnitExt,
};

/// Hands out one magnifier, as the test's `PlatformMagnifierFactory(platformMagnifier)`.
struct OneMagnifier {
    magnifier: Rc<TestMagnifier>,
    can_update_zoom: bool,
}

impl PlatformMagnifierFactory for OneMagnifier {
    fn can_update_zoom(&self) -> bool {
        self.can_update_zoom
    }
    fn create(&self, _config: &PlatformMagnifierConfig) -> Rc<dyn PlatformMagnifier> {
        self.magnifier.clone()
    }
}

/// A magnifier of no size counting what it's told, as the test's `CountingPlatformMagnifier`.
fn counting_magnifier() -> Rc<TestMagnifier> {
    let config = PlatformMagnifierConfig {
        use_text_default: false,
        size: DpSize::UNSPECIFIED,
        corner_radius: Dp::UNSPECIFIED,
        elevation: Dp::UNSPECIFIED,
        clipping_enabled: true,
        density: Density::new(1.0),
        initial_zoom: f32::NAN,
    };
    Rc::new(TestMagnifier::new(config, IntSize::ZERO))
}

fn factory_of(
    magnifier: &Rc<TestMagnifier>,
    can_update_zoom: bool,
) -> Rc<dyn PlatformMagnifierFactory> {
    Rc::new(OneMagnifier { magnifier: magnifier.clone(), can_update_zoom })
}

/// A counting factory, as the test's `CountingPlatformMagnifierFactory`.
fn counting_factory(can_update_zoom: bool) -> Rc<TestMagnifierFactory> {
    let factory = Rc::new(TestMagnifierFactory::new());
    factory.set_can_update_zoom(can_update_zoom);
    factory
}

/// The arguments the tests pass: the source at the origin, no magnifier center, the zoom
/// unspecified, no size callback, `factory`.
fn args(factory: Rc<dyn PlatformMagnifierFactory>) -> MagnifierArgs {
    MagnifierArgs {
        magnifier_center: Some((|_: Density| Offset::UNSPECIFIED).into_callback()),
        platform_magnifier_factory: Some(factory),
        ..MagnifierArgs::new(|_: Density| Offset::ZERO)
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, magnifier_inspectorValue_whenSupported
#[test]
fn magnifier_inspectorValue_whenSupported() {
    // compose-rs's inspector info is read through the modifier's dump, which shows its name (its
    // properties aren't reachable from outside the tree).
    let modifier = Modifier.magnifier_with(
        |_: Density| Offset::new(42.0, 42.0),
        |_: Density| Offset::new(42.0, 42.0),
        None::<fn(DpSize)>,
        f32::NAN,
        DpSize::UNSPECIFIED,
        Dp::UNSPECIFIED,
        Dp::UNSPECIFIED,
        true,
    );
    assert_eq!(format!("{modifier:?}"), "Modifier.magnifier");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, magnifier_returnsEmptyModifier_whenNotSupported
#[test]
fn magnifier_returnsEmptyModifier_whenNotSupported() {
    let made: Rc<RefCell<Option<Modifier>>> = Rc::default();
    let kept = made.clone();
    let mut ui = ComposeTestRule::empty(default_host());
    ui.host().with_tree(|tree| tree.set_platform_magnifier_factory(None));
    ui.set_content(move || {
        *kept.borrow_mut() = Some(Modifier.magnifier(|_: Density| Offset::ZERO));
    });
    assert!(made.borrow().as_ref().expect("composed").is_empty());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_recreatesMagnifier_whenDensityChanged
#[test]
fn platformMagnifierModifier_recreatesMagnifier_whenDensityChanged() {
    let factory = counting_factory(false);
    let density = mutable_state_of(1.0f32);
    let read = density.handle();
    let given = factory.clone();
    let mut ui = compose_test(move || {
        let given = given.clone();
        compose_runtime::CompositionLocalProvider(
            compose_ui::LOCAL_DENSITY.provides(Density::new(read.get())),
            move || {
                Box!(modifier = Modifier.magnifier_of(args(given.clone())));
            },
        );
    });
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 1));
    density.set(density.get() * 2.0);
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_recreatesMagnifier_whenConfigurationChanged
#[test]
fn platformMagnifierModifier_recreatesMagnifier_whenConfigurationChanged() {
    let factory = counting_factory(false);
    let elevation = mutable_state_of(1.0f32);
    let read = elevation.handle();
    let given = factory.clone();
    let mut ui = compose_test(move || {
        let args = MagnifierArgs { elevation: read.get().dp(), ..args(given.clone()) };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 1));
    elevation.set(elevation.get() * 2.0);
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_recreatesMagnifier_whenConfigurationChangedToText
#[test]
fn platformMagnifierModifier_recreatesMagnifier_whenConfigurationChangedToText() {
    let factory = counting_factory(false);
    let use_text_default = mutable_state_of(false);
    let read = use_text_default.handle();
    let given = factory.clone();
    let mut ui = compose_test(move || {
        let args = MagnifierArgs { use_text_default: read.get(), ..args(given.clone()) };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 1));
    use_text_default.set(true);
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 2));
    assert!(factory.last().expect("made").config().use_text_default);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_recreatesMagnifier_whenCannotUpdateZoom
#[test]
fn platformMagnifierModifier_recreatesMagnifier_whenCannotUpdateZoom() {
    let factory = counting_factory(false);
    let zoom = mutable_state_of(1.0f32);
    let read = zoom.handle();
    let given = factory.clone();
    let mut ui = compose_test(move || {
        let args = MagnifierArgs { zoom: read.get(), ..args(given.clone()) };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 1));
    zoom.set(zoom.get() + 2.0);
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 2));
    assert_eq!(factory.last().expect("made").config().initial_zoom, 3.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_doesNotRecreateMagnifier_whenCanUpdateZoom
#[test]
fn platformMagnifierModifier_doesNotRecreateMagnifier_whenCanUpdateZoom() {
    let factory = counting_factory(true);
    let zoom = mutable_state_of(1.0f32);
    let read = zoom.handle();
    let given = factory.clone();
    let mut ui = compose_test(move || {
        let args = MagnifierArgs { zoom: read.get(), ..args(given.clone()) };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 1));
    zoom.set(zoom.get() + 2.0);
    ui.run_on_idle(|_| assert_eq!(factory.creation_count(), 1));
}

/// A box filling the window that draws a circle as big as `trigger`, then the magnifier.
fn redrawn_content(
    trigger: compose_runtime::MutableState<i32>,
    magnifier: Rc<TestMagnifier>,
) -> impl Fn() {
    move || {
        let factory = factory_of(&magnifier, false);
        Box!(
            modifier = Modifier
                // If the node has no size, it doesn't draw.
                .fill_max_size()
                .draw_behind(move |scope| {
                    // Read so a change draws it again.
                    let center = scope.center();
                    scope.draw_circle(center, trigger.get() as f32, Paint::fill(Color::BLACK));
                })
                .magnifier_of(args(factory))
        );
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_updatesContent_whenLayerRedrawn
#[test]
fn platformMagnifierModifier_updatesContent_whenLayerRedrawn() {
    let trigger = mutable_state_of(0);
    let magnifier = counting_magnifier();
    let mut ui = compose_test(redrawn_content(trigger.handle(), magnifier.clone()));
    // Once at once: there's always a first drawing.
    ui.run_on_idle(|_| assert_eq!(magnifier.content_update_count(), 1));
    trigger.set(trigger.get() + 1);
    ui.run_on_idle(|_| assert_eq!(magnifier.content_update_count(), 2));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_doesNotUpdateProperties_whenLayerRedrawn
#[test]
fn platformMagnifierModifier_doesNotUpdateProperties_whenLayerRedrawn() {
    let trigger = mutable_state_of(0);
    let magnifier = counting_magnifier();
    let mut ui = compose_test(redrawn_content(trigger.handle(), magnifier.clone()));
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 1));
    trigger.set(trigger.get() + 1);
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 1));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_updatesProperties_whenPlacementChanged
#[test]
fn platformMagnifierModifier_updatesProperties_whenPlacementChanged() {
    let layout_offset = mutable_state_of(IntOffset::ZERO);
    let read = layout_offset.handle();
    let magnifier = counting_magnifier();
    let given = magnifier.clone();
    let mut ui = compose_test(move || {
        let factory = factory_of(&given, false);
        Box!(modifier = Modifier.offset_with(move |_| read.get()).magnifier_of(args(factory)));
    });
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 1));
    layout_offset.set(layout_offset.get() + IntOffset::new(10, 1));
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 2));
    assert_eq!(magnifier.shown().expect("shown").source_center, Offset::new(10.0, 1.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_updatesProperties_whenSourceCenterChanged
#[test]
fn platformMagnifierModifier_updatesProperties_whenSourceCenterChanged() {
    let source_center = mutable_state_of(Offset::new(1.0, 1.0));
    let read = source_center.handle();
    let magnifier = counting_magnifier();
    let given = magnifier.clone();
    let mut ui = compose_test(move || {
        let factory = factory_of(&given, false);
        let args = MagnifierArgs {
            source_center: (move |_: Density| read.get()).into_callback(),
            ..args(factory)
        };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 1));
    source_center.set(source_center.get() + Offset::new(1.0, 1.0));
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 2));
    assert_eq!(magnifier.shown().expect("shown").source_center, Offset::new(2.0, 2.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_updatesProperties_whenMagnifierCenterChanged
#[test]
fn platformMagnifierModifier_updatesProperties_whenMagnifierCenterChanged() {
    let magnifier_center = mutable_state_of(Offset::new(1.0, 1.0));
    let read = magnifier_center.handle();
    let magnifier = counting_magnifier();
    let given = magnifier.clone();
    let mut ui = compose_test(move || {
        let factory = factory_of(&given, false);
        let args = MagnifierArgs {
            magnifier_center: Some((move |_: Density| read.get()).into_callback()),
            ..args(factory)
        };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 1));
    magnifier_center.set(magnifier_center.get() + Offset::new(1.0, 1.0));
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 2));
    assert_eq!(magnifier.shown().expect("shown").magnifier_center, Offset::new(2.0, 2.0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_updatesProperties_whenZoomChanged
#[test]
fn platformMagnifierModifier_updatesProperties_whenZoomChanged() {
    let zoom = mutable_state_of(1.0f32);
    let read = zoom.handle();
    let magnifier = counting_magnifier();
    let given = magnifier.clone();
    let mut ui = compose_test(move || {
        let factory = factory_of(&given, true);
        let args = MagnifierArgs { zoom: read.get(), ..args(factory) };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 1));
    zoom.set(zoom.get() + 1.0);
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 2));
    assert_eq!(magnifier.shown().expect("shown").zoom, 2.0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_dismissesMagnifier_whenRemovedFromComposition
#[test]
fn platformMagnifierModifier_dismissesMagnifier_whenRemovedFromComposition() {
    let show = mutable_state_of(true);
    let read = show.handle();
    let magnifier = counting_magnifier();
    let given = magnifier.clone();
    let mut ui = compose_test(move || {
        let modifier = if read.get() {
            Modifier.magnifier_of(args(factory_of(&given, false)))
        } else {
            Modifier
        };
        Box!(modifier = modifier);
    });
    let initial = ui.run_on_idle(|_| magnifier.dismiss_count());
    show.set(false);
    ui.run_on_idle(|_| assert_eq!(magnifier.dismiss_count(), initial + 1));
    assert!(!magnifier.is_showing());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_dismissesMagnifier_whenCenterUnspecified
#[test]
fn platformMagnifierModifier_dismissesMagnifier_whenCenterUnspecified() {
    // Shown first, then hidden: dismissed, not just never shown.
    let source_center = mutable_state_of(Offset::ZERO);
    let read = source_center.handle();
    let magnifier = counting_magnifier();
    let given = magnifier.clone();
    let mut ui = compose_test(move || {
        let args = MagnifierArgs {
            source_center: (move |_: Density| read.get()).into_callback(),
            ..args(factory_of(&given, false))
        };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    ui.run_on_idle(|_| assert_eq!(magnifier.property_update_count(), 1));
    let initial = ui.run_on_idle(|_| magnifier.dismiss_count());
    source_center.set(Offset::UNSPECIFIED);
    ui.run_on_idle(|_| {
        assert_eq!(magnifier.property_update_count(), 1);
        assert_eq!(magnifier.dismiss_count(), initial + 1);
    });
    // compose-rs's own check: its semantics say no position any more (D2073), as Compose's tests'
    // `assertNoMagnifierExists` reads them.
    ui.on_node(key_is_defined(&MAGNIFIER_POSITION_IN_ROOT)).assert_does_not_exist();
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_dismissesMagnifier_whenMagnifierRecreated
#[test]
fn platformMagnifierModifier_dismissesMagnifier_whenMagnifierRecreated() {
    let elevation = mutable_state_of(1.0f32);
    let read = elevation.handle();
    let magnifier = counting_magnifier();
    let given = magnifier.clone();
    let mut ui = compose_test(move || {
        let args = MagnifierArgs { elevation: read.get().dp(), ..args(factory_of(&given, false)) };
        Box!(modifier = Modifier.magnifier_of(args));
    });
    let initial = ui.run_on_idle(|_| magnifier.dismiss_count());
    elevation.set(elevation.get() + 1.0);
    ui.run_on_idle(|_| assert_eq!(magnifier.dismiss_count(), initial + 1));
}

/// A magnifier of 10 by 11 shown over `source_center`, its sizes told into `events`.
fn sized_content(
    magnifier: Rc<TestMagnifier>,
    source_center: impl Fn() -> Offset + 'static,
    size: impl Fn() -> DpSize + 'static,
    events: Rc<RefCell<Vec<DpSize>>>,
) -> impl Fn() {
    let source_center = Rc::new(source_center);
    move || {
        let source_center = source_center.clone();
        let events = events.clone();
        let args = MagnifierArgs {
            source_center: (move |_: Density| source_center()).into_callback(),
            on_size_changed: Some(
                (move |size: DpSize| events.borrow_mut().push(size)).into_callback(),
            ),
            size: size(),
            ..args(factory_of(&magnifier, false))
        };
        Box!(modifier = Modifier.magnifier_of(args));
    }
}

fn dp_size(size: IntSize) -> DpSize {
    // The rule's density is 1.
    DpSize::new((size.width as f32).dp(), (size.height as f32).dp())
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_firesOnSizeChanged_initially
#[test]
fn platformMagnifierModifier_firesOnSizeChanged_initially() {
    let size = IntSize::new(10, 11);
    let events: Rc<RefCell<Vec<DpSize>>> = Rc::default();
    let magnifier = counting_magnifier();
    magnifier.set_size(size);
    let mut ui = compose_test(sized_content(
        magnifier,
        || Offset::ZERO,
        || DpSize::UNSPECIFIED,
        events.clone(),
    ));
    assert_eq!(ui.density(), Density::new(1.0));
    ui.run_on_idle(|_| assert_eq!(*events.borrow(), vec![dp_size(size)]));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_doesNotFireOnSizeChanged_initially_whenSourceCenterUnspecified
#[test]
fn platformMagnifierModifier_doesNotFireOnSizeChanged_initially_whenSourceCenterUnspecified() {
    let events: Rc<RefCell<Vec<DpSize>>> = Rc::default();
    let magnifier = counting_magnifier();
    magnifier.set_size(IntSize::new(10, 11));
    let mut ui = compose_test(sized_content(
        magnifier,
        || Offset::UNSPECIFIED,
        || DpSize::UNSPECIFIED,
        events.clone(),
    ));
    ui.run_on_idle(|_| assert!(events.borrow().is_empty()));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_firesOnSizeChanged_afterSourceCenterIsSpecified
#[test]
fn platformMagnifierModifier_firesOnSizeChanged_afterSourceCenterIsSpecified() {
    let size = IntSize::new(10, 11);
    let events: Rc<RefCell<Vec<DpSize>>> = Rc::default();
    let magnifier = counting_magnifier();
    magnifier.set_size(size);
    let source_center = mutable_state_of(Offset::UNSPECIFIED);
    let read = source_center.handle();
    let mut ui = compose_test(sized_content(
        magnifier,
        move || read.get(),
        || DpSize::UNSPECIFIED,
        events.clone(),
    ));
    ui.run_on_idle(|_| assert!(events.borrow().is_empty()));
    source_center.set(Offset::new(1.0, 1.0));
    ui.run_on_idle(|_| assert_eq!(*events.borrow(), vec![dp_size(size)]));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_firesOnSizeChanged_whenNewSizeRequested
#[test]
fn platformMagnifierModifier_firesOnSizeChanged_whenNewSizeRequested() {
    let size1 = IntSize::new(10, 11);
    let size2 = IntSize::new(20, 22);
    let events: Rc<RefCell<Vec<DpSize>>> = Rc::default();
    let magnifier = counting_magnifier();
    magnifier.set_size(size1);
    let size = mutable_state_of(size1);
    let read = size.handle();
    let mut ui = compose_test(sized_content(
        magnifier.clone(),
        || Offset::ZERO,
        move || dp_size(read.get()),
        events.clone(),
    ));
    ui.run_on_idle(|_| {
        // The fake reports the new size when asked, and the state recomposes.
        magnifier.set_size(size2);
        size.set(size2);
    });
    ui.run_on_idle(|_| assert_eq!(*events.borrow(), vec![dp_size(size1), dp_size(size2)]));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/MagnifierTest.kt, platformMagnifierModifier_reportsSemantics
#[test]
fn platformMagnifierModifier_reportsSemantics() {
    let offset = mutable_state_of(Offset::ZERO);
    let read = offset.handle();
    let mut ui = compose_test(move || {
        Box!(modifier = Modifier.magnifier(move |_: Density| read.get()));
    });
    let nodes =
        ui.on_all_nodes(key_is_defined(&MAGNIFIER_POSITION_IN_ROOT)).fetch_semantics_nodes();
    let position = match nodes.as_slice() {
        [node] => node.config().get(&MAGNIFIER_POSITION_IN_ROOT).cloned().expect("its position"),
        _ => panic!("one magnifier"),
    };
    ui.run_on_idle(|_| assert_eq!(position.get(), offset.get()));
    // Moved: the same function says where it is now.
    offset.set(Offset::new(42.0, 24.0));
    ui.run_on_idle(|_| assert_eq!(position.get(), offset.get()));
    // Through the rule's factory, which the platform's stands for.
    let shown = ui.magnifiers().last().expect("made").shown().expect("shown");
    assert_eq!(shown.source_center, Offset::new(42.0, 24.0));
}
