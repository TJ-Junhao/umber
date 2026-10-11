import umber


def test_public_names() -> None:
    assert umber.__all__ == ["DataError", "StyleError", "UmberError", "UsageError", "__version__"]
