"""Static typing expectations for the type parameter defaults of ``Axes``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
``Figure.add_axes()`` has one signature: ``X`` and ``Y`` default to ``float``, so the call
without arguments is ``Axes[float, float]`` under both checkers.
"""

from typing import assert_type

import umber as ub

# Positive: the call without a scale, or with a numeric one, is numeric on both axes.
assert_type(ub.Figure().add_axes(), ub.Axes[float, float])
assert_type(ub.Figure().add_axes(x=ub.Scale.linear()), ub.Axes[float, float])
assert_type(ub.Figure().add_axes(x=ub.Scale.log()), ub.Axes[float, float])
assert_type(ub.Figure().add_axes(y=ub.Scale.log()), ub.Axes[float, float])
assert_type(ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log()), ub.Axes[float, float])

# Positive: a category x scale selects ``str``.
assert_type(ub.Figure().add_axes(x=ub.Scale.category()), ub.Axes[str, float])
assert_type(ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.log()), ub.Axes[str, float])

# Positive: retrieving axes and the quick functions have exact types.
assert_type(ub.Figure().axes_at(), ub.Axes[float, float])
assert_type(ub.line([0.0], [1.0]), ub.Axes[float, float])
assert_type(ub.scatter([0.0], [1.0]), ub.Axes[float, float])
assert_type(ub.bar(["A"], [1.0]), ub.Axes[str, float])
assert_type(ub.hist([1.0]), ub.Axes[float, float])


# Positive: a bare ``Axes`` annotation means ``Axes[float, float]``.
def bare(axes: ub.Axes) -> None:
    assert_type(axes, ub.Axes[float, float])


# Negative: with an expected type, mypy solves the free ``X`` of ``add_axes()`` as ``str`` and
# accepts the line below; pyright rejects it. Only the pyright marker is therefore present.
wrong: ub.Axes[str, float] = ub.Figure().add_axes()  # pyright: ignore[reportAssignmentType]
