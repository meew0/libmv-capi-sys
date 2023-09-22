use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    Command::new("make")
        .current_dir(Path::new(&manifest_dir).join("libmv"))
        .arg("release")
        .status()
        .expect("failed to run make");

    let libmv_library_dir = Path::new(&manifest_dir).join("libmv/bin-opt/lib");
    if !libmv_library_dir.join("libmultiview.so").exists() {
        panic!("Missing compiled libmultiview.so!");
    }

    println!(
        "cargo:rustc-link-search=native={}",
        libmv_library_dir.display()
    );

    // libmv libraries
    // TODO: investigate whether it's possible to bundle all of these into one,
    // to avoid this mess...
    println!("cargo:rustc-link-lib=autotrack");
    println!("cargo:rustc-link-lib=base");
    println!("cargo:rustc-link-lib=camera");
    println!("cargo:rustc-link-lib=correspondence");
    println!("cargo:rustc-link-lib=descriptor");
    println!("cargo:rustc-link-lib=detector");
    println!("cargo:rustc-link-lib=image");
    println!("cargo:rustc-link-lib=image_io");
    println!("cargo:rustc-link-lib=multiview");
    println!("cargo:rustc-link-lib=numeric");
    println!("cargo:rustc-link-lib=reconstruction");
    println!("cargo:rustc-link-lib=simple_pipeline");
    println!("cargo:rustc-link-lib=tools");
    println!("cargo:rustc-link-lib=tracking");

    // Dependencies required for the C API wrapper itself
    println!("cargo:rustc-link-lib=png");
    println!("cargo:rustc-link-lib=gflags");
    println!("cargo:rustc-link-lib=glog");

    // Compilation script adapted from https://github.com/h33p/ofps/blob/b18a0dda2981def429634834b4bce0acfbeffa22/libmv-rust/build.rs

    let src = [
        "capi/intern/autotrack.cc",
        "capi/intern/image.cc",
        "capi/intern/homography.cc",
        "capi/intern/reconstruction.cc",
        "capi/intern/frame_accessor.cc",
        "capi/intern/detector.cc",
        "capi/intern/camera_intrinsics.cc",
        "capi/intern/tracks.cc",
        "capi/intern/tracksN.cc",
        "capi/intern/logging.cc",
        "capi/intern/track_region.cc",
    ];

    for i in &src {
        println!("cargo:rerun-if-changed={}", i);
    }

    let mut builder = cc::Build::new();

    // Disable warnings coming from eigen
    let build = builder
        .cpp(true)
        .files(src.iter())
        .define("LIBMV_GFLAGS_NAMESPACE", Some("gflags"))
        .include("libmv/src/")
        .include("capi")
        .include("libmv/src/third_party/eigen")
        .include("libmv/src/third_party/glog/src")
        .include("libmv/src/third_party/gflags")
        .flag_if_supported("-Wno-deprecated-declarations")
        .flag_if_supported("-Wno-ignored-attributes")
        .flag_if_supported("-Wno-int-in-bool-context")
        .flag_if_supported("-Wno-deprecated-copy")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-misleading-indentation");

    build.compile("mv-capi");

    let capi_absolute_path = std::fs::canonicalize(Path::new(&manifest_dir).join("capi"))
        .expect("canonicalizing the capi path should succeed");

    let bindings = bindgen::Builder::default()
        .header("capi/libmv-capi.h")
        .clang_arg(format!("-I{}", capi_absolute_path.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .derive_default(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
