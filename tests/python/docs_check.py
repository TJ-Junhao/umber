"""Checks that enforce the documentation rules for the public API."""

import ast
import doctest
import enum
import inspect
import textwrap
from collections.abc import Iterable
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


def undocumented_enum_members(objects: Iterable[tuple[str, object]]) -> list[str]:
    """Return ``<name>.MEMBER`` for enum members not followed by a string literal.

    Enums are found by their runtime type, not by how their base class is spelled, so
    ``from enum import Enum as E`` is handled. Each member's docstring must be a string
    literal right after the assignment, which is the form pyright shows on hover. An
    annotated member (``X: int = 1``) is reported too: enum does not treat it as a member.
    """
    missing: list[str] = []
    for name, obj in objects:
        if not (inspect.isclass(obj) and issubclass(obj, enum.Enum)):
            continue
        tree = ast.parse(textwrap.dedent(inspect.getsource(obj)))
        node = tree.body[0]
        if not isinstance(node, ast.ClassDef):
            continue
        body = node.body
        for i, stmt in enumerate(body):
            if isinstance(stmt, ast.AnnAssign):
                # An annotated name in an enum body is not a member, so it is never valid.
                if isinstance(stmt.target, ast.Name) and not stmt.target.id.startswith("_"):
                    missing.append(f"{name}.{stmt.target.id}")
                continue
            if not (isinstance(stmt, ast.Assign) and len(stmt.targets) == 1):
                continue
            target = stmt.targets[0]
            if not isinstance(target, ast.Name) or target.id.startswith("_"):
                continue
            nxt = body[i + 1] if i + 1 < len(body) else None
            documented = (
                isinstance(nxt, ast.Expr)
                and isinstance(nxt.value, ast.Constant)
                and isinstance(nxt.value.value, str)
            )
            if not documented:
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


def _assigned_name(stmt: ast.stmt) -> str | None:
    """Return the single name a module-level alias, constant or class attribute binds."""
    if isinstance(stmt, ast.TypeAlias):
        return stmt.name.id
    if isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name):
        return stmt.target.id
    if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1:
        target = stmt.targets[0]
        if isinstance(target, ast.Name):
            return target.id
    return None


def _is_class_var(stmt: ast.stmt) -> bool:
    """Return True if stmt is an annotated assignment whose annotation is ``ClassVar``."""
    if not isinstance(stmt, ast.AnnAssign):
        return False
    annotation = stmt.annotation
    if isinstance(annotation, ast.Subscript):
        annotation = annotation.value
    if isinstance(annotation, ast.Attribute):
        return annotation.attr == "ClassVar"
    return isinstance(annotation, ast.Name) and annotation.id == "ClassVar"


def undocumented_attributes(package_dir: Path, public: Iterable[str]) -> list[str]:
    """Return exported type aliases, constants and class constants lacking documentation.

    The package files are read as syntax trees, not through ``inspect.getsource``, because
    the public classes are re-labelled to live in the top-level package. Every exported
    ``type`` alias and module-level constant must be followed by a string literal holding
    a summary and a runnable ``Example:``. Every ``ClassVar`` declared in the body of a
    public class must be followed by a string literal (presence only).

    Example:
        >>> from pathlib import Path
        >>> undocumented_attributes(Path("no-such-directory"), ["X"])
        []
    """
    exported = set(public)
    missing: list[str] = []
    for path in sorted(package_dir.glob("*.py")):
        body = ast.parse(path.read_text(encoding="utf-8")).body
        for i, stmt in enumerate(body):
            nxt = body[i + 1] if i + 1 < len(body) else None
            if isinstance(stmt, ast.ClassDef):
                if stmt.name in exported:
                    missing.extend(_undocumented_class_vars(stmt))
                continue
            name = _assigned_name(stmt)
            if name is None or name not in exported:
                continue
            literal = _string_literal(nxt)
            if literal is None or not _is_complete(str(literal.value)):
                missing.append(name)
    return missing


def _undocumented_class_vars(node: ast.ClassDef) -> list[str]:
    """Return ``<class>.<name>`` for ClassVar declarations not followed by a string literal."""
    missing: list[str] = []
    for i, stmt in enumerate(node.body):
        if not _is_class_var(stmt):
            continue
        name = _assigned_name(stmt)
        nxt = node.body[i + 1] if i + 1 < len(node.body) else None
        if name is not None and _string_literal(nxt) is None:
            missing.append(f"{node.name}.{name}")
    return missing
