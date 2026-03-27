"""Python module docs."""

# Module context that should stay.

DEFAULT_NAME = "treebrief"


@cache
def top_level(
    value: int,
) -> int:
    """Handle the top-level case."""
    # Normalize before branching.
    def nested(value: int) -> int:
        # Leave the nested exit visible.
        return value + 1

    if value < 0:
        raise ValueError("negative")

    return nested(value)


def stream_names(names: list[str]):
    # Yield the normalized values.
    for name in names:
        yield name.upper()


class Greeter:
    """Greeter docs."""

    async def load_many(
        self,
        names: list[str],
    ) -> list[str]:
        """Format several names."""
        # Keep the final result visible.
        return [name.upper() for name in names]
