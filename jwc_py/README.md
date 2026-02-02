# jwc (from jwc_py)

Python frontend for `jwc` (JSON with comments).

## Install

From the `jwc_py` directory:

```bash
maturin develop
```

or build a wheel:

```bash
maturin build --release
```

If your environment uses `sccache` or a restricted cache dir and `maturin develop` fails, run:

```bash
RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/jwc-target UV_CACHE_DIR=/tmp/uv-cache maturin develop --pip-path pip
```

## Tests

```bash
maturin develop
pytest -q
```

## API

- `parse(source: str, include_comments: bool = False) -> object`
- `parse_document(source: str) -> Document`
- `compact(source: str) -> str`
- `pretty(source: str, indent: str | None = None) -> str`
- `pointer(source: str, path: str) -> object | None`
- `comments(source: str, path: str | None = None) -> dict | None`
- `patch(source: str, operations: list[dict], pretty_output: bool | None = None, indent: str | None = None) -> str`

`parse(..., include_comments=True)` returns a `Document` object.

`Document` methods:

- `add_comment(text, path=None, kind="line")`
- `comments(path=None)`
- `pointer(path)`
- `value()`
- `to_json(pretty=True, indent=None)`
- `to_ast()`

`to_ast()` returns a comment-aware AST-like structure with:

- `trivia`
- `comma`
- recursive `value` nodes (and key trivia for object entries)

## Example

```python
import jwc

text = '''
{
  // service
  "port": 8080,
  "enabled": true,
}
'''

obj = jwc.parse(text)
assert obj["port"] == 8080

print(jwc.pointer(text, "/enabled"))

doc = jwc.parse(text, include_comments=True)
doc.add_comment("extra", path="/port", kind="line")
print(doc.to_json(pretty=False))
```
