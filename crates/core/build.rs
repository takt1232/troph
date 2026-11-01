use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=../../VERSION");
    let version = fs::read_to_string("../../VERSION")
        .expect("cannot read VERSION")
        .trim()
        .to_string();
    println!("cargo:rustc-env=APP_VERSION={version}");
}
