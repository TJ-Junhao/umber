import doctest
import enum
import inspect

import umber

EXPECTED = [
    "MAX_BINS",
    "Color",
    "DataError",
    "Dash",
    "Font",
    "GridLines",
    "LegendStyle",
    "Marker",
    "Scale",
    "StyleError",
    "SvgText",
    "UmberError",
    "UsageError",
    "__version__",
]


def test_public_names() -> None:
    assert umber.__all__ == EXPECTED


def test_every_public_name_exists() -> None:
    for name in umber.__all__:
        assert hasattr(umber, name), name


def test_max_bins_is_the_native_limit() -> None:
    from umber import _umber

    assert umber.MAX_BINS == _umber.MAX_BINS == 10_000


def test_exported_classes_report_the_package_as_their_module() -> None:
    for name in umber.__all__:
        obj = getattr(umber, name)
        if inspect.isclass(obj) or inspect.isfunction(obj):
            assert obj.__module__ == "umber", name


def test_enums_are_exported_as_enums() -> None:
    enums = {
        name
        for name in umber.__all__
        if inspect.isclass(obj := getattr(umber, name)) and issubclass(obj, enum.Enum)
    }
    assert enums == {"Dash", "Font", "GridLines", "LegendStyle", "Marker", "SvgText"}


def test_every_exported_class_and_function_has_doctest_examples() -> None:
    finder = doctest.DocTestFinder(recurse=False)
    missing: list[str] = []
    for name in umber.__all__:
        obj = getattr(umber, name)
        if not (inspect.isclass(obj) or inspect.isfunction(obj)):
            continue
        found = finder.find(obj, name, module=umber)
        if not any(test.examples for test in found):
            missing.append(name)
    assert missing == []
