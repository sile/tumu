//! The stack: an ordered list of sheets whose regions are derived, not stored.
//!
//! The stack is the whole layout model. A sheet's position in the list is its
//! rank in the stack: index zero is the top, the highest index is the bottom.
//! Nothing here holds a rectangle, so the arrangement is a pure function of the
//! terminal size, the order, and the ratio -- which is what lets a rotation
//! re-lay the stack out rather than try to restore it.

use crate::{Ratio, Sheet, TermnixSessionId, geometry, id::SheetId};

/// An ordered stack of sheets filling a terminal.
#[derive(Debug, Clone)]
pub struct Stack {
    sheets: Vec<(SheetId, Sheet)>,
    next_id: std::num::NonZeroU64,
    ratio: Ratio,
}

impl Stack {
    /// Builds an empty stack with the default `1 : 1` odds.
    pub const fn new() -> Self {
        Self {
            sheets: Vec::new(),
            next_id: std::num::NonZeroU64::MIN,
            ratio: Ratio::DEFAULT,
        }
    }

    /// Returns the odds the stack splits by.
    pub fn ratio(&self) -> Ratio {
        self.ratio
    }

    /// Sets the odds the stack splits by.
    ///
    /// Every sheet's region is derived from this on demand, so the next
    /// [`regions()`](Stack::regions) reflects the change; the edge resizes the
    /// sessions to match.
    pub fn set_ratio(&mut self, ratio: Ratio) {
        self.ratio = ratio;
    }

    /// Returns the sheets from the top of the stack to the bottom.
    pub fn sheets(&self) -> &[(SheetId, Sheet)] {
        &self.sheets
    }

    /// Returns the number of sheets.
    pub fn len(&self) -> usize {
        self.sheets.len()
    }

    /// Returns whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }

    /// Pushes a new sheet onto the top of the stack and returns its id.
    ///
    /// A new sheet goes on top, as the README requires; the sheet it used to be
    /// above keeps its other neighbours and simply loses rows to the new one.
    pub fn push_top(&mut self) -> SheetId {
        let id = SheetId::new(self.next_id);
        self.next_id = std::num::NonZeroU64::new(self.next_id.get() + 1)
            .expect("an id past the last used is non-zero");
        self.sheets.insert(0, (id, Sheet::new()));
        id
    }

    /// Removes the sheet at `index`, returning it.
    ///
    /// Returns `None` when there is no such sheet. The remaining sheets keep
    /// their order and re-derive their regions, so closing one grows the rest.
    pub fn remove(&mut self, index: usize) -> Option<(SheetId, Sheet)> {
        if index < self.sheets.len() {
            Some(self.sheets.remove(index))
        } else {
            None
        }
    }

    /// Moves the sheet at `index` to the top of the stack.
    ///
    /// Does nothing when there is no such sheet. This is `prefix t`: the sheet
    /// is lifted out and put back first, the sheets above it closing ranks
    /// behind it. There is no matching `move_to_bottom`, because putting a sheet
    /// at the bottom is not a command -- every sheet that is not at the top
    /// shares that place, so "the bottom" names no one place the keyboard could
    /// be aimed at.
    pub fn move_to_top(&mut self, index: usize) {
        if index < self.sheets.len() {
            let entry = self.sheets.remove(index);
            self.sheets.insert(0, entry);
        }
    }

    /// Attaches `session` to the sheet at `index`.
    ///
    /// Does nothing when there is no such sheet.
    pub fn attach_session(&mut self, index: usize, session: TermnixSessionId) {
        if let Some((_, sheet)) = self.sheets.get_mut(index) {
            sheet.session = Some(session);
        }
    }

    /// Sets the title of the sheet at `index`.
    ///
    /// Does nothing when there is no such sheet. The title is stored as it
    /// arrives -- empty included -- because it is the child's own label, and
    /// clearing it (a terminal reset) is as meaningful as setting it.
    pub fn set_title(&mut self, index: usize, title: impl Into<String>) {
        if let Some((_, sheet)) = self.sheets.get_mut(index) {
            sheet.title = title.into();
        }
    }

    /// Returns the region of every sheet, top to bottom (or left to right in a
    /// horizontal stack), for a terminal of `size`.
    ///
    /// The derivation is a left fold over the sheets along `orientation`.
    /// Each sheet takes its share of the cells left, measured against
    /// everything still below it as a single opponent; the last sheet takes
    /// whatever remains. The share is the stack's [ratio](Stack::ratio) read as
    /// `sheet : rest`, so the default `1 : 1` halves the space at each step:
    /// two sheets are `1:1` and three are `1:1` then `1:1` of the rest --
    /// exactly the odds the README gives.
    ///
    /// The axis is an argument rather than something read from `size`, because
    /// it is a setting: the same terminal can stack either way, and the size
    /// alone no longer decides.
    ///
    /// The regions are returned in stack order and their extents along the axis
    /// sum to the terminal's, the last taking whatever remains.
    pub fn regions(
        &self,
        size: tuinix::Size,
        orientation: crate::Orientation,
    ) -> Vec<tuinix::Region> {
        let mut regions = Vec::with_capacity(self.sheets.len());
        if self.sheets.is_empty() {
            return regions;
        }

        let axis = orientation;
        let mut remaining = geometry::terminal_region(size);
        for index in 0..self.sheets.len() {
            let is_last = index + 1 == self.sheets.len();
            if is_last {
                regions.push(remaining);
                break;
            }

            // The share of the cells left that this sheet claims, from the
            // stack's ratio read as `sheet : rest`. With the default odds
            // each sheet takes half of what is left, which is the `1:1` the
            // README gives.
            //
            // The share is kept within what the sheet below can leave: each
            // of them needs a cell, so this sheet may claim at most
            // `extent - below_count`. That keeps the extents summing to the
            // terminal's along the axis even when the sheets outnumber the
            // cells, instead of rounding a share up into cells that do not
            // exist.
            let extent = geometry::region_extent(remaining, axis);
            let below = self.sheets.len() - index - 1;
            let wanted = self.ratio.share_of(extent);
            let share = wanted.clamp(1, extent.saturating_sub(below).max(1));
            let (lead, rest) = geometry::split_region(remaining, share, axis);
            regions.push(lead);
            remaining = rest;
        }
        regions
    }

    /// Returns the region of the sheet at `index`, top to bottom, for a
    /// terminal of `size` laid out along `orientation`.
    ///
    /// Returns `None` when there is no such sheet.
    pub fn region_of(
        &self,
        index: usize,
        size: tuinix::Size,
        orientation: crate::Orientation,
    ) -> Option<tuinix::Region> {
        self.regions(size, orientation).into_iter().nth(index)
    }

    /// Returns the index of the sheet at `position` in a terminal of `size`
    /// laid out along `orientation`, or `None` when the position is outside
    /// every sheet.
    ///
    /// The topmost sheet wins when regions overlap, so the first match in
    /// stack order is returned.
    pub fn hit_test(
        &self,
        position: tuinix::Position,
        size: tuinix::Size,
        orientation: crate::Orientation,
    ) -> Option<usize> {
        self.regions(size, orientation).iter().position(|region| {
            let row_in = position.row >= region.position.row
                && position.row < region.position.row + region.size.rows;
            let col_in = position.col >= region.position.col
                && position.col < region.position.col + region.size.cols;
            row_in && col_in
        })
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}
