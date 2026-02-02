# Python Frontend (`jwc`)

The `jwc_py` crate builds a Python module imported as `jwc`.

## Build/Install

```bash
cd jwc_py
maturin develop
```

If build/install fails in a restricted environment:

```bash
RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/jwc-target UV_CACHE_DIR=/tmp/uv-cache maturin develop --pip-path pip
```

## Functions

- `parse(source)`
  - Parses JSONC and returns regular Python values.
- `parse(source, include_comments=True)`
  - Returns a `Document` object backed by JWC's AST.
- `parse_document(source)`
  - Explicit constructor for `Document`.
- `compact(source)`
- `pretty(source, indent=None)`
- `pointer(source, path)`
- `comments(source, path=None)`
  - Returns `trivia` and `comma` for the requested node.
- `patch(source, operations, pretty_output=True, indent=None)`

## `Document` methods

- `add_comment(text, path=None, kind="line")`
- `comments(path=None)`
- `pointer(path)`
- `value()`
- `to_json(pretty=True, indent=None)`
- `to_ast()`

## Example

```python
import jwc

doc = jwc.parse_document('{"settings": {"theme": "light"}}')
doc.add_comment("edited", path="/settings/theme", kind="line")
print(doc.to_json(pretty=False))
```
