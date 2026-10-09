"""Static typing expectations for the exception hierarchy.

Checked by pyright and mypy in strict mode. Lines marked ``type: ignore`` MUST produce an
error; if they stop erroring, ``reportUnnecessaryTypeIgnoreComment`` / ``warn_unused_ignores``
fail the check.
"""

import umber as ub

as_base: ub.UmberError = ub.DataError("ok")
as_value_error: ValueError = ub.StyleError("ok")

usage_is_not_value_error: ValueError = ub.UsageError("x")  # type: ignore[assignment]
