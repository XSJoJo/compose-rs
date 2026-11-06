//! Half floats as a color packs them: Compose's `floatToHalf` and `halfToFloat`, bit for bit.

/// `value` as a half float's bits, rounding to nearest, as Compose's `floatToHalf` (an overflow
/// gives what Compose's gives).
pub(crate) fn float_to_half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = bits >> 31;
    let mut exponent = ((bits >> 23) & 0xff) as i32;
    let mut significand = bits & 0x7f_ffff;
    let mut out_exponent: u32 = 0;
    let mut out_significand: u32 = 0;
    if exponent == 0xff {
        // Infinite or NaN.
        out_exponent = 0x1f;
        out_significand = if significand != 0 { 0x200 } else { 0 };
    } else {
        exponent = exponent - 127 + 15;
        if exponent >= 0x1f {
            // Overflow, as Compose writes it.
            out_exponent = 0x31;
        } else if exponent <= 0 {
            // Underflow: a denormal half, or zero below its smallest.
            if exponent >= -10 {
                significand = (significand | 0x80_0000) >> (1 - exponent);
                if significand & 0x1000 != 0 {
                    significand += 0x2000;
                }
                out_significand = significand >> 13;
            }
        } else {
            out_exponent = exponent as u32;
            out_significand = significand >> 13;
            if significand & 0x1000 != 0 {
                // Round half up.
                let out = ((out_exponent << 10) | out_significand) + 1;
                return (out | (sign << 15)) as u16;
            }
        }
    }
    ((sign << 15) | (out_exponent << 10) | out_significand) as u16
}

/// A half float's value, as Compose's `halfToFloat`.
pub(crate) fn half_to_float(half: u16) -> f32 {
    let bits = u32::from(half);
    let sign = bits & 0x8000;
    let exponent = (bits >> 10) & 0x1f;
    let significand = bits & 0x3ff;
    let mut out_exponent = 0;
    let mut out_significand = 0;
    if exponent == 0 {
        if significand != 0 {
            // A denormal half, as a normal float.
            const DENORMAL_MAGIC: u32 = 126 << 23;
            let value =
                f32::from_bits(DENORMAL_MAGIC + significand) - f32::from_bits(DENORMAL_MAGIC);
            return if sign == 0 { value } else { -value };
        }
    } else {
        out_significand = significand << 13;
        if exponent == 0x1f {
            out_exponent = 0xff;
            if out_significand != 0 {
                // A quiet NaN.
                out_significand |= 0x40_0000;
            }
        } else {
            out_exponent = exponent + 127 - 15;
        }
    }
    f32::from_bits((sign << 16) | (out_exponent << 23) | out_significand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_round_trip_what_they_hold() {
        for value in [0.0f32, 1.0, -1.0, 0.5, 0.25, 65504.0, -0.799, 2.399, 6.1e-5, 1.0e-7] {
            let back = half_to_float(float_to_half(value));
            assert!((back - value).abs() <= value.abs() * 1e-3 + 1e-7, "{value} came back {back}");
        }
        assert_eq!(float_to_half(1.0), 0x3c00);
        assert_eq!(float_to_half(-2.0), 0xc000);
        assert_eq!(half_to_float(0x7bff), 65504.0);
        assert!(half_to_float(float_to_half(f32::NAN)).is_nan());
        assert_eq!(half_to_float(float_to_half(f32::INFINITY)), f32::INFINITY);
    }
}
