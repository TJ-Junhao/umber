//! Helpers shared by unit tests in different modules. Compiled only for `cargo test`.

pub(crate) mod polygon;
pub(crate) mod scene;

// The integration tests include this file by path, so it only uses `std` and the `png` crate.
pub(crate) mod image;

/// The height, in points, that `err` names as needed for the plot area, if it is the layout
/// error for a figure too short.
pub(crate) fn named_height(err: &crate::Error) -> Option<f64> {
    let message = err.to_string();
    let (_, rest) = message.split_once(" pt tall; at least ")?;
    rest.strip_suffix(" pt is needed for the plot area.")?
        .parse()
        .ok()
}
