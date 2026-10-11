//! Error type shared by all of umber-core.

/// Every failure umber-core can report.
///
/// Each non-I/O variant carries the API call where the problem was detected (`context`,
/// e.g. `"Axes.line()"`), a message of the form `"<what is wrong>; <what is required>."`, the
/// Python parameter the error is about (`parameter`) and a one-sentence suggestion (`hint`).
/// `Display` prints only `"{context}: {message}"`; the parameter and the hint are for callers
/// that format them separately, such as the Python layer. The Python layer maps `Data` to
/// `umber.DataError`, `Style` to `umber.StyleError`, `Usage` to `umber.UsageError`, and `Io` to
/// the built-in `OSError`.
///
/// # Examples
///
/// ```
/// let err = umber_core::Error::Style {
///     context: "Color.hex()",
///     message: "\"#12\" is not a hex color; use #RRGGBB or #RRGGBBAA.".into(),
///     parameter: Some("code"),
///     hint: None,
/// };
/// assert!(err.to_string().starts_with("Color.hex(): "));
/// assert_eq!(err.parameter(), Some("code"));
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
        /// The Python parameter the error is about, e.g. `"y"`; `None` when no single parameter
        /// is at fault. Nested data names its top-level parameter: `series[1].heights[3]` is
        /// `"series"`.
        parameter: Option<&'static str>,
        /// One sentence that suggests a likely fix; `None` when there is no likely fix.
        hint: Option<&'static str>,
    },
    /// A non-data argument has an invalid value, e.g. a malformed color or a negative size.
    #[error("{context}: {message}")]
    Style {
        /// The API call where the problem was detected.
        context: &'static str,
        /// What is wrong and what is required.
        message: String,
        /// The Python parameter the error is about; `None` when no single parameter is at fault.
        parameter: Option<&'static str>,
        /// One sentence that suggests a likely fix; `None` when there is no likely fix.
        hint: Option<&'static str>,
    },
    /// The API was used in an unsupported way, e.g. adding a second axes in v0.1.
    #[error("{context}: {message}")]
    Usage {
        /// The API call where the problem was detected.
        context: &'static str,
        /// What is wrong and what is required.
        message: String,
        /// The Python parameter the error is about; `None` when no single parameter is at fault.
        parameter: Option<&'static str>,
        /// One sentence that suggests a likely fix; `None` when there is no likely fix.
        hint: Option<&'static str>,
    },
    /// Reading or writing a file failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// An [`Error::Data`] detected in `context`, without a parameter or a hint.
    pub(crate) fn data(context: &'static str, message: impl Into<String>) -> Error {
        Error::Data {
            context,
            message: message.into(),
            parameter: None,
            hint: None,
        }
    }

    /// An [`Error::Style`] detected in `context`, without a parameter or a hint.
    pub(crate) fn style(context: &'static str, message: impl Into<String>) -> Error {
        Error::Style {
            context,
            message: message.into(),
            parameter: None,
            hint: None,
        }
    }

    /// An [`Error::Usage`] detected in `context`, without a parameter or a hint.
    pub(crate) fn usage(context: &'static str, message: impl Into<String>) -> Error {
        Error::Usage {
            context,
            message: message.into(),
            parameter: None,
            hint: None,
        }
    }

    /// Names the Python parameter the error is about. [`Error::Io`] is returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// let err = umber_core::Alpha::new(1.5).unwrap_err().with_parameter("a");
    /// assert_eq!(err.parameter(), Some("a"));
    /// ```
    #[must_use]
    pub fn with_parameter(mut self, name: &'static str) -> Error {
        if let Error::Data { parameter, .. }
        | Error::Style { parameter, .. }
        | Error::Usage { parameter, .. } = &mut self
        {
            *parameter = Some(name);
        }
        self
    }

    /// Adds a one-sentence suggestion for a likely fix. [`Error::Io`] is returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// let err = umber_core::Alpha::new(1.5)
    ///     .unwrap_err()
    ///     .with_hint("pass 1.0 for a fully opaque color.");
    /// assert_eq!(err.hint(), Some("pass 1.0 for a fully opaque color."));
    /// ```
    #[must_use]
    pub fn with_hint(mut self, text: &'static str) -> Error {
        if let Error::Data { hint, .. } | Error::Style { hint, .. } | Error::Usage { hint, .. } =
            &mut self
        {
            *hint = Some(text);
        }
        self
    }

    /// Reports the error under another API call, for a caller that detects a problem on behalf of
    /// it, such as a method that saves what `Figure.to_png()` renders. The message, parameter and
    /// hint are kept. [`Error::Io`] is returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let err = figure.to_png(1).unwrap_err().with_context("Figure.save_png()");
    /// assert!(err.to_string().starts_with("Figure.save_png(): dpi 1 is out of range"));
    /// assert_eq!(err.parameter(), Some("dpi"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn with_context(mut self, new_context: &'static str) -> Error {
        if let Error::Data { context, .. }
        | Error::Style { context, .. }
        | Error::Usage { context, .. } = &mut self
        {
            *context = new_context;
        }
        self
    }

    /// The Python parameter the error is about, or `None` when no single parameter is at fault
    /// and for [`Error::Io`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let err = Figure::new(0.0, 4.0, Theme::pigment()).unwrap_err();
    /// assert_eq!(err.parameter(), Some("width"));
    /// ```
    pub fn parameter(&self) -> Option<&'static str> {
        match self {
            Error::Data { parameter, .. }
            | Error::Style { parameter, .. }
            | Error::Usage { parameter, .. } => *parameter,
            Error::Io(_) => None,
        }
    }

    /// A one-sentence suggestion for a likely fix, or `None` when there is none and for
    /// [`Error::Io`]. The hint is not part of the `Display` text.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let err = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap_err();
    /// assert_eq!(err.hint(), Some("create a new Figure for another chart."));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Error::Data { hint, .. } | Error::Style { hint, .. } | Error::Usage { hint, .. } => {
                *hint
            }
            Error::Io(_) => None,
        }
    }
}

/// Shorthand for `std::result::Result<T, umber_core::Error>`.
///
/// # Examples
///
/// ```
/// fn parse_alpha(text: &str) -> umber_core::Result<f64> {
///     text.parse().map_err(|_| umber_core::Error::Style {
///         context: "Color.alpha()",
///         message: format!("{text:?} is not a number; use a value from 0.0 to 1.0."),
///         parameter: Some("a"),
///         hint: None,
///     })
/// }
///
/// assert_eq!(parse_alpha("0.5").unwrap(), 0.5);
/// assert!(parse_alpha("half").is_err());
/// ```
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: &str) -> Error {
        match kind {
            "data" => Error::data("Axes.line()", "y[0] is inf; values must be finite."),
            "style" => Error::style(
                "Color.alpha()",
                "alpha is 2; alpha must be between 0.0 and 1.0.",
            ),
            _ => Error::usage("Figure.add_axes()", "the figure already has axes."),
        }
    }

    #[test]
    fn display_is_context_then_message() {
        let err = Error::Data {
            context: "Axes.line()",
            message: "x has 3 values but y has 2; x and y must have the same length.".into(),
            parameter: Some("y"),
            hint: Some("pass equally long x and y."),
        };
        assert_eq!(
            err.to_string(),
            "Axes.line(): x has 3 values but y has 2; x and y must have the same length."
        );
    }

    #[test]
    fn constructors_start_without_parameter_or_hint() {
        for kind in ["data", "style", "usage"] {
            let err = sample(kind);
            assert_eq!(err.parameter(), None, "{kind}");
            assert_eq!(err.hint(), None, "{kind}");
        }
    }

    #[test]
    fn every_kind_keeps_its_parameter_and_hint_through_question_mark_and_display() {
        fn fail(kind: &str) -> Result<()> {
            Err(sample(kind).with_parameter("y").with_hint("use NaN."))?;
            Ok(())
        }
        for (kind, display) in [
            ("data", "Axes.line(): y[0] is inf; values must be finite."),
            (
                "style",
                "Color.alpha(): alpha is 2; alpha must be between 0.0 and 1.0.",
            ),
            ("usage", "Figure.add_axes(): the figure already has axes."),
        ] {
            let err = fail(kind).unwrap_err();
            assert_eq!(err.parameter(), Some("y"), "{kind}");
            assert_eq!(err.hint(), Some("use NaN."), "{kind}");
            assert_eq!(err.to_string(), display, "{kind}");
        }
    }

    #[test]
    fn with_context_keeps_parameter_and_hint() {
        for kind in ["data", "style", "usage"] {
            let err = sample(kind)
                .with_parameter("y")
                .with_hint("use NaN.")
                .with_context("Figure.save_png()");
            assert!(err.to_string().starts_with("Figure.save_png(): "), "{kind}");
            assert_eq!(err.parameter(), Some("y"), "{kind}");
            assert_eq!(err.hint(), Some("use NaN."), "{kind}");
        }
        let io = Error::from(std::io::Error::other("full")).with_context("Figure.save_png()");
        assert!(matches!(io, Error::Io(_)));
        assert_eq!(io.to_string(), "full");
    }

    #[test]
    fn io_errors_convert_and_have_no_parameter_or_hint() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: Error = io.into();
        assert!(matches!(err, Error::Io(_)));
        let err = err.with_parameter("path").with_hint("check the path.");
        assert_eq!(err.parameter(), None);
        assert_eq!(err.hint(), None);
    }
}
