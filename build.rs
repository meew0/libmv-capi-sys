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

    // Relevant for Windows only.
    // On Linux these are system libraries rather than targets.
    "png",
    "zlib",
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

const CACHE_DIR_VAR: &str = "LIBMV_CAPI_SYS_CACHE_DIR";
const STUB_VAR: &str = "LIBMV_CAPI_SYS_STUB";

/// The hash function used to name cache directories.
///
/// Does not need to be cryptographic, but needs to be consistent across environments,
/// so we implement it manually rather than using the Rust stdlib.
fn hash(bytes: &[u8], seed: u64) -> u64 {
    bytes.iter().fold(seed, |accumulator, byte| {
        (accumulator ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Runs a command, returning its stdout if it succeeded.
fn output_of(program: &str, arguments: &[&str], directory: &Path) -> Option<String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .ok()?;

    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Generate a hash for the current state of the libmv source tree,
/// based on the current commit and the content hashes of uncommitted files.
fn libmv_revision(manifest_dir: &Path) -> String {
    let libmv = manifest_dir.join("libmv");

    let Some(commit) = output_of("git", &["rev-parse", "HEAD"], &libmv) else {
        return format!("v{}", env::var("CARGO_PKG_VERSION").unwrap_or_default());
    };

    let mut revision = commit.trim().to_owned();

    let dirty = output_of("git", &["status", "--porcelain", "-uall"], &libmv);
    for line in dirty.iter().flat_map(|status| status.lines()) {
        let Some(path) = line.get(3..) else { continue };
        let contents = std::fs::read(libmv.join(path)).unwrap_or_default();
        revision.push_str(&format!(
            "+{path}:{:016x}",
            hash(&contents, 0x1a2b_3c4d_5e6f_7a8b)
        ));
    }

    revision
}

/// Determine the directory the libmv build output should be stored in,
/// and whether something is already stored in that directory.
///
/// By default this is inside `OUT_DIR`, which Cargo discards on `cargo clean`
/// and does not share between profiles, so switching between debug and release
/// rebuilds all of libmv even though the cmake build is always `Release`.
/// Setting `LIBMV_CAPI_SYS_CACHE_DIR` moves it somewhere
/// such that the output persists across Cargo profiles.
///
/// The directory name is a hash of all the factors that might change the produced libraries:
/// the libmv sources, the cmake flags, the targets being built, the target triple and the compiler.
/// Notably it does *not* include the Cargo profile or anything else that varies `OUT_DIR`.
fn libmv_build_root(
    manifest_dir: &Path,
    out_dir: &Path,
    flags: &[String],
    targets: &BTreeSet<&str>,
    compiler: &str,
) -> (PathBuf, bool) {
    let mut key = 0x9a8b_7c6d_5e4f_3a2b;
    key = hash(libmv_revision(manifest_dir).as_bytes(), key);
    key = hash(compiler.as_bytes(), key);
    key = hash(env::var("TARGET").unwrap_or_default().as_bytes(), key);
    for flag in flags {
        key = hash(flag.as_bytes(), key);
    }
    for target in targets {
        key = hash(target.as_bytes(), key);
    }

    let Some(cache) = env::var_os(CACHE_DIR_VAR) else {
        return (out_dir.join(format!("libmv-{key:016x}")), false);
    };

    let cache = PathBuf::from(cache);
    if std::fs::create_dir_all(&cache).is_err() {
        // Fall back to OUT_DIR when we cannot use the specified cache directory
        println!(
            "cargo:warning={CACHE_DIR_VAR} is set to {} but could not be created; \
             building into OUT_DIR instead",
            cache.display()
        );
        return (out_dir.join(format!("libmv-{key:016x}")), false);
    }

    let directory = cache.join(format!("libmv-{key:016x}"));
    let cached = directory.is_dir();
    (directory, cached)
}

/// The C++ compiler cmake will use,
/// identified with enough precision such that a toolchain change,
/// but not a minor bugfix update, will invalidate the build cache.
fn compiler_identity() -> String {
    let compiler = cc::Build::new().cpp(true).get_compiler();
    let path = compiler.path().to_path_buf();

    let version = output_of(&path.display().to_string(), &["--version"], Path::new("."))
        .and_then(|text| text.lines().next().map(str::to_owned))
        .unwrap_or_default();

    // Keep the leading component of the first `x.y.z` found, if any.
    let major = version
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|token| token.split('.').count() >= 3)
        .and_then(|token| token.split('.').next())
        .unwrap_or("unknown");

    format!("{}-{major}", path.display())
}

/// The cmake flags used for building libmv.
fn cmake_flags(manifest_dir: &Path) -> Vec<String> {
    let eigen_dir = manifest_dir.join("libmv/src/third_party/eigen");

    let mut flags = vec![
        "-DCMAKE_POLICY_VERSION_MINIMUM=3.5".to_owned(),
        "-DBUILD_SHARED_LIBS=OFF".to_owned(),
        "-DCMAKE_BUILD_TYPE=Release".to_owned(),
        "-DCMAKE_POSITION_INDEPENDENT_CODE=ON".to_owned(),
        format!("-DEIGEN_INCLUDE_DIR={}", eigen_dir.display()),
        "-DSUITESPARSE=OFF".to_owned(),
        "-DCXSPARSE=OFF".to_owned(),
        "-DLAPACK=OFF".to_owned(),
        "-DOPENMP=OFF".to_owned(),
    ];

    // Ceres' fixed-size Schur eliminator specializations are half of its
    // translation units and a fifth of the whole build, but they only speed up
    // bundle adjustment, which is reached only through `reconstruction`.
    // Everything else (`track_region` in particular) solves tiny dense
    // problems with DENSE_QR and will never benefit from those,
    // so we can disable the specializations otherwise.
    if !feature_enabled("reconstruction") {
        flags.push("-DSCHUR_SPECIALIZATIONS=OFF".to_owned());
    }

    // MSVC removes std::binder1st/binder2nd (C++17) and std::tr1 that the
    // bundled Eigen 3.2.7 and gtest rely on; C++14 keeps them. GCC/Clang are
    // fine with C++17 because they retain those symbols as extensions.
    #[cfg(windows)]
    {
        flags.push("-DCMAKE_CXX_STANDARD=14".to_owned());
        flags.push("-DMINIGLOG=ON".to_owned());
    }
    #[cfg(not(windows))]
    flags.push("-DCMAKE_CXX_STANDARD=17".to_owned());

    flags.push("-DCMAKE_CXX_STANDARD_REQUIRED=ON".to_owned());
    flags.push("-DBUILD_TESTING=OFF".to_owned());
    flags.push("-DCMAKE_SKIP_INSTALL_RULES=TRUE".to_owned());

    flags
}

/// Configures and builds libmv, and returns the directory holding the resulting
/// static libraries.
///
/// The build steps are run in a scratch directory and only moved into place once
/// the build has succeeded, so an interrupted or failing build never leaves something
/// behind that a later run might mistake for a finished one.
fn build_libmv(
    manifest_dir: &Path,
    build_root: &Path,
    flags: &[String],
    targets: &BTreeSet<&str>,
) -> PathBuf {
    let libmv_src = manifest_dir.join("libmv/src");

    let scratch = build_root.with_extension(format!("partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("failed to create the libmv build directory");

    let status = Command::new("cmake")
        .current_dir(&scratch)
        .args(flags)
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
        .current_dir(&scratch)
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

    if !status.success() {
        let _ = std::fs::remove_dir_all(&scratch);
        panic!("cmake build step completed unsuccessfully");
    }

    if std::fs::rename(&scratch, build_root).is_err() {
        // Either another build finished first, or the scratch directory and the
        // cache are on different filesystems. Both are fine as long as the
        // libraries ended up somewhere; prefer whatever is already published.
        if build_root.is_dir() {
            let _ = std::fs::remove_dir_all(&scratch);
        } else {
            return scratch.join("lib");
        }
    }

    // CMakeLists.txt sets CMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE to
    // <build dir>/lib directly on both platforms.
    build_root.join("lib")
}

/// Builds the stub implementation of the C API instead of libmv and the real C API.
fn build_stub() {
    println!(
        "cargo:warning={STUB_VAR} is set: libmv is stubbed out. every libmv_* function will do nothing and report failure"
    );

    println!("cargo:rerun-if-changed=capi/intern/stub.cc");

    cc::Build::new()
        .cpp(true)
        .file("capi/intern/stub.cc")
        .include("capi")
        .include("libmv/src")
        .include("libmv/src/third_party/eigen")
        .flag_if_supported("-Wno-unused-parameter")
        .compile("mv-capi");
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
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

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

    // Toggling either of these has to re-run the script, or the previous run's
    // artifacts are silently kept and the change will appear to do nothing.
    println!("cargo:rerun-if-env-changed={STUB_VAR}");
    println!("cargo:rerun-if-env-changed={CACHE_DIR_VAR}");

    if env::var_os(STUB_VAR).is_some() {
        build_stub();
    } else {
        build_real(&manifest_dir, &out_dir, areas.as_slice(), &targets);
    }

    generate_bindings(&manifest_dir, areas.as_slice());
}

/// Builds libmv and the parts of the C API wrapper the enabled areas need.
fn build_real(
    manifest_dir: &Path,
    out_dir: &Path,
    areas: &[&'static Area],
    targets: &BTreeSet<&str>,
) {
    // libpng and zlib are system libraries on Linux,
    // but on Windows libmv builds its own bundled copies,
    // which makes them ordinary cmake targets that have to be requested explicitly.
    #[cfg(windows)]
    let targets = &{
        let mut targets = (*targets).clone();
        targets.insert("png");
        targets.insert("zlib");
        targets
    };

    let flags = cmake_flags(manifest_dir);
    let compiler = compiler_identity();
    let (build_root, cached) = libmv_build_root(manifest_dir, out_dir, &flags, targets, &compiler);

    let library_dir = if cached {
        build_root.join("lib")
    } else {
        build_libmv(manifest_dir, &build_root, &flags, targets)
    };

    verify_built(&library_dir, targets);

    println!("cargo:rustc-link-search=native={}", library_dir.display());

    assert!(
        targets.iter().all(|target| LINK_ORDER.contains(target)),
        "every CMake target in AREAS must appear in LINK_ORDER so that it is \
         passed to the linker in dependency order"
    );

    for library in LINK_ORDER.iter().filter(|name| targets.contains(*name)) {
        println!("cargo:rustc-link-lib=static={library}");
    }

    // On Linux libpng is a system shared library, so it is not a cmake target
    // and is not covered by the loop above.
    #[cfg(not(windows))]
    println!("cargo:rustc-link-lib=dylib=png");

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
}

/// Generates `bindings.rs` for the enabled areas. Identical in stub and real
/// builds, so that the two are interchangeable.
fn generate_bindings(manifest_dir: &Path, areas: &[&'static Area]) {
    // `CARGO_MANIFEST_DIR` is already absolute,
    // so we don't need any further canonicalization
    let capi_dir = manifest_dir.join("capi");

    let mut bindings = bindgen::Builder::default()
        .header(capi_dir.join("libmv-capi.h").display().to_string())
        .clang_arg(format!("-I{}", capi_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .derive_default(true);

    // `libmv-capi.h` includes every area's header regardless of which ones are
    // being built, so we need to restrict the output to the enabled ones.
    // bindgen will still emit any type they transitively depend on.
    //
    // These match on the trailing `intern/<name>.h` rather than on the full
    // path, and accept backslashes or forward slashes, to ensure reproducibility
    // across different platforms and build runs.
    for header in areas.iter().flat_map(|area| area.headers) {
        bindings = bindings.allowlist_file(format!(r".*[/\\]intern[/\\]{header}\.h"));
    }

    let bindings = bindings.generate().expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
