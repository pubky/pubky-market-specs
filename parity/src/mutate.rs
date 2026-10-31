//! Deterministic structural mutations of a JSON document.
//!
//! Every node of the document is replaced in turn by a fixed set of values
//! (other types, boundary numbers, edge-case strings, and variations of its
//! own value), every object member is removed in turn, and every object
//! gains an unknown member. The set is the same on every run, so a parity
//! failure always reproduces.

use serde_json::{json, Value};

#[derive(Clone, Debug)]
enum Seg {
    Key(String),
    Index(usize),
}

fn label(path: &[Seg], what: &str) -> String {
    let mut out = String::from("$");
    for seg in path {
        match seg {
            Seg::Key(key) => {
                out.push('.');
                out.push_str(key);
            }
            Seg::Index(index) => out.push_str(&format!("[{index}]")),
        }
    }
    format!("{out} {what}")
}

fn node_mut<'a>(root: &'a mut Value, path: &[Seg]) -> &'a mut Value {
    let mut node = root;
    for seg in path {
        node = match seg {
            Seg::Key(key) => node.get_mut(key.as_str()).expect("path exists"),
            Seg::Index(index) => node.get_mut(*index).expect("path exists"),
        };
    }
    node
}

fn with_replaced(root: &Value, path: &[Seg], value: Value) -> Value {
    let mut copy = root.clone();
    *node_mut(&mut copy, path) = value;
    copy
}

fn with_removed(root: &Value, path: &[Seg], key: &str) -> Value {
    let mut copy = root.clone();
    if let Value::Object(map) = node_mut(&mut copy, path) {
        map.remove(key);
    }
    copy
}

fn generic_values() -> Vec<(&'static str, Value)> {
    vec![
        ("null", Value::Null),
        ("true", json!(true)),
        ("false", json!(false)),
        ("0", json!(0)),
        ("-1", json!(-1)),
        ("1", json!(1)),
        ("1.5", json!(1.5)),
        ("2^53-1", json!(9_007_199_254_740_991_i64)),
        ("2^53", json!(9_007_199_254_740_992_i64)),
        ("-2^53", json!(-9_007_199_254_740_992_i64)),
        ("u64 max", json!(u64::MAX)),
        ("empty string", json!("")),
        ("space", json!(" ")),
        ("x", json!("x")),
        ("long string", json!("a".repeat(6000))),
        ("non-ascii", json!("\u{00e9}t\u{00e9}")),
        ("empty array", json!([])),
        ("empty object", json!({})),
    ]
}

fn own_values(node: &Value) -> Vec<(String, Value)> {
    match node {
        Value::String(s) => {
            let chars: Vec<char> = s.chars().collect();
            let half: String = chars[..chars.len() / 2].iter().collect();
            let mut out = vec![
                ("uppercase".to_string(), json!(s.to_uppercase())),
                ("padded".to_string(), json!(format!("  {s}  "))),
                ("trailing newline".to_string(), json!(format!("{s}\n"))),
                ("first half".to_string(), json!(half)),
                ("doubled".to_string(), json!(format!("{s}{s}"))),
                ("plus non-ascii".to_string(), json!(format!("{s}\u{00e9}"))),
            ];
            if let Some((first, rest)) = s.split_at_checked(1) {
                out.push(("drop first char".to_string(), json!(rest)));
                let flipped = if first == "Z" { "z" } else { "Z" };
                out.push((
                    "first char Z".to_string(),
                    json!(format!("{flipped}{rest}")),
                ));
            }
            if let Some(last) = chars.last() {
                let mut swapped: String = chars[..chars.len() - 1].iter().collect();
                swapped.push(if *last == 'y' { 'o' } else { 'y' });
                out.push(("last char swapped".to_string(), json!(swapped)));
            }
            out
        }
        Value::Number(n) => {
            let mut out = Vec::new();
            if let Some(v) = n.as_i64() {
                for (name, delta) in [("+1", 1_i64), ("-1", -1)] {
                    if let Some(x) = v.checked_add(delta) {
                        out.push((format!("value {name}"), json!(x)));
                    }
                }
                if let Some(x) = v.checked_neg() {
                    out.push(("negated".to_string(), json!(x)));
                }
                if let Some(x) = v.checked_mul(1000) {
                    out.push(("x1000".to_string(), json!(x)));
                }
                out.push(("as float".to_string(), json!(v as f64 + 0.5)));
                out.push(("as string".to_string(), json!(v.to_string())));
            }
            out
        }
        Value::Array(items) => {
            let mut out = Vec::new();
            if let Some(first) = items.first() {
                let mut dup = items.clone();
                dup.push(first.clone());
                out.push(("duplicate first".to_string(), Value::Array(dup)));
                out.push(("drop first".to_string(), Value::Array(items[1..].to_vec())));
                let mut reversed = items.clone();
                reversed.reverse();
                out.push(("reversed".to_string(), Value::Array(reversed)));
                out.push((
                    "repeat first x600".to_string(),
                    Value::Array(vec![first.clone(); 600]),
                ));
            }
            out
        }
        Value::Object(map) => {
            let mut with_unknown = map.clone();
            with_unknown.insert("zzUnknownMember".to_string(), json!(1));
            vec![("unknown member".to_string(), Value::Object(with_unknown))]
        }
        _ => Vec::new(),
    }
}

fn walk(root: &Value, path: &mut Vec<Seg>, node: &Value, out: &mut Vec<(String, Value)>) {
    for (name, value) in generic_values() {
        out.push((
            label(path, &format!("= {name}")),
            with_replaced(root, path, value),
        ));
    }
    for (name, value) in own_values(node) {
        out.push((
            label(path, &format!("= {name}")),
            with_replaced(root, path, value),
        ));
    }
    match node {
        Value::Object(map) => {
            for (key, child) in map {
                path.push(Seg::Key(key.clone()));
                out.push((
                    label(path, "removed"),
                    with_removed(root, &path[..path.len() - 1], key),
                ));
                walk(root, path, child, out);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push(Seg::Index(index));
                walk(root, path, child, out);
                path.pop();
            }
        }
        _ => {}
    }
}

/// All mutations of `seed`, each with a human-readable label.
pub fn mutations(seed: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    walk(seed, &mut Vec::new(), seed, &mut out);
    out
}
