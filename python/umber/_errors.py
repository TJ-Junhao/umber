"""Exception hierarchy raised by umber, and the details attached to each raised error.

Every exception umber raises on purpose carries two private attributes: the name of the
parameter the error is about, and a hint that suggests a fix. Either may be ``None``. Their
presence marks the exception as raised by umber; the notes shown under the message are built
from them (see ``umber._report``).
"""

_PARAMETER = "_umber_parameter"
_HINT = "_umber_hint"
_MESSAGE = "_umber_message"
_ROW = "_umber_row"
_UNSET = object()


def new_error[E: BaseException](
    cls: type[E], context: str, message: str, *, parameter: str | None, hint: str | None
) -> E:
    """Build an exception of ``cls`` with the message ``"<context>: <message>"`` and its details.

    Args:
        cls: The exception class; an umber error class or a built-in such as ``TypeError``.
        context: The API call that reports the error, such as ``"Axes.line()"``.
        message: What is wrong and what is required, such as ``"y[3] is inf; values must be
            finite."``.
        parameter: The name of the parameter the error is about, or ``None``.
        hint: A suggested fix, or ``None``.

    Returns:
        The exception, ready to be raised.
    """
    err = cls(f"{context}: {message}")
    setattr(err, _PARAMETER, parameter)
    setattr(err, _HINT, hint)
    setattr(err, _MESSAGE, message)
    return err


def at_row[E: BaseException](err: E, index: int) -> E:
    """Attach the original input row to a conversion error.

    Args:
        err: The error raised while converting an input element.
        index: The element's zero-based index in the original input column.

    Returns:
        The same error, with its input row attached.
    """
    setattr(err, _ROW, index)
    return err


def details(err: BaseException) -> tuple[str | None, str | None] | None:
    """Return the ``(parameter, hint)`` of an exception umber raised, or ``None`` for any other.

    Args:
        err: Any exception.

    Returns:
        The parameter name and hint, either of which may be ``None``; or ``None`` when the
        exception carries no umber details.
    """
    parameter: object = getattr(err, _PARAMETER, _UNSET)
    hint: object = getattr(err, _HINT, None)
    if parameter is _UNSET:
        return None
    if not (parameter is None or isinstance(parameter, str)):
        return None
    if not (hint is None or isinstance(hint, str)):
        return None
    return parameter, hint


class UmberError(Exception):
    """Base class of every error raised by umber.

    Catch this to handle any umber failure in one place. Wrong argument *types* are
    reported with the built-in ``TypeError`` and file system failures with ``OSError``;
    neither is wrapped in ``UmberError``.

    Example:
        >>> import umber as ub
        >>> try:
        ...     fig = ub.Figure()
        ...     ax = fig.add_axes()
        ...     fig.add_axes()
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

    For example, adding axes to an occupied grid cell or retrieving axes with different
    scales raises this error. It is deliberately not a ``ValueError``: the arguments can
    be valid on their own but incompatible with the figure's state.

    Example:
        >>> import umber as ub
        >>> issubclass(ub.UsageError, ValueError)
        False
    """
