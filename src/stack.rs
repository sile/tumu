//! The stack: an ordered list of windows whose regions are derived, not stored.
//!
//! The stack is the whole layout model. A window's position in the list is its
//! rank in the stack: index zero is the top, the highest index is the bottom.
//! Nothing here holds a rectangle, so the arrangement is a pure function of the
//! terminal size, the order, and the weights -- which is what lets a rotation
//! re-lay the stack out rather than try to restore it.

use crate::{TermnixSessionId, Window, geometry, id::WindowId};

/// An ordered stack of windows filling a terminal.
#[derive(Debug, Clone)]
pub struct Stack {
    windows: Vec<(WindowId, Window)>,
    next_id: std::num::NonZeroU64,
}

impl Stack {
    /// Builds an empty stack.
    pub const fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_id: std::num::NonZeroU64::MIN,
        }
    }

    /// Returns the windows from the top of the stack to the bottom.
    pub fn windows(&self) -> &[(WindowId, Window)] {
        &self.windows
    }

    /// Returns the number of windows.
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Returns whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Pushes a new window onto the top of the stack and returns its id.
    ///
    /// A new window goes on top, as the README requires; the window it used to
    /// be above keeps its other neighbours and simply loses rows to the new one.
    pub fn push_top(&mut self) -> WindowId {
        let id = WindowId::new(self.next_id);
        self.next_id = std::num::NonZeroU64::new(self.next_id.get() + 1)
            .expect("an id past the last used is non-zero");
        self.windows.insert(0, (id, Window::new()));
        id
    }

    /// Removes the window at `index`, returning it.
    ///
    /// Returns `None` when there is no such window. The remaining windows keep
    /// their order and re-derive their regions, so closing one grows the rest.
    pub fn remove(&mut self, index: usize) -> Option<(WindowId, Window)> {
        if index < self.windows.len() {
            Some(self.windows.remove(index))
        } else {
            None
        }
    }

    /// Moves the window at `index` to the top of the stack.
    ///
    /// Does nothing when there is no such window.
    pub fn move_to_top(&mut self, index: usize) {
        if index < self.windows.len() {
            let entry = self.windows.remove(index);
            self.windows.insert(0, entry);
        }
    }

    /// Moves the window at `index` to the bottom of the stack.
    ///
    /// Does nothing when there is no such window.
    pub fn move_to_bottom(&mut self, index: usize) {
        if index < self.windows.len() {
            let entry = self.windows.remove(index);
            self.windows.push(entry);
        }
    }

    /// Attaches `session` to the window at `index`.
    ///
    /// Does nothing when there is no such window.
    pub fn attach_session(&mut self, index: usize, session: TermnixSessionId) {
        if let Some((_, window)) = self.windows.get_mut(index) {
            window.session = Some(session);
        }
    }

    /// Returns the region of every window, top to bottom, for a terminal of
    /// `size`.
    ///
    /// The derivation is a left fold over the windows. Each window takes its
    /// weight's share of the rows left, measured against a single unit for all
    /// the windows still below it; the last window takes whatever remains. With
    /// every weight one this halves the space at each step, so two windows are
    /// `1:1` and three are `1:1` then `1:1` of the rest -- exactly the odds the
    /// README gives.
    ///
    /// The regions are returned in stack order and their heights sum to the
    /// terminal's rows, the last taking whatever remains.
    pub fn regions(&self, size: tuinix::Size) -> Vec<tuinix::Region> {
        let mut regions = Vec::with_capacity(self.windows.len());
        if self.windows.is_empty() {
            return regions;
        }

        let mut remaining = geometry::terminal_region(size);
        for index in 0..self.windows.len() {
            let is_last = index + 1 == self.windows.len();
            if is_last {
                regions.push(remaining);
                break;
            }

            // The share of the rows left that this window claims. Every window
            // still below is one sibling whose weight is a single unit, so the
            // split is `own : 1`: with the default weights each window takes
            // half of what is left, which is the `1:1` odds the README gives.
            //
            // The share is kept within what the windows below can leave: each
            // of them needs a row, so this window may claim at most
            // `rows - below_count`. That keeps the heights summing to the
            // terminal's rows even when the windows outnumber them, instead of
            // rounding a share up into rows that do not exist.
            let own = u32::from(self.windows[index].1.weight);
            let rows = remaining.size.rows;
            let below = self.windows.len() - index - 1;
            let wanted = if own == 0 {
                rows
            } else {
                (rows as u32 * own).div_ceil(own + 1) as usize
            };
            let share = wanted.clamp(1, rows.saturating_sub(below).max(1));
            let (top, rest) = geometry::split_region(remaining, share);
            regions.push(top);
            remaining = rest;
        }
        regions
    }

    /// Returns the region of the window at `index`, top to bottom, for a
    /// terminal of `size`.
    ///
    /// Returns `None` when there is no such window.
    pub fn region_of(&self, index: usize, size: tuinix::Size) -> Option<tuinix::Region> {
        self.regions(size).into_iter().nth(index)
    }

    /// Returns the index of the window at `position` in a terminal of `size`,
    /// top to bottom, or `None` when the position is outside every window.
    ///
    /// The topmost window wins when regions overlap, so the first match in
    /// stack order is returned.
    pub fn hit_test(&self, position: tuinix::Position, size: tuinix::Size) -> Option<usize> {
        self.regions(size).iter().position(|region| {
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
