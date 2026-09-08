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

    def test_v13_evidence_keeps_bounded_verification_distinct_from_visual_acceptance(self):
        evidence = HANDOFF / 'yellow-crane/evidence'
        def read(suffix):
            return json.loads((evidence / f'exterior-whitebox-13-{suffix}.json').read_text())
        geometry, disk, final, visual = (read(s) for s in
                                       ('readback', 'disk-readback', 'final-verification', 'visual-verdict'))
        self.assertEqual(geometry['v13_file_sha256'], disk['source_file_sha256'])
        self.assertEqual(final['v13_source_sha256'], disk['source_file_sha256'])
        self.assertTrue(disk['scene_geometry_matches_live'])
        self.assertEqual(disk['meshes'], 34)
        self.assertEqual(set(geometry['changed_meshes']), {'Fifth_lower_canopy', 'L5_support_PROXY'})
        self.assertEqual(len(geometry['unchanged_meshes']), 32)
        self.assertEqual(len(geometry['endpoint_checks']), 40)
        tips = [p for p in geometry['endpoint_checks'] if p['is_tip']]
        self.assertEqual(len(tips), 12)
        for point in geometry['endpoint_checks']:
            target = 38.8 if point['is_tip'] else 37.02
            self.assertLessEqual(abs(point['actual_xyz_m'][2]-target), 3e-6)
        self.assertEqual(len(geometry['clearance_checks']), 20)
        for support in geometry['clearance_checks']:
            self.assertGreaterEqual(support['vertical_clearance_m'], .02-3e-6)
            self.assertGreater(support['support_height_m'], .04)
        self.assertTrue(read('post-verification')['duplicate_version_rejected'])
        self.assertFalse(final['export_to_game'])
        self.assertEqual(visual['verdict'], 'revise')
        self.assertLess(visual['score'], 90)

    def test_v14_evidence_keeps_endpoint_fix_and_unchecked_posts_bounded(self):
        evidence = HANDOFF / 'yellow-crane/evidence'
        def read(suffix):
            return json.loads((evidence / f'exterior-whitebox-14-{suffix}.json').read_text())
        geometry, final, post, visual = (read(s) for s in
                                      ('readback', 'final-verification', 'post-verification', 'visual-verdict'))
        self.assertEqual(set(geometry['changed_meshes']), {'Ground_canopy', 'Third_canopy'})
        self.assertEqual(len(geometry['unchanged_meshes']), 32)
        self.assertEqual(len(geometry['endpoint_checks']), 120)
        for label, target in final['endpoint_constraints_m'].items():
            points = [p for p in geometry['endpoint_checks'] if p['canopy'] == label]
            self.assertEqual(len(points), 40)
            self.assertEqual(sum(p['is_tip'] for p in points), 12)
            for point in points:
                self.assertLessEqual(abs(point['actual_z_m'] - target['tip' if point['is_tip'] else 'low']), 3e-6)
        self.assertEqual(final['v14_source_sha256'], geometry['v14_file_sha256'])
        self.assertTrue(post['live_geometry_matches_disk'])
        self.assertTrue(post['old_scene_digests_preserved'])
        self.assertTrue(post['source_files_unchanged'])
        self.assertTrue(geometry['old_evaluated_disk_scene_digests_preserved'])
        self.assertTrue(geometry['duplicate_version_rejected'])
        self.assertEqual(len(geometry['clearance_checks']), 20)
        for support in geometry['clearance_checks']:
            self.assertGreaterEqual(support['vertical_clearance_m'], .02-3e-6)
        checked = [p for p in geometry['post_checks'] if 'not_checked' not in p]
        self.assertEqual(len(checked), 68)
        self.assertEqual(len(geometry['post_checks']) - len(checked), 48)
        self.assertEqual(final['lower_post_squares_not_checked'], 48)
        existing_negative = [p for p in checked if p['v13_min_vertical_gap_m'] < -3e-6]
        self.assertEqual(len(existing_negative), 16)
        self.assertEqual(final['lower_post_squares_existing_negative'], 16)
        for point in checked:
            if point['v13_min_vertical_gap_m'] >= -3e-6:
                self.assertGreaterEqual(point['v14_min_vertical_gap_m'], -3e-6)
        worsening = max(p['v13_min_vertical_gap_m'] - p['v14_min_vertical_gap_m']
                        for p in existing_negative)
        self.assertAlmostEqual(final['maximum_existing_negative_gap_worsening_m'], worsening)
        self.assertGreater(worsening, 0)
        self.assertEqual(visual['verdict'], 'revise')
        self.assertLess(visual['score'], 90)
        self.assertFalse(final['export_to_game'])
        self.assertEqual(final['portable_checkpoint'], '08')

    def test_post_preflight_is_not_promoted_to_runtime_acceptance(self):
        report = json.loads((HANDOFF / 'yellow-crane/evidence/post-contact-v14-source-preflight.json').read_text())
        self.assertFalse(report['actual_blender_readback'])
        self.assertFalse(report['solid_intersection_verified'])
        self.assertFalse(report['geometry_modified'])
        self.assertFalse(report['export_to_game'])
        self.assertEqual(report['summary']['posts'], 116)
        self.assertEqual(len(report['rows']), 116)
        for status,count in [('full',68),('partial',8),('none',40)]:
            self.assertEqual(sum(r['v14']['coverage'] == status for r in report['rows']), count)
        negative = [r for r in report['rows'] if r['v14']['coverage'] == 'full'
                    and r['v14']['vertical_gap_m'] < -3e-6]
        self.assertEqual(len(negative), 16)
        self.assertTrue(all(r['floor'] == 'L3' for r in negative))
        self.assertTrue(all(r['footprint_sides'] == 16 for r in report['rows']))
        self.assertIn('NOT actual Blender', report['limits'][0])

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
