use pyo3::types::{PyDict, PyList};
use pyo3::{IntoPyObjectExt, prelude::*};
use toolapi::value::structured::{InstantSeqEvent, PhantomTissue, Volume};
use toolapi::value::typed::{TypedDict, TypedList};

// TODO: we should register new PyException classes for errors and use those in the code below

/// A Python module implemented in Rust.
#[pymodule]
mod _core {
    use pyo3::exceptions::PyException;

    use super::*;

    #[pyfunction]
    #[pyo3(signature = (address, input, on_message=None))]
    fn call(
        py: Python<'_>,
        address: &str,
        input: toolapi::Value,
        on_message: Option<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
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

        let result = py
            .detach(|| toolapi::call(address, input, on_message))
            .map_err(|err| PyException::new_err(format!("ToolCallError: {err}")));

        result.and_then(|value| value_to_obj(py, value))
    }
}

// =============================================================================
// Rust Value -> Python conversion
// =============================================================================

// TODO: could implement IntoPyObject in `toolapi` under `pyo3` feature flag

fn value_to_obj(py: Python<'_>, value: toolapi::Value) -> PyResult<Py<PyAny>> {
    match value {
        toolapi::Value::None(()) => Ok(py.None()),
        toolapi::Value::Bool(b) => b.into_py_any(py),
        toolapi::Value::Int(i) => i.into_py_any(py),
        toolapi::Value::Float(f) => f.into_py_any(py),
        toolapi::Value::Str(s) => s.into_py_any(py),
        toolapi::Value::Complex(c) => c.into_py_any(py),

        toolapi::Value::Vec3(v) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("Vec3")?;
            cls.call1((v.0.to_vec(),)).map(|o| o.unbind())
        }
        toolapi::Value::Vec4(v) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("Vec4")?;
            cls.call1((v.0.to_vec(),)).map(|o| o.unbind())
        }

        toolapi::Value::InstantSeqEvent(event) => instant_seq_event_to_obj(py, event),

        toolapi::Value::Volume(vol) => volume_to_obj(py, vol),

        toolapi::Value::PhantomTissue(pt) => phantom_tissue_to_obj(py, pt),

        toolapi::Value::SegmentedPhantom(sp) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("SegmentedPhantom")?;
            let tissues = PyList::empty(py);
            for t in sp.tissues {
                let obj = phantom_tissue_to_obj(py, t)?;
                tissues.append(obj)?;
            }
            let b1_tx = PyList::empty(py);
            for v in sp.b1_tx {
                let obj = volume_to_obj(py, v)?;
                b1_tx.append(obj)?;
            }
            let b1_rx = PyList::empty(py);
            for v in sp.b1_rx {
                let obj = volume_to_obj(py, v)?;
                b1_rx.append(obj)?;
            }
            cls.call1((tissues, b1_tx, b1_rx)).map(|o| o.unbind())
        }

        toolapi::Value::Dict(d) => {
            let dict = PyDict::new(py);
            for (key, value) in d.0.into_iter() {
                let obj = value_to_obj(py, value)?;
                dict.set_item(key, obj)?;
            }
            dict.into_py_any(py)
        }
        toolapi::Value::List(l) => {
            let list = PyList::empty(py);
            for item in l.0 {
                let obj = value_to_obj(py, item)?;
                list.append(obj)?;
            }
            list.into_py_any(py)
        }

        toolapi::Value::TypedList(tl) => typed_list_to_obj(py, tl),
        toolapi::Value::TypedDict(td) => typed_dict_to_obj(py, td),
    }
}

fn instant_seq_event_to_obj(py: Python<'_>, event: InstantSeqEvent) -> PyResult<Py<PyAny>> {
    let module = py.import("toolapi.value")?;
    let cls = module.getattr("InstantSeqEvent")?;
    match event {
        InstantSeqEvent::Pulse { angle, phase } => cls
            .call_method1("Pulse", (angle, phase))
            .map(|o| o.unbind()),
        InstantSeqEvent::Fid { kt } => {
            // Build a Vec4 wrapper for the kt field
            let vec4_cls = module.getattr("Vec4")?;
            let kt_obj = vec4_cls.call1((kt.0.to_vec(),))?;
            cls.call_method1("Fid", (kt_obj,)).map(|o| o.unbind())
        }
        InstantSeqEvent::Adc { phase } => cls.call_method1("Adc", (phase,)).map(|o| o.unbind()),
    }
}

fn volume_to_obj(py: Python<'_>, vol: Volume) -> PyResult<Py<PyAny>> {
    let module = py.import("toolapi.value")?;
    let cls = module.getattr("Volume")?;
    let shape = vol.shape.to_vec();
    let affine: Vec<Vec<f64>> = vol.affine.iter().map(|row| row.to_vec()).collect();
    let data = typed_list_to_py_list(py, vol.data)?;
    cls.call1((shape, affine, data)).map(|o| o.unbind())
}

fn phantom_tissue_to_obj(py: Python<'_>, pt: PhantomTissue) -> PyResult<Py<PyAny>> {
    let module = py.import("toolapi.value")?;
    let cls = module.getattr("PhantomTissue")?;
    let density = volume_to_obj(py, pt.density)?;
    let db0 = volume_to_obj(py, pt.db0)?;
    cls.call1((density, db0, pt.t1, pt.t2, pt.t2dash, pt.adc))
        .map(|o| o.unbind())
}

/// Convert a TypedList into a plain Python list.
fn typed_list_to_py_list(py: Python<'_>, tl: TypedList) -> PyResult<Py<PyList>> {
    let list = match tl {
        TypedList::None(v) => {
            let l = PyList::empty(py);
            for _ in v {
                l.append(py.None())?;
            }
            l
        }
        TypedList::Bool(v) => PyList::new(py, v)?,
        TypedList::Int(v) => PyList::new(py, v)?,
        TypedList::Float(v) => PyList::new(py, v)?,
        TypedList::Str(v) => PyList::new(py, v)?,
        TypedList::Complex(v) => PyList::new(py, v)?,
        TypedList::Vec3(v) => {
            let l = PyList::empty(py);
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("Vec3")?;
            for item in v {
                l.append(cls.call1((item.0.to_vec(),))?)?;
            }
            l
        }
        TypedList::Vec4(v) => {
            let l = PyList::empty(py);
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("Vec4")?;
            for item in v {
                l.append(cls.call1((item.0.to_vec(),))?)?;
            }
            l
        }
        TypedList::InstantSeqEvent(v) => {
            let l = PyList::empty(py);
            for item in v {
                l.append(instant_seq_event_to_obj(py, item)?)?;
            }
            l
        }
        TypedList::Volume(v) => {
            let l = PyList::empty(py);
            for item in v {
                l.append(volume_to_obj(py, item)?)?;
            }
            l
        }
        TypedList::SegmentedPhantom(v) => {
            let l = PyList::empty(py);
            for item in v {
                l.append(value_to_obj(py, toolapi::Value::SegmentedPhantom(item))?)?;
            }
            l
        }
        TypedList::PhantomTissue(v) => {
            let l = PyList::empty(py);
            for item in v {
                l.append(phantom_tissue_to_obj(py, item)?)?;
            }
            l
        }
    };
    Ok(list.unbind())
}

/// Convert a TypedList to a top-level Python object (for Value::TypedList).
fn typed_list_to_obj(py: Python<'_>, tl: TypedList) -> PyResult<Py<PyAny>> {
    typed_list_to_py_list(py, tl).and_then(|l| l.into_py_any(py))
}

/// Convert a TypedDict to a Python dict.
fn typed_dict_to_obj(py: Python<'_>, td: TypedDict) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    match td {
        TypedDict::None(m) => {
            for (k, _) in m {
                dict.set_item(k, py.None())?;
            }
        }
        TypedDict::Bool(m) => {
            for (k, v) in m {
                dict.set_item(k, v)?;
            }
        }
        TypedDict::Int(m) => {
            for (k, v) in m {
                dict.set_item(k, v)?;
            }
        }
        TypedDict::Float(m) => {
            for (k, v) in m {
                dict.set_item(k, v)?;
            }
        }
        TypedDict::Str(m) => {
            for (k, v) in m {
                dict.set_item(k, v)?;
            }
        }
        TypedDict::Complex(m) => {
            for (k, v) in m {
                dict.set_item(k, v)?;
            }
        }
        TypedDict::Vec3(m) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("Vec3")?;
            for (k, v) in m {
                dict.set_item(k, cls.call1((v.0.to_vec(),))?)?;
            }
        }
        TypedDict::Vec4(m) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("Vec4")?;
            for (k, v) in m {
                dict.set_item(k, cls.call1((v.0.to_vec(),))?)?;
            }
        }
        TypedDict::InstantSeqEvent(m) => {
            for (k, v) in m {
                dict.set_item(k, instant_seq_event_to_obj(py, v)?)?;
            }
        }
        TypedDict::Volume(m) => {
            for (k, v) in m {
                dict.set_item(k, volume_to_obj(py, v)?)?;
            }
        }
        TypedDict::SegmentedPhantom(m) => {
            for (k, v) in m {
                dict.set_item(k, value_to_obj(py, toolapi::Value::SegmentedPhantom(v))?)?;
            }
        }
        TypedDict::PhantomTissue(m) => {
            for (k, v) in m {
                dict.set_item(k, phantom_tissue_to_obj(py, v)?)?;
            }
        }
    }
    dict.into_py_any(py)
}
