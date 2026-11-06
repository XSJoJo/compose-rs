//! A 4 × 4 transform of 2D points, as Compose's `ui-graphics` `Matrix`.

use std::fmt;
use std::ops::{Index, IndexMut, MulAssign};

use crate::geometry::{MutableRect, Offset, Rect};

/// A transform of 2D points as Compose's `Matrix`: 4 × 4 values, `values[row * 4 + column]` as
/// Compose's `get(row, column)`, a point (x, y) a row vector (x, y, 0, 1) multiplied on the left,
/// so rows 0, 1 and 3 map it, divided by w (column 3) when it isn't 1. Compose's matrix wraps an
/// array it changes in place; this one is `Copy`, changed in place through `&mut`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Matrix {
    pub values: [f32; 16],
}

impl Default for Matrix {
    fn default() -> Matrix {
        Matrix::new()
    }
}

/// Sine and cosine of `degrees`, computed in `f64` as Compose's.
fn sin_cos(degrees: f32) -> (f32, f32) {
    let radians = f64::from(degrees) * (std::f64::consts::PI / 180.0);
    (radians.sin() as f32, radians.cos() as f32)
}

impl Matrix {
    /// The index of the x scale in [`values`](Self::values), as `Matrix.ScaleX`.
    pub const SCALE_X: usize = 0;
    /// As `Matrix.SkewY`.
    pub const SKEW_Y: usize = 1;
    /// As `Matrix.Perspective0`.
    pub const PERSPECTIVE_0: usize = 3;
    /// As `Matrix.SkewX`.
    pub const SKEW_X: usize = 4;
    /// As `Matrix.ScaleY`.
    pub const SCALE_Y: usize = 5;
    /// As `Matrix.Perspective1`.
    pub const PERSPECTIVE_1: usize = 7;
    /// As `Matrix.ScaleZ`.
    pub const SCALE_Z: usize = 10;
    /// As `Matrix.TranslateX`.
    pub const TRANSLATE_X: usize = 12;
    /// As `Matrix.TranslateY`.
    pub const TRANSLATE_Y: usize = 13;
    /// As `Matrix.TranslateZ`.
    pub const TRANSLATE_Z: usize = 14;
    /// As `Matrix.Perspective2`.
    pub const PERSPECTIVE_2: usize = 15;

    /// The identity, as `Matrix()`.
    pub const fn new() -> Matrix {
        let mut values = [0.0; 16];
        values[0] = 1.0;
        values[5] = 1.0;
        values[10] = 1.0;
        values[15] = 1.0;
        Matrix { values }
    }

    /// A move by (`x`, `y`).
    pub fn translation(x: f32, y: f32) -> Matrix {
        let mut matrix = Matrix::new();
        matrix.values[12] = x;
        matrix.values[13] = y;
        matrix
    }

    /// A scale by `x` and `y` around the origin.
    pub fn scaling(x: f32, y: f32) -> Matrix {
        let mut matrix = Matrix::new();
        matrix.values[0] = x;
        matrix.values[5] = y;
        matrix
    }

    /// A rotation by `degrees` (clockwise, y down) around the origin, as `rotateZ`.
    pub fn rotation(degrees: f32) -> Matrix {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let mut matrix = Matrix::new();
        matrix.values[0] = cos;
        matrix.values[1] = sin;
        matrix.values[4] = -sin;
        matrix.values[5] = cos;
        matrix
    }

    /// Makes it first do what it does, then `other`, as `this *= other` reads in Compose's
    /// column order (`other` × this).
    pub fn post_concat(&mut self, other: &Matrix) {
        let (a, b) = (other.values, self.values);
        for column in 0..4 {
            for row in 0..4 {
                self.values[column * 4 + row] =
                    (0..4).map(|k| a[k * 4 + row] * b[column * 4 + k]).sum();
            }
        }
    }

    /// Whether it's the identity, as `isIdentity`.
    pub fn is_identity(&self) -> bool {
        self.values == Matrix::new().values
    }

    /// Makes it the identity, as `reset`.
    pub fn reset(&mut self) {
        *self = Matrix::new();
    }

    /// Copies `matrix`'s values, as `setFrom`.
    pub fn set_from(&mut self, matrix: &Matrix) {
        self.values = matrix.values;
    }

    /// Inverts it, as `invert()`: a singular one stays as it is.
    pub fn invert(&mut self) {
        let m = self.values;
        let (a00, a01, a02, a03) = (m[0], m[1], m[2], m[3]);
        let (a10, a11, a12, a13) = (m[4], m[5], m[6], m[7]);
        let (a20, a21, a22, a23) = (m[8], m[9], m[10], m[11]);
        let (a30, a31, a32, a33) = (m[12], m[13], m[14], m[15]);
        let b00 = a00 * a11 - a01 * a10;
        let b01 = a00 * a12 - a02 * a10;
        let b02 = a00 * a13 - a03 * a10;
        let b03 = a01 * a12 - a02 * a11;
        let b04 = a01 * a13 - a03 * a11;
        let b05 = a02 * a13 - a03 * a12;
        let b06 = a20 * a31 - a21 * a30;
        let b07 = a20 * a32 - a22 * a30;
        let b08 = a20 * a33 - a23 * a30;
        let b09 = a21 * a32 - a22 * a31;
        let b10 = a21 * a33 - a23 * a31;
        let b11 = a22 * a33 - a23 * a32;
        let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
        if det == 0.0 {
            return;
        }
        let i = 1.0 / det;
        self.values = [
            (a11 * b11 - a12 * b10 + a13 * b09) * i,
            (-a01 * b11 + a02 * b10 - a03 * b09) * i,
            (a31 * b05 - a32 * b04 + a33 * b03) * i,
            (-a21 * b05 + a22 * b04 - a23 * b03) * i,
            (-a10 * b11 + a12 * b08 - a13 * b07) * i,
            (a00 * b11 - a02 * b08 + a03 * b07) * i,
            (-a30 * b05 + a32 * b02 - a33 * b01) * i,
            (a20 * b05 - a22 * b02 + a23 * b01) * i,
            (a10 * b10 - a11 * b08 + a13 * b06) * i,
            (-a00 * b10 + a01 * b08 - a03 * b06) * i,
            (a30 * b04 - a31 * b02 + a33 * b00) * i,
            (-a20 * b04 + a21 * b02 - a23 * b00) * i,
            (-a10 * b09 + a11 * b07 - a12 * b06) * i,
            (a00 * b09 - a01 * b07 + a02 * b06) * i,
            (-a30 * b03 + a31 * b01 - a32 * b00) * i,
            (a20 * b03 - a21 * b01 + a22 * b00) * i,
        ];
    }

    /// Where `point` goes, as `map(Offset)`.
    pub fn map(&self, point: Offset) -> Offset {
        let v = &self.values;
        let x = v[0] * point.x + v[4] * point.y + v[12];
        let y = v[1] * point.x + v[5] * point.y + v[13];
        let w = v[3] * point.x + v[7] * point.y + v[15];
        // As Compose's: a point mapped to infinity (w = 0) is the origin.
        let scale = 1.0 / w;
        let scale = if scale.is_finite() { scale } else { 0.0 };
        Offset::new(x * scale, y * scale)
    }

    /// The bounds of `rect`'s four corners mapped, as `map(Rect)`.
    pub fn map_rect(&self, rect: Rect) -> Rect {
        let corners = [
            self.map(Offset::new(rect.left, rect.top)),
            self.map(Offset::new(rect.left, rect.bottom)),
            self.map(Offset::new(rect.right, rect.top)),
            self.map(Offset::new(rect.right, rect.bottom)),
        ];
        let min = |a: f32, b: f32| if a < b { a } else { b };
        let max = |a: f32, b: f32| if a > b { a } else { b };
        Rect::new(
            min(corners[0].x, min(corners[1].x, min(corners[2].x, corners[3].x))),
            min(corners[0].y, min(corners[1].y, min(corners[2].y, corners[3].y))),
            max(corners[0].x, max(corners[1].x, max(corners[2].x, corners[3].x))),
            max(corners[0].y, max(corners[1].y, max(corners[2].y, corners[3].y))),
        )
    }

    /// Maps `rect` in place, as `map(MutableRect)`.
    pub fn map_mutable_rect(&self, rect: &mut MutableRect) {
        let mapped = self.map_rect(rect.to_rect());
        rect.set(mapped.left, mapped.top, mapped.right, mapped.bottom);
    }

    /// Rotates it by `degrees` about the x axis first (before what it does), as `rotateX`.
    pub fn rotate_x(&mut self, degrees: f32) {
        let (s, c) = sin_cos(degrees);
        for row in 0..4 {
            let (a1, a2) = (self[(row, 1)], self[(row, 2)]);
            self[(row, 1)] = a1 * c - a2 * s;
            self[(row, 2)] = a1 * s + a2 * c;
        }
    }

    /// Rotates it by `degrees` about the y axis first, as `rotateY`.
    pub fn rotate_y(&mut self, degrees: f32) {
        let (s, c) = sin_cos(degrees);
        for row in 0..4 {
            let (a0, a2) = (self[(row, 0)], self[(row, 2)]);
            self[(row, 0)] = a0 * c + a2 * s;
            self[(row, 2)] = -a0 * s + a2 * c;
        }
    }

    /// Rotates it by `degrees` about the z axis (clockwise, y down) first, as `rotateZ`.
    pub fn rotate_z(&mut self, degrees: f32) {
        let (s, c) = sin_cos(degrees);
        for column in 0..4 {
            let (a0, a1) = (self[(0, column)], self[(1, column)]);
            self[(0, column)] = c * a0 + s * a1;
            self[(1, column)] = -s * a0 + c * a1;
        }
    }

    /// Scales it by `x`, `y` and `z` first, as `scale` (Compose's defaults are 1).
    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        for (row, factor) in [(0, x), (1, y), (2, z)] {
            for column in 0..4 {
                self[(row, column)] *= factor;
            }
        }
    }

    /// Moves it by (`x`, `y`, `z`) first, as `translate` (Compose's defaults are 0).
    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        for column in 0..4 {
            // Summed in Compose's order, the translation row last.
            let moved = self[(0, column)] * x
                + self[(1, column)] * y
                + self[(2, column)] * z
                + self[(3, column)];
            self[(3, column)] = moved;
        }
    }

    /// Makes it a graphics layer's transform: around the pivot, scale, then rotate about z, y
    /// and x, then translate, as `resetToPivotedTransform` (Compose's defaults: a zero pivot,
    /// translations and rotations, unit scales).
    #[allow(clippy::too_many_arguments)]
    pub fn reset_to_pivoted_transform(
        &mut self,
        pivot_x: f32,
        pivot_y: f32,
        translation_x: f32,
        translation_y: f32,
        translation_z: f32,
        rotation_x: f32,
        rotation_y: f32,
        rotation_z: f32,
        scale_x: f32,
        scale_y: f32,
        scale_z: f32,
    ) {
        let (rsx, rcx) = sin_cos(rotation_x);
        let mut v11 = rcx;
        let mut v12 = rsx;
        let mut v21 = -rsx;
        let mut v22 = rcx;
        let v31 = translation_y * rcx - translation_z * rsx;
        let mut v32 = translation_y * rsx + translation_z * rcx;

        let (rsy, rcy) = sin_cos(rotation_y);
        let mut v00 = rcy;
        let mut v02 = -rsy;
        let mut v10 = v12 * rsy;
        v12 *= rcy;
        let mut v20 = v22 * rsy;
        v22 *= rcy;
        let v30 = translation_x * rcy + v32 * rsy;
        v32 = -translation_x * rsy + v32 * rcy;

        let (rsz, rcz) = sin_cos(rotation_z);
        let a10 = v10;
        v10 = -rsz * v00 + rcz * v10;
        v00 = rcz * v00 + rsz * a10;
        let mut v01 = rsz * v11;
        v11 *= rcz;
        let a12 = v12;
        v12 = -rsz * v02 + rcz * a12;
        v02 = rcz * v02 + rsz * a12;

        v00 *= scale_x;
        v01 *= scale_x;
        v02 *= scale_x;
        v10 *= scale_y;
        v11 *= scale_y;
        v12 *= scale_y;
        v20 *= scale_z;
        v21 *= scale_z;
        v22 *= scale_z;

        self.values = [
            v00,
            v01,
            v02,
            0.0,
            v10,
            v11,
            v12,
            0.0,
            v20,
            v21,
            v22,
            0.0,
            -pivot_x * v00 - pivot_y * v10 + v30 + pivot_x,
            -pivot_x * v01 - pivot_y * v11 + v31 + pivot_y,
            -pivot_x * v02 - pivot_y * v12 + v32,
            1.0,
        ];
    }
}

/// `matrix[(row, column)]`, as Compose's `matrix[row, column]`.
impl Index<(usize, usize)> for Matrix {
    type Output = f32;
    fn index(&self, (row, column): (usize, usize)) -> &f32 {
        &self.values[row * 4 + column]
    }
}

impl IndexMut<(usize, usize)> for Matrix {
    fn index_mut(&mut self, (row, column): (usize, usize)) -> &mut f32 {
        &mut self.values[row * 4 + column]
    }
}

/// `this *= other`, as Compose's `timesAssign`: it then does what it did, then `other`.
impl MulAssign<Matrix> for Matrix {
    fn mul_assign(&mut self, other: Matrix) {
        self.post_concat(&other);
    }
}

impl MulAssign<&Matrix> for Matrix {
    fn mul_assign(&mut self, other: &Matrix) {
        self.post_concat(other);
    }
}

impl fmt::Display for Matrix {
    /// As Compose's `toString`: four rows between bars.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..4 {
            if row > 0 {
                formatter.write_str("\n")?;
            }
            let [a, b, c, d] = [0, 1, 2, 3].map(|column| self[(row, column)]);
            write!(formatter, "|{a:?} {b:?} {c:?} {d:?}|")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_at_infinity_maps_to_the_origin() {
        let mut matrix = Matrix::new();
        matrix.values[15] = 0.0;
        assert_eq!(matrix.map(Offset::new(3.0, 4.0)), Offset::ZERO);
        assert_eq!(Matrix::translation(1.0, 2.0).map(Offset::new(3.0, 4.0)), Offset::new(4.0, 6.0));
    }

    #[test]
    fn times_assign_is_post_concat() {
        let mut matrix = Matrix::translation(1.0, 2.0);
        matrix *= Matrix::scaling(2.0, 3.0);
        assert_eq!(matrix.map(Offset::ZERO), Offset::new(2.0, 6.0));
    }
}
