use std::collections::HashMap;

use num_complex::Complex64;
use pyo3::exceptions::PyTypeError;
use pyo3::types::{PyDict, PyList};
use pyo3::{prelude::*, IntoPyObjectExt};
use toolapi::value::atomic::{Vec3, Vec4};
use toolapi::value::dynamic::{Dict, List};
use toolapi::value::structured::{InstantSeqEvent, PhantomTissue, SegmentedPhantom, Volume};
use toolapi::value::typed::{TypedDict, TypedList};

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
        input: Py<PyAny>,
        on_message: Option<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let input = obj_to_value(py, input)?;

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
            .detach(|| toolapi::call(address, input, on_message))
            // TODO: if done right we should create new python exception classes for this
            .map_err(|err| PyException::new_err(format!("ToolCallError: {err}")));

        result.and_then(|value| value_to_obj(py, value))
    }
}

// =============================================================================
// Python -> Rust Value conversion
// =============================================================================

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
            Ok(toolapi::Value::Str(s))
        } else if let Ok(c) = obj.extract::<Complex64>() {
            Ok(toolapi::Value::Complex(c))
        } else if obj.is_instance_of::<PyDict>() {
            obj_to_dict(py, obj)
        } else if obj.is_instance_of::<PyList>() {
            obj_to_list(py, obj)
        } else if let Ok(type_name) = obj.get_type().name().map(|n| n.to_string()) {
            match type_name.as_str() {
                "Vec3" => obj_to_vec3(obj),
                "Vec4" => obj_to_vec4(obj),
                "Volume" => obj_to_volume(py, obj),
                "PhantomTissue" => obj_to_phantom_tissue(py, obj),
                "SegmentedPhantom" => obj_to_segmented_phantom(py, obj),
                "InstantSeqEvent" => obj_to_instant_seq_event(py, obj),
                other => Err(PyTypeError::new_err(format!(
                    "unknown toolapi value type: {other}"
                ))),
            }
        } else {
            Err(PyTypeError::new_err(format!(
                "unsupported Python type for Value conversion: {}",
                obj.get_type().name()?
            )))
        }
    })
}

fn obj_to_vec3(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let data: Vec<f64> = obj.getattr("data")?.extract()?;
    let arr: [f64; 3] = data
        .try_into()
        .map_err(|_| PyTypeError::new_err("Vec3.data must have 3 elements"))?;
    Ok(toolapi::Value::Vec3(Vec3(arr)))
}

fn obj_to_vec4(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let data: Vec<f64> = obj.getattr("data")?.extract()?;
    let arr: [f64; 4] = data
        .try_into()
        .map_err(|_| PyTypeError::new_err("Vec4.data must have 4 elements"))?;
    Ok(toolapi::Value::Vec4(Vec4(arr)))
}

fn obj_to_volume(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let shape_vec: Vec<u64> = obj.getattr("shape")?.extract()?;
    let shape: [u64; 3] = shape_vec
        .try_into()
        .map_err(|_| PyTypeError::new_err("Volume.shape must have 3 elements"))?;

    let affine_obj = obj.getattr("affine")?;
    let affine = extract_affine(&affine_obj)?;

    let data_obj = obj.getattr("data")?;
    let data = py_list_to_typed_list(py, &data_obj)?;

    Ok(toolapi::Value::Volume(Volume {
        shape,
        affine,
        data,
    }))
}

fn extract_affine(obj: &Bound<'_, PyAny>) -> PyResult<[[f64; 4]; 3]> {
    let rows: Vec<Vec<f64>> = obj.extract()?;
    if rows.len() != 3 {
        return Err(PyTypeError::new_err("affine must have 3 rows"));
    }
    let mut affine = [[0.0f64; 4]; 3];
    for (i, row) in rows.into_iter().enumerate() {
        let arr: [f64; 4] = row
            .try_into()
            .map_err(|_| PyTypeError::new_err("each affine row must have 4 elements"))?;
        affine[i] = arr;
    }
    Ok(affine)
}

fn obj_to_phantom_tissue(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let density = extract_volume(py, &obj.getattr("density")?)?;
    let db0 = extract_volume(py, &obj.getattr("db0")?)?;
    Ok(toolapi::Value::PhantomTissue(PhantomTissue {
        density,
        db0,
        t1: obj.getattr("t1")?.extract()?,
        t2: obj.getattr("t2")?.extract()?,
        t2dash: obj.getattr("t2dash")?.extract()?,
        adc: obj.getattr("adc")?.extract()?,
    }))
}

fn extract_volume(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Volume> {
    let shape_vec: Vec<u64> = obj.getattr("shape")?.extract()?;
    let shape: [u64; 3] = shape_vec
        .try_into()
        .map_err(|_| PyTypeError::new_err("Volume.shape must have 3 elements"))?;
    let affine_obj = obj.getattr("affine")?;
    let affine = extract_affine(&affine_obj)?;
    let data_obj = obj.getattr("data")?;
    let data = py_list_to_typed_list(py, &data_obj)?;
    Ok(Volume {
        shape,
        affine,
        data,
    })
}

fn obj_to_segmented_phantom(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let tissues_list = obj.getattr("tissues")?;
    let tissues_py = tissues_list.cast::<PyList>()?;
    let mut tissues = Vec::with_capacity(tissues_py.len());
    for item in tissues_py.iter() {
        tissues.push(extract_phantom_tissue(py, &item)?);
    }

    let b1_tx = extract_volume_list(py, &obj.getattr("b1_tx")?)?;
    let b1_rx = extract_volume_list(py, &obj.getattr("b1_rx")?)?;

    Ok(toolapi::Value::SegmentedPhantom(SegmentedPhantom {
        tissues,
        b1_tx,
        b1_rx,
    }))
}

fn extract_phantom_tissue(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<PhantomTissue> {
    let density = extract_volume(py, &obj.getattr("density")?)?;
    let db0 = extract_volume(py, &obj.getattr("db0")?)?;
    Ok(PhantomTissue {
        density,
        db0,
        t1: obj.getattr("t1")?.extract()?,
        t2: obj.getattr("t2")?.extract()?,
        t2dash: obj.getattr("t2dash")?.extract()?,
        adc: obj.getattr("adc")?.extract()?,
    })
}

fn extract_volume_list(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Vec<Volume>> {
    let list = obj.cast::<PyList>()?;
    let mut volumes = Vec::with_capacity(list.len());
    for item in list.iter() {
        volumes.push(extract_volume(py, &item)?);
    }
    Ok(volumes)
}

fn obj_to_instant_seq_event(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let event = extract_instant_seq_event(py, obj)?;
    Ok(toolapi::Value::InstantSeqEvent(event))
}

fn extract_instant_seq_event(_py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<InstantSeqEvent> {
    let variant: String = obj.getattr("variant")?.extract()?;
    let fields = obj.getattr("fields")?;
    match variant.as_str() {
        "Pulse" => Ok(InstantSeqEvent::Pulse {
            angle: fields.get_item("angle")?.extract()?,
            phase: fields.get_item("phase")?.extract()?,
        }),
        "Fid" => {
            let kt_obj = fields.get_item("kt")?;
            // kt is a Vec4 wrapper object
            let kt_data: Vec<f64> = kt_obj.getattr("data")?.extract()?;
            let kt_arr: [f64; 4] = kt_data
                .try_into()
                .map_err(|_| PyTypeError::new_err("kt must have 4 elements"))?;
            Ok(InstantSeqEvent::Fid { kt: Vec4(kt_arr) })
        }
        "Adc" => Ok(InstantSeqEvent::Adc {
            phase: fields.get_item("phase")?.extract()?,
        }),
        other => Err(PyTypeError::new_err(format!(
            "unknown InstantSeqEvent variant: {other}"
        ))),
    }
}

/// Convert a Python dict (with string keys and Value-convertible values) to Value::Dict.
fn obj_to_dict(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let dict = obj.cast::<PyDict>()?;
    let mut map = HashMap::new();
    for (key, value) in dict.iter() {
        let key: String = key.extract()?;
        let value = obj_to_value(py, value.into_py_any(py)?)?;
        map.insert(key, value);
    }
    Ok(toolapi::Value::Dict(Dict(map)))
}

/// Convert a Python list to Value::List (heterogeneous).
fn obj_to_list(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let list = obj.cast::<PyList>()?;
    let mut items = Vec::with_capacity(list.len());
    for item in list.iter() {
        items.push(obj_to_value(py, item.into_py_any(py)?)?);
    }
    Ok(toolapi::Value::List(List(items)))
}

/// Convert a Python list to a TypedList by inspecting element types.
///
/// Heuristic: look at the first element to determine the type, then extract
/// all elements as that type. Falls back to TypedList::Float(vec![]) for
/// empty lists.
fn py_list_to_typed_list(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<TypedList> {
    let list = obj.cast::<PyList>()?;
    if list.is_empty() {
        return Ok(TypedList::Float(vec![]));
    }

    let first = list.get_item(0)?;

    // Try complex before float, since complex can't extract as f64
    if first.extract::<Complex64>().is_ok() {
        let data: Vec<Complex64> = list.extract()?;
        return Ok(TypedList::Complex(data));
    }
    if first.extract::<f64>().is_ok() {
        let data: Vec<f64> = list.extract()?;
        return Ok(TypedList::Float(data));
    }
    if first.extract::<i64>().is_ok() {
        let data: Vec<i64> = list.extract()?;
        return Ok(TypedList::Int(data));
    }
    if first.extract::<bool>().is_ok() {
        let data: Vec<bool> = list.extract()?;
        return Ok(TypedList::Bool(data));
    }
    if first.extract::<String>().is_ok() {
        let data: Vec<String> = list.extract()?;
        return Ok(TypedList::Str(data));
    }

    // Check for structured types by class name
    if let Ok(type_name) = first.get_type().name().map(|n| n.to_string()) {
        match type_name.as_str() {
            "Vec3" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    let v: Vec<f64> = item.getattr("data")?.extract()?;
                    let arr: [f64; 3] = v
                        .try_into()
                        .map_err(|_| PyTypeError::new_err("Vec3.data must have 3 elements"))?;
                    data.push(Vec3(arr));
                }
                return Ok(TypedList::Vec3(data));
            }
            "Vec4" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    let v: Vec<f64> = item.getattr("data")?.extract()?;
                    let arr: [f64; 4] = v
                        .try_into()
                        .map_err(|_| PyTypeError::new_err("Vec4.data must have 4 elements"))?;
                    data.push(Vec4(arr));
                }
                return Ok(TypedList::Vec4(data));
            }
            "InstantSeqEvent" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    data.push(extract_instant_seq_event(py, &item)?);
                }
                return Ok(TypedList::InstantSeqEvent(data));
            }
            "Volume" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    data.push(extract_volume(py, &item)?);
                }
                return Ok(TypedList::Volume(data));
            }
            "PhantomTissue" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    data.push(extract_phantom_tissue(py, &item)?);
                }
                return Ok(TypedList::PhantomTissue(data));
            }
            "SegmentedPhantom" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    // Reuse the extraction logic, stripping the Value wrapper
                    let val = obj_to_value(py, item.into_py_any(py)?)?;
                    match val {
                        toolapi::Value::SegmentedPhantom(sp) => data.push(sp),
                        _ => return Err(PyTypeError::new_err("expected SegmentedPhantom in list")),
                    }
                }
                return Ok(TypedList::SegmentedPhantom(data));
            }
            _ => {}
        }
    }

    Err(PyTypeError::new_err(
        "cannot determine TypedList element type from list contents",
    ))
}

// =============================================================================
// Rust Value -> Python conversion
// =============================================================================

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
