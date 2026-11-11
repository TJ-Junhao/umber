//! Named instances of variable fonts, and which of them draws a font's regular and bold faces.
//!
//! A font file whose `fvar` table has a `wght` axis and named instances is drawn in named
//! instances, chosen by their `wght` coordinate ([`choose`]). Each chosen instance is used at
//! its coordinates on every axis, as the font defines them. A file without a `wght` axis or
//! without named instances is drawn at its default location.

use std::cmp::Ordering;
use std::sync::Arc;

use read_fonts::types::{F2Dot14, Tag};
use skrifa::{FontRef, MetadataProvider};

/// The `wght` axis tag.
const WGHT: Tag = Tag::new(b"wght");

/// The weight a regular face is chosen near.
const REGULAR_WEIGHT: f32 = 400.0;

/// The weight a bold face is chosen near, and the least weight of a bold face.
const BOLD_WEIGHT: f32 = 600.0;

/// What a named instance is chosen to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// The regular face.
    Regular,
    /// The bold face of the file that also draws the regular face.
    Bold,
    /// The bold face of a file given as the bold file.
    BoldFile,
}

/// A named instance of a variable font with a `wght` axis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NamedInstance {
    /// The position of the instance in the font's instance list.
    index: usize,
    /// The `wght` coordinate.
    weight: f32,
    /// How far the instance lies from the default on the axes other than `wght`: the sum, over
    /// those axes, of the distance from the axis default as a fraction of the axis range.
    offset: f32,
    /// The location of the instance.
    location: Location,
}

impl NamedInstance {
    /// The `wght` coordinate.
    pub(crate) fn weight(&self) -> f32 {
        self.weight
    }

    /// The location of the instance.
    pub(crate) fn location(&self) -> &Location {
        &self.location
    }
}

/// A location in a font's design space, in the order of the font's axes.
///
/// The default location has no coordinates. A location whose normalized coordinates are all
/// zero is the default location too, so it holds no normalized coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Location {
    /// The normalized coordinates, as outlines, metrics and the shaper read them; empty at the
    /// default location.
    coords: Arc<[F2Dot14]>,
    /// The user coordinates the normalized ones were made from, as the PDF subsetter reads
    /// them.
    user: Arc<[(Tag, f32)]>,
}

impl Location {
    /// The normalized coordinates; empty at the default location.
    pub(crate) fn coords(&self) -> &[F2Dot14] {
        &self.coords
    }

    /// The user coordinates of every axis.
    pub(crate) fn user(&self) -> &[(Tag, f32)] {
        &self.user
    }

    /// The `wght` coordinate, when the location names one.
    pub(crate) fn weight(&self) -> Option<f32> {
        self.user
            .iter()
            .find(|(tag, _)| *tag == WGHT)
            .map(|&(_, value)| value)
    }
}

/// The named instances of `font`: none without a `wght` axis or without named instances.
pub(crate) fn named_instances(font: &FontRef<'_>) -> Vec<NamedInstance> {
    let axes = font.axes();
    let Some(wght) = axes.iter().position(|axis| axis.tag() == WGHT) else {
        return Vec::new();
    };
    font.named_instances()
        .iter()
        .enumerate()
        .filter_map(|(index, instance)| {
            let user: Vec<(Tag, f32)> = axes
                .iter()
                .zip(instance.user_coords())
                .map(|(axis, value)| (axis.tag(), value))
                .collect();
            let weight = user.get(wght).map(|&(_, value)| value)?;
            let offset = axes
                .iter()
                .zip(&user)
                .filter(|(axis, _)| axis.tag() != WGHT)
                .map(|(axis, &(_, value))| {
                    let range = axis.max_value() - axis.min_value();
                    if range > 0.0 {
                        (value - axis.default_value()).abs() / range
                    } else {
                        0.0
                    }
                })
                .sum();
            let normalized = instance.location();
            let coords: Arc<[F2Dot14]> = if normalized.coords().iter().all(|c| *c == F2Dot14::ZERO)
            {
                Arc::default()
            } else {
                normalized.coords().into()
            };
            Some(NamedInstance {
                index,
                weight,
                offset,
                location: Location {
                    coords,
                    user: user.into(),
                },
            })
        })
        .collect()
}

/// The named instance that draws `role`, from `instances`.
///
/// - [`Role::Regular`]: the instance whose weight is closest to 400.
/// - [`Role::Bold`]: among the instances of weight 600 or more, the one closest to 600; none
///   when no instance is that heavy, so the font has no bold face.
/// - [`Role::BoldFile`]: as [`Role::Bold`], or, when no instance is that heavy, the instance
///   closest to 600, because the file was given as the bold file.
///
/// Ties go to the lighter instance (350 before 450 for the regular face), then to the instance
/// nearest the font's default on its other axes (the sum, over those axes, of the distance from
/// the axis default as a fraction of the axis range), then to the first in the font's list.
/// `None` when there are no instances.
pub(crate) fn choose(instances: &[NamedInstance], role: Role) -> Option<&NamedInstance> {
    let bold = |instance: &&NamedInstance| instance.weight >= BOLD_WEIGHT;
    let (target, mut candidates): (f32, Vec<&NamedInstance>) = match role {
        Role::Regular => (REGULAR_WEIGHT, instances.iter().collect()),
        Role::Bold | Role::BoldFile => (BOLD_WEIGHT, instances.iter().filter(bold).collect()),
    };
    if candidates.is_empty() && role == Role::BoldFile {
        candidates = instances.iter().collect();
    }
    candidates.into_iter().min_by(|a, b| closer(a, b, target))
}

/// The order of `a` and `b` by [`choose`]'s rule for an instance near `target`.
fn closer(a: &NamedInstance, b: &NamedInstance, target: f32) -> Ordering {
    (a.weight - target)
        .abs()
        .total_cmp(&(b.weight - target).abs())
        .then(a.weight.total_cmp(&b.weight))
        .then(a.offset.total_cmp(&b.offset))
        .then(a.index.cmp(&b.index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(index: usize, weight: f32, offset: f32) -> NamedInstance {
        NamedInstance {
            index,
            weight,
            offset,
            location: Location::default(),
        }
    }

    fn chosen(instances: &[NamedInstance], role: Role) -> Option<usize> {
        choose(instances, role).map(|instance| instance.index)
    }

    #[test]
    fn regular_is_closest_to_400_and_bold_closest_to_600_from_above() {
        let instances = [
            instance(0, 200.0, 0.0),
            instance(1, 380.0, 0.0),
            instance(2, 590.0, 0.0),
            instance(3, 700.0, 0.0),
            instance(4, 900.0, 0.0),
        ];
        assert_eq!(chosen(&instances, Role::Regular), Some(1));
        assert_eq!(chosen(&instances, Role::Bold), Some(3));
        assert_eq!(chosen(&instances, Role::BoldFile), Some(3));
    }

    #[test]
    fn ties_go_to_the_lighter_then_the_nearer_default_then_the_first() {
        let lighter = [instance(0, 450.0, 0.0), instance(1, 350.0, 0.0)];
        assert_eq!(chosen(&lighter, Role::Regular), Some(1));
        let nearer = [instance(0, 400.0, 0.5), instance(1, 400.0, 0.0)];
        assert_eq!(chosen(&nearer, Role::Regular), Some(1));
        let first = [instance(0, 600.0, 0.25), instance(1, 600.0, 0.25)];
        assert_eq!(chosen(&first, Role::Bold), Some(0));
    }

    #[test]
    fn without_an_instance_of_600_only_a_bold_file_has_a_bold_instance() {
        let instances = [instance(0, 300.0, 0.0), instance(1, 500.0, 0.0)];
        assert_eq!(chosen(&instances, Role::Bold), None);
        assert_eq!(chosen(&instances, Role::BoldFile), Some(1));
        assert_eq!(chosen(&[], Role::Regular), None);
        assert_eq!(chosen(&[], Role::BoldFile), None);
    }
}
