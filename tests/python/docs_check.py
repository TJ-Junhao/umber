"""Checks that enforce the documentation rules for the public API."""

import ast
import doctest
import enum
import inspect
import textwrap
from collections.abc import Iterable
from functools import cached_property


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
    their own ``__doc__``, which is what hover and ``help()`` show.
    """
    missing: list[str] = []
    for name, obj in objects:
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
    literal right after the assignment, which is the form pyright shows on hover.
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
