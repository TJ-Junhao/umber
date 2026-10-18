"""Generate the API reference and synchronize the repository skill copy.

Hand-written Markdown lives in ``python/umber/_skill/umber``. Only
``references/api.md`` is generated there; every file is mirrored to ``skills/umber``.
Run with ``--check`` to print differences without changing either directory.
"""

import argparse
import ast
import builtins
import copy
import difflib
import doctest
import inspect
from pathlib import Path

import umber


def _name(node: ast.stmt) -> str:
    if isinstance(node, ast.ClassDef | ast.FunctionDef | ast.AsyncFunctionDef):
        return node.name
    if isinstance(node, ast.TypeAlias):
        return node.name.id
    if isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
        return node.target.id
    if isinstance(node, ast.Assign) and len(node.targets) == 1:
        target = node.targets[0]
        if isinstance(target, ast.Name):
            return target.id
    return ""


def _doc(body: list[ast.stmt], index: int) -> str:
    node = body[index]
    if isinstance(node, ast.ClassDef | ast.FunctionDef | ast.AsyncFunctionDef):
        return ast.get_docstring(node) or ""
    if index + 1 < len(body):
        following = body[index + 1]
        if (
            isinstance(following, ast.Expr)
            and isinstance(following.value, ast.Constant)
            and isinstance(following.value.value, str)
        ):
            return inspect.cleandoc(following.value.value)
    return ""


def _definition(
    package: Path, module: str, name: str, visited: tuple[tuple[str, str], ...] = ()
) -> tuple[str, ast.stmt, str]:
    key = (module, name)
    if key in visited:
        raise ValueError(f"Cyclic import for {module}.{name}")
    relative = module.split(".")[1:]
    path = package.joinpath(*relative).with_suffix(".py") if relative else package / "__init__.py"
    body = ast.parse(path.read_text(encoding="utf-8")).body
    for index in range(len(body) - 1, -1, -1):
        node = body[index]
        if _name(node) == name:
            return module, node, _doc(body, index)
        if isinstance(node, ast.ImportFrom):
            for alias in node.names:
                if (alias.asname or alias.name) != name:
                    continue
                origin = node.module or ""
                if node.level:
                    origin = ".".join(module.split(".")[: -node.level] + [origin])
                if origin == "umber" or origin.startswith("umber."):
                    return _definition(package, origin, alias.name, (*visited, key))
                return origin, node, ""
        if isinstance(node, ast.Import):
            for alias in node.names:
                if (alias.asname or alias.name.split(".")[0]) == name:
                    return alias.name if alias.asname else name, node, ""
    raise ValueError(f"Cannot find the source definition of {module}.{name}")


def _public_members(package: Path, name: str) -> list[tuple[str, ast.stmt, str]]:
    """List source-defined public members, including inherited ones, in name order."""
    classes: dict[tuple[str, str], ast.ClassDef] = {}

    def lineage(module: str, class_name: str) -> list[tuple[str, str]]:
        module, node, _ = _definition(package, module, class_name)
        if not isinstance(node, ast.ClassDef):
            return []
        key = (module, node.name)
        classes[key] = node
        parents: list[list[tuple[str, str]]] = []
        for base in node.bases:
            if isinstance(base, ast.Subscript):
                base = base.value
            if isinstance(base, ast.Name):
                if base.id in vars(builtins):
                    continue
                parents.append(lineage(module, base.id))
            elif isinstance(base, ast.Attribute):
                parts = ast.unparse(base).split(".")
                origin, _, _ = _definition(package, module, parts[0])
                if origin == "umber" or origin.startswith("umber."):
                    parents.append(lineage(".".join([origin, *parts[1:-1]]), parts[-1]))
        # C3 order keeps overrides correct even when two parents share a base.
        pending = [parent[:] for parent in parents if parent]
        pending.append([parent[0] for parent in parents if parent])
        result = [key]
        while pending := [order for order in pending if order]:
            for order in pending:
                candidate = order[0]
                if not any(candidate in other[1:] for other in pending):
                    break
            else:
                raise ValueError(f"Cannot resolve the inheritance order of {name}")
            result.append(candidate)
            for order in pending:
                if order[0] == candidate:
                    order.pop(0)
        return result

    members: dict[str, tuple[ast.stmt, str]] = {}
    for key in lineage("umber", name):
        for index, member in enumerate(classes[key].body):
            member_name = _name(member)
            if member_name and not member_name.startswith("_"):
                members.setdefault(member_name, (member, _doc(classes[key].body, index)))
    result: list[tuple[str, ast.stmt, str]] = []
    for member_name, (member, doc) in sorted(members.items()):
        if not doc.strip():
            raise ValueError(
                f"{name}.{member_name} has no docstring; every public member needs one."
            )
        result.append((member_name, member, doc))
    return result


def _signature(name: str, node: ast.stmt) -> str:
    if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef):
        result = f" -> {ast.unparse(node.returns)}" if node.returns else ""
        return f"{name}({ast.unparse(node.args)}){result}"
    if isinstance(node, ast.ClassDef):
        for member in node.body:
            if isinstance(member, ast.FunctionDef) and member.name == "__init__":
                doc = ast.get_docstring(member) or ""
                if not doctest.DocTestParser().get_examples(doc):
                    continue
                arguments = copy.deepcopy(member.args)
                arguments.args = arguments.args[1:]
                return f"{name}({ast.unparse(arguments)})"
        bases = ", ".join(ast.unparse(base) for base in node.bases)
        return f"class {name}" + (f"({bases})" if bases else "")
    if isinstance(node, ast.TypeAlias):
        return ast.unparse(node)
    if isinstance(node, ast.AnnAssign):
        return f"{name}: {ast.unparse(node.annotation)}"
    return name


def _markdown_doc(doc: str) -> str:
    """Fence runnable examples while keeping their expected output and surrounding prose."""
    parts: list[str] = []
    examples: list[str] = []

    def flush() -> None:
        if examples:
            parts.append("```python\n" + "".join(examples).rstrip() + "\n```")
            examples.clear()

    for part in doctest.DocTestParser().parse(doc):
        if isinstance(part, str):
            if part.strip():
                flush()
                parts.append(part.strip())
        else:
            lines = part.source.splitlines()
            examples.append(">>> " + lines[0] + "\n")
            examples.extend("... " + line + "\n" for line in lines[1:])
            examples.append(part.want)
    flush()
    return "\n\n".join(parts)


def _section(name: str, node: ast.stmt, doc: str, *, level: int) -> str:
    signature = _signature(name, node)
    return f"{'#' * level} `umber.{name}`\n\n```text\n{signature}\n```\n\n{_markdown_doc(doc)}\n"


def _reference(package: Path) -> str:
    sections = [
        "# umber API\n\n"
        f"Generated from umber {umber.__version__}: public exports, source signatures and "
        "docstrings. Regenerate with `python scripts/gen_skill.py`; do not edit this file.\n\n"
        "Signatures are shown as text; Python blocks are executable examples. "
        "Typed `self` parameters show which axes accept a method.\n"
    ]
    for name in umber.__all__:
        if name == "__version__":
            sections.append(
                "## `umber.__version__`\n\nThe installed package version.\n\n"
                "```python\n>>> import umber as ub\n>>> ub.__version__\n"
                f"{umber.__version__!r}\n```\n"
            )
            continue
        _, node, doc = _definition(package, "umber", name)
        sections.append(_section(name, node, doc, level=2))
        if isinstance(node, ast.ClassDef):
            for member_name, member, member_doc in _public_members(package, name):
                sections.append(_section(f"{name}.{member_name}", member, member_doc, level=3))
    return "\n".join(sections)


def _files(directory: Path) -> dict[Path, bytes]:
    return {
        path.relative_to(directory): path.read_bytes()
        for path in sorted(directory.rglob("*"))
        if path.is_file()
    }


def _difference(path: Path, actual: bytes, expected: bytes) -> None:
    try:
        before = actual.decode("utf-8").splitlines(keepends=True)
        after = expected.decode("utf-8").splitlines(keepends=True)
    except UnicodeDecodeError:
        print(f"Binary content differs: {path}")
        return
    difference = "".join(
        difflib.unified_diff(before, after, fromfile=str(path), tofile=f"{path} (generated)")
    )
    print(difference, end="" if difference.endswith("\n") else "\n")


def _generate(root: Path, *, check: bool) -> int:
    package = root / "python/umber"
    source = package / "_skill/umber"
    mirror = root / "skills/umber"
    actual = _files(source)
    if Path("SKILL.md") not in actual:
        raise ValueError(f"{source / 'SKILL.md'}: the hand-written skill is missing.")
    reference = Path("references/api.md")
    expected = {**actual, reference: _reference(package).encode("utf-8")}
    stale = False
    for directory, before, after in (
        (source, {reference: actual.get(reference, b"")}, {reference: expected[reference]}),
        (mirror, _files(mirror), expected),
    ):
        for relative in sorted(before.keys() | after.keys()):
            if relative in before and relative in after and before[relative] == after[relative]:
                continue
            stale = True
            path = directory / relative
            if check:
                _difference(
                    path.relative_to(root), before.get(relative, b""), after.get(relative, b"")
                )
            elif relative in after:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(after[relative])
            else:
                path.unlink()
    if check and stale:
        print(
            'The skill reference is out of date; run "uv run python scripts/gen_skill.py" '
            "and commit python/umber/_skill and skills/umber."
        )
    return int(check and stale)


def _main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="print differences without writing files"
    )
    arguments = parser.parse_args()
    return _generate(Path(__file__).resolve().parents[1], check=arguments.check)


if __name__ == "__main__":
    raise SystemExit(_main())
