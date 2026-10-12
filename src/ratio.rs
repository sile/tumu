//! The odds a stack splits by: a plain rational, kept exact.
//!
//! A stack splits what is left at each step in the proportion `num : den` --
//! `ratio` standing for the sheet at hand and `1` for every sheet below it.
//! Keeping the two sides as whole numbers, rather than a float or a per-sheet
//! weight, is what makes the arrangement independent of the terminal's size: a
//! rotation changes the rows being divided, not the odds dividing them, so the
//! same ratio yields the same shape at any size.

/// The parts the sheet at hand claims when the user has chosen nothing.
///
/// The default `1 : 1` is the halving the README spells out. It is written here
/// rather than as a literal in `main` because `noargs` takes a `&'static str`;
/// the binary spells the same number as its built-in default and the two must
/// stay equal.
pub const DEFAULT_NUM: u32 = 1;

/// The widest a sheet's share may be made, as the `num` of a `num : 1` ratio.
///
/// A sheet claims `num` parts against the one part every sheet below it shares,
/// so the odds are always `num : 1`. Past a handful the sheet is most of the
/// screen and a step more is a change nobody can see, so `num` is kept within
/// this bound: `prefix +` at the top of the range is a no-op rather than a
/// ratio that grows without end.
pub const MAX_NUM: u32 = 5;

/// The narrowest a sheet's share may be made, as the `num` of a `num : 1`
/// ratio.
///
/// One part is the least a sheet can claim while still taking it, so the odds
/// never go below `1 : 1`: `prefix -` at the bottom of the range is a no-op
/// rather than a sheet squeezed away.
pub const MIN_NUM: u32 = 1;

/// The odds one sheet claims against the space still below it.
///
/// A ratio of `num : den` splits the rows left at each step so the sheet at
/// hand takes `num` parts and everything below takes `den`. The default of
/// `1 : 1` is the halving the README spells out. The value is a plain fraction
/// and is not normalised, so equal odds can be written several ways and
/// compare unequal by field even when they mean the same thing -- nothing in
/// the size derivation depends on the fields being reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    /// The parts the sheet at hand claims.
    pub num: u32,

    /// The parts every sheet below it claims together.
    pub den: u32,
}

impl Ratio {
    /// The odds the README gives: a sheet against all the rest, `1 : 1`.
    pub const DEFAULT: Self = Self {
        num: DEFAULT_NUM,
        den: 1,
    };

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

    /// Makes the odds `num : 1`, clamping `num` into the adjustable range.
    ///
    /// This is the one-argument form the settings use: `--ratio N` and `prefix
    /// +/-` speak of a sheet's share against everything below it as a single
    /// number, and that number is always `num` against a fixed `den` of one.
    /// Clamping rather than rejecting an out-of-range number means a `--ratio
    /// 99` starts at the widest share tumu offers instead of refusing to run,
    /// and a `0` starts at the narrowest instead of making a ratio of no odds.
    pub const fn from_num(num: u32) -> Self {
        let num = if num < MIN_NUM {
            MIN_NUM
        } else if num > MAX_NUM {
            MAX_NUM
        } else {
            num
        };
        Self { num, den: 1 }
    }

    /// Returns the odds with the sheet's share one part wider, or the same odds
    /// when it is already as wide as it may go.
    ///
    /// This is `prefix +`. Only `num` moves: `den` is kept, so in the `num : 1`
    /// form every setting and step uses, the step is one part of a fixed shape
    /// rather than a scaling of both sides. `num` is kept within
    /// [`MIN_NUM`]..=[`MAX_NUM`], so at the ceiling the share in hand is what a
    /// part wider asks for and the value comes back unchanged -- which is what
    /// makes `prefix +` at the end a no-op.
    pub fn widened(self) -> Self {
        let next = self.num.saturating_add(1);
        if next > MAX_NUM {
            self
        } else {
            Self {
                num: next,
                den: self.den,
            }
        }
    }

    /// Returns the odds with the sheet's share one part narrower, or the same
    /// odds when it is already as narrow as it may go.
    ///
    /// This is `prefix -`: the inverse of [`widened`](Ratio::widened).
    pub fn narrowed(self) -> Self {
        let next = self.num.saturating_sub(1);
        if next < MIN_NUM {
            self
        } else {
            Self {
                num: next,
                den: self.den,
            }
        }
    }

    /// Returns the share of `rows` the sheet at hand claims.
    ///
    /// This is `rows * num / (num + den)`, rounded up, so a sheet always gets
    /// at least one row of the odd remainder unless `num` is zero. A ratio of
    /// `1 : 1` halves the rows and rounds the odd one to the sheet on top.
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
