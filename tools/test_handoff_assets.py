"""Offline checks for the small, portable handoff package (no Blender required)."""
import hashlib
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
HANDOFF = ROOT / 'docs/handoff'


class HandoffAssetsTests(unittest.TestCase):
    def test_manifest_files_are_portable_and_match_hashes(self):
        manifest = json.loads((HANDOFF / 'manifest.json').read_text(encoding='utf-8'))
        paths = []
        for entry in manifest['files']:
            path = Path(entry['path'])
            self.assertFalse(path.is_absolute())
            self.assertNotIn('..', path.parts)
            self.assertTrue(path.as_posix().startswith('docs/handoff/'))
            data = (ROOT / path).read_bytes()
            self.assertEqual(len(data), entry['bytes'], str(path))
            self.assertEqual(hashlib.sha256(data).hexdigest(), entry['sha256'], str(path))
            paths.append(path.as_posix())
        self.assertEqual(len(paths), len(set(paths)))
        self.assertIn('docs/handoff/yellow-crane/exterior-whitebox-08-portable.blend', paths)

    def test_snapshot_roundtrip_evidence(self):
        proof = json.loads((HANDOFF / 'yellow-crane/portability-verification.json').read_text(encoding='utf-8'))
        artifact = ROOT / proof['artifact']
        self.assertEqual(hashlib.sha256(artifact.read_bytes()).hexdigest(), proof['artifact_sha256'])
        self.assertEqual(proof['mesh_count'], 33)
        self.assertEqual(proof['triangles'], 106008)
        self.assertEqual(proof['source_geometry_sha256'], proof['roundtrip_geometry_sha256'])
        self.assertEqual(proof['contents']['scenes'], ['HHL_Exterior_Whitebox_08_Portable'])
        for key in ('images', 'texts', 'libraries', 'sounds', 'movieclips', 'fonts'):
            self.assertEqual(proof['contents'][key], [], key)
        self.assertEqual(proof['external_dependencies'], [])
        self.assertEqual(proof['drivers'], [])
        self.assertTrue(proof['source_file_unchanged'])
        self.assertTrue(proof['existing_scene_geometry_preserved'])

    def test_omx_allowlist_excludes_private_and_runtime_files(self):
        ignored = [
            '.omx/state/notepad-before-context-portability.md',
            '.omx/state/yellow-crane-replica/ralph-progress.json',
            '.omx/references/yellow-crane/figure-2-2-1.jpeg',
            '.omx/references/yellow-crane/exterior-whitebox-08.blend',
            '.omx/experiments/build_yellow_crane_unverified.py',
            '.omx/project-memory.json', '.omx/future-runtime/cache.json',
            '.omx/plans/.DS_Store', '.omx/plans/secret.json',
        ]
        included = ['.omx/notepad.md', '.omx/plans/context-portability-20260908.md']
        for name in ignored + included:
            result = subprocess.run(
                ['git', 'check-ignore', '--no-index', '-q', name],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0 if name in ignored else 1,
                             f'{name}: {result.stderr}')

    def test_source_inventory_does_not_bundle_images(self):
        inventory = json.loads((HANDOFF / 'yellow-crane/source-inventory.json').read_text(encoding='utf-8'))
        self.assertFalse(inventory['images_distributed'])
        self.assertEqual(len(inventory['originals']), 10)
        for original in inventory['originals']:
            self.assertEqual(len(original['sha256']), 64)
            self.assertGreater(original['bytes_expected'], 0)
        for path in HANDOFF.rglob('*'):
            self.assertNotIn(path.suffix.lower(), {'.jpg', '.jpeg', '.png', '.blend1'})


if __name__ == '__main__':
    unittest.main()
