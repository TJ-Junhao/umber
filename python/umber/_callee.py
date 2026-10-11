"""Which function a call in the caller's source invokes, proven without running code.

Names are read from the caller's frame and attributes are looked up with
``inspect.getattr_static``, which calls no descriptors and no ``__getattr__``. A call counts
as a direct call of a function only when the arguments written in the source are the
arguments the function receives, after the ``self`` or ``cls`` the call binds. Anything
that cannot be shown this way, such as a ``functools.partial``, a wrapper stored on an
instance or an unbound method called through its class, is not a direct call.

A receiver that is itself a call is resolved through the return annotation of the function
it calls, when that function belongs to the same package as the reporting function and is
declared to return ``Self`` or its own class. Methods that return ``Self`` make chains such
as ``ax.line(t, y).xlim(0, 1)`` the usual style.
"""

import ast
import inspect
import os
import sys
import typing
from types import CodeType, FrameType, FunctionType, MethodType, ModuleType
from typing import cast

_MISSING = object()
_SELF: object = typing.Self
_MAX_CHAIN = 32
"""The longest chain of calls followed through return annotations."""


type _Proven = tuple[FunctionType, type | None]
"""A function a call provably invokes, and the class of the object it binds, if any."""


def calls(func: ast.expr, frame: FrameType, target: CodeType) -> bool:
    """Return whether calling ``func`` in ``frame`` provably calls ``target`` directly.

    Args:
        func: The callee expression of a call in the source of ``frame``.
        frame: The frame the call runs in.
        target: The code of the function the call must invoke.

    Returns:
        True when the call passes its arguments straight to ``target``.
    """
    package = _directory(target.co_filename)
    if package is None:
        return False
    proven = _proven(func, frame, package, _MAX_CHAIN)
    return proven is not None and proven[0].__code__ is target


def _proven(func: ast.expr, frame: FrameType, package: str, depth: int) -> _Proven | None:
    """Return the function ``func`` provably invokes when called, or ``None``."""
    if isinstance(func, ast.Name):
        return _proven_value(_variable(frame, func.id))
    if not isinstance(func, ast.Attribute):
        return None
    if isinstance(func.value, ast.Call):
        if depth <= 0:
            return None
        receiver = _returned_class(func.value, frame, package, depth - 1)
        return None if receiver is None else _proven_member(receiver, func.attr, instance=True)
    owner = _resolve(func.value, frame)
    if owner is _MISSING:
        return None
    if isinstance(owner, ModuleType):
        return _proven_value(_static_attribute(owner, func.attr))
    if isinstance(owner, type):
        return _proven_member(owner, func.attr, instance=False)
    # Through an instance, the member must come from its class; an instance attribute could
    # be a wrapper.
    if _static_attribute(owner, func.attr) is not _static_attribute(type(owner), func.attr):
        return None
    return _proven_member(type(owner), func.attr, instance=True)


def _proven_value(value: object) -> _Proven | None:
    """Return what calling ``value``, read from a variable or a module, invokes."""
    if isinstance(value, type):
        init = _static_attribute(value, "__init__")
        return (init, value) if isinstance(init, FunctionType) else None
    if isinstance(value, MethodType):
        function: object = value.__func__
        owner: object = value.__self__
        bound_to = owner if isinstance(owner, type) else type(owner)
        return (function, bound_to) if isinstance(function, FunctionType) else None
    if isinstance(value, FunctionType) and not _is_method(value):
        return (value, None)
    return None


def _proven_member(cls: type, name: str, *, instance: bool) -> _Proven | None:
    """Return what calling attribute ``name`` of ``cls``, or of an instance of it, invokes."""
    member = _static_attribute(cls, name)
    if isinstance(member, classmethod | staticmethod):
        wrapper = cast("classmethod[object, ..., object] | staticmethod[..., object]", member)
        function: object = wrapper.__func__
        if not isinstance(function, FunctionType):
            return None
        return (function, cls if isinstance(member, classmethod) else None)
    if isinstance(member, FunctionType):
        # Through an instance a method is bound to it; through the class nothing is bound,
        # so only a plain function fits.
        if instance and _is_method(member):
            return (member, cls)
        if not instance and not _is_method(member):
            return (member, None)
        return None
    if isinstance(member, type) and not instance:
        return _proven_value(member)
    return None


def _returned_class(call: ast.Call, frame: FrameType, package: str, depth: int) -> type | None:
    """Return the class of the object ``call`` returns, from its function's annotation."""
    proven = _proven(call.func, frame, package, depth)
    if proven is None:
        return None
    function, bound_to = proven
    if _directory(function.__code__.co_filename) != package:
        return None
    annotation = _return_annotation(function)
    if annotation is _SELF or (isinstance(annotation, str) and annotation == "Self"):
        return bound_to
    if isinstance(annotation, str):
        # A string is never evaluated; only the name of the function's own class counts.
        defining = _defining_class(bound_to, function)
        return defining if defining is not None and annotation == defining.__name__ else None
    if isinstance(annotation, type):
        return annotation
    origin = typing.get_origin(annotation)  # Axes[X, float] names Axes
    return origin if isinstance(origin, type) else None


def _return_annotation(function: FunctionType) -> object:
    """Return the return annotation of ``function`` without evaluating string annotations."""
    try:
        if sys.version_info >= (3, 14):
            import annotationlib

            # Names that are not defined at run time stay unresolved instead of failing.
            annotations = annotationlib.get_annotations(
                function, format=annotationlib.Format.FORWARDREF
            )
        else:
            annotations = inspect.get_annotations(function)
    except Exception:
        return _MISSING
    return annotations.get("return", _MISSING)


def _defining_class(cls: type | None, function: FunctionType) -> type | None:
    """Return the class in the method resolution order of ``cls`` that defines ``function``."""
    if cls is None:
        return None
    for klass in cls.__mro__:
        for value in vars(klass).values():
            if isinstance(value, classmethod | staticmethod):
                wrapper = cast(
                    "classmethod[object, ..., object] | staticmethod[..., object]", value
                )
                value = wrapper.__func__
            if value is function:
                return klass
    return None


def _is_method(function: FunctionType) -> bool:
    """Return whether ``function`` is defined in a class body (its qualified name has a dot)."""
    return function.__qualname__ != function.__name__


def _resolve(node: ast.expr, frame: FrameType) -> object:
    """Return the value of a dotted name such as ``ub.Color`` without running code."""
    if isinstance(node, ast.Name):
        return _variable(frame, node.id)
    if isinstance(node, ast.Attribute):
        owner = _resolve(node.value, frame)
        return _MISSING if owner is _MISSING else _static_attribute(owner, node.attr)
    return _MISSING


def _variable(frame: FrameType, name: str) -> object:
    """Return the value ``name`` has in ``frame``: local, enclosing, global or builtin."""
    for namespace in (frame.f_locals, frame.f_globals, frame.f_builtins):
        if name in namespace:
            return namespace[name]
    return _MISSING


def _static_attribute(owner: object, name: str) -> object:
    """Return ``owner.name`` as stored, without calling descriptors or ``__getattr__``."""
    try:
        return inspect.getattr_static(owner, name)
    except AttributeError:
        return _MISSING


def _directory(filename: str) -> str | None:
    """Return the real directory of a source file, or ``None`` when it has no file."""
    if filename.startswith("<"):
        return None
    try:
        return os.path.dirname(os.path.realpath(filename))
    except (OSError, ValueError):
        return None
