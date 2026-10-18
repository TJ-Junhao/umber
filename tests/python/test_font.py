"""File and installed fonts: validation, identity, weights and rendering."""

import copy
import pickle
from pathlib import Path
from typing import cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details

ROOT = Path(__file__).resolve().parents[2]
BUNDLED = ROOT / "crates/umber-core/fonts"
REGULAR = BUNDLED / "Inter-Regular.ttf"
BOLD = BUNDLED / "Inter-SemiBold.ttf"
FONTS = ROOT / "tests/fonts"
MISSING_BOLD_HINT = (
    "pass bold= to Font.from_file, choose a family with a bold face, or pass bold_fallback=True."
)


def _figure(font: ub.Font, *, title: str = "Revenue 2026") -> ub.Figure:
    theme = ub.Theme.PIGMENT.with_title_font(font).with_body_font(font)
    return ub.line([0, 1, 2], [-1, 3, 2], theme=theme).title(title).figure


def test_from_file_regular_and_bold(tmp_path: Path) -> None:
    font = ub.Font.from_file(REGULAR, bold=str(BOLD))
    assert isinstance(font, ub.Font)
    assert font != ub.Font.INTER
    renamed = tmp_path / "renamed.ttf"
    renamed.write_bytes(REGULAR.read_bytes())
    equal = ub.Font.from_file(str(renamed), bold=BOLD)
    assert font == equal
    assert hash(font) == hash(equal)
    assert len({font, equal}) == 1
    assert font != ub.Font.from_file(REGULAR)
    assert font != ub.Font.from_file(BOLD, bold=REGULAR)
    assert copy.copy(font) is font
    assert copy.deepcopy(font) is font
    assert pickle.loads(pickle.dumps(font)) == font
    assert "Font.from_file(" in repr(font)
    assert _figure(font).to_svg() == _figure(equal).to_svg()
    assert _figure(font).to_png() == _figure(equal).to_png()
    assert _figure(font).to_svg() == _figure(ub.Font.INTER).to_svg()
    assert _figure(font).to_png() == _figure(ub.Font.INTER).to_png()
    svg = _figure(font).to_svg(text=ub.SvgText.EDITABLE)
    assert "Inter" in svg and "Revenue 2026" in svg
    renamed.unlink()
    assert _figure(equal).to_png().startswith(b"\x89PNG")


@pytest.mark.parametrize("parameter", ["regular", "bold"])
def test_from_file_errors_exact(tmp_path: Path, parameter: str) -> None:
    missing = tmp_path / "missing.ttf"
    with pytest.raises(ub.UsageError) as caught:
        if parameter == "regular":
            ub.Font.from_file(missing)
        else:
            ub.Font.from_file(REGULAR, bold=missing)
    assert str(caught.value) == (
        f"Font.from_file(): {parameter} is {str(missing)!r}, which cannot be read; "
        "it must be a readable font file."
    )
    assert details(caught.value) == (parameter, "check the path.")
    assert any("^" in note for note in getattr(caught.value, "__notes__", []))
    for data in (b"not a font", REGULAR.read_bytes()[:100], b""):
        broken = tmp_path / "broken.ttf"
        broken.write_bytes(data)
        with pytest.raises(ub.DataError) as caught_data:
            if parameter == "regular":
                ub.Font.from_file(broken)
            else:
                ub.Font.from_file(REGULAR, bold=broken)
        assert str(caught_data.value) == (
            f"Font.from_file(): {parameter} is {str(broken)!r}, which is not a valid font; "
            "it must be a TrueType or OpenType font."
        )
        assert details(caught_data.value) == (parameter, "pass a .ttf or .otf file.")


def test_unreadable_and_invalid_paths(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    with pytest.raises(ub.UsageError):
        ub.Font.from_file(tmp_path)
    with pytest.raises(ub.UsageError):
        ub.Font.from_file("bad\x00.ttf")

    def denied(self: Path) -> bytes:
        raise PermissionError("denied")

    monkeypatch.setattr(Path, "read_bytes", denied)
    with pytest.raises(ub.UsageError) as caught:
        ub.Font.from_file(REGULAR)
    assert details(caught.value) == ("regular", "check the path.")


@pytest.mark.parametrize("value", [None, 12, b"font.ttf", ub.AUTO])
def test_from_file_rejects_regular_types(value: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Font.from_file(cast(str, value))
    detail = details(caught.value)
    assert detail is not None and detail[0] == "regular"


@pytest.mark.parametrize("value", [None, 12, b"font.ttf"])
def test_from_file_checks_bold_type_before_io(value: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Font.from_file("missing.ttf", bold=cast(str, value))
    detail = details(caught.value)
    assert detail is not None and detail[0] == "bold"


@pytest.mark.parametrize("parameter", ["regular", "bold"])
def test_ttc_rejected(tmp_path: Path, parameter: str) -> None:
    collection = tmp_path / "collection.ttf"
    collection.write_bytes(b"ttcf" + b"\x00" * 32)
    with pytest.raises(ub.DataError) as caught:
        if parameter == "regular":
            ub.Font.from_file(collection)
        else:
            ub.Font.from_file(REGULAR, bold=collection)
    assert details(caught.value) == (
        parameter,
        "extract one face from the collection, or use Font.system().",
    )


def _installed() -> tuple[str, ub.Font]:
    for family in ("DejaVu Sans", "Liberation Sans", "Arial", "Times New Roman"):
        try:
            return family, ub.Font.system(family)
        except ub.UsageError:
            continue
    pytest.skip("no known installed family")


def test_system_family_found_or_skipped() -> None:
    family, font = _installed()
    equal = ub.Font.system(family.lower())
    assert font == equal
    assert hash(font) == hash(equal)
    assert pickle.loads(pickle.dumps(font)) == font
    assert repr(font).startswith("Font.system(")
    assert _figure(font).to_png().startswith(b"\x89PNG")
    assert family in _figure(font).to_svg(text=ub.SvgText.EDITABLE)


@pytest.mark.parametrize("bold_fallback", [False, True])
def test_system_pickle_looks_up_the_family_again(
    monkeypatch: pytest.MonkeyPatch, bold_fallback: bool
) -> None:
    family, _ = _installed()
    font = ub.Font.system(family, bold_fallback=bold_fallback)
    saved = pickle.dumps(font)
    queries: list[tuple[str, bool]] = []

    def lookup(name: str, *, bold_fallback: bool = False) -> ub.Font:
        queries.append((name, bold_fallback))
        return ub.Font.INTER

    monkeypatch.setattr(ub.Font, "system", lookup)
    assert pickle.loads(saved) is ub.Font.INTER
    assert queries == [(family, bold_fallback)]


def test_system_unknown_lists_closest() -> None:
    family, _ = _installed()
    unknown = family[:-1] + "zzzz"
    with pytest.raises(ub.UsageError) as caught:
        ub.Font.system(unknown)
    assert str(caught.value).startswith(
        f"Font.system(): no installed family is named {unknown!r}; "
        "it must name an installed font family. Closest: "
    )
    assert family in str(caught.value)
    assert details(caught.value) == ("family", "check the installed family names.")
    assert any("^" in note for note in getattr(caught.value, "__notes__", []))


def test_system_requires_string() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Font.system(cast(str, None))
    detail = details(caught.value)
    assert detail is not None and detail[0] == "family"


@pytest.mark.parametrize("output", ["png", "svg"])
def test_missing_bold_fails_when_bold_text_is_drawn(output: str) -> None:
    font = ub.Font.from_file(REGULAR)
    figure = _figure(font, title="Needs bold")
    with pytest.raises(ub.StyleError) as caught:
        if output == "png":
            figure.to_png()
        else:
            figure.to_svg()
    assert "Needs bold" in str(caught.value)
    assert "Inter" in str(caught.value)
    assert details(caught.value) == (None, MISSING_BOLD_HINT)
    plain = ub.line([0, 1], [0, 1], theme=ub.Theme.PIGMENT.with_body_font(font)).figure
    assert plain.to_png().startswith(b"\x89PNG")
    assert "<svg" in plain.to_svg()
    direct = ub.line([0, 1], [0, 1], label="Series", theme=ub.Theme.PIGMENT.with_body_font(font))
    direct.legend(ub.LegendStyle.DIRECT)
    with pytest.raises(ub.StyleError, match="Series"):
        direct.figure.to_svg()
    top = ub.Figure(theme=ub.Theme.PIGMENT.with_title_font(font)).title("Figure heading")
    with pytest.raises(ub.StyleError, match="Figure heading"):
        top.to_svg()


@pytest.mark.parametrize("filename", ["SourceSerif4-Regular.otf", "SourceSerif4Variable-Roman.ttf"])
def test_font_file_edge_cases_formats(filename: str) -> None:
    path = FONTS / filename
    font = ub.Font.from_file(path, bold=path)
    figure = _figure(font)
    svg, png = figure.to_svg(), figure.to_png()
    assert svg == figure.to_svg() and png == figure.to_png()
    assert "<path" in svg and png.startswith(b"\x89PNG")
    assert svg != _figure(ub.Font.INTER).to_svg()
    assert "Source Serif" in figure.to_svg(text=ub.SvgText.EDITABLE)


def test_font_file_edge_cases_paths_and_missing_glyphs(tmp_path: Path) -> None:
    unicode_path = tmp_path / "字体-é.ttf"
    unicode_path.write_bytes(REGULAR.read_bytes())
    link = tmp_path / "linked.otf"
    link.symlink_to(unicode_path)
    original = ub.Font.from_file(REGULAR, bold=REGULAR)
    assert ub.Font.from_file(unicode_path, bold=link) == original
    assert _figure(original).to_png() == _figure(ub.Font.from_file(link, bold=link)).to_png()
    sparse = ROOT / "crates/umber-core/tests/fixtures/fallback-only.ttf"
    fallback = _figure(ub.Font.from_file(sparse, bold=sparse), title="123 −")
    assert fallback.to_png().startswith(b"\x89PNG")
    svg = fallback.to_svg()
    assert "<path" in svg
    assert svg == fallback.to_svg()


def test_font_fixtures_are_excluded_from_distributions(tmp_path: Path) -> None:
    import subprocess
    import tarfile

    subprocess.run(
        ["maturin", "sdist", "--out", str(tmp_path)], cwd=ROOT, check=True, capture_output=True
    )
    [sdist] = tmp_path.glob("*.tar.gz")
    with tarfile.open(sdist) as archive:
        names = archive.getnames()
    assert any(name.endswith("/python/umber/_font.py") for name in names)
    assert not any("/tests/fonts/" in name for name in names)


def test_font_file_without_name_table_uses_stem_in_identity(tmp_path: Path) -> None:
    data = bytearray(REGULAR.read_bytes())
    for offset in range(12, 12 + 16 * int.from_bytes(data[4:6], "big"), 16):
        if bytes(data[offset : offset + 4]) == b"name":
            data[offset : offset + 4] = b"namf"
            break
    else:
        pytest.fail("fixture has no name table")
    first, second = tmp_path / "First.ttf", tmp_path / "Second.ttf"
    first.write_bytes(data)
    second.write_bytes(data)
    a, b = ub.Font.from_file(first, bold=first), ub.Font.from_file(second, bold=second)
    assert a != b
    assert hash(a) != hash(b)
    assert a == ub.Font.from_file(first, bold=first)
    assert "First" in _figure(a).to_svg(text=ub.SvgText.EDITABLE)
    assert "Second" in _figure(b).to_svg(text=ub.SvgText.EDITABLE)


@pytest.mark.parametrize("with_bold", [False, True])
def test_file_pickle_preserves_paths_without_reopening(tmp_path: Path, with_bold: bool) -> None:
    regular, bold = tmp_path / "字体-Regular.ttf", tmp_path / "字体-Bold.ttf"
    regular.write_bytes(REGULAR.read_bytes())
    bold.write_bytes(BOLD.read_bytes())
    font = ub.Font.from_file(regular, bold=bold if with_bold else ub.AUTO)
    saved = pickle.dumps(font)
    regular.unlink()
    bold.unlink()
    restored = pickle.loads(saved)
    assert restored == font
    assert repr(restored) == repr(font)
    assert str(regular) in repr(restored)


def test_file_bytes_live_only_in_native_value(monkeypatch: pytest.MonkeyPatch) -> None:
    reads: list[Path] = []
    original = Path.read_bytes

    def read(path: Path) -> bytes:
        reads.append(path)
        return original(path)

    monkeypatch.setattr(Path, "read_bytes", read)
    font = ub.Font.from_file(REGULAR, bold=REGULAR)
    assert reads == [REGULAR]
    assert not hasattr(font, "_source")
    source = font.__reduce__()[1][0]
    assert isinstance(source, tuple)
    assert source[0] is source[1]
    restored = pickle.loads(pickle.dumps(font))
    assert reads == [REGULAR]
    assert restored == font


@pytest.mark.parametrize("constructor", ["from_file", "system"])
@pytest.mark.parametrize("value", [None, 0, 1, 1.0, "True", ub.AUTO, np.bool_(True)])
def test_bold_fallback_type_checked_before_io(
    monkeypatch: pytest.MonkeyPatch, constructor: str, value: object
) -> None:
    from umber import _umber

    def unexpected(*args: object) -> None:
        pytest.fail("type validation must precede file I/O and system lookup")

    monkeypatch.setattr(Path, "read_bytes", unexpected)
    monkeypatch.setattr(_umber, "font_system", unexpected)
    with pytest.raises(TypeError) as caught:
        if constructor == "from_file":
            ub.Font.from_file("missing.ttf", bold_fallback=cast(bool, value))
        else:
            ub.Font.system("Missing family", bold_fallback=cast(bool, value))
    detail = details(caught.value)
    assert detail is not None and detail[0] == "bold_fallback"
    assert any("^" in note for note in getattr(caught.value, "__notes__", []))


@pytest.mark.parametrize(
    ("regular", "bold"),
    [(Path("missing.ttf"), Path("missing-bold.ttf")), (REGULAR, BOLD), (REGULAR, REGULAR)],
)
def test_explicit_bold_with_fallback_is_a_usage_error_before_io(
    monkeypatch: pytest.MonkeyPatch, regular: Path, bold: Path
) -> None:
    def unexpected(*args: object) -> bytes:
        pytest.fail("bold option validation must precede file I/O")

    monkeypatch.setattr(Path, "read_bytes", unexpected)
    with pytest.raises(ub.UsageError) as caught:
        ub.Font.from_file(regular, bold=bold, bold_fallback=True)
    assert str(caught.value) == (
        "Font.from_file(): bold_fallback is True with bold given; "
        "the fallback applies only to a font without a bold face."
    )
    assert details(caught.value) == ("bold_fallback", "drop bold_fallback, or drop bold.")
    assert any("^" in note for note in getattr(caught.value, "__notes__", []))


def _bold_role(font: ub.Font, role: str) -> ub.Figure:
    theme = ub.Theme.PIGMENT.with_title_font(font).with_body_font(font)
    axes = ub.line([0, 1, 2], [1, 3, 2], theme=theme, label="Series")
    axes.legend(ub.LegendStyle.DIRECT if role == "direct" else ub.LegendStyle.NONE)
    if role == "axes":
        axes.title("Heading")
    elif role == "figure":
        axes.figure.title("Heading")
    return axes.figure


@pytest.mark.parametrize("role", ["axes", "figure", "direct"])
@pytest.mark.parametrize("output", ["png", "svg", "editable"])
@pytest.mark.parametrize("bold_fallback", [False, True])
def test_bold_roles_use_regular_only_when_fallback_is_enabled(
    role: str, output: str, bold_fallback: bool
) -> None:
    def render(figure: ub.Figure) -> bytes | str:
        if output == "png":
            return figure.to_png()
        return figure.to_svg(
            text=ub.SvgText.EDITABLE if output == "editable" else ub.SvgText.OUTLINES
        )

    figure = _bold_role(ub.Font.from_file(REGULAR, bold_fallback=bold_fallback), role)
    if not bold_fallback:
        with pytest.raises(ub.StyleError) as caught:
            render(figure)
        assert ("Series" if role == "direct" else "Heading") in str(caught.value)
        assert details(caught.value) == (None, MISSING_BOLD_HINT)
        return
    actual = render(figure)
    if output == "editable":
        assert isinstance(actual, str)
        assert 'font-weight="600"' not in actual
        assert 'font-weight="400"' in actual
    else:
        # The same role and size, with regular outlines explicitly supplied as the bold face.
        expected = _bold_role(ub.Font.from_file(REGULAR, bold=REGULAR), role)
        assert actual == render(expected)


def test_file_bold_fallback_is_part_of_value_and_pickle(tmp_path: Path) -> None:
    path = tmp_path / "字体-Regular.ttf"
    path.write_bytes(REGULAR.read_bytes())
    plain = ub.Font.from_file(path)
    font = ub.Font.from_file(path, bold_fallback=True)
    equal = ub.Font.from_file(REGULAR, bold=ub.AUTO, bold_fallback=True)
    assert plain == ub.Font.from_file(path, bold_fallback=False)
    assert font != plain and hash(font) != hash(plain)
    assert font == equal and hash(font) == hash(equal)
    assert len({plain, font, equal}) == 2
    assert repr(plain) == f"Font.from_file({str(path)!r})"
    assert repr(font) == f"Font.from_file({str(path)!r}, bold_fallback=True)"
    assert copy.copy(font) is font and copy.deepcopy(font) is font
    saved = pickle.dumps(font)
    path.unlink()
    restored = pickle.loads(saved)
    assert restored == font and hash(restored) == hash(font)
    assert repr(restored) == repr(font)
    assert _figure(restored).to_png() == _figure(font).to_png()


def test_system_bold_fallback_keeps_existing_bold_and_value_identity() -> None:
    family, plain = _installed()
    font = ub.Font.system(family, bold_fallback=True)
    equal = ub.Font.system(family.lower(), bold_fallback=True)
    assert plain == ub.Font.system(family, bold_fallback=False)
    assert font != plain and hash(font) != hash(plain)
    assert font == equal and hash(font) == hash(equal)
    assert "bold_fallback" not in repr(plain)
    assert repr(font) == f"Font.system({family!r}, bold_fallback=True)"
    restored = pickle.loads(pickle.dumps(font))
    assert restored == font and hash(restored) == hash(font)
    assert repr(restored) == repr(font)
    for role in ("axes", "figure", "direct"):
        expected, actual = _bold_role(plain, role), _bold_role(font, role)
        assert actual.to_png() == expected.to_png()
        assert actual.to_svg() == expected.to_svg()
        assert actual.to_svg(text=ub.SvgText.EDITABLE) == expected.to_svg(text=ub.SvgText.EDITABLE)


def test_system_empty_family_has_specific_error() -> None:
    with pytest.raises(ub.UsageError) as caught:
        ub.Font.system("")
    assert str(caught.value) == (
        "Font.system(): the family name is empty; it must name an installed font family."
    )
    assert details(caught.value) == ("family", "check the installed family names.")


def test_font_constructor_and_invalid_value_hint_include_custom_fonts() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Font()
    assert "Font.from_file()" in str(caught.value)
    assert "Font.system()" in str(caught.value)
    detail = details(caught.value)
    assert detail is not None and detail[1] is not None
    assert "Font.from_file()" in detail[1] and "Font.system()" in detail[1]
    with pytest.raises(TypeError) as invalid:
        ub.Theme.PIGMENT.with_body_font(cast(ub.Font, "Inter"))
    assert details(invalid.value) == ("value", detail[1])
