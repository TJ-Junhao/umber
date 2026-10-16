"""Static typing expectations for ``Cmap``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

from typing import assert_type

import umber as ub

# Positive: the built-ins are ``Cmap`` values and every constructor returns one.
assert_type(ub.Cmap.VIRIDIS, ub.Cmap)
assert_type(ub.Cmap.MAGMA, ub.Cmap)
assert_type(ub.Cmap.CIVIDIS, ub.Cmap)
assert_type(ub.Cmap.BLUE_RED, ub.Cmap)
assert_type(ub.Cmap.from_colors([ub.Color.RED, ub.Color.BLUE]), ub.Cmap)
assert_type(ub.Cmap.from_colors((ub.Color.RED, ub.Color.BLUE)), ub.Cmap)
assert_type(ub.Cmap.VIRIDIS.reversed(), ub.Cmap)
assert_type(ub.Theme.PIGMENT.with_cmap(ub.Cmap.MAGMA), ub.Theme)
assert_type(ub.Theme.MIDNIGHT.with_diverging_cmap(ub.Cmap.BLUE_RED.reversed()), ub.Theme)
assert_type(ub.Cmap.VIRIDIS == ub.Cmap.MAGMA, bool)
assert_type(hash(ub.Cmap.VIRIDIS), int)

# Negative: wrong argument types.
ub.Cmap.from_colors(["#ffffff", "#000000"])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
ub.Cmap.from_colors(ub.Color.RED)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_cmap("viridis")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_diverging_cmap(ub.Color.RED)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
