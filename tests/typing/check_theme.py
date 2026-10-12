"""Static typing expectations for ``Theme``, ``Color`` and ``Scale``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

from typing import assert_type

import umber as ub

# Positive: presets and the ``with_*`` chain return a ``Theme``.
theme = ub.Theme.PIGMENT.with_line_width(2).with_title_font(ub.Font.INTER)
assert_type(theme, ub.Theme)
assert_type(ub.Theme.GRAPHITE.with_palette([ub.Color.RED, ub.Color.BLUE]), ub.Theme)
assert_type(ub.Theme.MIDNIGHT.with_grid_lines(ub.GridLines.HORIZONTAL), ub.Theme)
assert_type(ub.Color.hex("#112233").alpha(0.5), ub.Color)
assert_type(ub.Scale.linear(), ub.Scale[float])
assert_type(ub.Scale.log(2), ub.Scale[float])
assert_type(ub.Scale.category(), ub.Scale[str])

# Negative: wrong value types.
ub.Theme.PIGMENT.with_line_width("2")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_title_font("Inter")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_background("#ffffff")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_palette(["#ffffff"])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
ub.Color.hex(0xFFFFFF)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: ``Scale`` is invariant, so a category scale is not a numeric one and the reverse.
numeric: ub.Scale[float] = ub.Scale.category()  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]
text: ub.Scale[str] = ub.Scale.linear()  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]

# Negative: ``Axes`` is invariant too.
cats = ub.Figure().add_axes(x=ub.Scale.category())
as_numeric: ub.Axes[float, float] = cats  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]
numeric_axes = ub.Figure().add_axes()
as_text: ub.Axes[str, float] = numeric_axes  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]

# With an expected type, mypy solves the free ``X`` of ``add_axes()`` as ``str``, so
# ``text_axes: ub.Axes[str, float] = ub.Figure().add_axes()`` passes mypy (pyright reports it).
# A stored value, as above, is checked by both.

# Negative: ``Axes`` and ``Theme`` have no public constructor arguments.
ub.Axes(1)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
