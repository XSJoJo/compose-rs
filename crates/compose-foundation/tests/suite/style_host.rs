//! Compose's style host tests (`androidHostTest/.../style`): `CommonStyleTest`,
//! `StylePropertyTest`, `StyleStateTest` and `StyleCollectorTest`'s resolution and observation,
//! rewritten against the resolver's test hooks (`collect_for_test`, `bind_parent_for_test`).

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::style::*;
use compose_runtime::CompositionLocalAnchor;
use compose_runtime::snapshots::Snapshot;
use compose_runtime::snapshots::StateObjectId;
use compose_ui::{Brush, Color, Density, Dp, Shape, UnitExt};

static CUSTOM_BACKGROUND: ProvidableStyleProperty<Fill> =
    style_property_of("customBackground", || Fill::Color(Color::TRANSPARENT));
static CUSTOM_FLOAT: ProvidableStyleProperty<f32> =
    style_property_of_with("customFloatProperty", |a, b, t| compose_ui::lerp(*a, *b, t), || 0.0);
static CUSTOM_FLOAT_LOCAL: ProvidableStyleProperty<f32> =
    style_local_of_with("customFloatLocal", |a, b, t| compose_ui::lerp(*a, *b, t), || 0.0);
static CUSTOM_NESTED: NestedStyleKey = NestedStyleKey::new("custom");

fn custom_float(scope: &mut dyn CommonStyleScope, value: f32) {
    scope.provide(&CUSTOM_FLOAT, value);
}

fn float_local(scope: &mut dyn CommonStyleScope, value: f32) {
    scope.provide(&CUSTOM_FLOAT_LOCAL, value);
}

fn custom_background(scope: &mut dyn CommonStyleScope, fill: impl Into<Fill>) {
    scope.provide(&CUSTOM_BACKGROUND, fill.into());
}

fn style(block: impl Fn(&mut dyn CommonStyleScope) + 'static) -> CommonStyle {
    CommonStyle::new(block)
}

/// `collect(style, state) { … }`: resolves `style` for `state` and reads it.
fn collect(
    style: &CommonStyle,
    state: Option<&MutableStyleState>,
    block: impl FnOnce(&StyleResolverScope),
) {
    let state = state.cloned().unwrap_or_else(|| MutableStyleState::new(None));
    let resolver = StyleResolver::new(style.clone(), state);
    resolver.collect_for_test(Density::new(10.0));
    resolver.resolve(block);
}

/// `collectNested`: a root, a parent of `parent_style` and a child of `style` sharing a state,
/// resolved and read unpressed, pressed, then unpressed again.
fn collect_nested(
    style: CommonStyle,
    parent_style: CommonStyle,
    block: impl Fn(&StyleResolverScope, &StyleState),
) {
    let root = StyleResolver::of(CommonStyle::EMPTY);
    let state = MutableStyleState::new(None);
    let parent = StyleResolver::new(parent_style, state.clone());
    parent.bind_parent_for_test(&root);
    let child = StyleResolver::new(style, state.clone());
    child.bind_parent_for_test(&parent);
    let density = Density::new(100.0);
    let resolve = || {
        root.collect_for_test(density);
        parent.collect_for_test(density);
        child.collect_for_test(density);
        child.resolve(|scope| block(scope, &state));
    };
    resolve();
    state.set_pressed(true);
    resolve();
    state.set_pressed(false);
    resolve();
}

/// `validateObserve`: reads `read`'s value from a child unpressed, then resolves everything
/// pressed, and gives `validate` what the read observed and what the resolution changed.
fn validate_observe_with(
    style: CommonStyle,
    parent_style: CommonStyle,
    read: impl Fn(&StyleResolverScope),
    validate: impl FnOnce(&[StateObjectId], &[StateObjectId]),
) {
    let density = Density::new(100.0);
    let state = MutableStyleState::new(None);
    let root = StyleResolver::new(CommonStyle::EMPTY, state.clone());
    let parent = StyleResolver::new(parent_style, state.clone());
    parent.bind_parent_for_test(&root);
    let resolver = StyleResolver::new(style, state.clone());
    resolver.bind_parent_for_test(&parent);
    root.collect_for_test(density);
    parent.collect_for_test(density);
    resolver.collect_for_test(density);
    state.set_pressed(true);
    Snapshot::send_apply_notifications();
    let reads = Rc::new(RefCell::new(Vec::new()));
    let observed = reads.clone();
    Snapshot::observe(Some(Rc::new(move |id| observed.borrow_mut().push(id))), None, || {
        resolver.resolve(|scope| read(scope))
    });
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let handle = Snapshot::register_apply_observer(move |set, _| {
        changed.borrow_mut().extend(set.iter());
    });
    root.collect_for_test(density);
    parent.collect_for_test(density);
    resolver.collect_for_test(density);
    Snapshot::send_apply_notifications();
    drop(handle);
    validate(&reads.borrow(), &changes.borrow());
}

fn validate_observe(
    style: CommonStyle,
    parent_style: CommonStyle,
    read: impl Fn(&StyleResolverScope),
) {
    validate_observe_with(style, parent_style, read, |read, changes| {
        assert!(changes.iter().any(|change| read.contains(change)), "no change was read");
    });
}

fn read_float(scope: &StyleResolverScope) {
    scope.value(&CUSTOM_FLOAT);
}

fn read_float_local(scope: &StyleResolverScope) {
    scope.value(&CUSTOM_FLOAT_LOCAL);
}

fn read_background(scope: &StyleResolverScope) {
    scope.value(&CUSTOM_BACKGROUND);
}

// StyleCollectorTest

#[test]
fn can_read_default_custom_property() {
    collect(&CommonStyle::EMPTY, None, |scope| {
        assert_eq!(scope.value(&CUSTOM_BACKGROUND), Fill::Color(Color::TRANSPARENT));
    });
}

#[test]
fn can_read_set_custom_property() {
    collect(&style(|s| custom_background(s, Color::BLUE)), None, |scope| {
        assert_eq!(scope.value(&CUSTOM_BACKGROUND), Fill::Color(Color::BLUE));
    });
}

#[test]
fn can_read_brush_custom_property() {
    let brush = Brush::Solid(Color::BLUE);
    collect(&style(move |s| custom_background(s, brush.clone())), None, |scope| {
        assert_eq!(scope.value(&CUSTOM_BACKGROUND), Fill::Brush(Brush::Solid(Color::BLUE)));
    });
}

#[test]
fn can_set_custom_float_property() {
    collect(&style(|s| custom_float(s, 10.0)), None, |scope| {
        assert_eq!(scope.value(&CUSTOM_FLOAT), 10.0);
    });
}

#[test]
fn can_set_custom_float_property_with_state() {
    let state = MutableStyleState::new(None);
    state.set_pressed(true);
    let style = style(|s| {
        custom_float(s, 10.0);
        s.pressed(|s| custom_float(s, 100.0));
    });
    collect(&style, Some(&state), |scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 100.0));
}

#[test]
fn can_read_style_state() {
    let state = MutableStyleState::new(None);
    state.set_pressed(true);
    let style = style(|s| {
        custom_background(s, Color::RED);
        s.pressed(|s| custom_background(s, Color::BLUE));
    });
    collect(&style, Some(&state), |scope| {
        assert_eq!(scope.value(&CUSTOM_BACKGROUND), Fill::Color(Color::BLUE));
    });
}

#[test]
fn validate_property_layers() {
    let style = style(|s| {
        custom_float(s, 10.0);
        s.style_layer(|s| custom_float(s, 20.0));
        custom_float(s, 30.0);
    });
    collect(&style, None, |scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 20.0));
}

#[test]
fn validate_disjoint_property_layers() {
    let style = style(|s| {
        custom_float(s, 10.0);
        s.style_layer(|s| custom_float(s, 100.0));
        s.style_layer(|s| custom_float(s, 200.0));
        custom_float(s, 30.0);
    });
    collect(&style, None, |scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 200.0));
}

#[test]
fn validate_state_helpers_are_layers() {
    let start = style(|s| custom_float(s, 10.0));
    let pressed_hovered = style(|s| s.pressed(|s| s.hovered(|s| custom_float(s, 400.0))));
    let pressed = style(|s| s.pressed(|s| custom_float(s, 200.0)));
    let hovered = style(|s| s.hovered(|s| custom_float(s, 300.0)));
    let last = style(|s| custom_float(s, 20.0));
    let orders = [
        [&pressed_hovered, &start, &pressed, &hovered, &last],
        [&start, &pressed_hovered, &pressed, &hovered, &last],
        [&start, &pressed, &pressed_hovered, &hovered, &last],
        [&start, &pressed, &hovered, &pressed_hovered, &last],
        [&pressed_hovered, &pressed, &start, &hovered, &last],
        [&pressed, &pressed_hovered, &hovered, &start, &last],
    ];
    for order in orders {
        let style = CommonStyle::of(&order.map(|style| style.clone()));
        let expect = |state: &MutableStyleState, expected: f32| {
            collect(&style, Some(state), |scope| assert_eq!(scope.value(&CUSTOM_FLOAT), expected));
        };
        let state = MutableStyleState::new(None);
        expect(&state, 20.0);
        state.set_pressed(true);
        expect(&state, 200.0);
        state.set_hovered(true);
        expect(&state, 400.0);
        state.set_pressed(false);
        expect(&state, 300.0);
        state.set_hovered(false);
        expect(&state, 20.0);
    }
}

#[test]
fn can_observe_color_change() {
    let style = style(|s| {
        custom_background(s, Color::BLUE);
        s.pressed(|s| custom_background(s, Color::RED));
    });
    validate_observe(style, CommonStyle::EMPTY, read_background);
}

#[test]
fn can_observe_color_set() {
    let style = style(|s| s.pressed(|s| custom_background(s, Color::RED)));
    validate_observe(style, CommonStyle::EMPTY, read_background);
}

#[test]
fn validate_changing_a_property_to_same_values_does_not_report_a_change() {
    let style = style(|s| {
        custom_background(s, Color::RED);
        s.pressed(|s| custom_background(s, Color::RED));
    });
    validate_observe_with(style, CommonStyle::EMPTY, read_background, |read, changes| {
        assert!(!changes.iter().any(|change| read.contains(change)));
    });
}

#[test]
fn can_observe_float_change() {
    let style = style(|s| {
        custom_float(s, 10.0);
        s.pressed(|s| custom_float(s, 100.0));
    });
    validate_observe(style, CommonStyle::EMPTY, read_float);
}

#[test]
fn can_observe_float_local_change() {
    let style = style(|s| {
        float_local(s, 10.0);
        s.pressed(|s| float_local(s, 100.0));
    });
    validate_observe(style, CommonStyle::EMPTY, read_float_local);
}

#[test]
fn can_observe_float_local_unprovided() {
    let style = style(|s| s.pressed(|s| float_local(s, 100.0)));
    validate_observe(style, CommonStyle::EMPTY, read_float_local);
}

#[test]
fn can_observe_parent_float_local() {
    let parent = style(|s| {
        float_local(s, 10.0);
        s.pressed(|s| float_local(s, 100.0));
    });
    validate_observe(CommonStyle::EMPTY, parent, read_float_local);
}

#[test]
fn can_observe_parent_float_local_unprovided() {
    let parent = style(|s| s.pressed(|s| float_local(s, 100.0)));
    validate_observe(CommonStyle::EMPTY, parent, read_float_local);
}

#[test]
fn can_update_local() {
    let parent = style(|s| {
        float_local(s, 10.0);
        s.pressed(|s| float_local(s, 100.0));
    });
    collect_nested(CommonStyle::EMPTY, parent, |scope, state| {
        let expected = if state.is_pressed() { 100.0 } else { 10.0 };
        assert_eq!(scope.value(&CUSTOM_FLOAT_LOCAL), expected);
    });
}

#[test]
fn can_unprovide_local() {
    let parent = style(|s| s.pressed(|s| float_local(s, 100.0)));
    collect_nested(CommonStyle::EMPTY, parent, |scope, state| {
        let expected = if state.is_pressed() { 100.0 } else { 0.0 };
        assert_eq!(scope.value(&CUSTOM_FLOAT_LOCAL), expected);
    });
}

#[test]
fn can_provide_nested_style() {
    let parent = style(|s| {
        custom_float(s, 100.0);
        s.provide_nested_style(&CUSTOM_NESTED, &style(|s| custom_float(s, 10.0)));
    });
    let child = style(|s| s.apply_nested_style(&CUSTOM_NESTED));
    collect(&parent, None, |scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 100.0));
    collect_nested(child, parent, |scope, _| assert_eq!(scope.value(&CUSTOM_FLOAT), 10.0));
}

#[test]
fn can_observe_changes_in_parent_nested_style() {
    let child = || style(|s| s.apply_nested_style(&CUSTOM_NESTED));
    let parent = style(|s| {
        custom_float(s, 10.0);
        let nested = style(|s| s.pressed(|s| custom_float(s, 100.0)));
        s.provide_nested_style(&CUSTOM_NESTED, &nested);
    });
    validate_observe(child(), parent, read_float);
    let parent = style(|s| {
        custom_float(s, 100.0);
        s.pressed(|s| {
            s.provide_nested_style(&CUSTOM_NESTED, &style(|s| custom_float(s, 10.0)));
        });
    });
    validate_observe(child(), parent, read_float);
}

/// compose-rs's (D1583): a write after a style gave a nested style still counts, and a nested
/// style given before isn't given after it stops being given.
#[test]
fn writes_after_providing_a_nested_style_count_on_every_resolution() {
    let parent = style(|s| {
        s.provide_nested_style(&CUSTOM_NESTED, &style(|s| custom_float(s, 10.0)));
        custom_float(s, 100.0);
    });
    let resolver = StyleResolver::of(parent);
    resolver.collect_for_test(Density::new(1.0));
    resolver.collect_for_test(Density::new(1.0));
    resolver.resolve(|scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 100.0));
}

#[test]
fn a_nested_style_no_longer_given_no_longer_applies() {
    let state = MutableStyleState::new(None);
    state.set_pressed(true);
    let parent = StyleResolver::new(
        style(|s| {
            s.pressed(|s| {
                s.provide_nested_style(&CUSTOM_NESTED, &style(|s| custom_float(s, 10.0)));
            });
        }),
        state.clone(),
    );
    let child = StyleResolver::of(style(|s| s.apply_nested_style(&CUSTOM_NESTED)));
    child.bind_parent_for_test(&parent);
    let density = Density::new(1.0);
    parent.collect_for_test(density);
    child.collect_for_test(density);
    child.resolve(|scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 10.0));
    state.set_pressed(false);
    parent.collect_for_test(density);
    child.collect_for_test(density);
    child.resolve(|scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 0.0));
}

#[test]
fn a_property_no_longer_set_reads_its_default_and_is_dropped_after() {
    let state = MutableStyleState::new(None);
    state.set_pressed(true);
    let resolver =
        StyleResolver::new(style(|s| s.pressed(|s| custom_float(s, 5.0))), state.clone());
    let density = Density::new(1.0);
    resolver.collect_for_test(density);
    resolver.resolve(|scope| assert!(scope.is_set(&CUSTOM_FLOAT)));
    state.set_pressed(false);
    resolver.collect_for_test(density);
    resolver.resolve(|scope| {
        // Kept for one resolution (it may animate out), at its default.
        assert!(scope.is_set(&CUSTOM_FLOAT));
        assert_eq!(scope.value(&CUSTOM_FLOAT), 0.0);
    });
    resolver.collect_for_test(density);
    resolver.resolve(|scope| assert!(!scope.is_set(&CUSTOM_FLOAT)));
}

#[test]
#[should_panic(expected = "Unbound style resolver")]
fn reading_an_unbound_resolver_panics() {
    StyleResolver::of(CommonStyle::EMPTY).resolve(|_| ());
}

// CommonStyleTest

fn style_test(
    expected: &[&str],
    make: impl FnOnce(&Rc<RefCell<Vec<&'static str>>>) -> CommonStyle,
) {
    let result = Rc::new(RefCell::new(Vec::new()));
    let style = make(&result);
    let resolver = StyleResolver::of(style.clone());
    resolver.collect_for_test(Density::new(1.0));
    assert_eq!(*result.borrow(), expected);
    assert_eq!(style.combined_count_for_test(), expected.len());
    if expected.is_empty() {
        assert_eq!(style, CommonStyle::EMPTY);
    }
}

fn adding(result: &Rc<RefCell<Vec<&'static str>>>, name: &'static str) -> CommonStyle {
    let result = result.clone();
    CommonStyle::new(move |_| result.borrow_mut().push(name))
}

#[test]
fn test_one_style() {
    style_test(&["A"], |result| adding(result, "A"));
}

#[test]
fn test_two_styles() {
    style_test(&["A", "B"], |result| CommonStyle::of(&[adding(result, "A"), adding(result, "B")]));
}

#[test]
fn test_three_styles() {
    style_test(&["A", "B", "C"], |result| {
        CommonStyle::of(&[adding(result, "A"), adding(result, "B"), adding(result, "C")])
    });
}

#[test]
fn test_four_styles() {
    style_test(&["A", "B", "C", "D"], |result| {
        let (a, b, c, d) =
            (adding(result, "A"), adding(result, "B"), adding(result, "C"), adding(result, "D"));
        CommonStyle::of(&[a, b, c, d])
    });
}

#[test]
fn test_empty() {
    style_test(&[], |_| CommonStyle::of(&[CommonStyle::EMPTY, CommonStyle::EMPTY]));
}

#[test]
fn test_empty_first() {
    style_test(&["A"], |result| CommonStyle::of(&[adding(result, "A"), CommonStyle::EMPTY]));
}

#[test]
fn test_empty_second() {
    style_test(&["B"], |result| CommonStyle::of(&[CommonStyle::EMPTY, adding(result, "B")]));
}

#[test]
fn then_flattens_and_skips_the_empty_style() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let (a, b, c) = (adding(&result, "A"), adding(&result, "B"), adding(&result, "C"));
    assert_eq!(CommonStyle::EMPTY.then(&a), a);
    assert_eq!(a.then(&CommonStyle::EMPTY), a);
    let all = a.then(&b).then(&c);
    assert_eq!(all.combined_count_for_test(), 3);
    let resolver = StyleResolver::of(all);
    resolver.collect_for_test(Density::new(1.0));
    assert_eq!(*result.borrow(), ["A", "B", "C"]);
}

// StylePropertyTest

/// A scope keeping what's provided and reading it back, as the test's `SimpleScope`: the
/// traits are an app's to implement.
#[derive(Default)]
struct SimpleScope {
    values: Vec<(ErasedProperty, Rc<dyn std::any::Any>)>,
}

impl CustomStyleScope for SimpleScope {
    fn density(&self) -> Density {
        Density::new(100.0)
    }
    fn composition_locals(&self) -> Option<CompositionLocalAnchor> {
        None
    }
    fn provide_erased(&mut self, property: ErasedProperty, value: Rc<dyn std::any::Any>) {
        self.values.retain(|(set, _)| !set.same_property(&property));
        self.values.push((property, value));
    }
}

impl StylePropertyAccessorScope for SimpleScope {
    fn get_or_null_erased(&self, property: ErasedProperty) -> Option<Rc<dyn std::any::Any>> {
        self.values.iter().find(|(set, _)| set.same_property(&property)).map(|(_, v)| v.clone())
    }
    fn is_set_erased(&self, property: ErasedProperty) -> bool {
        self.get_or_null_erased(property).is_some()
    }
}

fn scope(block: impl FnOnce(&mut SimpleScope)) {
    block(&mut SimpleScope::default());
}

#[test]
fn can_create_and_use_int_property() {
    static INT: ProvidableStyleProperty<i32> = style_property_of("intProperty", || 0);
    scope(|scope| {
        assert_eq!(scope.value(&INT), 0);
        scope.provide(&INT, 10);
        assert_eq!(scope.value(&INT), 10);
    });
}

#[test]
fn can_animate_an_int_property() {
    static INT: ProvidableStyleProperty<i32> =
        style_property_of_with("int", |a, b, t| compose_ui::lerp(*a, *b, t), || 0);
    assert_eq!(INT.lerp(&0, &100, 0.0), 0);
    assert_eq!(INT.lerp(&0, &100, 1.0), 100);
    let middle = INT.lerp(&0, &100, 0.5);
    assert!(middle > 49 && middle < 51);
}

#[test]
fn can_create_and_use_long_property() {
    static LONG: ProvidableStyleProperty<i64> = style_property_of("longProperty", || 0);
    scope(|scope| {
        assert_eq!(scope.value(&LONG), 0);
        scope.provide(&LONG, 10);
        assert_eq!(scope.value(&LONG), 10);
    });
}

#[test]
fn can_animate_a_long_property() {
    static LONG: ProvidableStyleProperty<i64> =
        style_property_of_with("long", |a, b, t| compose_ui::lerp(*a, *b, t), || 0);
    assert_eq!(LONG.lerp(&0, &100, 0.0), 0);
    assert_eq!(LONG.lerp(&0, &100, 1.0), 100);
    let middle = LONG.lerp(&0, &100, 0.5);
    assert!(middle > 49 && middle < 51);
}

#[test]
fn can_create_and_use_float_property() {
    static FLOAT: ProvidableStyleProperty<f32> = style_property_of("floatProperty", || 0.0);
    scope(|scope| {
        assert_eq!(scope.value(&FLOAT), 0.0);
        scope.provide(&FLOAT, 10.0);
        assert_eq!(scope.value(&FLOAT), 10.0);
    });
}

#[test]
fn can_animate_a_float_property() {
    static FLOAT: ProvidableStyleProperty<f32> =
        style_property_of_with("float", |a, b, t| compose_ui::lerp(*a, *b, t), || 0.0);
    assert_eq!(FLOAT.lerp(&0.0, &100.0, 0.0), 0.0);
    assert_eq!(FLOAT.lerp(&0.0, &100.0, 1.0), 100.0);
    let middle = FLOAT.lerp(&0.0, &100.0, 0.5);
    assert!(middle > 49.0 && middle < 51.0);
}

#[test]
fn can_animate_a_float_with_default_lerp() {
    static FLOAT: ProvidableStyleProperty<f32> = style_property_of("test property", || 0.0);
    assert_eq!(FLOAT.lerp(&0.0, &100.0, 0.0), 0.0);
    assert_eq!(FLOAT.lerp(&0.0, &100.0, 1.0), 100.0);
    let middle = FLOAT.lerp(&0.0, &100.0, 0.5);
    assert!(middle > 49.0 && middle < 51.0);
}

#[test]
fn can_animate_a_dp_with_default_lerp() {
    static DP: ProvidableStyleProperty<Dp> = style_property_of("test property", || 0.dp());
    assert_eq!(DP.lerp(&0.dp(), &100.dp(), 0.0), 0.dp());
    assert_eq!(DP.lerp(&0.dp(), &100.dp(), 1.0), 100.dp());
    let middle = DP.lerp(&0.dp(), &100.dp(), 0.5);
    assert!(middle.0 > 49.0 && middle.0 < 51.0);
}

#[test]
fn can_create_and_use_object_property() {
    static SHAPE: ProvidableStyleProperty<Shape> =
        style_property_of("shape", || compose_foundation::shape::CIRCLE_SHAPE);
    scope(|scope| {
        assert_eq!(scope.value(&SHAPE), compose_foundation::shape::CIRCLE_SHAPE);
        scope.provide(&SHAPE, Shape::Rectangle);
        assert_eq!(scope.value(&SHAPE), Shape::Rectangle);
    });
}

#[test]
fn a_type_without_interpolation_jumps_half_way() {
    static FLAG: ProvidableStyleProperty<bool> = style_property_of("flag", || false);
    assert!(!FLAG.lerp(&false, &true, 0.49));
    assert!(FLAG.lerp(&false, &true, 0.5));
}

#[test]
fn fills_breadths_and_shadows_lerp_as_compose_s() {
    let blue = Fill::Color(Color::BLUE);
    let red_brush = Fill::Brush(Brush::Solid(Color::RED));
    assert_eq!(Fill::style_lerp(&blue, &red_brush, 0.0), Fill::Brush(Brush::Solid(Color::BLUE)));
    assert_eq!(Fill::style_lerp(&blue, &red_brush, 1.0), red_brush);
    assert_eq!(Fill::style_lerp(&blue, &Fill::Color(Color::RED), 1.0), Fill::Color(Color::RED));
    assert_eq!(
        Breadth::style_lerp(&Breadth::None, &Breadth::Distance(10.dp()), 0.5),
        Breadth::Distance(5.dp())
    );
    assert_eq!(
        Breadth::style_lerp(&Breadth::Fraction(1.0), &Breadth::None, 0.5),
        Breadth::Fraction(0.5)
    );
    assert_eq!(
        Breadth::style_lerp(&Breadth::Fraction(1.0), &Breadth::Distance(10.dp()), 0.4),
        Breadth::Fraction(1.0)
    );
    assert_eq!(lerp_dp(Dp(-10.0), 10.dp(), 0.5), 5.dp());
}

// StyleStateTest

#[derive(Default)]
struct Observed {
    read: Vec<StateObjectId>,
    written: Vec<StateObjectId>,
    changed: Vec<StateObjectId>,
}

fn observe(block: impl FnOnce()) -> Observed {
    Snapshot::send_apply_notifications();
    let read = Rc::new(RefCell::new(Vec::new()));
    let written = Rc::new(RefCell::new(Vec::new()));
    let changed = Rc::new(RefCell::new(Vec::new()));
    let changes = changed.clone();
    let handle = Snapshot::register_apply_observer(move |set, _| {
        changes.borrow_mut().extend(set.iter());
    });
    let (reads, writes) = (read.clone(), written.clone());
    Snapshot::observe(
        Some(Rc::new(move |id| reads.borrow_mut().push(id))),
        Some(Rc::new(move |id| writes.borrow_mut().push(id))),
        block,
    );
    Snapshot::send_apply_notifications();
    drop(handle);
    Observed { read: read.take(), written: written.take(), changed: changed.take() }
}

static CUSTOM_STATE_KEY: StyleStateKey<i32> = StyleStateKey::new("customState", || 0);

#[test]
fn test_predefined_defaults() {
    let state = MutableStyleState::new(None);
    assert!(!state.is_hovered());
    assert!(!state.is_pressed());
    assert!(!state.is_focused());
    assert!(!state.is_selected());
    assert!(!state.is_checked());
    assert!(state.is_enabled());
    assert_eq!(state.tri_state_toggle(), compose_ui::ToggleableState::Off);
}

#[test]
fn test_predefined_states_set() {
    let state = MutableStyleState::new(None);
    state.set_hovered(true);
    state.set_pressed(true);
    state.set_focused(true);
    state.set_selected(true);
    state.set_checked(true);
    assert!(state.is_hovered());
    assert!(state.is_pressed());
    assert!(state.is_focused());
    assert!(state.is_selected());
    assert!(state.is_checked());
}

#[test]
fn test_predefined_states_partial_set() {
    let state = MutableStyleState::new(None);
    state.set_pressed(true);
    state.set_selected(true);
    assert!(!state.is_hovered());
    assert!(state.is_pressed());
    assert!(!state.is_focused());
    assert!(state.is_selected());
    assert!(!state.is_checked());
}

#[test]
fn test_observe_predefined_read() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| assert!(!state.is_pressed()));
    assert!(!observed.read.is_empty());
}

#[test]
fn test_observe_predefined_written() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| state.set_pressed(true));
    assert!(!observed.written.is_empty());
}

#[test]
fn test_observe_predefined_changed() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| state.set_pressed(true));
    assert!(!observed.changed.is_empty());
    assert!(observed.read.is_empty());
}

#[test]
fn test_observe_predefined_unchanged() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| state.set_pressed(false));
    assert!(observed.changed.is_empty());
    assert!(observed.read.is_empty());
}

#[test]
fn test_observe_predefined_read_and_write_matches_changed() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| {
        assert!(!state.is_pressed());
        state.set_pressed(true);
        assert!(state.is_pressed());
    });
    assert!(!observed.read.is_empty());
    assert!(observed.changed.iter().all(|id| observed.read.contains(id)));
    assert!(observed.written.iter().all(|id| observed.changed.contains(id)));
}

#[test]
fn test_custom_state_set() {
    let state = MutableStyleState::new(None);
    state.set(&CUSTOM_STATE_KEY, 10);
    assert_eq!(state.get(&CUSTOM_STATE_KEY), 10);
    state.remove(&CUSTOM_STATE_KEY);
    assert_eq!(state.get(&CUSTOM_STATE_KEY), 0);
}

#[test]
fn test_observe_custom_state_read() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| assert_eq!(state.get(&CUSTOM_STATE_KEY), 0));
    assert!(!observed.read.is_empty());
}

#[test]
fn test_observe_custom_state_written() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| {
        let value = state.get(&CUSTOM_STATE_KEY);
        state.set(&CUSTOM_STATE_KEY, value + 1);
    });
    assert!(!observed.written.is_empty());
}

#[test]
fn test_observe_custom_state_changed() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| state.set(&CUSTOM_STATE_KEY, 100));
    assert!(!observed.changed.is_empty());
    assert!(observed.read.is_empty());
}

#[test]
fn test_observe_custom_read_and_write_matches_changed() {
    let state = MutableStyleState::new(None);
    let observed = observe(|| {
        assert_eq!(state.get(&CUSTOM_STATE_KEY), 0);
        state.set(&CUSTOM_STATE_KEY, 10);
        assert_eq!(state.get(&CUSTOM_STATE_KEY), 10);
    });
    assert!(!observed.read.is_empty());
    assert!(observed.changed.iter().all(|id| observed.read.contains(id)));
    assert!(observed.written.iter().all(|id| observed.changed.contains(id)));
}

#[test]
#[should_panic(expected = "Cannot remove an internal StyleStateKey")]
fn removing_a_predefined_key_panics() {
    MutableStyleState::new(None).remove(&StyleStateKey::PRESSED);
}

#[test]
fn state_scope_helpers_read_their_states() {
    let state = MutableStyleState::new(None);
    let style = style(|s| {
        s.checked(|s| custom_float(s, 1.0));
        s.selected(|s| custom_float(s, 2.0));
        s.disabled(|s| custom_float(s, 3.0));
        s.tri_state_toggle_indeterminate(|s| custom_float(s, 4.0));
        s.focused(|s| custom_float(s, 5.0));
        s.state(&StyleStateKey::SELECTED, |s| float_local(s, 6.0));
        s.state_when(&CUSTOM_STATE_KEY, |key, state| state.get(key) > 1, |s| float_local(s, 7.0));
    });
    let value = |state: &MutableStyleState| {
        let mut result = (0.0, 0.0);
        collect(&style, Some(state), |scope| {
            result = (scope.value(&CUSTOM_FLOAT), scope.value(&CUSTOM_FLOAT_LOCAL));
        });
        result
    };
    assert_eq!(value(&state), (0.0, 0.0));
    state.set_checked(true);
    assert_eq!(value(&state), (1.0, 0.0));
    state.set_selected(true);
    assert_eq!(value(&state), (2.0, 6.0));
    state.set_enabled(false);
    assert_eq!(value(&state), (3.0, 6.0));
    state.set_tri_state_toggle(compose_ui::ToggleableState::Indeterminate);
    assert_eq!(value(&state), (4.0, 6.0));
    state.set_focused(true);
    state.set(&CUSTOM_STATE_KEY, 2);
    assert_eq!(value(&state), (5.0, 7.0));
}

/// Applying a nested style applies what the ancestor gives for the key, not the rest of its
/// style (Compose's collector ignores the ancestor's own writes there).
#[test]
fn applying_a_nested_style_takes_only_the_nested_style() {
    let parent = style(|s| {
        custom_float(s, 100.0);
        float_local(s, 5.0);
        s.provide_nested_style(&CUSTOM_NESTED, &style(|s| custom_background(s, Color::RED)));
    });
    let child = style(|s| s.apply_nested_style(&CUSTOM_NESTED));
    collect_nested(child, parent, |scope, _| {
        assert_eq!(scope.value(&CUSTOM_FLOAT), 0.0);
        assert!(!scope.is_set(&CUSTOM_FLOAT));
        assert_eq!(scope.value(&CUSTOM_BACKGROUND), Fill::Color(Color::RED));
    });
}

/// The nested styles of every ancestor giving the key apply, the outermost first, so the
/// nearest wins.
#[test]
fn nested_styles_apply_outermost_first() {
    let giving = |value: f32| {
        StyleResolver::of(style(move |s| {
            s.provide_nested_style(&CUSTOM_NESTED, &style(move |s| custom_float(s, value)));
        }))
    };
    let grandparent = giving(1.0);
    let parent = giving(2.0);
    parent.bind_parent_for_test(&grandparent);
    let child = StyleResolver::of(style(|s| s.apply_nested_style(&CUSTOM_NESTED)));
    child.bind_parent_for_test(&parent);
    for resolver in [&grandparent, &parent, &child] {
        resolver.collect_for_test(Density::new(1.0));
    }
    child.resolve(|scope| assert_eq!(scope.value(&CUSTOM_FLOAT), 2.0));
}
