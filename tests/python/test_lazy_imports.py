"""Public exports and native exception types survive globally lazy imports."""

import subprocess
import sys

import pytest


@pytest.mark.skipif(sys.version_info < (3, 15), reason="forced lazy imports require Python 3.15+")
def test_lazy_imports_in_fresh_interpreter() -> None:
    # https://docs.python.org/3.15/using/cmdline.html#cmdoption-X
    result = subprocess.run(
        [
            sys.executable,
            "-X",
            "lazy_imports=all",
            "-c",
            """
import sys
import types
import umber as ub

# https://docs.python.org/3.15/library/sys.html#sys.get_lazy_imports
assert sys.get_lazy_imports() == "all"
assert sys.get_lazy_imports_filter() is None
# Dictionary access inspects the proxy without resolving it (PEP 810).
# https://docs.python.org/3.15/library/types.html#types.LazyImportType
assert type(globals()["ub"]) is types.LazyImportType
assert ub.Figure.__module__ == "umber"
assert ub.line.__module__ == "umber"
assert ub.Figure.to_svg.__module__ == "umber"
assert isinstance(ub.__version__, str)
assert ub.MAX_BINS == 10000
# Check an import inside umber too, before rendering resolves it.
assert type(vars(sys.modules["umber._figure"])["_axes"]) is types.LazyImportType
fig = ub.line([0, 1], [1, 2]).figure
assert "<svg" in fig.to_svg()
assert type(vars(sys.modules["umber._figure"])["_axes"]) is types.ModuleType
try:
    ub.line([0, 1], [1, float("inf")])
except ub.DataError as error:
    assert isinstance(error, ub.UmberError)
    assert error.__notes__[-1] == "hint: use NaN to leave a gap in the line."
else:
    raise AssertionError("invalid data was accepted")
""",
        ],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
