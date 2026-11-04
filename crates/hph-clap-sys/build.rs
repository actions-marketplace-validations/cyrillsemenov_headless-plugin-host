use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let clap_include = manifest_dir
        .join("../../lib/clap/include")
        .canonicalize()
        .expect(
            "lib/clap submodule must be initialized at ../../lib/clap (run: `git submodule update --init --recursive`)",
        );

    let clap_header = clap_include.join("clap/clap.h");
    assert!(
        clap_header.exists(),
        "Expected CLAP header at {} (run: `git submodule update --init --recursive`)",
        clap_header.display()
    );

    println!("cargo:include={}", clap_include.display());
    println!("cargo:rerun-if-changed={}", clap_header.display());
}
