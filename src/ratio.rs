//! The odds a stack splits by: a plain rational, kept exact.
//!
//! A stack splits what is left at each step in the proportion `num : den` --
//! `ratio` standing for the window at hand and `1` for every window below it.
//! Keeping the two sides as whole numbers, rather than a float or a per-window
//! weight, is what makes the arrangement independent of the terminal's size: a
//! rotation changes the rows being divided, not the odds dividing them, so the
//! same ratio yields the same shape at any size.

/// The odds one window claims against the space still below it.
///
/// A ratio of `num : den` splits the rows left at each step so the window at
/// hand takes `num` parts and everything below takes `den`. The default of
/// `1 : 1` is the halving the README spells out. The value is a plain fraction
/// and is not normalised, so equal odds can be written several ways and
/// compare unequal by field even when they mean the same thing -- nothing in
/// the size derivation depends on the fields being reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    /// The parts the window at hand claims.
    pub num: u32,

    /// The parts every window below it claims together.
    pub den: u32,
}

impl Ratio {
    /// The odds the README gives: a window against all the rest, `1 : 1`.
    pub const DEFAULT: Self = Self { num: 1, den: 1 };

    /// Makes a ratio of `num : den`.
    ///
    /// Returns `None` when both sides are zero, which would leave no odds to
    /// divide by. A side of zero is allowed and means the other side takes
    /// everything, though the min-one-row rule still gives the empty side a
    /// row.
    pub const fn new(num: u32, den: u32) -> Option<Self> {
        if num == 0 && den == 0 {
            None
        } else {
            Some(Self { num, den })
        }
    }

    /// Returns the share of `rows` the window at hand claims.
    ///
    /// This is `rows * num / (num + den)`, rounded up, so a window always gets
    /// at least one row of the odd remainder unless `num` is zero. A ratio of
    /// `1 : 1` halves the rows and rounds the odd one to the window on top.
    pub fn share_of(self, rows: usize) -> usize {
        let total = u64::from(self.num) + u64::from(self.den);
        if total == 0 || self.num == 0 {
            return 0;
        }
        let rows = rows as u64;
        let numerator = rows * u64::from(self.num);
        (numerator.div_ceil(total)) as usize
    }
}

impl Default for Ratio {
    fn default() -> Self {
        Self::DEFAULT
    }
}
