use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn build_libmv(manifest_dir: &str) {
    let libmv_src = Path::new(manifest_dir).join("libmv/src");
    let bin_dir = Path::new(manifest_dir).join("libmv/bin-static-minimal");
    std::fs::create_dir_all(&bin_dir).expect("failed to create bin-static-minimal dir");

    let eigen_dir = Path::new(manifest_dir).join("libmv/src/third_party/eigen");

    // Configure
    let mut command = Command::new("cmake");
    command
        .current_dir(&bin_dir)
        .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5")
        .arg("-DBUILD_SHARED_LIBS=OFF")
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg(format!("-DEIGEN_INCLUDE_DIR={}", eigen_dir.display()))
        .arg("-DSUITESPARSE=OFF")
        .arg("-DCXSPARSE=OFF")
        .arg("-DLAPACK=OFF")
        .arg("-DOPENMP=OFF");

    #[cfg(windows)]
    command.arg("-DMINIGLOG=ON");

    let status = command
        .arg("-DBUILD_TESTING=OFF")
        .arg("-DCMAKE_SKIP_INSTALL_RULES=TRUE")
        .arg(&libmv_src)
        .status()
        .expect("cmake configure failed");

    assert!(status.success(), "cmake configure step failed");

    // Build
    let status = Command::new("cmake")
        .current_dir(&bin_dir)
        .arg("--build")
        .arg(".")
        .arg("--config")
        .arg("Release")
        .arg("--parallel")
        .arg("8")
        .status()
        .expect("cmake build failed");

    assert!(status.success(), "cmake build step failed");
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    build_libmv(&manifest_dir);

    let libmv_library_dir = if cfg!(feature = "dynamic") {
        let libmv_library_dir = Path::new(&manifest_dir).join("libmv/bin-opt/lib");
        if !libmv_library_dir.join("libmultiview.so").exists() {
            panic!("Missing compiled libmultiview.so! (libmv build failure?)");
        }
        libmv_library_dir
    } else {
        #[cfg(windows)]
        let libmv_library_dir = {
            // CMakeLists.txt sets CMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE to bin-static-minimal/lib directly
            let d = Path::new(&manifest_dir).join("libmv/bin-static-minimal/lib");
            if !d.join("multiview.lib").exists() {
                panic!("Missing compiled multiview.lib! (libmv build failure?)");
            }
            d
        };
        #[cfg(not(windows))]
        let libmv_library_dir = {
            let d = Path::new(&manifest_dir).join("libmv/bin-static-minimal/lib");
            if !d.join("libmultiview.a").exists() {
                panic!("Missing compiled libmultiview.a! (libmv build failure?)");
            }
            d
        };
        libmv_library_dir
    };

    println!(
        "cargo:rustc-link-search=native={}",
        libmv_library_dir.display()
    );

    // libmv libraries
    if cfg!(feature = "dynamic") {
        println!("cargo:rustc-link-lib=dylib=autotrack");
        println!("cargo:rustc-link-lib=dylib=base");
        println!("cargo:rustc-link-lib=dylib=camera");
        println!("cargo:rustc-link-lib=dylib=correspondence");
        println!("cargo:rustc-link-lib=dylib=descriptor");
        println!("cargo:rustc-link-lib=dylib=detector");
        println!("cargo:rustc-link-lib=dylib=image");
        println!("cargo:rustc-link-lib=dylib=image_io");
        println!("cargo:rustc-link-lib=dylib=multiview");
        println!("cargo:rustc-link-lib=dylib=numeric");
        println!("cargo:rustc-link-lib=dylib=reconstruction");
        println!("cargo:rustc-link-lib=dylib=simple_pipeline");
        println!("cargo:rustc-link-lib=dylib=tools");
        println!("cargo:rustc-link-lib=dylib=tracking");

        // Dependencies required for the C API wrapper itself
        println!("cargo:rustc-link-lib=dylib=gflags");
        println!("cargo:rustc-link-lib=dylib=glog");
    } else {
        println!("cargo:rustc-link-lib=static=autotrack");
        println!("cargo:rustc-link-lib=static=base");
        println!("cargo:rustc-link-lib=static=camera");
        println!("cargo:rustc-link-lib=static=correspondence");
        println!("cargo:rustc-link-lib=static=descriptor");
        println!("cargo:rustc-link-lib=static=detector");
        println!("cargo:rustc-link-lib=static=image");
        // println!("cargo:rustc-link-lib=static=image_io"); // appears to be included within `image` itself as well
        println!("cargo:rustc-link-lib=static=multiview");
        println!("cargo:rustc-link-lib=static=numeric");
        println!("cargo:rustc-link-lib=static=reconstruction");
        println!("cargo:rustc-link-lib=static=simple_pipeline");
        println!("cargo:rustc-link-lib=static=tools");
        println!("cargo:rustc-link-lib=static=tracking");

        // Dependencies required for the C API wrapper itself
        println!("cargo:rustc-link-lib=static=gflags");
        println!("cargo:rustc-link-lib=static=glog");

        // Runtime dependencies of libmv
        println!("cargo:rustc-link-lib=static=colamd");
        println!("cargo:rustc-link-lib=static=daisy");
        println!("cargo:rustc-link-lib=static=fast");
        println!("cargo:rustc-link-lib=static=flann");
        println!("cargo:rustc-link-lib=static=ldl");
        println!("cargo:rustc-link-lib=static=V3D");
        println!("cargo:rustc-link-lib=static=ceres");
    }

    // libpng + zlib: on Linux these are system dylibs; on Windows built statically by libmv's cmake
    #[cfg(not(windows))]
    println!("cargo:rustc-link-lib=dylib=png");
    #[cfg(windows)]
    {
        println!("cargo:rustc-link-lib=static=png");
        println!("cargo:rustc-link-lib=static=zlib");
    }

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
        .include("libmv/src/third_party/png")
        .include("libmv/src/third_party/zlib")
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
