import pytest

import umber as ub


@pytest.mark.parametrize("cls", [ub.DataError, ub.StyleError])
def test_value_errors_are_catchable_as_value_error(cls: type[ub.UmberError]) -> None:
    with pytest.raises(ValueError):
        raise cls("Axes.line(): something is wrong; something is required.")


@pytest.mark.parametrize("cls", [ub.DataError, ub.StyleError, ub.UsageError])
def test_all_errors_derive_from_umber_error(cls: type[ub.UmberError]) -> None:
    assert issubclass(cls, ub.UmberError)


def test_usage_error_is_not_a_value_error() -> None:
    assert not issubclass(ub.UsageError, ValueError)


def test_message_is_preserved() -> None:
    msg = "Axes.line(): x has 3 values but y has 2; x and y must have the same length."
    assert str(ub.DataError(msg)) == msg
