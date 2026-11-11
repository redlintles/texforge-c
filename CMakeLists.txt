# CMake build for the TextForge C library.
#
# Builds the Rust crate with cargo and installs one linkage per build, as
# package managers expect:
#   BUILD_SHARED_LIBS=ON   shared library (Linux: versioned SONAME + symlinks)
#   BUILD_SHARED_LIBS=OFF  static library (default) with its system libraries
# plus include/textforge.h, a CMake package (find_package(textforge-c),
# target textforge-c::textforge-c) and the pkg-config module textforge-c.
#
# Requires cargo (stable Rust). Options:
#   TEXTFORGE_C_RUST_TARGET  Rust target triple for cross builds (empty: host)
#   TEXTFORGE_C_OFFLINE      build only from vendored crates (default: ON when
#                            the source tree contains .cargo/config.toml, as
#                            release tarballs do)
#   CARGO_EXECUTABLE         path to cargo
cmake_minimum_required(VERSION 3.21)

file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml" _textforge_version_line REGEX "^version = \"[^\"]+\"" LIMIT_COUNT 1)
string(REGEX REPLACE "^version = \"([^\"]+)\"" "\\1" _textforge_version "${_textforge_version_line}")
project(textforge-c VERSION ${_textforge_version} DESCRIPTION "C bindings for TextForge" LANGUAGES C)

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

option(BUILD_SHARED_LIBS "Build and install the shared library instead of the static one" OFF)
set(TEXTFORGE_C_RUST_TARGET "" CACHE STRING "Rust target triple (empty selects the host)")
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/.cargo/config.toml")
    set(_textforge_offline_default ON)
else()
    set(_textforge_offline_default OFF)
endif()
option(TEXTFORGE_C_OFFLINE "Build only from vendored crates" ${_textforge_offline_default})
find_program(CARGO_EXECUTABLE cargo HINTS "$ENV{CARGO_HOME}/bin" "$ENV{HOME}/.cargo/bin" "$ENV{USERPROFILE}/.cargo/bin")
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo was not found. Install a stable Rust toolchain (https://rustup.rs) "
                        "or set CARGO_EXECUTABLE.")
endif()

# ABI version: 0.<minor> while the binding is 0.x, otherwise the major version.
if(PROJECT_VERSION_MAJOR EQUAL 0)
    set(TEXTFORGE_C_ABI "0.${PROJECT_VERSION_MINOR}")
else()
    set(TEXTFORGE_C_ABI "${PROJECT_VERSION_MAJOR}")
endif()

# Artifact names produced by cargo for the target platform.
if(WIN32)
    if(MSVC)
        set(_shared_built "textforge_c.dll")
        set(_import_built "textforge_c.dll.lib")
        set(_static_built "textforge_c.lib")
        set(_import_installed "textforge_c.lib")
        set(_system_libs advapi32 bcrypt kernel32 ntdll userenv ws2_32 dbghelp)
    else()
        set(_shared_built "textforge_c.dll")
        set(_import_built "libtextforge_c.dll.a")
        set(_static_built "libtextforge_c.a")
        set(_import_installed "libtextforge_c.dll.a")
        set(_system_libs advapi32 bcrypt kernel32 ntdll userenv ws2_32)
    endif()
elseif(APPLE)
    set(_shared_built "libtextforge_c.dylib")
    set(_static_built "libtextforge_c.a")
    set(_system_libs iconv System c m)
else()
    set(_shared_built "libtextforge_c.so")
    set(_static_built "libtextforge_c.a")
    set(_system_libs m rt pthread gcc_s util dl)
endif()

# Cargo profile and output directory per configuration (multi-config aware).
set(_cargo_dir "${CMAKE_CURRENT_BINARY_DIR}/cargo")
set(_profile "$<IF:$<CONFIG:Debug>,dev,release>")
set(_profile_dir "$<IF:$<CONFIG:Debug>,debug,release>")
set(_cargo_args build --lib --locked --profile ${_profile})
if(TEXTFORGE_C_OFFLINE)
    list(APPEND _cargo_args --offline)
endif()
if(TEXTFORGE_C_RUST_TARGET)
    list(APPEND _cargo_args --target ${TEXTFORGE_C_RUST_TARGET})
    set(_artifact_dir "${_cargo_dir}/${TEXTFORGE_C_RUST_TARGET}/${_profile_dir}")
else()
    set(_artifact_dir "${_cargo_dir}/${_profile_dir}")
endif()
set(_rustflags "$ENV{RUSTFLAGS}")
# A static MSVC runtime (/MT) requires Rust to link the CRT statically too.
if(MSVC AND CMAKE_MSVC_RUNTIME_LIBRARY MATCHES "^MultiThreaded(Debug)?$")
    string(APPEND _rustflags " -Ctarget-feature=+crt-static")
endif()

if(BUILD_SHARED_LIBS)
    set(_primary "${_artifact_dir}/${_shared_built}")
else()
    set(_primary "${_artifact_dir}/${_static_built}")
endif()
file(GLOB_RECURSE _rust_sources CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
     "${CMAKE_CURRENT_SOURCE_DIR}/vendor/textforge/src/*.rs")
add_custom_command(
    OUTPUT "${_primary}"
    # cargo schedules its own jobs; an inherited make jobserver only produces warnings.
    COMMAND ${CMAKE_COMMAND} -E env --unset=MAKEFLAGS --unset=MFLAGS
            "CARGO_TARGET_DIR=${_cargo_dir}" "RUSTFLAGS=${_rustflags}"
            "${CARGO_EXECUTABLE}" ${_cargo_args}
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    DEPENDS ${_rust_sources} Cargo.toml Cargo.lock build.rs
    COMMENT "Building textforge_c with cargo"
    USES_TERMINAL
    VERBATIM)
add_custom_target(textforge_c_cargo ALL DEPENDS "${_primary}")

# ---------------------------------------------------------------- install
install(FILES include/textforge.h DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
set(_config_dir "${CMAKE_INSTALL_LIBDIR}/cmake/textforge-c")

if(BUILD_SHARED_LIBS)
    if(WIN32)
        install(FILES "${_artifact_dir}/${_shared_built}" DESTINATION ${CMAKE_INSTALL_BINDIR})
        install(FILES "${_artifact_dir}/${_import_built}" DESTINATION ${CMAKE_INSTALL_LIBDIR}
                RENAME "${_import_installed}")
        set(TEXTFORGE_C_LOCATION "${CMAKE_INSTALL_BINDIR}/${_shared_built}")
        set(TEXTFORGE_C_IMPLIB "${CMAKE_INSTALL_LIBDIR}/${_import_installed}")
    elseif(APPLE)
        install(FILES "${_artifact_dir}/${_shared_built}" DESTINATION ${CMAKE_INSTALL_LIBDIR})
        set(TEXTFORGE_C_LOCATION "${CMAKE_INSTALL_LIBDIR}/${_shared_built}")
    else()
        set(_real "libtextforge_c.so.${PROJECT_VERSION}")
        set(_soname "libtextforge_c.so.${TEXTFORGE_C_ABI}")
        install(FILES "${_artifact_dir}/${_shared_built}" DESTINATION ${CMAKE_INSTALL_LIBDIR}
                RENAME "${_real}" PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ
                GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)
        install(CODE "
            set(_dir \"\$ENV{DESTDIR}\${CMAKE_INSTALL_PREFIX}/${CMAKE_INSTALL_LIBDIR}\")
            file(CREATE_LINK \"${_real}\" \"\${_dir}/${_soname}\" SYMBOLIC)
            file(CREATE_LINK \"${_soname}\" \"\${_dir}/libtextforge_c.so\" SYMBOLIC)")
        set(TEXTFORGE_C_LOCATION "${CMAKE_INSTALL_LIBDIR}/${_real}")
        set(TEXTFORGE_C_SONAME "${_soname}")
    endif()
    set(TEXTFORGE_C_TYPE SHARED)
    set(TEXTFORGE_C_INTERFACE_LIBS "")
else()
    install(FILES "${_artifact_dir}/${_static_built}" DESTINATION ${CMAKE_INSTALL_LIBDIR})
    set(TEXTFORGE_C_LOCATION "${CMAKE_INSTALL_LIBDIR}/${_static_built}")
    set(TEXTFORGE_C_TYPE STATIC)
    set(TEXTFORGE_C_INTERFACE_LIBS "${_system_libs}")
endif()
# pkg-config uses Libs.private only for --static links, so it is always listed.
list(TRANSFORM _system_libs PREPEND "-l" OUTPUT_VARIABLE PRIVATE_LIBS)
list(JOIN PRIVATE_LIBS " " PRIVATE_LIBS)
set(TEXTFORGE_C_INCLUDEDIR "${CMAKE_INSTALL_INCLUDEDIR}")

configure_package_config_file(cmake/textforge-c-config.cmake.in
    "${CMAKE_CURRENT_BINARY_DIR}/textforge-c-config.cmake" INSTALL_DESTINATION "${_config_dir}")
# 0.x releases are compatible only within the same minor version.
if(PROJECT_VERSION_MAJOR EQUAL 0)
    set(_compatibility SameMinorVersion)
else()
    set(_compatibility SameMajorVersion)
endif()
write_basic_package_version_file("${CMAKE_CURRENT_BINARY_DIR}/textforge-c-config-version.cmake"
    COMPATIBILITY ${_compatibility})
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/textforge-c-config.cmake"
              "${CMAKE_CURRENT_BINARY_DIR}/textforge-c-config-version.cmake"
        DESTINATION "${_config_dir}")

# One location file per configuration (Debug and Release can be installed side by side).
if(CMAKE_CONFIGURATION_TYPES)
    set(_configurations ${CMAKE_CONFIGURATION_TYPES})
elseif(CMAKE_BUILD_TYPE)
    set(_configurations ${CMAKE_BUILD_TYPE})
else()
    set(_configurations Release)
endif()
foreach(_configuration IN LISTS _configurations)
    string(TOUPPER "${_configuration}" TEXTFORGE_C_CONFIG_UPPER)
    string(TOLOWER "${_configuration}" _configuration_lower)
    set(_targets_file "${CMAKE_CURRENT_BINARY_DIR}/textforge-c-targets-${_configuration_lower}.cmake")
    configure_file(cmake/textforge-c-targets-config.cmake.in "${_targets_file}" @ONLY)
    install(FILES "${_targets_file}" DESTINATION "${_config_dir}" CONFIGURATIONS ${_configuration})
endforeach()

# The pkg-config file is relocatable: its paths are relative to its own location
# (${pcfiledir}), so `cmake --install --prefix <dir>` works whatever prefix was
# configured. Absolute install directories are written as they are.
if(IS_ABSOLUTE "${CMAKE_INSTALL_LIBDIR}" OR IS_ABSOLUTE "${CMAKE_INSTALL_INCLUDEDIR}")
    set(PREFIX "${CMAKE_INSTALL_PREFIX}")
    set(LIBDIR "${CMAKE_INSTALL_FULL_LIBDIR}")
    set(INCLUDEDIR "${CMAKE_INSTALL_FULL_INCLUDEDIR}")
else()
    file(RELATIVE_PATH _pc_to_prefix "/prefix/${CMAKE_INSTALL_LIBDIR}/pkgconfig" "/prefix")
    set(PREFIX "\${pcfiledir}/${_pc_to_prefix}")
    set(LIBDIR "\${prefix}/${CMAKE_INSTALL_LIBDIR}")
    set(INCLUDEDIR "\${prefix}/${CMAKE_INSTALL_INCLUDEDIR}")
endif()
set(VERSION "${PROJECT_VERSION}")
configure_file(textforge-c.pc.in "${CMAKE_CURRENT_BINARY_DIR}/textforge-c.pc" @ONLY)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/textforge-c.pc" DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig")
