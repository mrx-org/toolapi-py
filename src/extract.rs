use std::collections::HashMap;

use pyo3::{
    IntoPyObjectExt,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyDict, PyList},
};

use toolapi::value::{atomic::*, dynamic::*, structured::*};

use crate::extract_typed::py_list_to_typed_list;

pub fn obj_to_dict(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Dict> {
    let dict = obj.cast::<PyDict>()?;
    let mut map = HashMap::new();
    for (key, value) in dict.iter() {
        let key: String = key.extract()?;
        let value = super::obj_to_value(py, value.into_py_any(py)?)?;
        map.insert(key, value);
    }
    Ok(Dict(map))
}

pub fn obj_to_list(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<List> {
    let list = obj.cast::<PyList>()?;
    let mut items = Vec::with_capacity(list.len());
    for item in list.iter() {
        items.push(super::obj_to_value(py, item.into_py_any(py)?)?);
    }
    Ok(List(items))
}

pub fn obj_to_vec3(obj: &Bound<'_, PyAny>) -> PyResult<Vec3> {
    let data: Vec<f64> = obj.getattr("data")?.extract()?;
    let arr: [f64; 3] = data
        .try_into()
        .map_err(|_| PyTypeError::new_err("Vec3.data must have 3 elements"))?;
    Ok(Vec3(arr))
}

pub fn obj_to_vec4(obj: &Bound<'_, PyAny>) -> PyResult<Vec4> {
    let data: Vec<f64> = obj.getattr("data")?.extract()?;
    let arr: [f64; 4] = data
        .try_into()
        .map_err(|_| PyTypeError::new_err("Vec4.data must have 4 elements"))?;
    Ok(Vec4(arr))
}

pub fn obj_to_volume(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Volume> {
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

pub fn obj_to_phantom_tissue(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<PhantomTissue> {
    let density = obj_to_volume(py, &obj.getattr("density")?)?;
    let db0 = obj_to_volume(py, &obj.getattr("db0")?)?;
    Ok(PhantomTissue {
        density,
        db0,
        t1: obj.getattr("t1")?.extract()?,
        t2: obj.getattr("t2")?.extract()?,
        t2dash: obj.getattr("t2dash")?.extract()?,
        adc: obj.getattr("adc")?.extract()?,
    })
}

pub fn obj_to_segmented_phantom(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<SegmentedPhantom> {
    let tissues_list = obj.getattr("tissues")?;
    let tissues_py = tissues_list.cast::<PyList>()?;
    let mut tissues = Vec::with_capacity(tissues_py.len());
    for item in tissues_py.iter() {
        tissues.push(obj_to_phantom_tissue(py, &item)?);
    }

    let b1_tx = extract_volume_list(py, &obj.getattr("b1_tx")?)?;
    let b1_rx = extract_volume_list(py, &obj.getattr("b1_rx")?)?;

    Ok(SegmentedPhantom {
        tissues,
        b1_tx,
        b1_rx,
    })
}

pub fn obj_to_instant_seq_event(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<InstantSeqEvent> {
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

// =============================================================================
// Helpers
// =============================================================================

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

fn extract_volume_list(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Vec<Volume>> {
    let list = obj.cast::<PyList>()?;
    let mut volumes = Vec::with_capacity(list.len());
    for item in list.iter() {
        volumes.push(obj_to_volume(py, &item)?);
    }
    Ok(volumes)
}
