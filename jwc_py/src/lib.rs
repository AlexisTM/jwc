use ::jwc as jwc_rs;
use jwc_rs::{Node, ObjectEntry, PatchOperation, Trivia, Value};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyFloat, PyInt, PyList, PyModule, PyString};
use std::sync::{Arc, Mutex};

fn to_py_value_error(msg: String) -> PyErr {
    PyValueError::new_err(msg)
}

fn decode_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

fn encode_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn node_at_path<'a>(root: &'a Node, path: &str) -> Option<&'a Node> {
    if path.is_empty() {
        return Some(root);
    }
    if !path.starts_with('/') {
        return None;
    }

    let mut current = root;
    for token in path.split('/').skip(1) {
        let key = decode_token(token);
        match &current.value {
            Value::Object(members) => {
                if let Some(entry) = members.iter().find(|e| e.key == key) {
                    current = &entry.value;
                } else {
                    return None;
                }
            }
            Value::Array(elements) => {
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(node) = elements.get(idx) {
                        current = node;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(current)
}

fn node_at_path_mut<'a>(root: &'a mut Node, path: &str) -> Option<&'a mut Node> {
    if path.is_empty() {
        return Some(root);
    }
    if !path.starts_with('/') {
        return None;
    }

    let mut current = root;
    for token in path.split('/').skip(1) {
        let key = decode_token(token);
        match &mut current.value {
            Value::Object(members) => {
                if let Some(entry) = members.iter_mut().find(|e| e.key == key) {
                    current = &mut entry.value;
                } else {
                    return None;
                }
            }
            Value::Array(elements) => {
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(node) = elements.get_mut(idx) {
                        current = node;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(current)
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
    out.set_item("comma", node.comma)?;
    Ok(out.unbind().into_any())
}

#[pyclass]
struct Document {
    node: Arc<Mutex<Node>>,
}

#[pyclass]
struct NodeRef {
    node: Arc<Mutex<Node>>,
    path: String,
}

#[pymethods]
impl Document {
    #[staticmethod]
    fn parse(source: &str) -> PyResult<Self> {
        let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
        Ok(Self {
            node: Arc::new(Mutex::new(node)),
        })
    }

    #[pyo3(signature=(pretty=true, indent=None))]
    fn to_json(&self, pretty: bool, indent: Option<String>) -> PyResult<String> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        if pretty {
            jwc_rs::to_string_pretty(&node, indent.as_deref()).map_err(to_py_value_error)
        } else {
            jwc_rs::to_string(&node).map_err(to_py_value_error)
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
        let target = node_at_path(&node, path.unwrap_or(""));
        if let Some(target) = target {
            Ok(Some(node_comments_to_py(py, target)?))
        } else {
            Ok(None)
        }
    }

    #[pyo3(signature=(text, path=None, kind=None))]
    fn add_comment(&mut self, text: &str, path: Option<&str>, kind: Option<&str>) -> PyResult<()> {
        let mut node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node_at_path_mut(&mut node, path.unwrap_or(""))
            .ok_or_else(|| PyValueError::new_err("Path not found"))?;

        let comment = match kind.unwrap_or("line") {
            "line" => Trivia::LineComment(text.to_string()),
            "block" => Trivia::BlockComment(text.to_string()),
            other => {
                return Err(PyValueError::new_err(format!(
                    "Unsupported kind: {other}. Use 'line' or 'block'"
                )));
            }
        };

        target.trivia.push(comment);

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
        if node_at_path(&node, &path).is_none() {
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
    #[pyo3(signature=(text, kind=None))]
    fn add_comment(&mut self, text: &str, kind: Option<&str>) -> PyResult<()> {
        let mut node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node_at_path_mut(&mut node, &self.path)
            .ok_or_else(|| PyValueError::new_err("Path not found"))?;

        let comment = match kind.unwrap_or("line") {
            "line" => Trivia::LineComment(text.to_string()),
            "block" => Trivia::BlockComment(text.to_string()),
            other => {
                return Err(PyValueError::new_err(format!(
                    "Unsupported kind: {other}. Use 'line' or 'block'"
                )));
            }
        };
        target.trivia.push(comment);

        Ok(())
    }

    fn comments(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let node = self
            .node
            .lock()
            .map_err(|_| PyValueError::new_err("Document lock poisoned"))?;
        let target = node_at_path(&node, &self.path);
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
        let target = node_at_path(&node, &self.path)
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
        if node_at_path(&node, &child).is_none() {
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
            if let Ok(i) = n.parse::<i64>() {
                out.set_item("value", i)?;
            } else {
                out.set_item("value", n.as_f64().map_err(to_py_value_error)?)?;
            }
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
        Value::Number(n) => {
            if let Ok(i) = n.parse::<i64>() {
                Ok(i.into_pyobject(py)?.unbind().into_any())
            } else {
                let f = n.as_f64().map_err(to_py_value_error)?;
                Ok(f.into_pyobject(py)?.unbind().into_any())
            }
        }
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
        let i = any.extract::<i64>()?;
        return Ok(Value::from(i as f64));
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

        let op = required_string(&dict, "op")?;
        let path = required_string(&dict, "path")?;

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
                from: required_string(&dict, "from")?,
                path,
            },
            "copy" => PatchOperation::Copy {
                from: required_string(&dict, "from")?,
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

#[pyfunction]
fn parse(py: Python<'_>, source: &str, include_comments: Option<bool>) -> PyResult<Py<PyAny>> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    if include_comments.unwrap_or(false) {
        Ok(Py::new(
            py,
            Document {
                node: Arc::new(Mutex::new(node)),
            },
        )?
        .into_any())
    } else {
        value_to_py(py, &node.value)
    }
}

#[pyfunction]
fn parse_document(py: Python<'_>, source: &str) -> PyResult<Py<Document>> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    Py::new(
        py,
        Document {
            node: Arc::new(Mutex::new(node)),
        },
    )
}

#[pyfunction]
fn compact(source: &str) -> PyResult<String> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    jwc_rs::to_string(&node).map_err(to_py_value_error)
}

#[pyfunction]
fn pretty(source: &str, indent: Option<String>) -> PyResult<String> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    jwc_rs::to_string_pretty(&node, indent.as_deref()).map_err(to_py_value_error)
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
fn comments(py: Python<'_>, source: &str, path: Option<&str>) -> PyResult<Option<Py<PyAny>>> {
    let node = jwc_rs::from_str(source).map_err(to_py_value_error)?;
    let target = node_at_path(&node, path.unwrap_or(""));

    if let Some(target) = target {
        Ok(Some(node_comments_to_py(py, target)?))
    } else {
        Ok(None)
    }
}

#[pyfunction]
fn patch(
    source: &str,
    operations: &Bound<'_, PyAny>,
    pretty_output: Option<bool>,
    indent: Option<String>,
) -> PyResult<String> {
    let mut value = jwc_rs::from_str(source).map_err(to_py_value_error)?.value;
    let ops = parse_patch(operations)?;
    value.apply_patch(ops).map_err(to_py_value_error)?;

    let node = Node::new(value);
    if pretty_output.unwrap_or(true) {
        jwc_rs::to_string_pretty(&node, indent.as_deref()).map_err(to_py_value_error)
    } else {
        jwc_rs::to_string(&node).map_err(to_py_value_error)
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
