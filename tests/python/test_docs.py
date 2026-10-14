import importlib
import os
import sys
from functools import cached_property
from pathlib import Path
from types import ModuleType

import pytest

import umber
from docs_check import (
    failing_attribute_examples,
    missing_docs,
    undocumented_attributes,
    undocumented_enum_members,
)

PACKAGE_DIR = Path(umber.__file__).resolve().parent


def _public_objects() -> list[tuple[str, object]]:
    return [(name, getattr(umber, name)) for name in umber.__all__ if name != "__version__"]


def test_every_public_object_is_documented() -> None:
    assert missing_docs(_public_objects()) == []


def test_every_enum_member_has_a_docstring() -> None:
    assert undocumented_enum_members(PACKAGE_DIR, _public_objects()) == []


def test_every_alias_constant_and_class_constant_is_documented() -> None:
    assert undocumented_attributes(PACKAGE_DIR, umber.__all__) == []


def test_every_alias_and_constant_example_runs() -> None:
    assert failing_attribute_examples(PACKAGE_DIR, umber.__all__) == []


def test_cell_property_examples_run() -> None:
    import doctest
    import inspect

    for name in ("row", "col"):
        member = vars(umber.Cell)[name]
        assert isinstance(member, property)
        doc = inspect.getdoc(member)
        assert doc is not None
        test = doctest.DocTestParser().get_doctest(doc, {}, f"Cell.{name}", None, 0)
        assert test.examples
        assert doctest.DocTestRunner().run(test).failed == 0


# --- self-tests: the checker must catch what it is meant to catch ---

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


def _write_package(root: Path, files: dict[str, str], name: str = "pkg") -> Path:
    package = root / name
    package.mkdir()
    for filename, source in files.items():
        (package / filename).write_text(source)
    return package


def _import_package(
    root: Path, monkeypatch: pytest.MonkeyPatch, name: str, files: dict[str, str]
) -> tuple[Path, ModuleType]:
    """Write a package to disk and import it, so its classes are real runtime objects."""
    package = _write_package(root, files, name)
    monkeypatch.setattr(sys, "path", [str(root), *sys.path])
    importlib.invalidate_caches()
    return package, importlib.import_module(name)


_DASH_DOCUMENTED = (
    "from enum import Enum{base_import}\n"
    "class Dash({base}):\n"
    '    """Doc."""\n'
    "    SOLID = 0\n"
    '    """Documented."""\n'
    "{extra}"
)


def _enum_files(package_name: str, *, extra: str, base: str = "Enum") -> dict[str, str]:
    base_import = " as E" if base == "E" else ""
    return {
        "__init__.py": (
            f'from ._defs import Dash\nDash.__module__ = "{package_name}"\n__all__ = ["Dash"]\n'
        ),
        "_defs.py": _DASH_DOCUMENTED.format(base_import=base_import, base=base, extra=extra),
    }


def _enum_members(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, package_name: str, files: dict[str, str]
) -> list[str]:
    package, module = _import_package(tmp_path, monkeypatch, package_name, files)
    return undocumented_enum_members(package, [("Dash", module.Dash)])


def test_enum_checker_flags_member_without_docstring_after_module_rewrite(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    files = _enum_files("enum_plain_bad", extra="    DASHED = 1\n")
    assert _enum_members(tmp_path, monkeypatch, "enum_plain_bad", files) == ["Dash.DASHED"]


def test_enum_checker_flags_aliased_enum_base(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    files = _enum_files("enum_alias_bad", extra="    DASHED = 1\n", base="E")
    assert _enum_members(tmp_path, monkeypatch, "enum_alias_bad", files) == ["Dash.DASHED"]


def test_enum_checker_accepts_documented_aliased_enum(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    extra = '    DASHED = 1\n    """Documented too."""\n'
    files = _enum_files("enum_alias_good", extra=extra, base="E")
    assert _enum_members(tmp_path, monkeypatch, "enum_alias_good", files) == []


def test_enum_checker_flags_annotated_member(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    extra = '    DASHED: int = 1\n    """Documented, but annotated."""\n'
    files = _enum_files("enum_annotated_bad", extra=extra)
    assert _enum_members(tmp_path, monkeypatch, "enum_annotated_bad", files) == ["Dash.DASHED"]


def test_enum_checker_follows_a_renamed_export(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    files = _enum_files("enum_renamed", extra="    DASHED = 1\n")
    files["__init__.py"] = "from ._defs import Dash as Stroke\n"
    package, module = _import_package(tmp_path, monkeypatch, "enum_renamed", files)
    found = undocumented_enum_members(package, [("Stroke", module.Stroke)])
    assert found == ["Stroke.DASHED"]


def test_enum_checker_fails_when_the_definition_cannot_be_found(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    files = _enum_files("enum_missing", extra="")
    package, module = _import_package(tmp_path, monkeypatch, "enum_missing", files)
    (package / "__init__.py").write_text("")
    with pytest.raises(LookupError):
        undocumented_enum_members(package, [("Dash", module.Dash)])


type _Alias = list[int]


def test_checker_skips_aliases_and_constants() -> None:
    assert missing_docs([("_Alias", _Alias), ("LIMIT", 3)]) == []


_DOC = '"""A documented object.\n\nExample:\n    >>> 1\n    1\n"""\n'
_ALIAS = "type Values = list[float]\n"


def _exporting(root: Path, init: str, **modules: str) -> Path:
    files = {"__init__.py": init} | {f"{name}.py": source for name, source in modules.items()}
    return _write_package(root, files)


def test_attribute_checker_flags_alias_without_docstring(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import Values\n", mod=_ALIAS)
    assert undocumented_attributes(package, ["Values"]) == ["Values"]


def test_attribute_checker_accepts_documented_alias(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import Values\n", mod=_ALIAS + _DOC)
    assert undocumented_attributes(package, ["Values"]) == []


def test_attribute_checker_flags_constant_without_example(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import LIMIT\n", mod='LIMIT = 3\n"""The limit."""\n')
    assert undocumented_attributes(package, ["LIMIT"]) == ["LIMIT"]


def test_attribute_checker_accepts_documented_constant(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import LIMIT\n", mod="LIMIT: int = 3\n" + _DOC)
    assert undocumented_attributes(package, ["LIMIT"]) == []


def test_attribute_checker_ignores_names_that_are_not_exported(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import LIMIT\n", mod="LIMIT = 3\n")
    assert undocumented_attributes(package, []) == []


def test_attribute_checker_ignores_a_same_named_private_definition(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .good import Values\n", good=_ALIAS + _DOC, _other=_ALIAS)
    assert undocumented_attributes(package, ["Values"]) == []


def test_attribute_checker_follows_a_renamed_alias(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import Values as Series\n", mod=_ALIAS)
    assert undocumented_attributes(package, ["Series"]) == ["Series"]


_CLASS = (
    "from typing import ClassVar\n"
    "class Palette:\n"
    '    """Doc."""\n'
    "    SIZE: ClassVar[int] = 3\n"
    '    """Documented."""\n'
    "    COUNT: ClassVar[int] = 4\n"
    "    other: int = 5\n"
)


def test_attribute_checker_flags_undocumented_class_var(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import Palette\n", mod=_CLASS)
    assert undocumented_attributes(package, ["Palette"]) == ["Palette.COUNT"]


def test_attribute_checker_follows_a_renamed_class(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import Palette as Colors\n", mod=_CLASS)
    assert undocumented_attributes(package, ["Colors"]) == ["Colors.COUNT"]


def test_attribute_checker_follows_an_assignment_to_a_documented_class(tmp_path: Path) -> None:
    documented = _CLASS.replace(
        "    COUNT: ClassVar[int] = 4\n", '    COUNT: ClassVar[int] = 4\n    """Documented."""\n'
    )
    package = _exporting(tmp_path, "from . import mod\nPalette = mod.Palette\n", mod=documented)
    assert undocumented_attributes(package, ["Palette"]) == []


def test_attribute_checker_flags_class_var_behind_an_assignment(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from . import mod\nPalette = mod.Palette\n", mod=_CLASS)
    assert undocumented_attributes(package, ["Palette"]) == ["Palette.COUNT"]


@pytest.mark.parametrize(
    ("header", "annotation"),
    [
        ("from typing import ClassVar as CV\n", "CV[int]"),
        ("import typing as t\n", "t.ClassVar[int]"),
        ("import typing\n", "typing.ClassVar[int]"),
        ("from typing_extensions import ClassVar\n", "ClassVar[int]"),
        ("from typing import ClassVar\n", '"ClassVar[int]"'),
        ("from typing import ClassVar as CV\n", '"CV[int]"'),
    ],
)
def test_attribute_checker_recognises_every_class_var_spelling(
    tmp_path: Path, header: str, annotation: str
) -> None:
    source = f'{header}class Color:\n    """Doc."""\n    SIZE: {annotation} = 3\n'
    package = _exporting(tmp_path, "from .mod import Color\n", mod=source)
    assert undocumented_attributes(package, ["Color"]) == ["Color.SIZE"]


def test_attribute_checker_ignores_a_plain_annotation(tmp_path: Path) -> None:
    source = 'class Color:\n    """Doc."""\n    size: int = 3\n'
    package = _exporting(tmp_path, "from .mod import Color\n", mod=source)
    assert undocumented_attributes(package, ["Color"]) == []


def test_example_runner_flags_a_failing_alias_example(tmp_path: Path) -> None:
    failing = '"""A failing alias.\n\nExample:\n    >>> 1 / 0\n    1\n"""\n'
    package = _exporting(tmp_path, "from .mod import Values\n", mod=_ALIAS + failing)
    assert failing_attribute_examples(package, ["Values"]) == ["Values"]


def test_example_runner_accepts_a_passing_constant_example(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "from .mod import LIMIT\n", mod="LIMIT = 3\n" + _DOC)
    assert failing_attribute_examples(package, ["LIMIT"]) == []


def test_attribute_checker_follows_an_assignment_from_an_imported_name(tmp_path: Path) -> None:
    init = "from .mod import Palette as _Palette\nPalette = _Palette\n"
    package = _exporting(tmp_path, init, mod=_CLASS)
    assert undocumented_attributes(package, ["Palette"]) == ["Palette.COUNT"]


def test_resolution_fails_closed_for_a_conditional_import(tmp_path: Path) -> None:
    init = "try:\n    from .mod import Color\nexcept ImportError:\n    Color = None\n"
    package = _exporting(tmp_path, init, mod='class Color:\n    """Doc."""\n')
    with pytest.raises(LookupError, match="Color"):
        undocumented_attributes(package, ["Color"])


def test_resolution_fails_closed_beyond_the_depth_guard(tmp_path: Path) -> None:
    modules = {f"m{i}": f"from .m{i + 1} import Values\n" for i in range(20)}
    modules["m20"] = _ALIAS + _DOC
    package = _exporting(tmp_path, "from .m0 import Values\n", **modules)
    with pytest.raises(LookupError, match="Values"):
        undocumented_attributes(package, ["Values"])


def test_resolution_fails_closed_for_an_undefined_export(tmp_path: Path) -> None:
    package = _exporting(tmp_path, "")
    with pytest.raises(LookupError, match="Missing"):
        undocumented_attributes(package, ["Missing"])


def test_resolution_skips_functions_and_names_outside_the_package(tmp_path: Path) -> None:
    init = "from ._native import version\nfrom .mod import helper\n"
    package = _exporting(tmp_path, init, mod="def helper() -> None:\n    pass\n")
    assert undocumented_attributes(package, ["version", "helper"]) == []


def test_class_var_import_in_another_scope_does_not_shadow_the_module(tmp_path: Path) -> None:
    source = (
        "from typing import ClassVar\n"
        "class Other:\n"
        "    from builtins import int as ClassVar\n"
        "class Color:\n"
        '    """Doc."""\n'
        "    SIZE: ClassVar[int] = 3\n"
    )
    package = _exporting(tmp_path, "from .mod import Color\n", mod=source)
    assert undocumented_attributes(package, ["Color"]) == ["Color.SIZE"]


def test_class_var_import_in_the_class_body_is_used(tmp_path: Path) -> None:
    source = (
        "class Color:\n"
        '    """Doc."""\n'
        "    from typing import ClassVar as CV\n"
        "    SIZE: CV[int] = 3\n"
    )
    package = _exporting(tmp_path, "from .mod import Color\n", mod=source)
    assert undocumented_attributes(package, ["Color"]) == ["Color.SIZE"]


def _example_doc(code: str, expected: str = "") -> str:
    return f'"""Alias.\n\nExample:\n    >>> {code}\n    {expected}\n"""\n'.replace("    \n", "")


def test_attribute_examples_do_not_share_a_directory(tmp_path: Path) -> None:
    make = "import pathlib; pathlib.Path('workspace').mkdir()"
    package = _exporting(
        tmp_path,
        "from .mod import A, B\n",
        mod=f"type A = int\n{_example_doc(make)}type B = int\n{_example_doc(make)}",
    )
    assert failing_attribute_examples(package, ["A", "B"]) == []


def test_attribute_examples_do_not_share_the_environment(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("FORCE_COLOR", "1")
    change = "import os; os.environ['NO_COLOR'] = '0'; os.environ['KEEP'] = 'x'"
    read = (
        "import os; "
        "print(os.environ['NO_COLOR'], os.environ.get('KEEP'), 'FORCE_COLOR' in os.environ)"
    )
    second = _example_doc(read, "1 None False")
    package = _exporting(
        tmp_path,
        "from .mod import A, B\n",
        mod=f"type A = int\n{_example_doc(change)}type B = int\n{second}",
    )
    assert failing_attribute_examples(package, ["A", "B"]) == []
    assert os.environ["FORCE_COLOR"] == "1"
    assert "KEEP" not in os.environ
