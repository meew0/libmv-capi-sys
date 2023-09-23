# `libmv-capi-sys`

Unsafe Rust FFI bindings around [libmv](https://projects.blender.org/blender/libmv), the motion tracking library used by [Blender](https://www.blender.org/). Makes use of [Blender's own C bindings](https://projects.blender.org/blender/blender/src/branch/main/intern/libmv/intern) combined with the [standalone distribution of libmv](https://projects.blender.org/blender/libmv).

## Important licensing note

While libmv itself is MIT licensed, the C bindings come directly from Blender's source code, which is licensed as GPLv2 or later. As a consequence, this crate is also licensed as GPLv2 or later, which you must keep in mind when using it.

## Dependencies; dynamic vs. static linking

Regardless of the way libmv is built, building this crate requires [libpng](http://www.libpng.org/pub/png/libpng.html) to be available. This should usually not pose a problem.

By default, libmv will be built and linked to statically. This is a bit inconvenient because it also requires a few additional libraries ([GOMP](https://gcc.gnu.org/projects/gomp/), [SuiteSparse](https://people.engr.tamu.edu/davis/suitesparse.html), and a BLAS provider like [OpenBLAS](https://www.openblas.net/)) to be available at compile time, even if the runtime code does not necessarily use it.

The alternative is to build libmv as a dynamic library and link to it dynamically. This can be achieved by selecting the `"dynamic"` Cargo feature. However, libmv is not usually packaged by Linux distributions (it is not even in the AUR), and (or maybe, because) compiling it does not result in one convenient `libmv.so` package, but in 13 individual files with names like `libbase.so` or `libimage.so` that make them awkward to install system-wide. So depending on your use case, this might be even more inconvenient.
