use num_complex::Complex64;
use pyo3::{IntoPyObjectExt, exceptions::PyTypeError, prelude::*, types::PyList};
use toolapi::value::{atomic::*, typed::*};

use crate::{extract::*, obj_to_value};

/// Convert a Python list to a TypedList by inspecting element types.
///
/// Heuristic: look at the first element to determine the type, then extract
/// all elements as that type. Falls back to TypedList::Float(vec![]) for
/// empty lists.
pub fn py_list_to_typed_list(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<TypedList> {
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
                    data.push(obj_to_instant_seq_event(py, &item)?);
                }
                return Ok(TypedList::InstantSeqEvent(data));
            }
            "Volume" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    data.push(obj_to_volume(py, &item)?);
                }
                return Ok(TypedList::Volume(data));
            }
            "PhantomTissue" => {
                let mut data = Vec::with_capacity(list.len());
                for item in list.iter() {
                    data.push(obj_to_phantom_tissue(py, &item)?);
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
