# Packaging: Conan and vcpkg

The C library is a Rust crate, so every package build needs `cargo`. Both
package recipes build from the **offline source release**, which vendors all
Rust dependencies: no network access is needed during the build, and only the
cargo tool (stable Rust) must be present on the build machine.

## Where the packages can be published

| Channel | Status | Reason |
| --- | --- | --- |
| ConanCenter (conan-center-index) | **Blocked** | ConanCenter builds from source and its infrastructure has no Rust toolchain; on 2026-11-05 the maintainers declined a Rust library with a C ABI for exactly this reason, and also declined repackaging prebuilt binaries ([conan-center-index#31112](https://github.com/conan-io/conan-center-index/pull/31112); related questions: #10081, #8938). |
| vcpkg curated registry (microsoft/vcpkg) | **Not supported upstream** | vcpkg does not provide a Rust toolchain, and its maintainers have discouraged Rust-built ports ([microsoft/vcpkg#15441](https://github.com/microsoft/vcpkg/issues/15441)). A submission would depend on cargo being available on their CI. |
| Project Conan remote | Ready | The recipe works unchanged from any Conan 2 remote. |
| Project vcpkg git registry | Ready | `packaging/vcpkg` is a complete registry (ports + versions). |

The recipes follow the upstream layouts (`recipes/<name>/all`, `config.yml`,
`conandata.yml`, `test_package`; `ports/<name>`, `versions/`), so they can be
submitted unchanged if either policy changes. Re-check both policies before
submitting.

## Layout

```text
packaging/
├── source.json                      release URL template, homepage, license
├── conan/recipes/textforge-c/       ConanCenter layout
│   ├── config.yml
│   └── all/{conanfile.py, conandata.yml, test_package/}
└── vcpkg/                           vcpkg git registry
    ├── ports/textforge-c/{vcpkg.json, portfile.cmake, usage}
    └── versions/{baseline.json, t-/textforge-c.json}
```

Both recipes call the CMake build (`CMakeLists.txt`) with one linkage per
package, the target triple mapped to a Rust target and `TEXTFORGE_C_OFFLINE=ON`.
Consumers get `find_package(textforge-c CONFIG REQUIRED)` with the target
`textforge-c::textforge-c`, and the pkg-config module `textforge-c`.

| Setting | Conan | vcpkg |
| --- | --- | --- |
| Linkage | `shared` option (default static) | triplet `VCPKG_LIBRARY_LINKAGE` |
| Build type | `build_type` (Debug uses cargo's dev profile) | debug and release builds |
| MSVC runtime | `compiler.runtime` → `CMAKE_MSVC_RUNTIME_LIBRARY`; a static CRT adds `+crt-static` | `VCPKG_CRT_LINKAGE` |
| Rust target | from `os`/`arch` (`x86_64`, `armv8`, `x86`) | from the triplet |
| Static system libraries | `cpp_info.system_libs` | CMake package `INTERFACE_LINK_LIBRARIES`, pkg-config `Libs.private` |
| Platforms accepted | `validate()`: Linux x86_64 | `"supports": "linux & x64"` |

Platforms are limited to what CI verifies. The CMake build also has macOS and
Windows (MSVC and MinGW) code paths, exercised by the experimental CI job;
widen `_supported` in the recipe and `supports` in the port once those jobs pass
consistently.

## Release process

1. Bump the version in `Cargo.toml` (maintainer approval), update the changelog
   and commit.
2. `python3 scripts/make_source_release.py --update-recipes` on that clean commit.
   It writes `dist/textforge-c-<version>.tar.gz` (+ `.sha256`, `.sha512`) and
   updates the URL/hashes in both recipes and the vcpkg versions database.
3. `python3 scripts/check_packaging.py` (needs `conan` and `VCPKG_ROOT`), then
   commit `packaging/`. The archive excludes `packaging/` and `.github/` and is
   stamped with the last commit that changed archived files, so it is
   byte-identical when rebuilt from this commit.
4. Tag `v<version>` and upload the archive as a release asset at the URL in
   `packaging/source.json` (assumed repository: `redlintles/textforge-c`; change
   the template there if the repository differs, then rerun step 2).
5. Publish:
   * **Conan:** `conan create packaging/conan/recipes/textforge-c/all --version <version>`
     for each configuration, then `conan upload "textforge-c/<version>" -r <remote>`.
   * **vcpkg:** push the contents of `packaging/vcpkg` to the registry repository
     (for example with `git subtree split --prefix packaging/vcpkg`). The
     `git-tree` recorded in `versions/` is the tree id of the port directory and
     is the same in both repositories.

Steps 4 and 5 publish artifacts and require maintainer approval.

## Consuming the project channels

vcpkg (`vcpkg-configuration.json` next to the project's `vcpkg.json`):

```json
{
  "default-registry": {
    "kind": "git",
    "repository": "https://github.com/microsoft/vcpkg",
    "baseline": "<vcpkg commit>"
  },
  "registries": [
    {
      "kind": "git",
      "repository": "<registry repository URL>",
      "baseline": "<registry commit>",
      "packages": ["textforge-c"]
    }
  ]
}
```

Conan: `conan remote add textforge <remote URL>`, then require
`textforge-c/0.1.0` in `conanfile.txt` or `conanfile.py`.

The build machine needs `cargo`. With vcpkg on Windows, environment variables
are filtered: keep `CARGO_HOME;RUSTUP_HOME` with `VCPKG_KEEP_ENV_VARS` in the
triplet if cargo is not on the default `PATH`. Cross builds also need the Rust
standard library for the target (`rustup target add <triple>`).

## Verification

`scripts/check_packaging.py` (CI job `packaging`) builds the release archive,
builds it with CMake offline (empty `CARGO_HOME`, unreachable proxy) in both
linkages and checks Debug/Release consumers; runs `conan create` static and
shared with `test_package`; installs the vcpkg overlay port for `x64-linux` and
`x64-linux-dynamic` (failing on post-build warnings) with Debug/Release
consumers; and consumes the port through a git registry built from
`packaging/vcpkg` in manifest mode. Recipe files are never modified: test hashes
are substituted in temporary copies.
