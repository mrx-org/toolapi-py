from typing import Any, Callable

def call(
    address: str, input: Any, on_message: Callable[[str], bool] | None = None
) -> Any: ...
