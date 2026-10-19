"""Color and scale arguments use the shared scalar checks."""

import numbers
from collections.abc import Callable
from decimal import Decimal
from fractions import Fraction
from typing import cast

import numpy as np
import pytest

import umber as ub
from umber import _args
from umber._errors import details


class RegisteredReal:
    def __float__(self) -> float:
        return 2.0


class RegisteredIntegral:
    def __int__(self) -> int:
        return 2

    def __index__(self) -> int:
        return 2


numbers.Real.register(RegisteredReal)
numbers.Integral.register(RegisteredIntegral)


type ScalarCall = tuple[Callable[[object], object], str, str]

CALL_HINTS = {
    "Color.hex()": 'pass a string such as "#3366B8".',
    "Color.alpha()": "pass a number from 0.0 to 1.0.",
    "Scale.log()": "pass a finite number at least 1.000000000001, such as 10.",
    "Color.rgb()": "pass a whole number such as 3.",
    "Color.palette()": "pass a whole number such as 3.",
}
REAL_CALLS: list[ScalarCall] = [
    (lambda value: ub.Color.BLUE.alpha(cast(float, value)), "Color.alpha()", "a"),
    (lambda value: ub.Scale.log(cast(float, value)), "Scale.log()", "base"),
]
INTEGER_CALLS: list[ScalarCall] = [
    (lambda value: ub.Color.rgb(cast(int, value), 0, 0), "Color.rgb()", "r"),
    (lambda value: ub.Color.rgb(0, cast(int, value), 0), "Color.rgb()", "g"),
    (lambda value: ub.Color.rgb(0, 0, cast(int, value)), "Color.rgb()", "b"),
    (lambda value: ub.Color.palette(cast(int, value)), "Color.palette()", "index"),
]
ALL_CALLS: list[
    tuple[Callable[[object], object], str, str, Callable[[str, str, object], object]]
] = [
    (
        lambda value: ub.Color.hex(cast(str, value)),
        "Color.hex()",
        "code",
        lambda context, name, value: _args.text(context, name, value, one_line="color codes"),
    ),
    *[(call, context, parameter, _args.real) for call, context, parameter in REAL_CALLS],
    *[(call, context, parameter, _args.integer) for call, context, parameter in INTEGER_CALLS],
]


@pytest.mark.parametrize(("call", "context", "parameter", "check"), ALL_CALLS)
def test_none_uses_the_shared_type_message(
    call: Callable[[object], object],
    context: str,
    parameter: str,
    check: Callable[[str, str, object], object],
) -> None:
    with pytest.raises(TypeError) as expected:
        check(context, parameter, None)
    with pytest.raises(TypeError) as caught:
        call(None)
    assert str(caught.value) == str(expected.value)
    assert details(caught.value) == (parameter, CALL_HINTS[context])
    assert " is None;" in str(caught.value)


@pytest.mark.parametrize("code", ["\ud800", "#12\udfff", "x" * 1001])
def test_hex_uses_the_shared_text_checks(code: str) -> None:
    with pytest.raises(ub.StyleError) as expected:
        _args.text("Color.hex()", "code", code, one_line="color codes")
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.hex(code)
    assert str(caught.value) == str(expected.value)
    assert details(caught.value) == details(expected.value)


def test_palette_reports_an_integer_beyond_the_decimal_limit() -> None:
    index = 10**5000
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.palette(index)
    assert str(caught.value) == (
        f"Color.palette(): index is int ({index.bit_length():,} bits); "
        "a palette index must be at most 9223372036854775807."
    )
    assert details(caught.value) == (
        "index",
        "indexes wrap around the palette, so a smaller one picks the same color.",
    )


@pytest.mark.parametrize(
    "value",
    [
        Fraction(1, 2),
        Decimal("0.5"),
        RegisteredReal(),
        RegisteredIntegral(),
        np.bool_(True),
        np.timedelta64(2, "D"),
    ],
)
@pytest.mark.parametrize(("call", "context", "parameter"), REAL_CALLS)
def test_real_arguments_reject_types_outside_the_shared_contract(
    value: object, call: Callable[[object], object], context: str, parameter: str
) -> None:
    with pytest.raises(TypeError) as expected:
        _args.real(context, parameter, value)
    with pytest.raises(TypeError) as caught:
        call(value)
    assert str(caught.value) == str(expected.value)
    assert details(caught.value) == (parameter, CALL_HINTS[context])


@pytest.mark.parametrize("value", [RegisteredIntegral(), np.bool_(True), np.timedelta64(2, "D")])
@pytest.mark.parametrize(("call", "context", "parameter"), INTEGER_CALLS)
def test_integer_arguments_reject_types_outside_the_shared_contract(
    value: object, call: Callable[[object], object], context: str, parameter: str
) -> None:
    with pytest.raises(TypeError) as expected:
        _args.integer(context, parameter, value)
    with pytest.raises(TypeError) as caught:
        call(value)
    assert str(caught.value) == str(expected.value)
    assert details(caught.value) == details(expected.value)
