"""Checks that enforce the documentation rules for the public API."""

import ast
import doctest
import enum
import inspect
from collections.abc import Iterable
from dataclasses import dataclass
from functools import cached_property
from pathlib import Path
from typing import TypeAliasType


def _own_doc(obj: object) -> str:
    """Return the docstring defined on obj itself, never one inherited from a base class."""
    if inspect.isclass(obj):
        doc = obj.__dict__.get("__doc__")
        return doc if isinstance(doc, str) else ""
    return inspect.getdoc(obj) or ""


def _is_complete(doc: str) -> bool:
    """Return True if doc has a summary line, an ``Example:`` heading and a runnable example."""
    doc = inspect.cleandoc(doc)
    lines = [line.strip() for line in doc.splitlines()]
    has_heading = bool(lines) and bool(lines[0]) and "Example:" in lines
    return has_heading and bool(doctest.DocTestParser().get_examples(doc))


def missing_docs(objects: Iterable[tuple[str, object]]) -> list[str]:
    """Return qualified names of objects lacking a summary line or a runnable ``Example:``.

    For classes, every public method, property, cached property, staticmethod and
    classmethod defined on the class itself is checked as well. Properties are judged by
    their own ``__doc__``, which is what hover and ``help()`` show. Type aliases and other
    objects that are neither classes nor functions are skipped.
    """
    missing: list[str] = []
    for name, obj in objects:
        # Aliases and constants have no ``__doc__`` of their own; ``undocumented_attributes``
        # judges them by the string literal that follows their assignment.
        if isinstance(obj, TypeAliasType) or not (inspect.isclass(obj) or inspect.isfunction(obj)):
            continue
        if not _is_complete(_own_doc(obj)):
            missing.append(name)
        if inspect.isclass(obj):
            for attr, member in vars(obj).items():
                if attr.startswith("_"):
                    continue
                if isinstance(member, property | cached_property):
                    doc = member.__doc__ or ""
                else:
                    # staticmethod and classmethod wrap the real function in ``__func__``.
                    target: object = getattr(member, "__func__", member)
                    if not inspect.isfunction(target):
                        continue
                    doc = inspect.getdoc(target) or ""
                if not _is_complete(doc):
                    missing.append(f"{name}.{attr}")
    return missing


@dataclass(frozen=True)
class _Definition:
    """Where an exported name is really defined, found through the package's import bindings."""

    is_class: bool
    path: Path
    body: list[ast.stmt]
    index: int
    imports: dict[str, str]

    @property
    def statement(self) -> ast.stmt:
        return self.body[self.index]

    @property
    def following(self) -> ast.stmt | None:
        return self.body[self.index + 1] if self.index + 1 < len(self.body) else None


_MAX_DEPTH = 16
_CLASS_VAR_NAMES = frozenset({"typing.ClassVar", "typing_extensions.ClassVar"})


def _parse(path: Path) -> ast.Module:
    return ast.parse(path.read_text(encoding="utf-8"))


def _import_map(tree: ast.Module) -> dict[str, str]:
    """Map each locally bound import name to its dotted origin."""
    imports: dict[str, str] = {}
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.asname:
                    imports[alias.asname] = alias.name
                else:
                    head = alias.name.split(".")[0]
                    imports[head] = head
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            for alias in node.names:
                imports[alias.asname or alias.name] = f"{node.module}.{alias.name}"
    return imports


def _dotted(node: ast.expr) -> str | None:
    """Return ``a.b.c`` for a chain of attribute accesses on a name, else None."""
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        base = _dotted(node.value)
        return None if base is None else f"{base}.{node.attr}"
    return None


def _is_class_var(stmt: ast.stmt, imports: dict[str, str]) -> bool:
    """Return True if stmt annotates a name with ``typing.ClassVar``, however it is spelled."""
    if not isinstance(stmt, ast.AnnAssign):
        return False
    annotation = stmt.annotation
    if isinstance(annotation, ast.Constant) and isinstance(annotation.value, str):
        try:
            annotation = ast.parse(annotation.value, mode="eval").body
        except SyntaxError:
            return False
    if isinstance(annotation, ast.Subscript):
        annotation = annotation.value
    dotted = _dotted(annotation)
    if dotted is None:
        return False
    head, _, rest = dotted.partition(".")
    return (imports.get(head, head) + (f".{rest}" if rest else "")) in _CLASS_VAR_NAMES


def _locate(package_dir: Path, current: Path, level: int, module: str | None) -> Path | None:
    """Return the package file an import statement in ``current`` refers to, if it is ours."""
    parts = module.split(".") if module else []
    if level:
        base = current.parent
        for _ in range(level - 1):
            base = base.parent
    elif parts and parts[0] == package_dir.name:
        base, parts = package_dir, parts[1:]
    else:
        return None
    target = base.joinpath(*parts)
    candidates = [target / "__init__.py"] if not parts else [target.with_suffix(".py")]
    candidates.append(target / "__init__.py")
    return next((c for c in candidates if c.is_file()), None)


def _module_binding(package_dir: Path, path: Path, tree: ast.Module, name: str) -> Path | None:
    """Return the package module file that ``name`` is bound to by an import, if any."""
    for stmt in tree.body:
        if isinstance(stmt, ast.ImportFrom):
            for alias in stmt.names:
                if (alias.asname or alias.name) != name:
                    continue
                parent = _locate(package_dir, path, stmt.level, stmt.module)
                if parent is None:
                    continue
                for candidate in (
                    parent.parent / f"{alias.name}.py",
                    parent.parent / alias.name / "__init__.py",
                ):
                    if candidate.is_file():
                        return candidate
        elif isinstance(stmt, ast.Import):
            for alias in stmt.names:
                if alias.asname == name:
                    return _locate(package_dir, path, 0, alias.name)
    return None


def _resolve(package_dir: Path, path: Path, name: str, depth: int = 0) -> _Definition | None:
    """Follow ``name`` in ``path`` through imports and assignments to its definition.

    Names that lead outside the package (the native extension, the standard library) and
    names bound to functions resolve to None. The last binding in a module wins.
    """
    if depth > _MAX_DEPTH:
        return None
    tree = _parse(path)
    imports = _import_map(tree)
    found: _Definition | None = None

    def defined(index: int, *, is_class: bool) -> _Definition:
        return _Definition(is_class, path, tree.body, index, imports)

    for index, stmt in enumerate(tree.body):
        if isinstance(stmt, ast.ClassDef) and stmt.name == name:
            found = defined(index, is_class=True)
        elif isinstance(stmt, ast.TypeAlias) and stmt.name.id == name:
            found = defined(index, is_class=False)
        elif isinstance(stmt, ast.Assign | ast.AnnAssign):
            targets = stmt.targets if isinstance(stmt, ast.Assign) else [stmt.target]
            if len(targets) != 1 or not (
                isinstance(targets[0], ast.Name) and targets[0].id == name
            ):
                continue
            value = stmt.value
            followed: _Definition | None = None
            if isinstance(value, ast.Name):
                followed = _resolve(package_dir, path, value.id, depth + 1)
            elif isinstance(value, ast.Attribute) and isinstance(value.value, ast.Name):
                module = _module_binding(package_dir, path, tree, value.value.id)
                if module is not None:
                    followed = _resolve(package_dir, module, value.attr, depth + 1)
            found = followed or defined(index, is_class=False)
        elif isinstance(stmt, ast.FunctionDef | ast.AsyncFunctionDef) and stmt.name == name:
            found = None
        elif isinstance(stmt, ast.ImportFrom):
            for alias in stmt.names:
                if alias.name == "*" or (alias.asname or alias.name) != name:
                    continue
                target = _locate(package_dir, path, stmt.level, stmt.module)
                found = None
                if target is not None and target != path:
                    found = _resolve(package_dir, target, alias.name, depth + 1)
    return found


def _class_definition(package_dir: Path, name: str) -> _Definition | None:
    return _resolve(package_dir, package_dir / "__init__.py", name)


def undocumented_enum_members(
    package_dir: Path, objects: Iterable[tuple[str, object]]
) -> list[str]:
    """Return ``<name>.MEMBER`` for enum members not followed by a string literal.

    Enums are found by their runtime type, not by how their base class is spelled, so
    ``from enum import Enum as E`` is handled. Each enum's definition is located through the
    import bindings of ``package_dir/__init__.py``, never through ``inspect.getsource``,
    which cannot find a class whose ``__module__`` was rewritten. Each member's docstring
    must be a string literal right after the assignment, which is the form pyright shows on
    hover. An annotated member (``X: int = 1``) is rejected as well, because the typing
    convention forbids annotated enum members.

    Raises:
        LookupError: An exported enum has no class definition in the package.

    Example:
        >>> from pathlib import Path
        >>> package = Path("pkg")
        >>> package.mkdir()
        >>> _ = (package / "__init__.py").write_text("")
        >>> undocumented_enum_members(package, [])
        []
    """
    missing: list[str] = []
    for name, obj in objects:
        if not (inspect.isclass(obj) and issubclass(obj, enum.Enum)):
            continue
        definition = _class_definition(package_dir, name)
        node = definition.statement if definition is not None else None
        if not isinstance(node, ast.ClassDef):
            raise LookupError(f"no class definition found for the enum {name!r}")
        body = node.body
        for i, stmt in enumerate(body):
            if isinstance(stmt, ast.AnnAssign):
                if isinstance(stmt.target, ast.Name) and not stmt.target.id.startswith("_"):
                    missing.append(f"{name}.{stmt.target.id}")
                continue
            if not (isinstance(stmt, ast.Assign) and len(stmt.targets) == 1):
                continue
            target = stmt.targets[0]
            if not isinstance(target, ast.Name) or target.id.startswith("_"):
                continue
            nxt = body[i + 1] if i + 1 < len(body) else None
            if _string_literal(nxt) is None:
                missing.append(f"{name}.{target.id}")
    return missing


def _string_literal(stmt: ast.stmt | None) -> ast.Constant | None:
    """Return the string literal if stmt is a bare string expression, else None."""
    if (
        isinstance(stmt, ast.Expr)
        and isinstance(stmt.value, ast.Constant)
        and isinstance(stmt.value.value, str)
    ):
        return stmt.value
    return None


def _exported(package_dir: Path, public: Iterable[str]) -> list[tuple[str, _Definition]]:
    """Resolve each public name through ``__init__.py`` to the place it is defined."""
    init = package_dir / "__init__.py"
    pairs = [(name, _resolve(package_dir, init, name)) for name in public]
    return [(name, definition) for name, definition in pairs if definition is not None]


def _value_docs(package_dir: Path, public: Iterable[str]) -> list[tuple[str, str | None]]:
    """Return each exported alias or constant with the string literal that documents it."""
    docs: list[tuple[str, str | None]] = []
    for name, definition in _exported(package_dir, public):
        if definition.is_class:
            continue
        literal = _string_literal(definition.following)
        docs.append((name, None if literal is None else str(literal.value)))
    return docs


def undocumented_attributes(package_dir: Path, public: Iterable[str]) -> list[str]:
    """Return exported type aliases, constants and class constants lacking documentation.

    Each public name is followed from ``package_dir/__init__.py`` through renamed imports and
    ``X = module.Y`` assignments to its definition, which is then judged by its kind. The
    package is read as syntax trees, not through ``inspect.getsource``, because the public
    classes are re-labelled to live in the top-level package. Every exported ``type`` alias
    and module-level constant must be followed by a string literal holding a summary and a
    runnable ``Example:``. Every ``ClassVar`` declared in the body of a public class,
    however ``typing.ClassVar`` is spelled, must be followed by a string literal (presence
    only).

    Example:
        >>> from pathlib import Path
        >>> package = Path("pkg")
        >>> package.mkdir()
        >>> _ = (package / "__init__.py").write_text("")
        >>> undocumented_attributes(package, ["X"])
        []
    """
    missing: list[str] = []
    documented = dict(_value_docs(package_dir, public))
    for name, doc in documented.items():
        if doc is None or not _is_complete(doc):
            missing.append(name)
    for name, definition in _exported(package_dir, public):
        node = definition.statement
        if not (definition.is_class and isinstance(node, ast.ClassDef)):
            continue
        for i, stmt in enumerate(node.body):
            if not _is_class_var(stmt, definition.imports):
                continue
            attr = (
                stmt.target.id
                if isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name)
                else None
            )
            nxt = node.body[i + 1] if i + 1 < len(node.body) else None
            if attr is not None and _string_literal(nxt) is None:
                missing.append(f"{name}.{attr}")
    return missing


def failing_attribute_examples(package_dir: Path, public: Iterable[str]) -> list[str]:
    """Return exported aliases and constants whose ``Example:`` does not run cleanly.

    Attribute docstrings are not collected by ``--doctest-modules``, so they are run here.
    The caller provides the isolation (working directory, colour) as for any other doctest.

    Example:
        >>> from pathlib import Path
        >>> package = Path("pkg")
        >>> package.mkdir()
        >>> _ = (package / "__init__.py").write_text("")
        >>> failing_attribute_examples(package, ["X"])
        []
    """
    parser = doctest.DocTestParser()
    failed: list[str] = []
    for name, doc in _value_docs(package_dir, public):
        if doc is None:
            continue
        test = parser.get_doctest(inspect.cleandoc(doc), {}, name, None, 0)
        runner = doctest.DocTestRunner()
        if runner.run(test, out=lambda _text: None).failed:
            failed.append(name)
    return failed
