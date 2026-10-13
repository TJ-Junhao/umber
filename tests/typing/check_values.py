"""Static typing expectations for data arguments and wrong argument types.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl

import umber as ub

ax = ub.Figure().add_axes()

# Positive: every data container accepted for ``Values[float]``.
floats: npt.NDArray[np.float64] = np.array([1.0, 2.0, 3.0])
ints: npt.NDArray[np.int64] = np.array([1, 2, 3])
ax.line([1, 2, 3], [1.5, 2.5, 3.5])
ax.line((1, 2, 3), (1.5, 2.5, 3.5))
ax.line(range(3), [1.5, 2.5, 3.5])
ax.line(floats, ints)
ax.line(pd.Series([1.0, 2.0, 3.0]), pd.Series([1, 2, 3]))
ax.line(pl.Series([1.0, 2.0, 3.0]), pl.Series([1, 2, 3]))
ax.hist(floats)
ax.hist(pl.Series([1.0, 2.0]))
cats = ub.Figure().add_axes(x=ub.Scale.category())
cats.bar(["A", "B"], [3, 5])
cats.bar(("A", "B"), floats[:2])
cats.bar(np.array(["A", "B"]), pd.Series([3.0, 5.0]))
cats.bar(pl.Series(["A", "B"]), pl.Series([3.0, 5.0]))
ub.Series("North", floats)
ub.Series("North", pl.Series([1.0]))

# Negative: ``bar`` categories must be a sequence of ``str`` or an array-like.
cats.bar([1, 2], [3, 5])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]

# Negative: a heights argument that is neither a sequence nor array-like.
ax.line([1, 2], 3.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.hist(3.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# A plain ``str`` is not accepted as ``values``: ``str`` is a ``Sequence[str]``, not a
# ``Sequence[float]``, so both checkers reject it (and it fails at runtime with a ``TypeError``,
# see ``test_bars.py``).
ub.Series("a", "abc")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
