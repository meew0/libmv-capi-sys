### `libmv-capi-sys/build.rs` — Windows cmake build

The original `build.rs` calls `make` to build libmv, which does not exist on Windows. The following
changes were made:

1. **`build_libmv_windows()` function added** — on Windows, configures and builds libmv using
   cmake instead of make. Key cmake flags:
   - `-DBUILD_SHARED_LIBS=OFF` — static build
   - `-DEIGEN_INCLUDE_DIR=<path>` — libmv's cmake does not find its bundled Eigen automatically
   - `-DSUITESPARSE=OFF -DCXSPARSE=OFF -DLAPACK=OFF -DOPENMP=OFF` — disable Linux-only Ceres
     optional dependencies
   - `-DMINIGLOG=ON` — use glog's miniglog mode (avoids needing a system glog install)
   - `-DBUILD_TESTING=OFF -DCMAKE_SKIP_INSTALL_RULES=TRUE` — skip unused build targets

2. **Output library path** — CMakeLists.txt explicitly sets `CMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE`
   to `bin-static/lib` (not the default `bin-static/lib/Release` that MSVC would use otherwise),
   so the path check was updated accordingly.

3. **Removed Unix-only link libraries on Windows** — `gomp`, `cholmod`, `cxsparse`, `spqr`, `blas`
   are not needed because SuiteSparse and OpenMP are disabled on Windows.

4. **Added include paths for bundled libpng and zlib** — the CAPI wrapper source files include
   `png.h`, which on Linux is satisfied by the system libpng headers but on Windows requires the
   bundled copies from `libmv/src/third_party/png` and `libmv/src/third_party/zlib`.

5. **Link bundled libpng and zlib statically on Windows** — on Linux, `libpng` is a system dylib;
   on Windows, cmake builds and bundles both `png.lib` and `zlib.lib`.

### `libmv-capi-sys/libmv/src/CMakeLists.txt` — MSVC compiler flags

Added inside the existing `IF(MSVC)` block:

```cmake
SET(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /D__STDC_LIMIT_MACROS")
```

**Why:** VS 2022 17.11 introduced a regression where `<xlocnum>` references `PTRDIFF_MAX` without
including the header that defines it. Normally, force-including `<cstdint>` would fix this, but
libmv vendors `third_party/msinttypes/stdint.h` (a C++98-era portability library) which guards all
limit macros including `PTRDIFF_MAX` behind `#if !defined(__cplusplus) || defined(__STDC_LIMIT_MACROS)`.
Since msinttypes appears earlier in the include search path than the MSVC system headers, it wins —
and in C++ mode without `__STDC_LIMIT_MACROS`, `PTRDIFF_MAX` is never defined.

Defining `__STDC_LIMIT_MACROS` unlocks the limit macros in msinttypes' `stdint.h` and resolves the
MSVC STL regression without modifying any vendored headers.

`CMAKE_CXX_FLAGS` is used (rather than `ADD_COMPILE_OPTIONS`) so that the flag only applies to C++
compilation units. The bundled C-only libraries (jpeg, zlib, ldl) must not receive this flag
because force-including a C++ header into a C file causes MSVC to abort with `STL1003: Unexpected
compiler, expected C++ compiler`.

### `libmv-capi-sys/libmv/src/third_party/glog/src/glog/logging.h` — `_WIN32` guard

The header used `#ifdef WIN32` to decide whether to include the Windows-specific variant. MSVC
defines `_WIN32`, not `WIN32`. Changed to:

```c
#if defined(WIN32) || defined(_WIN32)
#  include "windows/glog/logging.h"
#else
   // ... Unix header content
#endif
```

### `libmv-capi-sys/libmv/src/third_party/daisy/include/kutility/general.h` — `_WIN32` guards

Same issue as glog: `#ifndef WIN32` guards around `<sys/mman.h>` and the `strncasecmp` definition
were not triggered by MSVC. Changed both guards to check `_WIN32` as well:

```c
#if !defined(WIN32) && !defined(_WIN32)
#include <sys/mman.h>
// ...
#endif

#if defined(WIN32) || defined(_WIN32)
#define strncasecmp _strnicmp
// ...
#endif
```

### `libmv-capi-sys/libmv/src/third_party/gflags/util.h` — missing `<windows.h>`

The MSVC-specific `MakeTmpdir` function calls `GetTempPathA`, which requires `<windows.h>`. The
original file had `#include <direct.h>` but not `<windows.h>`. Added:

```c
#elif defined(_MSC_VER)
#include <direct.h>
#include <windows.h>    // <-- added
inline void MakeTmpdir ...
```

### `libmv-capi-sys/libmv/src/third_party/gflags/gflags.cc` — `strcasecmp` not available in MSVC

`strcasecmp` is a POSIX function; MSVC provides `_stricmp` instead. Added near the top of the file:

```c
#ifdef _MSC_VER
#  define strcasecmp _stricmp
#endif
```
