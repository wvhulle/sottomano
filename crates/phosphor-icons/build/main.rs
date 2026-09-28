mod phosphor;

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build");
    println!("cargo:rerun-if-env-changed=PHOSPHOR_SVG");
    let assets_dir: PathBuf = env::var_os("PHOSPHOR_SVG")
        .expect("PHOSPHOR_SVG is set by the flake dev shell")
        .into();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("icons.rs"), phosphor::generate(&assets_dir)).unwrap();
}
