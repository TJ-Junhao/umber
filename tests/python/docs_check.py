"""Checks that enforce the documentation rules of spec section 10."""

import ast
import inspect
from collections.abc import Iterable
from pathlib import Path


def _own_doc(obj: object) -> str:
    """Return the docstring defined on obj itself, never one inherited from a base class."""
    if inspect.isclass(obj):
        doc = obj.__dict__.get("__doc__")
        return doc if isinstance(doc, str) else ""
    return inspect.getdoc(obj) or ""


def _is_complete(doc: str) -> bool:
    lines = [line.strip() for line in doc.strip().splitlines()]
    return bool(lines) and bool(lines[0]) and "Example:" in lines


def missing_docs(objects: Iterable[tuple[str, object]]) -> list[str]:
    """Return qualified names of objects lacking a summary line or an ``Example:`` section.

    For classes, every public method, property and staticmethod defined on the class
    itself is checked as well.
    """
    missing: list[str] = []
    for name, obj in objects:
        if not _is_complete(_own_doc(obj)):
            missing.append(name)
        if inspect.isclass(obj):
            for attr, member in vars(obj).items():
                if attr.startswith("_"):
                    continue
                target: object = member.fget if isinstance(member, property) else member
                # staticmethod and classmethod wrap the real function in ``__func__``.
                target = getattr(target, "__func__", target)
                if inspect.isfunction(target) and not _is_complete(inspect.getdoc(target) or ""):
                    missing.append(f"{name}.{attr}")
    return missing


def undocumented_enum_members(source_dir: Path) -> list[str]:
    """Return ``module.Enum.MEMBER`` for enum members not followed by a string literal."""
    missing: list[str] = []
    for path in sorted(source_dir.rglob("*.py")):
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if not isinstance(node, ast.ClassDef):
                continue
            bases = {ast.unparse(b) for b in node.bases}
            if not bases & {"Enum", "enum.Enum", "IntEnum", "enum.IntEnum"}:
                continue
            body = node.body
            for i, stmt in enumerate(body):
                if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1:
                    target = stmt.targets[0]
                    if isinstance(target, ast.Name) and not target.id.startswith("_"):
                        nxt = body[i + 1] if i + 1 < len(body) else None
                        documented = (
                            isinstance(nxt, ast.Expr)
                            and isinstance(nxt.value, ast.Constant)
                            and isinstance(nxt.value.value, str)
                        )
                        if not documented:
                            missing.append(f"{path.stem}.{node.name}.{target.id}")
    return missing
