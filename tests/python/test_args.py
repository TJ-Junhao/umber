"""Tests of the scalar, text and path argument checks."""

import ast
import doctest
import os
from collections.abc import Callable
from decimal import Decimal
from enum import Enum
from pathlib import Path, PurePosixPath

import numpy as np
import pytest

import umber as ub
from umber import _args
from umber._args import (
    MAX_TEXT_LENGTH,
    checked_text,
    instance,
    integer,
    path,
    real,
    short_repr,
    text,
    type_name,
)
from umber._errors import details

TEXT_HINT = "pass a str; convert other values with str()."
REAL_HINT = "pass a number such as 1.5."
INTEGER_HINT = "pass a whole number such as 3."
PATH_HINT = "pass a str or a pathlib.Path."
BYTES_PATH_HINT = "decode a bytes path with os.fsdecode()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
SURROGATE_HINT = 'remove the character, or replace it: value.encode("utf-8", "replace").decode().'


def error(cls: type[BaseException], call: Callable[[], object]) -> tuple[str, object, object]:
    """Run a call that must raise exactly ``cls``; return its message, parameter and hint."""
    with pytest.raises(cls) as info:
        call()
    assert type(info.value) is cls
    found = details(info.value)
    assert found is not None
    return str(info.value), found[0], found[1]


# --- text ---


class Label(str):
    def __str__(self) -> str:
        return "overridden"


def test_text_returns_plain_str() -> None:
    assert text("Axes.title()", "text", "Revenue") == "Revenue"
    for value in (Label("Revenue"), np.str_("Revenue")):
        result = text("Axes.title()", "text", value)
        assert type(result) is str
        assert result == "Revenue"


@pytest.mark.parametrize(
    ("value", "shown"),
    [(42, "int"), (None, "None"), (b"title", "bytes"), (["a"], "list"), (1.5, "float")],
)
def test_text_rejects_other_types(value: object, shown: str) -> None:
    assert error(TypeError, lambda: text("Axes.title()", "text", value)) == (
        f"Axes.title(): text is {shown}; text must be a str.",
        "text",
        TEXT_HINT,
    )


def test_text_length_cap() -> None:
    assert MAX_TEXT_LENGTH == 1000
    assert text("Axes.title()", "text", "x" * 1000) == "x" * 1000
    assert error(ub.StyleError, lambda: text("Axes.title()", "text", "x" * 1001)) == (
        "Axes.title(): text has 1,001 characters; text must be at most 1,000 characters.",
        "text",
        TOO_LONG_HINT,
    )


def test_text_cap_counts_characters_not_bytes() -> None:
    wide = "水" * 1000
    assert text("Axes.title()", "text", wide) == wide


def test_text_rejects_a_lone_surrogate() -> None:
    assert error(ub.StyleError, lambda: text("Axes.xlabel()", "text", "ab\udc80c")) == (
        "Axes.xlabel(): text has a lone surrogate at index 2; text must be valid Unicode.",
        "text",
        SURROGATE_HINT,
    )


def test_text_accepts_paired_surrogates_written_as_one_character() -> None:
    assert text("Axes.title()", "text", "\U0001f600") == "\U0001f600"


def test_text_length_is_checked_before_the_surrogate() -> None:
    message, _, _ = error(ub.StyleError, lambda: text("Axes.title()", "text", "\ud800" * 1001))
    assert "1,001 characters" in message


def test_checked_text_uses_the_given_label_and_class() -> None:
    def call() -> str:
        return checked_text("Axes.bar()", "categories", "categories[3]", "\ud800", ub.DataError)

    assert error(ub.DataError, call) == (
        "Axes.bar(): categories[3] has a lone surrogate at index 0; text must be valid Unicode.",
        "categories",
        SURROGATE_HINT,
    )
    long = "y" * 1001
    assert error(
        ub.StyleError,
        lambda: checked_text("Axes.bar()", "categories", "categories[0]", long, ub.DataError),
    ) == (
        "Axes.bar(): categories[0] has 1,001 characters; text must be at most 1,000 characters.",
        "categories",
        TOO_LONG_HINT,
    )


# --- real ---


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        (3, 3.0),
        (-2.5, -2.5),
        (np.int8(-4), -4.0),
        (np.uint64(2**64 - 1), float(2**64 - 1)),
        (np.float16(0.5), 0.5),
        (np.float32(1.25), 1.25),
        (np.float64(7.5), 7.5),
        (2**1023, float(2**1023)),
    ],
)
def test_real_accepts_numbers(value: object, expected: float) -> None:
    result = real("Axes.xlim()", "low", value)
    assert type(result) is float
    assert result == expected


def test_real_keeps_nan_and_inf_for_the_core_to_check() -> None:
    assert np.isnan(real("Axes.xlim()", "low", float("nan")))
    assert real("Axes.xlim()", "low", float("-inf")) == float("-inf")


def test_real_turns_ints_beyond_the_float_range_into_infinity() -> None:
    assert real("Axes.xlim()", "high", 10**400) == float("inf")
    assert real("Axes.xlim()", "high", -(10**400)) == float("-inf")


@pytest.mark.parametrize(
    ("value", "shown"),
    [
        (True, "bool"),
        (np.True_, "bool"),
        ("1.5", "str"),
        (None, "None"),
        (Decimal("1.5"), "Decimal"),
        (1j, "complex"),
        ([1.0], "list"),
    ],
)
def test_real_rejects_other_types(value: object, shown: str) -> None:
    assert error(TypeError, lambda: real("Axes.xlim()", "low", value)) == (
        f"Axes.xlim(): low is {shown}; low must be a float.",
        "low",
        REAL_HINT,
    )


# --- integer ---


@pytest.mark.parametrize(
    ("value", "expected"),
    [(30, 30), (np.int32(-7), -7), (np.uint8(200), 200), (np.uint64(2**64 - 1), 2**63 - 1)],
)
def test_integer_accepts_ints(value: object, expected: int) -> None:
    result = integer("Axes.hist()", "bins", value)
    assert type(result) is int
    assert result == expected


def test_integer_clamps_to_the_i64_range() -> None:
    assert integer("Axes.hist()", "bins", 2**63 - 1) == 2**63 - 1
    assert integer("Axes.hist()", "bins", 2**200) == 2**63 - 1
    assert integer("Axes.hist()", "bins", -(2**63)) == -(2**63)
    assert integer("Axes.hist()", "bins", -(2**200)) == -(2**63)


@pytest.mark.parametrize(
    ("value", "shown"),
    [
        (True, "bool"),
        (np.False_, "bool"),
        (3.0, "float"),
        (np.float64(3.0), "float64"),
        ("3", "str"),
        (None, "None"),
    ],
)
def test_integer_rejects_other_types(value: object, shown: str) -> None:
    assert error(TypeError, lambda: integer("Axes.hist()", "bins", value)) == (
        f"Axes.hist(): bins is {shown}; bins must be an int.",
        "bins",
        INTEGER_HINT,
    )


# --- instance ---


class Dash(Enum):
    SOLID = 0


class Inset:
    pass


def test_instance_returns_the_value_unchanged() -> None:
    assert instance("Axes.line()", "dash", Dash.SOLID, Dash, "use Dash.SOLID.") is Dash.SOLID


def test_instance_rejects_other_types_with_the_given_hint() -> None:
    assert error(
        TypeError, lambda: instance("Axes.line()", "dash", "solid", Dash, "use Dash.SOLID.")
    ) == ("Axes.line(): dash is str; dash must be a Dash.", "dash", "use Dash.SOLID.")


def test_instance_message_uses_an_before_a_vowel() -> None:
    message, _, _ = error(
        TypeError, lambda: instance("Figure.add_axes()", "inset", 3, Inset, "pass an Inset.")
    )
    assert message == "Figure.add_axes(): inset is int; inset must be an Inset."


def test_instance_accepts_subclasses() -> None:
    class Wide(Inset):
        pass

    value = Wide()
    assert instance("Figure()", "inset", value, Inset, "pass an Inset.") is value


# --- path ---


class BytesPath:
    def __fspath__(self) -> bytes:
        return b"plot.png"


def test_path_accepts_str_and_path_like() -> None:
    assert path("Figure.save_png()", "path", "plot.png") == "plot.png"
    assert path("Figure.save_png()", "path", Path("out") / "plot.png") == os.path.join(
        "out", "plot.png"
    )
    result = path("Figure.save_png()", "path", PurePosixPath("plot.svg"))
    assert type(result) is str
    assert result == "plot.svg"
    assert type(path("Figure.save_png()", "path", np.str_("plot.png"))) is str


def test_path_rejects_bytes() -> None:
    assert error(TypeError, lambda: path("Figure.save_png()", "path", b"plot.png")) == (
        "Figure.save_png(): path is bytes; path must be a str or a path-like object.",
        "path",
        BYTES_PATH_HINT,
    )


def test_path_rejects_a_path_like_object_with_a_bytes_path() -> None:
    assert error(TypeError, lambda: path("Figure.save_svg()", "path", BytesPath())) == (
        "Figure.save_svg(): path is a bytes path; path must be a str path.",
        "path",
        BYTES_PATH_HINT,
    )


@pytest.mark.parametrize(("value", "shown"), [(None, "None"), (3, "int"), (["a"], "list")])
def test_path_rejects_other_types(value: object, shown: str) -> None:
    assert error(TypeError, lambda: path("Figure.save_png()", "path", value)) == (
        f"Figure.save_png(): path is {shown}; path must be a str or a path-like object.",
        "path",
        PATH_HINT,
    )


# --- message helpers ---


def test_type_name() -> None:
    assert type_name(None) == "None"
    assert type_name(np.float32(1)) == "float32"
    assert type_name(Dash.SOLID) == "Dash"


def test_short_repr_cuts_at_forty_characters() -> None:
    assert short_repr("a" * 38) == repr("a" * 38)
    assert len(repr("a" * 38)) == 40
    cut = short_repr("a" * 39)
    assert cut == "'" + "a" * 38 + "…"
    assert len(cut) == 40


def _attribute_doc(module_path: Path, name: str) -> str:
    """The string literal right after the top-level assignment of ``name``."""
    body = ast.parse(module_path.read_text(encoding="utf-8")).body
    for statement, following in zip(body, body[1:], strict=False):
        target = statement.target if isinstance(statement, ast.AnnAssign) else None
        if isinstance(target, ast.Name) and target.id == name:
            assert isinstance(following, ast.Expr)
            assert isinstance(following.value, ast.Constant)
            assert isinstance(following.value.value, str)
            return following.value.value
    raise AssertionError(f"{name} is not documented")


@pytest.mark.usefixtures("isolated_environment")
def test_max_text_length_example_runs() -> None:
    doc = _attribute_doc(Path(_args.__file__), "MAX_TEXT_LENGTH")
    test = doctest.DocTestParser().get_doctest(doc, {}, "MAX_TEXT_LENGTH", _args.__file__, 0)
    assert test.examples
    runner = doctest.DocTestRunner()
    runner.run(test)
    assert runner.summarize(verbose=False).failed == 0
