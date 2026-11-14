//! Ground truth: serde's verdict on each case, plus the value re-serialized from the Rust type (round trip).
use s5_schema::PackManifest;
use serde_json::{Value, json};

fn main() {
    let path = std::env::args().nth(1).expect("usage: verdict <cases.json>");
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut out = vec![];
    for c in cases {
        let value = &c["value"];
        match serde_json::from_value::<PackManifest>(value.clone()) {
            Ok(m) => {
                let again = serde_json::to_value(&m).unwrap();
                let m2: PackManifest = serde_json::from_value(again.clone()).unwrap();
                out.push(json!({"name": c["name"], "serde": "ok", "reserialized": again, "idempotent": m == m2}));
            }
            Err(e) => out.push(json!({"name": c["name"], "serde": "err", "message": e.to_string()})),
        }
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
