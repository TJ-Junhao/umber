"""Fonts: every bundled font reaches the renderer, and fonts are immutable values."""

import copy
import pickle
from typing import cast

import pytest

import umber as ub
from umber._errors import details

FONTS = [
    ub.Font.SOURCE_SERIF_4,
    ub.Font.IBM_PLEX_SANS,
    ub.Font.GEIST,
    ub.Font.INTER,
    ub.Font.JETBRAINS_MONO,
]


def _title_svg(font: ub.Font) -> str:
    theme = ub.Theme.PIGMENT.with_title_font(font)
    return ub.line([0, 1], [0, 1], theme=theme).title("Quarterly revenue").figure.to_svg()


def test_each_font_draws_a_distinct_title() -> None:
    outputs = {font: _title_svg(font) for font in FONTS}
    assert len(set(outputs.values())) == 5


def test_font_output_is_outlines_not_text() -> None:
    for font in FONTS:
        svg = _title_svg(font)
        assert "<text" not in svg
        assert "Quarterly revenue" not in svg


def test_a_font_is_deterministic() -> None:
    assert _title_svg(ub.Font.GEIST) == _title_svg(ub.Font.GEIST)


def test_fonts_are_immutable_values() -> None:
    assert ub.Font.INTER == ub.Font.INTER
    assert ub.Font.INTER != ub.Font.GEIST
    assert ub.Font.INTER != cast(object, "Inter")
    assert len({hash(font) for font in FONTS}) == len(FONTS)
    with pytest.raises(AttributeError):
        ub.Font.INTER._native = (0, 2)  # pyright: ignore[reportPrivateUsage]
    with pytest.raises(AttributeError):
        del ub.Font.INTER._native  # pyright: ignore[reportPrivateUsage]


@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_fonts_copy_and_pickle_to_the_same_constant(protocol: int) -> None:
    for font in FONTS:
        assert copy.copy(font) is font
        assert copy.deepcopy(font) is font
        assert pickle.loads(pickle.dumps(font, protocol=protocol)) is font


def test_font_construction_is_refused_with_a_hint() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Font()
    assert str(caught.value) == "Font(): Font has no public constructor; use a bundled font."
    assert details(caught.value) == (
        None,
        "use Font.SOURCE_SERIF_4, Font.IBM_PLEX_SANS, Font.GEIST, Font.INTER or "
        "Font.JETBRAINS_MONO.",
    )
