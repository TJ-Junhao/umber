//! ECDF marks: drawn as the step line of their steps, which holds each level up to the next
//! distinct value.

use super::MarkContext;
use super::step::step_items;
use crate::Rgba;
use crate::model::EcdfMark;
use crate::resolve::Resolved;
use crate::scene::Item;

/// The scene items of one ECDF in `color`, `width` pt wide: the stroke of its step line
/// ([`EcdfMark::steps`]), reduced, clipped and stroked as [`step_items`] does a step line's, and
/// nothing when no part of it is visible.
pub(super) fn ecdf_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &EcdfMark,
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    step_items(context, resolved, mark.steps(), (color, width), items);
}
