//! Error type shared by all of umber-core.

/// Input references retained separately from an error's display text.
///
/// # Examples
///
/// ```
/// let message: umber_core::ErrorMessage = "invalid data".into();
/// assert!(message.location().is_none());
/// ```
#[derive(Debug, Default)]
pub struct InputLocation {
    /// The column used by each position.
    pub name: String,
    /// Local row positions; `None` refers to the column as a whole.
    pub indices: Vec<Option<usize>>,
    /// Text before, between and after the input references.
    pub fragments: Vec<String>,
    /// The series position in a grouped call, if known.
    pub group: Option<usize>,
}

/// Error text with optional structured input positions for long-form data.
/// It displays and dereferences to the same text as an ordinary message.
///
/// # Examples
///
/// ```
/// let message: umber_core::ErrorMessage = "values must be finite.".into();
/// assert_eq!(message.to_string(), "values must be finite.");
/// ```
#[derive(Debug)]
pub struct ErrorMessage {
    text: String,
    location: Option<Box<InputLocation>>,
}

impl ErrorMessage {
    /// The input references recorded during validation, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// let message: umber_core::ErrorMessage = "invalid data".into();
    /// assert!(message.location().is_none());
    /// ```
    pub fn location(&self) -> Option<&InputLocation> {
        self.location.as_deref()
    }
}

impl From<String> for ErrorMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            location: None,
        }
    }
}

impl From<&str> for ErrorMessage {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}

impl std::fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.text.fmt(f)
    }
}

impl std::ops::Deref for ErrorMessage {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

impl<T: AsRef<str> + ?Sized> PartialEq<T> for ErrorMessage {
    fn eq(&self, other: &T) -> bool {
        self.text == other.as_ref()
    }
}

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
        message: ErrorMessage,
        /// The Python parameter the error is about, e.g. `"y"`; `None` when no single parameter
        /// is at fault. Nested data names its top-level parameter: `series[1].values[3]` is
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
        message: ErrorMessage,
        /// The Python parameter the error is about; `None` when no single parameter is at fault.
        parameter: Option<&'static str>,
        /// One sentence that suggests a likely fix; `None` when there is no likely fix.
        hint: Option<&'static str>,
    },
    /// The API was used in an unsupported way, e.g. adding axes to a cell that holds some.
    #[error("{context}: {message}")]
    Usage {
        /// The API call where the problem was detected.
        context: &'static str,
        /// What is wrong and what is required.
        message: ErrorMessage,
        /// The Python parameter the error is about; `None` when no single parameter is at fault.
        parameter: Option<&'static str>,
        /// One sentence that suggests a likely fix; `None` when there is no likely fix.
        hint: Option<&'static str>,
    },
    /// Reading or writing a file failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Prints `v` for an error message: plain decimal from `1e-4` up to (not including) `1e6` in
/// magnitude, scientific form (`1e301`, `5e-324`) outside it. Zero prints as `0` and negative
/// zero as `-0`, negatives with `-`, and `NaN`, `inf` and `-inf` as such.
pub(crate) fn number(v: f64) -> impl std::fmt::Display {
    struct Number(f64);
    impl std::fmt::Display for Number {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let v = self.0;
            let magnitude = v.abs();
            if v == 0.0 || !v.is_finite() || (1e-4..1e6).contains(&magnitude) {
                write!(f, "{v}")
            } else {
                write!(f, "{v:e}")
            }
        }
    }
    Number(v)
}

/// The most characters an error message shows of user text, quotes and escapes included; the
/// Python layer's `short_repr` uses the same limit.
const QUOTED_CHARS: usize = 40;

/// Quotes user `text` for an error message, as Rust string literal syntax (`"North"`, with
/// quotes, backslashes and control characters escaped). A quotation longer than
/// [`QUOTED_CHARS`] characters is cut to its first `QUOTED_CHARS - 1` characters followed by
/// `…`, so it has no closing quote, and may end inside an escape. This is how the Python
/// layer's `short_repr` cuts its representation.
pub(crate) fn quoted(text: &str) -> String {
    // Every character quotes to at least one, so the first `QUOTED_CHARS` characters decide
    // the first `QUOTED_CHARS - 1` of the quotation, and whether it is cut at all.
    let head: String = text.chars().take(QUOTED_CHARS).collect();
    let full = format!("{head:?}");
    if full.chars().count() <= QUOTED_CHARS {
        full
    } else {
        let mut cut: String = full.chars().take(QUOTED_CHARS - 1).collect();
        cut.push('…');
        cut
    }
}

impl Error {
    /// Builds a data error from a column's positions and the text around them.
    pub(crate) fn at_input(
        context: &'static str,
        name: &str,
        indices: Vec<Option<usize>>,
        fragments: Vec<String>,
    ) -> Error {
        let mut message = String::new();
        for (i, fragment) in fragments.iter().enumerate() {
            message.push_str(fragment);
            if let Some(index) = indices.get(i) {
                message.push_str(name);
                if let Some(index) = index {
                    use std::fmt::Write;
                    let _ = write!(message, "[{index}]");
                }
            }
        }
        Error::Data {
            context,
            message: ErrorMessage {
                text: message,
                location: Some(Box::new(InputLocation {
                    name: name.into(),
                    indices,
                    fragments,
                    group: None,
                })),
            },
            parameter: None,
            hint: None,
        }
    }

    /// Records which group's validation failed without changing core display text.
    pub(crate) fn in_group(mut self, index: usize) -> Error {
        if let Error::Data { message, .. }
        | Error::Style { message, .. }
        | Error::Usage { message, .. } = &mut self
        {
            message.location.get_or_insert_with(Default::default).group = Some(index);
        }
        self
    }

    /// An [`Error::Data`] detected in `context`, without a parameter or a hint.
    pub(crate) fn data(context: &'static str, message: impl Into<ErrorMessage>) -> Error {
        Error::Data {
            context,
            message: message.into(),
            parameter: None,
            hint: None,
        }
    }

    /// An [`Error::Style`] detected in `context`, without a parameter or a hint.
    pub(crate) fn style(context: &'static str, message: impl Into<ErrorMessage>) -> Error {
        Error::Style {
            context,
            message: message.into(),
            parameter: None,
            hint: None,
        }
    }

    /// An [`Error::Usage`] detected in `context`, without a parameter or a hint.
    pub(crate) fn usage(context: &'static str, message: impl Into<ErrorMessage>) -> Error {
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

    /// The error with `prefix` put before its message, such as the grid cell of the axes it is
    /// about. The context, parameter and hint are kept. [`Error::Io`] is returned unchanged.
    pub(crate) fn with_message_prefix(mut self, prefix: &str) -> Error {
        if let Error::Data { message, .. }
        | Error::Style { message, .. }
        | Error::Usage { message, .. } = &mut self
        {
            message.text.insert_str(0, prefix);
            if let Some(location) = &mut message.location
                && let Some(first) = location.fragments.first_mut()
            {
                first.insert_str(0, prefix);
            }
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
    /// assert_eq!(err.hint(), Some("each cell holds one axes; pick an empty cell."));
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
///         message: format!("{text:?} is not a number; use a value from 0.0 to 1.0.").into(),
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

    #[test]
    fn quoted_text_is_escaped_then_cut_to_forty_characters_as_in_python() {
        // (text, Python's `short_repr(text)`, `quoted(text)`). Python quotes with `'` unless
        // the text holds `'` but no `"`; otherwise the two escape alike, so the quotations are
        // equally long and cut at the same character.
        let rows: [(String, &str, &str); 18] = [
            ("North".into(), "'North'", "\"North\""),
            (String::new(), "''", "\"\""),
            (
                "W".repeat(38),
                "'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW'",
                "\"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW\"",
            ),
            (
                "W".repeat(39),
                "'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW…",
                "\"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW…",
            ),
            (
                "W".repeat(41),
                "'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW…",
                "\"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW…",
            ),
            (
                "W".repeat(100),
                "'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW…",
                "\"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW…",
            ),
            (
                "é".repeat(38),
                "'éééééééééééééééééééééééééééééééééééééé'",
                "\"éééééééééééééééééééééééééééééééééééééé\"",
            ),
            (
                "é".repeat(41),
                "'éééééééééééééééééééééééééééééééééééééé…",
                "\"éééééééééééééééééééééééééééééééééééééé…",
            ),
            (
                "日本".repeat(30),
                "'日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本…",
                "\"日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本日本…",
            ),
            (
                "\\".repeat(19),
                r"'\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\'",
                r#""\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\""#,
            ),
            (
                "\\".repeat(41),
                r"'\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\…",
                r#""\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\…"#,
            ),
            (
                "\n".repeat(19),
                r"'\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n'",
                r#""\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n""#,
            ),
            (
                "\n".repeat(41),
                r"'\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n…",
                r#""\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n…"#,
            ),
            ("a\tb".into(), r"'a\tb'", r#""a\tb""#),
            ("a'b".into(), r#""a'b""#, r#""a'b""#),
            // Python needs no escape here; the Rust quotation is one character longer.
            ("a\"b".into(), r#"'a"b'"#, r#""a\"b""#),
            (
                "\"'".repeat(25),
                r#"'"\'"\'"\'"\'"\'"\'"\'"\'"\'"\'"\'"\'"\…"#,
                r#""\"'\"'\"'\"'\"'\"'\"'\"'\"'\"'\"'\"'\"…"#,
            ),
            (
                format!("{}\u{7}", "W".repeat(30)),
                r"'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWW\x07'",
                r#""WWWWWWWWWWWWWWWWWWWWWWWWWWWWWW\u{7}""#,
            ),
        ];
        for (text, python, want) in &rows {
            let got = quoted(text);
            assert_eq!(&got, want, "{text:?}");
            assert!(got.chars().count() <= QUOTED_CHARS, "{got}");
            let python_escapes_alike = !text.contains('"') || text.contains('\'');
            let same_escapes = !text.contains('\u{7}');
            if python_escapes_alike && same_escapes {
                assert_eq!(
                    got.chars().count(),
                    python.chars().count(),
                    "{got} vs {python}"
                );
                assert_eq!(
                    got.ends_with('…'),
                    python.ends_with('…'),
                    "{got} vs {python}"
                );
            }
        }
    }

    #[test]
    fn number_prints_plain_decimals_in_range_and_scientific_outside() {
        let shown = |v: f64| number(v).to_string();
        assert_eq!(shown(1e-4), "0.0001");
        assert_eq!(shown(9.9e-5), "9.9e-5");
        assert_eq!(shown(999_999.0), "999999");
        assert_eq!(shown(1e6), "1e6");
        assert_eq!(shown(1e300), "1e300");
        assert_eq!(shown(-1e300), "-1e300");
        assert_eq!(shown(5e-324), "5e-324");
        assert_eq!(shown(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(shown(-2.5), "-2.5");
        assert_eq!(shown(0.0), "0");
        assert_eq!(shown(-0.0), "-0");
        assert_eq!(shown(f64::NAN), "NaN");
        assert_eq!(shown(f64::INFINITY), "inf");
        assert_eq!(shown(f64::NEG_INFINITY), "-inf");
    }
}
