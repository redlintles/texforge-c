"""Verify the bundled core snapshot and its reviewed patch against published TextForge 0.2.0.

Use --update only after reviewing the listed production corrections and tests.
No network access or changes to the upstream reference are performed here.
"""
from pathlib import Path
import difflib
import hashlib
import json
import os
import sys

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / 'vendor/textforge'
PROVENANCE = ROOT / 'docs/core-provenance.json'
PATCH = ROOT / 'docs/core-0.2.0.patch'
PATCHED_FILES = {
    'src/api/portable/instruction.rs',
    'src/api/mod.rs',
    'src/api/portable/mod.rs',
    'src/api/replace_map_builder.rs',
    'src/api/processor.rs',
    'src/api/processor/buffered.rs',
    'src/context/execution_context.rs',
    'src/utils/test_helpers.rs',
    'src/api/portable/buffered.rs',
    'src/bytecode/writer.rs',
    'src/parser/resolve_var.rs',
    'src/parser/params.rs',
    'src/text/writer.rs',
    'src/utils/validations.rs',
    'src/watchers/mod.rs',
    'src/tokens/transforms/raw/mod.rs',
    'src/tokens/transforms/rfw/mod.rs',
    'src/tokens/instructions/null/mod.rs',
}
manifest = json.loads(PROVENANCE.read_text())
actual = {
    str(path.relative_to(CORE)): hashlib.sha256(path.read_bytes()).hexdigest()
    for path in sorted(CORE.rglob('*'))
    if path.is_file() and 'target' not in path.parts and path.name != 'Cargo.lock'
}
cargo = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
upstream = next((cargo / 'registry/src').glob('*/textforge-0.2.0'))
assert json.loads((upstream / '.cargo_vcs_info.json').read_text())['git']['sha1'] == manifest['revision']
patch = []
changed = set()
for relative in actual:
    if relative == 'Cargo.toml':
        continue  # Documented library-only snapshot manifest.
    old = (upstream / relative).read_bytes() if (upstream / relative).exists() else b''
    new = (CORE / relative).read_bytes()
    if old != new:
        assert relative in PATCHED_FILES, f'Unexpected core modification: {relative}'
        changed.add(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(True), new.decode().splitlines(True),
            fromfile='a/' + relative, tofile='b/' + relative,
        ))
assert changed == PATCHED_FILES, 'The documented set of core corrections changed'
if '--update' in sys.argv:
    manifest['files'] = actual
    PROVENANCE.write_text(json.dumps(manifest, indent=2) + '\n')
    PATCH.write_text(''.join(patch))
else:
    assert actual == manifest['files'], 'Bundled core content changed; review provenance'
    assert ''.join(patch) == PATCH.read_text(), 'Core correction patch changed'
print(f'Verified {len(actual)} bundled files and {len(changed)} corrected files against upstream')
