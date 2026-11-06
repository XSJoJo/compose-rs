//! Color spaces, as Compose's `androidx.compose.ui.graphics.colorspace`
//! (docs/parity/design/color-spaces.md): `ColorSpace` and its models, the named spaces of
//! [`ColorSpaces`], RGB spaces ([`Rgb`]), and [`Connector`]s converting between two spaces.

use std::fmt;
use std::sync::{Arc, LazyLock};

mod connector;
pub(crate) mod half;
mod rgb;

pub use connector::Connector;
pub(crate) use connector::connect;
pub use rgb::Rgb;
pub(crate) use rgb::RgbData;

use crate::graphics::Color;

/// What a color space's components are, as Compose's `ColorModel`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ColorModel {
    /// Red, green and blue.
    Rgb,
    /// CIE XYZ.
    Xyz,
    /// Lightness and two opponents (CIE L*a*b*, Oklab).
    Lab,
    /// Cyan, magenta, yellow and black.
    Cmyk,
}

impl ColorModel {
    /// How many components a color has in it, as `componentCount`.
    pub const fn component_count(self) -> usize {
        match self {
            ColorModel::Cmyk => 4,
            ColorModel::Rgb | ColorModel::Xyz | ColorModel::Lab => 3,
        }
    }
}

impl fmt::Display for ColorModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, formatter)
    }
}

/// How a connector maps colors outside the destination's gamut, as Compose's `RenderIntent`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum RenderIntent {
    /// Compressed to fit, keeping relations between colors (as relative here, as Compose's).
    #[default]
    Perceptual,
    /// Clipped, white points matched.
    Relative,
    /// As relative here, as Compose's.
    Saturation,
    /// Clipped, white points kept.
    Absolute,
}

impl fmt::Display for RenderIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, formatter)
    }
}

/// A white point in the CIE xyY space, as Compose's `WhitePoint`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WhitePoint {
    pub x: f32,
    pub y: f32,
}

impl WhitePoint {
    /// From its chromaticity, as `WhitePoint(x, y)`.
    pub const fn new(x: f32, y: f32) -> WhitePoint {
        WhitePoint { x, y }
    }

    /// From its tristimulus values, as `WhitePoint(x, y, z)`.
    pub fn from_xyz(x: f32, y: f32, z: f32) -> WhitePoint {
        let sum = x + y + z;
        WhitePoint { x: x / sum, y: y / sum }
    }

    /// Its XYZ values, Y being 1.
    pub(crate) fn to_xyz(self) -> [f32; 3] {
        [self.x / self.y, 1.0, (1.0 - self.x - self.y) / self.y]
    }
}

/// The CIE standard illuminants' white points, as Compose's `Illuminant`.
pub struct Illuminant;

impl Illuminant {
    pub const A: WhitePoint = WhitePoint::new(0.44757, 0.40745);
    pub const B: WhitePoint = WhitePoint::new(0.34842, 0.35161);
    pub const C: WhitePoint = WhitePoint::new(0.31006, 0.31616);
    pub const D50: WhitePoint = WhitePoint::new(0.34567, 0.35850);
    pub const D55: WhitePoint = WhitePoint::new(0.33242, 0.34743);
    pub const D60: WhitePoint = WhitePoint::new(0.32168, 0.33767);
    pub const D65: WhitePoint = WhitePoint::new(0.31271, 0.32902);
    pub const D75: WhitePoint = WhitePoint::new(0.29902, 0.31485);
    pub const E: WhitePoint = WhitePoint::new(0.33333, 0.33333);
}

/// D50's XYZ values, as Compose's `Illuminant.D50Xyz`.
pub(crate) const D50_XYZ: [f32; 3] = [0.964212, 1.0, 0.825188];

/// A chromatic adaptation transform, as Compose's `Adaptation`: from XYZ to a cone response
/// space, column-major.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Adaptation {
    name: &'static str,
    pub(crate) transform: [f32; 9],
}

impl Adaptation {
    pub const BRADFORD: Adaptation = Adaptation {
        name: "Bradford",
        transform: [0.8951, -0.7502, 0.0389, 0.2664, 1.7135, -0.0685, -0.1614, 0.0367, 1.0296],
    };
    pub const VON_KRIES: Adaptation = Adaptation {
        name: "VonKries",
        transform: [0.40024, -0.22630, 0.0, 0.70760, 1.16532, 0.0, -0.08081, 0.04570, 0.91822],
    };
    pub const CIECAT02: Adaptation = Adaptation {
        name: "Ciecat02",
        transform: [0.7328, -0.7036, 0.0030, 0.4296, 1.6975, 0.0136, -0.1624, 0.0061, 0.9834],
    };
}

impl fmt::Display for Adaptation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name)
    }
}

/// The parameters of an ICC parametric curve, as Compose's `TransferParameters`: from encoded
/// to linear, `Y = (a * X + b) ^ gamma + e` from `d` up, `Y = c * X + f` below. A `gamma` of
/// -2 or -3 makes them a PQ or an HLG curve's, as Compose's.
#[derive(Clone, Copy, PartialEq, Debug)]
#[non_exhaustive]
pub struct TransferParameters {
    pub gamma: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

const TYPE_PQ_ISH: f64 = -2.0;
const TYPE_HLG_ISH: f64 = -3.0;

impl TransferParameters {
    /// Without offsets (`e` and `f` 0), as `TransferParameters(gamma, a, b, c, d)`.
    pub fn new(gamma: f64, a: f64, b: f64, c: f64, d: f64) -> TransferParameters {
        TransferParameters::new_with(gamma, a, b, c, d, 0.0, 0.0)
    }

    /// As `TransferParameters(gamma, a, b, c, d, e, f)`.
    ///
    /// # Panics
    ///
    /// For a NaN, or (unless `gamma` makes a PQ or HLG curve) a curve that isn't increasing or
    /// is constant, or a `d` outside 0 to 1, as Compose throws.
    pub fn new_with(
        gamma: f64,
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        e: f64,
        f: f64,
    ) -> TransferParameters {
        assert!(
            ![a, b, c, d, e, f, gamma].iter().any(|value| value.is_nan()),
            "compose-ui: transfer parameters cannot be NaN"
        );
        if gamma != TYPE_PQ_ISH && gamma != TYPE_HLG_ISH {
            assert!((0.0..=1.0).contains(&d), "compose-ui: parameter d must be in 0 to 1, was {d}");
            assert!(
                !(d == 0.0 && (a == 0.0 || gamma == 0.0)),
                "compose-ui: parameter a or gamma is zero, the transfer function is constant"
            );
            assert!(
                !(d >= 1.0 && c == 0.0),
                "compose-ui: parameter c is zero, the transfer function is constant"
            );
            assert!(
                !((a == 0.0 || gamma == 0.0) && c == 0.0),
                "compose-ui: parameter a or gamma is zero, and c is zero, the transfer function \
                 is constant"
            );
            assert!(c >= 0.0, "compose-ui: the transfer function must be increasing");
            assert!(
                a >= 0.0 && gamma >= 0.0,
                "compose-ui: the transfer function must be positive or increasing"
            );
        }
        TransferParameters { gamma, a, b, c, d, e, f }
    }

    pub(crate) fn is_hlg_ish(&self) -> bool {
        self.gamma == TYPE_HLG_ISH
    }

    pub(crate) fn is_pq_ish(&self) -> bool {
        self.gamma == TYPE_PQ_ISH
    }
}

/// A color space of the app's own, as a subclass of Compose's `ColorSpace`: wrap it with
/// [`ColorSpace::custom`]. Its colors can't be packed in a [`Color`], as Compose's (their id is
/// -1), but connectors convert values from and to it.
pub trait CustomColorSpace: Send + Sync + std::any::Any {
    /// Whether it holds more than sRGB's colors, as `isWideGamut`.
    fn is_wide_gamut(&self) -> bool;
    /// Whether it is sRGB, as `isSrgb`.
    fn is_srgb(&self) -> bool {
        false
    }
    /// A component's lowest value, as `getMinValue`.
    fn min_value(&self, component: usize) -> f32;
    /// A component's highest value, as `getMaxValue`.
    fn max_value(&self, component: usize) -> f32;
    /// Converts `values` (its components) to CIE XYZ (D50) in place, as `toXyz(v)`.
    fn to_xyz(&self, values: &mut [f32]);
    /// Converts `values` (XYZ, D50) to its components in place, as `fromXyz(v)`.
    #[allow(clippy::wrong_self_convention)] // Compose's name.
    fn from_xyz(&self, values: &mut [f32]);
}

/// A color space, as Compose's `ColorSpace`: a shared handle, compared by Compose's `equals`
/// (its kind, id, name and model, and an RGB space's values). [`ColorSpaces`] has the named
/// ones; [`Rgb`]'s constructors make RGB ones; [`ColorSpace::custom`] any other.
#[derive(Clone)]
pub struct ColorSpace(Arc<Space>);

pub(crate) struct Space {
    name: Box<str>,
    model: ColorModel,
    id: i32,
    pub(crate) kind: Kind,
}

// The space is behind an `Arc`: its size doesn't matter.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Kind {
    Rgb(RgbData),
    Xyz,
    Lab,
    Oklab,
    Custom(Box<dyn CustomColorSpace>),
}

/// The id of a color space that isn't named, as Compose's `ColorSpace.MinId`.
pub const MIN_ID: i32 = -1;
/// The highest id a color packs, as Compose's `ColorSpace.MaxId`.
pub const MAX_ID: i32 = 63;

impl ColorSpace {
    pub(crate) fn new(name: &str, model: ColorModel, id: i32, kind: Kind) -> ColorSpace {
        assert!(
            !name.is_empty(),
            "compose-ui: the name of a color space cannot be null and must contain at least 1 \
             character"
        );
        assert!((MIN_ID..=MAX_ID).contains(&id), "compose-ui: the id must be between -1 and 63");
        ColorSpace(Arc::new(Space { name: name.into(), model, id, kind }))
    }

    /// A color space of the app's own, as a subclass of Compose's `ColorSpace(name, model)`.
    ///
    /// # Panics
    ///
    /// If `name` is empty, as Compose throws.
    pub fn custom(name: &str, model: ColorModel, space: impl CustomColorSpace) -> ColorSpace {
        ColorSpace::new(name, model, MIN_ID, Kind::Custom(Box::new(space)))
    }

    pub fn name(&self) -> &str {
        &self.0.name
    }

    pub fn model(&self) -> ColorModel {
        self.0.model
    }

    /// Its id, which a color packs: 0 to 63 for the named spaces, -1 for any other.
    pub fn id(&self) -> i32 {
        self.0.id
    }

    /// How many components its colors have, as `componentCount`.
    pub fn component_count(&self) -> usize {
        self.0.model.component_count()
    }

    /// An RGB space's values, if it is one.
    pub(crate) fn rgb_data(&self) -> Option<&RgbData> {
        match &self.0.kind {
            Kind::Rgb(data) => Some(data),
            _ => None,
        }
    }

    /// The same space as an [`Rgb`], if it is one (Kotlin's `as Rgb`).
    pub fn as_rgb(&self) -> Option<Rgb> {
        self.rgb_data().map(|_| Rgb(self.clone()))
    }

    /// Whether it holds more colors than sRGB, as `isWideGamut`.
    pub fn is_wide_gamut(&self) -> bool {
        match &self.0.kind {
            Kind::Rgb(data) => data.is_wide_gamut,
            Kind::Xyz | Kind::Lab | Kind::Oklab => true,
            Kind::Custom(space) => space.is_wide_gamut(),
        }
    }

    /// Whether it is sRGB, as `isSrgb`: an RGB space with sRGB's primaries, white point,
    /// range and transfer functions.
    pub fn is_srgb(&self) -> bool {
        match &self.0.kind {
            Kind::Rgb(data) => data.is_srgb,
            Kind::Xyz | Kind::Lab | Kind::Oklab => false,
            Kind::Custom(space) => space.is_srgb(),
        }
    }

    /// A component's lowest value, as `getMinValue`.
    pub fn min_value(&self, component: usize) -> f32 {
        match &self.0.kind {
            Kind::Rgb(data) => data.min,
            Kind::Xyz => -2.0,
            Kind::Lab => {
                if component == 0 {
                    0.0
                } else {
                    -128.0
                }
            }
            Kind::Oklab => {
                if component == 0 {
                    0.0
                } else {
                    -0.5
                }
            }
            Kind::Custom(space) => space.min_value(component),
        }
    }

    /// A component's highest value, as `getMaxValue`.
    pub fn max_value(&self, component: usize) -> f32 {
        match &self.0.kind {
            Kind::Rgb(data) => data.max,
            Kind::Xyz => 2.0,
            Kind::Lab => {
                if component == 0 {
                    100.0
                } else {
                    128.0
                }
            }
            Kind::Oklab => {
                if component == 0 {
                    1.0
                } else {
                    0.5
                }
            }
            Kind::Custom(space) => space.max_value(component),
        }
    }

    /// The color `(r, g, b)` (its three components) in CIE XYZ, D50, as `toXyz(r, g, b)`.
    pub fn to_xyz(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let mut values = [r, g, b];
        self.to_xyz_in_place(&mut values);
        values
    }

    /// Converts `values` (at least three components) to CIE XYZ (D50) in place, as
    /// `toXyz(v)`.
    pub fn to_xyz_in_place(&self, values: &mut [f32]) {
        match &self.0.kind {
            Kind::Rgb(data) => data.to_xyz_in_place(values),
            Kind::Xyz => {
                for value in &mut values[..3] {
                    *value = coerce_in(*value, -2.0, 2.0);
                }
            }
            Kind::Lab => lab_to_xyz(values),
            Kind::Oklab => oklab_to_xyz(values),
            Kind::Custom(space) => space.to_xyz(values),
        }
    }

    /// The XYZ (D50) color `(x, y, z)` in this space, as `fromXyz(x, y, z)` (a space of four
    /// components has its fourth left out: [`from_xyz_in_place`](Self::from_xyz_in_place)
    /// gives it).
    pub fn from_xyz(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let mut values = [0.0; 4];
        values[..3].copy_from_slice(&[x, y, z]);
        self.from_xyz_in_place(&mut values[..self.component_count().max(3)]);
        [values[0], values[1], values[2]]
    }

    /// Converts `values` (XYZ, D50, then room for the components) to this space in place, as
    /// `fromXyz(v)`.
    pub fn from_xyz_in_place(&self, values: &mut [f32]) {
        match &self.0.kind {
            Kind::Rgb(data) => data.xyz_to_values_in_place(values),
            Kind::Xyz => {
                for value in &mut values[..3] {
                    *value = coerce_in(*value, -2.0, 2.0);
                }
            }
            Kind::Lab => xyz_to_lab(values),
            Kind::Oklab => xyz_to_oklab(values),
            Kind::Custom(space) => space.from_xyz(values),
        }
    }

    /// `(v0, v1, v2)` in XYZ, as Compose's `toXy` and `toZ` together (each computes the same
    /// values as they do).
    pub(crate) fn to_xyz_values(&self, v0: f32, v1: f32, v2: f32) -> [f32; 3] {
        match &self.0.kind {
            Kind::Rgb(data) => data.to_xyz_values(v0, v1, v2),
            _ => {
                let mut values = [v0, v1, v2, 0.0];
                let count = self.component_count().max(3);
                self.to_xyz_in_place(&mut values[..count]);
                [values[0], values[1], values[2]]
            }
        }
    }

    /// The color of XYZ `(x, y, z)` and alpha `a` in this space, packed as one of
    /// `color_space`, as Compose's `xyzaToColor`.
    pub(crate) fn xyza_to_color(
        &self,
        x: f32,
        y: f32,
        z: f32,
        a: f32,
        color_space: &ColorSpace,
    ) -> Color {
        match &self.0.kind {
            Kind::Rgb(data) => {
                let [r, g, b] = data.xyz_to_values(x, y, z);
                Color::new_with(r, g, b, a, color_space)
            }
            _ => {
                let [r, g, b] = self.from_xyz(x, y, z);
                Color::new_with(r, g, b, a, color_space)
            }
        }
    }

    /// A connector from this space to `destination`, perceptual, as `connect(destination)`.
    pub fn connect(&self, destination: &ColorSpace) -> Connector {
        self.connect_with(destination, RenderIntent::Perceptual)
    }

    /// A connector from this space to `destination` with `intent`, as
    /// `connect(destination, intent)`: the same connector for the same named spaces and
    /// intent, made once.
    pub fn connect_with(&self, destination: &ColorSpace, intent: RenderIntent) -> Connector {
        connector::connect(self, destination, intent).into_owned()
    }

    /// This space adapted to `white_point` with the Bradford transform, as `adapt(whitePoint)`.
    pub fn adapt(&self, white_point: WhitePoint) -> ColorSpace {
        self.adapt_with(white_point, Adaptation::BRADFORD)
    }

    /// An RGB space adapted to `white_point` with `adaptation` (this space if it has that white
    /// point), any other as it is, as `adapt(whitePoint, adaptation)`.
    pub fn adapt_with(&self, white_point: WhitePoint, adaptation: Adaptation) -> ColorSpace {
        if let Some(rgb) = self.rgb_data() {
            if compare_white_points(rgb.white_point, white_point) {
                return self.clone();
            }
            let adaptation_transform = chromatic_adaptation(
                &adaptation.transform,
                rgb.white_point.to_xyz(),
                white_point.to_xyz(),
            );
            let transform = mul3x3(&adaptation_transform, &rgb.transform);
            return rgb.adapted(self.name(), transform, white_point);
        }
        self.clone()
    }

    /// Whether `other` is this very space (Kotlin's `===`).
    pub fn same_instance(&self, other: &ColorSpace) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for ColorSpace {
    fn eq(&self, other: &ColorSpace) -> bool {
        if self.same_instance(other) {
            return true;
        }
        let (this, that) = (&*self.0, &*other.0);
        let same_kind = match (&this.kind, &that.kind) {
            (Kind::Rgb(a), Kind::Rgb(b)) => a.same(b),
            (Kind::Xyz, Kind::Xyz) | (Kind::Lab, Kind::Lab) | (Kind::Oklab, Kind::Oklab) => true,
            (Kind::Custom(a), Kind::Custom(b)) => {
                let (a, b): (&dyn std::any::Any, &dyn std::any::Any) = (&**a, &**b);
                a.type_id() == b.type_id()
            }
            _ => false,
        };
        same_kind && this.id == that.id && this.name == that.name && this.model == that.model
    }
}

impl fmt::Display for ColorSpace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} (id={}, model={})", self.name(), self.id(), self.model())
    }
}

impl fmt::Debug for ColorSpace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// The named color spaces, as Compose's `ColorSpaces`: constants dereferencing to the one
/// space each (an [`Rgb`] for the RGB ones), made on first use.
pub struct ColorSpaces;

/// A named RGB space of [`ColorSpaces`]: it dereferences to its [`Rgb`] (and so to a
/// [`ColorSpace`]).
#[derive(Clone, Copy)]
pub struct NamedRgb(u8);

/// A named space of [`ColorSpaces`] that isn't RGB: it dereferences to its [`ColorSpace`].
#[derive(Clone, Copy)]
pub struct NamedColorSpace(u8);

impl std::ops::Deref for NamedRgb {
    type Target = Rgb;
    fn deref(&self) -> &Rgb {
        named().rgbs[self.0 as usize].as_ref().expect("an RGB space")
    }
}

impl std::ops::Deref for NamedColorSpace {
    type Target = ColorSpace;
    fn deref(&self) -> &ColorSpace {
        &named().spaces[self.0 as usize]
    }
}

impl fmt::Debug for NamedRgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

impl fmt::Debug for NamedColorSpace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

impl PartialEq<NamedRgb> for ColorSpace {
    fn eq(&self, other: &NamedRgb) -> bool {
        *self == ***other
    }
}

impl PartialEq<NamedColorSpace> for ColorSpace {
    fn eq(&self, other: &NamedColorSpace) -> bool {
        *self == **other
    }
}

/// How many spaces are named: their ids are 0 to this, less one.
pub(crate) const NAMED_COUNT: usize = 20;

struct Named {
    /// By id.
    spaces: Vec<ColorSpace>,
    /// By id, the RGB ones.
    rgbs: Vec<Option<Rgb>>,
}

fn named() -> &'static Named {
    static NAMED: LazyLock<Named> = LazyLock::new(|| {
        let spaces = rgb::named_spaces();
        let rgbs = spaces.iter().map(ColorSpace::as_rgb).collect();
        Named { spaces, rgbs }
    });
    &NAMED
}

pub(crate) const SRGB_PRIMARIES: [f32; 6] = [0.640, 0.330, 0.300, 0.600, 0.150, 0.060];
pub(crate) const NTSC_1953_PRIMARIES: [f32; 6] = [0.67, 0.33, 0.21, 0.71, 0.14, 0.08];
pub(crate) const BT2020_PRIMARIES: [f32; 6] = [0.708, 0.292, 0.170, 0.797, 0.131, 0.046];
pub(crate) const SRGB_TRANSFER_PARAMETERS: TransferParameters = TransferParameters {
    gamma: 2.4,
    a: 1.0 / 1.055,
    b: 0.055 / 1.055,
    c: 1.0 / 12.92,
    d: 0.04045,
    e: 0.0,
    f: 0.0,
};

impl ColorSpaces {
    /// sRGB IEC61966-2.1.
    pub const SRGB: NamedRgb = NamedRgb(0);
    /// sRGB with a linear transfer function.
    pub const LINEAR_SRGB: NamedRgb = NamedRgb(1);
    /// scRGB-nl: sRGB extended to -0.799 to 2.399.
    pub const EXTENDED_SRGB: NamedRgb = NamedRgb(2);
    /// scRGB: linear sRGB extended to -0.5 to 7.499.
    pub const LINEAR_EXTENDED_SRGB: NamedRgb = NamedRgb(3);
    pub const BT709: NamedRgb = NamedRgb(4);
    pub const BT2020: NamedRgb = NamedRgb(5);
    pub const DCI_P3: NamedRgb = NamedRgb(6);
    pub const DISPLAY_P3: NamedRgb = NamedRgb(7);
    pub const NTSC1953: NamedRgb = NamedRgb(8);
    pub const SMPTE_C: NamedRgb = NamedRgb(9);
    pub const ADOBE_RGB: NamedRgb = NamedRgb(10);
    pub const PRO_PHOTO_RGB: NamedRgb = NamedRgb(11);
    pub const ACES: NamedRgb = NamedRgb(12);
    pub const ACESCG: NamedRgb = NamedRgb(13);
    pub const CIE_XYZ: NamedColorSpace = NamedColorSpace(14);
    pub const CIE_LAB: NamedColorSpace = NamedColorSpace(15);
    /// The space of `Color::UNSPECIFIED`, as Compose's internal `ColorSpaces.Unspecified`.
    pub const UNSPECIFIED: NamedRgb = NamedRgb(16);
    pub const BT2020_HLG: NamedRgb = NamedRgb(17);
    pub const BT2020_PQ: NamedRgb = NamedRgb(18);
    pub const OKLAB: NamedColorSpace = NamedColorSpace(19);

    /// The named space of `id`, as Compose's internal `getColorSpace`.
    pub fn get(id: usize) -> Option<&'static ColorSpace> {
        named().spaces.get(id)
    }

    /// Every named space, by id, as Compose's internal `ColorSpacesArray`.
    pub fn all() -> &'static [ColorSpace] {
        &named().spaces
    }

    /// The named RGB space with the transform `to_xyz_d50` (to XYZ, D50) and transfer
    /// `function`, if any, as `match` (extended sRGB isn't matched, as Compose's).
    pub fn matching(
        to_xyz_d50: &[f32; 9],
        function: &TransferParameters,
    ) -> Option<&'static ColorSpace> {
        ColorSpaces::all().iter().find(|space| {
            let adapted = space.adapt(Illuminant::D50);
            adapted.rgb_data().is_some_and(|rgb| {
                compare_floats(to_xyz_d50, &rgb.transform)
                    && rgb
                        .transfer_parameters
                        .is_some_and(|parameters| compare_transfer(function, &parameters))
            })
        })
    }
}

/// `value` kept in `minimum` to `maximum`, as Kotlin's `fastCoerceIn` (a NaN stays NaN).
#[inline]
pub(crate) fn coerce_in(value: f32, minimum: f32, maximum: f32) -> f32 {
    let value = if value < minimum { minimum } else { value };
    if value > maximum { maximum } else { value }
}

pub(crate) fn rcp_response(x: f64, p: &TransferParameters) -> f64 {
    if x >= p.d * p.c { ((x - p.e).powf(1.0 / p.gamma) - p.b) / p.a } else { (x - p.f) / p.c }
}

pub(crate) fn response(x: f64, p: &TransferParameters) -> f64 {
    if x >= p.d { (p.a * x + p.b).powf(p.gamma) + p.e } else { p.c * x + p.f }
}

pub(crate) fn abs_rcp_response(x: f64, p: &TransferParameters) -> f64 {
    rcp_response(if x < 0.0 { -x } else { x }, p).copysign(x)
}

pub(crate) fn abs_response(x: f64, p: &TransferParameters) -> f64 {
    response(if x < 0.0 { -x } else { x }, p).copysign(x)
}

pub(crate) fn hlg_oetf(params: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let mut abs_x = x * sign;
    let r = 1.0 / params.a;
    let g = 1.0 / params.b;
    let a = 1.0 / params.c;
    let b = params.d;
    let c = params.e;
    let k = params.f + 1.0;
    abs_x /= k;
    let result = if abs_x <= 1.0 { r * abs_x.powf(g) } else { a * (abs_x - b).ln() + c };
    sign * result
}

pub(crate) fn hlg_eotf(params: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x * sign;
    let r = params.a;
    let g = params.b;
    let a = params.c;
    let b = params.d;
    let c = params.e;
    let k = params.f + 1.0;
    let result = if abs_x * r <= 1.0 { (abs_x * r).powf(g) } else { ((abs_x - c) * a).exp() + b };
    k * sign * result
}

pub(crate) fn pq_oetf(params: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x * sign;
    let a = -params.a;
    let b = params.d;
    let c = 1.0 / params.f;
    let d = params.b;
    let e = -params.e;
    let f = 1.0 / params.c;
    let tmp = (a + b * abs_x.powf(c)).max(0.0);
    sign * (tmp / (d + e * abs_x.powf(c))).powf(f)
}

pub(crate) fn pq_eotf(pq: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x * sign;
    let tmp = (pq.a + pq.b * abs_x.powf(pq.c)).max(0.0);
    sign * (tmp / (pq.d + pq.e * abs_x.powf(pq.c))).powf(pq.f)
}

pub(crate) fn compare_transfer(a: &TransferParameters, b: &TransferParameters) -> bool {
    (a.a - b.a).abs() < 1e-3
        && (a.b - b.b).abs() < 1e-3
        && (a.c - b.c).abs() < 1e-3
        // sRGB's curves vary in d.
        && (a.d - b.d).abs() < 2e-3
        && (a.e - b.e).abs() < 1e-3
        && (a.f - b.f).abs() < 1e-3
        && (a.gamma - b.gamma).abs() < 1e-3
}

pub(crate) fn compare_white_points(a: WhitePoint, b: WhitePoint) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

pub(crate) fn compare_floats(a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).all(|(a, b)| a.total_cmp(b).is_eq() || (a - b).abs() <= 1e-3)
}

/// The inverse of a 3x3 column-major matrix.
pub(crate) fn inverse3x3(m: &[f32; 9]) -> [f32; 9] {
    let (a, b, c) = (m[0], m[3], m[6]);
    let (d, e, f) = (m[1], m[4], m[7]);
    let (g, h, i) = (m[2], m[5], m[8]);
    let x_a = e * i - f * h;
    let x_b = f * g - d * i;
    let x_c = d * h - e * g;
    let det = a * x_a + b * x_b + c * x_c;
    [
        x_a / det,
        x_b / det,
        x_c / det,
        (c * h - b * i) / det,
        (a * i - c * g) / det,
        (b * g - a * h) / det,
        (b * f - c * e) / det,
        (c * d - a * f) / det,
        (a * e - b * d) / det,
    ]
}

/// `lhs * rhs`, 3x3 column-major matrices.
pub(crate) fn mul3x3(lhs: &[f32; 9], rhs: &[f32; 9]) -> [f32; 9] {
    [
        lhs[0] * rhs[0] + lhs[3] * rhs[1] + lhs[6] * rhs[2],
        lhs[1] * rhs[0] + lhs[4] * rhs[1] + lhs[7] * rhs[2],
        lhs[2] * rhs[0] + lhs[5] * rhs[1] + lhs[8] * rhs[2],
        lhs[0] * rhs[3] + lhs[3] * rhs[4] + lhs[6] * rhs[5],
        lhs[1] * rhs[3] + lhs[4] * rhs[4] + lhs[7] * rhs[5],
        lhs[2] * rhs[3] + lhs[5] * rhs[4] + lhs[8] * rhs[5],
        lhs[0] * rhs[6] + lhs[3] * rhs[7] + lhs[6] * rhs[8],
        lhs[1] * rhs[6] + lhs[4] * rhs[7] + lhs[7] * rhs[8],
        lhs[2] * rhs[6] + lhs[5] * rhs[7] + lhs[8] * rhs[8],
    ]
}

/// `lhs * (r0, r1, r2)`.
#[inline]
pub(crate) fn mul3x3_float3(lhs: &[f32; 9], r0: f32, r1: f32, r2: f32) -> [f32; 3] {
    [
        lhs[0] * r0 + lhs[3] * r1 + lhs[6] * r2,
        lhs[1] * r0 + lhs[4] * r1 + lhs[7] * r2,
        lhs[2] * r0 + lhs[5] * r1 + lhs[8] * r2,
    ]
}

/// `lhs * values[..3]`, in place.
pub(crate) fn mul3x3_in_place(lhs: &[f32; 9], values: &mut [f32]) {
    let product = mul3x3_float3(lhs, values[0], values[1], values[2]);
    values[..3].copy_from_slice(&product);
}

/// A diagonal matrix (of `lhs`) times `rhs`.
pub(crate) fn mul3x3_diag(lhs: [f32; 3], rhs: &[f32; 9]) -> [f32; 9] {
    [
        lhs[0] * rhs[0],
        lhs[1] * rhs[1],
        lhs[2] * rhs[2],
        lhs[0] * rhs[3],
        lhs[1] * rhs[4],
        lhs[2] * rhs[5],
        lhs[0] * rhs[6],
        lhs[1] * rhs[7],
        lhs[2] * rhs[8],
    ]
}

/// The transform adapting colors from `source_white` to `destination_white` (their XYZ) with
/// the cone response `matrix`, as Compose's `chromaticAdaptation`.
pub(crate) fn chromatic_adaptation(
    matrix: &[f32; 9],
    source_white: [f32; 3],
    destination_white: [f32; 3],
) -> [f32; 9] {
    let source = mul3x3_float3(matrix, source_white[0], source_white[1], source_white[2]);
    let destination =
        mul3x3_float3(matrix, destination_white[0], destination_white[1], destination_white[2]);
    let lms = [destination[0] / source[0], destination[1] / source[1], destination[2] / source[2]];
    mul3x3(&inverse3x3(matrix), &mul3x3_diag(lms, matrix))
}

/// A fast cube root, as Compose's `fastCbrt`: an estimate from the float's bits, refined twice.
pub(crate) fn fast_cbrt(x: f32) -> f32 {
    let bits = i64::from(x.to_bits() as i32) & 0x1_ffff_ffff;
    let mut estimate = f32::from_bits(0x2a51_0554i32.wrapping_add((bits / 3) as i32) as u32);
    estimate -= (estimate - x / (estimate * estimate)) * (1.0 / 3.0);
    estimate -= (estimate - x / (estimate * estimate)) * (1.0 / 3.0);
    estimate
}

// CIE L*a*b*, as Compose's `Lab`.
const LAB_A: f32 = 216.0 / 24389.0;
const LAB_B: f32 = 841.0 / 108.0;
const LAB_C: f32 = 4.0 / 29.0;
const LAB_D: f32 = 6.0 / 29.0;

fn lab_to_xyz(values: &mut [f32]) {
    values[0] = coerce_in(values[0], 0.0, 100.0);
    values[1] = coerce_in(values[1], -128.0, 128.0);
    values[2] = coerce_in(values[2], -128.0, 128.0);
    let fy = (values[0] + 16.0) / 116.0;
    let fx = fy + values[1] * 0.002;
    let fz = fy - values[2] * 0.005;
    let f = |t: f32| if t > LAB_D { t * t * t } else { (1.0 / LAB_B) * (t - LAB_C) };
    values[0] = f(fx) * D50_XYZ[0];
    values[1] = f(fy) * D50_XYZ[1];
    values[2] = f(fz) * D50_XYZ[2];
}

fn xyz_to_lab(values: &mut [f32]) {
    // Kotlin's `cbrt` of a float is the double's, narrowed.
    let f = |t: f32| if t > LAB_A { f64::from(t).cbrt() as f32 } else { LAB_B * t + LAB_C };
    let fx = f(values[0] / D50_XYZ[0]);
    let fy = f(values[1] / D50_XYZ[1]);
    let fz = f(values[2] / D50_XYZ[2]);
    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let b = 200.0 * (fy - fz);
    values[0] = coerce_in(l, 0.0, 100.0);
    values[1] = coerce_in(a, -128.0, 128.0);
    values[2] = coerce_in(b, -128.0, 128.0);
}

// Oklab, as Compose's `Oklab`: `M1` takes D50 XYZ to D65 before Oklab's matrix.
struct OklabMatrices {
    m1: [f32; 9],
    inverse_m1: [f32; 9],
    inverse_m2: [f32; 9],
}

const OKLAB_M2: [f32; 9] = [
    0.210_454_26,
    1.977_998_5,
    0.025_904_037,
    0.793_617_8,
    -2.428_592_2,
    0.782_771_77,
    -0.004_072_047,
    0.450_593_7,
    -0.808_675_77,
];

fn oklab_matrices() -> &'static OklabMatrices {
    static MATRICES: LazyLock<OklabMatrices> = LazyLock::new(|| {
        let m1 = mul3x3(
            &[
                0.818_933,
                0.032_984_544,
                0.048_200_3,
                0.361_866_74,
                0.929_311_9,
                0.264_366_27,
                -0.128_859_71,
                0.036_145_64,
                0.633_851_7,
            ],
            &chromatic_adaptation(
                &Adaptation::BRADFORD.transform,
                Illuminant::D50.to_xyz(),
                Illuminant::D65.to_xyz(),
            ),
        );
        OklabMatrices { m1, inverse_m1: inverse3x3(&m1), inverse_m2: inverse3x3(&OKLAB_M2) }
    });
    &MATRICES
}

fn oklab_to_xyz(values: &mut [f32]) {
    let matrices = oklab_matrices();
    values[0] = coerce_in(values[0], 0.0, 1.0);
    values[1] = coerce_in(values[1], -0.5, 0.5);
    values[2] = coerce_in(values[2], -0.5, 0.5);
    mul3x3_in_place(&matrices.inverse_m2, values);
    for value in &mut values[..3] {
        *value = *value * *value * *value;
    }
    mul3x3_in_place(&matrices.inverse_m1, values);
}

fn xyz_to_oklab(values: &mut [f32]) {
    mul3x3_in_place(&oklab_matrices().m1, values);
    for value in &mut values[..3] {
        *value = fast_cbrt(*value);
    }
    mul3x3_in_place(&OKLAB_M2, values);
}
