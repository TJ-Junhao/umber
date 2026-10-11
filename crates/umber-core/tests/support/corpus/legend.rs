//! Output-gate cases for legend figures.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.

use umber_core::FontLibrary;

use super::super::Output;

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(_lib: &FontLibrary, _sink: &mut impl FnMut(Output)) {}
