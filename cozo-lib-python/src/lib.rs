/*
 * Copyright 2022, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::collections::{BTreeMap, BTreeSet};

use miette::{IntoDiagnostic, Report, Result};
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyByteArray, PyBytes, PyDict, PyList, PyModule, PyString, PyTuple};
use serde_json::json;

use cozo::*;

fn py_to_rows(ob: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<DataValue>>> {
    let rows = ob.extract::<Vec<Vec<Bound<'_, PyAny>>>>()?;
    let res: Vec<Vec<DataValue>> = rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|el| py_to_value(&el))
                .collect::<PyResult<_>>()
        })
        .collect::<PyResult<_>>()?;
    Ok(res)
}

fn report2py(r: Report) -> PyErr {
    PyException::new_err(r.to_string())
}

fn py_to_named_rows(ob: &Bound<'_, PyAny>) -> PyResult<NamedRows> {
    let d = ob.downcast::<PyDict>()?;
    let rows = d
        .get_item("rows")?
        .ok_or_else(|| PyException::new_err("named rows must contain 'rows'"))?;
    let rows = py_to_rows(&rows)?;
    let headers = d
        .get_item("headers")?
        .ok_or_else(|| PyException::new_err("named rows must contain 'headers'"))?;
    let headers = headers.extract::<Vec<String>>()?;
    Ok(NamedRows::new(headers, rows))
}

fn py_to_value(ob: &Bound<'_, PyAny>) -> PyResult<DataValue> {
    Ok(if ob.is_none() {
        DataValue::Null
    } else if let Ok(b) = ob.downcast::<PyBool>() {
        DataValue::from(b.is_true())
    } else if let Ok(i) = ob.extract::<i64>() {
        DataValue::from(i)
    } else if let Ok(f) = ob.extract::<f64>() {
        DataValue::from(f)
    } else if let Ok(s) = ob.extract::<String>() {
        DataValue::from(s)
    } else if let Ok(b) = ob.downcast::<PyBytes>() {
        DataValue::Bytes(b.as_bytes().to_vec())
    } else if let Ok(b) = ob.downcast::<PyByteArray>() {
        DataValue::Bytes(unsafe { b.as_bytes() }.to_vec())
    } else if let Ok(l) = ob.downcast::<PyTuple>() {
        let mut coll = Vec::with_capacity(l.len());
        for el in l {
            let el = py_to_value(&el)?;
            coll.push(el)
        }
        DataValue::List(Box::new(coll))
    } else if let Ok(l) = ob.downcast::<PyList>() {
        let mut coll = Vec::with_capacity(l.len());
        for el in l {
            let el = py_to_value(&el)?;
            coll.push(el)
        }
        DataValue::List(Box::new(coll))
    } else if let Ok(d) = ob.downcast::<PyDict>() {
        let mut coll = serde_json::Map::default();
        for (k, v) in d {
            let k = serde_json::Value::from(py_to_value(&k)?);
            let k = match k {
                serde_json::Value::String(s) => s,
                s => s.to_string(),
            };
            let v = serde_json::Value::from(py_to_value(&v)?);
            coll.insert(k, v);
        }
        DataValue::Json(Box::new(JsonData(json!(coll))))
    } else {
        return Err(PyException::new_err(format!(
            "Cannot convert {ob} into Cozo value"
        )));
    })
}

fn convert_params(ob: &Bound<'_, PyDict>) -> PyResult<BTreeMap<String, DataValue>> {
    let mut ret = BTreeMap::new();
    for (k, v) in ob {
        let k: String = k.extract()?;
        let v = py_to_value(&v)?;
        ret.insert(k, v);
    }
    Ok(ret)
}

fn options_to_py<'py>(
    opts: BTreeMap<String, DataValue>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    let ret = PyDict::new_bound(py);

    for (k, v) in opts {
        let val = value_to_py(v, py);
        ret.set_item(k, val)?;
    }
    Ok(ret)
}

fn json_to_py(val: serde_json::Value, py: Python<'_>) -> PyObject {
    match val {
        serde_json::Value::Null => py.None(),
        serde_json::Value::Bool(b) => b.into_py(py),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py(py)
            } else if let Some(f) = n.as_f64() {
                f.into_py(py)
            } else {
                py.None()
            }
        }
        serde_json::Value::String(s) => s.into_py(py),
        serde_json::Value::Array(a) => {
            let vs: Vec<_> = a.into_iter().map(|v| json_to_py(v, py)).collect();
            vs.into_py(py)
        }
        serde_json::Value::Object(o) => {
            let d = PyDict::new_bound(py);
            for (k, v) in o {
                d.set_item(k, json_to_py(v, py)).unwrap();
            }
            d.into()
        }
    }
}

fn value_to_py(val: DataValue, py: Python<'_>) -> PyObject {
    match val {
        DataValue::Null => py.None(),
        DataValue::Bool(b) => b.into_py(py),
        DataValue::Num(num) => match num {
            Num::Int(i) => i.into_py(py),
            Num::Float(f) => f.into_py(py),
        },
        DataValue::Str(s) => s.as_str().into_py(py),
        DataValue::Bytes(b) => PyBytes::new_bound(py, &b).into(),
        DataValue::Uuid(uuid) => uuid.0.to_string().into_py(py),
        DataValue::Regex(rx) => rx.0.as_str().into_py(py),
        DataValue::List(l) => {
            let vs: Vec<_> = l.into_iter().map(|v| value_to_py(v, py)).collect();
            vs.into_py(py)
        }
        DataValue::Set(l) => {
            let vs: Vec<_> = l.into_iter().map(|v| value_to_py(v, py)).collect();
            vs.into_py(py)
        }
        DataValue::Validity(vld) => {
            [vld.timestamp.0 .0.into_py(py), vld.is_assert.0.into_py(py)].into_py(py)
        }
        DataValue::Bot => py.None(),
        DataValue::Vec(v) => match *v {
            Vector::F32(a) => {
                let vs: Vec<_> = a.into_iter().map(|v| v.into_py(py)).collect();
                vs.into_py(py)
            }
            Vector::F64(a) => {
                let vs: Vec<_> = a.into_iter().map(|v| v.into_py(py)).collect();
                vs.into_py(py)
            }
        },
        DataValue::Json(j) => json_to_py(j.0, py),
    }
}

fn rows_to_py_rows<R>(rows: Vec<R>, py: Python<'_>) -> PyObject
where
    R: IntoIterator<Item = DataValue>,
{
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|val| value_to_py(val, py))
                .collect::<Vec<_>>()
                .into_py(py)
        })
        .collect::<Vec<_>>()
        .into_py(py)
}

fn named_rows_to_py(named_rows: NamedRows, py: Python<'_>) -> PyObject {
    let rows = rows_to_py_rows(named_rows.rows, py);
    let headers = named_rows.headers.into_py(py);
    let next = match named_rows.next {
        None => py.None(),
        Some(nxt) => named_rows_to_py(*nxt, py),
    };
    BTreeMap::from([("rows", rows), ("headers", headers), ("next", next)]).into_py(py)
}

#[pyclass]
struct CozoDbPy {
    db: Option<DbInstance>,
}

#[pyclass]
struct CozoDbMulTx {
    tx: MultiTransaction,
}

const DB_CLOSED_MSG: &str = r##"{"ok":false,"message":"database closed"}"##;

#[pymethods]
impl CozoDbPy {
    #[new]
    fn new(engine: &str, path: &str, options: &str) -> PyResult<Self> {
        match DbInstance::new(engine, path, options) {
            Ok(db) => Ok(Self { db: Some(db) }),
            Err(err) => Err(PyException::new_err(format!("{err:?}"))),
        }
    }
    pub fn run_script(
        &self,
        py: Python<'_>,
        query: &str,
        params: &Bound<'_, PyDict>,
        immutable: bool,
    ) -> PyResult<PyObject> {
        if let Some(db) = &self.db {
            let params = convert_params(params)?;
            match py.allow_threads(|| {
                db.run_script(
                    query,
                    params,
                    if immutable {
                        ScriptMutability::Immutable
                    } else {
                        ScriptMutability::Mutable
                    },
                )
            }) {
                Ok(rows) => Ok(named_rows_to_py(rows, py)),
                Err(err) => {
                    let reports = format_error_as_json(err, Some(query)).to_string();
                    let json_mod = py.import_bound("json")?;
                    let loads_fn = json_mod.getattr("loads")?;
                    let args = PyTuple::new_bound(py, [PyString::new_bound(py, &reports)]);
                    let msg = loads_fn.call1(args)?;
                    Err(PyException::new_err(msg.into_py(py)))
                }
            }
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG))
        }
    }
    pub fn register_callback(&self, rel: &str, callback: &Bound<'_, PyAny>) -> PyResult<u32> {
        if let Some(db) = &self.db {
            let cb: Py<PyAny> = callback.clone().unbind();
            let (id, ch) = db.register_callback(rel, None);
            rayon::spawn(move || {
                for (op, new, old) in ch {
                    Python::with_gil(|py| {
                        let op = PyString::new_bound(py, op.as_str());
                        let new_py = rows_to_py_rows(new.rows, py);
                        let old_py = rows_to_py_rows(old.rows, py);
                        let args = PyTuple::new_bound(
                            py,
                            [op.into_any(), new_py.into_bound(py), old_py.into_bound(py)],
                        );
                        let callable = cb.bind(py);
                        if let Err(err) = callable.call1(args) {
                            eprintln!("{}", err);
                        }
                    })
                }
            });
            Ok(id)
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG))
        }
    }
    pub fn register_fixed_rule(
        &self,
        name: String,
        arity: usize,
        callback: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if let Some(db) = &self.db {
            let cb: Py<PyAny> = callback.clone().unbind();
            let rule_impl = SimpleFixedRule::new(arity, move |inputs, options| -> Result<_> {
                Python::with_gil(|py| -> Result<NamedRows> {
                    let py_inputs = PyList::new_bound(
                        py,
                        inputs.into_iter().map(|nr| rows_to_py_rows(nr.rows, py)),
                    );
                    let py_opts = options_to_py(options, py).into_diagnostic()?;
                    let args =
                        PyTuple::new_bound(py, vec![py_inputs.into_any(), py_opts.into_any()]);
                    let res = cb.bind(py).call1(args).into_diagnostic()?;
                    Ok(NamedRows::new(vec![], py_to_rows(&res).into_diagnostic()?))
                })
            });
            db.register_fixed_rule(name, rule_impl).map_err(report2py)
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG))
        }
    }
    pub fn unregister_callback(&self, id: u32) -> bool {
        if let Some(db) = &self.db {
            db.unregister_callback(id)
        } else {
            false
        }
    }
    pub fn unregister_fixed_rule(&self, name: &str) -> PyResult<bool> {
        if let Some(db) = &self.db {
            match db.unregister_fixed_rule(name) {
                Ok(b) => Ok(b),
                Err(err) => Err(PyException::new_err(err.to_string())),
            }
        } else {
            Ok(false)
        }
    }
    pub fn export_relations(&self, py: Python<'_>, relations: Vec<String>) -> PyResult<PyObject> {
        if let Some(db) = &self.db {
            let res = match py.allow_threads(|| db.export_relations(relations.iter())) {
                Ok(res) => res,
                Err(err) => return Err(PyException::new_err(err.to_string())),
            };
            let ret = PyDict::new_bound(py);
            for (k, v) in res {
                ret.set_item(k, named_rows_to_py(v, py))?;
            }
            Ok(ret.into())
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG.to_string()))
        }
    }
    pub fn import_relations(&self, py: Python<'_>, data: &Bound<'_, PyDict>) -> PyResult<()> {
        if let Some(db) = &self.db {
            let mut arg = BTreeMap::new();
            for (k, v) in data.iter() {
                let k = k.extract::<String>()?;
                let vals = py_to_named_rows(&v)?;
                arg.insert(k, vals);
            }
            py.allow_threads(|| db.import_relations(arg))
                .map_err(report2py)
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG.to_string()))
        }
    }
    pub fn backup(&self, py: Python<'_>, path: &str) -> PyResult<()> {
        if let Some(db) = &self.db {
            py.allow_threads(|| db.backup_db(path)).map_err(report2py)
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG.to_string()))
        }
    }
    pub fn restore(&self, py: Python<'_>, path: &str) -> PyResult<()> {
        if let Some(db) = &self.db {
            py.allow_threads(|| db.restore_backup(path))
                .map_err(report2py)
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG.to_string()))
        }
    }
    pub fn import_from_backup(
        &self,
        py: Python<'_>,
        in_file: &str,
        relations: Vec<String>,
    ) -> PyResult<()> {
        if let Some(db) = &self.db {
            py.allow_threads(|| db.import_from_backup(in_file, &relations))
                .map_err(report2py)
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG.to_string()))
        }
    }
    pub fn close(&mut self) -> bool {
        self.db.take().is_some()
    }
    pub fn multi_transact(&self, write: bool) -> PyResult<CozoDbMulTx> {
        if let Some(db) = &self.db {
            Ok(CozoDbMulTx {
                tx: db.multi_transaction(write),
            })
        } else {
            Err(PyException::new_err(DB_CLOSED_MSG.to_string()))
        }
    }
}

#[pymethods]
impl CozoDbMulTx {
    pub fn abort(&self) -> PyResult<()> {
        self.tx
            .abort()
            .map_err(|err| PyException::new_err(err.to_string()))
    }
    pub fn commit(&self) -> PyResult<()> {
        self.tx
            .commit()
            .map_err(|err| PyException::new_err(err.to_string()))
    }
    pub fn run_script(
        &self,
        py: Python<'_>,
        query: &str,
        params: &Bound<'_, PyDict>,
    ) -> PyResult<PyObject> {
        let params = convert_params(params)?;
        match py.allow_threads(|| self.tx.run_script(query, params)) {
            Ok(rows) => Ok(named_rows_to_py(rows, py)),
            Err(err) => {
                let reports = format_error_as_json(err, Some(query)).to_string();
                let json_mod = py.import_bound("json")?;
                let loads_fn = json_mod.getattr("loads")?;
                let args = PyTuple::new_bound(py, [PyString::new_bound(py, &reports)]);
                let msg = loads_fn.call1(args)?;
                Err(PyException::new_err(msg.into_py(py)))
            }
        }
    }
}

#[pyfunction]
fn eval_expressions(
    py: Python<'_>,
    query: &str,
    params: &Bound<'_, PyDict>,
    bindings: &Bound<'_, PyDict>,
) -> PyResult<PyObject> {
    let params = convert_params(params).unwrap();
    let bindings = convert_params(bindings).unwrap();
    match evaluate_expressions(query, &params, &bindings) {
        Ok(v) => Ok(value_to_py(v, py)),
        Err(err) => {
            let reports = format_error_as_json(err, Some(query)).to_string();
            let json_mod = py.import_bound("json")?;
            let loads_fn = json_mod.getattr("loads")?;
            let args = PyTuple::new_bound(py, [PyString::new_bound(py, &reports)]);
            let msg = loads_fn.call1(args)?;
            Err(PyException::new_err(msg.into_py(py)))
        }
    }
}

#[pyfunction]
fn variables(
    _py: Python<'_>,
    query: &str,
    params: &Bound<'_, PyDict>,
) -> PyResult<BTreeSet<String>> {
    let params = convert_params(params).unwrap();
    match get_variables(query, &params) {
        Ok(rows) => Ok(rows),
        Err(err) => {
            let reports = format_error_as_json(err, Some(query)).to_string();
            let json_mod = _py.import_bound("json")?;
            let loads_fn = json_mod.getattr("loads")?;
            let args = PyTuple::new_bound(_py, [PyString::new_bound(_py, &reports)]);
            let msg = loads_fn.call1(args)?;
            Err(PyException::new_err(msg.into_py(_py)))
        }
    }
}

#[pymodule]
fn cozo_embedded(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CozoDbPy>()?;
    m.add_class::<CozoDbMulTx>()?;
    m.add_function(wrap_pyfunction!(eval_expressions, m)?)?;
    m.add_function(wrap_pyfunction!(variables, m)?)?;
    Ok(())
}
