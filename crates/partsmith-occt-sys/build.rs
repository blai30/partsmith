//! Compiles the C++ shim and links the static OCCT toolkits from `OCCT_ROOT`.

use std::env;
use std::path::PathBuf;

/// Static OCCT toolkits in link order: each toolkit precedes the toolkits it depends on.
/// Derived from the EXTERNLIB files of the pinned OCCT release (see scripts/build-occt.sh).
const TOOLKITS: &[&str] = &[
    "TKDESTL",
    "TKDESTEP",
    "TKXCAF",
    "TKVCAF",
    "TKV3d",
    "TKHLR",
    "TKService",
    "TKCAF",
    "TKLCAF",
    "TKCDF",
    "TKXSBase",
    "TKDE",
    "TKMesh",
    "TKFeat",
    "TKOffset",
    "TKFillet",
    "TKBool",
    "TKBO",
    "TKShHealing",
    "TKPrim",
    "TKTopAlgo",
    "TKGeomAlgo",
    "TKBRep",
    "TKGeomBase",
    "TKG3d",
    "TKG2d",
    "TKMath",
    "TKernel",
];

const SOURCES: &[&str] = &["cpp/shim.cpp"];

fn main() {
    let target = env::var("TARGET").expect("cargo sets TARGET for build scripts");
    let emscripten = target.contains("emscripten");

    // A target-specific install (OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN) wins over OCCT_ROOT,
    // so native and WebAssembly builds can coexist in one shell.
    let target_variable = format!("OCCT_ROOT_{}", target.to_uppercase().replace('-', "_"));
    println!("cargo:rerun-if-env-changed={target_variable}");
    println!("cargo:rerun-if-env-changed=OCCT_ROOT");
    let root = PathBuf::from(
        env::var(&target_variable)
            .or_else(|_| env::var("OCCT_ROOT"))
            .expect("OCCT_ROOT must point at an OCCT install built with scripts/build-occt.sh"),
    );

    let mut build = cxx_build::bridge("src/ffi.rs");
    build
        .files(SOURCES)
        .include(root.join("include/opencascade"))
        .std("c++17");
    if emscripten {
        build.flag("-fwasm-exceptions");
    }
    build.compile("partsmith_occt_shim");

    println!("cargo:rustc-link-search=native={}", root.join("lib").display());
    for toolkit in TOOLKITS {
        println!("cargo:rustc-link-lib=static={toolkit}");
    }
    if target.contains("linux") {
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=dl");
    }

    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=cpp");
}
