import os
import shutil

from conan import ConanFile
from conan.errors import ConanInvalidConfiguration
from conan.tools.cmake import CMake, CMakeToolchain, cmake_layout
from conan.tools.files import copy, get, rmdir

required_conan_version = ">=2.4"


class TextforgeCConan(ConanFile):
    name = "textforge-c"
    description = "C bindings for the TextForge text-processing DSL"
    license = "GPL-3.0-only"
    url = "https://github.com/conan-io/conan-center-index"
    homepage = "https://github.com/redlintles/textforge-c"
    topics = ("text-processing", "dsl", "pipeline", "rust", "ffi")
    package_type = "library"
    settings = "os", "arch", "compiler", "build_type"
    options = {
        "shared": [True, False],
        "fPIC": [True, False],
    }
    default_options = {
        "shared": False,
        "fPIC": True,
    }
    languages = "C"

    # Platforms whose builds are verified by the upstream CI.
    _supported = {("Linux", "x86_64")}
    _rust_arch = {"x86_64": "x86_64", "armv8": "aarch64", "x86": "i686"}

    def config_options(self):
        if self.settings.os == "Windows":
            del self.options.fPIC

    def configure(self):
        if self.options.shared:
            self.options.rm_safe("fPIC")

    def layout(self):
        cmake_layout(self, src_folder="src")

    def validate(self):
        if (str(self.settings.os), str(self.settings.arch)) not in self._supported:
            raise ConanInvalidConfiguration(
                f"{self.ref} is only validated on {sorted(self._supported)}")

    def validate_build(self):
        if self._cargo is None:
            raise ConanInvalidConfiguration(
                f"{self.ref} is a Rust library with a C ABI: building it requires cargo from a "
                "stable Rust toolchain (https://rustup.rs) on the build machine")

    @property
    def _cargo(self):
        candidates = [shutil.which("cargo")]
        for home in (os.environ.get("CARGO_HOME"), os.path.join(os.path.expanduser("~"), ".cargo")):
            if home:
                candidates.append(os.path.join(home, "bin", "cargo.exe" if os.name == "nt" else "cargo"))
        return next((path for path in candidates if path and os.path.isfile(path)), None)

    @property
    def _rust_target(self):
        arch = self._rust_arch[str(self.settings.arch)]
        os_name = str(self.settings.os)
        if os_name == "Windows":
            return f"{arch}-pc-windows-{'gnu' if self.settings.compiler == 'gcc' else 'msvc'}"
        if os_name == "Macos":
            return f"{arch}-apple-darwin"
        return f"{arch}-unknown-linux-gnu"

    def source(self):
        get(self, **self.conan_data["sources"][self.version], strip_root=True)

    def generate(self):
        tc = CMakeToolchain(self)
        tc.cache_variables["CARGO_EXECUTABLE"] = self._cargo.replace("\\", "/")
        tc.cache_variables["TEXTFORGE_C_RUST_TARGET"] = self._rust_target
        # The release archive vendors every crate: no network access during build().
        tc.cache_variables["TEXTFORGE_C_OFFLINE"] = True
        tc.generate()

    def build(self):
        cmake = CMake(self)
        cmake.configure()
        cmake.build()

    def package(self):
        copy(self, "LICENSE", self.source_folder, os.path.join(self.package_folder, "licenses"))
        cmake = CMake(self)
        cmake.install()
        rmdir(self, os.path.join(self.package_folder, "lib", "cmake"))
        rmdir(self, os.path.join(self.package_folder, "lib", "pkgconfig"))

    def package_info(self):
        self.cpp_info.set_property("cmake_file_name", "textforge-c")
        self.cpp_info.set_property("cmake_target_name", "textforge-c::textforge-c")
        self.cpp_info.set_property("pkg_config_name", "textforge-c")
        self.cpp_info.libs = ["textforge_c"]
        if not self.options.shared:
            # System libraries required by the Rust standard library.
            if self.settings.os in ["Linux", "FreeBSD"]:
                self.cpp_info.system_libs = ["m", "rt", "pthread", "gcc_s", "util", "dl"]
            elif self.settings.os == "Macos":
                self.cpp_info.system_libs = ["iconv", "System", "c", "m"]
            elif self.settings.os == "Windows":
                self.cpp_info.system_libs = ["advapi32", "bcrypt", "kernel32", "ntdll", "userenv", "ws2_32"]
