//! The x values of a mark, as given by the caller.

/// The x values of a mark: numbers for a continuous axis, names for a category axis.
///
/// # Examples
///
/// ```
/// use umber_core::figure::AxisData;
///
/// let names = AxisData::Categories(vec!["a".into(), "b".into()]);
/// assert_ne!(names, AxisData::Numbers(vec![0.0, 1.0]));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AxisData {
    /// Values on a linear or log axis. NaN marks a gap.
    Numbers(Vec<f64>),
    /// Category names on a category axis.
    Categories(Vec<String>),
}

impl AxisData {
    /// The number of values.
    pub(super) fn len(&self) -> usize {
        match self {
            AxisData::Numbers(v) => v.len(),
            AxisData::Categories(v) => v.len(),
        }
    }
}
