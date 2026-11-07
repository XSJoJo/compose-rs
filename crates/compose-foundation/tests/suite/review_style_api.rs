//! Review of the `style-api` branch (D1580 to D1586): the placement's offsets against Compose's
//! measure, property identity against the predefined names, nested styles beyond the two
//! collector fixes D1583 claims, the state's flags under an interaction source, and what a
//! removed styled node leaves behind.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose_animation::tween;
use compose_foundation::layout::*;
use compose_foundation::style::properties::*;
use compose_foundation::style::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Constraints, FocusInteraction, IntSize, MutableInteractionSource, Offset, Press,
    PressInteraction, UnitExt,
};

use crate::style_support::{StyleSetters, styleable, styled};

static FLOAT: ProvidableStyleProperty<f32> =
    style_property_of_with("review.float", |a, b, t| compose_ui::lerp(*a, *b, t), || 0.0);
static OTHER_FLOAT: ProvidableStyleProperty<f32> = style_property_of("review.other", || 0.0);
static KEY_A: NestedStyleKey = NestedStyleKey::new("review.a");
static KEY_B: NestedStyleKey = NestedStyleKey::new("review.b");

fn style(block: impl Fn(&mut dyn CommonStyleScope) + 'static) -> CommonStyle {
    CommonStyle::new(block)
}

// ---- Placement ---------------------------------------------------------------------------

/// Compose's placement adds `(left + right).toPx().fastRoundToInt()` to its size: with one side
/// unspecified that's NaN, rounded to 0, so `left` alone moves the content without growing the
/// node (as `offset`). compose-rs zeroes each unspecified side before adding (`placement.rs`),
/// so the node grows by the offset.
#[test]
#[ignore = "review: a lone left/top offset grows the node; Compose's NaN sum adds nothing"]
fn a_lone_left_offset_moves_the_content_without_growing_the_node() {
    let mut ui = ComposeTestRule::sized(IntSize::new(500, 500), || {
        Box!(
            modifier = styled(Modifier.test_tag("offset"), None, |s| {
                s.left(10.dp());
                s.top(20.dp());
                s.size(50.dp());
            }),
            {
                Box!(modifier = Modifier.test_tag("content").fill_max_size());
            }
        );
    });
    ui.on_node_with_tag("content")
        .assert_left_position_in_root_is_equal_to(10.dp())
        .assert_top_position_in_root_is_equal_to(20.dp());
    ui.on_node_with_tag("offset")
        .assert_width_is_equal_to(50.dp())
        .assert_height_is_equal_to(50.dp());
}

/// Both sides given: Compose adds both (the sum is finite), as compose-rs.
#[test]
fn left_and_right_offsets_add_to_the_size() {
    let mut ui = ComposeTestRule::sized(IntSize::new(500, 500), || {
        Box!(
            modifier = styled(Modifier.test_tag("offset"), None, |s| {
                s.left(10.dp());
                s.right(5.dp());
                s.size(50.dp());
            })
        );
    });
    ui.on_node_with_tag("offset").assert_width_is_equal_to(65.dp());
}

// ---- Property identity -------------------------------------------------------------------

/// A design system's own property, named as a predefined one: in Compose a property is its
/// object, so `stylePropertyOf("background")` isn't foundation's `backgroundProperty` and the
/// style appearance doesn't draw it. compose-rs identifies properties by name and type
/// (D1581), and the predefined names are short and now public (D1580): this one is
/// `BACKGROUND`.
static DESIGN_SYSTEM_BACKGROUND: ProvidableStyleProperty<Fill> =
    style_property_of("background", || Fill::None);

#[test]
#[ignore = "review: a property named as a predefined one (same type) is the predefined one"]
fn a_property_named_as_a_predefined_one_is_another_property() {
    let resolver = StyleResolver::of(style(|s| {
        s.provide(&DESIGN_SYSTEM_BACKGROUND, Fill::Color(Color::RED));
    }));
    resolver.collect_for_test(compose_ui::Density::new(1.0));
    resolver.resolve(|scope| {
        assert!(scope.is_set(&DESIGN_SYSTEM_BACKGROUND));
        assert!(!scope.is_set(&BACKGROUND), "the design system's property set foundation's");
    });
}

// ---- Nested styles -----------------------------------------------------------------------

/// D1583's first fix: while a child applies an ancestor's style for a key, only the nested
/// style given for that key writes, whatever its place in the ancestor's style.
#[test]
fn only_the_nested_style_writes_into_the_child_wherever_it_is() {
    let parent = StyleResolver::of(style(|s| {
        s.provide_nested_style(&KEY_A, &style(|s| s.provide(&FLOAT, 10.0)));
        s.provide(&FLOAT, 100.0);
        s.provide(&OTHER_FLOAT, 5.0);
    }));
    let child = StyleResolver::of(style(|s| s.apply_nested_style(&KEY_A)));
    child.bind_parent_for_test(&parent);
    let density = compose_ui::Density::new(1.0);
    parent.collect_for_test(density);
    child.collect_for_test(density);
    parent.resolve(|scope| {
        assert_eq!(scope.value(&FLOAT), 100.0);
        assert_eq!(scope.value(&OTHER_FLOAT), 5.0);
    });
    child.resolve(|scope| {
        assert_eq!(scope.value(&FLOAT), 10.0);
        assert!(!scope.is_set(&OTHER_FLOAT), "the parent's own write reached the child");
    });
}

/// A child applying an ancestor's nested style for A also runs the ancestor's
/// `provide_nested_style(B, …)`; compose-rs records B as a key the child gives (its collector
/// pushes every key other than the one applied), so a grandchild applying B also runs the
/// child's style, which applies A into the grandchild. Only the nested style for B should
/// reach it: the keys a style gives are the ones its own style provides, not those of an
/// ancestor's style it applies.
#[test]
#[ignore = "review: a child advertises the nested keys of the ancestor style it applies"]
fn an_ancestor_s_other_nested_keys_are_not_given_by_the_child() {
    let parent = StyleResolver::of(style(|s| {
        s.provide_nested_style(&KEY_A, &style(|s| s.provide(&FLOAT, 1.0)));
        s.provide_nested_style(&KEY_B, &style(|s| s.provide(&OTHER_FLOAT, 2.0)));
    }));
    let child = StyleResolver::of(style(|s| s.apply_nested_style(&KEY_A)));
    child.bind_parent_for_test(&parent);
    let grandchild = StyleResolver::of(style(|s| s.apply_nested_style(&KEY_B)));
    grandchild.bind_parent_for_test(&child);
    let density = compose_ui::Density::new(1.0);
    parent.collect_for_test(density);
    child.collect_for_test(density);
    grandchild.collect_for_test(density);
    child.resolve(|scope| {
        assert_eq!(scope.value(&FLOAT), 1.0);
        assert!(!scope.is_set(&OTHER_FLOAT));
    });
    grandchild.resolve(|scope| {
        assert_eq!(scope.value(&OTHER_FLOAT), 2.0);
        assert!(!scope.is_set(&FLOAT), "A's nested style reached the grandchild applying B");
    });
}

/// The nearest ancestor giving a key wins over a farther one (they run farthest first).
#[test]
fn the_nearest_ancestor_s_nested_style_wins() {
    let grandparent = StyleResolver::of(style(|s| {
        s.provide_nested_style(&KEY_A, &style(|s| s.provide(&FLOAT, 1.0)));
    }));
    let parent = StyleResolver::of(style(|s| {
        s.provide_nested_style(&KEY_A, &style(|s| s.provide(&FLOAT, 2.0)));
    }));
    parent.bind_parent_for_test(&grandparent);
    let child = StyleResolver::of(style(|s| s.apply_nested_style(&KEY_A)));
    child.bind_parent_for_test(&parent);
    let density = compose_ui::Density::new(1.0);
    grandparent.collect_for_test(density);
    parent.collect_for_test(density);
    child.collect_for_test(density);
    child.resolve(|scope| assert_eq!(scope.value(&FLOAT), 2.0));
}

/// As Compose's collector: the giver's style runs for the giver's state (whether it gives the
/// nested style), the nested style itself for the child's (its `nestedStateOverride` is
/// cleared while it runs).
#[test]
fn a_nested_style_reads_the_child_s_state_and_its_giver_s_around_it() {
    let parent_state = MutableStyleState::new(None);
    let parent = StyleResolver::new(
        style(|s| {
            s.pressed(|s| {
                let nested = style(|s| {
                    s.provide(&FLOAT, 1.0);
                    s.hovered(|s| s.provide(&FLOAT, 2.0));
                });
                s.provide_nested_style(&KEY_A, &nested);
            });
        }),
        parent_state.clone(),
    );
    let child_state = MutableStyleState::new(None);
    child_state.set_hovered(true);
    let child = StyleResolver::new(style(|s| s.apply_nested_style(&KEY_A)), child_state.clone());
    child.bind_parent_for_test(&parent);
    let density = compose_ui::Density::new(1.0);
    let resolve = || {
        parent.collect_for_test(density);
        child.collect_for_test(density);
        child.resolve(|scope| scope.get_or_null(&FLOAT))
    };
    assert_eq!(resolve(), None, "given only while the giver is pressed");
    parent_state.set_pressed(true);
    assert_eq!(resolve(), Some(2.0), "the nested style reads the child's hover");
    child_state.set_hovered(false);
    assert_eq!(resolve(), Some(1.0));
    parent_state.set_hovered(true);
    assert_eq!(resolve(), Some(1.0), "the giver's hover isn't the nested style's");
}

// ---- State flags under an interaction source ----------------------------------------------

/// A box bound to a resolver of `style` for `state`.
fn bound(style: CommonStyle, state: &MutableStyleState) -> ComposeTestRule {
    let state = state.clone();
    ComposeTestRule::sized(IntSize::new(300, 300), move || {
        let (style, state) = (style.clone(), state.clone());
        let resolver = remember_observer(move || StyleResolver::new(style, state));
        Box!(modifier = styleable(Modifier.size(40.dp()), &resolver));
    })
}

/// Compose's `processInteractions` sets only the flag an interaction is about (a press sets
/// `isPressed`); compose-rs's listener writes all three flags from its own counts on every
/// interaction (`state.rs`, `listen`), so a focus the component set by hand (the setters are
/// public) is cleared by a press.
#[test]
#[ignore = "review: a press clears a focused flag set by hand; Compose sets only isPressed"]
fn a_press_leaves_a_focus_set_by_hand() {
    let source = MutableInteractionSource::new();
    let state = MutableStyleState::new(Some(source.as_source().clone()));
    let mut ui = bound(style(|s| s.size(40.dp())), &state);
    state.set_focused(true);
    source.try_emit(PressInteraction::Press(Press::new(Offset::ZERO)));
    ui.wait_for_idle();
    assert!(state.is_pressed());
    assert!(state.is_focused(), "the press cleared the focus");
}

/// The flags count each kind by identity: a focus ending doesn't end another one.
#[test]
fn focuses_are_counted_until_each_ends() {
    let source = MutableInteractionSource::new();
    let state = MutableStyleState::new(Some(source.as_source().clone()));
    let mut ui = bound(style(|s| s.size(40.dp())), &state);
    let (first, second) = (compose_ui::Focus::new(), compose_ui::Focus::new());
    source.try_emit(FocusInteraction::Focus(first));
    source.try_emit(FocusInteraction::Focus(second));
    source.try_emit(FocusInteraction::Unfocus(first));
    ui.wait_for_idle();
    assert!(state.is_focused());
    source.try_emit(FocusInteraction::Unfocus(second));
    ui.wait_for_idle();
    assert!(!state.is_focused());
}

// ---- What a removed styled node leaves ------------------------------------------------------

/// A value that knows how many hold it.
#[derive(Clone)]
struct Tracked(Rc<()>);

impl PartialEq for Tracked {
    fn eq(&self, other: &Tracked) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl StyleInterpolate for Tracked {}

static TRACKED: ProvidableStyleProperty<Tracked> =
    style_property_of("review.tracked", || Tracked(Rc::new(())));

/// What's left holding a styled box's style once the box is removed: the number of holders of
/// a value its style provides, beyond the test's own two (its handle and the content's).
fn left_after_removal(press: bool, parent: bool, styled_chain: bool) -> usize {
    let tracker = Rc::new(());
    let state = MutableStyleState::new(None);
    let show = mutable_state_of(true);
    let shown = show.handle();
    let tick = mutable_state_of(0);
    let ticked = tick.handle();
    let (held, style_state) = (tracker.clone(), state.clone());
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        ticked.get();
        if !shown.get() {
            return;
        }
        let held = held.clone();
        let style_state = style_state.clone();
        let resolver = remember_observer(move || {
            let held = held.clone();
            let style = style(move |s| {
                s.provide(&TRACKED, Tracked(held.clone()));
                s.provide(&FLOAT, 0.0);
                s.pressed(|s| s.animate_with(tween(1000), |s| s.provide(&FLOAT, 100.0)));
            });
            StyleResolver::new(style, style_state)
        });
        let modifier = if styled_chain {
            styleable(Modifier.size(20.dp()), &resolver)
        } else {
            Modifier.style_resolver((*resolver).clone())
        };
        if parent {
            let parent = remember_observer(|| {
                StyleResolver::of(style(|s| s.provide(&CONTENT_FILL, Fill::Color(Color::RED))))
            });
            Box!(modifier = Modifier.style_resolver((*parent).clone()), move |_| {
                Box!(modifier = modifier.clone());
            });
        } else {
            Box!(modifier = modifier);
        }
    });
    ui.advance_time_by_frame();
    if press {
        state.set_pressed(true);
    }
    for _ in 0..5 {
        ui.advance_time_by_frame();
    }
    assert!(Rc::strong_count(&tracker) > 2, "the style holds it");
    show.set(false);
    for _ in 0..3 {
        ui.advance_time_by_frame();
    }
    tick.set(1);
    for _ in 0..3 {
        ui.advance_time_by_frame();
    }
    Rc::strong_count(&tracker) - 2
}

/// A styled box removed (alone, under a parent resolver, with the whole chain, while its
/// animation runs): its resolver, style and values are freed once the next pass frees the
/// states it retired (no cycle through the animations, the node, the parent reference or the
/// collector).
#[test]
fn a_removed_styled_node_frees_its_resolver() {
    for press in [false, true] {
        for parent in [false, true] {
            for chain in [false, true] {
                let left = left_after_removal(press, parent, chain);
                assert_eq!(left, 0, "pressed: {press}, parent: {parent}, chain: {chain}");
            }
        }
    }
}

// ---- Invalidation --------------------------------------------------------------------------

/// A box styled by `style` for `state`, counting the measurements of a layout inside the
/// style's chain and the compositions of its content.
fn counted(
    style: CommonStyle,
    state: &MutableStyleState,
    measures: Rc<std::cell::Cell<u32>>,
    compositions: Rc<std::cell::Cell<u32>>,
) -> ComposeTestRule {
    let state = state.clone();
    ComposeTestRule::sized(IntSize::new(300, 300), move || {
        let (style, state) = (style.clone(), state.clone());
        let resolver = remember_observer(move || StyleResolver::new(style, state));
        let measures = measures.clone();
        let counting = Modifier.layout(move |scope, measurable, constraints: Constraints| {
            measures.set(measures.get() + 1);
            let placeable = scope.measure(measurable, constraints);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        });
        let compositions = compositions.clone();
        Box!(modifier = styleable(Modifier, &resolver).then(counting), move |_| {
            compositions.set(compositions.get() + 1);
            Box!(modifier = Modifier.size(10.dp()));
        });
    })
}

/// A state change touching only a layer property and a drawing one (both set before: a
/// property newly set changes the set every reader observes, as Compose's map) neither
/// measures nor recomposes; a content padding change measures but doesn't recompose.
#[test]
fn state_changes_recompose_nothing_and_measure_only_for_layout_properties() {
    let state = MutableStyleState::new(None);
    let style = style(|s| {
        s.size(40.dp());
        s.background(Color::RED);
        s.alpha(0.9);
        s.pressed(|s| {
            s.background(Color::BLUE);
            s.alpha(0.5);
        });
        s.hovered(|s| s.content_padding(4.dp()));
    });
    let measures = Rc::new(std::cell::Cell::new(0));
    let compositions = Rc::new(std::cell::Cell::new(0));
    let mut ui = counted(style, &state, measures.clone(), compositions.clone());
    let (measured, composed) = (measures.get(), compositions.get());
    state.set_pressed(true);
    ui.wait_for_idle();
    assert_eq!(measures.get(), measured, "a drawing and layer change measured");
    assert_eq!(compositions.get(), composed);
    state.set_hovered(true);
    ui.wait_for_idle();
    assert!(measures.get() > measured, "a padding change didn't measure");
    assert_eq!(compositions.get(), composed);
}

// ---- The first collection doesn't animate -----------------------------------------------------

/// Frames of `FLOAT` as `resolver` reads it, bound in `content` on the manual clock: the value
/// after the first frame, then each of the next ones.
fn first_frames(resolver: StyleResolver, content: impl Fn(&StyleResolver) + 'static) -> Vec<f32> {
    let bound = resolver.clone();
    let mut ui = ComposeTestRule::with_manual_clock(move || content(&bound));
    let read = || resolver.resolve(|scope| scope.value(&FLOAT));
    let mut values = vec![read()];
    for _ in 0..5 {
        ui.advance_time_by_frame();
        values.push(read());
    }
    values
}

/// Compose's collector records animations only from its second collection on (`previous` is
/// null in the first, `doneCollect` skips them): a style showing up with a value inside
/// `animate { }` shows it at once. compose-rs's collector starts with an empty `previous`, so
/// every animated property of the first collection animates in from its default.
#[test]
#[ignore = "review: a resolver's first collection animates its animated properties in from their defaults"]
fn a_first_collection_shows_animated_values_at_once() {
    let resolver = StyleResolver::of(style(|s| {
        s.animate_with(tween(1000), |s| s.provide(&FLOAT, 100.0));
    }));
    let values = first_frames(resolver, |resolver| {
        Box!(modifier = styleable(Modifier.size(20.dp()), resolver));
    });
    assert!(values.iter().all(|value| *value == 100.0), "animated in: {values:?}");
}

/// A child collected before its parent (children attach first, D1583) collects again once the
/// parent has given its nested style: in that second collection the nested style's animated
/// value is new, and animates in from the default. Compose collects the child at its first read,
/// after the parent's modifiers have read (and collected) the parent: its first collection has
/// the nested style, and animates nothing.
#[test]
#[ignore = "review: a child collected before its parent animates its parent's nested style in"]
fn a_child_shows_its_parent_s_animated_nested_style_at_once() {
    let child = StyleResolver::of(style(|s| s.apply_nested_style(&KEY_A)));
    let values = first_frames(child, |child| {
        let parent = remember_observer(|| {
            StyleResolver::of(style(|s| {
                let nested = style(|s| s.animate_with(tween(1000), |s| s.provide(&FLOAT, 100.0)));
                s.provide_nested_style(&KEY_A, &nested);
            }))
        });
        let child = child.clone();
        Box!(modifier = styleable(Modifier, &parent), move |_| {
            Box!(modifier = styleable(Modifier.size(20.dp()), &child));
        });
    });
    assert!(values.iter().all(|value| *value == 100.0), "animated in: {values:?}");
}

// ---- Animation specs and interruptions ---------------------------------------------------------

/// A box bound to `resolver` on the manual clock.
fn on_clock(resolver: StyleResolver) -> ComposeTestRule {
    ComposeTestRule::with_manual_clock(move || {
        Box!(modifier = styleable(Modifier.size(20.dp()), &resolver));
    })
}

/// Frames until `resolver`'s `FLOAT` is `target` (300 at most).
fn frames_until(ui: &mut ComposeTestRule, resolver: &StyleResolver, target: f32) -> u32 {
    let mut frames = 0;
    while resolver.resolve(|scope| scope.value(&FLOAT)) != target && frames < 300 {
        ui.advance_time_by_frame();
        frames += 1;
    }
    frames
}

/// Into a state whose to spec is the default spring (not kept, as Compose's `recordWrite`), the
/// spec is the last state's from spec before this state's own from spec, as
/// `recordAnimations`: `toSpecs`, then `previousFromSpecs`, then `fromSpecs`.
#[test]
fn without_a_to_spec_the_last_state_s_from_spec_comes_before_this_one_s() {
    let state = MutableStyleState::new(None);
    let resolver = StyleResolver::new(
        style(|s| {
            s.provide(&FLOAT, 0.0);
            s.hovered(|s| s.animate_both(tween(100), tween(100), |s| s.provide(&FLOAT, 50.0)));
            s.pressed(|s| {
                let spring: compose_animation::AnimationSpec = compose_animation::spring().into();
                s.animate_both(spring, tween(3000), |s| s.provide(&FLOAT, 100.0));
            });
        }),
        state.clone(),
    );
    let mut ui = on_clock(resolver.clone());
    state.set_hovered(true);
    frames_until(&mut ui, &resolver, 50.0);
    state.set_hovered(false);
    state.set_pressed(true);
    let frames = frames_until(&mut ui, &resolver, 100.0);
    assert!(frames < 20, "pressed with the hover's 100 ms from spec: {frames} frames");
}

/// Interrupted late in its run, an animation turns back from the value it showed: no frame
/// shows the new start lerped by the old fraction (Compose's `Interrupted` entry reads 0).
#[test]
fn an_animation_interrupted_late_turns_back_without_a_jump() {
    let state = MutableStyleState::new(None);
    let resolver = StyleResolver::new(
        style(|s| {
            s.provide(&FLOAT, 0.0);
            s.pressed(|s| s.animate_with(tween(500), |s| s.provide(&FLOAT, 100.0)));
        }),
        state.clone(),
    );
    let mut ui = on_clock(resolver.clone());
    let read = || resolver.resolve(|scope| scope.value(&FLOAT));
    state.set_pressed(true);
    let mut values = Vec::new();
    for _ in 0..22 {
        ui.advance_time_by_frame();
        values.push(read());
    }
    assert!(values.last().is_some_and(|value| *value > 70.0 && *value < 100.0), "{values:?}");
    state.set_pressed(false);
    values.push(read());
    for _ in 0..40 {
        ui.wait_for_idle();
        values.push(read());
        ui.advance_time_by_frame();
        values.push(read());
    }
    for pair in values.windows(2) {
        assert!((pair[1] - pair[0]).abs() < 15.0, "jumped: {values:?}");
    }
    assert_eq!(*values.last().unwrap(), 0.0);
}

// ---- Mutant survivors ----------------------------------------------------------------------

static LOCAL: ProvidableStyleProperty<f32> = style_local_of("review.local", || 0.0);

/// A local a child stops setting reads its parent's value (not the default) while its slot is
/// kept, as `discardUnneededProperties`' `resolveLocal(property) ?: defaultValue()`.
#[test]
fn a_local_no_longer_set_reads_the_parent_s_value() {
    let parent = StyleResolver::of(style(|s| s.provide(&LOCAL, 10.0)));
    let state = MutableStyleState::new(None);
    state.set_pressed(true);
    let child =
        StyleResolver::new(style(|s| s.pressed(|s| s.provide(&LOCAL, 100.0))), state.clone());
    child.bind_parent_for_test(&parent);
    let density = compose_ui::Density::new(1.0);
    parent.collect_for_test(density);
    child.collect_for_test(density);
    child.resolve(|scope| assert_eq!(scope.value(&LOCAL), 100.0));
    state.set_pressed(false);
    child.collect_for_test(density);
    child.resolve(|scope| assert_eq!(scope.value(&LOCAL), 10.0));
    child.collect_for_test(density);
    child.resolve(|scope| assert_eq!(scope.value(&LOCAL), 10.0));
}

/// Listening to a source starts from no press, hover or focus (Compose's `processInteractions`
/// clears them first), whatever was set by hand before.
#[test]
fn listening_to_a_source_clears_the_interaction_flags() {
    let source = MutableInteractionSource::new();
    let state = MutableStyleState::new(Some(source.as_source().clone()));
    state.set_pressed(true);
    state.set_hovered(true);
    state.set_focused(true);
    state.set_selected(true);
    let mut ui = bound(style(|s| s.size(40.dp())), &state);
    ui.wait_for_idle();
    assert!(!state.is_pressed() && !state.is_hovered() && !state.is_focused());
    assert!(state.is_selected(), "not an interaction flag");
}

/// A hairline border pads by one pixel each side (`effectiveBorderWidth`).
#[test]
fn a_hairline_border_pads_by_a_pixel() {
    let mut ui = ComposeTestRule::sized(IntSize::new(300, 300), || {
        Box!(
            modifier = styled(Modifier.test_tag("hairline"), None, |s| {
                s.border(compose_ui::Dp::HAIRLINE, Color::RED);
            }),
            {
                Box!(modifier = Modifier.size(10.dp()));
            }
        );
    });
    let density = ui.density().density;
    let bounds = ui.on_node_with_tag("hairline").get_bounds_in_root();
    let width = bounds.right.0 - bounds.left.0;
    assert!((width - (10.0 + 2.0 / density)).abs() < 0.01, "{width}");
}
