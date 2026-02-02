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

- `parse(source, include_comments=False, max_depth=128, duplicate_keys="reject")`
  - Parses JSONC and returns regular Python values. Integers are exact;
    `duplicate_keys="allow"` keeps the last value instead of raising.
- `parse(source, include_comments=True)`
  - Returns a `Document` object backed by JWC's AST.
- `parse_document(source, max_depth=128, duplicate_keys="reject")`
  - Explicit constructor for `Document`.
- `compact(source)`
  - Minified output that keeps comments (so it is JSONC, not JSON).
- `pretty(source, indent=None)`
- `pointer(source, path)`
- `comments(source, path=None)`
  - Returns `trivia` (before the value), `trailing` (same line after it),
    `dangling` (before the closing bracket) and `comma` for the requested node.
- `patch(source, operations, pretty_output=True, indent=None, preserve=True)`
  - Applies RFC 6902 operations; comments survive and, with `preserve`, so does
    the layout of everything the patch did not touch.

Parse errors raise `ValueError` with the position: `duplicate key "a" at 3:3`.

## `Document` methods

- `add_comment(text, path=None, kind="line", position="leading")`
  - `position` is `"leading"`, `"trailing"` or `"dangling"` (containers only).
- `comments(path=None)`
- `pointer(path)`
- `value()`
- `to_json(pretty=True, indent=None, preserve=True)`
  - With `preserve`, untouched nodes are copied from the source text, alignment
    and blank lines included; `indent=None` then means "as in the source".
- `to_ast()`
  - Every node carries `trivia`, `trailing`, `dangling`, `comma`, `kind` and
    `value`; object entries add `key` and `key_trivia`.

## Example

```python
import jwc

doc = jwc.parse_document('{"settings": {"theme": "light"}}')
doc.add_comment(" edited", path="/settings/theme", position="trailing")
doc.add_comment(" more settings go here", path="/settings", position="dangling")
print(doc.to_json(indent="  "))
# {
#   "settings": {
#     "theme": "light" // edited
#     // more settings go here
#   }
# }
```
