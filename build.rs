use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=libmv-c.h");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let libmv_library_dir = Path::new(&manifest_dir).join("libmv/bin-opt/lib");
    println!(
        "cargo:rustc-link-search=native={}",
        libmv_library_dir.display()
    );

    println!("cargo:rustc-link-lib=multiview");

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
