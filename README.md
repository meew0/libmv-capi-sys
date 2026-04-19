# `libmv-capi-sys`

![Crates.io Version](https://img.shields.io/crates/v/libmv-capi-sys)

Unsafe Rust FFI bindings around [libmv](https://projects.blender.org/blender/libmv), the motion tracking library used by [Blender](https://www.blender.org/). Makes use of [Blender's own C bindings](https://projects.blender.org/blender/blender/src/branch/main/intern/libmv/intern) combined with the [standalone distribution of libmv](https://projects.blender.org/blender/libmv).

## Usage

See [src/lib.rs](https://github.com/meew0/libmv-capi-sys/blob/master/src/lib.rs) for a very basic usage example.

Unfortunately, there is almost no documentation available on how to use libmv. In cases where the function names and signatures in the C header files aren't self-explanatory, your best bet is probably checking Blender's source code for how the functions are used.

## Important licensing note

While libmv itself is MIT licensed, the C bindings come directly from Blender's source code, which is licensed as GPLv2 or later. As a consequence, this crate is also licensed as GPLv2 or later, which you must keep in mind when using it.

## Dependencies; dynamic vs. static linking

libmv will be built and linked to statically. Building this crate requires [libpng](http://www.libpng.org/pub/png/libpng.html) to be available. This should usually not pose a problem. All other dependencies are bundled with the code.

Building this crate has been tested on Linux and Windows.
