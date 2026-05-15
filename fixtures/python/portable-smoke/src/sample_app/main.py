"""Main entry point for CodeLattice Python Phase A testing."""

from sample_app.math_utils import add, multiply
from sample_app.service import UserService, fetch_data


def main() -> None:
    """Main function."""
    result = add(1, 2)
    product = multiply(3, 4)
    user = UserService("alice")
    user.run()
    print(f"Result: {result}, Product: {product}")


if __name__ == "__main__":
    main()
