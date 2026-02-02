use jwc::{JwcDeserializable, JwcSerializable, Node, Trivia, Value};
use jwcc_derive::{JwcDeserializable, JwcSerializable};
use std::collections::HashMap;

#[derive(JwcSerializable, JwcDeserializable, PartialEq, Debug, Clone)]
struct TestObject {
    name: String,
    age: i32,
}

#[test]
fn test_derive_macro() {
    let obj = TestObject {
        name: "Alice".to_string(),
        age: 30,
    };

    // Serialize
    let val = obj.to_jwc();
    let s = val.to_string();
    println!("Serialized: {s}");

    assert!(s.contains("\"name\":\"Alice\""));
    assert!(s.contains("\"age\":30"));

    // Deserialize
    let obj2 = TestObject::from_jwc(val).expect("Failed to deserialize");
    assert_eq!(obj, obj2);
}

#[derive(JwcSerializable, JwcDeserializable, PartialEq, Debug)]
struct NestedObject {
    id: i32,
    child: TestObject,
}

#[test]
fn test_nested_derive() {
    let nested = NestedObject {
        id: 1,
        child: TestObject {
            name: "Bob".to_string(),
            age: 5,
        },
    };

    let val = nested.to_jwc();
    let s = val.to_string();
    println!("Nested: {s}");

    let nested2 = NestedObject::from_jwc(val).expect("Failed to deserialize nested");
    assert_eq!(nested, nested2);
}

#[derive(JwcSerializable, JwcDeserializable, PartialEq, Debug)]
struct ExhaustiveStruct {
    // Primitives
    b: bool,
    i8_: i8,
    i16_: i16,
    i32_: i32,
    i64_: i64,
    u8_: u8,
    u16_: u16,
    u32_: u32,
    u64_: u64,
    f32_: f32,
    f64_: f64,
    string: String,

    // Collections
    vec_int: Vec<i32>,

    // Option
    opt_some: Option<i32>,
    opt_none: Option<i32>,

    // Unit
    unit: (),
}

#[test]
fn test_exhaustive_derive() {
    let obj = ExhaustiveStruct {
        b: true,
        i8_: -8,
        i16_: -16,
        i32_: -32,
        i64_: -64,
        u8_: 8,
        u16_: 16,
        u32_: 32,
        u64_: 64,
        f32_: 32.5,
        f64_: 64.5,
        string: "test".to_string(),
        vec_int: vec![1, 2],
        opt_some: Some(100),
        opt_none: None,
        unit: (),
    };

    // Serialize
    let val = obj.to_jwc();
    let s = val.to_string();
    println!("Exhaustive Serialized: {s}");

    // Verify specific fields
    assert!(s.contains("\"i8_\":-8"));
    assert!(s.contains("\"u64_\":64"));
    assert!(s.contains("\"opt_some\":100"));
    assert!(s.contains("\"opt_none\":null"));

    // Deserialize
    let obj2 = ExhaustiveStruct::from_jwc(val).expect("Failed to deserialize exhaustive");
    assert_eq!(obj, obj2);
}

#[derive(JwcSerializable, JwcDeserializable, PartialEq, Debug)]
struct ComplexCollections {
    user_map: HashMap<String, TestObject>,
    user_vec: Vec<TestObject>,
}

#[test]
fn test_complex_collections() {
    let mut map = HashMap::new();
    let alice = TestObject {
        name: "Alice".to_string(),
        age: 30,
    };
    map.insert("alice".to_string(), alice.clone());

    let bob = TestObject {
        name: "Bob".to_string(),
        age: 40,
    };
    let vec = vec![alice, bob];

    let obj = ComplexCollections {
        user_map: map,
        user_vec: vec,
    };

    // Serialize
    let val = obj.to_jwc();
    let s = val.to_string();
    // Verify structure roughly
    assert!(s.contains("\"user_vec\":["));
    assert!(s.contains("{\"name\":\"Alice\",\"age\":30}"));

    // Deserialize
    let obj2 = ComplexCollections::from_jwc(val).expect("Failed to deserialize complex");
    // Checking equality might be tricky with HashMap order, but Rust's PartialEq for HashMap handles it.
    assert_eq!(obj, obj2);
}

#[test]
fn test_derive_modification() {
    let obj = TestObject {
        name: "Charlie".to_string(),
        age: 10,
    };

    // 1. Convert to AST
    let mut node_val = obj.to_jwc();

    // 2. Modify AST (Add comment to 'age' key)
    if let Value::Object(ref mut members) = node_val {
        for entry in members {
            if entry.key == "age" {
                entry
                    .key_trivia
                    .push(Trivia::LineComment("// Age in years".to_string()));
            }
        }
    }

    // 3. Serialize
    let s = Node::new(node_val).to_string();
    assert!(s.contains("// Age in years"));
    assert!(s.contains("\"age\":10"));
}

#[derive(JwcSerializable, JwcDeserializable, PartialEq, Debug)]
struct FatObject {
    simple: TestObject,
    nested: NestedObject,
    exhaustive: ExhaustiveStruct,
    complex: ComplexCollections,
}

#[test]
fn test_fat_object() {
    let simple = TestObject {
        name: "Simple".to_string(),
        age: 1,
    };
    let nested = NestedObject {
        id: 2,
        child: simple.clone(),
    };

    let exhaustive = ExhaustiveStruct {
        b: true,
        i8_: 8,
        i16_: 16,
        i32_: 32,
        i64_: 64,
        u8_: 8,
        u16_: 16,
        u32_: 32,
        u64_: 64,
        f32_: 1.0,
        f64_: 2.0,
        string: "s".to_string(),
        vec_int: vec![1],
        opt_some: Some(1),
        opt_none: None,
        unit: (),
    };

    let mut map = HashMap::new();
    map.insert("key".to_string(), simple.clone());
    let complex = ComplexCollections {
        user_map: map,
        user_vec: vec![simple.clone()],
    };

    let fat = FatObject {
        simple,
        nested,
        exhaustive,
        complex,
    };

    // Serialize
    let val = fat.to_jwc();
    let s = val.to_string();
    println!("FatObject: {s}");

    // Deserialize
    let fat2 = FatObject::from_jwc(val).expect("Failed to deserialize fat object");
    assert_eq!(fat, fat2);
}
