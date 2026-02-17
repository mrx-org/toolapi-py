use pyo3::exceptions::PyException;
use pyo3::prelude::*;

// TODO: we should register new PyException classes for errors and use those in the code below

/// A Python module implemented in Rust.
#[pymodule]
mod _core {
    use super::*;

    #[pyfunction]
    #[pyo3(signature = (address, input, on_message=None))]
    fn call(
        py: Python<'_>,
        address: &str,
        input: toolapi::Value,
        on_message: Option<Py<PyAny>>,
    ) -> PyResult<toolapi::Value> {
        // Wraps the user callback, returns `true` (continue tool) if:
        // - no callback was provided
        // - callback returned true
        // Returns `false` (abort the tool) if:
        // - callback raised an exception
        // - return value was not a bool
        // - callback returned false
        let on_message = |msg: String| -> bool {
            match on_message.as_ref() {
                // User provided a callback: try to call it
                Some(func) => Python::attach(|py| {
                    match func.call1(py, (msg,)) {
                        // Call succeeded: convert result to bool (false on error)
                        Ok(ret) => ret.extract(py).unwrap_or(false),
                        // Callback raised an exception: stop tool
                        Err(_) => false,
                    }
                }),
                // No user callback: don't stop tool
                None => true,
            }
        };

        // Run the tool and return the result - pyo3 will convert Value to python
        py.detach(|| toolapi::call(address, input, on_message))
            .map_err(|err| PyException::new_err(format!("ToolCallError: {err}")))
    }
}
