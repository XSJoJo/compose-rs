//! `Constraints`: the size range a layout is measured within, as Compose's.

use std::fmt;

use compose_runtime::ComposeArg;

use crate::unit::IntSize;

/// Minimum and maximum width and height, in pixels; `Constraints::INFINITY` is unbounded.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Constraints {
    pub min_width: i32,
    pub max_width: i32,
    pub min_height: i32,
    pub max_height: i32,
}

impl Constraints {
    /// An unbounded maximum, as `Constraints.Infinity`.
    pub const INFINITY: i32 = i32::MAX;

    /// As `Constraints(minWidth, maxWidth, minHeight, maxHeight)`: panics, with Compose's message,
    /// unless each maximum is at least its minimum and the minima aren't negative.
    pub fn new(min_width: i32, max_width: i32, min_height: i32, max_height: i32) -> Constraints {
        if !(0 <= min_width
            && min_width <= max_width
            && 0 <= min_height
            && min_height <= max_height)
        {
            invalid_constraints(min_width, max_width, min_height, max_height);
        }
        Constraints { min_width, max_width, min_height, max_height }
    }

    /// Exactly `width` by `height`, as `Constraints.fixed`.
    pub fn fixed(width: i32, height: i32) -> Constraints {
        assert!(width >= 0 && height >= 0, "width({width}) and height({height}) must be >= 0");
        Constraints { min_width: width, max_width: width, min_height: height, max_height: height }
    }

    /// Exactly `width` wide, any height, as `Constraints.fixedWidth`.
    pub fn fixed_width(width: i32) -> Constraints {
        assert!(width >= 0, "width({width}) must be >= 0");
        Constraints {
            min_width: width,
            max_width: width,
            min_height: 0,
            max_height: Self::INFINITY,
        }
    }

    /// Exactly `height` high, any width, as `Constraints.fixedHeight`.
    pub fn fixed_height(height: i32) -> Constraints {
        assert!(height >= 0, "height({height}) must be >= 0");
        Constraints {
            min_width: 0,
            max_width: Self::INFINITY,
            min_height: height,
            max_height: height,
        }
    }

    /// As `Constraints.fitPrioritizingWidth`: Compose cuts the values down to what its packed
    /// `Long` holds, width first; compose-rs's fields hold every `i32`, so these are the values.
    pub fn fit_prioritizing_width(
        min_width: i32,
        max_width: i32,
        min_height: i32,
        max_height: i32,
    ) -> Constraints {
        Constraints::new(min_width, max_width, min_height, max_height)
    }

    /// As `Constraints.fitPrioritizingHeight`: the values, as
    /// [`fit_prioritizing_width`](Self::fit_prioritizing_width).
    pub fn fit_prioritizing_height(
        min_width: i32,
        max_width: i32,
        min_height: i32,
        max_height: i32,
    ) -> Constraints {
        Constraints::new(min_width, max_width, min_height, max_height)
    }

    /// A maximum is zero, so whatever is measured has no area, as `isZero`.
    pub fn is_zero(&self) -> bool {
        self.max_width == 0 || self.max_height == 0
    }

    /// Whether `size` is within both ranges, as `isSatisfiedBy`.
    pub fn is_satisfied_by(&self, size: IntSize) -> bool {
        (self.min_width..=self.max_width).contains(&size.width)
            && (self.min_height..=self.max_height).contains(&size.height)
    }

    /// The same maxima, minima zero, as `copyMaxDimensions` (and [`loosen`](Self::loosen)).
    pub fn copy_max_dimensions(&self) -> Constraints {
        self.loosen()
    }

    /// At most `width` by `height`, as `Constraints(maxWidth = …, maxHeight = …)`.
    pub fn at_most(width: i32, height: i32) -> Constraints {
        Constraints::new(0, width, 0, height)
    }

    pub fn unbounded() -> Constraints {
        Constraints::new(0, Self::INFINITY, 0, Self::INFINITY)
    }

    pub fn has_bounded_width(&self) -> bool {
        self.max_width != Self::INFINITY
    }

    pub fn has_bounded_height(&self) -> bool {
        self.max_height != Self::INFINITY
    }

    pub fn has_fixed_width(&self) -> bool {
        self.min_width == self.max_width
    }

    pub fn has_fixed_height(&self) -> bool {
        self.min_height == self.max_height
    }

    pub fn constrain_width(&self, width: i32) -> i32 {
        width.clamp(self.min_width, self.max_width)
    }

    pub fn constrain_height(&self, height: i32) -> i32 {
        height.clamp(self.min_height, self.max_height)
    }

    pub fn constrain(&self, size: IntSize) -> IntSize {
        IntSize::new(self.constrain_width(size.width), self.constrain_height(size.height))
    }

    /// The same maxima, minima dropped to zero, as `copy(minWidth = 0, minHeight = 0)`.
    pub fn loosen(&self) -> Constraints {
        Constraints::new(0, self.max_width, 0, self.max_height)
    }

    /// Grown by `horizontal` and `vertical` pixels (negative shrinks), clamped at zero, keeping
    /// infinity infinite, as Compose's `Constraints.offset`.
    pub fn offset(&self, horizontal: i32, vertical: i32) -> Constraints {
        let add = |value: i32, by: i32| {
            if value == Self::INFINITY { value } else { value.saturating_add(by).max(0) }
        };
        let max_width = add(self.max_width, horizontal);
        let max_height = add(self.max_height, vertical);
        Constraints::new(
            add(self.min_width, horizontal).min(max_width),
            max_width,
            add(self.min_height, vertical).min(max_height),
            max_height,
        )
    }

    /// These constraints restricted to `other`'s ranges, as Compose's `constrain(otherConstraints)`.
    pub fn constrain_by(&self, other: Constraints) -> Constraints {
        Constraints::new(
            other.min_width.clamp(self.min_width, self.max_width),
            other.max_width.clamp(self.min_width, self.max_width),
            other.min_height.clamp(self.min_height, self.max_height),
            other.max_height.clamp(self.min_height, self.max_height),
        )
    }
}

/// Compose's message for constraints that can't be, out of the constructor's way.
#[cold]
#[inline(never)]
#[track_caller]
fn invalid_constraints(min_width: i32, max_width: i32, min_height: i32, max_height: i32) -> ! {
    panic!(
        "maxWidth({max_width}) must be >= than minWidth({min_width}),\n\
         maxHeight({max_height}) must be >= than minHeight({min_height}),\n\
         minWidth({min_width}) and minHeight({min_height}) must be >= 0"
    )
}

/// Unbounded, as `Constraints()`.
impl Default for Constraints {
    fn default() -> Constraints {
        Constraints::unbounded()
    }
}

impl fmt::Debug for Constraints {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bound = |value: i32| {
            if value == Self::INFINITY { "inf".to_string() } else { value.to_string() }
        };
        write!(
            formatter,
            "Constraints(w {}..{}, h {}..{})",
            self.min_width,
            bound(self.max_width),
            self.min_height,
            bound(self.max_height)
        )
    }
}

impl ComposeArg for Constraints {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_shrinks_and_keeps_infinity() {
        let constraints = Constraints::new(10, 100, 0, Constraints::INFINITY);
        assert_eq!(constraints.offset(-20, -5), Constraints::new(0, 80, 0, Constraints::INFINITY));
        assert_eq!(Constraints::fixed(10, 10).offset(-30, -30), Constraints::fixed(0, 0));
        assert_eq!(Constraints::fixed(10, 10).offset(5, 0), Constraints::new(15, 15, 10, 10));
    }

    #[test]
    fn constrain_clamps() {
        let constraints = Constraints::new(10, 20, 5, 5);
        assert_eq!(constraints.constrain(IntSize::new(30, 1)), IntSize::new(20, 5));
        assert_eq!(constraints.loosen(), Constraints::new(0, 20, 0, 5));
        assert_eq!(
            Constraints::new(0, 100, 0, 100).constrain_by(Constraints::fixed(150, 50)),
            Constraints::new(100, 100, 50, 50)
        );
    }
}
