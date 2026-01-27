use pyo3::exceptions::PyTypeError;
use pyo3::types::PyDict;
use pyo3::{IntoPyObjectExt, prelude::*};

/// A Python module implemented in Rust.
#[pymodule]
mod _core {
    use pyo3::exceptions::PyException;

    use super::*;

    #[pyfunction]
    #[pyo3(signature = (address, on_message=None, /, **kwargs))]
    fn call(
        py: Python<'_>,
        address: &str,
        on_message: Option<Py<PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyDict>> {
        let inputs = kwargs_to_value_dict(py, kwargs)?;

        let on_message = |msg: String| -> bool {
            // no on_message: continue. Exception or wrong return type: break.
            if let Some(func) = on_message.as_ref() {
                Python::attach(|py| {
                    func.call1(py, (msg,))
                        .map_or(false, |ret| ret.extract(py).unwrap_or(false))
                })
            } else {
                true
            }
        };

        let result = py
            .detach(|| toolapi::call(address, inputs, on_message))
            // TODO: if done right we should create new python exception classes for this
            .map_err(|err| PyException::new_err(format!("ToolCallError: {err}")));

        result.and_then(|values| {
            let dict = PyDict::new(py);
            for (key, value) in values.into_iter() {
                let obj = value_to_obj(py, value)?;
                dict.set_item(key, obj)?;
            }
            Ok(dict.unbind())
        })
    }
}

fn kwargs_to_value_dict(
    py: Python<'_>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<toolapi::ValueDict> {
    kwargs.map_or_else(
        || Ok(toolapi::ValueDict::from([] as [(String, toolapi::Value); 0])),
        |dict| {
            dict.iter()
                .map(|(key, value)| {
                    let key: String = key.extract()?;
                    let value = obj_to_value(py, value.into_py_any(py)?)?;
                    Ok::<_, PyErr>((key, value))
                })
                .collect::<PyResult<toolapi::ValueDict>>()
        },
    )
}

fn obj_to_value(_py: Python<'_>, obj: Py<PyAny>) -> PyResult<toolapi::Value> {
    Python::attach(|py| {
        let obj = obj.bind(py);
        if obj.is_none() {
            Ok(toolapi::Value::None(()))
        } else if let Ok(b) = obj.extract::<bool>() {
            Ok(toolapi::Value::Bool(b))
        } else if let Ok(i) = obj.extract::<i64>() {
            Ok(toolapi::Value::Int(i))
        } else if let Ok(f) = obj.extract::<f64>() {
            Ok(toolapi::Value::Float(f))
        } else if let Ok(s) = obj.extract::<String>() {
            Ok(toolapi::Value::String(s))
        } else {
            Err(PyTypeError::new_err(format!(
                "unsupported Python type for Value conversion: {}",
                obj.get_type().name()?
            )))
        }
    })
}

fn value_to_obj(py: Python<'_>, value: toolapi::Value) -> PyResult<Py<PyAny>> {
    match value {
        toolapi::Value::None(()) => Ok(py.None()),
        toolapi::Value::Bool(b) => b.into_py_any(py),
        toolapi::Value::Int(i) => i.into_py_any(py),
        toolapi::Value::Float(f) => f.into_py_any(py),
        toolapi::Value::String(s) => s.into_py_any(py),
        _ => Err(PyTypeError::new_err(
            "unsupported Value type for Python conversion".to_string(),
        )),
    }
}
