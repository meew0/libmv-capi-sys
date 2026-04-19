# `libmv-capi-sys`

Unsafe Rust FFI bindings around [libmv](https://projects.blender.org/blender/libmv), the motion tracking library used by [Blender](https://www.blender.org/). Makes use of [Blender's own C bindings](https://projects.blender.org/blender/blender/src/branch/main/intern/libmv/intern) combined with the [standalone distribution of libmv](https://projects.blender.org/blender/libmv).

## Important licensing note

While libmv itself is MIT licensed, the C bindings come directly from Blender's source code, which is licensed as GPLv2 or later. As a consequence, this crate is also licensed as GPLv2 or later, which you must keep in mind when using it.

## Dependencies; dynamic vs. static linking

 libmv will be built and linked to statically. Building this crate requires [libpng](http://www.libpng.org/pub/png/libpng.html) to be available. This should usually not pose a problem. All other dependencies are bundled with the code.
