from typing import Any, Callable

def call(
    address: str, input: Any, on_message: Callable[[str], bool] | None = None
) -> Any: ...

class MessageFn:
    """Sends a message to the client. Raises an exception if the client requested abort."""
    def __call__(self, msg: str) -> None: ...

def run_server(
    tool: Callable[[Any, MessageFn], Any], index_html: str | None = None
) -> None:
    """Start a tool server on 0.0.0.0:8080. Blocks until the process is killed.

    Can only be called once per process.

    Args:
        tool: A callable ``(input, send_msg) -> result``. Called for each client
            connection. ``send_msg`` sends a message string to the client and
            raises if the client requested abort.
        index_html: Optional HTML string served at the ``/`` route.
    """
    ...
