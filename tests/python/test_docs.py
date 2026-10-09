from pathlib import Path

import umber
from docs_check import missing_docs, undocumented_enum_members

PACKAGE_DIR = Path(umber.__file__).parent


def test_every_public_object_is_documented() -> None:
    objects = [(name, getattr(umber, name)) for name in umber.__all__ if name != "__version__"]
    assert missing_docs(objects) == []


def test_every_enum_member_has_a_docstring() -> None:
    assert undocumented_enum_members(PACKAGE_DIR) == []


# --- self-tests: the checker must catch what it is meant to catch (Review Focus 3) ---


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


def test_enum_checker_flags_member_without_docstring(tmp_path: Path) -> None:
    (tmp_path / "mod.py").write_text(
        "from enum import Enum\n"
        "class Dash(Enum):\n"
        '    """Doc."""\n'
        "    SOLID = 0\n"
        '    """Documented."""\n'
        "    DASHED = 1\n"
    )
    assert undocumented_enum_members(tmp_path) == ["mod.Dash.DASHED"]
