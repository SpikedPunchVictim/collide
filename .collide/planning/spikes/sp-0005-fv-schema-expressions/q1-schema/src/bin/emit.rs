//! Emits each pipeline's artifacts into the directory in argv[1]:
//!   schemars -> schema.json ; ts-rs -> ts-rs/*.ts ; specta -> specta.ts
use std::{fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: emit <out-dir>"));
    fs::create_dir_all(&out).unwrap();

    #[cfg(feature = "schemars")]
    {
        let schema = schemars::schema_for!(s5_schema::PackManifest);
        fs::write(out.join("schema.json"), serde_json::to_string_pretty(&schema).unwrap()).unwrap();
        println!("wrote schema.json");
        use schemars::generate::SchemaSettings;
        let ser = SchemaSettings::draft2020_12().for_serialize().into_generator().into_root_schema_for::<s5_schema::PackManifest>();
        fs::write(out.join("schema.serialize.json"), serde_json::to_string_pretty(&ser).unwrap()).unwrap();
        println!("wrote schema.serialize.json");
    }

    #[cfg(feature = "ts")]
    {
        use ts_rs::TS;
        let dir = if cfg!(feature = "ts-fix") { "ts-rs-optional" } else { "ts-rs" };
        let cfg = ts_rs::Config::default().with_out_dir(out.join(dir));
        s5_schema::PackManifest::export_all(&cfg).unwrap();
        println!("wrote {dir}/*.ts");
    }

    #[cfg(feature = "specta")]
    {
        use specta::{Types};
        let types = Types::default().register::<s5_schema::PackManifest>();
        specta_typescript::Typescript::default()
            .export_to(out.join("specta.ts"), &types, specta_serde::Format)
            .unwrap();
        println!("wrote specta.ts");
    }
    let _ = &out;
}
