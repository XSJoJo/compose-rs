//! Units and geometry: `Dp`, `TextUnit`, `Density`, sizes, offsets and rectangles, as Compose's
//! `androidx.compose.ui.unit` and `geometry`.

use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

use compose_runtime::ComposeArg;

use crate::geometry::same_value;
pub(crate) use crate::geometry::{Offset, Rect, Size};
use crate::math::{Lerp, lerp};

/// Density-independent pixels: the unit of layout, as Compose's `Dp`.
///
/// Equal when their values are, `UNSPECIFIED` (NaN) included, and `-0` equal to `0`; hashed
/// alike when equal. Ordered by value: a NaN compares equal to a NaN and to nothing else (where
/// Compose's `compareTo` calls a NaN equal to anything, so its `<=` with one NaN is true).
#[derive(Clone, Copy, Default)]
pub struct Dp(pub f32);

impl PartialEq for Dp {
    fn eq(&self, other: &Self) -> bool {
        same_value(self.0, other.0)
    }
}

/// NaN equals NaN, so equality is reflexive.
impl Eq for Dp {}

impl std::hash::Hash for Dp {
    /// By the value's bits, `-0` as `0` and every NaN as one, as equality has them.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let bits = if self.0.is_nan() { f32::NAN.to_bits() } else { (self.0 + 0.0).to_bits() };
        bits.hash(state);
    }
}

impl PartialOrd for Dp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.0.is_nan() && other.0.is_nan() {
            return Some(std::cmp::Ordering::Equal);
        }
        self.0.partial_cmp(&other.0)
    }
}

/// `x` rounded to the nearest integer, halves up, as Kotlin's `roundToInt` (`Math.round`), which
/// Compose's layout uses: `-1.5` rounds to `-1`.
pub fn round_to_int(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}

impl Dp {
    pub const ZERO: Dp = Dp(0.0);
    /// The thinnest line, as `Dp.Hairline`: no space taken, a stroke or border of it a single pixel
    /// wide whatever the density (a stroke of width 0).
    pub const HAIRLINE: Dp = Dp(0.0);
    /// Infinitely large, as `Dp.Infinity`.
    pub const INFINITY: Dp = Dp(f32::INFINITY);
    /// No value: a default meaning "decide elsewhere", as `Dp.Unspecified`.
    pub const UNSPECIFIED: Dp = Dp(f32::NAN);

    pub fn is_specified(self) -> bool {
        !self.0.is_nan()
    }

    pub fn is_unspecified(self) -> bool {
        self.0.is_nan()
    }

    /// Neither infinite nor unspecified, as `isFinite`.
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }

    /// Itself if specified, else `or_else()`, as `takeOrElse`.
    pub fn take_or_else(self, or_else: impl FnOnce() -> Dp) -> Dp {
        if self.is_specified() { self } else { or_else() }
    }

    pub fn value(self) -> f32 {
        self.0
    }

    /// The smaller, as Compose's `min(a, b)`: unspecified if either is.
    pub fn min(self, other: Dp) -> Dp {
        if self.0.is_nan() || other.0.is_nan() { Dp::UNSPECIFIED } else { Dp(self.0.min(other.0)) }
    }

    /// The larger, as Compose's `max(a, b)`: unspecified if either is.
    pub fn max(self, other: Dp) -> Dp {
        if self.0.is_nan() || other.0.is_nan() { Dp::UNSPECIFIED } else { Dp(self.0.max(other.0)) }
    }

    /// Clamped to `minimum..=maximum`, as `coerceIn`: panics if `minimum` is above `maximum`, as
    /// Compose's throws.
    pub fn coerce_in(self, minimum: Dp, maximum: Dp) -> Dp {
        assert!(
            minimum.0 <= maximum.0 || minimum.0.is_nan() || maximum.0.is_nan(),
            "Cannot coerce value to an empty range: maximum {maximum} is less than minimum {minimum}."
        );
        self.coerce_at_least(minimum).coerce_at_most(maximum)
    }

    /// At least `minimum`, as `coerceAtLeast`.
    pub fn coerce_at_least(self, minimum: Dp) -> Dp {
        if self.0 < minimum.0 { minimum } else { self }
    }

    /// At most `maximum`, as `coerceAtMost`.
    pub fn coerce_at_most(self, maximum: Dp) -> Dp {
        if self.0 > maximum.0 { maximum } else { self }
    }
}

impl fmt::Debug for Dp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.dp", self.0)
    }
}

impl fmt::Display for Dp {
    /// As Compose's `toString`: `1.0.dp`, or `Dp.Unspecified`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_unspecified() {
            formatter.write_str("Dp.Unspecified")
        } else {
            write!(formatter, "{:?}.dp", self.0)
        }
    }
}

impl ComposeArg for Dp {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Add for Dp {
    type Output = Dp;
    fn add(self, other: Dp) -> Dp {
        Dp(self.0 + other.0)
    }
}

impl Sub for Dp {
    type Output = Dp;
    fn sub(self, other: Dp) -> Dp {
        Dp(self.0 - other.0)
    }
}

impl Mul<f32> for Dp {
    type Output = Dp;
    fn mul(self, factor: f32) -> Dp {
        Dp(self.0 * factor)
    }
}

impl Mul<i32> for Dp {
    type Output = Dp;
    fn mul(self, factor: i32) -> Dp {
        Dp(self.0 * factor as f32)
    }
}

impl Mul<Dp> for f32 {
    type Output = Dp;
    fn mul(self, dp: Dp) -> Dp {
        Dp(self * dp.0)
    }
}

impl Mul<Dp> for i32 {
    type Output = Dp;
    fn mul(self, dp: Dp) -> Dp {
        Dp(self as f32 * dp.0)
    }
}

/// As Compose's `Double.times(Dp)`: the double made a float first.
impl Mul<Dp> for f64 {
    type Output = Dp;
    fn mul(self, dp: Dp) -> Dp {
        Dp(self as f32 * dp.0)
    }
}

impl Div<f32> for Dp {
    type Output = Dp;
    fn div(self, divisor: f32) -> Dp {
        Dp(self.0 / divisor)
    }
}

impl Div<i32> for Dp {
    type Output = Dp;
    fn div(self, divisor: i32) -> Dp {
        Dp(self.0 / divisor as f32)
    }
}

/// How many times `other` fits, as Compose's `Dp.div(Dp)`.
impl Div<Dp> for Dp {
    type Output = f32;
    fn div(self, other: Dp) -> f32 {
        self.0 / other.0
    }
}

impl Neg for Dp {
    type Output = Dp;
    fn neg(self) -> Dp {
        Dp(-self.0)
    }
}

/// As Compose's `lerp(Dp, Dp, Float)`: a negative zero made zero, as its `+ 0f`.
impl Lerp for Dp {
    fn lerp(start: Dp, stop: Dp, fraction: f32) -> Dp {
        Dp(lerp(start.0, stop.0, fraction) + 0.0)
    }
}

/// What a [`TextUnit`] measures, as Compose's `TextUnitType`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextUnitType {
    /// No value: decided elsewhere (inherited, or a default).
    #[default]
    Unspecified,
    /// Scale-independent pixels: density-independent pixels times the font scale.
    Sp,
    /// A multiple of the font size (for a font size, of the size it's relative to).
    Em,
}

/// A text measure, as Compose's `TextUnit`: in `sp`, in `em`, or unspecified (decided
/// elsewhere). Made with `14.sp()` and `1.5.em()` ([`UnitExt`]) or [`TextUnit::sp`] and
/// [`TextUnit::em`].
///
/// Equal when their types and their values' bits are, as Compose's packed value: unspecified
/// equals unspecified. Arithmetic needs a specified unit, and comparisons two of the same type:
/// otherwise they panic, as Compose's throw.
#[derive(Clone, Copy)]
pub struct TextUnit {
    value: f32,
    unit: TextUnitType,
}

impl TextUnit {
    /// As `TextUnit.Unspecified`: its value is NaN.
    pub const UNSPECIFIED: TextUnit = TextUnit { value: f32::NAN, unit: TextUnitType::Unspecified };

    /// As `TextUnit(value, type)`. An unspecified type's value is NaN, whatever is given.
    pub const fn new(value: f32, unit: TextUnitType) -> TextUnit {
        match unit {
            TextUnitType::Unspecified => TextUnit::UNSPECIFIED,
            _ => TextUnit { value, unit },
        }
    }

    /// `value` sp, as `value.sp`.
    pub const fn sp(value: f32) -> TextUnit {
        TextUnit { value, unit: TextUnitType::Sp }
    }

    /// `value` em, as `value.em`.
    pub const fn em(value: f32) -> TextUnit {
        TextUnit { value, unit: TextUnitType::Em }
    }

    /// The value, in its type's unit (NaN when unspecified).
    pub const fn value(self) -> f32 {
        self.value
    }

    /// Its type, as Compose's `type`.
    pub const fn unit_type(self) -> TextUnitType {
        self.unit
    }

    pub const fn is_sp(self) -> bool {
        matches!(self.unit, TextUnitType::Sp)
    }

    pub const fn is_em(self) -> bool {
        matches!(self.unit, TextUnitType::Em)
    }

    pub const fn is_specified(self) -> bool {
        !self.is_unspecified()
    }

    pub const fn is_unspecified(self) -> bool {
        matches!(self.unit, TextUnitType::Unspecified)
    }

    /// `self` if specified, else `other`, as `takeOrElse`.
    pub const fn or(self, other: TextUnit) -> TextUnit {
        if self.is_specified() { self } else { other }
    }

    /// Between `start` and `stop`, as Compose's `lerp(TextUnit, TextUnit, Float)`: both must be
    /// specified and of the same type.
    pub fn lerp(start: TextUnit, stop: TextUnit, fraction: f32) -> TextUnit {
        check_arithmetic_pair(start, stop);
        TextUnit { value: lerp_f32(start.value, stop.value, fraction), unit: start.unit }
    }

    fn checked(self) -> TextUnit {
        assert!(self.is_specified(), "Cannot perform operation for Unspecified type.");
        self
    }
}

/// Between `start` and `stop`, as Compose's `lerp(Float, Float, Float)`.
pub(crate) fn lerp_f32(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

fn check_arithmetic_pair(a: TextUnit, b: TextUnit) {
    assert!(a.is_specified() && b.is_specified(), "Cannot perform operation for Unspecified type.");
    assert!(a.unit == b.unit, "Cannot perform operation for {:?} and {:?}", a.unit, b.unit);
}

impl Default for TextUnit {
    fn default() -> Self {
        TextUnit::UNSPECIFIED
    }
}

impl PartialEq for TextUnit {
    fn eq(&self, other: &Self) -> bool {
        self.unit == other.unit && self.value.to_bits() == other.value.to_bits()
    }
}

impl Eq for TextUnit {}

impl std::hash::Hash for TextUnit {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.unit.hash(state);
        self.value.to_bits().hash(state);
    }
}

/// Compares two specified units of the same type, as Compose's `compareTo`: anything else
/// panics, as Compose's throws.
impl PartialOrd for TextUnit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        check_arithmetic_pair(*self, *other);
        self.value.partial_cmp(&other.value)
    }
}

impl Neg for TextUnit {
    type Output = TextUnit;
    fn neg(self) -> TextUnit {
        let unit = self.checked();
        TextUnit { value: -unit.value, ..unit }
    }
}

impl Mul<f32> for TextUnit {
    type Output = TextUnit;
    fn mul(self, other: f32) -> TextUnit {
        let unit = self.checked();
        TextUnit { value: unit.value * other, ..unit }
    }
}

impl Mul<TextUnit> for f32 {
    type Output = TextUnit;
    fn mul(self, other: TextUnit) -> TextUnit {
        let unit = other.checked();
        TextUnit { value: self * unit.value, ..unit }
    }
}

impl Div<f32> for TextUnit {
    type Output = TextUnit;
    fn div(self, other: f32) -> TextUnit {
        let unit = self.checked();
        TextUnit { value: unit.value / other, ..unit }
    }
}

/// Compose's `Int` and `Double` overloads of `times` and `div`: computed in `f32`, as
/// Compose's (a `Double` is converted after the operation).
macro_rules! text_unit_arithmetic {
    ($($type:ty),*) => {
        $(
            impl Mul<$type> for TextUnit {
                type Output = TextUnit;
                fn mul(self, other: $type) -> TextUnit {
                    let unit = self.checked();
                    TextUnit { value: (unit.value as f64 * other as f64) as f32, ..unit }
                }
            }

            impl Mul<TextUnit> for $type {
                type Output = TextUnit;
                fn mul(self, other: TextUnit) -> TextUnit {
                    other * self
                }
            }

            impl Div<$type> for TextUnit {
                type Output = TextUnit;
                fn div(self, other: $type) -> TextUnit {
                    let unit = self.checked();
                    TextUnit { value: (unit.value as f64 / other as f64) as f32, ..unit }
                }
            }
        )*
    };
}

text_unit_arithmetic!(i32, f64);

impl fmt::Debug for TextUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.unit {
            TextUnitType::Unspecified => formatter.write_str("Unspecified"),
            TextUnitType::Sp => write!(formatter, "{:?}.sp", self.value),
            TextUnitType::Em => write!(formatter, "{:?}.em", self.value),
        }
    }
}

impl fmt::Display for TextUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, formatter)
    }
}

impl ComposeArg for TextUnit {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// `16.dp()`, `14.sp()`, `1.5.em()`, as Kotlin's `16.dp`, `14.sp` and `1.5.em`.
pub trait UnitExt {
    fn dp(self) -> Dp;
    fn sp(self) -> TextUnit;
    fn em(self) -> TextUnit;
}

macro_rules! unit_ext {
    ($($type:ty),*) => {
        $(impl UnitExt for $type {
            fn dp(self) -> Dp {
                Dp(self as f32)
            }
            fn sp(self) -> TextUnit {
                TextUnit::sp(self as f32)
            }
            fn em(self) -> TextUnit {
                TextUnit::em(self as f32)
            }
        })*
    };
}

unit_ext!(i32, u32, f32, f64, usize);

/// Pixels per `Dp`, and the font scale applied to `sp`, as Compose's `Density` (and its
/// `FontScaling`): a struct where Compose's is an interface, its conversions methods taking what
/// Compose's extensions are called on (`density.to_px(dp)` for `dp.toPx()`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Density {
    pub density: f32,
    pub font_scale: f32,
}

impl compose_runtime::ComposeArg for Density {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Default for Density {
    fn default() -> Self {
        Density { density: 1.0, font_scale: 1.0 }
    }
}

impl Density {
    /// As `Density(density)`: a font scale of 1.
    pub fn new(density: f32) -> Density {
        Density { density, font_scale: 1.0 }
    }

    /// As `Density(density, fontScale)`.
    pub const fn with_font_scale(density: f32, font_scale: f32) -> Density {
        Density { density, font_scale }
    }

    /// The number of pixels for `dp`, rounded as Compose's `roundToPx`.
    pub fn round_to_px(&self, dp: Dp) -> i32 {
        let px = dp.0 * self.density;
        if px.is_infinite() { if px > 0.0 { i32::MAX } else { i32::MIN } } else { round_to_int(px) }
    }

    pub fn to_px(&self, dp: Dp) -> f32 {
        dp.0 * self.density
    }

    /// The pixels for a measure in `sp`, as Compose's `TextUnit.toPx`. Panics for another
    /// type ("Only Sp can convert to Px", as Compose's): an `em` is relative to a font size.
    pub fn sp_to_px(&self, sp: TextUnit) -> f32 {
        assert!(sp.is_sp(), "Only Sp can convert to Px");
        sp.value() * self.font_scale * self.density
    }

    /// [`sp_to_px`](Self::sp_to_px) rounded half up, as `TextUnit.roundToPx`.
    pub fn sp_round_to_px(&self, sp: TextUnit) -> i32 {
        round_to_int(self.sp_to_px(sp))
    }

    /// As `Int.toDp` and `Float.toDp`.
    pub fn to_dp(&self, px: f32) -> Dp {
        Dp(px / self.density)
    }

    /// The `sp` for `px` pixels, as `Int.toSp` and `Float.toSp`.
    pub fn to_sp(&self, px: f32) -> TextUnit {
        self.dp_to_sp(self.to_dp(px))
    }

    /// As `FontScaling`'s `Dp.toSp`: divided by the font scale (linearly: Android 14's
    /// non-linear scaling of large fonts is feature:nonlinear-font-scaling).
    pub fn dp_to_sp(&self, dp: Dp) -> TextUnit {
        TextUnit::sp(dp.0 / self.font_scale)
    }

    /// As `FontScaling`'s `TextUnit.toDp`: times the font scale. Panics for anything but `sp`, as
    /// Compose's.
    pub fn sp_to_dp(&self, sp: TextUnit) -> Dp {
        assert!(sp.is_sp(), "Only Sp can convert to Px");
        Dp(sp.value() * self.font_scale)
    }

    /// As `DpRect.toRect`.
    pub fn to_rect(&self, rect: DpRect) -> Rect {
        Rect::new(
            self.to_px(rect.left),
            self.to_px(rect.top),
            self.to_px(rect.right),
            self.to_px(rect.bottom),
        )
    }

    /// As `DpSize.toSize`: unspecified stays unspecified.
    pub fn to_size(&self, size: DpSize) -> Size {
        if size.is_specified() {
            Size::new(self.to_px(size.width), self.to_px(size.height))
        } else {
            Size::UNSPECIFIED
        }
    }

    /// As `Size.toDpSize`: unspecified stays unspecified.
    pub fn to_dp_size(&self, size: Size) -> DpSize {
        if size.is_specified() {
            DpSize::new(self.to_dp(size.width), self.to_dp(size.height))
        } else {
            DpSize::UNSPECIFIED
        }
    }
}

/// A size in pixels, as `IntSize`.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct IntSize {
    pub width: i32,
    pub height: i32,
}

impl IntSize {
    pub const ZERO: IntSize = IntSize { width: 0, height: 0 };

    pub const fn new(width: i32, height: i32) -> IntSize {
        IntSize { width, height }
    }

    /// Half of it, each side shifted right (rounding down, as Compose's), as `IntSize.center`.
    pub const fn center(self) -> IntOffset {
        IntOffset::new(self.width >> 1, self.height >> 1)
    }

    /// The rect from the origin of this size, as `toIntRect`.
    pub const fn to_int_rect(self) -> IntRect {
        IntRect::new(0, 0, self.width, self.height)
    }

    /// As `IntSize.toSize`.
    pub fn to_size(self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }
}

impl fmt::Debug for IntSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}x{}", self.width, self.height)
    }
}

impl fmt::Display for IntSize {
    /// As Compose's `toString`: `3 x 4`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} x {}", self.width, self.height)
    }
}

impl Mul<i32> for IntSize {
    type Output = IntSize;
    fn mul(self, other: i32) -> IntSize {
        IntSize::new(self.width.wrapping_mul(other), self.height.wrapping_mul(other))
    }
}

impl Mul<IntSize> for i32 {
    type Output = IntSize;
    fn mul(self, size: IntSize) -> IntSize {
        size * self
    }
}

impl Div<i32> for IntSize {
    type Output = IntSize;
    fn div(self, other: i32) -> IntSize {
        IntSize::new(self.width / other, self.height / other)
    }
}

impl From<IntSize> for Size {
    fn from(size: IntSize) -> Size {
        size.to_size()
    }
}

impl Size {
    /// Each side truncated toward zero, as `Size.toIntSize`.
    pub fn to_int_size(self) -> IntSize {
        IntSize::new(self.width as i32, self.height as i32)
    }

    /// Each side rounded half up, as `Size.roundToIntSize`.
    pub fn round_to_int_size(self) -> IntSize {
        IntSize::new(round_to_int(self.width), round_to_int(self.height))
    }
}

/// A position in pixels, as `IntOffset`.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct IntOffset {
    pub x: i32,
    pub y: i32,
}

impl IntOffset {
    pub const ZERO: IntOffset = IntOffset { x: 0, y: 0 };
    /// The largest offset, as `IntOffset.Max`.
    pub const MAX: IntOffset = IntOffset { x: i32::MAX, y: i32::MAX };

    pub const fn new(x: i32, y: i32) -> IntOffset {
        IntOffset { x, y }
    }

    /// As `IntOffset.toOffset`.
    pub fn to_offset(self) -> Offset {
        Offset::new(self.x as f32, self.y as f32)
    }
}

impl fmt::Debug for IntOffset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "({}, {})", self.x, self.y)
    }
}

impl fmt::Display for IntOffset {
    /// As Compose's `toString`: `(3, 4)`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, formatter)
    }
}

impl Add for IntOffset {
    type Output = IntOffset;
    fn add(self, other: IntOffset) -> IntOffset {
        IntOffset::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for IntOffset {
    type Output = IntOffset;
    fn sub(self, other: IntOffset) -> IntOffset {
        IntOffset::new(self.x - other.x, self.y - other.y)
    }
}

impl Neg for IntOffset {
    type Output = IntOffset;
    fn neg(self) -> IntOffset {
        IntOffset::new(-self.x, -self.y)
    }
}

/// Each coordinate scaled and rounded half up, as Compose's `IntOffset.times(Float)`.
impl Mul<f32> for IntOffset {
    type Output = IntOffset;
    fn mul(self, operand: f32) -> IntOffset {
        IntOffset::new(round_to_int(self.x as f32 * operand), round_to_int(self.y as f32 * operand))
    }
}

/// Each coordinate divided and rounded half up, as Compose's `IntOffset.div(Float)`.
impl Div<f32> for IntOffset {
    type Output = IntOffset;
    fn div(self, operand: f32) -> IntOffset {
        IntOffset::new(round_to_int(self.x as f32 / operand), round_to_int(self.y as f32 / operand))
    }
}

impl Rem<i32> for IntOffset {
    type Output = IntOffset;
    fn rem(self, operand: i32) -> IntOffset {
        IntOffset::new(self.x % operand, self.y % operand)
    }
}

impl Add<IntOffset> for Offset {
    type Output = Offset;
    fn add(self, offset: IntOffset) -> Offset {
        Offset::new(self.x + offset.x as f32, self.y + offset.y as f32)
    }
}

impl Sub<IntOffset> for Offset {
    type Output = Offset;
    fn sub(self, offset: IntOffset) -> Offset {
        Offset::new(self.x - offset.x as f32, self.y - offset.y as f32)
    }
}

impl Add<Offset> for IntOffset {
    type Output = Offset;
    fn add(self, offset: Offset) -> Offset {
        Offset::new(self.x as f32 + offset.x, self.y as f32 + offset.y)
    }
}

impl Sub<Offset> for IntOffset {
    type Output = Offset;
    fn sub(self, offset: Offset) -> Offset {
        Offset::new(self.x as f32 - offset.x, self.y as f32 - offset.y)
    }
}

impl Offset {
    /// Each coordinate rounded half up, as `Offset.round`.
    pub fn round(self) -> IntOffset {
        IntOffset::new(round_to_int(self.x), round_to_int(self.y))
    }
}

impl Lerp for IntOffset {
    fn lerp(start: IntOffset, stop: IntOffset, fraction: f32) -> IntOffset {
        IntOffset::new(lerp(start.x, stop.x, fraction), lerp(start.y, stop.y, fraction))
    }
}

/// An integer rectangle, as Compose's `IntRect`.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash, Debug)]
pub struct IntRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl IntRect {
    /// As `IntRect.Zero`.
    pub const ZERO: IntRect = IntRect::new(0, 0, 0, 0);

    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> IntRect {
        IntRect { left, top, right, bottom }
    }

    /// As `IntRect(offset, size)`.
    pub const fn from_offset_size(offset: IntOffset, size: IntSize) -> IntRect {
        IntRect::new(offset.x, offset.y, offset.x + size.width, offset.y + size.height)
    }

    /// As `IntRect(topLeft, bottomRight)`.
    pub const fn from_corners(top_left: IntOffset, bottom_right: IntOffset) -> IntRect {
        IntRect::new(top_left.x, top_left.y, bottom_right.x, bottom_right.y)
    }

    /// The square around `center` with sides `2 × radius`, as `IntRect(center, radius)`.
    pub const fn from_center(center: IntOffset, radius: i32) -> IntRect {
        IntRect::new(center.x - radius, center.y - radius, center.x + radius, center.y + radius)
    }

    pub const fn width(&self) -> i32 {
        self.right - self.left
    }

    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub const fn size(&self) -> IntSize {
        IntSize::new(self.width(), self.height())
    }

    /// No area, as `isEmpty`.
    pub const fn is_empty(&self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }

    /// Moved by `offset`, as `translate(offset)`.
    pub const fn translate(&self, offset: IntOffset) -> IntRect {
        self.translate_xy(offset.x, offset.y)
    }

    /// Moved by (`x`, `y`), as `translate(translateX, translateY)`.
    pub const fn translate_xy(&self, x: i32, y: i32) -> IntRect {
        IntRect::new(self.left + x, self.top + y, self.right + x, self.bottom + y)
    }

    /// Every side moved out by `delta`, as `inflate`.
    pub const fn inflate(&self, delta: i32) -> IntRect {
        IntRect::new(self.left - delta, self.top - delta, self.right + delta, self.bottom + delta)
    }

    /// Every side moved in by `delta`, as `deflate`.
    pub const fn deflate(&self, delta: i32) -> IntRect {
        self.inflate(-delta)
    }

    /// What it and `other` both cover, as `intersect`.
    pub fn intersect(&self, other: IntRect) -> IntRect {
        IntRect::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        )
    }

    /// Whether it and `other` share some area (touching isn't), as `overlaps`.
    pub const fn overlaps(&self, other: IntRect) -> bool {
        !(self.right <= other.left
            || other.right <= self.left
            || self.bottom <= other.top
            || other.bottom <= self.top)
    }

    pub fn min_dimension(&self) -> i32 {
        self.width().abs().min(self.height().abs())
    }

    pub fn max_dimension(&self) -> i32 {
        self.width().abs().max(self.height().abs())
    }

    pub const fn top_left(&self) -> IntOffset {
        IntOffset::new(self.left, self.top)
    }

    pub const fn top_center(&self) -> IntOffset {
        IntOffset::new(self.left + self.width() / 2, self.top)
    }

    pub const fn top_right(&self) -> IntOffset {
        IntOffset::new(self.right, self.top)
    }

    pub const fn center_left(&self) -> IntOffset {
        IntOffset::new(self.left, self.top + self.height() / 2)
    }

    pub const fn center(&self) -> IntOffset {
        IntOffset::new(self.left + self.width() / 2, self.top + self.height() / 2)
    }

    pub const fn center_right(&self) -> IntOffset {
        IntOffset::new(self.right, self.top + self.height() / 2)
    }

    pub const fn bottom_left(&self) -> IntOffset {
        IntOffset::new(self.left, self.bottom)
    }

    pub const fn bottom_center(&self) -> IntOffset {
        IntOffset::new(self.left + self.width() / 2, self.bottom)
    }

    pub const fn bottom_right(&self) -> IntOffset {
        IntOffset::new(self.right, self.bottom)
    }

    /// Whether `offset` is inside: the left and top edges are, the right and bottom ones aren't,
    /// as `contains`.
    pub const fn contains(&self, offset: IntOffset) -> bool {
        offset.x >= self.left
            && offset.x < self.right
            && offset.y >= self.top
            && offset.y < self.bottom
    }

    /// As `IntRect.toRect`.
    pub fn to_rect(&self) -> Rect {
        Rect::new(self.left as f32, self.top as f32, self.right as f32, self.bottom as f32)
    }
}

impl fmt::Display for IntRect {
    /// As Compose's `toString`: `IntRect.fromLTRB(1, 2, 3, 4)`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let IntRect { left, top, right, bottom } = self;
        write!(formatter, "IntRect.fromLTRB({left}, {top}, {right}, {bottom})")
    }
}

impl Lerp for IntRect {
    fn lerp(start: IntRect, stop: IntRect, fraction: f32) -> IntRect {
        IntRect::new(
            lerp(start.left, stop.left, fraction),
            lerp(start.top, stop.top, fraction),
            lerp(start.right, stop.right, fraction),
            lerp(start.bottom, stop.bottom, fraction),
        )
    }
}

impl Rect {
    /// Each side rounded half up, as `Rect.roundToIntRect`.
    pub fn round_to_int_rect(&self) -> IntRect {
        IntRect::new(
            round_to_int(self.left),
            round_to_int(self.top),
            round_to_int(self.right),
            round_to_int(self.bottom),
        )
    }
}

/// A width and a height in `Dp`, as Compose's `DpSize`: what `Modifier.size` takes, from a
/// `Dp` (square) or a `(width, height)` tuple.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DpSize {
    pub width: Dp,
    pub height: Dp,
}

impl DpSize {
    /// As `DpSize.Zero`.
    pub const ZERO: DpSize = DpSize::new(Dp::ZERO, Dp::ZERO);
    /// As `DpSize.Unspecified`.
    pub const UNSPECIFIED: DpSize = DpSize::new(Dp::UNSPECIFIED, Dp::UNSPECIFIED);

    pub const fn new(width: Dp, height: Dp) -> DpSize {
        DpSize { width, height }
    }

    /// Whether it's not [`UNSPECIFIED`](Self::UNSPECIFIED), as `DpSize.isSpecified`.
    pub fn is_specified(self) -> bool {
        self.width.is_specified() || self.height.is_specified()
    }

    /// Whether it's [`UNSPECIFIED`](Self::UNSPECIFIED), as `DpSize.isUnspecified`.
    pub fn is_unspecified(self) -> bool {
        !self.is_specified()
    }

    /// Itself if specified, else `or_else()`, as `takeOrElse`.
    pub fn take_or_else(self, or_else: impl FnOnce() -> DpSize) -> DpSize {
        if self.is_specified() { self } else { or_else() }
    }

    /// Half of it, as `DpSize.center`.
    pub fn center(self) -> DpOffset {
        DpOffset::new(self.width / 2.0, self.height / 2.0)
    }
}

impl fmt::Display for DpSize {
    /// As Compose's `toString`: `1.0.dp x 2.0.dp`, or `DpSize.Unspecified`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_specified() {
            write!(formatter, "{} x {}", self.width, self.height)
        } else {
            formatter.write_str("DpSize.Unspecified")
        }
    }
}

impl Add for DpSize {
    type Output = DpSize;
    fn add(self, other: DpSize) -> DpSize {
        DpSize::new(self.width + other.width, self.height + other.height)
    }
}

impl Sub for DpSize {
    type Output = DpSize;
    fn sub(self, other: DpSize) -> DpSize {
        DpSize::new(self.width - other.width, self.height - other.height)
    }
}

impl Mul<i32> for DpSize {
    type Output = DpSize;
    fn mul(self, other: i32) -> DpSize {
        DpSize::new(self.width * other, self.height * other)
    }
}

impl Mul<f32> for DpSize {
    type Output = DpSize;
    fn mul(self, other: f32) -> DpSize {
        DpSize::new(self.width * other, self.height * other)
    }
}

impl Mul<DpSize> for i32 {
    type Output = DpSize;
    fn mul(self, size: DpSize) -> DpSize {
        size * self
    }
}

impl Mul<DpSize> for f32 {
    type Output = DpSize;
    fn mul(self, size: DpSize) -> DpSize {
        size * self
    }
}

impl Div<i32> for DpSize {
    type Output = DpSize;
    fn div(self, other: i32) -> DpSize {
        DpSize::new(self.width / other, self.height / other)
    }
}

impl Div<f32> for DpSize {
    type Output = DpSize;
    fn div(self, other: f32) -> DpSize {
        DpSize::new(self.width / other, self.height / other)
    }
}

impl Lerp for DpSize {
    fn lerp(start: DpSize, stop: DpSize, fraction: f32) -> DpSize {
        DpSize::new(
            lerp(start.width, stop.width, fraction),
            lerp(start.height, stop.height, fraction),
        )
    }
}

/// A rectangle in `Dp`, as Compose's `DpRect`: what a test's bounds assertions read
/// (`getUnclippedBoundsInRoot`).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct DpRect {
    pub left: Dp,
    pub top: Dp,
    pub right: Dp,
    pub bottom: Dp,
}

impl DpRect {
    pub const fn new(left: Dp, top: Dp, right: Dp, bottom: Dp) -> DpRect {
        DpRect { left, top, right, bottom }
    }

    /// As `DpRect(origin, size)`.
    pub fn from_origin_size(origin: DpOffset, size: DpSize) -> DpRect {
        DpRect::new(origin.x, origin.y, origin.x + size.width, origin.y + size.height)
    }

    pub fn width(&self) -> Dp {
        Dp(self.right.0 - self.left.0)
    }

    pub fn height(&self) -> Dp {
        Dp(self.bottom.0 - self.top.0)
    }

    pub fn size(&self) -> DpSize {
        DpSize::new(self.width(), self.height())
    }
}

/// An offset in `Dp`, as Compose's `DpOffset`: what a menu's `offset` takes.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct DpOffset {
    pub x: Dp,
    pub y: Dp,
}

impl DpOffset {
    pub const ZERO: DpOffset = DpOffset { x: Dp::ZERO, y: Dp::ZERO };
    /// No value, as `DpOffset.Unspecified`.
    pub const UNSPECIFIED: DpOffset = DpOffset { x: Dp::UNSPECIFIED, y: Dp::UNSPECIFIED };

    pub const fn new(x: Dp, y: Dp) -> DpOffset {
        DpOffset { x, y }
    }

    /// Whether it has a value, as `isSpecified`: both coordinates do.
    pub fn is_specified(self) -> bool {
        self.x.is_specified() && self.y.is_specified()
    }

    pub fn is_unspecified(self) -> bool {
        !self.is_specified()
    }

    /// Itself if specified, else `or_else()`, as `takeOrElse`.
    pub fn take_or_else(self, or_else: impl FnOnce() -> DpOffset) -> DpOffset {
        if self.is_specified() { self } else { or_else() }
    }

    /// From `start` to `stop` by `fraction`, as `lerp(DpOffset, DpOffset, Float)`.
    pub fn lerp(start: DpOffset, stop: DpOffset, fraction: f32) -> DpOffset {
        DpOffset::new(lerp(start.x, stop.x, fraction), lerp(start.y, stop.y, fraction))
    }
}

impl Lerp for DpOffset {
    fn lerp(start: DpOffset, stop: DpOffset, fraction: f32) -> DpOffset {
        DpOffset::lerp(start, stop, fraction)
    }
}

impl fmt::Debug for DpOffset {
    /// As Compose's `toString`: `(1.0.dp, 2.0.dp)`, or `DpOffset.Unspecified`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_specified() {
            write!(formatter, "({:?}.dp, {:?}.dp)", self.x.0, self.y.0)
        } else {
            formatter.write_str("DpOffset.Unspecified")
        }
    }
}

impl ComposeArg for DpOffset {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Add for DpOffset {
    type Output = DpOffset;
    fn add(self, other: DpOffset) -> DpOffset {
        DpOffset::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for DpOffset {
    type Output = DpOffset;
    fn sub(self, other: DpOffset) -> DpOffset {
        DpOffset::new(self.x - other.x, self.y - other.y)
    }
}

impl From<(Dp, Dp)> for DpOffset {
    fn from((x, y): (Dp, Dp)) -> DpOffset {
        DpOffset::new(x, y)
    }
}

impl From<Dp> for DpSize {
    /// A square.
    fn from(size: Dp) -> DpSize {
        DpSize::new(size, size)
    }
}

impl From<(Dp, Dp)> for DpSize {
    fn from((width, height): (Dp, Dp)) -> DpSize {
        DpSize::new(width, height)
    }
}

/// A two-dimensional velocity, in pixels per second, as Compose's `Velocity`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Velocity {
    pub const ZERO: Velocity = Velocity { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Velocity {
        Velocity { x, y }
    }
}

impl fmt::Display for Velocity {
    /// As Compose's `toString`: `(3.0, -7.0) px/sec`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "({:?}, {:?}) px/sec", self.x, self.y)
    }
}

impl Add for Velocity {
    type Output = Velocity;
    fn add(self, other: Velocity) -> Velocity {
        Velocity::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Velocity {
    type Output = Velocity;
    fn sub(self, other: Velocity) -> Velocity {
        Velocity::new(self.x - other.x, self.y - other.y)
    }
}

impl Neg for Velocity {
    type Output = Velocity;
    fn neg(self) -> Velocity {
        Velocity::new(-self.x, -self.y)
    }
}

impl Mul<f32> for Velocity {
    type Output = Velocity;
    fn mul(self, operand: f32) -> Velocity {
        Velocity::new(self.x * operand, self.y * operand)
    }
}

impl Div<f32> for Velocity {
    type Output = Velocity;
    fn div(self, operand: f32) -> Velocity {
        Velocity::new(self.x / operand, self.y / operand)
    }
}

impl Rem<f32> for Velocity {
    type Output = Velocity;
    fn rem(self, operand: f32) -> Velocity {
        Velocity::new(self.x % operand, self.y % operand)
    }
}

impl Lerp for TextUnit {
    fn lerp(start: TextUnit, stop: TextUnit, fraction: f32) -> TextUnit {
        TextUnit::lerp(start, stop, fraction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dp_converts_with_density() {
        let density = Density::new(2.0);
        assert_eq!(density.round_to_px(16.dp()), 32);
        assert_eq!(density.round_to_px(Dp(0.3)), 1);
        assert_eq!(density.to_dp(10.0), 5.dp());
        assert_eq!(Density { density: 2.0, font_scale: 1.5 }.sp_to_px(10.sp()), 30.0);
        assert!(!Dp::UNSPECIFIED.is_specified());
        assert!(Dp::UNSPECIFIED.same(&Dp::UNSPECIFIED));
    }
}

macro_rules! compose_arg_by_value {
    ($($type:ty),*) => {
        $(impl ComposeArg for $type {
            fn same(&self, other: &Self) -> bool {
                self == other
            }
        })*
    };
}

compose_arg_by_value!(IntOffset, IntSize, IntRect, Offset, Size, Rect);
