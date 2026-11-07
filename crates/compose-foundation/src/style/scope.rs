//! Styles and the scopes they run in, as Compose's `CommonStyle.kt`, `CustomStyle.kt`,
//! `NestedStyles.kt`, `StyleLayerScope.kt` and the non-deprecated parts of `StyleScope.kt` and
//! `StyleState.kt` (`state`, `pressed`, …, `animate`, `apply`).

use std::fmt;
use std::rc::Rc;

use compose_animation::AnimationSpec;
use compose_runtime::{ComposeArg, CompositionLocal, CompositionLocalAnchor};
use compose_ui::{Brush, Color, Density, ToggleableState};

use super::property::{
    AnyStyleProperty, ErasedProperty, ProvidableStyleProperty, StyleProperty, Value,
};
use super::state::{StyleState, StyleStateKey};

/// Where a style sets properties, as Compose's `CustomStyleScope` (with
/// `StylePropertyProviderScope`): a design system's own scope builds on it. Its generic helpers
/// are [`CustomStyleScopeExt`]'s.
pub trait CustomStyleScope {
    /// The density, as the scope's `Density`.
    fn density(&self) -> Density;

    /// Where composition locals are read from (`None`: their defaults), as
    /// `CompositionLocalAccessorScope`.
    fn composition_locals(&self) -> Option<CompositionLocalAnchor>;

    /// Sets `property` to `value` (a value of its type), as `provide`.
    #[doc(hidden)]
    fn provide_erased(&mut self, property: ErasedProperty, value: Value);
}

/// The generic helpers of every [`CustomStyleScope`].
pub trait CustomStyleScopeExt: CustomStyleScope {
    /// Sets `property` to `value`, as `property.provide(value)`.
    fn provide<T: Clone + PartialEq + 'static>(
        &mut self,
        property: &'static ProvidableStyleProperty<T>,
        value: T,
    ) {
        self.provide_erased(property.erased(), Rc::new(value));
    }

    /// The value of `local` where the style is resolved, as `local.currentValue`.
    fn current_value<T: Clone + 'static>(&self, local: &CompositionLocal<T>) -> T {
        match self.composition_locals() {
            Some(anchor) => anchor.get(local),
            None => local.current(),
        }
    }
}

impl<S: CustomStyleScope + ?Sized> CustomStyleScopeExt for S {}

/// A block run in a scope, handed the scope (a Rust closure can't hold it while the scope runs
/// it).
pub type StyleBlock<'a> = &'a mut dyn FnMut(&mut dyn CommonStyleScope);

/// Where a [`CommonStyle`] runs, as Compose's `CommonStyleScope`: its `CustomStyleScope`,
/// `AnimateStyleScope`, `StyleStateScope`, `NestedStyleProviderStyleScope`,
/// `NestedStyleConsumerStyleScope` and `StyleLayerScope` in one object-safe trait (D1582). Its
/// generic helpers (`state`, `pressed`, `animate`, …) are [`CommonStyleScopeExt`]'s.
pub trait CommonStyleScope: CustomStyleScope {
    /// The state the style is resolved for, as `StyleStateScope.state`.
    fn style_state(&self) -> StyleState;

    /// Runs `block` if `active`, in a layer (as `StyleStateScope.state(key, block, active)`
    /// with `active(key, state)` already asked).
    fn state_dyn(&mut self, active: bool, block: StyleBlock<'_>);

    /// Runs `block` one layer deeper, as `styleLayer`: what it sets wins over what's set
    /// shallower, before or after.
    fn style_layer_dyn(&mut self, block: StyleBlock<'_>);

    /// Runs `block` with what it sets animated, into this state with `to_spec` and out of it
    /// with `from_spec`, as `animate(toSpec, fromSpec)`.
    fn animate_dyn(
        &mut self,
        to_spec: AnimationSpec,
        from_spec: AnimationSpec,
        block: StyleBlock<'_>,
    );

    /// Gives the resolvers below `style` for `key`, as `provideNestedStyle`.
    fn provide_nested_style(&mut self, key: &NestedStyleKey, style: &CommonStyle);

    /// Applies the styles the resolvers above give for `key`, outermost first, as
    /// `applyNestedStyle`.
    fn apply_nested_style(&mut self, key: &NestedStyleKey);
}

/// The generic helpers of every [`CommonStyleScope`].
pub trait CommonStyleScopeExt: CommonStyleScope {
    /// Runs `block` when `active(key, state)`, in a layer, as
    /// `state(key, block, active)`.
    fn state_when<T: 'static>(
        &mut self,
        key: &StyleStateKey<T>,
        active: impl FnOnce(&StyleStateKey<T>, &StyleState) -> bool,
        mut block: impl FnMut(&mut dyn CommonStyleScope),
    ) {
        let is_active = active(key, &self.style_state());
        self.state_dyn(is_active, &mut block);
    }

    /// Runs `block` when `key` is true, as `state(key, block)`.
    fn state(&mut self, key: &StyleStateKey<bool>, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(key, |key, state| state.get(key), block);
    }

    /// Runs `block` while pressed, as `pressed { }`.
    fn pressed(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(&StyleStateKey::PRESSED, |_, state| state.is_pressed(), block);
    }

    /// Runs `block` while hovered, as `hovered { }`.
    fn hovered(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(&StyleStateKey::HOVERED, |_, state| state.is_hovered(), block);
    }

    /// Runs `block` while focused, as `focused { }`.
    fn focused(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(&StyleStateKey::FOCUSED, |_, state| state.is_focused(), block);
    }

    /// Runs `block` while selected, as `selected { }`.
    fn selected(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(&StyleStateKey::SELECTED, |_, state| state.is_selected(), block);
    }

    /// Runs `block` while checked (the toggle on), as `checked { }`.
    fn checked(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(&StyleStateKey::TOGGLE, |_, state| state.is_checked(), block);
    }

    /// Runs `block` while disabled, as `disabled { }`.
    fn disabled(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(&StyleStateKey::ENABLED, |_, state| !state.is_enabled(), block);
    }

    /// Runs `block` while the toggle is on, as `triStateToggleOn { }`.
    fn tri_state_toggle_on(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(
            &StyleStateKey::TOGGLE,
            |_, state| state.tri_state_toggle() == ToggleableState::On,
            block,
        );
    }

    /// Runs `block` while the toggle is off, as `triStateToggleOff { }`.
    fn tri_state_toggle_off(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(
            &StyleStateKey::TOGGLE,
            |_, state| state.tri_state_toggle() == ToggleableState::Off,
            block,
        );
    }

    /// Runs `block` while the toggle is indeterminate, as `triStateToggleIndeterminate { }`.
    fn tri_state_toggle_indeterminate(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.state_when(
            &StyleStateKey::TOGGLE,
            |_, state| state.tri_state_toggle() == ToggleableState::Indeterminate,
            block,
        );
    }

    /// Runs `block` one layer deeper, as `styleLayer { }`.
    fn style_layer(&mut self, mut block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.style_layer_dyn(&mut block);
    }

    /// Runs `block` with what it sets animated by a spring both ways, as `animate { }`.
    fn animate(&mut self, block: impl FnMut(&mut dyn CommonStyleScope)) {
        self.animate_with(super::animations::default_spring_spec(), block);
    }

    /// Runs `block` with what it sets animated by `spec` both ways, as `animate(spec) { }`.
    fn animate_with(
        &mut self,
        spec: impl Into<AnimationSpec>,
        block: impl FnMut(&mut dyn CommonStyleScope),
    ) {
        let spec = spec.into();
        self.animate_both(spec.clone(), spec, block);
    }

    /// As `animate(toSpec, fromSpec) { }`.
    fn animate_both(
        &mut self,
        to_spec: impl Into<AnimationSpec>,
        from_spec: impl Into<AnimationSpec>,
        mut block: impl FnMut(&mut dyn CommonStyleScope),
    ) {
        self.animate_dyn(to_spec.into(), from_spec.into(), &mut block);
    }
}

impl<S: CommonStyleScope + ?Sized> CommonStyleScopeExt for S {}

impl dyn CommonStyleScope + '_ {
    /// Applies `style` here, as `apply(style)`.
    pub fn apply(&mut self, style: &CommonStyle) {
        style.apply_style(self);
    }
}

/// What a style or resolver reads properties through, as Compose's
/// `StylePropertyAccessorScope`. Its generic helpers are [`StylePropertyAccessorScopeExt`]'s.
pub trait StylePropertyAccessorScope {
    /// `property`'s value if set (a local: here or above), as `getOrNull`, erased.
    #[doc(hidden)]
    fn get_or_null_erased(&self, property: ErasedProperty) -> Option<Value>;

    /// Whether `property` is set (a local: here or above), as `isSet`, erased.
    #[doc(hidden)]
    fn is_set_erased(&self, property: ErasedProperty) -> bool;
}

/// The generic helpers of every [`StylePropertyAccessorScope`].
pub trait StylePropertyAccessorScopeExt: StylePropertyAccessorScope {
    /// `property`'s value, its default if unset, as `property.value`.
    fn value<T: Clone + PartialEq + 'static>(&self, property: &'static StyleProperty<T>) -> T {
        self.get_or_null(property).unwrap_or_else(|| property.default_value())
    }

    /// Whether `property` is set, as `property.isSet`.
    fn is_set<T: Clone + PartialEq + 'static>(&self, property: &'static StyleProperty<T>) -> bool {
        self.is_set_erased(property.erased())
    }

    /// `property`'s value if set, as `getOrNull(property)`.
    fn get_or_null<T: Clone + PartialEq + 'static>(
        &self,
        property: &'static StyleProperty<T>,
    ) -> Option<T> {
        let value = self.get_or_null_erased(property.erased())?;
        value.downcast_ref::<T>().cloned()
    }

    /// Whether any of `properties` is set, as `anySet`.
    fn any_set(&self, properties: &[&'static dyn AnyStyleProperty]) -> bool {
        properties.iter().any(|property| self.is_set_erased(property.erased()))
    }

    /// Runs `block` with `property`'s value if it's set, as `ifSet`.
    fn if_set<T: Clone + PartialEq + 'static>(
        &self,
        property: &'static StyleProperty<T>,
        block: impl FnOnce(T),
    ) {
        if let Some(value) = self.get_or_null(property) {
            block(value);
        }
    }

    /// `property`'s value if set, else `default_value()`, as `getOrElse`.
    fn get_or_else<T: Clone + PartialEq + 'static>(
        &self,
        property: &'static StyleProperty<T>,
        default_value: impl FnOnce() -> T,
    ) -> T {
        self.get_or_null(property).unwrap_or_else(default_value)
    }

    /// The content's fill, as `contentFill`: the content fill local.
    fn content_fill(&self) -> super::Fill {
        self.value(&super::properties::CONTENT_FILL)
    }

    /// The content's color, as `contentColor`: its fill's color.
    fn content_color(&self) -> Color {
        self.content_fill().as_color()
    }

    /// The content's brush, as `contentBrush`: its fill's brush.
    fn content_brush(&self) -> Option<Brush> {
        self.content_fill().as_brush()
    }
}

impl<S: StylePropertyAccessorScope + ?Sized> StylePropertyAccessorScopeExt for S {}

/// A style a design system's own scope runs, as Compose's `CustomStyle<ScopeT>`.
pub trait CustomStyle<S: ?Sized> {
    /// Sets the style's properties in `scope`, as `applyStyle`.
    fn apply_style(&self, scope: &mut S);
}

/// What a [`CommonStyle`] runs.
type StyleFn = dyn Fn(&mut dyn CommonStyleScope);

/// How a [`CommonStyle`] is made.
#[derive(Clone)]
enum StyleKind {
    /// No properties: Compose's `CommonStyle` companion.
    Empty,
    /// A block.
    Block(Rc<StyleFn>),
    /// Two styles, as `DualCommonStyle`.
    Dual(Rc<(CommonStyle, CommonStyle)>),
    /// More, as `CombinedCommonStyle`.
    Combined(Rc<[CommonStyle]>),
}

/// A style: what it sets in a [`CommonStyleScope`], as Compose's `CommonStyle`. A handle,
/// cloned to share, compared by identity. [`EMPTY`](Self::EMPTY) sets nothing; combined
/// styles (`CommonStyle::of`, [`then`](Self::then)) flatten into one list.
///
/// ```ignore
/// let style = CommonStyle::new(|s| {
///     s.provide(&BACKGROUND, Fill::Color(Color::RED));
///     s.pressed(|s| s.provide(&BACKGROUND, Fill::Color(Color::BLUE)));
/// });
/// ```
#[derive(Clone)]
pub struct CommonStyle(StyleKind);

impl CommonStyle {
    /// The style that sets nothing, as the `CommonStyle` companion.
    pub const EMPTY: CommonStyle = CommonStyle(StyleKind::Empty);

    /// The style `block` sets, as `CommonStyle { … }`.
    pub fn new(block: impl Fn(&mut dyn CommonStyleScope) + 'static) -> CommonStyle {
        CommonStyle(StyleKind::Block(Rc::new(block)))
    }

    /// `styles` one after the other, as `CommonStyle(style1, style2, …)`: flattened, empty
    /// ones dropped; one style left is that style, none [`EMPTY`](Self::EMPTY).
    pub fn of(styles: &[CommonStyle]) -> CommonStyle {
        let mut flat: Vec<CommonStyle> = Vec::new();
        for style in styles {
            match &style.0 {
                StyleKind::Empty => {}
                StyleKind::Dual(pair) => {
                    flat.push(pair.0.clone());
                    flat.push(pair.1.clone());
                }
                StyleKind::Combined(all) => flat.extend(all.iter().cloned()),
                StyleKind::Block(_) => flat.push(style.clone()),
            }
        }
        match flat.len() {
            0 => CommonStyle::EMPTY,
            1 => flat.pop().unwrap_or(CommonStyle::EMPTY),
            2 => {
                let second = flat.pop().unwrap_or(CommonStyle::EMPTY);
                let first = flat.pop().unwrap_or(CommonStyle::EMPTY);
                CommonStyle(StyleKind::Dual(Rc::new((first, second))))
            }
            _ => CommonStyle(StyleKind::Combined(flat.into())),
        }
    }

    /// This style then `other`, as `this then other`.
    pub fn then(&self, other: &CommonStyle) -> CommonStyle {
        match (&self.0, &other.0) {
            (StyleKind::Empty, _) => other.clone(),
            (_, StyleKind::Empty) => self.clone(),
            (StyleKind::Block(_), StyleKind::Block(_)) => {
                CommonStyle(StyleKind::Dual(Rc::new((self.clone(), other.clone()))))
            }
            _ => CommonStyle::of(&[self.clone(), other.clone()]),
        }
    }

    /// Whether it's the empty style.
    pub fn is_empty(&self) -> bool {
        matches!(self.0, StyleKind::Empty)
    }

    /// How many styles it combines: 0 empty, 1 a block, 2 or more combined. For tests (Compose's
    /// test checks its `DualCommonStyle` and `CombinedCommonStyle`).
    #[doc(hidden)]
    pub fn combined_count_for_test(&self) -> usize {
        match &self.0 {
            StyleKind::Empty => 0,
            StyleKind::Block(_) => 1,
            StyleKind::Dual(_) => 2,
            StyleKind::Combined(all) => all.len(),
        }
    }

    /// Sets the style's properties in `scope`, as `applyStyle`.
    pub fn apply_style(&self, scope: &mut dyn CommonStyleScope) {
        match &self.0 {
            StyleKind::Empty => {}
            StyleKind::Block(block) => block(scope),
            StyleKind::Dual(pair) => {
                pair.0.apply_style(scope);
                pair.1.apply_style(scope);
            }
            StyleKind::Combined(all) => {
                for style in all.iter() {
                    style.apply_style(scope);
                }
            }
        }
    }

    fn ptr(&self) -> Option<*const ()> {
        match &self.0 {
            StyleKind::Empty => None,
            StyleKind::Block(block) => Some(Rc::as_ptr(block) as *const ()),
            StyleKind::Dual(pair) => Some(Rc::as_ptr(pair) as *const ()),
            StyleKind::Combined(all) => Some(Rc::as_ptr(all) as *const ()),
        }
    }
}

impl CustomStyle<dyn CommonStyleScope + '_> for CommonStyle {
    fn apply_style(&self, scope: &mut (dyn CommonStyleScope + '_)) {
        CommonStyle::apply_style(self, scope);
    }
}

impl Default for CommonStyle {
    fn default() -> CommonStyle {
        CommonStyle::EMPTY
    }
}

impl PartialEq for CommonStyle {
    /// The same style, by identity.
    fn eq(&self, other: &CommonStyle) -> bool {
        self.ptr() == other.ptr()
    }
}

impl ComposeArg for CommonStyle {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Debug for CommonStyle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            StyleKind::Empty => formatter.write_str("CommonStyle::EMPTY"),
            _ => write!(formatter, "CommonStyle({})", self.combined_count_for_test()),
        }
    }
}

/// The name under which a resolver gives its descendants a style, as Compose's `NestedStyleKey`:
/// a `static`, identified by its label (D1581), which must be unique.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NestedStyleKey(&'static str);

impl NestedStyleKey {
    /// A key labelled `label`, as `NestedStyleKey(label)`.
    pub const fn new(label: &'static str) -> NestedStyleKey {
        NestedStyleKey(label)
    }
}

impl fmt::Debug for NestedStyleKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "NestedStyleKey(label = {})", self.0)
    }
}
