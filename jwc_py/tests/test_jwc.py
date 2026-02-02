import pytest

jwc = pytest.importorskip("jwc")


def test_parse_returns_python_values():
    source = """
    {
      // port comment
      "port": 8080,
      "enabled": true,
      "tags": ["a", "b"],
      "meta": {"env": "dev"}
    }
    """
    obj = jwc.parse(source)
    assert obj["port"] == 8080
    assert obj["enabled"] is True
    assert obj["tags"] == ["a", "b"]
    assert obj["meta"]["env"] == "dev"


def test_compact_and_pretty():
    source = '{"a":1,"b":2}'

    compact = jwc.compact(source)
    assert compact == '{"a":1,"b":2}'

    pretty = jwc.pretty(source, "  ")
    assert '"a": 1' in pretty
    assert "\n" in pretty


def test_pointer_existing_and_missing():
    source = '{"settings":{"theme":"light","ports":[8080,8081]}}'

    assert jwc.pointer(source, "/settings/theme") == "light"
    assert jwc.pointer(source, "/settings/ports/1") == 8081
    assert jwc.pointer(source, "/settings/missing") is None


def test_patch_replace_and_add():
    source = '{"port":8080}'
    out = jwc.patch(
        source,
        [
            {"op": "replace", "path": "/port", "value": 9090},
            {"op": "add", "path": "/name", "value": "api"},
        ],
        pretty_output=False,
    )

    obj = jwc.parse(out)
    assert obj["port"] == 9090
    assert obj["name"] == "api"


def test_patch_remove():
    source = '{"a":1,"b":2}'
    out = jwc.patch(
        source,
        [{"op": "remove", "path": "/a"}],
        pretty_output=False,
    )
    obj = jwc.parse(out)
    assert "a" not in obj
    assert obj["b"] == 2


def test_patch_invalid_op_raises_value_error():
    with pytest.raises(ValueError):
        jwc.patch(
            '{"a":1}',
            [{"op": "boom", "path": "/a", "value": 2}],
            pretty_output=False,
        )


def test_parse_include_comments_exposes_trivia():
    source = """
    {
      // root
      "a": 1, // trailing-a
      "b": true
    }
    """
    doc = jwc.parse(source, include_comments=True)
    ast = doc.to_ast()
    assert ast["kind"] == "object"
    assert isinstance(ast["trivia"], list)
    assert isinstance(ast["value"], list)

    first_entry = ast["value"][0]
    assert first_entry["key"] == "a"
    assert first_entry["value"]["kind"] == "number"


def test_comments_at_path():
    source = """
    {
      "x": 1, // keep-me
      "y": {"z": 2}
    }
    """
    c_root = jwc.comments(source)
    assert c_root is not None
    assert "trivia" in c_root

    c_y = jwc.comments(source, "/y")
    assert c_y is not None
    assert isinstance(c_y["trivia"], list)

    assert jwc.comments(source, "/missing") is None


def test_document_add_comment_and_to_json():
    source = '{"x": 1}'
    doc = jwc.parse_document(source)
    doc.add_comment("added-by-test", path="/x", kind="line")

    c = doc.comments("/x")
    assert c is not None
    assert any(t["text"] == "added-by-test" for t in c["trivia"])

    out = doc.to_json(pretty=False)
    assert "added-by-test" in out
