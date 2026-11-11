//! The committed C header must match what cbindgen generates from the crate.
//!
//! Regenerate after changing the exported API:
//! `TEXTFORGE_UPDATE_HEADER=1 cargo test --test header`.

use std::path::Path;

#[test]
fn header_is_current() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config =
        cbindgen::Config::from_file(root.join("cbindgen.toml")).expect("valid cbindgen.toml");
    let mut generated = Vec::new();
    cbindgen::Builder::new()
        .with_crate(root)
        .with_config(config)
        .generate()
        .expect("cbindgen generates the header")
        .write(&mut generated);
    let generated = String::from_utf8(generated).expect("UTF-8 header");
    let path = root.join("include/textforge.h");
    if std::env::var_os("TEXTFORGE_UPDATE_HEADER").is_some() {
        std::fs::create_dir_all(root.join("include")).expect("include directory");
        std::fs::write(&path, &generated).expect("write header");
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "include/textforge.h is stale; run TEXTFORGE_UPDATE_HEADER=1 cargo test --test header"
    );
}
