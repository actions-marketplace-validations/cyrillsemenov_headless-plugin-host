use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let vst3_include = manifest_dir
        .join("../../lib/vst3_c_api")
        .canonicalize()
        .expect(
            "lib/vst3_c_api submodule must be initialized at ../../lib/vst3_c_api (run: `git submodule update --init --recursive`)",
        );

    let vst3_header = vst3_include.join("vst3_c_api.h");
    assert!(
        vst3_header.exists(),
        "Expected VST3 C API header at {} (run: `git submodule update --init --recursive`)",
        vst3_header.display()
    );

    println!("cargo:include={}", vst3_include.display());
    println!("cargo:rerun-if-changed={}", vst3_header.display());
}
