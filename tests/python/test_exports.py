import ast
import doctest
import enum
import inspect
from pathlib import Path

import pytest

import umber

EXPECTED = [
    "AUTO",
    "MAX_BINS",
    "Align",
    "Auto",
    "Axes",
    "Cell",
    "Cmap",
    "Color",
    "ColorBy",
    "ColorbarPosition",
    "Dash",
    "DataError",
    "Distribution",
    "Figure",
    "FillStyle",
    "Font",
    "GridLines",
    "Hist",
    "Legend",
    "LegendPosition",
    "LegendStyle",
    "Marker",
    "PieAxes",
    "Render",
    "Scale",
    "Series",
    "Share",
    "Size",
    "Stat",
    "Step",
    "StyleError",
    "SupportsArray",
    "SvgText",
    "Theme",
    "TickFormat",
    "Time",
    "UmberError",
    "UsageError",
    "Values",
    "__version__",
    "bar",
    "hist",
    "line",
    "pie",
    "scatter",
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


def test_quick_functions_are_exported_as_functions() -> None:
    functions = {name for name in umber.__all__ if inspect.isfunction(getattr(umber, name))}
    assert functions == {"bar", "hist", "line", "pie", "scatter"}


def test_enums_are_exported_as_enums() -> None:
    enums = {
        name
        for name in umber.__all__
        if inspect.isclass(obj := getattr(umber, name)) and issubclass(obj, enum.Enum)
    }
    assert enums == {
        "Align",
        "Auto",
        "Share",
        "Legend",
        "LegendPosition",
        "Dash",
        "FillStyle",
        "GridLines",
        "LegendStyle",
        "Marker",
        "Render",
        "Step",
        "SvgText",
        "ColorbarPosition",
        "Hist",
        "Stat",
    }


def _public_members() -> list[tuple[str, object]]:
    """Every public method, property and ``ClassVar`` of the exported classes, qualified."""
    found: list[tuple[str, object]] = []
    for name in umber.__all__:
        obj = getattr(umber, name)
        if not inspect.isclass(obj) or issubclass(obj, BaseException):
            continue
        for attr, member in vars(obj).items():
            if not attr.startswith("_") and not issubclass(obj, enum.Enum):
                found.append((f"{name}.{attr}", member))
    return found


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


def test_every_public_method_reports_the_package_and_has_examples() -> None:
    parser = doctest.DocTestParser()
    missing: list[str] = []
    for name, member in _public_members():
        function = getattr(member, "__func__", member)
        if not inspect.isfunction(function):
            continue
        assert function.__module__ == "umber", name
        if not parser.get_examples(inspect.getdoc(function) or ""):
            missing.append(name)
    assert missing == []


def _run_examples(name: str, doc: str) -> doctest.TestResults:
    test = doctest.DocTestParser().get_doctest(inspect.cleandoc(doc), {}, name, None, 0)
    return doctest.DocTestRunner().run(test, out=lambda _text: None)


@pytest.mark.usefixtures("isolated_environment")
def test_public_method_examples_run() -> None:
    # Examples that save a figure write to the working directory, a temporary one here.
    failed = [
        name
        for name, member in _public_members()
        if inspect.isfunction(function := getattr(member, "__func__", member))
        and _run_examples(name, inspect.getdoc(function) or "").failed
    ]
    assert failed == []


def _constant_docs() -> dict[str, str]:
    """Docstrings that follow an assignment in ``__init__.py`` and the constant-holding classes."""
    package = Path(umber.__file__).resolve().parent
    docs: dict[str, str] = {}

    def collect(body: list[ast.stmt], prefix: str) -> None:
        for stmt, following in zip(body, body[1:], strict=False):
            if not isinstance(stmt, ast.Assign | ast.AnnAssign):
                continue
            if not (isinstance(following, ast.Expr) and isinstance(following.value, ast.Constant)):
                continue
            target = stmt.targets[0] if isinstance(stmt, ast.Assign) else stmt.target
            text = following.value.value
            if isinstance(target, ast.Name) and isinstance(text, str):
                docs[prefix + target.id] = text

    collect(ast.parse((package / "__init__.py").read_text()).body, "")
    for module, class_name in (
        ("_color.py", "Color"),
        ("_font.py", "Font"),
        ("_size.py", "Size"),
        ("_theme.py", "Theme"),
        ("_ticks.py", "TickFormat"),
    ):
        for stmt in ast.parse((package / module).read_text()).body:
            if isinstance(stmt, ast.ClassDef) and stmt.name == class_name:
                collect(stmt.body, f"{class_name}.")
    return docs


def test_class_constant_and_attribute_examples_run() -> None:
    docs = _constant_docs()
    assert {
        "MAX_BINS",
        "Color.AUTO",
        "Color.BLUE",
        "Color.BROWN",
        "Font.INTER",
        "Font.JETBRAINS_MONO",
        "Size.AUTO",
        "TickFormat.AUTO",
    } <= set(docs)
    assert {"Theme.PIGMENT", "Theme.GRAPHITE", "Theme.MIDNIGHT"} <= set(docs)
    failed = [name for name, doc in docs.items() if _run_examples(name, doc).failed]
    assert failed == []
    assert all(doctest.DocTestParser().get_examples(doc) for doc in docs.values())


def test_values_alias_is_exported_from_its_module() -> None:
    from umber import _types

    assert umber.Values is _types.Values
    # A ``type`` alias cannot be given another module; its name is what hover shows.
    assert umber.Values.__name__ == "Values"
    assert umber.SupportsArray is _types.SupportsArray
