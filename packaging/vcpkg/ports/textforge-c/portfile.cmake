# textforge-c is a Rust crate exposing a C ABI. The release archive vendors all
# Rust dependencies, so cargo builds offline; only the cargo tool itself must be
# installed on the host (vcpkg does not provide a Rust toolchain).
vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/redlintles/textforge-c/releases/download/v${VERSION}/textforge-c-${VERSION}.tar.gz"
    FILENAME "textforge-c-${VERSION}.tar.gz"
    SHA512 dec8a55b0c429cd2af75f024ba2743af870d6596cae26053e1c5707d44501d5262aa08ffcf6c491eb9fbdd61d429ae12ad245ba1fc9ad1ba62110496ec246591
)
vcpkg_extract_source_archive(SOURCE_PATH ARCHIVE "${ARCHIVE}")

find_program(CARGO cargo HINTS "$ENV{CARGO_HOME}/bin" "$ENV{HOME}/.cargo/bin" "$ENV{USERPROFILE}/.cargo/bin")
if(NOT CARGO)
    message(FATAL_ERROR "${PORT} requires cargo from a stable Rust toolchain (https://rustup.rs). "
        "If cargo is installed, add its directory to PATH or keep CARGO_HOME/RUSTUP_HOME "
        "through VCPKG_KEEP_ENV_VARS in your triplet.")
endif()

# Rust target triple for the vcpkg triplet.
set(rust_arch_x64 x86_64)
set(rust_arch_arm64 aarch64)
set(rust_arch_x86 i686)
set(rust_arch "${rust_arch_${VCPKG_TARGET_ARCHITECTURE}}")
if(NOT rust_arch)
    message(FATAL_ERROR "Unsupported architecture for ${PORT}: ${VCPKG_TARGET_ARCHITECTURE}")
endif()
if(VCPKG_TARGET_IS_WINDOWS AND VCPKG_TARGET_IS_MINGW)
    set(rust_target "${rust_arch}-pc-windows-gnu")
elseif(VCPKG_TARGET_IS_WINDOWS)
    set(rust_target "${rust_arch}-pc-windows-msvc")
elseif(VCPKG_TARGET_IS_OSX)
    set(rust_target "${rust_arch}-apple-darwin")
elseif(VCPKG_TARGET_IS_LINUX)
    set(rust_target "${rust_arch}-unknown-linux-gnu")
else()
    message(FATAL_ERROR "Unsupported platform for ${PORT}")
endif()

set(options "")
if(VCPKG_TARGET_IS_WINDOWS AND VCPKG_CRT_LINKAGE STREQUAL "static")
    list(APPEND options "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

vcpkg_cmake_configure(
    SOURCE_PATH "${SOURCE_PATH}"
    OPTIONS
        "-DCARGO_EXECUTABLE=${CARGO}"
        "-DTEXTFORGE_C_RUST_TARGET=${rust_target}"
        -DTEXTFORGE_C_OFFLINE=ON
        ${options}
)
vcpkg_cmake_install()
vcpkg_cmake_config_fixup(PACKAGE_NAME textforge-c CONFIG_PATH lib/cmake/textforge-c)
vcpkg_fixup_pkgconfig()

file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")
file(INSTALL "${CMAKE_CURRENT_LIST_DIR}/usage" DESTINATION "${CURRENT_PACKAGES_DIR}/share/${PORT}")
vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/LICENSE")
