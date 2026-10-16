# pyright: reportPrivateUsage=false
import copy
import pickle
import typing
from collections.abc import Sequence
from pathlib import Path
from typing import cast

import numpy as np
import pytest

import umber as ub
from umber import _umber
from umber._cmap import native
from umber._errors import details
from umber._theme import native as theme_native

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
COLORS_HINT = "pass a list of colors, e.g. [Color.RED, Color.BLUE]."
COUNT_HINT = "pass between 2 and 16 colors, e.g. [Color.BLUE, Color.RED]."
OPAQUE_HINT = "use opaque colors; set transparency on the mark instead."
CONCRETE_HINT = "use Color.hex(), Color.rgb() or a named color."
BUILTINS = ["VIRIDIS", "MAGMA", "CIVIDIS", "BLUE_RED"]
THREE = [ub.Color.hex("#112233"), ub.Color.hex("#445566"), ub.Color.hex("#778899")]


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _raised(tmp_path: Path, line: str) -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    path = tmp_path / f"user_code_{abs(hash(line))}.py"
    path.write_text(f"import umber as ub\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def _builtin(name: str) -> ub.Cmap:
    return cast(ub.Cmap, getattr(ub.Cmap, name))


@pytest.mark.parametrize("name", BUILTINS)
def test_builtins_wrap_the_native_colormaps(name: str) -> None:
    factory = getattr(_umber.Cmap, name.lower())
    assert native(_builtin(name)) == factory()
    assert isinstance(_builtin(name), ub.Cmap)


def test_builtins_are_distinct() -> None:
    maps = [_builtin(name) for name in BUILTINS]
    assert all(a != b for i, a in enumerate(maps) for b in maps[i + 1 :])


def test_builtin_tables_have_256_stops() -> None:
    for name in ("VIRIDIS", "MAGMA", "CIVIDIS"):
        assert len(native(_builtin(name)).colors()) == 256
    assert native(ub.Cmap.BLUE_RED).colors() == [0x2B5C8AFF, 0xF4F1ECFF, 0xB23A2EFF]
    assert native(ub.Cmap.VIRIDIS).colors()[0] == 0x440154FF


def test_construction_is_refused_with_a_hint() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Cmap()
    assert str(caught.value) == (
        "Cmap(): Cmap has no public constructor; use a built-in colormap or Cmap.from_colors()."
    )
    assert details(caught.value) == (
        None,
        "use Cmap.VIRIDIS, Cmap.BLUE_RED or Cmap.from_colors([Color.BLUE, Color.RED]).",
    )


def test_from_colors_builds_a_colormap_from_the_colors_in_order() -> None:
    cmap = ub.Cmap.from_colors(THREE)
    assert isinstance(cmap, ub.Cmap)
    assert native(cmap).colors() == [0x112233FF, 0x445566FF, 0x778899FF]
    assert cmap == ub.Cmap.from_colors(tuple(THREE))
    assert cmap != ub.Cmap.from_colors(THREE[::-1])
    assert cmap != ub.Cmap.from_colors(THREE[:2])


def test_from_colors_accepts_two_to_sixteen_colors() -> None:
    for count in (2, 16):
        assert len(native(ub.Cmap.from_colors([ub.Color.RED] * count)).colors()) == count


def test_from_colors_rejects_transparent_colors(tmp_path: Path) -> None:
    colors = [ub.Color.RED, ub.Color.BLUE, ub.Color.GREEN.alpha(0.5)]
    with pytest.raises(ub.StyleError) as caught:
        ub.Cmap.from_colors(colors)
    assert str(caught.value) == (
        "Cmap.from_colors(): colors[2] has alpha 0.5; colormap colors must be opaque."
    )
    assert details(caught.value) == ("colors", OPAQUE_HINT)
    line = "ub.Cmap.from_colors([ub.Color.RED, ub.Color.BLUE.alpha(0.5)])"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert _notes(err)[0] == _marked(line, "[ub.Color.RED, ub.Color.BLUE.alpha(0.5)]")


@pytest.mark.parametrize(("count", "noun"), [(0, "0 colors"), (1, "1 color"), (17, "17 colors")])
def test_from_colors_rejects_other_counts(count: int, noun: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Cmap.from_colors([ub.Color.RED] * count)
    assert str(caught.value) == (
        f"Cmap.from_colors(): colors has {noun}; a colormap needs 2 to 16 colors."
    )
    assert details(caught.value) == ("colors", COUNT_HINT)


def test_from_colors_count_error_highlights_the_colors(tmp_path: Path) -> None:
    line = "ub.Cmap.from_colors([ub.Color.RED])"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert _notes(err) == [_marked(line, "[ub.Color.RED]"), f"hint: {COUNT_HINT}"]


@pytest.mark.parametrize(
    ("colors", "message"),
    [
        ([ub.Color.RED, ub.Color.BLUE, ub.Color.AUTO], "colors[2] is Color.AUTO"),
        ([ub.Color.palette(4), ub.Color.RED], "colors[0] is Color.palette(4)"),
    ],
)
def test_from_colors_names_the_color_that_is_not_concrete(
    colors: list[ub.Color], message: str
) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Cmap.from_colors(colors)
    assert str(caught.value) == (
        f"Cmap.from_colors(): {message}; colormap colors must be concrete colors."
    )
    assert details(caught.value) == ("colors", CONCRETE_HINT)


@pytest.mark.parametrize("colors", ["red", b"red", 5, None, {ub.Color.RED}, np.array([1, 2])])
def test_from_colors_rejects_a_non_sequence(colors: object) -> None:
    name = "None" if colors is None else type(colors).__name__
    with pytest.raises(TypeError) as caught:
        ub.Cmap.from_colors(cast(Sequence[ub.Color], colors))
    assert str(caught.value) == (
        f"Cmap.from_colors(): colors is {name}; colors must be a sequence of Color."
    )
    assert details(caught.value) == ("colors", COLORS_HINT)


def test_from_colors_names_the_element_that_is_not_a_color() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Cmap.from_colors(cast(list[ub.Color], [ub.Color.RED, "blue", 3]))
    assert str(caught.value) == (
        "Cmap.from_colors(): colors[1] is str; colors[1] must be of type Color."
    )
    assert details(caught.value) == ("colors", COLOR_HINT)


def test_from_colors_type_errors_come_before_native_errors() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Cmap.from_colors(cast(list[ub.Color], [ub.Color.AUTO, 3]))
    assert "colors[1] is int" in str(caught.value)


def test_from_colors_type_error_highlights_the_colors(tmp_path: Path) -> None:
    line = "ub.Cmap.from_colors([ub.Color.RED, 7])"
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err) == [_marked(line, "[ub.Color.RED, 7]"), f"hint: {COLOR_HINT}"]


@pytest.mark.parametrize("name", BUILTINS)
def test_reversed_twice_is_the_same_colormap(name: str) -> None:
    cmap = _builtin(name)
    assert cmap.reversed() != cmap
    assert cmap.reversed().reversed() == cmap
    assert hash(cmap.reversed().reversed()) == hash(cmap)
    assert native(cmap.reversed()).colors() == native(cmap).colors()[::-1]


def test_reversed_of_a_custom_colormap_reads_backwards() -> None:
    assert ub.Cmap.from_colors(THREE).reversed() == ub.Cmap.from_colors(THREE[::-1])


def test_equal_colormaps_hash_alike() -> None:
    one = ub.Cmap.from_colors(THREE)
    two = ub.Cmap.from_colors(list(THREE))
    assert one == two and hash(one) == hash(two)
    assert len({one, two, ub.Cmap.VIRIDIS, ub.Cmap.VIRIDIS}) == 2
    assert {ub.Cmap.MAGMA: 1}[ub.Cmap.MAGMA] == 1
    assert ub.Cmap.VIRIDIS != "viridis"


@pytest.mark.parametrize("name", BUILTINS)
def test_repr_of_a_builtin_names_it(name: str) -> None:
    assert repr(_builtin(name)) == f"Cmap.{name}"
    assert repr(_builtin(name).reversed()) == f"Cmap.{name}.reversed()"
    assert repr(_builtin(name).reversed().reversed()) == f"Cmap.{name}"


def test_repr_of_a_custom_colormap_lists_its_colors() -> None:
    cmap = ub.Cmap.from_colors([ub.Color.RED, ub.Color.hex("#112233"), ub.Color.BLUE])
    assert repr(cmap) == 'Cmap.from_colors([Color.RED, Color.hex("#112233"), Color.BLUE])'
    assert eval(repr(cmap), {"Cmap": ub.Cmap, "Color": ub.Color}) == cmap
    assert repr(cmap.reversed()) == (
        'Cmap.from_colors([Color.BLUE, Color.hex("#112233"), Color.RED])'
    )


def test_colormaps_are_immutable() -> None:
    cmap = ub.Cmap.VIRIDIS
    with pytest.raises(AttributeError):
        cmap._native = _umber.Cmap.magma()
    with pytest.raises(AttributeError):
        cmap.extra = 1
    with pytest.raises(AttributeError):
        del cmap._native
    assert not hasattr(cmap, "__dict__")


def test_copy_and_deepcopy_return_the_same_colormap() -> None:
    cmap = ub.Cmap.from_colors(THREE)
    assert copy.copy(cmap) is cmap
    assert copy.deepcopy(cmap) is cmap


@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_pickling_is_refused(protocol: int) -> None:
    with pytest.raises(TypeError) as caught:
        pickle.dumps(ub.Cmap.VIRIDIS, protocol)
    assert str(caught.value) == "Cmap: a Cmap cannot be pickled."
    assert details(caught.value) == (
        None,
        "rebuild it with Cmap.from_colors([Color.BLUE, Color.RED]).",
    )


def test_class_annotations_resolve_with_the_package_as_module() -> None:
    hints = typing.get_type_hints(ub.Cmap)
    assert hints["VIRIDIS"] == typing.ClassVar[ub.Cmap]
    assert typing.get_type_hints(ub.Cmap.reversed)["return"] is ub.Cmap
    assert typing.get_type_hints(ub.Cmap.from_colors)["colors"] == Sequence[ub.Color]
    assert ub.Cmap.__module__ == "umber"
    assert ub.Cmap.from_colors.__module__ == "umber"


@pytest.mark.parametrize("setter", ["with_cmap", "with_diverging_cmap"])
def test_theme_setters_replace_one_default(setter: str) -> None:
    base = ub.Theme.GRAPHITE
    changed = cast(ub.Theme, getattr(base, setter)(ub.Cmap.MAGMA))
    assert changed != base
    assert base == ub.Theme.GRAPHITE
    assert changed == getattr(base, setter)(ub.Cmap.MAGMA)
    assert changed != getattr(base, setter)(ub.Cmap.CIVIDIS)
    assert theme_native(changed) == getattr(theme_native(base), setter)(native(ub.Cmap.MAGMA))
    other = "with_diverging_cmap" if setter == "with_cmap" else "with_cmap"
    assert changed != cast(ub.Theme, getattr(base, other)(ub.Cmap.MAGMA))


@pytest.mark.parametrize("setter", ["with_cmap", "with_diverging_cmap"])
def test_theme_setters_reject_a_non_cmap(setter: str) -> None:
    with pytest.raises(TypeError) as caught:
        getattr(ub.Theme.PIGMENT, setter)("viridis")
    assert str(caught.value) == f"Theme.{setter}(): cmap is str; cmap must be of type Cmap."
    assert details(caught.value) == (
        "cmap",
        "use Cmap.VIRIDIS, Cmap.BLUE_RED or Cmap.from_colors([Color.BLUE, Color.RED]).",
    )


def test_theme_setter_type_error_highlights_the_argument(tmp_path: Path) -> None:
    line = "ub.Theme.PIGMENT.with_cmap('viridis')"
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err)[0] == _marked(line, "'viridis'")
