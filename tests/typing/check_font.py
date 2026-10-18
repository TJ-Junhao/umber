"""Static typing expectations for ``Font``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

from pathlib import Path
from typing import assert_type

import umber as ub

# Positive: the bundled constants are fonts, and the theme font setters take them.
assert_type(ub.Font.INTER, ub.Font)
assert_type(ub.Theme.PIGMENT.with_title_font(ub.Font.INTER), ub.Theme)
assert_type(ub.Theme.MIDNIGHT.with_body_font(ub.Font.JETBRAINS_MONO), ub.Theme)
same: bool = ub.Font.GEIST == ub.Font.GEIST
assert_type(ub.Font.from_file("regular.ttf"), ub.Font)
assert_type(ub.Font.from_file(Path("regular.ttf"), bold=Path("bold.otf")), ub.Font)
assert_type(ub.Font.from_file("regular.ttf", bold=ub.AUTO), ub.Font)
assert_type(ub.Font.system("Inter"), ub.Font)
assert_type(ub.Font.from_file("regular.ttf", bold_fallback=True), ub.Font)
assert_type(ub.Font.from_file("regular.ttf", bold="bold.ttf", bold_fallback=False), ub.Font)
assert_type(ub.Font.system("Inter", bold_fallback=True), ub.Font)
assert_type(ub.Font.system("Inter", bold_fallback=False), ub.Font)

ub.Font.from_file("regular.ttf", bold_fallback=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Font.from_file("regular.ttf", bold_fallback=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Font.system("Inter", bold_fallback="yes")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Font.system("Inter", bold_fallback=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Font.system("Inter", True)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]

ub.Font.from_file(None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Font.from_file(b"regular.ttf")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Font.from_file("regular.ttf", bold=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Font.from_file("regular.ttf", "bold.ttf")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ub.Font.system(3)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: a family name, an index or another enum is not a font.
ub.Theme.PIGMENT.with_title_font("Inter")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_body_font(3)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_title_font(ub.Dash.SOLID)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: ``Font`` is not an enum, so it cannot be iterated or indexed by name.
list(ub.Font)  # type: ignore[call-overload]  # pyright: ignore[reportArgumentType]
ub.Font["INTER"]  # type: ignore[misc]  # pyright: ignore[reportInvalidTypeArguments]
