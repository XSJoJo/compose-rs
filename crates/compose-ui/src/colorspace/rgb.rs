//! RGB color spaces, as Compose's `colorspace.Rgb`, and the named ones.

use std::fmt;
use std::sync::Arc;

use super::{
    BT2020_PRIMARIES, ColorModel, ColorSpace, Illuminant, Kind, MIN_ID, NTSC_1953_PRIMARIES,
    SRGB_PRIMARIES, SRGB_TRANSFER_PARAMETERS, TransferParameters, WhitePoint, abs_rcp_response,
    abs_response, compare_floats, compare_white_points, hlg_eotf, hlg_oetf, inverse3x3,
    mul3x3_float3, mul3x3_in_place, pq_eotf, pq_oetf, rcp_response, response,
};

/// A transfer function: an OETF (linear to encoded) or an EOTF (encoded to linear).
#[derive(Clone)]
pub(crate) enum Transfer {
    Identity,
    /// `x` (0 below 0) to the power, as the gamma constructors' functions.
    Power(f64),
    Response(TransferParameters),
    RcpResponse(TransferParameters),
    /// Mirrored below 0, as extended sRGB's.
    AbsResponse(TransferParameters),
    AbsRcpResponse(TransferParameters),
    HlgOetf(TransferParameters),
    HlgEotf(TransferParameters),
    PqOetf(TransferParameters),
    PqEotf(TransferParameters),
    Function(Arc<dyn Fn(f64) -> f64 + Send + Sync>),
}

impl Transfer {
    #[inline]
    pub(crate) fn apply(&self, x: f64) -> f64 {
        match self {
            Transfer::Identity => x,
            Transfer::Power(power) => (if x < 0.0 { 0.0 } else { x }).powf(*power),
            Transfer::Response(parameters) => response(x, parameters),
            Transfer::RcpResponse(parameters) => rcp_response(x, parameters),
            Transfer::AbsResponse(parameters) => abs_response(x, parameters),
            Transfer::AbsRcpResponse(parameters) => abs_rcp_response(x, parameters),
            Transfer::HlgOetf(parameters) => hlg_oetf(parameters, x),
            Transfer::HlgEotf(parameters) => hlg_eotf(parameters, x),
            Transfer::PqOetf(parameters) => pq_oetf(parameters, x),
            Transfer::PqEotf(parameters) => pq_eotf(parameters, x),
            Transfer::Function(function) => function(x),
        }
    }

    /// The OETF of `parameters`, as Compose's `generateOetf`.
    fn oetf_of(parameters: TransferParameters) -> Transfer {
        if parameters.is_hlg_ish() {
            Transfer::HlgOetf(parameters)
        } else if parameters.is_pq_ish() {
            Transfer::PqOetf(parameters)
        } else {
            Transfer::RcpResponse(parameters)
        }
    }

    /// The EOTF of `parameters`, as Compose's `generateEotf`.
    fn eotf_of(parameters: TransferParameters) -> Transfer {
        if parameters.is_hlg_ish() {
            Transfer::HlgEotf(parameters)
        } else if parameters.is_pq_ish() {
            Transfer::PqEotf(parameters)
        } else {
            Transfer::Response(parameters)
        }
    }

    /// Whether it's the same function, as Compose compares an RGB space's functions: a
    /// function of the app's by identity.
    fn same(&self, other: &Transfer) -> bool {
        match (self, other) {
            (Transfer::Identity, Transfer::Identity) => true,
            (Transfer::Power(a), Transfer::Power(b)) => a == b,
            (Transfer::Function(a), Transfer::Function(b)) => Arc::ptr_eq(a, b),
            (Transfer::Response(a), Transfer::Response(b))
            | (Transfer::RcpResponse(a), Transfer::RcpResponse(b))
            | (Transfer::AbsResponse(a), Transfer::AbsResponse(b))
            | (Transfer::AbsRcpResponse(a), Transfer::AbsRcpResponse(b))
            | (Transfer::HlgOetf(a), Transfer::HlgOetf(b))
            | (Transfer::HlgEotf(a), Transfer::HlgEotf(b))
            | (Transfer::PqOetf(a), Transfer::PqOetf(b))
            | (Transfer::PqEotf(a), Transfer::PqEotf(b)) => a == b,
            _ => false,
        }
    }
}

/// An RGB space's values.
pub(crate) struct RgbData {
    /// Red, green and blue's xy chromaticities.
    pub(crate) primaries: [f32; 6],
    pub(crate) white_point: WhitePoint,
    /// RGB (linear) to XYZ, column-major.
    pub(crate) transform: [f32; 9],
    pub(crate) inverse_transform: [f32; 9],
    oetf: Transfer,
    eotf: Transfer,
    pub(crate) min: f32,
    pub(crate) max: f32,
    pub(crate) transfer_parameters: Option<TransferParameters>,
    pub(crate) is_wide_gamut: bool,
    pub(crate) is_srgb: bool,
}

impl RgbData {
    /// The OETF, its result kept in the range, as `oetfFunc`.
    #[inline]
    pub(crate) fn oetf(&self, x: f64) -> f64 {
        self.oetf.apply(x).clamp(f64::from(self.min), f64::from(self.max))
    }

    /// The EOTF of a value kept in the range, as `eotfFunc`.
    #[inline]
    pub(crate) fn eotf(&self, x: f64) -> f64 {
        self.eotf.apply(x.clamp(f64::from(self.min), f64::from(self.max)))
    }

    pub(crate) fn decode_values(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        [
            self.eotf(f64::from(r)) as f32,
            self.eotf(f64::from(g)) as f32,
            self.eotf(f64::from(b)) as f32,
        ]
    }

    pub(crate) fn encode_values(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        [
            self.oetf(f64::from(r)) as f32,
            self.oetf(f64::from(g)) as f32,
            self.oetf(f64::from(b)) as f32,
        ]
    }

    pub(crate) fn to_xyz_values(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let [r, g, b] = self.decode_values(r, g, b);
        mul3x3_float3(&self.transform, r, g, b)
    }

    pub(crate) fn to_xyz_in_place(&self, values: &mut [f32]) {
        let xyz = self.to_xyz_values(values[0], values[1], values[2]);
        values[..3].copy_from_slice(&xyz);
    }

    pub(crate) fn xyz_to_values(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let [r, g, b] = mul3x3_float3(&self.inverse_transform, x, y, z);
        self.encode_values(r, g, b)
    }

    pub(crate) fn xyz_to_values_in_place(&self, values: &mut [f32]) {
        mul3x3_in_place(&self.inverse_transform, values);
        let encoded = self.encode_values(values[0], values[1], values[2]);
        values[..3].copy_from_slice(&encoded);
    }

    /// The same space with `transform` and `white_point` (an adapted one), unnamed, as
    /// Compose's `Rgb(colorSpace, transform, whitePoint)`.
    pub(crate) fn adapted(
        &self,
        name: &str,
        transform: [f32; 9],
        white_point: WhitePoint,
    ) -> ColorSpace {
        build(
            name,
            &self.primaries,
            white_point,
            Some(transform),
            self.oetf.clone(),
            self.eotf.clone(),
            self.min,
            self.max,
            self.transfer_parameters,
            MIN_ID,
        )
    }

    /// Compose's `Rgb.equals`, past the name, model and id.
    pub(crate) fn same(&self, other: &RgbData) -> bool {
        if self.min.to_bits() != other.min.to_bits()
            || self.max.to_bits() != other.max.to_bits()
            || self.white_point != other.white_point
            || self.primaries.iter().zip(&other.primaries).any(|(a, b)| a.to_bits() != b.to_bits())
        {
            return false;
        }
        match (self.transfer_parameters, other.transfer_parameters) {
            (Some(a), b) => Some(a) == b,
            (None, Some(_)) => false,
            (None, None) => self.oetf.same(&other.oetf) && self.eotf.same(&other.eotf),
        }
    }
}

/// An RGB color space, as Compose's `Rgb`: a [`ColorSpace`] (it dereferences to it) with
/// primaries, a white point and transfer functions. Made by its constructors, or taken from a
/// space with [`ColorSpace::as_rgb`].
#[derive(Clone)]
pub struct Rgb(pub(crate) ColorSpace);

impl std::ops::Deref for Rgb {
    type Target = ColorSpace;
    fn deref(&self) -> &ColorSpace {
        &self.0
    }
}

impl PartialEq for Rgb {
    fn eq(&self, other: &Rgb) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<ColorSpace> for Rgb {
    fn eq(&self, other: &ColorSpace) -> bool {
        self.0 == *other
    }
}

impl fmt::Debug for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl From<Rgb> for ColorSpace {
    fn from(rgb: Rgb) -> ColorSpace {
        rgb.0
    }
}

impl Rgb {
    fn data(&self) -> &RgbData {
        self.0.rgb_data().expect("compose-ui: an Rgb is an RGB space")
    }

    /// From its RGB to XYZ transform (column-major, D50), and its OETF and EOTF, in 0 to 1, as
    /// `Rgb(name, toXYZ, oetf, eotf)`.
    ///
    /// # Panics
    ///
    /// If `name` is empty, as Compose throws.
    pub fn new(
        name: &str,
        to_xyz: [f32; 9],
        oetf: impl Fn(f64) -> f64 + Send + Sync + 'static,
        eotf: impl Fn(f64) -> f64 + Send + Sync + 'static,
    ) -> Rgb {
        Rgb(build(
            name,
            &compute_primaries(&to_xyz),
            compute_white_point(&to_xyz),
            None,
            Transfer::Function(Arc::new(oetf)),
            Transfer::Function(Arc::new(eotf)),
            0.0,
            1.0,
            None,
            MIN_ID,
        ))
    }

    /// From its primaries (6 values, xy, or 9, XYZ), white point, OETF and EOTF, in `min` to
    /// `max`, as `Rgb(name, primaries, whitePoint, oetf, eotf, min, max)`.
    ///
    /// # Panics
    ///
    /// If `name` is empty, the primaries aren't 6 or 9 values, or `min` isn't below `max`, as
    /// Compose throws.
    pub fn from_primaries(
        name: &str,
        primaries: &[f32],
        white_point: WhitePoint,
        oetf: impl Fn(f64) -> f64 + Send + Sync + 'static,
        eotf: impl Fn(f64) -> f64 + Send + Sync + 'static,
        min: f32,
        max: f32,
    ) -> Rgb {
        Rgb(build(
            name,
            primaries,
            white_point,
            None,
            Transfer::Function(Arc::new(oetf)),
            Transfer::Function(Arc::new(eotf)),
            min,
            max,
            None,
            MIN_ID,
        ))
    }

    /// From its RGB to XYZ transform and an ICC parametric curve, as
    /// `Rgb(name, toXYZ, function)`.
    pub fn with_transfer_parameters(
        name: &str,
        to_xyz: [f32; 9],
        function: TransferParameters,
    ) -> Rgb {
        Rgb(from_parameters(
            name,
            &compute_primaries(&to_xyz),
            compute_white_point(&to_xyz),
            function,
            MIN_ID,
        ))
    }

    /// From its primaries, white point and an ICC parametric curve, as
    /// `Rgb(name, primaries, whitePoint, function)`.
    pub fn from_primaries_with_transfer_parameters(
        name: &str,
        primaries: &[f32],
        white_point: WhitePoint,
        function: TransferParameters,
    ) -> Rgb {
        Rgb(from_parameters(name, primaries, white_point, function, MIN_ID))
    }

    /// From its RGB to XYZ transform and a gamma, as `Rgb(name, toXYZ, gamma)`.
    pub fn with_gamma(name: &str, to_xyz: [f32; 9], gamma: f64) -> Rgb {
        Rgb(from_gamma(
            name,
            &compute_primaries(&to_xyz),
            compute_white_point(&to_xyz),
            gamma,
            0.0,
            1.0,
            MIN_ID,
        ))
    }

    /// From its primaries, white point and a gamma, as `Rgb(name, primaries, whitePoint, gamma)`.
    pub fn from_primaries_with_gamma(
        name: &str,
        primaries: &[f32],
        white_point: WhitePoint,
        gamma: f64,
    ) -> Rgb {
        Rgb(from_gamma(name, primaries, white_point, gamma, 0.0, 1.0, MIN_ID))
    }

    pub fn white_point(&self) -> WhitePoint {
        self.data().white_point
    }

    /// Red, green and blue's xy chromaticities, as `getPrimaries`.
    pub fn primaries(&self) -> [f32; 6] {
        self.data().primaries
    }

    /// RGB (linear) to XYZ, column-major, as `getTransform`.
    pub fn transform(&self) -> [f32; 9] {
        self.data().transform
    }

    /// XYZ to RGB (linear), column-major, as `getInverseTransform`.
    pub fn inverse_transform(&self) -> [f32; 9] {
        self.data().inverse_transform
    }

    /// Its ICC parametric curve, if it has one, as `transferParameters`.
    pub fn transfer_parameters(&self) -> Option<TransferParameters> {
        self.data().transfer_parameters
    }

    /// Its OETF (linear to encoded), the result kept in its range, as `oetf`.
    pub fn oetf(&self, x: f64) -> f64 {
        self.data().oetf(x)
    }

    /// Its EOTF (encoded to linear) of a value kept in its range, as `eotf`.
    pub fn eotf(&self, x: f64) -> f64 {
        self.data().eotf(x)
    }

    /// `(r, g, b)` with the EOTF applied, as `toLinear(r, g, b)`.
    pub fn to_linear(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        self.data().decode_values(r, g, b)
    }

    /// `values[..3]` with the EOTF applied in place, as `toLinear(v)`.
    pub fn to_linear_in_place(&self, values: &mut [f32]) {
        let linear = self.data().decode_values(values[0], values[1], values[2]);
        values[..3].copy_from_slice(&linear);
    }

    /// `(r, g, b)` with the OETF applied, as `fromLinear(r, g, b)`.
    pub fn from_linear(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        self.data().encode_values(r, g, b)
    }

    /// `values[..3]` with the OETF applied in place, as `fromLinear(v)`.
    pub fn from_linear_in_place(&self, values: &mut [f32]) {
        let encoded = self.data().encode_values(values[0], values[1], values[2]);
        values[..3].copy_from_slice(&encoded);
    }
}

fn from_parameters(
    name: &str,
    primaries: &[f32],
    white_point: WhitePoint,
    function: TransferParameters,
    id: i32,
) -> ColorSpace {
    build(
        name,
        primaries,
        white_point,
        None,
        Transfer::oetf_of(function),
        Transfer::eotf_of(function),
        0.0,
        1.0,
        Some(function),
        id,
    )
}

fn from_gamma(
    name: &str,
    primaries: &[f32],
    white_point: WhitePoint,
    gamma: f64,
    min: f32,
    max: f32,
    id: i32,
) -> ColorSpace {
    let (oetf, eotf) = if gamma == 1.0 {
        (Transfer::Identity, Transfer::Identity)
    } else {
        (Transfer::Power(1.0 / gamma), Transfer::Power(gamma))
    };
    let parameters = TransferParameters::new(gamma, 1.0, 0.0, 0.0, 0.0);
    build(name, primaries, white_point, None, oetf, eotf, min, max, Some(parameters), id)
}

/// Compose's primary constructor.
#[allow(clippy::too_many_arguments)]
fn build(
    name: &str,
    primaries: &[f32],
    white_point: WhitePoint,
    transform: Option<[f32; 9]>,
    oetf: Transfer,
    eotf: Transfer,
    min: f32,
    max: f32,
    transfer_parameters: Option<TransferParameters>,
    id: i32,
) -> ColorSpace {
    // The name first, as `ColorSpace`'s init runs before `Rgb`'s.
    assert!(
        !name.is_empty(),
        "compose-ui: the name of a color space cannot be null and must contain at least 1 \
         character"
    );
    assert!(
        primaries.len() == 6 || primaries.len() == 9,
        "compose-ui: the color space's primaries must be defined as an array of 6 floats in xyY \
         or 9 floats in XYZ"
    );
    assert!(
        min < max,
        "compose-ui: invalid range: min={min}, max={max}; min must be strictly < max"
    );
    let primaries = xy_primaries(primaries);
    let transform = transform.unwrap_or_else(|| compute_xyz_matrix(&primaries, white_point));
    let inverse_transform = inverse3x3(&transform);
    let is_wide_gamut = is_wide_gamut(&primaries, min, max);
    let is_srgb = is_srgb(&primaries, white_point, &oetf, &eotf, min, max, id);
    let data = RgbData {
        primaries,
        white_point,
        transform,
        inverse_transform,
        oetf,
        eotf,
        min,
        max,
        transfer_parameters,
        is_wide_gamut,
        is_srgb,
    };
    ColorSpace::new(name, ColorModel::Rgb, id, Kind::Rgb(data))
}

fn is_srgb(
    primaries: &[f32; 6],
    white_point: WhitePoint,
    oetf: &Transfer,
    eotf: &Transfer,
    min: f32,
    max: f32,
    id: i32,
) -> bool {
    if id == 0 {
        return true;
    }
    if !compare_floats(primaries, &SRGB_PRIMARIES)
        || !compare_white_points(white_point, Illuminant::D65)
        || min != 0.0
        || max != 1.0
    {
        return false;
    }
    let srgb_oetf = Transfer::RcpResponse(SRGB_TRANSFER_PARAMETERS);
    let srgb_eotf = Transfer::Response(SRGB_TRANSFER_PARAMETERS);
    let mut x = 0.0;
    while x <= 1.0 {
        if (oetf.apply(x) - srgb_oetf.apply(x)).abs() > 1e-3
            || (eotf.apply(x) - srgb_eotf.apply(x)).abs() > 1e-3
        {
            return false;
        }
        x += 1.0 / 255.0;
    }
    true
}

fn is_wide_gamut(primaries: &[f32; 6], min: f32, max: f32) -> bool {
    (area(primaries) / area(&NTSC_1953_PRIMARIES) > 0.9 && contains(primaries, &SRGB_PRIMARIES))
        || (min < 0.0 && max > 1.0)
}

fn area(primaries: &[f32; 6]) -> f32 {
    let [rx, ry, gx, gy, bx, by] = *primaries;
    let det = rx * gy + ry * bx + gx * by - gy * bx - ry * gx - rx * by;
    let r = 0.5 * det;
    if r < 0.0 { -r } else { r }
}

fn cross(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    ax * by - ay * bx
}

/// Whether the triangle `p1` contains `p2`.
fn contains(p1: &[f32; 6], p2: &[f32; 6]) -> bool {
    let p0: [f32; 6] = std::array::from_fn(|index| p1[index] - p2[index]);
    if cross(p0[0], p0[1], p2[0] - p2[4], p2[1] - p2[5]) < 0.0
        || cross(p2[0] - p2[2], p2[1] - p2[3], p0[0], p0[1]) < 0.0
    {
        return false;
    }
    if cross(p0[2], p0[3], p2[2] - p2[0], p2[3] - p2[1]) < 0.0
        || cross(p2[2] - p2[4], p2[3] - p2[5], p0[2], p0[3]) < 0.0
    {
        return false;
    }
    !(cross(p0[4], p0[5], p2[4] - p2[2], p2[5] - p2[3]) < 0.0
        || cross(p2[4] - p2[0], p2[5] - p2[1], p0[4], p0[5]) < 0.0)
}

pub(crate) fn compute_primaries(to_xyz: &[f32; 9]) -> [f32; 6] {
    let r = mul3x3_float3(to_xyz, 1.0, 0.0, 0.0);
    let g = mul3x3_float3(to_xyz, 0.0, 1.0, 0.0);
    let b = mul3x3_float3(to_xyz, 0.0, 0.0, 1.0);
    let r_sum = r[0] + r[1] + r[2];
    let g_sum = g[0] + g[1] + g[2];
    let b_sum = b[0] + b[1] + b[2];
    [r[0] / r_sum, r[1] / r_sum, g[0] / g_sum, g[1] / g_sum, b[0] / b_sum, b[1] / b_sum]
}

fn compute_white_point(to_xyz: &[f32; 9]) -> WhitePoint {
    let w = mul3x3_float3(to_xyz, 1.0, 1.0, 1.0);
    let sum = w[0] + w[1] + w[2];
    WhitePoint::new(w[0] / sum, w[1] / sum)
}

fn xy_primaries(primaries: &[f32]) -> [f32; 6] {
    if primaries.len() == 9 {
        let xy = |index: usize| {
            let sum = primaries[index] + primaries[index + 1] + primaries[index + 2];
            (primaries[index] / sum, primaries[index + 1] / sum)
        };
        let ((rx, ry), (gx, gy), (bx, by)) = (xy(0), xy(3), xy(6));
        [rx, ry, gx, gy, bx, by]
    } else {
        std::array::from_fn(|index| primaries[index])
    }
}

fn compute_xyz_matrix(primaries: &[f32; 6], white_point: WhitePoint) -> [f32; 9] {
    let [rx, ry, gx, gy, bx, by] = *primaries;
    let (wx, wy) = (white_point.x, white_point.y);
    let one_rx_ry = (1.0 - rx) / ry;
    let one_gx_gy = (1.0 - gx) / gy;
    let one_bx_by = (1.0 - bx) / by;
    let one_wx_wy = (1.0 - wx) / wy;
    let rx_ry = rx / ry;
    let gx_gy = gx / gy;
    let bx_by = bx / by;
    let wx_wy = wx / wy;
    let by_numerator =
        (one_wx_wy - one_rx_ry) * (gx_gy - rx_ry) - (wx_wy - rx_ry) * (one_gx_gy - one_rx_ry);
    let by_denominator =
        (one_bx_by - one_rx_ry) * (gx_gy - rx_ry) - (bx_by - rx_ry) * (one_gx_gy - one_rx_ry);
    let b_y = by_numerator / by_denominator;
    let g_y = (wx_wy - rx_ry - b_y * (bx_by - rx_ry)) / (gx_gy - rx_ry);
    let r_y = 1.0 - g_y - b_y;
    let r_y_ry = r_y / ry;
    let g_y_gy = g_y / gy;
    let b_y_by = b_y / by;
    [
        r_y_ry * rx,
        r_y,
        r_y_ry * (1.0 - rx - ry),
        g_y_gy * gx,
        g_y,
        g_y_gy * (1.0 - gx - gy),
        b_y_by * bx,
        b_y,
        b_y_by * (1.0 - bx - by),
    ]
}

/// Compose's `ColorSpaces`, by id.
pub(super) fn named_spaces() -> Vec<ColorSpace> {
    let bt709_parameters =
        || TransferParameters::new(1.0 / 0.45, 1.0 / 1.099, 0.099 / 1.099, 1.0 / 4.5, 0.081);
    let none_parameters =
        TransferParameters::new(2.2, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045);
    let hlg_parameters = TransferParameters::new_with(
        -3.0,
        2.0,
        2.0,
        1.0 / 0.17883277,
        0.28466892,
        0.55991073,
        -0.685490157,
    );
    let pq_parameters = TransferParameters::new_with(
        -2.0,
        -1.555223,
        1.860454,
        32.0 / 2523.0,
        2413.0 / 128.0,
        -2392.0 / 128.0,
        8192.0 / 1305.0,
    );
    let p3_primaries = [0.680, 0.320, 0.265, 0.690, 0.150, 0.060];
    let d65 = Illuminant::D65;
    vec![
        from_parameters("sRGB IEC61966-2.1", &SRGB_PRIMARIES, d65, SRGB_TRANSFER_PARAMETERS, 0),
        from_gamma("sRGB IEC61966-2.1 (Linear)", &SRGB_PRIMARIES, d65, 1.0, 0.0, 1.0, 1),
        build(
            "scRGB-nl IEC 61966-2-2:2003",
            &SRGB_PRIMARIES,
            d65,
            None,
            Transfer::AbsRcpResponse(SRGB_TRANSFER_PARAMETERS),
            Transfer::AbsResponse(SRGB_TRANSFER_PARAMETERS),
            -0.799,
            2.399,
            Some(SRGB_TRANSFER_PARAMETERS),
            2,
        ),
        from_gamma("scRGB IEC 61966-2-2:2003", &SRGB_PRIMARIES, d65, 1.0, -0.5, 7.499, 3),
        from_parameters("Rec. ITU-R BT.709-5", &SRGB_PRIMARIES, d65, bt709_parameters(), 4),
        from_parameters(
            "Rec. ITU-R BT.2020-1",
            &BT2020_PRIMARIES,
            d65,
            TransferParameters::new(1.0 / 0.45, 1.0 / 1.0993, 0.0993 / 1.0993, 1.0 / 4.5, 0.08145),
            5,
        ),
        from_gamma(
            "SMPTE RP 431-2-2007 DCI (P3)",
            &p3_primaries,
            WhitePoint::new(0.314, 0.351),
            2.6,
            0.0,
            1.0,
            6,
        ),
        from_parameters("Display P3", &p3_primaries, d65, SRGB_TRANSFER_PARAMETERS, 7),
        from_parameters("NTSC (1953)", &NTSC_1953_PRIMARIES, Illuminant::C, bt709_parameters(), 8),
        from_parameters(
            "SMPTE-C RGB",
            &[0.630, 0.340, 0.310, 0.595, 0.155, 0.070],
            d65,
            bt709_parameters(),
            9,
        ),
        from_gamma(
            "Adobe RGB (1998)",
            &[0.64, 0.33, 0.21, 0.71, 0.15, 0.06],
            d65,
            2.2,
            0.0,
            1.0,
            10,
        ),
        from_parameters(
            "ROMM RGB ISO 22028-2:2013",
            &[0.7347, 0.2653, 0.1596, 0.8404, 0.0366, 0.0001],
            Illuminant::D50,
            TransferParameters::new(1.8, 1.0, 0.0, 1.0 / 16.0, 0.031248),
            11,
        ),
        from_gamma(
            "SMPTE ST 2065-1:2012 ACES",
            &[0.73470, 0.26530, 0.0, 1.0, 0.00010, -0.0770],
            Illuminant::D60,
            1.0,
            -65504.0,
            65504.0,
            12,
        ),
        from_gamma(
            "Academy S-2014-004 ACEScg",
            &[0.713, 0.293, 0.165, 0.830, 0.128, 0.044],
            Illuminant::D60,
            1.0,
            -65504.0,
            65504.0,
            13,
        ),
        ColorSpace::new("Generic XYZ", ColorModel::Xyz, 14, Kind::Xyz),
        ColorSpace::new("Generic L*a*b*", ColorModel::Lab, 15, Kind::Lab),
        from_parameters("None", &SRGB_PRIMARIES, d65, none_parameters, 16),
        build(
            "Hybrid Log Gamma encoding",
            &BT2020_PRIMARIES,
            d65,
            None,
            Transfer::HlgOetf(hlg_parameters),
            Transfer::HlgEotf(hlg_parameters),
            0.0,
            1.0,
            Some(hlg_parameters),
            17,
        ),
        build(
            "Perceptual Quantizer encoding",
            &BT2020_PRIMARIES,
            d65,
            None,
            Transfer::PqOetf(pq_parameters),
            Transfer::PqEotf(pq_parameters),
            0.0,
            1.0,
            Some(pq_parameters),
            18,
        ),
        ColorSpace::new("Oklab", ColorModel::Lab, 19, Kind::Oklab),
    ]
}
