"""Every bundled font reaches the renderer: each family draws its own glyph outlines."""

import umber as ub


def _title_svg(font: ub.Font) -> str:
    theme = ub.Theme.PIGMENT.with_title_font(font)
    return ub.line([0, 1], [0, 1], theme=theme).title("Quarterly revenue").figure.to_svg()


def test_each_font_draws_a_distinct_title() -> None:
    fonts = list(ub.Font)
    assert len(fonts) == 5
    outputs = {font: _title_svg(font) for font in fonts}
    assert len(set(outputs.values())) == 5


def test_font_output_is_outlines_not_text() -> None:
    for font in ub.Font:
        svg = _title_svg(font)
        assert "<text" not in svg
        assert "Quarterly revenue" not in svg


def test_a_font_is_deterministic() -> None:
    assert _title_svg(ub.Font.GEIST) == _title_svg(ub.Font.GEIST)
