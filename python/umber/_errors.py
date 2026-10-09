"""Exception hierarchy raised by umber."""


class UmberError(Exception):
    """Base class of every error raised by umber.

    Catch this to handle any umber failure in one place. Wrong argument *types* are
    reported with the built-in ``TypeError`` and file system failures with ``OSError``;
    neither is wrapped in ``UmberError``.

    Example:
        >>> import umber as ub
        >>> try:
        ...     raise ub.UsageError("Figure.add_axes(): this figure already has an axes; "
        ...                         "umber 0.1 supports one axes per figure.")
        ... except ub.UmberError as err:
        ...     print(type(err).__name__)
        UsageError
    """


class DataError(UmberError, ValueError):
    """The data passed to a call cannot be plotted.

    Raised at the call site when arrays differ in length, are not one-dimensional, contain
    infinities, have an unsupported dtype, or hold non-positive values on a log axis. It is
    also a ``ValueError``, so existing ``except ValueError`` blocks keep working.

    Example:
        >>> import umber as ub
        >>> try:
        ...     raise ub.DataError("Axes.line(): x has 3 values but y has 2; "
        ...                        "x and y must have the same length.")
        ... except ValueError as err:
        ...     print(err)
        Axes.line(): x has 3 values but y has 2; x and y must have the same length.
    """


class StyleError(UmberError, ValueError):
    """A non-data argument has an invalid value.

    Covers colors, alpha values, sizes, bin counts, log bases, axis limits and theme
    palettes. It is also a ``ValueError``.

    Example:
        >>> import umber as ub
        >>> issubclass(ub.StyleError, ValueError)
        True
    """


class UsageError(UmberError):
    """The API was called in a way umber does not support.

    For example, umber 0.1 allows one axes per figure, so a second ``Figure.add_axes()``
    raises this error. It is deliberately not a ``ValueError``: the arguments are fine,
    the sequence of calls is not.

    Example:
        >>> import umber as ub
        >>> issubclass(ub.UsageError, ValueError)
        False
    """
