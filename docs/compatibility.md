# Compatibility matrix

| Component | Supported baseline | Evidence |
| --- | --- | --- |
| Binding | 0.1.0 (unreleased) | Local build and full validation |
| Core | Published TextForge 0.2.0, revision `c2ed4b8f4c96c9e3ddc53576a3cdbf6ccd38bc9e`, plus the reviewed corrections tracking `701c5b2` ([core.md](core.md)) | Hash manifest, patch and independent public-Rust oracles |
| C language | C11 headers and tests (`-std=c11 -Wall -Wextra -Wpedantic -Werror`); `extern "C"` guards for C++ | Header examples and suites compiled in every run |
| Rust toolchain | Stable, edition 2024; dependencies locked by `Cargo.lock` | Local 1.97.0; no lower MSRV claim |
| Linux x86_64, glibc | **Supported and tested**: shared library with SONAME `libtextforge_c.so.0.1`, static library, pkg-config | Local runs (GCC, glibc, valgrind 3.x, ASan/UBSan) and the CI workflow |
| CMake | 3.21 or newer; Ninja and Makefile generators tested; multi-config generators handled | Local builds, Conan and vcpkg checks |
| Conan | Conan 2 (recipe requires ≥ 2.4); Linux x86_64, static and shared, Debug and Release | Local `conan create` with Conan 2.32 |
| vcpkg | Triplets `x64-linux` and `x64-linux-dynamic`; overlay port and git registry | Local install with vcpkg 2026-10-29 |
| Cargo features of the core | `bytecode`, `watchers`, `buffered` always enabled; `tf_features()` reports them | Capability test |
| `test_access` | Test oracles only (`test-reference` feature) | Production library excludes it |

**Unverified source-build possibilities** (not supported, not tested): other
Linux architectures, musl, macOS and Windows. The Rust code has no
platform-specific logic beyond lossless path handling, and the CMake build has
macOS and Windows (MSVC, MinGW) code paths exercised by the experimental CI job
(`cmake-experimental`, allowed to fail), but the test runner (`mkdtemp`,
`valgrind`, `readelf`) and the recipes' supported-platform lists are Linux-only
until that job passes consistently. On Windows, `tf_file_path_from_windows_wide`/`to_windows_wide` are the
native representation and the native-byte functions fail with
`TF_STATUS_CONVERSION`; on Unix it is the reverse.

**Toolchain notes.** The static library needs the system libraries listed in
`Libs.private` of `textforge-c.pc` (`-lm -lrt -lpthread -lgcc_s -lutil -ldl`),
which match `cargo rustc -- --print native-static-libs` for this target. The
CMake package and the Conan recipe declare the same list; the macOS and Windows
lists are unverified.

The CI workflow (`.github/workflows/ci.yml`) runs `scripts/check.py` and
`scripts/check_packaging.py` on Ubuntu 24.04 x86_64. Its remote status is not claimed until it has actually run.
Review this matrix whenever the core snapshot or the binding changes.
