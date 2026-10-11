# pyright: reportPrivateUsage=false
import math
from typing import cast

import pytest

import umber as ub
from umber._errors import details


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def test_linear_log_and_category_scales() -> None:
    assert ub.Scale.linear()._kind_and_base()[0] == 0
    assert ub.Scale.log()._kind_and_base() == (1, 10.0)
    assert ub.Scale.log(2)._kind_and_base() == (1, 2.0)
    assert ub.Scale.category()._kind_and_base()[0] == 2


def test_default_log_base_is_ten() -> None:
    assert ub.Scale.log() == ub.Scale.log(10.0)


def test_equality_and_hash() -> None:
    assert ub.Scale.linear() == ub.Scale.linear()
    assert ub.Scale.category() == ub.Scale.category()
    assert ub.Scale.log(2) == ub.Scale.log(2.0)
    assert hash(ub.Scale.log(2)) == hash(ub.Scale.log(2.0))
    assert ub.Scale.log(2) != ub.Scale.log(10)
    assert ub.Scale.linear() != ub.Scale.log()
    assert ub.Scale.linear() != ub.Scale.category()
    assert ub.Scale.linear() != "linear"
    assert len({ub.Scale.linear(), ub.Scale.linear(), ub.Scale.category()}) == 2


def test_repr() -> None:
    assert repr(ub.Scale.linear()) == "Scale.linear()"
    assert repr(ub.Scale.log()) == "Scale.log(10.0)"
    assert repr(ub.Scale.log(2)) == "Scale.log(2.0)"
    assert repr(ub.Scale.category()) == "Scale.category()"


def test_scale_is_immutable_and_has_no_public_constructor() -> None:
    scale = ub.Scale.linear()
    with pytest.raises(AttributeError):
        scale._kind = 1
    with pytest.raises(AttributeError):
        del scale._kind
    with pytest.raises(AttributeError):
        scale.extra = 1
    with pytest.raises(TypeError) as caught:
        ub.Scale()
    assert str(caught.value) == (
        "Scale(): Scale has no public constructor; use one of its constructors."
    )
    assert details(caught.value) == (None, "use Scale.linear(), Scale.log() or Scale.category().")


@pytest.mark.parametrize(
    ("base", "shown"),
    [(1.0, "1"), (0.5, "0.5"), (-2, "-2"), (math.inf, "inf"), (math.nan, "NaN"), (10**400, "inf")],
)
def test_log_rejects_invalid_bases_with_the_native_message(base: float, shown: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Scale.log(base)
    assert str(caught.value) == (
        f"Scale.log(): base is {shown}; a log base must be finite and greater than 1."
    )
    assert details(caught.value) == ("base", None)


def test_log_error_highlights_the_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Scale.log(1.0)
    assert str(caught.value) == (
        "Scale.log(): base is 1; a log base must be finite and greater than 1."
    )
    assert _notes(caught.value) == [_marked("ub.Scale.log(1.0)", "1.0")]


def test_log_error_highlights_a_keyword_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Scale.log(base=0.5)
    assert _notes(caught.value) == [_marked("ub.Scale.log(base=0.5)", "base=0.5")]


@pytest.mark.parametrize("base", ["10", None, True, [10]])
def test_log_rejects_other_types(base: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Scale.log(cast(float, base))
    name = type(base).__name__
    assert str(caught.value) == f"Scale.log(): base is {name}; base must be a float."
    assert details(caught.value) == ("base", "pass a number greater than 1, such as 10.")


def test_log_type_error_highlights_the_argument() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Scale.log(cast(float, "10"))
    assert _notes(caught.value) == [
        _marked('ub.Scale.log(cast(float, "10"))', 'cast(float, "10")'),
        "hint: pass a number greater than 1, such as 10.",
    ]


def test_log_accepts_numpy_numbers() -> None:
    import numpy as np

    assert ub.Scale.log(cast(float, np.int64(2))) == ub.Scale.log(2.0)
    assert ub.Scale.log(cast(float, np.float32(4.0))) == ub.Scale.log(4.0)
