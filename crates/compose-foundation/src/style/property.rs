//! Style properties and their values, as Compose's `StyleProperty.kt`: `StyleProperty`,
//! `ProvidableStyleProperty`, `stylePropertyOf`, `styleLocalOf`, `Fill`, `Breadth`, `Shadows`
//! and the style `lerp`s.

use std::any::{Any, TypeId};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use compose_runtime::{KeyHasher, key_of};
use compose_ui::shadow::Shadow;
use compose_ui::{Brush, Color, Dp, FontStyle, FontWeight, Shape, TextUnit};

/// A property's value, type-erased: the resolver's maps hold any property's.
pub(crate) type Value = Rc<dyn Any>;

/// A property a style sets and its resolver gives, as Compose's `StyleProperty<T>`: its name,
/// whether it's a *local* (a resolver's descendants inherit it, read through their own resolver
/// when they don't set it), its value when no style sets it, and how it animates (`lerp`).
///
/// Declared as a `static` (D1581), as a composition local is, and identified by its name, its
/// value type and whether it's local, which must be unique together (an address isn't an
/// identity: a `const` has a new one at each use):
///
/// ```ignore
/// static CORNER: ProvidableStyleProperty<Dp> = style_property_of("corner", || Dp(4.0));
/// ```
pub struct StyleProperty<T: 'static> {
    name: &'static str,
    is_local: bool,
    default_value: fn() -> T,
    lerp: fn(&T, &T, f32) -> T,
}

impl<T: 'static> Clone for StyleProperty<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for StyleProperty<T> {}

/// A jump at half way, as `StyleProperty.lerp`'s default.
fn step<T: Clone>(start: &T, stop: &T, fraction: f32) -> T {
    if fraction < 0.5 { start.clone() } else { stop.clone() }
}

impl<T: Clone + PartialEq + 'static> StyleProperty<T> {
    /// A property no style can set (only [`ProvidableStyleProperty`]s are set), animating by a
    /// jump at half way, as `StyleProperty(name, isLocal, defaultValue)`.
    pub const fn new(name: &'static str, is_local: bool, default_value: fn() -> T) -> Self {
        StyleProperty { name, is_local, default_value, lerp: step::<T> }
    }

    /// As [`new`](Self::new), animating with `lerp` (Compose's `lerp` overridden).
    pub const fn with_lerp(
        name: &'static str,
        is_local: bool,
        lerp: fn(&T, &T, f32) -> T,
        default_value: fn() -> T,
    ) -> Self {
        StyleProperty { name, is_local, default_value, lerp }
    }

    /// Its name, as `name`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Whether it's a local, as `isLocal`.
    pub fn is_local(&self) -> bool {
        self.is_local
    }

    /// Its value when no style sets it, as `defaultValue()`.
    pub fn default_value(&self) -> T {
        (self.default_value)()
    }

    /// Between `start` and `stop` by `fraction`, as `lerp(a, b, t)`.
    pub fn lerp(&self, start: &T, stop: &T, fraction: f32) -> T {
        (self.lerp)(start, stop, fraction)
    }

    /// Its identity: its name, its value type and whether it's local.
    pub(crate) fn key(&self) -> u64 {
        let mut hasher = KeyHasher::default();
        TypeId::of::<T>().hash(&mut hasher);
        key_of(self.name) ^ hasher.finish().rotate_left(29) ^ u64::from(self.is_local)
    }
}

impl<T: 'static> fmt::Debug for StyleProperty<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.is_local { "local" } else { "property" };
        write!(formatter, "{} {kind}", self.name)
    }
}

/// A property a style can set (`provide`), as Compose's `ProvidableStyleProperty<T>`. Made with
/// [`style_property_of`] or [`style_local_of`] (and their `_with` forms).
pub struct ProvidableStyleProperty<T: 'static>(StyleProperty<T>);

impl<T: 'static> Clone for ProvidableStyleProperty<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for ProvidableStyleProperty<T> {}

impl<T: Clone + PartialEq + 'static> ProvidableStyleProperty<T> {
    /// As `ProvidableStyleProperty(name, local, defaultValue)`, animating by a jump at half way.
    pub const fn new(name: &'static str, is_local: bool, default_value: fn() -> T) -> Self {
        ProvidableStyleProperty(StyleProperty::new(name, is_local, default_value))
    }

    /// The property, as a [`StyleProperty`].
    pub const fn property(&self) -> &StyleProperty<T> {
        &self.0
    }
}

impl<T: 'static> std::ops::Deref for ProvidableStyleProperty<T> {
    type Target = StyleProperty<T>;
    fn deref(&self) -> &StyleProperty<T> {
        &self.0
    }
}

impl<T: 'static> fmt::Debug for ProvidableStyleProperty<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A property that animates with its type's [`StyleInterpolate`], as
/// `stylePropertyOf(name) { default }` (Compose's default interpolation).
pub const fn style_property_of<T: StyleInterpolate>(
    name: &'static str,
    default_value: fn() -> T,
) -> ProvidableStyleProperty<T> {
    ProvidableStyleProperty(StyleProperty::with_lerp(name, false, T::style_lerp, default_value))
}

/// A property that animates with `interpolate`, as `stylePropertyOf(name, interpolate)`.
pub const fn style_property_of_with<T: Clone + PartialEq + 'static>(
    name: &'static str,
    interpolate: fn(&T, &T, f32) -> T,
    default_value: fn() -> T,
) -> ProvidableStyleProperty<T> {
    ProvidableStyleProperty(StyleProperty::with_lerp(name, false, interpolate, default_value))
}

/// A local (inherited by the resolvers below) that animates with its type's
/// [`StyleInterpolate`], as `styleLocalOf(name) { default }`.
pub const fn style_local_of<T: StyleInterpolate>(
    name: &'static str,
    default_value: fn() -> T,
) -> ProvidableStyleProperty<T> {
    ProvidableStyleProperty(StyleProperty::with_lerp(name, true, T::style_lerp, default_value))
}

/// A local that animates with `interpolate`, as `styleLocalOf(name, interpolate)`.
pub const fn style_local_of_with<T: Clone + PartialEq + 'static>(
    name: &'static str,
    interpolate: fn(&T, &T, f32) -> T,
    default_value: fn() -> T,
) -> ProvidableStyleProperty<T> {
    ProvidableStyleProperty(StyleProperty::with_lerp(name, true, interpolate, default_value))
}

/// Any property, its value type erased: what the resolver keeps, and what
/// [`any_set`](super::StylePropertyAccessorScopeExt::any_set) takes.
pub trait AnyStyleProperty: Sync + 'static {
    #[doc(hidden)]
    fn erased(&'static self) -> ErasedProperty;
}

impl<T: Clone + PartialEq + 'static> AnyStyleProperty for StyleProperty<T> {
    fn erased(&'static self) -> ErasedProperty {
        ErasedProperty { key: self.key(), vtable: self }
    }
}

impl<T: Clone + PartialEq + 'static> AnyStyleProperty for ProvidableStyleProperty<T> {
    fn erased(&'static self) -> ErasedProperty {
        self.0.erased()
    }
}

/// A property's operations on erased values.
pub(crate) trait PropertyOps {
    fn name(&self) -> &'static str;
    fn is_local(&self) -> bool;
    fn default_value(&self) -> Value;
    fn lerp(&self, start: &Value, stop: &Value, fraction: f32) -> Value;
    fn same(&self, a: &Value, b: &Value) -> bool;
}

impl<T: Clone + PartialEq + 'static> PropertyOps for StyleProperty<T> {
    fn name(&self) -> &'static str {
        self.name
    }
    fn is_local(&self) -> bool {
        self.is_local
    }
    fn default_value(&self) -> Value {
        Rc::new((self.default_value)())
    }
    fn lerp(&self, start: &Value, stop: &Value, fraction: f32) -> Value {
        match (start.downcast_ref::<T>(), stop.downcast_ref::<T>()) {
            (Some(start), Some(stop)) => Rc::new((self.lerp)(start, stop, fraction)),
            _ => {
                if fraction < 0.5 {
                    start.clone()
                } else {
                    stop.clone()
                }
            }
        }
    }
    fn same(&self, a: &Value, b: &Value) -> bool {
        Rc::ptr_eq(a, b) || a.downcast_ref::<T>() == b.downcast_ref::<T>()
    }
}

/// A property with its type erased: its identity and its operations.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct ErasedProperty {
    pub(crate) key: u64,
    pub(crate) vtable: &'static dyn PropertyOps,
}

impl ErasedProperty {
    /// Whether it's the same property.
    pub fn same_property(&self, other: &ErasedProperty) -> bool {
        self.key == other.key
    }

    pub(crate) fn is_local(&self) -> bool {
        self.vtable.is_local()
    }
}

impl fmt::Debug for ErasedProperty {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.vtable.name())
    }
}

/// How a value animates by default, as Compose's `stylePropertyOf` default interpolation:
/// Compose's interpolations for its types (`Dp`, `f32`, `i32`, `i64`, `Color`, `TextUnit`,
/// [`Fill`], [`Breadth`], [`Shadows`]), a jump at half way for the rest. An app's type
/// implements it with an empty `impl` for the jump.
pub trait StyleInterpolate: Clone + PartialEq + 'static {
    /// Between `start` and `stop` by `fraction`.
    fn style_lerp(start: &Self, stop: &Self, fraction: f32) -> Self {
        step(start, stop, fraction)
    }
}

impl StyleInterpolate for f32 {
    fn style_lerp(start: &f32, stop: &f32, fraction: f32) -> f32 {
        compose_ui::lerp(*start, *stop, fraction)
    }
}

impl StyleInterpolate for i32 {
    fn style_lerp(start: &i32, stop: &i32, fraction: f32) -> i32 {
        compose_ui::lerp(*start, *stop, fraction)
    }
}

impl StyleInterpolate for i64 {
    fn style_lerp(start: &i64, stop: &i64, fraction: f32) -> i64 {
        compose_ui::lerp(*start, *stop, fraction)
    }
}

impl StyleInterpolate for Dp {
    fn style_lerp(start: &Dp, stop: &Dp, fraction: f32) -> Dp {
        lerp_dp(*start, *stop, fraction)
    }
}

impl StyleInterpolate for Color {
    fn style_lerp(start: &Color, stop: &Color, fraction: f32) -> Color {
        Color::lerp(*start, *stop, fraction)
    }
}

impl StyleInterpolate for TextUnit {
    fn style_lerp(start: &TextUnit, stop: &TextUnit, fraction: f32) -> TextUnit {
        lerp_text_unit(*start, *stop, fraction)
    }
}

impl StyleInterpolate for bool {}
impl StyleInterpolate for u32 {}
impl StyleInterpolate for u64 {}
impl StyleInterpolate for usize {}
impl StyleInterpolate for String {}
impl StyleInterpolate for &'static str {}
/// A jump: Compose lerps its corner-based shapes (D1054's `shapes` feature).
impl StyleInterpolate for Shape {}
impl StyleInterpolate for Brush {}
impl StyleInterpolate for FontWeight {}
impl StyleInterpolate for FontStyle {}
/// A jump, as Compose's for a value it doesn't know how to interpolate (a `null` included).
impl<T: Clone + PartialEq + 'static> StyleInterpolate for Option<T> {}
impl StyleInterpolate for compose_ui::FontFamily {}
impl StyleInterpolate for compose_ui::BaselineShift {}
impl StyleInterpolate for compose_ui::TextDecoration {}
impl StyleInterpolate for compose_ui::FontSynthesis {}
impl StyleInterpolate for compose_ui::TextMotion {}

/// `Dp`s between, as the style package's `lerp(Dp, Dp, Float)`: an unspecified or negative end
/// counts as 0.
pub fn lerp_dp(start: Dp, stop: Dp, fraction: f32) -> Dp {
    let clamp = |dp: Dp| if !dp.is_specified() || dp.0 < 0.0 { 0.0 } else { dp.0 };
    Dp(compose_ui::lerp(clamp(start), clamp(stop), fraction))
}

/// Text units between, as the style package's `lerp(TextUnit, TextUnit, Float)`: both in `sp`
/// or both in `em`, else a jump at half way.
pub fn lerp_text_unit(start: TextUnit, stop: TextUnit, fraction: f32) -> TextUnit {
    use compose_ui::TextUnitType;
    match (start.unit_type(), stop.unit_type()) {
        (TextUnitType::Sp, TextUnitType::Sp) | (TextUnitType::Em, TextUnitType::Em) => {
            TextUnit::new(
                compose_ui::lerp(start.value(), stop.value(), fraction),
                start.unit_type(),
            )
        }
        _ => step(&start, &stop, fraction),
    }
}

/// A size in one direction, as Compose's style `Breadth`: none, a distance, or a fraction of
/// the space there is.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Breadth {
    /// As `Breadth.None`.
    #[default]
    None,
    /// As `Breadth.Distance`.
    Distance(Dp),
    /// As `Breadth.Fraction`.
    Fraction(f32),
}

impl From<Dp> for Breadth {
    /// As `Breadth(value: Dp)`.
    fn from(value: Dp) -> Breadth {
        Breadth::Distance(value)
    }
}

impl From<f32> for Breadth {
    /// As `Breadth(value: Float)`.
    fn from(value: f32) -> Breadth {
        Breadth::Fraction(value)
    }
}

impl StyleInterpolate for Breadth {
    /// As `Breadth.lerp`: a none end is a zero of the other's kind; a distance and a fraction
    /// jump at half way.
    fn style_lerp(start: &Breadth, stop: &Breadth, fraction: f32) -> Breadth {
        let zero_like = |other: &Breadth| match other {
            Breadth::Distance(_) => Breadth::Distance(Dp(0.0)),
            Breadth::Fraction(_) => Breadth::Fraction(0.0),
            Breadth::None => Breadth::None,
        };
        let a = if *start == Breadth::None { zero_like(stop) } else { *start };
        let b = if *stop == Breadth::None { zero_like(start) } else { *stop };
        match (a, b) {
            (Breadth::Distance(a), Breadth::Distance(b)) => {
                Breadth::Distance(lerp_dp(a, b, fraction))
            }
            (Breadth::Fraction(a), Breadth::Fraction(b)) => {
                Breadth::Fraction(compose_ui::lerp(a, b, fraction))
            }
            _ => step(start, stop, fraction),
        }
    }
}

/// How an area is painted, as Compose's style `Fill`: nothing, a color, or a brush.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum Fill {
    /// As `Fill.None`.
    #[default]
    None,
    /// As `Fill.Color`.
    Color(Color),
    /// As `Fill.Brush`.
    Brush(Brush),
}

impl Fill {
    /// Its color, as `asColor()`: a solid brush's, unspecified for none or a gradient.
    pub fn as_color(&self) -> Color {
        match self {
            Fill::Color(color) | Fill::Brush(Brush::Solid(color)) => *color,
            _ => Color::UNSPECIFIED,
        }
    }

    /// Its brush, as `asBrush()`: a color's solid one, none for none.
    pub fn as_brush(&self) -> Option<Brush> {
        match self {
            Fill::None => None,
            Fill::Color(color) => Some(Brush::Solid(*color)),
            Fill::Brush(brush) => Some(brush.clone()),
        }
    }

    /// A color or none: lerped as colors.
    fn is_color(&self) -> bool {
        matches!(self, Fill::None | Fill::Color(_))
    }

    /// What it lerps as when the other isn't a color: none is a transparent brush.
    fn lerp_target(&self) -> Brush {
        match self {
            Fill::None => Brush::Solid(Color::TRANSPARENT),
            Fill::Color(color) => Brush::Solid(*color),
            Fill::Brush(brush) => brush.clone(),
        }
    }
}

impl From<Color> for Fill {
    /// As `Fill(color)`.
    fn from(color: Color) -> Fill {
        Fill::Color(color)
    }
}

impl From<Brush> for Fill {
    /// As `Fill(brush)`.
    fn from(brush: Brush) -> Fill {
        Fill::Brush(brush)
    }
}

impl StyleInterpolate for Fill {
    /// As `Fill.lerp`: two colors (none a transparent one, D1581) lerp as colors; two solid
    /// brushes as their colors, in a brush; other brushes jump at half way (compose-rs's
    /// gradients don't interpolate).
    fn style_lerp(start: &Fill, stop: &Fill, fraction: f32) -> Fill {
        let color = |fill: &Fill| match fill {
            Fill::Color(color) => *color,
            _ => Color::TRANSPARENT,
        };
        if start.is_color() && stop.is_color() {
            return Fill::Color(Color::lerp(color(start), color(stop), fraction));
        }
        match (start.lerp_target(), stop.lerp_target()) {
            (Brush::Solid(a), Brush::Solid(b)) => {
                Fill::Brush(Brush::Solid(Color::lerp(a, b, fraction)))
            }
            (a, b) => Fill::Brush(step(&a, &b, fraction)),
        }
    }
}

/// Shadows under or inside a shape, as Compose's style `Shadows`: none, one, or several.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum Shadows {
    /// As `Shadows.None`.
    #[default]
    None,
    /// As `Shadows.Simple`.
    Simple(Shadow),
    /// As `Shadows.Compound`.
    Compound(Rc<[Shadow]>),
}

impl From<Shadow> for Shadows {
    /// As `Shadows(shadow)`.
    fn from(shadow: Shadow) -> Shadows {
        Shadows::Simple(shadow)
    }
}

impl From<Vec<Shadow>> for Shadows {
    /// As `Shadows(vararg shadows)`.
    fn from(shadows: Vec<Shadow>) -> Shadows {
        Shadows::Compound(shadows.into())
    }
}

impl StyleInterpolate for Shadows {
    /// As `Shadows.lerp`: two simple shadows lerp as shadows; the rest jump at half way.
    fn style_lerp(start: &Shadows, stop: &Shadows, fraction: f32) -> Shadows {
        match (start, stop) {
            (Shadows::Simple(a), Shadows::Simple(b)) => {
                match compose_ui::shadow::lerp(Some(a), Some(b), fraction) {
                    Some(shadow) => Shadows::Simple(shadow),
                    None => step(start, stop, fraction),
                }
            }
            _ => step(start, stop, fraction),
        }
    }
}
