use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

// For selectively building libmv/the C API, we divide it into “areas”.
// The `sources`/`headers`/`targets` of every enabled area are unioned together
// to decide what actually gets compiled and linked.
//
// The sets were determined by linking each area's `capi/intern` objects against libmv and
// resolving the undefined symbols that remained, so they are guaranteed to be minimal.
// Adding a library will cause additional link time for no benefit,
// while removing one will cause link errors for dependent crates.
struct Area {
    /// Cargo feature enabling this area, or `None` if it always needs to be built.
    feature: Option<&'static str>,

    /// Sources under `capi/intern` implementing it.
    sources: &'static [&'static str],

    /// Headers under `capi/intern` declaring it.
    ///
    /// Only these are included in the generated bindings,
    /// such that calling a function belonging to a disabled area
    /// is a compile error at the call site rather than an
    /// undefined symbol at link time.
    headers: &'static [&'static str],

    /// libmv CMake targets that are required for this area.
    /// CMake builds each target's own dependencies,
    /// so only the directly referenced ones are listed.
    targets: &'static [&'static str],
}

// Note that an area does *not* list the sources of the areas it depends on;
// those are specified through the corresponding Cargo feature
// (`track-region` enables `image`, and so on).
const AREAS: &[Area] = &[
    Area {
        feature: None,
        sources: &["logging"],
        headers: &["logging"],
        targets: &["gflags", "glog"],
    },
    Area {
        feature: Some("image"),
        sources: &["image"],
        headers: &["image"],
        targets: &[
            "ceres",
            "gflags",
            "glog",
            "image",
            "multiview",
            "numeric",
            "tracking",
        ],
    },
    Area {
        feature: Some("track-region"),
        sources: &["track_region"],
        headers: &["track_region"],
        targets: &[
            "ceres",
            "gflags",
            "glog",
            "image",
            "multiview",
            "numeric",
            "tracking",
        ],
    },
    Area {
        feature: Some("homography"),
        sources: &["homography"],
        headers: &["homography"],
        targets: &["ceres", "gflags", "glog", "multiview", "numeric"],
    },
    Area {
        feature: Some("camera-intrinsics"),
        sources: &["camera_intrinsics"],
        headers: &["camera_intrinsics"],
        targets: &["gflags", "glog", "simple_pipeline"],
    },
    Area {
        feature: Some("tracks"),
        sources: &["tracks"],
        headers: &["tracks"],
        targets: &["simple_pipeline"],
    },
    Area {
        feature: Some("detector"),
        sources: &["detector"],
        headers: &["detector"],
        targets: &[
            "ceres",
            "fast",
            "gflags",
            "glog",
            "image",
            "multiview",
            "numeric",
            "simple_pipeline",
            "tracking",
        ],
    },
    Area {
        feature: Some("autotrack"),
        sources: &["autotrack", "tracksN", "frame_accessor"],
        headers: &["autotrack", "tracksN", "frame_accessor", "region"],
        targets: &[
            "autotrack",
            "ceres",
            "gflags",
            "glog",
            "image",
            "multiview",
            "numeric",
            "tracking",
        ],
    },
    Area {
        feature: Some("reconstruction"),
        sources: &["reconstruction"],
        headers: &["reconstruction"],
        targets: &[
            "ceres",
            "gflags",
            "glog",
            "multiview",
            "numeric",
            "simple_pipeline",
        ],
    },
];

/// Order in which the static libraries must be passed to the linker.
/// Every library appears before the ones it depends on.
/// Selected targets are emitted in this order rather than the order they were collected in.
const LINK_ORDER: &[&str] = &[
    "autotrack",
    "simple_pipeline",
    "reconstruction",
    "tracking",
    "camera",
    "correspondence",
    "descriptor",
    "detector",
    "multiview",
    "image",
    "numeric",
    "base",
    "tools",
    "ceres",
    "V3D",
    "daisy",
    "fast",
    "flann",
    "colamd",
    "ldl",
    "glog",
    "gflags",
];

fn feature_enabled(feature: &str) -> bool {
    // Cargo uppercases feature names and replaces `-` with `_`.
    let var = format!("CARGO_FEATURE_{}", feature.to_uppercase().replace('-', "_"));
    env::var_os(var).is_some()
}

fn enabled_areas() -> Vec<&'static Area> {
    AREAS
        .iter()
        .filter(|area| area.feature.is_none_or(feature_enabled))
        .collect()
}

/// Configures and builds libmv, returning the directory holding the resulting
/// static libraries.
fn build_libmv(manifest_dir: &str, out_dir: &str, targets: &BTreeSet<&str>) -> PathBuf {
    let libmv_src = Path::new(manifest_dir).join("libmv/src");
    let bin_dir = Path::new(out_dir).join("libmv/bin-static-minimal");
    std::fs::create_dir_all(&bin_dir).expect("failed to create bin-static-minimal dir");

    let eigen_dir = Path::new(manifest_dir).join("libmv/src/third_party/eigen");

    // Configure
    let mut command = Command::new("cmake");
    command
        .current_dir(&bin_dir)
        .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5")
        .arg("-DBUILD_SHARED_LIBS=OFF")
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg("-DCMAKE_POSITION_INDEPENDENT_CODE=ON")
        .arg(format!("-DEIGEN_INCLUDE_DIR={}", eigen_dir.display()))
        .arg("-DSUITESPARSE=OFF")
        .arg("-DCXSPARSE=OFF")
        .arg("-DLAPACK=OFF")
        .arg("-DOPENMP=OFF");

    // Ceres' fixed-size Schur eliminator specializations are half of its
    // translation units and a fifth of the whole build, but they only speed up
    // bundle adjustment, which is reached only through `reconstruction`.
    // Everything else (`track_region` in particular) solves tiny dense
    // problems with DENSE_QR and will never benefit from those,
    // so we can disable the specializations otherwise.
    if !feature_enabled("reconstruction") {
        command.arg("-DSCHUR_SPECIALIZATIONS=OFF");
    }

    // MSVC removes std::binder1st/binder2nd (C++17) and std::tr1 that the
    // bundled Eigen 3.2.7 and gtest rely on; C++14 keeps them. GCC/Clang are
    // fine with C++17 because they retain those symbols as extensions.
    #[cfg(windows)]
    {
        command.arg("-DCMAKE_CXX_STANDARD=14");
        command.arg("-DMINIGLOG=ON");
    }
    #[cfg(not(windows))]
    command.arg("-DCMAKE_CXX_STANDARD=17");

    command.arg("-DCMAKE_CXX_STANDARD_REQUIRED=ON");

    let status = command
        .arg("-DBUILD_TESTING=OFF")
        .arg("-DCMAKE_SKIP_INSTALL_RULES=TRUE")
        .arg(&libmv_src)
        .status()
        .expect("cmake configure could not be run (is cmake installed & accessible?)");

    assert!(
        status.success(),
        "cmake configure step completed unsuccessfully"
    );

    // Build only the targets the enabled areas actually need.
    // CMake pulls in each target's own dependencies,
    // so e.g. asking for `simple_pipeline` also builds `V3D`, `multiview` and `image`.
    let mut command = Command::new("cmake");
    command
        .current_dir(&bin_dir)
        .arg("--build")
        .arg(".")
        .arg("--config")
        .arg("Release")
        // Cargo sets NUM_JOBS from its own `-j`, so this respects the job limit
        // the user asked for (e.g. when Cargo is running several build scripts at once)
        .arg("--parallel")
        .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "8".to_owned()))
        .arg("--target");

    for target in targets {
        command.arg(target);
    }

    let status = command.status().expect("cmake build could not be run");

    assert!(
        status.success(),
        "cmake build step completed unsuccessfully"
    );

    // CMakeLists.txt sets CMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE to
    // bin-static-minimal/lib directly on both platforms.
    bin_dir.join("lib")
}

/// Panics with a useful message if a target that was asked for did not produce
/// a static library, which otherwise shows up as an inscrutable link error.
fn verify_built(library_dir: &Path, targets: &BTreeSet<&str>) {
    for target in targets {
        #[cfg(windows)]
        let library = library_dir.join(format!("{target}.lib"));
        #[cfg(not(windows))]
        let library = library_dir.join(format!("lib{target}.a"));

        assert!(
            library.exists(),
            "Missing compiled {}! (libmv build failure?)",
            library.display()
        );
    }
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    let areas = enabled_areas();

    // Without this, edits to the libmv submodule, including its CMakeLists,
    // do not re-run this script, so cmake is never re-invoked and the change
    // silently does not take effect.
    println!("cargo:rerun-if-changed=libmv/src");

    let targets: BTreeSet<&str> = areas
        .iter()
        .flat_map(|area| area.targets)
        .copied()
        .collect();

    let library_dir = build_libmv(&manifest_dir, &out_dir, &targets);
    verify_built(&library_dir, &targets);

    println!("cargo:rustc-link-search=native={}", library_dir.display());

    assert!(
        targets.iter().all(|target| LINK_ORDER.contains(target)),
        "every CMake target in AREAS must appear in LINK_ORDER so that it is \
         passed to the linker in dependency order"
    );

    for library in LINK_ORDER.iter().filter(|name| targets.contains(*name)) {
        println!("cargo:rustc-link-lib=static={library}");
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

    let sources: Vec<String> = areas
        .iter()
        .flat_map(|area| area.sources)
        .map(|source| format!("capi/intern/{source}.cc"))
        .collect();

    for source in &sources {
        println!("cargo:rerun-if-changed={source}");
    }

    let mut builder = cc::Build::new();

    // Disable warnings coming from eigen
    let build = builder
        .cpp(true)
        .files(sources.iter())
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
        .flag_if_supported("-Wno-misleading-indentation")
        .flag_if_supported("-Wno-deprecated-enum-enum-conversion");

    build.compile("mv-capi");

    let capi_absolute_path = std::fs::canonicalize(Path::new(&manifest_dir).join("capi"))
        .expect("canonicalizing the capi path should succeed");

    // The header must be given as an absolute path
    let mut bindings = bindgen::Builder::default()
        .header(
            capi_absolute_path
                .join("libmv-capi.h")
                .display()
                .to_string(),
        )
        .clang_arg(format!("-I{}", capi_absolute_path.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .derive_default(true);

    // `libmv-capi.h` includes every area's header regardless of which ones are
    // being built, so we need to restrict the output to the enabled ones.
    // bindgen will still emit any type they transitively depend on.
    for header in areas.iter().flat_map(|area| area.headers) {
        bindings = bindings.allowlist_file(format!(
            ".*{}",
            regex_escape(
                &capi_absolute_path
                    .join(format!("intern/{header}.h"))
                    .display()
                    .to_string()
            )
        ));
    }

    let bindings = bindings.generate().expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

/// Escapes the regex metacharacters that can occur in a path, so that
/// `allowlist_file` matches it literally.
fn regex_escape(path: &str) -> String {
    path.chars()
        .flat_map(|c| {
            let escape = matches!(
                c,
                '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\'
            );
            escape.then_some('\\').into_iter().chain(std::iter::once(c))
        })
        .collect()
}
