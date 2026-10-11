"""Static typing expectations for the exception hierarchy.

Checked by pyright and mypy in strict mode.

Convention: pyright ignores ``# type: ignore`` comments (``enableTypeIgnoreComments`` is
off), so each checker reads only its own marker. A line that MUST produce an error carries
both ``# type: ignore[<mypy-code>]`` and ``# pyright: ignore[<rule>]``. If the line stops
erroring, ``warn_unused_ignores`` (mypy) or ``reportUnnecessaryTypeIgnoreComment`` (pyright)
fails the check. When only one checker reports an error, put just its marker on that line.
"""

import umber as ub

as_base: ub.UmberError = ub.DataError("ok")
as_value_error: ValueError = ub.StyleError("ok")

usage_is_not_value_error: ValueError = ub.UsageError("x")  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]
