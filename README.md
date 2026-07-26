# `libmv-capi-sys`

![Crates.io Version](https://img.shields.io/crates/v/libmv-capi-sys)

Unsafe Rust FFI bindings around [libmv](https://projects.blender.org/blender/libmv), the motion tracking library used by [Blender](https://www.blender.org/). Makes use of [Blender's own C bindings](https://projects.blender.org/blender/blender/src/branch/main/intern/libmv/intern) combined with the [standalone distribution of libmv](https://projects.blender.org/blender/libmv).

## Usage

See [src/lib.rs](https://github.com/meew0/libmv-capi-sys/blob/master/src/lib.rs) for a very basic usage example.

Unfortunately, there is almost no documentation available on how to use libmv. In cases where the function names and signatures in the C header files aren't self-explanatory, your best bet is probably checking Blender's source code for how the functions are used.

## Features

By default the entirety of libmv is built, which takes a few minutes. If you only need part of the C API, you can enable only the features you need. For example, `track-region` alone is roughly three times faster to build than `full`.

| Feature | C API |
| --- | --- |
| `full` (default) | everything below |
| `image` | image conversion and planar patch sampling |
| `track-region` | tracking a single region between two images (implies `image`) |
| `homography` | 2D homography from correspondences |
| `camera-intrinsics` | camera intrinsics, distortion and undistortion |
| `tracks` | the `simple_pipeline` track container |
| `detector` | feature detection (implies `image`) |
| `autotrack` | autotrack, its track container and frame accessors (implies `track-region`) |
| `reconstruction` | modal and full reconstruction solving (implies `camera-intrinsics`) |

Logging is always available. Functions belonging to a disabled feature are left out of the generated bindings, so calling one will result in a compile-time error (rather than a link-time one).

```toml
[dependencies]
libmv-capi-sys = { version = "0.1", default-features = false, features = ["track-region"] }
```

## Environment variables

### `LIBMV_CAPI_SYS_CACHE_DIR`

Stores the built libmv libraries in the given directory instead of inside `OUT_DIR`.

`OUT_DIR` is discarded by `cargo clean` and is not shared between profiles, so by default a debug and a release build will build all of libmv separately, and any change that gives the crate a new metadata hash (a dependency bump, a `RUSTFLAGS` change, etc.) unnecessarily discards the build. The cmake build is always `Release` and does not depend on most of those factors, so setting the build target folder to a directory outside `target/` avoids this repetition.

Entries are named by a hash of everything that changes the libraries produced: the libmv sources (including uncommitted changes), the cmake flags, the enabled features, the target triple and the C++ compiler's major version. A stale entry will never be reused. Changing any of those details will simply build a new one. If the directory cannot be created, the build falls back to `OUT_DIR` with a warning, so this property is safe to set unconditionally.

A convenient way to set it for one project is via `.cargo/config.toml`:

```toml
[env]
LIBMV_CAPI_SYS_CACHE_DIR = { value = "target/libmv-cache", relative = true }
```

### `LIBMV_CAPI_SYS_STUB`

If enabled, a stub implementation of the C API will be built instead of libmv. In the stub implementation, every function exists but does nothing (and reports failure), and no cmake, Ceres or libmv build is needed at all. Building it takes less than a second, rather than minutes.

The generated bindings are identical, so a stub build and a real one are interchangeable at compile time Only the runtime behavior is different. This makes it useful for jobs that never execute any code, such as CI lint runs. By design, the test suite will fail if the stub implementation is enabled.

## Important licensing note

While libmv itself is MIT licensed, the C bindings come directly from Blender's source code, which is licensed as GPLv2 or later. As a consequence, this crate is also licensed as GPLv2 or later, which you must keep in mind when using it.

## Dependencies; dynamic vs. static linking

libmv will be built and linked to statically. Building this crate requires [libpng](http://www.libpng.org/pub/png/libpng.html) to be available. This should usually not pose a problem. All other dependencies are bundled with the code.

Building this crate has been tested on Linux and Windows.
