use ::jwc as jwc_rs;
use jwc_rs::pointer::encode_token;
use jwc_rs::{DuplicateKeys, Node, ObjectEntry, ParseOptions, PatchOperation, Trivia, Value};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyFloat, PyInt, PyList, PyModule, PyString};
use std::sync::{Arc, Mutex};

fn to_py_value_error(err: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(err.to_string())
}

fn parse_options(max_depth: Option<usize>, duplicate_keys: Option<&str>) -> PyResult<ParseOptions> {
    let mut options = ParseOptions::default();
    if let Some(depth) = max_depth {
        options.max_depth = depth;
    }
    options.duplicate_keys = match duplicate_keys.unwrap_or("reject") {
        "reject" => DuplicateKeys::Reject,
        "allow" => DuplicateKeys::Allow,
        other => {
            return Err(PyValueError::new_err(format!(
                "Unsupported duplicate_keys: {other}. Use 'reject' or 'allow'"
            )));
        }
    };
    Ok(options)
}

fn number_to_py(py: Python<'_>, n: &jwc_rs::Number) -> PyResult<Py<PyAny>> {
    if let Some(i) = n.as_i64() {
        Ok(i.into_pyobject(py)?.unbind().into_any())
    } else if let Some(u) = n.as_u64() {
        Ok(u.into_pyobject(py)?.unbind().into_any())
    } else {
        Ok(n.as_f64().into_pyobject(py)?.unbind().into_any())
    }
}

fn trivia_to_py(py: Python<'_>, trivia: &[Trivia]) -> PyResult<Py<PyAny>> {
    let out = PyList::empty(py);
    for item in trivia {
        let d = PyDict::new(py);
        match item {
            Trivia::LineComment(text) => {
                d.set_item("kind", "line")?;
                d.set_item("text", text)?;
            }
            Trivia::BlockComment(text) => {
                d.set_item("kind", "block")?;
                d.set_item("text", text)?;
            }
        }
        out.append(d)?;
    }
    Ok(out.unbind().into_any())
}

fn node_comments_to_py(py: Python<'_>, node: &Node) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    out.set_item("trivia", trivia_to_py(py, &node.trivia)?)?;
    out.set_item("trailing", trivia_to_py(py, &node.trailing)?)?;
    out.set_item("dangling", trivia_to_py(py, &node.dangling)?)?;
    out.set_item("comma", node.comma)?;
    Ok(out.unbind().into_any())
}

fn make_comment(text: &str, kind: Option<&str>) -> PyResult<Trivia> {
    match kind.unwrap_or("line") {
        "line" => Ok(Trivia::LineComment(text.to_string())),
        "block" => Ok(Trivia::BlockComment(text.to_string())),
        other => Err(PyValueError::new_err(format!(
            "Unsupported kind: {other}. Use 'line' or 'block'"
        ))),
    }
}

fn comment_slot<'n>(node: &'n mut Node, position: Option<&str>) -> PyResult<&'n mut Vec<Trivia>> {
    match position.unwrap_or("leading") {
        "leading" => Ok(&mut node.trivia),
        "trailing" => Ok(&mut node.trailing),
        "dangling" => match node.value {
            Value::Array(_) | Value::Object(_) => Ok(&mut node.dangling),
            _ => Err(PyValueError::new_err(
                "Only arrays and objects take dangling comments",
            )),
        },
        other => Err(PyValueError::new_err(format!(
            "Unsupported position: {other}. Use 'leading', 'trailing' or 'dangling'"
        ))),
    }
}

#[pyclass]
struct Document {
    node: Arc<Mutex<Node>>,
    /// The text it was parsed from, for layout-preserving output.
    source: Option<String>,
}

impl Document {
    fn from_source(source: &str, options: ParseOptions) -> PyResult<Self> {
        let node = jwc_rs::from_str_with(source, options).map_err(to_py_value_error)?;
        Ok(Self {
            node: Arc::new(Mutex::new(node)),
            source: Some(source.to_string()),
        })
    }
}

#[pyclass]
struct NodeRef {
    node: Arc<Mutex<Node>>,
    path: String,
}

#[pymethods]
impl Document {
    #[staticmethod]
    #[pyo3(signature=(source, max_depth=None, duplicate_keys=None))]
    fn parse(
        source: &str,
        max_depth: Option<usize>,
        duplicate_keys: Option<&str>,
    ) -> PyResult<Self> {
        Self::from_source(source, parse_options(max_depth, duplicate_keys)?)
    }

    /// `preserve` keeps the original text of everything that did not change
    /// (whitespace and alignment included) and only lays out edited
    /// containers again. `indent=None` then means "as in the source".
    #[pyo3(signature=(pretty=true, indent=None, preserve=true))]
    fn to_json(&self, pretty: bool, indent: Option<&str>, preserve: bool) -> PyResult<String> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        match (&self.source, pretty, preserve) {
            (Some(source), true, true) => {
                jwc_rs::to_string_preserving(&node, source, indent).map_err(to_py_value_error)
            }
            (_, true, _) => jwc_rs::to_string_pretty(&node, indent).map_err(to_py_value_error),
            _ => jwc_rs::to_string(&node).map_err(to_py_value_error),
        }
    }

    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        value_to_py(py, &node.value)
    }

    fn pointer(&self, py: Python<'_>, path: &str) -> PyResult<Option<Py<PyAny>>> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        match node.value.pointer(path) {
            Some(v) => Ok(Some(value_to_py(py, v)?)),
            None => Ok(None),
        }
    }

    #[pyo3(signature=(path=None))]
    fn comments(&self, py: Python<'_>, path: Option<&str>) -> PyResult<Option<Py<PyAny>>> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node.pointer(path.unwrap_or(""));
        if let Some(target) = target {
            Ok(Some(node_comments_to_py(py, target)?))
        } else {
            Ok(None)
        }
    }

    #[pyo3(signature=(text, path=None, kind=None, position=None))]
    fn add_comment(
        &mut self,
        text: &str,
        path: Option<&str>,
        kind: Option<&str>,
        position: Option<&str>,
    ) -> PyResult<()> {
        let comment = make_comment(text, kind)?;
        let mut node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node
            .pointer_mut(path.unwrap_or(""))
            .ok_or_else(|| PyValueError::new_err("Path not found"))?;
        comment_slot(target, position)?.push(comment);
        Ok(())
    }

    fn to_ast(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        node_to_py_with_comments(py, &node)
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<NodeRef>> {
        let token = if let Ok(k) = key.extract::<String>() {
            k
        } else if let Ok(i) = key.extract::<usize>() {
            i.to_string()
        } else {
            return Err(PyTypeError::new_err("Key must be str or int"));
        };

        let path = format!("/{}", encode_token(&token));
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        if node.pointer(&path).is_none() {
            return Err(PyValueError::new_err("Path not found"));
        }

        Py::new(
            py,
            NodeRef {
                node: Arc::clone(&self.node),
                path,
            },
        )
    }
}

#[pymethods]
impl NodeRef {
    #[pyo3(signature=(text, kind=None, position=None))]
    fn add_comment(
        &mut self,
        text: &str,
        kind: Option<&str>,
        position: Option<&str>,
    ) -> PyResult<()> {
        let comment = make_comment(text, kind)?;
        let mut node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node
            .pointer_mut(&self.path)
            .ok_or_else(|| PyValueError::new_err("Path not found"))?;
        comment_slot(target, position)?.push(comment);
        Ok(())
    }

    fn comments(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node.pointer(&self.path);
        if let Some(target) = target {
            Ok(Some(node_comments_to_py(py, target)?))
        } else {
            Ok(None)
        }
    }

    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node
            .pointer(&self.path)
            .ok_or_else(|| PyValueError::new_err("Path not found"))?;
        value_to_py(py, &target.value)
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<NodeRef>> {
        let token = if let Ok(k) = key.extract::<String>() {
            k
        } else if let Ok(i) = key.extract::<usize>() {
            i.to_string()
        } else {
            return Err(PyTypeError::new_err("Key must be str or int"));
        };

        let child = format!("{}/{}", self.path, encode_token(&token));
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        if node.pointer(&child).is_none() {
            return Err(PyValueError::new_err("Path not found"));
        }

        Py::new(
            py,
            NodeRef {
                node: Arc::clone(&self.node),
                path: child,
            },
        )
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let v = self.value(py)?;
        Ok(v.bind(py).repr()?.to_str()?.to_string())
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        let v = self.value(py)?;
        Ok(v.bind(py).str()?.to_str()?.to_string())
    }

    fn __int__(&self, py: Python<'_>) -> PyResult<i64> {
        let v = self.value(py)?;
        v.bind(py).extract::<i64>()
    }

    fn __float__(&self, py: Python<'_>) -> PyResult<f64> {
        let v = self.value(py)?;
        v.bind(py).extract::<f64>()
    }

    fn __bool__(&self, py: Python<'_>) -> PyResult<bool> {
        let v = self.value(py)?;
        v.bind(py).is_truthy()
    }
}

fn node_to_py_with_comments(py: Python<'_>, node: &Node) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    out.set_item("trivia", trivia_to_py(py, &node.trivia)?)?;
    out.set_item("trailing", trivia_to_py(py, &node.trailing)?)?;
    out.set_item("dangling", trivia_to_py(py, &node.dangling)?)?;
    out.set_item("comma", node.comma)?;

    match &node.value {
        Value::Null => {
            out.set_item("kind", "null")?;
            out.set_item("value", py.None())?;
        }
        Value::Bool(b) => {
            out.set_item("kind", "bool")?;
            out.set_item("value", *b)?;
        }
        Value::Number(n) => {
            out.set_item("kind", "number")?;
            out.set_item("value", number_to_py(py, n)?)?;
        }
        Value::String(s) => {
            out.set_item("kind", "string")?;
            out.set_item("value", s)?;
        }
        Value::Array(items) => {
            out.set_item("kind", "array")?;
            let arr = PyList::empty(py);
            for item in items {
                arr.append(node_to_py_with_comments(py, item)?)?;
            }
            out.set_item("value", arr)?;
        }
        Value::Object(members) => {
            out.set_item("kind", "object")?;
            let arr = PyList::empty(py);
            for entry in members {
                let d = PyDict::new(py);
                d.set_item("key", &entry.key)?;
                d.set_item("key_trivia", trivia_to_py(py, &entry.key_trivia)?)?;
                d.set_item("value", node_to_py_with_comments(py, &entry.value)?)?;
                arr.append(d)?;
            }
            out.set_item("value", arr)?;
        }
        Value::Lazy(lazy) => {
            out.set_item("kind", "lazy")?;
            match lazy.as_ref() {
                jwc_rs::LazyValue::Parsed(v) => {
                    let n = Node::new(v.clone());
                    out.set_item("value", node_to_py_with_comments(py, &n)?)?;
                }
                jwc_rs::LazyValue::Unknown(raw)
                | jwc_rs::LazyValue::UnknownObject(raw)
                | jwc_rs::LazyValue::UnknownVector(raw) => {
                    out.set_item("value", raw)?;
                }
            }
        }
    }

    Ok(out.unbind().into_any())
}

fn value_to_py(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().unbind().into_any()),
        Value::Number(n) => number_to_py(py, n),
        Value::String(s) => Ok(s.into_pyobject(py)?.unbind().into_any()),
        Value::Array(items) => {
            let out = PyList::empty(py);
            for item in items {
                out.append(value_to_py(py, &item.value)?)?;
            }
            Ok(out.unbind().into_any())
        }
        Value::Object(members) => {
            let out = PyDict::new(py);
            for entry in members {
                out.set_item(&entry.key, value_to_py(py, &entry.value.value)?)?;
            }
            Ok(out.unbind().into_any())
        }
        Value::Lazy(lazy) => match lazy.as_ref() {
            jwc_rs::LazyValue::Parsed(v) => value_to_py(py, v),
            jwc_rs::LazyValue::Unknown(raw)
            | jwc_rs::LazyValue::UnknownObject(raw)
            | jwc_rs::LazyValue::UnknownVector(raw) => {
                Ok(raw.into_pyobject(py)?.unbind().into_any())
            }
        },
    }
}

fn py_to_value(any: &Bound<'_, PyAny>) -> PyResult<Value> {
    if any.is_none() {
        return Ok(Value::Null);
    }

    if any.is_instance_of::<PyBool>() {
        return Ok(Value::Bool(any.extract::<bool>()?));
    }

    if any.is_instance_of::<PyInt>() {
        if let Ok(i) = any.extract::<i64>() {
            return Ok(Value::from(i));
        }
        return Ok(Value::from(any.extract::<u64>()?));
    }

    if any.is_instance_of::<PyFloat>() {
        return Ok(Value::from(any.extract::<f64>()?));
    }

    if any.is_instance_of::<PyString>() {
        return Ok(Value::from(any.extract::<String>()?));
    }

    if let Ok(list) = any.cast::<PyList>() {
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            out.push(Node::new(py_to_value(&item)?));
        }
        return Ok(Value::Array(out));
    }

    if let Ok(dict) = any.cast::<PyDict>() {
        let mut members = Vec::with_capacity(dict.len());
        for (k, v) in dict.iter() {
            let key = k
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("Object keys must be strings"))?;
            members.push(ObjectEntry::new(key, Node::new(py_to_value(&v)?)));
        }
        return Ok(Value::Object(members));
    }

    Err(PyTypeError::new_err(
        "Unsupported value type. Use None, bool, int, float, str, list, or dict.",
    ))
}

fn required_string(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    dict.get_item(key)?
        .ok_or_else(|| PyValueError::new_err(format!("Missing field: {key}")))?
        .extract::<String>()
}

fn parse_patch(ops: &Bound<'_, PyAny>) -> PyResult<Vec<PatchOperation>> {
    let list = ops
        .cast::<PyList>()
        .map_err(|_| PyTypeError::new_err("patch operations must be a list of dicts"))?;

    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        let dict = item
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("each patch operation must be a dict"))?;

        let op = required_string(dict, "op")?;
        let path = required_string(dict, "path")?;

        let parsed = match op.as_str() {
            "add" => PatchOperation::Add {
                path,
                value: py_to_value(
                    &dict
                        .get_item("value")?
                        .ok_or_else(|| PyValueError::new_err("Missing field: value"))?,
                )?,
            },
            "remove" => PatchOperation::Remove { path },
            "replace" => PatchOperation::Replace {
                path,
                value: py_to_value(
                    &dict
                        .get_item("value")?
                        .ok_or_else(|| PyValueError::new_err("Missing field: value"))?,
                )?,
            },
            "move" => PatchOperation::Move {
                from: required_string(dict, "from")?,
                path,
            },
            "copy" => PatchOperation::Copy {
                from: required_string(dict, "from")?,
                path,
            },
            "test" => PatchOperation::Test {
                path,
                value: py_to_value(
                    &dict
                        .get_item("value")?
                        .ok_or_else(|| PyValueError::new_err("Missing field: value"))?,
                )?,
            },
            _ => {
                return Err(PyValueError::new_err(format!(
                    "Unsupported op: {op}. Use add/remove/replace/move/copy/test"
                )));
            }
        };

        out.push(parsed);
    }

    Ok(out)
}

/// Plain Python values. `include_comments=True` returns a `Document` instead
/// (kept for compatibility; `parse_document` says the same thing).
#[pyfunction]
#[pyo3(signature=(source, include_comments=None, max_depth=None, duplicate_keys=None))]
fn parse(
    py: Python<'_>,
    source: &str,
    include_comments: Option<bool>,
    max_depth: Option<usize>,
    duplicate_keys: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let options = parse_options(max_depth, duplicate_keys)?;
    if include_comments.unwrap_or(false) {
        return Ok(Py::new(py, Document::from_source(source, options)?)?.into_any());
    }
    let node = jwc_rs::from_str_with(source, options).map_err(to_py_value_error)?;
    value_to_py(py, &node.value)
}

#[pyfunction]
#[pyo3(signature=(source, max_depth=None, duplicate_keys=None))]
fn parse_document(
    py: Python<'_>,
    source: &str,
    max_depth: Option<usize>,
    duplicate_keys: Option<&str>,
) -> PyResult<Py<Document>> {
    Py::new(
        py,
        Document::from_source(source, parse_options(max_depth, duplicate_keys)?)?,
    )
}

#[pyfunction]
fn compact(source: &str) -> PyResult<String> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    jwc_rs::to_string(&node).map_err(to_py_value_error)
}

#[pyfunction]
#[pyo3(signature=(source, indent=None))]
fn pretty(source: &str, indent: Option<&str>) -> PyResult<String> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    jwc_rs::to_string_pretty(&node, indent).map_err(to_py_value_error)
}

#[pyfunction]
fn pointer(py: Python<'_>, source: &str, path: &str) -> PyResult<Option<Py<PyAny>>> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    match node.value.pointer(path) {
        Some(v) => Ok(Some(value_to_py(py, v)?)),
        None => Ok(None),
    }
}

#[pyfunction]
#[pyo3(signature=(source, path=None))]
fn comments(py: Python<'_>, source: &str, path: Option<&str>) -> PyResult<Option<Py<PyAny>>> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    let target = node.pointer(path.unwrap_or(""));

    if let Some(target) = target {
        Ok(Some(node_comments_to_py(py, target)?))
    } else {
        Ok(None)
    }
}

/// Apply RFC 6902 operations. Comments survive, and with `preserve` (the
/// default for pretty output) so does the layout of everything untouched.
#[pyfunction]
#[pyo3(signature=(source, operations, pretty_output=None, indent=None, preserve=None))]
fn patch(
    source: &str,
    operations: &Bound<'_, PyAny>,
    pretty_output: Option<bool>,
    indent: Option<&str>,
    preserve: Option<bool>,
) -> PyResult<String> {
    let mut node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    let ops = parse_patch(operations)?;
    node.value.apply_patch(ops).map_err(to_py_value_error)?;

    match (pretty_output.unwrap_or(true), preserve.unwrap_or(true)) {
        (true, true) => {
            jwc_rs::to_string_preserving(&node, source, indent).map_err(to_py_value_error)
        }
        (true, false) => jwc_rs::to_string_pretty(&node, indent).map_err(to_py_value_error),
        (false, _) => jwc_rs::to_string(&node).map_err(to_py_value_error),
    }
}

#[pymodule]
fn jwc(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Document>()?;
    m.add_class::<NodeRef>()?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(parse_document, m)?)?;
    m.add_function(wrap_pyfunction!(compact, m)?)?;
    m.add_function(wrap_pyfunction!(pretty, m)?)?;
    m.add_function(wrap_pyfunction!(pointer, m)?)?;
    m.add_function(wrap_pyfunction!(comments, m)?)?;
    m.add_function(wrap_pyfunction!(patch, m)?)?;
    Ok(())
}
