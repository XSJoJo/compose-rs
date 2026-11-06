//! Blend modes and color filters, as Compose's `BlendMode`, `ColorFilter` and `ColorMatrix`:
//! what a paint, an image or a layer composites with and recolors its pixels by.

use crate::graphics::Color;

/// How a drawing's pixels (the source) combine with what's under them (the destination), as
/// Compose's `BlendMode`: Porter-Duff's modes, then the separable and non-separable ones. The
/// order is Compose's (and Skia's), what the display list's bytes carry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum BlendMode {
    Clear,
    Src,
    Dst,
    /// The source over the destination: the default.
    #[default]
    SrcOver,
    DstOver,
    SrcIn,
    DstIn,
    SrcOut,
    DstOut,
    SrcAtop,
    DstAtop,
    Xor,
    Plus,
    Modulate,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    Hardlight,
    Softlight,
    Difference,
    Exclusion,
    Multiply,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl compose_runtime::ComposeArg for BlendMode {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// A 4 × 5 matrix transforming a color's red, green, blue and alpha (0 to 255), as Compose's
/// `ColorMatrix`: each output channel is a row's dot product with (R, G, B, A, 1), its last
/// column an offset in 0 to 255. `values` is row-major. The default is the identity.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorMatrix {
    pub values: [f32; 20],
}

const IDENTITY: [f32; 20] = [
    1.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 1.0, 0.0,
];

impl Default for ColorMatrix {
    fn default() -> Self {
        ColorMatrix { values: IDENTITY }
    }
}

impl ColorMatrix {
    /// The identity, as `ColorMatrix()`.
    pub fn new() -> ColorMatrix {
        ColorMatrix::default()
    }

    /// A matrix of these row-major values, as `ColorMatrix(values)`.
    pub fn from_values(values: [f32; 20]) -> ColorMatrix {
        ColorMatrix { values }
    }

    /// The value at `row` (0 to 3) and `column` (0 to 4), as `get(row, column)`.
    pub fn get(&self, row: usize, column: usize) -> f32 {
        self.values[row * 5 + column]
    }

    /// Sets the value at `row` and `column`, as `set(row, column, v)`.
    pub fn set(&mut self, row: usize, column: usize, value: f32) {
        self.values[row * 5 + column] = value;
    }

    /// Back to the identity, as `reset`.
    pub fn reset(&mut self) {
        self.values = IDENTITY;
    }

    /// `other`'s values, as `set(src)`.
    pub fn set_from(&mut self, other: &ColorMatrix) {
        self.values = other.values;
    }

    /// A saturation of `saturation` (0 gray, 1 unchanged), as `setToSaturation`.
    pub fn set_to_saturation(&mut self, saturation: f32) {
        self.reset();
        let inverse = 1.0 - saturation;
        let (red, green, blue) = (0.213 * inverse, 0.715 * inverse, 0.072 * inverse);
        for row in 0..3 {
            self.set(row, 0, red);
            self.set(row, 1, green);
            self.set(row, 2, blue);
            let own = self.get(row, row) + saturation;
            self.set(row, row, own);
        }
    }

    /// Each channel scaled, as `setToScale`.
    pub fn set_to_scale(&mut self, red: f32, green: f32, blue: f32, alpha: f32) {
        self.reset();
        self.set(0, 0, red);
        self.set(1, 1, green);
        self.set(2, 2, blue);
        self.set(3, 3, alpha);
    }

    /// The identity with the rotation by `degrees` in the plane of channels `first` and
    /// `second`, `sine` on `first`'s row.
    fn set_to_rotation(&mut self, degrees: f32, first: usize, second: usize) {
        self.reset();
        let (sine, cosine) = f64::from(degrees).to_radians().sin_cos();
        let (sine, cosine) = (sine as f32, cosine as f32);
        self.set(first, first, cosine);
        self.set(first, second, sine);
        self.set(second, first, -sine);
        self.set(second, second, cosine);
    }

    /// A rotation of green and blue around red's axis, as `setToRotateRed`.
    pub fn set_to_rotate_red(&mut self, degrees: f32) {
        self.set_to_rotation(degrees, 1, 2);
    }

    /// A rotation of red and blue around green's axis, as `setToRotateGreen`.
    pub fn set_to_rotate_green(&mut self, degrees: f32) {
        self.set_to_rotation(degrees, 2, 0);
    }

    /// A rotation of red and green around blue's axis, as `setToRotateBlue`.
    pub fn set_to_rotate_blue(&mut self, degrees: f32) {
        self.set_to_rotation(degrees, 0, 1);
    }

    /// From RGB to YUV, as `convertRgbToYuv`.
    pub fn convert_rgb_to_yuv(&mut self) {
        self.reset();
        let rows = [[0.299, 0.587, 0.114], [-0.16874, -0.33126, 0.5], [0.5, -0.41869, -0.08131]];
        self.set_color_rows(rows);
    }

    /// From YUV to RGB, as `convertYuvToRgb`.
    pub fn convert_yuv_to_rgb(&mut self) {
        self.reset();
        let rows = [[1.0, 0.0, 1.402], [1.0, -0.34414, -0.71414], [1.0, 1.772, 0.0]];
        self.set_color_rows(rows);
    }

    fn set_color_rows(&mut self, rows: [[f32; 3]; 3]) {
        for (row, values) in rows.iter().enumerate() {
            for (column, value) in values.iter().enumerate() {
                self.set(row, column, *value);
            }
        }
    }

    /// The color this matrix makes of `color`, as a renderer computes it (clamped to 0 to 255).
    pub fn transform(&self, color: Color) -> Color {
        let argb = color.to_argb();
        let input = [16, 8, 0, 24].map(|shift| f32::from((argb >> shift) as u8));
        let channel = |row: usize| {
            let value = (0..4).map(|column| self.get(row, column) * input[column]).sum::<f32>()
                + self.get(row, 4);
            value.round().clamp(0.0, 255.0) as u32
        };
        Color::from_argb(channel(3) << 24 | channel(0) << 16 | channel(1) << 8 | channel(2))
    }
}

/// `self` becomes `self × other`: `other` applied first, then `self`, as `timesAssign`.
impl std::ops::MulAssign for ColorMatrix {
    fn mul_assign(&mut self, other: ColorMatrix) {
        let mut result = [0.0; 20];
        for row in 0..4 {
            for column in 0..5 {
                let mut value = (0..4).map(|k| self.get(row, k) * other.get(k, column)).sum();
                if column == 4 {
                    value += self.get(row, 4);
                }
                result[row * 5 + column] = value;
            }
        }
        self.values = result;
    }
}

/// What recolors a drawing's pixels before they're blended, as Compose's `ColorFilter`: its
/// three kinds, `BlendModeColorFilter`, `LightingColorFilter` and `ColorMatrixColorFilter`, as
/// variants. Equal when their kind and values are.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorFilter {
    /// `color` blended over each pixel with `blend_mode`, as `BlendModeColorFilter`.
    BlendMode { color: Color, blend_mode: BlendMode },
    /// Each channel multiplied by `multiply`'s, then `add`'s added (alpha unchanged), as
    /// `LightingColorFilter`.
    Lighting { multiply: Color, add: Color },
    /// Each color transformed by the matrix, as `ColorMatrixColorFilter`.
    ColorMatrix(ColorMatrix),
}

impl ColorFilter {
    /// Pixels tinted with `color`, as `ColorFilter.tint(color)`: `SrcIn`, the tint as opaque
    /// as each pixel.
    pub fn tint(color: Color) -> ColorFilter {
        ColorFilter::tint_with(color, BlendMode::SrcIn)
    }

    /// Pixels tinted with `color` by `blend_mode`, as `ColorFilter.tint(color, blendMode)`.
    pub fn tint_with(color: Color, blend_mode: BlendMode) -> ColorFilter {
        ColorFilter::BlendMode { color, blend_mode }
    }

    /// As `ColorFilter.lighting(multiply, add)`.
    pub fn lighting(multiply: Color, add: Color) -> ColorFilter {
        ColorFilter::Lighting { multiply, add }
    }

    /// As `ColorFilter.colorMatrix(colorMatrix)`.
    pub fn color_matrix(color_matrix: ColorMatrix) -> ColorFilter {
        ColorFilter::ColorMatrix(color_matrix)
    }

    /// The tint's color if it's a `SrcIn` tint, the one a paint's color can take instead.
    pub(crate) fn src_in_tint(&self) -> Option<Color> {
        match self {
            ColorFilter::BlendMode { color, blend_mode: BlendMode::SrcIn } => Some(*color),
            _ => None,
        }
    }
}

impl compose_runtime::ComposeArg for ColorFilter {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// A color filter of the display list a paint, image or layer names it in
/// (`DisplayList::color_filters`), by index: a paint stays small and `Copy`.
/// Kept as the index plus one (`NonZeroU32`), so an `Option` of it is 4 bytes and a `Paint`
/// naming a shader and a filter stays 32 (28 with a 32-bit color).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ColorFilterIndex(std::num::NonZeroU32);

impl ColorFilterIndex {
    /// The one naming entry `index`.
    pub fn new(index: u32) -> ColorFilterIndex {
        ColorFilterIndex(std::num::NonZeroU32::new(index + 1).expect("an index below u32::MAX"))
    }

    /// The entry it names.
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    /// The one naming the entry `by` later: a list's entries copied after another's.
    pub fn offset(self, by: u32) -> ColorFilterIndex {
        ColorFilterIndex(self.0.checked_add(by).expect("an index below u32::MAX"))
    }
}
