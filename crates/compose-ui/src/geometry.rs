//! Geometry, as Compose's `androidx.compose.ui.geometry`: [`Offset`], [`Size`], [`Rect`],
//! [`MutableRect`], [`RoundRect`] and [`CornerRadius`].
//!
//! Compose packs `Offset`, `Size` and `CornerRadius` into a `Long`; compose-rs keeps two `f32`
//! fields, which is what a `Long` of two floats is to Rust, and reads the same (D1906). Their
//! `toString`s are their `Display`s; their `Debug`s are Rust's, which display list dumps print.

use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

use crate::math::{Lerp, lerp};

/// Equal, unspecified (NaN) included: unspecified equals unspecified, as Compose's value classes
/// compare their bits.
pub(crate) fn same_value(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// `value` with `digits` decimals, as Compose's internal `Float.toStringAsFixed`: halves rounded
/// up, then printed as Kotlin prints a float (`10.0`).
pub(crate) fn to_string_as_fixed(value: f32, digits: i32) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    let digits = digits.max(0);
    let pow = 10.0_f32.powi(digits);
    let shifted = value * pow;
    let decimal = shifted - (shifted as i32) as f32;
    let rounded_shifted = if decimal >= 0.5 { shifted as i32 + 1 } else { shifted as i32 };
    let rounded = rounded_shifted as f32 / pow;
    if digits > 0 { format!("{rounded:?}") } else { (rounded as i32).to_string() }
}

/// A floating-point position, as `Offset`: pointer positions and drawing. Equal when both
/// coordinates are, unspecified (NaN) included: `UNSPECIFIED` equals itself, as Compose's
/// packed value.
#[derive(Clone, Copy, Default, Debug)]
pub struct Offset {
    pub x: f32,
    pub y: f32,
}

impl PartialEq for Offset {
    fn eq(&self, other: &Self) -> bool {
        same_value(self.x, other.x) && same_value(self.y, other.y)
    }
}

impl Offset {
    pub const ZERO: Offset = Offset { x: 0.0, y: 0.0 };
    /// Infinitely far, as `Offset.Infinite`: a gradient's default end (the drawing's size).
    pub const INFINITE: Offset = Offset { x: f32::INFINITY, y: f32::INFINITY };
    /// No position, as `Offset.Unspecified`: a radial gradient's default center (the
    /// drawing's center).
    pub const UNSPECIFIED: Offset = Offset { x: f32::NAN, y: f32::NAN };

    pub const fn new(x: f32, y: f32) -> Offset {
        Offset { x, y }
    }

    /// Whether it's a position (not `UNSPECIFIED`), as `isSpecified`.
    pub fn is_specified(self) -> bool {
        !self.x.is_nan() && !self.y.is_nan()
    }

    /// Whether it's [`UNSPECIFIED`](Self::UNSPECIFIED), as `isUnspecified`.
    pub fn is_unspecified(self) -> bool {
        !self.is_specified()
    }

    /// Neither coordinate is NaN (an infinite one is valid), as `isValid`.
    pub fn is_valid(self) -> bool {
        !self.x.is_nan() && !self.y.is_nan()
    }

    /// Neither coordinate is infinite or NaN, as `isFinite`.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Itself if specified, else `or_else()`, as `takeOrElse`.
    pub fn take_or_else(self, or_else: impl FnOnce() -> Offset) -> Offset {
        if self.is_specified() { self } else { or_else() }
    }

    /// Its distance from the origin, as `getDistance`.
    pub fn get_distance(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// The square of [`get_distance`](Self::get_distance), as `getDistanceSquared`.
    pub fn get_distance_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
}

impl fmt::Display for Offset {
    /// As Compose's `toString`: `Offset(1.0, 2.0)`, or `Offset.Unspecified`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_specified() {
            let (x, y) = (to_string_as_fixed(self.x, 1), to_string_as_fixed(self.y, 1));
            write!(formatter, "Offset({x}, {y})")
        } else {
            formatter.write_str("Offset.Unspecified")
        }
    }
}

impl Add for Offset {
    type Output = Offset;
    fn add(self, other: Offset) -> Offset {
        Offset::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Offset {
    type Output = Offset;
    fn sub(self, other: Offset) -> Offset {
        Offset::new(self.x - other.x, self.y - other.y)
    }
}

impl Neg for Offset {
    type Output = Offset;
    fn neg(self) -> Offset {
        Offset::new(-self.x, -self.y)
    }
}

impl Mul<f32> for Offset {
    type Output = Offset;
    fn mul(self, operand: f32) -> Offset {
        Offset::new(self.x * operand, self.y * operand)
    }
}

impl Mul<Offset> for f32 {
    type Output = Offset;
    fn mul(self, offset: Offset) -> Offset {
        offset * self
    }
}

impl Div<f32> for Offset {
    type Output = Offset;
    fn div(self, operand: f32) -> Offset {
        Offset::new(self.x / operand, self.y / operand)
    }
}

/// Each coordinate's remainder, as Compose's `rem` (Kotlin's `%` on floats is Rust's).
impl Rem<f32> for Offset {
    type Output = Offset;
    fn rem(self, operand: f32) -> Offset {
        Offset::new(self.x % operand, self.y % operand)
    }
}

impl Lerp for Offset {
    fn lerp(start: Offset, stop: Offset, fraction: f32) -> Offset {
        Offset::new(lerp(start.x, stop.x, fraction), lerp(start.y, stop.y, fraction))
    }
}

/// A floating-point size, as `Size`.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size { width: 0.0, height: 0.0 };
    /// As `Size.Unspecified`: neither a width nor a height.
    pub const UNSPECIFIED: Size = Size { width: f32::NAN, height: f32::NAN };

    pub const fn new(width: f32, height: f32) -> Size {
        Size { width, height }
    }

    /// Whether it's not [`UNSPECIFIED`](Self::UNSPECIFIED) (both sides NaN), as `isSpecified`.
    pub fn is_specified(self) -> bool {
        !self.is_unspecified()
    }

    /// Whether both sides are NaN, as `isUnspecified`.
    pub fn is_unspecified(self) -> bool {
        self.width.is_nan() && self.height.is_nan()
    }

    /// Itself if specified, else `or_else()`, as `takeOrElse`.
    pub fn take_or_else(self, or_else: impl FnOnce() -> Size) -> Size {
        if self.is_specified() { self } else { or_else() }
    }

    /// Unspecified, or a side not positive, as `isEmpty`.
    pub fn is_empty(self) -> bool {
        self.is_unspecified() || self.width <= 0.0 || self.height <= 0.0
    }

    /// The shorter side's magnitude, as `minDimension`.
    pub fn min_dimension(self) -> f32 {
        self.width.abs().min(self.height.abs())
    }

    /// The longer side's magnitude, as `maxDimension`.
    pub fn max_dimension(self) -> f32 {
        self.width.abs().max(self.height.abs())
    }

    /// Half of it, as `Size.center`.
    pub fn center(self) -> Offset {
        Offset::new(self.width / 2.0, self.height / 2.0)
    }

    /// The rect from the origin of this size, as `Size.toRect`.
    pub fn to_rect(self) -> Rect {
        Rect::from_offset_size(Offset::ZERO, self)
    }
}

impl fmt::Display for Size {
    /// As Compose's `toString`: `Size(10.0, 20.0)`, or `Size.Unspecified`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_specified() {
            let width = to_string_as_fixed(self.width, 1);
            let height = to_string_as_fixed(self.height, 1);
            write!(formatter, "Size({width}, {height})")
        } else {
            formatter.write_str("Size.Unspecified")
        }
    }
}

impl Mul<f32> for Size {
    type Output = Size;
    fn mul(self, operand: f32) -> Size {
        Size::new(self.width * operand, self.height * operand)
    }
}

impl Mul<Size> for f32 {
    type Output = Size;
    fn mul(self, size: Size) -> Size {
        size * self
    }
}

impl Div<f32> for Size {
    type Output = Size;
    fn div(self, operand: f32) -> Size {
        Size::new(self.width / operand, self.height / operand)
    }
}

impl Lerp for Size {
    fn lerp(start: Size, stop: Size, fraction: f32) -> Size {
        Size::new(
            lerp(start.width, stop.width, fraction),
            lerp(start.height, stop.height, fraction),
        )
    }
}

/// A floating-point rectangle, as `Rect`.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Whether `value` is neither infinite nor NaN, by its bits as Compose's `isFinite`.
fn is_finite_bits(value: f32) -> bool {
    (value.to_bits() & 0x7fff_ffff) < 0x7f80_0000
}

impl Rect {
    /// As `Rect.Zero`.
    pub const ZERO: Rect = Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        Rect { left, top, right, bottom }
    }

    /// As `Rect(offset, size)`.
    pub fn from_offset_size(offset: Offset, size: Size) -> Rect {
        Rect::new(offset.x, offset.y, offset.x + size.width, offset.y + size.height)
    }

    /// As `Rect(topLeft, bottomRight)`.
    pub const fn from_corners(top_left: Offset, bottom_right: Offset) -> Rect {
        Rect::new(top_left.x, top_left.y, bottom_right.x, bottom_right.y)
    }

    /// The square around `center` with sides `2 × radius`, as `Rect(center, radius)`.
    pub fn from_center(center: Offset, radius: f32) -> Rect {
        Rect::new(center.x - radius, center.y - radius, center.x + radius, center.y + radius)
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// As `size`.
    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    /// A side is at positive infinity, as `isInfinite`.
    pub fn is_infinite(&self) -> bool {
        self.left == f32::INFINITY
            || self.top == f32::INFINITY
            || self.right == f32::INFINITY
            || self.bottom == f32::INFINITY
    }

    /// No side is infinite or NaN, as `isFinite`.
    pub fn is_finite(&self) -> bool {
        is_finite_bits(self.left)
            && is_finite_bits(self.top)
            && is_finite_bits(self.right)
            && is_finite_bits(self.bottom)
    }

    /// No area (a side at or past its opposite), as `isEmpty`.
    pub fn is_empty(&self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }

    /// Moved by `offset`, as `translate(offset)`.
    pub fn translate(&self, offset: Offset) -> Rect {
        self.translate_xy(offset.x, offset.y)
    }

    /// Moved by (`x`, `y`), as `translate(translateX, translateY)`.
    pub fn translate_xy(&self, x: f32, y: f32) -> Rect {
        Rect::new(self.left + x, self.top + y, self.right + x, self.bottom + y)
    }

    /// Every side moved out by `delta`, as `inflate`.
    pub fn inflate(&self, delta: f32) -> Rect {
        Rect::new(self.left - delta, self.top - delta, self.right + delta, self.bottom + delta)
    }

    /// Every side moved in by `delta`, as `deflate`.
    pub fn deflate(&self, delta: f32) -> Rect {
        self.inflate(-delta)
    }

    /// What it and `other` both cover (empty if nothing), as `intersect(other)`.
    pub fn intersect(&self, other: Rect) -> Rect {
        self.intersect_ltrb(other.left, other.top, other.right, other.bottom)
    }

    /// As `intersect(otherLeft, otherTop, otherRight, otherBottom)`.
    pub fn intersect_ltrb(&self, left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        Rect::new(
            self.left.max(left),
            self.top.max(top),
            self.right.min(right),
            self.bottom.min(bottom),
        )
    }

    /// Whether it and `other` share some area (touching isn't), as `overlaps`.
    pub fn overlaps(&self, other: Rect) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }

    /// The shorter side's magnitude, as `minDimension`.
    pub fn min_dimension(&self) -> f32 {
        self.width().abs().min(self.height().abs())
    }

    /// The longer side's magnitude, as `maxDimension`.
    pub fn max_dimension(&self) -> f32 {
        self.width().abs().max(self.height().abs())
    }

    pub fn top_left(&self) -> Offset {
        Offset::new(self.left, self.top)
    }

    pub fn top_center(&self) -> Offset {
        Offset::new(self.left + self.width() / 2.0, self.top)
    }

    pub fn top_right(&self) -> Offset {
        Offset::new(self.right, self.top)
    }

    pub fn center_left(&self) -> Offset {
        Offset::new(self.left, self.top + self.height() / 2.0)
    }

    pub fn center(&self) -> Offset {
        Offset::new(self.left + self.width() / 2.0, self.top + self.height() / 2.0)
    }

    pub fn center_right(&self) -> Offset {
        Offset::new(self.right, self.top + self.height() / 2.0)
    }

    pub fn bottom_left(&self) -> Offset {
        Offset::new(self.left, self.bottom)
    }

    pub fn bottom_center(&self) -> Offset {
        Offset::new(self.left + self.width() / 2.0, self.bottom)
    }

    pub fn bottom_right(&self) -> Offset {
        Offset::new(self.right, self.bottom)
    }

    /// Whether `point` is inside: its left and top edges are, its right and bottom ones aren't,
    /// as `contains`.
    pub fn contains(&self, point: Offset) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }
}

impl fmt::Display for Rect {
    /// As Compose's `toString`: `Rect.fromLTRB(1.0, 2.0, 3.0, 4.0)`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Rect.fromLTRB({}, {}, {}, {})",
            to_string_as_fixed(self.left, 1),
            to_string_as_fixed(self.top, 1),
            to_string_as_fixed(self.right, 1),
            to_string_as_fixed(self.bottom, 1)
        )
    }
}

impl Lerp for Rect {
    fn lerp(start: Rect, stop: Rect, fraction: f32) -> Rect {
        Rect::new(
            lerp(start.left, stop.left, fraction),
            lerp(start.top, stop.top, fraction),
            lerp(start.right, stop.right, fraction),
            lerp(start.bottom, stop.bottom, fraction),
        )
    }
}

/// A rectangle changed in place, as Compose's `MutableRect`: what a matrix maps without making
/// a new one.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct MutableRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl MutableRect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> MutableRect {
        MutableRect { left, top, right, bottom }
    }

    /// As `MutableRect(offset, size)`.
    pub fn from_offset_size(offset: Offset, size: Size) -> MutableRect {
        MutableRect::new(offset.x, offset.y, offset.x + size.width, offset.y + size.height)
    }

    /// As `MutableRect(topLeft, bottomRight)`.
    pub const fn from_corners(top_left: Offset, bottom_right: Offset) -> MutableRect {
        MutableRect::new(top_left.x, top_left.y, bottom_right.x, bottom_right.y)
    }

    /// As `MutableRect(center, radius)`.
    pub fn from_center(center: Offset, radius: f32) -> MutableRect {
        MutableRect::new(center.x - radius, center.y - radius, center.x + radius, center.y + radius)
    }

    /// Its sides now, as `toRect`.
    pub const fn to_rect(&self) -> Rect {
        Rect::new(self.left, self.top, self.right, self.bottom)
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    pub fn is_infinite(&self) -> bool {
        self.to_rect().is_infinite()
    }

    pub fn is_finite(&self) -> bool {
        self.to_rect().is_finite()
    }

    pub fn is_empty(&self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }

    /// Moves it by `offset`, as `translate(offset)`.
    pub fn translate(&mut self, offset: Offset) {
        self.translate_xy(offset.x, offset.y);
    }

    /// Moves it by (`x`, `y`), as `translate(translateX, translateY)`.
    pub fn translate_xy(&mut self, x: f32, y: f32) {
        self.left += x;
        self.top += y;
        self.right += x;
        self.bottom += y;
    }

    /// Moves every side out by `delta`, as `inflate`.
    pub fn inflate(&mut self, delta: f32) {
        self.left -= delta;
        self.top -= delta;
        self.right += delta;
        self.bottom += delta;
    }

    /// Moves every side in by `delta`, as `deflate`.
    pub fn deflate(&mut self, delta: f32) {
        self.inflate(-delta);
    }

    /// Keeps what it and the other rectangle both cover, as `intersect`.
    pub fn intersect(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.left = left.max(self.left);
        self.top = top.max(self.top);
        self.right = right.min(self.right);
        self.bottom = bottom.min(self.bottom);
    }

    /// Whether it and `other` share some area, as `overlaps(MutableRect)`.
    pub fn overlaps(&self, other: &MutableRect) -> bool {
        !(self.right <= other.left
            || other.right <= self.left
            || self.bottom <= other.top
            || other.bottom <= self.top)
    }

    /// Whether it and `other` share some area, as `overlaps(Rect)`.
    pub fn overlaps_rect(&self, other: Rect) -> bool {
        self.to_rect().overlaps(other)
    }

    pub fn min_dimension(&self) -> f32 {
        self.to_rect().min_dimension()
    }

    pub fn max_dimension(&self) -> f32 {
        self.to_rect().max_dimension()
    }

    pub fn top_left(&self) -> Offset {
        self.to_rect().top_left()
    }

    pub fn top_center(&self) -> Offset {
        self.to_rect().top_center()
    }

    pub fn top_right(&self) -> Offset {
        self.to_rect().top_right()
    }

    pub fn center_left(&self) -> Offset {
        self.to_rect().center_left()
    }

    pub fn center(&self) -> Offset {
        self.to_rect().center()
    }

    pub fn center_right(&self) -> Offset {
        self.to_rect().center_right()
    }

    pub fn bottom_left(&self) -> Offset {
        self.to_rect().bottom_left()
    }

    pub fn bottom_center(&self) -> Offset {
        self.to_rect().bottom_center()
    }

    pub fn bottom_right(&self) -> Offset {
        self.to_rect().bottom_right()
    }

    /// As [`Rect::contains`].
    pub fn contains(&self, point: Offset) -> bool {
        self.to_rect().contains(point)
    }

    /// Replaces every side, as `set`.
    pub fn set(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        *self = MutableRect::new(left, top, right, bottom);
    }
}

impl fmt::Display for MutableRect {
    /// As Compose's `toString`: `MutableRect(1.0, 2.0, 3.0, 4.0)`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "MutableRect({}, {}, {}, {})",
            to_string_as_fixed(self.left, 1),
            to_string_as_fixed(self.top, 1),
            to_string_as_fixed(self.right, 1),
            to_string_as_fixed(self.bottom, 1)
        )
    }
}

/// A corner's radii, horizontal and vertical, as Compose's `CornerRadius`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CornerRadius {
    pub x: f32,
    pub y: f32,
}

impl CornerRadius {
    pub const ZERO: CornerRadius = CornerRadius { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> CornerRadius {
        CornerRadius { x, y }
    }

    /// The same radius both ways, as `CornerRadius(radius)`.
    pub const fn circular(radius: f32) -> CornerRadius {
        CornerRadius { x: radius, y: radius }
    }

    /// Either radius (or both) is zero, of either sign, as `isZero`: such a corner is square.
    pub fn is_zero(self) -> bool {
        self.x == 0.0 || self.y == 0.0
    }

    /// Both radii have the same bits, as `isCircular`.
    pub fn is_circular(self) -> bool {
        self.x.to_bits() == self.y.to_bits()
    }

    /// The same bits, as Compose compares two radii's packed values.
    fn same_bits(self, other: CornerRadius) -> bool {
        self.x.to_bits() == other.x.to_bits() && self.y.to_bits() == other.y.to_bits()
    }
}

impl fmt::Display for CornerRadius {
    /// As Compose's `toString`: `CornerRadius.circular(1.0)`, or
    /// `CornerRadius.elliptical(1.0, 2.0)`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = to_string_as_fixed(self.x, 1);
        if self.x == self.y {
            write!(formatter, "CornerRadius.circular({x})")
        } else {
            write!(formatter, "CornerRadius.elliptical({x}, {})", to_string_as_fixed(self.y, 1))
        }
    }
}

impl Add for CornerRadius {
    type Output = CornerRadius;
    fn add(self, other: CornerRadius) -> CornerRadius {
        CornerRadius::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for CornerRadius {
    type Output = CornerRadius;
    fn sub(self, other: CornerRadius) -> CornerRadius {
        CornerRadius::new(self.x - other.x, self.y - other.y)
    }
}

impl Neg for CornerRadius {
    type Output = CornerRadius;
    fn neg(self) -> CornerRadius {
        CornerRadius::new(-self.x, -self.y)
    }
}

impl Mul<f32> for CornerRadius {
    type Output = CornerRadius;
    fn mul(self, operand: f32) -> CornerRadius {
        CornerRadius::new(self.x * operand, self.y * operand)
    }
}

impl Div<f32> for CornerRadius {
    type Output = CornerRadius;
    fn div(self, operand: f32) -> CornerRadius {
        CornerRadius::new(self.x / operand, self.y / operand)
    }
}

impl Lerp for CornerRadius {
    fn lerp(start: CornerRadius, stop: CornerRadius, fraction: f32) -> CornerRadius {
        CornerRadius::new(lerp(start.x, stop.x, fraction), lerp(start.y, stop.y, fraction))
    }
}

/// A rectangle with rounded corners, as Compose's `RoundRect`: each corner's radii. Made with a
/// struct literal, as Compose's primary constructor (`..RoundRect::ZERO` for the radii it leaves
/// at zero), or [`new`](Self::new), [`from_ltrb`](Self::from_ltrb) and
/// [`with_radii`](Self::with_radii) for its other constructors.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RoundRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub top_left_corner_radius: CornerRadius,
    pub top_right_corner_radius: CornerRadius,
    pub bottom_right_corner_radius: CornerRadius,
    pub bottom_left_corner_radius: CornerRadius,
}

impl RoundRect {
    /// As `RoundRect.Zero`.
    pub const ZERO: RoundRect = RoundRect::from_ltrb(0.0, 0.0, 0.0, 0.0, CornerRadius::ZERO);

    /// `rect` with every corner rounded by `corner_radius`, as `RoundRect(rect, cornerRadius)`
    /// (and `RoundRect(rect, radiusX, radiusY)` with `CornerRadius::new(radiusX, radiusY)`).
    pub const fn new(rect: Rect, corner_radius: CornerRadius) -> RoundRect {
        RoundRect::from_ltrb(rect.left, rect.top, rect.right, rect.bottom, corner_radius)
    }

    /// As `RoundRect(left, top, right, bottom, cornerRadius)` (and its `radiusX, radiusY` form).
    pub const fn from_ltrb(
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        corner_radius: CornerRadius,
    ) -> RoundRect {
        RoundRect {
            left,
            top,
            right,
            bottom,
            top_left_corner_radius: corner_radius,
            top_right_corner_radius: corner_radius,
            bottom_right_corner_radius: corner_radius,
            bottom_left_corner_radius: corner_radius,
        }
    }

    /// `rect` with each corner's radii, as `RoundRect(rect, topLeft, topRight, bottomRight,
    /// bottomLeft)`.
    pub const fn with_radii(
        rect: Rect,
        top_left: CornerRadius,
        top_right: CornerRadius,
        bottom_right: CornerRadius,
        bottom_left: CornerRadius,
    ) -> RoundRect {
        RoundRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            top_left_corner_radius: top_left,
            top_right_corner_radius: top_right,
            bottom_right_corner_radius: bottom_right,
            bottom_left_corner_radius: bottom_left,
        }
    }

    /// Its bounds, as `boundingRect`.
    pub const fn rect(&self) -> Rect {
        Rect::new(self.left, self.top, self.right, self.bottom)
    }

    /// Its bounds, as `RoundRect.boundingRect`.
    pub const fn bounding_rect(&self) -> Rect {
        self.rect()
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// The scale Compose's `scaledRadiiRect` applies to every radius so that two radii sharing a
    /// side fit in it.
    fn radii_scale(&self) -> f32 {
        fn min_radius(min: f32, radius1: f32, radius2: f32, limit: f32) -> f32 {
            let sum = radius1 + radius2;
            if sum > limit && sum != 0.0 { min.min(limit / sum) } else { min }
        }
        let (width, height) = (self.width(), self.height());
        let mut scale = 1.0;
        scale = min_radius(
            scale,
            self.bottom_left_corner_radius.y,
            self.top_left_corner_radius.y,
            height,
        );
        scale =
            min_radius(scale, self.top_left_corner_radius.x, self.top_right_corner_radius.x, width);
        scale = min_radius(
            scale,
            self.top_right_corner_radius.y,
            self.bottom_right_corner_radius.y,
            height,
        );
        min_radius(
            scale,
            self.bottom_right_corner_radius.x,
            self.bottom_left_corner_radius.x,
            width,
        )
    }

    /// Whether `point` is inside, its corners' radii scaled down to fit together as Compose's,
    /// as `contains`.
    pub fn contains(&self, point: Offset) -> bool {
        if point.x < self.left
            || point.x >= self.right
            || point.y < self.top
            || point.y >= self.bottom
        {
            return false;
        }
        let scale = self.radii_scale();
        let top_left = self.top_left_corner_radius * scale;
        let top_right = self.top_right_corner_radius * scale;
        let bottom_right = self.bottom_right_corner_radius * scale;
        let bottom_left = self.bottom_left_corner_radius * scale;
        let (x, y, radius) = if point.x < self.left + top_left.x && point.y < self.top + top_left.y
        {
            (point.x - self.left - top_left.x, point.y - self.top - top_left.y, top_left)
        } else if point.x > self.right - top_right.x && point.y < self.top + top_right.y {
            (point.x - self.right + top_right.x, point.y - self.top - top_right.y, top_right)
        } else if point.x > self.right - bottom_right.x && point.y > self.bottom - bottom_right.y {
            let x = point.x - self.right + bottom_right.x;
            (x, point.y - self.bottom + bottom_right.y, bottom_right)
        } else if point.x < self.left + bottom_left.x && point.y > self.bottom - bottom_left.y {
            (
                point.x - self.left - bottom_left.x,
                point.y - self.bottom + bottom_left.y,
                bottom_left,
            )
        } else {
            return true;
        };
        let (x, y) = (x / radius.x, y / radius.y);
        x * x + y * y <= 1.0
    }

    /// The largest rect inside it that its corners leave whole (each inset by the radius times
    /// 1 − cos(π/4)), as `safeInnerRect`.
    pub fn safe_inner_rect(&self) -> Rect {
        const INSET_FACTOR: f32 = 0.292_893_2;
        let left = self.bottom_left_corner_radius.x.max(self.top_left_corner_radius.x);
        let top = self.top_left_corner_radius.y.max(self.top_right_corner_radius.y);
        let right = self.top_right_corner_radius.x.max(self.bottom_right_corner_radius.x);
        let bottom = self.bottom_right_corner_radius.y.max(self.bottom_left_corner_radius.y);
        Rect::new(
            self.left + left * INSET_FACTOR,
            self.top + top * INSET_FACTOR,
            self.right - right * INSET_FACTOR,
            self.bottom - bottom * INSET_FACTOR,
        )
    }

    /// No area, as `isEmpty`.
    pub fn is_empty(&self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }

    /// No side infinite or NaN, as `isFinite`.
    pub fn is_finite(&self) -> bool {
        self.rect().is_finite()
    }

    /// Every radius zero, as `isRect`.
    pub fn is_rect(&self) -> bool {
        self.top_left_corner_radius.is_zero()
            && self.top_right_corner_radius.is_zero()
            && self.bottom_left_corner_radius.is_zero()
            && self.bottom_right_corner_radius.is_zero()
    }

    /// One radius for every corner, reaching at least the middle of each side, as `isEllipse`.
    pub fn is_ellipse(&self) -> bool {
        let top_left = self.top_left_corner_radius;
        top_left.same_bits(self.top_right_corner_radius)
            && self.top_right_corner_radius.same_bits(self.bottom_right_corner_radius)
            && self.bottom_right_corner_radius.same_bits(self.bottom_left_corner_radius)
            && f64::from(self.width()) <= 2.0 * f64::from(top_left.x)
            && f64::from(self.height()) <= 2.0 * f64::from(top_left.y)
    }

    /// A square ellipse, as `isCircle`.
    pub fn is_circle(&self) -> bool {
        self.width() == self.height() && self.is_ellipse()
    }

    /// One circular radius for every corner, as `isSimple`.
    pub fn is_simple(&self) -> bool {
        let top_left = self.top_left_corner_radius;
        top_left.is_circular()
            && top_left.same_bits(self.top_right_corner_radius)
            && top_left.same_bits(self.bottom_right_corner_radius)
            && top_left.same_bits(self.bottom_left_corner_radius)
    }

    pub fn min_dimension(&self) -> f32 {
        self.width().abs().min(self.height().abs())
    }

    pub fn max_dimension(&self) -> f32 {
        self.width().abs().max(self.height().abs())
    }

    pub fn center(&self) -> Offset {
        Offset::new(self.left + self.width() / 2.0, self.top + self.height() / 2.0)
    }

    /// Moved by `offset`, its radii kept, as `RoundRect.translate`.
    pub fn translate(&self, offset: Offset) -> RoundRect {
        RoundRect {
            left: self.left + offset.x,
            top: self.top + offset.y,
            right: self.right + offset.x,
            bottom: self.bottom + offset.y,
            ..*self
        }
    }
}

impl fmt::Display for RoundRect {
    /// As Compose's `toString`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rect = format!(
            "{}, {}, {}, {}",
            to_string_as_fixed(self.left, 1),
            to_string_as_fixed(self.top, 1),
            to_string_as_fixed(self.right, 1),
            to_string_as_fixed(self.bottom, 1)
        );
        let (top_left, top_right) = (self.top_left_corner_radius, self.top_right_corner_radius);
        let (bottom_right, bottom_left) =
            (self.bottom_right_corner_radius, self.bottom_left_corner_radius);
        if top_left == top_right && top_right == bottom_right && bottom_right == bottom_left {
            let x = to_string_as_fixed(top_left.x, 1);
            if top_left.x == top_left.y {
                return write!(formatter, "RoundRect(rect={rect}, radius={x})");
            }
            let y = to_string_as_fixed(top_left.y, 1);
            return write!(formatter, "RoundRect(rect={rect}, x={x}, y={y})");
        }
        write!(
            formatter,
            "RoundRect(rect={rect}, topLeft={top_left}, topRight={top_right}, \
             bottomRight={bottom_right}, bottomLeft={bottom_left})"
        )
    }
}

impl Lerp for RoundRect {
    fn lerp(start: RoundRect, stop: RoundRect, fraction: f32) -> RoundRect {
        RoundRect {
            left: lerp(start.left, stop.left, fraction),
            top: lerp(start.top, stop.top, fraction),
            right: lerp(start.right, stop.right, fraction),
            bottom: lerp(start.bottom, stop.bottom, fraction),
            top_left_corner_radius: lerp(
                start.top_left_corner_radius,
                stop.top_left_corner_radius,
                fraction,
            ),
            top_right_corner_radius: lerp(
                start.top_right_corner_radius,
                stop.top_right_corner_radius,
                fraction,
            ),
            bottom_right_corner_radius: lerp(
                start.bottom_right_corner_radius,
                stop.bottom_right_corner_radius,
                fraction,
            ),
            bottom_left_corner_radius: lerp(
                start.bottom_left_corner_radius,
                stop.bottom_left_corner_radius,
                fraction,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_contain_their_top_left_but_not_their_bottom_right() {
        let rect = Rect::new(0.0, 0.0, 10.0, 5.0);
        assert!(rect.contains(Offset::new(0.0, 0.0)));
        assert!(rect.contains(Offset::new(9.9, 4.9)));
        assert!(!rect.contains(Offset::new(10.0, 2.0)));
    }

    #[test]
    fn fixed_decimals_round_halves_up() {
        assert_eq!(to_string_as_fixed(2.25, 1), "2.3");
        assert_eq!(to_string_as_fixed(10.0, 1), "10.0");
        assert_eq!(to_string_as_fixed(2.5, 0), "3");
        assert_eq!(to_string_as_fixed(f32::NEG_INFINITY, 1), "-Infinity");
    }
}
