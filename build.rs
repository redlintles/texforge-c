//! Build script: gives the Linux shared library a versioned SONAME.
//!
//! C consumers link against `libtextforge_c.so.<major>` so an incompatible ABI
//! release can coexist with older installations. While the binding is 0.x the
//! ABI major is `0.<minor>`, following the usual pre-1.0 convention.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR").unwrap_or_default();
    let minor = std::env::var("CARGO_PKG_VERSION_MINOR").unwrap_or_default();
    if target_os == "linux" {
        let abi = if major == "0" {
            format!("0.{minor}")
        } else {
            major
        };
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libtextforge_c.so.{abi}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
