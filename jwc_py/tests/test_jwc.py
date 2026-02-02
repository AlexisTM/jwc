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


def test_parse_defaults_to_plain_values():
    assert jwc.parse('{"a": 1}') == {"a": 1}
    assert jwc.pretty('{"a":1}') == '{\n    "a": 1\n}'


def test_comments_keep_their_position():
    source = '{\n\t"a": [\n\t\t1, // one\n\t],\n\t// before close\n}\n// tail'
    doc = jwc.parse_document(source)
    assert doc.to_json(pretty=True, indent="\t") == source

    root = doc.comments()
    assert root["dangling"] == [{"kind": "line", "text": " before close"}]
    assert root["trailing"] == [{"kind": "line", "text": " tail"}]
    assert doc.comments("/a/0")["trailing"] == [{"kind": "line", "text": " one"}]

    ast = doc.to_ast()
    assert ast["dangling"][0]["text"] == " before close"
    assert ast["value"][0]["value"]["value"][0]["trailing"][0]["text"] == " one"


def test_add_comment_positions():
    doc = jwc.parse_document('{"a": 1}')
    doc.add_comment(" end", position="dangling")
    doc.add_comment(" same line", path="/a", position="trailing")
    doc["a"].add_comment(" why ", kind="block")
    assert doc.to_json(pretty=True, indent="  ") == (
        '{\n  "a": /* why */ 1 // same line\n  // end\n}'
    )
    with pytest.raises(ValueError):
        doc.add_comment("x", path="/a", position="dangling")
    with pytest.raises(ValueError):
        doc.add_comment("x", position="sideways")


def test_parse_is_strict_by_default():
    with pytest.raises(ValueError, match=r'duplicate key "dup" at 1:10'):
        jwc.parse('{"dup":1,"dup":2}')
    assert jwc.parse('{"dup":1,"dup":2}', duplicate_keys="allow") == {"dup": 2}
    with pytest.raises(ValueError, match="nesting deeper than 128"):
        jwc.parse("[" * 200 + "]" * 200)
    assert jwc.parse("[[[1]]]", max_depth=3) == [[[1]]]
    with pytest.raises(ValueError, match="nesting deeper than 2"):
        jwc.parse("[[[1]]]", max_depth=2)
    for bad in ["01", "1.", ".5", "1e400"]:
        with pytest.raises(ValueError):
            jwc.parse(bad)


def test_numbers_are_exact():
    assert jwc.parse("9007199254740993") == 9007199254740993
    assert jwc.parse("18446744073709551615") == 18446744073709551615
    assert jwc.parse("-9223372036854775808") == -(2**63)
    assert jwc.parse("1.0") == 1.0 and isinstance(jwc.parse("1.0"), float)
    assert isinstance(jwc.parse("-0"), int)
    assert jwc.parse_document("[1.0, 1e3, 0.10, -0]").to_json() == "[1.0, 1e3, 0.10, -0]"


def test_preserving_output_keeps_untouched_layout():
    source = (
        "// header\n"
        "{\n"
        '\t"a":     [1,   2], // aligned\n'
        "\n"
        '\t"b": {\n'
        '\t\t"x": 1,\n'
        '\t\t"y": 2,\n'
        "\t},\n"
        "}\n"
    )
    doc = jwc.parse_document(source)
    assert doc.to_json() == source
    assert doc.to_json(preserve=False) != source

    out = jwc.patch(source, [{"op": "replace", "path": "/b/y", "value": 3}])
    assert out == (
        "// header\n"
        "{\n"
        '\t"a":     [1,   2], // aligned\n'
        "\n"
        '\t"b": {\n'
        '\t\t"x": 1,\n'
        '\t\t"y": 3,\n'
        "\t},\n"
        "}\n"
    )
    # Errors say where.
    with pytest.raises(ValueError, match="found .:. at 3:6"):
        jwc.parse('{\n  "a": [1,\n  "b": 2]}')
