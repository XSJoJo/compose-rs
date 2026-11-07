//! Compose's `StyleResolverNodeTest` (device and host): a resolver bound to its node in a
//! tree, swapped, detached, reading its parents' locals and nested styles; and compose-rs's
//! checks of what a state change invalidates (D1583) and of interactions setting the state.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::style::properties::*;
use compose_foundation::style::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::DrawOp;
use compose_ui::{
    Constraints, HoverEnter, HoverInteraction, IntSize, Interaction, MutableInteractionSource,
    Offset, Press, PressInteraction, UnitExt,
};

use crate::style_support::{StyleSetters, styleable};

fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(300, 300), content)
}

fn fails(block: impl FnOnce()) -> bool {
    catch_unwind(AssertUnwindSafe(block)).is_err()
}

/// A layout modifier reading `read` from `resolver` while measuring, into `out`.
fn reading<T: 'static>(
    resolver: StyleResolver,
    read: impl Fn(&StyleResolverScope) -> T + 'static,
    out: Rc<Cell<Option<T>>>,
) -> Modifier {
    Modifier.layout(move |scope, measurable, constraints| {
        out.set(Some(resolver.resolve(&read)));
        let placeable = scope.measure(measurable, constraints);
        scope.layout_with(placeable.width, placeable.height, move |placement| {
            placement.place(&placeable, 0, 0);
        })
    })
}

static CUSTOM_COLOR: ProvidableStyleProperty<Color> =
    style_property_of("customColor", || Color::BLACK);
static TEST_PROP: ProvidableStyleProperty<i32> = style_property_of("testProp", || 0);
static TEST_STRING: ProvidableStyleProperty<&'static str> =
    style_property_of("testProp", || "default");
static SIZE_LOCAL: ProvidableStyleProperty<i32> = style_local_of("sizeLocal", || 10);
static CHILD_KEY: NestedStyleKey = NestedStyleKey::new("child");

fn providing<T: Clone + PartialEq + 'static>(
    property: &'static ProvidableStyleProperty<T>,
    value: T,
) -> StyleResolver {
    StyleResolver::of(CommonStyle::new(move |s| s.provide(property, value.clone())))
}

#[test]
fn node_reuse_recomposition_swaps_resolvers() {
    let resolver_a = providing(&CUSTOM_COLOR, Color::RED);
    let resolver_b = providing(&CUSTOM_COLOR, Color::BLUE);
    let resolver = mutable_state_of(resolver_a.clone());
    let current = resolver.handle();
    let resolved = Rc::new(Cell::new(None));
    let out = resolved.clone();
    let mut ui = rule(move || {
        let resolver = current.get();
        let read = reading(resolver.clone(), |s| s.value(&CUSTOM_COLOR), out.clone());
        Box!(modifier = Modifier.style_resolver(resolver).then(read));
    });
    assert_eq!(resolved.get(), Some(Color::RED));
    assert_eq!(resolver_a.resolve(|s| s.value(&CUSTOM_COLOR)), Color::RED);
    resolver.set(resolver_b.clone());
    ui.wait_for_idle();
    assert_eq!(resolved.get(), Some(Color::BLUE));
    assert_eq!(resolver_b.resolve(|s| s.value(&CUSTOM_COLOR)), Color::BLUE);
    assert!(fails(|| {
        resolver_a.resolve(|s| s.value(&CUSTOM_COLOR));
    }));
}

#[test]
fn node_reuse_styleable_modifier_updates_layout() {
    let resolver_a = StyleResolver::of(CommonStyle::new(|s| s.size(50.dp())));
    let resolver_b = StyleResolver::of(CommonStyle::new(|s| s.size(100.dp())));
    let resolver = mutable_state_of(resolver_a.clone());
    let current = resolver.handle();
    let mut ui = rule(move || {
        Box!(modifier = styleable(Modifier.test_tag("box"), &current.get()));
    });
    ui.on_node_with_tag("box").assert_width_is_equal_to(50.dp()).assert_height_is_equal_to(50.dp());
    resolver.set(resolver_b);
    ui.wait_for_idle();
    ui.on_node_with_tag("box")
        .assert_width_is_equal_to(100.dp())
        .assert_height_is_equal_to(100.dp());
    assert!(fails(|| resolver_a.resolve(|_| ())));
}

#[test]
fn node_reuse_multiple_successive_updates() {
    let resolvers = [providing(&TEST_PROP, 1), providing(&TEST_PROP, 2), providing(&TEST_PROP, 3)];
    let resolver = mutable_state_of(resolvers[0].clone());
    let current = resolver.handle();
    let resolved = Rc::new(Cell::new(None));
    let out = resolved.clone();
    let mut ui = rule(move || {
        let resolver = current.get();
        let read = reading(resolver.clone(), |s| s.value(&TEST_PROP), out.clone());
        Box!(modifier = Modifier.style_resolver(resolver).then(read));
    });
    assert_eq!(resolved.get(), Some(1));
    for (index, next) in resolvers.iter().enumerate().skip(1) {
        resolver.set(next.clone());
        ui.wait_for_idle();
        let expected = index as i32 + 1;
        assert_eq!(resolved.get(), Some(expected));
        assert_eq!(next.resolve(|s| s.value(&TEST_PROP)), expected);
        assert!(fails(|| {
            resolvers[index - 1].resolve(|s| s.value(&TEST_PROP));
        }));
    }
}

#[test]
fn node_reuse_same_resolver_instance_preserved_on_recomposition() {
    let resolver = providing(&TEST_STRING, "hello");
    let counter = mutable_state_of(0);
    let count = counter.handle();
    let resolved = Rc::new(Cell::new(None));
    let out = resolved.clone();
    let bound = resolver.clone();
    let mut ui = rule(move || {
        count.get();
        let read = reading(bound.clone(), |s| s.value(&TEST_STRING), out.clone());
        Box!(modifier = Modifier.style_resolver(bound.clone()).then(read));
    });
    assert_eq!(resolved.get(), Some("hello"));
    counter.set(1);
    ui.wait_for_idle();
    assert_eq!(resolved.get(), Some("hello"));
    assert_eq!(resolver.resolve(|s| s.value(&TEST_STRING)), "hello");
}

#[test]
fn node_reuse_detaching_disposes_active_resolver() {
    let resolver_a = providing(&TEST_PROP, 20);
    let resolver_b = providing(&TEST_PROP, 30);
    let resolver = mutable_state_of(resolver_a.clone());
    let show = mutable_state_of(true);
    let (current, shown) = (resolver.handle(), show.handle());
    let mut ui = rule(move || {
        if shown.get() {
            Box!(modifier = Modifier.style_resolver(current.get()));
        }
    });
    assert_eq!(resolver_a.resolve(|s| s.value(&TEST_PROP)), 20);
    resolver.set(resolver_b.clone());
    ui.wait_for_idle();
    assert_eq!(resolver_b.resolve(|s| s.value(&TEST_PROP)), 30);
    assert!(fails(|| {
        resolver_a.resolve(|s| s.value(&TEST_PROP));
    }));
    show.set(false);
    ui.wait_for_idle();
    assert!(fails(|| {
        resolver_b.resolve(|s| s.value(&TEST_PROP));
    }));
}

#[test]
fn node_reuse_conditional_remembered_style_swap_resolves_new_style() {
    let large = mutable_state_of(false);
    let use_large = large.handle();
    let resolved_in_new_style = Rc::new(Cell::new(false));
    let flag = resolved_in_new_style.clone();
    let mut ui = rule(move || {
        let is_large = use_large.get();
        let flag = flag.clone();
        let resolver = remember_observer_keyed(is_large, move || {
            let style = if is_large {
                CommonStyle::new(move |s| {
                    flag.set(true);
                    s.size(100.dp());
                })
            } else {
                CommonStyle::new(|s| s.size(50.dp()))
            };
            StyleResolver::of(style)
        });
        Box!(modifier = styleable(Modifier.test_tag("box"), &resolver));
    });
    ui.on_node_with_tag("box").assert_width_is_equal_to(50.dp());
    large.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag("box").assert_width_is_equal_to(100.dp());
    assert!(resolved_in_new_style.get());
}

#[test]
fn parent_resolver_swap_child_reading_style_local_updates() {
    let large = mutable_state_of(false);
    let use_large = large.handle();
    let resolved = Rc::new(Cell::new(None));
    let out = resolved.clone();
    let mut ui = rule(move || {
        let is_large = use_large.get();
        let parent = remember_observer_keyed(is_large, move || {
            providing(&SIZE_LOCAL, if is_large { 100 } else { 50 })
        });
        let child = remember_observer(|| StyleResolver::of(CommonStyle::EMPTY));
        let out = out.clone();
        Box!(modifier = Modifier.style_resolver((*parent).clone()), move |_| {
            let child = (*child).clone();
            let read = reading(child.clone(), |s| s.value(&SIZE_LOCAL), out.clone());
            Box!(modifier = Modifier.style_resolver(child).then(read));
        });
    });
    assert_eq!(resolved.get(), Some(50));
    large.set(true);
    ui.wait_for_idle();
    assert_eq!(resolved.get(), Some(100));
}

#[test]
fn parent_resolver_swap_child_applying_nested_style_updates() {
    let large = mutable_state_of(false);
    let use_large = large.handle();
    let mut ui = rule(move || {
        let is_large = use_large.get();
        let parent = remember_observer_keyed(is_large, move || {
            let size = if is_large { 100.dp() } else { 50.dp() };
            StyleResolver::of(CommonStyle::new(move |s| {
                s.provide_nested_style(&CHILD_KEY, &CommonStyle::new(move |s| s.size(size)));
            }))
        });
        let child = remember_observer(|| {
            StyleResolver::of(CommonStyle::new(|s| s.apply_nested_style(&CHILD_KEY)))
        });
        Box!(modifier = Modifier.style_resolver((*parent).clone()), move |_| {
            Box!(modifier = styleable(Modifier.test_tag("child"), &child));
        });
    });
    ui.on_node_with_tag("child").assert_width_is_equal_to(50.dp());
    large.set(true);
    ui.wait_for_idle();
    ui.on_node_with_tag("child").assert_width_is_equal_to(100.dp());
}

/// Compose's host `unattachedNode_updateResolver_bindsOnAttach`: a resolver is read only once
/// its node is attached, and not after it's detached.
#[test]
fn a_resolver_is_bound_while_its_node_is_attached() {
    let resolver = providing(&TEST_STRING, "valueB");
    assert!(fails(|| {
        resolver.resolve(|s| s.value(&TEST_STRING));
    }));
    let show = mutable_state_of(true);
    let shown = show.handle();
    let bound = resolver.clone();
    let mut ui = rule(move || {
        if shown.get() {
            Box!(modifier = Modifier.style_resolver(bound.clone()));
        }
    });
    assert_eq!(resolver.resolve(|s| s.value(&TEST_STRING)), "valueB");
    show.set(false);
    ui.wait_for_idle();
    assert!(fails(|| {
        resolver.resolve(|s| s.value(&TEST_STRING));
    }));
}

#[test]
#[should_panic(expected = "Cannot bind a style resolver to more than one styleResolver modifier")]
fn a_resolver_binds_to_one_node() {
    let resolver = providing(&TEST_PROP, 1);
    let _ui = rule(move || {
        Box!(modifier = Modifier.style_resolver(resolver.clone()));
        Box!(modifier = Modifier.style_resolver(resolver.clone()));
    });
}

/// The fills drawn, in order.
fn fills(ui: &mut ComposeTestRule) -> Vec<Color> {
    let list = ui.host().display_list();
    list.ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::DrawRect(_, paint) => Some(paint.color),
            _ => None,
        })
        .collect()
}

/// A box styled by `style` for `state`, counting the measurements of its content.
fn counted_box(
    style: CommonStyle,
    state: &MutableStyleState,
    measures: Rc<Cell<u32>>,
) -> ComposeTestRule {
    let state = state.clone();
    rule(move || {
        let state = state.clone();
        let style = style.clone();
        let resolver = remember_observer(move || StyleResolver::new(style, state));
        let measures = measures.clone();
        let counting = Modifier.layout(move |scope, measurable, constraints: Constraints| {
            measures.set(measures.get() + 1);
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        });
        Box!(modifier = styleable(counting, &resolver), |_| {
            Box!(modifier = Modifier.size(10.dp()));
        });
    })
}

/// D1583: a state change that only changes a drawing property draws again and measures
/// nothing, as Compose's appearance node observing its block.
#[test]
fn a_drawing_only_state_change_draws_without_measuring() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.size(40.dp());
        s.background(Color::RED);
        s.pressed(|s| s.background(Color::BLUE));
    });
    let measures = Rc::new(Cell::new(0));
    let mut ui = counted_box(style, &state, measures.clone());
    assert_eq!(fills(&mut ui), vec![Color::RED]);
    let measured = measures.get();
    state.set_pressed(true);
    ui.wait_for_idle();
    assert_eq!(fills(&mut ui), vec![Color::BLUE]);
    assert_eq!(measures.get(), measured, "a background change measured again");
}

/// A layout property's change measures again.
#[test]
fn a_layout_state_change_measures_again() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.size(40.dp());
        s.pressed(|s| s.content_padding(5.dp()));
    });
    let measures = Rc::new(Cell::new(0));
    let mut ui = counted_box(style, &state, measures.clone());
    let measured = measures.get();
    state.set_pressed(true);
    ui.wait_for_idle();
    assert!(measures.get() > measured);
}

/// An alpha change updates the layer and measures nothing.
#[test]
fn a_layer_state_change_measures_nothing() {
    let state = MutableStyleState::new(None);
    let style = CommonStyle::new(|s| {
        s.size(40.dp());
        s.alpha(0.5);
        s.pressed(|s| s.alpha(0.25));
    });
    let measures = Rc::new(Cell::new(0));
    let mut ui = counted_box(style, &state, measures.clone());
    let measured = measures.get();
    state.set_pressed(true);
    ui.wait_for_idle();
    assert_eq!(measures.get(), measured, "an alpha change measured again");
}

/// The state's source sets its pressed and hovered flags (Compose's `processInteractions`),
/// each counted until its end.
#[test]
fn the_interaction_source_sets_the_state() {
    let source = MutableInteractionSource::new();
    let state = MutableStyleState::new(Some(source.as_source().clone()));
    let style = CommonStyle::new(|s| {
        s.size(40.dp());
        s.background(Color::RED);
        s.hovered(|s| s.background(Color::GREEN));
        s.pressed(|s| s.background(Color::BLUE));
    });
    let mut ui = counted_box(style, &state, Rc::new(Cell::new(0)));
    let first = Press::new(Offset::ZERO);
    let second = Press::new(Offset::ZERO);
    source.try_emit(PressInteraction::Press(first));
    source.try_emit(PressInteraction::Press(second));
    ui.wait_for_idle();
    assert!(state.is_pressed());
    assert_eq!(fills(&mut ui), vec![Color::BLUE]);
    source.try_emit(PressInteraction::Release(first));
    ui.wait_for_idle();
    assert!(state.is_pressed(), "a press is still under way");
    source.try_emit(PressInteraction::Cancel(second));
    let enter = HoverEnter::new();
    source.try_emit(HoverInteraction::Enter(enter));
    ui.wait_for_idle();
    assert!(!state.is_pressed());
    assert!(state.is_hovered());
    assert_eq!(fills(&mut ui), vec![Color::GREEN]);
    source.try_emit(Interaction::Hover(HoverInteraction::Exit(enter)));
    ui.wait_for_idle();
    assert_eq!(fills(&mut ui), vec![Color::RED]);
}

static TOUCHES: StyleStateKey<i32> = StyleStateKey::with_interactions(
    "touches",
    || 0,
    |interaction, state| {
        if let Interaction::Custom(_) = interaction {
            state.set(&TOUCHES, state.get(&TOUCHES) + 1);
        }
    },
);

/// A custom key set in the state is given the other interactions, as `processInteraction`.
#[test]
fn a_custom_key_processes_the_other_interactions() {
    let source = MutableInteractionSource::new();
    let state = MutableStyleState::new(Some(source.as_source().clone()));
    let style = CommonStyle::new(|s| {
        s.size(40.dp());
        s.background(Color::RED);
        s.state_when(&TOUCHES, |key, state| state.get(key) >= 2, |s| s.background(Color::BLUE));
    });
    let mut ui = counted_box(style, &state, Rc::new(Cell::new(0)));
    source.try_emit(Interaction::custom(1));
    ui.wait_for_idle();
    assert_eq!(state.get(&TOUCHES), 0, "a key not set in the state is given nothing");
    state.set(&TOUCHES, 1);
    source.try_emit(Interaction::custom(2));
    ui.wait_for_idle();
    assert_eq!(state.get(&TOUCHES), 2);
    assert_eq!(fills(&mut ui), vec![Color::BLUE]);
}

/// A child resolver reads its parent's content fill (a local) through the tree.
#[test]
fn a_child_reads_its_parent_s_locals_through_the_tree() {
    let state = MutableStyleState::new(None);
    let parent_style = CommonStyle::new(|s| {
        s.provide(&CONTENT_FILL, Fill::Color(Color::RED));
        s.pressed(|s| s.provide(&CONTENT_FILL, Fill::Color(Color::BLUE)));
    });
    let parent_state = state.clone();
    let mut ui = rule(move || {
        let (style, state) = (parent_style.clone(), parent_state.clone());
        let parent = remember_observer(move || StyleResolver::new(style, state));
        let child = remember_observer(|| StyleResolver::of(CommonStyle::EMPTY));
        Box!(modifier = Modifier.style_resolver((*parent).clone()), move |_| {
            let reader = (*child).clone();
            Box!(
                modifier = Modifier.style_resolver((*child).clone()).size(10.dp()).appearance(
                    move |appearance| {
                        appearance.background = reader.resolve(|s| s.content_fill());
                    }
                )
            );
        });
    });
    assert_eq!(fills(&mut ui), vec![Color::RED]);
    state.set_pressed(true);
    ui.wait_for_idle();
    assert_eq!(fills(&mut ui), vec![Color::BLUE]);
}

/// Compose's host `styleResolverElement_createAndEquals`: elements of the same resolver are the
/// same; of another, not.
#[test]
fn style_resolver_elements_are_equal_for_the_same_resolver() {
    let resolver_a = StyleResolver::of(CommonStyle::EMPTY);
    let resolver_b = StyleResolver::of(CommonStyle::EMPTY);
    let a1 = Modifier.style_resolver(resolver_a.clone());
    let a2 = Modifier.style_resolver(resolver_a);
    let b = Modifier.style_resolver(resolver_b);
    assert!(a1.same(&a2));
    assert!(!a1.same(&b));
}

/// A resolver's parent is its nearest ancestor's: a local set at both levels reads the
/// nearer one.
#[test]
fn a_child_reads_the_nearest_ancestor_s_local() {
    let resolved = Rc::new(Cell::new(None));
    let out = resolved.clone();
    let _ui = rule(move || {
        let outer = remember_observer(|| providing(&SIZE_LOCAL, 1));
        let inner = remember_observer(|| providing(&SIZE_LOCAL, 2));
        let child = remember_observer(|| StyleResolver::of(CommonStyle::EMPTY));
        let out = out.clone();
        Box!(modifier = Modifier.style_resolver((*outer).clone()), move |_| {
            let (child, out) = (child.clone(), out.clone());
            Box!(modifier = Modifier.style_resolver((*inner).clone()), move |_| {
                let child = (*child).clone();
                let read = reading(child.clone(), |s| s.value(&SIZE_LOCAL), out.clone());
                Box!(modifier = Modifier.style_resolver(child).then(read));
            });
        });
    });
    assert_eq!(resolved.get(), Some(2));
}

/// A border is drawn when half its width is a pixel or more, as Compose's appearance node.
#[test]
fn a_border_under_two_pixels_wide_is_not_drawn() {
    let strokes = |width: f32| {
        let mut ui = rule(move || {
            let resolver = remember_observer(move || {
                StyleResolver::of(CommonStyle::new(move |s| {
                    s.size(20.dp());
                    s.border(compose_ui::Dp(width), Color::RED);
                }))
            });
            Box!(modifier = styleable(Modifier, &resolver));
        });
        fills(&mut ui).len()
    };
    assert_eq!(strokes(0.8), 0);
    assert_eq!(strokes(1.2), 1);
}
