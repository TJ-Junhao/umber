import importlib.util
import sys
from functools import cached_property
from pathlib import Path
from types import ModuleType

import pytest

import umber
from docs_check import missing_docs, undocumented_enum_members


def _public_objects() -> list[tuple[str, object]]:
    return [(name, getattr(umber, name)) for name in umber.__all__ if name != "__version__"]


def test_every_public_object_is_documented() -> None:
    assert missing_docs(_public_objects()) == []


def test_every_enum_member_has_a_docstring() -> None:
    assert undocumented_enum_members(_public_objects()) == []


# --- self-tests: the checker must catch what it is meant to catch (Review Focus 3) ---

_GOOD_DOC = """A documented object.

Example:
    >>> 1
    1
"""


class _NoDocs:
    def method(self) -> None:
        pass


class _NoExample:
    """Has a summary but no example."""


class _GoodClassBadMethod:
    """A documented class.

    Example:
        >>> 1
        1
    """

    def method(self) -> None:
        """Summary only."""


class _NoDocError(Exception):
    pass


class _ProseOnlyExample:
    """Has an Example heading but nothing runnable.

    Example:
        Call it with a number and it returns that number.
    """


class _CachedUndocumented:
    """A documented class.

    Example:
        >>> 1
        1
    """

    @cached_property
    def value(self) -> int:
        return 1


class _CachedDocumented:
    """A documented class.

    Example:
        >>> 1
        1
    """

    @cached_property
    def value(self) -> int:
        """Return one.

        Example:
            >>> 1
            1
        """
        return 1


def _undocumented_getter(self: object) -> int:
    return 1


def _documented_getter(self: object) -> int:
    """Return one.

    Example:
        >>> 1
        1
    """
    return 1


class _PropertyDocOnProperty:
    """A documented class.

    Example:
        >>> 1
        1
    """

    value = property(_undocumented_getter, doc=_GOOD_DOC)


class _PropertyGetterOnly:
    """A documented class.

    Example:
        >>> 1
        1
    """

    value = property(_documented_getter, doc="Summary only.")


def test_checker_flags_missing_class_docstring() -> None:
    assert "_NoDocs" in missing_docs([("_NoDocs", _NoDocs)])


def test_checker_flags_missing_example() -> None:
    assert missing_docs([("_NoExample", _NoExample)]) == ["_NoExample"]


def test_checker_flags_method_without_example() -> None:
    assert missing_docs([("_GoodClassBadMethod", _GoodClassBadMethod)]) == [
        "_GoodClassBadMethod.method"
    ]


def test_checker_does_not_accept_inherited_exception_docstring() -> None:
    assert "_NoDocError" in missing_docs([("_NoDocError", _NoDocError)])


def test_checker_flags_example_heading_without_runnable_example() -> None:
    assert missing_docs([("_ProseOnlyExample", _ProseOnlyExample)]) == ["_ProseOnlyExample"]


def test_checker_flags_undocumented_cached_property() -> None:
    assert missing_docs([("_CachedUndocumented", _CachedUndocumented)]) == [
        "_CachedUndocumented.value"
    ]


def test_checker_accepts_documented_cached_property() -> None:
    assert missing_docs([("_CachedDocumented", _CachedDocumented)]) == []


def test_checker_judges_property_by_its_own_doc_not_the_getter() -> None:
    assert missing_docs([("_PropertyDocOnProperty", _PropertyDocOnProperty)]) == []


def test_checker_flags_property_whose_own_doc_is_incomplete() -> None:
    assert missing_docs([("_PropertyGetterOnly", _PropertyGetterOnly)]) == [
        "_PropertyGetterOnly.value"
    ]


def _import_source(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, name: str, source: str
) -> ModuleType:
    """Write source to a temporary module file and import it, so inspect can find its source."""
    path = tmp_path / f"{name}.py"
    path.write_text(source)
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, name, module)
    spec.loader.exec_module(module)
    return module


def test_enum_checker_flags_member_without_docstring(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = _import_source(
        tmp_path,
        monkeypatch,
        "enum_plain_bad",
        "from enum import Enum\n"
        "class Dash(Enum):\n"
        '    """Doc."""\n'
        "    SOLID = 0\n"
        '    """Documented."""\n'
        "    DASHED = 1\n",
    )
    assert undocumented_enum_members([("Dash", module.Dash)]) == ["Dash.DASHED"]


def test_enum_checker_flags_aliased_enum_base(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = _import_source(
        tmp_path,
        monkeypatch,
        "enum_alias_bad",
        "from enum import Enum as E\n"
        "class Dash(E):\n"
        '    """Doc."""\n'
        "    SOLID = 0\n"
        '    """Documented."""\n'
        "    DASHED = 1\n",
    )
    assert undocumented_enum_members([("Dash", module.Dash)]) == ["Dash.DASHED"]


def test_enum_checker_accepts_documented_aliased_enum(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = _import_source(
        tmp_path,
        monkeypatch,
        "enum_alias_good",
        "from enum import Enum as E\n"
        "class Dash(E):\n"
        '    """Doc."""\n'
        "    SOLID = 0\n"
        '    """Documented."""\n'
        "    DASHED = 1\n"
        '    """Documented too."""\n',
    )
    assert undocumented_enum_members([("Dash", module.Dash)]) == []
