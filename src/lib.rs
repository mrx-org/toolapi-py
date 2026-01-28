use num_complex::Complex64;
use pyo3::exceptions::PyTypeError;
use pyo3::types::{PyDict, PyList};
use pyo3::{IntoPyObjectExt, prelude::*};
use toolapi::value::{
    Event, EventSeq, MultiTissuePhantom, PhantomTissue, TissueProperties,
    VoxelGridPhantom, VoxelShape,
};

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
            Ok(toolapi::Value::String(s))
        } else if let Ok(type_name) = obj.getattr("_type").and_then(|t| t.extract::<String>()) {
            match type_name.as_str() {
                "TissueProperties" => obj_to_tissue_properties(obj),
                "VoxelGridPhantom" => obj_to_voxel_grid_phantom(obj),
                "MultiTissuePhantom" => obj_to_multi_tissue_phantom(obj),
                "Event" => obj_to_event(obj),
                "EventSeq" => obj_to_event_seq(obj),
                "BlockSeq" => obj_to_block_seq(obj),
                other => Err(PyTypeError::new_err(format!(
                    "unknown toolapi value _type: {other}"
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

fn extract_voxel_shape(obj: &Bound<'_, PyAny>) -> PyResult<VoxelShape> {
    let shape_type: String = obj.getattr("voxel_shape_type")?.extract()?;
    let shape_data: Vec<f64> = obj.getattr("voxel_shape_data")?.extract()?;
    let arr: [f64; 3] = shape_data
        .try_into()
        .map_err(|_| PyTypeError::new_err("voxel_shape_data must have 3 elements"))?;
    match shape_type.as_str() {
        "AASinc" => Ok(VoxelShape::AASinc(arr)),
        "AABox" => Ok(VoxelShape::AABox(arr)),
        other => Err(PyTypeError::new_err(format!(
            "unknown voxel shape type: {other}"
        ))),
    }
}

fn extract_complex_vecs(obj: &Bound<'_, PyAny>, attr: &str) -> PyResult<Vec<Vec<Complex64>>> {
    obj.getattr(attr)?.extract()
}

fn obj_to_tissue_properties(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    Ok(toolapi::Value::TissueProperties(TissueProperties {
        t1: obj.getattr("t1")?.extract()?,
        t2: obj.getattr("t2")?.extract()?,
        t2dash: obj.getattr("t2dash")?.extract()?,
        adc: obj.getattr("d")?.extract()?, // Python uses `d`, Rust uses `adc`
    }))
}

fn obj_to_voxel_grid_phantom(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let voxel_shape = extract_voxel_shape(obj)?;
    let grid_spacing: Vec<f64> = obj.getattr("grid_spacing")?.extract()?;
    let grid_size: Vec<usize> = obj.getattr("grid_size")?.extract()?;

    Ok(toolapi::Value::VoxelGridPhantom(VoxelGridPhantom {
        voxel_shape,
        grid_spacing: grid_spacing
            .try_into()
            .map_err(|_| PyTypeError::new_err("grid_spacing must have 3 elements"))?,
        grid_size: grid_size
            .try_into()
            .map_err(|_| PyTypeError::new_err("grid_size must have 3 elements"))?,
        pd: obj.getattr("pd")?.extract()?,
        t1: obj.getattr("t1")?.extract()?,
        t2: obj.getattr("t2")?.extract()?,
        t2dash: obj.getattr("t2dash")?.extract()?,
        adc: obj.getattr("adc")?.extract()?,
        b0: obj.getattr("b0")?.extract()?,
        b1: extract_complex_vecs(obj, "b1")?,
        coil_sens: extract_complex_vecs(obj, "coil_sens")?,
    }))
}

fn obj_to_multi_tissue_phantom(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let voxel_shape = extract_voxel_shape(obj)?;
    let grid_spacing: Vec<f64> = obj.getattr("grid_spacing")?.extract()?;
    let grid_size: Vec<usize> = obj.getattr("grid_size")?.extract()?;

    let tissues_obj = obj.getattr("tissues")?;
    let tissues_list = tissues_obj.cast::<PyList>()?;
    let mut tissues = Vec::with_capacity(tissues_list.len());
    for item in tissues_list.iter() {
        // Each tissue is a tuple (pd: list[float], b0: list[float], TissueProperties)
        let tuple = item.cast::<pyo3::types::PyTuple>()?;
        let pd: Vec<f64> = tuple.get_item(0)?.extract()?;
        let b0: Vec<f64> = tuple.get_item(1)?.extract()?;
        let props_obj = tuple.get_item(2)?;
        let props = TissueProperties {
            t1: props_obj.getattr("t1")?.extract()?,
            t2: props_obj.getattr("t2")?.extract()?,
            t2dash: props_obj.getattr("t2dash")?.extract()?,
            adc: props_obj.getattr("d")?.extract()?,
        };
        tissues.push(PhantomTissue { pd, b0, props });
    }

    Ok(toolapi::Value::MultiTissuePhantom(MultiTissuePhantom {
        voxel_shape,
        grid_spacing: grid_spacing
            .try_into()
            .map_err(|_| PyTypeError::new_err("grid_spacing must have 3 elements"))?,
        grid_size: grid_size
            .try_into()
            .map_err(|_| PyTypeError::new_err("grid_size must have 3 elements"))?,
        tissues,
        b1: extract_complex_vecs(obj, "b1")?,
        coil_sens: extract_complex_vecs(obj, "coil_sens")?,
    }))
}

fn obj_to_event(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let variant: String = obj.getattr("variant")?.extract()?;
    let fields = obj.getattr("fields")?;
    let event = match variant.as_str() {
        "Pulse" => Event::Pulse {
            angle: fields.get_item("angle")?.extract()?,
            phase: fields.get_item("phase")?.extract()?,
        },
        "Fid" => {
            let kt_vec: Vec<f64> = fields.get_item("kt")?.extract()?;
            Event::Fid {
                kt: kt_vec
                    .try_into()
                    .map_err(|_| PyTypeError::new_err("kt must have 4 elements"))?,
            }
        }
        "Adc" => Event::Adc {
            phase: fields.get_item("phase")?.extract()?,
        },
        other => {
            return Err(PyTypeError::new_err(format!(
                "unknown Event variant: {other}"
            )))
        }
    };
    Ok(toolapi::Value::EventSeq(EventSeq(vec![event])))
}

fn obj_to_event_seq(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let events_obj = obj.getattr("events")?;
    let events_list = events_obj.cast::<PyList>()?;
    let mut events = Vec::with_capacity(events_list.len());
    for item in events_list.iter() {
        let variant: String = item.getattr("variant")?.extract()?;
        let fields = item.getattr("fields")?;
        let event = match variant.as_str() {
            "Pulse" => Event::Pulse {
                angle: fields.get_item("angle")?.extract()?,
                phase: fields.get_item("phase")?.extract()?,
            },
            "Fid" => {
                let kt_vec: Vec<f64> = fields.get_item("kt")?.extract()?;
                Event::Fid {
                    kt: kt_vec
                        .try_into()
                        .map_err(|_| PyTypeError::new_err("kt must have 4 elements"))?,
                }
            }
            "Adc" => Event::Adc {
                phase: fields.get_item("phase")?.extract()?,
            },
            other => {
                return Err(PyTypeError::new_err(format!(
                    "unknown Event variant: {other}"
                )))
            }
        };
        events.push(event);
    }
    Ok(toolapi::Value::EventSeq(EventSeq(events)))
}

fn obj_to_block_seq(obj: &Bound<'_, PyAny>) -> PyResult<toolapi::Value> {
    let _ = obj;
    // Stub: BlockSeq conversion not yet needed by test/util.py
    Err(PyTypeError::new_err(
        "BlockSeq conversion not yet implemented",
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
        toolapi::Value::String(s) => s.into_py_any(py),
        toolapi::Value::TissueProperties(tp) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("TissueProperties")?;
            cls.call1((tp.t1, tp.t2, tp.t2dash, tp.adc))
                .map(|o| o.unbind())
        }
        toolapi::Value::EventSeq(seq) => {
            let module = py.import("toolapi.value")?;
            let event_cls = module.getattr("Event")?;
            let seq_cls = module.getattr("EventSeq")?;
            let events = PyList::empty(py);
            for event in seq.0 {
                let py_event = match event {
                    Event::Pulse { angle, phase } => {
                        event_cls.call_method1("Pulse", (angle, phase))?
                    }
                    Event::Fid { kt } => {
                        let kt_list = PyList::new(py, kt)?;
                        event_cls.call_method1("Fid", (kt_list,))?
                    }
                    Event::Adc { phase } => event_cls.call_method1("Adc", (phase,))?,
                };
                events.append(py_event)?;
            }
            seq_cls.call1((events,)).map(|o| o.unbind())
        }
        toolapi::Value::VoxelGridPhantom(p) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("VoxelGridPhantom")?;
            let (shape_type, shape_data) = voxel_shape_to_py(&p.voxel_shape);
            let b1 = complex_vecs_to_py(py, &p.b1)?;
            let coil_sens = complex_vecs_to_py(py, &p.coil_sens)?;
            cls.call1((
                shape_type,
                shape_data.to_vec(),
                p.grid_spacing.to_vec(),
                p.grid_size.to_vec(),
                p.pd,
                p.t1,
                p.t2,
                p.t2dash,
                p.adc,
                p.b0,
                b1,
                coil_sens,
            ))
            .map(|o| o.unbind())
        }
        toolapi::Value::MultiTissuePhantom(p) => {
            let module = py.import("toolapi.value")?;
            let cls = module.getattr("MultiTissuePhantom")?;
            let tp_cls = module.getattr("TissueProperties")?;
            let (shape_type, shape_data) = voxel_shape_to_py(&p.voxel_shape);
            let b1 = complex_vecs_to_py(py, &p.b1)?;
            let coil_sens = complex_vecs_to_py(py, &p.coil_sens)?;
            let tissues = PyList::empty(py);
            for t in &p.tissues {
                let props = tp_cls.call1((t.props.t1, t.props.t2, t.props.t2dash, t.props.adc))?;
                let tuple = pyo3::types::PyTuple::new(py, [
                    t.pd.clone().into_py_any(py)?,
                    t.b0.clone().into_py_any(py)?,
                    props.unbind().into_py_any(py)?,
                ])?;
                tissues.append(tuple)?;
            }
            cls.call1((
                shape_type,
                shape_data.to_vec(),
                p.grid_spacing.to_vec(),
                p.grid_size.to_vec(),
                b1,
                coil_sens,
                tissues,
            ))
            .map(|o| o.unbind())
        }
        toolapi::Value::Signal(sig) => {
            let outer = PyList::empty(py);
            for row in sig.0 {
                let inner = PyList::new(py, row)?;
                outer.append(inner)?;
            }
            outer.into_py_any(py)
        }
        _ => Err(PyTypeError::new_err(
            "unsupported Value type for Python conversion".to_string(),
        )),
    }
}

fn voxel_shape_to_py(shape: &VoxelShape) -> (&str, [f64; 3]) {
    match shape {
        VoxelShape::AASinc(d) => ("AASinc", *d),
        VoxelShape::AABox(d) => ("AABox", *d),
    }
}

fn complex_vecs_to_py(
    py: Python<'_>,
    vecs: &[Vec<Complex64>],
) -> PyResult<Py<PyList>> {
    let outer = PyList::empty(py);
    for inner in vecs {
        outer.append(PyList::new(py, inner)?)?;
    }
    Ok(outer.unbind())
}
