//! Error type shared by all of umber-core.

/// Every failure umber-core can report.
///
/// Each non-I/O variant carries the API call where the problem was detected (`context`,
/// e.g. `"Axes.line()"`) and a message of the form `"<what is wrong>; <what is required>."`.
/// The Python layer maps `Data` to `umber.DataError`, `Style` to `umber.StyleError`,
/// `Usage` to `umber.UsageError`, and `Io` to the built-in `OSError`.
///
/// # Examples
///
/// ```
/// let err = umber_core::Error::Style {
///     context: "Color.hex()",
///     message: "\"#12\" is not a hex color; use #RRGGBB or #RRGGBBAA.".into(),
/// };
/// assert!(err.to_string().starts_with("Color.hex(): "));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The data passed to a call is unusable, e.g. lengths differ or a value is infinite.
    #[error("{context}: {message}")]
    Data {
        /// The API call where the problem was detected, e.g. `"Axes.line()"`.
        context: &'static str,
        /// What is wrong and what is required.
        message: String,
    },
    /// A non-data argument has an invalid value, e.g. a malformed color or a negative size.
    #[error("{context}: {message}")]
    Style {
        /// The API call where the problem was detected.
        context: &'static str,
        /// What is wrong and what is required.
        message: String,
    },
    /// The API was used in an unsupported way, e.g. adding a second axes in v0.1.
    #[error("{context}: {message}")]
    Usage {
        /// The API call where the problem was detected.
        context: &'static str,
        /// What is wrong and what is required.
        message: String,
    },
    /// Reading or writing a file failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Shorthand for `std::result::Result<T, umber_core::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_context_then_message() {
        let err = Error::Data {
            context: "Axes.line()",
            message: "x has 3 values but y has 2; x and y must have the same length.".into(),
        };
        assert_eq!(
            err.to_string(),
            "Axes.line(): x has 3 values but y has 2; x and y must have the same length."
        );
    }

    #[test]
    fn io_errors_convert() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: Error = io.into();
        assert!(matches!(err, Error::Io(_)));
    }
}
