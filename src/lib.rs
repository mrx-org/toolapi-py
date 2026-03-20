use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
mod _core {
    use super::*;
    use pyo3::exceptions::PyException;

    #[pyfunction]
    #[pyo3(signature = (address, input, on_message=None))]
    fn call(
        py: Python<'_>,
        address: &str,
        input: toolapi::Value,
        on_message: Option<Py<PyAny>>,
    ) -> PyResult<toolapi::Value> {
        // Store the first on_message exception (if any) to re-raise later.
        // Mutex is needed because py.detach() requires Send.
        let on_message_err = std::sync::Mutex::new(None::<String>);

        let on_message = |msg: String| -> bool {
            match on_message.as_ref() {
                Some(func) => Python::attach(|py| {
                    match func.call1(py, (msg,)) {
                        Ok(ret) => ret.extract(py).unwrap_or(false),
                        Err(err) => {
                            // Store exception message, then abort the tool
                            let mut stored = on_message_err.lock().unwrap();
                            if stored.is_none() {
                                *stored = Some(format!("{err}"));
                            }
                            false
                        }
                    }
                }),
                None => true,
            }
        };

        let result = py.detach(|| toolapi::call(address, input, on_message));

        // If on_message raised, surface that exception instead of generic abort
        if let Some(err_msg) = on_message_err.into_inner().unwrap() {
            return Err(PyException::new_err(err_msg));
        }

        result.map_err(|err| PyException::new_err(format!("ToolCallError: {err}")))
    }

    // =========================================================================
    // Server
    // =========================================================================

    use std::sync::OnceLock;

    /// Global storage for the Python tool callback. Only one server per process.
    static TOOL_CALLBACK: OnceLock<Py<PyAny>> = OnceLock::new();

    /// Trampoline function with the `ToolFn` signature (`fn(Value, &mut MessageFn)`)
    /// that reads the Python callback from the global and calls it.
    fn tool_trampoline(
        input: toolapi::Value,
        send_msg: &mut toolapi::MessageFn,
    ) -> Result<toolapi::Value, toolapi::ToolError> {
        let func = TOOL_CALLBACK.get().expect("TOOL_CALLBACK not initialized");

        Python::attach(|py| {
            // Wrap send_msg as a Python-callable object. We use a raw pointer
            // because send_msg is a borrowed &mut dyn FnMut which we can't move
            // into a pyclass. This is safe because the PySendMsg is only usable
            // within this scope while send_msg is alive.
            let send_msg_ptr = send_msg as *mut toolapi::MessageFn;
            let py_send_msg = Py::new(
                py,
                PySendMsg {
                    ptr: Some(send_msg_ptr),
                },
            )
            .expect("failed to create PySendMsg");

            // Call the Python tool: tool(input, send_msg) -> result
            let result = func.call1(py, (input, &py_send_msg));

            // Invalidate the pointer so it can't be called after this scope
            py_send_msg.borrow_mut(py).ptr = None;

            match result {
                Ok(obj) => {
                    // Convert the Python return value back to a toolapi::Value
                    obj.extract::<toolapi::Value>(py)
                        .map_err(|err| toolapi::ToolError::Custom(format!("{err}")))
                }
                Err(err) => Err(toolapi::ToolError::Custom(format!("{err}"))),
            }
        })
    }

    /// Python-callable wrapper around the Rust `MessageFn`.
    ///
    /// Holds a raw pointer to the `&mut MessageFn` which is only valid during
    /// the tool invocation. Raises a RuntimeError if called after the tool
    /// returns (pointer set to None).
    #[pyclass(name = "MessageFn")]
    struct PySendMsg {
        ptr: Option<*mut toolapi::MessageFn>,
    }

    // SAFETY: PySendMsg is only used within a single spawn_blocking thread.
    // The pointer is valid for the duration of tool_trampoline's scope and is
    // set to None before that scope exits.
    unsafe impl Send for PySendMsg {}
    unsafe impl Sync for PySendMsg {}

    #[pymethods]
    impl PySendMsg {
        fn __call__(&self, msg: String) -> PyResult<()> {
            let send_msg = unsafe {
                self.ptr
                    .ok_or_else(|| PyException::new_err("send_msg called outside of tool scope"))?
                    .as_mut()
                    .ok_or_else(|| PyException::new_err("send_msg: null pointer"))?
            };
            send_msg(msg).map_err(|err| PyException::new_err(format!("Tool aborted: {err}")))
        }
    }

    /// Start a tool server, running `tool` for every requesting client.
    ///
    /// This blocks forever (until the process is killed). Only one server can
    /// be started per process.
    ///
    /// Args:
    ///     tool: A callable with signature `(input, send_msg) -> result`.
    ///         `input` is the value sent by the client, `send_msg` is a callable
    ///         that sends a message string to the client (raises on abort).
    ///     index_html: Optional HTML string served at the `/` route.
    #[pyfunction]
    #[pyo3(signature = (tool, index_html=None))]
    fn run_server(py: Python<'_>, tool: Py<PyAny>, index_html: Option<String>) -> PyResult<()> {
        // Store the callback globally — fails if run_server was already called
        TOOL_CALLBACK
            .set(tool)
            .map_err(|_| PyException::new_err("run_server can only be called once per process"))?;

        // Leak the string to get a &'static str (fine — server runs until exit)
        let index_html: Option<&'static str> = index_html.map(|s| &*Box::leak(s.into_boxed_str()));

        // Release the GIL and block on the server
        py.detach(|| toolapi::run_server(tool_trampoline, index_html))
            .map_err(|err| PyException::new_err(format!("ServerError: {err}")))
    }
}
