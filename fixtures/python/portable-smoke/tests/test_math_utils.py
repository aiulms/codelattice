"""Tests for math_utils module."""

from sample_app.math_utils import add, multiply


def test_add():
    """Test add function."""
    assert add(1, 2) == 3
    assert add(-1, 1) == 0


def test_multiply():
    """Test multiply function."""
    assert multiply(2, 3) == 6
    assert multiply(0, 5) == 0


def test_add_with_multiply():
    """Test add combined with multiply."""
    result = add(multiply(2, 3), 4)
    assert result == 10
