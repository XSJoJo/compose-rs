//! Interpolation, as Compose's `ui-util` `MathHelpers` (`lerp` of floats, ints and longs) and the
//! `lerp` overloads of `ui-geometry` and `ui-unit`: one [`lerp`] for every type that implements
//! [`Lerp`], so a call reads as Compose's (`lerp(start, stop, 0.5)`), D1907.

/// A value between two others, as Compose's `lerp(start, stop, fraction)` overloads.
pub trait Lerp: Sized {
    /// `start` at 0, `stop` at 1, and in between (or beyond) linearly.
    fn lerp(start: Self, stop: Self, fraction: f32) -> Self;
}

/// Between `start` and `stop` by `fraction`, as Compose's `lerp`: floats exactly at both ends
/// (`(1 - fraction) * start + fraction * stop`), integers rounded half up.
pub fn lerp<T: Lerp>(start: T, stop: T, fraction: f32) -> T {
    T::lerp(start, stop, fraction)
}

impl Lerp for f32 {
    fn lerp(start: f32, stop: f32, fraction: f32) -> f32 {
        (1.0 - fraction) * start + fraction * stop
    }
}

/// `x` rounded half up in `f64`, as Kotlin's `Math.round` on a `Double`.
fn round_half_up(x: f64) -> f64 {
    (x + 0.5).floor()
}

impl Lerp for i32 {
    /// As Compose's: the difference scaled in `f64`, rounded half up, then added.
    fn lerp(start: i32, stop: i32, fraction: f32) -> i32 {
        let step = round_half_up(f64::from(stop.wrapping_sub(start)) * f64::from(fraction));
        start.wrapping_add(step as i32)
    }
}

impl Lerp for i64 {
    /// As Compose's: the difference scaled in `f64`, rounded half up, then added.
    fn lerp(start: i64, stop: i64, fraction: f32) -> i64 {
        let step = round_half_up(stop.wrapping_sub(start) as f64 * f64::from(fraction));
        start.wrapping_add(step as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_round_half_up() {
        assert_eq!(lerp(0, 3, 0.5), 2);
        assert_eq!(lerp(0, -3, 0.5), -1);
        assert_eq!(lerp(0_i64, 3, 0.5), 2);
        assert_eq!(lerp(2.0_f32, 4.0, 0.5), 3.0);
    }
}
