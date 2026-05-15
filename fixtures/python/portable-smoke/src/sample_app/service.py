"""Service module for CodeLattice Python Phase A testing."""

from sample_app.math_utils import add
from sample_app.math_utils import multiply as mul


class UserService:
    """User service class for testing."""

    def __init__(self, name: str):
        """Initialize user service."""
        self.name = name
        self._internal_id = add(id(self), 0)

    def run(self) -> str:
        """Run the service."""
        result = mul(len(self.name), 2)
        return f"{self.name}: {result}"

    def _private_helper(self) -> int:
        """Private helper method."""
        return add(1, 2)


async def fetch_data(url: str) -> dict:
    """Fetch data asynchronously."""
    return {"url": url, "status": "ok"}


def process_items(items: list) -> list:
    """Process items using add."""
    return [add(item, 1) for item in items]
