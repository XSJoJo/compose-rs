//! Colors, shapes and paint, as Compose's `androidx.compose.ui.graphics`.

use std::fmt;

use compose_runtime::ComposeArg;

use crate::colorspace::half::{float_to_half, half_to_float};
use crate::colorspace::{ColorModel, ColorSpace, ColorSpaces, MIN_ID, Rgb, coerce_in};
use crate::unit::{Density, Dp, Rect, Size};

/// A color, as Compose's `Color`: 64 bits holding its components and its color space
/// (docs/parity/design/color-spaces.md). An sRGB color is its ARGB in the high 32 bits (its
/// space's id, the low 6 bits, being 0); a color of another space is red, green and blue as
/// half floats, alpha in 10 bits and the space's id. `Color::UNSPECIFIED` means "use the
/// default".
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(u64);

impl Color {
    pub const BLACK: Color = Color::from_argb(0xFF00_0000);
    pub const WHITE: Color = Color::from_argb(0xFFFF_FFFF);
    pub const RED: Color = Color::from_argb(0xFFFF_0000);
    pub const GREEN: Color = Color::from_argb(0xFF00_FF00);
    pub const BLUE: Color = Color::from_argb(0xFF00_00FF);
    pub const GRAY: Color = Color::from_argb(0xFF88_8888);
    pub const LIGHT_GRAY: Color = Color::from_argb(0xFFCC_CCCC);
    pub const DARK_GRAY: Color = Color::from_argb(0xFF44_4444);
    pub const YELLOW: Color = Color::from_argb(0xFFFF_FF00);
    pub const CYAN: Color = Color::from_argb(0xFF00_FFFF);
    pub const MAGENTA: Color = Color::from_argb(0xFFFF_00FF);
    pub const TRANSPARENT: Color = Color::from_argb(0x0000_0000);
    /// No color: as `Color.Unspecified`, transparent black in the unspecified color space
    /// (Compose's `0x10`).
    pub const UNSPECIFIED: Color = Color(0x10);

    /// From packed sRGB ARGB, as `Color(0xFF6200EE)`.
    pub const fn from_argb(argb: u32) -> Color {
        Color((argb as u64) << 32)
    }

    /// Opaque, from sRGB bytes.
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Color {
        Color::from_rgba(red, green, blue, 0xFF)
    }

    /// From sRGB bytes, as `Color(red: Int, green: Int, blue: Int, alpha: Int)`.
    pub const fn from_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        Color::from_argb(
            (alpha as u32) << 24 | (red as u32) << 16 | (green as u32) << 8 | blue as u32,
        )
    }

    /// From its packed value, as `Color(value: ULong)`.
    pub const fn from_value(value: u64) -> Color {
        Color(value)
    }

    /// Its packed value, as `Color.value`.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// An sRGB color from components in 0 to 1 (kept in it), as
    /// `Color(red, green, blue, alpha)`.
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
        Color::pack_srgb(
            coerce_in(red, 0.0, 1.0),
            coerce_in(green, 0.0, 1.0),
            coerce_in(blue, 0.0, 1.0),
            coerce_in(alpha, 0.0, 1.0),
        )
    }

    /// A color of `color_space` (one of [`ColorSpaces`]), its components kept in the space's
    /// range and alpha in 0 to 1, as `Color(red, green, blue, alpha, colorSpace)`.
    ///
    /// # Panics
    ///
    /// If the space doesn't have three components, or isn't a named one, as Compose throws.
    pub fn new_with(
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
        color_space: &ColorSpace,
    ) -> Color {
        if color_space.is_srgb() {
            return Color::new(red, green, blue, alpha);
        }
        assert!(
            color_space.component_count() == 3,
            "compose-ui: Color only works with ColorSpaces with 3 components"
        );
        assert!(
            color_space.id() != MIN_ID,
            "compose-ui: unknown color space, please use a color space in ColorSpaces"
        );
        let component = |value: f32, index: usize| {
            float_to_half(coerce_in(
                value,
                color_space.min_value(index),
                color_space.max_value(index),
            ))
        };
        Color::pack_wide(
            component(red, 0),
            component(green, 1),
            component(blue, 2),
            coerce_in(alpha, 0.0, 1.0),
            color_space.id(),
        )
    }

    /// A color of `color_space` from components not kept in a range, as Compose's internal
    /// `UncheckedColor`.
    pub(crate) fn unchecked(
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
        color_space: &ColorSpace,
    ) -> Color {
        if color_space.is_srgb() {
            return Color::pack_srgb(red, green, blue, alpha);
        }
        let alpha = if alpha.is_nan() { alpha } else { alpha.clamp(0.0, 1.0) };
        Color::pack_wide(
            float_to_half(red),
            float_to_half(green),
            float_to_half(blue),
            alpha,
            color_space.id(),
        )
    }

    /// sRGB components packed as Compose packs them (each rounded half up, not kept in a
    /// range: callers keep them).
    fn pack_srgb(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
        let byte = |value: f32| (value * 255.0 + 0.5) as i32;
        let argb = byte(alpha) << 24 | byte(red) << 16 | byte(green) << 8 | byte(blue);
        Color((argb as u32 as u64) << 32)
    }

    fn pack_wide(red: u16, green: u16, blue: u16, alpha: f32, id: i32) -> Color {
        let alpha = (alpha * 1023.0 + 0.5) as i32;
        Color(
            u64::from(red) << 48
                | u64::from(green) << 32
                | u64::from(blue) << 16
                | (alpha as u64 & 0x3ff) << 6
                | (id as u64 & 0x3f),
        )
    }

    /// Whether it's packed as sRGB (its space's id 0): the fast path.
    #[inline]
    const fn is_packed_srgb(self) -> bool {
        self.0 & 0x3f == 0
    }

    /// Its red component, in its color space's range, as `red`.
    pub fn red(self) -> f32 {
        if self.is_packed_srgb() {
            ((self.0 >> 48) & 0xff) as f32 / 255.0
        } else {
            half_to_float((self.0 >> 48) as u16)
        }
    }

    /// Its green component, as `green`.
    pub fn green(self) -> f32 {
        if self.is_packed_srgb() {
            ((self.0 >> 40) & 0xff) as f32 / 255.0
        } else {
            half_to_float((self.0 >> 32) as u16)
        }
    }

    /// Its blue component, as `blue`.
    pub fn blue(self) -> f32 {
        if self.is_packed_srgb() {
            ((self.0 >> 32) & 0xff) as f32 / 255.0
        } else {
            half_to_float((self.0 >> 16) as u16)
        }
    }

    /// Its alpha, 0 to 1, as `alpha`.
    pub fn alpha(self) -> f32 {
        if self.is_packed_srgb() {
            ((self.0 >> 56) & 0xff) as f32 / 255.0
        } else {
            ((self.0 >> 6) & 0x3ff) as f32 / 1023.0
        }
    }

    /// Its color space, as `colorSpace`.
    pub fn color_space(self) -> &'static ColorSpace {
        let id = (self.0 & 0x3f) as usize;
        ColorSpaces::get(id)
            .unwrap_or_else(|| panic!("compose-ui: a color of an unknown color space ({id})"))
    }

    /// Red, green, blue, alpha and its color space, as Kotlin's destructuring
    /// (`val (red, green, blue, alpha, colorSpace) = color`).
    pub fn components(self) -> (f32, f32, f32, f32, &'static ColorSpace) {
        (self.red(), self.green(), self.blue(), self.alpha(), self.color_space())
    }

    /// The same color in `color_space`, as `convert`.
    pub fn convert(self, color_space: &ColorSpace) -> Color {
        if self.is_packed_srgb() && color_space.id() == 0 {
            return self;
        }
        let source = self.color_space();
        crate::colorspace::connect(source, color_space, Default::default()).transform_to_color(self)
    }

    /// The same color with other components (in its space), as `copy(alpha, red, green, blue)`.
    pub fn copy(self, alpha: f32, red: f32, green: f32, blue: f32) -> Color {
        Color::new_with(red, green, blue, alpha, self.color_space())
    }

    /// Its sRGB ARGB, converted if it's of another space, as `toArgb`.
    #[inline]
    pub fn to_argb(self) -> u32 {
        if self.is_packed_srgb() { (self.0 >> 32) as u32 } else { self.to_argb_converted() }
    }

    #[cold]
    fn to_argb_converted(self) -> u32 {
        (self.convert(&ColorSpaces::SRGB).0 >> 32) as u32
    }

    /// Its relative luminance, 0 to 1, as `luminance()`.
    ///
    /// # Panics
    ///
    /// If its space isn't RGB, as Compose throws.
    pub fn luminance(self) -> f32 {
        let color_space = self.color_space();
        let rgb = color_space.rgb_data().filter(|_| color_space.model() == ColorModel::Rgb);
        let Some(rgb) = rgb else {
            panic!(
                "compose-ui: the specified color must be encoded in an RGB color space. The \
                 supplied color space is {}",
                color_space.model()
            )
        };
        let r = rgb.eotf(f64::from(self.red()));
        let g = rgb.eotf(f64::from(self.green()));
        let b = rgb.eotf(f64::from(self.blue()));
        coerce_in((0.2126 * r + 0.7152 * g + 0.0722 * b) as f32, 0.0, 1.0)
    }

    /// An opaque sRGB color from hue (0 to 360), saturation and value (0 to 1), as
    /// `Color.hsv(hue, saturation, value)`.
    pub fn hsv(hue: f32, saturation: f32, value: f32) -> Color {
        Color::hsv_with(hue, saturation, value, 1.0, &ColorSpaces::SRGB)
    }

    /// As `Color.hsv(hue, saturation, value, alpha, colorSpace)`.
    ///
    /// # Panics
    ///
    /// If hue, saturation or value is out of its range, as Compose throws.
    pub fn hsv_with(hue: f32, saturation: f32, value: f32, alpha: f32, color_space: &Rgb) -> Color {
        assert!(
            (0.0..=360.0).contains(&hue)
                && (0.0..=1.0).contains(&saturation)
                && (0.0..=1.0).contains(&value),
            "compose-ui: HSV ({hue}, {saturation}, {value}) must be in range (0..360, 0..1, 0..1)"
        );
        let component = |n: f32| {
            let k = (n + hue / 60.0) % 6.0;
            value - value * saturation * 0f32.max(k.min(4.0 - k).min(1.0))
        };
        Color::new_with(component(5.0), component(3.0), component(1.0), alpha, color_space)
    }

    /// An opaque sRGB color from hue (0 to 360), saturation and lightness (0 to 1), as
    /// `Color.hsl(hue, saturation, lightness)`.
    pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Color {
        Color::hsl_with(hue, saturation, lightness, 1.0, &ColorSpaces::SRGB)
    }

    /// As `Color.hsl(hue, saturation, lightness, alpha, colorSpace)`.
    ///
    /// # Panics
    ///
    /// If hue, saturation or lightness is out of its range, as Compose throws.
    pub fn hsl_with(
        hue: f32,
        saturation: f32,
        lightness: f32,
        alpha: f32,
        color_space: &Rgb,
    ) -> Color {
        assert!(
            (0.0..=360.0).contains(&hue)
                && (0.0..=1.0).contains(&saturation)
                && (0.0..=1.0).contains(&lightness),
            "compose-ui: HSL ({hue}, {saturation}, {lightness}) must be in range (0..360, 0..1, \
             0..1)"
        );
        let component = |n: f32| {
            let k = (n + hue / 30.0) % 12.0;
            let a = saturation * lightness.min(1.0 - lightness);
            lightness - a * (-1f32).max((k - 3.0).min(9.0 - k).min(1.0))
        };
        Color::new_with(component(0.0), component(8.0), component(4.0), alpha, color_space)
    }

    pub fn is_specified(self) -> bool {
        self != Color::UNSPECIFIED
    }

    pub fn is_unspecified(self) -> bool {
        self == Color::UNSPECIFIED
    }

    /// The same color with `alpha` (0 to 1) of opacity, as `copy(alpha = …)`.
    pub fn with_alpha(self, alpha: f32) -> Color {
        if self.is_packed_srgb() {
            let alpha = (coerce_in(alpha, 0.0, 1.0) * 255.0 + 0.5) as u32;
            Color(u64::from(alpha) << 56 | (self.0 & 0x00FF_FFFF_0000_0000))
        } else {
            self.copy(alpha, self.red(), self.green(), self.blue())
        }
    }

    /// This color drawn over `background`, in the background's space, as `compositeOver`.
    pub fn composite_over(self, background: Color) -> Color {
        let foreground = self.convert(background.color_space());
        let background_alpha = background.alpha();
        let foreground_alpha = foreground.alpha();
        let alpha = foreground_alpha + background_alpha * (1.0 - foreground_alpha);
        let component = |front: f32, back: f32| {
            if alpha == 0.0 {
                0.0
            } else {
                (front * foreground_alpha + back * background_alpha * (1.0 - foreground_alpha))
                    / alpha
            }
        };
        Color::unchecked(
            component(foreground.red(), background.red()),
            component(foreground.green(), background.green()),
            component(foreground.blue(), background.blue()),
            alpha,
            background.color_space(),
        )
    }

    /// `self` if specified, else `other`, as `takeOrElse`.
    pub fn or(self, other: Color) -> Color {
        if self.is_specified() { self } else { other }
    }

    /// `self` if specified, else what `block` gives, as `takeOrElse`.
    pub fn take_or_else(self, block: impl FnOnce() -> Color) -> Color {
        if self.is_specified() { self } else { block() }
    }

    /// Between `start` and `stop`, interpolated in the Oklab color space and given in `stop`'s,
    /// as Compose's `lerp(Color, Color, Float)`; `fraction` is kept in 0 to 1. The interpolated
    /// Oklab color is packed (as half floats) before it's converted, as Compose's.
    pub fn lerp(start: Color, stop: Color, fraction: f32) -> Color {
        let oklab = &*ColorSpaces::OKLAB;
        let start_color = start.convert(oklab);
        let end_color = stop.convert(oklab);
        let t = coerce_in(fraction, 0.0, 1.0);
        let mix = |start: f32, stop: f32| crate::unit::lerp_f32(start, stop, t);
        let interpolated = Color::unchecked(
            mix(start_color.red(), end_color.red()),
            mix(start_color.green(), end_color.green()),
            mix(start_color.blue(), end_color.blue()),
            mix(start_color.alpha(), end_color.alpha()),
            oklab,
        );
        interpolated.convert(stop.color_space())
    }

    /// This color with its alpha multiplied by `alpha` (when not NaN and below 1), as
    /// Compose's internal `Color.modulate` for text.
    pub(crate) fn modulate(self, alpha: f32) -> Color {
        if alpha.is_nan() || alpha >= 1.0 { self } else { self.with_alpha(self.alpha() * alpha) }
    }
}

impl fmt::Debug for Color {
    /// `#AARRGGBB` for an sRGB color (what display-list dumps show), else as `Display`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Color::UNSPECIFIED {
            formatter.write_str("Color::UNSPECIFIED")
        } else if self.is_packed_srgb() {
            write!(formatter, "#{:08X}", self.to_argb())
        } else {
            fmt::Display::fmt(self, formatter)
        }
    }
}

impl fmt::Display for Color {
    /// As Compose's `toString`: `Color(red, green, blue, alpha, space's name)`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Color({:?}, {:?}, {:?}, {:?}, {})",
            self.red(),
            self.green(),
            self.blue(),
            self.alpha(),
            self.color_space().name()
        )
    }
}

impl ComposeArg for Color {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// A corner's size, as Compose's `CornerSize`: fixed in dp or pixels, or a percentage of the
/// shape's shorter side.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CornerSize {
    /// As `CornerSize(size: Dp)`.
    Dp(Dp),
    /// As `CornerSize(size: Float)`, in pixels.
    Px(f32),
    /// As `CornerSize(percent: Int)`, of the shorter side (0 to 100).
    Percent(f32),
}

impl CornerSize {
    /// No corner, as `ZeroCornerSize`.
    pub const ZERO: CornerSize = CornerSize::Px(0.0);

    /// A percentage of the shorter side, as `CornerSize(percent)`: panics outside 0 to 100, as
    /// Compose throws.
    pub fn percent(percent: i32) -> CornerSize {
        assert!(
            (0..=100).contains(&percent),
            "compose-ui: the percent should be in the range of [0, 100], was {percent}"
        );
        CornerSize::Percent(percent as f32)
    }

    /// The corner's size in pixels for a shape of `shape_size`, as `toPx` (not limited: the
    /// shape scales its corners down to fit).
    pub fn to_px(self, shape_size: Size, density: &Density) -> f32 {
        match self {
            CornerSize::Dp(dp) => density.to_px(dp),
            CornerSize::Px(px) => px,
            CornerSize::Percent(percent) => {
                shape_size.width.min(shape_size.height) * (percent / 100.0)
            }
        }
    }
}

impl From<Dp> for CornerSize {
    fn from(size: Dp) -> CornerSize {
        CornerSize::Dp(size)
    }
}

impl From<f32> for CornerSize {
    /// Pixels, as `CornerSize(size: Float)`.
    fn from(px: f32) -> CornerSize {
        CornerSize::Px(px)
    }
}

impl From<i32> for CornerSize {
    /// A percentage, as `CornerSize(percent: Int)`.
    fn from(percent: i32) -> CornerSize {
        CornerSize::percent(percent)
    }
}

/// What the corner-based shapes take: one size for every corner (a `Dp`, an `f32` in pixels, an
/// `i32` percentage, a `CornerSize`), or four, Compose's overloads taking `topStart`, `topEnd`,
/// `bottomEnd` and `bottomStart` (top left, top right, bottom right and bottom left for the
/// absolute shapes).
pub trait CornerSizes {
    fn corner_sizes(self) -> [CornerSize; 4];
}

impl<T: Into<CornerSize>> CornerSizes for T {
    fn corner_sizes(self) -> [CornerSize; 4] {
        [self.into(); 4]
    }
}

impl<A: Into<CornerSize>, B: Into<CornerSize>, C: Into<CornerSize>, D: Into<CornerSize>> CornerSizes
    for (A, B, C, D)
{
    fn corner_sizes(self) -> [CornerSize; 4] {
        [self.0.into(), self.1.into(), self.2.into(), self.3.into()]
    }
}

impl CornerSizes for [CornerSize; 4] {
    fn corner_sizes(self) -> [CornerSize; 4] {
        self
    }
}

/// A shape of the app's own, as a class implementing Compose's `Shape`: wrap it with
/// [`Shape::custom`].
pub trait CustomShape: std::any::Any {
    /// Its outline in a box of `size`, as `createOutline`.
    fn create_outline(
        &self,
        size: Size,
        layout_direction: crate::alignment::LayoutDirection,
        density: &Density,
    ) -> Outline;

    /// Whether `other` is the same shape, as Compose's `equals`: a shape with no properties
    /// compares its type (`other.downcast_ref::<Self>().is_some()`).
    fn same(&self, other: &dyn CustomShape) -> bool;
}

impl dyn CustomShape {
    /// `self` as `T`, for [`same`](CustomShape::same).
    pub fn downcast_ref<T: CustomShape>(&self) -> Option<&T> {
        (self as &dyn std::any::Any).downcast_ref::<T>()
    }
}

/// A shape, as Compose's `Shape`: what backgrounds, borders, clips and shadows follow. Compose's
/// corner-based shapes are variants, so they're plain values (`foundation::RoundedCornerShape`
/// and its siblings make them); any other is `Custom` (`foundation::GenericShape`, or an app's
/// own [`CustomShape`]).
#[derive(Clone)]
pub enum Shape {
    /// As `RectangleShape`.
    Rectangle,
    /// Rounded corners: top start, top end, bottom end, bottom start (mirrored right to left),
    /// as `RoundedCornerShape`.
    Rounded([CornerSize; 4]),
    /// Cut corners, as `CutCornerShape`.
    Cut([CornerSize; 4]),
    /// Rounded corners: top left, top right, bottom right, bottom left, whatever the direction,
    /// as `AbsoluteRoundedCornerShape`.
    AbsoluteRounded([CornerSize; 4]),
    /// Cut corners top left, top right, bottom right, bottom left, as `AbsoluteCutCornerShape`.
    AbsoluteCut([CornerSize; 4]),
    /// Any other shape.
    Custom(std::rc::Rc<dyn CustomShape>),
}

/// As Compose's `RectangleShape`.
pub const RECTANGLE_SHAPE: Shape = Shape::Rectangle;

impl Shape {
    /// A shape of the app's own.
    pub fn custom(shape: impl CustomShape) -> Shape {
        Shape::Custom(std::rc::Rc::new(shape))
    }

    /// Its corners (top start, top end, bottom end, bottom start; or top left, top right,
    /// bottom right, bottom left for the absolute ones) if it's corner-based, as
    /// `CornerBasedShape`'s `topStart` and siblings.
    pub fn corners(&self) -> Option<[CornerSize; 4]> {
        match self {
            Shape::Rounded(corners)
            | Shape::Cut(corners)
            | Shape::AbsoluteRounded(corners)
            | Shape::AbsoluteCut(corners) => Some(*corners),
            Shape::Rectangle | Shape::Custom(_) => None,
        }
    }

    /// A corner-based shape of the same kind with `corners`, as `CornerBasedShape.copy`: one
    /// size for all, or four (`(top_start, top_end, bottom_end, bottom_start)`); another shape
    /// as it is.
    pub fn copy(&self, corners: impl CornerSizes) -> Shape {
        let corners = corners.corner_sizes();
        match self {
            Shape::Rounded(_) => Shape::Rounded(corners),
            Shape::Cut(_) => Shape::Cut(corners),
            Shape::AbsoluteRounded(_) => Shape::AbsoluteRounded(corners),
            Shape::AbsoluteCut(_) => Shape::AbsoluteCut(corners),
            Shape::Rectangle | Shape::Custom(_) => self.clone(),
        }
    }

    /// The shape's outline in a box of `size`, left to right.
    pub fn outline(&self, size: Size, density: &Density) -> Outline {
        self.outline_in(size, density, crate::alignment::LayoutDirection::Ltr)
    }

    /// The shape's outline in a box of `size`, as `createOutline`. A corner-based shape scales
    /// its two start corners down together, and its two end corners, when they're more than
    /// the shorter side, as Compose's `CornerBasedShape` (a negative corner panics, as Compose
    /// throws); the relative ones mirror their corners right to left.
    pub fn outline_in(
        &self,
        size: Size,
        density: &Density,
        direction: crate::alignment::LayoutDirection,
    ) -> Outline {
        let rect = Rect::new(0.0, 0.0, size.width, size.height);
        let (corners, rounded, absolute) = match self {
            Shape::Rectangle => return Outline::Rectangle(rect),
            Shape::Custom(shape) => return shape.create_outline(size, direction, density),
            Shape::Rounded(corners) => (corners, true, false),
            Shape::Cut(corners) => (corners, false, false),
            Shape::AbsoluteRounded(corners) => (corners, true, true),
            Shape::AbsoluteCut(corners) => (corners, false, true),
        };
        let [mut top_start, mut top_end, mut bottom_end, mut bottom_start] =
            corners.map(|corner| corner.to_px(size, density));
        let shorter = size.width.min(size.height);
        if top_start + bottom_start > shorter {
            let scale = shorter / (top_start + bottom_start);
            top_start *= scale;
            bottom_start *= scale;
        }
        if top_end + bottom_end > shorter {
            let scale = shorter / (top_end + bottom_end);
            top_end *= scale;
            bottom_end *= scale;
        }
        assert!(
            top_start >= 0.0 && top_end >= 0.0 && bottom_end >= 0.0 && bottom_start >= 0.0,
            "compose-ui: corner size in px can't be negative (top_start = {top_start}, top_end = \
             {top_end}, bottom_end = {bottom_end}, bottom_start = {bottom_start})"
        );
        if top_start + top_end + bottom_end + bottom_start == 0.0 {
            return Outline::Rectangle(rect);
        }
        // Top left, top right, bottom right, bottom left.
        let ltr = absolute || direction == crate::alignment::LayoutDirection::Ltr;
        let radii = if ltr {
            [top_start, top_end, bottom_end, bottom_start]
        } else {
            [top_end, top_start, bottom_start, bottom_end]
        };
        if rounded { Outline::Rounded(rect, radii) } else { Outline::cut(rect, radii) }
    }
}

impl PartialEq for Shape {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Shape::Rectangle, Shape::Rectangle) => true,
            (Shape::Rounded(a), Shape::Rounded(b))
            | (Shape::Cut(a), Shape::Cut(b))
            | (Shape::AbsoluteRounded(a), Shape::AbsoluteRounded(b))
            | (Shape::AbsoluteCut(a), Shape::AbsoluteCut(b)) => a == b,
            (Shape::Custom(a), Shape::Custom(b)) => std::rc::Rc::ptr_eq(a, b) || (**a).same(&**b),
            _ => false,
        }
    }
}

impl fmt::Debug for Shape {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shape::Rectangle => formatter.write_str("Rectangle"),
            Shape::Rounded(corners) => write!(formatter, "Rounded({corners:?})"),
            Shape::Cut(corners) => write!(formatter, "Cut({corners:?})"),
            Shape::AbsoluteRounded(corners) => write!(formatter, "AbsoluteRounded({corners:?})"),
            Shape::AbsoluteCut(corners) => write!(formatter, "AbsoluteCut({corners:?})"),
            Shape::Custom(_) => formatter.write_str("Custom"),
        }
    }
}

impl ComposeArg for Shape {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// A shape's outline in pixels, as `Outline`.
#[derive(Clone, PartialEq, Debug)]
pub enum Outline {
    Rectangle(Rect),
    /// Corner radii: top left, top right, bottom right, bottom left.
    Rounded(Rect, [f32; 4]),
    /// Any path, as `Outline.Generic(path)`: its contents (shared, as the display list holds
    /// them), moved by the offset.
    Generic(std::sync::Arc<crate::path::PathData>, crate::unit::Offset),
}

impl Outline {
    /// A path's outline, as `Outline.Generic(path)`: its contents now.
    pub fn generic(path: &crate::path::Path) -> Outline {
        Outline::Generic(path.data(), crate::unit::Offset::ZERO)
    }

    /// `rect` with its corners cut by `cuts` (top left, top right, bottom right, bottom left),
    /// as `CutCornerShape`'s path.
    fn cut(rect: Rect, cuts: [f32; 4]) -> Outline {
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        let path = crate::path::Path::new();
        path.move_to(0.0, cuts[0]);
        path.line_to(cuts[0], 0.0);
        path.line_to(width - cuts[1], 0.0);
        path.line_to(width, cuts[1]);
        path.line_to(width, height - cuts[2]);
        path.line_to(width - cuts[2], height);
        path.line_to(cuts[3], height);
        path.line_to(0.0, height - cuts[3]);
        path.close();
        Outline::Generic(path.data(), crate::unit::Offset::new(rect.left, rect.top))
    }

    /// Its bounds, as `Outline.bounds`.
    pub fn bounds(&self) -> Rect {
        match self {
            Outline::Rectangle(rect) | Outline::Rounded(rect, _) => *rect,
            Outline::Generic(path, offset) => {
                let bounds = path.bounds();
                Rect::new(
                    bounds.left + offset.x,
                    bounds.top + offset.y,
                    bounds.right + offset.x,
                    bounds.bottom + offset.y,
                )
            }
        }
    }

    /// The same outline moved by `by`.
    pub fn translate(&self, by: crate::unit::Offset) -> Outline {
        let moved = |rect: &Rect| {
            Rect::new(rect.left + by.x, rect.top + by.y, rect.right + by.x, rect.bottom + by.y)
        };
        match self {
            Outline::Rectangle(rect) => Outline::Rectangle(moved(rect)),
            Outline::Rounded(rect, radii) => Outline::Rounded(moved(rect), *radii),
            Outline::Generic(path, offset) => Outline::Generic(path.clone(), *offset + by),
        }
    }

    /// Whether `point` is inside the outline: for clipping hit tests.
    pub fn contains(&self, point: crate::unit::Offset) -> bool {
        match self {
            Outline::Rectangle(rect) => rect.contains(point),
            Outline::Generic(path, offset) => path.contains(point - *offset),
            Outline::Rounded(rect, radii) => {
                if !rect.contains(point) {
                    return false;
                }
                // Each corner's circle: top left, top right, bottom right, bottom left.
                let corners = [
                    (rect.left + radii[0], rect.top + radii[0], radii[0]),
                    (rect.right - radii[1], rect.top + radii[1], radii[1]),
                    (rect.right - radii[2], rect.bottom - radii[2], radii[2]),
                    (rect.left + radii[3], rect.bottom - radii[3], radii[3]),
                ];
                corners.iter().enumerate().all(|(index, &(x, y, radius))| {
                    let beyond_x = if index == 0 || index == 3 { point.x < x } else { point.x > x };
                    let beyond_y = if index < 2 { point.y < y } else { point.y > y };
                    if !(beyond_x && beyond_y) {
                        return true;
                    }
                    let (dx, dy) = (point.x - x, point.y - y);
                    dx * dx + dy * dy <= radius * radius
                })
            }
        }
    }
}

/// How a shape is drawn: filled, or stroked with a width in pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaintStyle {
    Fill,
    Stroke(f32),
}

/// How a stroke's open ends are drawn, as `StrokeCap`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeCap {
    /// Cut flat at the end.
    #[default]
    Butt,
    /// A half circle past the end.
    Round,
    /// A half square past the end.
    Square,
}

impl compose_runtime::ComposeArg for StrokeCap {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// What a draw operation paints with, as a subset of Compose's `Paint`.
///
/// A paint with a `shader` (a gradient, from `DrawScope::brush_paint`) paints with it; its
/// `color` is then the gradient's average, what a renderer without shaders paints instead.
#[derive(Clone, Copy, PartialEq, Debug)]
#[non_exhaustive]
pub struct Paint {
    pub color: Color,
    pub style: PaintStyle,
    /// A stroke's ends.
    pub cap: StrokeCap,
    /// The shader, in the display list it's recorded in (`DisplayList::shaders`).
    pub(crate) shader: Option<crate::brush::ShaderIndex>,
    /// How it blends with what's under it, as `Paint.blendMode`.
    pub blend_mode: crate::color_filter::BlendMode,
    /// Its color filter, in the display list it's recorded in (`DisplayList::color_filters`),
    /// as `Paint.colorFilter`: `DrawScope::paint_with_color_filter` sets it.
    pub(crate) color_filter: Option<crate::color_filter::ColorFilterIndex>,
    /// A blur mask filter's radius in pixels, 0 for none: what draws a shadow's soft edges, as
    /// the shadows' `BlurFilter` (Android's `BlurMaskFilter`, `NORMAL`).
    pub(crate) blur_radius: f32,
}

impl Paint {
    pub fn fill(color: Color) -> Paint {
        Paint { style: PaintStyle::Fill, ..Paint::stroke(color, 0.0) }
    }

    /// The same paint blending with `blend_mode`, as `Paint.blendMode`.
    pub fn with_blend_mode(self, blend_mode: crate::color_filter::BlendMode) -> Paint {
        Paint { blend_mode, ..self }
    }

    /// The color filter a paint names, in the display list it's recorded in
    /// (`DisplayList::color_filters`): for hosts.
    pub fn color_filter(&self) -> Option<crate::color_filter::ColorFilterIndex> {
        self.color_filter
    }

    /// This paint naming `color_filter` of the display list it goes in (a platform's index:
    /// `DrawScope::paint_with_color_filter` is the app-facing way).
    pub fn with_color_filter(self, color_filter: crate::color_filter::ColorFilterIndex) -> Paint {
        Paint { color_filter: Some(color_filter), ..self }
    }

    /// Whether it draws as plain color over what's under it: blending `SrcOver`, unfiltered,
    /// unblurred.
    pub fn is_plain(&self) -> bool {
        self.blend_mode == crate::color_filter::BlendMode::SrcOver
            && self.color_filter.is_none()
            && self.blur_radius == 0.0
    }

    /// The same paint with its shape blurred by `radius` pixels (a mask filter), as a shadow's.
    pub fn with_blur(self, radius: f32) -> Paint {
        Paint { blur_radius: radius.max(0.0), ..self }
    }

    /// Its blur mask filter's radius, 0 for none.
    pub fn blur_radius(&self) -> f32 {
        self.blur_radius
    }

    /// A stroke `width` pixels wide, its ends flat, as `Stroke(width)`.
    pub fn stroke(color: Color, width: f32) -> Paint {
        Paint::stroke_with(color, width, StrokeCap::Butt)
    }

    /// A stroke with `cap` ends, as `Stroke(width, cap = …)`.
    pub fn stroke_with(color: Color, width: f32, cap: StrokeCap) -> Paint {
        Paint {
            color,
            style: PaintStyle::Stroke(width),
            cap,
            shader: None,
            blend_mode: crate::color_filter::BlendMode::SrcOver,
            color_filter: None,
            blur_radius: 0.0,
        }
    }

    /// The same paint at `alpha` (0 to 1) of its opacity, as `Paint.alpha` and the draw calls'
    /// `alpha`: its color's alpha multiplied. A brush paint's gradient keeps the alpha given to
    /// `brush_paint` (only its fallback color changes): pass the alpha there.
    /// The shader a brush paint names, in the display list it's recorded in
    /// (`DisplayList::shaders`): for hosts.
    pub fn shader(&self) -> Option<crate::brush::ShaderIndex> {
        self.shader
    }

    /// This paint naming `shader` of the display list it goes in (a platform's shader index:
    /// `DrawScope::brush_paint` is the app-facing way).
    pub fn with_shader(self, shader: crate::brush::ShaderIndex) -> Paint {
        Paint { shader: Some(shader), ..self }
    }

    pub fn with_alpha(self, alpha: f32) -> Paint {
        let opacity = self.color.alpha() * alpha.clamp(0.0, 1.0);
        Paint { color: self.color.with_alpha(opacity), ..self }
    }
}

/// A shadow cast by text, as Compose's `Shadow`: its color, offset from the text, and blur
/// radius in pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shadow {
    pub color: Color,
    pub offset: crate::unit::Offset,
    pub blur_radius: f32,
}

impl Shadow {
    /// As `Shadow.None`: black, no offset, no blur.
    pub const NONE: Shadow =
        Shadow { color: Color::BLACK, offset: crate::unit::Offset::ZERO, blur_radius: 0.0 };

    /// Between `start` and `stop`, as Compose's `lerp(Shadow, Shadow, Float)`.
    pub fn lerp(start: Shadow, stop: Shadow, fraction: f32) -> Shadow {
        use crate::unit::lerp_f32;
        Shadow {
            color: Color::lerp(start.color, stop.color, fraction),
            offset: crate::unit::Offset::new(
                lerp_f32(start.offset.x, stop.offset.x, fraction),
                lerp_f32(start.offset.y, stop.offset.y, fraction),
            ),
            blur_radius: lerp_f32(start.blur_radius, stop.blur_radius, fraction),
        }
    }
}

impl Default for Shadow {
    fn default() -> Self {
        Shadow::NONE
    }
}

impl compose_runtime::ComposeArg for Shadow {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// How the corners of a stroke's segments are drawn, as `StrokeJoin`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeJoin {
    /// Sharp, up to the miter limit.
    #[default]
    Miter,
    Round,
    Bevel,
}

/// A stroke's geometry, as Compose's `Stroke` draw style (without a path effect).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stroke {
    /// The width in pixels; 0 is a hairline.
    pub width: f32,
    /// How far a sharp corner may reach, as a multiple of the width.
    pub miter: f32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
}

impl Stroke {
    /// As `Stroke.DefaultMiter`.
    pub const DEFAULT_MITER: f32 = 4.0;

    /// A stroke `width` pixels wide with Compose's defaults: miter 4, flat ends, mitered
    /// corners.
    pub const fn new(width: f32) -> Stroke {
        Stroke {
            width,
            miter: Stroke::DEFAULT_MITER,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
        }
    }

    /// The paint stroking with it in `color` (a paint has the width and the ends; its join and
    /// miter are the renderer's defaults, a miter of 4, as `Stroke.DefaultJoin`).
    pub fn paint(self, color: Color) -> Paint {
        Paint::stroke_with(color, self.width, self.cap)
    }
}

impl compose_runtime::ComposeArg for Stroke {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Stroke::new(0.0)
    }
}

/// Whether a drawing fills its shape or strokes its outline, as Compose's `DrawStyle` (`Fill`
/// or a `Stroke`).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum DrawStyle {
    #[default]
    Fill,
    Stroke(Stroke),
}

impl compose_runtime::ComposeArg for DrawStyle {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_pack_argb() {
        let color = Color::from_rgb(0x62, 0x00, 0xEE);
        assert_eq!(color.to_argb(), 0xFF62_00EE);
        assert_eq!(color.value(), 0xFF62_00EE_0000_0000);
        assert_eq!(
            (color.red(), color.green(), color.blue(), color.alpha()),
            (0x62 as f32 / 255.0, 0.0, 0xEE as f32 / 255.0, 1.0)
        );
        assert_eq!(color.with_alpha(0.5).to_argb() >> 24, 128);
        assert_eq!(Color::UNSPECIFIED.or(Color::RED), Color::RED);
    }

    /// A paint is 32 bytes with its 64-bit color: the indices' niches keep it from growing.
    #[test]
    fn a_paint_stays_small() {
        assert_eq!(std::mem::size_of::<Color>(), 8);
        assert_eq!(std::mem::size_of::<Paint>(), 32);
    }

    /// Corners larger than the box scale down to fit, as Compose's `CornerBasedShape`.
    #[test]
    fn oversized_rounded_corners_scale_down_to_fit() {
        let shape = Shape::Rounded([CornerSize::Dp(Dp(100.0)); 4]);
        let outline = shape.outline(Size::new(40.0, 20.0), &Density::default());
        assert_eq!(outline, Outline::Rounded(Rect::new(0.0, 0.0, 40.0, 20.0), [10.0; 4]));
        let pill = Shape::Rounded([CornerSize::Percent(50.0); 4]);
        assert_eq!(pill.outline(Size::new(40.0, 20.0), &Density::default()), outline);
    }
}
