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

/// Every C++ source of the shim, split by area (kernel spec section 3.9).
const SOURCES: &[&str] = &[
    "cpp/primitives.cpp",
    "cpp/profiles.cpp",
    "cpp/sweeps.cpp",
    "cpp/booleans.cpp",
    "cpp/blends.cpp",
    "cpp/transforms.cpp",
    "cpp/queries.cpp",
    "cpp/measure.cpp",
    "cpp/mesh.cpp",
    "cpp/mesh_algorithm.cpp",
    "cpp/delabella/delabella.cpp",
    "cpp/exchange.cpp",
    "cpp/shape.cpp",
    "cpp/validity.cpp",
    "cpp/history.cpp",
];

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
        .std("c++17")
        // As OCCT compiles its static toolkits, so its headers declare nothing dllexport or
        // dllimport on Windows; elsewhere the define changes nothing (kernel spec 3.12 item 5).
        .define("OCCT_STATIC_BUILD", None);
    if build.get_compiler().is_like_msvc() {
        // Standard C++ unwinding, so an exception releases the locks of the frames it leaves
        // (kernel spec section 3.9 item 9); cl.exe destroys no automatic object without it.
        build.flag("/EHsc");
    } else {
        // No multiplication and addition fused into one operation, which the double-double
        // arithmetic of the shim's triangulation relies on (kernel spec section 3.7 item 2).
        build.flag("-ffp-contract=off");
    }
    if emscripten {
        build.flag("-fwasm-exceptions");
    }
    build.compile("partsmith_occt_shim");

    println!(
        "cargo:rustc-link-search=native={}",
        root.join("lib").display()
    );
    for toolkit in TOOLKITS {
        println!("cargo:rustc-link-lib=static={toolkit}");
    }
    // macOS and Emscripten need nothing beyond the C++ standard library, which cxx-build links.
    let system_libraries: &[&str] = if target.contains("linux") {
        &["pthread", "dl"]
    } else if target.contains("windows") {
        &["advapi32", "gdi32", "user32", "wsock32", "psapi"]
    } else {
        &[]
    };
    for library in system_libraries {
        println!("cargo:rustc-link-lib=dylib={library}");
    }

    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=cpp");
}
