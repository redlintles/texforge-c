# Bundled core provenance

Base: crates.io `textforge` **0.2.0**, upstream revision
`c2ed4b8f4c96c9e3ddc53576a3cdbf6ccd38bc9e` (`vendor/CORE_REVISION`), as
recorded by the published `.cargo_vcs_info.json`. The library uses
`vendor/textforge` directly; it never picks up the neighbouring TextForge
checkout.

The snapshot is byte-for-byte the one reviewed for the Node.js binding: the
published library sources, license, README, original manifest and VCS
metadata, with a library-only effective manifest. The reviewable
[core-0.2.0.patch](core-0.2.0.patch) records the corrections applied on top
of the published crate (scoped replacement-map API, debug block binding,
numeric assignment normalization in the portable layer, persistence and path
fixes, `.tft`/`.tfb` aliases, and the `raw`/`rfw`/`null` opcode
implementations); they track core commit `701c5b2`. The binding itself never
changes core behavior; it only calls `api::portable`.

[core-provenance.json](core-provenance.json) hashes every bundled file.
`python3 scripts/check_core.py` verifies the snapshot, the recorded revision
and the exact patch against the published crate. It downloads nothing; run
`cargo fetch --manifest-path scripts/upstream/Cargo.toml` once to obtain the
independent upstream reference.

To update the core: obtain maintainer approval, replace the snapshot, review
and regenerate the patch/provenance (`scripts/check_core.py --update`), then
regenerate the builder functions, both Rust oracles, the header and the parity
mappings, update [compatibility.md](compatibility.md) and run
`python3 scripts/check.py`.
